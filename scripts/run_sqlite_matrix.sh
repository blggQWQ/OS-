#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUNNER="${SQLITE_WORKLOAD_RUNNER:-$ROOT_DIR/scripts/run_sqlite_workload.sh}"
OUT_DIR="${OUT_DIR:-$ROOT_DIR/docs/evidence/sqlite}"
WORK_DIR="${WORK_DIR:-$ROOT_DIR/target/benchmark/sqlite-matrix}"
RUNS="${RUNS:-5}"
ROWS="${ROWS:-10000}"
LOCK_FILE="${BENCHMARK_LOCK_FILE:-$ROOT_DIR/target/benchmark/performance.lock}"
KEY_HEX="${DM_KEY:-000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f}"

(( RUNS >= 5 )) || { echo "RUNS must be >= 5" >&2; exit 2; }
for command in flock losetup dmsetup mkfs.ext4 mount umount veritysetup sqlite3; do
  command -v "$command" >/dev/null || { echo "missing command: $command" >&2; exit 1; }
done
[[ "$EUID" -eq 0 ]] || { echo "run as root" >&2; exit 1; }

mkdir -p "$OUT_DIR" "$WORK_DIR" "$(dirname "$LOCK_FILE")"
exec 9>"$LOCK_FILE"
flock -n 9 || { echo "benchmark already running: pid=$(cat "${LOCK_FILE}.pid" 2>/dev/null || echo unknown)" >&2; exit 75; }
echo "$$" >"${LOCK_FILE}.pid"

OWNED_LOOPS=()
OWNED_MAPPERS=()
OWNED_MOUNTS=()
RESOURCE_LOG="$OUT_DIR/resources.log"

cleanup() {
  set +e
  for ((i=${#OWNED_MOUNTS[@]}-1; i>=0; i--)); do
    mountpoint -q "${OWNED_MOUNTS[i]}" && umount "${OWNED_MOUNTS[i]}"
  done
  for ((i=${#OWNED_MAPPERS[@]}-1; i>=0; i--)); do
    dmsetup remove "${OWNED_MAPPERS[i]}" >/dev/null 2>&1
  done
  for ((i=${#OWNED_LOOPS[@]}-1; i>=0; i--)); do
    loop="${OWNED_LOOPS[i]}"
    if losetup "$loop" >/dev/null 2>&1; then
      if findmnt -rn -S "$loop" >/dev/null 2>&1 || fuser "$loop" >/dev/null 2>&1; then
        echo "status=BLOCKED_BY_PROCESS loop=$loop" >>"$RESOURCE_LOG"
      else
        losetup -d "$loop"
      fi
    fi
  done
  rm -f "${LOCK_FILE}.pid"
}
trap cleanup EXIT INT TERM

new_loop() {
  local image="$1" loop
  loop="$(losetup --find --show --direct-io=on "$image")"
  OWNED_LOOPS+=("$loop")
  printf 'timestamp_utc=%s pid=%s image=%s loop=%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$$" "$image" "$loop" >>"$RESOURCE_LOG"
  NEW_LOOP="$loop"
}

run_rounds() {
  local target="$1" db_dir="$2" readonly_db="${3:-}" round
  for round in $(seq 1 "$RUNS"); do
    if compgen -G "$OUT_DIR/${target}-n${ROWS}-r${round}-*.csv" >/dev/null; then
      echo "resume: skip completed $target round $round"
      continue
    fi
    args=(--target "$target" --rows "$ROWS" --db-dir "$db_dir" --out "$OUT_DIR")
    [[ -n "$readonly_db" ]] && args+=(--readonly-db "$readonly_db")
    BENCH_ROUND="$round" bash "$RUNNER" "${args[@]}"
  done
}

RAW_IMAGE="$WORK_DIR/raw.img"
truncate -s 256M "$RAW_IMAGE"
new_loop "$RAW_IMAGE"; RAW_LOOP="$NEW_LOOP"
mkfs.ext4 -F -q "$RAW_LOOP"
RAW_MOUNT="$WORK_DIR/mnt-raw"
mkdir -p "$RAW_MOUNT"; mount "$RAW_LOOP" "$RAW_MOUNT"; OWNED_MOUNTS+=("$RAW_MOUNT")
run_rounds raw "$RAW_MOUNT"
umount "$RAW_MOUNT"; OWNED_MOUNTS=()
losetup -d "$RAW_LOOP"; OWNED_LOOPS=()

CRYPT_IMAGE="$WORK_DIR/crypt.img"
truncate -s 256M "$CRYPT_IMAGE"
new_loop "$CRYPT_IMAGE"; CRYPT_LOOP="$NEW_LOOP"
CRYPT_NAME="cscc-sqlite-crypt-$$"
SECTORS="$(blockdev --getsz "$CRYPT_LOOP")"
printf '0 %s crypt aes-xts-plain64 %s 0 %s 0\n' "$SECTORS" "$KEY_HEX" "$CRYPT_LOOP" | dmsetup create "$CRYPT_NAME"
OWNED_MAPPERS+=("$CRYPT_NAME")
dmsetup mknodes "$CRYPT_NAME"
mkfs.ext4 -F -q "/dev/mapper/$CRYPT_NAME"
CRYPT_MOUNT="$WORK_DIR/mnt-crypt"
mkdir -p "$CRYPT_MOUNT"; mount "/dev/mapper/$CRYPT_NAME" "$CRYPT_MOUNT"; OWNED_MOUNTS+=("$CRYPT_MOUNT")
run_rounds dm-crypt "$CRYPT_MOUNT"
umount "$CRYPT_MOUNT"; OWNED_MOUNTS=()
dmsetup remove "$CRYPT_NAME"; OWNED_MAPPERS=()
losetup -d "$CRYPT_LOOP"; OWNED_LOOPS=()

VERITY_DATA="$WORK_DIR/verity-data.img"
VERITY_HASH="$WORK_DIR/verity-hash.img"
truncate -s 256M "$VERITY_DATA"; truncate -s 16M "$VERITY_HASH"
new_loop "$VERITY_DATA"; DATA_LOOP="$NEW_LOOP"
new_loop "$VERITY_HASH"; HASH_LOOP="$NEW_LOOP"
mkfs.ext4 -F -q "$DATA_LOOP"
PREP_MOUNT="$WORK_DIR/mnt-verity-prep"
mkdir -p "$PREP_MOUNT"; mount "$DATA_LOOP" "$PREP_MOUNT"; OWNED_MOUNTS+=("$PREP_MOUNT")
BENCH_ROUND=prep bash "$RUNNER" --target raw --rows "$ROWS" --db-dir "$PREP_MOUNT" --out "$OUT_DIR/preparation"
umount "$PREP_MOUNT"; OWNED_MOUNTS=()
ROOT_HASH="$(veritysetup format "$DATA_LOOP" "$HASH_LOOP" | awk '/Root hash:/ {print $3}')"
[[ -n "$ROOT_HASH" ]] || { echo "failed to obtain verity root hash" >&2; exit 1; }
VERITY_NAME="cscc-sqlite-verity-$$"
veritysetup open "$DATA_LOOP" "$VERITY_NAME" "$HASH_LOOP" "$ROOT_HASH"
OWNED_MAPPERS+=("$VERITY_NAME")
VERITY_MOUNT="$WORK_DIR/mnt-verity"
mkdir -p "$VERITY_MOUNT"; mount -o ro "/dev/mapper/$VERITY_NAME" "$VERITY_MOUNT"; OWNED_MOUNTS+=("$VERITY_MOUNT")
run_rounds dm-verity "$VERITY_MOUNT" "$VERITY_MOUNT/cscc-${ROWS}.db"

echo "sqlite_matrix_out=$OUT_DIR"
