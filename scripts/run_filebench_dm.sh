#!/usr/bin/env bash
# Run Filebench against a mounted filesystem path (raw or dm-crypt backed).
# dm-verity + fileserver (write-heavy) is recorded as N/A unless --allow-verity-write.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET="raw"
WORKLOAD="$ROOT_DIR/scripts/workloads/fileserver.f"
MOUNT_POINT="${MOUNT_POINT:-/mnt/cscc-fb}"
OUT_DIR="${OUT_DIR:-$ROOT_DIR/docs/evidence/filebench}"
ALLOW_VERITY_WRITE=0

usage() {
  echo "Usage: $0 --target raw|dm-crypt|dm-verity [--workload path] [--mount path]"
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --target) TARGET="$2"; shift 2 ;;
    --workload) WORKLOAD="$2"; shift 2 ;;
    --mount) MOUNT_POINT="$2"; shift 2 ;;
    --allow-verity-write) ALLOW_VERITY_WRITE=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown arg: $1" >&2; usage; exit 2 ;;
  esac
done

mkdir -p "$OUT_DIR"
WL_NAME="$(basename "$WORKLOAD" .f)"
STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
META="$OUT_DIR/${TARGET}-${WL_NAME}-${STAMP}.metadata.txt"
LOG="$OUT_DIR/${TARGET}-${WL_NAME}-${STAMP}.log"
CSV="$OUT_DIR/${TARGET}-${WL_NAME}-${STAMP}.csv"

if [[ "$TARGET" == "dm-verity" && "$ALLOW_VERITY_WRITE" -eq 0 && "$WL_NAME" == "fileserver" ]]; then
  {
    echo "status=N/A"
    echo "reason=dm-verity_read_only_fileserver_has_writes"
    echo "workload=$WORKLOAD"
    echo "target=$TARGET"
  } | tee "$META"
  echo "platform,target,workload,status,ops_per_sec,mb_per_sec,latency_ms,runtime_s" >"$CSV"
  echo "linux-host,$TARGET,$WL_NAME,N/A,N/A,N/A,N/A,N/A" >>"$CSV"
  echo "Recorded N/A for dm-verity fileserver. Use webserver.f or --allow-verity-write."
  exit 0
fi

if ! bash "$ROOT_DIR/scripts/check_filebench.sh" "$OUT_DIR/check-report.txt"; then
  {
    echo "status=skipped"
    echo "reason=filebench_not_installed"
    echo "see=docs/filebench.md"
  } | tee "$META"
  exit 1
fi

if [[ ! -d "$MOUNT_POINT" ]]; then
  echo "mount point missing: $MOUNT_POINT" >&2
  echo "Mount a filesystem on raw or dm-crypt device first, then re-run." >&2
  exit 1
fi

TMP_WL="$(mktemp)"
sed "s#set \\\$dir=.*#set \$dir=${MOUNT_POINT}#" "$WORKLOAD" >"$TMP_WL"

{
  echo "git_commit=$(git -C "$ROOT_DIR" rev-parse HEAD 2>/dev/null || echo unknown)"
  echo "timestamp_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "target=$TARGET"
  echo "workload=$WORKLOAD"
  echo "mount_point=$MOUNT_POINT"
  echo "command=filebench -f $TMP_WL"
} >"$META"

set +e
filebench -f "$TMP_WL" 2>&1 | tee "$LOG"
rc=${PIPESTATUS[0]}
set -e
rm -f "$TMP_WL"

# Best-effort parse of common Filebench summary lines.
python3 - "$LOG" "$CSV" "$TARGET" "$WL_NAME" <<'PY'
import re, sys
from pathlib import Path
log, csv_path, target, wl = sys.argv[1:5]
text = Path(log).read_text(encoding="utf-8", errors="replace")
ops = re.search(r"([0-9.]+)\s*ops/s", text)
mb = re.search(r"([0-9.]+)\s*mb/s", text, re.I)
lat = re.search(r"([0-9.]+)\s*ms/op", text)
runtime = re.search(r"([0-9.]+)\s*s\s*$", text, re.M)
with open(csv_path, "w", encoding="utf-8") as out:
    out.write("platform,target,workload,status,ops_per_sec,mb_per_sec,latency_ms,runtime_s\n")
    out.write(
        "linux-host,%s,%s,%s,%s,%s,%s,%s\n"
        % (
            target,
            wl,
            "ok" if ops else "parse_incomplete",
            ops.group(1) if ops else "unavailable",
            mb.group(1) if mb else "unavailable",
            lat.group(1) if lat else "unavailable",
            runtime.group(1) if runtime else "unavailable",
        )
    )
print(f"filebench_csv={csv_path}")
PY

exit "$rc"
