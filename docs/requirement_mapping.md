# 赛题技术指标对应表

本文件按正式题面逐条对应当前工程中的实现、演示方式和仍需在 Asterinas 环境补齐的部分。

## 任务内容

> 在星绽操作系统上，使用 Rust 编程语言实现 dm-crypt 与 dm-verity 功能。

当前工程已将核心逻辑实现为 Rust crate：

- `crates/dm`: Device Mapper、dm-crypt、dm-verity 核心逻辑；
- `crates/dmctl`: 用户态镜像演示工具；
- `asterinas_port`: 接入 Asterinas 的适配层骨架。

在真实参赛环境中，需要把 `crates/dm/src` 迁移到 Asterinas 内核树，并补齐 `VirtioBlkAdapter`。

## 指标 1：dm-verity 基础功能实现

题面要求：

- 构建哈希树；
- 支持 SHA-256 等密码学哈希算法；
- 实现数据块完整性验证机制；
- 支持通过内核启动参数配置：

```text
rootfs_verity.scheme=dm-verity rootfs_verity.hash=<hash>
```

当前对应：

- `crates/dm/src/hash_tree.rs`: Merkle hash tree；
- `crates/dm/src/sha256.rs`: SHA-256；
- `crates/dm/src/dm_verity.rs`: 读取时完整性验证；
- `asterinas_port/cmdline_skeleton.rs`: 启动参数解析骨架；
- `dmctl verity-root`: 生成 root hash；
- `dmctl verity-verify`: 使用 root hash 验证镜像。

待 Asterinas 环境补齐：

- 在内核启动阶段解析 cmdline；
- 将 rootfs 块设备包装为 `DmVerityDevice`；
- 篡改 rootfs 后验证访问失败或启动失败。

## 指标 2：dm-crypt 基础功能实现

题面要求：

- 支持透明加解密 I/O 路径；
- 作为后端支撑用户程序 `cryptsetup` 对加密卷的创建和管理。

当前对应：

- `crates/dm/src/dm_crypt.rs`: 透明加解密 I/O wrapper；
- `DmCryptTable`: Linux dm-crypt table 参数解析；
- `aes-xts-plain64`: AES-256-XTS 与 plain64 IV；
- `scripts/verify_linux_xts_compat.sh`: 与 Linux 内核 dm-crypt 逐字节互操作验证；
- `dmctl crypt-write`: 明文经 mapper 加密写入；
- `dmctl raw-read-hex`: 直接查看底层密文；
- `dmctl crypt-read`: 通过 mapper 恢复明文。

待 Asterinas 环境补齐：

- 提供内核侧控制接口，使 `cryptsetup` 能创建和管理 dm-crypt 设备；
- 将 cryptsetup 解析出的 volume key 与 table 安全传给内核；LUKS metadata 与 PBKDF 保持在用户态。

## 指标 3：根文件系统加密

题面要求：

- 使用 dm-crypt 加解密根文件系统。

当前对应：

- [rootfs_protection.md](rootfs_protection.md) 给出 rootfs 加密和 verity 的接入顺序；
- `asterinas_port/cmdline_skeleton.rs` 预留 `dm_crypt.device` 和 `dm_crypt.key` 参数解析。

待 Asterinas 环境补齐：

- init 阶段创建 `DmCryptDevice`;
- 将 rootfs mount 目标切换到加密 mapper；
- 验证底层 rootfs 镜像不可直接读出明文文件内容。

## 指标 4：性能与启动要求

题面要求：

- dm-crypt 和 dm-verity 的 I/O 吞吐及延迟接近 Linux；
- dm-verity 不显著增加 kernel boot 时间。

当前对应：

- [performance.md](performance.md) 给出 fio/filebench/SQLite 测试方案；
- `scripts/benchmark.sh` 提供 plain/AES-XTS/verity 的 CSV 微基准入口；
- `scripts/linux_cryptsetup_baseline.sh` 提供 Linux 对照实验入口。

待 Asterinas 环境补齐：

- 在 QEMU 中分别测试 plain、dm-crypt、dm-verity；
- 记录 kernel boot 时间；
- 与 Linux dm-crypt/dm-verity 基线对比。

## 指标 5：技术探索

题面要求：

- 探索基于 Rust OS 的可信存储性能优化技术。

当前对应：

- hash tree 验证路径清晰，可加入 hash block cache；
- mapper 接口统一，可加入 I/O batch、zero-copy buffer 和异步块 I/O；
- 文档中已列出优化方向。

建议答辩重点：

1. Rust trait 将安全存储能力模块化；
2. verity hash cache 减少重复计算；
3. crypt 使用硬件 AES 或批量加密降低开销；
4. rootfs 启动阶段只验证读取路径，减少 boot time 影响。
