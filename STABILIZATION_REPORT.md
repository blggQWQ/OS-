# Stabilization Report

## 最终状态

当前稳定化结论为 **READY**。

- Asterinas commit：`9d79542cadddc6164ad3f31fd8cba776969d85a9`
- kernel build：PASS
- `aster-dm` compile：PASS
- R01/R02/R03 KVM gate：PASS
- dm-crypt correctness P0（C01-C05）：PASS
- dm-verity correctness P0（V01-V07）：PASS
- 产品 correctness bug：None

## 已完成修改

- runtime crypt options 仅声明已实现能力，未实现选项明确拒绝。
- runtime verity target 不再作为已实现 target 对外公布。
- 增加真实 KVM correctness 测试模式与原始证据。

没有修改 AES-XTS、Merkle tree 格式、BIO/virtio/DMA、filesystem 或核心算法。

## 当前测试证据

- [构建恢复报告](BUILD_RECOVERY_REPORT.md)
- [KVM correctness 报告](KVM_CORRECTNESS_REPORT.md)
- [KVM correctness 矩阵](KVM_CORRECTNESS_MATRIX.md)
- [平台评测环境](PLATFORM_EVAL_ENVIRONMENT.md)
- [物理篡改清单](PHYSICAL_MUTATION_MANIFEST.md)
- [完整测试矩阵](TEST_MATRIX.md)
- [验收清单](ACCEPTANCE_CHECKLIST.md)

历史构建与环境失败日志保留在 `docs/evidence/stabilization/`，仅供追溯，不代表当前状态。
