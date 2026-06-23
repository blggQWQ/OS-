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

正式接入时建议新增解析结构：

```rust
pub struct DmCryptTable {
    pub cipher: String,
    pub key_hex: String,
    pub iv_offset: u64,
    pub device: String,
    pub offset: u64,
}
```

然后由该 table 创建 `DmCryptDevice`。

## 第 3 层：LUKS header 与 cryptsetup 用户态接口

如果要让标准命令直接工作：

```bash
cryptsetup luksFormat /dev/vdb
cryptsetup open /dev/vdb secure_data
```

需要补齐：

- LUKS header 解析；
- key slot 处理；
- PBKDF 参数处理；
- 用户态到内核态的控制接口；
- mapper 设备节点创建。

如果时间有限，建议答辩口径：

> 当前版本完成 dm-crypt 内核核心 I/O 路径和 table 化设计，cryptsetup 完整兼容需要继续补 LUKS metadata 和控制接口。比赛展示中使用自定义参数和 Linux baseline 脚本验证语义一致性。

## Linux baseline 对照

见：

```bash
scripts/linux_cryptsetup_baseline.sh
```

该脚本用于在 Linux 上创建标准 cryptsetup 加密卷，作为性能和功能对照基线。

