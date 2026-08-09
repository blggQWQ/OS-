# 性能测试合同（Benchmark Contract）

本文件固定正式性能测试的公平条件与记录要求。对外数字只允许来自版本化的
逐轮原始数据与按本合汇总的 CSV。Rust 内存微基准、宿主机 Linux 基准与
Asterinas KVM 基准属于不同层级，不得互相冒充。

## 1. 平台与虚拟化（Asterinas / Linux guest 对照时）

| 字段 | 固定值 |
|---|---|
| 加速器 | KVM（`-accel kvm`） |
| CPU | `host` 或与现有启动脚本一致的固定模型；两端必须相同 |
| vCPU | 1 |
| 内存 | 8 GiB |
| 块设备 | virtio-blk，raw 镜像 |
| 存储路径 | 同一份数据盘 / 相同尺寸的受测区域 |

每次实验必须在 `metadata.txt` 中记录完整 QEMU 命令行或等价参数表。

## 2. FIO 合同

| 字段 | 正式默认值 |
|---|---|
| `direct` | 1 |
| `numjobs` | 1 |
| `runtime` | 由矩阵驱动写入 metadata（建议 ≥10 s；历史小矩阵可单独标注） |
| `ramp_time` | 由矩阵驱动写入 metadata |
| `runs` | ≥ 5，保留全部原始轮次，**不删除异常值** |
| `ioengine` | **如实记录**：Asterinas guest 现有路径为 `sync`；Linux host 对照可用 `libaio` |

### 2.1 目标矩阵（平台评测优先）

Targets：`raw`、`dm-crypt`、`dm-verity`

Operations：

| operation | raw | dm-crypt | dm-verity |
|---|---|---|---|
| read (seqread) | 测 | 测 | 测 |
| randread | 测 | 测 | 测 |
| write (seqwrite) | 测 | 测 | **N/A**（只读 target） |
| randwrite | 测 | 测 | **N/A** |

Block size：`4k`、`32k`、`256k`

Queue depth：

- 优先：`QD=1`（全矩阵）
- 额外：`4k randread QD=16`
- 扩展（时间允许）：`QD=4` / `QD=16` 其他单元格

### 2.2 并发诚实声明（强制）

- fio 参数 `iodepth=N` **不等于**“系统真实并发度为 N”。
- 字段分离：
  - `iodepth_set`：fio 配置值
  - `max_in_flight_observed`：若项目无法统计则为 `unavailable`；Asterinas
    当前 guest 路径使用 `ioengine=sync` 时应记为 `N/A (sync engine)`。
- **禁止**在 README / 答辩材料中把 `iodepth=16` 写成“真实 max_in_flight=16”。

### 2.3 必记指标

每轮原始数据至少包含：

- Bandwidth（MiB/s）
- IOPS
- p50 / p95 / p99 latency
- CPU utilization（能可靠获取时；否则 `unavailable`）
- `iodepth_set`、`max_in_flight_observed`

跨 5 轮汇总必须输出：**mean、median、standard deviation**。不得剔除异常值。

## 3. 元数据（每次实验）

每个输出目录必须有 `metadata.txt`，至少包含：

```text
git_commit=<sha>
git_dirty=0|1
timestamp_utc=<ISO8601>
platform=asterinas-kvm|linux-host|linux-guest
qemu_command=<...>   # 或逐项参数
image_path=<...>
image_size_bytes=<...>
fio_version=<...>
ioengine=<...>
direct=1
numjobs=1
block_size=<...>
iodepth_set=<...>
operation=<...>
target=<raw|dm-crypt|dm-verity>
runtime_seconds=<...>
ramp_seconds=<...>
runs=<...>
command=<完整驱动命令>
```

## 4. Filebench / SQLite / Boot

- Filebench：优先 `fileserver`；verity 写密集 workload 标 N/A 或改只读说明。
- SQLite：固定 N（默认 10000，可选 50000）；verity 仅 SELECT。
- Boot：`plain` / `dm-crypt` / `dm-verity` 各 ≥5 轮；定义为
  **QEMU 进程启动 → 串口出现统一 PASS marker**
  （默认 `PASS system root switched to protected mapper`）。禁止人工掐秒表。

## 5. 驱动入口

| 场景 | 脚本 |
|---|---|
| FIO 矩阵 | `scripts/run_fio_matrix.sh` |
| FIO 汇总 | `scripts/summarize_fio_matrix.py` |
| Filebench | `scripts/check_filebench.sh` / `scripts/run_filebench_dm.sh` |
| SQLite | `scripts/run_sqlite_workload.sh` |
| Boot 矩阵 | `scripts/run_boot_matrix.sh` |
| 包完整性 | `scripts/check_benchmark_package.py` |

证据根目录：`docs/evidence/`（见该目录 README）。
