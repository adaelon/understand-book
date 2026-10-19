# FP2 有界正文分页与原文映射

2026-10-14。状态：**FP2 本地完成，FP3 可开始**。34 项单元/组件、11 项短浏览器场景与 20 分钟/583 次正文资源验收通过。依据：[ADR-0166](../../adr/0166-focus-reading-pagination-and-performance.md)、[切片方案](../../切片方案-专注阅读双页与无感翻页.md)。源码基线 `6612932` 加当前工作区本片增量；未提交、未部署。

## 实现与接入

`FocusReaderSurface → ReaderPagination.prepare(规范游标, 几何) → PreparedScreen → show(screen)`。

| 文件 | 职责 |
| --- | --- |
| [reader-pagination.ts](../../../packages/web/src/reader-pagination.ts) | 当前/相邻/准备屏的局部分页、规范范围、资源核算、回收和重排 |
| [FocusReaderSurface.vue](../../../packages/web/src/components/FocusReaderSurface.vue) | 生命周期宿主；卸载释放页面、缓存和待完成读取 |
| [reader-pagination-render.ts](../../../packages/web/src/reader-pagination-render.ts) / [样式](../../../packages/web/src/reader-pagination.css) | 复用 Markdown、KaTeX、排版字体；宽对象页内滚动、列表续号和片段呈现 |
| [markdown-cooperative-alignment.ts](../../../packages/web/src/markdown-cooperative-alignment.ts) / [reader-cooperative.ts](../../../packages/web/src/reader-cooperative.ts) | 线性工作空间来源对齐，每约 4ms 让出任务；DOM 准备批次之间也让出 |
| [markdown-source-map.ts](../../../packages/web/src/markdown-source-map.ts) | 复用 DOM 语义投影和纯对齐结果；原同步选择 API 保持原行为 |
| [分页生产夹具](../../../packages/web/playwright/fixtures/focus-pagination.ts) / [场景](../../../packages/web/playwright/focus-pagination.spec.ts) | 真实导入源、跨窗口往返、源范围、重排帧、资源长测 |

分页器以一个来源版本的只读 `read(index)`、正文渲染器及字体/图片 ready 回调为输入。游标为叶序、可选 LID 与规范 UTF-16 偏移；`DisplayPage.fragments` 持有原文范围和语义范围。调用者按已知来源游标请求相邻屏，分页器不持有全书页码表或导航历史。正式 App 的读窗口、预取、解释资料与已读同步在 FP3 接入；正式专注按钮/手势在 FP4 接入。

每页使用固定高度原生多栏测量，按叶和段内语义前缀递增。长段候选每次增加 768 个语义单位，落在公式中的截点延伸到公式边缘；截取渲染 DOM 保留强调、列表和数学上下文。隐藏 Markdown 语法归入连续来源边界，零矩形尾空格仍归前一片。分页范围拼接覆盖完整规范文本，正文选择范围继续排除非显示语法。

已准备屏保持独立 DOM；`show()` 只切换可见、`inert` 与无障碍状态，不读正文、不测几何、不重新分页。准备区占用四屏之一；达到限额时回收非当前屏。释放句柄同时清空 DOM 子树和页片段引用，源字符串、HTML、语义文本和映射随最后一个使用者释放。对齐缓存只保留纯数据。

重排围绕当前或指定规范锚点准备新屏，成功后一次切换并释放旧几何。准备失败保留原屏；未完成读取在组件卸载后不能重新挂载 DOM。列表续接保留原序号，跨页继续的同一条目隐藏重复标记；代码、宽表与显示公式按完整对象保存来源，保留页内滚动。

## 运行命令

从仓库根运行；材料生成复用 FP1，不改动 FP1 历史证据。

```powershell
node packages/web/playwright/fixtures/prepare-focus-reading.mjs
pnpm --filter @understand-book/web build
pnpm --filter @understand-book/web exec vue-tsc --noEmit -p playwright/tsconfig.focus-pagination.json
pnpm --filter @understand-book/web exec vite build --config vite.focus-pagination.config.ts
pnpm --filter @understand-book/web exec vitest run src/reader-pagination.test.ts src/markdown-source-map.test.ts src/reader-pagination-budget.test.ts src/components/ReaderPane.test.ts src/useReaderSelection.test.ts
pnpm --filter @understand-book/web exec playwright test --config playwright.focus-pagination.config.ts --grep-invert 'long reading' --reporter=line
$env:FP2_LONG='1'
pnpm --filter @understand-book/web exec playwright test --config playwright.focus-pagination.config.ts --grep 'long reading' --reporter=line
node packages/web/playwright/fixtures/summarize-focus-pagination.mjs
```

独立生产 bundle 位于 `tmp/focus-reading-fp2/dist`，默认端口 4189，禁止复用未知服务。`FP2_OUTPUT` 可指定独立证据目录，`FP2_PORT` 可为并行的独立测量选择端口。长测以真实经过时间计满 20 分钟，不能用压缩时间或快速循环替代。

## 材料、口径与结果

沿用 [FP1 材料与机器](../focus-reading-fp1-20261014/README.md#材料与环境)：真实 EPUB、AI Infra Book 与 Kimi K3；额外覆盖长段 `stress.long`、60 项有序列表、100 行宽代码和六张本地 SVG。桌面 1440×900、移动视口 390×844；重排另覆盖 1024×900、844×390 与 320×844；19/22px，Noto Serif SC。性能结果只代表本机 Chromium 生产夹具。

34 项单元/组件测试和 11 项短浏览器场景通过；正式生产构建、独立夹具构建和类型检查通过。来源验证逐页核对规范偏移相接、渲染语义相符；反向跨越已回收窗口后，重建范围与原屏相同。选择回归同时验证非当前屏保持不可交互。图片解码后的尺寸、代码第 99 行与横向尾部实际可达。

来源对齐的真实长链接 `9.14.6.17` 首次异步计算约 21.4ms，独立准备场景未观察到 ≥50ms 主线程任务。合成公式长段的对齐总经过时间仍可达到约 396ms，但已分批让出；不把整个 Promise 的经过时间当成单次主线程任务，也不把冷准备时间当成正常翻页延迟。

P4 分开记录 `readyMs` 与重排后首次变化的浏览器捕获帧：通过 CDP screencast 的帧时间戳配对已发布新屏，保存最后一组可读帧。物理呈现时间为 `null`。PNG 编码/传输完成时间的保守上界另保留在失败记录，不作为实际画面时间戳。

P4 每种材料/宽度各 20 次热重排；以下为新内容首次变化的浏览器捕获帧，所有材料分别满足 P95 ≤100ms。保留每次样本、ready、帧时间戳与实际画面：[桌面](results/reflow-1440.json)、[移动视口](results/reflow-390.json)。

| 视口 / 材料 | 新内容帧 P95 | 最大值 |
| --- | ---: | ---: |
| 1440 / EPUB | 49.2ms | 64.1ms |
| 1440 / 复杂材料 | 59.0ms | 355.6ms |
| 1440 / Kimi | 73.4ms | 76.9ms |
| 390 / EPUB | 21.6ms | 39.3ms |
| 390 / 复杂材料 | 21.9ms | 38.0ms |
| 390 / Kimi | 21.8ms | 22.2ms |

桌面合并样本 ready P95 为 49.2ms、浏览器新内容帧 P95 为 71.1ms；移动视口分别为 7.5ms、21.9ms。桌面保留一次 355.6ms 尾部样本，不删样本或以平均值替代。捕获帧可能晚于页面已经画出的时间，仍按实得时间保留；本表不是物理显示延迟。

P5 正文长测实际经过 **1,200,737ms（20 分钟）**，完成 **583 次有效换屏**；覆盖 AI Infra Book 第 4–5 章样本、末尾返回、反向与每 100 次字号变化。每次换屏同时断言资源上限，峰值记录包含准备区和候选 DOM，非只采最终驻留。测试在同一分页器实例中运行，未通过定时重建实例清空累计状态。见 [原始结果](results/long-reading.json)、[汇总](results/summary.json)、[运行日志](long-reading-tests.txt)。

| 资源 | 长测峰值 | 冻结上限 |
| --- | ---: | ---: |
| 屏 / 页 | 4 / 8 | 4 屏；本次双页共 8 页 |
| 正文 UTF-16 载荷 | 22,848 bytes | 524,288 bytes |
| 解析载荷 | 584,948 bytes | 2,097,152 bytes |
| DOM 节点 | 6,128 | 12,000 |
| 图片固有面积 | 892,080 pixels | 8,000,000 pixels |
| 并行准备 | 1 | 1 |

真实极值和合成长段短测的准备峰值另为 11,984 DOM 节点，低于 12,000；长测峰值不能代替该极值负载。相邻缓存随可见来源移动释放，回返时重新按原文游标重建，保存的采样显示占用随内容变化而非随累计翻页增长。长测 `PerformanceObserver` 未记录 ≥50ms 主线程任务。

资源计数含当前、相邻、准备区及尚未加入页面的候选 DOM。原文以 UTF-16 载荷计数；解析载荷含 HTML、语义文本、映射数字与页片段元数据，并保守加入 DOM 的 HTML 载荷。DOM 含文本节点和 KaTeX 内部节点；图片以固有宽高面积计数。这些是所拥有的资源载荷，不能视为整个 JS/GPU 堆的实测值。

## 失败与修复

1. 超高代码的默认外边距使整个对象跳到第二列，空页也放不下。对象限高改为包含自身盒模型且外边距为零；代码末行/横向尾部和双页、单页回归通过。见 [首次失败](failures/initial-continuation.txt)。
2. 首次长段虽已异步对齐，整段公式 DOM 布局仍造成 102ms 任务。改为语义前缀递增试排，渲染、投影、布局各批之间留任务边界。独立场景在无审计渲染干扰时验证无 ≥50ms 任务。见 [旧路径实测](failures/full-leaf-layout.json)。
3. 删除已读列表 DOM 后，浏览器自动编号从 1 重启；原第 14 项显示为 1。保留原 `li.value` 并隐藏同一条目的续接标记。先保留失败，再通过原场景。见 [编号回归](failures/list-numbering.txt)。
4. 原空白场景继续翻到了 Kimi 的外部图片，地址解码失败。空白验证固定到 `1.5.2 / 1.5.3`；图片测试独立使用已导入的六张本地资源，不把不可用外链当成本地负载。此材料问题交 FP3 的准备失败处理。
5. PNG 请求往返包含截图等待、编码和传输，桌面 P95 上界 130.7ms，不能据此确定实际画面时刻；追加浏览器捕获帧时间戳与变化画面证据。见 [原始记录](failures/png-roundtrip-1440.json)。

## 已知限制与 FP3 接手

- 本片交付分页内核和正式源码中的呈现宿主，测量入口为独立生产夹具。完整 App 就绪/预取、手势、标注/问答与 PDF 分别归 FP3–FP6，整体正式入口验收归 FP7。
- P5 本片核对分页器拥有的正文、解析、DOM 与图像资源。夹具的材料服务数组和测试记录充当只读数据源/测量器；FP3 接入 App 后，外部原文/解析缓存也必须纳入同一预算，不能由分页器之外无限持有。
- 页片段已提供正文选择的规范映射；表格/代码等原子对象的细粒度选择、选区跨重排生命周期和批注几何由 FP5 对接原有处理器。分页 API 对原子对象返回 `null`，避免把局部选择冒充整对象引用。
- Kimi 的若干图片仍是外部地址且可能解码失败。ready 回调失败时保留旧屏；本片未替代 FP3 的错误状态、重试和按需解释加载策略。
- 冷字体、远距离读取及首次复杂解析的总等待仍需由 FP3 预取/就绪合同测量；目前无网络延迟及 P3 正常 200 次自动预取验收。长测换屏由夹具请求来源游标，不宣称已实现正式前后台调度。
- 本片未测实体手机、系统选择手柄、物理触摸延迟和实际高刷；后续按已确认的桌面/WebView2 可接入环境与移动模拟验收。P1/P2 的物理呈现不由本片浏览器捕获帧代替。
