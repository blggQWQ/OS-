# Combined-stage kernel stack evidence

Acceptance keywords (serial log):

- `[dm] stacked mapper: vdc -> dm-crypt0 -> dm-verity0`
- `[dm-test] PASS AES-256-XTS decrypted a valid ext2 superblock through dm-crypt`
- `[dm-test] PASS first data block verified through dm-verity`
- `[dm-root] PASS system root switched to protected mapper`

Reproduce:

```bash
sudo bash scripts/build_stack_rootfs.sh
# asterinas/ 需已 git am 0001–0007；启动走官方 Docker（内含 VDSO_LIBRARY_DIR）
ENABLE_KVM=0 bash scripts/run_dm_stack.sh 2>&1 | tee docs/evidence/stack/qemu-stack.log
# 宿主机有 /dev/kvm 时再用 ENABLE_KVM=1
```

`scripts/run_dm_stack.sh` 默认 `USE_DOCKER=1`，镜像
`asterinas/asterinas:$(cat asterinas/DOCKER_IMAGE_VERSION)`。
宿主机直接 `make run_kernel` 会因缺少 `VDSO_LIBRARY_DIR` 失败——这是预期行为。

Status: 已在本机 Docker+QEMU（无 KVM）跑通；日志见 `qemu-stack.log`。
