#!/usr/bin/env bash
# Build Linux QEMU protected-root boot artifacts: kernel, initramfs, and
# plain / dm-crypt / dm-verity ext4 images.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${OUT:-${ROOT}/target/linux-boot}"
SIZE_MIB="${SIZE_MIB:-16}"
BLOCK_SIZE=4096
DATA_BLOCKS=$((SIZE_MIB * 1024 * 1024 / BLOCK_SIZE))
HASH_OFFSET=$((SIZE_MIB * 1024 * 1024))
HASH_EXTRA_MIB="${HASH_EXTRA_MIB:-8}"
KEY_HEX="${DM_KEY:-000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f}"

# Auto-detect kernel: prefer explicit KVER, else newest installed in /boot/, fallback to uname -r
if [ -z "${KVER:-}" ]; then
  # Pick the newest vmlinuz that has matching modules
  KVER=""
  for vmlinuz in $(ls -t /boot/vmlinuz-* 2>/dev/null); do
    cand="${vmlinuz#/boot/vmlinuz-}"
    if [ -d "/lib/modules/$cand" ] && [ -r "/lib/modules/$cand/modules.dep" ]; then
      KVER="$cand"
      break
    fi
  done
  # Fallback to running kernel if no installed kernel found
  if [ -z "$KVER" ]; then
    KVER="$(uname -r)"
  fi
fi

echo "[linux-boot] Using kernel version: $KVER"
VMLINUZ="/boot/vmlinuz-${KVER}"

# Verify we have a kernel image
if [ ! -f "$VMLINUZ" ]; then
  echo "ERROR: Cannot find kernel image at $VMLINUZ" >&2
  echo "Install a kernel package first, e.g.: sudo apt install -y linux-image-virtual" >&2
  exit 1
fi
if [ ! -d "/lib/modules/${KVER}" ]; then
  echo "ERROR: Missing modules directory /lib/modules/${KVER}" >&2
  exit 1
fi

for cmd in qemu-system-x86_64 dmsetup veritysetup mke2fs zstd python3 cpio gzip; do
  command -v "$cmd" >/dev/null || { echo "missing command: $cmd" >&2; exit 1; }
done

mkdir -p "$OUT"
STAGE="${OUT}/staging"
INITRAMFS_DIR="${OUT}/initramfs"
rm -rf "$STAGE" "$INITRAMFS_DIR"
mkdir -p "$STAGE/etc" "$INITRAMFS_DIR"

copy_with_deps() {
  python3 - "$1" "$INITRAMFS_DIR" <<'PY'
import shutil, subprocess, sys
from pathlib import Path

src = Path(sys.argv[1])
root = Path(sys.argv[2])
seen = set()

def add(named):
    named = Path(named)
    if not named.exists():
        return
    key = str(named)
    if key in seen:
        return
    seen.add(key)
    dest = root / str(named).lstrip("/")
    dest.parent.mkdir(parents=True, exist_ok=True)
    real = named.resolve()
    if not dest.exists() and not dest.is_symlink():
        shutil.copy2(real, dest)
    try:
        out = subprocess.check_output(["ldd", str(real)], text=True, errors="replace")
    except subprocess.CalledProcessError:
        return
    for line in out.splitlines():
        line = line.strip()
        if "=>" in line:
            rhs = line.split("=>", 1)[1].strip().split()
            if rhs and rhs[0].startswith("/"):
                add(Path(rhs[0]))
        elif line.startswith("/") and ".so" in line:
            add(Path(line.split()[0]))

add(src)
PY
}

# Copy a kernel module (decompress if .zst) into initramfs
copy_module() {
  local name="$1"
  local mod_dir="${INITRAMFS_DIR}/lib/modules/${KVER}"
  mkdir -p "$mod_dir"
  local src=""
  # Search target kernel modules directly by name
  for cand in $(find "/lib/modules/${KVER}" -name "${name}.ko" -o -name "${name}.ko.zst" 2>/dev/null | head -5); do
    if [ -f "$cand" ]; then
      src="$cand"
      break
    fi
  done
  # Fallback: use modprobe with explicit kernel version
  if [ -z "$src" ] || [ ! -f "$src" ]; then
    src="$(modprobe --set-version="$KVER" -D "$name" 2>/dev/null | awk '/insmod/{print $2}' | tail -n1 || true)"
  fi
  # Check if it's built-in
  if [ -z "$src" ] || [ ! -f "$src" ]; then
    if grep -q "/${name}\.ko" "/lib/modules/${KVER}/modules.builtin" 2>/dev/null; then
      echo "  (builtin: $name)"
      return 0
    fi
    echo "  (skip $name: module not found)"
    return 0
  fi
  local base
  base="$(basename "$src")"
  local dest="${mod_dir}/${base%.zst}"
  if [[ "$src" == *.zst ]]; then
    zstd -d -q -f -o "$dest" "$src"
  else
    cp -a "$src" "$dest"
  fi
  echo "  copied: $name <- $src"
}

echo "[linux-boot] Copying kernel ${KVER}"
cp "$VMLINUZ" "${OUT}/vmlinuz"
chmod a+r "${OUT}/vmlinuz"

echo "[linux-boot] Packing initramfs tools"
mkdir -p "${INITRAMFS_DIR}/bin" "${INITRAMFS_DIR}/sbin" \
  "${INITRAMFS_DIR}/usr/bin" "${INITRAMFS_DIR}/usr/sbin" \
  "${INITRAMFS_DIR}/usr/lib/x86_64-linux-gnu" "${INITRAMFS_DIR}/usr/lib64" \
  "${INITRAMFS_DIR}/lib/modules/${KVER}"
ln -sfn usr/lib "${INITRAMFS_DIR}/lib"
ln -sfn usr/lib64 "${INITRAMFS_DIR}/lib64"
copy_with_deps /usr/bin/busybox
copy_with_deps /usr/sbin/dmsetup
copy_with_deps /usr/sbin/veritysetup
copy_with_deps /lib64/ld-linux-x86-64.so.2
ln -sfr "${INITRAMFS_DIR}/usr/bin/busybox" "${INITRAMFS_DIR}/bin/busybox"
ln -sfr "${INITRAMFS_DIR}/usr/sbin/dmsetup" "${INITRAMFS_DIR}/sbin/dmsetup"
ln -sfr "${INITRAMFS_DIR}/usr/sbin/veritysetup" "${INITRAMFS_DIR}/sbin/veritysetup"

# Copy required kernel modules
echo "[linux-boot] Copying kernel modules..."
MODULES=()
for mod in ext4 virtio virtio_ring virtio_pci virtio_blk virtio_pci_modern_dev virtio_pci_legacy_dev dm_mod dm_bufio dm_crypt dm_verity aesni_intel aes_x86_64 crypto_simd cryptd glue_helper xhci_pci xhci_hcd uhci_hcd ehci_hcd ohci_hcd usb_storage sd_mod sg; do
  packed="$(copy_module "$mod")"
done

# Copy modules.dep and related files for modprobe to work (optional, we insmod directly)
for f in modules.dep modules.builtin modules.alias; do
  if [ -f "/lib/modules/${KVER}/${f}" ]; then
    cp "/lib/modules/${KVER}/${f}" "${INITRAMFS_DIR}/lib/modules/${KVER}/"
  fi
done

{
  cat <<'INITEOF'
#!/bin/busybox sh
set -eu
/bin/busybox mkdir -p /proc /sys /dev /newroot /run /tmp /mapper
/bin/busybox mount -t proc proc /proc
/bin/busybox mount -t sysfs sys /sys
/bin/busybox mount -t devtmpfs dev /dev 2>/dev/null || true
export DM_DISABLE_UDEV=1
export PATH=/bin:/sbin:/usr/bin:/usr/sbin

# Load modules - try both .ko and .ko.zst names, wildcard kernel version
load_mod() {
  for path in /lib/modules/*/"$1".ko /lib/modules/*/"$1".ko.zst; do
    [ -f "$path" ] || continue
    if [ "${path%.zst}" != "$path" ]; then
      # compressed: extract to tmp and load
      tmp="/tmp/$(basename "$path" .zst)"
      zstd -d -q -f -o "$tmp" "$path" 2>/dev/null || continue
      /bin/busybox insmod "$tmp" 2>/dev/null || true
      rm -f "$tmp"
    else
      /bin/busybox insmod "$path" 2>/dev/null || true
    fi
  done
}

# Load necessary modules in dependency order
for m in virtio virtio_ring virtio_pci_modern_dev virtio_pci_legacy_dev virtio_pci virtio_blk; do
  load_mod "$m"
done
for m in dm_mod dm_bufio dm_crypt dm_verity; do
  load_mod "$m"
done
# ext4 deps: mbcache, jbd2, ext4
for m in mbcache jbd2 ext4; do
  load_mod "$m"
done
# If ext4 is built-in, insmod will fail silently - that's fine

# Wait for virtio disk to appear (up to 10 seconds)
SCENARIO=plain
KEY=
ROOT_HASH=
DATA_BLOCKS=
HASH_OFFSET=
DISK=""
for tok in $(cat /proc/cmdline); do
  case "$tok" in
    dm_boot=*) SCENARIO=${tok#dm_boot=} ;;
    dm_key=*) KEY=${tok#dm_key=} ;;
    dm_root_hash=*) ROOT_HASH=${tok#dm_root_hash=} ;;
    dm_data_blocks=*) DATA_BLOCKS=${tok#dm_data_blocks=} ;;
    dm_hash_offset=*) HASH_OFFSET=${tok#dm_hash_offset=} ;;
  esac
done

for _ in 1 2 3 4 5 6 7 8 9 10; do
  for cand in /dev/vda /dev/vdb /dev/vd0; do
    if [ -b "$cand" ]; then
      DISK=$cand
      break
    fi
  done
  [ -n "$DISK" ] && break
  /bin/busybox sleep 1
done
if [ -z "$DISK" ]; then
  echo "[linux-boot] FAIL no virtio disk"
  exec /bin/busybox sh
fi

echo "[linux-boot] scenario=${SCENARIO} disk=${DISK}"
TARGET=$DISK
case "$SCENARIO" in
  plain)
    TARGET=$DISK
    ;;
  dm-crypt)
    SECTORS=$(cat /sys/block/${DISK#/dev/}/size 2>/dev/null || echo 0)
    if [ "$SECTORS" = "0" ]; then
      echo "[linux-boot] FAIL cannot get disk size"
      exec /bin/busybox sh
    fi
    echo "0 ${SECTORS} crypt aes-xts-plain64 ${KEY} 0 ${DISK} 0" | /usr/sbin/dmsetup create dm-crypt0
    /usr/sbin/dmsetup mknodes dm-crypt0 || true
    TARGET=/dev/mapper/dm-crypt0
    ;;
  dm-verity)
    /usr/sbin/veritysetup open "$DISK" dm-verity0 "$DISK" "$ROOT_HASH" \
      --data-blocks="$DATA_BLOCKS" --hash-offset="$HASH_OFFSET"
    /usr/sbin/dmsetup mknodes dm-verity0 || true
    TARGET=/dev/mapper/dm-verity0
    ;;
  *)
    echo "[linux-boot] FAIL unknown scenario ${SCENARIO}"
    exec /bin/busybox sh
    ;;
esac

if ! /bin/busybox mount -t ext4 -o ro "$TARGET" /newroot; then
  echo "[linux-boot] FAIL mount ${TARGET}"
  exec /bin/busybox sh
fi
if [ ! -f /newroot/etc/dm-boot-proof ]; then
  echo "[linux-boot] FAIL missing proof file"
  exec /bin/busybox sh
fi
echo "[linux-boot] proof=$(cat /newroot/etc/dm-boot-proof)"
echo "PASS system root switched to protected mapper"
/bin/busybox sleep 3
echo o > /proc/sysrq-trigger 2>/dev/null || true
/bin/busybox poweroff -f 2>/dev/null || true
exec /bin/busybox sh
INITEOF
} > "${INITRAMFS_DIR}/init"
chmod +x "${INITRAMFS_DIR}/init"

echo "[linux-boot] Packing initramfs.cpio.gz"
(
  cd "$INITRAMFS_DIR"
  find . -print0 | cpio --null -o -H newc 2>/dev/null | gzip -9 > "${OUT}/initramfs.cpio.gz"
)

printf 'LINUX_DM_BOOT_OK\n' > "$STAGE/etc/dm-boot-proof"
printf 'Linux QEMU protected-root boot image\n' > "$STAGE/README"

echo "[linux-boot] Formatting plain ext4 ${SIZE_MIB}MiB"
PLAIN_IMG="${OUT}/plain-rootfs.img"
truncate -s "$((SIZE_MIB * 1024 * 1024))" "$PLAIN_IMG"
mke2fs -q -F -t ext4 -b "$BLOCK_SIZE" -d "$STAGE" "$PLAIN_IMG"

echo "[linux-boot] Formatting dm-crypt ext4 ${SIZE_MIB}MiB"
CRYPT_IMG="${OUT}/crypt-rootfs.img"
truncate -s "$((SIZE_MIB * 1024 * 1024))" "$CRYPT_IMG"
MAPPER="linux_boot_crypt_$$"
LOOP_DEV="$(losetup --find --show "$CRYPT_IMG")"
cleanup_crypt() {
  set +e
  sync
  sleep 1
  dmsetup remove --force "$MAPPER" >/dev/null 2>&1 || true
  dmsetup remove "$MAPPER" >/dev/null 2>&1 || true
  losetup -d "$LOOP_DEV" >/dev/null 2>&1 || true
  losetup -D >/dev/null 2>&1 || true
}
trap cleanup_crypt EXIT
SECTORS=$((SIZE_MIB * 1024 * 1024 / 512))
echo "0 ${SECTORS} crypt aes-xts-plain64 ${KEY_HEX} 0 ${LOOP_DEV} 0" | dmsetup create "$MAPPER"
dmsetup mknodes "$MAPPER"
# Run mke2fs without udev synchronization to avoid automount interference
mke2fs -q -F -t ext4 -b "$BLOCK_SIZE" -E no_udev -d "$STAGE" "/dev/mapper/${MAPPER}" 2>/dev/null || \
  mke2fs -q -F -t ext4 -b "$BLOCK_SIZE" -d "$STAGE" "/dev/mapper/${MAPPER}"
sync
sleep 1
# Force removal, ignore errors (cleanup trap will retry)
dmsetup remove --force "$MAPPER" >/dev/null 2>&1 || true
sleep 1
losetup -d "$LOOP_DEV" >/dev/null 2>&1 || true
trap - EXIT

echo "[linux-boot] Formatting dm-verity ext4 ${SIZE_MIB}MiB + hash"
VERITY_IMG="${OUT}/verity-rootfs.img"
truncate -s "$((SIZE_MIB * 1024 * 1024))" "$VERITY_IMG"
mke2fs -q -F -t ext4 -b "$BLOCK_SIZE" -d "$STAGE" "$VERITY_IMG"
truncate -s "$(( (SIZE_MIB + HASH_EXTRA_MIB) * 1024 * 1024 ))" "$VERITY_IMG"
FORMAT_OUT="$(veritysetup --data-block-size="$BLOCK_SIZE" --hash-block-size="$BLOCK_SIZE" \
  --data-blocks="$DATA_BLOCKS" --hash-offset="$HASH_OFFSET" --no-superblock \
  format "$VERITY_IMG" "$VERITY_IMG")"
printf '%s\n' "$FORMAT_OUT"
ROOT_HASH="$(printf '%s\n' "$FORMAT_OUT" | awk -F: '/Root hash/{gsub(/[ \t]/,"",$2); print $2}')"
if [ -z "$ROOT_HASH" ]; then
  # veritysetup 2.x outputs "Root hash:" differently - try alternative parsing
  ROOT_HASH="$(printf '%s\n' "$FORMAT_OUT" | grep -i "root hash" | awk '{print $NF}' | tr -d ' \t\r')"
fi
[[ -n "$ROOT_HASH" ]] || { echo "failed to parse verity root hash" >&2; echo "$FORMAT_OUT" >&2; exit 1; }

cat > "${OUT}/boot.env" <<EOF
KVER=$KVER
SIZE_MIB=$SIZE_MIB
BLOCK_SIZE=$BLOCK_SIZE
DM_KEY=$KEY_HEX
DM_DATA_BLOCKS=$DATA_BLOCKS
DM_HASH_OFFSET=$HASH_OFFSET
DM_ROOT_HASH=$ROOT_HASH
PLAIN_IMAGE=$PLAIN_IMG
CRYPT_IMAGE=$CRYPT_IMG
VERITY_IMAGE=$VERITY_IMG
KERNEL=${OUT}/vmlinuz
INITRD=${OUT}/initramfs.cpio.gz
EOF

echo ""
echo "=== Build complete ==="
echo "artifacts=$OUT"
echo "kernel=$KVER"
echo "root_hash=$ROOT_HASH"
echo "env=${OUT}/boot.env"
