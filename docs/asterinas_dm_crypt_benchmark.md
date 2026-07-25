# Asterinas dm-crypt / Linux dm-crypt 同条件对照

本页定义补齐国赛性能主张所需的 Asterinas dm-crypt 数据采集流程。结果必须由
KVM 实跑生成；在此之前不得把 Linux 基线中的百分比描述为 Asterinas 性能。

## 共同约束

- AES-256-XTS、plain64 IV、4 KiB I/O；
- 每个工况五轮，保留逐轮 CSV，使用中位数；
- 记录吞吐、IOPS、p95/p99 延迟和实际 AES 后端；
- Linux 与 Asterinas 使用同一宿主机、KVM CPU 模型和原始镜像。

Linux 的完整 QD1/QD32、顺序/随机读写矩阵由下列命令生成：

```bash
sudo RUNS=5 RUNTIME=10 SIZE_MIB=256 QUEUE_DEPTHS='1 32' \
  bash scripts/benchmark_linux_dm.sh target/benchmark/linux-dm.csv
python3 scripts/summarize_benchmark.py target/benchmark/linux-dm.csv
```

## Asterinas dm-crypt 五轮矩阵

先生成与启动测试一致的加密 ext2 镜像，再从 Asterinas 工作树运行：

```bash
sudo bash scripts/build_crypt_rootfs.sh target/dm-crypt-test.img
cd asterinas
DM_IMAGE=../target/dm-crypt-test.img ENABLE_KVM=1 \
  bash tools/run_dm_crypt_bench.sh \
  | tee ../target/benchmark/asterinas-dm-crypt-kvm.log
cd ..
python3 scripts/summarize_asterinas_dm_crypt_bench.py \
  target/benchmark/asterinas-dm-crypt-kvm.log \
  docs/evidence/asterinas-dm-crypt-kvm.csv \
  docs/evidence/asterinas-dm-crypt-kvm-summary.csv
```

该启动器在同一 KVM 客体依次读取原始 `/dev/vdc` 和 `/dev/dm-crypt0`，各五轮，
每轮输出 fio JSON。汇总器会拒绝缺少任一轮或标签不匹配的日志，并将 AES-NI / 软件
后端写入 CSV。因此，这份数据可作为“Rust dm-crypt 相对 Linux 基线”的正式证据，
而不是单次截图。

## 当前状态

采集和汇总链路已于 2026-07-15 在 KVM 完成五轮实跑，结果见
`docs/evidence/asterinas-dm-crypt-kvm-2026-07-15.md` 与同目录 CSV。下一步将 Linux
guest 放入相同 QEMU/KVM 虚拟磁盘和 fio 参数，形成严格的跨系统绝对吞吐对照；在此之前，
明确区分“原始设备归一化开销”和“跨内核绝对吞吐”。
