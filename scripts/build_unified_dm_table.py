#!/usr/bin/env python3
"""Build the auditable three-row raw/dm-crypt/dm-verity result table."""

from __future__ import annotations

import argparse
import csv
import statistics
from pathlib import Path


CONTRACT_FIELDS = (
    "build_profile",
    "vcpus",
    "memory",
    "accelerator",
    "cpu_model",
    "device_model",
    "image_size_bytes",
    "rw",
    "block_size",
    "ioengine",
    "direct",
    "queue_depth",
    "size_mib",
    "ramp_seconds",
    "runtime_seconds",
    "runs",
)
METRICS = ("bw_mib_s", "p50_us", "p95_us", "p99_us")


def read_metadata(path: Path) -> dict[str, str]:
    values: dict[str, str] = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        if "=" in line:
            key, value = line.split("=", 1)
            values[key] = value
    return values


def read_rows(path: Path) -> list[dict[str, str]]:
    with path.open(newline="", encoding="utf-8") as source:
        return list(csv.DictReader(source))


def medians(rows: list[dict[str, str]], label: str) -> dict[str, float]:
    group = [row for row in rows if row["label"] == label]
    if sorted(int(row["run"]) for row in group) != [1, 2, 3, 4, 5]:
        raise ValueError(f"{label}: expected runs 1..5")
    return {metric: statistics.median(float(row[metric]) for row in group) for metric in METRICS}


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("crypt_csv", type=Path)
    parser.add_argument("verity_csv", type=Path)
    parser.add_argument("crypt_metadata", type=Path)
    parser.add_argument("verity_metadata", type=Path)
    parser.add_argument("output_csv", type=Path)
    args = parser.parse_args()

    crypt_meta = read_metadata(args.crypt_metadata)
    verity_meta = read_metadata(args.verity_metadata)
    mismatches = [
        field
        for field in CONTRACT_FIELDS
        if crypt_meta.get(field) != verity_meta.get(field)
    ]
    if mismatches:
        raise ValueError("benchmark contract mismatch: " + ", ".join(mismatches))
    expected = {
        "build_profile": "release-lto",
        "accelerator": "kvm",
        "cpu_model": "host",
        "size_mib": "256",
        "runtime_seconds": "10",
        "runs": "5",
    }
    for field, value in expected.items():
        if crypt_meta.get(field) != value:
            raise ValueError(f"formal contract requires {field}={value}")

    crypt_rows = read_rows(args.crypt_csv)
    verity_rows = read_rows(args.verity_csv)
    raw_crypt = medians(crypt_rows, "raw")
    raw_verity = medians(verity_rows, "raw")
    crypt = medians(crypt_rows, "dm-crypt")
    verity = medians(verity_rows, "dm-verity")
    pooled_raw = {
        metric: statistics.median(
            [
                float(row[metric])
                for row in crypt_rows + verity_rows
                if row["label"] == "raw"
            ]
        )
        for metric in METRICS
    }
    output_rows = [
        {"device": "raw", **pooled_raw, "paired_raw_mib_s": pooled_raw["bw_mib_s"], "relative_raw_percent": 100.0},
        {"device": "dm-crypt", **crypt, "paired_raw_mib_s": raw_crypt["bw_mib_s"], "relative_raw_percent": 100.0 * crypt["bw_mib_s"] / raw_crypt["bw_mib_s"]},
        {"device": "dm-verity", **verity, "paired_raw_mib_s": raw_verity["bw_mib_s"], "relative_raw_percent": 100.0 * verity["bw_mib_s"] / raw_verity["bw_mib_s"]},
    ]
    args.output_csv.parent.mkdir(parents=True, exist_ok=True)
    with args.output_csv.open("w", newline="", encoding="utf-8") as output:
        writer = csv.DictWriter(output, fieldnames=list(output_rows[0]))
        writer.writeheader()
        writer.writerows(output_rows)
    print(f"unified_csv={args.output_csv}")


if __name__ == "__main__":
    main()
