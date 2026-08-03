#!/usr/bin/env python3
"""Extract per-run Device Mapper stage deltas from an Asterinas serial log."""

from __future__ import annotations

import argparse
import csv
import re
from pathlib import Path


META_RE = re.compile(r"\[dm-bench-meta] (?P<body>.*)")
BEFORE_RE = re.compile(
    r"\[dm-bench] STATS_BEFORE label=(?P<label>\S+) run=(?P<run>\d+) (?P<body>.*)"
)
AFTER_RE = re.compile(
    r"\[dm-bench] STATS label=(?P<label>\S+) run=(?P<run>\d+) (?P<body>.*)"
)
KEY_VALUE_RE = re.compile(r"(?P<key>[a-z0-9_]+)=(?P<value>[^\s]+)")
COUNTERS = (
    "operations",
    "bytes",
    "failures",
    "total_ns",
    "validation_ns",
    "scratch_ns",
    "copy_ns",
    "crypto_ns",
    "lower_wait_ns",
    "data_sha256_ns",
    "cache_lookup_ns",
    "hash_wait_ns",
    "hash_sha256_ns",
    "digest_compare_ns",
    "cache_insert_ns",
    "tree_levels",
)
STAGES = tuple(name for name in COUNTERS if name.endswith("_ns") and name != "total_ns")


def key_values(text: str) -> dict[str, str]:
    return {match.group("key"): match.group("value") for match in KEY_VALUE_RE.finditer(text)}


def parse(path: Path, target: str) -> list[dict[str, object]]:
    metadata: dict[str, str] = {}
    before: dict[tuple[str, int], dict[str, str]] = {}
    after: dict[tuple[str, int], dict[str, str]] = {}
    text = path.read_text(encoding="utf-8", errors="replace")
    for line in text.splitlines():
        if match := META_RE.search(line):
            metadata.update(key_values(match.group("body")))
        if match := BEFORE_RE.search(line):
            before[(match.group("label"), int(match.group("run")))] = key_values(
                match.group("body")
            )
        if match := AFTER_RE.search(line):
            perf = match.group("body").split("perf=", 1)
            if len(perf) == 2:
                after[(match.group("label"), int(match.group("run")))] = key_values(perf[1])

    if metadata.get("profile") != "release-lto":
        raise ValueError(f"{path}: formal evidence must use profile=release-lto")
    if metadata.get("stage_stats") != "1":
        raise ValueError(f"{path}: stage_stats=1 is required")
    if target == "dm-crypt" and not re.search(r"(?:AES\s+)?backend[=:]AES-NI\b", text):
        raise ValueError(f"{path}: AES-NI confirmation is missing from the serial log")
    if set(before) != set(after):
        raise ValueError(f"{path}: STATS_BEFORE/STATS pairs do not match")

    rows: list[dict[str, object]] = []
    for key in sorted(before, key=lambda item: (item[1], item[0])):
        label, run = key
        baseline = before[key]
        final = after[key]
        if final.get("enabled") != "1":
            raise ValueError(f"{path}: stage attribution disabled for {label} run {run}")
        deltas: dict[str, int] = {}
        for counter in COUNTERS:
            start = int(baseline.get(counter, 0))
            end = int(final.get(counter, 0))
            if end < start:
                raise ValueError(f"{path}: counter {counter} went backwards")
            deltas[counter] = end - start
        total = deltas["total_ns"]
        row: dict[str, object] = {
            "label": label,
            "run": run,
            "target": final.get("target", "unknown"),
            **deltas,
        }
        for stage in STAGES:
            row[f"{stage.removesuffix('_ns')}_percent"] = (
                100.0 * deltas[stage] / total if total else 0.0
            )
        rows.append(row)

    labels = {row["label"] for row in rows}
    if labels != {"raw", target}:
        raise ValueError(f"{path}: expected raw/{target}, found {sorted(labels)}")
    for label in labels:
        runs = sorted(int(row["run"]) for row in rows if row["label"] == label)
        if runs != [1, 2, 3, 4, 5]:
            raise ValueError(f"{path}: {label} must contain runs 1..5")
    return rows


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("serial_log", type=Path)
    parser.add_argument("output_csv", type=Path)
    parser.add_argument("--target", choices=("dm-crypt", "dm-verity"), required=True)
    args = parser.parse_args()

    rows = parse(args.serial_log, args.target)
    args.output_csv.parent.mkdir(parents=True, exist_ok=True)
    with args.output_csv.open("w", newline="", encoding="utf-8") as output:
        writer = csv.DictWriter(output, fieldnames=list(rows[0]))
        writer.writeheader()
        writer.writerows(rows)
    print(f"stage_csv={args.output_csv}")


if __name__ == "__main__":
    main()
