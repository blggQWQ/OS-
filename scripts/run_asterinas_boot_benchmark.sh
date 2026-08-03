#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"
BOOT_STAGE="${TMPDIR:-/tmp}/cscc-asterinas-boot-benchmark"
mkdir -p "${BOOT_STAGE}"
cp target/benchmark/asterinas-plain-root.iso "${BOOT_STAGE}/"
cp target/benchmark/asterinas-verity-root.iso "${BOOT_STAGE}/"
cp target/verity-rootfs.img "${BOOT_STAGE}/"
cp asterinas/test/initramfs/build/ext2.img "${BOOT_STAGE}/"
cp asterinas/test/initramfs/build/exfat.img "${BOOT_STAGE}/"
cp /usr/share/ovmf/OVMF.fd "${BOOT_STAGE}/OVMF.fd"
export ASTERINAS_BOOT_ARTIFACT_DIR="${BOOT_STAGE}"

python3 scripts/benchmark_boot_time.py \
    --plain-command "bash scripts/run_asterinas_boot_once.sh plain" \
    --protected-command "bash scripts/run_asterinas_boot_once.sh dm-verity" \
    --plain-marker "PASS system root switched to protected mapper" \
    --protected-marker "PASS system root switched to protected mapper" \
    --runs 5 \
    --timeout 300 \
    --output target/benchmark/asterinas-boot-time.csv \
    --log-dir target/benchmark/asterinas-boot-time-logs
