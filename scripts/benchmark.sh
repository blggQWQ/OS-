#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUTPUT="${1:-$ROOT_DIR/target/benchmark/rust.csv}"
BLOCKS="${BLOCKS:-4096}"
ITERATIONS="${ITERATIONS:-5}"

mkdir -p "$(dirname "$OUTPUT")"
cargo run --release --quiet --manifest-path "$ROOT_DIR/Cargo.toml" -p dmctl -- \
  benchmark "$BLOCKS" "$ITERATIONS" | tee "$OUTPUT"

echo "benchmark_csv=$OUTPUT" >&2
echo "Run on a quiet machine and repeat at least three times for defense-grade results." >&2
