# EX14.1-B：浏览器关键现场与可达性

2026-10-08，**EX14.1-B 完成，A06–A09、A11 通过**。采用现有 Author → BrowserPreview 路径，产品底座、工具 schema、指导正文及 ex13.v2 revision 保持。新增测试、F2-B 夹具及单独的圆盘浏览器补证。提供方调用 0。

## 结果与证据

| 验收 | 实际结果 | 证据 |
|---|---|---|
| A06 前后位置与读数 | 分别以 6.25、6.50、6.75 结束独立批次；每份末态 reading 绑定第 4 动作、活动 B、实际位置与单位 | [F2 验证](run-1/f2/verification.json)，对应 `*-request.json` / `*-response.json` |
| A07 新页面重放 | 每批滚动 → 选择 B → 4.25 → 目标，共 4 动作；每次初始 DOM 都为 A / 0.00 | 同上，9 组前后观察；每批初始与逐动作 PNG 保留 |
| A08 活动演示 | 蓝色 B 和橙色 A 的截图像素位置与身份一致；省略选择 B 的负向批次实际仍为 A | [遗漏选择回执](run-1/f2/omitted-selection-response.json) |
| A09 小数、回退、重复 | 三环境均执行 B → 6.50 → 2.25 → 2.25；物体像素、暂停状态正确，轨迹回退到 45×4 像素，无未来轨迹残留 | `*-rollback-response.json` 与 PNG |
| A11 能力边界 | 第五动作和未知 `set_value` 被拒绝；圆盘连续两批各 4 次左键均为 1.96，中心点击后 seek 保持半径 2 | [合同测试日志](run-1/contracts.log)、[圆盘验证](run-1/disk/verification.json) |

正式环境均显式使用 viewport：320×420 touch、640×240 touch、960×720 mouse。F2 共 14 次原生 Author preview、66 张逐动作截图；圆盘 3 次桌面 Author preview、13 张截图。全部回执无脚本错误、水平溢出或操作问题。F2 当前候选取得三环境回执；带有植入内容故障，未执行 deliver。

原生观察为测试代码调用工具所得，**没有模型自行观察、判断或修复**。每份图片另保存 candidate_id、environment_name、caption（含动作与位置）；后续模型请求是否收到图片归 EX14.2 验证。

## 受控故障与画面判断

[F2-B 源码](../../../../crates/server/tests/fixtures/presentation-ex14-f2-b.html) 含 1100px 前段正文、两个真实选择按钮、实时 SVG 与语义定位接口。标签故障人为植入 B 的 6.4–6.6 区间，本轮观察 6.25 / 6.50 / 6.75：分别为 B / 错标 A / B。

- [短屏故障画面](run-1/f2/short-content-fault-4.png)：活动 B、位置 6.50，图形上的标签错误显示 A。
- [窄屏变化后](run-1/f2/narrow-content-after-4.png)：位置 6.75、图形标签 B。
- [冻结图形版本](run-1/f2/frozen-geometry.html)：由同一夹具仅将 `freezeGeometry` 改为 true；[回执](run-1/f2/frozen-geometry-response.json) 报告 B / 6.50 / 暂停，但 [截图](run-1/f2/frozen-geometry-2.png) 的方块停在 0。

测试从 PNG 解码真实像素：核对固定颜色方块的面积和横坐标，以及回退后的轨迹面积。冻结图形的负向样本必须被这条断言识别；不会因为 scene snapshot 正确就通过。该断言只适用于本夹具，未扩展为任意教学图的自动内容判定。已有工具回执本来不负责判断图示语义，未发现需修复的产品底座缺陷。

## 圆盘不可达与独立补证

使用 EX11.7 batch12 `disk-1/delivered-raw.html` 原文，经现有 Konva 选择与 write 保存为候选 `candidate-1791458119491758500-1`；[完整候选](run-1/disk/candidate.json) 包含 HTML 和确切库资源。

半径控件初值 2，范围 1–3，步长 0.01。到 1.5 需要 50 次左键；Home 到 1、End 到 3，均不能在剩余动作中到达 1.5；中心点击落在 2，seek 只改角度，原页面没有可用的半径快捷按钮。两次四动作预览均从 2 开始、止于 1.96，证明确实不能跨批累积。该参数现场标为 **Author preview 未覆盖**。

[独立补证](run-1/disk-supplement/verification.json) 从上述完整候选加载原文与库，在同一页面对真实滑块发送 50 次 ArrowLeft，再 seek 到 2 + 0.25。Edge 154.0.4258.62 三环境通过：r₂=1.5，θ=2.25π/4，暂停；盘径比、两盘辐条与扇形、两条弧长条和三位小数读数均符合原数学关系。保存 3 张全页截图，无脚本错误及水平溢出。

补证使用独立浏览器操作，不受四动作工具预算约束。它与 Author 观察使用同一候选；不计为 Agent 自行检查，不产生 Author 三环境资格，也不替代正式 Reader 使用验收。

## 重跑与原始日志

运行目的：注册命令检测测试漏挂或编译失败，失败则先修测试接入；合同测试检测第五动作/未知动作被错误接受，失败则定位 Author 或反序列化边界；浏览器测试检测现场重放、读数、图形不一致，失败则区分夹具与底座；圆盘补证检测目标参数下的真实几何错误，失败则保存原记录并定位原页面。

```powershell
cargo test -p server --lib tests::presentation_author_tests::ex14:: -- --list
cargo test -p server --lib tests::presentation_author_tests::ex14:: -- --nocapture
$env:EX14_BROWSER_OUTPUT = '<new absolute output root>'
cargo test -p server --lib tests::presentation_author_tests::ex14::ex14_browser_ -- --ignored --nocapture --test-threads=1
node docs/performance/presentation-critical-moments-ex14/disk-supplement.mjs $env:EX14_BROWSER_OUTPUT
```

测试子目录已存在时拒绝覆盖，重跑用新根目录。原始记录：

- [首次注册失败](registration-failed.log)：测试误用要求 `AuthorResult: Debug` 的 `unwrap_err()`；改用 `err().expect()`，产品类型不变。
- [注册成功](registration-passed.log)：3 项确实挂入 `presentation_author_tests::ex14`。
- [合同测试](run-1/contracts.log)：1 passed；2 个环境测试在该次调用 ignored。
- [显式浏览器测试](run-1/browser.log)：2 passed，0 failed，0 ignored，71.81 秒。
- 独立补证：1 次脚本、3 环境 passed，见上方 JSON；成功日志为空，状态由 JSON 和退出码共同确认。

开始前对冻结 manifest 的 63 个文件逐字节比较；差异仅为 EX14.0 已追加的测试父模块及完成文档/checkpoint，浏览器、Author、Runtime 指导及 A 原文保持冻结内容。新增挂载保留父文件已有修改。新测试最后仅经 rustfmt 排版，未改语义。

## 已知限制与接续

EX14.1-M 未执行；Manim 原文继续作为三组共同基线。EX14.2 可接入通用检查指导，EX14.3 可随后接入浏览器/静态视觉方法。付费模型实验仍未授权；B/C 尚未形成，没有采用或学习效果结论。

当前结果限于本机 Windows / Edge 的直接 BrowserPreview；未新增 Linux 隔离 worker、实体手机、真实 Reader 连续使用或媒体证据。EX12.5 历史未完项沿用 [Linux 发布记录](../../../Linux上线-EX12-EX13-JL.md)。
