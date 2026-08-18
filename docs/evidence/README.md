# 最终证据索引

仓库只保留正式有效结果和复核结论所需的最小证据链。

| 路径 | 内容 | 状态 |
|---|---|---|
| `final-performance-20260811/PERFORMANCE_VALIDATION_REPORT.md` | 中文性能结论、环境与口径 | 正式 |
| `final-performance-20260811/PERFORMANCE_MATRIX.csv` | Linux/Asterinas FIO 全部正式单元格 | 正式 |
| `final-performance-20260811/asterinas-fio/` | 恢复后的 dm-crypt 与修正后的 dm-verity 原始证据 | 正式 |
| `linux-qemu-fio-20260816/` | Linux QEMU virtio raw / dm-crypt / dm-verity FIO JSON | 正式 |
| `linux-qemu-filebench-sqlite-20260817/` | Linux QEMU guest Filebench / SQLite | 正式 |
| `asterinas-linux-initramfs-4g-20260818/` | 4 GiB initramfs：Asterinas vs Linux Filebench / SQLite（新配置，单轮） | 补充对照 |
| `final-performance-20260811/filebench-valid/` | Linux host Filebench | 归档，不再引用 |
| `final-performance-20260811/sqlite/` | Linux host SQLite | 归档，不再引用 |
| `final-performance-20260811/boot-matrix/` | Asterinas 三场景共 15 轮 | 正式 |
| `stabilization/correctness-kvm/` | R01–R03、C01–C05、V02–V07 | 正式 |
| `stabilization/build-recovery/make-kernel-x86_64.log` | kernel build | 正式 |
| `stack/` | dm-crypt → dm-verity 组合栈启动证据 | 正式 |

旧假 mapper、失败重试、调试日志和早期小矩阵已从最终目录移除。真实性问题仍在正式性能报告中留有明确结论，但无效数字不会被再次汇总。
