#!/usr/bin/env python3
"""从正式 Filebench 原始日志重新生成逐轮 CSV。"""

import csv
import re
import sys
from pathlib import Path


SUMMARY_RE = re.compile(
    r"IO Summary:\s+\d+\s+ops\s+([0-9.]+)\s+ops/s\s+"
    r"\S+\s+rd/wr\s+([0-9.]+)mb/s\s+([0-9.]+)ms/op",
    re.IGNORECASE,
)
RUNTIME_RE = re.compile(r"Run took\s+([0-9.]+)\s+seconds", re.IGNORECASE)


def main() -> int:
    evidence = Path(sys.argv[1])
    failures = []
    for log in sorted(evidence.glob("*-fileserver-r*.log")):
        text = log.read_text(encoding="utf-8", errors="replace")
        summary = SUMMARY_RE.search(text)
        runtime = RUNTIME_RE.search(text)
        target = log.name.split("-fileserver-r", 1)[0]
        csv_path = log.with_suffix(".csv")
        status = "ok" if summary and runtime else "parse_incomplete"
        with csv_path.open("w", newline="", encoding="utf-8") as output:
            writer = csv.writer(output)
            writer.writerow(
                ["platform", "target", "workload", "status", "ops_per_sec",
                 "mb_per_sec", "latency_ms", "runtime_s"]
            )
            writer.writerow(
                ["linux-host", target, "fileserver", status,
                 summary.group(1) if summary else "unavailable",
                 summary.group(2) if summary else "unavailable",
                 summary.group(3) if summary else "unavailable",
                 runtime.group(1) if runtime else "unavailable"]
            )
        if status != "ok":
            failures.append(log.name)
    if failures:
        print("解析失败：" + ", ".join(failures), file=sys.stderr)
        return 1
    print(f"已重新生成 {len(list(evidence.glob('*-fileserver-r*.csv')))} 份 CSV")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
