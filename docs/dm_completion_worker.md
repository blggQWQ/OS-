# dm-crypt 完成 worker：实现、结果与答辩说明

## 先说结论

本轮在 `codex/dm-completion-worker` 分支完成了 dm 专用完成 worker。VirtIO 硬中断回调现在只记录下层 BIO 状态并把预分配任务放入队列；AES-XTS 读解密、scratch 归还和上层 BIO 完成都在内核线程上下文执行。

这项改动的主要收益是并发正确性和可扩展性，不是 QD1 吞吐的巨大增长：

| 场景 | before | after | 结论 |
|---|---:|---:|---|
| QD1 dm-crypt 中位吞吐 | 6.632 MiB/s | 6.647 MiB/s | +0.22%，基本持平 |
| QD1 配对 dm/raw 保持率 | 98.74% | 99.32% | 按 raw 归一化后 +0.59% |
| QD1 dm-crypt p95 | 847.872 us | 872.448 us | +2.90%，略有退化 |
| 32-job 并发 | 第一个 dm-crypt 样本停住 | 五轮全部完成 | `max_in_flight=32`，并发路径可运行 |

after 的 32-job 五轮中，dm/raw 逐轮配对保持率中位数为 92.63%。不能把该场景称为“单 job QD32”：Asterinas 当前 `posixaio` 测试虽然在 fio 侧显示 iodepth=32，到 dm 层实测仍为 `max_in_flight=1`。因此正式证据使用 `ioengine=sync, iodepth=1, numjobs=32`，名称是“32-job 并发”。

## 一次读 I/O 发生了什么

可以把路径理解成操作系统课程里的“上半部/下半部”拆分：

1. 上层向 dm-crypt 提交 BIO；dm 先准备 scratch、状态槽和完成任务。
2. dm 把下层 BIO 异步交给 VirtIO，不占着线程同步等待。
3. VirtIO 中断到来时，回调只保存完成状态并进行 O(1) 入队；这里不解密，也不动态扩容队列。
4. 专用 worker 被唤醒，在普通线程上下文执行 AES-XTS 解密并完成上层 BIO。

关键安全点是：工作项及 intrusive link 在提交下层 BIO 前就已分配，IRQ 入队不需要分配内存；队列锁内只做 push/pop，不做解密、I/O 或睡眠；`FnOnce` 和一次性取走的状态保证上层 BIO 至多完成一次。

## 代码从哪里看

- `asterinas/kernel/comps/dm/src/completion.rs`：定义组件与内核之间的完成任务接口，并把下层状态与一次性任务绑定。
- `asterinas/kernel/src/device/misc/dm_completion.rs`：专用线程、无分配 intrusive 队列、唤醒和出队逻辑。
- `asterinas/kernel/comps/dm/src/device.rs`：dm-crypt 提交下层 BIO；回调只登记状态，worker 执行解密和上层完成。
- `asterinas/kernel/comps/dm/src/statistics.rs`：`lower_wait_ns`、`completion_queue_ns`、`crypto_ns`、`max_in_flight` 等计数器。
- `asterinas/tools/run_dm_crypt_bench.sh` 与 `asterinas/test/initramfs/src/init`：显式记录 `ioengine`、`iodepth`、`numjobs`，并拒绝把 sync+iodepth>1 误报为有效 QD。

## 为什么还有排队开销

QD1 五轮中位阶段占比约为：下层等待 95.02%、完成队列 2.77%、AES-XTS 1.86%。32-job 时变为：下层等待 80.72%、完成队列 18.89%、AES-XTS 0.31%。

原因很直观：32 个请求可以同时在途，但最终都由一个 worker 串行完成。并发把旧的中断上下文问题解决后，单 worker 立刻成为新的排队点。这也符合 Amdahl 定律：继续微调只占 0.3% 的 AES 路径几乎不会改善 32-job 总时间。

## 下一步有哪些优化方式

按当前证据，建议优先级如下：

1. **多 worker 或 per-CPU 完成队列。** 这是 32-job 最直接的优化，但正式测试当前只有 1 vCPU；应先增加 2/4 vCPU 合同，再测试 worker 数量，避免在单核上只增加切换成本。
2. **批量出队和批量完成。** worker 一次唤醒处理多个完成项，减少 wakeup、锁和调度开销；这是单核环境也可能有效的下一处单变量实验。
3. **VirtIO 多队列。** 当前驱动路径尚未形成完整 multiqueue 能力；完成队列扩展后，下层单队列可能成为瓶颈。
4. **请求合并与更大 I/O。** 4 KiB 请求的固定提交/中断/完成成本很高。合并相邻 BIO 或测试 16/64 KiB 能摊薄固定成本，但必须保留 4 KiB 主合同，不能换参数冒充代码优化。
5. **真正的异步用户 I/O。** 当前 fio `posixaio` 深度没有传递到 dm，应继续检查 VFS/AIO 到 block 层的串行点；修通后才能报告真正的单 job QD32。
6. **减少复制。** 可研究 page/segment 所有权转移和 scatter-gather 原地处理，但 dm-crypt 写路径不能破坏调用方明文 BIO，必须先证明安全语义。
7. **AES 优化优先级较低。** AES-NI 已在同一份串口日志确认，且实测只占 QD1 约 1.86%、32-job 约 0.31%；此时继续手工微调 AES 收益上限很低。
8. **dm-verity 单独优化。** 重点是 hash block cache、预取和同一 Merkle 路径的查找合并，不应与本次 dm-crypt worker A/B 混成一个改动。

## 证据与边界

所有正式运行均为 clean commit、`release-lto`、KVM、CPU=host、1 vCPU、8 GiB、virtio-blk、256 MiB、4 KiB read、2 秒 ramp、10 秒 runtime、五轮，并在同一份串口日志中确认 `backend=AES-NI`。

证据位于 `docs/evidence/formal-256m-worker/`：`before-q1`、`after-q1` 和 `after-j32` 均保存 metadata、串口日志、逐轮 CSV、汇总 CSV 和阶段 CSV；`q1-ab.csv` 保存严格合同核对后的 A/B。`before-j32-failed` 保存旧路径停在首个 dm-crypt 样本的日志，不能伪造为五轮 CSV。

答辩时建议这样说：我们先用阶段计时证明瓶颈不在 AES，再把重活从硬中断回调迁到预分配的专用 worker。QD1 吞吐基本持平，高并发从停滞变成五轮稳定完成；与此同时，计时数据诚实暴露了单 worker 的 18.89% 排队瓶颈，所以下一步是批处理或多 worker，而不是继续盲目优化 AES。

## 后续单变量实验结果

### 批量 wake/drain（1 vCPU）

提交 `6b3ca2407` 只改两点：队列只有从空变为非空时才唤醒 worker；worker 醒来后持续清空当前完成队列。相对单 worker 初版 `ed4557fa6`：

| 场景 | 配对 dm/raw 保持率 before → after | 归一化变化 | dm-crypt p95 变化 |
|---|---:|---:|---:|
| QD1 | 99.32% → 106.93% | +7.66% | -30.05% |
| 32-job | 92.63% → 99.18% | +7.07% | -26.79% |

宿主 raw 在 after 批次同时提高 19%--26%，因此不把绝对吞吐增幅全部归因于代码，只引用同批次逐轮配对 raw 后的结果。32-job 完成队列占比仍为 18.82%，说明批处理减少了 wake/wait 固定开销，但没有消除单 worker 串行排队。

### 每 CPU 一个 worker（4 vCPU）

提交 `9a3483571` 创建共享完成队列，并为每个 CPU 创建一个绑定 worker。相对同为 4 vCPU 的单 worker 批量版：

| 场景 | 配对 dm/raw 保持率 before → after | 归一化变化 | dm-crypt p95 变化 | 完成队列占比 |
|---|---:|---:|---:|---:|
| QD1 | 98.36% → 94.32% | -4.10% | -37.69% | 10.91% → 8.34% |
| 32-job | 91.72% → 95.80% | +4.45% | -43.62% | 29.15% → 6.55% |

结论不是“worker 越多越好”：多 worker 显著改善 32-job 延迟和排队，但 QD1 的 raw 归一化吞吐保持率下降 4.10%。因此 1 vCPU 正式主合同推荐批量单 worker；多 worker 保留在 `codex/dm-completion-pool` 实验分支，用于多核高并发场景，后续应研究按积压量启用 helper 的自适应策略。

证据分别位于 `docs/evidence/formal-256m-batch/` 和 `docs/evidence/formal-256m-pool/`。

## 对其余优化项的代码审计

- **VirtIO multiqueue：尚不能作为本轮小改动。** 当前 `DeviceInner` 只有一个 `VirtQueue`、一组 DMA request/response 区、一套 ID allocator 和 submitted-request map，并且初始化代码明确只注册 queue 0。实现 MQ 必须同时改队列选择、每队列 DMA/ID 状态、IRQ 注册、完成路由和错误恢复，属于独立驱动项目，不能和 dm worker A/B 混在一个提交里。
- **相邻 BIO 合并：底层已经实现。** `BioRequestSingleQueue` 会在类型相同、物理扇区连续且 segment 数不超限时调用 `merge_bio`。因此不能把“新增合并”当作本队新贡献；后续更有价值的是增加 merge hit/miss 计数，确认 dm 映射是否破坏连续性。
- **真正的单 job QD32：仍受上层路径限制。** fio `posixaio, iodepth=32` 到 dm 层实测 `max_in_flight=1`。在修复 VFS/AIO 到 block 的提交并发前，只能诚实报告 32-job 并发。
- **复制和 AES：暂不继续。** 32-job 多 worker 下 AES 占比中位数约 0.48%；在没有 page 所有权/明文安全证明前，不为小比例收益冒险做原地写加密。

从冲分角度，本轮建议主推已经有 clean release-LTO/KVM 五轮证据的 batch 和多 worker，不在答辩前把 VirtIO MQ、VFS AIO 和零复制三个高风险改造同时塞进主线。
