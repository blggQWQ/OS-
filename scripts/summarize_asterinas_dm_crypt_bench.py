#!/usr/bin/env python3
"""Extract a five-run Asterinas raw/dm-crypt fio matrix from a serial log."""

from __future__ import annotations

import argparse
import csv
import json
import re
import statistics
from pathlib import Path

BEGIN_RE = re.compile(r"\[dm-bench\] BEGIN label=(\S+) run=(\d+) device=(\S+)")
STATS_RE = re.compile(r"\[dm-bench\] STATS label=(\S+) run=(\d+) (.*)")


def latency_us(read: dict, percentile: str) -> float:
    for key, scale in (("clat_ns", 1 / 1000), ("clat_us", 1), ("clat_ms", 1000)):
        value = read.get(key, {}).get("percentile", {}).get(percentile)
        if value is not None:
            return float(value) * scale
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
            current = {"label": begin.group(1), "run": int(begin.group(2)), "device": begin.group(3)}
            json_lines, collecting_json = [], False
            continue
        if current is not None and (collecting_json or line.lstrip().startswith("{")):
            collecting_json = True
            json_lines.append(line)
        stats = STATS_RE.search(line)
        if stats and current is not None:
            if json_lines and json_lines[-1] == line:
                json_lines.pop()
            read = json.loads("\n".join(json_lines))["jobs"][0]["read"]
            current.update({
                "bw_mib_s": float(read["bw_bytes"]) / (1024 * 1024),
                "iops": float(read["iops"]),
                "p50_us": latency_us(read, "50.000000"),
                "p95_us": latency_us(read, "95.000000"),
                "p99_us": latency_us(read, "99.000000"),
                "backend": re.search(r"backend=(\S+)", stats.group(3)).group(1) if "backend=" in stats.group(3) else "unknown",
            })
            rows.append(current)
            current, json_lines, collecting_json = None, [], False
    return rows


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("serial_log", type=Path)
    parser.add_argument("raw_csv", type=Path)
    parser.add_argument("summary_csv", type=Path)
    args = parser.parse_args()
    serial_text = args.serial_log.read_text(encoding="utf-8", errors="replace")
    if "profile=release-lto" not in serial_text:
        raise SystemExit("formal benchmark log must contain profile=release-lto")
    if not re.search(r"(?:AES\s+)?backend[=:]AES-NI\b", serial_text):
        raise SystemExit("formal benchmark log must confirm AES backend=AES-NI")
    rows = parse_log(args.serial_log)
    if len(rows) != 10 or {row["label"] for row in rows} != {"raw", "dm-crypt"}:
        raise SystemExit(f"expected five raw and five dm-crypt results, found {len(rows)} rows")
    args.raw_csv.parent.mkdir(parents=True, exist_ok=True)
    fields = ["label", "run", "device", "bw_mib_s", "iops", "p50_us", "p95_us", "p99_us", "backend"]
    with args.raw_csv.open("w", newline="", encoding="utf-8") as output:
        writer = csv.DictWriter(output, fieldnames=fields)
        writer.writeheader()
        writer.writerows(rows)
    summary = []
    for label in ("raw", "dm-crypt"):
        group = [row for row in rows if row["label"] == label]
        summary.append({
            "label": label, "runs": len(group), "backend": group[-1]["backend"],
            "bw_mib_s_median": f"{statistics.median(row['bw_mib_s'] for row in group):.3f}",
            "iops_median": f"{statistics.median(row['iops'] for row in group):.3f}",
            "p50_us_median": f"{statistics.median(row['p50_us'] for row in group):.3f}",
            "p95_us_median": f"{statistics.median(row['p95_us'] for row in group):.3f}",
            "p99_us_median": f"{statistics.median(row['p99_us'] for row in group):.3f}",
        })
    with args.summary_csv.open("w", newline="", encoding="utf-8") as output:
        writer = csv.DictWriter(output, fieldnames=list(summary[0]))
        writer.writeheader()
        writer.writerows(summary)


if __name__ == "__main__":
    main()
