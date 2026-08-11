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
    "docs/evidence/final-performance-20260811/PERFORMANCE_VALIDATION_REPORT.md",
    "docs/evidence/final-performance-20260811/PERFORMANCE_MATRIX.csv",
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
    root = ROOT / "docs/evidence/final-performance-20260811"
    matrix = root / "PERFORMANCE_MATRIX.csv"
    if not matrix.is_file():
        fail("missing final PERFORMANCE_MATRIX.csv")
        return
    with matrix.open(newline="", encoding="utf-8-sig") as source:
        rows = list(csv.DictReader(source))
    expected = {
        ("asterinas-kvm", "dm-crypt"): 13,
        ("asterinas-kvm", "dm-verity"): 7,
        ("linux-host", "raw"): 13,
        ("linux-host", "dm-crypt"): 13,
    }
    for key, count in expected.items():
        actual = sum(
            row.get("status") == "PASS"
            and (row.get("platform"), row.get("target")) == key
            for row in rows
        )
        if actual != count:
            fail(f"final FIO matrix {key}: expected {count} PASS cells, found {actual}")
    if sum(str(row.get("status", "")).startswith("N/A") for row in rows) != 6:
        fail("final FIO matrix must contain 6 read-only N/A cells")
    forbidden = ("/dm-crypt/", "invalid", "anomaly-audit")
    for row in rows:
        evidence = row.get("raw_evidence_path", "").lower()
        if row.get("platform") == "asterinas-kvm" and any(
            marker in evidence for marker in forbidden
        ):
            fail(f"final FIO matrix references excluded evidence: {evidence}")


def check_boot_matrix() -> None:
    boot = ROOT / "docs/evidence/final-performance-20260811/boot-matrix"
    csv_path = boot / "boot-time.csv"
    if not csv_path.is_file():
        fail("final boot-matrix/boot-time.csv is missing")
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
    print("NOTE: 已检查最终 FIO 矩阵、启动矩阵、必要脚本和文档。")
    return 0


if __name__ == "__main__":
    sys.exit(main())
