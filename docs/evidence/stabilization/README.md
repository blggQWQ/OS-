# Stabilization evidence

This directory contains evidence produced during the feature-freeze validation on
2026-08-09. No benchmark values are synthesized or estimated.

## Environment

- Repository commit at start: `0c854a93e08c951afbba44b498a7f9286b0a2eb8`
- Branch: `master`
- Working tree: dirty before this validation
- Python: `3.9.13`
- Docker client: `29.4.3`
- Test image: `asterinas/asterinas:0.18.0-20260618`

## Commands and outcomes

| Command | Outcome | Evidence |
|---|---|---|
| 在固定官方容器的仓库 workspace 中执行 `cargo test --workspace` | PASS once: 21 passed, 0 failed | Terminal result; subsequent archival retries were blocked by the Docker named pipe |
| `python -m unittest scripts.tests.test_release_tools -v` | PASS: 8 passed | `regression/release-tools-unittest.log` |
| `python scripts/verify_kvm_evidence.py` | PASS: 4 versioned logs | `regression/verify-kvm-evidence.log` |
| `python scripts/review_release_check.py` | PASS | `regression/review-release-check.log` |
| `cargo fmt --check -p aster-dm` in the Asterinas image | PASS | Terminal result |
| `cargo check -p aster-dm` in the Asterinas image | BLOCKED before `aster-dm`: 82 unresolved upstream dependency errors in `ostd` | Terminal result |

`correctness/cargo-test-docker-retry-failed.log` records the failed archival retry.
It is retained rather than hidden. It does not invalidate the earlier successful
run, but it demonstrates that Docker Desktop access was unstable during this
session.

The failed `cargo check` reported missing/unresolved `acpi`, `x86_64`, `x86`,
`tdx_guest`, `unwinding`, and related architecture dependencies while compiling
`ostd`. It did not reach the changed `aster-dm` crate, so it is not evidence for
or against the correctness of the patch.
