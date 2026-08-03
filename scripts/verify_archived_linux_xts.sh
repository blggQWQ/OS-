#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK="${ROOT}/target/final-demo/linux-xts"
IMAGE="${WORK}/device.img"
SECTOR="${WORK}/sector-0.bin"
MANIFEST="${ROOT}/docs/evidence/linux-xts-plain64.sha256"
KEY="000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f"

mkdir -p "${WORK}"
truncate -s 1M "${IMAGE}"
cargo run --quiet --manifest-path "${ROOT}/Cargo.toml" -p dmctl -- \
    crypt-table-fill "${IMAGE}" 512 \
    "aes-xts-plain64 ${KEY} 0 ignored 0" 0 5a 1 >/dev/null
dd if="${IMAGE}" of="${SECTOR}" bs=512 count=1 status=none

EXPECTED="$(awk '!/^#/ && NF == 2 {print $1}' "${MANIFEST}")"
ACTUAL="$(sha256sum "${SECTOR}" | awk '{print $1}')"
if [[ -z "${EXPECTED}" || "${ACTUAL}" != "${EXPECTED}" ]]; then
    echo "expected=${EXPECTED}" >&2
    echo "actual=${ACTUAL}" >&2
    exit 1
fi
echo "linux_vector_sha256=${EXPECTED}"
echo "rust_sector_sha256=${ACTUAL}"
echo "aes-xts-plain64 archived Linux compatibility: PASS"
