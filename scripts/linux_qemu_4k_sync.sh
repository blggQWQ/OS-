#!/usr/bin/env bash
# Linux QEMU/KVM guest 256 MiB contract: 4 KiB sequential read, sync, direct=1,
# QD1, ramp 2s, runtime 10s, five rounds. Run inside the guest as root.
#
#   sudo bash scripts/linux_qemu_4k_sync.sh
#
# Creates aes-xts-plain64 and dm-verity mappers on RAW_DEVICE, then times
# raw / dm-crypt / dm-verity. Writes per-run.csv and a median summary.
# This is not the 13-cell libaio matrix (see linux_qemu_fio_matrix.sh).
set -euo pipefail

RAW_DEVICE="${RAW_DEVICE:-/dev/vdb}"
OUT_DIR="${OUT_DIR:-$PWD/linux-qemu-4k-sync}"
RUNS="${RUNS:-5}"
SIZE_MIB="${SIZE_MIB:-256}"
RUNTIME="${RUNTIME:-10}"
RAMP_TIME="${RAMP_TIME:-2}"
KEY_HEX="${DM_KEY:-000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f}"

CRYPT_NAME="cscc-linux-4k-crypt"
DATA_NAME="cscc-linux-4k-verity-data"
HASH_NAME="cscc-linux-4k-verity-hash"
VERITY_NAME="cscc-linux-4k-verity"

(( RUNS >= 5 )) || { echo "-- RUNS must be >= 5" >&2; exit 2; }

for c in fio python3 dmsetup blockdev veritysetup awk; do
  command -v "$c" >/dev/null || { echo "missing command: $c" >&2; exit 1; }
done
[[ "$EUID" -eq 0 ]] || { echo "run with sudo" >&2; exit 1; }
[[ -b "$RAW_DEVICE" ]] || { echo "$RAW_DEVICE is not a block device" >&2; exit 1; }

cleanup_all() {
  set +e
  veritysetup close "$VERITY_NAME" >/dev/null 2>&1
  dmsetup remove "$VERITY_NAME" >/dev/null 2>&1
  dmsetup remove "$CRYPT_NAME" >/dev/null 2>&1
  dmsetup remove "$DATA_NAME" >/dev/null 2>&1
  dmsetup remove "$HASH_NAME" >/dev/null 2>&1
}
trap cleanup_all EXIT
cleanup_all

SIZE_BYTES=$((SIZE_MIB * 1024 * 1024))
DATA_SECTORS=$((SIZE_BYTES / 512))
HASH_SECTORS=$((64 * 1024 * 1024 / 512))
REQUIRED_BYTES=$(((SIZE_MIB + 64) * 1024 * 1024))
DEVICE_BYTES="$(blockdev --getsize64 "$RAW_DEVICE")"
if (( DEVICE_BYTES < REQUIRED_BYTES )); then
  echo "device too small: $((DEVICE_BYTES / 1024 / 1024)) MiB, need $((REQUIRED_BYTES / 1024 / 1024)) MiB" >&2
  exit 1
fi

mkdir -p "$OUT_DIR"
SUMMARY="$OUT_DIR/per-run.csv"
printf '%s\n' 'label,run,bw_mib_s,iops' > "$SUMMARY"

drop_caches() {
  sync
  echo 3 > /proc/sys/vm/drop_caches
}

run_sync_read() {
  local label="$1"
  local device="$2"
  local run="$3"
  local json="$OUT_DIR/${label}-${run}.json"
  drop_caches
  echo "===== ${label} round ${run}/${RUNS} ====="
  fio --name="${label}-${run}" \
    --filename="$device" \
    --rw=read \
    --bs=4k \
    --ioengine=sync \
    --direct=1 \
    --iodepth=1 \
    --size="${SIZE_MIB}m" \
    --time_based=1 \
    --ramp_time="$RAMP_TIME" \
    --runtime="$RUNTIME" \
    --numjobs=1 \
    --group_reporting=1 \
    --output-format=json \
    --output="$json" >/dev/null
  python3 - "$label" "$run" "$json" "$SUMMARY" <<'PY'
import json, sys
label, run, path, summary = sys.argv[1:5]
job = json.load(open(path))["jobs"][0]["read"]
bw = job.get("bw_bytes", job["bw"] * 1024) / 1048576
with open(summary, "a", encoding="utf-8") as fh:
    fh.write(f"{label},{run},{bw:.3f},{job['iops']:.3f}\n")
PY
}

echo "[linux-4k-sync] filling ${RAW_DEVICE} (${SIZE_MIB} MiB data + 64 MiB hash)"
dd if=/dev/zero of="$RAW_DEVICE" bs=1M count="$((SIZE_MIB + 64))" conv=fsync status=progress

for run in $(seq 1 "$RUNS"); do
  run_sync_read raw "$RAW_DEVICE" "$run"
done

printf '0 %s crypt aes-xts-plain64 %s 0 %s 0\n' "$DATA_SECTORS" "$KEY_HEX" "$RAW_DEVICE" | dmsetup create "$CRYPT_NAME"
CRYPT_DEVICE="/dev/mapper/$CRYPT_NAME"
for run in $(seq 1 "$RUNS"); do
  run_sync_read dm-crypt "$CRYPT_DEVICE" "$run"
done
dmsetup remove "$CRYPT_NAME"

printf '0 %s linear %s 0\n' "$DATA_SECTORS" "$RAW_DEVICE" | dmsetup create "$DATA_NAME"
printf '0 %s linear %s %s\n' "$HASH_SECTORS" "$RAW_DEVICE" "$DATA_SECTORS" | dmsetup create "$HASH_NAME"
DATA_DEVICE="/dev/mapper/$DATA_NAME"
HASH_DEVICE="/dev/mapper/$HASH_NAME"
dd if=/dev/zero of="$DATA_DEVICE" bs=1M count="$SIZE_MIB" conv=fsync status=none
ROOT_HASH="$(veritysetup format "$DATA_DEVICE" "$HASH_DEVICE" | awk '/Root hash:/ {print $3}')"
veritysetup open "$DATA_DEVICE" "$VERITY_NAME" "$HASH_DEVICE" "$ROOT_HASH"
VERITY_DEVICE="/dev/mapper/$VERITY_NAME"
for run in $(seq 1 "$RUNS"); do
  run_sync_read dm-verity "$VERITY_DEVICE" "$run"
done

{
  echo "platform=linux-qemu-kvm"
  echo "contract=4k_sync_qd1_256m"
  echo "raw_device=$RAW_DEVICE"
  echo "size_mib=$SIZE_MIB"
  echo "runs=$RUNS"
  echo "runtime_seconds=$RUNTIME"
  echo "ramp_seconds=$RAMP_TIME"
  echo "ioengine=sync"
  echo "direct=1"
  echo "iodepth=1"
  echo "cipher=aes-xts-plain64"
  echo "verity_root_hash=$ROOT_HASH"
  echo "fio_version=$(fio --version)"
} > "$OUT_DIR/metadata.txt"

python3 - "$SUMMARY" <<'PY'
import csv, statistics, sys
rows = list(csv.DictReader(open(sys.argv[1], encoding="utf-8")))
print()
print("Linux QEMU 4K sync QD1 256 MiB (five-run median)")
for label in ("raw", "dm-crypt", "dm-verity"):
    vals = sorted(float(r["bw_mib_s"]) for r in rows if r["label"] == label)
    print(f"  {label:10s}  median={statistics.median(vals):.3f} MiB/s  n={len(vals)}")
PY

echo "summary=$SUMMARY"
echo "out=$OUT_DIR"
