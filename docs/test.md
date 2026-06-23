# 测试方案

## 本地 Rust 测试

运行：

```powershell
cargo test
```

覆盖场景：

| 场景 | 预期结果 |
| --- | --- |
| `dm-crypt` 写入后读取 | 返回数据与明文一致 |
| 加密写入后直接读取下层设备 | 数据与明文不同 |
| 使用错误密钥读取 | 数据与明文不一致 |
| `dm-verity` 正常读取 | 校验通过 |
| 数据块被篡改 | 读取返回 `IntegrityViolation` |
| root hash 错误 | 读取返回 `IntegrityViolation` |
| 写入 `dm-verity` | 返回 `ReadOnlyDevice` |

## QEMU / Asterinas 测试

1. 在 QEMU 中启动 Asterinas；
2. 通过 `if=virtio` 挂载原始 `test.img`；
3. 确认操作系统能够发现磁盘；
4. 执行透传读写；
5. 通过 `DmCryptDevice` 加密写入并检查原始磁盘字节；
6. 为只读镜像生成 verity root hash；
7. 篡改镜像并确认受保护读取失败；
8. 从 mapper 挂载 ext2，并通过 `pivot_root` 切换系统 `/`。

## 参赛证明材料

- QEMU 启动日志；
- virtio-blk 设备发现截图；
- 下层设备为密文的截图或日志；
- verity 在镜像篡改后拒绝读取的截图或日志；
- mapper 根文件系统切换日志；
- 测试命令及完整输出。

## CLI 演示测试

运行：

```powershell
cargo run -p dmctl -- demo-crypt
cargo run -p dmctl -- demo-verity
```

`demo-crypt` 的预期证据：

- `plain` 包含可读明文；
- `raw-hex` 与明文不同；
- `read` 与明文一致；
- 输出 `ciphertext_differs: true`。

`demo-verity` 的预期证据：

- 输出 root hash；
- 篡改块读取报告 `tamper_detected: true`。
