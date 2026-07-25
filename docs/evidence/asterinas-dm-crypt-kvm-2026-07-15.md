# Asterinas dm-crypt KVM 五轮基准（2026-07-15）

## 结论

在同一 Asterinas KVM 客体、同一加密镜像和 4 KiB/QD1/direct 顺序读条件下，
dm-crypt 的中位吞吐为 **1.506 MiB/s**，原始块设备为 **3.018 MiB/s**，即保留
**49.9%** 吞吐；p95 延迟从 **1892.352 us** 升至 **3620.864 us**（+91.3%）。

五轮 raw 和五轮 dm-crypt 均在串口日志中确认 `backend=AES-NI`，因此该数据不是
便携软件 AES 的结果。

| 标签 | 轮数 | 后端 | 中位吞吐 MiB/s | 中位 IOPS | 中位 p95 us | 中位 p99 us |
|---|---:|---|---:|---:|---:|---:|
| raw | 5 | AES-NI | 3.018 | 772.727 | 1892.352 | 2965.504 |
| dm-crypt | 5 | AES-NI | 1.506 | 385.614 | 3620.864 | 5079.040 |

## 可复现性

- Asterinas 官方容器：`asterinas/asterinas:0.18.0-20260618`
- 加速：KVM，CPU 模型 `host`
- 密码：AES-256-XTS / plain64 IV；测试镜像由 `scripts/build_crypt_rootfs.sh` 生成
- fio：4 KiB、`sync`、direct I/O、QD1、8 MiB、1 秒 ramp、2 秒 runtime、各 5 轮
- 原始日志：`target/benchmark/asterinas-dm-crypt-kvm.log`
- 版本化逐轮数据：`asterinas-dm-crypt-kvm.csv`
- 汇总：`asterinas-dm-crypt-kvm-summary.csv`

运行结束时日志含 `PASS five-run raw/dm-crypt matrix completed`，并由汇总器校验
必须恰有 5 条 raw 与 5 条 dm-crypt 记录。

## 对 Linux 的边界

现有 `linux-dm-summary.csv` 使用的是 Linux/WSL 的更大矩阵（QD1/QD32、顺序/随机
读写、10 秒运行），可用作 Linux 行为与性能上界参考；**不能**与本页 2 秒 QD1
Asterinas KVM 数据直接比较绝对 MiB/s。

本轮已经补齐了 Asterinas 端的真实、可重复基线。下一步的严格跨系统结论是：使用
同一 QEMU/KVM 虚拟磁盘和本页 fio 参数运行 Linux guest，再将两份五轮中位数并排。
