#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUTPUT="${1:-$ROOT_DIR/target/benchmark/linux-dm.csv}"
METADATA="${OUTPUT%.csv}.metadata.txt"
WORK_DIR="${WORK_DIR:-$ROOT_DIR/target/benchmark/linux-work}"
SIZE_MIB="${SIZE_MIB:-512}"
RUNTIME="${RUNTIME:-20}"
RUNS="${RUNS:-5}"
BLOCK_SIZE="${BLOCK_SIZE:-4k}"
QUEUE_DEPTHS="${QUEUE_DEPTHS:-1 32}"
KEY_HEX="${DM_KEY:-000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f}"
MAPPER="cscc-bench-$$"
IMAGE="$WORK_DIR/backing.img"

for command in fio jq losetup dmsetup blockdev; do
  command -v "$command" >/dev/null || {
    echo "missing command: $command" >&2
    exit 1
  }
done
if [[ "$EUID" -ne 0 ]]; then
  echo "run as root: sudo $0 [output.csv]" >&2
  exit 1
fi
if (( RUNS < 3 )); then
  echo "RUNS must be at least 3 for a defensible median" >&2
  exit 1
fi

mkdir -p "$WORK_DIR" "$(dirname "$OUTPUT")"
{
  echo "timestamp_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "kernel=$(uname -srmo)"
  echo "cpu=$(lscpu | sed -n 's/^Model name:[[:space:]]*//p')"
  echo "cpu_count=$(nproc)"
  echo "aes_flag=$(grep -q -m1 -w aes /proc/cpuinfo && echo true || echo false)"
  echo "fio=$(fio --version)"
  echo "dmsetup=$(dmsetup version 2>/dev/null | sed -n 's/^Driver version:[[:space:]]*//p')"
  echo "size_mib=$SIZE_MIB"
  echo "runtime_seconds=$RUNTIME"
  echo "runs=$RUNS"
  echo "block_size=$BLOCK_SIZE"
  echo "queue_depths=$QUEUE_DEPTHS"
  echo "cipher=aes-xts-plain64"
  echo "crypt_options=no_read_workqueue,no_write_workqueue"
} > "$METADATA"
truncate -s "${SIZE_MIB}M" "$IMAGE"
LOOP_DEV="$(losetup --find --show --direct-io=on "$IMAGE")"
SECTORS="$(blockdev --getsz "$LOOP_DEV")"

cleanup() {
  set +e
  dmsetup remove "$MAPPER" >/dev/null 2>&1
  losetup -d "$LOOP_DEV" >/dev/null 2>&1
}
trap cleanup EXIT

printf '0 %s crypt aes-xts-plain64 %s 0 %s 0 2 no_read_workqueue no_write_workqueue\n' \
  "$SECTORS" "$KEY_HEX" "$LOOP_DEV" | dmsetup create "$MAPPER"
CRYPT_DEV="/dev/mapper/$MAPPER"

printf '%s\n' \
  'implementation,operation,block_size,queue_depth,run,mib_per_second,iops,mean_latency_us,p50_latency_us,p95_latency_us,p99_latency_us,cpu_percent' \
  > "$OUTPUT"

run_case() {
  local implementation="$1"
  local filename="$2"
  local operation="$3"
  local queue_depth="$4"
  local run="$5"
  local json="$WORK_DIR/${implementation}-${operation}-q${queue_depth}-r${run}.json"

  fio --name=dm-bench \
    --filename="$filename" \
    --rw="$operation" \
    --bs="$BLOCK_SIZE" \
    --ioengine=libaio \
    --iodepth="$queue_depth" \
    --direct=1 \
    --time_based=1 \
    --runtime="$RUNTIME" \
    --ramp_time=3 \
    --size="${SIZE_MIB}M" \
    --group_reporting=1 \
    --output-format=json \
    --output="$json"

  jq -r --arg implementation "$implementation" \
    --arg operation "$operation" \
    --arg block_size "$BLOCK_SIZE" \
    --arg queue_depth "$queue_depth" \
    --arg run "$run" '
      .jobs[0] as $job |
      (if $operation == "read" or $operation == "randread" then $job.read else $job.write end) as $io |
      [
        $implementation,
        $operation,
        $block_size,
        $queue_depth,
        $run,
        ($io.bw_bytes / 1048576),
        $io.iops,
        ($io.clat_ns.mean / 1000),
        ($io.clat_ns.percentile["50.000000"] / 1000),
        ($io.clat_ns.percentile["95.000000"] / 1000),
        ($io.clat_ns.percentile["99.000000"] / 1000),
        ($job.usr_cpu + $job.sys_cpu)
      ] | @csv' "$json" >> "$OUTPUT"
}

# Materialize the sparse backing file, then bring both targets to the same
# warm-cache state before collecting any result.
fio --name=precondition --filename="$LOOP_DEV" --rw=write --bs=1M \
  --ioengine=libaio --iodepth=32 --direct=1 --size="${SIZE_MIB}M" >/dev/null
fio --name=warmup --filename="$LOOP_DEV" --rw=read --bs="$BLOCK_SIZE" \
  --ioengine=libaio --iodepth=32 --direct=1 --size="${SIZE_MIB}M" >/dev/null
fio --name=warmup --filename="$CRYPT_DEV" --rw=read --bs="$BLOCK_SIZE" \
  --ioengine=libaio --iodepth=32 --direct=1 --size="${SIZE_MIB}M" >/dev/null

for queue_depth in $QUEUE_DEPTHS; do
  for operation in read write randread randwrite; do
    for run in $(seq 1 "$RUNS"); do
      # Alternate target order per run to reduce thermal/order bias.
      if (( run % 2 == 1 )); then
        run_case linux-raw "$LOOP_DEV" "$operation" "$queue_depth" "$run"
        run_case linux-dm-crypt "$CRYPT_DEV" "$operation" "$queue_depth" "$run"
      else
        run_case linux-dm-crypt "$CRYPT_DEV" "$operation" "$queue_depth" "$run"
        run_case linux-raw "$LOOP_DEV" "$operation" "$queue_depth" "$run"
      fi
    done
  done
done

sync
echo "benchmark_csv=$OUTPUT"
echo "benchmark_metadata=$METADATA"
echo "raw_device=$LOOP_DEV"
echo "crypt_device=$CRYPT_DEV"
echo "Keep the per-run CSV; compute medians rather than selecting the best run." >&2
