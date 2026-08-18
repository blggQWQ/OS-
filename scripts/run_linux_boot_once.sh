#!/usr/bin/env bash
# One Linux QEMU/KVM protected-root boot. Prints the shared PASS marker on serial.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCENARIO="${1:?usage: run_linux_boot_once.sh plain|dm-crypt|dm-verity}"
ARTIFACT_DIR="${LINUX_BOOT_ARTIFACT_DIR:-${ROOT}/target/linux-boot}"
ENV_FILE="${ARTIFACT_DIR}/boot.env"
MEM="${MEM:-8G}"
SMP="${SMP:-1}"

[[ -f "$ENV_FILE" ]] || {
  echo "missing ${ENV_FILE}; run: bash scripts/build_linux_boot_artifacts.sh" >&2
  exit 1
}
# shellcheck disable=SC1090
set -a
# shellcheck source=/dev/null
source "$ENV_FILE"
set +a

KERNEL="${KERNEL:-${ARTIFACT_DIR}/vmlinuz}"
INITRD="${INITRD:-${ARTIFACT_DIR}/initramfs.cpio.gz}"
case "$SCENARIO" in
  plain) IMAGE="${PLAIN_IMAGE}" ;;
  dm-crypt) IMAGE="${CRYPT_IMAGE}" ;;
  dm-verity) IMAGE="${VERITY_IMAGE}" ;;
  *) echo "unknown scenario: ${SCENARIO}" >&2; exit 2 ;;
esac

for file in "$KERNEL" "$INITRD" "$IMAGE"; do
  [[ -f "$file" ]] || { echo "missing boot artifact: $file" >&2; exit 1; }
done
[[ -c /dev/kvm ]] || { echo "/dev/kvm is unavailable" >&2; exit 1; }

APPEND="console=ttyS0,115200n8 ignore_loglevel dm_boot=${SCENARIO}"
if [[ "$SCENARIO" == "dm-crypt" ]]; then
  APPEND="${APPEND} dm_key=${DM_KEY}"
fi
if [[ "$SCENARIO" == "dm-verity" ]]; then
  APPEND="${APPEND} dm_root_hash=${DM_ROOT_HASH} dm_data_blocks=${DM_DATA_BLOCKS} dm_hash_offset=${DM_HASH_OFFSET}"
fi

exec qemu-system-x86_64 \
  -accel kvm -cpu host -smp "$SMP" -m "$MEM" \
  -machine q35 \
  -no-reboot -nographic -display none \
  -monitor none \
  -kernel "$KERNEL" \
  -initrd "$INITRD" \
  -append "$APPEND" \
  -drive "if=none,format=raw,id=root,file=${IMAGE}" \
  -device virtio-blk-pci,drive=root,serial=linuxroot
