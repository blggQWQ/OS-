#!/bin/bash

# SPDX-License-Identifier: MPL-2.0

# Run a five-round raw-vs-dm-crypt fio matrix in the Asterinas KVM guest.
# Keep the I/O contract aligned with run_dm_verity_bench.sh: 4 KiB, QD1,
# direct reads, five rounds.  Capture stdout to a serial log, then summarize
# it with scripts/summarize_asterinas_dm_crypt_bench.py.
set -euo pipefail

if [ "${DM_AES_NI:-1}" = "1" ]; then
    export OSDK_ENABLE_AES_NI=1
else
    unset OSDK_ENABLE_AES_NI
fi

DM_IMAGE=${DM_IMAGE:-./dm-crypt-test.img}
DM_DEVICE=${DM_DEVICE:-vdc}
DM_NAME=${DM_NAME:-dm-crypt0}
DM_CRYPT_SCRATCH_POOL=${DM_CRYPT_SCRATCH_POOL:-32}
DM_BENCH_RW=${DM_BENCH_RW:-read}
DM_BENCH_BS=${DM_BENCH_BS:-4k}
DM_BENCH_QD=${DM_BENCH_QD:-1}
DM_BENCH_SIZE=${DM_BENCH_SIZE:-8m}
DM_BENCH_RUNTIME=${DM_BENCH_RUNTIME:-2}
DM_BENCH_RUNS=${DM_BENCH_RUNS:-5}
DM_BENCH_RAMP=${DM_BENCH_RAMP:-1}
DM_BENCH_METADATA=${DM_BENCH_METADATA:-../target/benchmark/asterinas-dm-crypt.metadata.txt}
DM_KEY=${DM_KEY:-000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f}
export CONSOLE=${DM_CONSOLE:-hvc0}

if [ ! -f "${DM_IMAGE}" ]; then
    echo "dm-crypt image not found: ${DM_IMAGE}" >&2
    exit 1
fi
case "${DM_BENCH_SIZE}" in
    *m|*M) DM_BENCH_SIZE_MIB=${DM_BENCH_SIZE%[mM]} ;;
    *)
        echo "DM_BENCH_SIZE must use an MiB suffix, for example 8m" >&2
        exit 1
        ;;
esac

mkdir -p "$(dirname "${DM_BENCH_METADATA}")"
{
    echo "host=$(uname -srmo)"
    echo "vcpus=${SMP:-1}"
    echo "memory=${MEM:-8G}"
    echo "accelerator=$([ "${ENABLE_KVM:-0}" = "1" ] && echo kvm || echo tcg)"
    echo "cpu_model=$([ "${ENABLE_KVM:-0}" = "1" ] && echo host || echo default)"
    echo "device_model=virtio-blk"
    echo "image_region_sha256=$(dd if="${DM_IMAGE}" bs=1M count="${DM_BENCH_SIZE_MIB}" status=none | sha256sum | awk '{print $1}')"
    echo "cipher=aes-xts-plain64"
    echo "rw=${DM_BENCH_RW}"
    echo "block_size=${DM_BENCH_BS}"
    echo "ioengine=sync"
    echo "direct=1"
    echo "queue_depth=${DM_BENCH_QD}"
    echo "size_mib=${DM_BENCH_SIZE_MIB}"
    echo "ramp_seconds=${DM_BENCH_RAMP}"
    echo "runtime_seconds=${DM_BENCH_RUNTIME}"
    echo "runs=${DM_BENCH_RUNS}"
    echo "scratch_pool=${DM_CRYPT_SCRATCH_POOL}"
} >"${DM_BENCH_METADATA}"

KCMD_ARGS="console=${CONSOLE} \
dm_crypt.device=${DM_DEVICE} \
dm_crypt.name=${DM_NAME} \
dm_crypt.key=${DM_KEY} \
dm_crypt.scratch_pool=${DM_CRYPT_SCRATCH_POOL}"
export DM_IMAGE
OSDK_ARGS="--kcmd-args=\"${KCMD_ARGS}\" --init-args=\"dm-bench /dev/${DM_DEVICE} /dev/${DM_NAME} ${DM_BENCH_RW} ${DM_BENCH_BS} ${DM_BENCH_QD} ${DM_BENCH_SIZE} ${DM_BENCH_RUNTIME} ${DM_BENCH_RUNS} ${DM_BENCH_RAMP} /bin/true\""
if [ "${ENABLE_KVM:-0}" = "1" ]; then
    OSDK_ARGS="${OSDK_ARGS} --qemu-args=\"-accel kvm -cpu host\""
fi

exec make run_kernel ENABLE_KVM=0 BENCHMARK=dm-verity CARGO_OSDK_BUILD_ARGS="${OSDK_ARGS}"
