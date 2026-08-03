#!/usr/bin/env python3
"""Quantifies the observer overhead of Device Mapper stage attribution."""

from __future__ import annotations

import argparse
import csv
import statistics
from pathlib import Path


def target_median(path: Path) -> tuple[str, float]:
    with path.open(newline="", encoding="utf-8") as source:
        rows = list(csv.DictReader(source))
    labels = {row["label"] for row in rows} - {"raw"}
    if len(labels) != 1:
        raise ValueError(f"{path}: expected one mapped target")
    label = labels.pop()
    group = [float(row["bw_mib_s"]) for row in rows if row["label"] == label]
    if len(group) != 5:
        raise ValueError(f"{path}: expected five {label} runs")
    return label, statistics.median(group)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("disabled_csv", type=Path)
    parser.add_argument("enabled_csv", type=Path)
    parser.add_argument("output_csv", type=Path)
    args = parser.parse_args()
    disabled_label, disabled = target_median(args.disabled_csv)
    enabled_label, enabled = target_median(args.enabled_csv)
    if disabled_label != enabled_label:
        raise ValueError("target labels do not match")
    overhead = 100.0 * (disabled - enabled) / disabled
    row = {
        "target": disabled_label,
        "stats_disabled_mib_s": disabled,
        "stats_enabled_mib_s": enabled,
        "throughput_overhead_percent": overhead,
        "within_3_percent": abs(overhead) <= 3.0,
    }
    args.output_csv.parent.mkdir(parents=True, exist_ok=True)
    with args.output_csv.open("w", newline="", encoding="utf-8") as output:
        writer = csv.DictWriter(output, fieldnames=list(row))
        writer.writeheader()
        writer.writerow(row)
    print(f"overhead_csv={args.output_csv}")


if __name__ == "__main__":
    main()
