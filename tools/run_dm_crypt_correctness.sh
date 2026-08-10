#!/bin/bash
set -euo pipefail

export OSDK_ENABLE_AES_NI=1
DM_IMAGE=${DM_IMAGE:-./dm-crypt-test.img}
PERSIST_EXPECT=${PERSIST_EXPECT:-0}
TEST_COMMIT=${TEST_COMMIT:-unknown}
export CONSOLE=${DM_CONSOLE:-hvc0}
test -f "${DM_IMAGE}"
export DM_IMAGE
OSDK_ARGS="--kcmd-args=\"console=${CONSOLE}\" --init-args=\"crypt-correctness /dev/vdc ${PERSIST_EXPECT} ${TEST_COMMIT} /bin/true\""
if [ "${ENABLE_KVM:-0}" = "1" ]; then
    OSDK_ARGS="${OSDK_ARGS} --qemu-args=\"-accel kvm -cpu host\""
fi
exec make run_kernel ENABLE_KVM=0 CARGO_OSDK_BUILD_ARGS="${OSDK_ARGS}"
