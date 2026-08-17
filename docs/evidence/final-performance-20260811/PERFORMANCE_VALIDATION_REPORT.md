# 性能验证报告

## 基线与口径

- 顶层测试基线：`fc548a9e7fadcfd2d03a6f436754b0892bebd1fe`
- Asterinas 测试基线：`426db48c7d01c94cf0c6e761d0edda63040d787b`
- Asterinas 官方开发镜像：`sha256:7510071cb103309cc9396b970325c354713da749dfee34a807108248cc97331e`
- Asterinas：QEMU/KVM、`-cpu host`、1 vCPU、8 GiB 内存
- 每个有效单元格保留 5 轮原始结果；带宽单位为 MiB/s
- Linux 与 Asterinas 结果分平台列示，不进行跨平台倍率比较

## FIO 完成情况

| 平台 | 目标 | 有效单元格 | 状态 |
|---|---|---:|---|
| Asterinas KVM | dm-crypt | 13 | PASS |
| Asterinas KVM | dm-verity | 7 | PASS；另有 6 个只读 N/A |
| Linux QEMU virtio | raw block device | 13 | PASS |
| Linux QEMU virtio | dm-crypt mapper | 13 | PASS |
| Linux QEMU virtio | dm-verity 读 | 7 | PASS；另有 6 个只读 N/A |

Linux QEMU virtio FIO 证据见 [`linux-qemu-fio-20260816`](../linux-qemu-fio-20260816/)。Asterinas 13 格 mapper 见 [`PERFORMANCE_MATRIX.csv`](PERFORMANCE_MATRIX.csv)。Linux raw、dm-crypt 与 dm-verity 读使用同一套 `libaio/direct=1/numjobs=1` 参数；Asterinas 保护目标使用 guest 内实际 mapper。13 格 mapper 矩阵不含 Asterinas raw：恢复后的 dm-crypt 批次没有同一次启动的配对 raw，旧 raw 批次曾与假 mapper 混在同一证据族中，未纳入该矩阵。

## 256 MiB 正式合同（Asterinas raw / dm-crypt / dm-verity）

KVM、1 vCPU、8 GiB、virtio-blk、4 KiB 同步读、QD1、10 秒 runtime、交替顺序五轮。raw 行合并两组共十次测量；target 的相对值使用同一镜像、同一轮次合同的配对 raw 中位数，不与上一节 13 格 mapper（8.908 / 7.505）混算保留率。

| 设备 | 五轮中位吞吐（MiB/s） | 中位 p50（us） | 中位 p95（us） | 相对配对 raw |
|---|---:|---:|---:|---:|
| raw（两组共十轮） | 4.249 | 892.928 | 1277.952 | 100.0% |
| dm-crypt | 4.059 | 921.600 | 1318.912 | 95.6% |
| dm-verity | 3.815 | 954.368 | 1433.600 | 87.3% |

阶段差值：dm-crypt 下层等待中位占 97.84%、AES-XTS 占 1.82%；dm-verity 下层等待中位占 93.01%、数据 SHA-256 占 5.74%。

同一宿主机、同一 virtio-blk 数据盘前 256 MiB 的 Linux guest 对照（4 KiB sync QD1）：Linux raw / dm-crypt / dm-verity 为 5.150 / 4.746 / 4.369，Asterinas 为 4.245 / 4.059 / 3.815。crypt 保留率相对表中 raw 为 92.158% vs 95.621%；verity 保留率相对配对 raw（Linux 4.641、Asterinas 4.368）为 94.1% vs 87.3%。Asterinas 相对 Linux guest 的绝对吞吐为 raw 82.4%、dm-crypt 85.5%、dm-verity 87.3%（3.815 / 4.369）。自身保留率高于或不等于 Linux，不代表绝对吞吐超过 Linux。

## Filebench fileserver（Linux QEMU/KVM guest）

| 目标 | 轮数 | 中位 ops/s | 中位 MB/s | 中位延迟 ms/op |
|---|---:|---:|---:|---:|
| raw | 5 | 27855.155 | 656.9 | 3.3 |
| dm-crypt | 5 | 25666.016 | 605.0 | 3.7 |

Linux QEMU virtio `/dev/vdb`，fileserver，2000 文件，50 线程，60 s。沿用 Filebench 原始输出的 MB/s 口径。dm-verity 为只读目标，与 fileserver 写密集负载不兼容，记为 N/A。证据：`docs/evidence/linux-qemu-filebench-sqlite-20260817/filebench/`。旧 Linux host 目录 `filebench-valid/` 仅归档，不再引用。

## SQLite（Linux QEMU/KVM guest，10,000 行）

| 目标 | 轮数 | 中位总耗时 s | 中位查询耗时 s | 说明 |
|---|---:|---:|---:|---|
| raw | 5 | 0.138095 | 0.000191 | 完整写入与查询 |
| dm-crypt | 5 | 0.143336 | 0.000149 | 完整写入与查询 |
| dm-verity | 5 | 0.001151 | 0.000571 | 只读查询；不可与完整事务总耗时横比 |

证据：`docs/evidence/linux-qemu-filebench-sqlite-20260817/sqlite/`。旧 Linux host 目录 `final-performance-20260811/sqlite/` 仅归档，不再引用。

## Asterinas 启动耗时

| 场景 | 轮数 | 中位耗时 s |
|---|---:|---:|
| plain | 5 | 51.331303 |
| dm-crypt rootfs | 5 | 51.629820 |
| dm-verity rootfs | 5 | 52.743415 |

相对 plain 的中位开销：dm-crypt 0.582%，dm-verity 2.751%。计时边界为 QEMU 进程启动至串口出现受保护根文件系统 PASS 标记。

## 证据目录

- `asterinas-fio/asterinas-kvm/dm-crypt-recovery/`：真实性审计后重新执行的 dm-crypt
- `asterinas-fio/asterinas-kvm/dm-verity/`：修正 runner 后重新执行的 dm-verity
- `docs/evidence/linux-qemu-fio-20260816/`：Linux QEMU virtio raw/dm-crypt/dm-verity FIO JSON 与汇总
- `docs/evidence/linux-qemu-filebench-sqlite-20260817/`：Linux QEMU guest Filebench / SQLite
- `filebench-valid/`：Linux host Filebench 归档，不再引用
- `sqlite/`：Linux host SQLite 归档，不再引用
- `boot-matrix/`：Asterinas 启动 15 轮记录
- `cargo/`：构建与测试记录

## 结论

最终表只包含可定位到真实 block device/mapper、参数完整且轮数满足要求的数据。旧假 mapper、失败重试、调试日志和手工异常值均不进入生成输入；未修改 dm-crypt、dm-verity、BIO、virtio、DMA 或文件系统核心代码。
