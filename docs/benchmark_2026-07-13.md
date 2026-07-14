# Linux raw / dm-crypt 五轮基准（2026-07-13）

## 结论

在 4 KiB、QD1/QD32、顺序/随机读写共 8 组工况中，Linux dm-crypt 的吞吐中位数为 raw 的 79.02%–98.33%。除 QD32 随机写外，其余 7 组均达到 raw 的 88.29% 以上；QD1 四组均达到 96.07% 以上。当前首要优化对象因此明确为高并发随机写，而不是平均分散精力。

这份数据是 Linux 参考上限，不代表 Asterinas 已达到同样水平。Asterinas 必须用相同 fio 参数、相同 KVM CPU/磁盘配置生成另一列，才能形成国赛答辩所需的直接对照。

## 环境与方法

- 宿主：Intel Core Ultra 5 125H，18 vCPU，CPU flags 含 `aes`/`vaes`
- Linux：WSL2 kernel 6.6.87.2
- device-mapper driver：4.48.0；fio：3.36；cryptsetup：2.7.0
- 映射：`aes-xts-plain64`，512-bit XTS key，`no_read_workqueue no_write_workqueue`
- fio：libaio、direct I/O、4 KiB、QD1/QD32、每组 10 秒、3 秒 ramp、每组 5 次
- 为降低顺序/温度偏差，奇偶轮交替执行 raw 和 dm-crypt；报告中只使用 5 次中位数
- 原始逐轮结果：`target/benchmark/linux-dm.csv`；版本化摘要：`docs/evidence/linux-dm-summary.csv`

## 中位数摘要

| 工况 | QD | raw MiB/s | dm-crypt MiB/s | dm/raw | dm p95 us | dm p99 us |
|---|---:|---:|---:|---:|---:|---:|
| 顺序读 | 1 | 17.87 | 17.25 | 96.54% | 317.44 | 382.98 |
| 顺序读 | 32 | 192.01 | 185.63 | 96.68% | 1236.99 | 1646.59 |
| 顺序写 | 1 | 16.35 | 16.07 | 98.33% | 333.82 | 403.46 |
| 顺序写 | 32 | 159.98 | 141.25 | 88.29% | 1630.21 | 2023.42 |
| 随机读 | 1 | 15.10 | 14.51 | 96.07% | 366.59 | 444.42 |
| 随机读 | 32 | 210.72 | 195.99 | 93.01% | 1105.92 | 1368.06 |
| 随机写 | 1 | 15.24 | 14.90 | 97.75% | 350.21 | 411.65 |
| 随机写 | 32 | 183.58 | 145.07 | 79.02% | 1499.14 | 1875.97 |

## 对 Asterinas 实现的直接约束

1. dm-crypt 写路径不能原地修改调用方 BIO；当前实现改为按 segment 精确分配 ciphertext bounce buffer，省去“加密后恢复明文”的第二次变换。
2. 删除 RustCrypto AES 的 `force-soft`，启用基于 CPUID 的 AES-NI 运行时分派，并在串口打印实际后端。
3. dm-verity 对齐 BIO 直接认证已完成的数据，避免重新读取数据块；认证通过的 hash block 才进入有界缓存。
4. 后续 Asterinas 数据必须同时报告吞吐、p95/p99、CPU、hash-cache hit/miss 和 data-reread，且保留逐轮 CSV。

## 可复现命令

```bash
sudo RUNS=5 RUNTIME=10 SIZE_MIB=256 QUEUE_DEPTHS='1 32' \
  bash scripts/benchmark_linux_dm.sh target/benchmark/linux-dm.csv
python3 scripts/summarize_benchmark.py target/benchmark/linux-dm.csv
```

## Asterinas KVM dm-verity cache A/B（2026-07-14）

同一 16 MiB 根文件系统镜像、同一 KVM 环境下，分别运行 `hash_cache=0` 与
`hash_cache=256` 的五轮 fio 矩阵。只比较 dm-verity 行，避免将跨 guest boot 的
raw block 热态差异误判为优化收益。

| cache blocks | dm-verity median MiB/s | p95 us | p99 us | final hash hits/misses |
|---:|---:|---:|---:|---:|
| 0 | 0.320 | 15007.744 | 18219.008 | 0 / 2420 |
| 256 | 0.398 | 13697.024 | 18219.008 | 2785 / 5 |

结论：认证后 hash cache 提升吞吐 **24.4%**，降低 p95 **8.7%**；两组均为
`data_rereads=0`，说明收益来自消除 Merkle hash block 重读而不是跳过数据认证。
逐轮 JSON、CSV 与串口日志见 `docs/evidence/FINAL_VALIDATION.md`。
