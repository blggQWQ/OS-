# 开发环境状态

## 已安装

- Ubuntu 24.04 WSL2；
- Rust 1.75.0 / Cargo 1.75.0（项目用户态测试）；
- QEMU 8.2.2；
- clang/LLVM/lld/build-essential；
- Docker Desktop 29.4.3；
- Asterinas 官方开发镜像 `asterinas/asterinas:0.18.0-20260618`；
- Asterinas 基线 commit `c6284a9`，当前集成 commit `ebf59b0`。

## 存储位置

为避免占用 C 盘，大体积开发数据已迁移到 D 盘：

- Ubuntu WSL VHDX: `D:\WSL\Ubuntu-24.04\ext4.vhdx`；
- Docker 数据 VHDX: `D:\DockerData\disk\docker_data.vhdx`；
- 项目源码: `D:\project`；
- Asterinas ISO: `D:\project\artifacts\asterinas-kernel.iso`。

Docker 原数据路径
`C:\Users\潘丽园\AppData\Local\Docker\wsl\disk`
现为指向 `D:\DockerData\disk` 的 Windows 目录联接，本身不再保存大体积数据。不要单独删除该联接或 D 盘目标目录。

## 已验证

### 用户态 dm 工程

```bash
wsl -d Ubuntu-24.04 -u root -- bash -lc "cd /mnt/d/project && cargo test"
```

结果：13 个测试全部通过。

### dm-crypt 演示

```bash
wsl -d Ubuntu-24.04 -u root -- bash -lc "cd /mnt/d/project && cargo run -p dmctl -- demo-crypt"
```

结果：明文可正常读回，底层块为密文，`ciphertext_differs: true`。

### dm-verity 演示

```bash
wsl -d Ubuntu-24.04 -u root -- bash -lc "cd /mnt/d/project && cargo run -p dmctl -- demo-verity"
```

结果：篡改块被检测，`tamper_detected: true block=1`。

### Asterinas 构建

由于 Windows bind mount 不支持 `fallocate`，Asterinas 在 Docker Linux volume `asterinas-src` 中构建：

```powershell
docker run --rm --privileged --network=host `
  -v asterinas-src:/root/asterinas `
  -w /root/asterinas `
  asterinas/asterinas:0.18.0-20260618 make kernel
```

结果：成功生成 `aster-kernel-osdk-bin.iso`。

### Asterinas QEMU 启动

```powershell
docker run --rm --privileged --network=host `
  -v asterinas-src:/root/asterinas `
  -w /root/asterinas `
  asterinas/asterinas:0.18.0-20260618 make run_kernel
```

已观测到：

```text
Spawn the first kernel thread
[kernel] unpacking initramfs.cpio.gz to rootfs ...
[kernel] rootfs is ready
```

## 构建产物

- `D:\project\artifacts\asterinas-kernel.iso`，约 150 MB。

## 下一阶段

已完成额外 VirtIO 测试盘的真实 I/O 验证：

```text
[dm-test] PASS AES-256-XTS decrypted a valid ext2 superblock through dm-crypt
[dm-test] PASS first data block verified through dm-verity
[dm-test] FAIL dm-verity rejected first data block: IntegrityViolation
```

下一阶段只剩把已注册 mapper 接入系统 `/` 的正式根挂载流程，并记录启动耗时。
