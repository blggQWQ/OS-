#!/usr/bin/env python3
"""Builds the strict Linux/Asterinas dm-crypt comparison table."""

from __future__ import annotations

import argparse
import csv
import statistics
from pathlib import Path


METRICS = ("bw_mib_s", "iops", "p50_us", "p95_us", "p99_us")
SHARED_CONTRACT = (
    "vcpus",
    "memory",
    "accelerator",
    "cpu_model",
    "device_model",
    "image_region_sha256",
    "cipher",
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


def load_guest(path: Path, expected_runs: int) -> dict[str, dict[str, float]]:
    groups: dict[str, list[dict[str, str]]] = {"raw": [], "dm-crypt": []}
    with path.open(newline="", encoding="utf-8") as source:
        for row in csv.DictReader(source):
            label = row.get("label")
            if label not in groups:
                raise ValueError(f"{path}: unexpected label {label!r}")
            groups[label].append(row)
    for label, rows in groups.items():
        runs = sorted(int(row["run"]) for row in rows)
        expected = list(range(1, expected_runs + 1))
        if runs != expected:
            raise ValueError(
                f"{path}: {label} runs must be exactly 1..{expected_runs}, found {runs}"
            )
    return {
        label: {
            metric: statistics.median(float(row[metric]) for row in rows)
            for metric in METRICS
        }
        for label, rows in groups.items()
    }


def summarize(
    system: str, data: dict[str, dict[str, float]], runs: int
) -> dict[str, object]:
    raw = data["raw"]
    crypt = data["dm-crypt"]
    return {
        "system": system,
        "runs_per_target": runs,
        "raw_median_mib_s": raw["bw_mib_s"],
        "dm_crypt_median_mib_s": crypt["bw_mib_s"],
        "throughput_retention_percent": 100 * crypt["bw_mib_s"] / raw["bw_mib_s"],
        "dm_crypt_median_iops": crypt["iops"],
        "dm_crypt_median_p50_us": crypt["p50_us"],
        "dm_crypt_median_p95_us": crypt["p95_us"],
        "dm_crypt_median_p99_us": crypt["p99_us"],
    }


def load_metadata(path: Path) -> dict[str, str]:
    result: dict[str, str] = {}
    for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if not line or line.startswith("#"):
            continue
        if "=" not in line:
            raise ValueError(f"{path}:{number}: expected key=value")
        key, value = line.split("=", 1)
        result[key] = value
    missing = [key for key in SHARED_CONTRACT if not result.get(key)]
    if missing:
        raise ValueError(f"{path}: missing contract keys: {', '.join(missing)}")
    return result


def require_same_contract(linux: dict[str, str], asterinas: dict[str, str]) -> None:
    mismatches = [
        f"{key}: Linux={linux[key]!r}, Asterinas={asterinas[key]!r}"
        for key in SHARED_CONTRACT
        if linux[key] != asterinas[key]
    ]
    if mismatches:
        raise ValueError("benchmark contract mismatch:\n  " + "\n  ".join(mismatches))


def format_num(value: object) -> str:
    return f"{float(value):.3f}"


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("linux_csv", type=Path)
    parser.add_argument("asterinas_csv", type=Path)
    parser.add_argument("output_csv", type=Path)
    parser.add_argument("output_markdown", type=Path)
    parser.add_argument("--linux-metadata", required=True, type=Path)
    parser.add_argument("--asterinas-metadata", required=True, type=Path)
    args = parser.parse_args()
    linux_metadata = load_metadata(args.linux_metadata)
    asterinas_metadata = load_metadata(args.asterinas_metadata)
    require_same_contract(linux_metadata, asterinas_metadata)

    runs = int(linux_metadata["runs"])
    rows = [
        summarize("Linux guest", load_guest(args.linux_csv, runs), runs),
        summarize("Asterinas", load_guest(args.asterinas_csv, runs), runs),
    ]
    args.output_csv.parent.mkdir(parents=True, exist_ok=True)
    with args.output_csv.open("w", newline="", encoding="utf-8") as target:
        writer = csv.DictWriter(target, fieldnames=list(rows[0]))
        writer.writeheader()
        writer.writerows(rows)

    linux_crypt = float(rows[0]["dm_crypt_median_mib_s"])
    asterinas_crypt = float(rows[1]["dm_crypt_median_mib_s"])
    lines = [
        "# Linux guest vs Asterinas dm-crypt 同条件五轮对照",
        "",
        "共同条件已由脚本逐项核对："
        f"{linux_metadata['accelerator']}、CPU={linux_metadata['cpu_model']}、"
        f"{linux_metadata['vcpus']} vCPU、内存 {linux_metadata['memory']}、"
        f"{linux_metadata['device_model']}；",
        f"{linux_metadata['cipher']}；fio {linux_metadata['block_size']}、"
        f"{linux_metadata['rw']}、{linux_metadata['ioengine']}、"
        f"direct={linux_metadata['direct']}、QD{linux_metadata['queue_depth']}、"
        f"{linux_metadata['size_mib']} MiB、ramp {linux_metadata['ramp_seconds']} 秒、"
        f"runtime {linux_metadata['runtime_seconds']} 秒；每个目标 "
        f"{linux_metadata['runs']} 轮并取中位数。",
        f"测试区域 SHA-256：`{linux_metadata['image_region_sha256']}`。",
        "",
        "| 系统 | raw MiB/s | dm-crypt MiB/s | 保留率 | dm-crypt IOPS | p50 us | p95 us | p99 us |",
        "|---|---:|---:|---:|---:|---:|---:|---:|",
    ]
    for row in rows:
        lines.append(
            f"| {row['system']} | {format_num(row['raw_median_mib_s'])} | "
            f"{format_num(row['dm_crypt_median_mib_s'])} | "
            f"{format_num(row['throughput_retention_percent'])}% | "
            f"{format_num(row['dm_crypt_median_iops'])} | "
            f"{format_num(row['dm_crypt_median_p50_us'])} | "
            f"{format_num(row['dm_crypt_median_p95_us'])} | "
            f"{format_num(row['dm_crypt_median_p99_us'])} |"
        )
    lines.extend(
        [
            "",
            f"Asterinas dm-crypt 的绝对吞吐为 Linux guest 的 "
            f"**{100 * asterinas_crypt / linux_crypt:.1f}%**。该比例仅在上述条件全部",
            "由两端 metadata 和原始 CSV 核对一致时有效；条件不一致时脚本会拒绝出表。",
            "",
        ]
    )
    args.output_markdown.write_text("\n".join(lines), encoding="utf-8")
    print(f"comparison_csv={args.output_csv}")
    print(f"comparison_markdown={args.output_markdown}")


if __name__ == "__main__":
    main()
