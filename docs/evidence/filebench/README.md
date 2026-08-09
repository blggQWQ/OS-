# Filebench 证据

状态：待环境探测。先运行：

```bash
bash scripts/check_filebench.sh
```

若可用：

```bash
sudo bash scripts/run_filebench_dm.sh --target raw
sudo bash scripts/run_filebench_dm.sh --target dm-crypt
# dm-verity: fileserver 含写，默认记录 N/A（见 docs/filebench.md）
```

详见 [docs/filebench.md](../../filebench.md)。
