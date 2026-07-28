#!/usr/bin/env python3
"""Fail closed on unsupported public claims before creating a review tag."""

from __future__ import annotations

import csv
import re
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
REQUIRED_EVIDENCE = (
    "docs/evidence/asterinas-dm-crypt-kvm.csv",
    "docs/evidence/asterinas-dm-crypt-kvm-summary.csv",
    "docs/evidence/asterinas-verity-fio-cache0-kvm.csv",
    "docs/evidence/asterinas-verity-fio-cache256-kvm.csv",
    "docs/evidence/FINAL_VALIDATION.md",
)
PUBLIC_DOCS = (
    "README.md",
    "docs/performance.md",
    "docs/asterinas_dm_crypt_benchmark.md",
)
FORBIDDEN_CLAIMS = (
    re.compile(r"85\s*[%％]\s*[–—~-]\s*90\s*[%％]"),
    re.compile(r"Asterinas.{0,30}(?:达到|相当于).{0,20}Linux.{0,20}\d+\s*[%％]"),
)


def fail(message: str) -> None:
    raise SystemExit(f"FAIL: {message}")


def validate_five_runs(relative: str) -> None:
    path = ROOT / relative
    with path.open(newline="", encoding="utf-8") as source:
        rows = list(csv.DictReader(source))
    grouped: dict[str, list[int]] = {}
    for row in rows:
        grouped.setdefault(row["label"], []).append(int(row["run"]))
    for label, runs in grouped.items():
        if sorted(runs) != [1, 2, 3, 4, 5]:
            fail(f"{relative}: {label} must contain runs 1..5, found {sorted(runs)}")


def main() -> None:
    for relative in REQUIRED_EVIDENCE:
        if not (ROOT / relative).is_file():
            fail(f"missing evidence file: {relative}")
    for relative in (
        "docs/evidence/asterinas-dm-crypt-kvm.csv",
        "docs/evidence/asterinas-verity-fio-cache0-kvm.csv",
        "docs/evidence/asterinas-verity-fio-cache256-kvm.csv",
    ):
        validate_five_runs(relative)
    for relative in PUBLIC_DOCS:
        text = (ROOT / relative).read_text(encoding="utf-8")
        for pattern in FORBIDDEN_CLAIMS:
            if pattern.search(text):
                fail(f"unsupported public claim in {relative}: {pattern.pattern}")
    diff_check = subprocess.run(
        ["git", "diff", "--check"],
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=False,
    )
    if diff_check.returncode:
        fail(diff_check.stdout + diff_check.stderr)
    print("PASS: required evidence, five-run CSV structure, claim scan, and diff check")
    print("NOTE: Linux/Asterinas and boot-time CSVs remain release gates until measured.")


if __name__ == "__main__":
    main()
