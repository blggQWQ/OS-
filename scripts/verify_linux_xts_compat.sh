#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK_DIR="${1:-$ROOT_DIR/target/xts-compat}"
SECTORS="${SECTORS:-2048}"
MAPPER="rust_dm_xts_compat_$$"
KEY_HEX="000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f"

for command in dmsetup losetup sha256sum cmp cargo; do
  command -v "$command" >/dev/null || { echo "missing command: $command" >&2; exit 1; }
done
if [[ "${EUID}" -ne 0 ]]; then
  echo "run as root: sudo $0" >&2
  exit 1
fi

mkdir -p "$WORK_DIR"
IMAGE="$WORK_DIR/device.img"
LINUX_RAW="$WORK_DIR/linux.raw"
RUST_RAW="$WORK_DIR/rust.raw"
truncate -s "$((SECTORS * 512))" "$IMAGE"
LOOP_DEV="$(losetup --find --show "$IMAGE")"
cleanup() {
  set +e
  dmsetup remove "$MAPPER" >/dev/null 2>&1
  losetup -d "$LOOP_DEV" >/dev/null 2>&1
}
trap cleanup EXIT

echo "0 $SECTORS crypt aes-xts-plain64 $KEY_HEX 0 $LOOP_DEV 0" | dmsetup create "$MAPPER"
head -c 512 /dev/zero | tr '\0' '\132' | dd of="/dev/mapper/$MAPPER" bs=512 count=1 conv=fsync status=none
dd if="$IMAGE" of="$LINUX_RAW" bs=512 count=1 status=none
dmsetup remove "$MAPPER"
losetup -d "$LOOP_DEV"
trap - EXIT

truncate -s "$((SECTORS * 512))" "$IMAGE"
cargo run --quiet --manifest-path "$ROOT_DIR/Cargo.toml" -p dmctl -- \
  crypt-table-fill "$IMAGE" 512 "aes-xts-plain64 $KEY_HEX 0 ignored 0" 0 5a 1
dd if="$IMAGE" of="$RUST_RAW" bs=512 count=1 status=none

cmp "$LINUX_RAW" "$RUST_RAW"
echo "linux_sha256=$(sha256sum "$LINUX_RAW" | awk '{print $1}')"
echo "rust_sha256=$(sha256sum "$RUST_RAW" | awk '{print $1}')"
echo "aes-xts-plain64 compatibility: PASS"
