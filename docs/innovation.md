# 决赛技术亮点与验证口径

本项目不把“实现 dm-crypt 与 dm-verity”当作终点，而是围绕 Rust OS 中可信存储的
可组合性、可验证性能和安全优化形成一条完整技术主线。

## 1. 类型安全的 Device Mapper 组合框架

`DmStackBuilder` 可以在同一个 `BlockDevice` 抽象上动态组合 passthrough、crypt 和
verity target。每一层声明自己提供的安全能力，最终栈自动汇总：

- `confidentiality`：底层介质不暴露明文；
- `integrity`：读取数据能够追溯到可信 root hash；
- `read_only`：完整性保护层禁止写入。

这使答辩重点从“写了两个独立模块”提升为“实现了一套可扩展的 Rust Device Mapper
框架”。运行以下命令可直接展示组合结果：

```bash
cargo run -p dmctl -- demo-stack
```

`InstrumentedDevice` 可以插在任意两层之间，分别统计 I/O 次数、字节数、失败次数、
累计和最大延迟。上下各放一个插桩点即可把 target 计算开销与底层设备等待时间分离。

## 2. dm-verity 热路径安全优化

Asterinas 内核版本包含两项配套优化：

1. 对 4 KiB 对齐 BIO，直接校验已经完成的 BIO buffer，不再为了计算 SHA-256 重读
   同一个数据块；只有部分块请求才使用保守的完整块补读路径。
2. 缓存已经认证的 hash block，减少相邻数据块反复读取相同 Merkle 上层节点。

缓存遵循“先认证、后插入”：新读取的 hash block 必须沿完整路径成功到达可信 root
hash 后才能进入缓存。缓存命中不会降低完整性边界。

内核暴露以下热路径计数器，并在启动自检后打印 `[dm-perf]` 日志：

- `verified`：成功认证的数据块数；
- `bio_blocks`：直接复用 BIO 数据的块数；
- `data_rereads`：因未对齐而补读的数据块数；
- `hash_hits` / `hash_misses`：认证哈希块缓存命中与缺失。

决赛演示应同时展示“篡改仍被拒绝”和“第二轮顺序读 hash cache 命中率提升”，避免
只讲性能而不证明安全语义。

## 3. Linux 对照不是装饰性跑分

`scripts/benchmark_linux_dm.sh` 使用相同镜像、块大小、队列深度和 fio 参数交替测试
Linux raw block 与内核 dm-crypt。每个场景至少运行五次，保留每轮原始 CSV，并记录：

- MiB/s 与 IOPS；
- 平均、p50、p95、p99 完成延迟；
- fio 用户态与内核态 CPU 占用。

`scripts/summarize_benchmark.py` 计算各指标中位数，并直接给出 dm-crypt 相对 raw 的
吞吐保留率和 p95 延迟开销。报告不得选择最好的一轮，也不得把内存微基准当成 Linux
对照结果。

## 4. cryptsetup 兼容边界

`DmCryptTable` 按 Linux table 的 option-count 语法解析参数，支持当前实现语义不变的
调度提示，并明确拒绝尚未实现的 `sector_size:4096`、integrity profile 等选项。与其
静默产生不兼容密文，项目选择在配置边界立即失败。

当前已经完成 Linux `aes-xts-plain64` 逐字节互操作；下一阶段的完整 `cryptsetup open`
需要补 Asterinas 的 mapper 控制接口和设备节点生命周期。LUKS2 PBKDF、keyslot 与
metadata 继续由 cryptsetup 用户态管理，内核只接收 volume key 和 target table。

## 决赛验收线

代码和材料只有同时满足以下条件才进入封板：

1. 用户态 workspace 测试全通过，Linux AES-XTS 固定向量逐字节一致；
2. Asterinas `make kernel` 通过，QEMU 中 crypt/verity 正反例与 rootfs 切换通过；
3. Linux 与 Asterinas 使用同一测试矩阵，各场景至少五轮并报告中位数和尾延迟；
4. 优化前后保留原始数据，能量化 BIO 重读消除与 hash cache 的收益；
5. 演示脚本、内核参数、镜像生成命令和原始日志均纳入提交包。

## 2026-07-14 最终状态

以上验收线已完成：Asterinas 提供 `/dev/mapper/control` 的 Linux dm-ioctl
控制面，真实 `cryptsetup 2.7.5 open/read/close` 已在 KVM 中通过；加密根文件
系统串口明确报告 `AES backend=AES-NI`，并在保存/恢复 XMM 状态的临界区内完成
`pivot_root`。dm-verity 的未篡改镜像报告 `status=verified` 并成功切根；只篡改
首数据块一个字节的镜像报告 `status=integrity-rejected`，没有任何 rootfs PASS。

完整串口、五轮 fio CSV 和 hash-cache A/B 汇总见
`docs/evidence/FINAL_VALIDATION.md`。
