#!/usr/bin/env bash
set -euo pipefail

MOUNT_POINT="${1:-/mnt/dm-test}"
SIZE="${2:-128M}"

if ! command -v fio >/dev/null 2>&1; then
  echo "fio is required for performance tests" >&2
  exit 1
fi

fio --name=dm-crypt-rw --filename="$MOUNT_POINT/testfile" --size="$SIZE" --rw=readwrite --bs=4k
fio --name=dm-verity-read --filename="$MOUNT_POINT/testfile" --size="$SIZE" --rw=read --bs=4k

