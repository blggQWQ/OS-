#!/usr/bin/env bash
set -euo pipefail

IMAGE="${IMAGE:-/tmp/cscc-cryptsetup-trace.img}"
KEY_FILE="${KEY_FILE:-/tmp/cscc-cryptsetup-trace.key}"
MAPPER="${MAPPER:-cscc_trace_mapper}"
TRACE="${TRACE:-/tmp/cscc-cryptsetup.trace}"

for command in cryptsetup losetup modprobe strace truncate; do
  command -v "$command" >/dev/null || {
    echo "missing command: $command" >&2
    exit 1
  }
done
if [[ "$EUID" -ne 0 ]]; then
  echo "run as root" >&2
  exit 1
fi

modprobe dm-crypt
truncate -s 16M "$IMAGE"
head -c 64 /dev/urandom > "$KEY_FILE"
LOOP_DEV="$(losetup --find --show "$IMAGE")"
cleanup() {
  set +e
  cryptsetup close "$MAPPER" >/dev/null 2>&1
  losetup -d "$LOOP_DEV" >/dev/null 2>&1
}
trap cleanup EXIT

strace -f -yy -s 1024 -o "$TRACE.open" \
  -e trace=openat,ioctl,close,mknodat,unlink,read,write \
  cryptsetup open --type plain --cipher aes-xts-plain64 --key-size 512 \
  --key-file "$KEY_FILE" "$LOOP_DEV" "$MAPPER"
strace -f -yy -s 1024 -o "$TRACE.close" \
  -e trace=openat,ioctl,close,mknodat,unlink,read,write \
  cryptsetup close "$MAPPER"

grep -h -E '/dev/mapper/control|DM_|ioctl\(' "$TRACE.open" "$TRACE.close" | tail -180
