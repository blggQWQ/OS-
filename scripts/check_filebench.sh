#!/usr/bin/env bash
# Probe whether Filebench is available without modifying any kernel code.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPORT="${1:-$ROOT_DIR/docs/evidence/filebench/check-report.txt}"
mkdir -p "$(dirname "$REPORT")"

{
  echo "timestamp_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "host=$(uname -srmo)"
  if command -v filebench >/dev/null 2>&1; then
    echo "filebench_path=$(command -v filebench)"
    echo "filebench_version=$(filebench -h 2>&1 | head -n 3 | tr '\n' ' ' || true)"
    echo "status=available"
    echo "STATUS=available"
  else
    echo "filebench_path="
    echo "status=missing"
    echo "STATUS=missing"
    echo "tried=command -v filebench"
    echo "hint=See docs/filebench.md for install options (do not patch Asterinas for this)."
  fi
} | tee "$REPORT"

if grep -q '^STATUS=available$' "$REPORT"; then
  exit 0
fi
exit 1
