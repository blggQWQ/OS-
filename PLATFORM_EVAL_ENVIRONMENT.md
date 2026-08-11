# 平台评测环境

## 最终结论

当前联网固定环境为 **READY**，可以稳定执行 kernel build 与 KVM correctness 测试。

## 固定基线

- Docker image：`asterinas/asterinas:0.18.0-20260618`
- image digest：`sha256:7510071cb103309cc9396b970325c354713da749dfee34a807108248cc97331e`
- Asterinas commit：`9d79542cadddc6164ad3f31fd8cba776969d85a9`
- Rust toolchain：`nightly-2026-04-03`
- Cargo cache volume：`asterinas-cargo`
- rustup cache volume：`asterinas-rustup`
- 架构：`x86_64`
- QEMU：KVM，CPU host

## 已安装的编译目标

- `x86_64-unknown-none`
- `riscv64imac-unknown-none-elf`
- `loongarch64-unknown-none-softfloat`
- `x86_64-unknown-linux-gnu`

## 验证结果

| 检查 | 结果 |
|---|---|
| Cargo metadata locked/offline | PASS |
| Cargo fetch locked/offline | PASS |
| rustup target persistence in a fresh container | PASS |
| kernel build | PASS |
| `aster-dm` compile | PASS |
| R01 cryptsetup | PASS |
| R02 dm-crypt rootfs | PASS |
| R03 dm-verity rootfs | PASS |

完整原始日志位于 `docs/evidence/stabilization/platform-eval/` 和 `docs/evidence/stabilization/correctness-kvm/`。

## 历史问题（已解决）

- `static.crates.io` 临时 DNS 故障：已由持久 Cargo cache 解决。
- 一次性 rustup targets：已由独立 rustup volume 解决。
- R01 曾在 QEMU 前阻塞：已解决，当前 R01-R03 均 PASS。

Nix runtime 在全新容器中可能访问 binary cache；这属于完全离线复现事项，不影响当前联网 correctness 结论，也不是产品 correctness FAIL。
