# 保护状态一致性：当前实现与边界

核心实现位于 `crates/dm/src/protection.rs`。`Configured` 经策略核验产生不可复制的 `Checked`；激活消费句柄，返回逻辑租约 `Mounted`。核验绑定依赖图各对象的唯一身份及配置版本。激活在同一注册表锁内先复核全部依赖，再固定引用，失败不产生部分固定。相关对象在租约活动期间禁止修改或移除；关闭或析构释放本租约引用。

这是用户态控制模型，不执行 Asterinas 内核挂载、加密或可信根认证。根哈希为调用者提供的模型值。保证依赖可信注册表，且所有配置变更经过受控接口；无法阻止绕过接口的内核修改。Node.js 通过标准输入输出调用实际 Rust 二进制，设备信息为模拟数据，正式 Axum 服务待实现。

验证：12 项 Rust 测试通过，含 100 次双线程竞争；覆盖同名重建、版本失效、策略缺失、依赖环、共享依赖、关闭释放、无关设备可改以及失败无部分引用。HTTP/浏览器验证复现 Stale、Busy 和关闭后恢复修改；记录见 `demo/protection-verification.json`。这些结果不是模型检查、形式化证明或真实设备性能数据。

论文启发：

- [SquirrelFS, OSDI 2024](https://www.usenix.org/conference/osdi24/presentation/leblanc)：类型状态约束顺序；本文模型处理保护流程，不声称同等崩溃一致性保证。
- [Capsicum, Security 2010](https://www.cl.cam.ac.uk/research/security/capsicum/)：对象能力；本文模型绑定依赖对象身份，不等同于完整权限隔离。
- [TxOS, HotOS 2009](https://www.usenix.org/legacy/event/hotos09/tech/full_papers/porter/porter_html/)：原子操作启发；本文仅在单注册表临界区复核与固定，不提供系统级事务。

后续需把对象身份、版本更新和租约生命周期接入真实 Device Mapper 路径，验证所有重配置入口是否受约束；补充故障注入及与现有设备管理方案的对比，再评估创新优势。
