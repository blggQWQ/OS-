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

Linux QEMU virtio FIO 证据见 [`linux-qemu-fio-20260816`](../linux-qemu-fio-20260816/)。Asterinas 正式单元格见 [`PERFORMANCE_MATRIX.csv`](PERFORMANCE_MATRIX.csv)。Linux raw、dm-crypt 与 dm-verity 读使用同一套 `libaio/direct=1/numjobs=1` 参数；Asterinas 保护目标使用 guest 内实际 mapper。Asterinas raw 的旧批次与不存在 mapper 的假 dm-crypt 记录混在同一证据族中，未纳入本报告，也未由生成器读取。

## Filebench fileserver（Linux host）

| 目标 | 轮数 | 中位 ops/s | 中位 MB/s | 中位延迟 ms/op |
|---|---:|---:|---:|---:|
| raw | 5 | 147672.049 | 3542.7 | 0.7 |
| dm-crypt | 5 | 156507.741 | 3752.1 | 0.7 |

这里沿用 Filebench 原始输出的 MB/s 口径。dm-verity 为只读目标，与 fileserver 写密集负载不兼容，记为 N/A。

## SQLite（Linux host，10,000 行）

| 目标 | 轮数 | 中位总耗时 s | 中位查询耗时 s | 说明 |
|---|---:|---:|---:|---|
| raw | 5 | 0.060128 | 0.000153 | 完整写入与查询 |
| dm-crypt | 5 | 0.066395 | 0.000147 | 完整写入与查询 |
| dm-verity | 5 | 0.003865 | 0.000426 | 只读查询；不可与完整事务总耗时横比 |

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
- `filebench-valid/`：Filebench 5+5 轮原始日志和 CSV
- `sqlite/`：SQLite 5+5+5 轮记录
- `boot-matrix/`：Asterinas 启动 15 轮记录
- `cargo/`：构建与测试记录

## 结论

最终表只包含可定位到真实 block device/mapper、参数完整且轮数满足要求的数据。旧假 mapper、失败重试、调试日志和手工异常值均不进入生成输入；未修改 dm-crypt、dm-verity、BIO、virtio、DMA 或文件系统核心代码。
