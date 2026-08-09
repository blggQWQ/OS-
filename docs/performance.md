# 性能测试与优化方案

对外性能数字只允许来自版本化的串口日志和逐轮 CSV。Rust 内存微基准、WSL/Linux
宿主机基准与 Asterinas KVM 基准属于不同测试层级，不得互相冒充。

## 正式 256 MiB 性能合同与阶段归因

本轮正式证据统一使用以下不可省略的合同：

| 字段 | 固定值 |
|---|---|
| 内核构建 | `release-lto` |
| 虚拟化 | KVM、CPU=host、1 vCPU、8 GiB |
| 块设备 | virtio-blk，raw 容器均为 264 MiB |
| 受测数据区 | 前 256 MiB |
| fio | read、4 KiB、sync、direct=1、QD1 |
| 时长 | 2 s ramp + 10 s runtime |
| 重复 | raw/target 交替顺序，各五轮 |

构建并执行完整矩阵：

```bash
sudo bash scripts/run_formal_dm_benchmark.sh --build-images
```

启动器把 commit、dirty 状态、构建 profile、KVM/CPU、镜像大小和 fio 参数打印到
同一份串口日志，并写入 metadata。dm-crypt 日志还必须同时出现
`backend=AES-NI`；缺少 release-LTO、AES-NI 或恰好五轮时，汇总器拒绝生成
正式结果。

内核参数 `dm.perf_stats=1` 开启分阶段累计计数。guest 在每轮 fio 前后分别读取
`/proc/dm-performance-stats`，`summarize_dm_performance_stages.py` 使用差值生成：

- dm-crypt：参数检查、scratch pool、数据复制、AES-XTS、下层等待；
- dm-verity：数据等待、数据 SHA-256、cache 查找、哈希块等待、哈希块 SHA-256、
  摘要比较、cache 插入和树层数。

汇总器还输出 `accounted_ns` 与 `unaccounted_ns`。后者是单次 I/O 总时间减去所有
已命名阶段之和，包含计时调用、原子计数更新和函数衔接等未单独命名的开销，防止阶段
百分比看似完整、实际却没有闭合。

所有计时只累计到原子计数器，每轮结束才由 procfs 输出，热路径不打印串口。使用
下列命令校准观察者开销：

```bash
bash scripts/run_dm_stats_calibration.sh crypt
bash scripts/run_dm_stats_calibration.sh verity
```

`compare_dm_stats_overhead.py` 输出 stats=0/1 的五轮中位吞吐差异。绝对开销超过 3%
时，阶段数据只用于定性定位，不进入正式百分比归因，后续应改成抽样计时。

## 本轮正式结果

2026-08-05 的干净提交 `f9da5469f` release-LTO/KVM 五轮结果已归档到
`docs/evidence/formal-256m/`：

| 设备 | 中位吞吐（MiB/s） | 中位 p50（us） | 中位 p95（us） | 相对配对 raw |
|---|---:|---:|---:|---:|
| raw（两组 raw 共十轮） | 4.249 | 892.928 | 1277.952 | 100.0% |
| dm-crypt | 4.059 | 921.600 | 1318.912 | 95.6% |
| dm-verity | 3.815 | 954.368 | 1433.600 | 87.3% |

raw 行用于展示统一量级，是两组共十轮的中位数；每个 target 的百分比使用各自配对
raw 的五轮中位数。正式结果显示 dm-crypt 吞吐损失 4.4%，dm-verity 为 12.7%；不把
pooled raw 行直接作为任一 target 的百分比基线。

| target | 最大阶段（五轮占比中位数） | 次大阶段 | 结论 |
|---|---:|---:|---|
| dm-crypt | 下层等待 97.84% | AES-XTS 1.82% | AES-NI 与复制不是主要瓶颈 |
| dm-verity | 下层等待 93.01% | 数据 SHA-256 5.74% | cache/哈希树查找不是主要瓶颈 |

五轮未归因占比范围为 dm-crypt 0.14%--0.15%、dm-verity 0.23%--0.33%，说明当前
阶段划分覆盖了 99.67% 以上的 target wall time；这部分余量不被归入 AES、SHA 或
下层设备，以免夸大任何一个阶段。

这轮定位把后续优化范围收敛到同步 `submit_and_wait` 架构。为验证这个判断，实验分支
`codex/dm-async-bio`（提交 `a8fc0238a`）只改 dm-crypt：下层 BIO 改为完成回调，写路径
先加密再提交，读路径在提交前取得 scratch，回调中解密并完成上层 BIO；dm-verity 保持
不变。回调不睡眠、不分配，并用 `Option<SubmittedBio>` 保证上层 BIO 只完成一次。

实验版通过 release-LTO 构建、下层“先完成后返回”的竞态 ktest，以及真实 KVM
AES-NI 加密读回；相同 256 MiB/10 s 合同的 after 五轮也全部 `failures=0`：

| 阶段 | raw 中位 MiB/s | dm-crypt 中位 MiB/s | 逐轮配对效率中位 | dm-crypt p95 us |
|---|---:|---:|---:|---:|
| before `f9da5469f` | 4.245 | 4.059 | 97.40% | 1318.912 |
| async after `a8fc0238a` | 6.308 | 6.049 | 96.80% | 937.984 |

绝对 dm-crypt 吞吐增加 49.0%，但 raw 同时增加 48.6%；raw 归一化后的效率变化为
-0.62%，不能证明异步带来吞吐收益。p95 下降也伴随 raw 延迟整体下降，不能单独归因。
因此该实现与五轮证据保留用于说明工程探索和严谨否证，暂不建议替换正式基线。证据见
`docs/evidence/formal-256m-async-after/`，比较器为
`scripts/compare_dm_crypt_async_ab.py`。下一步若继续，应使用同一进程/时间窗口交错提交
版本，或把解密从 IRQ 完成回调迁移到专用 worker，再重新做五轮 A/B。

## 已有历史证据

| 项目 | 固定条件 | 五轮中位数 |
|---|---|---:|
| Asterinas raw 读取 | KVM、CPU=host、4 KiB、sync、direct=1、QD1、8 MiB、1 s ramp + 2 s runtime | 3.018 MiB/s |
| Asterinas dm-crypt 读取 | 同上，AES-256-XTS，AES-NI | 1.506 MiB/s |
| Asterinas dm-verity，cache=0 | 同一 16 MiB 镜像、4 KiB fio 读 | 0.320 MiB/s |
| Asterinas dm-verity，cache=256 | 同上 | 0.398 MiB/s |
| Linux guest dm-crypt | KVM、CPU=host、1 vCPU、8 GiB、同一 256 MiB 测试区 | 4.746 MiB/s |
| Asterinas dm-crypt（严格对照轮次） | 与 Linux guest 相同 I/O 合约和数据区 | 4.059 MiB/s |

证据位于 `docs/evidence/`。cache=256 相对 cache=0 的吞吐提升为 24.4%，p95
降低 8.7%；两组 `data_rereads=0`。这只能证明 Asterinas 内部 A/B，不能推出
Asterinas 达到 Linux 性能的某个百分比。严格同条件对照数据见下一节。

## Linux guest 与 Asterinas 同条件对照

“Linux guest 镜像”是用于启动 Linux 的系统盘，不是拿 WSL 数据替代 KVM。做法是：

1. 在同一宿主机用 QEMU/KVM 启动 Linux，配置 `-accel kvm -cpu host -smp 1 -m 8G`；
2. 除 Linux 系统盘外，再挂同一份 raw 数据盘为 virtio-blk；
3. Asterinas 也把这份数据盘挂为 `/dev/vdc`；
4. 两端都对该数据盘前 256 MiB 运行相同 fio 合约。

Linux guest 内运行：

```bash
sudo VCPUS=1 MEMORY=8G ACCELERATOR=kvm CPU_MODEL=host \
  DEVICE_MODEL=virtio-blk RAW_DEVICE=/dev/vdc RUNS=5 SIZE_MIB=256 \
  RUNTIME=10 RAMP_TIME=2 \
  bash scripts/benchmark_linux_guest_dm_crypt.sh \
  target/benchmark/linux-guest-dm-crypt.csv
```

Asterinas 端运行：

```bash
cd asterinas
DM_IMAGE=../target/dm-crypt-test.img ENABLE_KVM=1 SMP=1 MEM=8G \
  DM_BENCH_RW=read DM_BENCH_BS=4k DM_BENCH_QD=1 \
  DM_BENCH_SIZE=8m DM_BENCH_RUNTIME=2 DM_BENCH_RAMP=1 \
  DM_BENCH_RUNS=5 bash tools/run_dm_crypt_bench.sh \
  | tee ../target/benchmark/asterinas-dm-crypt-kvm.log
```

汇总后生成直接对照：

```bash
python3 scripts/compare_cross_guest_dm_crypt.py \
  target/benchmark/linux-guest-dm-crypt.csv \
  target/benchmark/asterinas-dm-crypt-kvm.csv \
  docs/evidence/linux-vs-asterinas-dm-crypt.csv \
  docs/evidence/linux-vs-asterinas-dm-crypt.md \
  --linux-metadata target/benchmark/linux-guest-dm-crypt.metadata.txt \
  --asterinas-metadata target/benchmark/asterinas-dm-crypt.metadata.txt
```

比较器会强制核对 vCPU、内存、加速器、CPU 模型、设备模型、测试区域 SHA-256、
密码参数和 fio 参数；任一项不同就拒绝出表。Linux guest 的 fio 版本和实际 CPU/
内存信息也记录在 metadata 中，人工复核时应一并归档。

本次实测的 Linux guest 与 Asterinas 原始矩阵均为五轮，测试区域 SHA-256 同为
`d1a3f55df4e589f052946b83e08a41a192c84fca4b268bf8bb5b294cc69c584f`：

| 系统 | raw MiB/s | dm-crypt MiB/s | 保留率 | dm-crypt p95 us | dm-crypt p99 us |
|---|---:|---:|---:|---:|---:|
| Linux guest | 5.150 | 4.746 | 92.158% | 1384.448 | 2998.272 |
| Asterinas | 4.245 | 4.059 | 95.621% | 1318.912 | 2023.424 |

Asterinas raw、dm-crypt 绝对吞吐分别为 Linux guest 的 82.4% 和 85.5%。95.621%
是相对 Asterinas 自身 raw 的保留率；它高于 Linux 的 92.158%，表示本轮固定合同中
测得的增量 dm 层损耗较小，不表示 Asterinas 绝对吞吐超过 Linux。原始 CSV、metadata
和比较表均位于 `docs/evidence/`；该结论只适用于表中固定的同步读矩阵。

## 扩展 fio 矩阵

Asterinas 两个启动器均支持 `DM_BENCH_RW`、`DM_BENCH_BS`、`DM_BENCH_QD`、
`DM_BENCH_SIZE`、`DM_BENCH_RUNTIME`、`DM_BENCH_RUNS` 和 `DM_BENCH_RAMP`。
必须至少覆盖：

| 目标 | 负载 | 块大小 | QD |
|---|---|---:|---:|
| raw / dm-crypt | read、write、randread、randwrite | 4 KiB | 1、32 |
| raw / dm-verity | read、randread | 4 KiB | 1、32 |

示例：

```bash
DM_BENCH_RW=randread DM_BENCH_QD=32 DM_BENCH_RUNS=5 \
  bash asterinas/tools/run_dm_crypt_bench.sh \
  | tee target/benchmark/asterinas-crypt-randread-qd32.log
python3 scripts/summarize_asterinas_dm_matrix.py \
  target/benchmark/asterinas-crypt-randread-qd32.log \
  target/benchmark/asterinas-crypt-randread-qd32.csv \
  target/benchmark/asterinas-crypt-randread-qd32-summary.csv
```

dm-verity 是只读映射，启动脚本会拒绝 write/randwrite。每个工况至少五轮并报告
中位数、p50、p95、p99；不要用单次截图代替 CSV。

## 启动时间

`scripts/benchmark_boot_time.py` 交替运行 plain 与 dm-verity，记录每行宿主机单调
时钟时间戳，并输出五轮原始 CSV 与中位数：

```bash
python3 scripts/benchmark_boot_time.py \
  --plain-command "<plain QEMU command>" \
  --protected-command "<dm-verity QEMU command>" \
  --plain-marker "rootfs is ready" \
  --protected-marker "PASS protected mapper root switch" \
  --runs 5 --output target/benchmark/boot-time.csv
```

两条命令除了保护参数与对应镜像外保持 QEMU 配置一致。本次交替顺序五轮实测：

| 场景 | 五轮中位数 |
|---|---:|
| plain root | 16.058 s |
| dm-verity root | 16.902 s |

dm-verity 到 root-switch PASS 的中位增量为 5.256%。逐轮 CSV、汇总和十份串口日志
位于 `docs/evidence/`。

## 本轮代码优化、测量与 A/B 开关

- dm-crypt 使用有界临时缓冲池，`dm_crypt.scratch_pool=32` 默认开启，
  `dm_crypt.scratch_pool=0` 关闭，便于同镜像五轮 A/B；
- dm-verity 对已完成 root hash 认证的 hash block 使用可信后缀捷径，新增
  `auth_shortcuts` 计数；仍先比对当前数据摘要，不跳过数据认证；
- 完整对齐 BIO 直接复用已读取数据，已有 `data_rereads` 计数；
- hash cache 容量继续由 `rootfs_verity.cache_blocks` 控制。
- `dm.perf_stats=0/1` 控制内核阶段归因，前后快照消除启动自检与跨轮累计污染；
- `run_formal_dm_benchmark.sh` 强制 release-LTO、256 MiB/10 秒/五轮合同，并生成
  raw/dm-crypt/dm-verity 三行表；
- 阶段数据产生后，只允许针对占比最高且安全边界可证明的一处做优化，使用相同合同
  保存 before/after 五轮 CSV，不同时叠加多项热路径修改。

这些代码改动在新的 KVM A/B 数据产生前只标为“已实现、待量化”，不能把历史
cache A/B 数字归因给本轮新代码。

评审 tag 前运行：

```bash
python3 scripts/review_release_check.py
```
