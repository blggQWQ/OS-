# Asterinas KVM smoke evidence

This directory contains the three minimal smoke regressions after the successful kernel build.

- `cryptsetup-smoke-kvm-retry.log`: PASS; real cryptsetup open/read/close.
- `crypt-rootfs-smoke-kvm.log`: PASS; dm-crypt protected root switch.
- `verity-rootfs-smoke-kvm.log`: PASS; verified dm-verity protected root switch.
- `run-*.sh`: exact auditable one-shot commands used inside the fixed official container.

Failed or invalid setup attempts are retained rather than overwritten. They document the parent-workspace mount error, a deliberately terminated TCG attempt, and the subsequent stale image lock recovery.
