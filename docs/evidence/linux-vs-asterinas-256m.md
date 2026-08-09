# Linux guest vs Asterinas dm-crypt 同条件五轮对照

共同条件已由脚本逐项核对：kvm、CPU=host、1 vCPU、内存 8G、virtio-blk；
aes-xts-plain64；fio 4k、read、sync、direct=1、QD1、256 MiB、ramp 2 秒、runtime 10 秒；每个目标 5 轮并取中位数。
测试区域 SHA-256：`62d015b319d06e76b40d2c0b81537ab1a7c903d04bed38757c289971cb0c18b9`。

| 系统 | raw MiB/s | dm-crypt MiB/s | 保留率 | dm-crypt IOPS | p50 us | p95 us | p99 us |
|---|---:|---:|---:|---:|---:|---:|---:|
| Linux guest | 46.092 | 42.995 | 93.280% | 11006.699 | 71.168 | 214.016 | 337.920 |
| Asterinas | 24.230 | 23.280 | 96.079% | 5960.000 | 140.288 | 280.576 | 428.032 |

Asterinas dm-crypt 的绝对吞吐为 Linux guest 的 **54.1%**。该比例仅在上述条件全部
由两端 metadata 和原始 CSV 核对一致时有效；条件不一致时脚本会拒绝出表。
