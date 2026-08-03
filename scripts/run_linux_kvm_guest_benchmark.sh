#!/usr/bin/env bash
# Boot an Ubuntu 24.04 KVM guest, attach the same virtio data image used by
# Asterinas, execute the strict five-run benchmark, and copy evidence back.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK="${ROOT}/target/benchmark/linux-kvm-guest"
BASE_IMAGE="${WORK}/ubuntu-24.04-minimal-cloudimg-amd64.img"
BASE_URL="${LINUX_CLOUD_IMAGE_URL:-https://cloud-images.ubuntu.com/minimal/releases/noble/release/ubuntu-24.04-minimal-cloudimg-amd64.img}"
APT_MIRROR="${LINUX_APT_MIRROR:-https://mirrors.ustc.edu.cn/ubuntu}"
OVERLAY="${WORK}/ubuntu-benchmark-overlay.qcow2"
SEED="${WORK}/seed.img"
USER_DATA="${WORK}/user-data"
META_DATA="${WORK}/meta-data"
SSH_KEY_DIR="${HOME}/.cache/cscc-linux-kvm"
SSH_KEY="${SSH_KEY_DIR}/id_ed25519"
SERIAL_LOG="${WORK}/serial.log"
PID_FILE="${WORK}/qemu.pid"
DATA_IMAGE="${DM_IMAGE:-${ROOT}/target/dm-crypt-test.img}"
OUTPUT="${OUTPUT:-${ROOT}/target/benchmark/linux-guest-dm-crypt.csv}"
METADATA="${OUTPUT%.csv}.metadata.txt"
SSH_PORT="${SSH_PORT:-2222}"
RUNS="${RUNS:-5}"
SIZE_MIB="${SIZE_MIB:-8}"
RUNTIME="${RUNTIME:-2}"
RAMP_TIME="${RAMP_TIME:-1}"

for command in cloud-localds curl qemu-img qemu-system-x86_64 scp ssh ssh-keygen; do
    command -v "${command}" >/dev/null || {
        echo "missing host command: ${command}" >&2
        exit 1
    }
done
[[ -c /dev/kvm ]] || {
    echo "/dev/kvm is unavailable" >&2
    exit 1
}
[[ -f "${DATA_IMAGE}" ]] || {
    echo "data image not found: ${DATA_IMAGE}" >&2
    exit 1
}

mkdir -p "${WORK}" "$(dirname "${OUTPUT}")" "${SSH_KEY_DIR}"
chmod 700 "${SSH_KEY_DIR}"
if [[ ! -f "${BASE_IMAGE}" ]]; then
    echo "downloading Ubuntu cloud image from ${BASE_URL}"
    curl --fail --location --retry 4 --continue-at - \
        --output "${BASE_IMAGE}.part" "${BASE_URL}"
    mv "${BASE_IMAGE}.part" "${BASE_IMAGE}"
fi

if [[ ! -f "${SSH_KEY}" ]]; then
    ssh-keygen -q -t ed25519 -N "" -f "${SSH_KEY}"
fi
PUBLIC_KEY="$(cat "${SSH_KEY}.pub")"
cat >"${USER_DATA}" <<EOF
#cloud-config
users:
  - default
  - name: reviewer
    groups: [adm, sudo]
    shell: /bin/bash
    sudo: ALL=(ALL) NOPASSWD:ALL
    ssh_authorized_keys:
      - ${PUBLIC_KEY}
ssh_pwauth: false
disable_root: true
EOF
cat >"${META_DATA}" <<EOF
instance-id: cscc-dm-review-$(date +%s)
local-hostname: cscc-linux-guest
EOF
cloud-localds "${SEED}" "${USER_DATA}" "${META_DATA}"

if [[ -f "${PID_FILE}" ]]; then
    OLD_PID="$(cat "${PID_FILE}")"
    if kill -0 "${OLD_PID}" 2>/dev/null; then
        echo "benchmark VM is already running with pid ${OLD_PID}" >&2
        exit 1
    fi
fi
rm -f "${OVERLAY}" "${SERIAL_LOG}" "${PID_FILE}"
qemu-img create -q -f qcow2 -F qcow2 -b "${BASE_IMAGE}" "${OVERLAY}"

cleanup() {
    if [[ -f "${PID_FILE}" ]]; then
        VM_PID="$(cat "${PID_FILE}")"
        if kill -0 "${VM_PID}" 2>/dev/null; then
            kill "${VM_PID}" 2>/dev/null || true
        fi
    fi
}
trap cleanup EXIT

qemu-system-x86_64 \
    -name cscc-linux-guest \
    -accel kvm -cpu host -smp 1 -m 8G \
    -drive "file=${OVERLAY},if=virtio,format=qcow2,cache=none" \
    -drive "file=${DATA_IMAGE},if=virtio,format=raw,cache=none" \
    -drive "file=${SEED},if=virtio,format=raw,readonly=on" \
    -nic "user,model=virtio-net-pci,hostfwd=tcp:127.0.0.1:${SSH_PORT}-:22" \
    -display none -serial "file:${SERIAL_LOG}" \
    -no-reboot -daemonize -pidfile "${PID_FILE}"

SSH_ARGS=(
    -i "${SSH_KEY}"
    -p "${SSH_PORT}"
    -o StrictHostKeyChecking=no
    -o UserKnownHostsFile=/dev/null
    -o ConnectTimeout=5
)
SCP_ARGS=(
    -i "${SSH_KEY}"
    -P "${SSH_PORT}"
    -o StrictHostKeyChecking=no
    -o UserKnownHostsFile=/dev/null
    -o ConnectTimeout=5
)

echo "waiting for guest SSH and cloud-init"
for attempt in $(seq 1 120); do
    if ssh "${SSH_ARGS[@]}" reviewer@127.0.0.1 true >/dev/null 2>&1; then
        break
    fi
    if [[ "${attempt}" -eq 120 ]]; then
        echo "guest SSH did not become ready; see ${SERIAL_LOG}" >&2
        exit 1
    fi
    sleep 2
done
ssh "${SSH_ARGS[@]}" reviewer@127.0.0.1 "cloud-init status --wait"
echo "installing benchmark dependencies with bounded retries"
ssh "${SSH_ARGS[@]}" reviewer@127.0.0.1 \
    "sudo sed -i \
       -e 's|http://archive.ubuntu.com/ubuntu|${APT_MIRROR}|g' \
       -e 's|http://security.ubuntu.com/ubuntu|${APT_MIRROR}|g' \
       /etc/apt/sources.list.d/ubuntu.sources && \
     sudo env DEBIAN_FRONTEND=noninteractive timeout 300 \
       apt-get -o Acquire::Retries=5 -o Acquire::ForceIPv4=true \
       -o Acquire::http::Timeout=20 update && \
     sudo env DEBIAN_FRONTEND=noninteractive timeout 600 \
       apt-get -o Acquire::Retries=5 -o Acquire::ForceIPv4=true \
       -o Acquire::http::Timeout=20 install --no-install-recommends -y \
       fio jq dmsetup && \
     command -v fio jq dmsetup"
scp "${SCP_ARGS[@]}" \
    "${ROOT}/scripts/benchmark_linux_guest_dm_crypt.sh" \
    reviewer@127.0.0.1:/tmp/benchmark_linux_guest_dm_crypt.sh
ssh "${SSH_ARGS[@]}" reviewer@127.0.0.1 \
    "chmod +x /tmp/benchmark_linux_guest_dm_crypt.sh && \
     sudo env RAW_DEVICE=/dev/vdb RUNS=${RUNS} SIZE_MIB=${SIZE_MIB} \
       RUNTIME=${RUNTIME} RAMP_TIME=${RAMP_TIME} VCPUS=1 MEMORY=8G \
       ACCELERATOR=kvm CPU_MODEL=host DEVICE_MODEL=virtio-blk \
       /tmp/benchmark_linux_guest_dm_crypt.sh /tmp/linux-guest-dm-crypt.csv"
scp "${SCP_ARGS[@]}" \
    reviewer@127.0.0.1:/tmp/linux-guest-dm-crypt.csv "${OUTPUT}"
scp "${SCP_ARGS[@]}" \
    reviewer@127.0.0.1:/tmp/linux-guest-dm-crypt.metadata.txt "${METADATA}"
ssh "${SSH_ARGS[@]}" reviewer@127.0.0.1 "sudo poweroff" || true

for _ in $(seq 1 30); do
    [[ -f "${PID_FILE}" ]] || break
    VM_PID="$(cat "${PID_FILE}")"
    kill -0 "${VM_PID}" 2>/dev/null || break
    sleep 1
done
trap - EXIT
echo "linux_guest_csv=${OUTPUT}"
echo "linux_guest_metadata=${METADATA}"
echo "linux_guest_serial=${SERIAL_LOG}"
