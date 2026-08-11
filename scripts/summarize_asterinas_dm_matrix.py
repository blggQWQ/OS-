#!/usr/bin/env python3
"""Extract configurable Asterinas fio runs into auditable CSV files."""

from __future__ import annotations

import argparse
import csv
import json
import re
import statistics
from pathlib import Path


BEGIN_RE = re.compile(
    r"^\[dm-bench\] BEGIN label=(?P<label>\S+) run=(?P<run>\d+) "
    r"device=(?P<device>\S+) operation=(?P<operation>\S+) "
    r"block_size=(?P<block_size>\S+) (?:ioengine=(?P<ioengine>\S+) )?"
    r"queue_depth=(?P<queue_depth>\d+)(?: jobs=\d+)?$"
)
END_RE = re.compile(r"^\[dm-bench\] END label=(?P<label>\S+) run=(?P<run>\d+)$")
PERCENTILES = ("50.000000", "95.000000", "99.000000")
FIELDS = (
    "label",
    "run",
    "device",
    "operation",
    "block_size",
    "ioengine",
    "queue_depth",
    "bw_mib_s",
    "iops",
    "p50_us",
    "p95_us",
    "p99_us",
)


def parse_log(path: Path) -> list[dict[str, object]]:
    rows: list[dict[str, object]] = []
    pending: dict[str, str] | None = None
    json_lines: list[str] = []
    json_started = False
    brace_depth = 0
    for raw_line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        line = raw_line.strip()
        begin = BEGIN_RE.match(line)
        if begin:
            if pending is not None:
                raise ValueError(f"{path}: nested BEGIN marker")
            pending = begin.groupdict()
            json_lines = []
            json_started = False
            brace_depth = 0
            continue
        if pending is None:
            continue
        end = END_RE.match(line)
        if end:
            if end.group("label") != pending["label"] or end.group("run") != pending["run"]:
                raise ValueError(f"{path}: BEGIN/END mismatch")
            if not json_lines or brace_depth != 0:
                raise ValueError(f"{path}: incomplete fio JSON payload")
            payload = json.loads("\n".join(json_lines))
            operation = pending["operation"]
            io_key = "read" if operation in {"read", "randread"} else "write"
            io = payload["jobs"][0][io_key]
            percentile = io["clat_ns"]["percentile"]
            rows.append(
                {
                    **pending,
                    "run": int(pending["run"]),
                    "queue_depth": int(pending["queue_depth"]),
                    "bw_mib_s": io["bw_bytes"] / 1048576,
                    "iops": io["iops"],
                    "p50_us": percentile[PERCENTILES[0]] / 1000,
                    "p95_us": percentile[PERCENTILES[1]] / 1000,
                    "p99_us": percentile[PERCENTILES[2]] / 1000,
                }
            )
            pending = None
            json_lines = []
            continue
        if not json_started and line.startswith("{"):
            json_started = True
        if json_started and brace_depth == 0 and json_lines:
            continue
        if json_started:
            json_lines.append(raw_line)
            brace_depth += raw_line.count("{") - raw_line.count("}")
    if pending is not None:
        raise ValueError(f"{path}: missing END marker")
    if not rows:
        raise ValueError(f"{path}: no configurable benchmark runs found")
    return rows


def validate(rows: list[dict[str, object]]) -> None:
    groups: dict[tuple[object, ...], list[int]] = {}
    for row in rows:
        key = (
            row["label"],
            row["operation"],
            row["block_size"],
            row["queue_depth"],
        )
        groups.setdefault(key, []).append(int(row["run"]))
    for key, runs in groups.items():
        expected = list(range(1, max(runs) + 1))
        if sorted(runs) != expected or len(runs) < 5:
            raise ValueError(f"{key}: need contiguous runs 1..N with N>=5, found {sorted(runs)}")


def write_csv(path: Path, rows: list[dict[str, object]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", newline="", encoding="utf-8") as target:
        writer = csv.DictWriter(target, fieldnames=FIELDS)
        writer.writeheader()
        writer.writerows(rows)


def write_summary(path: Path, rows: list[dict[str, object]]) -> None:
    grouped: dict[tuple[object, ...], list[dict[str, object]]] = {}
    for row in rows:
        key = (
            row["label"],
            row["operation"],
            row["block_size"],
            row["queue_depth"],
        )
        grouped.setdefault(key, []).append(row)
    fields = (
        "label",
        "operation",
        "block_size",
        "queue_depth",
        "runs",
        "metric", "mean", "median", "stddev",
    )
    with path.open("w", newline="", encoding="utf-8") as target:
        writer = csv.DictWriter(target, fieldnames=fields)
        writer.writeheader()
        for key, group in sorted(grouped.items()):
            for metric in ("bw_mib_s", "iops", "p50_us", "p95_us", "p99_us"):
                values = [float(row[metric]) for row in group]
                writer.writerow({
                    "label": key[0], "operation": key[1], "block_size": key[2],
                    "queue_depth": key[3], "runs": len(group), "metric": metric,
                    "mean": statistics.fmean(values), "median": statistics.median(values),
                    "stddev": statistics.stdev(values) if len(values) >= 2 else 0.0,
                })


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("log", type=Path)
    parser.add_argument("raw_csv", type=Path)
    parser.add_argument("summary_csv", type=Path)
    args = parser.parse_args()
    rows = parse_log(args.log)
    validate(rows)
    write_csv(args.raw_csv, rows)
    write_summary(args.summary_csv, rows)
    print(f"raw_csv={args.raw_csv}")
    print(f"summary_csv={args.summary_csv}")


if __name__ == "__main__":
    main()
