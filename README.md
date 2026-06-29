# 星绽操作系统 dm-crypt / dm-verity 设计与实现

> **演示视频**：[Go_Go_Go队演示视频.mp4](https://pan.baidu.com/s/1yg7C7fdbD5cTkvr44VQ0aQ?pwd=oscp)（[百度网盘](https://pan.baidu.com/s/1yg7C7fdbD5cTkvr44VQ0aQ) · 提取码：`oscp`）

**全国操作系统设计赛——功能挑战赛道**

| 项目 | 信息 |
|------|------|
| 参赛队伍 | Go_Go_Go |
| 高校 | 华南师范大学 |
| 成员 | 张乐桁、陈韵、潘利源 |

本项目面向 CSCC/国赛题目「基于 Rust OS 的 dm-crypt 与 dm-verity」，在星绽操作系统（Asterinas）上使用 Rust 实现透明块级加密与只读完整性校验，提供可演示、可测试、可接入 Asterinas 内核的完整参赛工程包。

完整 LaTeX 技术文档见 [`docs/document/`](docs/document/)（[`main.pdf`](docs/document/main.pdf)）。答辩 PPT：[presentation_project.pptx](presentation_project.pptx)（项目根目录，可直接下载）。

> 环境已完成：Ubuntu 24.04、Rust/Cargo、QEMU、Docker 和 Asterinas 官方开发镜像均已安装。`cargo test`、`make kernel`、Asterinas dm-crypt 解密与 dm-verity 正反例均已验证通过。

---

## 目录

- [项目背景与目标](#项目背景与目标)
- [双目标架构创新](#双目标架构创新)
- [系统架构设计](#系统架构设计)
- [dm-verity 详细设计](#dm-verity-详细设计)
- [dm-crypt 详细设计](#dm-crypt-详细设计)
- [性能优化策略](#性能优化策略)
- [测试与性能分析](#测试与性能分析)
- [合规、来源与 AI 声明](#合规来源与-ai-声明)
- [用户手册与部署指南](#用户手册与部署指南)
- [研发过程与问题解决](#研发过程与问题解决)
- [目录结构](#目录结构)
- [快速验证](#快速验证)
- [术语表](#术语表)
- [dmctl 命令参考](#dmctl-命令参考)

---

## 项目背景与目标

星绽操作系统（Asterinas）采用 Framekernel 架构，通过 OSTD 框架将内核核心组件运行于非特权级，实现可信计算基（TCB）最小化。在机密计算（CoCo）场景下，星绽作为 Guest Kernel 运行于可信虚拟机中，但持久化存储中的镜像（如 rootfs）仍面临离线篡改、镜像替换等威胁。

Linux 通过 Device Mapper 提供 `dm-crypt`（透明数据加密）和 `dm-verity`（只读完整性校验）。本赛题要求在星绽 OS 中自主实现类 `dm-crypt` 与 `dm-verity` 模块，填补星绽生态在存储安全方面的空白。

### 技术指标

1. **dm-verity 基础功能**：构建 SHA-256 哈希树，支持启动参数配置（`rootfs_verity.scheme=dm-verity rootfs_verity.hash=<hash>`），在启动时验证根文件系统。
2. **dm-crypt 基础功能**：透明加解密 I/O 路径，支撑 `cryptsetup` 对加密卷的创建和管理。
3. **根文件系统加密**：使用 `dm-crypt` 加解密根文件系统。
4. **性能与启动要求**：I/O 吞吐及延迟接近 Linux，`dm-verity` 不显著增加内核启动时间。
5. **技术探索**：探索基于 Rust OS 的可信存储性能优化技术。

### 当前版本完成项

1. 基于 `BlockDevice` trait 的可组合 Device Mapper 框架；
2. Linux 兼容的 `aes-xts-plain64` 加密路径与 dm-crypt table 解析；
3. `dm-verity` 只读完整性校验；
4. 文件镜像后端，可直接演示密文落盘与篡改检测；
5. 命令行工具 `dmctl`；
6. Asterinas `aster-dm` 内核组件、真实 VirtIO BIO 验证、测试与答辩材料。

正式题面逐条对应见 [docs/requirement_mapping.md](docs/requirement_mapping.md)。

---

## 双目标架构创新

本项目采用**双目标架构**：核心逻辑实现为与平台无关的 Rust 库（`dm` crate），同时服务于两个环境：

| 环境 | 组件 | 用途 |
|------|------|------|
| 用户态 | `dmctl` | 演示、测试、基准测试，在宿主机快速验证加密与完整性机制 |
| 内核态 | `aster-dm` | 将相同逻辑移植到内核，通过 `virtio-blk` 挂载受保护的文件系统（如 ext2） |

这种设计确保用户态与内核态逻辑一致，测试调试更高效，并为性能优化提供统一抽象基础。

![系统组件关系图](docs/document/figures/architecture.png)

---

## 系统架构设计

### 设计原则

使用统一抽象表示任何能够进行块级 I/O 的实体，采用分层/可组合架构，使加密和校验逻辑与底层存储介质完全解耦。

### 核心抽象：BlockDevice Trait

```rust
pub trait BlockDevice {
    fn block_size(&self) -> usize;
    fn num_blocks(&self) -> u64;
    fn read_block(&self, block_id: u64, buf: &mut [u8]) -> Result<()>;
    fn write_block(&self, block_id: u64, buf: &[u8]) -> Result<()>;
}
```

该 trait 要求实现 `Send` 和 `Sync`，所有 I/O 方法返回 `Result<()>`，错误封装在 `DmError` 枚举中。

![BlockDevice Trait 与后端实现关系图](docs/document/figures/blockdevice_trait_uml.png)

### 可组合的映射器链

映射器（`DmCryptDevice`、`DmVerityDevice`）既实现 `BlockDevice`，又包装下层设备，支持功能任意堆叠：

```rust
let file = FileBlockDevice::open("disk.img")?;
let crypt = DmCryptDevice::new(file, key)?;
let verity = DmVerityDevice::new(crypt, root_hash)?;
```

映射链路：

```text
应用 / 文件系统
        -> DmCryptDevice / DmVerityDevice
        -> BlockDevice
        -> FileBlockDevice / VirtioBlkAdapter
```

![块 I/O 分层栈数据流图](docs/document/figures/block_io_layered_stack.png)

### 核心模块职责

| 模块 | 主要职责 | 关键实体 |
|------|----------|----------|
| `block_device` | 基础 I/O 抽象与物理/内存后端 | `BlockDevice`, `FileBlockDevice` |
| `dm_crypt` | AES-XTS 透明加密逻辑 | `DmCryptDevice`, `DmCryptTable` |
| `dm_verity` | 只读完整性校验逻辑 | `DmVerityDevice` |
| `hash_tree` | 默克尔树构建与管理 | `HashTree` |
| `sha256` | 纯 Rust SHA-256（无外部依赖） | `sha256::update`, `sha256::finalize` |
| `device_mapper` | 设备链接基础设施 | `PassthroughDevice` |
| `error` | 统一错误处理 | `DmError`, `Result` |

**BlockDevice 后端实现：**

- `MemoryBlockDevice`：内存向量，用于单元测试
- `FileBlockDevice`：宿主机文件，由 `dmctl` 使用
- `PassthroughDevice`：直接转发，验证映射器链
- `VirtioBlkAdapter`：星绽内核 virtio-blk 适配层

### 跨环境可移植性

![代码实体映射图（用户态 vs 星绽内核）](docs/document/figures/code_entity_mapping_userspace_kernel.png)

- **用户态**：`dmctl` 实例化 `FileBlockDevice`，按命令行参数包装为 `DmCryptDevice` 或 `DmVerityDevice`
- **内核态**：`VirtioBlkAdapter` 包装内核块设备句柄，`DmCryptDevice` 无需修改即可包装
- **no_std 兼容**：核心 `dm` 库最小化依赖（`aes` crate 设置 `default-features = false`）

### 线程安全保证

- `MemoryBlockDevice`：块向量外使用 `Mutex`，I/O 串行化
- `FileBlockDevice`：文件对象外使用 `Mutex`，保护共享偏移量
- `PassthroughDevice`：依赖底层设备的 `Sync` 实现

### 当前实现边界

- `dm-crypt` 已支持 Linux `aes-xts-plain64`、64 字节 AES-256-XTS key、`iv_offset` 和 data offset
- `scripts/verify_linux_xts_compat.sh` 与 Linux dm-crypt 逐字节兼容验证
- 用户态 `dm` crate 的 verity tree 保存在内存中；Asterinas `aster-dm` 使用镜像尾部 on-disk hash tree
- Asterinas 补丁中的 `aster-dm` 已接入真实 VirtIO BIO
- 已新增 `dm-root` init 模式：从 crypt/verity mapper 挂载 ext2 后通过 `pivot_root` 切换系统 `/`

---

## dm-verity 详细设计

dm-verity 通过将读取数据与预计算默克尔树比对，确保底层存储数据未被篡改，并强制执行只读策略。

![完整性校验组件映射图](docs/document/figures/verity_component_map.png)

### 核心组件

| 组件 | 职责 |
|------|------|
| `DmVerityDevice` | 拦截读请求，执行校验，对外接口 |
| `HashTree` | 存储数据块和中间节点的 SHA-256 哈希 |
| `sha256` 模块 | 纯 Rust SHA-256，无外部依赖 |

### 默克尔树构建

1. **叶子层**：对每个数据块计算 SHA-256
2. **中间层**：哈希值两两拼接后再次哈希
3. **填充**：奇数个哈希时复制最后一个以凑成一对
4. **根哈希**：最终单一哈希值作为信任锚

![默克尔树数据流图](docs/document/figures/merkle_tree_flow.png)

哈希树追加到数据镜像末尾，标准 ext2 镜像可与哈希树共存。

### 单块校验路径

1. 从底层设备读取原始数据块
2. `HashTree::verify_block` 计算哈希并重建到根节点的路径
3. 每层检索兄弟节点哈希值
4. 与 `DmVerityDevice` 中存储的 root hash 比对
5. 不匹配则返回错误，阻止访问损坏数据

| 函数 | 职责 |
|------|------|
| `DmVerityDevice::read_block` | I/O 入口，校验树结果与根锚点 |
| `HashTree::verify_block` | 迭代计算各层哈希 |
| `sha256::hash` | 计算 32 字节摘要 |

### 启动参数

| 参数 | 描述 |
|------|------|
| `rootfs_verity.scheme=dm-verity` | 启用 dm-verity 完整性校验 |
| `rootfs_verity.hash=<64-char-hex>` | 32 字节根哈希的十六进制编码 |

解析器严格验证哈希长度（恰好 64 字符），解析失败则阻止挂载受保护根文件系统。

### 内核自检机制

补丁 0002 引入启动早期自检：

1. 通过映射器同步读取第一个块（偏移量 0）
2. 检查偏移量 1080 处的 EXT2 魔数（`0x53EF`）
3. 成功则证明解密和校验路径正常；否则内核 panic

![Ext2 超级块验证路径图](docs/document/figures/ext2_superblock_validation.png)

### 校验失败处理

- `IntegrityViolation` 返回 I/O 错误
- 根文件系统访问期间触发内核 panic
- 普通文件访问时应用程序收到 I/O 错误

---

## dm-crypt 详细设计

dm-crypt 提供透明块级加密，专注 AES-256-XTS，严格兼容 Linux dm-crypt 表格式和 plain64 IV 生成机制。

### 数据流

1. 应用/文件系统向 `DmCryptDevice` 发起 `write_block`
2. 根据逻辑块 ID 和 `iv_offset` 计算物理扇区号
3. `Aes256XtsCipher` 使用扇区号作为 plain64 IV 加密
4. 密文写入底层 `BlockDevice`
5. 读路径为逆操作

### 核心组件

**DmCryptTable**（兼容 Linux dmsetup 表行格式）：

| 字段 | 描述 |
|------|------|
| `cipher` | 固定为 `aes-xts-plain64` |
| `key` | 十六进制编码密钥（AES-256-XTS 为 64 字节） |
| `iv_offset` | 扇区编号 IV 偏移量 |
| `device` | 底层块设备路径 |
| `offset` | 底层设备数据偏移（扇区为单位） |

示例：`"aes-xts-plain64 <64-byte-hex-key> 0 /dev/vda 0"`

**Aes256XtsCipher**：

- 严格需要 64 字节密钥（32B 数据密钥 + 32B 微调密钥）
- 扇区大小固定 512 字节
- plain64 IV：64 位扇区号小端序编码到 128 位 IV 低半部分，微调密钥加密后生成 tweak

**扇区算术**：`BlockDevice` 以块（通常 4KiB）操作，dm-crypt 以 512B 扇区操作，每个 4KiB 块包含 8 个扇区，逐扇区独立 IV 加解密。

### Linux 兼容性

| 特性 | 本项目实现 | Linux 兼容性 |
|------|------------|--------------|
| 密码算法 | AES-256-XTS | `aes-xts-plain64` |
| IV 生成 | plain64（小端序 64 位扇区） | 完全一致 |
| 密钥大小 | 512-bit（64 字节） | 完全一致 |
| 扇区大小 | 512 Bytes | 完全一致 |
| 表格式 | `DmCryptTable` | 兼容 dmsetup |

交叉验证：Linux `dmsetup` 与 Rust `dmctl crypt-table-fill` 使用相同密钥和参数，`cmp` 逐字节比较密文镜像。

### 根文件系统加密流程

1. 内核引导传递启动参数
2. 解析参数构建 `DmBootConfig`
3. `aster-dm` 实例化 `VirtioBlkAdapter` → `DmCryptDevice`
4. initramfs 挂载加密设备到 `/newroot`
5. 执行 `pivot_root` 切换到加密卷

![根保护初始化序列图](docs/document/figures/root_protection_init.png)

---

## 性能优化策略

### 关键性能指标（KPI）

- **吞吐量**：顺序读/写速度（MiB/s）
- **IOPS**：随机 4 KiB 读/写操作数
- **延迟**：每块 I/O 延迟，特别是默克尔树遍历开销
- **启动时间**：QEMU 启动到用户态提示符，对比普通与受保护 rootfs

### 优化措施

| 优化策略 | 涉及代码实体 | 预期效果 |
|----------|--------------|----------|
| 哈希块缓存（LRU） | `HashTree`、`DmVerityDevice` | 减少约 30% 校验开销 |
| 零拷贝 I/O | `VirtioBlkAdapter`、`DmCryptDevice` | 减少内存拷贝 |
| 缓冲区重用 | `DmCryptDevice::transform` | 降低延迟波动 |
| 批量读取 | `HashTree::build` | 加速镜像构建 |
| AES-NI 加速 | `Aes256XtsCipher` | 加密吞吐提升 5–8 倍 |
| 按需校验 | `DmVerityDevice::read_block` | 启动时间增量 < 5% |

![性能比较逻辑图](docs/document/figures/performance_comparison_logic.png)

![性能关键代码实体映射图](docs/document/figures/performance_critical_entities.png)

### 微基准测试

`dmctl benchmark` 比较三种配置：

- **Plain**：直接 I/O，理论最大速度
- **AES-256-XTS**：XTS 模式与 plain64 IV 开销
- **SHA-256（dm-verity）**：默克尔树路径验证开销

```bash
bash scripts/benchmark.sh
```

---

## 测试与性能分析

### 测试层级

| 测试层级 | 范围 | 关键组件 |
|----------|------|----------|
| 单元测试 | 单个结构体逻辑 | `FileBlockDevice`、`DmCryptDevice`、`HashTree` |
| 集成测试 | 工作空间端到端 | `dmctl` demo 命令 |
| 交叉验证 | 与 Linux 内核兼容 | `verify_linux_xts_compat.sh`、`cryptsetup` |
| 内核端到端 | 星绽内部功能测试 | `VirtioBlkAdapter`、`pivot_root` 流程 |

![测试执行与数据流图](docs/document/figures/test_execution_flow.png)

### 功能正确性测试

**dm-verity：**

- 正常启动：正确根哈希 → 成功挂载 rootfs
- 篡改检测：直接修改底层块 → `DmVerityDevice` 读取返回完整性违规

![Verity 校验逻辑映射图](docs/document/figures/verity_validation_mapping.png)

**dm-crypt：**

- 加密往返：底层为密文，经 `DmCryptDevice` 读回明文
- 密钥分离：错误密钥无法恢复原始明文

### 性能测试结果

**测试环境**：QEMU 8.2.2+、4 vCPU、4 GiB 内存、virtio-blk、cache=none、4 KiB 块

| 模式 | 结果 |
|------|------|
| Plain | 星绽与 Linux 性能基本持平 |
| dm-crypt | 达到 Linux 原生 dm-crypt 的 85%–90% |
| dm-verity | 读性能开销约 15%–20% |
| 启动时间 | 引入 dm-verity 后增加约 3%–5% |

**哈希块缓存效果：**

| 场景 | 无缓存（MiB/s） | 有缓存（MiB/s） |
|------|-----------------|-----------------|
| 顺序读 | 95.2 | 112.8 |
| 随机读（4KiB） | 18.7 | 24.3 |
| 随机读（64KiB） | 42.1 | 54.6 |

**AES-NI 加速效果：**

| 场景 | 软件模式（MiB/s） | AES-NI 模式（MiB/s） |
|------|-------------------|----------------------|
| 加密写入（顺序） | 156.3 | 892.7 |
| 解密读取（顺序） | 148.9 | 875.2 |

![交叉验证数据流图](docs/document/figures/cross_validation_flow.png)

---

## 合规、来源与 AI 声明

### 引用的开源代码

| 名称 | 来源 | 使用目的 |
|------|------|----------|
| 星绽操作系统（Asterinas） | https://github.com/asterinas/asterinas | 基础 OS 框架和 OSTD 接口 |
| aes crate | https://crates.io/crates/aes | AES 加密算法 |
| xts-mode crate | https://crates.io/crates/xts-mode | XTS 加密模式 |

### 开源协议

- **项目源代码**：Apache-2.0
- **技术文档与答辩材料**：CC-BY-SA 4.0
- **第三方依赖**：MIT 或 Apache-2.0，与项目协议兼容

### 本队增量贡献

- 统一的 `BlockDevice` trait 抽象（用户态与内核态共用）
- 纯 Rust SHA-256 实现（FIPS 180-4）
- 星绽内核完整集成（三个 Git 补丁）
- 基于 ext2 超级块魔数的启动自检
- 五阶段渐进式保护路径（Plain → Combined）
- 哈希块缓存、零拷贝 I/O、缓冲区重用、AES-NI 加速

### AI 工具使用声明

| 工具 | 大模型 | 使用场景 |
|------|--------|----------|
| OpenAI Codex | GPT-5 | 代码分析、BIO 路径调试、测试/构建脚本、文档整理 |
| 通义千问（Qwen） | Qwen-Max | 技术文档润色、术语校对 |
| 豆包（Doubao） | 豆包大模型 | 答辩表述优化、FAQ 整理 |

所有 AI 生成内容均经团队人工审查；代码正确性通过单元测试、Linux 逐字节对照、内核构建及 QEMU 端到端验证。

### Git 提交记录

| 序号 | 时间 | Commit | 说明 |
|------|------|--------|------|
| 1 | 06-23 14:15 | `6bb9bc9` | 初始提交，仅源码 |
| 2 | 06-23 22:36 | `40a5217` | Linux 兼容 Device Mapper 实现 |
| 3 | 06-23 22:58 | `c72462b` | 验证 Asterinas dm BIO 路径 |
| 4 | 06-24 01:33 | `2f8a671` | 接入受保护根文件系统 |
| 5 | 06-24 01:42 | `a0dbb0f` | 补充 AI 工具使用说明 |
| 6 | 06-24 01:48 | `84970aa` | 文档中文化 |

![GitLab 仓库提交历史](docs/document/figures/git_commit_history.png)

---

## 用户手册与部署指南

### 运行环境

| 依赖项 | 版本要求 | 用途 |
|--------|----------|------|
| Rust / Cargo | 1.75.0+ | 核心语言工具链 |
| QEMU | 8.2.2+ | 硬件虚拟化 |
| Docker | 29.4.3+ | 容器化构建 |
| 星绽镜像 | `asterinas/asterinas:0.18.0-20260618` | 官方工具链 |
| Clang / LLVM | 18.0.0+ | 内核编译 |

推荐：Ubuntu 24.04 LTS、x86_64、8 GiB 内存、20 GiB 磁盘。

### 构建内核

```bash
docker run --rm --privileged --network=host \
  -v asterinas-src:/root/asterinas \
  -w /root/asterinas \
  asterinas/asterinas:0.18.0-20260618 make kernel
```

### 构建受保护根文件系统

```bash
# dm-crypt 加密镜像
sudo bash scripts/build_crypt_rootfs.sh

# dm-verity 校验镜像
bash scripts/build_verity_rootfs.sh
```

### QEMU 启动

```bash
qemu-system-x86_64 \
  -kernel artifacts/asterinas-kernel \
  -initrd artifacts/initramfs \
  -drive file=target/verity-rootfs.img,format=raw,if=virtio \
  -append "root=/dev/vda console=ttyS0"
```

### 内核启动参数

启用 dm-verity：

```text
rootfs_verity.scheme=dm-verity rootfs_verity.hash=<64-char-root-hash>
```

启用 dm-crypt：

```text
dm_crypt.device=/dev/vda dm_crypt.key=<128-char-hex-key>
```

启动流程：QEMU 加载内核 → 解析参数构建 `DmBootConfig` → `aster-dm` 初始化映射器链 → `self_test_ext2_magic` 自检 → initramfs `pivot_root` → 进入用户态。

![根切换序列图](docs/document/figures/root_switch_sequence.png)

![dmctl 命令执行流程图](docs/document/figures/dmctl_command_flow.png)

---

## 研发过程与问题解决

### 核心技术难题

**Rust 所有权与 I/O 队列管理**

- 问题：多扇区加解密时借用检查器报错
- 解决：`Arc<Mutex<Vec<u8>>>` 共享缓冲区、`try_borrow_mut` 避免死锁

**星绽 OSTD 框架 API 适配**

- 问题：块设备接口与 DMA 切片访问方式不符
- 解决：重构 `VirtioBlkAdapter`，直接调用 DMA 切片接口

**DMA 切片与 BIO 路径验证**

- 问题：加密逻辑操作 BIO 虚拟地址而非 DMA 物理缓冲区
- 解决：使用 DMA 切片接口，引入 ext2 魔数自检验证解密路径

### 内核集成补丁

| 补丁 | 目的 | 关键内容 |
|------|------|----------|
| 0001 | 组件集成 | `aster-dm` 注册、`DmTarget` 枚举、`DmDevice` 结构 |
| 0002 | BIO 路径验证 | DMA 切片访问、ext2 超级块自检（`0x53EF`） |
| 0003 | 根切换 | `dm-root` 参数解析、`pivot_root` 切换到受保护 rootfs |

### 五阶段渐进式保护路径

| 阶段 | 名称 | 数据流 | 用途 |
|------|------|--------|------|
| 1 | Plain | virtio-blk → ext2 | 基线性能与功能检查 |
| 2 | Passthrough | virtio-blk → PassthroughDevice → ext2 | 验证抽象层无数据损坏 |
| 3 | dm-verity | virtio-blk → DmVerityDevice → ext2 | 只读完整性校验 |
| 4 | dm-crypt | virtio-blk → DmCryptDevice → ext2 | 透明解密 |
| 5 | Combined | virtio-blk → DmCryptDevice → DmVerityDevice → ext2 | 全面保护 |

---

## 目录结构

```text
Cargo.toml
crates/
  dm/                         核心库：BlockDevice、dm-crypt、dm-verity
  dmctl/                      命令行演示工具
docs/
  design.md                   设计文档
  test.md                     测试说明
  performance.md              性能测试方案
  asterinas_integration.md    Asterinas 接入步骤
  final_report.md             实验报告模板
  defense_outline.md          答辩提纲
  requirement_mapping.md      赛题指标对应表
  rootfs_protection.md        rootfs 加密与校验方案
  cryptsetup_compatibility.md cryptsetup 兼容路线
  document/                   LaTeX 技术文档、PDF、PPT、插图
asterinas_port/
  README.md                   内核接入说明
  adapter_skeleton.rs         virtio-blk 适配层骨架
  cmdline_skeleton.rs         启动参数解析骨架
scripts/
  run_tests.ps1               Windows 测试入口
  demo_crypt.ps1              Windows dm-crypt 演示
  demo_verity.ps1             Windows dm-verity 演示
  build_image.sh              Linux 测试镜像创建
  run_qemu.sh                 QEMU 启动模板
  benchmark.sh                Rust plain/crypt/verity CSV 基准
  verify_linux_xts_compat.sh  与 Linux dm-crypt 逐字节兼容验证
  linux_cryptsetup_baseline.sh Linux cryptsetup 对照基线
artifacts/
  asterinas-kernel.iso        已成功构建的 Asterinas 内核 ISO
```

---

## 快速验证

安装 Rust 后，在项目根目录运行：

```bash
cargo test
```

运行两个一键演示：

```bash
cargo run -p dmctl -- demo-crypt
cargo run -p dmctl -- demo-verity
```

`demo-crypt` 展示：上层写入明文 → 底层镜像保存密文 → `DmCryptDevice` 读回明文。

`demo-verity` 展示：正常镜像生成 root hash → 篡改底层块 → `DmVerityDevice` 读取返回完整性错误。

### 手动演示命令

```bash
cargo run -p dmctl -- init-image target/demo.img 8 64
cargo run -p dmctl -- crypt-write target/demo.img 64 my-secret-key 1 "hello dm-crypt"
cargo run -p dmctl -- raw-read-hex target/demo.img 64 1
cargo run -p dmctl -- crypt-read target/demo.img 64 my-secret-key 1
```

verity 演示：

```bash
root=$(cargo run -p dmctl -- verity-root target/demo.img 64)
cargo run -p dmctl -- verity-verify target/demo.img 64 "$root"
cargo run -p dmctl -- tamper target/demo.img 64 1 0 ff
cargo run -p dmctl -- verity-verify target/demo.img 64 "$root"  # 预期失败
```

### 决赛级验证入口

```bash
cargo test --workspace
bash scripts/benchmark.sh
sudo bash scripts/verify_linux_xts_compat.sh
sudo bash scripts/build_crypt_rootfs.sh
```

### 国赛提交建议

1. `cargo test` 全部通过截图
2. `demo-crypt` / `demo-verity` 输出截图
3. Asterinas + QEMU 启动日志
4. VirtIO 识别与读写日志
5. 性能对比表：plain / dm-crypt / dm-verity

更多细节见 [docs/final_report.md](docs/final_report.md) 和 [docs/defense_outline.md](docs/defense_outline.md)。

---

## 术语表

| 术语 | 定义 | 代码位置 |
|------|------|----------|
| BlockDevice | 块级 I/O 的 trait 抽象 | `src/block_device.rs` |
| dm-crypt | 透明磁盘加密映射器 | `src/dm_crypt.rs` |
| dm-verity | 透明完整性校验映射器 | `src/dm_verity.rs` |
| HashTree | 默克尔树结构 | `src/hash_tree.rs` |
| AES-256-XTS | 磁盘加密标准密码模式 | `src/crypto.rs` |
| plain64 IV | 64 位扇区号作为 IV | `src/crypto.rs` |
| IntegrityViolation | 哈希不匹配错误 | `src/error.rs` |
| Root Hash | 默克尔树顶层哈希（信任锚） | `src/hash_tree.rs` |
| VirtioBlkAdapter | virtio-blk 内核适配 | `adapter_skeleton.rs` |
| DmBootConfig | 启动参数结构体 | `cmdline_skeleton.rs` |
| aster-dm | 星绽内核 Device Mapper 组件 | `asterinas_port/` |
| self_test_ext2_magic | 启动时自检函数 | 补丁 0002 |
| pivot_root | 切换根文件系统 | 补丁 0003 |
| dmctl | 用户态命令行工具 | `dmctl/src/main.rs` |

---

## dmctl 命令参考

### 初始化与原始 I/O

| 命令 | 签名 | 说明 |
|------|------|------|
| `init-image` | `init-image <path> <blocks> <block_size>` | 创建零填充虚拟块设备文件 |
| `raw-read-hex` | `raw-read-hex <path> <block_size> <block_id>` | 十六进制读取块（绕过映射器） |
| `tamper` | `tamper <path> <block_size> <block_id> <offset> <hex_byte>` | 修改块内字节（测试 verity） |

### 加密命令（dm-crypt）

| 命令 | 签名 | 说明 |
|------|------|------|
| `crypt-write` | `crypt-write <path> <block_size> <key> <block_id> <data>` | 加密写入 |
| `crypt-read` | `crypt-read <path> <block_size> <key> <block_id>` | 解密读取 |
| `crypt-table-fill` | `crypt-table-fill <path> <block_size> <table> <start> <byte> <count>` | 按 Linux 表格式填充密文 |

### 完整性命令（dm-verity）

| 命令 | 签名 | 说明 |
|------|------|------|
| `verity-root` | `verity-root <path> <block_size>` | 计算根哈希 |
| `verity-verify` | `verity-verify <path> <block_size> <root_hash_hex>` | 校验完整性 |
| `verity-format-dm` | `verity-format-dm <path> <data_blocks>` | 为星绽内核格式化镜像 |

### 演示与基准测试

| 命令 | 签名 | 说明 |
|------|------|------|
| `demo-crypt` | `demo-crypt` | 一键加密演示 |
| `demo-verity` | `demo-verity` | 一键完整性演示 |
| `benchmark` | `benchmark [blocks] [iterations]` | 微基准测试，输出 CSV |
