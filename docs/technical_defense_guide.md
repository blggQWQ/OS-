# dm-crypt / dm-verity 决赛技术答辩指南

本指南只使用仓库内可复核的实现和证据。凡是没有版本化日志或 CSV 支撑的性能数字，
答辩时均不作结论。

## 15 分钟建议节奏

| 时间 | 内容 | 必须展示的证据 |
|---:|---|---|
| 1 分钟 | 威胁模型与赛题目标 | 持久化镜像面临离线读取、篡改和替换 |
| 3 分钟 | 双目标 Device Mapper 架构 | `BlockDevice`、crypt/verity mapper、VirtIO BIO |
| 4 分钟 | dm-crypt 与 cryptsetup 兼容 | AES-256-XTS、plain64、table/control path |
| 3 分钟 | dm-verity 与根切换 | on-disk hash tree、只读策略、正反例 |
| 2 分钟 | 性能优化 | AES-NI、BIO 复用、hash cache、临时缓冲池 |
| 1 分钟 | 测试与证据链 | 单元测试、KVM 串口日志、五轮 CSV |
| 1 分钟 | 边界与展望 | 当前限制以及 I/O batch/异步路径 |

## 一句话结论

> 我们在 Asterinas 中实现了可组合的 dm-crypt 与 dm-verity 内核路径：
> dm-crypt 提供 Linux `aes-xts-plain64` 兼容的透明加密，dm-verity 使用
> on-disk Merkle tree 阻止被篡改的根文件系统启动；两条路径均通过真实 VirtIO
> BIO、KVM 根切换和可复核日志验证。

## 现场演示

默认演示不重新编译 Asterinas，而是运行用户态正确性测试并验证版本化 KVM 证据：

```bash
bash scripts/final_demo.sh
```

现场条件允许且已经预热官方容器时，可使用：

```bash
bash scripts/final_demo.sh --live-kvm
```

演示只有在最后出现 `FINAL: PASS` 时才算成功。若现场时间紧，不要临时修改镜像、
密钥或 QEMU 参数。

## 高频问题与回答要点

### 为什么不直接复用 Linux Device Mapper？

Asterinas 的 Framekernel、OSTD 和块设备接口与 Linux 内核不同，不能直接链接 Linux
实现。本项目复用了公开算法与协议定义，自主实现 mapper、控制面适配和 VirtIO BIO
路径，并保留补丁供逐行审计。

### 如何证明不是“用户态玩具”？

- `asterinas/kernel/comps/dm/` 是真实内核组件；
- mapper 包装真实 VirtIO block device；
- 串口日志显示 AES-NI、verity 状态和根文件系统 `pivot_root` 成功；
- 篡改镜像只修改一个字节，mapper 注册被拒绝且没有成功根切换标记。

### cryptsetup 兼容到什么程度？

已支持 plain target 的 `aes-xts-plain64`、512 位 XTS key、IV offset、data offset
和已声明的 table options；控制节点支持比赛演示所需的 create/load/resume/remove
流程。LUKS 元数据、PBKDF 和 keyslot 仍由用户态 cryptsetup 负责，不宣称实现完整
LUKS。

### dm-verity 的信任根在哪里？

根哈希通过启动参数注入，数据块和 on-disk hash block 逐层认证到该根哈希。若根哈希
本身来自不可信来源，verity 不能提供真实性，因此生产部署应由可信启动链、签名配置
或机密虚拟机证明绑定根哈希。

### 为什么性能不能只报一次？

虚拟化调度、页缓存和宿主机负载会造成波动。项目要求每个目标至少五轮、交替顺序并
报告中位数及 p95/p99；Linux/Asterinas 横向比较还会核对 CPU、内存、KVM、设备、
镜像区域哈希和 fio 参数，不一致时脚本拒绝出表。

### 你们如何证明时间花在哪里？

正式构建通过 `dm.perf_stats=1` 开启内核阶段归因。每轮 fio 前后读取一次
`/proc/dm-performance-stats`，再计算计数器差值；热路径只更新原子计数器，不逐 I/O
打印串口。dm-crypt 分为 lower wait、AES-XTS、copy、scratch 和 validation；
dm-verity 分为 data/hash lower wait、data/hash SHA-256、cache、digest compare 和
copy。另用 stats=0/1 五轮对照量化计时本身的观察者开销。

### 为什么这次只允许改一个瓶颈？

多个热路径改动叠加后无法从 A/B 数据判断收益来源，也更难证明没有削弱安全语义。
本轮正式五轮已经定位：dm-crypt 下层等待中位占 98.07%，AES-XTS 占 1.66%；
dm-verity 下层等待中位占 93.79%，数据 SHA-256 占 5.81%。因此没有继续微调 AES、
复制或 cache；真正的大项是同步 `submit_and_wait`。把它改成异步 BIO 完成会改变读完成
时的解密上下文和对象生命周期，不属于一次可安全证明的局部优化。若后续实施，仍须保留
完全相同合同下的 before/after 五轮 CSV，一次只改一个变量。

### 性能结果应该怎么表述？

严格同条件五轮矩阵中，Linux guest 与 Asterinas 的 dm-crypt 中位吞吐分别为
4.355 MiB/s 和 0.523 MiB/s，Asterinas 为 Linux 的 12.0%；不要把它包装成
“接近 Linux”，应明确这是下一阶段的主要优化空间。plain 与 dm-verity 冷启动
中位数分别为 16.058 秒和 16.902 秒，增量 5.256%。两组结论都只适用于归档
metadata 所定义的 1 vCPU、8 GiB、KVM 小规模同步读矩阵。

上述 Linux 对照的 8 MiB 数字属于历史证据，不能与新主表横向混用。新的主表已经按
release-LTO、相同 264 MiB 容器、前 256 MiB 数据区和 10 秒 runtime 的统一合同归档：
raw、dm-crypt、dm-verity 中位吞吐分别为 5.628、4.936、6.217 MiB/s。target 相对
各自配对 raw 的中位数为 102.0% 和 100.6%，应解释为“没有测出稳定吞吐损失”，
不能解释为映射层加速，也不能拿来声称达到 Linux 的某个百分比。

### 还有哪些限制？

- 用户态 `dm` crate 的 verity tree 用于快速测试，内核实现才使用 on-disk tree；
- 当前主路径聚焦 AES-256-XTS 和 SHA-256，没有宣称支持 Linux 的所有 cipher/target；
- mapper I/O 仍可继续引入 batch、异步完成和更细粒度并发；
- 密钥注入的生产化方案需要结合可信启动和远程证明。

## 答辩前检查清单

- `git status` 中没有来源不明的文件；
- `python3 scripts/review_release_check.py` 通过；
- `cargo test --workspace` 通过；
- `bash scripts/final_demo.sh` 最终输出 `FINAL: PASS`；
- PPT 中的每个性能数字都能指向 `docs/evidence/` 下的 CSV；
- 准备好解释 Linux/Asterinas 对照的全部 metadata 字段；
- 能从 `process_crypt`/`process_verity` 逐行指出每个阶段计时边界，并解释为什么采用
  前后快照而不是直接读取累计值；
- stats=0/1 观察者开销 CSV 已归档；
- 演示镜像、固定密钥和 KVM 日志均已离线保存。
