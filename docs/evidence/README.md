# 性能与功能证据索引

| 目录 / 文件 | 状态 | 说明 |
|---|---|---|
| `fio-matrix/` | 待跑（脚本已就绪） | 正式 FIO 全矩阵：raw / dm-crypt / dm-verity |
| `boot-matrix/` | 待跑（脚本已就绪） | plain / dm-crypt / dm-verity ×5 |
| `filebench/` | 待跑 / 视环境 | Filebench fileserver 等 |
| `sqlite/` | 待跑（脚本已就绪） | 固定 SQLite workload |
| `asterinas-dm-crypt-kvm.csv` 等 | 历史 | 4K seqread QD1 小矩阵，见下表 |
| `asterinas-verity-fio-*.csv` | 历史 | verity cache A/B，16 MiB |
| `FINAL_VALIDATION.md` | 历史功能链 | crypt/verity 正反例与 cache A/B |

## 历史 FIO 证据（非正式全矩阵）

| 证据 | 条件 | 备注 |
|---|---|---|
| `asterinas-dm-crypt-kvm.csv` | 4K read QD1，8 MiB，5 轮 | 不可与 32K/256K 新合同混表 |
| `asterinas-verity-fio-cache0-kvm.csv` | 4K read，cache=0 | 内部 A/B |
| `asterinas-verity-fio-cache256-kvm.csv` | 4K read，cache=256 | 内部 A/B |
| `linux-dm-summary.csv` | Linux host 4K × QD1/32 | 无完整 per-run 归档时仅作参考 |

正式对外表以 `docs/benchmark_contract.md` 与 `fio-matrix/` 下带 metadata 的结果为准。
