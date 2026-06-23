#!/usr/bin/env bash
set -euo pipefail

IMAGE="${1:-target/linux-cryptsetup.img}"
SIZE="${2:-128M}"
MAPPER="${3:-cscc_secure_data}"
KEY_FILE="${4:-target/cryptsetup-demo.key}"

if ! command -v cryptsetup >/dev/null 2>&1; then
  echo "cryptsetup is required" >&2
  exit 1
fi

if ! command -v losetup >/dev/null 2>&1; then
  echo "losetup is required" >&2
  exit 1
fi

mkdir -p "$(dirname "$IMAGE")"
truncate -s "$SIZE" "$IMAGE"
printf "cscc-demo-passphrase\n" > "$KEY_FILE"
chmod 600 "$KEY_FILE"

LOOP_DEV="$(sudo losetup --find --show "$IMAGE")"
cleanup() {
  set +e
  sudo cryptsetup close "$MAPPER" >/dev/null 2>&1
  sudo losetup -d "$LOOP_DEV" >/dev/null 2>&1
}
trap cleanup EXIT

sudo cryptsetup luksFormat "$LOOP_DEV" --batch-mode --key-file="$KEY_FILE"
sudo cryptsetup open "$LOOP_DEV" "$MAPPER" --key-file="$KEY_FILE"

echo "Linux cryptsetup baseline ready:"
echo "  image=$IMAGE"
echo "  loop=$LOOP_DEV"
echo "  mapper=/dev/mapper/$MAPPER"
echo "  key_file=$KEY_FILE"
echo
echo "Run fio/filebench against /dev/mapper/$MAPPER or a filesystem mounted on it."
