# Linux guest vs Asterinas dm-crypt 同条件五轮对照

共同条件已由脚本逐项核对：kvm、CPU=host、1 vCPU、内存 8G、virtio-blk；
aes-xts-plain64；fio 4k、read、sync、direct=1、QD1、256 MiB、ramp 2 秒、runtime 10 秒；每个目标 5 轮并取中位数。
测试区域 SHA-256：`d1a3f55df4e589f052946b83e08a41a192c84fca4b268bf8bb5b294cc69c584f`。

| 系统 | raw MiB/s | dm-crypt MiB/s | 保留率 | dm-crypt IOPS | p50 us | p95 us | p99 us |
|---|---:|---:|---:|---:|---:|---:|---:|
| Linux guest | 5.150 | 4.746 | 92.158% | 1215.078 | 684.032 | 1384.448 | 2998.272 |
| Asterinas | 4.245 | 4.059 | 95.621% | 1039.196 | 921.600 | 1318.912 | 2023.424 |

Asterinas raw 和 dm-crypt 的绝对吞吐分别为 Linux guest 的 **82.4%** 和 **85.5%**。
保留率以各系统自身 raw 为基线；Asterinas 的保留率较高不代表绝对吞吐超过 Linux。
以上比例仅在两端 metadata 和原始 CSV 全部核对一致时有效；条件不一致时脚本会拒绝出表。
