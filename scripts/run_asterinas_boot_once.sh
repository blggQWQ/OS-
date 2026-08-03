#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCENARIO="${1:?usage: run_asterinas_boot_once.sh plain|dm-verity}"
ARTIFACT_DIR="${ASTERINAS_BOOT_ARTIFACT_DIR:-}"
case "${SCENARIO}" in
    plain) ISO_NAME="asterinas-plain-root.iso" ;;
    dm-verity) ISO_NAME="asterinas-verity-root.iso" ;;
    *) echo "unknown scenario: ${SCENARIO}" >&2; exit 2 ;;
esac

if [[ -n "${ARTIFACT_DIR}" ]]; then
    ISO="${ARTIFACT_DIR}/${ISO_NAME}"
    IMAGE="${ARTIFACT_DIR}/verity-rootfs.img"
    EXT2="${ARTIFACT_DIR}/ext2.img"
    EXFAT="${ARTIFACT_DIR}/exfat.img"
    OVMF="${OVMF:-${ARTIFACT_DIR}/OVMF.fd}"
else
    ISO="${ROOT}/target/benchmark/${ISO_NAME}"
    IMAGE="${ROOT}/target/verity-rootfs.img"
    EXT2="${ROOT}/asterinas/test/initramfs/build/ext2.img"
    EXFAT="${ROOT}/asterinas/test/initramfs/build/exfat.img"
    OVMF="${OVMF:-/usr/share/ovmf/OVMF.fd}"
fi
for file in "${ISO}" "${IMAGE}" "${EXT2}" "${EXFAT}" "${OVMF}"; do
    [[ -f "${file}" ]] || {
        echo "required boot artifact missing: ${file}" >&2
        exit 1
    }
done
[[ -c /dev/kvm ]] || {
    echo "/dev/kvm is unavailable" >&2
    exit 1
}

exec qemu-system-x86_64 \
    -accel kvm -cpu Icelake-Server,+x2apic -smp 1 -m 8G \
    -machine q35,kernel-irqchip=split \
    -no-reboot -nographic -display none \
    -chardev stdio,id=mux,mux=on,signal=off \
    -monitor chardev:mux \
    -device virtio-serial-pci,disable-legacy=on,disable-modern=off \
    -device virtconsole,chardev=mux \
    -serial null \
    -drive "if=none,format=raw,id=x0,file=${EXT2}" \
    -drive "if=none,format=raw,id=x1,file=${EXFAT}" \
    -drive "if=none,format=raw,id=dmtest,file=${IMAGE}" \
    -device virtio-blk-pci,bus=pcie.0,addr=0x6,drive=x0,serial=vext2,disable-legacy=on,disable-modern=off,queue-size=64,num-queues=1,request-merging=off,backend_defaults=off,discard=off,write-zeroes=off,event_idx=off,indirect_desc=off,queue_reset=off \
    -device virtio-blk-pci,bus=pcie.0,addr=0x7,drive=x1,serial=vexfat,disable-legacy=on,disable-modern=off,queue-size=64,num-queues=1,request-merging=off,backend_defaults=off,discard=off,write-zeroes=off,event_idx=off,indirect_desc=off,queue_reset=off \
    -device virtio-blk-pci,bus=pcie.0,addr=0x9,drive=dmtest,serial=vdmtest,disable-legacy=on,disable-modern=off,queue-size=64,num-queues=1,request-merging=off,backend_defaults=off,discard=off,write-zeroes=off,event_idx=off,indirect_desc=off,queue_reset=off \
    -object rng-random,id=rng0,filename=/dev/urandom \
    -device virtio-rng-pci,bus=pcie.0,addr=0x8,disable-legacy=on,disable-modern=off,rng=rng0,event_idx=off,indirect_desc=off,queue_reset=off \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 \
    -bios "${OVMF}" \
    -cdrom "${ISO}"
