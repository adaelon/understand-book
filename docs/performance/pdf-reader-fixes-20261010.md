# PDF 阅读与翻译修复（2026-10-10）

本地分支 `codex/pdf-fixes`，基线 `8e3a76c3943b94c0ed1b1c1736fc2c67b409680c`。未部署，未替换书库原件。

## 根因与改动

| 问题 | 根因 | 修复 |
| --- | --- | --- |
| 已显示的当前页不能划线 | 全局 `geometryReady` 等待附近最多五页完成；邻页慢或失败会禁用全部文字层 | `PdfReaderPane` 按页解除文字层等待，停止过期计划的后续邻页工作 |
| 联网选区翻译返回未实现 | 多人 Host 未分派 `reader/selection.translate`，进入默认延后实现分支 | `PreparedTranslation` 短锁内校验选区，锁外接通模型及既有计费、额度、并发限制 |
| 翻译等待影响其他操作 | 前端临时翻译进入串行写队列；直接同步接模型还会占用普通 HTTP 工作线程 | 独立请求保留现场隔离，服务端复用受限等待线程，阅读和保存位置可并行 |
| 分段请求仍整本读取 | 后端读取整份 PDF 后由代理截取 | 授权后直接 seek/read 单段，返回 206、Content-Range 和 Accept-Ranges；合法越界返回 416 |
| 论文首屏本身数据过大 | 首页实际绘图引用 211 个图像对象，压缩图像流约 6.45 MB | 保留原件，另制图片降采样候选副本 |

翻译使用服务端生成的 `ReaderTask` 费用归属和 `selection_translation` 目的，不建立聊天或写入阅读成果。请求核验 attachment/generation，无需阅读 revision 在等待期间保持不变；身份和材料授权在执行前后复查。模型复用共享槽位、额度发送端口及 60 秒供应商超时。

Range 支持起止、开放结尾和后缀单段。未支持或语法无效的 Range 回退完整 200；未提供缓存验证器，带 If-Range 的请求返回完整响应；HEAD 保留完整长度。其他附件保持原行为。

## 真实 PDF 候选

样本：*A Survey of Agent Memory in the Second Half: Towards Self-Evolving and Long-Horizon Agents*，90 页。

| 指标 | 原件 | 优化副本 |
| --- | ---: | ---: |
| 文件字节数 | 16,504,342 | 12,629,020 |
| 第 1 页累计读取字节 | 8,508,950 | 4,699,164 |
| 第 1 页累计读取次数 | 43 | 28 |
| 第 4 页累计读取字节 | 11,720,214 | 7,648,284 |
| 前 5 页累计读取字节 | 11,720,214 | 7,713,820 |

文件减少 23.5%，首屏所需字节减少 44.8%。数据来自相同 PDF.js、初始 64 KiB、关闭流式和自动预取、顺序请求前五页的本地字节源，表示读取需求。

临时工具使用 PyMuPDF 1.28.2：超过 240 dpi 的图片降到 180 dpi、JPEG 质量 85，保留二值图像。工具仅装在本地临时目录。90 页的页尺寸、MediaBox、旋转、文字及词坐标全部相同；PDF.js 的 10,242 个文字项（文本、变换、宽高、方向、换行）逐项一致。原件与候选第 1、4 页均渲染并目视检查，正文、图表及页边界完整。

本地交付：`.worktree-local/pdf/agent-memory-reading-copy.pdf`。同目录 `optimization.json`、`pdfjs-validation.json` 保存比较结果；`.worktree-local/tmp/optimize_pdf.py`、`validate_pdf.mjs` 可复现。

## 验证

- 后端 `cargo test -p server pdf_ --lib`：34 项通过，包含真实 HTTP Range 和翻译计费联调。
- `mu4_selection_translation_reaches_the_configured_service` 与 `mu4_resources_authorize_before_head_range_conditionals_cache_and_revoke`：各 1 项通过。
- 前端 `PdfReaderPane`、`pdf-rendering`、`network-client`、`pdf-selection-translation`：共 47 项通过。
- 浏览器 `pdf-selection-actions`、`pdf-annotation`、`pdf-selection-translation` 及 `note-body-placement` 的 PDF/RE6 场景：22 项通过。覆盖邻页失败后真实划线、高亮、翻译、缩放、批注、刷新及视图恢复，包含桌面和手机。
- Web 类型检查、生产构建通过。

翻译联调使用真实 Host、授权、工作区、SQLite 额度账和模型 HTTP 适配器，仅供应商回复来自本地测试服务器。四个翻译同时等待时，阅读状态与 checkpoint 正常返回；四次调用分别结算，聊天数不变。另验证无权用户、过期现场、错误原文及额度不足；不足时供应商收不到请求。

运行前加载 `.worktree-local/environment.ps1`。浏览器命令从 `packages/web` 执行，配置为 `../../.worktree-local/playwright.config.ts`，前端端口 5174。批注测试旧桌面入口已改用当前“阅读工具 → 笔记”。

## 已知限制

- 未部署，未测本次修复在公网的端到端延迟。
- 限速测量预热了 PDF.js 引擎，使用冷 PDF 数据，统计页面绘制及文字提取，不包含登录、页面启动和文字 DOM 布局；单个真实样本不能代表全部 PDF。
- 取消后台绘制会阻止其继续占用画布处理；已经发出的共享字节请求仍由 PDF.js 完成和缓存，未实现跨页面强制中断共享请求。
- 优化文件是有损图像候选副本。正式采用须走既有材料发布流程并重新绑定 PDF 与来源映射，不能直接覆盖现有发布原件。
- 现有 MU4 历史夹具通过旧 `save_agent_history_path` 写事件型会话，会触发 `SESSION_EVENT_WRITE_REQUIRED`。相关资源与翻译测试已使用不含无关历史的同一授权夹具；整个 MU4 套件未通过。

## 保留原 PDF 的读取算法调查

读取调度已根据以下原件实验实现；原始 PDF 保持不变，阅读器不依赖图片优化副本。

参照项目：

- [Mozilla PDF.js 渲染队列](https://github.com/mozilla/pdf.js/blob/master/web/pdf_rendering_queue.js)：优先可见页、放大后的局部视图，然后处理滚动方向的邻页；支持暂停/继续。阅读器已将五页驻留集合与实际预渲染集合分开。
- [PDFium 按需加载接口](https://pdfium.googlesource.com/pdfium/+/refs/heads/main/public/fpdf_dataavail.h)：由页面可用性检查给出缺失字节区间，下载层处理重叠请求。PDF.js 已有按需和区间合并机制，不能把新增缓存视为必然收益。
- [Nutrient Document Engine](https://www.nutrient.io/guides/web/best-practices/performance/)：服务端渲染并向客户端提供结果，适用于大文档和弱客户端。可借鉴为原件之外的按页预览缓存，但属于另一条渲染链路，需承担文字层对齐与服务端存储/计算成本。

对同一未修改原件，以 PDF.js 6.1.200、禁止自动预取和流式下载，分别调整初始读取和 rangeChunkSize，提取第一页文字及绘图操作：

| 分段粒度 | 首页读取字节 | 请求次数（含初始读取） |
| --- | ---: | ---: |
| 64 KiB（原实现） | 8,508,950 | 43 |
| 256 KiB | 8,902,166 | 6 |
| 1 MiB | 10,212,886 | 4 |

256 KiB 以多读约 4.6% 字节换取约 86% 请求数下降。请求可并发，此数据不能换算为等比例耗时提升；实际浏览器限速结果见下方实施记录。

文字优先也有底层约束：[ObjectLoader](https://github.com/mozilla/pdf.js/blob/master/src/core/object_loader.js) 会递归预取资源流，文字资源键包含 XObject。临时复制 worker 并仅对文字预取跳过 Image 流，首页文字读取仍为 8,246,806 字节、42 次，仅省约 3.1%；634 个文字项及坐标一致。随后完整绘图仍需 8,508,950 字节。这个最小改法收益不足，不纳入应用。复制的 worker 仅留在临时实验目录，未修改 node_modules。

采用可见页优先和 256 KiB 分段。独立的按页预览与文字缓存暂未引入；现有正文范围映射能辅助定位，但不能直接当作完整 PDF.js 文字层替代。

复现：`.worktree-local/tmp/probe-pdf-reading.mjs`，参数 `baseline` / `text-skip-images`；结果为同目录 `reading-probe-*.json`。本轮不使用优化副本作为读取算法的输入。

## 读取算法实施

`planPdfRenderResidency` 返回五页驻留范围、当前可见页和滚动方向的一个邻页。`reconcileRenderedPages` 等同一可见页的在途任务完成，再处理邻页；用户跳页时取消未完成的非可见页绘制，已完成且仍在驻留范围的画布继续保留。画布总像素预算保持 12,000,000。

`pdfRangeSource` 将初始及后续请求粒度从 64 KiB 调整到 256 KiB，继续由 PDF.js 管理按需请求和已加载区间。服务端原生 Range 与身份校验沿上一轮修复。

同一原件、起始只显示第一页时，原五页预渲染 + 64 KiB 共请求 11,720,214 字节 / 58 次；当前可见页与一个邻页 + 256 KiB 共请求 8,902,166 字节 / 6 次，减少 24.0% 启动准备字节。这一数字包含后台准备，区别于上表单页需求。

本轮验证：组件、调度、Range、封面单元测试共 26 项通过；PDF 浏览器 23 项与完整响应/Range 两种封面场景 2 项通过；类型检查及生产构建通过。新增八页用例验证首次仅准备两页、向前跳页、向后预加载和五页驻留上限。

Chromium CDP 限速复测在其他测试/构建结束后单独执行：每种网络与分段组合三次，交替顺序，表内为中位数。引擎预热、PDF 数据冷启动，同一页面宽度 600 px；第 1、4 页的全部文字项/坐标及画布 PNG 直接相等比较均通过。

| 网络条件 | 64 KiB 首页绘制及文字提取 | 256 KiB 首页绘制及文字提取 | 缩短 | 64 → 256 KiB 跳到第 4 页 |
| --- | ---: | ---: | ---: | ---: |
| 10 Mbps，180 ms 延迟 | 13.784 秒 | 11.053 秒 | 19.8% | 5.365 → 5.109 秒 |
| 40 Mbps，40 ms 延迟 | 5.633 秒 | 4.728 秒 | 16.1% | 2.823 → 2.710 秒 |

分段复测单独比较传输配置；五页改为可见页加一邻页的收益由前述启动字节量和浏览器调度回归验证。初次探索同时比较过 1 MiB：请求数为 4，但首页读取增到 10.21 MB，未采用。

原始结果：[限速复测](pdf-loading-20261010/network-results.json)、[初次探索](pdf-loading-20261010/exploratory-results.json)、[启动读取量](pdf-loading-20261010/startup-demand.json)。本地复现：从仓库根运行 `node .worktree-local/tmp/benchmark-pdf-network.mjs --focused`。
