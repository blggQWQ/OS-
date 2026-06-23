#!/bin/bash

# SPDX-License-Identifier: MPL-2.0

set -euo pipefail

DM_IMAGE=${DM_IMAGE:-./dm-test.img}
DM_DEVICE=${DM_DEVICE:-vdc}
DM_NAME=${DM_NAME:-dm-verity0}
DM_ROOT_HASH=${DM_ROOT_HASH:-6db653b69a094065756dc2770d3af8880ec9beed29df41b5bb2fb2e11b2d49f3}
DM_DATA_BLOCKS=${DM_DATA_BLOCKS:-16}
DM_HASH_START=${DM_HASH_START:-16}

if [ ! -f "${DM_IMAGE}" ]; then
    echo "dm-verity image not found: ${DM_IMAGE}" >&2
    exit 1
fi

KCMD_ARGS="rootfs_verity.scheme=dm-verity \
rootfs_verity.device=${DM_DEVICE} \
rootfs_verity.name=${DM_NAME} \
rootfs_verity.hash=${DM_ROOT_HASH} \
rootfs_verity.data_blocks=${DM_DATA_BLOCKS} \
rootfs_verity.hash_start=${DM_HASH_START}"

export DM_IMAGE
OSDK_ARGS="--kcmd-args=\"${KCMD_ARGS}\" --init-args=\"dm-root /dev/${DM_NAME} /.initramfs/bin/true\""
QEMU_ARGS=""
if [ "${ENABLE_KVM:-0}" = "1" ]; then
    QEMU_ARGS="-accel kvm"
fi
if [ "${DM_GDB:-0}" = "1" ]; then
    QEMU_ARGS="${QEMU_ARGS} -s"
fi
if [ "${DM_GDB_WAIT:-0}" = "1" ]; then
    QEMU_ARGS="${QEMU_ARGS} -S"
fi
if [ -n "${QEMU_ARGS}" ]; then
    OSDK_ARGS="${OSDK_ARGS} --qemu-args=\"${QEMU_ARGS}\""
fi

exec make run_kernel CARGO_OSDK_BUILD_ARGS="${OSDK_ARGS}"
