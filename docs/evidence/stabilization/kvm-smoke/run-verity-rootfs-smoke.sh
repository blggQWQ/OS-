#!/bin/bash
set -euo pipefail

set -a
. /root/target/verity-rootfs.img.env
set +a
export ENABLE_KVM=1
export DM_IMAGE=/root/target/verity-rootfs.img
echo 'COMMAND: ENABLE_KVM=1 DM_IMAGE=/root/target/verity-rootfs.img bash tools/run_dm_verity.sh'
exec bash tools/run_dm_verity.sh
