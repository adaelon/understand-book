# Understand Book 本地 Debug

Windows 本地阅读器和 AI 运行引擎使用同一进程。窗口连接本机 `127.0.0.1`，不需要部署或连接 Understand Book 线上业务服务器。模型请求使用你配置的接口。

## 启动

在 `dist/UnderstandBookLocalDebug` 中双击 **Start.cmd**。保留同目录的 `resources`、`node_modules` 和 `understand-book-build.exe`。关闭应用窗口后本机后台一起退出。

首次打开会显示“选择阅读材料”，默认扫描打包时源码目录内的 `.understand-book`，从列表中选择材料即可。也可展开“通过目录打开 / 查看材料位置”，选择另一份预构建材料的目录（内含 `base.json` 和 `source.txt`）。进入阅读器后点击右上角“更多阅读工具” → “打开桌面设置”，填写模型服务地址、模型名称和 API Key，点击“保存并应用”。设置也可以切换整个书库位置。新材料通过书库的新建入口准备；模型未配置时仍可阅读已有材料。

启动脚本把独立数据保存在包内 `data`：

- 默认书库：打包时源码目录内的 `.understand-book`，位置记录在 `settings.json`；原书保留在该目录。
- `memory`：阅读记录、笔记、会话。
- `personal-note-drafts.json`：尚未提交的个人笔记草稿，包括待确认保存和放置进度；本机服务端口变化后仍可恢复。关闭窗口会先等待草稿写入，写入失败时保留窗口并提示重试。
- `private`：本地构建意图和私有产物。
- `settings.json`：书库位置和模型配置（API Key 明文存储）。
- `webview`：本地窗口配置。
- `debug.log`：运行日志。
- `debug-session.json`：运行期间的本机地址和进程号，可用浏览器访问该地址调试。

直接运行 EXE 时，数据默认位于 `%LOCALAPPDATA%/app.understandbook.local-debug`；也可通过 `UNDERSTAND_BOOK_DEBUG_DATA_DIR` 指定绝对路径。模型环境变量可写入数据目录的 `.env`，格式见 `model.env.example`。设置界面保存的配置优先。

本机已有仓库 `.env` 时，执行 `node apps/local-debug/configure-local.mjs`，把模型接口和本地语音识别配置接入启动脚本使用的数据目录，再重启应用。可传入另一数据目录作为第一个参数。此操作只写本机配置；打包流程不调用它，分发压缩包不包含密钥。语音输入需要有效的 `UNDERSTAND_BOOK_VOICE_PYTHON` 和 `UNDERSTAND_BOOK_VOICE_MODEL_DIR`；仅编译程序不会自动启用语音识别。

该脚本也接入以下本机依赖，可从仓库 `.env` 或当前进程环境变量读取；已有配置在重新打包时保留：

| 功能 | 配置 |
| --- | --- |
| 视频字幕、音频复听 | `VIDEO_WHISPER_PYTHON`、`VIDEO_WHISPER_MODEL_DIR`（本地 small.en） |
| 视频解码、抽帧 | `FFMPEG`、`FFPROBE`，未指定时使用 PATH |
| 科学图表 | `UNDERSTAND_BOOK_PLOT_PYTHON`，需要 Matplotlib |
| 动画 | `UNDERSTAND_BOOK_ANIMATION_PYTHON`，需要 `scripts/presentation-animation-requirements.txt` 中的 Manim 0.21.0 |
| 网页演示预览 | 默认发现本机 Edge，也可指定 `UNDERSTAND_BOOK_PREVIEW_BROWSER` |

数学公式动画还需要本机安装 TeX Live。扫描 PDF 的 Node、Canvas、OCR worker、WASM 和中英文识别数据随包提供，识别过程不下载模型。

## 开发

仓库根目录执行 `node apps/local-debug/package.mjs`，构建当前工作区前端（带 source map）、Rust debug 可执行文件和本地构建引擎。需要已安装项目依赖、Rust MSVC、Bun。运行需要 Windows WebView2。可通过 WebView 的开发者工具调试页面。

本地入口：`src-tauri/src/main.rs` → `local_desktop_host::run` → `reader_service::host::start_server` → `packages/web/src/LocalApp.vue`。设置、草稿、插件发现与图片导出由公共 `desktop-support` 维护，Debug 与正式本地版使用同一本地壳；本机服务随窗口退出而关闭。正式安装产品见[本地版](../local-desktop/README.md)，原有云端桌面发行入口独立保留。

## 验证记录（2026-11-13）

前端类型检查、带 source map 的本地前端构建、Rust debug 构建及 17 项原生测试通过。`node apps/local-debug/smoke.mjs` 使用独立目录和本机模拟模型端点验证真实 WebView 中的设置表单、模型 HTTP 请求与完成答案、笔记持久化、重启恢复，以及关闭窗口后服务停止；输出 `READY_FOR_CLOSE` 时关闭对应测试窗口，脚本继续第二轮。两轮页面重载均未产生外部 HTTP 请求（Tauri 本机 IPC 除外）。付费模型和完整新书预构建未在这轮测试中调用。

Windows debug 主线程保留 16 MiB 栈空间，解决未优化代码在恢复已保存会话时触发默认 1 MiB 栈溢出的问题。

后续功能排查已修复扫描 PDF 的运行资源缺失、视频转写绑定到结构节点导致发布失败、外部参考链接被窗口拦截，以及本地设置页无效的旧插件安装入口。设置页现在显示随附构建引擎状态。真实图表、动画、文字 PDF 和扫描 PDF 导入、视频转写及重启持久化已复测；详细范围和证据见 `docs/evidence/local-debug-20261113/README.md`。

## 已知限制

这是当前项目的本地功能入口：阅读、书库、笔记、AI 对话、教学及画板使用本机服务。线上账号、订阅套餐、邮件、线上共享书库及多用户管理属于云端功能。新材料的完整自动预构建仍需要项目已有的执行器及对应格式工具；附带构建引擎不替代 Codex 执行器、语音模型、Python 或媒体工具。已有预构建书库可直接打开。

联网范围由所用功能决定：AI 使用配置的模型接口，主动打开外部资料或网页研究需要网络。默认关闭 LangSmith 观测。此包不含你的模型密钥、私人原书或线上账号。
