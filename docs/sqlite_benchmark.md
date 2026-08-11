# SQLite 固定 workload

可重复的应用层基准，用于 raw / dm-crypt 完整读写，以及 dm-verity 只读查询。

## Workload（固定）

1. 创建数据库与表 `kv(id INTEGER PRIMARY KEY, value TEXT)`
2. 批量插入 N 行（`BEGIN` + `executemany`）
3. `COMMIT`
4. `SELECT COUNT(*)`
5. `UPDATE` 前 10% 行并 `COMMIT`
6. 再次 `SELECT` 中间行

N 仅允许：**10000**（默认）或 **50000**。

## 运行

```bash
# 在目标文件系统目录上跑（示例：已挂载的 raw/crypt 卷）
bash scripts/run_sqlite_workload.sh --target raw --rows 10000 \
  --db-dir /mnt/cscc-sqlite

bash scripts/run_sqlite_workload.sh --target dm-crypt --rows 10000 \
  --db-dir /mnt/cscc-sqlite-crypt

# dm-verity：提前在可写环境构建同 schema/同 N 的 DB，只测 SELECT
bash scripts/run_sqlite_workload.sh --target dm-verity --rows 10000 \
  --readonly-db /mnt/cscc-verity/cscc-10000.db
```

## 输出

`docs/evidence/sqlite/` 下：

- `*.csv`：`create_s` / `bulk_insert_s` / `transaction_s` / `select_s` /
  `update_s` / `select2_s` / `total_s`
- `*.metadata.txt`：commit、日期、sqlite 版本、路径
- `*.log`

verity 行中写阶段时间为 `0`，`status=ok_readonly`，文档与答辩需注明只读限制。
