# 附录 D 决策与演进索引

ADR适合回答“当时面对什么约束，为什么这样选择”。判断今天已经具备什么能力，还需要追到当前实现与后续验收。本附录按影响的机制选取重要决策，保留历史日期，并指出哪些后续材料改变了早期表述。

## 材料与构建边界

| 决策 | 当时确定的责任 | 阅读时接续到哪里 |
| --- | --- | --- |
| [ADR-0021](../../adr/0021-实现技术栈-预构建ts-读时后端rust-前端待定-基座schema-rust权威ts-rs生成.md)，2026-06-22 | TypeScript构建、Rust读时，以及Rust定义基座类型并生成TS类型 | “前端待定”是历史状态；当前Vue与Tauri见[第20章](../chapters/20-桌面插件与Linux发布.md)，材料边界见[第2章](../chapters/02-构建与阅读如何分工.md) |
| [ADR-0100](../../adr/0100-budget-routable-model-work-units-and-truthful-build-recovery.md)，2026-08-02 | LID保留原文身份，超限正文通过模型输入片路由 | [model-input-slice.ts](../../../packages/core/src/model-input-slice.ts)与[第7章](../chapters/07-模型工作单元与执行调度.md)；上游窗口覆盖仍有独立责任 |
| [ADR-0101](../../adr/0101-deterministic-prebuild-protocol-ownership-and-codex-semantic-boundary.md)，2026-08-08 | 封闭输入与版本合同能够唯一决定的工作交给Engine | [build-orchestrator.ts](../../../packages/core/src/build-orchestrator.ts)和[执行器会话](../../../packages/core/src/automatic-build-executor-session.ts)；模型仍承担语义候选生成 |
| [ADR-0125](../../adr/0125-candidate-validation-feedback-and-bounded-retry.md)，2026-09-12 | 可定位的候选字段格式错误允许原策略纠正 | [task-store](../../../packages/core/src/automatic-build-task-store.ts)的readAutomaticBuildCandidateRetryFeedback；证据不足与策略合同变化保持不同恢复要求，见[第23章](../chapters/23-真实失败与工程改进.md) |
| [ADR-0146](../../adr/0146-portable-build-workspaces-and-incremental-book-updates.md)，2026-09-29设计、10-01更新状态 | 分开工作区位置、内容版本与复用依据 | 记录明确U1—U2搬迁续建完成、U3—U11待实施；[第8章](../chapters/08-构建恢复与成果发布.md)区分已实现续建与后续增量迁移 |
| [ADR-0149](../../adr/0149-构建期语义候选召回与Core-owned-Embedding-Provider.md)，2026-09-30设计、10-01阶段记录 | 在限定构建消费者中引入语义候选召回，Embedding由Core管理 | 记录保留SR6两轮真实Gold未达发布门槛、默认词法路径；排名、对象身份与来源质量在[第6章](../chapters/06-全书结构与语义候选召回.md)、[第22章](../chapters/22-关键决策及其演进.md)分别解释 |

这些决策共同形成一条分工：程序准备和检验可确定的材料、输入与身份，模型在合同内提出需要语义判断的内容。准备成功、候选接纳和用户任务成功分别需要自己的依据。

## 运行、来源与持续执行

| 决策 | 当时确定的责任 | 后续解释 |
| --- | --- | --- |
| [ADR-0086](../../adr/0086-runtime-owned-user-visible-source-references.md)，2026-07-20 | Runtime拥有普通用户可见来源的呈现权威 | 当前quote接口和实际绑定路径见[orchestrator.rs](../../../crates/runtime/src/orchestrator.rs)及[第12章](../chapters/12-来源与流式交付.md)，不能照抄早期工具参数 |
| [ADR-0091](../../adr/0091-model-aware-agent-request-tool-exposure-and-active-context-budget.md)，2026-07-23 | 模型运行配置、请求计划、按需工具和活动上下文分别负责 | [ActiveContextBudget](../../../crates/runtime/src/auto_compaction.rs)限制单次请求容量；其中早期轮数安排还需接ADR-0150 |
| [ADR-0127](../../adr/0127-resident-agent-streaming-and-runtime-activity.md)，2026-09-13接受、09-14历史验收 | Run拥有执行生命周期，流式事件观察运行，等待离开共享状态锁 | 当前多人权威与保存恢复见[第18章](../chapters/18-Rust宿主与并发边界.md)、[第19章](../chapters/19-多用户阅读与调度.md)；历史单宿主阶段不限定后续所有入口 |
| [ADR-0150](../../adr/0150-resident-unbounded-tool-loop-and-progress-stops.md)，2026-09-30 | 普通Resident默认持续执行，按实际进展与失败停止 | 当前OuterConfig.max_turns=None与ProgressPhaseGuard见[orchestrator.rs](../../../crates/runtime/src/orchestrator.rs)；显式有限调用和历史实验预算保持各自条件 |
| [ADR-0152](../../adr/0152-resident-linear-jsonl-session-log.md)，2026-10-01 | 一聊天一份顺序JSONL，派生状态由事件折叠 | [SessionLog](../../../crates/server/src/session_log.rs)与[第16章](../chapters/16-会话日志与恢复.md)；旧聊天原样保留、未自动导入，其他领域存储仍独立提交 |
| [ADR-0155](../../adr/0155-goal-work-plan-and-version-centered-presentation-context.md)，2026-10-02 | Goal保存任务进展，演示围绕候选与确切版本组织指导和源码上下文 | 后续状态已包含EX13.6原生接续完成；[第17章](../chapters/17-可探索解释与演示版本.md)与[第23章](../chapters/23-真实失败与工程改进.md)保留工程预览、表示正确和完整交付的区别 |

一项新决策往往只替换早期方案的一部分。例如，追加日志改变聊天事实的持久化方式，并没有合并所有私人存储；默认取消十二轮上限，也没有把累计费用和总时长变成有保证的额度。

## 评测怎样反过来改变决策

[ADR-0137](../../adr/0137-stratified-reading-evals-and-diagnostic-improvement.md)于2026-09-25接受，按必要理解操作区分L1定位提取、L2解释与局部推导、L3多证据整合和L4情境诊断与迁移；单轮、多轮和跨会话作为另一条维度。

该ADR顶部仍写EV7待实施。后续[EV7原始记录](../../performance/agent-eval-ev7-20260926.md)已经给出六对十二份首次回答、事前保留门槛及候选撤回。当前[agent_prompt.rs](../../../crates/runtime/src/agent_prompt.rs)中的完成指导为v7。因此，查阅时应保留下面这条时间关系：

| 材料 | 能回答什么 |
| --- | --- |
| ADR-0137 | 为什么分层、怎样建立诊断与对照责任 |
| EV7冻结方案与配对结果 | 那一次提示候选在什么条件下被保留或撤回 |
| 当前完成指导与调用装配 | 今天实际使用哪个版本 |
| [第21章](../chapters/21-可观测性成本与效果评测.md)、[第23章](../chapters/23-真实失败与工程改进.md) | 怎样连起量尺、真实失败、当前问题与工程取舍 |

实验撤回不撤销此前已经成立的来源修复，也不证明所有范围提示都无效。它只结束了这一项冻结假设下的候选。

## 什么变化值得回到决策

[第22章](../chapters/22-关键决策及其演进.md)已经按真实约束讨论了重新判断的条件。实际维护时，可以从三类变化开始：原任务变了，原规模或共享方式变了，或者原验收依据被新证据推翻。例如，多实例共同写入会改变单写者权威；正文改版会触及位置与私人记录迁移；完整交付持续失败则要求定位到实际读取、来源支持、动作或终态中的具体环节。

先说明哪一个原假设不再成立，再提出需要改变的责任。这样才能判断需要的是局部修正、新的实验，还是重新划定架构边界。

## 依据范围

本附录于2026-10-08回读上述ADR状态、后续记录与关键实现入口；不修改历史ADR，也不重新宣布其所有阶段验收通过。历史文档中的待定项、已实现部分和未达到的门槛按各自日期保留。完整依据见[资料索引](../SOURCES.md)，实际运行与结果记录见[附录E](E-运行验证与实验入口.md)。

返回[全书入口](../README.md)。
