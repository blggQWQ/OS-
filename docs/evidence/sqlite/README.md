# SQLite 证据

```bash
# 默认 N=10000；完整读写
bash scripts/run_sqlite_workload.sh --target raw --rows 10000
bash scripts/run_sqlite_workload.sh --target dm-crypt --rows 10000

# dm-verity：仅 SELECT（需预构建只读库路径）
bash scripts/run_sqlite_workload.sh --target dm-verity --rows 10000 \
  --readonly-db path/to/prepared.db
```

详见 [docs/sqlite_benchmark.md](../../sqlite_benchmark.md)。
