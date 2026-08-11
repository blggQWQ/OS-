#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUNNER="${FILEBENCH_RUNNER:-$ROOT_DIR/scripts/run_filebench_dm.sh}"
OUT_DIR="${OUT_DIR:-$ROOT_DIR/docs/evidence/filebench}"
WORK_DIR="${WORK_DIR:-$ROOT_DIR/target/benchmark/filebench-matrix}"
RUNS="${RUNS:-5}"
LOCK_FILE="${BENCHMARK_LOCK_FILE:-$ROOT_DIR/target/benchmark/performance.lock}"
KEY_HEX="${DM_KEY:-000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f}"
WORKLOAD="${FILEBENCH_WORKLOAD:-$ROOT_DIR/scripts/workloads/fileserver.f}"
MOUNT_ROOT="${FILEBENCH_MOUNT_ROOT:-/mnt/cscc-filebench}"

(( RUNS >= 5 )) || exit 2
[[ "$EUID" -eq 0 ]] || exit 1
mkdir -p "$OUT_DIR" "$WORK_DIR" "$(dirname "$LOCK_FILE")"
exec 9>"$LOCK_FILE"
flock -n 9 || { echo "benchmark already running: pid=$(cat "${LOCK_FILE}.pid" 2>/dev/null || echo unknown)" >&2; exit 75; }
echo "$$" >"${LOCK_FILE}.pid"

OWNED_LOOP=""; OWNED_MAPPER=""; OWNED_MOUNT=""
cleanup() {
  set +e
  [[ -n "$OWNED_MOUNT" ]] && mountpoint -q "$OWNED_MOUNT" && umount "$OWNED_MOUNT"
  [[ -n "$OWNED_MAPPER" ]] && dmsetup remove "$OWNED_MAPPER" >/dev/null 2>&1
  if [[ -n "$OWNED_LOOP" ]] && losetup "$OWNED_LOOP" >/dev/null 2>&1; then
    if ! findmnt -rn -S "$OWNED_LOOP" >/dev/null 2>&1 && ! fuser "$OWNED_LOOP" >/dev/null 2>&1; then
      losetup -d "$OWNED_LOOP"
    else
      echo "status=BLOCKED_BY_PROCESS loop=$OWNED_LOOP" >>"$OUT_DIR/resources.log"
    fi
  fi
  rm -f "${LOCK_FILE}.pid"
}
trap cleanup EXIT INT TERM

run_target() {
  local target="$1" mount_dir="$2" round
  for round in $(seq 1 "$RUNS"); do
    if compgen -G "$OUT_DIR/${target}-fileserver-r${round}-*.csv" >/dev/null; then
      echo "resume: skip completed $target round $round"
      continue
    fi
    BENCH_ROUND="$round" OUT_DIR="$OUT_DIR" timeout --signal=TERM --kill-after=15 180 \
      bash "$RUNNER" --target "$target" --mount "$mount_dir" --workload "$WORKLOAD"
  done
}

RAW_IMAGE="$WORK_DIR/raw.img"; truncate -s 4G "$RAW_IMAGE"
OWNED_LOOP="$(losetup --find --show --direct-io=on "$RAW_IMAGE")"
echo "pid=$$ image=$RAW_IMAGE loop=$OWNED_LOOP" >>"$OUT_DIR/resources.log"
mkfs.ext4 -F -q "$OWNED_LOOP"
OWNED_MOUNT="${MOUNT_ROOT}-raw"; mkdir -p "$OWNED_MOUNT"; mount "$OWNED_LOOP" "$OWNED_MOUNT"
run_target raw "$OWNED_MOUNT"
umount "$OWNED_MOUNT"; OWNED_MOUNT=""; losetup -d "$OWNED_LOOP"; OWNED_LOOP=""

CRYPT_IMAGE="$WORK_DIR/crypt.img"; truncate -s 4G "$CRYPT_IMAGE"
OWNED_LOOP="$(losetup --find --show --direct-io=on "$CRYPT_IMAGE")"
echo "pid=$$ image=$CRYPT_IMAGE loop=$OWNED_LOOP" >>"$OUT_DIR/resources.log"
OWNED_MAPPER="cscc-filebench-crypt-$$"
SECTORS="$(blockdev --getsz "$OWNED_LOOP")"
printf '0 %s crypt aes-xts-plain64 %s 0 %s 0\n' "$SECTORS" "$KEY_HEX" "$OWNED_LOOP" | dmsetup create "$OWNED_MAPPER"
dmsetup mknodes "$OWNED_MAPPER"
mkfs.ext4 -F -q "/dev/mapper/$OWNED_MAPPER"
OWNED_MOUNT="${MOUNT_ROOT}-crypt"; mkdir -p "$OWNED_MOUNT"; mount "/dev/mapper/$OWNED_MAPPER" "$OWNED_MOUNT"
run_target dm-crypt "$OWNED_MOUNT"

BENCH_ROUND=1 OUT_DIR="$OUT_DIR" bash "$RUNNER" --target dm-verity --mount "$OWNED_MOUNT" --workload "$WORKLOAD"
echo "filebench_matrix_out=$OUT_DIR"
