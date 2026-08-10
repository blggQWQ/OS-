# KVM CORRECTNESS REPORT

## 最终结论

**CORRECTNESS READY**

固定官方镜像、持久 Cargo/rustup cache、Asterinas commit `9d79542cadddc6164ad3f31fd8cba776969d85a9` 与 `nightly-2026-04-03` 下，kernel build、R01-R03、C01-C05 和 V01-V07 均已真实执行并 PASS。环境下载问题已经解决，不再与当前 correctness 状态混写。

## 固定基线

- kernel：真实 Asterinas x86_64 kernel，`aster-dm` 已编译。
- 虚拟化：QEMU/KVM。
- build command：`make kernel TARGET_ARCH=x86_64`。
- crypt evidence：[C01-C05-kvm.log](docs/evidence/stabilization/correctness-kvm/crypt/C01-C05-kvm.log)。
- verity evidence：[corruption evidence](docs/evidence/stabilization/correctness-kvm/corruption/)。
- 物理位置与前后字节：[PHYSICAL_MUTATION_MANIFEST.md](PHYSICAL_MUTATION_MANIFEST.md)。

## R01-R03 gate

| Case | 结果 | 验证内容 |
|---|---|---|
| R01 | PASS | cryptsetup open、mapper read、close |
| R02 | PASS | dm-crypt protected rootfs boot |
| R03 | PASS | dm-verity verified rootfs boot |

## dm-crypt P0

| Case | 结果 | 证据结论 |
|---|---|---|
| C01 | PASS | correct key 下 mapper 写入并读回，SHA-256 相同 |
| C02 | PASS | wrong key 不能交付正确明文，SHA-256 不同 |
| C03 | PASS | 4 KiB mapper write/read round-trip |
| C04 | PASS | 32 KiB mapper write/read round-trip |
| C05 | PASS | 256 KiB mapper write/read round-trip |

C01-C05 均经过真实 mapper 数据路径，原始串口日志记录 commit、kernel cmdline、命令以及写入/读回 hash。

## dm-verity P0

| Case | 结果 | 篡改位置 | 结果 |
|---|---|---|---|
| V01 | PASS | 无篡改 | 正确 root hash 正常读取 |
| V02 | PASS | root hash | 错误 root hash 被拒绝 |
| V03 | PASS | 首数据块 | integrity rejection / IoError |
| V04 | PASS | 中间数据块 | integrity rejection / IoError |
| V05 | PASS | 末数据块 | integrity rejection / IoError |
| V06 | PASS | leaf hash block | integrity rejection / IoError |
| V07 | PASS | intermediate hash block | integrity rejection / IoError |

V03-V07 均证明修改了预期物理区域，继续使用原始正确 root hash；坏数据没有作为成功数据交付，没有 panic，也没有 fallback 到 lower device。

## 未执行范围

C06-C15、V08-V12、H01-H08 和 P01 属于后续 P1，当前保持 **UNTESTED**，不得解释为 PASS 或产品 FAIL。完整状态见 [KVM_CORRECTNESS_MATRIX.md](KVM_CORRECTNESS_MATRIX.md)。

## 产品代码约束

本轮没有新增功能，没有性能优化，没有修改 AES-XTS、Merkle tree、BIO、virtio、DMA，也没有降低验证条件。
