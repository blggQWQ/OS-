# 答辩提纲

## 1. 为什么做 dm-crypt / dm-verity

- dm-crypt 解决数据机密性问题：磁盘丢失或被直接读取时，看不到明文。
- dm-verity 解决数据完整性问题：系统镜像被恶意篡改后，启动或读取时能发现。
- 两者结合可以支撑可信 rootfs 和安全数据卷。

## 2. 项目架构怎么设计

核心是一层统一的 `BlockDevice` trait：

```text
上层文件系统
  -> Device Mapper wrapper
  -> 下层块设备
```

`dm-crypt` 和 `dm-verity` 都是 wrapper，所以可以组合、测试和替换底层设备。

## 3. 答辩现场优先展示什么

### 亮点一：篡改后拒绝创建并挂载 dm-verity rootfs

1. 用未篡改镜像启动，展示 `status=verified` 与 root switch PASS；
2. 只修改数据块 0 的 1 个字节（XOR `0xff`）；
3. 用相同 root hash 再启动，展示 `status=integrity-rejected`、mapper 不存在，且没有
   root switch PASS；
4. 对照日志：`docs/evidence/stabilization/correctness-kvm/regression/R03-verity-rootfs-persistent-cache.log`
   与 `docs/evidence/stabilization/correctness-kvm/corruption/V02-wrong-root.log`。

### 亮点二：完整性能真实性链

1. Linux raw 与 dm-crypt 使用同一 backing image、同一参数和真实 mapper；
2. Asterinas dm-crypt 在真实性审计后使用 guest 内真实 mapper 重新执行；
3. Asterinas dm-verity 使用修正后的 runner 执行 7 个只读工况；
4. 每个有效单元格保留五轮原始数据，不删除异常值；
5. 正式矩阵和证据路径见 `docs/evidence/final-performance-20260811/`。

## 4. dm-crypt 如何保证机密性

- 使用 Linux 兼容的 AES-256-XTS 与 plain64 扇区号语义；
- 写入时按 512 字节扇区加密，底层只保存密文；
- 读取时按相同 key、IV offset 与 data offset 解密；
- 直接查看底层镜像只能看到密文。

演示重点：

```bash
cargo run -p dmctl -- demo-crypt
```

## 5. dm-verity 如何保证完整性

- 对每个数据块计算 hash；
- 自底向上构建 Merkle tree；
- 将 root hash 作为可信锚点；
- 读取时校验当前块到 root hash 的路径；
- 任意数据块被篡改都会导致校验失败。

演示重点：

```bash
cargo run -p dmctl -- demo-verity
```

## 6. 如何接入 Asterinas

- 真实内核组件位于 `asterinas/kernel/comps/dm/`；
- `device.rs` 直接实现 Asterinas `BlockDevice`/BIO 映射；
- `lib.rs` 解析 crypt/verity 启动参数并注册 mapper；
- `verity.rs` 验证镜像尾部 on-disk Merkle tree，并维护认证后 hash cache；
- initramfs 从 mapper 挂载 ext2 后执行 `pivot_root`。

## 7. 创新点怎么讲

1. Rust 类型系统约束块设备接口，模块边界清晰。
2. 统一抽象同时覆盖加密和完整性校验。
3. 用户态 demo 与内核接入共享核心逻辑，便于验证和迁移。
4. 认证后 hash cache 具有五轮 A/B、逐轮 CSV 和内核计数器的完整证据链。
5. `data_rereads=0` 将 hash cache 收益与重复数据块读取明确区分。

## 8. 可能被问到的问题

### Q: 当前加密算法和 Linux cryptsetup 兼容吗？

当前核心 I/O 已支持 Linux `aes-xts-plain64`、64 字节 AES-256-XTS key、
plain64 IV 与 data offset，并已通过 Linux 内核逐字节对照。标准
当前版本已提供 `/dev/mapper/control` 的 Linux `dm_ioctl` 控制接口及动态设备节点：
在 KVM 客体中以 `cryptsetup 2.7.5 open` 创建 mapper，读取验证后再以
`cryptsetup close` 删除，三步均已留存串口 PASS 证据。LUKS2 的 PBKDF、keyslot
和 metadata 仍由用户态 cryptsetup 负责，内核只消费其下发的 dm-crypt table。

边界：LUKS2 metadata、PBKDF 和 keyslot 由用户态 cryptsetup 负责，内核消费其下发的
dm-crypt table；这与 Linux 的职责分层一致。

### Q: dm-verity 的 hash tree 是否落盘？

用户态 `dm` crate 为便于单元测试将 tree 保存在内存中；Asterinas
`aster-dm` 已使用镜像尾部的 on-disk hash tree，并从启动参数接收 root hash。

### Q: Rust 相比 C 内核模块的优势是什么？

Rust 的所有权、借用检查和类型系统能减少悬垂指针、越界访问、use-after-free 等内核常见内存安全问题。

### Q: 性能开销如何控制？

dm-crypt 可使用硬件加速 AES；dm-verity 可缓存 hash block，减少重复读取和计算。
最终结果分平台回答：Linux QEMU virtio raw/dm-crypt 完成 13+13 个五轮单元格，
dm-verity 读 7 项；Asterinas dm-crypt 完成 13 个、dm-verity 完成 7 个五轮单元格。
4 KiB 顺序读中位数分别为 Linux QEMU virtio 11.391/10.942/10.704 MiB/s，
Asterinas 保护 mapper 8.908/7.505 MiB/s。不同平台不计算倍率。
