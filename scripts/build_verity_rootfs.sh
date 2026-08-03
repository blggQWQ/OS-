#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IMAGE="${1:-$ROOT_DIR/target/verity-rootfs.img}"
DATA_SIZE_MIB="${DATA_SIZE_MIB:-${SIZE_MIB:-256}}"
CONTAINER_SIZE_MIB="${CONTAINER_SIZE_MIB:-264}"
BLOCK_SIZE=4096
DATA_BLOCKS=$((DATA_SIZE_MIB * 1024 * 1024 / BLOCK_SIZE))
STAGING="$ROOT_DIR/target/verity-rootfs-staging"
METADATA="${IMAGE}.env"

for command in mke2fs cargo; do
  command -v "$command" >/dev/null || { echo "missing command: $command" >&2; exit 1; }
done

# `pivot_root(2)` requires put_old to exist below the new root.  The verity
# filesystem is intentionally mounted read-only, so this mount point must be
# part of the authenticated image rather than created during boot.
mkdir -p "$STAGING/etc" "$STAGING/.initramfs" "$(dirname "$IMAGE")"
printf 'ASTERINAS_DM_VERITY_OK\n' > "$STAGING/etc/dm-verity-proof"
printf 'Generated for the Asterinas dm-verity rootfs integration test.\n' > "$STAGING/README"
for path in bin sbin lib lib64 usr nix dev proc sys; do
  if [[ ! -e "$STAGING/$path" && ! -L "$STAGING/$path" ]]; then
    ln -s "/.initramfs/$path" "$STAGING/$path"
  fi
done

if (( CONTAINER_SIZE_MIB < DATA_SIZE_MIB )); then
  echo "CONTAINER_SIZE_MIB must be at least DATA_SIZE_MIB" >&2
  exit 1
fi
truncate -s "$((DATA_SIZE_MIB * 1024 * 1024))" "$IMAGE"
mke2fs -q -F -t ext2 -b "$BLOCK_SIZE" -d "$STAGING" "$IMAGE"

FORMAT_OUTPUT="$(cargo run --release --quiet --manifest-path "$ROOT_DIR/Cargo.toml" -p dmctl -- \
  verity-format-dm "$IMAGE" "$DATA_BLOCKS")"
ROOT_HASH="$(printf '%s\n' "$FORMAT_OUTPUT" | sed -n 's/^rootfs_verity.hash=//p')"
HASH_START="$(printf '%s\n' "$FORMAT_OUTPUT" | sed -n 's/^rootfs_verity.hash_start=//p')"
FORMATTED_BYTES=$(stat -c %s "$IMAGE")
CONTAINER_BYTES=$((CONTAINER_SIZE_MIB * 1024 * 1024))
if (( FORMATTED_BYTES > CONTAINER_BYTES )); then
  echo "verity tree does not fit in ${CONTAINER_SIZE_MIB} MiB container" >&2
  exit 1
fi
truncate -s "$CONTAINER_BYTES" "$IMAGE"

cat > "$METADATA" <<EOF
DM_IMAGE=$IMAGE
DM_DEVICE=vdc
DM_NAME=dm-verity0
DM_ROOT_HASH=$ROOT_HASH
DM_DATA_BLOCKS=$DATA_BLOCKS
DM_HASH_START=$HASH_START
DM_DATA_SIZE_MIB=$DATA_SIZE_MIB
DM_CONTAINER_SIZE_MIB=$CONTAINER_SIZE_MIB
EOF

printf '%s\n' "$FORMAT_OUTPUT"
echo "metadata=$METADATA"
echo "data_size_mib=$DATA_SIZE_MIB"
echo "container_size_mib=$CONTAINER_SIZE_MIB"
echo "proof_file=/etc/dm-verity-proof"
