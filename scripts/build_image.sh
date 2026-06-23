#!/usr/bin/env bash
set -euo pipefail

IMAGE="${1:-target/test.img}"
BLOCKS="${2:-128}"
BLOCK_SIZE="${3:-4096}"

mkdir -p "$(dirname "$IMAGE")"
cargo run -p dmctl -- init-image "$IMAGE" "$BLOCKS" "$BLOCK_SIZE"

echo "image=$IMAGE blocks=$BLOCKS block_size=$BLOCK_SIZE"

