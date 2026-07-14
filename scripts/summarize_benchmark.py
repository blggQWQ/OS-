#!/usr/bin/env python3
"""Summarizes paired Linux raw/dm-crypt fio runs without third-party packages."""

from __future__ import annotations

import argparse
import csv
import statistics
from collections import defaultdict
from pathlib import Path


METRICS = (
    "mib_per_second",
    "iops",
    "mean_latency_us",
    "p95_latency_us",
    "p99_latency_us",
    "cpu_percent",
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("input", type=Path, help="CSV from benchmark_linux_dm.sh")
    parser.add_argument("output", type=Path, nargs="?", help="summary CSV path")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    output = args.output or args.input.with_name(f"{args.input.stem}-summary.csv")
    grouped: dict[tuple[str, str, str, str], dict[str, list[float]]] = defaultdict(
        lambda: defaultdict(list)
    )
    with args.input.open(newline="", encoding="utf-8") as source:
        for row in csv.DictReader(source):
            key = (
                row["implementation"],
                row["operation"],
                row["block_size"],
                row["queue_depth"],
            )
            for metric in METRICS:
                grouped[key][metric].append(float(row[metric]))

    output.parent.mkdir(parents=True, exist_ok=True)
    fieldnames = [
        "implementation",
        "operation",
        "block_size",
        "queue_depth",
        "runs",
        *(f"median_{metric}" for metric in METRICS),
        "throughput_vs_raw_percent",
        "p95_latency_overhead_percent",
    ]
    medians = {
        key: {metric: statistics.median(values) for metric, values in metrics.items()}
        for key, metrics in grouped.items()
    }
    with output.open("w", newline="", encoding="utf-8") as target:
        writer = csv.DictWriter(target, fieldnames=fieldnames)
        writer.writeheader()
        for key in sorted(medians):
            implementation, operation, block_size, queue_depth = key
            values = medians[key]
            raw = medians.get(("linux-raw", operation, block_size, queue_depth))
            throughput_ratio = ""
            latency_overhead = ""
            if raw and implementation != "linux-raw":
                throughput_ratio = 100.0 * values["mib_per_second"] / raw["mib_per_second"]
                latency_overhead = (
                    100.0 * values["p95_latency_us"] / raw["p95_latency_us"] - 100.0
                )
            row = {
                "implementation": implementation,
                "operation": operation,
                "block_size": block_size,
                "queue_depth": queue_depth,
                "runs": len(grouped[key]["iops"]),
                "throughput_vs_raw_percent": throughput_ratio,
                "p95_latency_overhead_percent": latency_overhead,
            }
            row.update({f"median_{metric}": value for metric, value in values.items()})
            writer.writerow(row)
    print(f"summary_csv={output}")


if __name__ == "__main__":
    main()
