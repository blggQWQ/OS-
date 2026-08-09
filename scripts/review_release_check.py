#!/usr/bin/env python3
"""Fail closed on unsupported public claims before creating a review tag."""

from __future__ import annotations

import csv
import re
import subprocess
import urllib.parse
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
REQUIRED_EVIDENCE = (
    "docs/evidence/asterinas-dm-crypt-kvm.csv",
    "docs/evidence/asterinas-dm-crypt-kvm-summary.csv",
    "docs/evidence/asterinas-verity-fio-cache0-kvm.csv",
    "docs/evidence/asterinas-verity-fio-cache256-kvm.csv",
    "docs/evidence/FINAL_VALIDATION.md",
    "docs/evidence/linux-xts-plain64.sha256",
    "docs/evidence/linux-guest-dm-crypt-kvm.csv",
    "docs/evidence/linux-guest-dm-crypt-kvm.metadata.txt",
    "docs/evidence/asterinas-dm-crypt-kvm-current.csv",
    "docs/evidence/asterinas-dm-crypt-kvm-current.metadata.txt",
    "docs/evidence/linux-vs-asterinas-dm-crypt.csv",
    "docs/evidence/linux-vs-asterinas-dm-crypt.md",
    "docs/evidence/asterinas-boot-time-kvm.csv",
    "docs/evidence/asterinas-boot-time-kvm-summary.csv",
)
DELIVERY_FILES = (
    ".gitattributes",
    "LICENSE",
    "presentation_project.pptx",
    "scripts/final_demo.sh",
    "scripts/run_formal_dm_benchmark.sh",
    "scripts/run_dm_stats_calibration.sh",
    "scripts/summarize_dm_performance_stages.py",
    "scripts/build_unified_dm_table.py",
    "scripts/compare_dm_stats_overhead.py",
)
FORMAL_EVIDENCE = tuple(
    f"docs/evidence/formal-256m/{name}"
    for name in (
        "dm-crypt.log",
        "dm-crypt.csv",
        "dm-crypt-summary.csv",
        "dm-crypt-stages.csv",
        "dm-crypt.metadata.txt",
        "dm-verity.log",
        "dm-verity.csv",
        "dm-verity-summary.csv",
        "dm-verity-stages.csv",
        "dm-verity.metadata.txt",
        "raw-crypt-verity.csv",
    )
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
TEXT_SUFFIXES = {
    ".bib",
    ".csv",
    ".gdb",
    ".lock",
    ".md",
    ".ps1",
    ".py",
    ".rs",
    ".sh",
    ".tex",
    ".toml",
    ".yaml",
    ".yml",
}
MARKDOWN_LINK = re.compile(r"!?\[[^\]]*]\(([^)]+)\)")


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


def validate_boot_runs(relative: str) -> None:
    path = ROOT / relative
    with path.open(newline="", encoding="utf-8") as source:
        rows = list(csv.DictReader(source))
    grouped: dict[str, list[int]] = {}
    for row in rows:
        grouped.setdefault(row["scenario"], []).append(int(row["run"]))
        log = ROOT / row["log"]
        if not log.is_file():
            fail(f"{relative}: missing per-run log: {row['log']}")
        if row["marker"] not in log.read_text(encoding="utf-8", errors="replace"):
            fail(f"{relative}: marker missing from {row['log']}")
    for scenario in ("plain", "dm-verity"):
        runs = grouped.get(scenario, [])
        if sorted(runs) != [1, 2, 3, 4, 5]:
            fail(
                f"{relative}: {scenario} must contain runs 1..5, found {sorted(runs)}"
            )


def validate_formal_evidence() -> None:
    present = [relative for relative in FORMAL_EVIDENCE if (ROOT / relative).is_file()]
    if not present:
        return
    missing = [relative for relative in FORMAL_EVIDENCE if not (ROOT / relative).is_file()]
    if missing:
        fail("partial formal 256 MiB evidence set: " + ", ".join(missing))
    for target in ("dm-crypt", "dm-verity"):
        validate_five_runs(f"docs/evidence/formal-256m/{target}.csv")
        metadata = {}
        metadata_path = ROOT / f"docs/evidence/formal-256m/{target}.metadata.txt"
        for line in metadata_path.read_text(encoding="utf-8").splitlines():
            if "=" in line:
                key, value = line.split("=", 1)
                metadata[key] = value
        required = {
            "git_dirty": "0",
            "build_profile": "release-lto",
            "accelerator": "kvm",
            "cpu_model": "host",
            "block_size": "4k",
            "ioengine": "sync",
            "direct": "1",
            "queue_depth": "1",
            "size_mib": "256",
            "ramp_seconds": "2",
            "runtime_seconds": "10",
            "runs": "5",
            "stage_stats": "1",
        }
        for key, expected in required.items():
            if metadata.get(key) != expected:
                fail(f"{metadata_path.relative_to(ROOT)}: expected {key}={expected}")
        log_path = ROOT / f"docs/evidence/formal-256m/{target}.log"
        log_text = log_path.read_text(encoding="utf-8", errors="replace")
        if (
            "dirty=0" not in log_text
            or "profile=release-lto" not in log_text
            or "stage_stats=1" not in log_text
        ):
            fail(f"{log_path.relative_to(ROOT)}: build/stats marker missing")
        if target == "dm-crypt" and not re.search(
            r"(?:AES\s+)?backend[=:]AES-NI\b", log_text
        ):
            fail(f"{log_path.relative_to(ROOT)}: AES-NI marker missing")
        stage_path = ROOT / f"docs/evidence/formal-256m/{target}-stages.csv"
        with stage_path.open(newline="", encoding="utf-8") as source:
            stage_rows = list(csv.DictReader(source))
        grouped: dict[str, list[int]] = {}
        for row in stage_rows:
            grouped.setdefault(row["label"], []).append(int(row["run"]))
        for label in ("raw", target):
            if sorted(grouped.get(label, [])) != [1, 2, 3, 4, 5]:
                fail(f"{stage_path.relative_to(ROOT)}: {label} must contain runs 1..5")


def git_lines(*args: str) -> list[str]:
    result = subprocess.run(
        ["git", *args],
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=False,
    )
    if result.returncode:
        fail(result.stdout + result.stderr)
    return result.stdout.splitlines()


def validate_repository_hygiene() -> list[str]:
    tracked = git_lines("ls-files")
    generated = [
        path
        for path in tracked
        if Path(path).suffix in {".aux", ".bbl", ".blg", ".out"}
    ]
    if generated:
        fail("tracked LaTeX build products: " + ", ".join(generated))
    if "docs/document/presentation_project.pptx" in tracked:
        fail("duplicate presentation must not be tracked under docs/document")

    for relative in tracked:
        path = ROOT / relative
        if not path.is_file() or path.suffix.lower() not in TEXT_SUFFIXES:
            continue
        data = path.read_bytes()
        if b"\r\n" in data:
            fail(f"{relative}: CRLF found in a repository text file; expected LF")
    return tracked


def validate_markdown_links(tracked: list[str]) -> None:
    markdown_paths = [ROOT / path for path in tracked if path.endswith(".md")]
    markdown_paths.extend(
        path
        for path in (ROOT / "docs/technical_defense_guide.md",)
        if path.is_file() and path not in markdown_paths
    )
    for markdown in markdown_paths:
        if not markdown.is_file():
            continue
        text = markdown.read_text(encoding="utf-8")
        for match in MARKDOWN_LINK.finditer(text):
            raw = match.group(1).strip().split(maxsplit=1)[0].strip("<>")
            if not raw or raw.startswith(("#", "http://", "https://", "mailto:")):
                continue
            target = urllib.parse.unquote(raw.split("#", 1)[0])
            if target and not (markdown.parent / target).exists():
                fail(f"{markdown.relative_to(ROOT)}: broken local link: {raw}")


def main() -> None:
    for relative in (*REQUIRED_EVIDENCE, *DELIVERY_FILES):
        if not (ROOT / relative).is_file():
            fail(f"missing delivery file: {relative}")
    for relative in (
        "docs/evidence/asterinas-dm-crypt-kvm.csv",
        "docs/evidence/linux-guest-dm-crypt-kvm.csv",
        "docs/evidence/asterinas-dm-crypt-kvm-current.csv",
        "docs/evidence/asterinas-verity-fio-cache0-kvm.csv",
        "docs/evidence/asterinas-verity-fio-cache256-kvm.csv",
    ):
        validate_five_runs(relative)
    validate_boot_runs("docs/evidence/asterinas-boot-time-kvm.csv")
    validate_formal_evidence()
    for relative in PUBLIC_DOCS:
        text = (ROOT / relative).read_text(encoding="utf-8")
        for pattern in FORBIDDEN_CLAIMS:
            if pattern.search(text):
                fail(f"unsupported public claim in {relative}: {pattern.pattern}")
    tracked = validate_repository_hygiene()
    validate_markdown_links(tracked)
    diff_check = subprocess.run(
        ["git", "diff", "--check", "--ignore-submodules=all"],
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=False,
    )
    if diff_check.returncode:
        fail(diff_check.stdout + diff_check.stderr)
    print(
        "PASS: evidence, five-run structure, claims, links, LF policy, "
        "repository hygiene, and diff check"
    )


if __name__ == "__main__":
    main()
