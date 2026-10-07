# EX11.8：真实模型局部修订验证

日期：2026-09-29。用户确认模型服务恢复后，使用当前正式 Runtime/Server、DeepSeek v4 Flash 和制作方法 ex11.v11，从原模型页面及其真实会话历史发起修订。生产实现和方法未再修改。输入、源码、模型请求、响应、压缩与用量保存在 [batch13](ex11-local-demonstrations/ex11.7/batch13/)；所有样本均为修订诊断，独立生成计数为 0。

阶段结论：EX11按工程能力建设阶段收口。EX11.7完整跨题质量合同未达成，遗留项转后续质量工作；本报告测试事实、失败及判定保持原样。收口口径见[方案§13](../切片方案-EX11-Konva通用能力与局部动态演示.md#13-工程阶段收口2026-09-29)。

## 学习率页

基准为 batch8/runs/learning-2 原稿。请求仅修公开 seek 的参数包装和静态推导中 η=0 的表述，其他内容与资源保持原样。模型请求中确认实际加载 ex11.v11。

实际流程：首次 read 因工具未暴露而失败 → tool.search 激活 → 6 次源码搜索 → 两段有效 read（offset 7150/length 2500、offset 26900/length 1200）→ patch。前两次 patch 分别误传 based_on 和 asset_refs，收到真实校验反馈后自行修正；第三次成功，生成一个新候选。三视口 preview 后 deliver，最终回合 `incomplete=false / warning=null`。

前后内容比较确认：入口 HTML 只改变指定句子和 seek 包装，其余 HTML、Konva、许可、静态 SVG、绘图源码/数据与其他内容字段一致，没有 write 整页重写。

- 315 组独立数值和实际图形位置全部通过，包含 η=0 的静止边界。
- Reader 三视口的鼠标/触摸拖动、播放暂停、回退、重复定位、保存、重开和追问全部通过。
- 11 次聊天模型请求、1 次历史压缩；用量完整，544,564 tokens，120.067 秒。
- 同一原页的 batch12 修订诊断曾 read 24 次、未交付，700,675 tokens、335.925 秒。本次有效读取为两段共 3,700 字符。两次输入末段分别要求旧 write 路径及按当前工具修订，属于观察对照，不是严格 A/B。

结果为指定源码修复及操作链通过。保存的 readable_content 仍保留“η<0.5 走不到（同侧接近）”，含 η=0，模型未同步修正这条原有摘要表述。该问题保留为未完成项。

证据：[判定](ex11-local-demonstrations/ex11.7/batch13/diagnostics/learning-2-repair/verdict.json)、[源码差异](ex11-local-demonstrations/ex11.7/batch13/diagnostics/learning-2-repair/source-diff.txt)、[完整内容审计](ex11-local-demonstrations/ex11.7/batch13/diagnostics/learning-2-repair/revision-audit.json)、[Reader](ex11-local-demonstrations/ex11.7/batch13/diagnostics/learning-2-repair/reader-check.json)、[数学检查](ex11-local-demonstrations/ex11.7/batch13/diagnostics/learning-2-repair/independent-math.json)。

## 圆盘页

基准为 batch12/runs/disk-1 原稿。原生滑块右端量化为 θ=6.283，页面显示 360.0°，却仍处在第7步+0.999764。新端点检查在三视口真实鼠标/触摸拖动上复现此失败，证据保存在 [原页基线](ex11-local-demonstrations/ex11.7/batch13/checks/disk-baseline/endpoint-check.json)。

首轮真实修订完成一次 read、一次 patch、三次 preview 和交付，同时更新文字摘要，生成 revision 2。read 一次完整返回24,819字符，未强制分片；6次聊天请求与1次压缩，用量342,570 tokens，132.786秒。84组数值/几何与常规Reader三视口通过。真实滑块右端、播放终点和终点重开都准确到达2π、8步、进度0。

端点检查发现新回归：模型也把 `snapEndpoint` 用于现场恢复，使合法的第7步+0.999（θ=6.282399909016188）重开后变成整圈（θ=6.283185307179586）。三视口全部复现；普通第2步+0.35能正确恢复。该连续位置属于既有公开 seek 与保存合同。

将精确失败交回真实 Agent 后，它以9次search、3次局部read、1次patch和3次preview完成revision 3。相对revision 2，源码只修改恢复赋值及邻近注释，同时更新摘要；滑块输入、图形、其余控件、资源和其他字段一致。10次聊天请求与1次压缩，用量355,433 tokens，98.027秒，回合正常结束。

revision 3 三视口复测全部通过：真实鼠标/触摸滑块右端、整圈保存重开、播放到终点、普通2+.35和接近终点7+.999的精确保存恢复，以及完整Reader操作/保存/追问。图形和数值计算未被后续修订修改，84组数学证据来自revision 2，未重复执行。

证据：[首轮判定](ex11-local-demonstrations/ex11.7/batch13/diagnostics/disk-1/verdict.json)、[首轮差异](ex11-local-demonstrations/ex11.7/batch13/diagnostics/disk-1/source-diff.txt)、[端点及恢复检查](ex11-local-demonstrations/ex11.7/batch13/diagnostics/disk-1/endpoint-check.json)。

最终证据：[revision 3 判定](ex11-local-demonstrations/ex11.7/batch13/diagnostics/disk-1-restore/verdict.json)、[后续差异](ex11-local-demonstrations/ex11.7/batch13/diagnostics/disk-1-restore/source-diff.txt)、[端点及恢复通过](ex11-local-demonstrations/ex11.7/batch13/diagnostics/disk-1-restore/endpoint-check.json)、[Reader通过](ex11-local-demonstrations/ex11.7/batch13/diagnostics/disk-1-restore/reader-check.json)。

## 成本与结论

| 实际修订 | 聊天请求 / 压缩 | 完整报告用量 | 模型运行时间 |
| --- | --- | --- | --- |
| 学习率两处修复 | 11 / 1 | 544,564 tokens | 120.067秒 |
| 圆盘端点首修 | 6 / 1 | 342,570 tokens | 132.786秒 |
| 圆盘恢复回归修复 | 10 / 1 | 355,433 tokens | 98.027秒 |
| 合计 | 27 / 3 | 1,242,567 tokens | 350.880秒 |

三次均实际交付，均无整页write。局部搜索/读取、预算内全文件读取、精确补丁与反馈后的再次局部修订都已由真实模型使用，内容差异及Reader验证已落盘。圆盘首修的回归说明局部补丁只约束改动范围，修改正确性仍需按具体行为验证。全部模型进程结束，Reader host已停止。

完整账目见 [cost-summary.json](ex11-local-demonstrations/ex11.7/batch13/cost-summary.json)，总体判定见 [verdict.json](ex11-local-demonstrations/ex11.7/batch13/verdict.json)。

## 已知限制

- 这些记录证明实际 Agent 使用了 read/search/patch 并能够交付局部修订；两个题目不代表一般任务上的稳定性。
- 原失败与原稿保留。未手工修改模型生成页，修订反馈通过正式会话入口交给模型。
- EX11.7 的六个独立生成与 plain 对照未在此批次执行，短屏媒体及机制生成等旧问题仍未完成；学习效果未评估。
- 本次源码变更仅为新增端点浏览器检查；未提交、未部署、未更新桌面安装包。
