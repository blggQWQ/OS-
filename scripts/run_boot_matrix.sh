#!/usr/bin/env bash
# plain / dm-crypt / dm-verity × >=5 boot timings under a unified PASS marker.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

OUT_DIR="${OUT_DIR:-${ROOT}/docs/evidence/boot-matrix}"
RUNS="${RUNS:-5}"
TIMEOUT="${TIMEOUT:-300}"
MARKER="${MARKER:-PASS system root switched to protected mapper}"
INCLUDE_CRYPT="${INCLUDE_CRYPT:-1}"

mkdir -p "${OUT_DIR}/logs" target/benchmark

BOOT_STAGE="${TMPDIR:-/tmp}/cscc-asterinas-boot-benchmark"
mkdir -p "${BOOT_STAGE}"
SOURCE_TARGET_DIR="${ASTERINAS_TARGET_DIR:-${ROOT}/target}"

copy_if_present() {
  local src="$1" dst="$2"
  if [[ -f "$src" ]]; then
    cp "$src" "$dst"
  fi
}

copy_if_present "${SOURCE_TARGET_DIR}/benchmark/asterinas-plain-root.iso" "${BOOT_STAGE}/"
copy_if_present "${SOURCE_TARGET_DIR}/benchmark/asterinas-verity-root.iso" "${BOOT_STAGE}/"
copy_if_present "${SOURCE_TARGET_DIR}/benchmark/asterinas-crypt-root.iso" "${BOOT_STAGE}/"
copy_if_present "${SOURCE_TARGET_DIR}/verity-rootfs.img" "${BOOT_STAGE}/"
copy_if_present "${SOURCE_TARGET_DIR}/crypt-rootfs.img" "${BOOT_STAGE}/"
copy_if_present asterinas/test/initramfs/build/ext2.img "${BOOT_STAGE}/"
copy_if_present asterinas/test/initramfs/build/exfat.img "${BOOT_STAGE}/"
copy_if_present "${SOURCE_TARGET_DIR}/benchmark/ext2.img" "${BOOT_STAGE}/"
copy_if_present "${SOURCE_TARGET_DIR}/benchmark/exfat.img" "${BOOT_STAGE}/"
copy_if_present /usr/share/ovmf/OVMF.fd "${BOOT_STAGE}/OVMF.fd"
export ASTERINAS_BOOT_ARTIFACT_DIR="${BOOT_STAGE}"

for required in asterinas-plain-root.iso asterinas-verity-root.iso verity-rootfs.img ext2.img exfat.img OVMF.fd; do
  [[ -f "${BOOT_STAGE}/${required}" ]] || {
    echo "missing boot artifact: ${BOOT_STAGE}/${required}" >&2
    echo "Build images (container): bash scripts/build_asterinas_boot_images.sh" >&2
    exit 1
  }
done

CRYPT_ARGS=()
if [[ "${INCLUDE_CRYPT}" == "1" ]]; then
  if [[ -f "${BOOT_STAGE}/asterinas-crypt-root.iso" && -f "${BOOT_STAGE}/crypt-rootfs.img" ]]; then
    CRYPT_ARGS+=(--crypt-command "bash scripts/run_asterinas_boot_once.sh dm-crypt")
  else
    echo "WARN: crypt ISO/image missing; running plain+verity only" >&2
  fi
fi

{
  echo "git_commit=$(git rev-parse HEAD 2>/dev/null || echo unknown)"
  echo "timestamp_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "marker=${MARKER}"
  echo "runs=${RUNS}"
  echo "qemu_script=scripts/run_asterinas_boot_once.sh"
  echo "definition=QEMU process start -> first serial line containing marker"
} >"${OUT_DIR}/metadata.txt"

python3 scripts/benchmark_boot_time.py \
  --plain-command "bash scripts/run_asterinas_boot_once.sh plain" \
  --verity-command "bash scripts/run_asterinas_boot_once.sh dm-verity" \
  "${CRYPT_ARGS[@]}" \
  --marker "${MARKER}" \
  --runs "${RUNS}" \
  --timeout "${TIMEOUT}" \
  --output "${OUT_DIR}/boot-time.csv" \
  --log-dir "${OUT_DIR}/logs"

echo "boot_matrix_out=${OUT_DIR}"
