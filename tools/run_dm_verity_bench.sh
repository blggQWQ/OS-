#!/bin/bash

# SPDX-License-Identifier: MPL-2.0

set -euo pipefail

DM_IMAGE=${DM_IMAGE:-./dm-test.img}
DM_DEVICE=${DM_DEVICE:-vdc}
DM_NAME=${DM_NAME:-dm-verity0}
DM_ROOT_HASH=${DM_ROOT_HASH:-6db653b69a094065756dc2770d3af8880ec9beed29df41b5bb2fb2e11b2d49f3}
DM_DATA_BLOCKS=${DM_DATA_BLOCKS:-16}
DM_HASH_START=${DM_HASH_START:-16}
DM_CACHE_BLOCKS=${DM_CACHE_BLOCKS:-256}
export CONSOLE=${DM_CONSOLE:-hvc0}

if [ ! -f "${DM_IMAGE}" ]; then
    echo "dm-verity image not found: ${DM_IMAGE}" >&2
    exit 1
fi

KCMD_ARGS="console=${CONSOLE} \
rootfs_verity.scheme=dm-verity \
rootfs_verity.device=${DM_DEVICE} \
rootfs_verity.name=${DM_NAME} \
rootfs_verity.hash=${DM_ROOT_HASH} \
rootfs_verity.data_blocks=${DM_DATA_BLOCKS} \
rootfs_verity.hash_start=${DM_HASH_START} \
rootfs_verity.cache_blocks=${DM_CACHE_BLOCKS}"

export DM_IMAGE
OSDK_ARGS="--kcmd-args=\"${KCMD_ARGS}\" --init-args=\"dm-bench /dev/${DM_DEVICE} /dev/${DM_NAME} /bin/true\""
if [ "${ENABLE_KVM:-0}" = "1" ]; then
    OSDK_ARGS="${OSDK_ARGS} --qemu-args=\"-accel kvm -cpu host\""
fi

exec make run_kernel ENABLE_KVM=0 BENCHMARK=dm-verity CARGO_OSDK_BUILD_ARGS="${OSDK_ARGS}"
