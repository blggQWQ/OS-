# Acceptance Checklist

- [x] 固定 Docker image digest
- [x] 固定 Asterinas commit 与 Rust toolchain
- [x] Cargo cache 可持久复用
- [x] rustup targets 可持久复用
- [x] kernel build PASS
- [x] `aster-dm` 实际编译
- [x] R01/R02/R03 KVM gate PASS
- [x] C01 correct-key mapper write/read
- [x] C02 wrong-key plaintext rejection
- [x] C03/C04/C05 4 KiB、32 KiB、256 KiB round-trip
- [x] V01/V02 correct/wrong root
- [x] V03/V04/V05 data corruption rejection
- [x] V06 leaf hash corruption rejection
- [x] V07 intermediate hash corruption rejection
- [x] corruption case 不交付坏数据
- [x] corruption case 无 panic、root switch 或 lower-device fallback
- [x] 产品核心算法未因测试修改

最终验收结论：**CORRECTNESS READY**。
