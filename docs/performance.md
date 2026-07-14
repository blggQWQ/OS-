# 性能测试方案

项目现已提供可复现的 Rust 微基准、Linux raw/dm-crypt 配对基准和结果汇总工具。
最终 Linux/Asterinas 对比必须在相同的 QEMU 配置和磁盘镜像下执行。

## 测试指标

- 顺序读取吞吐量；
- `dm-crypt` 顺序写入吞吐量；
- 随机 4 KiB 读写 IOPS；
- 启用和未启用 `dm-verity` 时的读取延迟；
- 启用和未启用受保护 rootfs 时的启动时间。

## Rust 基准

```bash
BLOCKS=16384 ITERATIONS=7 bash scripts/benchmark.sh target/benchmark/rust.csv
```

CSV 包含普通内存块 I/O、AES-256-XTS 读写和 verity 读取吞吐量。计时区域
不包含 mapper 构造过程，并加入读取校验和，防止相关操作被优化掉。

内存测试结果不能作为 Linux 性能对比数据，它只用于提供稳定的性能回归基线。

## Linux/Asterinas 对比

Linux 对照使用一键脚本。脚本以同一个 loop device 为后端创建
`aes-xts-plain64` mapper，交替运行 raw 与 dm-crypt，减少运行顺序和温度偏差：

```bash
sudo SIZE_MIB=512 RUNTIME=20 RUNS=5 \
  bash scripts/benchmark_linux_dm.sh target/benchmark/linux-dm.csv
python3 scripts/summarize_benchmark.py target/benchmark/linux-dm.csv
```

原始 CSV 每一行对应一轮 fio，包含 MiB/s、IOPS、平均延迟、p50/p95/p99 和 CPU
占用。summary CSV 使用中位数，并计算 dm-crypt 相对 raw 的吞吐保留率与 p95 延迟
开销。原始 JSON 位于 `target/benchmark/linux-work`，答辩后也应保留。

两端必须使用相同的虚拟 CPU 数量、内存、QEMU 缓存与 AIO 模式、镜像、块大小、
队列深度、运行时间和 fio 版本。每项结果旁应记录完整命令。

必测矩阵：

| 操作系统 | 目标 | 负载 | 块大小 | 队列深度 |
| --- | --- | --- | ---: | ---: |
| Linux | 普通 / dm-crypt / dm-verity | 顺序读写、随机读写 | 4 KiB | 1 和 32 |
| Asterinas | 普通 / dm-crypt / dm-verity | 同上 | 4 KiB | 1 和 32 |

预热一次后至少正式运行五次，报告中位数及离散程度。不能只记录吞吐量，还应保留
p50、p95、p99 延迟和 CPU 利用率。

启动时间从 QEMU 启动开始计时，到首次出现用户态提示符为止，分别记录：

```text
普通 rootfs
dm-verity 保护的 rootfs
```

## 优化方向

- 已实现：复用完整对齐 BIO 中的数据，避免 verity 校验重复读取 data block；
- 已实现：只缓存完成 root hash 认证后的 verity hash block；
- 已实现：输出 BIO 复用、数据补读、hash cache 命中/缺失计数；
- 下一步：crypt 写路径使用独立 bounce buffer 池，避免修改并恢复原 BIO；
- 构建 verity tree 时批量读取相邻块；
- 生产构建保留 AES-NI，并单独测试软件回退路径；
- 复用每次 I/O 的临时缓冲区，避免反复分配 `Vec`。

## 报告表格模板

| 模式 | 读取 MB/s | 写入 MB/s | IOPS | 平均延迟 | 启动时间 |
| --- | ---: | ---: | ---: | ---: | ---: |
| 普通 virtio-blk | 待测试 | 待测试 | 待测试 | 待测试 | 待测试 |
| dm-crypt | 待测试 | 待测试 | 待测试 | 待测试 | 不适用 |
| dm-verity | 待测试 | 不适用 | 待测试 | 待测试 | 待测试 |
