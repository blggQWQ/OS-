# Test Matrix

结果定义：仅真实执行并留存原始证据的项目标为 PASS。

| 范围 | 结果 | Evidence |
|---|---|---|
| kernel build / aster-dm compile | PASS | `docs/evidence/stabilization/platform-eval/kernel-build-reproducible-offline.log` |
| R01 cryptsetup smoke | PASS | `docs/evidence/stabilization/correctness-kvm/regression/R01-cryptsetup-persistent-cache.log` |
| R02 dm-crypt rootfs | PASS | `docs/evidence/stabilization/correctness-kvm/regression/R02-crypt-rootfs-persistent-cache.log` |
| R03 dm-verity rootfs | PASS | `docs/evidence/stabilization/correctness-kvm/regression/R03-verity-rootfs-persistent-cache.log` |
| C01-C05 dm-crypt P0 | PASS | `docs/evidence/stabilization/correctness-kvm/crypt/C01-C05-kvm.log` |
| V01 normal root | PASS | R03 evidence |
| V02 wrong root | PASS | `docs/evidence/stabilization/correctness-kvm/corruption/V02-wrong-root.log` |
| V03-V07 corruption P0 | PASS | `docs/evidence/stabilization/correctness-kvm/corruption/V03-targeted-probe.log` through `V07-targeted-probe.log` |
| C06-C15 | UNTESTED | P1 |
| V08-V12 | UNTESTED | P1 |
| H01-H08 | UNTESTED | P1 |
| P01 | UNTESTED | P1 |

详细逐项状态见 [KVM correctness 矩阵](KVM_CORRECTNESS_MATRIX.md)。
