#!/usr/bin/env python3
"""Compare five-run dm-crypt before/after data without hiding raw-host drift."""

from __future__ import annotations

import argparse
import csv
import statistics
from pathlib import Path


CONTRACT_FIELDS = (
    "build_profile",
    "stage_stats",
    "vcpus",
    "memory",
    "accelerator",
    "cpu_model",
    "device_model",
    "image_size_bytes",
    "image_region_sha256",
    "cipher",
    "rw",
    "block_size",
    "ioengine",
    "direct",
    "queue_depth",
    "jobs",
    "size_mib",
    "ramp_seconds",
    "runtime_seconds",
    "runs",
    "scratch_pool",
)


def read_metadata(path: Path) -> dict[str, str]:
    result: dict[str, str] = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        if "=" in line:
            key, value = line.split("=", 1)
            result[key] = value
    return result


def require_same_contract(before: dict[str, str], after: dict[str, str]) -> None:
    mismatches = [
        field for field in CONTRACT_FIELDS if before.get(field) != after.get(field)
    ]
    if mismatches:
        raise ValueError("benchmark contract mismatch: " + ", ".join(mismatches))
    for label, metadata in (("before", before), ("after", after)):
        if metadata.get("git_dirty") != "0":
            raise ValueError(f"{label}: formal comparison requires git_dirty=0")


def read_runs(path: Path) -> dict[str, dict[int, dict[str, str]]]:
    groups: dict[str, dict[int, dict[str, str]]] = {"raw": {}, "dm-crypt": {}}
    with path.open(newline="", encoding="utf-8") as source:
        for row in csv.DictReader(source):
            label = row["label"]
            if label in groups:
                groups[label][int(row["run"])] = row
    for label, runs in groups.items():
        if sorted(runs) != [1, 2, 3, 4, 5]:
            raise ValueError(f"{path}: {label} requires runs 1..5")
    return groups


def summarize(
    phase: str,
    metadata: dict[str, str],
    groups: dict[str, dict[int, dict[str, str]]],
) -> dict[str, str | float]:
    raw_bw = [float(groups["raw"][run]["bw_mib_s"]) for run in range(1, 6)]
    crypt_bw = [float(groups["dm-crypt"][run]["bw_mib_s"]) for run in range(1, 6)]
    paired_efficiency = [
        100.0 * crypt_bw[run - 1] / raw_bw[run - 1] for run in range(1, 6)
    ]
    return {
        "phase": phase,
        "commit": metadata["git_commit"],
        "raw_bw_mib_s_median": statistics.median(raw_bw),
        "crypt_bw_mib_s_median": statistics.median(crypt_bw),
        "crypt_over_raw_percent_median": statistics.median(paired_efficiency),
        "crypt_p95_us_median": statistics.median(
            float(groups["dm-crypt"][run]["p95_us"]) for run in range(1, 6)
        ),
    }


def compare(before: dict[str, str | float], after: dict[str, str | float]) -> dict[str, str | float]:
    raw_speedup = float(after["raw_bw_mib_s_median"]) / float(before["raw_bw_mib_s_median"])
    crypt_speedup = float(after["crypt_bw_mib_s_median"]) / float(before["crypt_bw_mib_s_median"])
    efficiency_speedup = float(after["crypt_over_raw_percent_median"]) / float(
        before["crypt_over_raw_percent_median"]
    )
    return {
        "phase": "change_percent",
        "commit": f'{before["commit"]}..{after["commit"]}',
        "raw_bw_mib_s_median": 100.0 * (raw_speedup - 1.0),
        "crypt_bw_mib_s_median": 100.0 * (crypt_speedup - 1.0),
        "crypt_over_raw_percent_median": 100.0 * (efficiency_speedup - 1.0),
        "crypt_p95_us_median": 100.0
        * (
            float(after["crypt_p95_us_median"])
            / float(before["crypt_p95_us_median"])
            - 1.0
        ),
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("before_csv", type=Path)
    parser.add_argument("after_csv", type=Path)
    parser.add_argument("before_metadata", type=Path)
    parser.add_argument("after_metadata", type=Path)
    parser.add_argument("output_csv", type=Path)
    args = parser.parse_args()

    before_metadata = read_metadata(args.before_metadata)
    after_metadata = read_metadata(args.after_metadata)
    require_same_contract(before_metadata, after_metadata)
    before = summarize("before", before_metadata, read_runs(args.before_csv))
    after = summarize("after", after_metadata, read_runs(args.after_csv))
    rows = [before, after, compare(before, after)]

    args.output_csv.parent.mkdir(parents=True, exist_ok=True)
    with args.output_csv.open("w", newline="", encoding="utf-8") as output:
        writer = csv.DictWriter(output, fieldnames=list(rows[0]), lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)
    print(f"ab_csv={args.output_csv}")


if __name__ == "__main__":
    main()
