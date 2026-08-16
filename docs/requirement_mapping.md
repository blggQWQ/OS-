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
- `asterinas/kernel/comps/dm/src/lib.rs`: 真实启动参数解析与 mapper 注册；
- `dmctl verity-root`: 生成 root hash；
- `dmctl verity-verify`: 使用 root hash 验证镜像。

内核侧已完成 cmdline 解析、mapper 注册、on-disk hash tree 验证、篡改拒绝和
受保护 rootfs 切换；串口证据见 `docs/evidence/asterinas-verity-rootfs-kvm.log` 与
`docs/evidence/asterinas-verity-rootfs-tampered-kvm.log`。

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

Asterinas 已提供 `/dev/mapper/control` 的 `dm_ioctl` 控制接口；KVM 客体中的
cryptsetup 2.7.5 已完成 `open` 创建可读 mapper 和 `close` 删除 mapper 的真实链路。
LUKS metadata、PBKDF 与 keyslot 仍按 Linux 分层留在用户态。

## 指标 3：根文件系统加密

题面要求：

- 使用 dm-crypt 加解密根文件系统。

当前对应：

- [rootfs_protection.md](rootfs_protection.md) 给出 rootfs 加密和 verity 的接入顺序；
- `asterinas/kernel/comps/dm/src/lib.rs` 定义并解析 `dm_crypt.device`、
  `dm_crypt.name` 和 `dm_crypt.key`。

内核已经能在 init 阶段创建 dm-crypt mapper，并从 mapper 挂载 ext2 后执行
`pivot_root`；完整 PASS 日志见 `docs/evidence/asterinas-crypt-rootfs-aesni-kvm.log`。

## 指标 4：性能与启动要求

题面要求：

- dm-crypt 和 dm-verity 的 I/O 吞吐及延迟接近 Linux；
- dm-verity 不显著增加 kernel boot 时间。

当前对应：

- [performance.md](performance.md) 给出可复现的 Linux/Asterinas 测试矩阵；
- `scripts/benchmark.sh` 提供 plain/AES-XTS/verity 的 CSV 微基准入口；
- `scripts/run_fio_matrix.sh` 提供 Linux raw/dm-crypt 同后端五轮配对测试和 Asterinas KVM 正式矩阵；
- `scripts/benchmark_boot_time.py` 提供带宿主机时间戳的五轮启动对照；
- `scripts/summarize_fio_matrix.py` 输出均值、中位数、标准差和原始证据路径。

当前已经完成 Asterinas KVM dm-crypt 13 项、dm-verity 7 项，Linux QEMU virtio
raw/dm-crypt 各 13 项、dm-verity 读 7 项以及 plain/dm-crypt/dm-verity 各五轮启动对照。
跨平台结果分开列示，不使用不同环境数据计算倍率；Linux FIO 见
`docs/evidence/linux-qemu-fio-20260816/`。

## 指标 5：技术探索

题面要求：

- 探索基于 Rust OS 的可信存储性能优化技术。

当前对应：

- Asterinas verity 已复用完成 BIO 的数据并加入“认证后缓存”的 hash block cache；
- dm-crypt 已加入可关闭的有界临时缓冲池，支持同条件 A/B；
- verity 已加入已认证 hash block 的可信后缀捷径与 `auth_shortcuts` 计数；
- 内核输出 BIO 复用、补读与 cache hit/miss 指标，可直接证明优化是否生效；
- mapper 接口统一，后续可继续加入 I/O batch 和异步块 I/O；
- 用户态 `DmStackBuilder` 支持 target 动态组合与安全能力传播；
- 内核补丁 `0007` 提供 Combined 叠栈启动路径（`dm_stack=crypt,verity`，
  `vdc → dm-crypt0 → dm-verity0`），与用户态 `demo-stack` 同序；
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
| 正式 FIO | Linux raw/dm-crypt 各 13 项；Asterinas dm-crypt 13 项、dm-verity 7 项；每项五轮 |
| 性能口径 | Linux 与 Asterinas 分平台展示，不计算跨环境倍率 |
| 启动开销 | plain 51.331303 s；dm-crypt 51.629820 s（+0.582%）；dm-verity 52.743415 s（+2.751%） |

原始 KVM 串口和逐轮 CSV 位于 `docs/evidence/`，入口索引为
`docs/evidence/README.md`。
