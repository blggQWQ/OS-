# Asterinas 接入指南

## 目标

把本仓库的 OS-independent Device Mapper 逻辑接入 Asterinas 的块设备层，使真实 `virtio-blk` 设备可以被包装成：

```text
virtio-blk -> DmCryptDevice -> 文件系统/测试程序
virtio-blk -> DmVerityDevice -> 只读 rootfs/测试程序
```

## 步骤 1：确认基础环境

在 Linux 主机上准备：

- Rust 工具链；
- QEMU；
- Asterinas 源码；
- 一个 raw 测试磁盘镜像。

先跑通官方 demo，确认 Asterinas 能启动，再挂载额外磁盘：

```bash
qemu-system-x86_64 \
  -drive file=test.img,format=raw,if=virtio \
  ...
```

## 步骤 2：接入 BlockDevice trait

在 Asterinas 中新增适配层，把真实块设备转成：

```rust
impl BlockDevice for VirtioBlkAdapter {
    fn read_block(&self, block_id: u64, buf: &mut [u8]) -> Result<()>;
    fn write_block(&self, block_id: u64, buf: &[u8]) -> Result<()>;
}
```

参考 [asterinas_port/adapter_skeleton.rs](../asterinas_port/adapter_skeleton.rs)。

## 步骤 3：先跑 passthrough

先不要直接上加密和 verity。流程应为：

```text
virtio-blk -> PassthroughDevice -> read/write test
```

如果 passthrough 读写正确，说明块设备转发路径没问题。

## 步骤 4：接入 dm-crypt

创建：

```rust
let lower = VirtioBlkAdapter::new(virtio_blk);
let crypt = DmCryptDevice::new(lower, key)?;
```

验证：

1. 通过 `crypt.write_block()` 写入明文；
2. 直接读底层 virtio 镜像，应看不到明文；
3. 通过 `crypt.read_block()` 读回，应恢复明文。

## 步骤 5：接入 dm-verity

对只读镜像构建 hash tree，记录 root hash：

```rust
let verity = DmVerityDevice::build(lower)?;
let root_hash = verity.root_hash();
```

正式启动时通过 cmdline 传入可信 root hash：

```text
rootfs_verity.scheme=dm-verity
rootfs_verity.hash=<root_hash_hex>
```

参考 [asterinas_port/cmdline_skeleton.rs](../asterinas_port/cmdline_skeleton.rs)。

## 步骤 6：rootfs 保护

推荐顺序：

```text
普通 rootfs 启动
-> passthrough rootfs 启动
-> dm-verity rootfs 只读启动
-> 篡改 rootfs 后启动失败或读失败
```

这条线是答辩时最容易讲清楚的安全闭环。

