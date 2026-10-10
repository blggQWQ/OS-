# 星盾本地前后端 Demo

先运行 `demo/build-rust.ps1` 编译并测试 Rust 模型，再运行 `node demo/server.cjs` 后访问 http://127.0.0.1:3138。此构建脚本使用当前工作区隔离安装的编译器及 GNU 链接器；其他环境可用标准 Rust 工具链直接编译 `crates/dm/src/bin/protection_demo.rs` 为 `demo/protection-demo.exe`。

HTML/JavaScript 前端通过实际 HTTP API 调用 Node.js 后端；后端调用实际 Rust 进程核验设备身份、配置版本及逻辑租约。前端提供状态变化实验和 JSON 导出。启动全新服务后运行 `node demo/verify.cjs`，随后运行 `node demo/verify-protection.cjs`，可执行当前环境中的交互验证并生成截图。后者覆盖旧版本失效、同名重建、活动租约拒绝修改、独立设备修改和关闭释放。详细机制和论文参考见 `docs/protection_consistency.md`。

所有设备与映射均为模拟，任务保存在进程内，重启清空。没有真实加密、挂载、密钥管理或 Asterinas 内核接入；声明可信根的复选框不代表密码学验证。Node.js Demo 不等于计划中的 Rust Axum 服务。仅监听本机地址，尚未实现生产级鉴权及权限隔离。
