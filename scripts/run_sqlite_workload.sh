#!/usr/bin/env bash
# Fixed, repeatable SQLite benchmark for raw / dm-crypt / dm-verity mounts.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET="raw"
ROWS=10000
DB_DIR="${DB_DIR:-}"
READONLY_DB=""
OUT_DIR="${OUT_DIR:-$ROOT_DIR/docs/evidence/sqlite}"
ROUND="${BENCH_ROUND:-1}"

usage() {
  cat <<EOF
Usage: $0 --target raw|dm-crypt|dm-verity [--rows 10000|50000] [options]

Options:
  --db-dir <path>       Directory on the mounted target FS (default: temp under OUT_DIR)
  --readonly-db <path>  For dm-verity: path to a pre-built DB; only SELECT is timed
  --out <dir>           Evidence directory
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --target) TARGET="$2"; shift 2 ;;
    --rows) ROWS="$2"; shift 2 ;;
    --db-dir) DB_DIR="$2"; shift 2 ;;
    --readonly-db) READONLY_DB="$2"; shift 2 ;;
    --out) OUT_DIR="$2"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown arg: $1" >&2; usage; exit 2 ;;
  esac
done

if [[ "$ROWS" != "10000" && "$ROWS" != "50000" ]]; then
  echo "--rows must be 10000 or 50000 for the fixed contract" >&2
  exit 2
fi

command -v sqlite3 >/dev/null || {
  echo "missing sqlite3; install sqlite3 package on the host/Linux guest" >&2
  exit 1
}

mkdir -p "$OUT_DIR"
STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
BASE="$OUT_DIR/${TARGET}-n${ROWS}-r${ROUND}-${STAMP}"
META="${BASE}.metadata.txt"
CSV="${BASE}.csv"
LOG="${BASE}.log"

if [[ -z "$DB_DIR" ]]; then
  DB_DIR="$OUT_DIR/workdir-${TARGET}-n${ROWS}"
fi
mkdir -p "$DB_DIR"

{
  echo "git_commit=$(git -C "$ROOT_DIR" rev-parse HEAD 2>/dev/null || echo unknown)"
  echo "timestamp_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "target=$TARGET"
  echo "rows=$ROWS"
  echo "round=$ROUND"
  echo "db_dir=$DB_DIR"
  echo "sqlite3=$(sqlite3 --version)"
  echo "journal_mode=DELETE"
  echo "synchronous=FULL"
  if [[ "$TARGET" == "dm-verity" ]]; then
    echo "workload=select_count,select_point_read"
    echo "read_only_target=1"
  else
    echo "workload=create,bulk_insert,commit,select,update,select"
  fi
} >"$META"

python3 - "$TARGET" "$ROWS" "$DB_DIR" "$READONLY_DB" "$CSV" "$LOG" <<'PY'
from __future__ import annotations

import csv
import sqlite3
import sys
import time
from pathlib import Path

target, rows_s, db_dir, readonly_db, csv_path, log_path = sys.argv[1:7]
rows = int(rows_s)
db_dir_p = Path(db_dir)
log_lines: list[str] = []


def log(msg: str) -> None:
    log_lines.append(msg)
    print(msg)


timings: dict[str, float] = {}

if target == "dm-verity":
    if not readonly_db:
        raise SystemExit(
            "dm-verity requires --readonly-db pointing to a pre-built database"
        )
    db_path = Path(readonly_db)
    if not db_path.is_file():
        raise SystemExit(f"readonly db missing: {db_path}")
    # read-only connection
    uri = f"file:{db_path}?mode=ro"
    t0 = time.perf_counter()
    conn = sqlite3.connect(uri, uri=True)
    cur = conn.cursor()
    t1 = time.perf_counter()
    timings["create_s"] = "N/A"
    timings["bulk_insert_s"] = "N/A"
    timings["transaction_s"] = "N/A"
    cur.execute("SELECT COUNT(*) FROM kv")
    count = cur.fetchone()[0]
    t2 = time.perf_counter()
    timings["select_s"] = t2 - t1
    timings["update_s"] = "N/A"
    cur.execute("SELECT value FROM kv WHERE id = ?", (max(1, count // 2),))
    _ = cur.fetchone()
    t3 = time.perf_counter()
    timings["select2_s"] = t3 - t2
    conn.close()
    timings["total_s"] = t3 - t0
    status = "ok_readonly"
    log(f"verity_readonly count={count} select_s={timings['select_s']:.6f}")
else:
    db_path = db_dir_p / f"cscc-{rows}.db"
    if db_path.exists():
        db_path.unlink()
    t0 = time.perf_counter()
    conn = sqlite3.connect(db_path)
    cur = conn.cursor()
    cur.execute("PRAGMA journal_mode=DELETE")
    cur.execute("PRAGMA synchronous=FULL")
    cur.execute("CREATE TABLE kv (id INTEGER PRIMARY KEY, value TEXT NOT NULL)")
    t1 = time.perf_counter()
    timings["create_s"] = t1 - t0
    cur.execute("BEGIN")
    cur.executemany(
        "INSERT INTO kv(id, value) VALUES (?, ?)",
        ((i, f"payload-{i:08d}") for i in range(1, rows + 1)),
    )
    t2 = time.perf_counter()
    timings["bulk_insert_s"] = t2 - t1
    conn.commit()
    t3 = time.perf_counter()
    timings["transaction_s"] = t3 - t2
    cur.execute("SELECT COUNT(*) FROM kv")
    count = cur.fetchone()[0]
    t4 = time.perf_counter()
    timings["select_s"] = t4 - t3
    cur.execute("UPDATE kv SET value = value || '-u' WHERE id <= ?", (rows // 10,))
    conn.commit()
    t5 = time.perf_counter()
    timings["update_s"] = t5 - t4
    cur.execute("SELECT value FROM kv WHERE id = ?", (rows // 2,))
    _ = cur.fetchone()
    t6 = time.perf_counter()
    timings["select2_s"] = t6 - t5
    conn.close()
    timings["total_s"] = t6 - t0
    status = "ok"
    log(f"readwrite count={count} total_s={timings['total_s']:.6f}")

Path(log_path).write_text("\n".join(log_lines) + "\n", encoding="utf-8")
with open(csv_path, "w", newline="", encoding="utf-8") as out:
    writer = csv.DictWriter(
        out,
        fieldnames=[
            "target",
            "rows",
            "status",
            "create_s",
            "bulk_insert_s",
            "transaction_s",
            "select_s",
            "update_s",
            "select2_s",
            "total_s",
            "db_path",
        ],
    )
    writer.writeheader()
    writer.writerow(
        {
            "target": target,
            "rows": rows,
            "status": status,
            "create_s": timings["create_s"] if isinstance(timings["create_s"], str) else f"{timings['create_s']:.6f}",
            "bulk_insert_s": timings["bulk_insert_s"] if isinstance(timings["bulk_insert_s"], str) else f"{timings['bulk_insert_s']:.6f}",
            "transaction_s": timings["transaction_s"] if isinstance(timings["transaction_s"], str) else f"{timings['transaction_s']:.6f}",
            "select_s": f"{timings['select_s']:.6f}",
            "update_s": timings["update_s"] if isinstance(timings["update_s"], str) else f"{timings['update_s']:.6f}",
            "select2_s": f"{timings['select2_s']:.6f}",
            "total_s": f"{timings['total_s']:.6f}",
            "db_path": str(db_path),
        }
    )
print(f"sqlite_csv={csv_path}")
print(f"sqlite_log={log_path}")
PY

echo "sqlite_metadata=$META"
