# Linux guest vs Asterinas dm-crypt 同条件五轮对照

共同条件已由脚本逐项核对：kvm、CPU=host、1 vCPU、内存 8G、virtio-blk；
aes-xts-plain64；fio 4k、read、sync、direct=1、QD1、8 MiB、ramp 1 秒、runtime 2 秒；每个目标 5 轮并取中位数。
测试区域 SHA-256：`93ff1ab9732d787a984f56c1cdb5f3601d9f55ce582951266624df69114f944d`。

| 系统 | raw MiB/s | dm-crypt MiB/s | 保留率 | dm-crypt IOPS | p50 us | p95 us | p99 us |
|---|---:|---:|---:|---:|---:|---:|---:|
| Linux guest | 4.652 | 4.355 | 93.629% | 1114.943 | 847.872 | 1335.296 | 2342.912 |
| Asterinas | 1.737 | 0.523 | 30.094% | 133.799 | 5472.256 | 15663.104 | 22937.600 |

Asterinas dm-crypt 的绝对吞吐为 Linux guest 的 **12.0%**。该比例仅在上述条件全部
由两端 metadata 和原始 CSV 核对一致时有效；条件不一致时脚本会拒绝出表。
