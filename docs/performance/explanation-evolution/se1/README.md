# SE1：追踪一次离散更新

2026-10-16。状态：完成。基于 HEAD `402d9be` 的现有工作树、ex14.v4 及 continuous-scene/state 合同，实现 D1 的首个候选能力单元。最终源码和验收对应 `run-3`；SE2 为下一入口。

## 交付

- [语法卡](../../../../skills/presentation/methods/discrete-update/v1/card.md)：按当前状态、更新量、下一状态的混淆选用，说明配方、适用边界和改写责任。
- [完整作品](../../../../skills/presentation/methods/discrete-update/v1/example/index.html)：自包含 HTML/SVG/JavaScript，直接浏览器打开即可运行；同一状态驱动图形、读数、控件与保存恢复。
- [运行与适配说明](../../../../skills/presentation/methods/discrete-update/v1/example/README.md)和 [检查合同 se1.v1](../../../../skills/presentation/methods/discrete-update/v1/checks.md)：包含模型推导、Author 字段、预期观察和迁移时必须重验的内容。
- [候选清单](candidate-catalog.json)：登记 `discrete-update@1` 的四个文件、理解障碍、边界、技术 needs 与执行入口。[正式目录](../../../../skills/presentation/methods/catalog.json)暂为空。

作品固定 `f(x)=(x−2)²`、`x₀=4`、默认 `η=0.25`，支持 `0≤η≤1.2` 及前 8 次更新。方框、空心圆、实心点分别表示当前、下一和画面插值。前后步骤、播放/暂停、重置、参数对照和小数定位共享确定性渲染。参数变化回到初值，恢复精确小数位置时暂停，静止时隐藏整个更新箭头。

## 验证结果

| 检查 | 实际结果 | 证据 |
|---|---|---|
| Author write → preview → deliver | 3 环境各 7 批，共 21 批；每批至多 4 动作，均无预览错误，成功交付并导出内容 | [Author 结果](run-3/author-result.json)、[调用与截图](run-3/author/)、[运行日志](author-run-3.log) |
| 独立数值与实际图形 | 250 组通过；闭式 `2+2(1−2η)ᵏ` 对照页面递推、当前/梯度/更新/下一/插值/损失和 SVG 点、箭头、轨迹 | [完整结果](run-3/math.json)、[日志](math-run-3.log) |
| 边界 | η=0、.25、.49、.5、.51、.75、.99、1、1.01、1.2；k=0..8，p=0/.35/.999；终点 p=0 | 同上 |
| 受控坏例 | 测试副本把运动点冻结在 x₀；读数和 scene 正确，实际点坐标不符，被相同几何判据识别 | [坏例截图](run-3/frozen-geometry.png)、math.json 的 negative_control |
| Reader 三环境 | 320×420 touch、640×240 touch、960×720 mouse；真实触摸/鼠标拖动、按钮、键盘 End、播放暂停、播放中换参数、自动终点、重置均通过 | [Reader 结果](run-3/reader.json)、[日志](reader-run-3.log) |
| 保存与重开 | 每环境保存 2+.35、7+.999、8+0，重启存储宿主并重载页面，9 次均保持 η、位置与实际几何且 paused | 同上；每次含 saved 和 after |
| 布局 | 三环境无横向溢出，例子全部按钮和输入控件至少 44×44；手机刻度调大后重新预览，短屏正常滚动观察 | 以下截图观察 |

Author 由 `presentation_se1_tests.rs::se1_author_preview_and_deliver` 直接调用现有工具、真实 Chromium/Edge 预览与持久化路径，无 Provider 调用。最终候选为 `candidate-1792112248786449100-1`，交付引用为 `presentation-1792112248786416000-0@1`。[content.json](run-3/content.json)保存确切交付内容；测试断言入口源码逐字等于资产文件。

Reader 复用 `presentation_store_tests.rs::presentation_ex10_browser_host`，将上述完整内容导入隔离测试会话，再由真实 `AgentPresentation`、保存 API 和存储重开路径验证。它使用自身测试引用；`reader.json` 内保存实际回执。浏览器测试额外核对导出 HTML 与资产逐字一致。

## 截图观察

实际查看了 Author 的重复定位、静止、一步到达、等幅往返、发散和近终点截图，以及 Reader 三环境的图、读数和控件。

- [320 图与刻度](run-3/reader-320-graph-bottom.png)：刻度可读，方框/空心圆/运动点与更新箭头可辨；中文图例换行正常。
- [640 短屏图形](run-3/reader-640-graph-bottom.png)与 [同一状态的算式](run-3/reader-640-equation.png)：滚动后能分别观察图形和计算。η=.75、k=2、p=.35 时，算式为 `2.5+(-.75)=1.75`，画面插值为 2.2375。
- [960 图文](run-3/reader-960-graph.png)：原 Reader 宿主内图形与读数对应，同一参数下回退与重复定位尺度固定。
- [320 播放控件](run-3/reader-320-controls.png)与 [参数控件](run-3/reader-320-parameters.png)：触摸目标和滚动可达，保存后读数与控件一致。
- [Author 静止](run-3/author/short-content-stationary-3.png)、[一步到达](run-3/author/desktop-content-arrival-3.png)、[等幅往返](run-3/author/desktop-content-oscillation-3.png)：零更新保留点并隐藏箭头；往返中的插值与真实迭代读数明确分开。

## 复现

从仓库根目录运行第一步，并为每次新运行指定新的绝对输出目录：

~~~powershell
$env:SE1_OUTPUT = Join-Path (Get-Location) 'tmp/se1-new-run'
cargo test -p server --lib se1_author_preview_and_deliver -- --ignored --nocapture
~~~

第一步结束后，在同一终端启动现有 Reader 测试宿主（最多运行 300 秒）：

~~~powershell
$env:EX10_CONTENT = Join-Path $env:SE1_OUTPUT 'content.json'
$env:PRESENTATION_TEST_PORT = '4185'
cargo test -p server --lib presentation_ex10_browser_host -- --ignored --nocapture
~~~

另一个终端设相同的 `SE1_OUTPUT`，进入 `packages/web`：

~~~powershell
$env:SE1_OUTPUT = '上一步输出目录的绝对路径'
$env:SE1_READER_HOST = 'http://127.0.0.1:4185'
pnpm exec playwright test presentation-se1.spec.ts --workers=1
Invoke-WebRequest -Method Post -Uri 'http://127.0.0.1:4185/stop'
~~~

测试挂载：[Rust Author](../../../../crates/server/src/tests/presentation_se1_tests.rs)、[Playwright 数学与 Reader](../../../../packages/web/playwright/presentation-se1.spec.ts)。本次宿主已正常停止。

## 已知限制与保留记录

- 这是候选作品可用性的证据。正式编译发现与 `read_method` 在 SE2，使用版本追溯在 SE3，跨题收益和采用分别在 SE8、SE9。尚未发布、部署或运行付费模型实验，未验证读者学习收益。
- 单文件直接打开支持计算与交互；持久保存由 Reader 宿主负责。实际 Reader 短屏需滚动查看图和读数，图形近收敛时位置接近，精确值由旁边读数提供。
- `run-1` 保留初版通过的计算、Author 与 Reader 结果；首次观察发现手机刻度过小及短屏截图没有覆盖运动点。`run-2` 补观察位置后通过；`run-3` 为字号修正后的最终候选，重做正式预览、数学与 Reader 验收。
- 第二次编译曾因本次 Reader 测试宿主仍占用 Windows exe 而发生 LNK1104（[日志](author-run-2.log)）；停止该宿主后重试通过（[日志](author-run-2-retry.log)）。代码未为此增加兼容层。构建中的既有 ts-rs serde 警告保留在日志。
