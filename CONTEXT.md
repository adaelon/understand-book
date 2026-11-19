# 本地源码结构

- `packages/web`：共享 Reader、本地书库、模型设置与本地应用界面。
- `crates/reader-service`：单用户服务、Book MCP 和公共运行接口；`crates/runtime` 承担 Agent 与教学执行。
- `crates/memory`：本地学习记录；其他公共 crates 承担资料合同和读取。
- `crates/desktop-support`、`crates/local-desktop-host`：桌面持久化、数据导入和本地宿主。
- `apps/local-desktop`、`apps/local-debug`：正式与调试应用壳。
- `packages/core`、`skills`、`plugins`、`crates/maintenance-host`：资料准备、构建插件及维护 CLI。
- `scripts/plugin`、`scripts/desktop`：公共可执行工具和平台资源装配。

源码与数据分别管理，发布来源见 [EDITION.md](EDITION.md)，工具接口见[工具契约参考](docs/工具契约参考.md)。
