#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IMAGE="${1:-$ROOT_DIR/target/crypt-rootfs.img}"
SIZE_MIB="${SIZE_MIB:-16}"
MAPPER="asterinas_crypt_build_$$"
KEY_HEX="${DM_KEY:-000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f}"
STAGING="$ROOT_DIR/target/crypt-rootfs-staging"
METADATA="${IMAGE}.env"

for command in dmsetup losetup mke2fs; do
  command -v "$command" >/dev/null || { echo "missing command: $command" >&2; exit 1; }
done
if [[ "${EUID}" -ne 0 ]]; then
  echo "run as root: sudo $0" >&2
  exit 1
fi

mkdir -p "$STAGING/etc" "$(dirname "$IMAGE")"
printf 'ASTERINAS_DM_CRYPT_OK\n' > "$STAGING/etc/dm-verity-proof"
printf 'Linux dm-crypt aes-xts-plain64 compatibility image.\n' > "$STAGING/README"
for path in bin sbin lib lib64 usr dev proc sys; do
  if [[ ! -e "$STAGING/$path" && ! -L "$STAGING/$path" ]]; then
    ln -s "/.initramfs/$path" "$STAGING/$path"
  fi
done

truncate -s "$((SIZE_MIB * 1024 * 1024))" "$IMAGE"
LOOP_DEV="$(losetup --find --show "$IMAGE")"
SECTORS=$((SIZE_MIB * 1024 * 1024 / 512))
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
mke2fs -q -F -t ext2 -b 4096 -d "$STAGING" "/dev/mapper/$MAPPER"
sync
remove_mapper
losetup -d "$LOOP_DEV"
trap - EXIT

cat > "$METADATA" <<EOF
DM_IMAGE=$IMAGE
DM_DEVICE=vdc
DM_NAME=dm-crypt0
DM_KEY=$KEY_HEX
EOF

echo "image=$IMAGE"
echo "metadata=$METADATA"
echo "cipher=aes-xts-plain64"
echo "proof_file=/etc/dm-verity-proof"
