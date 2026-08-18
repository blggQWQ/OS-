# 4 GiB initramfs 对照：Asterinas vs Linux（2026-08-18）

新配置，**不是** 8 GiB 正式 README 的 Filebench / SQLite 五轮表。两端跑 Asterinas 同一套 initramfs job，测的是 virtio-blk 上的 **ext2**，不是 dm-crypt / dm-verity mapper，不能和 Linux guest 三 target 表横比。

来源：本地 `retest/fb-sqlite-4g-v2/`。单轮，不取中位数。

## 配置

| 项 | 本表 |
|---|---|
| 日期 | 2026-08-18（UTC 起始 2026-08-17T17:32:35Z） |
| 加速 | KVM |
| 内存 | 4 GiB |
| vCPU | 1 |
| CPU 模型 | `Icelake-Server`（`-pcid,+x2apic`） |
| QEMU 锁页 | 关（无 mlock / prealloc） |
| 块设备 | virtio-blk，ext2 镜像（Asterinas 兼容 feature：`^ext_attr ^resize_inode ^dir_index`） |
| Asterinas 启动 | `make kernel BENCHMARK=… SMP=1 MEM=4G ENABLE_KVM=1 RELEASE_LTO=1 NETDEV=user` |
| Linux 对照 | 同一 initramfs job：`-kernel` + `-initrd`，`rdinit=/benchmark/common/bench_runner.sh` |
| 网卡 | virtio-net user |
| 轮数 | 各 1 轮 |

### SQLite

`sqlite/ext2_benchmarks`：`sqlite-speedtest1 --size 1000 /ext2/test.db`。指标为工具输出的 **TOTAL** 秒，越小越好。

### Filebench

`filebench/ext2_benchmarks`：Filebench 1.5-alpha3，CSCC fileserver。日志确认：10000 文件、dirwidth=20、总大小 1250.000 MB、runtime 60 s、1 个 filereader 进程。沿用工具原始 MB/s 与 ms/op。

## 主结果（无 IOMMU，`pair/`）

| 负载 | Asterinas | Linux initramfs | 相对 |
|---|---:|---:|---|
| SQLite TOTAL（s） | 244.649 | 94.040 | Asterinas 为 Linux 的 **2.60×**（更慢） |
| Filebench ops/s | 8398.724 | 11545.120 | Asterinas 为 Linux 的 **72.7%** |
| Filebench MB/s | 201.9 | 278.5 | Asterinas 为 Linux 的 **72.5%** |
| Filebench ms/op | 4.3 | 3.1 | — |

## SQLite 第二轮（`pair-iommu/`）

Asterinas `SCHEME=iommu`；Linux 仍未开 `intel-iommu`。各 1 轮。Linux TOTAL 也从 94.040 变到 55.302（宿主机噪声），**不能**单独归因于 IOMMU。

| | Asterinas TOTAL s | Linux TOTAL s | 倍率 |
|---|---:|---:|---:|
| 无 IOMMU（`pair/`） | 244.649 | 94.040 | 2.60× |
| `SCHEME=iommu`（`pair-iommu/`） | 111.956 | 55.302 | 2.02× |
