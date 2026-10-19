# FP3 页面就绪、预取与回收

2026-10-14，FP3 本地交付完成，下一片 FP4。依据：[切片方案](../../切片方案-专注阅读双页与无感翻页.md)、[ADR-0166](../../adr/0166-focus-reading-pagination-and-performance.md)。基线 `6612932` 与已有 FP2 工作区；本片未提交或部署。

## 实施步骤

- [x] 正文合并只读来源、公式解释按需加载；正文不等待解释资料，公式入口始终可用。
- [x] 正式 App 分页宿主、可取消的就绪/预取调度、统一预算、逆向与重排邻屏、隐藏/手势暂停。
- [x] 正式生产 App 的 P3/P5/P6、延迟/断网/现场替换测试；保存完整结果并刷新架构、代码链路、checkpoint。

## 验证目标

正文/字体/图片延迟验证就绪依赖，失败保留旧画面；公式资料延迟验证正文独立与按需入口。正常每秒一次 200 次（含反向/跨章）检查未就绪次数；固定 100ms RTT / 10Mbps 与断网检查预取和已准备屏。至少 20 分钟、500 次有效换屏检查上游正文、解析、页面和准备区的合计峰值。切换关键路径核对读取/解析/几何计数；只读预取期间监测全部 API 写入。

任一失败保留原始结果，修复所属链路后重跑受影响组。

## 实现与接入

```text
App: 现有现场 / source_fingerprint / manifest / assets
  → FocusSource（只读合并窗口、未消费正文尾部）
  → ReaderPane.prepareFocus（释放连续正文与旧排版等待）
  → FocusReaderPane（尺寸、字体、可见性与生命周期）
  → FocusReader（当前请求、串行补页、取消、暂停与失败状态）
  → ReaderPagination（原文/解析/页面预算及冻结屏）
```

`FocusSource` 每次读取最多八个相邻叶，合并跨度通常不超过 32KiB；必需的大叶单独读取。请求前为合并正文与拆分字符串共同预留容量，读入时只缓存未消费的尾部；消费的原文随分页器解析缓存持有。现有 `readWindowText` 仍沿原接口工作，新增的索引工厂供一个来源版本复用。GET 取消信号贯穿本地 HTTP 与多用户发布材料读取。

`FocusReader` 先准备当前屏，随后补前后邻屏，再向阅读方向多准备一屏。调用 `move()` 时就绪目标直接显示；准备工作排到后续任务。未就绪目标保留当前屏并进入等待；新目标覆盖旧请求，旧读取、字体等待、图片解码等待和可让出的来源对齐均可取消。隐藏宿主停止准备；`setPaused()` 把准备批次让给手势，恢复后接续。来源/现场更换释放宿主，来源版本变化沿 App 的重新读取路径装载最新 manifest 与资产。

前后相邻以规范 UTF-16 游标判断。最近 128 组边界用于重建已回收页，仅保留起止游标，并计入解析预算。字号/几何重排清空旧几何边界；冷跳转或缺失边界时，从当前游标前的局部叶重建，精确止于当前屏起点。准备区、上游正文尾部、边界记录、原文、HTML、来源映射、页面 DOM 和解码图片共同受 FP1 上限约束。

正文及 KaTeX 字体、图片解码完成后才发布页面。图像解码失败会移除图片地址并形成稳定的“图片暂不可用”内容，晚到资源不能挤动已发布页面。宽表复用 `reader-table / asset-table` 的不折行和横向滚动样式。公式正文先渲染；公式弹窗和右侧“公式”标签共用 `loadFormulaProfile` 按需取解释，显示等待、缺资料、错误与重试，关闭/来源换代或选择其他公式使旧回执失效。右侧标签打开及选中公式改变时触发读取，普通正文加载不请求解释资料。

## 验收记录

Windows 桌面、Chromium 149.0.7827.55、生产构建。真实材料沿 FP1 的 EPUB、复杂数学材料与 Kimi Markdown；连续阅读使用复杂材料第 4–5 章的 240 叶，另外两份材料各覆盖 20 次往返。固定弱网为 API 100ms 往返延迟和静态资源 10Mbps 限速。公开交互由下一片接入，测试直接驱动本片的准备接口。

- 8 个相关测试文件共 **189 项通过**；正式 build（含应用类型检查）、独立生产入口 build 和该入口的类型检查通过。
- **9 项短浏览器场景通过**：只读来源与切换工作量；离线保留画面；最新正文目标与隐藏取消；后台补页重试；冷字体、连续输入和重排反向；真实 EPUB/Kimi；图片解码；解释延迟与来源版本替换。
- **P5 长测通过**：1,202,034ms（20 分 2 秒）、588 次有效换屏、零未就绪；每 100 次修改字号，并持续反向与跨章。初始启动记录两次长任务（89ms、146ms），正常阅读未记录 ≥50ms 长任务。原始结果见 [long-reading.json](results/long-reading.json)。最后对公式侧栏补上的按需加载不经过分页或资源链路，单独由 App 和 RightRail 回归覆盖。
- **P3/P6 通过**：四组共 800 次有效换屏，零未就绪；切换调用内来源对齐、分页准备、几何探测计数均不增加。只读预取未触发读位或已读写入。最终日志 [normal-reading.log](normal-reading.log)，机器汇总 [summary.json](results/summary.json)；原始每次输入样本保存在同目录的四份 `normal-*.json`。

| 视口 / 网络 | 有效换屏 | 未就绪 | 同步切换 P95 / 最大值 | 输入开始后 ≥50ms 长任务 |
| --- | ---: | ---: | ---: | ---: |
| 1440 / 本地 | 200 | 0 | 1.5 / 2.5ms | 0 |
| 1440 / 固定弱网 | 200 | 0 | 1.7 / 4.6ms | 0 |
| 390 / 本地 | 200 | 0 | 0.9 / 1.7ms | 0 |
| 390 / 固定弱网 | 200 | 0 | 1.1 / 7.2ms | 1（61ms，见已知限制） |

| P5 资源 | 实测峰值 | 冻结上限 |
| --- | ---: | ---: |
| 屏 / 页（含准备区） | 4 / 8 | 4 / 8 |
| 正文与读取尾部 | 25,972 B | 524,288 B |
| 解析载荷及边界元数据 | 843,624 B | 2,097,152 B |
| DOM 节点 | 8,752 | 12,000 |
| 解码图片像素 | 876,400 | 8,000,000 |
| 同时准备任务 | 1 | 1 |

同步切换测量只用于确认已就绪屏的调用成本；物理呈现时延保持未测。完整日志与此前失败均随本目录保留。

日志入口：[189 项测试](unit-tests.log)、[正式构建](production-build.log)、[生产 App 入口构建](app-fixture-build.log)、[入口类型检查](app-fixture-types.log)、[初始短场景](short-initial.log)、[重排与后台重试](reflow-and-retry.log)、[真实材料及修正前断言](failures/optional-cursor-lid-assertion.txt)、[含长测的原始套件日志](long-reading-suite.log)。短场景按实际修补重跑受影响组，九项通过来自这些结果的合并。长测所在套件曾人为中断一个旧候选 worker，因此套件总计显示 1 failed / 4 passed；最终长测自身通过，四组正常翻页另用最终候选重跑，不能将该旧套件标为全通过。

## 运行入口

从仓库根运行。材料沿 FP1 已生成的三份真实来源与本地图像。

```powershell
pnpm --filter @understand-book/web build
pnpm --filter @understand-book/web exec vue-tsc --noEmit -p playwright/tsconfig.focus-ready.json
pnpm --filter @understand-book/web exec vite build --config vite.focus-ready.config.ts
pnpm --filter @understand-book/web exec vitest run src/reader-focus.test.ts src/reader-pagination.test.ts src/reader-window-text.test.ts src/components/ReaderPane.test.ts src/components/RightRail.test.ts src/App.startup.test.ts src/network-client.test.ts src/markdown-source-map.test.ts
pnpm --filter @understand-book/web exec playwright test --config playwright.focus-ready.config.ts --grep-invert 'normal 200|long reading' --reporter=line
$env:FP3_LONG='1'
pnpm --filter @understand-book/web exec playwright test --config playwright.focus-ready.config.ts --grep 'long reading' --reporter=line
pnpm --filter @understand-book/web exec playwright test --config playwright.focus-ready.config.ts --grep 'normal 200' --reporter=line
node packages/web/playwright/fixtures/summarize-focus-ready.mjs
```

独立生产 App bundle 位于 `tmp/focus-reading-fp3/dist`，固定端口 4190；`FP3_PORT / FP3_OUTPUT` 可指定独立端口和证据目录。夹具直接挂载生产 `App.vue`，仅通过 `ReaderPane` 公开的 FP3 准备接口驱动操作；真实 API 路径使用可控制延迟/失败的只读材料服务回执，App 的读取、组件、排版、预取与取消均为正式代码。

## 失败与修复

- 初始接线将来源对象命名为已有来源导航函数的同名符号，构建立即报错；已改用 `focusPaginationSource`。[构建失败](failures/initial-build.txt)。
- 首批取消检查把最初读取移到后续任务，旧生命周期测试还在读取开始前调用回调；保留首次读取的原时序，并在异步边界检查取消。[初始单测](failures/initial-unit.txt)。
- 正式入口的专注按钮与阅读设置使用实际 TopBar，测试最初定位了隐藏的 Workspace 控件；测试改走可见按钮及“展开工具栏”。[专注入口](failures/initial-app-entry.txt)、[阅读设置](failures/settings-selector.txt)。
- 接入过程中确认连续分支的惰性计算仍持有旧段落数组，切入分页后主动消费空结果，并取消旧字体等待。先前启动的长测在候选修订时终止，保留[当时采样](failures/pre-release-fix-long-progress.json)，不将其计为验收。
- 实际 App 截图显示宽表首列逐字折行；分页渲染补齐既有表格样式类，使用原有横向滚动。[修复前画面](failures/table-before.png)。
- 延迟字体期间翻页取消了整个重排，旧几何直接成为最终画面。增加独立的最新排版要求，输入只改目标读位，重排完成前保留仍可用的旧邻屏；真实浏览器先复现 `waiting → ready` 错误，再验证旧字体请求取消、最新字体继续等待与改向连续性。[失败回归](failures/reflow-input-red.txt)。
- 后台预取失败时没有前台失败目标，重试按钮原先没有重新排程；重试现在同时支持前台目标和后台补页，恢复网络后当前屏不变。
- 公式解释移出正文读取后，右侧公式标签仍依赖正文附带的资料，导致始终为空。新增先失败的 App 集成用例，再把标签打开、选择变化和重试接入共用的按需读取；App/RightRail 回归通过。[失败回归](failures/formula-panel-red.txt)。

## 已知限制与下一片

- 公开专注按钮、键盘、拖动、覆盖式翻页和退出接续由 FP4 接线；本片交付 App 的来源适配、准备宿主及调用接口。分页后的笔记/选区/公式操作与问答上下文按 FP5 接入。
- 分页基础仍逐叶排版，被导入为独立叶的行内公式会额外换行；实际 App 画面可见这一点。FP4 接公开显示入口时应复用 `ReaderPane.readerItems` 的行内分组，再执行该片的 P3 回归。
- 最终手机弱网组在页面时间 2354.2ms 记录一条 61ms 主线程长任务，位于第 2 次输入后、第 3 次输入前；该组 200 次均已就绪。随后使用相同来源/视口/网络进行 20 次带 CDP 时间线的短程检查，未复现输入后的 ≥50ms 长任务，未取得原始长任务的可靠归因。原始样本不删除，也不据短程结果宣布 P2 通过。FP4 的实际交互 P2 验收须继续覆盖这一早期阶段。[定位日志](task-attribution.log)、[定位时间线](results/mobile-weak-task-attribution.json)。
- 冷打开、冷字体、图片与远距离读取保留完整等待；固定弱网使用 100ms API 往返延迟与 10Mbps 静态资源限速，服务器和数据回执在本地受控。
- 资源数值是所拥有载荷与 DOM/图片名义面积；不代表浏览器总堆/GPU 内存。实体手机、系统手柄、实际高刷及物理显示时延沿既定未验证范围。
