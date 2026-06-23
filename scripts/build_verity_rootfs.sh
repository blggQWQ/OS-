#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IMAGE="${1:-$ROOT_DIR/target/verity-rootfs.img}"
SIZE_MIB="${SIZE_MIB:-16}"
BLOCK_SIZE=4096
DATA_BLOCKS=$((SIZE_MIB * 1024 * 1024 / BLOCK_SIZE))
STAGING="$ROOT_DIR/target/verity-rootfs-staging"
METADATA="${IMAGE}.env"

for command in mke2fs cargo; do
  command -v "$command" >/dev/null || { echo "missing command: $command" >&2; exit 1; }
done

mkdir -p "$STAGING/etc" "$(dirname "$IMAGE")"
printf 'ASTERINAS_DM_VERITY_OK\n' > "$STAGING/etc/dm-verity-proof"
printf 'Generated for the Asterinas dm-verity rootfs integration test.\n' > "$STAGING/README"
for path in bin sbin lib lib64 usr dev proc sys; do
  if [[ ! -e "$STAGING/$path" && ! -L "$STAGING/$path" ]]; then
    ln -s "/.initramfs/$path" "$STAGING/$path"
  fi
done

truncate -s "$((SIZE_MIB * 1024 * 1024))" "$IMAGE"
mke2fs -q -F -t ext2 -b "$BLOCK_SIZE" -d "$STAGING" "$IMAGE"

FORMAT_OUTPUT="$(cargo run --quiet --manifest-path "$ROOT_DIR/Cargo.toml" -p dmctl -- \
  verity-format-dm "$IMAGE" "$DATA_BLOCKS")"
ROOT_HASH="$(printf '%s\n' "$FORMAT_OUTPUT" | sed -n 's/^rootfs_verity.hash=//p')"
HASH_START="$(printf '%s\n' "$FORMAT_OUTPUT" | sed -n 's/^rootfs_verity.hash_start=//p')"

cat > "$METADATA" <<EOF
DM_IMAGE=$IMAGE
DM_DEVICE=vdc
DM_NAME=dm-verity0
DM_ROOT_HASH=$ROOT_HASH
DM_DATA_BLOCKS=$DATA_BLOCKS
DM_HASH_START=$HASH_START
EOF

printf '%s\n' "$FORMAT_OUTPUT"
echo "metadata=$METADATA"
echo "proof_file=/etc/dm-verity-proof"
