#!/usr/bin/env bash
# Build a Combined-stage rootfs: plaintext ext2 + verity hash tree, then
# whole-disk AES-XTS encryption. Layout after decrypt:
#   [ext2 data blocks][verity hash tree...]
# Boot stack (disk → FS): virtio-blk → dm-crypt → dm-verity → ext2
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IMAGE="${1:-$ROOT_DIR/target/stack-rootfs.img}"
DATA_SIZE_MIB="${DATA_SIZE_MIB:-${SIZE_MIB:-256}}"
CONTAINER_SIZE_MIB="${CONTAINER_SIZE_MIB:-264}"
BLOCK_SIZE=4096
DATA_BLOCKS=$((DATA_SIZE_MIB * 1024 * 1024 / BLOCK_SIZE))
STAGING="$ROOT_DIR/target/stack-rootfs-staging"
PLAIN="$ROOT_DIR/target/stack-rootfs-plain.img"
METADATA="${IMAGE}.env"
MAPPER="asterinas_stack_build_$$"
KEY_HEX="${DM_KEY:-000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f}"

# sudo drops rustup's PATH; recover cargo from the invoking user when possible.
if ! command -v cargo >/dev/null; then
  invoke_home="${HOME:-}"
  if [[ -n "${SUDO_USER:-}" ]]; then
    invoke_home="$(getent passwd "$SUDO_USER" | cut -d: -f6)"
    # rustup looks at HOME; keep it pointed at the invoking user under sudo.
    export HOME="$invoke_home"
    export CARGO_HOME="${CARGO_HOME:-$invoke_home/.cargo}"
    export RUSTUP_HOME="${RUSTUP_HOME:-$invoke_home/.rustup}"
  fi
  for candidate in \
    "${CARGO_HOME:+$CARGO_HOME/bin}" \
    "${invoke_home:+$invoke_home/.cargo/bin}"
  do
    [[ -n "$candidate" && -x "$candidate/cargo" ]] || continue
    PATH="$candidate:$PATH"
    break
  done
  export PATH
fi

for command in mke2fs dmsetup losetup dd; do
  command -v "$command" >/dev/null || { echo "missing command: $command" >&2; exit 1; }
done
if ! command -v cargo >/dev/null; then
  echo "missing command: cargo (install rustup, or: sudo env \"PATH=\$PATH\" bash $0)" >&2
  exit 1
fi
if [[ "${EUID}" -ne 0 ]]; then
  echo "run as root: sudo $0" >&2
  exit 1
fi

mkdir -p "$STAGING/etc" "$STAGING/.initramfs" "$(dirname "$IMAGE")"
printf 'ASTERINAS_DM_STACK_OK\n' > "$STAGING/etc/dm-verity-proof"
printf 'Stacked dm-crypt + dm-verity Asterinas rootfs image.\n' > "$STAGING/README"
for path in bin sbin lib lib64 usr nix dev proc sys; do
  if [[ ! -e "$STAGING/$path" && ! -L "$STAGING/$path" ]]; then
    ln -s "/.initramfs/$path" "$STAGING/$path"
  fi
done

if (( CONTAINER_SIZE_MIB < DATA_SIZE_MIB )); then
  echo "CONTAINER_SIZE_MIB must be at least DATA_SIZE_MIB" >&2
  exit 1
fi

# 1) Plaintext ext2 data region
truncate -s "$((DATA_SIZE_MIB * 1024 * 1024))" "$PLAIN"
mke2fs -q -F -t ext2 -b "$BLOCK_SIZE" -d "$STAGING" "$PLAIN"

# 2) Append on-disk verity hash tree (same helper as build_verity_rootfs.sh)
FORMAT_OUTPUT="$(cargo run --release --quiet --manifest-path "$ROOT_DIR/Cargo.toml" -p dmctl -- \
  verity-format-dm "$PLAIN" "$DATA_BLOCKS")"
ROOT_HASH="$(printf '%s\n' "$FORMAT_OUTPUT" | sed -n 's/^rootfs_verity.hash=//p')"
HASH_START="$(printf '%s\n' "$FORMAT_OUTPUT" | sed -n 's/^rootfs_verity.hash_start=//p')"
FORMATTED_BYTES=$(stat -c %s "$PLAIN")
CONTAINER_BYTES=$((CONTAINER_SIZE_MIB * 1024 * 1024))
if (( FORMATTED_BYTES > CONTAINER_BYTES )); then
  echo "verity tree does not fit in ${CONTAINER_SIZE_MIB} MiB container" >&2
  exit 1
fi
truncate -s "$CONTAINER_BYTES" "$PLAIN"

# 3) Whole-disk AES-XTS encrypt into the final image
truncate -s "$CONTAINER_BYTES" "$IMAGE"
LOOP_DEV="$(losetup --find --show "$IMAGE")"
SECTORS=$((CONTAINER_BYTES / 512))
cleanup() {
  set +e
  remove_mapper >/dev/null 2>&1
  losetup -d "$LOOP_DEV" >/dev/null 2>&1
}
remove_mapper() {
  for _ in $(seq 1 20); do
    if dmsetup remove "$MAPPER"; then
      return 0
    fi
    sync
    sleep 0.2
  done
  return 1
}
trap cleanup EXIT

echo "0 $SECTORS crypt aes-xts-plain64 $KEY_HEX 0 $LOOP_DEV 0" | dmsetup create "$MAPPER"
dmsetup mknodes "$MAPPER"
if [[ ! -b "/dev/mapper/$MAPPER" ]]; then
  echo "dmsetup created $MAPPER but its block node is unavailable" >&2
  exit 1
fi
dd if="$PLAIN" of="/dev/mapper/$MAPPER" bs=1M status=none conv=fsync
sync
remove_mapper
losetup -d "$LOOP_DEV"
trap - EXIT
rm -f "$PLAIN"

# Do not embed an absolute DM_IMAGE path: Docker runners remount the image.
cat > "$METADATA" <<EOF
DM_DEVICE=vdc
DM_NAME_CRYPT=dm-crypt0
DM_NAME_VERITY=dm-verity0
DM_KEY=$KEY_HEX
DM_ROOT_HASH=$ROOT_HASH
DM_DATA_BLOCKS=$DATA_BLOCKS
DM_HASH_START=$HASH_START
DM_DATA_SIZE_MIB=$DATA_SIZE_MIB
DM_CONTAINER_SIZE_MIB=$CONTAINER_SIZE_MIB
EOF

printf '%s\n' "$FORMAT_OUTPUT"
echo "image=$IMAGE"
echo "metadata=$METADATA"
echo "cipher=aes-xts-plain64"
echo "stack=crypt,verity"
echo "data_size_mib=$DATA_SIZE_MIB"
echo "container_size_mib=$CONTAINER_SIZE_MIB"
echo "proof_file=/etc/dm-verity-proof"
echo "boot_hint=dm_stack=crypt,verity rootfs_verity.device=dm-crypt0 dm-root /dev/dm-verity0"
if [[ -n "${SUDO_USER:-}" ]]; then
  chown "$SUDO_USER:$SUDO_USER" "$IMAGE" "$METADATA" 2>/dev/null || true
fi
