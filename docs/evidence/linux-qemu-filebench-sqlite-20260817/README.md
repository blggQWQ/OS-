# Linux QEMU/KVM Filebench 与 SQLite（2026-08-17）

正式 Linux Filebench / SQLite 对照。平台 `linux-qemu-kvm`，virtio `/dev/vdb`，五轮中位数。不要和 `final-performance-20260811/filebench-valid/`、`sqlite/` 的 Linux host 旧数混读，也不要和 Asterinas 启动耗时混成同一口径。

来源：`result/cscc-bench-results.tar.gz`。Filebench：fileserver，2000 文件，50 线程，60 s，`aes-xts-plain64`。SQLite：10000 行。中位取五轮排序后的中间值，不剔除异常值。

| 测试 | raw | dm-crypt | dm-verity |
|---|---:|---:|---|
| Filebench ops/s | 27855.155 | 25666.016 | N/A（只读） |
| Filebench MB/s | 656.9 | 605.0 | N/A（只读） |
| Filebench ms/op | 3.3 | 3.7 | N/A |
| SQLite 总耗时 s | 0.138095 | 0.143336 | 0.001151（仅查询） |

SQLite dm-verity 为 `ok_readonly`，只有 SELECT，不可与 raw/crypt 完整事务总耗时横比。
