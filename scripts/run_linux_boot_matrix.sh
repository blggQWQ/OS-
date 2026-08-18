#!/usr/bin/env bash
# Linux QEMU/KVM plain / dm-crypt / dm-verity protected-root boot timings.
# Timer matches Asterinas: QEMU process start -> first serial PASS marker.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

OUT_DIR="${OUT_DIR:-${ROOT}/docs/evidence/linux-qemu-boot-20260818}"
RUNS="${RUNS:-5}"
TIMEOUT="${TIMEOUT:-180}"
MARKER="${MARKER:-PASS system root switched to protected mapper}"
ARTIFACT_DIR="${LINUX_BOOT_ARTIFACT_DIR:-${ROOT}/target/linux-boot}"
MEM="${MEM:-8G}"
SMP="${SMP:-1}"

mkdir -p "${OUT_DIR}/logs" "$ARTIFACT_DIR"

if [[ ! -f "${ARTIFACT_DIR}/boot.env" || "${REBUILD_LINUX_BOOT:-0}" == "1" ]]; then
  OUT="$ARTIFACT_DIR" bash "${ROOT}/scripts/build_linux_boot_artifacts.sh"
fi
# shellcheck disable=SC1090
source "${ARTIFACT_DIR}/boot.env"

for required in vmlinuz initramfs.cpio.gz plain-rootfs.img crypt-rootfs.img verity-rootfs.img boot.env; do
  [[ -f "${ARTIFACT_DIR}/${required}" ]] || {
    echo "missing Linux boot artifact: ${ARTIFACT_DIR}/${required}" >&2
    exit 1
  }
done
[[ -c /dev/kvm ]] || { echo "/dev/kvm is unavailable" >&2; exit 1; }

{
  echo "git_commit=$(git rev-parse HEAD 2>/dev/null || echo unknown)"
  echo "timestamp_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "platform=linux-qemu-kvm"
  echo "definition=QEMU process start -> first serial line containing marker"
  echo "marker=${MARKER}"
  echo "runs=${RUNS}"
  echo "timeout_s=${TIMEOUT}"
  echo "accelerator=kvm"
  echo "cpu_model=host"
  echo "vcpus=${SMP}"
  echo "memory=${MEM}"
  echo "machine=q35"
  echo "device_model=virtio-blk"
  echo "boot_path=direct-kernel+initramfs"
  echo "guest_kernel=${KVER}"
  echo "rootfs=ext4"
  echo "image_data_mib=${SIZE_MIB}"
  echo "cipher=aes-xts-plain64"
  echo "verity_hash=sha256"
  echo "note=Not Ubuntu systemd/cloud-init. Same protected-root contract as Asterinas: initramfs opens mapper then mounts /."
  echo "qemu_script=scripts/run_linux_boot_once.sh"
} >"${OUT_DIR}/metadata.txt"

python3 scripts/benchmark_boot_time.py \
  --plain-command "LINUX_BOOT_ARTIFACT_DIR=${ARTIFACT_DIR} MEM=${MEM} SMP=${SMP} bash scripts/run_linux_boot_once.sh plain" \
  --crypt-command "LINUX_BOOT_ARTIFACT_DIR=${ARTIFACT_DIR} MEM=${MEM} SMP=${SMP} bash scripts/run_linux_boot_once.sh dm-crypt" \
  --verity-command "LINUX_BOOT_ARTIFACT_DIR=${ARTIFACT_DIR} MEM=${MEM} SMP=${SMP} bash scripts/run_linux_boot_once.sh dm-verity" \
  --marker "${MARKER}" \
  --runs "${RUNS}" \
  --timeout "${TIMEOUT}" \
  --output "${OUT_DIR}/boot-time.csv" \
  --log-dir "${OUT_DIR}/logs"

cp "${ARTIFACT_DIR}/boot.env" "${OUT_DIR}/boot.env"
echo "linux_boot_matrix_out=${OUT_DIR}"
