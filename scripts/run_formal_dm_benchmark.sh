#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ASTERINAS_DIR="${ASTERINAS_DIR:-$ROOT_DIR/asterinas}"
OUTPUT_DIR="${OUTPUT_DIR:-$ROOT_DIR/target/benchmark/formal-256m}"
CRYPT_IMAGE="${CRYPT_IMAGE:-$ROOT_DIR/target/crypt-rootfs.img}"
VERITY_IMAGE="${VERITY_IMAGE:-$ROOT_DIR/target/verity-rootfs.img}"

if [[ "${1:-}" == "--build-images" ]]; then
  DATA_SIZE_MIB=256 CONTAINER_SIZE_MIB=264 \
    bash "$ROOT_DIR/scripts/build_crypt_rootfs.sh" "$CRYPT_IMAGE"
  DATA_SIZE_MIB=256 CONTAINER_SIZE_MIB=264 \
    bash "$ROOT_DIR/scripts/build_verity_rootfs.sh" "$VERITY_IMAGE"
fi

for image in "$CRYPT_IMAGE" "$VERITY_IMAGE"; do
  if [[ ! -f "$image" || ! -f "${image}.env" ]]; then
    echo "missing benchmark image or metadata: $image" >&2
    exit 1
  fi
done

mkdir -p "$OUTPUT_DIR"
(
  cd "$ASTERINAS_DIR"
  DM_IMAGE="$CRYPT_IMAGE" DM_BENCH_METADATA="$OUTPUT_DIR/dm-crypt.metadata.txt" \
    ENABLE_KVM=1 SMP=1 MEM=8G DM_BENCH_SIZE=256m DM_BENCH_RUNTIME=10 \
    DM_BENCH_RAMP=2 DM_BENCH_RUNS=5 DM_PERF_STATS=1 RELEASE_LTO=1 \
    bash tools/run_dm_crypt_bench.sh
) | tee "$OUTPUT_DIR/dm-crypt.log"
(
  cd "$ASTERINAS_DIR"
  DM_IMAGE="$VERITY_IMAGE" DM_BENCH_METADATA="$OUTPUT_DIR/dm-verity.metadata.txt" \
    ENABLE_KVM=1 SMP=1 MEM=8G DM_BENCH_SIZE=256m DM_BENCH_RUNTIME=10 \
    DM_BENCH_RAMP=2 DM_BENCH_RUNS=5 DM_PERF_STATS=1 RELEASE_LTO=1 \
    bash tools/run_dm_verity_bench.sh
) | tee "$OUTPUT_DIR/dm-verity.log"

python3 "$ROOT_DIR/scripts/summarize_asterinas_dm_crypt_bench.py" \
  "$OUTPUT_DIR/dm-crypt.log" "$OUTPUT_DIR/dm-crypt.csv" "$OUTPUT_DIR/dm-crypt-summary.csv"
python3 "$ROOT_DIR/scripts/summarize_asterinas_verity_bench.py" \
  "$OUTPUT_DIR/dm-verity.log" "$OUTPUT_DIR/dm-verity.csv" "$OUTPUT_DIR/dm-verity-summary.csv"
python3 "$ROOT_DIR/scripts/summarize_dm_performance_stages.py" \
  "$OUTPUT_DIR/dm-crypt.log" "$OUTPUT_DIR/dm-crypt-stages.csv" --target dm-crypt
python3 "$ROOT_DIR/scripts/summarize_dm_performance_stages.py" \
  "$OUTPUT_DIR/dm-verity.log" "$OUTPUT_DIR/dm-verity-stages.csv" --target dm-verity
python3 "$ROOT_DIR/scripts/build_unified_dm_table.py" \
  "$OUTPUT_DIR/dm-crypt.csv" "$OUTPUT_DIR/dm-verity.csv" \
  "$OUTPUT_DIR/dm-crypt.metadata.txt" "$OUTPUT_DIR/dm-verity.metadata.txt" \
  "$OUTPUT_DIR/raw-crypt-verity.csv"

echo "formal_evidence=$OUTPUT_DIR"
