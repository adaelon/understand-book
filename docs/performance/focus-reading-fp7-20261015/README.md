# FP7 正式入口与可用环境联合验收

2026-10-15，性能联合验收按用户要求暂停，现有改动已于15:15 HKT随 `current-20261015` 发布。源码为 `6612932` 加工作区；既有改动保留。合同沿 ADR-0166 与专注阅读切片方案。

用户明确要求“先不用做了直接上线吧，把目前没上线的给上线”。续跑自动化已暂停，剩余性能验收不再作为本次发布前置条件；FP7仍未完整验收通过。发布结果另见 [发布与回退记录](../current-release-20261015/README.md)。下文保留原始过程与证据。

## 执行进度

- [x] 读取 checkpoint 全部冷启动读序，确认提交及冻结环境、材料和门槛。
- [x] 正式 index.html 构建、相关单元和普通阅读回归。
- [x] 正式入口的正文/PDF操作、后台问答、来源返回及 WebKit/WebView2 可用环境。
- [ ] 同一候选生产入口性能、正常连续阅读及长时间资源验收。
- [ ] 结果、限制、ADR、架构、代码链路及 checkpoint 收口。

## 入口

`playwright.focus-release.config.ts` 直接服务正式 `packages/web/dist/index.html`。FP4–FP6 用例通过 `FOCUS_ENTRY_URL` 进入首页，共用原来的观察器；观察器由测试端注入，读取已挂载组件，不替换产品挂载或样式。`FP7_SUITE=prose/pdf` 分开结果；`FP7_ENGINE=chromium/webkit` 选择浏览器。原材料/API回执夹具继续使用；结果与之前切片独立保存。

## 失败处置

原文范围、笔记草稿、来源返回或退出错位时修对应操作链；正常节奏未就绪时修准备调度；响应、帧或长任务超门槛时检查该次轨迹；长期资源或回返稳态增长时修持有关系。只重跑受影响组，保留失败结果。

## 已完成的功能证据

- `unit.log` 首轮146项暴露MCP启动来源导航失败与PDF卸载未处理拒绝；`pdf-dispose-red.log` 单独复现后者。修复后 `unit-fixed.log` 10文件147项通过。
- `build.log` 正式Web构建、`desktop-build.log` Tauri宿主、`server-build.log` 本地服务构建通过；`typecheck-native.log` 联合测试类型检查通过。
- Chromium 11项既有正文操作通过（`chromium-functional.log`）；后台问答夹具补全运行查询、历史和终态结构后，与两项真实触摸轻点DPR2/3一起通过（`entry-repaired.log`）。PDF 6项通过（`chromium-pdf-functional.log`）。
- WebKit 12项正文/后台问答/移动轻点与旋转、5项PDF功能通过（`webkit-functional.log`、`webkit-pdf-functional.log`）。
- 实际Tauri/WebView2的正文选区与公开入口2项、后台问答1项、PDF3项通过（`webview-entry-second.log`、`webview-background.log`、`webview-pdf-functional.log`）。原生测试窗口使用独立用户资料和私人目录。
- `live.log` 两项真实本地服务通过；直接加载 `.understand-book/understandbook` 和 `kimik3`，调用实际内容/读位接口。隔离私人存储，正式首页进入、四次翻页及退出，结果位于 `results/chromium/live/`。

## 修复

- PDF首屏发布后等待绘制时卸载，后续gate抛出取消异常。`PdfFocusReader.run` 在已销毁时正常结束，不再调用已销毁宿主；新回归锁住该时序。
- MCP来源导航与首次聊天恢复同时发生，导航被聊天上下文变化取消。`App.doGoto` 使用已有Reader上下文归属，材料/读者现场与请求序号仍约束导航；首次聊天恢复不再使同一材料的外部引用失效。


## 普通阅读回归

20个不同场景全部通过：`continuous-regression.log` 中13项通过，`continuous-repaired-first.log` 中3项通过，`continuous-repaired-second.log` 中4项通过。失败原样保留。

旧RE3–RE5测试仍查找已移除的笔记详情CSS、旧笔记编辑器和全屏问答的旧导航；正文夹具也只返回单叶且忽略`end`，使合并读取失真。夹具现在按真实span返回单读/合并原文，长段同步更新span，断言使用现有公开控件。分页进入验证原字符在当前屏内；退出以离开书页时的规范字符和几何为恢复基准，继续保留36px恢复误差、同一Reader/聊天实例、草稿和布局零写入断言。产品实现未因这些旧断言改变。

## 性能运行

帧记录先执行Chromium正文/PDF的五组翻页、桌面/手机显示变化，再测WebView2桌面的本地/弱网与显示变化。所有性能用例串行连接同一正式dist，期间不构建、不运行另一测试浏览器。


## 测量校准

WebView2 155.0.4283.45 的 BeginFrame 中位周期约6.06ms（165Hz）。旧观察器每帧调用getComputedStyle，造成额外主线程更新；原有逐帧诊断下缺失率1.24–2.00%，仅关闭截图仍超标。`with-raf-diagnostics/` 和原始日志保留该结果；`prose-frames/` 采集不含逐帧样式诊断的实际DrawFrame/DroppedFrame，两组均0%。最终实际画面轨迹也移除了该诊断；产品及1%门槛没有改变。

WebView2单次Tracing最多保留450张画面。P1/P4记录现在在已收尾操作之间分段，保留原始单调时间戳；导出时暂停计时节奏，下一次输入不会为了赶进度重叠。P4正文同时保留独立Page.screencastFrame，新屏ready之后的实际新画面可给出保守上界。配对范围限于下一次输入之前。旧缺捕获和分段节奏失败保存在 `capture-limit/`、`segmented-cadence/`，未换用DOM-ready或RAF值。

## 连续阅读矩阵

| 运行环境 | P3正常节奏 | P5长读 |
| --- | --- | --- |
| Chromium | 正文/PDF × 1440/390 × 本地/弱网，各200次 | 复杂正文，20分钟，字体变化、反向、跨章、来源返回 |
| WebView2 | 正文/PDF × 1440 × 本地/弱网，各200次 | 原版PDF，20分钟，反向、缩放及原页返回 |
| WebKit | 正文/PDF × 390 × 本地/弱网，各200次 | 移动复杂正文，20分钟，字体变化、反向、跨章、来源返回 |

正常节奏均每秒一次，弱网沿100ms RTT / 10Mbps。P1/P2和P4实际画面采集可用于Chromium及WebView2；当前WebKit接口不提供同等时间戳的画面/合成帧及Long Tasks API，其值保留未测。


## 可重跑入口

从仓库根目录准备FP1真实材料，再构建正式产品；性能阶段只运行一个测试浏览器。原功能失败与每次修复日志保留，最终命令仅重跑受影响用例。

```powershell
node packages/web/playwright/fixtures/prepare-focus-reading.mjs
pnpm --filter @understand-book/web build
cargo build -p understand-book-desktop
cargo build -p server --bin server
pnpm --dir packages/web exec vue-tsc --noEmit -p playwright/tsconfig.focus-release.json
```

联合入口通用参数：

```powershell
$env:FP7_ENGINE='chromium' # webview2 / webkit
$env:FP7_SUITE='prose'    # pdf / live
pnpm --dir packages/web exec playwright test --config playwright.focus-release.config.ts --grep 'normal 200|P5 annotated' --reporter=line
```

| 目标 | ENGINE / SUITE | grep |
| --- | --- | --- |
| Chromium正文帧和显示 | chromium / prose | `render trace\|P4 warm` |
| Chromium PDF帧和显示 | chromium / pdf | `render.*trace\|display changes` |
| WebView2正文帧和显示 | webview2 / prose | `render trace 1440\|P4 warm.*1440` |
| WebView2 PDF帧和显示 | webview2 / pdf | `render trace 1440\|display changes 1440` |
| Chromium正常阅读与正文长读 | chromium / prose | `normal 200\|P5 annotated` |
| Chromium PDF正常阅读 | chromium / pdf | `normal 200` |
| WebView2正文正常阅读 | webview2 / prose | `normal 200.*1440` |
| WebView2 PDF正常阅读与长读 | webview2 / pdf | `normal 200.*1440\|PDF long reading` |
| WebKit移动正文正常阅读与长读 | webkit / prose | `normal 200.*390\|P5 annotated` |
| WebKit移动PDF正常阅读 | webkit / pdf | `normal 200.*390` |
| 真实服务与正式首页 | chromium / live | 全部 |

全部浏览器结束后汇总：

```powershell
python packages/web/playwright/fixtures/summarize-focus-release.py
python packages/web/playwright/fixtures/plot-focus-release.py
```

联合汇总明确要求16组正常阅读、14组实际帧、6组显示变化及3次20分钟长读。旧FP4/FP6汇总器在单个平台子集下只提供分析数据；整体结论使用本目录根 `results/summary.json`。


## 待归因记录

Chromium PDF桌面本地正常阅读首轮200次零未就绪、预算/P6通过，但第63次附近出现52/50/60ms三次阅读长任务，其中前两次在动画内，PDF准备计数没有增加。其他三个正常阅读组无长任务。原件保存在 `results/chromium/pdf-first-normal/normal-1440-local.json`。保留原RAF样式观察方式的200次主线程轨迹复测未重现长任务，最长记录事件为34.172ms，P6五项全部0；见 `results/chromium/pdf-frames/diagnostic-analysis.json` 及同目录原始轨迹。最终矩阵使用该次完整诊断复测，原三次长任务仍未归因。


## 正文恢复调度修正

WebKit正文正常本地组200次零未就绪，但第26次的输入至收尾区间内alignments和geometryProbes各增加1；原件在 `results/webkit/prose-before-resume/`，日志 `webkit-prose-reading.log`。正文的resumeWork会立即唤醒等待中的准备，PDF则先等待当前画面绘制。

`FocusReader.schedule` 将后台补页与暂停恢复安排到RAF后的任务，并复查暂停、选择、可见性和销毁状态；卸载取消排队帧。前台重排直接排任务，尽早替换旧几何。三项确定性回归分别覆盖绘制前不能恢复、二次暂停不能被旧恢复穿过、前台不等待旧几何绘制；均先红后绿。受影响28项单元通过；连续性4项沿此前通过记录。正式Web已重建，测试类型检查通过。

首次修正将前台也延后，Chromium移动旋转P95为123.540ms；两次整页Layout占78.262/213.541ms。该轮在发现失败后停止长测，原件存 `results/chromium/prose/foreground-delay/`。缩小调度等待范围后复验正文各平台P1/P4/P3及Chromium/WebKit长读；此前测量保留在各自 `prose-before-resume/`，最终日志使用 `prose-verified-*`。

## 实测环境

Lenovo 82JU，AMD Ryzen 7 5800H，Windows 10 Pro 19044，系统可见内存约14.89GB（13.87GiB），电源模式“野兽模式”。Chromium 149.0.7827.55、WebKit 26.5；实际Tauri调试宿主中的WebView2 155.0.4283.45加载同一正式Web产物。桌面1440×900，移动390×844并覆盖320/1024及横屏；触摸功能另测DPR2/3。Chromium帧周期约16.67ms，WebView2约6.06ms。

## 已知问题与限制

- Chromium PDF首轮三次长任务未归因，完整诊断复测未复现；原始失败及最终复测同时保留，见上文。
- 实体手机、系统选择手柄和物理显示时延未测。WebKit缺少本次所需的时间戳画面/合成帧与Long Tasks API，P1/P2/P4时延及长任务保留未测。
- WebView2测量来自实际Tauri调试宿主加载正式Web构建，未验证打包安装器；165Hz是该环境浏览器帧事件的测量结果。
- 资源曲线统计阅读实例持有量，不代表浏览器总堆或GPU总占用；PDF固定栅格复用的清晰度限制沿FP6。
- 可控API回执与两项真实本地服务验证分别保存；后台问答验证生命周期，不调用外部模型。

## 邻页准备连续性修正

缩小绘制等待范围后，WebKit本地第53次反向翻页出现未就绪；原件在 `results/webkit/prose/cancelled-neighbor/`，日志 `webkit-prose-neighbor-first.log`。该复杂页的准备跨过多个输入，原ready翻页仍取消现有准备，导致反复重做。现在已就绪、同几何且无前台请求的翻页保留串行邻页准备，继续受暂停门控；目标未就绪、前台目标或几何变化仍取消。新确定性回归先红后绿，相关33项通过；重新验证全部正文翻页性能和正常/长读。P4独立路径未改变，沿已通过的 `chromium-prose-display-verified.log` / `webview-prose-display-verified.log`。

## 实际输入节奏

WebKit第102次失败的采集包含556/589ms相邻输入：脚本按总时长追赶先前延迟，实际已经变成快速输入。原件在 `results/webkit/prose/catch-up-inputs/`。正常阅读在浏览器内按上一次实际input时间加一秒点击公开按钮，并等待该次收尾；正文长读从实际输入等待两秒。自动化延迟不再积累为后续追赶。联合结果保存每组实际输入间隔的最小值、中位数和最大值，并拒绝短于900ms或中位间隔超过1050ms的正常组。此前有效PDF组及其他正常组的实际间隔均在允许节奏范围内，无需重复测量未改变的PDF产品路径。

## 表格来源对齐热点

阶段诊断排除了读取、字体、图片和未唤醒暂停：复杂表格仍在分页对齐，细碎计算与任务切换等待共同拖慢补页。Windows WebKit的裸MessageChannel/timeout中位等待约15ms；正式页面诊断有228ms单次等待。原件保存在 `diagnostic-pagination/`、`diagnostic-yields/`，临时探针均已移除。

Hirschberg内层现在预取UTF-16数值并比较，保留原算法、顺序、4ms计算片段及线性临时空间。三张真实表格的完整edit数组与原版完全相同；独立WebKit总耗时从110/238/174ms变为33/106/64ms，详见 `alignment-webkit-comparison.json`。从仓库根运行 `node docs/performance/focus-reading-fp7-20261015/alignment-comparison.cjs` 可复现原版/charCodeAt/数值数组比较。相关46项单元、正式构建及测试类型检查通过；全部相关单元累计11文件164个不同用例分批通过。最终门槛仍由正式入口的完整复验决定。

数值对齐优化后WebKit本地200次首次通过，但自动化外部click带来1156ms中位间隔；该组存automation-overhead/，仅作诊断。正式P3使用浏览器时钟的公开按钮点击，独立P1及功能用例继续覆盖实际鼠标/触摸输入。

## 长读回返的相同几何

按实际输入计时后，Chromium第一次长读在589次结束，字号停在22px，而初始是19px。该轮20分2.304秒零未就绪/长任务/P6增量，但回返资源不能当作相同版面的比较；原件保留在return-typography-first/。长读用例现记录初始字号、返回前恢复并断言一致，再比较原始规范锚点及资源。最终Chromium补验日志为chromium-prose-final-return.log，WebKit长读直接使用更新后的用例。

## 长读超时与环境负载

12:48的WebKit长读在约25次后等待后台补页超过5秒，保留webkit-prose-verified-reading.log及long-wait-first/截图。状态复现于第90次再次超时：当前页ready、翻页idle、零未就绪，后台仍preparing；见webkit-prose-long-state.log及long-wait-state/。第三次在第4次超时，随后10秒的记录显示对齐继续推进、补页自行完成，未发生死锁；见webkit-prose-long-timeline.log及long-wait-timeline/。

当时系统可用物理内存从2.4GB降至286,640KiB，另有多项外部书籍构建运行。13:01的独立WebKit表格基准，同一数值实现由33/106/64ms变为1948/5825/5847ms，原版与charCodeAt版本同样变慢；完整edit序列依然相同。证据为alignment-webkit-current-load.json、alignment-current-load.log。该负载影响无法用产品修复消除，后续性能过程须在稳定环境重新执行；三次超时均保留，不冒称已修复或已通过。

观察器另有独立口径错误：原pointermove监听也接受未按键移鼠，会把自动点击前的鼠标移动算作输入。正文/PDF观察器现只接受读区内按下状态的移动，并记录inputType；正式WebKit回归observer-input.log验证悬停不采样、随后公开按钮点击只采一次。既有通过的P1区间从更早时刻开始，是保守上界；既有P6在较宽区间仍为零。严格P3由浏览器内按钮点击驱动，无此移鼠事件。
