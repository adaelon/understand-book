# 本地应用公共源码

Windows 本地版 `v0.3.14` 基于已发布的 `v0.3.13`（提交 `5b3d4ebf26d38d46f0940eaf293ceec800348cc7`），修复阅读页顶栏和 EPUB 书名显示。源码基线与装配信息见 [edition-source.json](edition-source.json)。

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
