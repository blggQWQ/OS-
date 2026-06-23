#!/usr/bin/env bash
set -euo pipefail

ASTERINAS_DIR="${ASTERINAS_DIR:-../asterinas}"
IMAGE="${1:-target/test.img}"

cat <<EOF
QEMU launch template

1. Build Asterinas in:
   $ASTERINAS_DIR

2. Add this drive to the official Asterinas QEMU command:
   -drive file=$IMAGE,format=raw,if=virtio

3. Optional dm-verity cmdline:
   rootfs_verity.scheme=dm-verity rootfs_verity.hash=<root_hash_hex>

Use the exact build/run command from your checked-out Asterinas version.
EOF

