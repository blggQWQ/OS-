# FIO 正式矩阵证据

合同见 [docs/benchmark_contract.md](../../benchmark_contract.md)。

## 单元格清单

对每个 `target ∈ {raw, dm-crypt, dm-verity}`：

| operation | bs | qd | raw | dm-crypt | dm-verity |
|---|---|---:|---|---|---|
| read | 4k | 1 | 待跑 | 待跑 | 待跑 |
| randread | 4k | 1 | 待跑 | 待跑 | 待跑 |
| write | 4k | 1 | 待跑 | 待跑 | N/A |
| randwrite | 4k | 1 | 待跑 | 待跑 | N/A |
| read | 32k | 1 | 待跑 | 待跑 | 待跑 |
| randread | 32k | 1 | 待跑 | 待跑 | 待跑 |
| write | 32k | 1 | 待跑 | 待跑 | N/A |
| randwrite | 32k | 1 | 待跑 | 待跑 | N/A |
| read | 256k | 1 | 待跑 | 待跑 | 待跑 |
| randread | 256k | 1 | 待跑 | 待跑 | 待跑 |
| write | 256k | 1 | 待跑 | 待跑 | N/A |
| randwrite | 256k | 1 | 待跑 | 待跑 | N/A |
| randread | 4k | 16 | 待跑 | 待跑 | 待跑 |

## 目录布局（跑完后）

```text
fio-matrix/
  {platform}/
    {target}/
      {operation}_{bs}_qd{qd}/
        metadata.txt
        run-1.{json,log}
        ...
        run-5.{json,log}
        per-run.csv
  summary.csv
  summary-three-target.csv
  na-cells.csv
```

## 如何生成

```bash
# Linux host 对照（需 root）
sudo bash scripts/run_fio_matrix.sh --platform linux-host

# Asterinas KVM（需已构建 guest 工具与镜像）
bash scripts/run_fio_matrix.sh --platform asterinas-kvm \
  --asterinas-dir path/to/asterinas

python3 scripts/summarize_fio_matrix.py docs/evidence/fio-matrix
```
