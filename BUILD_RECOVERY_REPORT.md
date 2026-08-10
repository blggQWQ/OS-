# BUILD RECOVERY REPORT

## 1 根因

最初的 82 个 `ostd` 依赖/架构错误不是 `aster-dm` 算法错误。真实根因是 Asterinas 构建在 Windows host 或错误的父 workspace 中执行，未使用仓库固定的 Linux 容器、nightly toolchain 和 bare-metal targets。Docker Desktop 命名管道拒绝访问及 crates.io 临时 DNS 故障属于独立环境问题，均已解决。

## 2 正确构建环境

- Asterinas commit：`9d79542cadddc6164ad3f31fd8cba776969d85a9`
- 官方镜像：`asterinas/asterinas:0.18.0-20260618`
- Rust toolchain：`nightly-2026-04-03`
- targets：`x86_64-unknown-none`、`riscv64imac-unknown-none-elf`、`loongarch64-unknown-none-softfloat`
- Cargo cache：named volume `asterinas-cargo`
- rustup cache：named volume `asterinas-rustup`
- Asterinas checkout 作为独立 workspace 使用，不从顶层 Cargo workspace 启动。

## 3 正确构建命令

在上述固定容器的 Asterinas workspace 中执行：

```bash
make kernel TARGET_ARCH=x86_64
```

## 4 kernel build PASS/FAIL

**PASS**。x86_64 kernel ISO 成功生成，完整记录见 [make-kernel-x86_64.log](docs/evidence/stabilization/build-recovery/make-kernel-x86_64.log)。

## 5 aster-dm 是否真正被编译

**是**。构建日志包含 `aster-dm` 编译记录，构建目录中的 [README](docs/evidence/stabilization/build-recovery/README.md) 说明了证据范围。

## 6 cryptsetup smoke

**PASS**。R01 完成真实 `cryptsetup open`、mapper 读取和 close。证据见 [cryptsetup-smoke-kvm-retry.log](docs/evidence/stabilization/kvm-smoke/cryptsetup-smoke-kvm-retry.log)。

## 7 crypt rootfs smoke

**PASS**。R02 完成 dm-crypt mapper 根切换并保留 initramfs runtime。证据见 [crypt-rootfs-smoke-kvm.log](docs/evidence/stabilization/kvm-smoke/crypt-rootfs-smoke-kvm.log)。

## 8 verity rootfs smoke

**PASS**。R03 完成 `status=verified`、mapper 挂载和根切换。证据见 [verity-rootfs-smoke-kvm.log](docs/evidence/stabilization/kvm-smoke/verity-rootfs-smoke-kvm.log)。

## 9 修改了哪些文件

- `crates/dm/src/runtime.rs`：拒绝尚未实现的 crypt options。
- `crates/dm/src/dm_control.rs`：不再公布尚未实现的 runtime verity target。
- Asterinas initramfs 与测试工具：增加 KVM correctness 测试入口和证据采集。
- 本报告及测试矩阵、环境说明和验收文档。

## 10 是否修改算法逻辑

**否**。没有修改 AES-XTS、Merkle tree、BIO、virtio、DMA 或 dm-crypt/dm-verity 核心算法。

## 11 当前剩余 P0

构建恢复、R01/R02/R03 及 C01-C05、V01-V07 均已 PASS，当前没有未解决的 correctness P0。最终状态：**CORRECTNESS READY**。

历史环境问题只用于追溯，不代表当前状态；未运行完整 fio 或性能测试。
