# Windows 0.3.2 发布

2026-10-21 15:34:07 HKT 完成发布。安装包 **4,899,996 字节**，安装包及应用 EXE 的 Windows ProductVersion/FileVersion 均为 **0.3.2**。

[下载 Windows 0.3.2](https://understandbook.top/downloads/UnderstandBookSetup.exe?v=0.3.2)

## 内容与来源

沿用 0.3.1 原生客户端，统一更新 package.json、Cargo.toml、Cargo.lock 和 tauri.conf.json 中的版本。原生源文件、权限和安装钩子与本轮前的 0.3.1 输入一致；复制前确认这些文件的修改时间早于 0.3.1 构建。来源记录在 source.json 和 build-verification.json。

内嵌 Reader 使用 0.3.1 发布时保存的网站前端快照，加上当时的 desktop.html、desktop-main.ts、DesktopLibrary.vue。旧安装包内嵌的网络状态代码与发布快照不同；本轮采用已上线的 network-context.ts/network-client.ts，排除工作区中未发布的阅读恢复变动。原生客户端版本之外的后端功能仍由云端提供。

云端地址为 https://understandbook.top。客户端加载当前线上 Reader，使用已经发布的 Search、Fetch、模型/搜索/网页读取分账与缓存优化。没有打包密钥、服务端程序、Runtime 或准备引擎。

## 验证结果

- 隔离候选 NSIS Release 构建通过；安装包和应用 EXE 版本均为 0.3.2。
- 真实 Windows WebView2 中，TXT、Markdown、EPUB、PDF 打开通过；删除原 TXT 后本机副本可读；断网重载后仍可读。进入云端前请求云端次数为 0。
- 客户端“进入云端学习”打开线上首页，点击登录进入 `/index.html?account=login`，实际加载本轮已发布的 `reader-DLfIHaHZ.js`；原生桥可用，返回本机书库后资料仍在。页面错误为 0。
- WebView2 验收复用候选的真实原生入口、命令及权限，仅把资料/浏览器目录指向隔离目录并隐藏窗口。没有操作用户正在使用的安装或资料。
- 公网带 `?v=0.3.2` 和不带查询参数的两个下载地址均 HTTP 200，内容与候选逐字节一致。
- 服务 PID 仍为 520445，没有重启服务或更改数据库。本轮模型、Search、Fetch 付费调用均为 0。

首次自动化探针误把官网首页容器当作 Reader 的 `#app`，导致等待超时；现场页面正常且无 JS 错误。修正为走首页登录链接后，以上验收通过。

## 发布与恢复

仅原子替换 `/opt/understand-book/releases/search-cache-20261021/downloads/UnderstandBookSetup.exe`。网站原有下载链接无需修改，已经指向新包。安装包备份：`/opt/understand-book/acceptance/windows-032-20261021/UnderstandBookSetup-0.3.1.exe`。回退只需恢复该下载文件；不涉及数据库或服务回退。

本机最终包为 `dist/UnderstandBookSetup.exe`；隔离源码与构建材料在 `tmp/windows-release-20261021`。源码快照有 node_modules 目录联接，清理时不可递归跟随联接删除依赖。

## 已知限制

- 本轮没有安装到用户现有应用上，因此没有声称完成安装向导和历史用户资料升级实测。
- Windows 内没有重复执行付费 Search/Fetch 问答；工具与三类费用的生产实测见 [本次服务端发布](../web-search-jsonl/release-20261021/README.md)。本轮确认实际 WebView2 接入同一已发布 Reader。
- 本机资料直接请求云端 AI、仅临时处理而不保存云端文件的链路仍未接通。现有 AI 准备走显式云端同步；此次版本未扩展该功能。

## 后续登录回调诊断

用户报告一次本机回调拒绝连接。补测完整浏览器授权、原生兑换和 WebView2 登录会话成功；用户重新从客户端发起后也确认成功。该次取消来源未确定，未改代码或重新发包。见 [登录诊断](login-diagnostic/README.md)。
