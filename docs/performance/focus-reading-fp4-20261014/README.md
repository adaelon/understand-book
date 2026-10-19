# FP4 双页呈现与跟手翻页

2026-10-14，本地完成。基线 `6612932` 与 FP3 已验收工作区。公开专注入口、单双页呈现、覆盖式触摸翻页、按钮/键盘、工具呼出及进入退出接续已实现，P1/P2/P3/P6 在约定的 Chromium 桌面与移动模拟环境通过。下一片 FP5 接原文操作与持久读位。未提交、未部署。

依据：[切片方案](../../切片方案-专注阅读双页与无感翻页.md)、[ADR-0166](../../adr/0166-focus-reading-pagination-and-performance.md)、[FP1 冻结预算](../focus-reading-fp1-20261014/README.md)。

## 实现

`公开专注入口 → 原文锚点 → FocusReaderPane → 已就绪屏预览 → 拖动/收尾 → 确认当前屏 → 恢复补页`。

- App 的 `onFocusChange` 连接 `ReaderPane.prepareFocus / leaveFocus`；退出读取当前规范锚点附近连续正文并恢复。切书/重新载入清理旧进入退出请求。
- `reader-focus-turn.ts` 直接使用当前及相邻屏 DOM。宽窗口双页，窄窗口/手机单页；按钮、ArrowLeft/Right、PageUp/Down 与触摸覆盖式拖动共用已准备屏。首末屏及奇数末页按实际内容展示。
- 可翻页区域指针按下至拖动/收尾结束暂停准备；横向意图超过 8px、方向比 1.25 后接管。释放按距离/速度确认或回弹，收尾 80–200ms；新输入立即接管已确认页，减少动态效果直接切换。选择、输入和表格/代码局部滚动优先。
- 已准备屏在不透明底板下保留布局及绘制记录，每屏隔离布局/绘制。隐藏标记、inert、aria-hidden 隔离来源查询和输入。动画直接使用输入时的时间线；字体/尺寸变化延后到收尾，目录跳转、隐藏及来源替换清理旧动作。
- 工具浮层复用目录、阅读设置、问答和笔记工作区，呼出不改变正文尺寸；问答返回沿同一宿主和原文游标。
- `reader-flow.ts` 共享连续/分页行内公式合流规则，消除独立导入公式叶额外换行，每叶仍保留规范范围。首次数学正文准备完成 KaTeX 固定字体集加载，避免后续数学字体到达打断翻页。

## 验证结果

[最终汇总](results/summary.json) 的 `acceptance.passed=true`。正式 App 由生产入口挂载，测试桥只读取状态及计时；进入、翻页、目录和工具全部走公开控件，触摸使用浏览器输入事件。Chromium `149.0.7827.55`，桌面 1440×900、手机 390×844；弱网为 API 往返延迟 100ms、静态资源 10Mbps。材料沿 FP1 的真实导入来源。

### P1 / P2：实际浏览器渲染

每组 20 次。前四组含正反向按钮与键盘，共 64 次点击、16 次按键；最后一组为 20 次正反向拖动及收尾。实际 BeginFrame 周期中位数 16.666ms。

| 场景 | 首次正文变化 P95 | P99 / 最大值 | 缺失帧 / 帧序并集 | 缺失率 |
| --- | ---: | ---: | ---: | ---: |
| 桌面本地 | 36.319ms | 37.641ms | 1 / 228 | 0.44% |
| 桌面弱网 | 34.062ms | 45.890ms | 1 / 234 | 0.43% |
| 手机本地 | 22.419ms | 22.730ms | 0 / 232 | 0% |
| 手机弱网 | 21.188ms | 21.209ms | 0 / 228 | 0% |
| 手机拖动/收尾 | 36.628ms | 38.620ms | 0 / 320 | 0% |

全部满足 P95 ≤50ms、缺失率 ≤1%。每组完整阅读区间均无 ≥50ms 长任务，数学字体请求为零。触摸从 pointerdown 分派到首个有效拖动输入为 2.8–6.6ms；输入到画面反馈另含事件排队。每次的输入、位移、收尾及准备计数保存在 `results/render-*.json`，原始浏览器时间线为同名 `.trace.json.gz`。

### P3 / P6：连续阅读与交互期间工作

1440/390 × 本地/弱网，各按每秒一次执行 200 次，共 **800 次有效翻页、零未就绪**；四组分别持续 200019 / 200018 / 200036 / 200025ms，包含反向及跨章。整个输入到动画结束的准备、来源对齐、几何探测计数增量均为零；20 次拖动的完整拖动/收尾区间也为零。后台补页在交互结束后继续。

正常点击的输入到收尾 P95 为 214.1–214.4ms（单列动画总时间）；CSS 位移采样 P95 为 14.9–15.4ms，仅作为调度诊断。连续阅读的输入与输入之间也没有 ≥50ms 长任务。见 [最终性能日志](performance-final.log) 及 `results/normal-*.json`。

| 资源峰值 | 桌面 | 手机 | FP1 上限 |
| --- | ---: | ---: | ---: |
| 屏 / 页 | 4 / 8 | 4 / 4 | 4 屏，桌面8页/手机4页 |
| 正文载荷 | 28,932 B | 17,520 B | 524,288 B |
| 解析载荷 | 1,020,064 B | 588,746 B | 2,097,152 B |
| DOM | 10,580 | 5,923 | 12,000 |
| 解码图片名义像素 | 962,080 | 526,960 | 8,000,000 |
| 准备任务 | 1 | 1 | 1 |

### 功能与回归

- 7 项公开入口功能场景通过：进入退出/工具/键盘接续；触摸拖动/回弹/反向/减少动态效果；首末页/选择与40次快速输入；宽表横向滚动；320/1024/旋转/1440尺寸；目录/问答返回/来源替换；缓冲耗尽与动画中快速替换。详见 [功能与初轮渲染日志](functional-and-render.log)（其中包含4项渲染场景，共11项）。
- EPUB 1440、复杂公式材料 390、零矩形空白/相邻屏选区共 3 项来源连续性回归通过，证据在 [source-regression](source-regression.log) 与 `source-regression/`。
- [178 项相关单元/组件测试](unit-tests.log) 通过；最终字体修正后复跑其中 [14 项分页/准备测试](final-focus-tests.log)，全部通过。
- [正式构建](production-build.log)、[独立 App 入口构建](app-fixture-build.log)、[独立入口类型检查](app-fixture-types.log) 通过。最终四组正常阅读及五组时间线共 9 项通过，用时15.6分钟。

## 测量与复跑

`fixtures/summarize-focus-turn.py` 直接比较 CDP filmstrip 解码后的正文像素，保留进入页边缘、排除四周控件并过滤 JPEG 微小噪声。事件标记保留原始输入时间戳；P1 使用首次实际正文变化。P2 使用翻页区间的 DrawFrame/DroppedFrame，分母为绘制和缺失帧序号的并集，包含部分缺失更新。序号缺口单列诊断，因为静止指针的 NO_UPDATE_DESIRED 也会有间隔。[Chromium 记录语义](https://chromium.googlesource.com/chromium/src/+/bf9022ee78eadaf33d3de4ed5195f903cdd4efd1/cc/metrics/compositor_frame_reporter.cc)。RAF 与 CSS transform 采样单列。

P6 计数覆盖输入到动画结束，结束标记在恢复补页的后续任务前采集；实际时间线同时核对布局/绘制，避免遗漏字体到达引起的浏览器布局。启动前和阅读期间的长任务均保存，分开归类。

从仓库根运行，材料沿 FP1 已生成的真实来源：

```powershell
pnpm --filter @understand-book/web test src/reader-pagination.test.ts src/reader-focus.test.ts src/components/ReaderPane.test.ts src/components/ReaderWorkspace.test.ts src/components/RightRail.test.ts src/App.startup.test.ts src/markdown-source-map.test.ts
pnpm --filter @understand-book/web build
pnpm --filter @understand-book/web exec vue-tsc --noEmit -p playwright/tsconfig.focus-turn.json
pnpm --filter @understand-book/web exec vite build --config vite.focus-turn.config.ts
pnpm --filter @understand-book/web exec playwright test --config playwright.focus-turn.config.ts --grep-invert 'normal 200|render trace' --reporter=line
pnpm --filter @understand-book/web exec playwright test --config playwright.focus-turn.config.ts --grep 'normal 200|render trace' --reporter=line
python packages/web/playwright/fixtures/summarize-focus-turn.py --check
```

独立输出 `tmp/focus-reading-fp4/dist`；默认端口4192，`FP4_PORT` 可改端口。性能运行串行，避免互相干扰。

## 失败与修正

- 行内公式额外块级换行先由规范范围回归复现，再共享合流规则；[原失败](failures/inline-flow-red.log)。
- 隐式触摸捕获从文字节点转给容器时，旧节点的 lostpointercapture 冒泡误取消拖动；现在仅容器自身失去捕获才取消。[原事件记录](failures/implicit-capture-events.json)。点击翻页按钮后，按钮焦点曾阻断 PageUp；翻页按钮继续拥有翻页键，其他控件保持输入归属。[原失败](failures/button-keyboard.log)。
- 原 display:none 恢复导致整页重排和绘制层冷启动，桌面画面 P95 92.1ms、缺失34/276；首轮桌面200次后停止旧候选长测。[原记录](failures/before-render-latency-fix/summary.json)。保留布局后仍有60.6ms尾延迟；显式动画时间线及底板下保留绘制记录解决剩余启动等待。中间失败保留于 `failures/preserved-layout-pending-animation/` 和 `failures/input-timeline-before-painted-buffer/`。
- 桌面弱网后台 KaTeX_Size2 字体到达触发已显示公式布局，缺失4/227；首次数学正文准备固定字体集后解决。[原时间线汇总](failures/late-math-font/summary.json)。
- 触摸用例初次落点在宽表，正确交给横向滚动；后改在正文起拖，并单列宽表回归。初版画面分析裁掉反向进入的页边缘，同一时间线按完整正文区域重算；原始时间线保持不变。

## 已知限制与下一片

- 本片测量为 Chromium 桌面、移动视口及触摸模拟的浏览器渲染证据；物理面板显示延迟、实体手机、系统选择手柄和实际高刷未测。WebKit/WebView2 的联合验证沿 FP7。
- FP3 手机弱网历史61ms长任务没有可靠归因，原样本保留。本片最终完整阅读与渲染记录均未复现，不反向改写历史结论。App 初始化阶段的长任务保留在原始记录，不归入翻页时延。
- API 延迟/离线由本地材料回执控制；资源数值是拥有的正文、解析载荷、DOM和图片名义面积，不是浏览器总堆/GPU内存。冷打开与缓冲外跳转保留准备等待。
- FP5 接分页原文选择、批注、公式、问答上下文、来源返回及持久读位；工具路由与进入退出接续已完成。PDF 沿 FP6，完整联合验收沿 FP7。
