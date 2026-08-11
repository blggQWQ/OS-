# Filebench 基准说明

赛题允许使用 FIO、filebench 或其他工具评估性能。本仓库提供探测脚本与
标准 workload，**不修改 Asterinas 内核**去强行集成 Filebench。

## 探测

```bash
bash scripts/check_filebench.sh
```

报告写入 `docs/evidence/filebench/check-report.txt`。

## 正式执行

优先 workload：`scripts/workloads/fileserver.f`

```bash
# 先将 raw 或 dm-crypt 设备格式化并挂载到 /mnt/cscc-fb
sudo bash scripts/run_filebench_dm.sh --target raw
sudo bash scripts/run_filebench_dm.sh --target dm-crypt
```

`dm-verity` + `fileserver`（含写）默认记为 **N/A**。可改用偏读的
`webserver.f`，或显式 `--allow-verity-write`（不推荐，违反只读语义）。

记录字段：IOPS（ops/s）、throughput（mb/s）、latency（ms/op）、runtime。

## 本次兼容性处理与结果

Ubuntu 24.04 未提供可直接安装的 Filebench 包，因此使用归档的官方
Filebench 1.5-alpha3 源码构建。源码 SHA-256：
`2dedfc46458f5bb13e5f5a4a9f4db6263152f6186690d57653c96138610db35a`。

旧版源码在现代 glibc 下会触发 Fortify 缓冲区检查，并受地址随机化影响。
正式 runner 仅对 Filebench 工具使用无 Fortify 构建，并通过 `setarch -R`
关闭该进程的 ASLR；未修改产品源码、workload、目标文件系统或 60 秒计时区间。

正式结果：

- raw：5/5 PASS；
- dm-crypt：5/5 PASS；
- dm-verity fileserver：N/A（只读目标与写密集 workload 不兼容）。

完整原始日志、metadata 与逐轮 CSV 位于
`docs/evidence/final-performance-20260811/filebench-valid/`，正式汇总见
`docs/evidence/final-performance-20260811/PERFORMANCE_VALIDATION_REPORT.md`。
