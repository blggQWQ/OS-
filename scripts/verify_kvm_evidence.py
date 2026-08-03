#!/usr/bin/env python3
"""Verify that versioned serial logs contain the claimed KVM outcomes."""

from __future__ import annotations

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CHECKS = (
    (
        "docs/evidence/asterinas-crypt-rootfs-aesni-kvm.log",
        ("AES backend=AES-NI", "PASS system root switched to protected mapper"),
        (),
    ),
    (
        "docs/evidence/asterinas-cryptsetup-aesni-kvm.log",
        ("cryptsetup", "PASS"),
        (),
    ),
    (
        "docs/evidence/asterinas-verity-rootfs-kvm.log",
        ("status=verified", "PASS system root switched to protected mapper"),
        (),
    ),
    (
        "docs/evidence/asterinas-verity-rootfs-tampered-kvm.log",
        ("status=integrity-rejected",),
        ("PASS system root switched to protected mapper",),
    ),
)


def main() -> None:
    failures: list[str] = []
    for relative, required, forbidden in CHECKS:
        path = ROOT / relative
        if not path.is_file():
            failures.append(f"{relative}: missing")
            continue
        text = path.read_text(encoding="utf-8", errors="replace")
        missing = [marker for marker in required if marker not in text]
        unexpected = [marker for marker in forbidden if marker in text]
        if missing:
            failures.append(f"{relative}: missing markers {missing}")
        if unexpected:
            failures.append(f"{relative}: forbidden markers {unexpected}")
    if failures:
        raise SystemExit("KVM evidence validation failed:\n- " + "\n- ".join(failures))
    print(f"PASS: {len(CHECKS)} versioned KVM serial logs match the expected outcomes")


if __name__ == "__main__":
    main()
