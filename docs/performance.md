# 性能测试与 Benchmark

对外性能数字只允许来自版本化的串口/JSON 原始数据与按合同汇总的 CSV。
Rust 内存微基准、WSL/Linux 宿主机基准与 Asterinas KVM 基准属于不同测试层级，
不得互相冒充。

**正式合同**见 [benchmark_contract.md](benchmark_contract.md)。  
**证据索引**见 [evidence/README.md](evidence/README.md)。

本仓库当前任务是把测量脚本、参数与目录整理完整；Asterinas 内核封板后按同一
合同直接跑矩阵即可。不在本文承诺“达到 Linux 的某百分比”，除非存在同条件
对照 metadata 与 CSV。

## 一键入口

| 场景 | 命令 |
|---|---|
| FIO 矩阵（Linux host） | `sudo bash scripts/run_fio_matrix.sh --platform linux-host` |
| FIO 矩阵（Asterinas） | `bash scripts/run_fio_matrix.sh --platform asterinas-kvm --asterinas-dir <path>` |
| FIO 汇总 | `python3 scripts/summarize_fio_matrix.py docs/evidence/fio-matrix` |
| Boot 矩阵 | `bash scripts/run_boot_matrix.sh` |
| Filebench 探测 | `bash scripts/check_filebench.sh` |
| SQLite | `bash scripts/run_sqlite_workload.sh --target raw --rows 10000` |
| 包完整性 | `python3 scripts/check_benchmark_package.py` |

## 正式 FIO 矩阵（待跑）

Targets：`raw` / `dm-crypt` / `dm-verity`  
Ops：`read` / `randread` / `write` / `randwrite`（verity 写为 **N/A**）  
BS：`4k` / `32k` / `256k`  
QD：优先 `1`；额外 `4k randread QD=16`  
每格 ≥5 轮；汇总 **mean / median / stdev**；不删异常值。

诚实字段：

- `iodepth_set` = fio 配置
- `max_in_flight_observed` = 实测或 `unavailable` / `N/A (sync engine)`

Asterinas guest 现有路径使用 `ioengine=sync` 时，`iodepth=16` 只表示 fio
配置值；`max_in_flight_observed` 应记为 `N/A (sync engine)`，不可夸大并发。

驱动复用 guest 环境变量（不改内核补丁源码）：

`DM_BENCH_RW`、`DM_BENCH_BS`、`DM_BENCH_QD`、`DM_BENCH_SIZE`、
`DM_BENCH_RUNTIME`、`DM_BENCH_RUNS`、`DM_BENCH_RAMP`。

## 历史证据（非正式全矩阵）

| 项目 | 固定条件 | 五轮中位数 |
|---|---|---:|
| Asterinas raw 读取 | KVM、4 KiB、sync、QD1、8 MiB、短 runtime | 3.018 MiB/s |
| Asterinas dm-crypt 读取 | 同上，AES-NI | 1.506 MiB/s |
| Asterinas dm-verity，cache=0 | 16 MiB、4 KiB | 0.320 MiB/s |
| Asterinas dm-verity，cache=256 | 同上 | 0.398 MiB/s |

证据位于 `docs/evidence/`。cache=256 相对 cache=0 吞吐 +24.4%、p95 −8.7%，
且 `data_rereads=0`。这只证明 Asterinas 内部 A/B，**不能**推出相对 Linux 的百分比。

## Linux guest 与 Asterinas 同条件对照（历史小矩阵）

做法：同一宿主机、KVM、1 vCPU、8 GiB、同一 raw 数据盘前 8 MiB、相同 fio 合约。

```bash
sudo VCPUS=1 MEMORY=8G ACCELERATOR=kvm CPU_MODEL=host \
  DEVICE_MODEL=virtio-blk RAW_DEVICE=/dev/vdc RUNS=5 SIZE_MIB=8 \
  RUNTIME=2 RAMP_TIME=1 \
  bash scripts/benchmark_linux_guest_dm_crypt.sh \
  target/benchmark/linux-guest-dm-crypt.csv

cd asterinas
DM_IMAGE=../target/dm-crypt-test.img ENABLE_KVM=1 SMP=1 MEM=8G \
  DM_BENCH_RW=read DM_BENCH_BS=4k DM_BENCH_QD=1 \
  DM_BENCH_SIZE=8m DM_BENCH_RUNTIME=2 DM_BENCH_RAMP=1 \
  DM_BENCH_RUNS=5 bash tools/run_dm_crypt_bench.sh \
  | tee ../target/benchmark/asterinas-dm-crypt-kvm.log

python3 scripts/compare_cross_guest_dm_crypt.py \
  target/benchmark/linux-guest-dm-crypt.csv \
  target/benchmark/asterinas-dm-crypt-kvm.csv \
  docs/evidence/linux-vs-asterinas-dm-crypt.csv \
  docs/evidence/linux-vs-asterinas-dm-crypt.md \
  --linux-metadata target/benchmark/linux-guest-dm-crypt.metadata.txt \
  --asterinas-metadata target/benchmark/asterinas-dm-crypt.metadata.txt
```

比较器强制核对 vCPU、内存、加速器、设备、区域 SHA-256 与 fio 参数。

## Filebench / SQLite / Boot

- Filebench：见 [filebench.md](filebench.md)
- SQLite：见 [sqlite_benchmark.md](sqlite_benchmark.md)
- Boot：`scripts/run_boot_matrix.sh`；marker 统一定义见合同

## 用户态微基准（非 FIO）

```bash
bash scripts/benchmark.sh
cargo run -p dmctl -- benchmark-table
```

仅用于算法/映射层微基准，不得替代 KVM fio 表。

## 评审前检查

```bash
python3 scripts/check_benchmark_package.py
python3 scripts/review_release_check.py
```
