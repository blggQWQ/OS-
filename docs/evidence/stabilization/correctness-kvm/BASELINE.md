# KVM 正确性测试基线

- 日期：2026-08-10（Asia/Shanghai）
- 顶层提交：`09f62e5950ae36111de57f37becfdfdae27fcc69`
- Asterinas 提交：`9d79542cadddc6164ad3f31fd8cba776969d85a9`
- Docker 镜像：`asterinas/asterinas:0.18.0-20260618`
- 镜像摘要：`sha256:7510071cb103309cc9396b970325c354713da749dfee34a807108248cc97331e`
- Rust 工具链：`nightly-2026-04-03`
- 编译目标：OSDK `x86_64-unknown-none`
- 虚拟化：KVM，`-cpu host`

本目录仅保留 R01–R03、C01–C05、V02–V07 的正式串口证据；测试镜像继续保存在本机，不纳入仓库。
