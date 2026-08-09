#!/usr/bin/env bash
# Run inside the official Asterinas development container after cargo-osdk is
# installed. Builds plain / dm-crypt / dm-verity boot ISOs for the boot matrix.
# Does not modify dm-crypt/verity core logic.
set -euo pipefail

cd /root/asterinas
OUT=/root/target/benchmark
VERITY_IMAGE=/root/target/verity-rootfs.img
CRYPT_IMAGE=/root/target/crypt-rootfs.img
mkdir -p "${OUT}"

export CONSOLE=hvc0

PLAIN_ARGS='--kcmd-args="console=hvc0" --init-args="dm-root /dev/vdc /.initramfs/bin/true"'
make kernel ENABLE_KVM=0 CARGO_OSDK_BUILD_ARGS="${PLAIN_ARGS}"
cp target/osdk/aster-kernel-osdk-bin.iso "${OUT}/asterinas-plain-root.iso"

if [[ -f "${VERITY_IMAGE}" ]]; then
  # Hash must match the image built by scripts/build_verity_rootfs.sh; override via env.
  VERITY_HASH="${VERITY_HASH:-6c9cf49c4c1b46a9488ceb99155025b09b04b7a356194dcde636a3f150d428ff}"
  VERITY_ARGS="--kcmd-args=\"console=hvc0 rootfs_verity.scheme=dm-verity rootfs_verity.device=vdc rootfs_verity.name=dm-verity0 rootfs_verity.hash=${VERITY_HASH} rootfs_verity.data_blocks=4096 rootfs_verity.hash_start=4096 rootfs_verity.cache_blocks=256\" --init-args=\"dm-root /dev/dm-verity0 /.initramfs/bin/true\""
  make kernel ENABLE_KVM=0 CARGO_OSDK_BUILD_ARGS="${VERITY_ARGS}"
  cp target/osdk/aster-kernel-osdk-bin.iso "${OUT}/asterinas-verity-root.iso"
else
  echo "skip verity ISO: missing ${VERITY_IMAGE}" >&2
fi

if [[ -f "${CRYPT_IMAGE}" ]]; then
  CRYPT_KEY="${CRYPT_KEY:-000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f}"
  CRYPT_ARGS="--kcmd-args=\"console=hvc0 dm_crypt.device=vdc dm_crypt.name=dm-crypt0 dm_crypt.key=${CRYPT_KEY}\" --init-args=\"dm-root /dev/dm-crypt0 /.initramfs/bin/true\""
  make kernel ENABLE_KVM=0 CARGO_OSDK_BUILD_ARGS="${CRYPT_ARGS}"
  cp target/osdk/aster-kernel-osdk-bin.iso "${OUT}/asterinas-crypt-root.iso"
else
  echo "skip crypt ISO: missing ${CRYPT_IMAGE} (build with scripts/build_crypt_rootfs.sh)" >&2
fi

sha256sum "${OUT}"/asterinas-*-root.iso
