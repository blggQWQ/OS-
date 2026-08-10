#!/bin/bash
set -euo pipefail

set -a
. /root/target/verity-rootfs.img.env
set +a

for spec in V03:0 V04:32768 V05:65535 V06:0 V07:0; do
    id=${spec%%:*}
    block=${spec##*:}
    log="/evidence/corruption/${id}-targeted-probe.log"
    echo "TEST=${id} COMMIT=$(git rev-parse HEAD) IMAGE=/root/target/${id}-corrupt.img ROOT_HASH=${DM_ROOT_HASH}" | tee "$log"
    DM_IMAGE="/root/target/${id}-corrupt.img" PROBE_BLOCK="$block" \
        bash tools/run_dm_verity_probe.sh 2>&1 | tee -a "$log"
    grep -q 'PASS .*integrity rejected\|PASS mapper read returned IoError/integrity rejection' "$log"
    ! grep -q 'FAIL corrupted block delivered as successful data' "$log"
    ! grep -qi 'panic' "$log"
    echo "RESULT=${id}:PASS integrity rejected; no mapper/root switch/panic" | tee -a "$log"
done
