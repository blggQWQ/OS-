# Asterinas 移植说明

迁移已经完成。本目录保留可审计、可重放的集成补丁；当前真实实现位于：

- `asterinas/kernel/comps/dm/src/device.rs`：Asterinas `BlockDevice`/BIO 映射；
- `asterinas/kernel/comps/dm/src/lib.rs`：启动参数、mapper 注册与 rootfs 接入；
- `asterinas/kernel/comps/dm/src/crypto.rs`：AES-256-XTS；
- `asterinas/kernel/comps/dm/src/verity.rs`：on-disk Merkle 校验与 hash cache；
- `asterinas/kernel/src/fs/fs_impls/procfs/dm_verity_stats.rs`：可观测性能计数器。

早期 `adapter_skeleton.rs` 和 `cmdline_skeleton.rs` 设计草图已移除，避免与完成迁移的
状态冲突；真实代码和下面的补丁是唯一评审依据。

## 已验证的 Asterinas 集成补丁

主仓库中的 `asterinas/` 是独立 Git 仓库，内部提交不会被主仓库的普通
`git push` 自动上传。因此，当前完整内核集成以补丁形式保存在：

```text
patches/0001-feat-dm-integrate-crypt-and-verity-targets.patch
patches/0002-fix-dm-validate-crypt-and-verity-BIO-paths.patch
patches/0003-rootfs-switch.patch
patches/0004-Rust-Device-Mapper.patch
patches/0005-dm.patch
patches/0006-dm-performance-attribution.patch
patches/0007-dm-stack-crypt-verity.patch
```

补丁基于 Asterinas commit `c6284a9106f5a4c87deb7c8a990af6211dc5f540`。
应用方式：

```bash
cd asterinas
git checkout c6284a9106f5a4c87deb7c8a990af6211dc5f540
git am ../asterinas_port/patches/0001-feat-dm-integrate-crypt-and-verity-targets.patch \
       ../asterinas_port/patches/0002-fix-dm-validate-crypt-and-verity-BIO-paths.patch \
       ../asterinas_port/patches/0003-rootfs-switch.patch \
       ../asterinas_port/patches/0004-Rust-Device-Mapper.patch \
       ../asterinas_port/patches/0005-dm.patch \
       ../asterinas_port/patches/0006-dm-performance-attribution.patch \
       ../asterinas_port/patches/0007-dm-stack-crypt-verity.patch
```

该补丁包含 `aster-dm` 组件、AES-256-XTS、on-disk dm-verity、启动参数、
VirtIO 测试盘接入和 QEMU 演示脚本。

第三个补丁增加 `dm-root` init 模式：从已注册 mapper 挂载 ext2 后执行
`pivot_root`，将系统 `/` 切换到受保护卷；crypt 与 verity 运行脚本默认执行
该根切换验证并自动关机。

第四个补丁固化完整的 cryptsetup 控制面、KVM 证据链和评审版内核实现。第五个
补丁加入 dm-crypt 有界临时缓冲池、verity 已认证路径捷径、可配置 fio 矩阵及
对应 A/B 开关；第五个补丁已通过官方工具链 `make kernel`。

第六个补丁加入 `/proc/dm-performance-stats`、dm-crypt/dm-verity 分阶段计时、
release-LTO 正式基准合同以及同日志 AES-NI 证据；该补丁已通过 release-LTO 构建和
256 MiB、10 秒、五轮 KVM 矩阵验证。

第七个补丁实现内核态 Combined 叠栈：`dm_stack=crypt,verity`（或
`rootfs_verity.device=<crypt mapper>`）将 verity 挂在 crypt 之上，串口打印
`[dm] stacked mapper: vdc -> dm-crypt0 -> dm-verity0`，`dm-root` 指向顶层
`/dev/dm-verity0`，性能统计优先暴露顶层 mapper。配套脚本：

```bash
sudo bash ../scripts/build_stack_rootfs.sh
ENABLE_KVM=1 bash ../scripts/run_dm_stack.sh
# 或在已应用补丁的树内：bash tools/run_dm_stack.sh
```

第二个补丁修复 BIO DMA segment 的方向访问问题，并加入启动时真实 I/O 自检。
验收日志：

```text
[dm-test] PASS AES-256-XTS decrypted a valid ext2 superblock through dm-crypt
[dm-test] PASS first data block verified through dm-verity
[dm-test] FAIL dm-verity rejected first data block: IntegrityViolation
[dm] stacked mapper: vdc -> dm-crypt0 -> dm-verity0
[dm-root] PASS system root switched to protected mapper
```
