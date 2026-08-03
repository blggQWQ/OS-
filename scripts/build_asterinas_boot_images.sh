#!/usr/bin/env bash
# Run inside the official Asterinas development container after cargo-osdk is
# installed. The repository is expected at /root/asterinas and target at
# /root/target, matching the documented Docker invocation.
set -euo pipefail

cd /root/asterinas
OUT=/root/target/benchmark
IMAGE=/root/target/verity-rootfs.img
mkdir -p "${OUT}"
[[ -f "${IMAGE}" ]] || {
    echo "missing verity root image: ${IMAGE}" >&2
    exit 1
}

export CONSOLE=hvc0
export DM_IMAGE="${IMAGE}"

PLAIN_ARGS='--kcmd-args="console=hvc0" --init-args="dm-root /dev/vdc /.initramfs/bin/true"'
make kernel ENABLE_KVM=0 CARGO_OSDK_BUILD_ARGS="${PLAIN_ARGS}"
cp target/osdk/aster-kernel-osdk-bin.iso "${OUT}/asterinas-plain-root.iso"

VERITY_ARGS='--kcmd-args="console=hvc0 rootfs_verity.scheme=dm-verity rootfs_verity.device=vdc rootfs_verity.name=dm-verity0 rootfs_verity.hash=6c9cf49c4c1b46a9488ceb99155025b09b04b7a356194dcde636a3f150d428ff rootfs_verity.data_blocks=4096 rootfs_verity.hash_start=4096 rootfs_verity.cache_blocks=256" --init-args="dm-root /dev/dm-verity0 /.initramfs/bin/true"'
make kernel ENABLE_KVM=0 CARGO_OSDK_BUILD_ARGS="${VERITY_ARGS}"
cp target/osdk/aster-kernel-osdk-bin.iso "${OUT}/asterinas-verity-root.iso"

sha256sum "${OUT}/asterinas-plain-root.iso" "${OUT}/asterinas-verity-root.iso"
