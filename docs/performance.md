# 性能测试与优化方案

对外性能数字只允许来自版本化的串口日志和逐轮 CSV。Rust 内存微基准、WSL/Linux
宿主机基准与 Asterinas KVM 基准属于不同测试层级，不得互相冒充。

## 已有可引用证据

| 项目 | 固定条件 | 五轮中位数 |
|---|---|---:|
| Asterinas raw 读取 | KVM、CPU=host、4 KiB、sync、direct=1、QD1、8 MiB、1 s ramp + 2 s runtime | 3.018 MiB/s |
| Asterinas dm-crypt 读取 | 同上，AES-256-XTS，AES-NI | 1.506 MiB/s |
| Asterinas dm-verity，cache=0 | 同一 16 MiB 镜像、4 KiB fio 读 | 0.320 MiB/s |
| Asterinas dm-verity，cache=256 | 同上 | 0.398 MiB/s |

证据位于 `docs/evidence/`。cache=256 相对 cache=0 的吞吐提升为 24.4%，p95
降低 8.7%；两组 `data_rereads=0`。这只能证明 Asterinas 内部 A/B，不能推出
Asterinas 达到 Linux 性能的某个百分比。

## Linux guest 与 Asterinas 同条件对照

“Linux guest 镜像”是用于启动 Linux 的系统盘，不是拿 WSL 数据替代 KVM。做法是：

1. 在同一宿主机用 QEMU/KVM 启动 Linux，配置 `-accel kvm -cpu host -smp 1 -m 8G`；
2. 除 Linux 系统盘外，再挂同一份 raw 数据盘为 virtio-blk；
3. Asterinas 也把这份数据盘挂为 `/dev/vdc`；
4. 两端都对该数据盘前 8 MiB 运行相同 fio 合约。

Linux guest 内运行：

```bash
sudo VCPUS=1 MEMORY=8G ACCELERATOR=kvm CPU_MODEL=host \
  DEVICE_MODEL=virtio-blk RAW_DEVICE=/dev/vdc RUNS=5 SIZE_MIB=8 \
  RUNTIME=2 RAMP_TIME=1 \
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

两条命令除了保护参数与对应镜像外必须保持 QEMU 配置一致。没有生成 CSV 前，
README 和答辩稿继续标注“待测”。

## 本轮代码优化与 A/B 开关

- dm-crypt 使用有界临时缓冲池，`dm_crypt.scratch_pool=32` 默认开启，
  `dm_crypt.scratch_pool=0` 关闭，便于同镜像五轮 A/B；
- dm-verity 对已完成 root hash 认证的 hash block 使用可信后缀捷径，新增
  `auth_shortcuts` 计数；仍先比对当前数据摘要，不跳过数据认证；
- 完整对齐 BIO 直接复用已读取数据，已有 `data_rereads` 计数；
- hash cache 容量继续由 `rootfs_verity.cache_blocks` 控制。

这些代码改动在新的 KVM A/B 数据产生前只标为“已实现、待量化”，不能把历史
cache A/B 数字归因给本轮新代码。

评审 tag 前运行：

```bash
python3 scripts/review_release_check.py
```
