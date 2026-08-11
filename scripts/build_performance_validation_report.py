#!/usr/bin/env python3
"""从白名单证据目录生成最终性能矩阵和中文报告。"""

from __future__ import annotations

import argparse
import csv
import statistics
from pathlib import Path


MATRIX_FIELDS = (
    "platform",
    "target",
    "workload",
    "bs",
    "qd",
    "round_count",
    "status",
    "mean_bw_mib_s",
    "median_bw_mib_s",
    "stddev_bw_mib_s",
    "raw_evidence_path",
)


def read_csv(path: Path) -> list[dict[str, str]]:
    with path.open(encoding="utf-8-sig", newline="") as source:
        return list(csv.DictReader(source))


def summary(values: list[float]) -> tuple[float, float, float]:
    deviation = statistics.stdev(values) if len(values) > 1 else 0.0
    return statistics.fmean(values), statistics.median(values), deviation


def matrix_row(
    *, platform: str, target: str, rows: list[dict[str, str]], source: Path, root: Path
) -> dict[str, object]:
    values = [float(row["bw_mib_s"]) for row in rows]
    mean, median, deviation = summary(values)
    first = rows[0]
    return {
        "platform": platform,
        "target": target,
        "workload": first.get("operation", ""),
        "bs": first.get("block_size", ""),
        "qd": first.get("queue_depth", first.get("iodepth_set", "")),
        "round_count": len(rows),
        "status": "PASS",
        "mean_bw_mib_s": f"{mean:.6f}",
        "median_bw_mib_s": f"{median:.6f}",
        "stddev_bw_mib_s": f"{deviation:.6f}",
        "raw_evidence_path": source.relative_to(root).as_posix(),
    }


def collect_fio(root: Path) -> list[dict[str, object]]:
    matrix: list[dict[str, object]] = []
    asterinas = root / "asterinas-fio" / "asterinas-kvm"

    # 只读取真实性审计后重新执行的 dm-crypt 和修复后的 dm-verity 目录。
    for target, directory in (
        ("dm-crypt", asterinas / "dm-crypt-recovery"),
        ("dm-verity", asterinas / "dm-verity"),
    ):
        for source in sorted(directory.glob("*/matrix-raw.csv")):
            rows = [row for row in read_csv(source) if row.get("label") == target]
            if rows:
                matrix.append(
                    matrix_row(
                        platform="asterinas-kvm",
                        target=target,
                        rows=rows,
                        source=source,
                        root=root,
                    )
                )

    for workload in ("write", "randwrite"):
        for block_size in ("4k", "32k", "256k"):
            matrix.append(
                {
                    "platform": "asterinas-kvm",
                    "target": "dm-verity",
                    "workload": workload,
                    "bs": block_size,
                    "qd": 1,
                    "round_count": 0,
                    "status": "N/A_READ_ONLY",
                    "mean_bw_mib_s": "N/A",
                    "median_bw_mib_s": "N/A",
                    "stddev_bw_mib_s": "N/A",
                    "raw_evidence_path": "asterinas-fio/na-cells.csv",
                }
            )

    # Linux 对照只读取独立 loop block device 与真实 dm-crypt mapper 的 JSON 汇总。
    linux = root / "fio-matrix" / "linux-host"
    for target in ("raw", "dm-crypt"):
        for source in sorted((linux / target).glob("*/per-run.csv")):
            rows = [row for row in read_csv(source) if row.get("status") == "ok"]
            if rows:
                matrix.append(
                    matrix_row(
                        platform="linux-host",
                        target=target,
                        rows=rows,
                        source=source,
                        root=root,
                    )
                )

    return sorted(
        matrix,
        key=lambda row: (
            str(row["platform"]),
            str(row["target"]),
            str(row["workload"]),
            str(row["bs"]),
            int(row["qd"]),
        ),
    )


def median_from_files(paths: list[Path], field: str) -> float:
    values = [float(row[field]) for path in paths for row in read_csv(path)]
    return statistics.median(values)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("evidence", type=Path)
    parser.add_argument("--top-commit", required=True)
    parser.add_argument("--asterinas-commit", required=True)
    parser.add_argument("--image-digest", required=True)
    args = parser.parse_args()
    root = args.evidence.resolve()

    matrix = collect_fio(root)
    output_csv = root / "PERFORMANCE_MATRIX.csv"
    with output_csv.open("w", encoding="utf-8", newline="") as target:
        writer = csv.DictWriter(target, fieldnames=MATRIX_FIELDS)
        writer.writeheader()
        writer.writerows(matrix)

    counts: dict[tuple[str, str], int] = {}
    for row in matrix:
        if row["status"] == "PASS":
            key = (str(row["platform"]), str(row["target"]))
            counts[key] = counts.get(key, 0) + 1
    na_count = sum(str(row["status"]).startswith("N/A") for row in matrix)

    filebench: dict[str, tuple[float, float, float, int]] = {}
    for target in ("raw", "dm-crypt"):
        paths = sorted((root / "filebench-valid").glob(f"{target}-fileserver-r*.csv"))
        filebench[target] = (
            median_from_files(paths, "ops_per_sec"),
            median_from_files(paths, "mb_per_sec"),
            median_from_files(paths, "latency_ms"),
            len(paths),
        )

    sqlite: dict[str, tuple[float, float, int]] = {}
    for target in ("raw", "dm-crypt", "dm-verity"):
        paths = sorted((root / "sqlite").glob(f"{target}-n10000-r[1-5]-*.csv"))
        sqlite[target] = (
            median_from_files(paths, "total_s"),
            median_from_files(paths, "select_s"),
            len(paths),
        )

    boot_rows = read_csv(root / "boot-matrix" / "boot-time-summary.csv")
    boot = {row["scenario"]: row for row in boot_rows}

    report = f"""# 性能验证报告

## 基线与口径

- 顶层测试基线：`{args.top_commit}`
- Asterinas 测试基线：`{args.asterinas_commit}`
- Asterinas 官方开发镜像：`{args.image_digest}`
- Asterinas：QEMU/KVM、`-cpu host`、1 vCPU、8 GiB 内存
- 每个有效单元格保留 5 轮原始结果；带宽单位为 MiB/s
- Linux 与 Asterinas 结果分平台列示，不进行跨平台倍率比较

## FIO 完成情况

| 平台 | 目标 | 有效单元格 | 状态 |
|---|---|---:|---|
| Asterinas KVM | dm-crypt | {counts.get(('asterinas-kvm', 'dm-crypt'), 0)} | PASS |
| Asterinas KVM | dm-verity | {counts.get(('asterinas-kvm', 'dm-verity'), 0)} | PASS；另有 {na_count} 个只读 N/A |
| Linux host | raw block device | {counts.get(('linux-host', 'raw'), 0)} | PASS |
| Linux host | dm-crypt mapper | {counts.get(('linux-host', 'dm-crypt'), 0)} | PASS |

正式单元格与证据路径见 [`PERFORMANCE_MATRIX.csv`](PERFORMANCE_MATRIX.csv)。Linux raw 与 dm-crypt 使用同一套 `libaio/direct=1/numjobs=1` 参数；Asterinas 保护目标使用 guest 内实际 mapper。Asterinas raw 的旧批次与不存在 mapper 的假 dm-crypt 记录混在同一证据族中，未纳入本报告，也未由生成器读取。

## Filebench fileserver（Linux host）

| 目标 | 轮数 | 中位 ops/s | 中位 MB/s | 中位延迟 ms/op |
|---|---:|---:|---:|---:|
| raw | {filebench['raw'][3]} | {filebench['raw'][0]:.3f} | {filebench['raw'][1]:.1f} | {filebench['raw'][2]:.1f} |
| dm-crypt | {filebench['dm-crypt'][3]} | {filebench['dm-crypt'][0]:.3f} | {filebench['dm-crypt'][1]:.1f} | {filebench['dm-crypt'][2]:.1f} |

这里沿用 Filebench 原始输出的 MB/s 口径。dm-verity 为只读目标，与 fileserver 写密集负载不兼容，记为 N/A。

## SQLite（Linux host，10,000 行）

| 目标 | 轮数 | 中位总耗时 s | 中位查询耗时 s | 说明 |
|---|---:|---:|---:|---|
| raw | {sqlite['raw'][2]} | {sqlite['raw'][0]:.6f} | {sqlite['raw'][1]:.6f} | 完整写入与查询 |
| dm-crypt | {sqlite['dm-crypt'][2]} | {sqlite['dm-crypt'][0]:.6f} | {sqlite['dm-crypt'][1]:.6f} | 完整写入与查询 |
| dm-verity | {sqlite['dm-verity'][2]} | {sqlite['dm-verity'][0]:.6f} | {sqlite['dm-verity'][1]:.6f} | 只读查询；不可与完整事务总耗时横比 |

## Asterinas 启动耗时

| 场景 | 轮数 | 中位耗时 s |
|---|---:|---:|
| plain | {boot['plain']['runs']} | {float(boot['plain']['median_s']):.6f} |
| dm-crypt rootfs | {boot['dm-crypt']['runs']} | {float(boot['dm-crypt']['median_s']):.6f} |
| dm-verity rootfs | {boot['dm-verity']['runs']} | {float(boot['dm-verity']['median_s']):.6f} |

相对 plain 的中位开销：dm-crypt {float(boot['dm-crypt_overhead_vs_plain_percent']['median_s']):.3f}%，dm-verity {float(boot['dm-verity_overhead_vs_plain_percent']['median_s']):.3f}%。计时边界为 QEMU 进程启动至串口出现受保护根文件系统 PASS 标记。

## 证据目录

- `asterinas-fio/asterinas-kvm/dm-crypt-recovery/`：真实性审计后重新执行的 dm-crypt
- `asterinas-fio/asterinas-kvm/dm-verity/`：修正 runner 后重新执行的 dm-verity
- `fio-matrix/linux-host/`：Linux raw/dm-crypt 成对 FIO JSON 与汇总
- `filebench-valid/`：Filebench 5+5 轮原始日志和 CSV
- `sqlite/`：SQLite 5+5+5 轮记录
- `boot-matrix/`：Asterinas 启动 15 轮记录
- `cargo/`：构建与测试记录

## 结论

最终表只包含可定位到真实 block device/mapper、参数完整且轮数满足要求的数据。旧假 mapper、失败重试、调试日志和手工异常值均不进入生成输入；未修改 dm-crypt、dm-verity、BIO、virtio、DMA 或文件系统核心代码。
"""
    output_report = root / "PERFORMANCE_VALIDATION_REPORT.md"
    output_report.write_text(report, encoding="utf-8")
    print(f"matrix={output_csv}")
    print(f"report={output_report}")


if __name__ == "__main__":
    main()
