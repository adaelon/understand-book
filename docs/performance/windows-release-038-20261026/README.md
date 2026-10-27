# Windows 0.3.8 发布，2026-10-26

2026-10-26 **11:06:09 HKT** 已发布 Windows x64 安装包 **0.3.8**，共 **11,058,707 字节**。[下载最新版](https://understandbook.top/downloads/UnderstandBookSetup.exe?v=0.3.8)。官网原有无版本参数下载入口也已更新。

## 发布内容

安装包纳入本机书库与云端学习的中英文适配：原生菜单通过 `desktop_set_language` 同步界面语言，`CloudDesktop::navigation_url` 在本机／云端切换及登录后继续携带所选语言。内置阅读页面重新构建，保留本地资料导入、PDF 阅读与 OCR。

云端入口连接正式 `https://understandbook.top`，使用当天已上线的知识画板、公式输出、Harness 缓存机制与 LangSmith 修复等功能；这些云端功能的服务与浏览器证据见[合并发布记录](../combined-release-20261026/README.md)。桌面安装包仍为 Tauri/WebView2 阅读客户端，服务端运行程序不随包分发。

版本统一更新于 `apps/desktop/package.json`、`apps/desktop/src-tauri/Cargo.toml`、`apps/desktop/src-tauri/tauri.conf.json` 与 `Cargo.lock` 的桌面包条目。正式构建命令为设置 `UNDERSTAND_BOOK_CLOUD_ORIGIN=https://understandbook.top` 后执行 `pnpm -C apps/desktop package:windows`。

## 验证证据

| 范围 | 结果 |
| --- | --- |
| 生产打包 | 发布配置检查、内置阅读页面、Rust release 与 NSIS 构建完成；[构建日志](build.log) |
| 版本与文件 | 安装器和主程序均为 0.3.8；安装器 11,058,707 字节，未打安装标记的主程序 21,569,024 字节；[文件记录](build-verification.json) |
| 原生逻辑 | 7 项通过，覆盖浏览器授权／取消、图片保存、本机导入持久化、Range 读取、云端复制恢复、PDF 文本与批注；[测试日志](native-tests.log) |
| 桌面前端 | 4 个文件、8 项通过，覆盖本机书库路径选择、授权回调、账号切换及配置恢复；[测试日志](frontend-tests.log) |
| 安装内容检查 | 生成的 NSIS 文件区段仅复制 `UnderstandBook.exe`，本机阅读资源内嵌；外部运行程序与独立资源区段为空；采用当前用户安装模式，安装钩子保留用户资料及已有插件 |
| 公网下载 | 带 `?v=0.3.8` 与无参数两个入口均 HTTP 200，完整文件与构建候选逐字节一致；从服务器访问公开 HTTPS 验证；[发布回执](deployment.json) |

## 运行与恢复

本次只原子替换 `/opt/understand-book/releases/combined-20261026/downloads/UnderstandBookSetup.exe`。服务发布目录不变，前后 PID 均为 `162658`，没有重启服务或修改数据库。

旧包保存在 `/opt/understand-book/acceptance/windows-038-20261026/UnderstandBookSetup-0.3.7.exe`；候选及发布回执保存在同一目录。恢复下载时，将备份复制为当前下载目录中的 `UnderstandBookSetup.next.exe`，再原子替换公开文件，并比对两个公开入口。下载文件与历史发布目录可能存在硬链接，因此不要原地覆盖文件内容。无需恢复服务端或用户数据。

本地交付文件为 `dist/UnderstandBookSetup.exe`，冻结候选为 `tmp/windows-release-038-20261026/UnderstandBookSetup-0.3.8.exe`。

## 已知限制

本次未完成安装升级及真实 WebView2 启动／离线阅读验收。两次隔离验证进程启动分别使用本地调试端口、取消调试端口的方式，均被自动审批以 `blocked by policy` 拒绝，进程未启动；没有继续尝试启动，也没有运行安装器。上述构建、逻辑测试和公网下载检查不能替代安装与原生界面验证。[限制记录](native-verification-limit.json)。

本机 `E:\allwork\Understand Book\UnderstandBook.exe` 仍为 **0.3.7**，未自动安装 0.3.8；[本机版本记录](installed-before.json)。旧版本的 WebView2 验收仅作为历史证据，未计入本次通过项。
