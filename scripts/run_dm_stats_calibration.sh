#!/usr/bin/env bash
set -euo pipefail

TARGET="${1:?usage: run_dm_stats_calibration.sh crypt|verity}"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ASTERINAS_DIR="${ASTERINAS_DIR:-$ROOT_DIR/asterinas}"
OUTPUT_DIR="${OUTPUT_DIR:-$ROOT_DIR/target/benchmark/stats-calibration-$TARGET}"
mkdir -p "$OUTPUT_DIR"

case "$TARGET" in
  crypt)
    IMAGE="${DM_IMAGE:-$ROOT_DIR/target/crypt-rootfs.img}"
    RUNNER=tools/run_dm_crypt_bench.sh
    SUMMARIZER=scripts/summarize_asterinas_dm_crypt_bench.py
    ;;
  verity)
    IMAGE="${DM_IMAGE:-$ROOT_DIR/target/verity-rootfs.img}"
    RUNNER=tools/run_dm_verity_bench.sh
    SUMMARIZER=scripts/summarize_asterinas_verity_bench.py
    ;;
  *)
    echo "target must be crypt or verity" >&2
    exit 1
    ;;
esac

for ENABLED in 0 1; do
  (
    cd "$ASTERINAS_DIR"
    DM_IMAGE="$IMAGE" DM_BENCH_METADATA="$OUTPUT_DIR/stats-$ENABLED.metadata.txt" \
      ENABLE_KVM=1 SMP=1 MEM=8G DM_BENCH_SIZE=256m DM_BENCH_RUNTIME=10 \
      DM_BENCH_RAMP=2 DM_BENCH_RUNS=5 DM_PERF_STATS="$ENABLED" RELEASE_LTO=1 \
      bash "$RUNNER"
  ) | tee "$OUTPUT_DIR/stats-$ENABLED.log"
  python3 "$ROOT_DIR/$SUMMARIZER" "$OUTPUT_DIR/stats-$ENABLED.log" \
    "$OUTPUT_DIR/stats-$ENABLED.csv" "$OUTPUT_DIR/stats-$ENABLED-summary.csv"
done

python3 "$ROOT_DIR/scripts/compare_dm_stats_overhead.py" \
  "$OUTPUT_DIR/stats-0.csv" "$OUTPUT_DIR/stats-1.csv" "$OUTPUT_DIR/overhead.csv"
