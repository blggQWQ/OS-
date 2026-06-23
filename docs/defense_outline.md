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

## 3. dm-crypt 如何保证机密性

- 写入时根据 key 和 block id 生成块相关密钥流；
- 明文与密钥流变换后写到底层；
- 读取时使用相同 key 和 block id 恢复明文；
- 直接查看底层镜像只能看到密文。

演示重点：

```bash
cargo run -p dmctl -- demo-crypt
```

## 4. dm-verity 如何保证完整性

- 对每个数据块计算 hash；
- 自底向上构建 Merkle tree；
- 将 root hash 作为可信锚点；
- 读取时校验当前块到 root hash 的路径；
- 任意数据块被篡改都会导致校验失败。

演示重点：

```bash
cargo run -p dmctl -- demo-verity
```

## 5. 如何接入 Asterinas

- 先确认 QEMU 能启动 Asterinas；
- 挂载 virtio-blk 测试盘；
- 实现 `VirtioBlkAdapter`;
- 先跑 passthrough；
- 再接 `DmCryptDevice`;
- 最后接 `DmVerityDevice` 和 rootfs 启动参数。

## 6. 创新点怎么讲

1. Rust 类型系统约束块设备接口，模块边界清晰。
2. 统一抽象同时覆盖加密和完整性校验。
3. 用户态 demo 与内核接入共享核心逻辑，便于验证和迁移。
4. 提供完整测试路径：明文写入、密文落盘、明文恢复、篡改失败。

## 7. 可能被问到的问题

### Q: 当前加密算法和 Linux cryptsetup 兼容吗？

当前核心 I/O 已支持 Linux `aes-xts-plain64`、64 字节 AES-256-XTS key、
plain64 IV 与 data offset，并已通过 Linux 内核逐字节对照。标准
`cryptsetup open` 仍需要 Asterinas 的用户态 mapper 控制接口与设备节点管理。

更完整回答：本项目已完成 dm-crypt 核心 I/O 语义，`cryptsetup` 完整兼容还需要 table 参数解析、LUKS metadata 和用户态控制接口，路线见 `docs/cryptsetup_compatibility.md`。

### Q: dm-verity 的 hash tree 是否落盘？

用户态 `dm` crate 为便于单元测试将 tree 保存在内存中；Asterinas
`aster-dm` 已使用镜像尾部的 on-disk hash tree，并从启动参数接收 root hash。

### Q: Rust 相比 C 内核模块的优势是什么？

Rust 的所有权、借用检查和类型系统能减少悬垂指针、越界访问、use-after-free 等内核常见内存安全问题。

### Q: 性能开销如何控制？

dm-crypt 可使用硬件加速 AES；dm-verity 可缓存 hash block，减少重复读取和计算。
