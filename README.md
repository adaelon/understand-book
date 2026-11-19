# Understand Book 本地版

在自己的电脑上阅读材料、提问、学习和保存笔记。正式应用使用独立本地数据目录，已有材料可以直接阅读；AI 功能使用自己配置的模型服务。

## Quick Start

准备 Node.js、pnpm、Rust 和平台构建工具后，在此源码目录执行：

```powershell
pnpm install --frozen-lockfile
pnpm demo:prepare
pnpm build
cargo run -p reader-service --bin reader -- --reader-only .understand-book/quickstart-demo
```

服务启动后打开终端显示的本机地址。Windows 安装包和 macOS 构建入口见[本地应用说明](apps/local-desktop/README.md)，调试壳见[Local Debug](apps/local-debug/README.md)。

## 使用与开发

- [个人学习 CLI 与 MCP](docs/个人学习CLI与MCP.md)
- [模型服务配置](docs/模型配置使用说明.md)
- [学习材料下载、来源与范围](materials/README.md)
- [构建插件与执行器](plugins/understand-book/README.md)
- [工具契约参考](docs/工具契约参考.md)
- [源码结构](CONTEXT.md)、[贡献与验证](CONTRIBUTING.md)
- [本次源码版本与构建说明](EDITION.md)

## 许可与材料

项目代码沿用 [MIT 许可证](LICENSE)。内置第三方资源保留各自许可：[Three.js](assets/presentation/three-0.180.0/LICENSE)、[Konva](assets/presentation/konva-10.7.0/LICENSE)、[齿轮示例 Three.js](examples/gear-transmission/vendor/THREE-LICENSE.txt)。

下载资料独立于源码分发。原书、论文、乐谱和录音保留各自来源与权利说明；代码许可证不代替材料许可。详见[材料来源](materials/README.md#材料来源)。
