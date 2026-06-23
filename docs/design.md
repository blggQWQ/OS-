# 设计说明

## 目标

提供一个紧凑的 Rust Device Mapper 模型，并在块设备路径可用后接入 Asterinas。

## 核心接口

唯一必需的抽象是 `BlockDevice`：

```rust
fn read_block(&self, block_id: u64, buf: &mut [u8]) -> Result<()>;
fn write_block(&self, block_id: u64, buf: &[u8]) -> Result<()>;
```

每个 mapper 包装一个下层设备，并实现相同的 trait：

```text
应用程序 / 文件系统
        -> DmCryptDevice 或 DmVerityDevice
        -> BlockDevice 实现
        -> virtio-blk 或磁盘镜像
```

## 模块组成

- `block_device`：trait 以及用于测试的 `MemoryBlockDevice`；
- `device_mapper`：用于验证转发路径的透传包装器；
- `dm_crypt`：透明块加密包装器；
- `dm_verity`：只读完整性校验包装器；
- `hash_tree`：Merkle tree 的构建与验证；
- `sha256`：用于 verity 哈希及旧版最小演示的无外部依赖 SHA-256 实现。

## dm-crypt 流程

写入路径：

```text
明文扇区 -> AES-256-XTS（plain64 扇区 IV）-> 密文 -> 下层设备
```

读取路径：

```text
密文扇区 -> AES-256-XTS 解密 -> 明文 -> 调用方
```

Linux 兼容路径使用 AES-256-XTS 和 plain64 扇区编号，支持 dm-crypt table
中的 IV offset 与 data offset。旧版无外部依赖变换只保留给最小演示命令。

## dm-verity 流程

1. 从下层只读镜像读取全部数据块；
2. 使用 SHA-256 生成叶子哈希；
3. 对相邻子哈希进行哈希运算，逐层构建父节点；
4. 将 root hash 保存为可信值；
5. 读取时先根据 tree 和 root hash 验证数据块，再将数据返回调用方。

## Asterinas 接入

操作系统相关代码应隔离在单个适配器内：

```rust
struct VirtioBlkAdapter {
    inner: AsterinasVirtioBlockHandle,
}

impl BlockDevice for VirtioBlkAdapter {
    // 转发到真实的 Asterinas 块 I/O 接口
}
```

适配器完成后，mapper 即可包装真实设备，无需修改加密或 verity 核心逻辑。
