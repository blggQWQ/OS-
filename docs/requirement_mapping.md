# 赛题技术指标对应表

本文件按正式题面逐条对应当前工程中的实现、演示方式和仍需在 Asterinas 环境补齐的部分。

## 任务内容

> 在星绽操作系统上，使用 Rust 编程语言实现 dm-crypt 与 dm-verity 功能。

当前工程已将核心逻辑实现为 Rust crate：

- `crates/dm`: Device Mapper、dm-crypt、dm-verity 核心逻辑；
- `crates/dmctl`: 用户态镜像演示工具；
- `asterinas/kernel/comps/dm`: 已接入 Asterinas BIO 与块设备注册框架的内核组件；
- `asterinas_port`: 可复用补丁和移植说明。

用户态 crate 用于快速验证算法、table 兼容和镜像格式；内核组件使用 Asterinas 原生
`BlockDevice`/BIO 接口，不依赖额外用户态适配器。

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

内核侧已完成 cmdline 解析、mapper 注册、on-disk hash tree 验证和篡改拒绝路径；
最终提交仍需补齐 rootfs 切换成功的完整串口日志与性能数据。

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

内核已经能在 init 阶段创建 dm-crypt mapper，并包含从 mapper 挂载 ext2 后执行
`pivot_root` 的实现。最终提交需要补录完整根切换 PASS 日志，并保留底层镜像无法读取
明文的对照证据。

## 指标 4：性能与启动要求

题面要求：

- dm-crypt 和 dm-verity 的 I/O 吞吐及延迟接近 Linux；
- dm-verity 不显著增加 kernel boot 时间。

当前对应：

- [performance.md](performance.md) 给出可复现的 Linux/Asterinas 测试矩阵；
- `scripts/benchmark.sh` 提供 plain/AES-XTS/verity 的 CSV 微基准入口；
- `scripts/benchmark_linux_dm.sh` 提供 raw/dm-crypt 同后端五轮配对测试；
- `scripts/summarize_benchmark.py` 输出中位数、吞吐保留率和 p95 延迟开销。

待 Asterinas 环境补齐：

- 在 QEMU 中分别测试 plain、dm-crypt、dm-verity；
- 记录 kernel boot 时间；
- 与 Linux dm-crypt/dm-verity 基线对比。

## 指标 5：技术探索

题面要求：

- 探索基于 Rust OS 的可信存储性能优化技术。

当前对应：

- Asterinas verity 已复用完成 BIO 的数据并加入“认证后缓存”的 hash block cache；
- 内核输出 BIO 复用、补读与 cache hit/miss 指标，可直接证明优化是否生效；
- mapper 接口统一，可加入 I/O batch、zero-copy buffer 和异步块 I/O；
- 用户态 `DmStackBuilder` 支持 target 动态组合与安全能力传播；
- `InstrumentedDevice` 可插在任意层之间进行分层性能归因。

建议答辩重点：

1. Rust trait 将安全存储能力模块化；
2. verity hash cache 减少重复计算；
3. crypt 使用硬件 AES 或批量加密降低开销；
4. rootfs 启动阶段只验证读取路径，减少 boot time 影响。

## 2026-07-14 验收补充

| 验收项 | 最终证据 |
|---|---|
| cryptsetup 控制面 | `/dev/mapper/control`；真实 cryptsetup 2.7.5 open/read/close PASS |
| dm-crypt 根文件系统 | `AES backend=AES-NI`；mapper 上 ext2 根切换 PASS |
| dm-verity 正例 | `status=verified`、`data_rereads=0`、根切换 PASS |
| dm-verity 篡改反例 | 一字节篡改后 `status=integrity-rejected`；零个 root-switch PASS |
| 可量化优化 | hash cache=256 相对 cache=0：五轮 dm-verity 中位吞吐 +24.4%，p95 -8.7% |

原始 KVM 串口和逐轮 CSV 位于 `docs/evidence/`，入口索引为
`docs/evidence/FINAL_VALIDATION.md`。
