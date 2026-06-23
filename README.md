# 基于 Rust OS 的 dm-crypt / dm-verity 参赛实现

本项目面向 CSCC/国赛题目“基于 Rust OS 的 dm-crypt 与 dm-verity”，提供一套可演示、可测试、可继续接入 Asterinas 的完整参赛工程包。

当前版本完成了：

1. 基于 `BlockDevice` trait 的可组合 Device Mapper 框架；
2. Linux 兼容的 `aes-xts-plain64` 加密路径与 dm-crypt table 解析；
3. `dm-verity` 只读完整性校验；
4. 文件镜像后端，可直接演示密文落盘与篡改检测；
5. 命令行工具 `dmctl`；
6. Asterinas `aster-dm` 内核组件、真实 VirtIO BIO 验证、测试与答辩材料。

正式题面逐条对应见 [docs/requirement_mapping.md](docs/requirement_mapping.md)。

> 环境已完成：Ubuntu 24.04 WSL、Rust/Cargo、QEMU、Docker Desktop 和 Asterinas 官方开发镜像均已安装。`cargo test`、`make kernel`、Asterinas dm-crypt 解密与 dm-verity 正反例均已验证通过。

## 目录结构

```text
Cargo.toml
crates/
  dm/                         核心库：BlockDevice、dm-crypt、dm-verity
  dmctl/                      命令行演示工具
docs/
  design.md                   设计文档
  test.md                     测试说明
  performance.md              性能测试方案
  asterinas_integration.md    Asterinas 接入步骤
  final_report.md             实验报告模板
  defense_outline.md          答辩提纲
  requirement_mapping.md      赛题指标对应表
  rootfs_protection.md        rootfs 加密与校验方案
  cryptsetup_compatibility.md cryptsetup 兼容路线
asterinas_port/
  README.md                   内核接入说明
  adapter_skeleton.rs         virtio-blk 适配层骨架
  cmdline_skeleton.rs         启动参数解析骨架
scripts/
  run_tests.ps1               Windows 测试入口
  demo_crypt.ps1              Windows dm-crypt 演示
  demo_verity.ps1             Windows dm-verity 演示
  build_image.sh              Linux 测试镜像创建
  run_qemu.sh                 QEMU 启动模板
  benchmark.sh                Rust plain/crypt/verity CSV 基准
  verify_linux_xts_compat.sh  与 Linux dm-crypt 逐字节兼容验证
  linux_cryptsetup_baseline.sh Linux cryptsetup 对照基线
artifacts/
  asterinas-kernel.iso        已成功构建的 Asterinas 内核 ISO
```

## 快速验证

安装 Rust 后，在项目根目录运行：

```powershell
cargo test
```

运行两个一键演示：

```powershell
cargo run -p dmctl -- demo-crypt
cargo run -p dmctl -- demo-verity
```

`demo-crypt` 会展示：

- 上层写入明文；
- 底层镜像保存的是十六进制密文；
- 通过 `DmCryptDevice` 读回仍是明文。

`demo-verity` 会展示：

- 正常镜像生成 root hash；
- 手动篡改底层块；
- 通过 `DmVerityDevice` 读取时返回完整性错误。

## 手动演示命令

```powershell
cargo run -p dmctl -- init-image target/demo.img 8 64
cargo run -p dmctl -- crypt-write target/demo.img 64 my-secret-key 1 "hello dm-crypt"
cargo run -p dmctl -- raw-read-hex target/demo.img 64 1
cargo run -p dmctl -- crypt-read target/demo.img 64 my-secret-key 1
```

verity 演示：

```powershell
$root = cargo run -p dmctl -- verity-root target/demo.img 64
cargo run -p dmctl -- verity-verify target/demo.img 64 $root
cargo run -p dmctl -- tamper target/demo.img 64 1 0 ff
cargo run -p dmctl -- verity-verify target/demo.img 64 $root
```

最后一个命令预期失败，说明篡改被检测到。

## 核心设计

统一抽象：

```rust
pub trait BlockDevice {
    fn block_size(&self) -> usize;
    fn num_blocks(&self) -> u64;
    fn read_block(&self, block_id: u64, buf: &mut [u8]) -> Result<()>;
    fn write_block(&self, block_id: u64, buf: &[u8]) -> Result<()>;
}
```

映射链路：

```text
应用 / 文件系统
        -> DmCryptDevice / DmVerityDevice
        -> BlockDevice
        -> FileBlockDevice / Asterinas virtio-blk
```

## 当前实现边界

- `dm-crypt` 已支持 Linux `aes-xts-plain64`、64 字节 AES-256-XTS key、`iv_offset` 和 data offset；旧的 SHA-256 流变换仅保留给最小演示命令。
- `scripts/verify_linux_xts_compat.sh` 会让 Linux dm-crypt 与 Rust 实现加密同一扇区，并执行逐字节比较。当前固定向量的两端 SHA-256 均为 `4334c17f4b0380b3c1f902509cce0c0f98d9fa77aa93f6407ec940215d77921d`。
- 用户态 `dm` crate 的 verity tree 保存在内存中；Asterinas `aster-dm` 使用镜像尾部的 on-disk hash tree。
- Asterinas 补丁中的 `aster-dm` 已接入真实 VirtIO BIO：dm-crypt 能解密 Linux `aes-xts-plain64` 生成的 ext2；dm-verity 使用 on-disk hash tree，正常镜像通过且篡改镜像返回 `IntegrityViolation`。
- 已新增 `dm-root` init 模式：从 crypt/verity mapper 挂载 ext2 后通过
  `pivot_root` 切换系统 `/`。当前提交已通过完整内核构建和 mapper BIO
  启动验证；根切换最终串口 PASS 日志仍需在后续提交中补录。

## 决赛级验证入口

```bash
# 正确性测试
cargo test --workspace

# Rust 普通 / AES-XTS / verity，输出 CSV
bash scripts/benchmark.sh

# 需要 root；与 Linux 内核 dm-crypt 逐字节对照
sudo bash scripts/verify_linux_xts_compat.sh

# 构建 Linux dm-crypt 兼容的加密 ext2 测试盘（WSL root）
sudo bash scripts/build_crypt_rootfs.sh
```

## 国赛提交建议

提交前至少补齐这些证据：

1. `cargo test` 全部通过截图；
2. `demo-crypt` 输出截图；
3. `demo-verity` 篡改检测截图；
4. Asterinas + QEMU 启动日志；
5. Asterinas 识别 virtio-blk 日志；
6. 接入 `VirtioBlkAdapter` 后的读写日志；
7. 性能对比表：plain / dm-crypt / dm-verity。

更多细节见 [docs/final_report.md](docs/final_report.md) 和 [docs/defense_outline.md](docs/defense_outline.md)。
