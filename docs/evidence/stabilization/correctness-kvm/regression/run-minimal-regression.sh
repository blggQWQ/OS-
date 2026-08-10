#!/bin/bash
set -euo pipefail

echo '=== R01 cryptsetup smoke ==='
DM_IMAGE=/root/target/dm-crypt-test.img ENABLE_KVM=1 \
    bash tools/run_cryptsetup_compat.sh 2>&1 | tee /evidence/regression/R01-cryptsetup.log
grep -q 'PASS real cryptsetup open created a readable mapper' /evidence/regression/R01-cryptsetup.log
grep -q 'PASS real cryptsetup close removed the mapper' /evidence/regression/R01-cryptsetup.log

echo '=== R02 dm-crypt rootfs smoke ==='
DM_IMAGE=/root/target/crypt-rootfs.img ENABLE_KVM=1 \
    bash tools/run_dm_crypt.sh 2>&1 | tee /evidence/regression/R02-crypt-rootfs.log
grep -q 'PASS system root switched to protected mapper' /evidence/regression/R02-crypt-rootfs.log

echo '=== R03 dm-verity rootfs smoke ==='
set -a
. /root/target/verity-rootfs.img.env
set +a
DM_IMAGE=/root/target/verity-rootfs.img ENABLE_KVM=1 \
    bash tools/run_dm_verity.sh 2>&1 | tee /evidence/regression/R03-verity-rootfs.log
grep -q 'verity status=verified' /evidence/regression/R03-verity-rootfs.log
grep -q 'PASS system root switched to protected mapper' /evidence/regression/R03-verity-rootfs.log

echo 'MINIMAL_REGRESSION: PASS'
