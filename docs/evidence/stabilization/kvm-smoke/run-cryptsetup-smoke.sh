#!/bin/bash
set -euo pipefail

export DM_IMAGE=/root/target/dm-crypt-test.img
OSDK_ARGS='--kcmd-args="console=hvc0" --init-args="cryptsetup-smoke /dev/vdc /.initramfs/bin/true" --qemu-args="-accel kvm -cpu host"'
echo 'COMMAND: make run_kernel ENABLE_KVM=0 CARGO_OSDK_BUILD_ARGS=<OSDK_ARGS>'
make run_kernel ENABLE_KVM=0 CARGO_OSDK_BUILD_ARGS="${OSDK_ARGS}"
