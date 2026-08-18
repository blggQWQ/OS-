#!/usr/bin/env bash
# Build Linux QEMU protected-root boot artifacts: kernel, initramfs, and
# plain / dm-crypt / dm-verity ext4 images. Requires sudo for the host
# vmlinuz copy and for dmsetup when formatting the crypt image.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${OUT:-${ROOT}/target/linux-boot}"
SIZE_MIB="${SIZE_MIB:-16}"
KVER="${KVER:-$(uname -r)}"
KEY_HEX="${DM_KEY:-000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f}"
BLOCK_SIZE=4096
DATA_BLOCKS=$((SIZE_MIB * 1024 * 1024 / BLOCK_SIZE))
HASH_OFFSET=$((SIZE_MIB * 1024 * 1024))
HASH_EXTRA_MIB="${HASH_EXTRA_MIB:-8}"

for cmd in qemu-system-x86_64 dmsetup veritysetup mke2fs zstd python3 cpio gzip; do
  command -v "$cmd" >/dev/null || { echo "missing command: $cmd" >&2; exit 1; }
done
[[ -r "/lib/modules/${KVER}" ]] || { echo "missing modules for ${KVER}" >&2; exit 1; }

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
seen: set[str] = set()


def add(named: Path) -> None:
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

copy_module() {
  local name="$1"
  local src
  src="$(modprobe -D "$name" 2>/dev/null | awk '/insmod/{print $2}' | tail -n1 || true)"
  [[ -n "$src" && -f "$src" ]] || return 0
  local dest_dir="${INITRAMFS_DIR}/lib/modules/${KVER}"
  mkdir -p "$dest_dir"
  local base
  base="$(basename "$src")"
  base="${base%.zst}"
  [[ "$base" == *.ko ]] || base="${base}.ko"
  local dest="${dest_dir}/${base}"
  if [[ "$src" == *.zst ]]; then
    zstd -d -q -f -o "$dest" "$src"
  else
    cp -a "$src" "$dest"
  fi
  printf '%s\n' "$(basename "$dest")"
}

echo "[linux-boot] copying kernel ${KVER}"
sudo cp "/boot/vmlinuz-${KVER}" "${OUT}/vmlinuz"
sudo chmod a+r "${OUT}/vmlinuz"

echo "[linux-boot] packing initramfs tools"
mkdir -p "${INITRAMFS_DIR}/bin" "${INITRAMFS_DIR}/sbin" \
  "${INITRAMFS_DIR}/usr/bin" "${INITRAMFS_DIR}/usr/sbin" \
  "${INITRAMFS_DIR}/usr/lib/x86_64-linux-gnu" "${INITRAMFS_DIR}/usr/lib64"
ln -sfn usr/lib "${INITRAMFS_DIR}/lib"
ln -sfn usr/lib64 "${INITRAMFS_DIR}/lib64"
copy_with_deps /usr/bin/busybox
copy_with_deps /usr/sbin/dmsetup
copy_with_deps /usr/sbin/veritysetup
copy_with_deps /lib64/ld-linux-x86-64.so.2
ln -sfr "${INITRAMFS_DIR}/usr/bin/busybox" "${INITRAMFS_DIR}/bin/busybox"
ln -sfr "${INITRAMFS_DIR}/usr/sbin/dmsetup" "${INITRAMFS_DIR}/sbin/dmsetup"
ln -sfr "${INITRAMFS_DIR}/usr/sbin/veritysetup" "${INITRAMFS_DIR}/sbin/veritysetup"

MODULES=()
for mod in aesni_intel dm_bufio dm_crypt dm_verity; do
  packed="$(copy_module "$mod" || true)"
  [[ -n "$packed" ]] && MODULES+=("$packed")
done

{
  cat <<'EOF'
#!/bin/busybox sh
set -eu
/bin/busybox mkdir -p /proc /sys /dev /newroot /run /tmp /mapper
/bin/busybox mount -t proc proc /proc
/bin/busybox mount -t sysfs sys /sys
/bin/busybox mount -t devtmpfs dev /dev || true
export DM_DISABLE_UDEV=1
export PATH=/bin:/sbin:/usr/bin:/usr/sbin

for ko in /lib/modules/*/aesni-intel.ko /lib/modules/*/dm-bufio.ko /lib/modules/*/dm-crypt.ko /lib/modules/*/dm-verity.ko; do
  [ -f "$ko" ] && /bin/busybox insmod "$ko" || true
done

SCENARIO=plain
KEY=
ROOT_HASH=
DATA_BLOCKS=
HASH_OFFSET=
DISK=
for tok in $(cat /proc/cmdline); do
  case "$tok" in
    dm_boot=*) SCENARIO=${tok#dm_boot=} ;;
    dm_key=*) KEY=${tok#dm_key=} ;;
    dm_root_hash=*) ROOT_HASH=${tok#dm_root_hash=} ;;
    dm_data_blocks=*) DATA_BLOCKS=${tok#dm_data_blocks=} ;;
    dm_hash_offset=*) HASH_OFFSET=${tok#dm_hash_offset=} ;;
  esac
done

for _ in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20; do
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
    SECTORS=$(cat /sys/block/${DISK#/dev/}/size)
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
/bin/busybox sleep 8
echo o > /proc/sysrq-trigger || true
/bin/busybox poweroff -f || true
exec /bin/busybox sh
EOF
} > "${INITRAMFS_DIR}/init"
chmod +x "${INITRAMFS_DIR}/init"

echo "[linux-boot] packing initramfs.cpio.gz"
(
  cd "$INITRAMFS_DIR"
  find . -print0 | cpio --null -o -H newc 2>/dev/null | gzip -9 > "${OUT}/initramfs.cpio.gz"
)

printf 'LINUX_DM_BOOT_OK\n' > "$STAGE/etc/dm-boot-proof"
printf 'Linux QEMU protected-root boot image\n' > "$STAGE/README"

echo "[linux-boot] formatting plain ext4 ${SIZE_MIB}MiB"
PLAIN_IMG="${OUT}/plain-rootfs.img"
truncate -s "$((SIZE_MIB * 1024 * 1024))" "$PLAIN_IMG"
mke2fs -q -F -t ext4 -b "$BLOCK_SIZE" -d "$STAGE" "$PLAIN_IMG"

echo "[linux-boot] formatting dm-crypt ext4 ${SIZE_MIB}MiB"
CRYPT_IMG="${OUT}/crypt-rootfs.img"
truncate -s "$((SIZE_MIB * 1024 * 1024))" "$CRYPT_IMG"
MAPPER="linux_boot_crypt_$$"
LOOP_DEV="$(sudo losetup --find --show "$CRYPT_IMG")"
cleanup_crypt() {
  set +e
  sudo dmsetup remove "$MAPPER" >/dev/null 2>&1
  sudo losetup -d "$LOOP_DEV" >/dev/null 2>&1
}
trap cleanup_crypt EXIT
SECTORS=$((SIZE_MIB * 1024 * 1024 / 512))
echo "0 ${SECTORS} crypt aes-xts-plain64 ${KEY_HEX} 0 ${LOOP_DEV} 0" | sudo dmsetup create "$MAPPER"
sudo dmsetup mknodes "$MAPPER"
sudo mke2fs -q -F -t ext4 -b "$BLOCK_SIZE" -d "$STAGE" "/dev/mapper/${MAPPER}"
sync
sudo dmsetup remove "$MAPPER"
sudo losetup -d "$LOOP_DEV"
trap - EXIT

echo "[linux-boot] formatting dm-verity ext4 ${SIZE_MIB}MiB + hash"
VERITY_IMG="${OUT}/verity-rootfs.img"
truncate -s "$((SIZE_MIB * 1024 * 1024))" "$VERITY_IMG"
mke2fs -q -F -t ext4 -b "$BLOCK_SIZE" -d "$STAGE" "$VERITY_IMG"
truncate -s "$(( (SIZE_MIB + HASH_EXTRA_MIB) * 1024 * 1024 ))" "$VERITY_IMG"
FORMAT_OUT="$(veritysetup --data-block-size="$BLOCK_SIZE" --hash-block-size="$BLOCK_SIZE" \
  --data-blocks="$DATA_BLOCKS" --hash-offset="$HASH_OFFSET" format "$VERITY_IMG" "$VERITY_IMG")"
printf '%s\n' "$FORMAT_OUT"
ROOT_HASH="$(printf '%s\n' "$FORMAT_OUT" | awk -F: '/Root hash/{gsub(/[ \t]/,"",$2); print $2}')"
[[ -n "$ROOT_HASH" ]] || { echo "failed to parse verity root hash" >&2; exit 1; }

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
KERNEL=$OUT/vmlinuz
INITRD=$OUT/initramfs.cpio.gz
MODULES="${MODULES[*]}"
EOF

echo "artifacts=$OUT"
echo "env=${OUT}/boot.env"
echo "root_hash=$ROOT_HASH"
