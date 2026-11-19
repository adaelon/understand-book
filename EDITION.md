# 本地应用公共源码

这是双发行工作树的独立源码快照，基线提交为 `bd0d5057b2358a299e6c0159c4e3eb3dc0edc6d8`，源状态为 `working-tree`。准确状态见 `edition-source.json`；工作树快照不是已经发布的公共版本。

从本目录安装依赖并启动原创阅读样本：

```powershell
pnpm install --frozen-lockfile
pnpm demo:prepare
pnpm build
cargo run -p reader-service --bin reader -- --reader-only .understand-book/quickstart-demo
```

正式 Windows 安装包使用 `pnpm --filter @understand-book/local-desktop package:windows`。macOS 使用 `pnpm --filter @understand-book/local-desktop package:macos`，需在对应原生架构的 macOS 环境执行。导出时核对了本版的依赖归属。应用和命令行的构建、安装、签名与公证结果由各平台的实际验证记录提供。[本地版说明](apps/local-desktop/README.md)

公共 Book MCP 可执行文件使用 `pnpm build:book-mcp` 构建；其输出目录与目标架构由公共插件构建入口确定。

本目录包含公共 Reader、模型运行时、本地壳、构建插件、格式资源脚本与测试。云账号、控制库、网站和云客户端由云端目录持有。
