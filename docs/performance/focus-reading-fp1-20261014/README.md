# FP1 基线与局部分页验证

2026-10-14。状态：**FP1 本地完成，FP2 可开始**。依据：[ADR-0166](../../adr/0166-focus-reading-pagination-and-performance.md)、[切片方案](../../切片方案-专注阅读双页与无感翻页.md)。源码基线为 `6612932` 加当前工作区；本片新增预算参数和独立测量夹具，生产 Reader/App/PDF 组件沿用原实现。

局部固定高度多栏可继续用于 FP2。五种窗口、两种字号下的来源范围、长段跨列、公式、宽表格、超高代码及图片验证通过；连续阅读基线和限量窗口实验已保存。FP2 的具体约束是按实际页数截取局部窗口、保留未绘制空白的来源归属、复用来源映射，并消除首次长链接映射的同步阻塞。

## 实施与验证入口

| 文件 | 职责 |
| --- | --- |
| [prepare-focus-reading.mjs](../../../packages/web/playwright/fixtures/prepare-focus-reading.mjs) | 从既有导入成果抽取真实 LID、原文及图像；每次只生成本地测试资源 |
| [focus-reading.ts](../../../packages/web/playwright/fixtures/focus-reading.ts) / [样式](../../../packages/web/playwright/fixtures/focus-reading.css) | 挂载实际 ReaderPane / PdfReaderPane；分别测准备、原文映射、局部多栏与资源 |
| [focus-reading-performance.spec.ts](../../../packages/web/playwright/focus-reading-performance.spec.ts) | 15 个串行浏览器场景；从实际键盘事件到渲染轨迹、原文范围和局部滚动尾部 |
| [focus-reading-trace.ts](../../../packages/web/playwright/focus-reading-trace.ts) | 保存 CDP 原始轨迹、filmstrip 和布局/绘制统计，区分合成记录与物理呈现 |
| [生产构建配置](../../../packages/web/vite.focus-performance.config.ts) / [运行配置](../../../packages/web/playwright.focus-performance.config.ts) | 独立生产 bundle，固定 4188 端口，禁止复用未知服务器 |
| [reader-pagination-budget.ts](../../../packages/web/src/reader-pagination-budget.ts) | 冻结资源参数及按字号推导的单双页几何；产品接入由 FP2/FP3 完成 |

从仓库根目录运行：

```powershell
node packages/web/playwright/fixtures/prepare-focus-reading.mjs
pnpm --filter @understand-book/web build
pnpm --filter @understand-book/web exec vue-tsc --noEmit -p playwright/tsconfig.focus-performance.json
pnpm --filter @understand-book/web exec vite build --config vite.focus-performance.config.ts
pnpm --filter @understand-book/web exec playwright test --config playwright.focus-performance.config.ts --reporter=line
node packages/web/playwright/fixtures/summarize-focus-reading.mjs
pnpm --filter @understand-book/web exec vitest run src/reader-pagination-budget.test.ts src/markdown-source-map.test.ts
```

本次结果：生产构建（含类型检查）、夹具类型检查、独立生产 bundle、15 项浏览器场景及 14 项单元测试通过。首次运行的四个窄窗失败与修复依据保存在 [failures/README.md](failures/README.md)。受影响的列布局场景修复后先单独通过；增加图像负载、映射复用及首屏捕获计时后，最终 15 项一起通过。测试日志见 [verification.txt](verification.txt)。

资源和产物在 `tmp/focus-reading-fp1/`；默认证据目录为本页的 `results/`。可在运行前设置 `FP1_OUTPUT` 为另一绝对路径保留独立结果。单独复测用 `--grep 'continuous baseline complex'`、`--grep 'local columns'` 或 `--grep 'bounded local'`。原始轨迹 `*.trace.json.gz` 可解压后导入 Chromium 性能工具。

## 材料与环境

完整入口、源文件修改时间、字节数、样本范围和节点分类记录于 [environment.json](results/environment.json)。原文按导入成果的 UTF-16 span 切片；不用人工编造正文代替成本负载。

| 材料 | 规模与版本 | 测量范围 |
| --- | --- | --- |
| EPUB 导入的 Understand Book 项目书 | `understandbook`；790,549 字节、3,458 叶；2026-10-08 源文件 | LID `27.11.4`–`32.5.1`，240 叶，跨第 27–32 章；另取最长段、最大表格与代码 |
| AI Infra Book | `ai-infra-book-complete`；1,614,529 字节、9,111 叶，含 2,410 公式、127 表格、482 图像；2026-09-26 源文件 | LID `4.26.4`–`5.5.4.25`，240 叶；另取最长脚注、公式/表格/代码，以及 SVG 名义面积最大的六张图 |
| Kimi K3 | `kimik3`；266,578 字节、1,426 叶；2026-08-23 源文件 | LID `1.1`–`1.6.4.68`，240 叶；最长段 `1.5.2`、573 字符公式、29,421 字符表格；原版 PDF 为 1,790,685 字节 |
| 行为补充夹具 | 明确标记 `stress.long` / `stress.code` | 160 次中英混排、强调和行内公式；100 行宽代码。只验证长段和超高对象的续接，不用于真实材料性能结论 |

本地设备为 Lenovo 82JU、Ryzen 7 5800H、约 14.89 GB 系统可用物理内存、Windows 10 Pro；活动电源方案为“野兽模式”。本轮使用 Chromium 149.0.7827.55 headless、DPR 1、本地静态网络，窗口为 1440×900、1024×900、390×844、844×390、320×844；19/22px 字号，内置 Noto Serif SC。连续模式基线测 1440/390，其余尺寸测分页可行性。

系统显示设备报告 165Hz，但本轮 headless 的 RAF 调度中位间隔为 16.7ms；这两项均不作为实际呈现刷新率。后续验收采用桌面生产构建和移动视口/触摸模拟，环境范围如下：

| 目标 | 状态 |
| --- | --- |
| 本机 Windows 正式 Web / Chromium | 已测上述生产组件夹具；完整产品入口在 FP7 验收 |
| 本机 Windows Tauri / WebView2 | 已安装运行时 154/155；FP7 在可接入时补充运行时结果 |
| iPhone 13 / Safari | 移动布局参考，使用 WebKit 视口与触摸模拟 |
| Pixel 6a / Chrome（日常中档 Android） | 移动布局参考，使用 Chromium 视口与触摸模拟 |

## 计时口径

`load()` 分开记录本地读取、Vue/解析/字体与图像准备、ready 时间；ready 后截屏得到“首屏已捕获”的保守上界。连续模式保留原组件的图像懒加载；分页实验将当前局部批次图像提前 decode。固定每批 24 叶，逐批追加到 240 叶；在 24/120/240 叶采样资源并进行 15 次前进/反向键盘输入。

输入记录保留原始 `event.timeStamp`、handler 时间、提交时间及滚动位置。轨迹保存布局/绘制、合成事件和真实内容的 filmstrip；截图变化只在下一次输入之前配对。`handlerMarkToChangedCaptureMs` 是稀疏捕获诊断，不含完整的物理显示确认；无变化或缺捕获的输入保留在原始输入列表，不制造成功样本。

物理 `inputToPresentedFrameMs`、`droppedFrameRatio` 明确为 `null`，`supported=false`。本轮轨迹中的 `FramePresented` 只有两条边界事件，不是可用于逐帧统计的序列；RAF 单独作为调度诊断。后续 P1/P2 使用可用运行环境能够可靠采集的输入与渲染帧证据。

## 连续模式基线

数据见 [summary.json](results/summary.json) 及同目录六份 `continuous-*.json` / 原始轨迹。首屏 ready 含字体冷准备；每组热加载只有三次，表中的热值取最大值，不据此宣布合同级 P95。

| 材料 / 宽度 | 冷 ready | 已捕获首屏上界 | 热 ready 最大值 | DOM：24 → 120 → 240 叶 | 全轨迹最长主线程任务 |
| --- | ---: | ---: | ---: | --- | ---: |
| EPUB / 1440 | 140.2ms | 231.8ms | 9.5ms | 342 → 1,839 → 4,603 | 83ms |
| EPUB / 390 | 133.8ms | 186.7ms | 10.0ms | 342 → 1,839 → 4,603 | 82ms |
| 复杂材料 / 1440 | 232.4ms | 303.1ms | 16.5ms | 2,989 → 15,048 → 18,028 | 139ms |
| 复杂材料 / 390 | 213.7ms | 256.6ms | 16.4ms | 2,989 → 15,048 → 18,028 | 135ms |
| Kimi K3 / 1440 | 89.3ms | 169.1ms | 7.7ms | 247 → 4,802 → 9,264 | 81ms |
| Kimi K3 / 390 | 90.3ms | 141.1ms | 7.8ms | 247 → 4,802 → 9,264 | 82ms |

复杂材料的 HTML 字符载荷从 129,892 增至 759,062 字节。当前追加模式确实随已装载叶数增长，且追加阶段存在超过 50ms 的任务；FP3 必须回收和分批准备。所有基线轨迹启用了采样，有观测开销，不能把单次最大任务直接归因于某一个渲染函数。

PDF 实际首张画布和文字层就绪为 618ms；六次滚动、七次资源采样中，最高 4 个非空画布、4,141,008 backing pixels，低于现有 12,000,000 上限。见 [pdf.json](results/pdf.json) / [截图](results/pdf.png)。

## 局部分页结论

1. **固定高度原生多栏可保留原文顺序。** 30 组普通材料/字号/尺寸组合逐片核对来源范围和页序，无漏重或逆序；真实极值材料另核对来源范围及缓存复用前后一致。补充长段跨 7–17 列仍保留原生文本选择。
2. **尾随空白必须跟随原文。** Kimi K3 `1.5.3` 的列尾空格会得到零矩形。探针将未绘制空白归到前一片段，保持源范围；非空白零矩形仍报错。FP2 不能用零矩形的坐标生成页号。
3. **宽内容需要独立滚动区域。** 代码、表格与显示公式按对象保持完整，限制内部可见高度并允许滚动；100 行代码的末行和横向尾部实际可达。图片先解码、保持比例并约束页高。见 [320 宽截图](results/columns-320.png)、[图片页](results/images-1440.png)。
4. **几何与首次来源对齐要分开。** 普通材料单次映射为 0.8–3.9ms；真实脚注 `9.14.6.17` 的首次完整映射为 111.4–116.3ms，其中来源对齐占 110.1–115.2ms。保留同一 DOM/原文映射后，再测列边界只需 0.9–1.1ms，且新建映射次数为零。FP2 要把首次纯对齐从同步补页链路拆出，采取可让出的计算或 Worker，并保留本条真实输入回归；仅缓存无法消除首次阻塞。
5. **页数决定局部窗口大小。** 22px 下桌面有界样本保留 6–24 叶，手机保留 4–24 叶；其中普通正文会缩至 10–18 叶，六图窗口手机只保留四图。二分试排是 FP1 的预算探针，FP2 应按来源边界增量续接，不能把多次试排作为每次翻页路径。

## 冻结预算

参数由 [reader-pagination-budget.ts](../../../packages/web/src/reader-pagination-budget.ts) 提供，按当前屏、邻屏及准备区**合计**计量。页宽、字号及设备选择用 `focusPageGeometry()`；预算不是单批准备耗时的保证。

| 资源 | 冻结上限 | 测量依据及计数口径 |
| --- | --- | --- |
| 双页最小页宽 | 每页 20em，页间距 32px | 可用正文容器达到 `40 × fontSize + 32` 才双页；19px 为 792px、22px 为 912px。手机保持单页，窄窗单页可低于 20em |
| Markdown 驻留 | 4 屏：单页 4 页、双页 8 页 | 桌面实测 2–7 页、手机 4 页；准备页计入 |
| 正文字符串载荷 | 524,288 字节（512KiB） | 字符串按 `UTF-16 length × 2`；有界样本最高 14,320 字节，真实最大表格本身 58,842 字节，留出多窗口及长段输入空间 |
| 解析缓存载荷 | 2,097,152 字节（2MiB） | 本次以实际渲染 HTML 的 UTF-16 载荷计数；有界样本最高 129,892 字节。FP2/FP3 新增持有的解析字符串、映射数组也须纳入该预算 |
| DOM 节点 | 12,000 | 包括文本节点、KaTeX 内部节点和准备区；有界样本峰值 2,989；上限约为该复杂局部样本四份，页数门同时生效 |
| 解码图片名义像素 | 8,000,000 | 对常驻图像累加 naturalWidth × naturalHeight；SVG 为名义固有面积。六张最大名义 SVG 实际解码后合计 2,757,561，手机四图为 2,112,441；预算约 32MB RGBA 载荷 |
| 并行排版准备 | 1 个任务 | 手势进行时让出；测试单次热试排 0.8–18.3ms，因此 FP2/FP3 仍须按帧预算拆批 |
| PDF 专注画布 | 12,000,000 pixels；桌面 6 页、手机 3 页 | 既有总像素上限；FP6 接入完整跨页就绪，连续模式仍沿既有五页策略 |

预算实验的完整尝试和收敛值在 [bounded-1440.json](results/bounded-1440.json) 与 [bounded-390.json](results/bounded-390.json)。实验会先测超限批次再缩小，用于寻找边界；它不代表产品已经实现驻留回收。正式实现应先预留预算、停止远处预取，必要时从已知源边界分块；禁止靠裁掉内容满足上限。

## 已知问题与 FP2 / FP7 交接

- FP1 验证的是生产编译的真实 Reader 组件与局部分页实验。App 的完整网络加载、公式语义等待、已读写回及逻辑工作区没有接入该夹具；组件基线不包含服务器、模型或线上网络耗时。
- 首次长链接来源对齐仍有超过 100ms 的同步成本，已成为 FP2 必须解决的具体验收用例。局部试排一次最高 18.3ms，也高于部分设备单帧预算，不能直接放入拖动帧。
- 正式分页入口、跨局部窗口的增量续接/回收、拖动动效、选择与笔记联动、PDF 双页尚属 FP2–FP6。FP1 的源范围验证覆盖局部多栏，不宣称这些后续能力完成。
- 本轮没有 200 次正常翻页、20 分钟/500 次长阅读、100ms RTT/10Mbps 网络、DPR 2/3 或 WebView2 结果；这些可用环境场景按 P1–P6 在相应切片及 FP7 验收。
- 实体 iPhone/Android、手机系统级选择手柄、实体触摸延迟与实际高刷表现列为未验证范围，不阻塞本期交付。模拟结果仅适用于所测浏览器；无法采集的物理呈现指标保留未测。
- `performance.memory` 在当前 headless 环境呈现量化读数，只作附带诊断。字符载荷、DOM、名义图像面积和画布像素不等于整个浏览器进程、SVG 栅格缓存或 GPU 内存。
- 本地材料依赖 `.understand-book/` 中现有三个导入成果；原始正文和 PDF 不复制进 Git。材料变更后按 inventory 核对版本和样本，再重新建立该负载记录。
