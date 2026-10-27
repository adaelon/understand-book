# 开书等待、加载状态与超时重试（2026-10-23）

## 改动与发布范围

点击书架资料后，原实现会先恢复旧现场并挂载旧 Reader，再打开目标资料。冷启动关键路径含重复 workspace GET；恢复旧 Reader 还会读取旧资料，书架的 PDF 封面同时继续加载。所有卡片变淡且没有进度、取消或超时提示，网络停滞时会持续等待。

本次改为恢复连接后直接打开目标，最终收据返回前不安装旧 Reader。开书期间暂停未完成的 PDF 封面任务；显示目标名称、加载状态和取消按钮。15 秒总超时包含写队列等待；失败恢复书架操作，并提供“重试打开”和“返回书架”。已提交但响应丢失的 create/隐式 attach fork 可以复用原现场，避免反复创建直到占满现场容量。同一资料及学习空间接续原位置和会话。

生产源码仅覆盖六个文件：

- `packages/web/src/NetworkApp.vue`
- `packages/web/src/network-client.ts`
- `packages/web/src/components/MaterialCover.vue`
- `packages/web/src/components/BookCover.vue`
- `packages/web/src/learning-library.css`
- `crates/server/src/workspace_registry.rs`

Web 从独立目录构建：Git `df019a38` 的 Web/Core 源码，加本次前端文件；相关原文件与线上音乐图片版本核对相同。Linux 候选从实际生产 release 复制，再覆盖上述六个文件，保留线上 Admin、下载资源和其他已发布功能。工作区中其他并行修改不包含在发布包内。生产构建入口为 `reader-atgBXsKO.js` / `reader-C5n8Veje.css`。

## 开书实测

正式对照使用同一 Windows / Edge 155.0.4283.62，1440×1000，真实线上 reader 资料，每种目标各三次。每次重建 App，预先保存指向另一资料的书架现场，点击同一书架卡片；双方均用本机提供完整对应版本的静态资源，API 与正文/视频仍访问线上源站。两组测量期间没有服务端编译。所有测试未调用模型。

网络路径为保留 `https://understandbook.top` Host/Origin 的 SSH CONNECT 至实际 HTTPS 源站；这是源站控制实验，不包含用户所在网络和 Cloudflare 链路。测试浏览器单独允许源站 Origin CA；正式站点证书配置未更改。

| 目标与测量终点 | 原版中位数 | 新版中位数 | 缩短 |
| --- | ---: | ---: | ---: |
| CS336 视频：书架退出、进入目标 Reader | 17.425 秒 | 4.570 秒 | 73.8% |
| AI agent：书架退出、进入目标 Reader | 19.539 秒 | 3.860 秒 | 80.2% |
| CS336 视频：实际首帧 | 42.266 秒 | 7.011 秒 | 83.4% |
| AI agent：正文可见 | 41.204 秒 | 4.657 秒 | 88.7% |

开书主指标全部样本（秒）：

- 视频原版：16.776、17.430、17.425；新版：5.247、3.912、4.570。
- 文字原版：19.539、19.956、16.908；新版：3.875、3.860、3.849。

点击之后的 API 数量由视频 38–42 次降至 23 次、文字 33–39 次降至 16 次；新版不再出现旧 Reader 的加载流量。冷启动不同资料的关键写入链由 `GET → attach → GET → book/open` 减为 `GET → attach → book/open`；已有连接切换资料只需一次 `book/open`。同资料同学习空间采用 `GET → attach`，不重复重置对话。

原始样本与汇总见 [comparison.json](comparison.json)、[baseline-local-browser.json](baseline-local-browser.json) 和 [candidate-clean-browser.json](candidate-clean-browser.json)。原版首轮与本机 Vite 构建有短暂重叠，保留该样本；后两轮无此负载，新版主指标仍低于全部原版样本。正文/视频终点受媒体加载及网络波动影响，原版离散较大，保留原始范围，不作为所有用户的固定耗时承诺。

先前探索分别使用远程旧静态与本机新静态，传输不一致，其数字不作为上表依据。上述正式对照统一了静态资源提供方式，并确认新版实际运行的是最终 clean build。

## 交互和回归验证

真实浏览器在 1500×1000 和 390×844 下完成“点击 → 加载 → 请求停滞 → 15 秒超时 → 重试 → 正确目标”整条路径。加载提示分别在 109 / 130 ms 出现；超时分别为 15.242 / 15.248 秒。超时后书架恢复可操作，重试成功，无脚本错误、无模型请求。已目视桌面加载条和手机超时条，书名换行正常、取消和重试按钮完整可见。

证据：[candidate-interactions.json](candidate-interactions.json)、[桌面加载](opening-desktop.png)、[手机超时与重试](timeout-mobile.png)。

- 前端 99 项相关测试通过：开书协议 45、UI/接续/学习空间/封面 33、聊天/恢复/回顾/MCP 21。
- 服务端 `mu5_` 范围 26 项通过，含 5 个新重试用例；覆盖响应丢失、容量限制、多标签页、接管、detach、重启恢复、checkpoint/CAS 和撤销授权。
- Web 类型检查及独立生产构建通过。
- 独立协议审阅未发现阻止上线的问题。取消立即停止界面等待并屏蔽迟到响应；已经在服务端提交的操作会在下次打开时核对恢复。

验证摘要为 [validation.json](validation.json)，服务端日志为 [server-regression-tests.log](server-regression-tests.log)，独立 Web 构建日志为 [web-build-clean.log](web-build-clean.log)。本次 11 个生产/测试文件的差异保存在 [opening-source.patch](opening-source.patch)，不含工作区其他任务修改。

## 上线记录

2026-10-23 **17:43:11 HKT** 已发布 `/opt/understand-book/releases/book-open-20261023`，服务 PID **691209**，维护 **12.18 秒**。原 release 为 `music-score-images-20261023`，备份为 `/opt/understand-book/backups/book-open-20261023`；可变数据压缩备份 14,723,259 字节。数据根继续为 `/opt/understand-book/data/multi-reader-to-tb-20261012`、schema **20**。

切换前后 reader 的 15 项、admin 的 16 项授权资料版本完全相同；正文/manifest 与展示制作能力均通过。候选后端独立预检验证 create 和隐式 attach 各重放 6 次，始终保留两个独立现场，两边都能阅读；模型调用数为 0。发布收据见 [deployment.json](deployment.json)，独立预检见 [preflight.json](preflight.json)。

上线后浏览器重新读取正式 `index.html`、所有静态资源及 API，断言实际加载 `reader-atgBXsKO.js`，文字/视频各一次均通过。视频点击后 8.739 秒进入目标 Reader、11.837 秒出现首帧；文字 7.438 秒进入 Reader、8.374 秒正文可见。目标标题、正文与视频正确，脚本错误和模型请求均为 0，测试现场已释放。此轮静态资源也经过 SSH 源站链路，未使用本机静态替换；它是完整部署冒烟实测，与上表统一静态资源的三次对照分别记录，见 [published-browser.json](published-browser.json)。

随后通过 SSH CONNECT 转发至正式 Cloudflare 域名，开启正常 TLS 验证，完整正式资源的浏览器验收也通过两项：视频 11.931 秒进入 Reader、35.714 秒首帧；文字 11.909 秒进入 Reader、14.725 秒正文可见。仍为每类一次，无脚本错误和模型请求。这验证了正式域名/CDN/源站整条路径，不是用户本地直连测量，见 [published-public-browser.json](published-public-browser.json)。17:47:29 HKT 最终服务为 `active/running`、`NRestarts=0`，PID 691209，实际执行文件与新 release 一致，见 [service-final.json](service-final.json)。

Linux optimized 构建耗时 8 分 38 秒，server-only、`-j1`、codegen-units 64、CPU 100% / MemoryMax 2400 MiB；临时 1 GiB swap 已关闭并清理。首次预检在业务检查通过后遇到系统 Python 的 SQLite 3.34.1 不支持现有 `STRICT` schema；验收脚本改用项目既有 Node 24.20.0 / SQLite 3.53.4 只读连接，重新完整预检通过，未修改数据库。

部署脚本在切换前核对原 release、schema 20 和活动任务，先维护再停服，备份可变数据及 service/nginx/env/sandbox 配置；对照前后账号、授权资料与阅读健康，再恢复流量。无迁移。对外恢复前失败自动恢复旧配置与旧服务；恢复流量后的回退必须保留新用户写入，只回退相容的程序和配置，不覆盖当前数据根。旧 release、静态资源、下载及备份保留。

## 已知限制

本机直接访问公网域名的浏览器登录请求曾超时，服务器早期非浏览器探测曾返回 403；后来经过 Cloudflare 的真实浏览器验收通过。保留 [direct-https-unavailable.json](direct-https-unavailable.json)。所有成功的浏览器性能测量均经过 SSH 转发，不能表述为用户本地直连耗时。

正式域名测试中视频首帧仍有较长等待：开书请求 `book/open` 仅约 0.55 秒，首个 workspace GET 等响应头约 9.88 秒；视频元素在 12.729 秒出现后，首段媒体请求于 14.053 秒发出、16.260 秒收到响应头，下一段到 35.344 秒才发起。后段主要位于首段媒体传输/解复用/解码窗口。已有记录未区分响应体下载与媒体处理，不能单独归因于 Cloudflare，亦不声称这部分等待已经消除。详见 [published-public-timeline.json](published-public-timeline.json)。

本次没有数据库迁移；后台线程、账号授权、资料与课程内容沿用生产版本。Windows 安装包继续为已发布的 0.3.7，本次更新的是网站。
