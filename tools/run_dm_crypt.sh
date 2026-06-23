#!/bin/bash

# SPDX-License-Identifier: MPL-2.0

set -euo pipefail

DM_IMAGE=${DM_IMAGE:-./dm-crypt-test.img}
DM_DEVICE=${DM_DEVICE:-vdc}
DM_NAME=${DM_NAME:-dm-crypt0}
DM_KEY=${DM_KEY:-000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f}

if [ ! -f "${DM_IMAGE}" ]; then
    echo "dm-crypt image not found: ${DM_IMAGE}" >&2
    exit 1
fi

KCMD_ARGS="dm_crypt.device=${DM_DEVICE} \
dm_crypt.name=${DM_NAME} \
dm_crypt.key=${DM_KEY}"
export DM_IMAGE
OSDK_ARGS="--kcmd-args=\"${KCMD_ARGS}\" --init-args=\"dm-root /dev/${DM_NAME} /.initramfs/bin/true\""
QEMU_ARGS=""
if [ "${ENABLE_KVM:-0}" = "1" ]; then
    QEMU_ARGS="-accel kvm"
fi
if [ -n "${QEMU_ARGS}" ]; then
    OSDK_ARGS="${OSDK_ARGS} --qemu-args=\"${QEMU_ARGS}\""
fi

exec make run_kernel CARGO_OSDK_BUILD_ARGS="${OSDK_ARGS}"
