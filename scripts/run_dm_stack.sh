#!/usr/bin/env bash
# Launch Asterinas Combined-stage stack (crypt → verity) against a stacked image.
#
# Asterinas requires the official Docker toolchain (VDSO_LIBRARY_DIR, etc.).
# This wrapper runs tools/run_dm_stack.sh inside that image by default.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ASTERINAS_DIR="${ASTERINAS_DIR:-$ROOT_DIR/asterinas}"
TOOL="$ASTERINAS_DIR/tools/run_dm_stack.sh"
IMAGE="${DM_IMAGE:-$ROOT_DIR/target/stack-rootfs.img}"
USE_DOCKER="${USE_DOCKER:-1}"
DOCKER_IMAGE_VERSION="$(cat "$ASTERINAS_DIR/DOCKER_IMAGE_VERSION" 2>/dev/null || echo 0.18.0-20260618)"
DOCKER_IMAGE="${DOCKER_IMAGE:-asterinas/asterinas:${DOCKER_IMAGE_VERSION}}"

if [[ ! -x "$TOOL" && ! -f "$TOOL" ]]; then
  cat >&2 <<EOF
missing $TOOL

Apply asterinas_port/patches/0001–0007 under ASTERINAS_DIR, e.g.:
  cd \$ASTERINAS_DIR
  git checkout c6284a9106f5a4c87deb7c8a990af6211dc5f540
  git am \$ROOT_DIR/asterinas_port/patches/000*.patch
EOF
  exit 1
fi

if [[ ! -f "$IMAGE" ]]; then
  echo "stacked image not found: $IMAGE" >&2
  echo "build with: sudo bash scripts/build_stack_rootfs.sh" >&2
  exit 1
fi
if [[ ! -f "${IMAGE}.env" ]]; then
  echo "missing metadata ${IMAGE}.env (rebuild with build_stack_rootfs.sh)" >&2
  exit 1
fi

if [[ "${ENABLE_KVM:-0}" = "1" && ! -e /dev/kvm ]]; then
  echo "warning: ENABLE_KVM=1 but /dev/kvm is missing; falling back to ENABLE_KVM=0" >&2
  ENABLE_KVM=0
fi

if [[ "$USE_DOCKER" != "1" ]]; then
  if [[ -z "${VDSO_LIBRARY_DIR:-}" ]]; then
    cat >&2 <<EOF
VDSO_LIBRARY_DIR is unset. Host-side make run_kernel is unsupported.
Use the Docker path (default), or:
  USE_DOCKER=1 bash scripts/run_dm_stack.sh
EOF
    exit 1
  fi
  export DM_IMAGE="$IMAGE"
  export ENABLE_KVM="${ENABLE_KVM:-0}"
  cd "$ASTERINAS_DIR"
  exec bash "$TOOL"
fi

if ! command -v docker >/dev/null; then
  echo "docker is required (or set USE_DOCKER=0 inside the Asterinas toolchain container)" >&2
  exit 1
fi
if ! docker image inspect "$DOCKER_IMAGE" >/dev/null 2>&1; then
  echo "pulling $DOCKER_IMAGE ..." >&2
  docker pull "$DOCKER_IMAGE"
fi

CONTAINER_IMAGE="/root/stack-rootfs.img"
DOCKER_ARGS=(
  --rm
  --privileged
  --network=host
  -v "$ASTERINAS_DIR:/root/asterinas"
  # QEMU virtio-blk opens the image read-write even for a read-only rootfs boot.
  -v "$IMAGE:$CONTAINER_IMAGE"
  -v "${IMAGE}.env:${CONTAINER_IMAGE}.env:ro"
  -w /root/asterinas
  -e "DM_IMAGE=$CONTAINER_IMAGE"
  -e "ENABLE_KVM=${ENABLE_KVM:-0}"
)

# Nested KVM only when the host exposes it.
if [[ "${ENABLE_KVM:-0}" = "1" && -e /dev/kvm ]]; then
  DOCKER_ARGS+=(--device=/dev/kvm)
fi

echo "running stacked mapper boot via $DOCKER_IMAGE (ENABLE_KVM=${ENABLE_KVM:-0})" >&2
exec docker run "${DOCKER_ARGS[@]}" "$DOCKER_IMAGE" bash tools/run_dm_stack.sh
