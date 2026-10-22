# Windows 本机资料与显式云端同步 — 2026-10-21

Windows **0.3.1** 与配套 Reader 网页已发布，2026-10-21 10:55 HKT 完成公网字节验证。安装包 4,899,391 字节（约 4.67 MiB）：[下载](https://understandbook.top/downloads/UnderstandBookSetup.exe?v=0.3.1)。

## 修正后的行为

- Windows 工作区默认“导入到本机”，复制所选文件到应用本地数据目录的 `local-materials`。不读取账号空间、不检查云端套餐，也不发送上传请求。
- 本机书库随安装包提供，无需登录和联网。TXT、Markdown、EPUB、PDF 可阅读原文，MP4 使用本机文件播放；PPT/PPTX 原件使用系统演示应用打开，可另存 PDF 后导入内置阅读器。
- 用户点击“同步到云端”才检查套餐和空间，再调用原有账号上传与准备接口。已保存原件与云端副本独立，云端拒绝新增上传不影响本机阅读。中断重试保留上传与任务标识。
- 网站继续显示明确的云端上传入口。Windows 0.3.0 会提示安装新版，本机存储不能通过网页更新补出。
- 原 Server、Runtime、Build Engine、运营发布与音视频处理程序未重新纳入正式安装包。

## 验证

| 路径 | 结果 | 证据 |
| --- | --- | --- |
| Rust 原生持久化与读取 | 6 项通过，包含重建 store 后读取、原文件删除后副本保留、工作区归属、同步标识复用、原文分段读取及原授权回归 | `cargo test -p understand-book-desktop --bin UnderstandBook` |
| Reader 相关测试 | 11 项通过，包括无套餐导入零请求、同步拒绝后可读、显式同步走既有准入、旧 Windows 提示、EPUB、工作区与授权回归 | `pnpm -C packages/web test src/local-materials.test.ts src/local-epub.test.ts src/NetworkApp.learning-workspace.test.ts src/desktop-auth.test.ts` |
| 类型与发行 | `typecheck`、本机 Reader、隔离网站 Reader、NSIS Release 构建通过 | `windows-build.log`、`local-build.log`、`web-build.log` |
| 实际 Windows WebView2 | TXT、MD、EPUB、PDF 打开；删除原始 TXT 后副本可读；断网重载后仍可读；云端请求 0，页面错误 0 | `webview2.json`、`probe.mjs`、`local-library.png`、`local-pdf.png` |
| 公网 Reader 三种入口 | 使用已部署静态文件及隔离 API/原生桥夹具，网站、0.3.0、0.3.1 三种入口通过；未改生产账号数据 | `public-ui.json`、`public-ui.mjs`、`cloud-workspace-local.png` |
| 公网交付 | 两份入口 HTML、7 个入口资源、完整安装包与候选逐字节一致 | `deployment.json`、`verify-public.mjs` |

首次原生阅读验证发现原 PDF 组件要求预构建映射，不能直接显示本机原件，见保留的 `failure.json`。修正为复用 PDF 分段读取和 PDF.js 的独立原文阅读器后，实际 PDF 渲染与离线读取通过。公网 HTML 验证排除 CDN 明确注入的统计脚本，再比较剩余正文；资源及安装包直接比较原始字节。验证模型调用 0。

## 发布与恢复

沿现有 `cloud-20261021` 仅更新 Reader 静态文件和下载包；没有重启服务、变更数据库、修改商品或费率。基于已上线前端快照覆盖本片涉及的 9 个源文件，未混入工作树其他工作线的变化，见 `source.json`、`prepare-frontend.py`。

服务目录：`/opt/understand-book/releases/cloud-20261021`。发布材料：`/opt/understand-book/acceptance/local-library-20261021`。旧入口 HTML 与 0.3.0 下载包备份在该目录的 `backup`。旧静态资源保留，必要时可恢复入口 HTML 与旧下载包；无需数据库回退。

## 已知限制

- 这次恢复本机文件保存和原文阅读。本机资料直接请求云端 AI、仅临时处理而不保留云端文件的链路尚未接通；现有 AI 准备仍走云端副本与准备服务。常驻准备宿主、正式商品与费率的既有配置缺口保持。
- 本机文件按导入时的副本保存，原文件后续编辑不自动同步。PPT/PPTX 不在本机执行格式转换或深度理解。
- 本轮实际原生阅读覆盖 TXT、MD、EPUB、PDF；MP4 使用已有 WebView2 视频播放能力与本机分段响应，未新增真实视频播放验收。
- 未覆盖安装到用户现有应用及历史本地书库批量迁入；没有操作用户正在使用的安装。已有云端占用不会自动迁走或清零。
