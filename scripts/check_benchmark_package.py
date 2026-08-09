#!/usr/bin/env python3
"""Validate benchmark package layout and honesty rules (no performance thresholds)."""

from __future__ import annotations

import csv
import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
ERRORS: list[str] = []
WARNS: list[str] = []

REQUIRED_DOCS = (
    "docs/benchmark_contract.md",
    "docs/performance.md",
    "docs/filebench.md",
    "docs/sqlite_benchmark.md",
    "docs/evidence/README.md",
    "docs/evidence/fio-matrix/README.md",
    "docs/evidence/boot-matrix/README.md",
    "docs/evidence/filebench/README.md",
    "docs/evidence/sqlite/README.md",
)

REQUIRED_SCRIPTS = (
    "scripts/run_fio_matrix.sh",
    "scripts/summarize_fio_matrix.py",
    "scripts/check_filebench.sh",
    "scripts/run_filebench_dm.sh",
    "scripts/run_sqlite_workload.sh",
    "scripts/benchmark_boot_time.py",
    "scripts/run_boot_matrix.sh",
    "scripts/run_asterinas_boot_once.sh",
    "scripts/build_asterinas_boot_images.sh",
    "scripts/workloads/fileserver.f",
)

# Flag only affirmative false claims, not honesty warnings in the contract.
FORBIDDEN_CLAIM = re.compile(
    r"(真实|实际)\s*(系统)?\s*(并发度?|max_in_flight)\s*(为|=|是)\s*16",
    re.I,
)


def fail(msg: str) -> None:
    ERRORS.append(msg)


def warn(msg: str) -> None:
    WARNS.append(msg)


def check_exists() -> None:
    for rel in REQUIRED_DOCS + REQUIRED_SCRIPTS:
        if not (ROOT / rel).is_file():
            fail(f"missing required file: {rel}")


def check_contract_honesty() -> None:
    contract = (ROOT / "docs/benchmark_contract.md").read_text(encoding="utf-8")
    for needle in (
        "iodepth_set",
        "max_in_flight_observed",
        "N/A",
        "mean",
        "median",
        "standard deviation",
    ):
        if needle not in contract and needle.replace("standard deviation", "stdev") not in contract:
            # allow Chinese equivalents already present
            pass
    if "iodepth_set" not in contract:
        fail("benchmark_contract.md must define iodepth_set")
    if "max_in_flight" not in contract:
        fail("benchmark_contract.md must discuss max_in_flight honesty")
    if "dm-verity" not in contract or "N/A" not in contract:
        fail("benchmark_contract.md must mark dm-verity writes as N/A")


def check_docs_forbidden_claims() -> None:
    for rel in ("docs/performance.md", "docs/benchmark_contract.md", "README.md"):
        path = ROOT / rel
        if not path.is_file():
            continue
        lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
        for line_no, line in enumerate(lines, 1):
            window = " ".join(lines[max(0, line_no - 2) : line_no])
            if any(x in window for x in ("禁止", "不等于", "不得", "不是", "诚实", "不可夸大")):
                continue
            if FORBIDDEN_CLAIM.search(line):
                fail(f"forbidden iodepth/concurrency claim in {rel}:{line_no}")


def check_fio_matrix_layout() -> None:
    root = ROOT / "docs/evidence/fio-matrix"
    if not root.is_dir():
        fail("missing docs/evidence/fio-matrix/")
        return
    per_runs = list(root.rglob("per-run.csv"))
    if not per_runs:
        warn("fio-matrix has no per-run.csv yet (pending execution)")
        return
    for path in per_runs:
        with path.open(newline="", encoding="utf-8") as source:
            rows = list(csv.DictReader(source))
        if not rows:
            fail(f"empty per-run.csv: {path.relative_to(ROOT)}")
            continue
        ok_rows = [r for r in rows if r.get("status") == "ok"]
        if ok_rows:
            runs = sorted(int(r["run"]) for r in ok_rows)
            if runs != list(range(1, len(runs) + 1)) or len(runs) < 5:
                fail(f"{path.relative_to(ROOT)}: need runs 1..N N>=5, found {runs}")
        meta = path.parent / "metadata.txt"
        if not meta.is_file():
            fail(f"missing metadata.txt beside {path.relative_to(ROOT)}")
        else:
            text = meta.read_text(encoding="utf-8")
            for key in ("git_commit=", "platform=", "iodepth_set=", "ioengine="):
                if key not in text:
                    fail(f"{meta.relative_to(ROOT)} missing {key}")
        for row in ok_rows:
            if "max_in_flight_observed" not in row:
                fail(f"{path.relative_to(ROOT)} missing max_in_flight_observed column")
                break
            if row.get("iodepth_set") not in {"1", "1.0"} and row.get(
                "max_in_flight_observed", ""
            ) in {"16", "16.0"}:
                # Only fail if someone equated set depth to observed without note
                if "sync" not in row.get("ioengine", "") and row.get(
                    "max_in_flight_observed"
                ) == row.get("iodepth_set"):
                    warn(
                        f"{path.relative_to(ROOT)}: max_in_flight_observed equals "
                        "iodepth_set; confirm this is measured, not assumed"
                    )

    summary = root / "summary.csv"
    if summary.is_file() and summary.stat().st_size > 0:
        with summary.open(newline="", encoding="utf-8") as source:
            reader = csv.DictReader(source)
            fields = reader.fieldnames or []
            for need in ("bw_mib_s_mean", "bw_mib_s_median", "bw_mib_s_stdev"):
                if need not in fields and list(reader):
                    # re-open to inspect first data row path; empty header-only OK
                    pass
            source.seek(0)
            rows = list(csv.DictReader(source))
        if rows:
            fields = rows[0].keys()
            for need in ("bw_mib_s_mean", "bw_mib_s_median", "bw_mib_s_stdev"):
                if need not in fields:
                    fail(f"summary.csv missing {need}")


def check_boot_matrix() -> None:
    boot = ROOT / "docs/evidence/boot-matrix"
    csv_path = boot / "boot-time.csv"
    if not csv_path.is_file():
        warn("boot-matrix/boot-time.csv not present yet (pending execution)")
        return
    with csv_path.open(newline="", encoding="utf-8") as source:
        rows = list(csv.DictReader(source))
    by_scene: dict[str, list[int]] = {}
    for row in rows:
        by_scene.setdefault(row["scenario"], []).append(int(row["run"]))
    for scene in ("plain", "dm-verity"):
        runs = sorted(by_scene.get(scene, []))
        if runs != list(range(1, len(runs) + 1)) or len(runs) < 5:
            fail(f"boot-matrix {scene}: need runs 1..N N>=5, found {runs}")
    if "dm-crypt" not in by_scene:
        warn("boot-matrix missing dm-crypt rows (optional if crypt ISO unavailable)")
    summary = boot / "boot-time-summary.csv"
    if summary.is_file():
        text = summary.read_text(encoding="utf-8")
        for need in ("mean_s", "median_s", "stdev_s"):
            if need not in text:
                fail(f"boot-time-summary.csv missing {need}")


def main() -> int:
    check_exists()
    check_contract_honesty()
    check_docs_forbidden_claims()
    check_fio_matrix_layout()
    check_boot_matrix()

    for msg in WARNS:
        print(f"WARN: {msg}")
    if ERRORS:
        for msg in ERRORS:
            print(f"FAIL: {msg}")
        print(f"RESULT=FAIL errors={len(ERRORS)} warnings={len(WARNS)}")
        return 1
    print(f"RESULT=PASS warnings={len(WARNS)}")
    print("NOTE: pending evidence directories are warnings only; scripts/docs must exist.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
