# Asterinas Port Notes

这里放的是接入 Asterinas 时需要迁移到内核树中的骨架代码。

由于不同 Asterinas 版本的块设备 API 可能变化，本目录不硬编码具体 API 名称，而是标出必须填充的读写位置。

推荐迁移方式：

1. 把 `crates/dm/src` 中的核心模块复制到 Asterinas 的合适 crate/module；
2. 将 `std` 依赖替换为 Asterinas 可用的 `alloc`、同步原语和错误类型；
3. 按 `adapter_skeleton.rs` 实现真实 `virtio-blk` 适配层；
4. 按 `cmdline_skeleton.rs` 接入启动参数；
5. 先跑 passthrough，再跑 dm-crypt，最后跑 dm-verity rootfs。

## 已验证的 Asterinas 集成补丁

主仓库中的 `asterinas/` 是独立 Git 仓库，内部提交不会被主仓库的普通
`git push` 自动上传。因此，当前完整内核集成以补丁形式保存在：

```text
patches/0001-feat-dm-integrate-crypt-and-verity-targets.patch
```

补丁基于 Asterinas commit `c6284a9106f5a4c87deb7c8a990af6211dc5f540`。
应用方式：

```bash
cd asterinas
git checkout c6284a9106f5a4c87deb7c8a990af6211dc5f540
git am ../asterinas_port/patches/0001-feat-dm-integrate-crypt-and-verity-targets.patch
```

该补丁包含 `aster-dm` 组件、AES-256-XTS、on-disk dm-verity、启动参数、
VirtIO 测试盘接入和 QEMU 演示脚本。
