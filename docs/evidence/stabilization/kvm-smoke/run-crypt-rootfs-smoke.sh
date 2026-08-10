#!/bin/bash
set -euo pipefail

export ENABLE_KVM=1
export DM_IMAGE=/root/target/crypt-rootfs.img
echo 'COMMAND: ENABLE_KVM=1 DM_IMAGE=/root/target/crypt-rootfs.img bash tools/run_dm_crypt.sh'
exec bash tools/run_dm_crypt.sh
