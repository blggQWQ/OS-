#!/usr/bin/env python3
"""Aggregate FIO matrix per-run CSVs into mean/median/stdev summaries.

Does not drop outliers. Separates iodepth_set from max_in_flight_observed.
"""

from __future__ import annotations

import argparse
import csv
import statistics
from pathlib import Path

METRIC_FIELDS = (
    "bw_mib_s",
    "iops",
    "p50_us",
    "p95_us",
    "p99_us",
    "cpu_percent",
)


def mean(values: list[float]) -> float:
    return statistics.fmean(values) if values else float("nan")


def median(values: list[float]) -> float:
    return statistics.median(values) if values else float("nan")


def stdev(values: list[float]) -> float:
    if len(values) < 2:
        return 0.0 if values else float("nan")
    return statistics.stdev(values)


def load_per_run_csvs(root: Path) -> list[dict[str, str]]:
    rows: list[dict[str, str]] = []
    for path in sorted(root.rglob("per-run.csv")):
        with path.open(newline="", encoding="utf-8") as source:
            for row in csv.DictReader(source):
                row["_source"] = str(path.relative_to(root))
                rows.append(row)
    na_path = root / "na-cells.csv"
    if na_path.is_file():
        with na_path.open(newline="", encoding="utf-8") as source:
            for row in csv.DictReader(source):
                row.setdefault("status", "N/A")
                row.setdefault("run", "0")
                row["_source"] = "na-cells.csv"
                rows.append(row)
    return rows


def group_key(row: dict[str, str]) -> tuple[str, str, str, str, str]:
    return (
        row.get("platform", ""),
        row["target"],
        row["operation"],
        row["block_size"],
        str(row["iodepth_set"]),
    )


def parse_float(row: dict[str, str], field: str) -> float | None:
    raw = (row.get(field) or "").strip()
    if raw in {"", "unavailable", "N/A", "nan", "NaN"}:
        return None
    return float(raw)


def summarize_group(key: tuple[str, ...], group: list[dict[str, str]]) -> dict[str, object]:
    status = group[0].get("status", "ok")
    measured = [row for row in group if row.get("status", "ok") == "ok"]
    sample = measured[0] if measured else group[0]
    out: dict[str, object] = {
        "platform": key[0],
        "target": key[1],
        "operation": key[2],
        "block_size": key[3],
        "iodepth_set": key[4],
        "status": status if status != "ok" else ("ok" if measured else status),
        "runs": len(measured),
        "ioengine": sample.get("ioengine", ""),
        "max_in_flight_observed": sample.get(
            "max_in_flight_observed", "unavailable"
        ),
        "sources": ";".join(sorted({row.get("_source", "") for row in group})),
    }
    if status == "N/A" or not measured:
        for metric in METRIC_FIELDS:
            out[f"{metric}_mean"] = "N/A"
            out[f"{metric}_median"] = "N/A"
            out[f"{metric}_stdev"] = "N/A"
        if status != "N/A":
            out["status"] = "missing"
        return out

    runs = sorted(int(row["run"]) for row in measured)
    if runs != list(range(1, len(runs) + 1)) or len(runs) < 5:
        out["status"] = f"incomplete_runs:{runs}"

    for metric in METRIC_FIELDS:
        values = [v for row in measured if (v := parse_float(row, metric)) is not None]
        if not values:
            out[f"{metric}_mean"] = "unavailable"
            out[f"{metric}_median"] = "unavailable"
            out[f"{metric}_stdev"] = "unavailable"
            continue
        out[f"{metric}_mean"] = f"{mean(values):.6f}"
        out[f"{metric}_median"] = f"{median(values):.6f}"
        out[f"{metric}_stdev"] = f"{stdev(values):.6f}"
    return out


def write_three_target_table(path: Path, summaries: list[dict[str, object]]) -> None:
    """Same platform/op/bs/qd rows for raw, dm-crypt, dm-verity side by side."""
    by_cell: dict[tuple[str, str, str, str], dict[str, dict[str, object]]] = {}
    for row in summaries:
        cell = (
            str(row["platform"]),
            str(row["operation"]),
            str(row["block_size"]),
            str(row["iodepth_set"]),
        )
        by_cell.setdefault(cell, {})[str(row["target"])] = row

    fields = [
        "platform",
        "operation",
        "block_size",
        "iodepth_set",
        "raw_bw_median",
        "crypt_bw_median",
        "verity_bw_median",
        "raw_status",
        "crypt_status",
        "verity_status",
        "iodepth_note",
    ]
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", newline="", encoding="utf-8") as target:
        writer = csv.DictWriter(target, fieldnames=fields)
        writer.writeheader()
        for cell, targets in sorted(by_cell.items()):
            qd = cell[3]
            note = (
                "iodepth_set is fio config only; not observed max_in_flight"
                if qd != "1"
                else "qd=1"
            )
            writer.writerow(
                {
                    "platform": cell[0],
                    "operation": cell[1],
                    "block_size": cell[2],
                    "iodepth_set": qd,
                    "raw_bw_median": targets.get("raw", {}).get("bw_mib_s_median", "missing"),
                    "crypt_bw_median": targets.get("dm-crypt", {}).get(
                        "bw_mib_s_median", "missing"
                    ),
                    "verity_bw_median": targets.get("dm-verity", {}).get(
                        "bw_mib_s_median", "missing"
                    ),
                    "raw_status": targets.get("raw", {}).get("status", "missing"),
                    "crypt_status": targets.get("dm-crypt", {}).get("status", "missing"),
                    "verity_status": targets.get("dm-verity", {}).get("status", "missing"),
                    "iodepth_note": note,
                }
            )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "root",
        type=Path,
        nargs="?",
        default=Path("docs/evidence/fio-matrix"),
        help="Matrix evidence root containing per-run.csv files",
    )
    args = parser.parse_args()
    root = args.root
    if not root.is_dir():
        raise SystemExit(f"missing matrix root: {root}")

    rows = load_per_run_csvs(root)
    if not rows:
        # Still emit empty summaries so the package layout is predictable.
        summary_path = root / "summary.csv"
        summary_path.write_text(
            "platform,target,operation,block_size,iodepth_set,status,runs\n",
            encoding="utf-8",
        )
        (root / "summary-three-target.csv").write_text(
            "platform,operation,block_size,iodepth_set,raw_bw_median,"
            "crypt_bw_median,verity_bw_median,raw_status,crypt_status,"
            "verity_status,iodepth_note\n",
            encoding="utf-8",
        )
        print(f"summary_csv={summary_path} (no per-run.csv yet)")
        return

    grouped: dict[tuple[str, ...], list[dict[str, str]]] = {}
    for row in rows:
        for field in ("target", "operation", "block_size", "iodepth_set"):
            if field not in row:
                raise SystemExit(f"row missing {field}: {row}")
        grouped.setdefault(group_key(row), []).append(row)

    summaries = [summarize_group(key, group) for key, group in sorted(grouped.items())]
    summary_path = root / "summary.csv"
    fields = list(summaries[0].keys())
    with summary_path.open("w", newline="", encoding="utf-8") as target:
        writer = csv.DictWriter(target, fieldnames=fields)
        writer.writeheader()
        writer.writerows(summaries)

    three_path = root / "summary-three-target.csv"
    write_three_target_table(three_path, summaries)
    print(f"summary_csv={summary_path}")
    print(f"three_target_csv={three_path}")
    print(f"groups={len(summaries)}")


if __name__ == "__main__":
    main()
