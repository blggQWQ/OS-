#!/usr/bin/env python3
"""在提交评审版本前检查最终证据和公开结论。"""

from __future__ import annotations

import csv
import re
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
MATRIX = ROOT / "docs/evidence/final-performance-20260811/PERFORMANCE_MATRIX.csv"
REQUIRED_EVIDENCE = (
    "docs/evidence/stabilization/build-recovery/make-kernel-x86_64.log",
    "docs/evidence/stabilization/correctness-kvm/regression/R01-cryptsetup-persistent-cache.log",
    "docs/evidence/stabilization/correctness-kvm/regression/R02-crypt-rootfs-persistent-cache.log",
    "docs/evidence/stabilization/correctness-kvm/regression/R03-verity-rootfs-persistent-cache.log",
    "docs/evidence/stabilization/correctness-kvm/crypt/C01-C05-kvm.log",
    "docs/evidence/stabilization/correctness-kvm/corruption/V02-wrong-root.log",
    "docs/evidence/stabilization/correctness-kvm/corruption/V03-targeted-probe.log",
    "docs/evidence/stabilization/correctness-kvm/corruption/V04-targeted-probe.log",
    "docs/evidence/stabilization/correctness-kvm/corruption/V05-targeted-probe.log",
    "docs/evidence/stabilization/correctness-kvm/corruption/V06-targeted-probe.log",
    "docs/evidence/stabilization/correctness-kvm/corruption/V07-targeted-probe.log",
    "docs/evidence/final-performance-20260811/PERFORMANCE_VALIDATION_REPORT.md",
    "docs/evidence/final-performance-20260811/PERFORMANCE_MATRIX.csv",
)
PUBLIC_DOCS = ("README.md", "docs/performance.md")
FORBIDDEN_CLAIMS = (
    re.compile(r"85\s*[%％]\s*[–—~-]\s*90\s*[%％]"),
    re.compile(r"Asterinas.{0,30}(?:达到|相当于).{0,20}Linux.{0,20}\d+\s*[%％]"),
)


def fail(message: str) -> None:
    raise SystemExit(f"FAIL: {message}")


def main() -> None:
    for relative in REQUIRED_EVIDENCE:
        if not (ROOT / relative).is_file():
            fail(f"缺少必要证据：{relative}")

    with MATRIX.open(encoding="utf-8-sig", newline="") as source:
        rows = list(csv.DictReader(source))
    pass_rows = [row for row in rows if row.get("status") == "PASS"]
    if len(pass_rows) != 46:
        fail(f"最终 FIO 矩阵应有 46 个 PASS 单元格，实际为 {len(pass_rows)}")
    for row in pass_rows:
        if int(row["round_count"]) != 5:
            fail(f"FIO 单元格不是 5 轮：{row}")
        evidence = ROOT / "docs/evidence/final-performance-20260811" / row[
            "raw_evidence_path"
        ]
        if not evidence.is_file():
            fail(f"FIO 原始证据不存在：{evidence.relative_to(ROOT)}")

    for relative in PUBLIC_DOCS:
        text = (ROOT / relative).read_text(encoding="utf-8")
        for pattern in FORBIDDEN_CLAIMS:
            if pattern.search(text):
                fail(f"公开文档包含无证据结论：{relative}: {pattern.pattern}")

    diff_check = subprocess.run(
        ["git", "diff", "--check"],
        cwd=ROOT,
        text=True,
        encoding="utf-8",
        errors="replace",
        capture_output=True,
        check=False,
    )
    if diff_check.returncode:
        fail(diff_check.stdout + diff_check.stderr)
    print("PASS：最终证据、五轮 FIO、公开结论和 diff 检查均通过")


if __name__ == "__main__":
    main()
