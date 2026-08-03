#!/usr/bin/env bash
set -euo pipefail

# Runs the Linux half of the strict Linux/Asterinas comparison. Execute this
# inside a Linux QEMU/KVM guest with the same raw virtio-blk image attached as
# the Asterinas benchmark.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUTPUT="${1:-$ROOT_DIR/target/benchmark/linux-guest-dm-crypt.csv}"
METADATA="${OUTPUT%.csv}.metadata.txt"
WORK_DIR="${WORK_DIR:-$ROOT_DIR/target/benchmark/linux-guest-work}"
RAW_DEVICE="${RAW_DEVICE:-/dev/vdc}"
MAPPER_NAME="${MAPPER_NAME:-cscc-linux-guest-crypt}"
KEY_HEX="${DM_KEY:-000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f}"
RUNS="${RUNS:-5}"
SIZE_MIB="${SIZE_MIB:-8}"
RUNTIME="${RUNTIME:-2}"
RAMP_TIME="${RAMP_TIME:-1}"
VCPUS="${VCPUS:-1}"
MEMORY="${MEMORY:-8G}"
ACCELERATOR="${ACCELERATOR:-kvm}"
CPU_MODEL="${CPU_MODEL:-host}"
DEVICE_MODEL="${DEVICE_MODEL:-virtio-blk}"

for command in fio jq dmsetup blockdev; do
  command -v "$command" >/dev/null || {
    echo "missing command: $command" >&2
    exit 1
  }
done
if [[ "$EUID" -ne 0 ]]; then
  echo "run as root: sudo $0 [output.csv]" >&2
  exit 1
fi
if [[ ! -b "$RAW_DEVICE" ]]; then
  echo "raw benchmark device is not a block device: $RAW_DEVICE" >&2
  exit 1
fi
if (( RUNS < 5 )); then
  echo "RUNS must be at least 5" >&2
  exit 1
fi

mkdir -p "$WORK_DIR" "$(dirname "$OUTPUT")"
SECTORS="$(blockdev --getsz "$RAW_DEVICE")"
REQUIRED_BYTES=$((SIZE_MIB * 1024 * 1024))
AVAILABLE_BYTES="$(blockdev --getsize64 "$RAW_DEVICE")"
if (( AVAILABLE_BYTES < REQUIRED_BYTES )); then
  echo "$RAW_DEVICE is smaller than ${SIZE_MIB} MiB" >&2
  exit 1
fi

cleanup() {
  set +e
  dmsetup remove "$MAPPER_NAME" >/dev/null 2>&1
}
trap cleanup EXIT
dmsetup remove "$MAPPER_NAME" >/dev/null 2>&1 || true
printf '0 %s crypt aes-xts-plain64 %s 0 %s 0\n' \
  "$SECTORS" "$KEY_HEX" "$RAW_DEVICE" | dmsetup create "$MAPPER_NAME"
CRYPT_DEVICE="/dev/mapper/$MAPPER_NAME"

{
  echo "timestamp_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "kernel=$(uname -srmo)"
  echo "guest_cmdline=$(cat /proc/cmdline)"
  echo "guest_cpu=$(lscpu | sed -n 's/^Model name:[[:space:]]*//p')"
  echo "guest_cpu_count=$(nproc)"
  echo "guest_memory_kib=$(awk '/^MemTotal:/ {print $2}' /proc/meminfo)"
  echo "vcpus=$VCPUS"
  echo "memory=$MEMORY"
  echo "accelerator=$ACCELERATOR"
  echo "cpu_model=$CPU_MODEL"
  echo "device_model=$DEVICE_MODEL"
  echo "fio=$(fio --version)"
  echo "image_region_sha256=$(dd if="$RAW_DEVICE" bs=1M count="$SIZE_MIB" status=none | sha256sum | awk '{print $1}')"
  echo "raw_device=$RAW_DEVICE"
  echo "mapper=$CRYPT_DEVICE"
  echo "cipher=aes-xts-plain64"
  echo "rw=read"
  echo "block_size=4k"
  echo "ioengine=sync"
  echo "direct=1"
  echo "queue_depth=1"
  echo "size_mib=$SIZE_MIB"
  echo "ramp_seconds=$RAMP_TIME"
  echo "runtime_seconds=$RUNTIME"
  echo "runs=$RUNS"
} >"$METADATA"

printf '%s\n' \
  'label,run,device,bw_mib_s,iops,p50_us,p95_us,p99_us,backend' >"$OUTPUT"

run_case() {
  local label="$1"
  local device="$2"
  local run="$3"
  local json="$WORK_DIR/${label}-r${run}.json"

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
    --percentile_list=50:95:99:99.9 \
    --output-format=json \
    --output="$json"

  jq -r --arg label "$label" --arg run "$run" --arg device "$device" '
    .jobs[0].read as $io |
    [
      $label,
      $run,
      $device,
      ($io.bw_bytes / 1048576),
      $io.iops,
      ($io.clat_ns.percentile["50.000000"] / 1000),
      ($io.clat_ns.percentile["95.000000"] / 1000),
      ($io.clat_ns.percentile["99.000000"] / 1000),
      "linux-kernel"
    ] | @csv' "$json" >>"$OUTPUT"
}

for run in $(seq 1 "$RUNS"); do
  if (( run % 2 == 1 )); then
    run_case raw "$RAW_DEVICE" "$run"
    run_case dm-crypt "$CRYPT_DEVICE" "$run"
  else
    run_case dm-crypt "$CRYPT_DEVICE" "$run"
    run_case raw "$RAW_DEVICE" "$run"
  fi
done

echo "benchmark_csv=$OUTPUT"
echo "benchmark_metadata=$METADATA"
