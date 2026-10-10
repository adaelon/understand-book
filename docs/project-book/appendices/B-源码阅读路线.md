# 附录 B 源码阅读路线

面对大型文件，最有效的起点通常是一条输入和它应该形成的结果。本附录给出按责任组织的选读路线；每行依次列出起点、接续文件和需要带走的判断。它是阅读导航，文件排列不代表每两项之间都有直接函数调用。

## 从原文到可消费材料

| 需要回答的问题 | 选读顺序与关键符号 | 读完应能解释 |
| --- | --- | --- |
| 一个段落如何取得稳定位置 | [md-adapter.ts](../../../packages/core/src/md-adapter.ts)的markdownToBlocks → [segment.ts](../../../packages/core/src/segment.ts)的segment → [base-schema](../../../crates/base-schema/src/lib.rs)的Span、LidNode | 规范正文、层级路径与UTF-16区间怎样对齐；见[第4章](../chapters/04-原文定位与LID.md) |
| 过大的原文怎样进入模型 | [window.ts](../../../packages/core/src/window.ts) → [model-input-slice.ts](../../../packages/core/src/model-input-slice.ts)的routeModelInputSlices、validateModelInputSliceCoverage → [model-input-renderer.ts](../../../packages/core/src/model-input-renderer.ts) | 原LID、core覆盖、overlap与实际输入预算的关系；见[第7章](../chapters/07-模型工作单元与执行调度.md) |
| 全书结构如何从候选形成 | [book-structure.ts](../../../packages/core/src/book-structure.ts)的公开类型 → [book-structure-generation.ts](../../../packages/core/src/book-structure-generation.ts) → [book-structure-planning.ts](../../../packages/core/src/book-structure-planning.ts) | 模型工作与spine、key_stops、throughlines成品的区别；见[第6章](../chapters/06-全书结构与语义候选召回.md) |
| 谁决定构建下一步 | [build-orchestrator.ts](../../../packages/core/src/build-orchestrator.ts)的nextAutomaticBuildAction → [automatic-build-executor-session.ts](../../../packages/core/src/automatic-build-executor-session.ts) → [automatic-build-task-store.ts](../../../packages/core/src/automatic-build-task-store.ts)的nextAutomaticBuildExecutionIdentity | 准备、派发、交接、候选失败和当前待办如何连接；见[第7章](../chapters/07-模型工作单元与执行调度.md)与[第23章](../chapters/23-真实失败与工程改进.md) |
| 成功记录怎样变成可发布成果 | [automatic-build-publication.ts](../../../packages/core/src/automatic-build-publication.ts)的publishAutomaticBuildArtifactSet、hasCommittedAutomaticBuildPublication → [read-tools](../../../crates/read-tools/src/lib.rs)的Book::load → [published_library.rs](../../../crates/server/src/published_library.rs) | 正式文件、发布回执与读时加载分别检验什么；见[第8章](../chapters/08-构建恢复与成果发布.md) |

走第一条路线时，可以先用第4章的教学原文标出位置，再检查当前LidNode字段；走恢复路线时，则先写出失败发生前最后一个已经确认的事实。这样读源码，能避免把一份完成日志当成正式成果，也能避免把失联当成模型生成失败。

## 从读者问题到完成回答

| 需要回答的问题 | 选读顺序与关键符号 | 读完应能解释 |
| --- | --- | --- |
| 提问携带了哪一刻的材料与现场 | [App.vue](../../../packages/web/src/App.vue)的submitAgentMessage → [Server](../../../crates/server/src/lib.rs)的prepare_agent_chat → [run_scope.rs](../../../crates/server/src/run_scope.rs)的capture | 选区、聊天、现场与Provider如何成为原问题的固定输入；见[第3章](../chapters/03-数据身份与状态归属.md) |
| 一项工具能力如何进入模型请求 | [book-tool-contracts](../../../crates/book-tool-contracts/src/lib.rs) → [tool_registry.rs](../../../crates/runtime/src/tool_registry.rs) → [orchestrator.rs](../../../crates/runtime/src/orchestrator.rs) | 共同合同、暴露、发现、参数接纳和分派的责任；见[第10章](../chapters/10-工具与模型调用边界.md) |
| 线索怎样成为读者能打开的来源 | [read-tools](../../../crates/read-tools/src/lib.rs)的text、match_source_quote → [orchestrator.rs](../../../crates/runtime/src/orchestrator.rs)的TurnEvidenceLedger、compile_agent_answer、deliver_agent_answer → [Server](../../../crates/server/src/lib.rs)的route_agent_source_open | 实际读取、来源绑定、答案编译和后续Reader动作的差别；见[第9章](../chapters/09-一次有证据的回答.md)与[第12章](../chapters/12-来源与流式交付.md) |
| 长任务怎样继续而不把所有历史反复发送 | [context_fragment.rs](../../../crates/runtime/src/context_fragment.rs) → [tool_result.rs](../../../crates/runtime/src/tool_result.rs) → [auto_compaction.rs](../../../crates/runtime/src/auto_compaction.rs) → [orchestrator.rs](../../../crates/runtime/src/orchestrator.rs)的ProgressPhaseGuard | 当前活动投影、压缩安装与真实进展分别改变什么；见[第11章](../chapters/11-上下文组织与持续执行.md) |
| 制作演示怎样接续到确切版本 | [presentation_author.rs](../../../crates/runtime/src/presentation_author.rs)的AuthorRequest → [presentation_sandbox.rs](../../../crates/server/src/presentation_sandbox.rs) → [orchestrator.rs](../../../crates/runtime/src/orchestrator.rs) | 候选、预览、后续观察、交付与继续修订的连接；见[第17章](../chapters/17-可探索解释与演示版本.md) |

同一条来源链有两个容易混在一起的终点：模型完成来源编译，读者随后打开来源。阅读时要继续追到后者的真实状态操作，而不能停在回答里出现一个来源标记。

## 从状态操作到持久恢复

| 需要回答的问题 | 选读顺序与关键符号 | 读完应能解释 |
| --- | --- | --- |
| 模型等待期间怎样操作读者状态 | [agent_run.rs](../../../crates/server/src/agent_run.rs)的PreparedAgentChat、FinishedRun → [workspace_registry.rs](../../../crates/server/src/workspace_registry.rs)的NetworkRunPort → [run_scope.rs](../../../crates/server/src/run_scope.rs)的check_user、check_scene | 固定输入和实时权威的分工，以及私人数据与现场动作的不同条件；见[第18章](../chapters/18-Rust宿主与并发边界.md) |
| 聊天怎样追加并恢复 | [session_event.rs](../../../crates/server/src/session_event.rs) → [session_log.rs](../../../crates/server/src/session_log.rs)的append_with → [session_runtime.rs](../../../crates/server/src/session_runtime.rs) | 事件、投影、不确定写入与确认顺序；见[第16章](../chapters/16-会话日志与恢复.md) |
| 已接纳但中断的请求怎样处理 | [run_admission.rs](../../../crates/server/src/run_admission.rs)的complete_preparation、recover、save_finished、retry_save → [service_limits.rs](../../../crates/server/src/service_limits.rs) | 排队、领取、原结果保存重试与执行不明之间的区别；见[第19章](../chapters/19-多用户阅读与调度.md) |
| 任务进展和学习记录怎样跨轮保留 | [goal.rs](../../../crates/runtime/src/goal.rs) → [Server](../../../crates/server/src/lib.rs)的goal_turn_delivered → [memory](../../../crates/memory/src/lib.rs) | 工作判断、交付条件、私人记忆与学习事实分别保存什么；见[第13章](../chapters/13-持续目标与教学过程.md)与[第15章](../chapters/15-笔记记忆与学习事实.md) |
| 多人服务怎样维持权威与份额 | [multi_user_host.rs](../../../crates/server/src/multi_user_host.rs) → [workspace_registry.rs](../../../crates/server/src/workspace_registry.rs) → [service_limits.rs](../../../crates/server/src/service_limits.rs) → [control_store.rs](../../../crates/server/src/control_store.rs) | 用户、现场、模型许可和服务根单写者的作用范围；见[第19章](../chapters/19-多用户阅读与调度.md) |

## 从观察记录到效果判断

先沿[运行事件](../../../crates/runtime/src/run_events.rs)与[服务观测](../../../crates/server/src/observability/mod.rs)确认活动来自哪里，再读[provider-recorder.mjs](../../../evals/semantic/provider-recorder.mjs)的请求记录和用量口径。效果判断接到[agent-core.mjs](../../../evals/semantic/agent-core.mjs)、[task-quality-v2.mjs](../../../evals/semantic/task-quality-v2.mjs)及[diagnostic-core.mjs](../../../evals/semantic/diagnostic-core.mjs)。对应讲解是[第21章](../chapters/21-可观测性成本与效果评测.md)与[第23章](../chapters/23-真实失败与工程改进.md)。

这条路线至少要保留三个对象：真实运行、用于评分的证据、最终判定。修改评分依据不能改写原回答，诊断复跑也不能替换首次失败。历史报告入口集中在[附录E](E-运行验证与实验入口.md)。

## 阅读范围

本轮回读了附录所需的类型、入口和关键分支；没有沿每一条路线重新执行产品。源码后续变化时，先判断改动影响哪一项责任，再回到相关章节与SOURCES修订。旧章节记载的问题和实验结论保留其原验证范围。

返回[全书入口](../README.md)，或继续查[附录C：数据与接口速查](C-数据与接口速查.md)。
