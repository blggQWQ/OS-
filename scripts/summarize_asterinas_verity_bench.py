#!/usr/bin/env python3
"""Extract the five-run Asterinas raw/dm-verity fio matrix from a serial log."""

from __future__ import annotations

import argparse
import csv
import json
import re
import statistics
from pathlib import Path


BEGIN_RE = re.compile(r"\[dm-bench\] BEGIN label=(\S+) run=(\d+) device=(\S+)")
STATS_RE = re.compile(r"\[dm-bench\] STATS label=(\S+) run=(\d+) (.*)")
COUNTER_RE = re.compile(
    r"(cache_blocks|verified|bio_blocks|data_rereads|hash_hits|hash_misses|auth_shortcuts)=(\d+)"
)


def latency_us(read: dict, percentile: str) -> float:
    for key, scale in (("clat_ns", 1 / 1000), ("clat_us", 1), ("clat_ms", 1000)):
        values = read.get(key, {}).get("percentile", {})
        if percentile in values:
            return float(values[percentile]) * scale
    raise ValueError(f"fio JSON has no completion-latency percentile {percentile}")


def parse_log(path: Path) -> list[dict]:
    rows: list[dict] = []
    current: dict | None = None
    json_lines: list[str] = []
    collecting_json = False

    for raw_line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        line = raw_line.strip("\r")
        begin = BEGIN_RE.search(line)
        if begin:
            current = {
                "label": begin.group(1),
                "run": int(begin.group(2)),
                "device": begin.group(3),
            }
            json_lines = []
            collecting_json = False
            continue

        if current is not None and (collecting_json or line.lstrip().startswith("{")):
            collecting_json = True
            json_lines.append(line)

        stats = STATS_RE.search(line)
        if stats and current is not None:
            # The stats marker is not part of fio JSON.
            if json_lines and json_lines[-1] == line:
                json_lines.pop()
            payload = json.loads("\n".join(json_lines))
            read = payload["jobs"][0]["read"]
            current.update(
                {
                    "bw_mib_s": float(read["bw_bytes"]) / (1024 * 1024),
                    "iops": float(read["iops"]),
                    "p50_us": latency_us(read, "50.000000"),
                    "p95_us": latency_us(read, "95.000000"),
                    "p99_us": latency_us(read, "99.000000"),
                }
            )
            current.update({key: int(value) for key, value in COUNTER_RE.findall(stats.group(3))})
            rows.append(current)
            current = None
            json_lines = []
            collecting_json = False

    return rows


def write_rows(path: Path, rows: list[dict]) -> None:
    fields = [
        "label",
        "run",
        "device",
        "bw_mib_s",
        "iops",
        "p50_us",
        "p95_us",
        "p99_us",
        "cache_blocks",
        "verified",
        "bio_blocks",
        "data_rereads",
        "hash_hits",
        "hash_misses",
        "auth_shortcuts",
    ]
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", newline="", encoding="utf-8") as output:
        writer = csv.DictWriter(output, fieldnames=fields)
        writer.writeheader()
        writer.writerows(rows)


def write_summary(path: Path, rows: list[dict]) -> None:
    fields = [
        "label",
        "runs",
        "bw_mib_s_median",
        "iops_median",
        "p50_us_median",
        "p95_us_median",
        "p99_us_median",
    ]
    summary = []
    for label in ("raw", "dm-verity"):
        group = [row for row in rows if row["label"] == label]
        if not group:
            continue
        summary.append(
            {
                "label": label,
                "runs": len(group),
                "bw_mib_s_median": f"{statistics.median(row['bw_mib_s'] for row in group):.3f}",
                "iops_median": f"{statistics.median(row['iops'] for row in group):.3f}",
                "p50_us_median": f"{statistics.median(row['p50_us'] for row in group):.3f}",
                "p95_us_median": f"{statistics.median(row['p95_us'] for row in group):.3f}",
                "p99_us_median": f"{statistics.median(row['p99_us'] for row in group):.3f}",
            }
        )
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", newline="", encoding="utf-8") as output:
        writer = csv.DictWriter(output, fieldnames=fields)
        writer.writeheader()
        writer.writerows(summary)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("serial_log", type=Path)
    parser.add_argument("raw_csv", type=Path)
    parser.add_argument("summary_csv", type=Path)
    args = parser.parse_args()

    serial_text = args.serial_log.read_text(encoding="utf-8", errors="replace")
    if "profile=release-lto" not in serial_text:
        raise SystemExit("formal benchmark log must contain profile=release-lto")

    rows = parse_log(args.serial_log)
    if len(rows) != 10:
        raise SystemExit(f"expected 10 fio results (5 raw + 5 dm-verity), found {len(rows)}")
    write_rows(args.raw_csv, rows)
    write_summary(args.summary_csv, rows)


if __name__ == "__main__":
    main()
