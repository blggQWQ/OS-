# KVM correctness baseline

- Date: 2026-08-10 (Asia/Shanghai)
- Top-level branch: `master`
- Top-level commit: `09f62e5950ae36111de57f37becfdfdae27fcc69`
- Asterinas branch: `codex/dm-completion-pool`
- Asterinas commit: `9d79542cadddc6164ad3f31fd8cba776969d85a9`
- Working tree: dirty before testing; unrelated changes are excluded from conclusions
- Docker image: `asterinas/asterinas:0.18.0-20260618`
- Docker image digest: `sha256:7510071cb103309cc9396b970325c354713da749dfee34a807108248cc97331e`
- Rust toolchain: `nightly-2026-04-03`
- Target: `x86_64` / `x86_64-unknown-none` through OSDK
- QEMU: `-accel kvm -cpu host`
- Asterinas workspace: `asterinas/`
- Test image directory: `target/`
- Crypt image: `target/crypt-rootfs.img`
- Verity image: `target/verity-rootfs.img`

The full pre-test `git status` and `git diff --stat` were recorded in the task terminal before this file was created.
