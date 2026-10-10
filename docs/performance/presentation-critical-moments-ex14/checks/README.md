# EX14.2：关键解释时刻检查指导

2026-10-08，**通用/浏览器路径工程接入完成，A13–A18 通过，B 组 revision 为 ex14.v1**。真实提供方调用 0；自然任务收益与采用结论待 EX14.4。合同见 [EX14 §6](../../../切片方案-EX14-视觉方法指导与关键解释时刻检查.md)。

## 指导与冻结版本

- `phases/review.md`：观察关系真正发生的位置，按内容比较前后，核对图形、标签与读数；修改后复查当前候选，保留无法观察的现场及原因。原完整范围与 Goal 核对保持。
- `references/continuous-scene.md`：每批重建参数和活动演示；前提与目标操作合计不超过四个动作；无法到达时保留缺口；末动作读数与各自位置绑定。独立浏览器/Reader 补证不替代 Agent 观察或当前候选的三环境回执。
- Runtime `REVISION`、共同正文版本标识及指导 README 同步为 `ex14.v1`。模块身份、选择器、工具 schema、preview/媒体上限及状态合同保持。

[B/presentation/](B/presentation/) 保存全部 14 份指导；[manifest](B/manifest.json) 保存版本、范围与冻结时刻，[B/source/](B/source/) 保存 revision 入口、相关 Runtime 测试及图片回放夹具。相对 [A](../baseline/source/skills/presentation/) 的差异只有 review、continuous-scene、共同版本标识和 README；global、local、engineering 和 Manim 参考原文均保持。

B 回退到 A 时整体恢复相应指导及 Runtime revision，保留工程回归与历史内容。EX14.3 创建 C 时另存全文和实际输入，不改本目录的 B 快照。正式实验仍须冻结当时可运行的共同底座。

## 实际请求与验收

使用原 Resident 循环、脚本化 Adapter 和现有请求投影，保存 16 次请求计划、32 份 Native/ReAct 请求，以及压缩前后的采样消息。下表引用 [run-2](run-2/)；[verification.json](verification.json) 列出各次当前模块、追加事件数和 UTF-8 字节量。字节量不代表 token，用量字段为 null。

| 验收 | 本轮事实 | 证据 |
|---|---|---|
| A13 | 原纯文字请求未暴露 author，未载入演示指导，未产生写入或交付 | `plain/request-00.json`；`ex14_plain_request_has_no_author_or_presentation_guidance` |
| A14 | 首次 global 只有目录/共同合同；静态 local 无场景或媒体实现；返回 global 的新增指导仍只含 global | `selection/request-01/02/07.json`；历史已加载指导保留原追加位置 |
| A15 | continuous_scene 只带 state；Manim 带 state/continuous-scene/manim；Manim 原文逐字节等于 A | `selection/request-03/06.json`；模块单测及归档核对 |
| A16 | prepare 后的下一次采样生效；重复 prepare 的指导事件数保持 4；共同前缀不变；实际中途压缩后 review + continuous_scene 及当前设计保持 | `selection/`、`compaction/messages.json`；原 EX13 前缀回归 |
| A17 | EX14.1-B 五张真实 PNG 进入下一次 Native 和 ReAct 请求；每图保留环境、动作、位置与候选绑定，末态读数为 B / 6.50 / 第 4 动作 | `images/request-04.json`、`native-04.json`、`react-04.json` |
| A18 | 同批 preview+deliver 被拒绝；patch 得到 c2 后不能凭 c1 观察交付；Server 拒绝未预览候选，真实三环境旧候选可交付、新候选无资格 | Runtime 两条 `PRESENTATION_INSPECTION_REQUIRED`；Server 日志 |

图片回放夹具来自 [EX14.1-B desktop-content-fault](../browser/run-1/f2/desktop-content-fault-response.json)。PNG 与回执原样保存；回放到测试存储时只将候选 ID 映射为 c1。测试运行的是实际 Resident 请求装配，不执行新浏览器绘制、不调用模型、不证明模型自行识别标签错误。归档核对逐张比较 PNG 原始字节和 caption；Server 的真实浏览器复验另列。

## 测试结果与重跑

| 执行 | 结果 | 原始日志 |
|---|---|---|
| 新测试注册 | 4 项真实挂入 `presentation_authoring::ex14` | [registration.log](registration.log) |
| Runtime presentation 定向回归 | 87 passed，0 failed，0 ignored；包含新增 4 项、11 项模块测试及原阶段/图片/交付测试 | [runtime-run-2.log](runtime-run-2.log) |
| Server 未预览/来源/取消合同 | 1 passed | [server-contract.log](server-contract.log) |
| Server 三环境真实滚动预览与候选资格 | 显式运行原 ignored 测试，1 passed，0 ignored；3 环境 × 3 图 | [server-browser.log](server-browser.log) |
| 冻结内容与实际载荷核对 | B 全文差异、模块原文、A/B schema 相同及 5 张 PNG 来源通过 | [archive-verification-2.log](archive-verification-2.log) |

运行目的：注册检查发现漏挂或编译错误则修测试接入；Runtime 回归发现阶段串位、重复注入、压缩丢失或图片未送入下一请求则定位装配；Server 回归发现旧候选资格转移则定位交付；归档核对发现版本、schema、载荷或图片不一致则修正归档，禁止带着错配进入 C/付费对照。

```powershell
cargo test -p runtime --lib ex14_ -- --list
$env:EX14_GUIDANCE_OUTPUT = '<new absolute directory>'
cargo test -p runtime --lib presentation -- --nocapture --test-threads=1
cargo test -p server --lib presentation_author_requires_preview_and_current_source_bindings -- --nocapture
cargo test -p server --lib ex12_scroll_author_binds_images_reading_and_candidate_receipts -- --ignored --nocapture --test-threads=1
python docs/performance/presentation-critical-moments-ex14/checks/verify.py
```

录制使用新目录，已有同名结果拒绝覆盖。`verify.py` 核对本轮冻结的 run-2；新批次另存记录并更新其核对入口。

首轮 85 passed、2 failed：[runtime-run-1.log](runtime-run-1.log)。新测试误将结果 envelope 当成裸 body，并读取第一条而非最新阶段记录；实际 request 已包含正确读数，修正断言读取后通过，产品实现未因此修改。归档首轮因 Windows Path 排序与字符串预期顺序不同失败；改用集合比较，保留 [原始日志](archive-verification.log)。编译有既存 ts-rs serde 属性告警，未阻断本轮执行。

## 已知限制与接续

EX14.1-M 未执行，Manim 指导保持 A；C 组与 EX14.3 待实施。付费自然对照仍为 `blocked: paid experiment not authorized`，EX13 的 9 元额度不沿用。当前结果不支持指导收益、正式采用或学习效果结论。

EX14.1-B 的圆盘 r₂=1.5 仍为 Author preview 未覆盖，独立浏览器补证保持单列。Linux 隔离 worker、实体手机和 Reader 连续使用未在本轮新增验收，历史状态见 [Linux 发布记录](../../../Linux上线-EX12-EX13-JL.md)。本轮未提交、未部署；主视频 checkpoint 保持，接续入口已更新到 `checkpoint_ex.md`。
