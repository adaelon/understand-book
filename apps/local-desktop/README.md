# Understand Book 本地版

本地版使用独立的应用身份 `app.understandbook.local`，与云服务客户端并存。应用在本机运行 Reader 和学习服务，阅读已有材料无需网站账号。书内问答可以使用桌面设置中的模型，也可以交给读者连接的本机 Agent。

## 安装与使用

Windows 安装包为 `dist/UnderstandBookLocalSetup.exe`。从开始菜单或桌面打开 **Understand Book Local**，首次选择 **进入书库**。学习首页会列出本机书库中的资料，附带的原创入门材料也在其中。再次启动时自动回到书库。

顶部可以直接切换 **学习、我的理解、学习资料、学习画板**。在 **学习 → 查看这本书** 中选择 **阅读原文**，有课程的书也可以选择 **跟着课程学**。**学习资料** 可以打开已有材料目录或导入 PDF。**我的理解** 可以查看某本书的理解与进展。**学习画板** 可以按资料新建画板，也能重新打开已保存的画板。进入阅读器后，点击顶部 **学习** 就能回到书库。

已有本机书库可以在首次启动时选择，也可以稍后在首页右上角打开 **桌面设置**，在 **书库位置** 下点击 **选择目录**。阅读时可从 **更多阅读工具 → 打开桌面设置** 修改书库位置和模型配置。

本地版与云客户端分别拥有安装目录、快捷方式、数据目录和安装注册表项。安装、升级和卸载只操作本地产品的程序资源；用户书库、私人学习记录和独立安装的插件保留。

首次启动也可以选择 **接续旧本地资料**。关闭旧 Local Debug 的所有窗口，选中旧包的 `data` 目录或旧应用数据目录，查看将复制的内容后确认。程序将设置、模型配置、笔记、聊天、学习状态、私人产物与未提交草稿复制到独立的 `imported-data` 目录，并实际启动副本后切换数据位置。外部书库仍引用原位置；旧目录内的自有书库随副本迁移。原目录保留，目标已有内容时拒绝合并或覆盖，重试会继续已经完成的同一份副本。

数据默认在系统当前用户的 `app.understandbook.local` 应用数据目录。Windows 为 `%LOCALAPPDATA%\app.understandbook.local`。`settings.json` 保存书库与模型设置，`memory` 保存阅读及学习记录，`private` 保存私人产物，`personal-note-drafts.json` 保存未提交草稿。选择独立导入目录后，应用的 `data-location.json` 记录当前位置。

启动失败会保留输入并显示原因，可重试或在同一页打开 `startup.log`。需要迁移或复验时，可显式设置绝对路径 `UNDERSTAND_BOOK_LOCAL_DATA_DIR` 使用独立数据目录；它不会改变云客户端或 Debug 产品的数据位置。

## 资料准备与可选工具

要在书中使用自己的 Agent，打开桌面设置的「我的 Agent」，连接或安装 OpenCode、Codex，也可填写自定义 ACP 命令，并将每个启动参数单独填写。测试连接后，圈选一段原文，在问答框旁选择这个 Agent 再提问。点击回答里的来源可回到对应段落。选择使用远程模型的 Agent 时，问题、选区和它实际读取的原文会交给该模型服务。登录凭证由 Agent 自己管理。

在终端使用个人学习 CLI、让外部 Agent 继续读书，以及从 ChatGPT Web 连接本机书籍，见[个人学习 CLI 与 MCP](../../docs/个人学习CLI与MCP.md)。

正式包带有本地 Node、PDF 读取、Canvas 和中英文 OCR 运行资源，基础材料准备不依赖开发机器的 Node/Bun。完整语义预构建沿独立插件和外部 Harness 执行。应用升级不会安装或卸载 Harness。

语音、视频解码、科学绘图与动画沿对应工具配置启用。需要这些能力时，把原有 `UNDERSTAND_BOOK_VOICE_PYTHON`、`UNDERSTAND_BOOK_VOICE_MODEL_DIR`、`FFMPEG`、`FFPROBE`、`UNDERSTAND_BOOK_PLOT_PYTHON` 等设置写入当前数据目录 `.env`；基础阅读不要求这些工具。插件 CLI 可通过 `UNDERSTAND_BOOK_CODEX` 明确指定。模型设置在 Reader 界面保存，优先于环境配置。

## 构建

在仓库根目录安装工作区依赖后执行：

```powershell
pnpm --filter @understand-book/local-desktop package:windows
```

构建入口调用 `scripts/prepare-resources.mjs`，分别生成公共 Reader 前端、Quick Start 副本和平台原生格式工具，再由 Tauri 生成 currentUser NSIS 安装包。`scripts/assert-release-config.mjs --resources` 验证本地产品身份与运行资源闭包。

macOS 在对应架构的 macOS 环境安装原生 Node 和依赖后执行 `pnpm --filter @understand-book/local-desktop package:macos`。arm64 和 x86_64 分别原生构建，不能把 Windows Node/Canvas 复制到 macOS 包。签名、公证和原生安装验收沿[双发行方案](../../docs/切片方案-开源本地应用与云端服务双发行.md)。

## 验证入口

```powershell
cargo test -p desktop-support
cargo test -p local-desktop-host --lib
node scripts/desktop/reader-runtime-smoke.mjs
node apps/local-desktop/scripts/assert-release-config.mjs --resources
python scripts/verify-personal-connection.py --cli apps/local-desktop/src-tauri/resources/bin/understand-book.exe
```

最后一条命令用原创演示书和独立临时配置，检查个人 CLI 与访客 MCP 的读写范围、宿主重启、来源位置和撤销凭证。先用 `pnpm demo:prepare` 生成演示书。macOS 指定包内的 `Contents/Resources/bin/understand-book`；加 `--pdf <本机论文路径>` 可从真实文件导入、查询新材料的元数据，并等待原文准备完成。

`desktop-support` 覆盖设置、草稿、旧数据副本与恢复；`local-desktop-host` 覆盖原生公共合同。格式工具 smoke 将随包资源复制到含空格、无开发 PATH 的独立目录，实际完成文字 PDF 和扫描 PDF OCR。安装后的原生窗口、模型设置、草稿与笔记重启恢复沿 `apps/local-debug/smoke.mjs` 的参数化入口，实际证据和仍未验收的平台见双发行方案。

在 Windows 测试账号安装 Setup 后，可将已安装的程序和真实旧 Debug 样本传给以下入口。每次使用新的输出目录。模型请求由第一个脚本的本机受控端点响应。

```powershell
node apps/local-debug/smoke.mjs --edition local --executable <已安装的UnderstandBookLocal.exe> --out <窗口验证目录>
node apps/local-debug/migration-smoke.mjs --executable <已安装的UnderstandBookLocal.exe> --source <已关闭的Debug数据目录> --expected-note <原笔记文字> --expected-answer <原回答文字> --out <导入验证目录>
```

升级时从开始菜单打开前一版，再运行新版 Setup。用户可以先正常关闭应用，再重试升级。静默安装及卸载在应用仍运行时返回 1618。升级后用迁移命令加 `--resume-data <前版建立的独立数据根>` 检查原书库、模型设置、笔记和对话。卸载完成后，本地版的程序、快捷方式和安装注册项应移除，书库、学习资料以及已有云客户端应保留。
