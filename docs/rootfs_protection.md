# rootfs 加密与完整性保护方案

> 当前状态（2026-06-23）：Asterinas `aster-dm` 已接入真实 VirtIO BIO。
> dm-crypt 已解密 Linux `aes-xts-plain64` 生成的 ext2 superblock；dm-verity
> 已完成正常镜像通过、篡改镜像拒绝，并新增 `dm-root` init 模式，通过
> `pivot_root` 将 mapper 切换为系统 `/`。当前代码已通过完整内核构建和
> mapper BIO 启动验证；根切换最终串口 PASS 日志将在后续提交补录。

正式题目要求同时覆盖 rootfs 加密和 dm-verity 启动校验。本项目推荐按下面顺序接入，避免一开始直接改 rootfs 导致调试困难。

## 阶段 1：普通 rootfs 启动

先使用 Asterinas 官方流程启动 rootfs，记录：

- QEMU 启动命令；
- kernel boot log；
- rootfs mount 成功日志；
- boot time。

## 阶段 2：Passthrough rootfs

将 rootfs 底层块设备包装成 passthrough mapper：

```text
virtio-blk -> PassthroughDevice -> rootfs mount
```

目标是验证 mapper 层不会破坏原有块设备读写语义。

## 阶段 3：dm-verity rootfs

离线构建 rootfs 镜像的 hash tree，记录 root hash。

启动参数：

```text
rootfs_verity.scheme=dm-verity rootfs_verity.hash=<root_hash>
```

启动流程：

```text
parse cmdline
  -> locate rootfs block device
  -> open DmVerityDevice
  -> mount rootfs through verity device
```

验证点：

- 未篡改 rootfs 可启动；
- 修改底层 rootfs 任意块后，读取失败或启动失败；
- boot time 相比 plain rootfs 不显著增加。

## 阶段 4：dm-crypt rootfs

启动参数建议：

```text
dm_crypt.device=/dev/vda dm_crypt.key=<key> root=/dev/dm-crypt-root
```

启动流程：

```text
parse cmdline
  -> locate encrypted rootfs block device
  -> create DmCryptDevice
  -> mount decrypted mapper as rootfs
```

验证点：

- 系统可从加密 rootfs 启动；
- 直接查看底层 rootfs 镜像看不到明文文件；
- 通过 dm-crypt mapper 读取文件正常。

## 阶段 5：组合方案

推荐组合：

```text
dm-crypt: 保护机密性
dm-verity: 保护只读系统镜像完整性
```

常见组合方式：

```text
encrypted image -> DmCryptDevice -> DmVerityDevice -> read-only rootfs
```

或根据镜像布局调整为：

```text
verity-protected image -> DmVerityDevice -> DmCryptDevice -> rootfs
```

答辩时应说明选择哪种顺序，以及威胁模型是什么。
