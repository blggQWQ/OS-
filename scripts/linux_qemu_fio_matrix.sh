#!/usr/bin/env bash
# Linux QEMU/KVM guest FIO matrix on virtio-blk (/dev/vdb by default).
# Run inside the guest as root.
#
#   sudo bash scripts/linux_qemu_fio_matrix.sh rawcrypt
#   sudo bash scripts/linux_qemu_fio_matrix.sh verity
#
# rawcrypt: 13 cells × 5 runs for raw and dm-crypt (libaio, direct=1).
# verity:   7 read cells × 5 runs; writes recorded as N/A.
# Formal numbers: docs/evidence/linux-qemu-fio-20260816/
set -euo pipefail

MODE="${1:?usage: $0 rawcrypt|verity}"
RAW_DEVICE="${RAW_DEVICE:-/dev/vdb}"
OUT_DIR="${OUT_DIR:-$PWD/linux-qemu-fio-${MODE}}"
RUNS="${RUNS:-5}"
SIZE_MIB="${SIZE_MIB:-256}"
RUNTIME="${RUNTIME:-10}"
RAMP_TIME="${RAMP_TIME:-2}"
KEY_HEX="${DM_KEY:-000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f}"

CRYPT_NAME="cscc-linux-fio-crypt"
DATA_NAME="cscc-linux-fio-verity-data"
HASH_NAME="cscc-linux-fio-verity-hash"
VERITY_NAME="cscc-linux-fio-verity"

for c in fio jq dmsetup blockdev veritysetup awk sha256sum; do
  command -v "$c" >/dev/null || { echo "missing command: $c"; exit 1; }
done

if [[ "$EUID" -ne 0 ]]; then
  echo "run with sudo" >&2
  exit 1
fi

if [[ ! -b "$RAW_DEVICE" ]]; then
  echo "$RAW_DEVICE is not a block device" >&2
  exit 1
fi

cleanup_all() {
  set +e
  dmsetup remove "$VERITY_NAME" >/dev/null 2>&1
  dmsetup remove "$CRYPT_NAME" >/dev/null 2>&1
  dmsetup remove "$DATA_NAME" >/dev/null 2>&1
  dmsetup remove "$HASH_NAME" >/dev/null 2>&1
}
trap cleanup_all EXIT
cleanup_all

mkdir -p "$OUT_DIR"
rm -f "$OUT_DIR/per-run.csv"
find "$OUT_DIR" -mindepth 1 -type d -exec rm -rf {} + 2>/dev/null || true

SIZE_BYTES=$((SIZE_MIB * 1024 * 1024))
DATA_SECTORS=$((SIZE_BYTES / 512))
HASH_SECTORS=$((64 * 1024 * 1024 / 512))
DEVICE_BYTES="$(blockdev --getsize64 "$RAW_DEVICE")"

if [[ "$MODE" == "verity" ]]; then
  REQUIRED_BYTES=$(((SIZE_MIB + 64) * 1024 * 1024))
else
  REQUIRED_BYTES="$SIZE_BYTES"
fi

if (( DEVICE_BYTES < REQUIRED_BYTES )); then
  echo "device too small: $((DEVICE_BYTES / 1024 / 1024)) MiB" >&2
  exit 1
fi

SUMMARY="$OUT_DIR/per-run.csv"
printf '%s\n' 'platform,target,operation,block_size,iodepth,run,status,bw_mib_s,iops,p50_us,p95_us,p99_us,cpu_percent,json' > "$SUMMARY"

run_fio() {
  local target="$1"
  local device="$2"
  local operation="$3"
  local bs="$4"
  local qd="$5"
  local run="$6"
  local dir="$OUT_DIR/$target/${operation}_${bs}_qd${qd}"
  local json="$dir/run-${run}.json"
  local io_key

  mkdir -p "$dir"

  fio --name="${target}-${operation}-${bs}-qd${qd}-r${run}" \
    --filename="$device" \
    --rw="$operation" \
    --bs="$bs" \
    --ioengine=libaio \
    --iodepth="$qd" \
    --direct=1 \
    --time_based=1 \
    --runtime="$RUNTIME" \
    --ramp_time="$RAMP_TIME" \
    --size="${SIZE_MIB}m" \
    --numjobs=1 \
    --group_reporting=1 \
    --output-format=json \
    --output="$json"

  if [[ "$operation" == "read" || "$operation" == "randread" ]]; then
    io_key="read"
  else
    io_key="write"
  fi

  jq -r \
    --arg target "$target" \
    --arg operation "$operation" \
    --arg bs "$bs" \
    --arg qd "$qd" \
    --arg run "$run" \
    --arg io_key "$io_key" \
    --arg json "$json" '
      .jobs[0] as $job |
      $job[$io_key] as $io |
      [
        "linux-qemu-kvm",
        $target,
        $operation,
        $bs,
        $qd,
        $run,
        "ok",
        ($io.bw_bytes / 1048576),
        $io.iops,
        ($io.clat_ns.percentile["50.000000"] / 1000),
        ($io.clat_ns.percentile["95.000000"] / 1000),
        ($io.clat_ns.percentile["99.000000"] / 1000),
        ($job.usr_cpu + $job.sys_cpu),
        $json
      ] | @csv' "$json" >> "$SUMMARY"
}

run_cell() {
  local target="$1"
  local device="$2"
  local operation="$3"
  local bs="$4"
  local qd="$5"
  for run in $(seq 1 "$RUNS"); do
    echo "RUN target=$target op=$operation bs=$bs qd=$qd round=$run/$RUNS"
    run_fio "$target" "$device" "$operation" "$bs" "$qd" "$run"
  done
}

if [[ "$MODE" == "rawcrypt" ]]; then
  dd if=/dev/zero of="$RAW_DEVICE" bs=1M count="$SIZE_MIB" conv=fsync status=progress
  printf '0 %s crypt aes-xts-plain64 %s 0 %s 0\n' "$DATA_SECTORS" "$KEY_HEX" "$RAW_DEVICE" | dmsetup create "$CRYPT_NAME"
  CRYPT_DEVICE="/dev/mapper/$CRYPT_NAME"

  cat > "$OUT_DIR/metadata.txt" <<META
platform=linux-qemu-kvm
targets=raw,dm-crypt
raw_device=$RAW_DEVICE
crypt_device=$CRYPT_DEVICE
size_mib=$SIZE_MIB
runs=$RUNS
runtime_seconds=$RUNTIME
ramp_seconds=$RAMP_TIME
vcpus=1
memory=8G
accelerator=kvm
cpu_model=host
device_model=virtio-blk
fio_version=$(fio --version)
dm_crypt_cipher=aes-xts-plain64
ioengine=libaio
META

  for target in raw dm-crypt; do
    if [[ "$target" == "raw" ]]; then device="$RAW_DEVICE"; else device="$CRYPT_DEVICE"; fi
    for op in read randread write randwrite; do
      for bs in 4k 32k 256k; do
        run_cell "$target" "$device" "$op" "$bs" 1
      done
    done
    run_cell "$target" "$device" randread 4k 16
  done
elif [[ "$MODE" == "verity" ]]; then
  dd if=/dev/zero of="$RAW_DEVICE" bs=1M count="$((SIZE_MIB + 64))" conv=fsync status=progress
  printf '0 %s linear %s 0\n' "$DATA_SECTORS" "$RAW_DEVICE" | dmsetup create "$DATA_NAME"
  printf '0 %s linear %s %s\n' "$HASH_SECTORS" "$RAW_DEVICE" "$DATA_SECTORS" | dmsetup create "$HASH_NAME"
  DATA_DEVICE="/dev/mapper/$DATA_NAME"
  HASH_DEVICE="/dev/mapper/$HASH_NAME"

  dd if=/dev/zero of="$DATA_DEVICE" bs=1M count="$SIZE_MIB" conv=fsync status=none
  ROOT_HASH="$(veritysetup format "$DATA_DEVICE" "$HASH_DEVICE" | awk '/Root hash:/ {print $3}')"
  veritysetup open "$DATA_DEVICE" "$VERITY_NAME" "$HASH_DEVICE" "$ROOT_HASH"
  VERITY_DEVICE="/dev/mapper/$VERITY_NAME"

  cat > "$OUT_DIR/metadata.txt" <<META
platform=linux-qemu-kvm
target=dm-verity
raw_device=$RAW_DEVICE
verity_device=$VERITY_DEVICE
size_mib=$SIZE_MIB
runs=$RUNS
runtime_seconds=$RUNTIME
ramp_seconds=$RAMP_TIME
vcpus=1
memory=8G
accelerator=kvm
cpu_model=host
device_model=virtio-blk
fio_version=$(fio --version)
verity_root_hash=$ROOT_HASH
ioengine=libaio
META

  for op in read randread; do
    for bs in 4k 32k 256k; do
      run_cell dm-verity "$VERITY_DEVICE" "$op" "$bs" 1
    done
  done
  run_cell dm-verity "$VERITY_DEVICE" randread 4k 16

  cat > "$OUT_DIR/na-cells.csv" <<NA
platform,target,operation,block_size,iodepth,status,reason
linux-qemu-kvm,dm-verity,write,4k,1,N/A_READ_ONLY,dm-verity_read_only
linux-qemu-kvm,dm-verity,write,32k,1,N/A_READ_ONLY,dm-verity_read_only
linux-qemu-kvm,dm-verity,write,256k,1,N/A_READ_ONLY,dm-verity_read_only
linux-qemu-kvm,dm-verity,randwrite,4k,1,N/A_READ_ONLY,dm-verity_read_only
linux-qemu-kvm,dm-verity,randwrite,32k,1,N/A_READ_ONLY,dm-verity_read_only
linux-qemu-kvm,dm-verity,randwrite,256k,1,N/A_READ_ONLY,dm-verity_read_only
NA
else
  echo "unknown mode: $MODE" >&2
  exit 1
fi

echo "DONE mode=$MODE"
echo "summary=$SUMMARY"
