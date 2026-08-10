#!/bin/bash

# SPDX-License-Identifier: MPL-2.0

set -euo pipefail

DM_IMAGE=${DM_IMAGE:-../target/verity-rootfs.img}
if [ -r "${DM_IMAGE}.env" ]; then
    REQUESTED_DM_IMAGE=${DM_IMAGE}
    # shellcheck disable=SC1090
    . "${DM_IMAGE}.env"
    DM_IMAGE=${REQUESTED_DM_IMAGE}
fi
DM_DEVICE=${DM_DEVICE:-vdc}
DM_NAME=${DM_NAME:-dm-verity0}
DM_ROOT_HASH=${DM_ROOT_HASH:-6db653b69a094065756dc2770d3af8880ec9beed29df41b5bb2fb2e11b2d49f3}
DM_DATA_BLOCKS=${DM_DATA_BLOCKS:-65536}
DM_HASH_START=${DM_HASH_START:-65536}
DM_CACHE_BLOCKS=${DM_CACHE_BLOCKS:-256}
DM_BENCH_RW=${DM_BENCH_RW:-read}
DM_BENCH_BS=${DM_BENCH_BS:-4k}
DM_BENCH_QD=${DM_BENCH_QD:-1}
DM_BENCH_IOENGINE=${DM_BENCH_IOENGINE:-sync}
DM_BENCH_JOBS=${DM_BENCH_JOBS:-1}
DM_BENCH_SIZE=${DM_BENCH_SIZE:-256m}
DM_BENCH_RUNTIME=${DM_BENCH_RUNTIME:-10}
DM_BENCH_RUNS=${DM_BENCH_RUNS:-5}
DM_BENCH_RAMP=${DM_BENCH_RAMP:-2}
DM_PERF_STATS=${DM_PERF_STATS:-1}
RELEASE_LTO=${RELEASE_LTO:-1}
DM_BENCH_METADATA=${DM_BENCH_METADATA:-../target/benchmark/asterinas-dm-verity.metadata.txt}
export CONSOLE=${DM_CONSOLE:-hvc0}

if [ ! -f "${DM_IMAGE}" ]; then
    echo "dm-verity image not found: ${DM_IMAGE}" >&2
    exit 1
fi
if [ "${DM_BENCH_IOENGINE}" = "sync" ] && [ "${DM_BENCH_QD}" != "1" ]; then
    echo "sync ioengine cannot produce queue depth > 1; use posixaio" >&2
    exit 1
fi
if [ "${RELEASE_LTO}" != "1" ]; then
    echo "formal dm benchmark requires RELEASE_LTO=1" >&2
    exit 1
fi
case "${DM_BENCH_SIZE}" in
    *m|*M) DM_BENCH_SIZE_MIB=${DM_BENCH_SIZE%[mM]} ;;
    *)
        echo "DM_BENCH_SIZE must use an MiB suffix, for example 256m" >&2
        exit 1
        ;;
esac
if [ "${DM_DATA_BLOCKS}" -lt $((DM_BENCH_SIZE_MIB * 256)) ]; then
    echo "verity data area is smaller than benchmark region" >&2
    exit 1
fi

mkdir -p "$(dirname "${DM_BENCH_METADATA}")"
GIT_COMMIT=${BENCHMARK_GIT_COMMIT:-$(git rev-parse HEAD)}
GIT_DIRTY=${BENCHMARK_GIT_DIRTY:-$([ -n "$(git status --porcelain --untracked-files=no)" ] && echo 1 || echo 0)}
TIMESTAMP_UTC=$(date -u +%Y-%m-%dT%H:%M:%SZ)
{
    echo "git_commit=${GIT_COMMIT}"
    echo "git_dirty=${GIT_DIRTY}"
    echo "build_profile=release-lto"
    echo "stage_stats=${DM_PERF_STATS}"
    echo "timestamp_utc=${TIMESTAMP_UTC}"
    echo "host=$(uname -srmo)"
    echo "vcpus=${SMP:-1}"
    echo "memory=${MEM:-8G}"
    echo "accelerator=$([ "${ENABLE_KVM:-0}" = "1" ] && echo kvm || echo tcg)"
    echo "cpu_model=$([ "${ENABLE_KVM:-0}" = "1" ] && echo host || echo default)"
    echo "device_model=virtio-blk"
    echo "image_size_bytes=$(stat -c %s "${DM_IMAGE}")"
    echo "image_region_sha256=$(dd if="${DM_IMAGE}" bs=1M count="${DM_BENCH_SIZE_MIB}" status=none | sha256sum | awk '{print $1}')"
    echo "target=dm-verity"
    echo "data_blocks=${DM_DATA_BLOCKS}"
    echo "hash_start=${DM_HASH_START}"
    echo "root_hash=${DM_ROOT_HASH}"
    echo "rw=${DM_BENCH_RW}"
    echo "block_size=${DM_BENCH_BS}"
    echo "ioengine=${DM_BENCH_IOENGINE}"
    echo "direct=1"
    echo "queue_depth=${DM_BENCH_QD}"
    echo "jobs=${DM_BENCH_JOBS}"
    echo "size_mib=${DM_BENCH_SIZE_MIB}"
    echo "ramp_seconds=${DM_BENCH_RAMP}"
    echo "runtime_seconds=${DM_BENCH_RUNTIME}"
    echo "runs=${DM_BENCH_RUNS}"
    echo "cache_blocks=${DM_CACHE_BLOCKS}"
} >"${DM_BENCH_METADATA}"

echo "[dm-bench-meta] commit=${GIT_COMMIT} dirty=${GIT_DIRTY} profile=release-lto stage_stats=${DM_PERF_STATS} timestamp_utc=${TIMESTAMP_UTC}"
echo "[dm-bench-meta] accelerator=$([ "${ENABLE_KVM:-0}" = "1" ] && echo kvm || echo tcg) cpu_model=$([ "${ENABLE_KVM:-0}" = "1" ] && echo host || echo default) vcpus=${SMP:-1} memory=${MEM:-8G} device_model=virtio-blk"
echo "[dm-bench-meta] rw=${DM_BENCH_RW} block_size=${DM_BENCH_BS} ioengine=${DM_BENCH_IOENGINE} queue_depth=${DM_BENCH_QD} jobs=${DM_BENCH_JOBS} size=${DM_BENCH_SIZE} runtime=${DM_BENCH_RUNTIME} ramp=${DM_BENCH_RAMP} runs=${DM_BENCH_RUNS}"

KCMD_ARGS="console=${CONSOLE} \
rootfs_verity.scheme=dm-verity \
rootfs_verity.device=${DM_DEVICE} \
rootfs_verity.name=${DM_NAME} \
rootfs_verity.hash=${DM_ROOT_HASH} \
rootfs_verity.data_blocks=${DM_DATA_BLOCKS} \
rootfs_verity.hash_start=${DM_HASH_START} \
rootfs_verity.cache_blocks=${DM_CACHE_BLOCKS} \
dm.perf_stats=${DM_PERF_STATS}"

export DM_IMAGE
OSDK_ARGS="--kcmd-args=\"${KCMD_ARGS}\" --init-args=\"dm-bench /dev/${DM_DEVICE} /dev/${DM_NAME} ${DM_BENCH_RW} ${DM_BENCH_BS} ${DM_BENCH_QD} ${DM_BENCH_SIZE} ${DM_BENCH_RUNTIME} ${DM_BENCH_RUNS} ${DM_BENCH_RAMP} ${DM_BENCH_IOENGINE} ${DM_BENCH_JOBS} /bin/true\""
if [ "${ENABLE_KVM:-0}" = "1" ]; then
    OSDK_ARGS="${OSDK_ARGS} --qemu-args=\"-accel kvm -cpu host\""
fi

exec make run_kernel RELEASE_LTO=1 ENABLE_KVM=0 BENCHMARK=dm-verity EXTRA_CARGO_OSDK_BUILD_ARGS="${OSDK_ARGS}"
