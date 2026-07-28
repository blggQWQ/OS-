# cryptsetup 兼容方案

题面要求 dm-crypt 作为后端支撑用户程序 `cryptsetup` 对加密卷的创建和管理。该要求分为三层。

## 第 1 层：透明加解密 I/O 路径

当前工程已完成：

```text
write plaintext -> DmCryptDevice -> lower ciphertext
read ciphertext -> DmCryptDevice -> plaintext
```

演示命令：

```bash
cargo run -p dmctl -- demo-crypt
```

## 第 2 层：Linux dm-crypt table 参数兼容

`cryptsetup` 最终会把参数传给 device mapper table，典型格式包含：

```text
cipher key iv_offset device offset
```

当前已实现解析结构：

```rust
pub struct DmCryptTable {
    pub cipher: String,
    pub key: Vec<u8>,
    pub iv_offset: u64,
    pub device: String,
    pub offset: u64,
}
```

`DmCryptDevice::from_table` 当前支持：

- `aes-xts-plain64`；
- 64 字节 AES-256-XTS key；
- `iv_offset` 的 plain64 扇区编号语义；
- 以 512 字节扇区计数的 data offset；
- 512B 与 4KiB 下层块设备。

兼容证据不是只做 Rust 内部 round-trip。运行：

```bash
sudo bash scripts/verify_linux_xts_compat.sh
```

脚本分别调用 Linux 内核 dm-crypt 与本项目 Rust 实现加密相同的 512B
数据，然后用 `cmp` 做逐字节比较。固定测试向量的两端 SHA-256 均为：

```text
4334c17f4b0380b3c1f902509cce0c0f98d9fa77aa93f6407ec940215d77921d
```

## 第 3 层：LUKS header 与 cryptsetup 用户态接口

如果要让标准命令直接工作：

```bash
cryptsetup luksFormat /dev/vdb
cryptsetup open /dev/vdb secure_data
```

LUKS header、key slot 和 PBKDF 本来就由用户态 `cryptsetup` 处理，不应在内核
重复实现。Asterinas 内核需要提供的是 `dm_ioctl` 控制接口、mapper 设备节点和
dm-crypt table 数据面；当前三项均已接通，并已在 KVM 客体用 cryptsetup 2.7.5
完成 open/read/close。

建议答辩口径：

> 当前版本已完成 AES-256-XTS 核心 I/O、Linux dm-crypt table 解析，并与 Linux 内核输出完成逐字节互操作验证。Asterinas 已提供 `/dev/mapper/control` 的 Linux `dm_ioctl` 控制路径；KVM 客体中的 cryptsetup 2.7.5 已完成 `open` 创建可读 mapper 及 `close` 删除 mapper 的真实链路验证。LUKS2 的 PBKDF、keyslot 与 metadata 继续由用户态 cryptsetup 负责，内核只消费其下发的 dm-crypt table，不在内核重复实现。

## Linux 基准对照

见：

```bash
scripts/linux_cryptsetup_baseline.sh
```

该脚本用于在 Linux 上创建标准 cryptsetup 加密卷，作为性能和功能对照基线。
