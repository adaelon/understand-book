# Linux 阅读布局与首屏加载

## 范围与步骤

沿用现有阅读工作区、账号、材料版本、专注阅读和标注定义，修复桌面布局和网络首屏，无领域模型变更。

1. 顶部账号操作进入菜单；桌面工具栏可收起并随专注阅读收起；统一剩余高度，目录滚动且进度固定。验收：桌面、短屏、手机实际元素边界与操作。
2. 独立材料请求并行；首屏复用创建/恢复的阅读状态；正文不等标注和配置；只读请求脱离写队列。验收：延迟请求测试、读取与写入隔离、现场授权回归。
3. 版本静态资源缓存和压缩。验收：前端构建、Nginx 配置及 HTTP 响应；私人 API 与 HTML 不缓存。

## 实现

- `#app` 持有窗口高度，网络外壳内的 Reader 使用剩余高度。账号和退出操作位于工具栏菜单；提交待核对及错误有事件才显示。附属演示保留浮动账号菜单。
- 桌面工具栏提供“收起”，收起后右上角“菜单”可恢复；进入专注阅读自动收起。目录使用 flex 剩余高度，只有目录列表滚动，进度固定底部；论文地图与调试内容各自限制高度。
- `App.init` 并行请求清单、资源、来源身份与 PDF 信息，首屏复用 `NetworkWorkspace.reader`。阅读配置、标注、历史、画像不阻挡正文；保存为文字模式时 PDF 信息也不阻挡。默认 PDF 模式保留必要映射等待，异步结果受启动序号约束。
- `workspaceRead` 一次 POST 读取，不预读 revision、不进入写队列、不用读取回执覆盖更新的工作区。服务端只对白名单读取省略 revision 相等要求；账号、材料授权、generation 和 attachment 校验保留。写入继续 CAS。
- Nginx 对 `/assets/` 提供 gzip 和一年 immutable 缓存，补齐 PDF worker `.mjs` JavaScript MIME。HTML 与私人 API 仍 `no-store`。

## 验证与上线

- 前端相关单测 30 项通过：TopBar 4、ReaderWorkspace 10、network-client 9、App.startup 7。新增延迟测试先复现失败，再通过；包括正文不等标注/配置、清单并行、网络状态复用、文字模式不等 PDF、读取不回退较新的写入版本。
- Rust 相关测试 15 项通过：MU5 11、MU8 4（含本次 MU12 授权与读取测试）。
- 网络版浏览器 8 个场景分批通过：短屏进度、收起/专注、账号切换、丢失回执、双窗口刷新、附属演示、PDF、标注。另有桌面、手机竖屏/横屏、手机 WebKit 共 4 个布局场景通过。整组短时间登录触发原有每账号每分钟 5 次限制，分批复跑；旧测试的退出/注销入口已按展开菜单更新。
- Vue 类型检查、Vite 网络构建和 Linux release 构建成功。Nginx 配置校验成功。
- 2026-10-01 上线 `/opt/understand-book/releases/mu12-20261001`，切换前运行/未保存问答数为 0。旧 release 保留，原配置保存在 `/opt/understand-book/backups/mu12-config-20261001`，未迁移或覆盖私人数据。回执见 [部署记录](linux-reader-layout-startup-20261001/deploy.log)。
- 公网实际登录、正文、刷新、收起/展开、手机目录、退出均成功；真实 Transformer 论文 PDF canvas 和文本层可见，无页面异常。响应头见 [PDF 与缓存验证](linux-reader-layout-startup-20261001/production-headers-pdf.json)。

## 现网前后对照

同一网络、同一本 `ai-agent-engineering`、1280×560 窗口，每个版本各一次独立 Chromium 测量。

| 指标 | 改前 | 改后 |
| --- | ---: | ---: |
| 冷启动进入登录页 | 8.819 秒 | 2.764 秒 |
| 主 JS 传输量 | 1,904,113 字节 | 697,608 字节 |
| 主 JS 下载时间 | 7.722 秒 | 2.593 秒 |
| 选书至正文可读 | 3.004 秒 | 2.545 秒 |
| 刷新至正文可读 | 16.598 秒 | 2.855 秒 |
| 刷新主 JS 网络传输 | 未缓存 | 0 字节 |
| 560px 窗口中进度块底边 | 642.03px | 544px |

手机 390×844 的进度块底边为 831.20px，位于窗口内。原始记录：[改前](linux-reader-layout-startup-20261001/production-before.json)、[改后](linux-reader-layout-startup-20261001/production-after.json)。截图：[展开](linux-reader-layout-startup-20261001/desktop-expanded.png)、[收起](linux-reader-layout-startup-20261001/desktop-collapsed.png)、[手机目录](linux-reader-layout-startup-20261001/mobile-outline.png)。

## 已知限制

后续 MU13 已补充刷新逐请求根因对照、API JSON 压缩、HTTP/2 和 PDF 按需读取；当前刷新表现以 [MU13 记录](linux-reader-refresh-root-cause-20261001.md) 为准，下方保留 MU12 当时的测量边界。

- 这是各一次公网对照，网络下载波动会影响时间，不代表用户设备的固定时延；选书至正文仍约 2.55 秒。
- 本轮沿用现有按段正文和公式请求，没有新增正文批量接口；既有目录标题与历史来源优化保留。
