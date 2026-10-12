# TC2 — 可信选区直接帮助与首次教学入口

日期：2026-10-11（Asia/Hong_Kong）。依据 [ADR-0163](../adr/0163-tutor-continuation-and-contextual-reading-entry.md)与[切片方案 TC2](../切片方案-Tutor下一步协商与低门槛阅读入口.md#tc2可信选区直接帮助与首次教学入口)。

状态：**TC2 工程实现与定向验证完成，真实模型两步链路通过；内容质量未验收，目标读者验证未进行，完整 TC2 仍保留未勾选。**基于 `12b765bb33fe42fcd30fcda743187241ed56d1f3` 的现有工作树，保留 TC0／TC1 及其他在途修改，本片未提交、未部署。

## 1. 交付行为

Markdown 和已解析的 PDF 选区新增“帮我弄懂这段”。点击直接提交可信来源范围、版本和独立 UI 动作，message 可为空。原“问 AI”入口保留，未发送的文字／选区草稿独立保存。

首次解释下方提供“开启连续教学”，明确对所有书籍生效；开启只修改原 Tutor 控制状态。开启成功且资料 ready、会话可用后，“继续理解这段”才发起新的请求，引用原回答及点击时当前聊天／head。未就绪、暂停、结束或当前会话属于其他材料时，服务端拒绝继续，不替换已有会话。普通选区解释在关闭／未就绪时仍可使用。

当前待答题及其帮助关联的回答／原文，在接纳 Run 前分流为原题帮助。适用来源式与旧对象式活动；关闭／暂停仍保留原题归属。HTTP 回复包含 delivery_refs 与原聊天／回合定位，界面可跨聊天“前往原题帮助”，保留未发送草稿。定位本身不写 HelpDisplayed；真正使用提示／直接讲解继续走原活动合同。

## 2. 来源、事实与接纳

- 选区必须是完整 resolved 范围并匹配当前来源标识／版本。旧显示字符串、部分范围、换书／版本过期在接纳前拒绝。
- 接续引用本聊天已成功交付的公开回答；冻结原公开文字、原文范围与用户实际补充。历史普通回答复用其存储的选区或已核验 source bindings，不从新视口猜范围。
- 公开聊天显示操作标签；模型收到明确的 UI 动作与来源上下文。空动作不经过要求非空自由文本的画像意图判定。学习事实使用 reading_action，按钮文字及助手摘录不冒充用户能力表达；没有实际补充的点击不能单独支撑理解假设。
- 未写补充的直接帮助必须按 current_action 直接解释，不被会话默认探问覆盖。Tutor select 仍须先读取完整来源。
- 本地把原请求 key、归一化输入和解析快照共同写入 TurnDomain／turn.accepted。同 key 同内容回原 Run，不重看当前 head／来源；不同内容冲突。新请求须绑定点击时聊天与 head。未知执行在重启后标记中止，不自动重放。
- 多人动作先完成候选回合接纳，再冻结教学准备并保存 TurnPrepared，然后追加教学事件。pending／teaching_receipt／prepared／queued 的恢复复用原快照，避免重复建会话、重复模型执行。ActivityHelp 不入执行队列。

## 3. 验证与证据

每项检查针对具体失败：服务端验证来源与事实边界、去重／故障恢复；Memory 验证纯动作不会支持能力假设；Web 验证元数据传递、草稿和原题导航。失败时修改相应实现或夹具，构建环境中断单独记录。

| 检查 | 结果与证据 |
|---|---|
| Server Tutor 定向回归 | **63 通过、8 忽略**；[最新日志](tutor-continuation-tc2/interface-server-final.log)。忽略项含本片另行运行的真实模型测试、TC4／TC5 待办及既有人工／浏览器宿主测试 |
| Memory 存储／事实 | **149 通过**；[日志](tutor-continuation-tc2/memory-complete.log) |
| Runtime Tutor 合同／导出 | **24 通过**；[最新日志](tutor-continuation-tc2/interface-runtime-final.log) |
| Web 类型检查 | 通过；[日志](tutor-continuation-tc2/web-typecheck-final.log) |
| Web 输入／控制／回答／启动回归 | **4 文件、136 项通过**；[日志](tutor-continuation-tc2/web-regression-verified.log) |
| Playwright | **12 通过、1 跳过**（TC5 回答摘录）；[日志](tutor-continuation-tc2/browser-final.log) |
| 两步真实模型 | **工程链路 1 项通过**（内部两次请求，最终工具错误 0）；[最新日志](tutor-continuation-tc2/interface-live-model-final.log)与[结果摘要](tutor-continuation-tc2/interface-live-model-final.json)。整体内容质量未通过，见 §5 |

浏览器使用真实 App、Markdown 选区与 PDF.js 文本层，API／回答为替身；覆盖 Markdown／PDF × ready／未就绪、默认关闭→首次帮助→开启→继续、开启失败不发送、原题分流不残留虚假回合及跨聊天定位保留草稿。

[首次帮助截图](tutor-continuation-tc2/first-help.png)与[接续截图](tutor-continuation-tc2/continue-answer.png)记录原文引用、独立动作与保留的草稿。截图已检查布局；它们证明入口行为，不证明讲解质量。

复现命令（仓库根目录）：

```powershell
$env:CARGO_TARGET_DIR='C:/Users/Lenovo/.codex/tmp/understand-book-tc2-target'
$env:CARGO_PROFILE_DEV_DEBUG='0'
$env:CARGO_PROFILE_TEST_DEBUG='0'
$env:CARGO_INCREMENTAL='0'
$env:TEMP=(Resolve-Path 'tmp/tutor-continuation-tc2').Path
$env:TMP=$env:TEMP
cargo test -j 1 -p server --lib tutor
cargo test -j 1 -p memory --lib
cargo test -j 1 -p runtime --lib tutor
cargo test -j 1 -p server --lib continuation_tc2_live_first_help_enable_then_continue -- --ignored --nocapture
pnpm --filter @understand-book/web typecheck
pnpm --filter @understand-book/web test src/network-client.test.ts src/useTutorControl.test.ts src/components/RightRail.test.ts src/App.startup.test.ts --maxWorkers=1
pnpm --filter @understand-book/web exec playwright test playwright/tutor-continuation.spec.ts --workers=1
```

环境记录：D 盘构建曾拒绝执行辅助程序，E 盘曾空间不足；将本片沿用的 `tmp/tutor-continuation-tc1/target` 缓存移至上述 C 盘位置保留。最终回归期间机器另有构建任务，并行运行曾引发 LLVM 内存不足及前端 IPC 关闭；改为单路验证。以上中断不计为行为通过。较早行为运行发现空动作进入画像判定导致 INVALID_MEMORY_INTENT，修复后原 3 项失败转绿。

## 4. 真实模型发现与修正

两步真实请求使用 `.env` 所配 DeepSeek V4 Flash、隔离临时存储及相同平均速度材料，从默认关闭开始，message 均为空。首次试验显示两步均读取了来源，但接续没有交付 TeachingMove：模型先后尝试用纯 reading_action 与助手历史建立理解假设，校验拒绝后又改写引用重试。原始失败见 [日志](tutor-continuation-tc2/live-model-verified.log)与[摘要](tutor-continuation-tc2/live-model-before-guidance.json)。

修正：Runtime 对冻结 current_action 增加明确指导——纯点击没有理解假设证据，保持理解未知即可继续；evidence 不是 select 的前提；active 时读取全文后按 current_action 选教法。事实拒绝信息也说明不要改换引用重试。继续回答须将原回答作为待核对的模型输出；发现原回答错误应明确纠正。

该样本首答还误称“只有各段速度相同才可用算术平均”，等时间且速度不同同样可以；接续给出了正确的等时间反例，却未明确撤回原说法。这是实测内容问题，不能将工程链路通过等同于内容合格。第二轮不再从纯点击创建假设，并明确撤回上一轮无依据的“高速段权重小”说法，但 select 携带尚未开放的 continuation，重试又使用了 ContinueAnswer 不接受的 direct_explanation 意图，仍未交付活动。补齐指导与工具字段说明：continue_answer 采用 neutral（可配 explain），TC2 不输出 continuation。Runtime 断言固定该意图边界。见[第二轮摘要](tutor-continuation-tc2/live-model-neutral-gap.json)。最终两步链路通过：默认关闭时首次解释没有创建会话，显式开启不发送问题，继续请求读取原文并以 neutral/current_action 成功选择、交付教学活动。两次 outcome.incomplete 均为 false；tokens_spent 分别为 54,020 和 66,336（120,356 合计），不是请求数或费用金额。日志没有返回货币费用。

最终样本仍未达到内容质量验收：首答尝试在关闭时 select（一次参数形状错误、一次被控制边界拒绝）；接续将“用时相等”说成算术平均成立的必要条件，遗漏速度相等的情形；等距离推导出现 `d=d/v` 笔误，应为 `t₁=d/v₁`。本片没有为某一道材料硬编码答案或放宽校验。后续内容验证须从这些具体失败继续，而非仅重复成功的链路断言。

第二轮讲解仍有内容问题：“段数越多落差越明显”缺少成立条件；“总时间为正”等同于每段时间都非零也不成立。这些属于真实模型内容质量缺陷，已保留样本，后续质量验证不得跳过。

## 5. 工具合同与程序责任修正

前三轮实测中的关闭时 select、未开放 continuation、继续动作的意图来源失败，根因是本轮工具合同与实际能力不一致，并把程序已知的信息交给模型重新猜测。前述指导词修正是过程记录，本节取代其中关于字段填写和能力开放的现行做法。

- `ToolRegistry::with_tutor_context` 同时收窄模型采样参数和执行校验。observing 仅 trace／understanding／hypothesis evidence，reference 仅 trace，inactive／paused 无 Tutor 工具；active 不暴露尚未开放的 continuation。说明与路由卡同步采用本轮能力。
- `tutor::action_intent` 从服务端冻结的 ResolvedInteraction 和会话上下文解析确定意图。无补充的直接帮助采用 direct_explanation/current_action；继续动作保留显式会话默认，否则仍需解释已确认偏好，无两者时采用 neutral/current_action。实际补充继续由模型解析。
- 确定字段不再进入模型请求参数，由 `teaching::step` 在反序列化、原有来源和教法校验之前补齐。完整 TeachingMove 类型和旧数据读取保持原合同。模型选择 explain 不等于用户请求 direct_explanation。
- current_action 指导缩短为接受的任务、真实补充、来源范围及纯点击没有理解证据；取消用长提示修补接口冲突的做法。

实施验证：两个新增用例在修复前明确失败，见 [红测日志](tutor-continuation-tc2/interface-red.log)。修复后 Runtime Tutor **24 项通过**（[日志](tutor-continuation-tc2/interface-runtime.log)），Server Tutor **63 项通过、8 项默认忽略**（[日志](tutor-continuation-tc2/interface-server.log)）。新增服务端用例检查五次真实采样请求的工具合同，覆盖默认关闭的来源读取／首答、开启后读取／选择／交付，并验证最终保存 neutral/current_action 与 explain。会话默认、偏好与真实补充的优先级及历史类型合同均在 Runtime 回归中通过；运行中暂停、普通观察假设与原来源合同在 Server 回归中通过。本轮未改 Web／Memory，沿用 §3 已完成的对应验证。

首次接口修正后的真实模型复验完成两步交付，未再尝试关闭时 select、未开放 continuation 或重复填写意图字段；实际采样依次为 4 次 observing 和 4 次 active。接续仍因 `move.capability=source_explanation` 与 `target.capability=quantitative_reasoning` 不一致被拒绝一次，重试成功，见 [原始日志](tutor-continuation-tc2/interface-live-model.log)和[摘要](tutor-continuation-tc2/interface-live-model.json)。这也是重复填写同一信息的接口问题：类型化阅读动作的模型输入采用完整 TeachingTarget，能力只填在目标中，服务端投影到持久化 Move。普通旧对象式输入和历史存储合同保持可读。该补充修正的回归与复验继续记录在本节。

此轮输出仍不通过内容验收：接续把等时间说成算术平均相等的必要条件，又额外要求等距离；还把负时间的物理前提问题说成负数除法不成立。正数范围内，速度均为 10、用时分别 1 和 2 时，两种平均都为 10，足以否定“必须等时”；速度为 10 和 20、用时都为 1 时，两种平均都为 15，路程却不相等。该轮 tokens_spent 为 43,530 和 83,272，合计 126,802；未返回货币费用。

能力字段补充用例在旧合同上明确失败（[红测](tutor-continuation-tc2/interface-capability-red.log)）。补齐目标能力后，Runtime **24 项**与 Server **63 项／8 默认忽略**再次全部通过，见 [Runtime 最终日志](tutor-continuation-tc2/interface-runtime-final.log)、[Server 最终日志](tutor-continuation-tc2/interface-server-final.log)。实际采样与持久交付用例同时验证模型不填写 `move.capability`，最终交付按目标正确保存该字段。

最终真实模型复验：**1 项通过，内部两次请求均完整交付，工具校验错误 0**。实际采样为 3 次 observing、3 次 active；首次请求仅读取来源与定位来源，接续读取原文后一次 select 成功，未携带 continuation、意图元数据或重复的 move.capability。tokens_spent 为 **30,841 + 60,909 = 91,750**，没有货币费用返回。见 [最终原始日志](tutor-continuation-tc2/interface-live-model-final.log)和[最终摘要](tutor-continuation-tc2/interface-live-model-final.json)。这是一轮复验记录，不是成功率或费用基准。

最终公开回答的算例正确，并使用“用时不等时一般不等”及等距下的等速例外；然而 select 保存的 prompt 和 target.expected_performance 仍写着“只在各段用时相等时才成立”。持久教学目标与公开回答的准确度不一致，因此整体内容质量继续保留未验收。后续应单独验证模型内容及教学目标质量，不能把本次接口修复或一个更好的公开答案当成质量问题已经解决。

## 6. 已知限制与后续接点

- TC2A 已完成按需指导及现状／按需各 3 次固定输入观察。按需首个请求估算输入 17,250→14,020，但仍有一次 select 全部被拒、无持久目标与教学交付（incomplete=true），另外两次重试后交付；目标及正文的必要条件错误仍存在。TC2 的模型稳定交付与内容验收继续保留，具体原始请求、参数错误及数学核对见 [TC2A 实施记录](tutor-continuation-tc2a.md)。

- 无进展修复后的同一固定输入追加三次接续，**3／3 保存目标并交付**，无 `AGENT_NO_PROGRESS`；第 1 次两次教学校验失败后成功，第 3 次一次失败后成功。第 1 次首次报错同批有原文引用进展，没有单独触发新增恢复额度；该额度沿用修复前后隔离回归为证。目标或正文的等时必要条件概括仍有问题，内容与读者验收保持未完成。见[修复后复验](tutor-continuation-tc2a/recovery-20261011/README.md)。

- 目标读者形成性验证尚未进行；没有真实读者人数、无需指路比例或学习收益结论。保留 TC2 该验收项未勾选。后续从默认关闭的真实页面观察首次帮助、自主开启与第二次帮助，记录实际任务／人数／断点。
- 真实模型前期三轮见 §4，接口修正后的两轮见 §5。最终工程链路无工具错误；持久教学目标仍有错误的必要条件表达，内容质量未通过，也没有读者易用性或教学有效性结论。
- 建议生产／消费归 TC3／TC4；clarify_answer、回答选区及演示入口归 TC5。完整两宿主故障矩阵与移动端仍按 TC4／TC7 验收。本片没有开放尚未交付的动作。
- TC0 记录的 MU10 两项既有失败（admission_input 缺失、100 个 pending turn 恢复计数）未在本片修复；TC4／TC7 继续跟进。旧二进制读取新枚举／新 Move 不兼容，沿用 TC1 边界，正式降级演练归 TC8。
