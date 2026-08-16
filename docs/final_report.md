# 实验报告：基于 Rust OS 的 dm-crypt 与 dm-verity

## 1. 项目背景

块设备加密和完整性校验是可信存储系统中的两个核心能力。Linux 中已有成熟的 Device Mapper、dm-crypt 和 dm-verity 机制。本项目将这一思想迁移到 Rust OS 场景中，目标是在 Asterinas 上实现一套简化但可验证的块设备安全映射层。

本项目重点解决两个问题：

1. 数据机密性：上层写入明文，底层磁盘保存密文，读取时自动恢复明文。
2. 数据完整性：只读镜像被篡改后，读取路径能够检测并拒绝返回错误数据。

## 2. 总体设计

项目定义统一的 `BlockDevice` trait：

```rust
pub trait BlockDevice {
    fn block_size(&self) -> usize;
    fn num_blocks(&self) -> u64;
    fn read_block(&self, block_id: u64, buf: &mut [u8]) -> Result<()>;
    fn write_block(&self, block_id: u64, buf: &[u8]) -> Result<()>;
}
```

所有安全模块都包装一个下层块设备，并继续实现同一接口：

```text
应用 / 文件系统
        -> dm-crypt / dm-verity
        -> BlockDevice
        -> FileBlockDevice / Asterinas virtio-blk
```

这种设计让用户态文件镜像演示和 Asterinas 内核接入共享同一套核心逻辑。

## 3. 模块实现

### 3.1 Device Mapper 基础层

`PassthroughDevice` 实现最小映射层，只负责转发 `read_block` 和 `write_block`。它用于验证块设备路径是否正确。

### 3.2 dm-crypt

`DmCryptDevice` 在写入时对块数据加密，在读取时自动解密。

写路径：

```text
plaintext sectors -> AES-256-XTS(plain64 sector IV) -> ciphertext -> lower device
```

读路径：

```text
ciphertext sectors -> AES-256-XTS decrypt -> plaintext -> caller
```

当前兼容路径使用 AES-256-XTS 与 Linux `plain64` IV 语义，支持 Linux
dm-crypt table 的 IV offset 和 data offset；固定向量已与 Linux 内核输出
完成逐字节比较。旧 SHA-256 演示变换只保留给最小示例命令。

### 3.3 dm-verity

`DmVerityDevice` 为只读块设备构建 Merkle Hash Tree，并保存可信 root hash。读取任意块时重新计算该块到根节点的校验路径，一旦数据块或 root hash 不匹配，即返回 `IntegrityViolation`。

### 3.4 文件镜像后端

`FileBlockDevice` 支持把普通 `.img` 文件作为块设备使用，用于演示：

- 加密写入后，直接查看镜像只能看到密文；
- 生成 verity root hash 后，篡改镜像会导致验证失败。

## 4. 正确性测试

测试覆盖：

| 测试项 | 预期结果 |
| --- | --- |
| dm-crypt 写入后读取 | 返回明文 |
| 直接读取底层镜像 | 看不到明文 |
| 错误 key 读取 | 无法恢复原文 |
| dm-verity 正常读取 | 校验通过 |
| 篡改数据块 | 返回 `IntegrityViolation` |
| 错误 root hash | 返回 `IntegrityViolation` |
| dm-verity 写入 | 返回 `ReadOnlyDevice` |

本地运行：

```bash
cargo test
```

## 5. 演示流程

dm-crypt：

```bash
cargo run -p dmctl -- demo-crypt
```

dm-verity：

```bash
cargo run -p dmctl -- demo-verity
```

Asterinas/QEMU：

1. 启动 Asterinas；
2. 挂载 `virtio-blk` 测试磁盘；
3. 用 `VirtioBlkAdapter` 接入本项目 `BlockDevice` trait；
4. 先验证 passthrough；
5. 再验证 dm-crypt 密文落盘；
6. 最后验证 dm-verity 篡改检测。

## 6. 性能测试方案

指标：

- 顺序读写吞吐；
- 4 KiB 随机读写 IOPS；
- dm-verity 读延迟；
- rootfs 启动时间变化；
- CPU 开销。

测试工具：

```bash
fio --name=crypt-test --filename=/mnt/testfile --size=128M --rw=readwrite --bs=4k
fio --name=verity-read --filename=/mnt/testfile --size=128M --rw=read --bs=4k
```

## 7. 创新点

1. 使用 Rust trait 抽象统一 dm-crypt 与 dm-verity。
2. 用户态文件镜像和 Asterinas 内核适配共享同一核心逻辑。
3. 提供可复现实验 CLI，便于快速演示密文落盘和篡改检测。
4. 将 Linux Device Mapper 思路迁移到 Rust OS 场景，降低传统内核模块内存安全风险。

## 8. 当前限制与后续工作

当前限制：

- 已实现 `/dev/mapper/control` 的 Linux `dm_ioctl` 控制路径；在 KVM 客体中使用
  `cryptsetup 2.7.5 open/close` 完成了真实 mapper 的创建、可读验证与销毁验证。
- 用户态测试 tree 仍在内存中；Asterinas 内核路径使用镜像尾部的 on-disk hash tree。
- 已完成 Linux QEMU virtio raw/dm-crypt 各 13 项、dm-verity 读 7 项，以及
  Asterinas KVM dm-crypt 13 项与 dm-verity 7 项正式 FIO；不同平台分表展示，不计算跨环境倍率。
- 已完成 plain、dm-crypt、dm-verity 各五轮启动测试，中位数分别为
  51.331303、51.629820、52.743415 秒；相对 plain 开销为 0.582% 和 2.751%。

这些限制与正式题面的关系见 [requirement_mapping.md](requirement_mapping.md)。其中 rootfs 加密与验证见 [rootfs_protection.md](rootfs_protection.md)，cryptsetup 兼容路线见 [cryptsetup_compatibility.md](cryptsetup_compatibility.md)。

后续工作：

1. 扩展 `dm_ioctl` table 参数与错误码兼容性覆盖；
2. 将 Combined 叠栈（补丁 0007 + `scripts/build_stack_rootfs.sh` /
   `scripts/run_dm_stack.sh`）的 KVM 串口证据归档到 `docs/evidence/stack/`，
   并补充长时间端到端压力测试；
3. 扩展到 randread、write、QD32 与更大工作集；
4. 补充 dm-crypt 根的冷启动和三种场景的热启动对照。

说明：crypt+verity 叠加 rootfs 启动路径本身已在内核补丁 0007 与构建/启动脚本中实现；
待归档的是本机 KVM 串口日志（见 `docs/evidence/stack/README.md`）。
