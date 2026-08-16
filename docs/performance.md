# 性能测试与 Benchmark

最终性能数据只来自带原始 JSON/CSV/日志、环境元数据和五轮统计的正式证据。Linux QEMU virtio 与 Asterinas KVM 分平台展示，不进行跨平台倍率比较。

- 测试合同：[benchmark_contract.md](benchmark_contract.md)
- 最终中文报告：[PERFORMANCE_VALIDATION_REPORT.md](evidence/final-performance-20260811/PERFORMANCE_VALIDATION_REPORT.md)
- 完整矩阵：[PERFORMANCE_MATRIX.csv](evidence/final-performance-20260811/PERFORMANCE_MATRIX.csv)
- README 直接结果：[性能结果](../README.md#性能结果)

## 完成状态

| 测试 | 正式结果 |
|---|---|
| Asterinas FIO dm-crypt | 13 个单元格 × 5 轮，PASS |
| Asterinas FIO dm-verity | 7 个单元格 × 5 轮，PASS；6 个写单元格 N/A |
| Linux QEMU virtio FIO raw/dm-crypt/dm-verity 读 | 13 + 13 + 7 个单元格 × 5 轮，PASS |
| Filebench fileserver | raw 5 轮、dm-crypt 5 轮，PASS |
| SQLite | raw、dm-crypt、dm-verity 各 5 轮，PASS |
| Asterinas boot | plain、dm-crypt、dm-verity 各 5 轮，PASS |

旧 Asterinas dm-crypt FIO 曾把不存在的 mapper 路径作为普通文件测试，已判定为 `HARNESS_BUG`。旧数据及失败重试不进入仓库正式证据，生成器也不会读取这些目录。

## 正式入口

```bash
# Linux QEMU virtio FIO：raw + dm-crypt + dm-verity 读
# 正式数字见 docs/evidence/linux-qemu-fio-20260816/

# Asterinas KVM FIO
bash scripts/run_fio_matrix.sh --platform asterinas-kvm \
  --asterinas-dir "$PWD/asterinas" --runs 5

# Filebench、SQLite、启动耗时
sudo RUNS=5 bash scripts/run_filebench_matrix.sh
sudo RUNS=5 ROWS=10000 bash scripts/run_sqlite_matrix.sh
RUNS=5 ENABLE_KVM=1 bash scripts/run_boot_matrix.sh

# 汇总与门禁
python3 scripts/summarize_fio_matrix.py docs/evidence/fio-matrix
python3 scripts/summarize_filebench_results.py docs/evidence/filebench
python3 scripts/check_benchmark_package.py
python3 scripts/review_release_check.py
```

## 统计口径

- 每个有效单元格固定 5 轮，报告 mean、median 和 standard deviation，不选择最好一轮。
- FIO `iodepth_set` 只表示配置值；无法取得真实并发度时，`max_in_flight_observed` 必须写为 `unavailable`。
- dm-verity 是只读目标，write/randwrite 和写密集 Filebench workload 记为 N/A，不用零代替。
- Filebench 带宽沿用工具原始 MB/s；FIO 使用 MiB/s。
- SQLite dm-verity 只执行读取阶段，不与 raw/dm-crypt 完整写事务总耗时直接横比。

用户态 `scripts/benchmark.sh` 仅用于算法和映射层微基准，不替代真实 block device 或 KVM 测试。
