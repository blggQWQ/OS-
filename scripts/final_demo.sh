#!/usr/bin/env bash
# One-command, fail-closed review demo for the userspace implementation and
# versioned KVM evidence. Use --live-kvm to rerun the two protected-root boots.
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

LIVE_KVM=0
if [[ "${1:-}" == "--live-kvm" ]]; then
    LIVE_KVM=1
elif [[ $# -ne 0 ]]; then
    echo "usage: $0 [--live-kvm]" >&2
    exit 2
fi

PASS_COUNT=0
FAIL_COUNT=0
DEMO_DIR="${ROOT}/target/final-demo"
mkdir -p "${DEMO_DIR}"

run_step() {
    local name="$1"
    shift
    echo
    echo "=== ${name} ==="
    if "$@"; then
        echo "PASS: ${name}"
        PASS_COUNT=$((PASS_COUNT + 1))
    else
        echo "FAIL: ${name}" >&2
        FAIL_COUNT=$((FAIL_COUNT + 1))
    fi
}

check_output() {
    local name="$1"
    local pattern="$2"
    local output="$3"
    shift 3
    echo
    echo "=== ${name} ==="
    if "$@" >"${output}" 2>&1 && grep -Fq "${pattern}" "${output}"; then
        cat "${output}"
        echo "PASS: ${name}"
        PASS_COUNT=$((PASS_COUNT + 1))
    else
        cat "${output}" >&2
        echo "FAIL: ${name}; expected ${pattern@Q}" >&2
        FAIL_COUNT=$((FAIL_COUNT + 1))
    fi
}

echo "Go_Go_Go dm-crypt / dm-verity final review demo"
echo "commit=$(git rev-parse --short=12 HEAD 2>/dev/null || echo unavailable)"
echo "rustc=$(rustc --version 2>/dev/null || echo unavailable)"
echo "mode=$([[ ${LIVE_KVM} -eq 1 ]] && echo live-kvm || echo versioned-kvm-evidence)"

run_step "Rust workspace tests" cargo test --workspace
check_output \
    "dm-crypt ciphertext-at-rest" \
    "ciphertext_differs: true" \
    "${DEMO_DIR}/dm-crypt.log" \
    cargo run --quiet -p dmctl -- demo-crypt
check_output \
    "dm-verity single-byte tamper rejection" \
    "tamper_detected: true" \
    "${DEMO_DIR}/dm-verity.log" \
    cargo run --quiet -p dmctl -- demo-verity
run_step \
    "Linux aes-xts-plain64 byte compatibility vector" \
    bash scripts/verify_archived_linux_xts.sh

if [[ ${LIVE_KVM} -eq 1 ]]; then
    check_output \
        "Asterinas KVM dm-crypt protected-root switch" \
        "PASS system root switched to protected mapper" \
        "${DEMO_DIR}/asterinas-crypt-rootfs-live.log" \
        env ENABLE_KVM=1 \
        DM_IMAGE="${ROOT}/target/crypt-rootfs.img" \
        bash asterinas/tools/run_dm_crypt.sh
    check_output \
        "Asterinas KVM dm-verity protected-root switch" \
        "PASS system root switched to protected mapper" \
        "${DEMO_DIR}/asterinas-verity-rootfs-live.log" \
        env ENABLE_KVM=1 \
        DM_IMAGE="${ROOT}/target/verity-rootfs.img" \
        bash asterinas/tools/run_dm_verity.sh
else
    run_step \
        "Versioned KVM functional evidence" \
        python3 scripts/verify_kvm_evidence.py
fi

run_step "Release-package validation" python3 scripts/review_release_check.py

echo
echo "=== FINAL SUMMARY ==="
echo "passed=${PASS_COUNT}"
echo "failed=${FAIL_COUNT}"
echo "logs=${DEMO_DIR}"
if [[ ${FAIL_COUNT} -ne 0 ]]; then
    echo "FINAL: FAIL" >&2
    exit 1
fi
echo "FINAL: PASS"
