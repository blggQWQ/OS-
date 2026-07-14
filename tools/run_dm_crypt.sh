#!/bin/bash

# SPDX-License-Identifier: MPL-2.0

set -euo pipefail

# Freestanding targets deliberately disable cpufeatures runtime dispatch.
# Enable dm's isolated AES-NI data path; it pins each transform and excludes
# interrupts around the small architecture-specific XMM boundary.
if [ "${DM_AES_NI:-1}" = "1" ]; then
    export OSDK_ENABLE_AES_NI=1
else
    unset OSDK_ENABLE_AES_NI
fi

DM_IMAGE=${DM_IMAGE:-./dm-crypt-test.img}
DM_DEVICE=${DM_DEVICE:-vdc}
DM_NAME=${DM_NAME:-dm-crypt0}
DM_KEY=${DM_KEY:-000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f}
export CONSOLE=${DM_CONSOLE:-hvc0}

if [ ! -f "${DM_IMAGE}" ]; then
    echo "dm-crypt image not found: ${DM_IMAGE}" >&2
    exit 1
fi

KCMD_ARGS="console=${CONSOLE} \
dm_crypt.device=${DM_DEVICE} \
dm_crypt.name=${DM_NAME} \
dm_crypt.key=${DM_KEY}"
export DM_IMAGE
OSDK_ARGS="--kcmd-args=\"${KCMD_ARGS}\" --init-args=\"dm-root /dev/${DM_NAME} /.initramfs/bin/true\""
QEMU_ARGS=""
if [ "${ENABLE_KVM:-0}" = "1" ]; then
    QEMU_ARGS="-accel kvm -cpu host"
fi
if [ "${DM_QEMU_DEBUG:-0}" = "1" ]; then
    QEMU_ARGS="${QEMU_ARGS} -d int,cpu_reset,guest_errors -D /root/target/qemu-int.log -no-reboot"
fi
if [ -n "${QEMU_ARGS}" ]; then
    OSDK_ARGS="${OSDK_ARGS} --qemu-args=\"${QEMU_ARGS}\""
fi

# The script already supplies the complete QEMU argument set.  Prevent the
# top-level Makefile's ENABLE_KVM branch from appending a second --qemu-args
# value that can replace `-cpu host` and hide AES-NI from the guest.
exec make run_kernel ENABLE_KVM=0 CARGO_OSDK_BUILD_ARGS="${OSDK_ARGS}"
