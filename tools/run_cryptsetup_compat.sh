#!/bin/bash

# SPDX-License-Identifier: MPL-2.0

set -euo pipefail

export OSDK_ENABLE_AES_NI=1

DM_IMAGE=${DM_IMAGE:-./dm-crypt-test.img}
export CONSOLE=${DM_CONSOLE:-hvc0}
if [ ! -f "${DM_IMAGE}" ]; then
    echo "cryptsetup test image not found: ${DM_IMAGE}" >&2
    exit 1
fi

export DM_IMAGE
OSDK_ARGS="--kcmd-args=\"console=${CONSOLE}\" --init-args=\"cryptsetup-smoke /dev/vdc /bin/true\""
if [ "${ENABLE_KVM:-0}" = "1" ]; then
    OSDK_ARGS="${OSDK_ARGS} --qemu-args=\"-accel kvm -cpu host\""
fi

exec make run_kernel ENABLE_KVM=0 CARGO_OSDK_BUILD_ARGS="${OSDK_ARGS}"
