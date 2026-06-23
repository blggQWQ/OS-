# Asterinas Port Notes

这里放的是接入 Asterinas 时需要迁移到内核树中的骨架代码。

由于不同 Asterinas 版本的块设备 API 可能变化，本目录不硬编码具体 API 名称，而是标出必须填充的读写位置。

推荐迁移方式：

1. 把 `crates/dm/src` 中的核心模块复制到 Asterinas 的合适 crate/module；
2. 将 `std` 依赖替换为 Asterinas 可用的 `alloc`、同步原语和错误类型；
3. 按 `adapter_skeleton.rs` 实现真实 `virtio-blk` 适配层；
4. 按 `cmdline_skeleton.rs` 接入启动参数；
5. 先跑 passthrough，再跑 dm-crypt，最后跑 dm-verity rootfs。

