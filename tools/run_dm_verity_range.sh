#!/bin/bash
set -euo pipefail

DM_IMAGE=${DM_IMAGE:?}
DM_ROOT_HASH=${DM_ROOT_HASH:?}
DM_DATA_BLOCKS=${DM_DATA_BLOCKS:?}
DM_HASH_START=${DM_HASH_START:?}
RANGE_START=${RANGE_START:-0}
RANGE_COUNT=${RANGE_COUNT:-1}
RANGE_REPEATS=${RANGE_REPEATS:-1}
RANGE_LABEL=${RANGE_LABEL:-V09}
DM_CACHE_BLOCKS=${DM_CACHE_BLOCKS:-256}
export DM_IMAGE

KCMD_ARGS="console=hvc0 rootfs_verity.scheme=dm-verity rootfs_verity.device=vdc rootfs_verity.name=dm-verity0 rootfs_verity.hash=${DM_ROOT_HASH} rootfs_verity.data_blocks=${DM_DATA_BLOCKS} rootfs_verity.hash_start=${DM_HASH_START} rootfs_verity.cache_blocks=${DM_CACHE_BLOCKS}"
OSDK_ARGS="--kcmd-args=\"${KCMD_ARGS}\" --init-args=\"verity-range /dev/dm-verity0 ${RANGE_START} ${RANGE_COUNT} ${RANGE_REPEATS} ${RANGE_LABEL} /bin/true\" --qemu-args=\"-accel kvm -cpu host\""
exec make run_kernel ENABLE_KVM=0 CARGO_OSDK_BUILD_ARGS="${OSDK_ARGS}"
