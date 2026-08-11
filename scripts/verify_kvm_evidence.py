#!/usr/bin/env python3
"""验证版本化串口日志包含 R01–R03、C01–C05、V02–V07 结果。"""

from __future__ import annotations

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CHECKS = (
    (
        "docs/evidence/stabilization/correctness-kvm/regression/R01-cryptsetup-persistent-cache.log",
        ("PASS real cryptsetup open", "PASS real cryptsetup close"),
        (),
    ),
    (
        "docs/evidence/stabilization/correctness-kvm/regression/R02-crypt-rootfs-persistent-cache.log",
        ("PASS system root switched to protected mapper",),
        (),
    ),
    (
        "docs/evidence/stabilization/correctness-kvm/regression/R03-verity-rootfs-persistent-cache.log",
        ("status=verified", "PASS system root switched to protected mapper"),
        (),
    ),
    (
        "docs/evidence/stabilization/correctness-kvm/crypt/C01-C05-kvm.log",
        tuple(f"PASS C0{case}" for case in range(1, 6)),
        (),
    ),
    (
        "docs/evidence/stabilization/correctness-kvm/corruption/V02-wrong-root.log",
        ("status=integrity-rejected", "mapper device is missing"),
        ("PASS system root switched to protected mapper",),
    ),
    *tuple(
        (
            f"docs/evidence/stabilization/correctness-kvm/corruption/V0{case}-targeted-probe.log",
            (f"RESULT=V0{case}:PASS",),
            ("PASS system root switched to protected mapper", "Kernel panic"),
        )
        for case in range(3, 8)
    ),
)


def main() -> None:
    failures: list[str] = []
    for relative, required, forbidden in CHECKS:
        path = ROOT / relative
        if not path.is_file():
            failures.append(f"{relative}: 文件不存在")
            continue
        text = path.read_text(encoding="utf-8", errors="replace")
        missing = [marker for marker in required if marker not in text]
        unexpected = [marker for marker in forbidden if marker in text]
        if missing:
            failures.append(f"{relative}: 缺少标记 {missing}")
        if unexpected:
            failures.append(f"{relative}: 出现禁止标记 {unexpected}")
    if failures:
        raise SystemExit("KVM 证据验证失败：\n- " + "\n- ".join(failures))
    print(f"PASS：{len(CHECKS)} 组 KVM 串口证据符合预期")


if __name__ == "__main__":
    main()
