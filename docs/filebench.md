# Filebench 基准说明

赛题允许使用 FIO、filebench 或其他工具评估性能。本仓库提供探测脚本与
标准 workload，**不修改 Asterinas 内核**去强行集成 Filebench。

## 探测

```bash
bash scripts/check_filebench.sh
```

报告写入 `docs/evidence/filebench/check-report.txt`。

## 若已安装

优先 workload：`scripts/workloads/fileserver.f`  
可选：`webserver.f`、`varmail.f`

```bash
# 先将 raw 或 dm-crypt 设备格式化并挂载到 /mnt/cscc-fb
sudo bash scripts/run_filebench_dm.sh --target raw
sudo bash scripts/run_filebench_dm.sh --target dm-crypt
```

`dm-verity` + `fileserver`（含写）默认记为 **N/A**。可改用偏读的
`webserver.f`，或显式 `--allow-verity-write`（不推荐，违反只读语义）。

记录字段：IOPS（ops/s）、throughput（mb/s）、latency（ms/op）、runtime。

## 若当前无法安装 / 运行

典型原因：

1. 发行版未提供 `filebench` 包；
2. 需从源码编译且依赖旧内核头文件；
3. Asterinas guest 用户态未包含 filebench（本任务不改内核/rootfs 去硬装）。

建议正确运行方案（宿主机或 Linux guest，勿改星绽核心）：

```bash
# Ubuntu 示例：无官方包时从上游源码构建（在 Linux 环境，非 Asterinas）
git clone https://github.com/filebench/filebench.git
cd filebench
libtoolize && aclocal && autoheader && automake --add-missing && autoconf
./configure && make
sudo make install
filebench -h
```

将失败原因、尝试命令与时间写入：

`docs/evidence/filebench/check-report.txt`

并在答辩材料中说明：**功能与 FIO/SQLite 证据不受影响；Filebench 作为可选应用层补充。**
