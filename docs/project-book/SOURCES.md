# 资料与源码索引

本索引用于持续写作。章节入口表给出下一步应当读取的材料；分章证据表记录已经对照的具体实现。导读、第 1—5 章与第 9 章依据 2026-10-06 工作区，第 6—8、10 章依据 2026-10-07 工作区；这些章节读取时 HEAD 均为 5e10516，包含此后的未提交内容。第 4—5 章回读了窗口、工作单元、输入渲染与新来源加载实现，第 6 章回读了 BookStructure、检索与 Provider，第 7 章回读了输入切片、预算、派发、执行器、候选接纳及 Harness 适配，第 8 章回读了成果与执行身份、策略迁移、发布收口、搬迁续建及网络发布，第 10 章回读了共同 Book 合同、Registry、工具暴露与分派、Reader 笔记、MCP/REST 和模型适配。

第 11 章于 2026-10-07 重新读取请求组装、片段、活动工具结果、压缩和观测代码；此时 HEAD 为 4e0b68e（2026-10-07），相关产品文件读取时无未提交修改。各章保留各自核对日期和范围；本轮第 10 章只更新相邻导航，没有宣称前章已按新提交全部复核。

第 12 章同日以 4e0b68e 为基线，实际回读 Provider 流、公开草稿、来源绑定与编译、SSE 缓冲、Web 合并、终态日志和网络重试保存；相关产品路径读取时无未提交修改。第 11 章只更新导航。相同提交没有被用作省略源码回读的理由。

第 13 章同日以 4e0b68e 为基线，实际回读 Goal、Tutor 准备与交付、教学地图发布及就绪、私人生命周期、评估与证据投影、Web 活动入口；相关产品路径读取时无未提交修改。当前指定模块的 17 个既有用例在独立驱动内实际执行，另补 2 组边界观察；Server/Web 集成路径按源码核对。第 12 章仅修改导航。

第 14 章同日以 4e0b68e 为基线，回读 Reader 窗口、App 正文缓冲与提问、文字和页面锚点、Markdown/PDF 选区映射、Server 原文重建、来源往返、批注投影及布局计算；相关产品路径读取时无未提交修改。六份既有前端测试和五个新增局部观察实际执行，Rust 与浏览器集成路径按源码核对。第 13 章只更新相邻导航。

第 15 章同日以 4e0b68e 为基线，回读笔记与阅读活动、画像事实、前台操作和后台复核、投影与治理、跨书提升、学习证据及私人归属入口。28 个当前 memory 既有用例在独立临时驱动内实际执行，另执行 4 个教学算例与边界观察；Runtime、Server 与界面连接按源码核对。第 14 章只更新导航。

第 16 章同日以 4e0b68e 为基线，回读会话事件与投影、按聊天追加、冻结位置、运行保存、成果处置、准入恢复和阅读回顾；当前 Server 的 26 个定向既有用例直接执行通过。模型采用固定响应，数据写入书稿临时目录；没有复制或修改产品测试。第 15 章只更新相邻导航。

第 18 章同日以 4e0b68e 为基线，重新读取 HTTP Host、运行协调、固定输入与端口、取消连接、SSE、演示执行、已读持久化和后台复核；网络部分只核对本章必需的执行、状态端口和资源等待连接。Server 21、Runtime 1、Memory 2、Reader 1 个定向用例直接执行通过。相关产品路径读取时无未提交修改，实际依据仍为本轮回读内容；第 17 章仅更新下一章链接。

第 19 章同日以 4e0b68e 为基线，重新读取多人 Host、登录与授权、用户和现场注册表、发布书库、控制存储、接单与恢复、资源许可及相关定向测试。实际运行 29 个不同的 Server 用例，26 通过、3 个旧 MU4 用例在夹具准备阶段失败；失败与补充验证分开记录，不据测试标题声称已经验收。仅修改书稿，第 18 章只更新相邻导航。

第 20 章同日以 4e0b68e 为基线，回读桌面、插件与执行器发行合同、Linux 启动组合、材料发布和维护恢复。实际通过 12 个 Server 用例、5 个启动用例；两个构建用例首轮一过一失败，失败项在纠正 Git Bash 替身优先顺序后单独通过。另完成源码合同、3 组发行配置与 6 组临时注册。只修改书稿，第 19 章仅更新相邻导航；历史平台记录不当作当前在线验证。

第 21 章同日以 4e0b68e 为基线，实际回读执行活动、Provider 用量、观测传输与 spool、构建投影、评测记录器、任务评分和诊断路径。51 项当前局部验证通过，同时确认 SSE 记录器的已知计量缺口；历史实验按程序、题集与评分版本分列。仅修改书稿，第 20 章只更新相邻导航。

第 22 章同日以 4e0b68e 为基线，回读材料交付、类型与坐标、构建检索、默认轮数与进展、活动上下文、增量聊天及多人归属的当前路径，并按日期对照决策和历史验收。当前 Runtime 8 项、Core 6 项定向用例直接通过，Core 另 20 项按名称未选中；历史 LA9、SR6 与 BSR7 没有重跑。第 21 章只更新相邻导航，产品实现和测试未改动。

第23章同日以4e0b68e为基线，重新读取本章实际依赖，串联LA、EV4—EV7、CQ8、9月30日构建恢复、BSR7及EX12—EX13的失败、修订和当前路径。实际通过6个Core、1个Runtime用例；两项本地探针确认当前评测记录器的用途分类与流式预算缺口。历史报告、前章验证和本轮重放分别记录，仅修改书稿，第22章只更新相邻导航。

第24章同日以4e0b68e为基线，回读构建推进、Book加载、运行归属与状态端口、来源匹配、持续执行、日志追加、模型许可和服务单写者，把已核实事实组织成项目介绍、白板请求链和条件变化分析。本轮只做书稿引用与计算核对，没有重跑产品用例；前章验证和历史模型结果保留原日期与条件。第23章仅更新相邻导航，OUTLINE仅调整第24章中心问题与24.5主题，正文进度为24/24。

2026-10-08完成六份附录及全书导航、引用和重点术语校订。HEAD仍为4e0b68e；本轮重新读取附录所依赖的类型、字段、入口、默认配置、脚本与ADR状态，未以相同提交替代回读。旧正文保留原依据日期，新增选读路线不等于对所有旧结论作统一基线语义重审。本轮未运行产品用例、模型、Embedding或浏览器。

## 叙述方法参考

- [深入理解 AI Agent](../../.understand-book/ai-agent-engineering/source.txt)：本轮抽读入门、上下文工程、用户记忆和知识库、评估章节。借鉴从场景建立直觉、解释机制、展示实例，再通过条件变化题检验理解的组织方法。
- [深入理解 AI Infra](../../AI-Infra-Book-offline/AI-Infra-Book-complete.md)：本轮抽读前言、多轮任务、算子与运行时、批处理章节。借鉴明确假设、列出约束、估算工作量，再用实测校正判断的展开方式。

## 各章实现入口

下表是阅读入口。已成稿章节的具体依据见后文；待写章节需要在动笔时进一步回读实现与验证材料。

| 章 | 实现入口 | 设计或验证材料 |
| --- | --- | --- |
| 1 阅读任务 | [项目入口](../../README.md)、[Web App](../../packages/web/src/App.vue) | [当前架构](../架构.md) |
| 2 构建与阅读 | [构建编排](../../packages/core/src/build-orchestrator.ts)、[读取工具](../../crates/read-tools/src/lib.rs) | [技术栈决策](../adr/0021-实现技术栈-预构建ts-读时后端rust-前端待定-基座schema-rust权威ts-rs生成.md) |
| 3 数据与归属 | [数据定义](../../crates/base-schema/src/lib.rs)、[运行范围](../../crates/server/src/run_scope.rs) | [术语](../../CONTEXT.md)、[多人架构](../adr/0147-linux-multi-reader-service-without-redis.md) |
| 4 原文与 LID | [切分](../../packages/core/src/segment.ts)、[分区](../../packages/core/src/partition.ts)、[PDF 对应](../../packages/core/src/pdf-source-map.ts) | [LID 切分决策](../adr/0008-LID切分-纯确定性-句级独立层-忠实块映射-分区不变式.md) |
| 5 语义抽取 | [Pass1 输入](../../packages/core/src/pass1-input.ts)、[归并](../../packages/core/src/merge.ts)、[Pass2](../../packages/core/src/pass2-orchestrate.ts) | [局部和长程抽取](../adr/0010-语义边两遍抽取-双agent-硬屏障-全量目录优先-确定性投影-锚定基数分裂.md) |
| 6 全书结构 | [候选发现](../../packages/core/src/book-structure-discovery.ts)、[结构整理](../../packages/core/src/book-structure-organization.ts)、[语义召回](../../packages/core/src/semantic-retrieval.ts) | [结构与召回决策](../adr/0151-book-structure-global-outline-and-semantic-retrieval.md)、[结构评估](../../evals/book-structure/README.md) |
| 7 调度与执行 | [输入预算](../../packages/core/src/model-input-budget.ts)、[派发](../../packages/core/src/automatic-build-dispatch.ts)、[执行器会话](../../packages/core/src/automatic-build-executor-session.ts)、[DSH 接入](../../packages/dsh-plugin/src/index.ts) | [工作单元与预算](../adr/0100-budget-routable-model-work-units-and-truthful-build-recovery.md)、[多 Harness](../adr/0138-multi-harness-prebuild-and-deepseek-harness-adapter.md) |
| 8 恢复与发布 | [恢复](../../packages/core/src/automatic-build-recovery.ts)、[语义成果](../../packages/core/src/semantic-artifact.ts)、[发布](../../packages/core/src/automatic-build-publication.ts)、[收口](../../packages/core/src/automatic-build-close.ts)、[阅读发布库](../../crates/server/src/published_library.rs) | [预算与恢复](../adr/0100-budget-routable-model-work-units-and-truthful-build-recovery.md)、[可搬迁工作区](../adr/0146-portable-build-workspaces-and-incremental-book-updates.md)、[U1—U2 验证](../performance/workspace-u1-u2-20261001.md) |
| 9 一次回答 | [服务入口](../../crates/server/src/lib.rs)、[运行上下文](../../crates/runtime/src/run_context.rs)、[Agent 循环](../../crates/runtime/src/orchestrator.rs) | 见下方样章证据表 |
| 10 工具与模型 | [工具契约](../../crates/book-tool-contracts/src/lib.rs)、[注册表](../../crates/runtime/src/tool_registry.rs)、[工具发现](../../crates/runtime/src/tool_exposure.rs)、[模型适配](../../crates/runtime/src/lib.rs)、[MCP](../../crates/server/src/mcp.rs) | [开放能力路由](../adr/0113-open-natural-language-capability-routing-and-runtime-evidence-topology.md)；见下方第 10 章证据与验证记录 |
| 11 上下文 | [上下文片段](../../crates/runtime/src/context_fragment.rs)、[工具结果](../../crates/runtime/src/tool_result.rs)、[压缩](../../crates/runtime/src/compaction.rs)、[自动压缩](../../crates/runtime/src/auto_compaction.rs) | [上下文治理](../adr/0091-model-aware-agent-request-tool-exposure-and-active-context-budget.md)、[当前停止条件](../adr/0150-resident-unbounded-tool-loop-and-progress-stops.md)；见下方第 11 章证据与验证记录 |
| 12 流式交付 | [回答流](../../crates/runtime/src/answer_stream.rs)、[模型流](../../crates/runtime/src/provider_stream.rs)、[运行事件](../../crates/runtime/src/run_events.rs)、[服务端流](../../crates/server/src/agent_stream.rs)、[Web 状态](../../packages/web/src/agent-run-state.ts)、[网络保存重试](../../crates/server/src/run_admission.rs) | [流式运行](../adr/0127-resident-agent-streaming-and-runtime-activity.md)、[来源引用](../adr/0086-runtime-owned-user-visible-source-references.md) |
| 13 目标与教学 | [Goal](../../crates/runtime/src/goal.rs)、[Tutor](../../crates/runtime/src/tutor.rs)、[教学入口](../../crates/server/src/teaching.rs)、[私人生命周期](../../crates/memory/src/learning.rs)、[评估](../../crates/memory/src/assessment.rs) | [任务目标](../adr/0136-resident-goal-lifecycle-and-delivery-completion.md)、[全局 Tutor](../adr/0141-global-tutor-control-and-session-ownership.md)、[工作计划](../adr/0155-goal-work-plan-and-version-centered-presentation-context.md)；见下方第 13 章证据与验证记录 |
| 14 阅读现场 | [Reader](../../crates/reader/src/lib.rs)、[正文](../../packages/web/src/components/ReaderPane.vue)、[PDF 选区](../../packages/web/src/pdf-selection-draft.ts) | [阅读排版与批注](../adr/0148-reader-typography-annotations-and-motion.md)、[正文路径回退](../adr/0112-pre-phr-reader-body-path-rollback.md) |
| 15 记忆与学习 | [Memory](../../crates/memory/src/lib.rs)、[画像](../../crates/memory/src/profile.rs)、[学习证据](../../crates/memory/src/learning_evidence.rs) | [画像归属](../adr/0075-runtime-owned-evidence-backed-profile-memory.md)、[学习事实](../adr/0143-teaching-trace-assessment-and-learning-evidence.md) |
| 16 会话恢复 | [日志](../../crates/server/src/session_log.rs)、[会话集合](../../crates/server/src/session_store.rs)、[事件投影](../../crates/server/src/session_event/projection.rs)、[运行提交](../../crates/server/src/session_runtime.rs)、[回顾](../../crates/server/src/session_recap.rs) | [JSONL](../adr/0152-resident-linear-jsonl-session-log.md)、[阅读回顾](../adr/0153-session-reading-recap.md)；见下方第 16 章证据与验证记录 |
| 17 演示解释 | [运行时制作](../../crates/runtime/src/presentation_author.rs)、[版本保存](../../crates/server/src/presentation_store.rs)、[前端呈现](../../packages/web/src/components/AgentPresentation.vue) | [富呈现](../adr/0130-agent-rich-presentation-and-read-time-authoring.md)、[分阶段制作](../adr/0154-presentation-global-framework-and-staged-authoring.md)；见下方第 17 章证据与验证记录 |
| 18 宿主并发 | [Host](../../crates/server/src/host.rs)、[运行协调](../../crates/server/src/agent_run.rs)、[固定归属](../../crates/server/src/run_scope.rs)、[状态端口](../../crates/runtime/src/run_context.rs) | [运行隔离与流](../adr/0127-resident-agent-streaming-and-runtime-activity.md)、[已读异步持久化](../adr/0106-asynchronous-coalesced-read-ledger-persistence.md)；见下方第 18 章证据与验证记录 |
| 19 多人服务 | [多人宿主](../../crates/server/src/multi_user_host.rs)、[用户注册](../../crates/server/src/user_registry.rs)、[现场注册](../../crates/server/src/workspace_registry.rs)、[准入](../../crates/server/src/run_admission.rs)、[资源限制](../../crates/server/src/service_limits.rs) | [多人服务决策](../adr/0147-linux-multi-reader-service-without-redis.md)；见下方第 19 章证据、已知边界与验证记录 |
| 20 安装与部署 | [桌面入口](../../apps/desktop/src-tauri/src/main.rs)、[插件管理](../../apps/desktop/src-tauri/src/plugin_manager.rs)、[Linux 发布](../../scripts/linux/build-multi-reader.sh)、[维护恢复](../../crates/server/src/reader_maintenance.rs) | [Windows 构建](../Windows-Setup编译方法.md)、[Linux 发布运行单](../Linux多人阅读-MU11发布运行单.md)；见下方第 20 章依据、边界与验证 |
| 21 观测与评测 | [服务观测](../../crates/server/src/observability/mod.rs)、[构建观测](../../packages/observability/src/prebuild-projector.ts)、[Agent 评测](../../evals/semantic/agent-run.mjs)、[任务评分](../../evals/semantic/task-quality-v2.mjs) | [观测决策](../adr/0134-optional-langsmith-observability-and-evaluation.md)、[评测改进](../LA7-LA10实施与验收.md)、[分层评测](../adr/0137-stratified-reading-evals-and-diagnostic-improvement.md)；见下方第 21 章依据、边界与验证 |
| 22 设计演进 | [构建交付](../../packages/core/src/automatic-build-publication.ts)、[检索准备](../../packages/core/src/automatic-build-retrieval.ts)、[运行循环](../../crates/runtime/src/orchestrator.rs)、[聊天增量](../../crates/server/src/session_runtime.rs)、[多人权威](../../crates/server/src/workspace_registry.rs) | [技术栈](../adr/0021-实现技术栈-预构建ts-读时后端rust-前端待定-基座schema-rust权威ts-rs生成.md)、[语义召回](../adr/0149-构建期语义候选召回与Core-owned-Embedding-Provider.md)、[持续执行](../adr/0150-resident-unbounded-tool-loop-and-progress-stops.md)、[日志](../adr/0152-resident-linear-jsonl-session-log.md) |
| 23 失败改进 | [来源与交付](../../crates/runtime/src/orchestrator.rs)、[构建交接](../../packages/core/src/automatic-build-executor-session.ts)、[演示历史](../../crates/runtime/src/presentation_author.rs)、[评测记录](../../evals/semantic/provider-recorder.mjs) | [LA7至LA10](../LA7-LA10实施与验收.md)、[构建恢复](../performance/build-control-recovery-20260930.md)、[EX13.6](../performance/presentation-context-ex13/ex13-6/README.md)、[EV7](../performance/agent-eval-ev7-20260926.md)；见第23章依据、边界与验证 |
| 24 面试表达 | [运行归属](../../crates/server/src/run_scope.rs)、[状态端口](../../crates/server/src/workspace_registry.rs)、[运行与来源](../../crates/runtime/src/orchestrator.rs)、[资源许可](../../crates/server/src/service_limits.rs) | 前述章节与原始实验记录；见第24章具体依据、已知边界及书稿验证。24.5为结果依据与已知边界 |

## 第 1 章的具体依据

| 正文结论 | 已对照实现或材料 | 依据范围 |
| --- | --- | --- |
| 问题携带位置、选区和后续任务信息 | [App.vue](../../packages/web/src/App.vue) 的 submitAgentMessage、sendAgent | 锚点按选区、选中位置、视口顶部取值；正文摘录 agentRunCreate 参数 |
| 本地和网络入口使用共同输入校验 | [Server](../../crates/server/src/lib.rs) 的 AskQuote、validate_agent_input、prepare_agent_chat；[网络准入](../../crates/server/src/run_admission.rs) | 校验后的输入进入任务接纳，规范引文与范围对应 |
| 选区范围可以重建规范原文 | Server 的 agent_chat_selection_ranges_rebuild_canonical_quote、agent_chat_rejects_forged_canonical_selection_quote | 已阅读测试正文：重建结果与拒绝不一致的 resolved_quote；未重新执行 |
| 一次任务连接材料、现场和私人记录 | [Book](../../crates/read-tools/src/lib.rs)、[ReaderState](../../crates/reader/src/lib.rs)、[UserRuntime](../../crates/server/src/user_runtime.rs)、[RunContext](../../crates/runtime/src/run_context.rs) | 已对照各对象字段与职责，具体状态流转承接第 3、9 章 |
| 准备失败与重启后恢复失败应分别归因 | [Agent 评测协议](../../evals/semantic/AGENT_EVAL.md) | 使用文档中的任务与阶段定义说明验收口径；本批未开展新评测 |

## 第 2 章的具体依据

| 正文结论 | 已对照实现或材料 | 依据范围 |
| --- | --- | --- |
| 构建计划结合现有状态决定阶段与后续动作 | [build-orchestrator.ts](../../packages/core/src/build-orchestrator.ts) 的 AutomaticBuildStage、routeAutomaticBuildSnapshot、nextAutomaticBuildAction | 阶段声明、计划校验与路由、后续动作选择；未将所有阶段写成每种材料都必跑 |
| 原文与基座共同支撑读取，部分旁路成果可缺省 | [Book::load](../../crates/read-tools/src/lib.rs) | 已阅读 base.json、source.txt 和旁路加载分支；可加载不代表所有扩展能力已就绪 |
| TypeScript 与 Rust 通过材料成果和共同类型连接 | [ADR-0021](../adr/0021-实现技术栈-预构建ts-读时后端rust-前端待定-基座schema-rust权威ts-rs生成.md)、[base-schema](../../crates/base-schema/src/lib.rs)、[segment.ts](../../packages/core/src/segment.ts) | ADR 支持选择理由；当前定义和导入支持实现状态；前端现状对照[项目入口](../../README.md) |
| 同一内容的具体发布具有独立引用和就绪信息 | [published_library.rs](../../crates/server/src/published_library.rs) 的 PublishedBookRef、PublicationManifest、PublishedBook | 已对照结构字段；具体发布流程留待第 8、19 章 |
| 更多证据召回没有在该轮实验中转化为更高完整成功 | [2026-09-08 同环消融](../../evals/semantic/results/2026-09-08-la9-v2/report.md) | deepseek-v4-flash，24 个任务，text/tree/graph；正文数字引用原表，不推为当前总体效果 |
| 搬迁与内容修订具有不同身份规则，完整增量流程尚未完成 | [ADR-0146](../adr/0146-portable-build-workspaces-and-incremental-book-updates.md) | 对照文档中的已实现 U1—U2 与待实施 U3—U11 |

成本式 `qD`、`B + qR` 及交点 `B / (D - R)` 是本章教学推导。B=180、D=12、R=3 为自设算例；前提是材料稳定、统一成本单位与相同任务质量目标。它不引用项目费用或延迟测量。材料更新成本与首次等待时间另行说明。

## 第 3 章的具体依据

| 正文结论 | 已对照实现或材料 | 依据范围 |
| --- | --- | --- |
| 内容基座、具体发布和书内位置表达不同身份 | [PublishedBookRef](../../crates/server/src/published_library.rs)、[基座定义](../../crates/base-schema/src/lib.rs)、[术语](../../CONTEXT.md)、[ADR-0146](../adr/0146-portable-build-workspaces-and-incremental-book-updates.md) | PublishedBookRef、ReadOnlyBase、LidNode 与内容版本约定；跨版本自动衔接保留实现边界 |
| 范围单位跨语言统一为 UTF-16 code unit | [Span](../../crates/base-schema/src/lib.rs)、[TS 定义](../../packages/core/src/generated/LidNode.ts)、[切分](../../packages/core/src/segment.ts)、[Book::new 与 Book::text](../../crates/read-tools/src/lib.rs) | Rust 导出、TS 导入、UTF-16 加载与切片；A中B 是教学例子 |
| 服务、用户和现场拥有不同的可变状态 | [AppState](../../crates/server/src/lib.rs)、[ServiceState](../../crates/server/src/service_state.rs)、[UserRuntime](../../crates/server/src/user_runtime.rs)、[ReaderWorkspace](../../crates/server/src/reader_workspace.rs) | 已对照字段和构造、切换路径；没有把全服务组件都写进 ServiceState 字段 |
| 同一用户的多现场共享私人归属，现场分别管理 Reader | [UserRegistry](../../crates/server/src/user_registry.rs)、[WorkspaceRegistry](../../crates/server/src/workspace_registry.rs)、[ADR-0147](../adr/0147-linux-multi-reader-service-without-redis.md) | 已读注册表结构与所有权约定；详细准入与并发行为留待第 19 章 |
| 运行固定原材料、输入与归属，现场变更另行检查 | [RunScope](../../crates/server/src/run_scope.rs) 的 capture、check_user、check_scene、private_user；ReaderWorkspace::invalidate | 已对照完整 RunScope，以及绑定发布、选择聊天的代际变化；另一个窗口翻页不会自动使原现场失效 |
| 现场失效后，原私人结果与现场动作有不同处理 | [Agent 运行测试](../../crates/server/src/agent_run_tests.rs) 的 mu1c_old_run_cannot_touch_replaced_workspace_but_saves_original_note_and_answer | 已读用例正文；内部替换现场验证归属，不据此声称公开 Host 允许活跃 Run 任意切书；未重新执行 |
| 现场或运行持有引用时，Book 可以跨缓存淘汰继续存活 | [PublishedLibrary](../../crates/server/src/published_library.rs) 的 ResidentEntry、residents | Arc/Weak 字段与缓存生命周期约定；不涉及新的内存性能测量 |

## 第 4 章的具体依据

| 正文结论 | 已对照实现或材料 | 依据范围 |
| --- | --- | --- |
| Markdown 语法树位置映射到源块，标题展示文字与原文切片可不同 | [md-adapter.ts](../../packages/core/src/md-adapter.ts) 的 parseMarkdownSourceBlocks、sourceBlock、projectRootNode | 已读实现；标题、列表、资源和行内公式按当前分支说明 |
| EPUB 按 spine 提取并形成规范源串 | [epub-adapter.ts](../../packages/core/src/epub-adapter.ts) 的 epubToSource、xhtmlToBlocks | 范围在规范正文拼接时形成，不声称等同压缩包或 XHTML 原字节 |
| 已有 EPUB 正文快照从基座保留结构 | [book-source.ts](../../packages/core/src/book-source.ts) 的 loadBookSource | 已读 canonical_source.kind=epub 分支及书籍身份核对 |
| 标题容器与标题首叶共同构成位置树 | [segment.ts](../../packages/core/src/segment.ts) 的 segment、make；[切分测试](../../packages/core/test/segment.test.ts) | 已读实现与测试；本批另用教学材料实际重放，9 节点、6 叶子 |
| 分区检查处理重叠、非空白遗漏和层级关系 | [partition.ts](../../packages/core/src/partition.ts) 的 checkPartitionInvariant | coverage 为非空白字符串长度之比；没有把早期文档“字节”写成当前单位 |
| 窗口消费位置树，预算改变执行分组 | [window.ts](../../packages/core/src/window.ts) 的 splitWindows、emitSubtree；[pass1-reduction.ts](../../packages/core/src/pass1-reduction.ts) 的 fragmentRenderInput | 教学材料重放为 2 窗口；源片段保留 parent_lid，具体路由留第 7 章 |
| PDF-first 使用接纳后的规范正文与页面映射 | [ADR-0063](../adr/0063-paper-pdf-first-reconciled-source-build-workbench.md)、[hybrid-foundation-v2.ts](../../packages/core/src/hybrid-foundation-v2.ts)、[pdf-source-map.ts](../../packages/core/src/pdf-source-map.ts) | 已读候选生成、完整性与写入路径；本批未重新导入 PDF |
| 根级超预算叶子会漏出窗口 | window.ts 的 emitSubtree；[调用入口](../../skills/build/load-book.ts) 的 loadBookWindows | 已用当前默认预算重放有标题与无标题对照，见后文已知问题 |

## 第 5 章的具体依据

| 正文结论 | 已对照实现或材料 | 依据范围 |
| --- | --- | --- |
| Pass1 使用带 LID 的真实原文输入 | [pass1-input.ts](../../packages/core/src/pass1-input.ts)、[抽取器定义](../../agents/pass1-local-extractor.md) | 已读组装函数与抽取约定；教学窗口输入实际重放 |
| 概念多锚点与断言单锚点使用不同归并规则 | [merge.ts](../../packages/core/src/merge.ts) 的 mergeAndGate；[merge.test.ts](../../packages/core/test/merge.test.ts) | 已读完整归并和测试；教学 4 节点归并为 3 节点，缓存保留 3 锚点 |
| 当前任务接纳先限制获分配的证据范围 | [pass1-reduction.ts](../../packages/core/src/pass1-reduction.ts) 的 assertOutputEvidence、artifactPayloadForCandidate | 已读 whole/group、fragment、stitch 分支，未将底层清理行为扩大为所有入口都接受非法候选 |
| 全局目录从存活节点直接投影 | [catalog.ts](../../packages/core/src/catalog.ts) 的 projectCatalog；[pass1-batch.ts](../../skills/build/pass1-batch.ts) | 已读投影与调用位置；教学目录实际重放 |
| 当前 Pass2 使用重复节点与公式联系生成候选 | [pass2-build.ts](../../packages/core/src/pass2-build.ts) 的 buildLongRangeCandidates、isCrossWindow | 已读候选生成与关系合同；本例生成 1 候选，未调用分类模型 |
| 源窗口工作包与渲染路径直接决定输入内容 | [pass2-orchestrate.ts](../../packages/core/src/pass2-orchestrate.ts) 的 buildPass2WorkPacket；[model-input-renderer.ts](../../packages/core/src/model-input-renderer.ts) 的 renderPass2ModelInput；[pass2-input.ts](../../skills/build/pass2-input.ts) | 已读完整组装与直接 JSON 渲染；包内缺目标原文的范围见后文 |
| 通过结构接纳的 accepted 边进入基座，其他类别保留审计 | pass2-build.ts 的 gatePass2BuildOutput、acceptedEdgeDropReason；[Pass2 测试](../../packages/core/test/pass2-build.test.ts) | 已读结构条件与用例；不声称程序已经证明关系理由成立 |
| 篇章、公式和论文规则补充通用图谱 | [discourse-index.ts](../../packages/core/src/discourse-index.ts)、[formula-semantics.ts](../../packages/core/src/formula-semantics.ts)、[pass1-profile-input.ts](../../packages/core/src/pass1-profile-input.ts)、[content-profile.ts](../../packages/core/src/content-profile.ts) | 已读字段、公式接纳与规则输入；来源真相仍按当前 PDF-first 路径分别说明 |
| Pass2 从早期固定阶段演进为可选增强 | [ADR-0010](../adr/0010-语义边两遍抽取-双agent-硬屏障-全量目录优先-确定性投影-锚定基数分裂.md)、[ADR-0098](../adr/0098-optional-pass2-enrichment-for-book-structure.md)、[构建路由](../../packages/core/src/build-orchestrator.ts)、[可选输入测试](../../packages/core/test/book-structure-optional-pass2.test.ts) | 已对照历史与现行路由；未沿用固定并发和全量目录输入作当前事实 |

两章中的缓存材料和抽取节点均为本书教学设定。确定性重放调用 markdownToBlocks、segment、checkPartitionInvariant、splitWindows、buildPass1Inputs、mergeAndGate、projectCatalog、buildPass2Candidates 和 buildPass2WorkPacket；没有调用模型、没有写入产品构建工作区。

## 第 4—5 章发现的已知问题

**根级超预算叶子遗漏。** 当前默认 maxInputTokens=12000，输入为无标题的 `缓存` 重复 7000 次。位置树 1 叶子、分区通过，splitWindows 返回 0 窗口；在同一长段前加 `# 标题` 后，位置树 2 叶子、返回 2 窗口，其中一个 overBudget=true。原因是 emitSubtree 的超限分支只遍历 children，根级叶子没有子节点，未被发出。该行为可经现有 Markdown 加载与窗口接口到达。本批已直接执行对照，未修改实现。若修复，只需使这个分支也处理超限根叶，并以这组输入检验覆盖。

**Pass2 工作包缺目标原文。** 当前 buildPass2WorkPacket 的正文只来自源窗口，候选的 target_lids 提供位置指针，renderPass2ModelInput 不增加目标正文。教学例子的包携带窗口 0 的四段原文，候选目标为 2.2，但未携带 2.2 原文。因此该包本身不能证明双侧材料已经交付。已对照组装、渲染、CLI 与分类器要求，未运行真实模型验证是否另行补读。后续处理应明确补齐目标证据的交付或可追踪回读，再判断关系；本批未修改实现。

## 第 6 章的具体依据

| 正文结论 | 已对照实现或材料 | 依据范围 |
| --- | --- | --- |
| 全书结构公开 spine、key_stops、throughlines，阅读依赖位于 spine 单元 | [book-structure.ts](../../packages/core/src/book-structure.ts) 的 BookStructureSidecar、BookStructureSpineUnit、BookStructureThroughline | 已读字段；未把主题工作的 stages 与完整依赖理由写成公开 throughline 字段 |
| 技术学习路径先给框架，再发现正文并组织候选 | [build-orchestrator.ts](../../packages/core/src/build-orchestrator.ts) 的 routeBookStructureProductionStage；[book-structure-discovery.ts](../../packages/core/src/book-structure-discovery.ts) 的 structureSourceOutline、structureDiscoverySources | 已读 technical_learning 分支、完整源范围替换及组织调用；其他 profile 的既有归并分支仍存在 |
| 规范章节身份与顺序保持，框架只引用已交付概览或前言 | [book-structure-planning.ts](../../packages/core/src/book-structure-planning.ts) 的 acceptStructureOutline | 当前正式 source outline 从章节少量正文生成；初始框架不等于正文覆盖 |
| 候选身份由贡献与局部 ID 组成，完整含义、条件与来源保留 | [book-structure-candidates.ts](../../packages/core/src/book-structure-candidates.ts) 的 structureCandidateRef、collectStructureCandidates；[book-structure-generation.ts](../../packages/core/src/book-structure-generation.ts) 的 readStructureCandidateCatalog | 来源由当前任务的已接受贡献提供；同一 LID 可有不同候选，不自动进行语义合并 |
| 章节完整重点与宏观路线分开，宏观必须为已选引用的子集 | book-structure-candidates.ts 的 StructureChapterSelection、acceptStructureChapterSelection；[book-structure-materialization.ts](../../packages/core/src/book-structure-materialization.ts) 的 materializeStructureChapterSelections | 含义、条件、学习理由组装到公开 reason；教学例子实际运行得到 3 重点、2 宏观引用 |
| 浏览完整索引与交付判断证据分别记录 | book-structure-planning.ts 的 structureChapterInput、applyStructureChapterAction；[规划测试](../../packages/core/test/book-structure-planning.test.ts) | 已读预览不足、完整浏览、来源修订等用例；章节选择不要求重新 inspect 每一个已有候选，新摘要按当前证据检查 |
| 主题种子范围不限制全量候选检索 | [book-structure-themes.ts](../../packages/core/src/book-structure-themes.ts) 的 structureThemePlanningInput、structureThemeInput；[主题测试](../../packages/core/test/book-structure-themes.test.ts) | 宏观引用用于计划种子；主题执行仍使用完整目录 |
| 短向量投影与完整词法字段、候选正文分开 | [book-structure-retrieval.ts](../../packages/core/src/book-structure-retrieval.ts) 的 projectStructureRetrieval、structureRetrievalPage；[检索测试](../../packages/core/test/book-structure-retrieval.test.ts) | 候选 48/24/12 Unicode 码点；完整字段与位置保留；预览不授予证据 |
| 精确与别名优先，再交替融合词法和语义，空查询完整浏览 | [semantic-retrieval.ts](../../packages/core/src/semantic-retrieval.ts) 的 retrieveHybrid、retrievalPage | 每页 6 条；语义至多新增 12 条，不把 12 当作完整命中列表上限 |
| Core 异步准备 Provider 结果，保存结果必须匹配当前依赖 | [embedding-provider.ts](../../packages/core/src/embedding-provider.ts)、[semantic-retrieval-preparation.ts](../../packages/core/src/semantic-retrieval-preparation.ts)、[book-structure-organization.ts](../../packages/core/src/book-structure-organization.ts) | 已读身份、输入上限、取消与用量接口、准备匹配和待准备分支；没有执行真实 Provider |
| 主题成员需要已查看，依赖需要两侧来源，主题合并保留成员与依赖 | book-structure-themes.ts 的 applyStructureThemeAction、checkResult、reconcileStructureThemes、reopenStructureTheme | 已读实现及主题证据、依赖测试；教学重放确认一条主题不会自动产生 depends_on |
| 程序组装可以补入尚未选为章节重点的主题候选 | book-structure-themes.ts 的 materializeStructureThemes | 补 key_stops、增加 throughlines 和已接受依赖，保留原宏观路线 |
| 候选保留与 Core 管理检索的设计理由 | [ADR-0151](../adr/0151-book-structure-global-outline-and-semantic-retrieval.md)、[ADR-0149](../adr/0149-构建期语义候选召回与Core-owned-Embedding-Provider.md) | 区分旧多层摘要归并、当前技术学习路径和不同召回消费者；不把 BSR7 外推为更改全部默认模式 |
| 旧重点遗漏、局部选择改进与全书两组结果 | [BSR0](../performance/book-structure-bsr0.md)、[BSR2 Codex](../performance/book-structure-bsr2-codex.md)、[BSR7](../performance/book-structure-bsr7.md) | 保留日期、材料、复用前置成果、独立查询、序列化估算与未知模型用量；历史重放零调用与真实生成分开 |

第 6 章沿用缓存材料，但学习候选、摘要与主题由本书手工设定。实际调用 collectStructureCandidates、newStructureChapterWork、structureChapterInput、applyStructureChapterAction、materializeStructureChapterSelections、planStructureThemes、applyStructureThemeAction、reconcileStructureThemes 和 materializeStructureThemes。结果为 3 候选、3 公开重点、2 宏观引用、1 主题、0 前置依赖；失效条件继续保留。仅预览时的新摘要被拒绝，完整候选交付后接受；请求 inspect 本身尚未增加证据。未调用模型或 Embedding，也未写入产品构建工作区。

本章引用的 BSR2 是旧输入之上的局部组织实验；BSR7 是此前真实全书实验的既有记录，本次没有重跑。章节明确保留了历史候选中的数值语义误标及正式选择如何排除它，没有把保留候选等同于确认其正确。

## 第 7 章的具体依据

| 正文结论 | 已对照实现或材料 | 依据范围 |
| --- | --- | --- |
| 原 LID 保留，模型片区分 core 和 context 范围 | [model-input-slice.ts](../../packages/core/src/model-input-slice.ts) 的 ModelInputSliceV1、routeModelInputSlices、validateModelInputSliceCoverage | 已读路由、边界与完整覆盖；教学三句实际运行，不能替代第 4 章上游窗口遗漏的修复 |
| 预算按渲染结果计算，输入中标明父位置与责任范围 | [model-input-renderer.ts](../../packages/core/src/model-input-renderer.ts) 的 renderPass1SourceFragmentModelInput；[window.ts](../../packages/core/src/window.ts) 的 estimateTokens | 教学渲染器只拼三段文字；正式渲染另有字段与协议开销；估算不冒充模型 tokenizer |
| 工作描述把输入依据、策略、依赖和预算接到同一任务 | [stage-work-unit.ts](../../packages/core/src/stage-work-unit.ts) 的 WorkUnitDescriptorV4、ModelInputBasisV1 | 已读字段及派发校验调用；区别源片、语义投影和子成果归并 |
| 输入正文上限同时受阶段与上下文剩余量约束 | [model-input-budget.ts](../../packages/core/src/model-input-budget.ts) 的 evaluateModelExecutionBudget | 已读阶段、上下文、输入传输、候选传输四类阻塞；公式直接对应当前实现；合成例实际区分 stage_limit 与 candidate_transport |
| 候选 JSON 值需扣除提交封套，token 与字节同时限制 | [executor-transport.ts](../../packages/core/src/executor-transport.ts) 的 createCandidateTransportContract、measureExecutorCandidateRequest | 实际计算得到 2048−67=1981 估算 tokens、32768−267=32501 bytes；使用现有 Codex profile |
| 输出引用本身过长时需要有语义的分段提交 | [BSR7](../performance/book-structure-bsr7.md)、[book-structure-planning.ts](../../packages/core/src/book-structure-planning.ts)、[章节分段测试](../../packages/core/test/book-structure-chapter-selection.test.ts) | 历史 114/44、2643/1981、3500 预留、2228 下界、48/48/18+final；当前 continueStructureChapterSelection 与 select_stops 保留完整引用 |
| dispatch 按同类与策略分组，批次限制与单任务预算分开 | [automatic-build-dispatch.ts](../../packages/core/src/automatic-build-dispatch.ts) 的 AUTOMATIC_BUILD_DISPATCH_LIMITS、planAutomaticBuildExecutorDispatches、selectAutomaticBuildDispatchRefill | 已读绑定校验、顺序分组、单元数/输入/预测时长上限和补位；相关测试为源码阅读 |
| Engine 的四类控制动作投影当前计划，Harness 执行生命周期 | [automatic-build-driver.ts](../../skills/build/automatic-build-driver.ts) 的 AutomaticBuildStepActionV1、automaticBuildStep | 已读 dispatch、waiting、close_stage 与 done 分支；DONE 不从 child final 推导 |
| 当前交接绑定具体恢复身份，提交后可结束一个单元的交接 | [automatic-build-executor-session.ts](../../packages/core/src/automatic-build-executor-session.ts) 的 issueAutomaticBuildOpaqueHandoff、submitAutomaticBuildExecutorCandidateV3 | 当前 Codex V4、DSH V5 公共 handoff；已读 3 单元批次第一项提交即返回 committed 的现有测试；旧 V3 连续批次路径分别说明 |
| 收齐输入并确认最终序号后，才领取并启动语义尝试 | 同文件的 openAutomaticBuildExecutorSessionV3、nextAutomaticBuildExecutorInput、startAutomaticBuildExecutorGeneration；[执行器测试](../../packages/core/test/automatic-build-executor-session.test.ts) | 已读 open、generation.start 主路径与完整输入期无 attempt 用例；传输 chunk 与语义工作单元分开 |
| pending、reserved、running 由租约和启动记录区分 | [automatic-build-lease.ts](../../packages/core/src/automatic-build-lease.ts) 的 inspectAutomaticBuildTaskActivity、startAutomaticBuildLease、heartbeatAutomaticBuildLease | 已读字段和实际状态分支；恢复代次的完整处理留第 8 章 |
| 候选经当前阶段写入器形成成果与回执 | [automatic-build.ts](../../skills/build/automatic-build.ts) 的 submitAutomaticBuildTaskCandidate；[automatic-build-mailbox.ts](../../packages/core/src/automatic-build-mailbox.ts) 的 submitAutomaticBuildCandidate | 已读输入观察核对、阶段 writer、成果存在及身份、committed 回执路径；阶段发布不由本次 task receipt 单独证明 |
| 可纠正字段错误只反馈同一范围内的相关失败 | [automatic-build-task-store.ts](../../packages/core/src/automatic-build-task-store.ts) 的 readAutomaticBuildCandidateRetryFeedback；执行器测试的字段修正用例 | 返回 code/json_pointer/expected；application 误用为重点类型是现有用例构造，未冒充新的生产失败 |
| 两个 Harness 使用固定执行配置及不同交付批次 | [build-execution-profile.ts](../../packages/core/src/build-execution-profile.ts)、executor-transport.ts；[DSH 入口](../../packages/dsh-plugin/src/index.ts)、[控制器](../../packages/dsh-plugin/src/build-control.ts) | 当前 profile、session、transport 版本及批次数量；DSH 启动协商、实时容量、补位和清理 |
| 设计演进与实际 DSH 验证范围不同 | [ADR-0100](../adr/0100-budget-routable-model-work-units-and-truthful-build-recovery.md)、[ADR-0138](../adr/0138-multi-harness-prebuild-and-deepseek-harness-adapter.md)、[DSH 兼容报告](../performance/dsh-prebuild-compatibility-20260925.md) | 6992/5000 是历史路由触发点；DSH DH1—DH5 实现与确定性证据、DH0/DH6 未完成项分别保留，不以元数据作真实容量结论 |

第 7 章教学源串为“缓存复用已有结果。数据变化后应使缓存失效。批处理合并请求。”，长 29 UTF-16、估算 27 tokens。实际用 routeModelInputSlices 设置 stage_body_limit_tokens=14、context_overlap_utf16=1，示意渲染器只拼 context_before/core/context_after。得到核心范围 [0,9)、[9,21)、[21,29)，可见范围 [0,10)、[8,22)、[20,29)，对应估算 10/13/8；三片均指向 1.2，覆盖 29、缺口与核心重叠均为 0。

同次调用 createCandidateTransportContract 核对候选容量。用合成提示与合成工具封套调用 packExecutorTransportPayload、evaluateModelExecutionBudget，40-token 正文在正常配置通过，阶段上限 30 触发 stage_limit，候选声明 2643 触发 candidate_transport。没有写入产品构建工作区，没有模型、Embedding 或真实 Harness 调用。既有切片、预算、派发、执行器会话、章节分段提交相关用例为源码阅读，未执行产品测试套件。

## 第 8 章的具体依据

| 正文结论 | 已对照实现或材料 | 依据范围 |
| --- | --- | --- |
| 技术书续建消费已导入快照，EPUB 保留正文与结构关系 | [build-orchestrator.ts](../../packages/core/src/build-orchestrator.ts) 的 technicalLearningTargetFromWorkspace、resolveAutomaticBuildTarget；[book-source.ts](../../packages/core/src/book-source.ts) 的 loadBookSource | 已读内部正文、包快照、profile 与来源核对，及旧 EPUB 复用 base.lid_nodes；导入与搬迁实验引用 U1—U2 原报告 |
| 完成成果按内容、完整输入、单元和语义合同判断 | [semantic-artifact.ts](../../packages/core/src/semantic-artifact.ts) 的 SemanticArtifactEnvelopeV3、sameBuildContent、semanticArtifactMatches | 已读 V2/V3 判定；实际用 V3 成果核对 6 种期待条件；位置变化只改匹配参数，没有冒充完整搬迁试验 |
| 策略代次变化不能通过改标签跳过采用条件 | [automatic-build-policy-generation.ts](../../packages/core/src/automatic-build-policy-generation.ts) 的 recordAutomaticBuildPolicyMigration、recordAutomaticBuildPriorGenerationAdoption | 已读旧 V2 迁移、输入与合同判定、当前预算校验、活动租约阻塞、adopt_exact 与 rebuild，以及较早 V3 采用入口；设计理由对照 ADR-0100 |
| 有效租约、过期接管与候选失败分别处理 | [automatic-build-lease.ts](../../packages/core/src/automatic-build-lease.ts) 的 inspectAutomaticBuildTaskClaim、readAutomaticBuildLease；[task-store](../../packages/core/src/automatic-build-task-store.ts) 的 nextAutomaticBuildExecutionIdentity | 已读语义尝试与租约代次分支、当前执行目标检查；现有租约过期用例为源码阅读 |
| 已提交候选可以重放，同一身份不能换候选 | [automatic-build-mailbox.ts](../../packages/core/src/automatic-build-mailbox.ts) 的 submitAutomaticBuildCandidate；[执行器会话测试](../../packages/core/test/automatic-build-executor-session.test.ts) | 已读现有回执分支、候选与范围一致性、成功记录补齐；会话测试中 submit_revision=2、semantic_attempt=1 为用例断言，本次未重跑 |
| 耗尽后的恢复动作取决于故障类别 | [automatic-build-attempt-recovery.ts](../../packages/core/src/automatic-build-attempt-recovery.ts) 的 automaticBuildRetryBoundaryRequiredRecovery、createAutomaticBuildRetryRecoveryReceipt | 暂时 Provider、可纠正候选、其他 schema/evidence 与内部故障分别处理；输入交付与接收通道失败不能创建语义重试边界 |
| 路由及收口阻塞提供稳定原因和允许动作 | [automatic-build-recovery.ts](../../packages/core/src/automatic-build-recovery.ts) 的 AutomaticBuildRecoveryEnvelopeV1；[收口实现](../../packages/core/src/automatic-build-close.ts) 的 closeRecovery | 已读信封字段、代码与动作枚举；实际触发 close/publication_receipt_invalid 和 post_close/stage_close_postcondition_failed |
| 发布清单确定事务，公开文件改变后可重发相同字节 | [automatic-build-publication.ts](../../packages/core/src/automatic-build-publication.ts) 的 publishAutomaticBuildArtifactSet、buildAutomaticBuildStageBatchResult | 实际运行两文件故障注入、恢复、重复发布和文件改动后重发；已读候选、备份、替换、回执及 catch 回滚；强制终止后的自动修复未实测 |
| 收口检查发布前后质量、覆盖、策略、新鲜度与实际文件 | [automatic-build-close.ts](../../packages/core/src/automatic-build-close.ts) 的 closeAutomaticBuildStage、writeAutomaticBuildStageCloseResult；[质量实现](../../packages/core/src/automatic-build-quality.ts) 的 collectAutomaticBuildStageQuality | 已读完整收口路径、质量报告构成及 BookStructure 发布观察不进入质量摘要；现有 close 测试同时覆盖缺文件和质量漂移 |
| closed、新鲜度和当前计划 DONE 按不同层次重算 | [build-orchestrator.ts](../../packages/core/src/build-orchestrator.ts) 的 inspectAutomaticBuildStageFreshness、nextAutomaticBuildAction、nextPlannedAutomaticBuildAction；[Driver](../../skills/build/automatic-build-driver.ts) 的 automaticBuildStep | 已读阶段关闭、计划复用与阶段闭包、收口后 replan、私人产物后才 DONE；本次 Pass1 收口后实际下一步为 extract/profile_sidecar |
| 搬迁复用成果，但旧执行归属不迁移 | semantic-artifact.ts 的 relocateGenerationTask；task-store 的 attemptStates；[U1—U2 报告](../performance/workspace-u1-u2-20261001.md) | 已读位置投影、复制记录不占新位置重试额度、关闭回执按 freshness 分开；历史 29 章夹具重发 82,514 字节引用原报告，没有重新运行 |
| 阅读发布经过复制验证、封存和数据库登记 | [published_library.rs](../../crates/server/src/published_library.rs) 的 PublishedLibrary::publish、PublicationManifest；[mu3_tests.rs](../../crates/server/src/tests/mu3_tests.rs) | 已读发布主路径；登记失败不暴露、旧运行持有旧发布、同书正文或附件变化被拒绝等用例为源码阅读，未执行 Rust 测试 |
| 阅读就绪与教学就绪分别表达 | [服务入口](../../crates/server/src/lib.rs) 的 build_workbench_snapshot；published_library.rs 的 readiness 检查与清单 | 已读技术书专用路由、论文可信来源/基础成果路由及教学状态单独记录，不把可阅读写成全部教学能力完成 |
| 完整内容改版复用仍是后续设计 | [ADR-0146](../adr/0146-portable-build-workspaces-and-incremental-book-updates.md) | U1—U2 已实施，U3—U11 待实施；旧 EPUB 包身份限制、目录结构和活动执行归属边界见原验证报告 |

第 8 章实际重放使用现有 [model-input-routability-fixture.ts](../../packages/core/test/helpers/model-input-routability-fixture.ts) 的 512-token 合成段落与固定候选，调用 closeSyntheticPass1 准备已接受成果。对 semanticArtifactMatches 的 6 组期待条件，原条件与只换位置为 true，换书籍、输入、schema 或代次为 false。该步骤没有运行模型，也没有实际搬迁整个夹具。

随后对合法发布结果、只有普通日志、发布后删除 base.json 三种情况实际调用 closeAutomaticBuildStage，分别得到 closed/replan、close/publication_receipt_invalid、post_close/stage_close_postcondition_failed。成功收口后重新计算下一步为 extract/profile_sidecar。

独立临时目录的发布例使用两份教学 JSON 标记。第 1 个文件替换后注入异常，两份旧文件均恢复；重试得到含 2 个文件的 committed 回执；原样重放和改动一个公开文件后重发均维持相同事务身份，改动文件恢复预期字节。临时目录在确认位于系统临时目录且属于本次夹具后清理。本批没有修改产品文件，没有真实模型、Embedding 或 Harness 调用。现有租约、执行器提交、质量发布、close 和网络发布用例为源码阅读，未执行完整产品测试。

## 第 9 章样章的具体依据

| 样章结论 | 已对照实现或材料 | 对应验证入口 |
| --- | --- | --- |
| 提问绑定原书、聊天、阅读现场及其代际 | [RunScope](../../crates/server/src/run_scope.rs) 的 capture、check_owner、check_scene；[Server](../../crates/server/src/lib.rs) 的 prepare_agent_chat | [运行测试](../../crates/server/src/agent_run_tests.rs)，后续详写时按现场变化场景选取 |
| 一轮工作单独持有消息、证据、定位与进展 | [RunContext](../../crates/runtime/src/run_context.rs) 的字段和 ResidentStatePort | [Agent 循环](../../crates/runtime/src/orchestrator.rs) 中的请求与进展测试 |
| 已验证选区可直接进入证据账本 | orchestrator.rs 中 TurnEvidenceLedger::from_seed | source_presentation_verified_selection_seed_is_presentable |
| 定位线索不会自动成为可引用证据 | orchestrator.rs 中 observe_tool_evidence、TurnEvidenceLedger::prepare_present | source_presentation_rejects_context_route_state_error_and_unobserved_lid |
| 正文字面出现与充分上下文有不同含义 | observe_tool_evidence 的 book.search_text 分支与 LiteralOccurrence | source_presentation_accepts_search_as_a_literal_occurrence_claim |
| 引文必须落在本轮已观察原文内，重复位置需要消歧 | TurnEvidenceLedger::prepare_present | source_presentation_quote_requires_full_observation、source_presentation_repeated_quote_requires_disambiguation_even_in_one_read |
| 最终回答编译检查来源，保留一次既有修复路径 | compile_agent_answer、deliver_agent_answer | source_presentation_invalid_answer_repairs_once_without_persisting_invalid_text、source_presentation_second_invalid_answer_fails_closed_after_one_repair |
| 默认不设固定采样轮数上限 | OuterConfig::default、run_context 的进展停止分支；[ADR-0150](../adr/0150-resident-unbounded-tool-loop-and-progress-stops.md) | unbounded_default_continues_past_twelve_samplings_to_completion |
| 状态操作通过原归属的端口执行 | [RuntimeStatePort](../../crates/server/src/agent_run.rs) 的 submit_private、read_live_reader、apply_reader；RunScope::check_scene | [锁外等待决策](../adr/0127-resident-agent-streaming-and-runtime-activity.md)，对应 agent_run_tests.rs |
| 运行结果与目标完成分开判断 | Server 的 finalize_user_agent_turn、goal_turn_delivered | goal_completion_requires_a_mounted_delivery_owned_by_the_goal |
| 新聊天使用追加日志，写入确认后发布投影 | [SessionLog::append](../../crates/server/src/session_log.rs)、[会话提交](../../crates/server/src/session_runtime.rs)、[ADR-0152](../adr/0152-resident-linear-jsonl-session-log.md) | [日志测试](../../crates/server/src/session_log/tests.rs) 的 session_log_reopen_and_position_keep_frozen_prefix |

样章中列出的 Runtime 测试位于 [orchestrator.rs](../../crates/runtime/src/orchestrator.rs)；Server 测试位于 [agent_run_tests.rs](../../crates/server/src/agent_run_tests.rs)。本轮对照了关键实现及部分测试正文，未重新执行 Rust 或前端测试。首批验证针对书稿自身的链接、引用符号和表述。

2026-10-06 的 skill 试写补充回读了 `TurnEvidenceLedger::prepare_present`、`goal_turn_delivered` 与 `goal_completion_requires_a_mounted_delivery_owned_by_the_goal`。第 9 章新增两处 Rust 短摘录，分别说明观察范围如何限制引文匹配、Goal 通用检查实际覆盖到什么粒度；五道追问补充参考思路。上述测试为源码阅读，未重新执行。

## 第 10 章的具体依据

| 正文结论 | 已对照实现或材料 | 依据范围 |
| --- | --- | --- |
| 公共 Book 能力共用身份、别名、输入类型和合同元数据 | [book-tool-contracts](../../crates/book-tool-contracts/src/lib.rs) 的 BookToolId、SurfaceAliases、BookToolContract、TextInput、validate_input | 已读合同、Schema 生成及语义检查；Manifest 和 Guide 的入口差异分别说明 |
| Registry 连接模型说明、校验器、能力卡与执行 Handler | [tool_registry.rs](../../crates/runtime/src/tool_registry.rs) 的 ToolRegistration、ToolRegistry::try_new、routing_card_for | 已读完整构造校验及 Book 描述/Schema 同源检查，能力卡复用 use_when/do_not_use_when；没有把结果合同名称写成通用输出语义校验器 |
| 每次采样的可见工具受分类、激活与预算共同约束 | [tool_exposure.rs](../../crates/runtime/src/tool_exposure.rs) 的 ToolExposurePlan::build、classify、tool_schema_bytes、projected_schema_bytes | 已读 Direct/Deferred/Hidden、9/8 个直接名额、Tutor 额外名额及延迟工具加入；字节数不等于 token 或 Provider 请求总长 |
| 能力目录使模型能表达需要，运行状态由 Runtime 补全 | [agent_prompt.rs](../../crates/runtime/src/agent_prompt.rs) 的 TOOL_DISCOVERY；tool_exposure.rs 的 CapabilityRequestV2、TaskNeed、stamp_task_need | 已读有界目录、六个请求字段、512 字符与 1—6 条上限、权限及证据状态权威来源；相关状态伪造用例为源码阅读 |
| 发现先过滤，再排序与覆盖能力 | tool_exposure.rs 的 resolve_capabilities、search_and_activate；tool_registry.rs 的 routing_shape | 已读能力、scope、operation、效果与前提分支；词法字段权重、能力覆盖优先与未满足原因；relative_cost 没有被写成已实现的成本优化求解器 |
| 发现只改变后续请求，旧 Run 激活不继承 | [orchestrator.rs](../../crates/runtime/src/orchestrator.rs) 的 build_sample_request 与 sampled_tool_names 分派过滤；tool_exposure.rs 的 redact_history | 已读实际请求集合核对；tool_exposure_activation_applies_only_to_the_next_sampling、tool_exposure_search_activates_only_deferred_metadata_for_next_sampling 为用例阅读，未运行 |
| Reader 的笔记、高亮、导航分别授权 | tool_exposure.rs 的 classify_turn_intent、explicit_action、ToolExposureState::authorizes_reader_action；orchestrator.rs 的 READER_ACTION_NOT_REQUESTED 分支 | 已读 seed 与执行检查；实际抽取函数重放 9 组输入，发现原句、短语与“区别”否定误判；reader.scroll 无授权分支为静态结论 |
| 笔记保存与现场选择是两个结果 | orchestrator.rs 的 dispatch_state_tool；[Reader](../../crates/reader/src/lib.rs) 的 Reader::save_note；[Memory](../../crates/memory/src/lib.rs) 的 NoteSaveStatus | 已读 submit_private 后 apply_reader、CREATED/EXISTING、session 层及 reader_effect=not_applied；本轮未实际写笔记 |
| REST 绑定转为共同参数，MCP 按共同别名分派 | [Server](../../crates/server/src/lib.rs) 的 bind_rest_book_input、route_canonical_readonly_book_tool；[MCP](../../crates/server/src/mcp.rs) 的 tools_list_result、dispatch_mcp_tool、handle_tools_call | 已读 end→end_lid、结果封装及错误状态；book_tool_contract_has_schema_and_binding_parity 为源码阅读 |
| 外部 MCP 没有 Resident 私人动作入口，Guide 拥有独立临时状态 | mcp.rs 的 VisitorSession、route_book_guide、session_context | 已读 Visitor 分派及 guide 主路径；visitor_dispatch_has_no_reader_or_memory_branch、visitor_guide_never_reads_or_injects_reader_private_profile 为源码阅读 |
| 两种协议归一化成相同的 Runtime 调用对象 | [runtime/lib.rs](../../crates/runtime/src/lib.rs) 的 ModelAdapter、ToolCall、native_chat_request_projection、provider_tool_name、build_react_system、parse_react_assistant_turn、react_message_to_json | 已读 Native 双向名字映射、ReAct 文本 JSON、消息配对及错误解析；实际运行 3 组名称转换；现有 Native 回环和 ReAct 解析用例未重跑 |
| Provider 配置、模型运行约定与应用执行策略不同 | runtime/lib.rs 的 ProviderConfig、ProviderRegistry、ProviderContinuation；[model_runtime.rs](../../crates/runtime/src/model_runtime.rs) 的 ModelRuntimeCatalog::resolve、AgentRequestPlan::from_messages | 已读配置优先级、同模型私有续接、parallel_tool_calls=false 及顺序分派；不以目录配置冒充模型厂商实测规格 |
| 开放语义路由与有限动作授权各有职责 | [ADR-0113](../adr/0113-open-natural-language-capability-routing-and-runtime-evidence-topology.md)、[术语](../../CONTEXT.md) 的 TaskNeed、Resident Tool Routing Card、Turn Locator Ledger | 设计理由与当前代码分别说明；旧有限循环背景沿第 9 章及 ADR-0150 修订，不据旧 ADR 恢复固定默认轮数 |
| 类型合同未覆盖反向原文区间 | book-tool-contracts 的 validate_semantics；[Book::text](../../crates/read-tools/src/lib.rs)；REST/MCP Text 分派 | 已追踪公开范围参数到直接切片；抽取原方法在最小 Book 形状重放正向、反向、缺节点三种输入，反向触发 panic；没有启动 Server |
| schema_valid 尚未统一阻断无效参数的分派 | orchestrator.rs 的 run_context 中 schema_valid、executed 与后续 match handler；dispatch_state_tool 的 MemorySave 分支；tool_registry.rs 的 validate_schema | 静态读取确认无统一校验失败早退，非数组 citations 会在 MemorySave 中按缺省处理并进入保存；未重放这一写入路径 |

第 10 章的局部重放由本轮临时 Rust 驱动执行。分类器、引号/否定处理、动作判断、projected_schema_bytes 和 provider_tool_name 均从当前文件逐字抽取。驱动只补齐需要的类型形状和调用输入，没有改写函数逻辑。Book::text 也逐字抽取，但 Book、节点查找和数据装载使用最小教学形状，不能据此声称完整 Book 加载、MCP 或 Resident Run 已实测。

9 组意图输入及笔记/高亮/跳转布尔值见正文表。3 组预算输入设为 180/240/300 bytes，候选总量为 182/423/724；600-byte 限制是教学设置。3 组名称输入依次为 book.text、reader.note、book_text，输出 book_text、reader_note、book_text_2，最后一个是教学碰撞。3 组正文范围基于“甲乙丙丁戊己”和 [0,2)、[2,4)、[4,6)，分别得到完整正文、可捕获 panic 和节点缺失错误。18 组断言实际通过，没有模型或网络调用，临时目录已清理。

第 10 章新发现的问题集中在正文“已知边界与本轮验证”：动作语言识别及 scroll 映射、Schema 校验与分派脱节、Text 反向区间。前两类中只有意图函数进行了局部重放，其余注明静态依据；区间问题实际重放了读取方法，没有评价不同宿主捕获 panic 的行为。本轮只修改书稿。

## 第 11 章的具体依据

| 本章论断 | 当前源码、设计或记录 | 本轮核对与使用范围 |
| --- | --- | --- |
| 下一次采样使用运行投影 | [orchestrator.rs](../../crates/runtime/src/orchestrator.rs) 的 build_sample_request、messages_with_context_fragments；[run_context.rs](../../crates/runtime/src/run_context.rs) 的 RunContext | 已读检查点、历史回执、活动结果、冻结片段及动态快照的组合次序；仅描述阅读 Run |
| 请求计划组合指令、输入、工具及预算 | [model_runtime.rs](../../crates/runtime/src/model_runtime.rs) 的 AgentRequestPlan::from_messages、estimate_request_tokens、estimate_text_tokens | 已读首条 System 拆分、模块、Schema 计量、预留与容量；默认数字是应用配置，估计不是 Provider tokenizer |
| 固定片段按 key 保留最新版本，动态快照锚定原始消息位置 | [context_fragment.rs](../../crates/runtime/src/context_fragment.rs) 的 upsert、projected_messages、record_sampling、retain_sampling_history、project_sampling_snapshots | 已读完整实现及相关测试；本轮重放 5 组，包含计划重建、新旧前缀、压缩后锚点和任务数据角色 |
| 本轮正文共享 48 KiB，采样前不能淘汰，采样后按先后腾空间 | [tool_result.rs](../../crates/runtime/src/tool_result.rs) 的 ActiveToolResultLedger、make_room_for、project_messages、mark_projected_fresh_results_sampled | 已读账本路径；重放 2 组，20 KiB 是 model_body 序列化大小的教学设置 |
| 截断沿叶子范围尝试，以实际投影接纳证据 | tool_result.rs 的 project_text；orchestrator.rs 的 observe_tool_evidence 调用处 | 已读 continuation、evidence_arguments 与证据接纳；1.1—1.3 及只保留前两节是教学分支，本轮未跑实际 Book 截断 |
| 历史调用与结果保持配对，正文降为定位回执 | orchestrator.rs 的 historical_tool_receipt、tool_receipt、provider_history_projection、history_projection_through | 已读状态、book.search_text 排除、证据派生及角色/ID 保留；Native/ReAct 配对用例只读未跑 |
| 历史发现不继承激活，私有续接不进入摘要素材 | [tool_exposure.rs](../../crates/runtime/src/tool_exposure.rs) 的 redact_history；model_runtime.rs 的 project_legacy_deepseek_history；compaction.rs 的 assistant_source_content | 已读旧激活失效和模型切换投影；本轮驱动确认私有续接标记没有进入 eligible_items |
| 高水位、容量与累计用量各有口径 | [auto_compaction.rs](../../crates/runtime/src/auto_compaction.rs) 的 ActiveContextBudget::from_plan；model_runtime.rs 默认档案 | 三行预算和字符估计共 4 组重放；窗口 128,000、预留 12,000、高水位 96,000、目标 84,000 |
| 压缩消费合法旧回合，当前后缀保留 | [compaction.rs](../../crates/runtime/src/compaction.rs) 的 conversation_turns、turn_is_complete、prepare_compaction；orchestrator.rs 的 prepare_history_compaction | 已读 PreTurn/MidTurn、过去 User-only、配对与回执；重放当前问题和未完成调用原样保留 |
| 模型草稿与运行元数据分权，结构/来源/覆盖均校验 | compaction.rs 的 CompactionDraft、SourcedCheckpointItem、validate_draft、CompactionCheckpoint | 已读十个分区、必选/可选来源、白名单、替代关系与运行时组装；实际拒绝错误覆盖、伪造证据及失效旧来源 |
| 修复一次且保留前缀，分块按完整回合，安装前检查缩减 | compaction.rs 的 call_generator、generate_draft、compact_with_adapter、project_compaction_checkpoint_messages | 修复与连续失败实际重放；分层算法及其失败出口为源码阅读，未运行本轮分层夹具或真实生成器 |
| 预处理先用副本，安装后才提交活动变化 | orchestrator.rs 的 maybe_auto_compact；[agent_run.rs](../../crates/server/src/agent_run.rs) 的 RunCheckpointSink；[session_runtime.rs](../../crates/server/src/session_runtime.rs) 的 clean | 已追到用户聊天安装入口并读预检查回归；本轮投影重放不能替代实际磁盘保存测试 |
| 缓存与输入变化来自不同观测 | [agent_request_audit.rs](../../crates/runtime/src/agent_request_audit.rs) 的 AgentRequestAudit；[request_diagnostics.rs](../../crates/runtime/src/request_diagnostics.rs) 的 RequestTracker::observe；[provider_stream.rs](../../crates/runtime/src/provider_stream.rs) 的 model_usage；[run_events.rs](../../crates/runtime/src/run_events.rs) 的 ObservedAdapter | 已读逻辑计划计量、最终请求同用途比较、Provider 字段与 usage 替换；字节前缀不写成 KV 命中率 |
| 历史任务揭示旧消息变化与缓存未命中的关系 | [调用成本记录](../计划-调用成本与DeepSeek适配.md)、[Linux 观测报告](../修复记录-Linux缓存观测.md) | 引用 2026-10-03 共 18 次真实主问答调用汇总；未重跑、未读远端账单，只复算所引数字。旧预检查清理问题在当前代码已采用副本准备 |
| 默认持续执行，以停滞和其他实际失败停止 | orchestrator.rs 的 OuterConfig、tool_progress_signature、ToolCallProgressGuard、ProgressPhaseGuard；[ADR-0150](../adr/0150-resident-unbounded-tool-loop-and-progress-stops.md) | 已读文字缺口与工具共用停滞、显式轮数/实验边界和禁用工具收尾；状态机重放 1 组 |

20 组重放使用书稿目录内临时 Cargo 驱动，离线编译。context_fragment、model_runtime、compaction 复制当前非测试模块；预算、工具结果账本和停滞逻辑采用当前方法，消息与证据类型来自当前定义，ModelAdapter 和预览类型补齐最小形状，压缩草稿由脚本返回。验证范围是被调用逻辑，不是完整 Runtime/Server、磁盘安装、真实分词或模型质量。

压缩相关 8 组分别验证当前后缀、过去 User-only 素材、投影顺序、旧来源失效、一次修复、连续无效失败、伪造证据及结构合法的错误语义。最后一组实际通过接纳，说明来源链接不证明文字事实。生产 prepare_compaction 的 required_semantic_states 为空；require_open_for_test 只在测试配置存在，不能把测试中的未完成义务约束写成生产自动识别能力。

已阅读而未执行的产品用例包括 provider_history_projection_preserves_native_and_react_tool_pairing、tool_result_projection_active_fresh_bodies_share_one_turn_budget、compaction_checkpoint_install_projects_fixed_order_without_mutating_history、compaction_checkpoint_failure_is_byte_equivalent_and_stale_history_fails_closed、auto_compaction_precheck_redaction_is_committed_only_after_install、auto_compaction_ignores_cumulative_usage_when_active_request_fits。其他边界集中在正文，不扩大为实测全流程保证。

## 第 12 章的具体依据

2026-10-07 回读当前源码后完成 [第 12 章](chapters/12-来源与流式交付.md)。本章承接第 11 章的适用条件追问，分清模型片段、运行活动、公开草稿、已接纳视图和持久终态。引文、source_ref_demo、512-byte 缓冲和时刻表是教学设置；不把它们写成一次真实模型任务的记录。

| 正文结论 | 依据与关键符号 | 核对状态 |
| --- | --- | --- |
| Provider 流累计正文与工具参数，观察者不分派工具 | [provider_stream.rs](../../crates/runtime/src/provider_stream.rs) 的 ModelDelta、read_sse、model_usage；[orchestrator.rs](../../crates/runtime/src/orchestrator.rs) 的采样返回与 discard 分支 | 已读流帧、工具 index、完成检查和外层循环；4 组实际局部重放，未执行真实 HTTP/模型 |
| 活动与草稿分开，首文本和首 patch 有不同起点 | [run_events.rs](../../crates/runtime/src/run_events.rs) 的 RunActivity、RunEvents、ObservedAdapter、finish_model、answer_first_patch_ms | 已读 Request 诊断截断、usage 替换、活动完成与两个计时字段；0.8／2.3／5.9 秒仅作教学算术 |
| 草稿累计前缀，只解码顶层回答，出现工具则撤回 | [answer_stream.rs](../../crates/runtime/src/answer_stream.rs) 的 AnswerProjector、answer_field_prefix、stable_end | 已读整个模块；5 组实际草稿重放覆盖普通追加、结构字段、拆分来源、撤回和修复版本 |
| 引文仅在已观察区间定位，唯一匹配形成绑定 | orchestrator.rs 的 TurnEvidenceLedger::prepare_present、present_prepared、observed_intervals、refresh_labels；[read-tools](../../crates/read-tools/src/lib.rs) 的 match_source_quote、resolve_source | 已读零/一/多匹配、限定公式空白处理、标签与历史摘要复验；本轮没有执行 Book 引文定位夹具，编译驱动使用人工 SourceBinding |
| 公开视图的来源身份来自当前绑定 | orchestrator.rs 的 SourceBinding、AgentAnswerView、compile_answer_preview、compile_agent_answer、parse_source_marker | 已读标记、去重、已用绑定筛选；实际重放合法、领先、未知、未闭合标记 |
| 公开来源属性避免把版本号都视为 LID | orchestrator.rs 的 AnswerProvenanceLedger、violations、contains_locator_literal、explicit_answer_locator_violations；[ADR-0087](../adr/0087-provenance-aware-answer-delivery-and-compact-provider-history.md) | 已读来源通道及当前判断；公开数字碰撞与明示内部位置实际重放 |
| 当前先做有限归一化及局部 LID 修复，必要时模型修复一次 | orchestrator.rs 的 normalize_bound_source_suffixes、repair_raw_lid_leaks、deliver_agent_answer；[ADR-0086](../adr/0086-runtime-owned-user-visible-source-references.md) | 局部修复实际执行；模型修复调用、禁工具、失败回退为源码阅读，未调用模型 |
| 来源编译不判断主张真假 | compile_agent_answer 和教学绑定 | 实际接纳“条件可以全部忽略”的错误解释；所用预览明确陈述条件限制，结论只指向结构校验边界 |
| 草稿修复与事件重传使用两层顺序 | run_events.rs 的 answer_identity；answer_stream.rs 的 apply_patch；[agent-run-state.ts](../../packages/web/src/agent-run-state.ts) 的 reduceRun | 实际拒绝旧回答版本；Web 对重复 seq 的 append 保持同一状态，旧快照不被就地修改 |
| 草稿来源临时可解析，撤回后退出入口，历史点击复验 | [agent_stream.rs](../../crates/server/src/agent_stream.rs) 的 source_binding；[Server](../../crates/server/src/lib.rs) 的 workspace_source_binding、route_agent_source_resolve、route_agent_source_open | 草稿发布/撤回的来源资格实际重放；归属、出版引用、过期预览和禁止跳转为源码阅读 |
| 有界事件队列用原子快照边界恢复观察 | agent_stream.rs 的 update、read_after、with_observation、cursor、event_frame、serve_authorized；[multi_user_host.rs](../../crates/server/src/multi_user_host.rs) 的 events 路由 | 4 组缓冲重放含 512-byte 淘汰、snapshot(30) 后 event(31)、代次与终态；网络线程与观察许可为源码阅读 |
| Web 分批安装草稿并查询原 Run，重连不再提交问题 | [useAgentRun.ts](../../packages/web/src/useAgentRun.ts) 的 openSource、flushDraft、reconcile；[RightRail.vue](../../packages/web/src/components/RightRail.vue) 的草稿/来源渲染；[App.vue](../../packages/web/src/App.vue) 的 retryResidentSave | 已读联动代码；4 组直接导入 reducer 的实际重放，未做本轮浏览器渲染验收 |
| 公开终态来自保存后的 turn_view | [agent_run.rs](../../crates/server/src/agent_run.rs) 的 execute_model、execute_observed；Server 的 finalize_user_agent_turn；[session_runtime.rs](../../crates/server/src/session_runtime.rs) 的 finish | 已读 finalizing、候选会话、来源移出、效果与 TurnFinished；缓冲驱动只模拟保存成功/失败后调用 finish，不能当作落盘实测 |
| 日志确认在投影公布之前，保存可包含多条事实 | [session_log.rs](../../crates/server/src/session_log.rs) 的 append、append_with；[session_store.rs](../../crates/server/src/session_store.rs) 的 append、settle | 已读 write/flush/sync_all、投影更新和不确定写入处理；本轮未注入磁盘故障 |
| 网络使用原结果重试保存，已落盘终态可补齐后续协调 | [run_admission.rs](../../crates/server/src/run_admission.rs) 的 RunAdmissions、save_finished、retry_save、recover；agent_run.rs 的 UnsavedRun | 已读内存结果、already_saved、失败标记和重启中断分支；区分本地保留与网络重试，没有把重连写成恢复执行 |

实际重放共 23 组：Rust 19 组由来源编译 6、草稿投影 5、Provider 流 4、观察缓冲 4 组成；Web 4 组。Rust 使用临时独立 Cargo 工程离线编译，保留被调用函数正文；去除 TS 导出标注、HTTP 包装入口及未使用的队列构造器，补充最小配套类型、空取消信号和事件转发。AnswerProvenanceLedger 仅复制本次使用的公开/内部观察与违规检查方法，人工植入来源，不模拟完整 Book、消息历史、磁盘或模型。Web 直接导入实际 reducer。验证范围按调用到的逻辑记录，临时驱动在交付前清理。

已阅读而未执行的产品用例包括：provider_stream.rs 的 native_react_all_entrypoints_read_real_http_utf8_and_usage、tools_are_merged_by_index_and_incomplete_streams_fail、usage_only_frame_is_observed_before_a_truncated_stream_fails；agent_stream.rs 的 draft_snapshot_repair_and_commit_boundary_are_atomic、evicted_cursor_gets_atomic_snapshot_then_only_new_events、mu6d_epoch_and_overflow_reset_without_mixing_sequence_spaces；[agent_run_tests.rs](../../crates/server/src/agent_run_tests.rs) 的 resident_sse_streams_before_model_finishes_and_reconnect_never_dispatches、resident_run_api_cancel_and_persistence_failure_have_distinct_terminal_events、resident_commit_failure_keeps_pending_and_startup_recovers_interrupted；[mu6_tests.rs](../../crates/server/src/tests/mu6_tests.rs) 的 mu6b_unsaved_pins_chat_and_retries_exact_result_without_model、mu6b_unsaved_restart_reports_interruption_and_old_pending_stays_archived。Web 对照阅读 [agent-run-state.test.ts](../../packages/web/src/agent-run-state.test.ts) 与 [network-run.test.ts](../../packages/web/src/network-run.test.ts)，没有运行 Vitest 套件。

已知边界集中在正文：来源身份不判断语义真假；稳定边界是启发式，完整前缀反复编译有本地成本；事件队列有界不等于整个快照有界；保存后续步骤报错也可进入网络未保存分支。ADR-0127 的 Linux AS0—AS9 完成状态是历史文档记录，本轮没有复现实验，也不把其单宿主初版限制写成所有当前宿主的共同上限。

## 第 13 章的具体依据

2026-10-07 回读当前源码后完成 [第 13 章](chapters/13-持续目标与教学过程.md)，承接第 12 章已经交付的解释，继续区分任务、教学与表现。新增读者追问、speed-demo 等身份和中文题面是教学设置；平均速度、选项集合与条件计数借用现有局部测试的算例，不作为真实模型教学记录。

| 正文结论 | 依据与关键符号 | 核对状态 |
| --- | --- | --- |
| Run、聊天任务、用户教学会话各有所有者 | [goal.rs](../../crates/runtime/src/goal.rs) 的 ResidentGoal、GoalStatus；[learning.rs](../../crates/memory/src/learning.rs) 的 TutorControl、TutorSession、TutorState；[CONTEXT](../../CONTEXT.md) 的相关术语 | 已回读类型、更新路径与术语；Goal 8 组、生命周期 5 组局部用例实际执行 |
| 工作计划可以替换，不能代替用户要求和交付 | goal.rs 的 GoalWorkingState、GoalWorkItem、GoalUpdate、apply_update、objective_gap、projection | 实际覆盖省略/清空、空操作 revision、原话依据、无效列表不部分写入及 completed 不完成目标 |
| 更新先持久化，终态再核对结果 | [orchestrator.rs](../../crates/runtime/src/orchestrator.rs) 的 GoalUpdate 分支；[Server](../../crates/server/src/lib.rs) 的 append_pending_agent_turn、finalize_user_agent_turn、goal_turn_delivered；[agent_run.rs](../../crates/server/src/agent_run.rs) 的状态端口 | 已读当前调用；交付谓词直接提取执行，配套结果类型为最小替身；HTTP/日志链未执行 |
| 全局控制只属于当前用户，开关与默认教法独立 | learning.rs 的 TutorAction、TutorMutation、TutorState::apply、LearningStore::mutate、replay；[tutor_api.rs](../../crates/server/src/tutor_api.rs)；[useTutorControl.ts](../../packages/web/src/useTutorControl.ts) | 生命周期、原回执重试、过期 revision、事务回滚与重启恢复实际执行；权限设置未验收 |
| 公共素材的就绪独立于开关与原文可读 | [teaching-build.ts](../../packages/core/src/teaching-build.ts) 的 TeachingReadiness、closeTeachingStage；tutor_api.rs 的 source_readiness | 已读发布条件、四类覆盖、来源审阅、map 与 receipt；未运行新构建或就绪 HTTP 测试 |
| 教学请求冻结会话和素材，后续推进检查当前状态 | [Server teaching.rs](../../crates/server/src/teaching.rs) 的 start_request、FrozenTeachingPreparation、freeze_prepare、current、read_map；[memory teaching.rs](../../crates/memory/src/teaching.rs) 的 TeachingBinding | 已读当前调用，16 个对象摘要、8 条事实、6 项/6000 字符相关表现均来自当前常量；入口为静态核对 |
| 选择动作前分别读正式素材和原文 | teaching.rs 的 step/material/select；[tutor.rs](../../crates/runtime/src/tutor.rs) 的 TeachingMove、spec、instructions | 已读正式对象版本、material 事件、已观察范围、教法声明、评分来源和公开编译校验 |
| 候选、交付、展示、回应与帮助分别保留 | teaching.rs 的 record_user_delivery、behavior、route、scene_snapshot；[TutorActivities.vue](../../packages/web/src/components/TutorActivities.vue) 的 markDisplayed、send、可见性观察；[App.vue](../../packages/web/src/App.vue) 的 tutorAction | 已读真实链路与界面入口；Trace 幂等、不覆盖和写失败用例实际执行，Web 展示及 HTTP 链未执行 |
| 评分合同先冻结，封闭题确定性判断 | [assessment.rs](../../crates/memory/src/assessment.rs) 的 AssessmentContract、ScoringRule、assess_closed、AssessmentStatus | 2 组现有评估用例之一覆盖集合归一化、部分正确、错误、来源不足及解析失败 |
| 开放题隔离调用后接纳，失败保持未评分 | tutor.rs 的 evaluate；orchestrator.rs 的 TutorStep 与 tutor_assessment_attempts；teaching.rs 的 assessment_input、assessment_accept；assessment.rs 的 accept_open、save_assessment | 已读预算、无工具、3000 tokens、原回答/来源和接纳路径；完整性与伪造引用实际重放，未调用评估模型 |
| 学习证据保留帮助与尝试，不直接写掌握 | [learning_evidence.rs](../../crates/memory/src/learning_evidence.rs) 的 derive_assessed_evidence、correct_evidence、rebuild_learner_projection；teaching.rs 的 refresh_learning、learner_context | 现有用例实际覆盖 (1,1,1) 条件计数、未评分排除、纠正、重建和写失败；私人证据的完整领域留第 15 章 |
| 合法片段不证明评分语义正确 | assessment.rs 的 accept_open；书稿人工错误 Supported 报告 | 本轮实际接纳“最大速度”的错误解释，得到 Correct；报告有完整项目和确切来源，结论仅说明接纳器的语义边界 |
| 指令与实现仍有距离 | teaching.rs 的 select、record_user_delivery、activities；learning.rs 的 TutorSession 初始化与 TutorAction | 已核对当前显式教法优先级未全量语义判断、不同 move_id 未统一收敛；current_focus/path_instance_ref/progress_ref 未找到后续写入口。均为静态结论 |
| 当前 Tutor 和计划能力不能由旧 ADR 状态替代 | [ADR-0136](../adr/0136-resident-goal-lifecycle-and-delivery-completion.md)、[0141](../adr/0141-global-tutor-control-and-session-ownership.md)、[0142](../adr/0142-grounded-teaching-map-and-whole-source-readiness.md)、[0143](../adr/0143-teaching-trace-assessment-and-learning-evidence.md)、[0155](../adr/0155-goal-work-plan-and-version-centered-presentation-context.md)、[架构](../架构.md) 的 Tutor 小节 | 已读设计理由与后续状态，当前行为回到源码；没有重跑历史 EX13 或 Tutor 模型验收 |

本轮还阅读了 [tutor_loop_tests.rs](../../crates/server/src/tests/tutor_loop_tests.rs) 中 tutor_frozen_assessment_uses_original_key_and_retains_attempt_conditions、tutor_open_assessment_is_isolated_budgeted_and_contract_checked、tutor_candidate_failed_and_unmounted_deliveries_are_distinct，以及 [agent_run_tests.rs](../../crates/server/src/agent_run_tests.rs) 的 Goal 完成及续接用例。这些 Server 集成用例本轮未执行，不能与下面的局部驱动混称。

## 第 14 章的具体依据

| 正文结论 | 已对照实现或材料 | 依据范围 |
| --- | --- | --- |
| Reader 用叶子区间执行导航，窗口边界与请求目标不同 | [Reader](../../crates/reader/src/lib.rs)、[导航目标](../../packages/web/src/reader-navigation.ts)、[ReaderWorkspace](../../crates/server/src/reader_workspace.rs) | new、viewport、max_top_idx、goto_lid、scroll、enqueue_visible_read；书尾钳制保留 selection；前端导航测试 4 个实际执行，Rust 窗口用例仅阅读 |
| 连续正文保留已有段落，前插通过锚点补偿位置 | [App](../../packages/web/src/App.vue)、[ReaderPane](../../packages/web/src/components/ReaderPane.vue) | hydrateSegments、mergeSegments、loadWindow、onScrollEdge、doGoto、captureScrollAnchor、restoreScrollAnchor；迟到回填见下方问题记录 |
| 字符锚点比绝对像素更能表达重排前后的位置 | [reader-text-anchor](../../packages/web/src/reader-text-anchor.ts)、[ReaderPane](../../packages/web/src/components/ReaderPane.vue) | captureTextPosition、textPositionTop 与字体等待、恢复取消；四个受控 DOM 几何用例实际执行，未作真实字体和页面渲染验收 |
| Markdown 选择须映射回源串，再固定为问题输入 | [选择监听](../../packages/web/src/useReaderSelection.ts)、[Markdown 选择](../../packages/web/src/selection.ts)、[源串映射](../../packages/web/src/markdown-source-map.ts)、[App](../../packages/web/src/App.vue) | readReaderSelection、rangeToMarkdown、createMarkdownDomSourceMap、selectionRanges、askSelection、submitAgentMessage；映射的 12 个既有用例实际执行，包含标记、重复文字、UTF-16、实体与 KaTeX 原子 |
| PDF 几何、解析完整性和恢复依据分别处理 | [PDF 组件](../../packages/web/src/components/PdfReaderPane.vue)、[Server](../../crates/server/src/lib.rs)、[草稿](../../packages/web/src/pdf-selection-draft.ts)、[动作矩阵](../../packages/web/src/pdf-selection-capabilities.ts) | capturePdfSelection、rectToPdfRegion、route_pdf_selection_resolve、selection_hits_for_page、classify_pdf_selection_recovery；当前策略为 pdf_selection_recovery.v2；草稿 6、动作矩阵 4 个用例实际执行 |
| 问题引用由 Server 再次从真实范围重建 | [Server](../../crates/server/src/lib.rs) | validate_and_rebuild_selection_quote、parse_question_quote、verified_question_evidence、agent_question_with_provenance；范围顺序、合法性与一致性，raw quote 单独标识；对应 Rust 用例仅阅读 |
| 来源返回同时恢复位置和原工作对象 | [返回状态](../../packages/web/src/useReadingContinuity.ts)、[App](../../packages/web/src/App.vue)、[PDF 组件](../../packages/web/src/components/PdfReaderPane.vue) | captureReadingReturnPoint、openSourcePreview、syncAfterAgentSourceOpen、returnToAgentAnswer、captureReadingAnchor、restoreReadingAnchor；返回栈默认 16，调用方另核对 contextKey；四个状态用例实际执行 |
| 批注数据与正文几何分属不同环节 | [Reader](../../crates/reader/src/lib.rs)、[书内记录读取](../../packages/web/src/reader-annotations.ts)、[PDF 批注投影](../../packages/web/src/pdf-annotation-projection.ts)、[Server](../../crates/server/src/lib.rs) | save_highlight、select_annotation、buildPdfProjectionBatch、projectPdfAnnotations、route_user_note_save；选区型 Note 保留全部 ranges，用末尾 terminal_rect 放置标记，块/区域放置独立表达 |
| 屏幕布局是逻辑状态和当前交互的投影 | [布局计算](../../packages/web/src/workspace-layout.ts)、[ReaderWorkspace](../../packages/web/src/components/ReaderWorkspace.vue)、[现场身份](../../packages/web/src/network-context.ts) | resolveWorkspace、toggleFocus、sceneKey；732/200 与 1024/420 边界、输入优先、延后焦点；十个既有用例实际执行 |
| 排版偏好归当前设备和用户，历史滚动方案需分开 | [排版设置](../../packages/web/src/reader-typography.ts)、[设置状态](../../packages/web/src/useReaderTypography.ts)、[ADR-0148](../adr/0148-reader-typography-annotations-and-motion.md)、[ADR-0112](../adr/0112-pre-phr-reader-body-path-rollback.md)、[当前架构](../架构.md) | 偏好存 localStorage；已读 RE1—RE6 的设计与实现记录，未把历史平台验收当本轮实测；当前正文不是 PHR 有界回收窗口 |

### 第 14 章发现的已知问题

onScrollEdge 等待 hydrateSegments 后直接向此时的 segments 合并，不携带 doGoto 的请求序号或现场判断。局部驱动逐字提取这两个实际函数，用受控 Promise 模拟后文加载；等待期间将窗口从 1.1/1.2 换为 2.1/2.2，最后得到 2.1/2.2/1.3/1.4。普通目录跳转与尚未完成的预加载就能产生该顺序。这里实测的是函数合并行为，未声称启动浏览器或真实 HTTP。

App 的 confirmHighlight、highlightPdfSelection 使用 Promise.all 对每个范围分别保存，高亮组并非一个写事务。一项失败时，其余已成功的记录可能保留；本轮按源码记录这一限制，没有注入真实写失败。Reader 的部分注释仍使用旧锚点描述，正文采用实际区间实现解释。没有因这些发现修改产品代码。

## 第 15 章的具体依据

| 正文结论 | 已对照实现或材料 | 依据范围 |
| --- | --- | --- |
| 普通记录、画像与私人教学存储承担不同职责 | [MemoryStore](../../crates/memory/src/lib.rs)、[文档](../../crates/memory/src/document.rs)、[学习存储](../../crates/memory/src/learning.rs)、[UserRuntime](../../crates/server/src/user_runtime.rs) | Record、SaveInput、MemoryDocument、open_private_with_learning、learning_store、open_service；JSON 保存普通记录与画像，SQLite 保存私人教学事实，不能合称一个文件事务 |
| 新建 Note 是内容与来源身份下的零变更重试 | [MemoryStore](../../crates/memory/src/lib.rs)、[Server](../../crates/server/src/lib.rs)、[Reader](../../crates/reader/src/lib.rs)、[Resident 分派](../../crates/runtime/src/orchestrator.rs) | save_note、content_mem_id、save_user_note、ReaderNote；用户入口 long_term，Resident 建议 session；Existing 后可独立 promote，AgentEffect 只在 Created 时产生 |
| 修改内容、调整位置和持久化分开 | [MemoryStore](../../crates/memory/src/lib.rs) | replace、reanchor、promote、commit_document、persist_document_atomically；候选提交后替换内存；目标 ID 冲突与写路径失败保留旧记录；身份与修订序列实际执行 |
| 阅读活动不直接表达能力 | [阅读状态](../../crates/memory/src/reading_state.rs)、[MemoryStore](../../crates/memory/src/lib.rs) | BookReadingState、EngagementSignals、LegacyReaderProfileProjection、enqueue_read、flush_pending_reads；qa 数量的兼容标签不等于已证明的困难程度；合并与失败保留两个既有用例实际执行 |
| 画像有来源、范围、适用性与状态 | [ProfileFact](../../crates/memory/src/profile.rs) | ProfileScope、Applicability、EvidenceRef、initial_status、fact_is_resolvable、compare_resolution；历史候选单独 pending，当前解析先范围与适用性，再来源权威；信任和范围用例实际执行 |
| 明确记忆请求由运行时处理，画像与依据共同提交 | [前台意图处理](../../crates/runtime/src/memory_intent.rs)、[MemoryOp](../../crates/memory/src/operation.rs)、[Server](../../crates/server/src/lib.rs) | scan_memory_intent、evaluate_memory_intent、apply_memory_op、reduce_remember、reduce_correction、reduce_forget；profile_evidence 不进入普通 recall；Gate 与 Server 连接为源码阅读，存储操作有实际用例 |
| 后台复核以作业范围和水位提交 | [ReviewExecutor](../../crates/runtime/src/memory_review.rs)、[复核存储](../../crates/memory/src/review.rs)、[本地 Host](../../crates/server/src/host.rs)、[历史回填](../../crates/memory/src/backfill.rs) | ReviewSchedule、REVIEW_IDLE_MS、REVIEW_TURN_THRESHOLD、commit_review_result、reviewed_through；写失败时事实、观察和水位不前进已执行；没有调用提取模型或验收整个调度器 |
| 画像治理分别表达纠正、遗忘、范围变化与停止收集 | [治理](../../crates/memory/src/governance.rs)、[操作](../../crates/memory/src/operation.rs)、[界面撤销](../../packages/web/src/profile-memory.ts) | expected_document_revision、ProfileGovernanceAction、CollectionRule、correction_component、buildUndoProfileAction；同请求回放先于版本冲突判断，遗忘回执不含原文；界面连接仅阅读 |
| 快照按本轮范围与容量派生 | [快照](../../crates/memory/src/projection.rs)、[策略](../../crates/runtime/src/memory_policy.rs)、[缓存](../../crates/runtime/src/profile_context.rs)、[Server](../../crates/server/src/lib.rs) | ReaderProfileSnapshot、SnapshotBudgets、estimate_snapshot_tokens、MemoryPolicyRegistry、ProfileContextCache、user_profile_snapshot_request；2048 为条目估算预算；neutral 回退、Current/Stale 与已接入维度的边界按源码说明 |
| Markdown 是可检查的单向展示 | [Markdown](../../crates/memory/src/markdown.rs)、[MemoryStore](../../crates/memory/src/lib.rs) | render_reader_profile_md、render_handbook_md、write_profile_files、profile_markdown_projection_status、active_markdown_facts；提交后尽力刷新，current 只检查投影修订；到期过滤差异实际重放 |
| 跨书提升先生成待确认候选 | [全局合并](../../crates/memory/src/global_consolidation.rs) | reconcile_global_promotions、MIN_PROMOTION_BOOKS、MIN_PROMOTION_EVIDENCE、promotion_cluster_key；两书三证据身份，结构化 payload 相等；同书不提升、确认和来源变化三个既有用例实际执行 |
| 学习纠正追加新依据并重建投影 | [学习证据](../../crates/memory/src/learning_evidence.rs)、[教学入口](../../crates/server/src/teaching.rs) | correct_evidence、rebuild_learner_projection、learner_projection、refresh_learning、learner_context；Uncertain 替代解释、证据水位、对象与材料版本过滤；既有 SQLite 用例实际执行，Server 连接仅阅读 |
| 私人归属、当前现场与工具查询范围独立约束 | [RunScope](../../crates/server/src/run_scope.rs)、[UserRuntime](../../crates/server/src/user_runtime.rs)、[Resident 分派](../../crates/runtime/src/orchestrator.rs) | private_user、check_user、check_scene、visitor、MemoryRecall、ProfileUsageTrace；召回显式固定当前书；快照注入与模型自述使用分开，未执行跨用户集成场景 |
| 历史决策解释动机，当前存储解释已实现行为 | [ADR-0075](../adr/0075-runtime-owned-evidence-backed-profile-memory.md)、[ADR-0143](../adr/0143-teaching-trace-assessment-and-learning-evidence.md)、[术语](../../CONTEXT.md) | 画像从主 Agent 自愿工具调用转向运行时捕获与消费；教学事实、评分和学习解释分开；旧 ADR 的单 JSON 范围不能覆盖当前全部私人学习存储 |

### 第 15 章发现的已知边界

选区 Note 保存缺少问题入口那样的原文回切核对。save_user_note 仅对 ranges 中的 LID 调用 book.text，MemoryStore::save_note 验证结构却不持有 Book。本轮以 partial、所填 resolved_quote 和 `[10000,10001)` 范围运行保存，记录得到保留。公开入口可能接纳与原文长度或文字不符的选区；存储接纳实际重放，Server 路径按源码确认，没有执行 HTTP。界面通常如何产生请求不改变公开入口目前的检查范围。

Markdown 的 active_markdown_facts 只按 Confirmed/Provisional 取事实。本轮让一条用户偏好在 10:00 到期，11:00 的 resolve_profile_facts 返回零条，但两个 Markdown 渲染结果仍含该值。此时文件修订检查不能代表值仍适用于当前时间。两项均记录为产品边界，没有顺带修改实现。

当前 user_profile_snapshot_request 只填 book_id、content_profile、now，前台意图准备的 paper_subtype、domain 也为空，不能把底层的全部 applicability 维度当作已经接通的 Resident 功能。profile_status 的 stale 由私人存储不可用或本书未完成作业且 last_error 存在触发，普通无错误排队仍可 current；pending_context 仅在相应复核失败情形加入。以上为连接代码核对，未以模型结果推测。

## 第 16 章的具体依据

| 正文结论 | 已对照实现或材料 | 依据范围 |
| --- | --- | --- |
| 普通消息按差异后缀提交 | [运行提交](../../crates/server/src/session_runtime.rs)、[存储](../../crates/server/src/session_store.rs) | revision、progress、changed_goals、append；公共前缀决定 MessageAppended 或 HistoryRevised，活动按 turn/step 比较；增量落盘不代表全程恒定 CPU/内存 |
| 每聊天日志与选择元数据分开 | [SessionStore](../../crates/server/src/session_store.rs)、[对应测试](../../crates/server/src/session_store/tests.rs) | SessionPaths、load_chat_storage、open、create、select、delete、repair_selection；URL_SAFE_NO_PAD 表示文件名；五个用例直接执行，包含旧快照不读取、选择失败和两个用户重开 |
| 写入确认先于发布投影 | [SessionLog](../../crates/server/src/session_log.rs)、[日志测试](../../crates/server/src/session_log/tests.rs) | append、append_with、committed_len、uncertain、read_prefix、read_event；write_all/flush/sync_all 后 apply；部分或完整写入后报错、原事件重试、坏行拒绝、追加字节与清理用例实际执行 |
| 事件位置、消息下标、SSE 位置分开 | [事件合同](../../crates/server/src/session_event.rs)、[纯投影](../../crates/server/src/session_event/projection.rs) | SessionEvent、HistoryPosition、MessageRevision、validate、apply、fold；连续序号、单基线、pending 与终态、Goal 引用、来源归属，纯投影不执行工具 |
| 冻结输入复用已提交历史位置 | [SessionStore::accept](../../crates/server/src/session_store.rs)、[网络准入](../../crates/server/src/run_admission.rs) | FrozenTurn、map_messages、frozen_input；生产恢复取得 raw messages，测试辅助 frozen_messages 另有压缩投影，不能混写 |
| 工具分派前后通过持久化端口保存 | [循环](../../crates/runtime/src/orchestrator.rs)、[RunCheckpointSink](../../crates/server/src/agent_run.rs)、[运行提交](../../crates/server/src/session_runtime.rs) | persist_progress、persist_effects、prepare_persisted_messages、checkpoint、finish；工具活动开始后先保存，返回后保存结果及 effects；RunCheckpointSink 并非只负责摘要 |
| 终态与必要消息清理共同提交 | [TurnFinished](../../crates/server/src/session_event.rs)、[投影测试](../../crates/server/src/session_event/tests.rs)、[运行测试](../../crates/server/src/tests/session_runtime_tests.rs) | history_revision、revision、validate_persisted_checkpoint；失败后缀、Goal 引用与终态共同接纳；旧历史不兼容修订清除 checkpoint，相关用例直接执行 |
| 来源和成果保持可回指身份 | [运行提交](../../crates/server/src/session_runtime.rs)、[运行测试](../../crates/server/src/tests/session_runtime_tests.rs) | deliver_effects、deliver、link_teaching、SourcesBound、TeachingLink；教学只连接原回执；绑定来源跨压缩重开且仍可解析，用例实际执行 |
| 处置用开始记录和回执跨接私人存储 | [成果处置](../../crates/server/src/effect_disposition.rs)、[夹具](../../crates/server/src/tests/effect_disposition_tests.rs)、[回顾测试](../../crates/server/src/tests/session_recap_tests.rs) | start、execute、complete、reconcile、retained_object_for_undo；预分配高亮保留结果 ID，Note 保留核对 long_term；回顾用例实际覆盖保留再撤销和五条证据，未另行运行全部成果处置测试 |
| 本地 pending 与网络 admitted 分开恢复 | [运行恢复](../../crates/server/src/session_runtime.rs)、[用户加载](../../crates/server/src/user_runtime.rs)、[RunAdmissions](../../crates/server/src/run_admission.rs) | close_interrupted、recover、interrupted_summary、open_service、complete_preparation、repair_saved；本地恢复重复执行不增加字节已测试；claimed/queued 的网络连接为源码核对 |
| 回顾历史截点与当前可用性分开 | [回顾](../../crates/server/src/session_recap.rs)、[回顾测试](../../crates/server/src/tests/session_recap_tests.rs)、[界面](../../packages/web/src/components/SessionRecap.vue)、[状态文字](../../packages/web/src/session-recap.ts) | request、snapshot、project、resolve、Recap、Evidence、through_seq、through_at、generated_at、recapStatus；四个既有回顾用例实际执行，含只读、重开确定性、越界拒绝、待核对/保留/撤销与当前删除；Web 按源码核对 |
| Failed 在回顾中统一显示为中断 | [status](../../crates/server/src/session_recap.rs)、[失败夹具](../../crates/server/src/tests/effect_disposition_tests.rs)、[recapStatus](../../packages/web/src/session-recap.ts) | 夹具以 PROVIDER_ERROR 完成，已执行的回顾用例断言 interrupted；中文“中断”按前端映射核对，未做浏览器验收，未修改产品 |
| 累计写入公式与实际运行观测口径不同 | [ADR-0152](../adr/0152-resident-linear-jsonl-session-log.md)、[发布记录](../Linux上线-EX12-EX13-JL.md)、[原始追加回执](../performance/linux-ex13-jl-20261002/provider-smoke.json) | k=4 KiB、n=100/1000/10000 是教学模型；2026-10-02 release 的真实 Provider 追问新增 19,280 字节，原 39,631 字节前缀及其他三文件不变，非本轮重新运行 |
| 实现范围与整体发布验收分开 | [ADR-0153](../adr/0153-session-reading-recap.md)、[实施记录](../切片方案-会话JSONL与阅读回顾.md)、[发布边界](../Linux上线-EX12-EX13-JL.md) | 当前 JSONL/回顾已经实现；JL10 有 Linux 发布与备份恢复证据，Windows 整体、历史长度矩阵和完整连续使用仍未全部关闭 |

### 第 16 章的已知边界

日志首轮加载和 at 仍顺序折叠，内存保存消息与补充事实；progress 复制并比较消息，finish 构造候选，故追加字节不能代表整个请求成本。跨 JSONL、学习 SQLite、记忆和演示没有共同事务；能用结果身份确认的保留有 reconcile，其余不确定处置保持待核对。补 INTERRUPTED 回执只关闭未配对协议调用，不证明领域副作用没有发生。

load_chat_storage 从新日志加载，旧 agent-history.json 原样保留但不自动导入；选择文件不构成聊天存在的依据。回顾将全部 Failed 映射到 interrupted，Provider 失败也会显示“中断”；本轮执行的失败夹具与回顾断言确认了后端行为，前端中文映射按源码核对。产品代码未修改。

## 第 17 章的具体依据

| 正文结论 | 已对照实现或材料 | 依据范围 |
| --- | --- | --- |
| 表达方式由理解障碍决定 | [presentation-method](../../skills/presentation/SKILL.md)、[能力目录](../../skills/presentation/capabilities.md)、[ADR-0139](../adr/0139-explorable-explanation.md) | 当前方法要求具体关系与操作用途；学习率二次函数是设计金样板，本章独立重算，未开展读者学习实验 |
| 内容、候选、版本与现场分别有身份 | [数据合同](../../crates/runtime/src/presentation.rs)、[制作合同](../../crates/runtime/src/presentation_author.rs) | PresentationContent、PresentationCandidate、AgentPresentation、PresentationRef、PresentationState、PresentationFollowUp；逻辑文件、来源、假设和状态合同 |
| write、patch 保存新候选，候选限本 Run | [AuthorSession](../../crates/server/src/presentation_author.rs)、[存储](../../crates/server/src/presentation_store.rs)、[编辑用例](../../crates/server/src/tests/presentation_edit_tests.rs) | edit_base、create_presentation_candidate、created_by_turn_id；候选不可变，连续 patch 保留旧对象，新 Run 拒绝旧句柄；四个非浏览器编辑用例实际执行 |
| 预览记录绑定候选、环境与动作 | [预览合同](../../crates/runtime/src/presentation_preview.rs)、[浏览器执行](../../crates/server/src/presentation_preview.rs)、[Server 制作分派](../../crates/server/src/presentation_author.rs) | REQUIRED_PREVIEW_ENVIRONMENTS、environment、observe、interact；显式三视口、四动作上限、横向溢出、44 CSS 像素、预览问题；未重跑真实浏览器 |
| 浏览器通过与后续模型观察分开 | [Resident 分派](../../crates/runtime/src/orchestrator.rs)、[受控循环](../../crates/runtime/src/presentation_authoring_tests.rs) | pending_previews、inspected_presentations、unobserved_preview_environments、pending_plot_refs；新候选重新预览，图片进入后续成功采样；实际执行的 Runtime 用例用脚本化制作端口 |
| 数值读取不能丢掉参数与步号 | [BrowserPreview::observe](../../crates/server/src/presentation_preview.rs)、[结果投影](../../crates/runtime/src/tool_result.rs) | read_selector 唯一区域、暂停条件、page_state/controls/scene/action_step、8 KiB；project_selected_result 超预算省略整条读取；当前路径为源码核对 |
| 版本落盘、会话交付和可打开性分开 | [版本存储](../../crates/server/src/presentation_store.rs)、[公开入口](../../crates/server/src/presentation_api.rs)、[重开用例](../../crates/server/src/tests/presentation_store_tests.rs) | persist_presentation_candidate 对同候选幂等，revision 按最大已有版本加一，based_on 独立保留；delivered_version 核对回答或 domain effect；本轮执行重开/旧基底修订，已提交成果后终态中断沿当前源码与第 16 章记录 |
| 绘图与媒体在 write 时归属于版本 | [绘图](../../crates/server/src/presentation_plot.rs)、[动画](../../crates/server/src/presentation_animation.rs) | render、asset_refs、PlotAsset、RenderedAnimation、AnimationAsset；静态 SVG/PNG 与固定 MP4、代码和数据，默认尺寸、执行时限和媒体上限；未运行 Python 或 Manim |
| 固定库随内容版本保存 | [库装配](../../crates/server/src/presentation_libraries.rs)、[Reader 组装](../../packages/web/src/presentation-document.ts) | assemble、inline、metadata；当前 Konva 10.7.0，旧版本重开使用自身资源，write 重新选择、patch 保留；对应既有库用例实际执行 |
| 连续场景分离算法步骤与视觉过渡 | [场景合同](../../crates/runtime/src/presentation_preview.rs)、[前后观察](../../crates/server/src/presentation_preview.rs)、[媒体生命周期](../../packages/web/src/presentation-media.js) | semantic_state、transition_progress、seek、snapshot、actual/after_capture；本轮执行三个合同用例，浏览器定位和视频暂停逻辑只读源码 |
| 本地制作与网络隔离不同 | [AuthorSession](../../crates/server/src/presentation_author.rs)、[sandbox](../../crates/server/src/presentation_sandbox.rs) | 本地直接 renderer 与网络 Job/Sandbox 分派，Linux systemd cgroup/bubblewrap；本章只交代执行边界，未验收网络沙箱或部署 |
| 页面公开观察由宿主接纳 | [bridge](../../packages/web/src/presentation-bridge.js)、[消息归属](../../packages/web/src/presentation-host.ts)、[页面组件](../../packages/web/src/components/AgentPresentation.vue)、[公开 API](../../crates/server/src/presentation_api.rs) | observe、accepted、maskUnaccepted、capture、requestTeachingAction；文字/来源接受与页面几何变化分开，教学动作仍经宿主明确提交；前端只读实现 |
| 追问先保存确切现场 | [AgentPresentation.followUp](../../packages/web/src/components/AgentPresentation.vue)、[现场存储](../../crates/server/src/presentation_store.rs)、[follow_up_context](../../crates/server/src/presentation_api.rs)、[现场测试](../../crates/server/src/tests/presentation_store_tests.rs) | snapshot→saveState→emit；save_presentation_state、read_presentation_state、latest_presentation_state、route_with_restore；冻结回执、混淆拒绝与保存失败无回执的选定用例实际执行 |
| 共用原聊天并保留确切内容选择 | [RightRail](../../packages/web/src/components/RightRail.vue)、[工作区键](../../packages/web/src/presentation-workspace.ts)、[ADR-0144](../adr/0144-shared-presentation-conversation-workspace.md) | selectPresentation、locateScene、workspaceKey；定位/恢复分开，已选工作区不随新结果切换；附属窗口入口另读 network-presentation.ts，本章不展开网络授权 |
| 局部源码操作和新建意图明确 | [源码操作](../../crates/server/src/presentation_source.rs)、[EX13 现有测试](../../crates/server/src/tests/presentation_ex13_tests.rs)、[AuthorRequest](../../crates/runtime/src/presentation_author.rs) | read 按 chars 范围，search 字面匹配，patch 唯一命中与全有全无；follow-up write 要确切 based_on 或显式 new_object；3 个 EX13 测试实际执行 |
| 修订只继承兼容的冻结参数 | [inherit_parameters](../../crates/server/src/presentation_author.rs)、[参数用例](../../crates/server/src/tests/presentation_store_tests.rs) | 非空定义相等、Bool/Number/String 同类型；对象不继承，新键保留默认，prepared 后更新现场不替换原回执，候选 patch 不重新继承；五行算例直接对应本轮通过断言 |
| 阶段职责与设计权限分开 | [提示组装](../../crates/runtime/src/agent_prompt/presentation.rs)、[global](../../skills/presentation/phases/global.md)、[local](../../skills/presentation/phases/local.md)、[review](../../skills/presentation/phases/review.md)、[工程合同](../../skills/presentation/engineering.md) | common_modules、selected_modules、PresentationAuthoringContext、prepare；完整替换、下一采样、技术依赖、简单修订；动态 framework/focus 保持 user 数据角色，不授予来源或交付 |
| 固定指导稳定、阶段变化后部追加 | [build_sample_request](../../crates/runtime/src/orchestrator.rs)、[ADR-0154](../adr/0154-presentation-global-framework-and-staged-authoring.md)、[ADR-0155](../adr/0155-goal-work-plan-and-version-centered-presentation-context.md)、[EX13.1](../performance/presentation-context-ex13/ex13-1/README.md) | ContextFragmentLedger、presentation_guidance_state、instruction_assets；本轮执行稳定前缀回归；50,547→67,009 字节属原离线记录 |
| 成功落点允许按需回读源码 | [工具账本](../../crates/runtime/src/tool_result.rs)、[受控样本测试](../../crates/runtime/src/presentation_authoring_tests.rs)、[EX13.2](../performance/presentation-context-ex13/ex13-2/README.md) | SavedAuthorCall、insert_call、project_messages、retire_edits；首次成功回执采样保留完整输入，后续只改请求副本；本轮执行冻结 HTML 重放，四行字节表引用原报告 |
| 历史真实接续具有有限适用范围 | [EX13.6](../performance/presentation-context-ex13/ex13-6/README.md)、[ADR-0130](../adr/0130-agent-rich-presentation-and-read-time-authoring.md)、[ADR-0133](../adr/0133-agent-led-explorable-explanations-and-evolving-presentation-brief.md) | 同对象 revision 1→2、27 来源、12 项替换、3×7 路径、78 截图；379.231 秒和 20/22 次调用、token/费用为历史，不是本轮；持续 PresentationBrief 仍属条件性方向 |

### 第 17 章的已知边界

当前 preview_contract_complete 接受 legacy 或全部三个显式环境；省略 viewport 的受支持旧调用仍可取得 legacy 回执。两个预览合同用例本轮实际通过，不能把指导中的三视口要求写成所有入口都强制。Runtime 另行要求截图进入后续采样，此门仍不能证明数值、解释和学习效果正确。

Server 的 compile 错误提示建议 patch.source_ref_ids，但 AuthorRequest::Patch 没有该字段，deny_unknown_fields 会拒绝这一建议产生的请求。该可达路径由当前错误分支与反序列化合同确认，本轮未执行组合复现；新增引用当前应通过 write 指定完整列表，未修改产品代码。

state_contract 只以定义字符串和标量 JSON 类型判断继承；自定义恢复依赖页面 restorer，缺失时显示部分恢复。播放性能、真实触控、DOM 扫描成本和学习效果需要另外观测。framework/focus 为运行内交接，跨轮设计 Brief 尚未实现；历史单次接续不支持一般费用下降结论。


## 第 18 章的具体依据

2026-10-07 承接演示制作场景，完成 [第 18 章](chapters/18-Rust宿主与并发边界.md)。主线为本地 HTTP Run，网络仅追到复用执行核心、用户/现场端口、模型许可与制作沙箱；不把本地单活动 slot 推为全服务限制。9.115 秒、80/55 毫秒及读者等待数字均为本章教学设定。

| 正文结论 | 依据与关键符号 | 核对范围 |
| --- | --- | --- |
| 接入、执行与观察有独立生命周期 | [Host](../../crates/server/src/host.rs)、[协调器](../../crates/server/src/agent_run.rs) | start_server_with_memory_path 的四线程接入，is_resident_turn_request、RunCoordinator::reserve/start/run/execute；真实本机 HTTP 定向用例执行，异步返回202，旧入口等待同一核心 |
| 准备先接纳问题，再安装活动位置 | [Server](../../crates/server/src/lib.rs)、[运行测试](../../crates/server/src/agent_run_tests.rs) | prepare_agent_chat、precommit_agent_turn_with_goal、teaching::prepare 与 RunScope::capture；预提交故障后无 Provider 调用且可再次接纳的用例实际执行 |
| slot 保留到执行和保存路径退出 | [agent_run](../../crates/server/src/agent_run.rs)、[session_runtime](../../crates/server/src/session_runtime.rs)、[日志](../../crates/server/src/session_log.rs) | execute_model、execute_observed、FinishedRun、UnsavedRun、RunGuard、finalize_user_agent_turn、finish/append；取消、终态写入故障、shutdown 用例实际执行，跨文件提交沿第16章口径 |
| 提问输入与实时状态分开 | [RunScope](../../crates/server/src/run_scope.rs)、[RunContext](../../crates/runtime/src/run_context.rs)、[ReaderWorkspace](../../crates/server/src/reader_workspace.rs) | capture、ReaderInputSnapshot、reader_input、generation、select_chat/invalidate；本轮切走再切回用例确认旧输入保持且实时访问仍失效 |
| 权威状态按操作借用 | agent_run 的 AppStatePort、UserStatePort、RuntimeStatePort、BorrowedAppPort；run_context 的 ResidentStatePort | 回读完整端口定义及主要实现；外部模型不在状态闭包内，闭包仍可能进行磁盘操作；本轮等待期间导航、手工笔记及历史读取实际成功 |
| 取输入、模型判断与接纳分开 | [Server 准备后执行](../../crates/server/src/lib.rs)、[Runtime 循环](../../crates/runtime/src/orchestrator.rs) | run_precommitted_with_ports、evaluate_memory_intent、tutor_assessment_input、evaluate、tutor_assessment_accept；画像取消用例实际执行，Tutor 本轮只读连接 |
| 原用户归属与现场有效性分开检查 | run_scope 的 check_owner/check_user/check_workspace/check_scene/private_context/private_user | 旧现场用例实际保留原书记录和原聊天回答，新 Reader 与新现场消息不变；用户更换阻止私人闭包。旧现场由测试内部替换，公开本地切换另有停止边界 |
| Reader 失败会遗漏已保存记录的 effect | orchestrator 的 ReaderNote/ReaderHighlight 分支、effect_created 与 effects.push；agent_run_tests 的 mu1c_old_run_cannot_touch_replaced_workspace_but_saves_original_note_and_answer | 已执行用例同时断言私有记录存在、effects 为空；静态追到 select_annotation 失败返回 None，故不会经本轮 effects 形成成果关联；未修改产品 |
| 模型取消贯通各用途和真实传输 | run_context 的 CancellationToken/CancellableAdapter；[Native/ReAct](../../crates/runtime/src/lib.rs)、[流读取](../../crates/runtime/src/provider_stream.rs) | execute_model 安装令牌，set_run_cancellation、post_chat_completions、read_response/read_sse；迟到工具和画像接纳被阻止，Runtime 本机 TCP 用例实际验证两协议取消后不重试 |
| 取消不是立即抢断阻塞网络或提交 | provider_stream 的 read_line/into_json，NativeAdapter 的同步 send_json；[Host 超时](../../crates/server/src/host_lifecycle.rs) | Resident 请求配置300秒；同步读前后检查、请求重试边界和提交调用按源码确认，未测静默网络取消延迟或300秒期限 |
| 制作工作先回读候选，再锁外等待 | [AuthorSession](../../crates/server/src/presentation_author.rs)、[预览](../../crates/server/src/presentation_preview.rs)、[绘图](../../crates/server/src/presentation_plot.rs)、[动画](../../crates/server/src/presentation_animation.rs) | with_private、author、BrowserProcess::stop、Cdp::call、render；预览100毫秒读取超时，Python50毫秒检查和20/180秒时限；本轮未启动对应进程 |
| 网络制作取消越过资源与进程边界 | [presentation_sandbox](../../crates/server/src/presentation_sandbox.rs)、[NetworkRunPort](../../crates/server/src/workspace_registry.rs) | Execution::run、try_acquire、Sandbox::execute/stop、worker；等许可50毫秒、监测25毫秒、KillMode=control-group；子进程默认令牌，父侧停止unit。本轮只读，Linux忽略用例未选中运行 |
| 上下文切换等待时不持 AppState | agent_run 的 with_boundary、BoundaryGuard、stop_and_wait；host 的 is_review_boundary、shutdown | 实际执行 new、select、delete、book 和有序退出用例；等待时Reader仍可读取，取消终态先于宿主退出 |
| 活动时长和父子关系来自执行 | [run_events](../../crates/runtime/src/run_events.rs)、agent_run 的 RunEventFanout、orchestrator 的工具分派 | RunActivity、RunEvents::begin/finish/model、ObservedAdapter、EventScope；内层综合用例确认parent_step_id及用量只记一次，拒绝来源用例确认无started_ms/duration_ms |
| socket 写入不持状态或事件锁 | [RunStream](../../crates/server/src/agent_stream.rs) | update/read_after/serve_checked、Buffer、EVENT_BYTES；本轮五SSE连接仍可读Reader，Last-Event-ID重连不增加采样。队列淘汰与原子快照实现回读，未另跑第12章缓冲局部用例 |
| 已读触达通过原Store合并冲刷 | [MemoryStore](../../crates/memory/src/lib.rs)、[Reader](../../crates/reader/src/lib.rs)、Host冲刷函数及Server切书 | enqueue_read、pending_reads_ready、flush_pending_reads、spawn_read_ledger_worker、force_flush_read_ledger；25/250毫秒默认值按实现，实际用例用零静默/1毫秒轮询；失败保留、合并计数、重开和退出冲刷直接执行 |
| 复核独立串行，模型等待不占应用锁 | host 的 ReviewCoordinator::run_one_at、copy_review_input、serial_gate、ReviewSchedule | 受控executor确认max_active=1、配置model-a/model-b、前台写入保留；假时钟验证60秒/8轮触发；owner更换用例实际阻止后台写入 |
| 边界复核超时不取消执行 | host 的 drain_boundary_with_waiter、ReviewDrainStatus、record_review_error | fake_boundary_timeout_projects_stale_pending_context_and_visible_error 实际执行；受控超时后仍可导航，放行后任务提交并清除错误；10秒为默认等待预算，不是本轮真实等待 |
| 网络复用核心，模型活动可包含许可等待 | [RunAdmissions](../../crates/server/src/run_admission.rs)、[service_limits](../../crates/server/src/service_limits.rs)、workspace_registry | run_one 组合NetworkUserPort/NetworkRunPort/LimitedAdapter，再调用execute_model；LimitedAdapter::call/acquire_model/resource_wait位于观察包装内。当前只核对本章所需片段，未运行多人调度用例 |
| 历史理由与当前性能分开 | [ADR-0127](../adr/0127-resident-agent-streaming-and-runtime-activity.md)、[ADR-0106](../adr/0106-asynchronous-coalesced-read-ledger-persistence.md) | 0127记载长锁改造和Linux AS0—AS9；0106记载20叶请求瀑布约99%等待及合并提交决定。均为历史设计/诊断记录，本轮未重跑历史实验 |

### 第 18 章的已知边界

短时端口表示按操作借用，不保证固定毫秒上界；候选读写、会话差异、JSONL同步和已读合并仍可能持锁。同步模型网络读取与磁盘提交不被令牌强制抢断；300秒是本地Resident请求配置，不能推成取消延迟或整轮时限。Python/浏览器/沙箱的检查间隔与实际回收耗时也分开。

已读延后保存使导航成功不代表触达耐久；强制冲刷失败记录错误并保留批次，正常进程退出不能替代每次提交成功。网络Reader提交另有即时冲刷连接。复核边界超时只终止等待，既有复核继续；本地复核串行门不等于跨用户模型调度。256 KiB只约束事件队列，完整快照、Run上下文和本地观察线程另有成本。

当前真实缺口：ReaderNote/ReaderHighlight的私人写入成功后，若实时Reader动作因现场失效或冲突失败，分派返回None effect。记录留在Store，但本轮effects与由其驱动的会话成果关联缺项；工具正文仍带reader_effect.status=not_applied。旧现场既有用例本轮通过并断言这一组合，不以effects为空推断没有副作用。未修改实现，公开本地切换与内部替换的适用范围在正文分别交代。

## 第 19 章的具体依据

2026-10-07 完成 [第 19 章](chapters/19-多用户阅读与调度.md)，沿两个用户、三个窗口、原发布和原 Run 展开。ServiceState 与服务职责层次分开，当前持久聊天按 JSONL 叙述，不沿用 ADR 中早期个人 JSON 快照的现行时态。模型四秒、单份额十六秒、双模型许可八秒均为明确教学设定。

| 正文结论 | 依据与关键符号 | 核对与验证范围 |
| --- | --- | --- |
| 网络服务按用户、现场与共享能力组合 | [Authorization](../../crates/server/src/authorization.rs)、[多人 Host](../../crates/server/src/multi_user_host.rs)、[ServiceState](../../crates/server/src/service_state.rs) | new/context、start/start_with_access/dispatch；Authorization 实际持有各注册表与 Resources，不把这些字段误写进 ServiceState |
| 同用户只加载一份私人权威 | [UserRegistry](../../crates/server/src/user_registry.rs)、[UserRuntime](../../crates/server/src/user_runtime.rs)、[私有路径](../../crates/server/src/user_storage_paths.rs) | UserHandle、get/load/open_service、for_service；多窗口用例实际确认 Arc::ptr_eq 和 A/B 笔记文件分离 |
| 现场独立，私人聊天与教学共享归属 | [ReaderWorkspace](../../crates/server/src/reader_workspace.rs)、[WorkspaceRegistry](../../crates/server/src/workspace_registry.rs)、[MU5 用例](../../crates/server/src/tests/mu5_tests.rs) | residents、create_bound、request、select_chat、invalidate；实际通过不同用户/窗口导航及 Tutor 关闭、重新开启不复活旧绑定 |
| 网络 Reader 用户操作优先 | workspace_registry 的 NetworkRunPort::check_effect_revision/read_live_reader/apply_reader；[RunScope](../../crates/server/src/run_scope.rs) | 原 generation 与 effect_revision 分别判断；实际导航用例可读新现场，迟到动作返回 READER_USER_ACTION_SUPERSEDED |
| 身份在宿主构造，正文不指定私人根 | [auth](../../crates/server/src/auth.rs)、multi_user_host 的 Site::validate/dispatch | Principal 私有字段、authenticate/validate、Cookie/CSRF、loopback 与 HTTPS origin 要求；客户端 owner/dir 字段拒绝，普通 capability 表核对。登录细节按源码，本轮不声称真实代理验收 |
| 对象授权使用可信 owner | authorization 的 capability/context/history/turn/workspace、[MU8](../../crates/server/src/tests/mu8_tests.rs) | 笔记、画像与来源三个当前路径用例中的相关断言直接运行；部分私人演示由夹具植入。旧 MU4 全对象 HTTP 用例失败于夹具，未到断言 |
| 观察授权保留确切登录会话与材料 | authorization 的 observation/AuthorizedObservation::allows、[agent_stream](../../crates/server/src/agent_stream.rs) 的 serve_authorized/serve_checked | 补充用例实际验证 owner、4 个用户许可、释放后再申请、logout 后 allows=false 与原 queued 不取消；已打开连接撤权的 MU4 用例未成功执行，传输检查按源码 |
| 发布先封存文件，再事务登记 | [PublishedLibrary](../../crates/server/src/published_library.rs) | publish、dependencies、seal、record、PublishedBookRef；当前原文/PDF/assets 比较与 readiness 沿实现；本章不重复执行全部发布导入矩阵 |
| 默认发布更新不替换旧引用 | published_library 的 set_default/default_ref/load；[MU3](../../crates/server/src/tests/mu3_tests.rs) | 实际执行旧 Run、历史发布与容量引用用例：默认更新后原引用不变，旧 Run 放手后容量才释放 |
| 缓存和资源响应仍检查材料资格 | published_library 的 authorize/load/asset，multi_user_host 的 Capability::Book | 缓存命中也授权，HEAD/Range/条件头之前授权；当前 Range 可返完整200。对应旧 MU4 用例失败于夹具，本轮此项为源码核对 |
| SQLite 保存接单控制，不替代私人业务日志 | [control_schema.sql](../../crates/server/src/control_schema.sql)、[run_admission_schema.sql](../../crates/server/src/run_admission_schema.sql)、[run_admission](../../crates/server/src/run_admission.rs)、[SessionStore](../../crates/server/src/session_store.rs) | 复合主键/外键、FrozenTurn、frozen_input、HistoryPosition、load_chat_storage；JSONL 故障与原输入用例实际执行 |
| 新请求202在持久准备完成之后 | run_admission 的 admit/complete_preparation，multi_user_host 的 admit_reply | preparing→Pending Turn→教学回执/TurnPrepared→queued；实际在六个阶段注入失败后重开；本轮不是进程强杀或断电 |
| 幂等键直接比较原规范化字段 | run_admission 的 normalize/repeat/by_key/lookup | 同用户同键同内容原 turn；已执行字段变化冲突与完成后不增加模型调用。preparing 状态返回准备中，查询可用原 client_request_id |
| 教学恢复复用原事件与绑定 | run_admission 的 complete_preparation、FrozenTurn::teaching；MU6 的 jl3_teaching_receipt_reuses_frozen_attempt | 实际注入首份回执后失败，再添加晚动作并关 Tutor；重开保留原 attempt/control_revision，TurnPrepared 一次，终态后的教学写失败只补关联 |
| 领取前恢复与领取后中断分开 | run_admission 的 recover/run_one/validate_frozen/end_pending/repair_saved | jl3_admission_windows_recover_and_repeat_original_request 实际确认 preparing无turn闭键、完整未领可调度、claimed中断、terminal仅修索引；临时确认过期按源码与已读用例，不计为本轮运行 |
| 冻结输入不随切书和后续消息改变 | run_admission 的 frozen_input/FrozenTurn::prepared，workspace_registry 的 resume；MU6 的 jl3_frozen_position_survives_later_append_and_workspace_switch | 本轮实际追加较晚消息并切换现场，原调用输入不含晚消息，原书笔记保留，新Reader不动；假Provider主动失败，不能把此用例写成完成回答 |
| 原模型绑定不可用不自动换模型 | run_admission 的 ProviderBinding/configure/run_one | 本轮变更配置用例没有请求新endpoint，也没有调用原probe，以 PROVIDER_BINDING_UNAVAILABLE 收口；binding不包含凭据 |
| 取消、未保存与删除共用原占位 | run_admission 的 cancel/retry_save/delete_chat/Unsaved/ActiveGuard | 排队取消、活动笔记后取消、终态写失败重试三项实际执行；原笔记保留，聊天忙，retry-save不增加模型计数；取消是原用户turn范围 |
| 准入和模型有两层份额与轮转 | run_admission 的 run_one/State::reserved/last_owner；[service_limits](../../crates/server/src/service_limits.rs) 的 acquire_model/Permit | Run层轮转和用户份额直接测试；模型ticket、Condvar释放锁、取消移票按源码。资源用例验证独立计数、析构释放和已取消请求拒绝，未测争用队列取消延迟 |
| 默认容量分别约束不同对象 | service_limits 的 ServiceLimits::default/load/Resources::bounds | 准入20/4实际含preparing/queued/claimed，Run4/1，模型2/1，制作各1，SSE40/4，同步20/4；不是实测容量结论 |
| 旧同步等待与普通HTTP分离 | multi_user_host 的 SyncWait/admit_reply/wait_for_turn/start_with_access | 实际四等待者停在同一受控Run，B仍能读，第五等待返回原turn的202，采样一次；不把两秒测试断言当成生产性能 |
| 累计usage不构成硬停机预算 | service_limits 的 LimitedAdapter::call/output_limit/RunUsage，MU6的大usage用例 | 本轮固定两次各120001合为240002，中间笔记保存，正常完成且许可释放；流式末份累计usage及None语义按源码 |
| 制作资源有独立许可，算法不同 | [presentation_sandbox](../../crates/server/src/presentation_sandbox.rs) 的 Execution::run、workspace_registry 的 author_presentation | try_acquire每50毫秒轮询并检查取消，没有模型ticket轮转；未实际启动Linux沙箱，不能声称制作公平或平台验收 |
| 附着、分叉、接管和读写版本不同 | workspace_registry 的 WorkspaceStamp/check_attachment/check/request/persist | MU5附着与CAS、MU8的mu12读取旧revision用例实际执行；主attachment冲突分叉，takeover换代，普通读可拿最新revision而写需匹配；演示linked接线按源码 |
| 现场回收与冷加载恢复归属 | workspace_registry 的 evict_idle/busy/reserve/scene/create_bound，user_runtime 的 prepare_eviction，user_registry 的 evict_idle | 实际未来Instant回收、Run引用固定、原位置发布恢复和重启恢复；用户30分钟回收条件按源码，未实际等待；checkpoint_seq取服务端顺序 |
| 活跃Book引用仍计入预算 | published_library 的 ResidentEntry/resident_usage/evict_cache/load；[Book计费](../../crates/read-tools/src/lib.rs) 的 resident_budget_bytes | 旧Run容量用例与现场回收用例实际通过；Weak/Arc计数及2GiB计费核对，非RSS测量 |
| 重连恢复观察，不重发执行 | agent_stream 的 cursor/event_frame，run_admission 的 boot/stream；MU6观察恢复用例 | queued→执行→saved→重开并带旧游标，实际得到新epoch快照且Provider计数不变 |
| 一个服务根受单写入者协议约束 | [ControlStore/ServiceWriter](../../crates/server/src/control_store.rs)、[ADR-0147](../adr/0147-linux-multi-reader-service-without-redis.md) | acquire/open、WAL/FULL/外键/5秒busy设置；本轮相关夹具实际用SQLite3.53.2，未另跑锁/备份矩阵。无Redis是当前单主服务取舍，非分布式已实现 |
| 旧MU4夹具不适配当前日志入口 | [MU4 Fixture::seed_private](../../crates/server/src/tests/mu4_tests.rs)、[save_agent_history_path](../../crates/server/src/lib.rs)、session_store的load_chat_storage | 三个实际失败均位于mu4_tests.rs:76，SESSION_EVENT_WRITE_REQUIRED；已创建JSONL目录禁止快照写。未修改夹具，没有把失败当成运行时越权 |

### 第 19 章的已知边界

Run与模型轮转不抢占已有执行，不能给后来用户保证固定等待；制作资源只做独立计数与轮询竞争。累计usage没有调用次数、Token或金额硬额度。默认常驻数与Book估算字节不代表实测容量，已结日志和接单会继续增长；模型活动时间可包含许可等待。

多份存储没有共同事务；claimed不明执行中断而非自动重放，未保存结果继续占位。Reader检查点回滚不撤销已发生私人写入；第18章effects缺项继续保留。logout使确切登录观察资格失效但不取消原Run，权限与取消检查不抢断阻塞I/O。冷加载需要重新附着，Provider绑定不符结束原回合，临时确认丢失要求重新确认；视频同源补齐自动接入在ADR中仍为待实现设计。

本轮新确认的测试问题是旧MU4私人夹具仍写快照，被当前JSONL合同拒绝。三项完整对象/资源/已打开SSE授权用例尚未进入断言；补充用例覆盖当前私人对象和观察许可，不替代HEAD/Range与真实连接撤权验收。没有修改产品代码或测试。

## 第 20 章的具体依据

2026-10-07 完成[第 20 章](chapters/20-桌面插件与Linux发布.md)，以桌面构建、Linux共享阅读、指定归属迁入和恢复为贯穿场景。实现以当前文件为准，MU11的dev/release、制作补齐和旧程序读取是历史证据；8 GiB与24/16 GiB是教学容量假设。

| 正文结论 | 依据与关键符号 | 核对与验证范围 |
| --- | --- | --- |
| 桌面窗口连接实际本机Host | [main.rs](../../apps/desktop/src-tauri/src/main.rs)、[host.rs](../../crates/server/src/host.rs) | main/setup、web_dist、ServerHostConfig::desktop、WebviewWindowBuilder、RunningServer::shutdown；随机端口、退出收口与资源路径按源码，本轮未启动Tauri |
| Web入口由构建变量选择 | [Web main](../../packages/web/src/main.ts)、[tauri配置](../../apps/desktop/src-tauri/tauri.conf.json)、[Linux build](../../scripts/linux/build-multi-reader.sh) | VITE_MULTI_USER、NetworkApp/App、frontendDist/externalBin/resources；脚本替身验证dist与dist-multi分开，非实际Web构建 |
| 书库和Reader Provider有单独持久设置 | [library_settings](../../apps/desktop/src-tauri/src/library_settings.rs)、桌面main、Host | initial_root/resolve_library_root/apply_selection/apply_provider、save_desktop_provider_settings、provider_status、set_library_root/set_provider_config；路径不等于迁移，空Key复用及状态不返回Key均按实现 |
| Reader与插件安装分别完成 | [桌面说明](../../apps/desktop/README.md)、[NSIS钩子](../../apps/desktop/src-tauri/windows/installer-hooks.nsh)、[PluginManager](../../apps/desktop/src-tauri/src/plugin_manager.rs) | handle_maintenance_command、install_with_runner、PluginReceipt、receipt_matches_install、migrate_owned_marketplace、uninstall_owned；实际CLI与Setup未运行，相关Mock用例入口仅阅读 |
| 薄插件使用已安装二进制 | [市场入口](../../.agents/plugins/marketplace.json)、[发行manifest](../../plugins/understand-book/.codex-plugin/plugin.json)、[MCP](../../plugins/understand-book/.mcp.json)、[Book启动器](../../scripts/start-book-mcp.cmd)、[Executor启动器](../../scripts/start-build-executor-mcp.cmd) | 显式环境变量优先、HKCU InstallDir回退；四工具与bootstrap/session.v3按源码合同检查通过，没有执行真实安装缓存 |
| 两个sidecar有不同编译路径 | [build-sidecar](../../apps/desktop/scripts/build-sidecar.mjs)、[build-book-mcp](../../apps/desktop/scripts/build-book-mcp.mjs)、[桌面package](../../apps/desktop/package.json) | Bun compile Windows x64文本资源，Cargo release book_mcp及目标后缀；package:windows先检查既有sidecar、Tauri内再构建，冷检出需预生成。本轮只读 |
| 角色注册不等于插件安装 | [注册脚本](../../scripts/register-executor-agent.ps1)、[模板](../../assets/codex-agents/understand-book-executor.toml) | Scope、WorkspaceRoot、Test-AgentBytesEqual/Test-AgentTextEqual、Write-SuccessResult；6组临时项目调用直接执行，原始个人/项目配置不改；当前/未知/已知前代与备份冲突均验证 |
| 固定市场提交由发布流程承担 | [Windows说明](../Windows-Setup编译方法.md)、[发行配置检查](../../apps/desktop/scripts/assert-release-config.mjs) | 说明建议仓库@commit，publicGit正则允许无commit，不联网验证。3组实际输入确认缺失/本地路径拒绝、公开仓库通过 |
| 源码合同、产物与安装验证各有范围 | [assert-plugin-release](../../apps/desktop/scripts/assert-plugin-release.mjs)、[Book冒烟](../../apps/desktop/scripts/smoke-book-mcp-plugin.mjs)、[T7冒烟](../../apps/desktop/scripts/smoke-t7-executor-release.ts) | sourceContractOnly实际通过；后两者仅读：固定alpha搜索、脚本候选、重复/恢复/超限、installed_launcher_executed条件与capability_isolation=false；没有执行编译产物或真实模型 |
| Linux在同一release组装网络Web和五个入口 | [build](../../scripts/linux/build-multi-reader.sh)、[start](../../scripts/linux/start-reader.sh)、[CLI](../../crates/server/src/main.rs)、[源码定位](../../crates/server/src/lib.rs) | 五bin、locked/release、成功收据、workspace_root；launcher5通过，build首轮1通过1失败并单项补验；直接CLI8787与脚本8788分开 |
| 数据根、release与外部配置分开 | [unit](../../scripts/linux/understand-book-multi.service)、[环境](../../scripts/linux/multi-reader.env.example)、[Nginx](../../scripts/linux/nginx-multi-reader.conf.example)、[运行单](../Linux多人阅读-MU11发布运行单.md) | 两个unit路径与静态root同release，服务数据根独立；外部EnvironmentFile/证书/工具不自动进入服务根快照 |
| HTTPS身份与流沿代理进入实际Host | [multi_user_host](../../crates/server/src/multi_user_host.rs)、Nginx模板 | Site::new/validate/start/start_with_access、loopback/origin、应用身份、assets缓存、API no-store、buffering off及PDF代理Range；当前按源码，未运行真实代理 |
| 制作能力来自环境探针 | [Sandbox](../../crates/server/src/presentation_sandbox.rs)、[Authorization](../../crates/server/src/authorization.rs)、[配置模板](../../scripts/linux/presentation-sandbox.json.example) | load/reclaim/execute/capability、not_configured/probe_failed/ready；配置在Host创建时加载，实际Linux制作本轮只读 |
| 停止与强杀有不同终态 | CLI、multi_user_host、[RunAdmissions](../../crates/server/src/run_admission.rs)、unit | STOP_REQUESTED、shutdown、stop、取消与join/flush_reads、SIGTERM/180秒/mixed；当前信号路径只读，未重复第18章取消测试 |
| 材料发布不公开整个构建工作区 | [publish_book](../../crates/server/src/bin/publish_book.rs)、[PublishedLibrary](../../crates/server/src/published_library.rs) | dependencies/OPTIONAL/publish、readiness=reader、seal、登记/default/grant；维护夹具实际使用发布入口，未重跑完整材料矩阵 |
| 账号与发布管理共用离线写锁 | [manage_reader](../../crates/server/src/bin/manage_reader.rs)、publish_book、[ControlStore](../../crates/server/src/control_store.rs) | ServiceWriter::acquire、stdin密码、CONTROL_SCHEMA_VERSION=4、SQLite>=3.51.3；本轮维护争用用例通过，非真实账号操作 |
| 迁入固定账号、材料关系和输入快照 | [reader_maintenance](../../crates/server/src/reader_maintenance.rs) | MigrationPlan/BookMapping/canonical_plan/prepare/migration_preview/migrate_inner、known_file/material_refs、snapshot/marker/finish；MU9直接执行映射、未知文件、源PDF/学习引用、文件及元数据中断恢复 |
| 完成迁入不能覆盖新业务写入 | reader_maintenance与[MU9测试](../../crates/server/src/tests/mu9_tests.rs) | completed_files、same_file/same_tree、verify_metadata、固定plan；计划变更与完成目标变化拒绝用例实际通过 |
| 旧JSON保全与当前聊天可见性不同 | reader_maintenance、[SessionStore](../../crates/server/src/session_store.rs)、[MU6测试](../../crates/server/src/tests/mu6_tests.rs) | load_chat_storage、mu9_t60与mu9_t64：旧JSON字段保留但当前投影无旧chat；queued按JSONL接单恢复。早期MU9文档的旧History恢复描述不作现行结论 |
| 全根停写与SQLite Backup API共同确定恢复点 | reader_maintenance | copy_file/copy_tree/snapshot/backup_service、run_to_completion与integrity_check、跳过WAL/SHM/journal、排除锁/marker；实际已提交WAL用户恢复，非只复制主文件 |
| 独立根恢复保持确切发布与私人对象 | reader_maintenance、MU9/MU6 | restore_service/restore_inner、book_publications.directory前缀重定位、marker、seal_files；中断续接与快照不变通过；JL7恢复JSONL、选中聊天、来源、演示、笔记、教学和checkpoint，Probe计数0 |
| 旧版导出是指定账号的独立副本 | reader_maintenance、MU9/MU6 | export_user、busy准入拒绝、memory/private/books、checkpoint_seq、export.json；本轮用当前读取器检查对象，真实旧binary读取仅引历史MU11 |
| 历史平台证据有明确产物条件 | [MU11实施记录](../performance/linux-multi-reader-mu11-20261001.md)、[正式收据](../performance/linux-multi-reader-mu11-20261001/production-build-receipt.txt)、[制作日志](../performance/linux-multi-reader-mu11-20261001/production-render-integration.log)、[运行说明](../Linux多人阅读-现网运行说明.md) | 2026-10-01早期dev候选与正式release分开，原基线5e10516工作树副本；缺配置/权限补齐后4通过57.27秒属当次集成测试。EX13/JL和BSR7是后续文件记录，本轮未访问现网 |

### 第 20 章的已知边界

市场来源形式检查不强制commit，也不验证远端；source-contract-only在安装缓存和二进制执行之前返回。角色与四工具没有调用者认证，注册要求新任务加载。桌面Key按既有产品约定在私人settings中明文保存，状态不返回Key。

PluginManager部分补偿命令使用let _忽略错误，失败文案不证明外部恢复成功。status对收据只比较plugin_name，install的receipt_matches_install还比较marketplace_name；受支持的配置换市场且目标已有同名外部插件时，状态可能误标Setup归属。两项按源码确认，未进行真实市场操作，产品未改。

旧JSON迁入是档案保全与补绑定，当前load_chat_storage不自动导入；export-user不反向转换所有新格式给旧程序。ServiceWriter约束协作写入者，--stopped依赖旧源确实停写；snapshot文件清单比较路径与长度，不检测任意同长度改写。外部环境、证书、运行时和release由部署另行匹配，恢复命令不切入口。

Git Bash脚本重放不等于Linux部署；Server测试在Windows运行，Unix条件权限断言未执行。历史MU11容量/混合负载/实体设备等范围不被本轮局部通过关闭，未运行真实模型或在线探测。

## 第 21 章的具体依据

2026-10-07 完成[第 21 章](chapters/21-可观测性成本与效果评测.md)。从已部署的一次请求建立时间、用量和三维终态，再追踪元数据队列、任务判定与实验裁决。10.70秒时间线、0.0164虚构货币单位和3/3、1/2、0/1指标例均为教学假设。历史效果与缓存数字只回读、复算，没有重跑模型。

| 正文结论 | 依据与关键符号 | 核对与验证范围 |
| --- | --- | --- |
| 活动记录真实父子包含关系，拒绝没有执行时长 | [run_events](../../crates/runtime/src/run_events.rs) 的 RunEvents、RunActivity、scope、EventScope、begin/finish | 回读计时、用途及父级恢复；当前开始/结束与拒绝用例实际通过，父子累计算例单独复算 |
| 模型首文本与公开首patch有不同起点 | run_events 的 model_first_text_ms、answer_first_patch_ms、ObservedAdapter | 首patch只计一次、失败后保留首文本和最新usage用例实际执行；不等同浏览器绘制时间 |
| 用量取末份累计快照，子项不重复加总 | run_events、[provider_stream](../../crates/runtime/src/provider_stream.rs)、[observation](../../crates/runtime/src/observation.rs)、[mapping](../../crates/server/src/observability/mapping.rs) | usage.replace、TokenUsage::provider_reported、usage_metadata；5→7→失败保持7与思考输出子项实际通过；17,000与0.0164为教学数 |
| 三维终态与业务保存连接 | [agent_run](../../crates/server/src/agent_run.rs) 的 execute_observed、[lifecycle](../../crates/server/src/observability/lifecycle.rs) 的 business_states/delivery_failed、[RunAdmissions](../../crates/server/src/run_admission.rs) | execute_model之后，本地经with_app/finalize_agent_turn、网络经save_finished保存，再finish；正式准入一用例含正常/Provider错误/保存错误三分支，实际执行 |
| 观测根不覆盖此前queued，模型活动可含许可等待 | run_admission 的 run_one、[LimitedAdapter::call](../../crates/server/src/service_limits.rs)、[RunStream::resource_wait](../../crates/server/src/agent_stream.rs) | 当前连接回读，未重复第19章公平调度测试；教学时间线明确起点 |
| 元数据与读者正文分流 | agent_run 的 RunEventFanout、observation 的 ObservationEnvelope/ObservationMetadata、[policy](../../crates/server/src/observability/policy.rs) | 活动给两端，patch和来源绑定给流，计数及首patch时间给观测；正文canary、拒绝及证据补记用例实际执行 |
| 请求差异来自最终Provider请求，不是KV测量 | [request_diagnostics](../../crates/runtime/src/request_diagnostics.rs) 的 RequestTracker::observe、run_events 的 observe_request | Native/ReAct本机HTTP实际经过最终请求、同用途比较、用量及导出；不以字节前缀预测服务商命中率 |
| 默认关闭、队列及单迹容量有界 | [config](../../crates/server/src/observability/config.rs)、[queue](../../crates/server/src/observability/queue.rs) | from_getter、try_push、QueueLimits；off忽略全局环境和容量拒绝实际通过，1024/4MiB/64KiB/512按当前默认值 |
| 导出线程在发送前持久化，按父链确认 | [ObservabilityRuntime](../../crates/server/src/observability/mod.rs)、[HTTP传输](../../crates/server/src/observability/langsmith.rs) | export_loop、send_with_retry、CreateState；父确认/父拒绝和持久重放实际执行。HTTP状态分类、409及退出超时连接本轮按源码，不跑真实外网 |
| 恢复观测不补业务成功或离线时长 | [spool](../../crates/server/src/observability/spool.rs) 的 open/persist/synthesize_interrupted_updates | last_observed、unknown和reconstructed；本轮重开、损坏隔离、目标变化、限额/保留期用例实际通过，无掉电或强杀 |
| 队列与退出存在完整性边界 | queue 的 pop/begin_shutdown，mod 的 export_loop/status/note_drop/root finish | 到期clear不逐项记drop；入队到persist间隙；根确认清理整trace及后台发送失败统计按源码推导，未新注入真实超时或故障 |
| 构建按持久回执投影，不造内部模型事实 | [prebuild-projector](../../packages/observability/src/prebuild-projector.ts)、[prebuild-export](../../packages/observability/src/prebuild-export.ts)、[export-ledger](../../packages/observability/src/export-ledger.ts) | projectPrebuildReceipts、readPrebuildProjection、timing/projectUsage、ensure/confirm；三项选中测试实际区分attempt并验证源不变、重复跳过与失败不确认 |
| 评测导出保留合同、授权范围与未知 | [eval-adapt](../../packages/observability/src/eval-adapt.ts)、[合同](../../packages/observability/src/eval-export-contract.ts)、[feedback](../../packages/observability/src/eval-feedback.ts)、[consent](../../packages/observability/src/eval-consent.ts)、[import](../../packages/observability/src/eval-import.ts) | 旧QualityReportSource形状、EvalScore来源、unavailable、不适用、authorizeEvalImport、timingFailures及uncertain；三项导入测试使用RecordingTransport，未外发 |
| 任务层次与交互条件分别固定 | [task-spec](../../evals/semantic/task-spec.mjs)、[reading-dataset](../../evals/semantic/reading-dataset.mjs)、[ADR-0137](../adr/0137-stratified-reading-evals-and-diagnostic-improvement.md) | requirements、reading_level、interaction_condition、productInput；当前代码与ADR决策回读，没有将恢复当成最高理解层 |
| 召回只算实际可见且被接纳的原文 | [agent-core](../../evals/semantic/agent-core.mjs) 的 observedEvidence、toolMessages、scoreNatural、aggregateNatural | 真实正文/元数据区别、缺失用量和动作条件的两项用例实际执行；旧百分比按必要组/事实/任务分别计 |
| 支持与定位分开，依据位置由程序算 | [task-quality-v2](../../evals/semantic/task-quality-v2.mjs) 的 v2Requirements、resolveV2Basis、validateV2Judgment、v2RequirementJudgment | 六项basis用例直接执行，覆盖after公式、185字范围、互补来源、歧义/错part/公共参考拒绝及旧数值记录；语义仍由评分判断 |
| 失败、未决、评分状态与偏好分别保留 | [task-quality](../../evals/semantic/task-quality.mjs) 的 productState、scoringStatus、taskQualityReport | 四项定向汇总用例实际覆盖已确认失败优先、来源支持/定位分离、评委超时与证据不足；完整分母没有删除未知样本 |
| 导航和恢复必须经过真实状态观察 | agent-core 的 navigationOK， [agent-run](../../evals/semantic/agent-run.mjs) 的 readTurn/restart | 代码回读，动作断言局部执行；真实进程重启及前端点击本轮未运行，LA10按历史记录引用 |
| 计时、请求用途与评委成本分账 | [eval-timing](../../evals/semantic/eval-timing.mjs)、[provider-recorder](../../evals/semantic/provider-recorder.mjs)、[agent-report](../../evals/semantic/agent-report.mjs)、[core](../../evals/semantic/core.mjs) | runTimedProduct、mergeProductTimings、requestPurpose、usageByPurpose、measuredUsage、percentile；六项记录器和两项假时钟用例实际通过 |
| 当前记录器不提取SSE usage且整包缓冲 | provider-recorder 的 startProviderRecorder、arrayBuffer/JSON.parse | 书稿内一项本机SSE重放：携带13 tokens，转发字节一致，记录non_json_response、1缺失、total=null；不是Provider真实缺用量的证明 |
| 产品比较不能归因单项图谱能力 | [LA7/LA9实施](../LA7-LA10实施与验收.md)、[LA7摘要](../../evals/semantic/results/2026-09-08-la7/summary.json)、[LA9摘要](../../evals/semantic/results/2026-09-08-la9-v2/summary.json) | 当时固定程序/题集/模型，15/24对20/24、三组10/11/9、用量和P95只复算；旧独立修复不拼回主表 |
| 构建成本未知，不能给数值回收点 | [LA9 prebuild](../../evals/semantic/results/2026-09-08-la9-v2/prebuild.json) | 530次、34,814,058字节、actual_provider_tokens与elapsed_wall_ms未知；并行累计与墙钟分开 |
| 当前默认没有历史十二轮限制 | [OuterConfig](../../crates/runtime/src/orchestrator.rs)、[AGENT_EVAL](../../evals/semantic/AGENT_EVAL.md)、[ADR-0150](../adr/0150-resident-unbounded-tool-loop-and-progress-stops.md) | max_turns=None；旧默认说明落后，LA9的十二轮仍是该历史合同，没有改旧协议文档 |
| 缓存百分比需要加权，canary与正式入口不同 | [Linux缓存记录](../修复记录-Linux缓存观测.md)、[调用成本计划](../计划-调用成本与DeepSeek适配.md)、host/multi_user_host/run_admission | 历史18次输入及缓存比例复算；后续正式入口已接线、本轮准入用例通过；未读远端或重跑模型 |
| 充分证据与共同回答器是干预 | [diagnostic-core](../../evals/semantic/diagnostic-core.mjs)、[EV5—EV6记录](../performance/agent-eval-ev5-ev6-20260925.md) | evidenceIntervention、actualCanonicalEvidence、commonAnswerMessages；三项既有用例实际通过。660/1362/5550为历史输入，未复现真实语义评分 |
| 量尺修订与程序改进分别报告 | [EV3—EV4](../performance/agent-eval-ev3-ev4-20260925.md)、[CQ8](../performance/source-delivery-cq8-20260926.md) | 18样本、评分无效及未批准范围、十案人审校准和九案开发诊断分开，未声称当前全量效果重测 |
| EV7已完成并撤回候选 | [EV7记录](../performance/agent-eval-ev7-20260926.md)、[agent_prompt](../../crates/runtime/src/agent_prompt.rs) 的 FINISH_POLICY_REVISION | 六对十二份、四对迁移一改善两持平一未决；当前v7与恢复记录相符。语义核查是当时Codex对照，不冒充人审；ADR状态行较早 |

### 第 21 章的已知边界

活动覆盖、用量完整度与效果判定分开；没有用宿主duration推导GPU计算、锁占用或屏幕首字。入队尚未落盘可能丢观测，发送失败及到期清空没有完整回填每个外部根的coverage；根终态确认会清除同迹spool。两秒退出预算不抢断已在途的十秒传输。这些按当前代码记录，本轮没有新做真实网络超时或断电实验。

固定SSE重放确认当前评测代理JSON-only用量解析缺口：Provider已发送13，记录总量仍未知；整包缓冲也不适合测生产首片段。没有顺带修改产品。eval-adapt沿旧质量报告形状，新task-v2不能原样作为同合同输入。默认十二轮及ADR的EV7待实施分别落后于当前OuterConfig与后续实验记录；只在书稿解释版本，不改旧证据。

历史效果保持单书、小样本、程序与评委协议条件。旧语义枚举冲突、支持与定位混判、未经批准范围和未决均不抹去；量尺变化不当成能力提升。工程交付、来源可核验与真实学习效果仍需分别证明。

## 第 22 章的具体依据

核对日期：2026-10-07，HEAD 4e0b68e。机制承接前章，本轮回读对本章选择理由有决定作用的路径；设计文件说明当时约束，当前实现决定今天的行为。下表不是把整份历史 ADR 都视为当前合同。

| 正文位置与结论 | 当前实现或历史材料 | 核对重点 |
| --- | --- | --- |
| 22.1 本地构建与独立阅读进程 | [ADR-0001](../adr/0001-本地plugin形态.md)、[ADR-0021](../adr/0021-实现技术栈-预构建ts-读时后端rust-前端待定-基座schema-rust权威ts-rs生成.md) | 2026-06-21 本地形态及同日 localhost 补充；次日 TS/Rust 分工的约束，不推导语言性能比值 |
| 22.1 候选接纳与材料交付 | [build-orchestrator.ts](../../packages/core/src/build-orchestrator.ts)、[automatic-build-publication.ts](../../packages/core/src/automatic-build-publication.ts)、[published_library.rs](../../crates/server/src/published_library.rs) | routeBookStructureProductionStage、publishAutomaticBuildArtifactSet 与 PublishedLibrary 分属构建路由、成果提升、网络材料登记 |
| 22.1 类型、坐标与当前技术栈 | [base-schema](../../crates/base-schema/src/lib.rs)、[Span.ts](../../packages/core/src/generated/Span.ts)、[Book](../../crates/read-tools/src/lib.rs)、[runtime 依赖](../../crates/runtime/Cargo.toml)、[server 依赖](../../crates/server/Cargo.toml)、[Web 入口](../../packages/web/src/main.ts) | Rust 类型生成 TS；Span 的 UTF-16 单位与 source_u16；当前依赖及 Vue 入口，不继承 ADR“前端待定”状态 |
| 22.2 位置与语义策略分开 | [ADR-0123](../adr/0123-lid-foundation-and-reliable-agent-completion.md)、[Book](../../crates/read-tools/src/lib.rs)、[LA9-v2 汇总](../../evals/semantic/results/2026-09-08-la9-v2/summary.json) | LID 保留依据、lid_idx/node_idx；历史同环 10/24、11/24、9/24 不能作单项消融归因 |
| 22.2 不做向量与有限引入的演进 | [ADR-0002](../adr/0002-图谱中心-砍向量索引.md)、[ADR-0149](../adr/0149-构建期语义候选召回与Core-owned-Embedding-Provider.md)、[ADR-0151](../adr/0151-book-structure-global-outline-and-semantic-retrieval.md) | 早期本地依赖成本与转述召回条件；后续 formal_objects、book_structure 是不同消费者，相似不代替身份判断 |
| 22.2 受控准备和依赖复用 | [automatic-build-retrieval.ts](../../packages/core/src/automatic-build-retrieval.ts)、[semantic-retrieval-preparation.ts](../../packages/core/src/semantic-retrieval-preparation.ts)、[build-intent.ts](../../packages/core/src/build-intent.ts) | prepareBuildRetrieval 确认、配置、消费者、Provider 与预算入口；retrievalPreparationMatches 比较当前完整依赖 |
| 22.2 融合及候选内容保留 | [semantic-retrieval.ts](../../packages/core/src/semantic-retrieval.ts)、[book-structure-retrieval.ts](../../packages/core/src/book-structure-retrieval.ts)、[book-structure-organization.ts](../../packages/core/src/book-structure-organization.ts) | exact/alias 优先、每页六项、语义新增上限；短向量与完整词法字段不同，默认 fallback 为 lexical_only |
| 22.2 历史排名和完整验收 | [SR6 报告](../performance/semantic-retrieval-20260930.md)、[SR6 机器记录](../performance/semantic-retrieval-sr6-20261001.json)、[BSR7 报告](../performance/book-structure-bsr7.md) | 排名 12.5%/12.5%/100% 不补齐未完成 Agent 质量；SR6 未通过，BSR7 保留书籍、查询、usage 和用户选择条件 |
| 22.3 默认轮数与进展 | [ADR-0150](../adr/0150-resident-unbounded-tool-loop-and-progress-stops.md)、[orchestrator.rs](../../crates/runtime/src/orchestrator.rs)、[service_limits.rs](../../crates/server/src/service_limits.rs) | max_turns=None、turn_limit_reached、tool_progress_signature、两类进展守卫；并发及单次请求约束不等于累计费用限额 |
| 22.3 当前输入与历史投影 | [run_context.rs](../../crates/runtime/src/run_context.rs)、[auto_compaction.rs](../../crates/runtime/src/auto_compaction.rs)、[tool_result.rs](../../crates/runtime/src/tool_result.rs)、[tool_exposure.rs](../../crates/runtime/src/tool_exposure.rs) | 活动输入加预留、48 KiB model_body 范围、历史回执、旧能力清理；累计 Provider usage 不作窗口用量 |
| 22.3 指导与压缩提交 | [context_fragment.rs](../../crates/runtime/src/context_fragment.rs)、[compaction.rs](../../crates/runtime/src/compaction.rs)、[orchestrator.rs](../../crates/runtime/src/orchestrator.rs)、[ADR-0155](../adr/0155-goal-work-plan-and-version-centered-presentation-context.md) | record_sampling 的追加位置与角色；maybe_auto_compact 使用副本，安装成功才写回，来源校验不升级为摘要语义保证 |
| 22.3 工作计划与任务要求 | [goal.rs](../../crates/runtime/src/goal.rs)、[ADR-0155](../adr/0155-goal-work-plan-and-version-centered-presentation-context.md) | apply_update 的 working/revision；工作项 completed 不自动改变 Goal 生命周期，不代替必要页面交付 |
| 22.4 日志形状与提交 | [ADR-0152](../adr/0152-resident-linear-jsonl-session-log.md)、[session_runtime.rs](../../crates/server/src/session_runtime.rs)、[session_log.rs](../../crates/server/src/session_log.rs)、[session_store.rs](../../crates/server/src/session_store.rs) | 消息公共前缀与差异后缀；追加成功才发布投影，聊天日志与压缩输入分开，旧快照保留 |
| 22.4 用户权威与现场代次 | [ADR-0147](../adr/0147-linux-multi-reader-service-without-redis.md)、[user_registry.rs](../../crates/server/src/user_registry.rs)、[workspace_registry.rs](../../crates/server/src/workspace_registry.rs) | 同用户一份 UserHandle；NetworkRunPort 原用户写入、原现场检查和 effect_revision；不以缓存位置代替业务归属 |
| 22.4 接单恢复与材料身份 | [control_store.rs](../../crates/server/src/control_store.rs)、[run_admission.rs](../../crates/server/src/run_admission.rs)、[published_library.rs](../../crates/server/src/published_library.rs) | ServiceWriter 与单主服务，SQLite/JSONL 无共同事务；recover 先修终态再处理 claimed，不自动重放不明副作用；PublishedBookRef 固定发布 |
| 22.5 重新讨论的条件 | [ADR-0134](../adr/0134-optional-langsmith-observability-and-evaluation.md)、[ADR-0137](../adr/0137-stratified-reading-evals-and-diagnostic-improvement.md)、[第21章](chapters/21-可观测性成本与效果评测.md) | 观测是可替换投影；实验保留条件、退化及撤回结果；未来方案表属于基于约束的分析 |
| 当前 Runtime 验证 | [orchestrator 测试](../../crates/runtime/src/orchestrator.rs)、[容量测试](../../crates/runtime/src/auto_compaction.rs)、[Goal 测试](../../crates/runtime/src/goal.rs) | 本轮实际运行八项，固定模型与安装端口；详见下方记录 |
| 当前 Core 验证 | [融合测试](../../packages/core/test/semantic-retrieval-hybrid.test.ts)、[准备测试](../../packages/core/test/automatic-build-retrieval.test.ts)、[结构检索测试](../../packages/core/test/book-structure-retrieval.test.ts)、[固定 Provider](../../packages/core/test/fixtures/embedding-provider.ts) | 本轮实际运行六项，真实产品函数、临时文件、固定向量；未选中用例不计通过 |

### 第22章的已知边界

1. ADR 中的前端待定、候选库、旧聊天形状、十二轮和阶段待验收是各自日期的状态。正文只将当前路径已核实的部分写为实现；历史 LA9、SR6、BSR7 与前章 EV7 按原条件引用，不把它们拼成同一个新实验。
2. 生成类型不自动证明坐标范围或语义。LID、来源读取、相似召回和任务成功分属不同责任；图谱的产品对照不能单独裁定位置合同，召回提高不能补造完整身份质量。
3. 默认持续执行不提供累计费用或总时间硬保证；活动上下文容量、并发许可和单次输出分别有自己的约束。48 KiB 只约束活动工具 model_body，压缩校验不证明摘要所有主张正确。
4. JSONL 减少重复写入但保留读取、比较和复制成本；本章 50.5 倍是简化写入量比例，未测产品延迟。单主服务写入者锁不是跨服务器协调，SQLite、JSONL 与领域文件没有统一事务，claimed 不明执行不自动重跑。
5. SR6 排名改善与两轮真实 Gold 验收未通过同时成立。BSR7 主成果依据特定运行和用户选择，不能外推其他消费者、默认策略或普遍费用收益。当前十四项定向测试只证明相应控制流与数据合同，未新增真实效果实测。

## 第 23 章的具体依据

本章按“具体任务、触发症状、定位、当时处理、当前机制、回归条件”展开。下表记录结论对应的材料，不将报告中的旧错误码、旧路由或待实施状态直接写成当前行为。

| 章节主题 | 已核对的来源 | 支撑的结论与适用范围 |
| --- | --- | --- |
| 23.1 召回与任务差距 | [LA7—LA10](../LA7-LA10实施与验收.md)、[agent-core.mjs](../../evals/semantic/agent-core.mjs) | 2026-09-08固定32题中24道共享问答，Agent锚组召回100%、问答15/24，Chunk20/24；observedEvidence查实际工具正文，scoreNatural区分证据、事实和动作 |
| 23.1 评分与诊断 | [EV3—EV4](../performance/agent-eval-ev3-ev4-20260925.md)、[EV5—EV6](../performance/agent-eval-ev5-ev6-20260925.md)、[diagnostic-core.mjs](../../evals/semantic/diagnostic-core.mjs) | target-l2原内容满足而来源失败；前两次合法工具干预660 UTF-16字符；共同回答器各自材料1362/5550，6000上限；枚举冲突保留评分无效、未决 |
| 23.2 引用后缀与旧修复 | [LA1—LA6](../LA1-LA6实施与验收.md)、[orchestrator.rs](../../crates/runtime/src/orchestrator.rs) | 冒号遗漏使已绑定来源未进入正式引用；当前normalize_bound_source_suffixes含冒号；历史单题仍因无工具收尾违规失败，格式修复不等于整题通过 |
| 23.2 当前来源和终答 | [orchestrator.rs](../../crates/runtime/src/orchestrator.rs)、[第12章](chapters/12-来源与流式交付.md) | TurnEvidenceLedger的prepare_present与present_prepared、实际循环调用、compile_agent_answer与deliver_agent_answer；当前只用非空连续quote，唯一匹配；确定性修复后至多一次无工具模型修复 |
| 23.2 选区、动作与持久化 | [LA10记录](../LA7-LA10实施与验收.md)、[App.vue](../../packages/web/src/App.vue)、[本地入口](../../crates/server/src/lib.rs) | Markdown选区复用markdownSelectionContext；笔记验收等错端点；来源打开漏存session的历史修订及当前成功goto_lid后的save_session |
| 23.2 保存与恢复准备 | [agent_run.rs](../../crates/server/src/agent_run.rs)、[run_admission.rs](../../crates/server/src/run_admission.rs)、[第16章](chapters/16-会话日志与恢复.md)、[第19章](chapters/19-多用户阅读与调度.md) | execute_model、execute_observed、save_finished、retry_save、complete_preparation、recover的当前调用；复用原结果、终态对账、冻结准备与claimed中断分开 |
| 23.3 真实交接超时 | [9月30日构建恢复](../performance/build-control-recovery-20260930.md)、[executor session](../../packages/core/src/automatic-build-executor-session.ts)、[Driver](../../skills/build/automatic-build-driver.ts) | 三路九次120秒超时，启动落盘仍成功；请求内task descriptor复用、活动恢复使用当前时钟；修复前后秒数仅属于旧全书stitch准备 |
| 23.3 语义尝试与代次 | [task store](../../packages/core/src/automatic-build-task-store.ts)、[executor session](../../packages/core/src/automatic-build-executor-session.ts)、[构建编排](../../packages/core/src/build-orchestrator.ts)、[第7章](chapters/07-模型工作单元与执行调度.md) | nextAutomaticBuildExecutionIdentity区分候选失败与租约接管；同范围字段反馈；单交接DONE不等于整书DONE，任务分母可随展开改变 |
| 23.3 章节选择放不下 | [BSR7](../performance/book-structure-bsr7.md)、[book-structure-planning.ts](../../packages/core/src/book-structure-planning.ts) | 114重点/44宏观引用，候选1981、完整2643、下界2228；从已接受动作22接续，48/48/18及final；现有selection_draft保留已接受事实，旧工作任务不改写 |
| 23.3 质量检查与发布 | [构建恢复记录](../performance/build-control-recovery-20260930.md)、[quality](../../packages/core/src/automatic-build-quality.ts)、[publication](../../packages/core/src/automatic-build-publication.ts)、[close](../../packages/core/src/automatic-build-close.ts)、[第8章](chapters/08-构建恢复与成果发布.md) | 历史702/702仍完整性失败；当前按叶序号覆盖、确切final与依赖闭包检查；发布与收口继续核对正式文件、质量和新鲜度 |
| 23.4 演示范围和计算错误 | [EX12.4](../performance/presentation-staged-authoring-ex12-4.md)、[独立代入值](../performance/presentation-staged-authoring-ex12-4/expected-calculations.json) | v5已进入review仍漏两练习和1.3.4；状态读取使正交点后移、无正分母时无交点；原105.4797减半为52.7399，动态变化不证明静态摘要正确 |
| 23.4 指导和源码投影 | [ADR-0155](../adr/0155-goal-work-plan-and-version-centered-presentation-context.md)、[context_fragment.rs](../../crates/runtime/src/context_fragment.rs)、[tool_result.rs](../../crates/runtime/src/tool_result.rs) | record_sampling追加变化指导；SavedAuthorCall只处理成功保存的write/patch，首次后续采样后才用saved_source投影，旧edits在后继已保存且观察后收敛 |
| 23.4 预览与版本交付 | [Runtime循环](../../crates/runtime/src/orchestrator.rs)、[Server制作](../../crates/server/src/presentation_author.rs)、[sandbox](../../crates/server/src/presentation_sandbox.rs)、[第17章](chapters/17-可探索解释与演示版本.md) | pending_previews和inspected_presentations按候选跟踪；图片进入后续成功采样；based_on确切版本或显式new_object；沙箱执行资源不裁决内容语义 |
| 23.4 旧历史字节与真实接续 | [Runtime制作](../../crates/runtime/src/presentation_author.rs)、[EX13.6](../performance/presentation-context-ex13/ex13-6/README.md) | 条件保留已有new_object字段；EX13.6经准备失败后真实交付同对象revision 2、27来源；11项加公式修订、三环境七路径78截图、20次成功接续及2次先前辅助调用 |
| 23.5 来源评分修订 | [CQ8](../performance/source-delivery-cq8-20260926.md)、[task-quality-v2.mjs](../../evals/semantic/task-quality-v2.mjs) | resolveV2Basis将逐字依据定位为UTF-16区间；C4.after[7,89)；原10个人审判例一致，新增9个开发例7个标签一致，差异保留 |
| 23.5 单变量与撤回 | [EV7](../performance/agent-eval-ev7-20260926.md)、[当前指导](../../crates/runtime/src/agent_prompt.rs)、[ADR-0137](../adr/0137-stratified-reading-evals-and-diagnostic-improvement.md) | 六对十二份首次回答，四对迁移题1改善2持平1未决；控制题新增章节断言，候选撤回；当前finish v7，旧待实施状态不再适用 |
| 23.5 观察和判定口径 | [EV7](../performance/agent-eval-ev7-20260926.md)、[task-quality.mjs](../../evals/semantic/task-quality.mjs)、[第21章](chapters/21-可观测性成本与效果评测.md) | MSE在交付扩展前文但不在实际工具正文；产品状态、评分状态、已知必要失败与未决分开，不以未知评分消除已知失败 |
| 当前记录器缺口 | [provider-recorder.mjs](../../evals/semantic/provider-recorder.mjs)、[agent-run.mjs](../../evals/semantic/agent-run.mjs)、[当前来源修复](../../crates/runtime/src/orchestrator.rs) | v4未被v3用途识别分支接受；整包JSON解析丢SSE usage；measuredUsage保留null，但可选tokenLimit将缺失作为0累计 |
| 本轮现有用例 | [executor session测试](../../packages/core/test/automatic-build-executor-session.test.ts)、[Driver测试](../../packages/core/test/automatic-build-driver.test.ts)、[章节选择测试](../../packages/core/test/book-structure-chapter-selection.test.ts)、[Runtime历史测试](../../crates/runtime/src/presentation_author.rs) | 6个Core、1个Runtime实际通过；固定输入、临时目录及受控时钟；两项记录器探针另列，不算产品正确用例 |

### 第23章的已知问题与边界

1. 历史LA、EV、CQ8、构建恢复与BSR7、EX12—EX13属于不同条件，不能合并成功率、费用或性能收益。本文启动耗时测量准备落盘；候选Token是合同估算；公式重算是历史模型代入，不是设备实测。
2. 当前来源编译不裁决完整语义，当前legacy预览仍可交付，Patch错误提示仍包含不受合同接受的source_ref_ids字段。后两项回读当前代码并复用第17章证据，没有重复运行其Server矩阵。
3. 保存重试依赖原内存结果或已存终态，跨存储没有共同事务；构建可捕获异常回滚不证明断电原子性。当前构建路线与旧全书stitch不同，历史启动秒数不作当前全路径承诺。
4. 新复现：source_answer_repair.v4被requestPurpose计为unclassified，用途统计会漏计source_repair；总用量并未因此消失。新复现：SSE有usage而记录为null时，tokenLimit=1仍放行两次各7 tokens的请求。measuredUsage继续保持未知，不补零；预算和报告的行为分开。未修产品。
5. EV7完成且撤回，当前v7；首次歧义、基线过强推论和候选控制题的新断言都保留。CQ8开发标签与人审校准不同；未决评分不直接记为产品失败。真实浏览器、模型和费用记录仅引用历史，本轮未重跑。

## 第 24 章的具体依据

| 正文结论 | 已对照实现或材料 | 依据范围 |
| --- | --- | --- |
| 介绍从持续阅读任务进入构建与阅读分工 | [项目入口](../../README.md)、[构建编排](../../packages/core/src/build-orchestrator.ts)、[Book加载](../../crates/read-tools/src/lib.rs) | 使用已接纳材料，Book::load与Book::new加载正文和索引；桌面及网络组合回查第20章，不新增部署验收 |
| 白板中的Book能力与私人状态责任不同 | [MCP分派](../../crates/server/src/mcp.rs)的dispatch_mcp_tool与dispatch_artifact_tool；[第10章](chapters/10-工具与模型调用边界.md) | Visitor提供Book和成果读取，没有私人Reader、Memory写入口；外部模型循环不归Resident控制 |
| 一次请求固定原归属，而现场和私人状态分别判断 | [RunScope](../../crates/server/src/run_scope.rs)的capture、check_user、check_scene、private_user；[NetworkRunPort](../../crates/server/src/workspace_registry.rs)的private | 当前字段与调用顺序；正文摘录授权、用户锁、原归属检查和私人操作四行，未把原书私人写入与任意现场动作视为同一合同 |
| 准备、模型执行和保存是不同阶段 | [本地准备](../../crates/server/src/lib.rs)的prepare_agent_chat；[执行](../../crates/server/src/agent_run.rs)的execute_model；[网络准入](../../crates/server/src/run_admission.rs)的complete_preparation、recover、save_finished、retry_save | 回读冻结输入、准备落盘、claimed且无终态的中断收口；终态与来源提交复用第12、16、19、23章已核实路径，不重跑其Server用例 |
| 来源匹配限定在本轮已观察区间 | [运行循环](../../crates/runtime/src/orchestrator.rs)的TurnEvidenceLedger::prepare_present、observed_intervals、compile_agent_answer与deliver_agent_answer；[原文匹配](../../crates/read-tools/src/lib.rs)的match_source_quote | 两行摘录连续来自当前方法；唯一匹配形成绑定，零匹配与歧义反馈分开，合法绑定不证明开放语义正确 |
| 默认持续执行、活动容量和通用完成条件分别负责不同问题 | [进展](../../crates/runtime/src/orchestrator.rs)的OuterConfig与ProgressPhaseGuard；[容量](../../crates/runtime/src/auto_compaction.rs)的ActiveContextBudget::from_plan；[交付条件](../../crates/server/src/lib.rs)的goal_turn_delivered | 当前max_turns=None；容量按单次请求估计；Content分支返回true不等于逐项语义验收。第22章定向结果只作为此前证据 |
| 构建推进不靠子执行器自然语言汇报决定 | [编排](../../packages/core/src/build-orchestrator.ts)的nextAutomaticBuildAction；[执行身份](../../packages/core/src/automatic-build-task-store.ts)的nextAutomaticBuildExecutionIdentity | 回读阶段准备、工作选择、收口，以及semantic_attempt与lease_epoch变化；交接、候选和发布失败的完整因果链回查第7—8、23章 |
| 日志重试处理的是原事件，不是所有领域的共同事务 | [SessionLog::append_with](../../crates/server/src/session_log.rs) | 相同序号与payload重放；不确定写入核对原字节并完成确认后推进投影；领域存储与SQLite准入仍有独立提交边界 |
| 运行份额与模型调用许可独立，默认模型槽位为2 | [服务许可](../../crates/server/src/service_limits.rs)的ServiceLimits::default、acquire_model与LimitedAdapter::call | 回读默认值、用户轮转、等待取消和许可释放；正文K=8、每分钟6任务、每调用12秒均为教学假设，未测P95或真实容量 |
| 多实例共同写入需要改变现有权威，而不是只改连接地址 | [ServiceWriter::acquire](../../crates/server/src/control_store.rs)；[第19章](chapters/19-多用户阅读与调度.md) | 当前服务根操作系统锁与进程内私人权威；按用户分区、事务存储、租约和观察路由是条件变化分析，未声称实施 |
| 十倍材料与模型替换要分别分析真实成本 | [Book::new](../../crates/read-tools/src/lib.rs)、[ProviderMode](../../crates/runtime/src/lib.rs)、[ADR-0146](../adr/0146-portable-build-workspaces-and-incremental-book-updates.md) | 当前UTF-16正文及索引、Native/ReAct协议路径；U1—U2已实现而U3—U11待实施，不能把搬迁续建外推增量迁移 |
| 评分引文、模型读到的正文和读者收到的来源视图有不同用途 | [resolveV2Basis](../../evals/semantic/task-quality-v2.mjs)、[第23章](chapters/23-真实失败与工程改进.md)的MSE复核 | 当前评分在交付视图定位连续引文；历史复核沿原报告条件，不为模型补算未读取的文字 |
| 历史成功率与撤回结论保留各自分母 | [LA7—LA10](../LA7-LA10实施与验收.md)、[EV7](../performance/agent-eval-ev7-20260926.md)、[当前完成指导](../../crates/runtime/src/agent_prompt.rs) | LA7证据锚组100%与完整成功15/24；LA9-v2的Text 10/24、Tree 11/24、Graph 9/24；EV7六对十二份首次回答、四对迁移只一对明确改善，候选撤回、finish恢复v7 |
| 确切演示版本可以验收，不能代替长期学习效果 | [EX13.6](../performance/presentation-context-ex13/ex13-6/README.md)、[第17章](chapters/17-可探索解释与演示版本.md)与[第23章](chapters/23-真实失败与工程改进.md) | 原聊天续作、同对象revision 2及历史浏览器操作属于原记录；本轮未运行模型或浏览器 |
| 缺失用量保持未知，当前记录器缺口仍在 | [provider-recorder.mjs](../../evals/semantic/provider-recorder.mjs)的requestPurpose、startProviderRecorder与measuredUsage；[第23章验证](SOURCES.md#第-23-章续写验证记录) | 当前仍只识别旧来源修复签名，用整包JSON提取usage；复用前章两项问题重放结果，本轮未重跑或修订实现 |

### 第 24 章的已知边界

- 本章回读支撑新的口述与条件分析，不表示前二十三章都已按当前工作区重审。回查表是阅读路线，各章既有验证保留原日期。
- 请求场景与容量公式为教学分析；8槽位、6任务/分钟、12秒/调用以及5次或10次调用均为假设。默认值为2，算例排除更紧的活动Run、用户份额与上游约束；没有新增压测、费用或成功率数据。
- 当前来源与Goal语义粒度、跨存储提交、单写者与增量更新范围，以及SSE用量缺口集中保留；协议适配不证明不同模型任务质量相等。
- 三分钟口述未做真人计时。全书24章正文完成，统一基线校订和独立附录尚未开展；本轮依据用户最新取舍将24.5调整为结果依据，贡献表达暂缓。

## 附录与全书导航校订的具体依据

| 附录或修订 | 本轮对照的依据 | 使用范围 |
| --- | --- | --- |
| A术语与状态归属 | [CONTEXT](../../CONTEXT.md)、[RunScope](../../crates/server/src/run_scope.rs)、[会话事件](../../crates/server/src/session_event.rs)、[输入片](../../packages/core/src/model-input-slice.ts)、[执行身份](../../packages/core/src/automatic-build-task-store.ts) | 结合原章节区分材料、位置、用户、现场、Run、候选和执行代次；实际行为用例沿用原章记录 |
| B源码阅读路线 | [Book加载](../../crates/read-tools/src/lib.rs)、[编排](../../packages/core/src/build-orchestrator.ts)、[执行器会话](../../packages/core/src/automatic-build-executor-session.ts)、[成果发布](../../packages/core/src/automatic-build-publication.ts)、[来源交付](../../crates/runtime/src/orchestrator.rs)与各章选读 | 路线按职责排列，明确不把文件相邻顺序当作直接调用边；入口及符号核对不替代全链路验收 |
| C身份、单位与输入 | [基座类型](../../crates/base-schema/src/lib.rs)、[PublishedBookRef](../../crates/server/src/published_library.rs)、[Book合同](../../crates/book-tool-contracts/src/lib.rs)、[来源与回答](../../crates/runtime/src/orchestrator.rs)、[AuthorRequest](../../crates/runtime/src/presentation_author.rs) | 两段连续实际摘录、四个教学JSON输入；Text按LID范围、Span按UTF-16半开区间，SourcePresentArgs只接quote |
| C坐标参照系与默认配置 | [read-tools](../../crates/read-tools/src/lib.rs)的match_source_quote、SourceSelectedRange与SourceExcerpt；[ServiceLimits](../../crates/server/src/service_limits.rs)、[OuterConfig](../../crates/runtime/src/orchestrator.rs)、[ActiveContextBudget](../../crates/runtime/src/auto_compaction.rs) | 全局原文、LID内部和返回excerpt内部范围分开；A中B及120起点为教学算例。默认配置是实现值，不是实测容量 |
| D决策与演进 | ADR-0021、0086、0091、0100、0101、0125、0127、0137、0146、0149、0150、0152、0155；具体链接见[附录D](appendices/D-决策与演进索引.md) | 回读原状态，保留早期前端待定、旧轮数和EV7待实施的时间；后续实现、阶段门槛和撤回分别定位 |
| E运行与验证入口 | [根脚本](../../package.json)、[Core脚本](../../packages/core/package.json)、[Web脚本](../../packages/web/package.json)、[演示准备](../../examples/quickstart/create.ts)、[agent-run](../../evals/semantic/agent-run.mjs)、[quality-run](../../evals/semantic/quality-run.mjs)及第16—24章验证记录 | 核对命令目标和协议选择，没有运行这些命令；历史报告索引保留题集、模型、程序与预算边界 |
| F面试回查 | [第1—24章](README.md#阅读入口)各章论证、章末问答与第24章请求链 | 每章一组典型问题、回答主线与条件变化，按AI Agent、后端和构建方向连接 |
| 全书导航与编辑 | README、OUTLINE、导读与第24章附录链接；第3章原身份表 | 六份附录接入阅读入口。第3章原句称“三个”但表有四行，改为“四类相关但用途不同的标识或位置”；其余正文仅按实际检查发现处理 |

### 附录与全书导航校订的边界

本批完成24章与六份附录之间的导航、引用和重点术语校订。章节事实继续保留各自读取日期，不宣称本次逐条重审或重跑了全书实现与实测。个人贡献按用户取舍暂缓；排版导出尚未进行。既有来源、Goal、跨存储恢复、记录器用量和历史夹具问题保留原章记录，没有借附录修改产品。

## 第 1—3 章续写验证记录

2026-10-06 检查本批涉及的 8 份文档：233 处本地链接可达，44 个引用符号存在，代码围栏闭合。新增的 5 处摘录分别来自 App.vue、Book::load、PublishedBookRef、Book::text 和 ReaderWorkspace::invalidate，均与当前文件中的文字逐字一致。

第 2 章实验表所选列与原报告一致；教学成本交点为 q=20，三行计算结果正确。第 3 章 A中B 的 UTF-16 与 UTF-8 范围例子已计算核对。职责与边界的判断依据为本索引列出的源码、测试正文和 ADR；符号存在检查只用于发现误引。本批未修改产品代码，未执行产品测试、构建评测或性能测量。

## 第 4—5 章续写验证记录

2026-10-06 检查本批涉及的 8 份文档：292 处本地链接可达，38 个引用符号存在，代码围栏闭合。4 处新增 TypeScript 摘录分别来自 segment、buildPass1Input、mergeAndGate 和 acceptedEdgeDropReason，与当前文件逐字一致。

通过现有 Node/tsx 运行确定性教学链路，输出再与书稿对照：61 个 UTF-16 单位的源串、9 行节点范围与回切文字、2 个窗口、Pass1 标注输入、4→3 节点归并、0.5 锚定率、3 条目录项和 1 个长程候选一致。模型抽取结果在本例中为手工教学设定，没有进行模型调用。根级长段对照另行执行，结果见“第 4—5 章发现的已知问题”。

已阅读 segment、pass1-input、merge、pass2-build、pass2-orchestrate、pdf-source-map 及可选 Pass2 相关测试中的对应用例；未重新运行这些测试套件。源码与 ADR 对照负责行为解释，引用检查仅用于发现断链或误引。本批仅修改书稿。

## 第 6 章续写验证记录

2026-10-07 检查本批涉及的 7 份文档：313 处本地链接可达，34 个引用符号存在，代码围栏闭合。2 处 TypeScript 摘录来自 structureCandidateRef 与 StructureChapterSelection，均与当前源码逐字一致。BSR7 实验表 7 行分别从原报告对应列核对，正文成稿章号为 1—6 与 9，共 7 章。

确定性教学重放结果及证据状态见本章依据表之后的记录。BSR0、BSR2、BSR7 的任务条件、旧成果复用、计量和真实来源问题已对照原报告；本批没有重跑这些实验。链接与符号检查只发现断链和误引，行为说明依据实际阅读的实现与相关测试正文。本批仅修改项目书稿。

## 第 7 章续写验证记录

2026-10-07 检查本批涉及的 7 份文档：353 处本地链接可达，33 个引用符号存在，代码围栏闭合。2 处 TypeScript 摘录分别来自 evaluateModelExecutionBudget 与 inspectAutomaticBuildTaskActivity，均与当前源码逐字一致。正文成稿章号为 1—7 与 9，共 8 章。

本章切片与候选容量的 6 行表格对照本次确定性函数输出；派发限制与 Harness 配置的 9 行对照当前实现中的常量与绑定。实际重放的设置、范围、容量与失败原因见本章依据表后的记录。BSR7 历史容量案例及 DSH 验收范围对照原报告，没有重新运行原实验。现有产品测试为源码阅读，未执行测试套件；本批仅修改项目书稿。

## 第 8 章续写验证记录

2026-10-07 检查本批涉及的 7 份文档：384 处本地链接可达，31 个引用符号可定位，代码围栏闭合。2 处 TypeScript 摘录分别来自 sameBuildContent 与 closeAutomaticBuildStage，均与当前源码逐字一致。正文成稿章号为连续的 1—9，共 9 章。

成果匹配、两文件发布、阶段收口共 13 行表格对照本次实际函数输出；Pass1 收口后 extract/profile_sidecar 的动作与实际重算一致。U1—U2 的 29 章夹具、82,514 字节重发数据核对原报告。夹具设置及执行结果见本章依据表后的记录。源码和用例阅读负责行为解释，链接与符号检查用于发现断链和误引。本批仅修改书稿，没有重新运行完整产品测试或历史实验。

## 第 10 章续写验证记录

2026-10-07 对本轮 5 份变更书稿进行定向检查：第 10 章全部本地链接，以及 README、SOURCES、检查点和第 9 章新增链接目标，共 39 项可达，涉及的 Markdown 锚点有效；未重新统计未变动章节的全部旧链接。75 个符号能在记录的源码文件定位，其中包含第 11 章检查点入口；符号存在不代替行为核实。

6 处 Rust 摘录分别来自 TextInput、ToolRegistry::try_new、stamp_task_need、循环的 Handler 可见性过滤、ToolCall 与 Native 名称还原，均与对应当前源码逐字一致。代码围栏闭合；4 个 JSON 输入例子可解析，字段与本章使用的调用形状对应；5 行 Book 入口别名与 CONTRACTS 对应。第 11 章链接指向既有目录安排，没有创建未成稿正文。

18 行教学结果与前述实际局部重放对照一致；9 组授权判断、182/423/724 bytes、三个 Provider 名称以及正向/反向/缺节点的文本结果分别核对。预算尺寸与 600-byte 限制、最小 Book 数据和碰撞名称是教学设置。没有把局部驱动写成完整 Runtime、Server 或模型实测。

编辑核对确认本章按既有第 10 章范围回答能力声明、可见性、发现、授权和执行入口问题，并承接第 9 章请求；上下文压缩细节留给第 11 章，流式来源交付留给第 12 章。完成状态为导读及第 1—10 章，检查点压缩为 49 行。本轮没有修改产品代码、运行产品测试套件、调用真实模型或重跑历史实验。

## 第 11 章续写验证记录

2026-10-07 完成上述 20 组离线局部重放，全部断言通过。5 组片段、2 组正文预算、4 组活动预算与字符估计、8 组压缩、1 组进展判断的结果分别进入正文。结构合法但语义错误的草稿被接纳，是边界用例的预期观察，不应写成语义校验通过。

本章教学预算、20 KiB 正文、压缩锚点和脚本草稿都有明确假设。历史缓存数据来自既有报告，复算输入总量、合计命中率及三个请求的未命中占比；双请求 54% 的合计命中率是教学算例。本轮不把这些算术核对记作真实模型或生产缓存实验。

书稿检查覆盖第 11 章全部本地链接及其他四份变更书稿的新增链接目标，共 56 项可达，涉及的 Markdown 锚点有效；90 个符号在对应源码文件中可定位，其中包含第 12 章续写入口。符号存在检查用于发现错引，行为判断仍来自前述源码阅读与重放。

7 处 Rust 摘录与当前实现逐字一致；代码围栏闭合，1 个 JSON 输入可解析，2 幅 Mermaid 图未做渲染验收。三行预算、正文 8／28／8 KiB、缓存合计 72.4573%、三个请求未命中占比 70.33% 及教学合计 54% 的算术核对通过；20 组重放结果与正文分组一致。三个完整复制的实现模块在书稿检查时仍与重放版本相同，临时驱动目录已清理。

编辑核对确认本章沿“工具返回后怎样继续采样”展开，覆盖原目录 11.1—11.5；来源与流式交付留第 12 章，跨 Run 目标留第 13 章，日志恢复留第 16 章。完成状态为导读及第 1—11 章，检查点为 46 行。仅更新本章、README、SOURCES、检查点和第 10 章导航；相关产品文件无未提交修改，本轮未运行产品测试套件或提交 Git。

## 第 12 章续写验证记录

2026-10-07 完成上述 23 组离线重放，全部断言通过。Provider 用量用 7→9 的累计帧观察截断前仍有最后快照；本章不将两帧相加为 16。来源编译接纳错误语义是边界观察，不记为回答正确。512-byte 缓冲、30→31 的序号和分片来源表对照实际驱动输出；时刻 1.0／1.8／2.3／8.0／8.2 秒为教学设定。

书稿检查覆盖第 12 章全部本地链接，以及 README、SOURCES、检查点和第 11 章新增链接目标，共 50 项可达，相关 Markdown 锚点有效；106 个关键符号在对应文件可定位，含第 13 章续写入口。7 处 Rust 摘录与当前源码逐字一致；3 个 JSON 例子可解析，输入引文、绑定预览、输出 parts 与 sources 的关联一致，代码围栏闭合。0.8／2.3／5.9 秒、23 组分类和 30→31 序号的口径核对通过。两幅 Mermaid 图按源码编辑核对，本轮没有做渲染验收。

重放使用的三段模块代码与来源属性/编译函数在最终核对时仍与当前源码一致，直接比较文本，未用 Git 提交相同来替代回读。临时驱动目录已清理；续写检查点为 45 行。

本章覆盖原目录 12.1—12.5；第 13 章继续 Goal 与 Tutor，第 16 章展开日志，第 19 章展开多人调度。完成状态为导读及第 1—12 章。仅更新本章、README、SOURCES、检查点和第 11 章导航；没有修改产品实现、提交 Git、运行整套产品测试或调用真实模型。

## 第 13 章续写验证记录

2026-10-07 完成 19 组离线 Rust 局部重放，全部通过。驱动使用当前 goal.rs，以及 memory 的 learning.rs、teaching.rs、assessment.rs、learning_evidence.rs 的实现与既有测试；learning.rs 仅移除 ts-rs 导出标注以避免生成产品文件，ToolError 与私人目录操作使用最小配套类型，所有 LearningStore 以 private=false 打开。共执行既有用例 17 个：Goal 8、生命周期 5、教学事实 1、评估规则 2、学习证据投影 1；另提取当前 goal_turn_delivered 检查实际归属和不完整终态，并构造合法片段下的错误语义判定，各 1 组。

生命周期、Trace、Assessment 与投影使用真实临时 SQLite；写失败用触发器模拟，包含回滚、幂等重试、派生投影重建与重开数据库。本轮没有模拟真实断电、验收私人权限、运行完整产品测试、教学构建、浏览器或模型。评估接纳器实际返回错误语义的 Correct 是边界观察，不记为正确教学结果。

正文的工作计划 revision 1→2→2→3→4→5、集合答案的六行结果，以及独立/有帮助/改答的 (1,1,1) 与纠正后的 (0,1,1) 对照本轮实际用例。Server 对封闭题解析错误保留 unassessed 的连接由 behavior 与已有集成测试源码核对，未在本轮调用 HTTP。

书稿收口检查通过：本章全部本地链接及配套文档新增链接共 61 处可达，相关 Markdown 锚点有效；129 个关键符号可定位，7 处 Rust 摘录逐字匹配当前源码，3 个 JSON 示例可解析并核对活动、来源与提交身份，代码围栏闭合。对 15 份依赖产品文件直接比较文本，写作期间内容未变；5 份重放模块和提取的完成函数仍匹配当前源码。工作计划 revision、评分表与证据计数已核对。一幅 Mermaid 时序图未做渲染验收，临时驱动已清理，检查点为 45 行。本章覆盖既定 13.1—13.5；第 14 章转入阅读现场，第 15 章深入私人学习事实，第 16 章讲日志，第 17 章讲演示。完成状态为导读及第 1—13 章，第三篇已经成稿。本批仅修改本章、README、SOURCES、检查点和第 12 章导航。

## 第 14 章续写验证记录

2026-10-07 实际执行 45 个局部用例，全部通过。六份既有前端测试为 reader-navigation 4、markdown-source-map 12、pdf-selection-capabilities 4、pdf-selection-draft 6、useReadingContinuity 4、workspace-layout 10，共 40 个；直接运行当前源码与既有测试，未修改测试实现。另在书稿临时目录增加 reader-text-anchor 的四个受控 DOM 几何用例，并提取 App 的 onScrollEdge、mergeSegments 重放迟到预加载，共五个观察。

文字映射使用当前 Markdown/KaTeX 渲染和源串对齐函数；模拟 DOM 的几何由驱动提供。A中😀B 的 [2,4)、20/140/180 像素输入与 +40 补偿、源串变化拒绝和原生选择保持均被实际断言。迟到数据被合并是已知问题的预期观察，不记为正确隔离。既有导航与 PDF 草稿文件包含连接关系的静态文本断言，不能把这些断言算成完整 App 事件链执行。

Reader 的 Rust 窗口用例、Server 的 pdf_selection_recovery_classifier_accepts_exact_and_known_representation_gaps、pdf_selection_recovery_classifier_rejects_material_or_ambiguous_gaps、PDF 选区路由和 agent_chat_selection_ranges_rebuild_canonical_quote、agent_chat_selection_ranges_reject_empty_out_of_order_and_overlap、agent_chat_rejects_forged_canonical_selection_quote 已阅读，未重新执行。本轮不调用模型，不渲染真实 PDF，不重跑历史 PHR、RE 平台实验；两处模拟 DOM 的 KaTeX quirks-mode 提示没有影响测试通过，也不作为视觉验收依据。

书稿收口检查通过：本章全部本地链接及其他四份变更书稿新增链接共 76 处可达，涉及的 Markdown 锚点有效；109 个关键符号在对应文件中可定位，含下一章入口；7 处代码摘录逐字匹配当前源码，1 个 PDF 输入 JSON 可解析，代码围栏闭合。四行窗口算例、Markdown 两组范围、UTF-16 表情字符、+40 像素补偿和 732 像素对照边界均核对一致。符号存在检查只发现错引，行为判断依据前述源码和实际用例。

对 25 份依赖文件直接比较原文，写作期间内容未变；提取的两段 App 函数仍匹配当前实现。只改动五份书稿，没有修改产品或提交 Git。临时测试驱动、结果和缓存已清理；一幅 Mermaid 图未作渲染验收，检查点为 45 行。本章保持既定 14.1—14.5，承接第 13 章并把长期私人事实留给第 15 章。完成状态为导读和第 1—14 章，剩余第 15—24 章。

## 第 15 章续写验证记录

2026-10-07 实际执行 32 个离线局部用例，均完成预期断言。28 个既有用例来自当前 memory：lib 10、profile 3、operation 4、governance 4、projection 2、global_consolidation 3、review 1、learning_evidence 1。覆盖严格 Note 新建、层级提升、位置与内容替换、写失败保留、延后阅读合并、来源信任、作用域和有效期、明确记住、纠正与整链遗忘、治理重试和收集规则、独立快照预算、跨书提升、复核提交和学习投影重建。

四个书稿驱动用例分别验证：笔记创建/重复/提升/修改的 `(1,1)→(1,1)→(2,2)→(3,3)` 修订序列；书 A 采用 provisional 书内偏好、书 B 采用 confirmed 全局偏好而 pending 不参与，以及默认预算合计 2048；选区 Note 结构接纳而未校验引文；有效期已过的事实仍出现在 Markdown。最后两项是边界观察，不代表相关功能正确。学习证据既有用例再次得到支持计数 `(1,1,1)→(0,1,1)` 和 uncertain=1，并验证投影写失败时保留旧值、删除投影后重建及重新打开数据库。

驱动复制当前 memory 的源模块与现有夹具到书稿临时目录。learning.rs 只移除 ts-rs 导入、派生和导出标注，避免测试生成产品类型文件；read-tools 仅提供同字段 ToolError，私人存储辅助实现仍取当前模块，测试以 private=false 打开。使用离线依赖与真实临时 JSON/SQLite，测试临时路径也限制在书稿临时目录。作用域与预算算例作为一个单独用例执行，其余 31 个用例成组执行；没有改动产品实现。

Runtime 的 scan_memory_intent/evaluate_memory_intent、ReviewExecutor、MemoryPolicyRegistry、ProfileContextCache 与 Server 的笔记、快照、用户和教学连接为源码阅读；界面撤销也只阅读了当前实现。没有运行完整产品测试、真实模型、HTTP、浏览器、权限验收或真实断电试验。后台复核原子提交用无效写路径模拟失败，学习投影用 SQLite 触发器模拟失败，各自只支持对应提交边界。

本章保持既定 15.1—15.5，承接第 14 章的保存笔记动作，将会话追加与恢复留给第 16 章。完成状态为导读及第 1—15 章，余第 16—24 章九章；本批只修改本章、README、SOURCES、检查点和第 14 章导航。书稿收口检查通过：本章全部本地链接及配套文档新增链接共 93 处可达，涉及的 Markdown 锚点有效；143 个关键符号可在指定文件定位，含下一章入口；6 处 Rust 摘录逐字匹配，1 个 JSON 输入可解析并核对来源字段，代码围栏闭合。检查时将 ADR 的 MemoryIntentGate 设计名与当前 scan_memory_intent/evaluate_memory_intent 函数分开表述，未把概念名冒充实现类型。直接比较 36 份依赖原文，写作期间内容未变；17 份重放模块与当前源码保持对应，只有上述导出标注处理和测试入口追加。笔记、作用域、2048 预算与学习计数均已核对。一幅 Mermaid 图未做渲染验收；临时驱动、结果与缓存已清理，检查点为 46 行。

## 第 16 章续写验证记录

2026-10-07 直接执行当前 Server 库的五组定向既有用例，共 26 个通过，0 失败、0 忽略，另 536 个未选中。分组为 session_event::tests 3、session_log::tests 8、session_store::tests 5、tests::session_runtime_tests 6、tests::session_recap_tests 4。没有复制源模块、替换配套类型或修改产品测试；使用项目现有构建缓存及离线锁定依赖。

执行入口为 `cargo test -p server --lib --offline --locked -- session_log::tests:: session_event::tests:: session_store::tests:: tests::session_runtime_tests:: tests::session_recap_tests:: --test-threads=1`。TEMP/TMP 及 TypeScript 导出目录指向书稿临时目录，所选测试不执行类型导出用例。编译阶段出现既有 ts-rs 对 serde 属性的解析提示，不影响这些测试结果。

日志用例实际验证只读打开忽略未完成尾行、写入口随后修复，完整坏行拒绝，部分或完整写入后报错不推进投影，原终态重试只接纳一次，旧前缀不变与制作正文清理。事件用例验证失败终态及后缀同条提交、压缩检查点适用性、教学/呈现关联与既有视图一致；存储用例覆盖旧快照不读取、文件名身份还原、选择失败、删除后选择修复及两用户重开。

运行连接使用固定 Hello ModelAdapter 和脚本化压缩草稿，验证本地运行重开、私人 Provider continuation 不进入公开历史、切书后仍写原聊天、准备前失败保留已接纳问题、清理与检查点一致、中断补回执幂等及来源跨压缩重开。回顾用例覆盖原截点、处置开始与回执、保留后撤销、成果后来删除、Goal 明确状态、未确认完整尾行不被读取接纳。只读用例比较日志字节、记忆、学习状态和 Reader 状态，重开后在统一 generated_at 的条件下输出一致。

用例包含内部路由调用，不启动真实 HTTP 服务。JSONL、记忆文件和学习数据库使用临时路径；故障是代码注入与不可写目标模拟，未验证真实断电。RunAdmissions 网络恢复与 Web 展示按源码阅读，没有运行网络准入集成、浏览器或整套产品测试，没有调用真实 Provider。历史 release 的 19,280 字节、7 个聊天/选择/演示文件备份恢复等属于原发布记录，本轮未重跑。

书稿收口检查通过：本章全部本地链接及配套文档新增链接共 83 处可达，相关 Markdown 锚点有效；123 个关键符号在指定文件可定位，含下一章入口；7 段 Rust 摘录与当前源码逐字一致，1 个 JSON 示例可解析并核对事件字段，代码围栏闭合。三行累计写入表、1,000 次提交的字节数、两种消息后缀算例和原始回执中的 39,631+19,280=58,911 字节均核对通过。符号存在检查用于发现错引，行为解释由源码阅读和上述实际用例支持。

直接比较 32 份已保存的依赖原文，写作期间内容未变，没有用相同 Git 提交替代比较。相对于本批开始时的书稿，仅五份文件发生预期变更，没有删除其他章节。一幅 Mermaid 图按调用关系编写，未做渲染验收；检查点为 46 行。临时验证目录 `.chapter16-replay-20261007` 保留了原文基线、定向测试日志、引用/算例检查驱动和临时数据。最终清理被工具策略拦截，返回 blocked by policy，未给出更具体原因；未改用其他方式绕过。

本章保持既定 16.1—16.5；导读与第 1—16 章成稿，余第 17—24 章八章。本批只修改本章、README、SOURCES、书稿检查点和第 15 章导航；下一章转入可探索解释与演示版本。

## 第 17 章续写验证记录

2026-10-07 直接执行当前 Server 与 Runtime 库的定向既有用例，共 25 个通过、0 失败。Server 16 个通过、1 个浏览器用例按原条件忽略、545 个未选中；Runtime 9 个通过、0 忽略、452 个未选中。复用项目构建缓存，使用离线锁定依赖，没有修改产品和测试实现。

Server 选择 editing 模块、presentation_store_tests::ex13 模块、preview_contract_tests、presentation_libraries::tests，以及精确版本重开、指定现场重开、冻结参数继承、混淆回执拒绝、现场保存失败与来源列表的六个具体用例。前四组分别执行 4、3、2、1 个非忽略用例，后六个各执行一次；结果见书稿临时目录的 server-tests.log。

Runtime 选择 presentation_preview::tests 的三个合同用例，另精确选择 ex13_guidance_changes_append_after_results_and_keep_provider_prefix、ex13_saved_source_projection_keeps_first_observation_and_raw_history、ex12_stage_round_trip_replaces_design_and_preserves_candidate_inspection、ex12_simple_local_can_deliver_without_framework_and_plain_answer_stays_plain、ex12_prepare_and_write_in_either_batch_order_wait_for_next_sampling、ex12_framework_does_not_grant_sources_or_delivery。结果见 runtime-tests.log。运行形式均为 cargo test -p 对应包 --lib --offline --locked -- 所选过滤项 --test-threads=1，TEMP/TMP 和 TS_RS_EXPORT_DIR 指向书稿临时目录，所选测试没有调用类型导出用例。编译阶段的既有 ts-rs serde 属性提示未影响用例通过。

Server 使用实际临时 JSON/聊天存储，验证旧内容及候选保持、同候选保存幂等、基于旧版本产生新 revision、失败补丁不写半成品、原 Run 候选跨界拒绝、follow-up 必须指定确切基底或明确新建。冻结参数用例在接纳追问后保存更新现场，仍得到 count=1；unit/mode/shape/new 保留新默认值。部分存储测试通过直接植入 legacy 预览回执继续检查版本，不能将其记为真实浏览器通过。EX10 的 test-only assemble_write 钩子在这些未设置实验装配状态的用例中原样返回输入。

Runtime 使用固定 RequestPlanRecordingAdapter 与脚本化 AuthoringPort；预览图片为测试值。用例实际执行制作循环的 prepare、候选身份、后续图片观察、设计清理、Native/ReAct 请求投影和原始消息保持。保存源码测试读取原冻结的两次 HTML，来源列表在受控端口中置空；真实来源与磁盘语义由 Server 定向用例另行覆盖。没有使用这些夹具宣称浏览器、视觉、数学或 Provider 行为已经验收。

BrowserPreview、Matplotlib、Manim、Web bridge/工作区/媒体生命周期和网络沙箱为源码阅读。一个真实浏览器编辑用例按既有 ignore 跳过，本轮没有启动浏览器、绘图/动画进程、HTTP 服务或真实模型，没有重跑 EX13 历史实验。

书稿收口检查通过：本章全部本地链接与其他四份书稿新增链接共 98 处可达，其中 5 处 Markdown 锚点有效；161 个引用符号在指定文件中可定位，包含下一章入口。7 段摘录（Rust 5、JavaScript 1、TypeScript 1）除展示缩进外逐字对应当前源码，1 个 JSON 预览示例可解析且字段与合同一致，代码围栏闭合。检查修正了索引中的 read_state/latest_state 简称和下一章误写的 LocalRuntimeStatePort，分别改为 read_presentation_state/latest_presentation_state 与 RuntimeStatePort；符号检查用于发现错引，行为结论仍依据源码与实际用例。

学习率的四行表使用精确分数独立计算，η=0.2/0.8 损失相等，η=1.1 的 4.4、−0.88、5.76、8.2944 均一致。EX13.2 四行字节表逐项对应原报告并重算差值及 59.01%/95.07%；EX13.1 两个体积与 EX13.6 的调用、截图、来源、时间、token 和费用数值对应原记录，1,832,847+88,009=1,920,856。没有把历史数据重新标记为本轮实测。

直接比较 44 份已保存的依赖原文，写作期间内容未变，没有以相同 Git 提交代替比较；相对于本轮开始的 21 份书稿，只有预期的五份 Markdown 发生变更。验证原文基线、verify.py、verification.json、两个测试日志和临时数据保留在书稿内 .chapter17-replay-20261007，便于复查实际口径；未触碰上一轮保留目录。一幅 Mermaid 图按调用关系编写，未作渲染验收；检查点为 46 行。

本章保持既定 17.1—17.5；承接第 16 章的已交付演示，沿解释选择、候选观察、资源、现场、修订和制作上下文展开，第四篇完成。导读与第 1—17 章成稿，余第 18—24 章七章；只修改本章、README、SOURCES、检查点和第 16 章导航。


## 第 18 章续写验证记录

2026-10-07 直接运行当前产品四个包的定向既有用例，共25个通过，0失败、0忽略：Server 21个（541个未选中）、Runtime 1个（460个未选中）、Memory 2个（123个未选中）、Reader 1个（53个未选中）。复用现有构建缓存，离线锁定依赖；未复制、删改产品模块或测试。四份日志和Server过滤项保存在书稿内 [.chapter18-replay-20261007](.chapter18-replay-20261007/)。

命令形式为 `cargo test -p 包名 --lib --offline --locked -- 过滤项 --test-threads=1`，TEMP/TMP/TS_RS_EXPORT_DIR指向本章验证目录。Server过滤项逐条保存在 [server-selected.txt](.chapter18-replay-20261007/server-selected.txt)，输出见 [server-tests.log](.chapter18-replay-20261007/server-tests.log)；编译中的既有ts-rs属性解析提示未影响测试通过，所选用例不执行类型导出。

Server包含12个host::agent_run_tests用例，覆盖模型等待期间的Reader/Memory/历史访问，旧现场原书归属，已保存笔记后取消迟到工具，画像判断取消，new/select/delete/book边界，shutdown保存，预提交失败重试，内层综合父子活动，五SSE观察者与重连，取消/保存失败两种终态，以及拒绝来源无执行时间。这里实际启动本机HTTP Host与假Provider，Provider响应由测试通道控制；没有真实网络Provider。`mu1c_old_run_cannot_touch_replaced_workspace_but_saves_original_note_and_answer`从内部切换现场，不代替公开切换入口验收。

另外8个host::tests用例覆盖后台owner保持、已读worker冲刷、忽略静默期强制冲刷、有序退出重开、复核串行与热配置、60秒空闲/8轮阈值、边界超时后的stale状态与继续完成。`tests::mu1c_chat_switch_back_keeps_old_run_stale_and_input_frozen`另验证代次、冻结输入和用户更换。复核使用受控executor、假时钟和受控超时等待器；不是计时性能测试。

Runtime执行 `run_context::tests::cancelled_native_and_react_transport_do_not_retry_after_connection_loss`，一个用例内部覆盖Native与ReAct两种协议。实际本机TCP收到请求后取消并断开，确认没有重试连接。Memory执行 `deferred_read_ledger_coalesces_touches_and_flushes_once` 与 `deferred_read_ledger_retains_batch_after_flush_failure`，使用实际临时文件和路径故障；Reader执行 `goto_scroll_enqueue_read_ledger_until_flush`，确认可见真叶登记与冲刷连接。结果分别见 [runtime-tests.log](.chapter18-replay-20261007/runtime-tests.log)、[memory-tests.log](.chapter18-replay-20261007/memory-tests.log)、[reader-tests.log](.chapter18-replay-20261007/reader-tests.log)。

BrowserPreview、Matplotlib、Manim、网络presentation沙箱、NetworkRunPort和LimitedAdapter的本章连接为源码阅读；Linux沙箱取消/恢复忽略用例只读相应入口，没有选中执行。没有运行真实浏览器、模型、Python渲染、多人准入调度、整套产品测试或历史PHR9/AS实验。代码中的检查间隔与超时按定义引用，不作为本轮取消延迟实测。

书稿收口检查通过：本章全部本地链接及其他四份书稿新增链接共84处可达，5处Markdown锚点有效；220个引用符号可在指定源码定位，含下一章入口。8段Rust摘录除展示缩进外逐字对应当前源码，代码围栏闭合。符号存在仅用于发现错引，行为判断依赖上述阅读和实际用例。一幅Mermaid时序图按当前调用顺序编辑核对，未做渲染验收。

八行教学阶段表独立求和为9,115毫秒、80毫秒锁占用、55毫秒锁内磁盘提交，三者的包含关系已核对；模型时间减半后的6,115毫秒和约1.49倍加速、读者8/48毫秒例子算术成立。教学数字不作为性能观测；测试日志25个通过与包内分组、过滤数逐一对应。

直接比较本轮留存的27份依赖原文，内容未变，没有用相同Git提交代替比较。对照本轮书稿原文，仅README、SOURCES、检查点、第17章导航发生预期修改，新增第18章；既有其余章节和OUTLINE未改。检查点为42行。验证目录保留原文、四份测试日志、临时数据、verify.py和verification.json；第16/17章验证目录保留，未清理。

本章保持既定18.1—18.5，正文从制作期间的读者操作推导生命周期、固定输入、权威写入、取消、观察、后台和关键路径。导读与第1—18章成稿，余第19—24章六章。第19章待回读完整用户/现场/发布、SQLite准入、公平调度与配额链路；本轮仅定位下一步，不宣称第19章事实已经核实。

## 第 19 章续写验证记录

2026-10-07 在当前工作区直接执行29个不同的Server定向既有用例。第一组26个：23通过、3失败、0忽略、536未选中，测试进程耗时66.01秒；补充组3个：3通过、0失败、0忽略、559未选中，耗时6.75秒。合计26通过、3失败，未重跑已通过用例，未运行整套产品测试。两次套件墙钟时间只描述验证运行，不是用户任务性能。

命令形式为 `cargo test -p server --lib --offline --locked -- 过滤项 --test-threads=1`。TEMP/TMP/TS_RS_EXPORT_DIR均指向书稿内 [.chapter19-replay-20261007](.chapter19-replay-20261007/)，复用现有构建缓存，未复制或修改产品模块。首组过滤项见 [server-selected.txt](.chapter19-replay-20261007/server-selected.txt)，原始结果见 [server-tests.log](.chapter19-replay-20261007/server-tests.log)；补充过滤项见 [supplement-selected.txt](.chapter19-replay-20261007/supplement-selected.txt)，结果见 [supplement-tests.log](.chapter19-replay-20261007/supplement-tests.log)。既有ts-rs属性解析提示未阻止编译。

通过范围包括1个MU3发布固定/Book预算用例、7个MU5现场/私人权威/教学/回收用例、MU8同代次旧revision读新现场用例、14个MU6准入/恢复/取消/保存/调度/观察用例（含补充的观察许可），1个service_limits资源释放用例，以及补充的2个MU8私人笔记/画像/来源用例。实际经过临时文件、SQLite3.53.2、JSONL、受控假Provider与本机HTTP。六个故障位置通过测试故障点返回错误后重新打开存储，未做进程强杀或断电。

三个失败均来自MU4旧夹具：`mu4_private_objects_are_owner_scoped_at_real_host`、`mu4_resources_authorize_before_head_range_conditionals_cache_and_revoke`、`mu4_open_sse_closes_on_logout_disable_and_material_revocation`。Fixture::seed_private在第76行调用save_agent_history_path，因JSONL目录已存在而得到SESSION_EVENT_WRITE_REQUIRED。失败发生在授权断言之前，不能报告“越权测试通过”或“发现线上越权”。保留原日志，不修改产品夹具。补充组实际验证当前会话存储下的私人归属和观察资格失效，完整已打开SSE撤权传输、HEAD/Range前置授权本轮为源码核对。

测试内四秒、十五秒等等待上界只用于协调或发现挂住；现场淘汰由未来Instant驱动。正文四秒单请求、两种16秒排程、双许可8秒与累计8秒资源等待是教学推导。测试的两次120001用量为夹具申报，不是真实模型消耗。没有运行真实模型、浏览器、Python制作、Linux沙箱、真实HTTPS代理、迁移备份或混合负载容量实验。

书稿收口检查通过：本章全部本地链接及配套新增链接共81处可达，其中5处Markdown锚点有效；221个实现符号与29个实际选中测试符号可定位，8段Rust摘录逐行匹配当前源码（忽略行首缩进及行尾空白），19个默认容量值及排程/用量算例核对通过。直接比较31份所保留依赖原文，核对时内容一致；已有正文只改变第18章导航，其他历史正文保持原文。检查点42行，代码围栏闭合；结果保存在本章验证目录的verification.json。

本章保持19.1—19.5，承接第18章宿主并发，将部署组合留第20章。当前完成导读和第1—19章，24章正文完成19章，剩余第20—24章五章。只修改本章、README、SOURCES、检查点、第18章相邻导航及本章验证记录；第16—18章验证目录原样保留。

## 第 20 章续写验证记录

2026-10-07 在当前工作区执行12个Server定向既有用例：12通过、0失败、0忽略、550未选中，测试进程耗时19.28秒。选中MU9的10项迁入/恢复测试，另选MU6中的mu9_t64_restored_old_chat_stays_archived_and_jsonl_queued_recovers与jl7_backup_restores_new_chats_checkpoint_and_original_domain_objects。命令为 `cargo test -p server --lib --offline --locked -- 过滤项 --test-threads=1`；TEMP/TMP/TS_RS_EXPORT_DIR指向书稿 [.chapter20-replay-20261007](.chapter20-replay-20261007/)。[过滤项](.chapter20-replay-20261007/server-selected.txt)与[原始日志](.chapter20-replay-20261007/server-tests.log)保留。既有ts-rs属性提示未阻止编译。

用例使用临时真实SQLite、JSONL、文件和固定Provider；注入文件/元数据提交错误后重开，不是强杀或掉电。WAL中的已提交用户进入恢复；旧JSON保持档案；当前JSONL与压缩检查点、来源、演示、笔记、学习和教学关联可重开；queued恢复及未结接单禁止导出按原断言通过。演示和教学部分由现有夹具生成，没有真实浏览器、模型或Python制作。

直接执行Linux脚本的现有Python夹具：test-start-reader.py中5项通过，2.222秒；test-build-multi-reader.py中首轮1通过1失败，3.291秒。运行环境是Windows与Git Bash，替身pnpm/cargo/node/rustc，没有安装依赖或实际编译。[启动日志](.chapter20-replay-20261007/linux-launcher-tests.log)、[构建首轮日志](.chapter20-replay-20261007/linux-build-tests.log)保留。

失败项test_network_build_preserves_local_dist_and_records_source_copy得到working-tree而非source-copy。定向[诊断](.chapter20-replay-20261007/linux-build-diagnostic.log)显示Git Bash先解析/mingw64/bin/git，再解析夹具tools/git；不是产品构建命令失败。只在书稿驱动中将tools重新放到启动后的PATH首位，以source执行未修改的真实脚本，单独重跑原失败用例，1通过、2.254秒；[补验日志](.chapter20-replay-20261007/linux-build-msys-retry.log)保留，没有重跑已通过项。初次失败与条件调整同时报告，不能合写成首轮全绿。

`node apps/desktop/scripts/assert-plugin-release.mjs --source-contract-only`实际通过，[日志](.chapter20-replay-20261007/plugin-source-contract.log)给出0.1.0+codex.20261003012800。另在[书稿驱动](.chapter20-replay-20261007/replay_packaging.py)中执行3组发行配置输入（缺失/本地路径预期拒绝，公开仓库通过）和6组注册状态（首次、相同、未知文件、旧版缺显式参数、备份后迁移、备份冲突）；[结构化结果](.chapter20-replay-20261007/packaging-results.json)与[运行日志](.chapter20-replay-20261007/packaging-tests.log)保留。所有注册写入都位于书稿临时项目，未改真实Codex配置、注册表或安装插件。

Book MCP/T7编译产物冒烟、真实Codex CLI、Setup、Linux优化构建、systemd/Nginx、远端服务及制作沙箱本轮仅回读实现或历史资料，没有执行。历史MU11的4通过57.27秒与本轮耗时分开；8 GiB容量例按8×3=24、8×2=16计算，非磁盘实测。新增引用、摘录和依赖原文的书稿核对结果在本目录verification.json保存。

书稿收口核对通过：本章全部本地链接与配套新增链接共119处可达，4处Markdown锚点有效；194个实现符号和12个实际Server测试符号可定位，8段Rust/TypeScript/Bash/CMD摘录逐行匹配当前源码（忽略行首缩进与行尾空白），1个JSON迁移例的字段、转义与目录独立性正确；版本组合、停止期限、后端缺省端口、历史收据和容量算例均核对。直接比较保留的73份依赖原文，内容未变；代码围栏闭合，检查点43行，一幅Mermaid图按结构阅读，未做渲染验收。对照书稿原文，仅README、SOURCES、检查点和第19章导航变化，新增第20章；其余章节与OUTLINE未改。

本章保持既定20.1—20.5，导读与第1—20章成稿，24章正文完成20章，剩余第21—24章四章。只修改本章、README、SOURCES、检查点、第19章导航及本章验证记录；第16—19章验证目录保留。第21章观测与评测入口仅作定位，未宣称其完整事实核对已完成。

## 第 21 章续写验证记录

2026-10-07 在当前工作区直接执行15个Server、4个Runtime既有定向用例，全部通过，均无失败或忽略。Server另有547项未选中，测试进程7.35秒；Runtime另有457项未选中，测试进程0.00秒。命令为 `cargo test -p 包名 --lib --offline --locked -- 过滤项 --test-threads=1`，过滤项、命令和驱动耗时保存在[执行记录](.chapter21-replay-20261007/checks.json)，[Server原始日志](.chapter21-replay-20261007/server-tests.log)和[Runtime原始日志](.chapter21-replay-20261007/runtime-tests.log)保留。TEMP/TMP/TS_RS_EXPORT_DIR指向书稿临时目录；观测默认关闭，夹具显式构造捕获器，没有使用私人spool或外部凭据。

实际覆盖开始/结束边界、拒绝无时长、首patch只计一次、失败后保留末份usage、元数据过滤、请求差异、父链确认、队列容量和spool重开/损坏/目标变化/淘汰。正式多人准入用例使用临时真实SQLite、JSONL和本机固定HTTP响应，分别经过正常、Provider失败、终态保存失败三个分支；不是只调用孤立映射函数。退出预算和根coverage的限制按源码确认，没有新做真实网络超时、掉电或强杀实验。

Node首组直接运行 provider-recorder.test.mjs、eval-timing.test.mjs、diagnostic-core.test.mjs、task-quality-basis.test.mjs，共17通过，0失败、0跳过，测试进程553.8097毫秒；[原始日志](.chapter21-replay-20261007/eval-tests.log)保留。随后从 agent-core.test.mjs 与 task-quality.test.mjs 选择6项实际证据、完整分母、必要失败优先及task-v2分账用例，6通过，0失败，217.6964毫秒，见[评分日志](.chapter21-replay-20261007/scoring-tests.log)。两组共23项，使用假时钟、固定评分与本机响应，没有运行真实评委。

TypeScript使用现有Vitest运行观测包的 contract.test.ts、prebuild.test.ts、eval-import.test.ts，按名称选中8项通过，7项因过滤未选中；3份文件通过，进程6.51秒，测试执行146毫秒。保留[观测包日志](.chapter21-replay-20261007/ts-observation-tests.log)。覆盖共享合同与未知字段、构建尝试身份及修订、源回执不变、失败不确认、缺真实时间或授权时整组拒绝、uncertain实验不自动重发。客户端为RecordingTransport或固定替身，没有外部上传。

另在书稿内[重放驱动](.chapter21-replay-20261007/replay_recorder.mjs)直接调用当前 startProviderRecorder，以本机HTTP返回携带13 tokens的SSE。实际断言转发字节一致，记录器却记 non_json_response，measuredUsage为1请求、1缺失、总量null；[结构化结果](.chapter21-replay-20261007/recorder-boundary.json)与[日志](.chapter21-replay-20261007/recorder-boundary.log)保留。该项通过表示已确认当前JSON-only采集缺口，并不表示流式用量计量正确。50项既有用例加这一项书稿重放，合计51项通过；产品代码未改。

历史LA7、LA9-v2、缓存记录、EV4—EV7及CQ8只回读、复算，没有重新调用模型、补跑样本或上传实验。教学时间线、嵌套重复计数、虚构单价、指标分母与缓存加权比例另作确定性计算。书稿收口检查及依赖原文直接比较记录在[引用核对结果](.chapter21-replay-20261007/verification.json)，检查驱动与原始材料均留在本章验证目录。

收口核对通过：本章全部本地链接与配套新增链接共123处可达，4处Markdown锚点有效；161个实现符号及19个实际Rust测试符号可定位，6段Rust/JavaScript摘录逐行匹配当前源码（忽略行首缩进和行尾空白），代码围栏闭合。LA9的72行原始结果重算成功数与P95；配对增失、未知构建费用、嵌套时间、缓存加权及虚构单价均与正文一致。115份保留依赖原文直接比较未变；检查点42行，一幅Mermaid图按结构阅读，未做渲染验收。

首次引用检查指出正文work_unit应为work_unit_id，另有核对表把网络save_finished归入本地文件、把HttpLangSmithTransport类型名次序写反。已回读并修正正文、保存路径说明及核对表，修正后通过；首轮结果保留为verification-initial.json。没有因此重跑已经通过的产品测试。

本章保持21.1—21.5及章末面试回答。导读与第1—21章成稿，24章正文完成21章，第三、四、五篇均完成，剩余第22—24章。仅修改第21章、README、SOURCES、检查点、第20章相邻导航及本章验证记录；保留第16—20章验证目录。第22章入口仅作定位，完整决策演进事实核对尚未进行。

## 第 22 章续写验证记录

日期：2026-10-07。运行前确定要区分的失败：移除默认轮数闸后，有用长任务仍被截断；重复文字或未变工具无限重试；累计用量被误作当前容量；压缩未安装就改写原消息；工作项自报完成越过 Goal 交付；检索未确认调用 Provider、过期准备错误复用或短投影删除完整字段。若出现，与源码逐项对照、修正书稿结论并保存失败，不顺带改产品。

[驱动](.chapter22-replay-20261007/run_checks.py)、[完整命令和状态](.chapter22-replay-20261007/checks.json)保留在书稿内。直接执行当前仓库用例，没有复制实现或重写测试；TEMP、TMP 与类型生成目录指向书稿临时位置。

| 本轮实际执行 | 选择范围与结果 | 能说明的行为 |
| --- | --- | --- |
| Runtime 库测试 | [八项名称](.chapter22-replay-20261007/runtime-selected.txt)、[原始输出](.chapter22-replay-20261007/runtime-tests.log)：8 通过、0 失败、0 忽略、453 未选中，测试过程 0.44 秒 | 十六次采样完成；重复文字、工具与缺交付的停止；真实动作推进；活动容量；压缩安装后才提交；工作项不冒充 Goal 完成 |
| Core 检索测试 | [名称过滤](.chapter22-replay-20261007/core-selected.txt)、[原始输出](.chapter22-replay-20261007/core-tests.log)：三份文件选中 6 通过，另 20 未选中；Vitest 过程 7.07 秒 | 精确与别名保留、有效 key、分页复用、词法变化刷新、未确认及配置漂移零调用、词法和空浏览零调用、完整候选字段保留 |

合计 **14 项通过**。Runtime 使用固定模型响应和脚本化安装端口；Core 使用固定 Embedding Provider 与临时目录。脚本中的 16 次采样来自 1 次定位、14 次读取和 1 次最终回答；不是 16 次真实网络推理。上述秒数只是当前测试进程记录，驱动含启动时间分别为 2.894 秒、9.060 秒，不用于比较产品延迟、模型速度或费用。

本轮未运行 Server 测试、真实模型、真实 Embedding、浏览器或多人容量实验。会话、恢复和权威写入按当前实现回读；第16、18—20章已有定向验证只保留其原日期、夹具与边界，不把它们重复计入十四项。Rust 现有 ts-rs 属性解析警告保留在日志，不影响所选用例完成。

书稿检查针对新增链接与锚点失效、符号改名、摘录偏离当前源码、算例单位混用和读取后依赖变化；发现后回读对应实现并修正正文或记录。检查由[书稿核对程序](.chapter22-replay-20261007/verify.py)记录到[核对结果](.chapter22-replay-20261007/verification.json)：范围是第22章全文及 README、SOURCES、检查点和前章导航的新增部分。七段短源码按行去除展示缩进后与对应连续原文比较；UTF-16/UTF-8、累计输入和写入量按教学假设重算；LA9 与 SR6 数字回到历史原文。依赖使用写作前保存的原文逐字节比较，不生成校验和。Mermaid 做结构与叙述核对，没有声称渲染验收。

实际结果：129处链接（含4处Markdown锚点）、105个实现符号、8个Rust测试符号、6个Core测试名称、7段源码摘录全部通过。三个教学算例及脚本采样次数一致，LA9成功数和SR6两轮排名/未通过发布状态一致；58份依赖原文在写作期间未变。书稿变更范围符合本轮约定，最终检查点43行，下一章22份入口文件存在。检查首次运行通过；随后补入统计，并将三个泛型类型用行内代码显示，只补查这些新增文字，不重复运行产品测试。

仅新增本章及其书稿验证材料，更新阅读入口、来源与简短交接；第21章只改相邻导航，既有16—21章验证目录完整保留。第23章入口只做定位，完整事实核对留待续写。

## 第 23 章续写验证记录

日期：2026-10-07。运行前说明要检测的具体失败：未交完输入就消耗语义尝试；已冻结输入又被阶段渲染；租约过期在同一invocation不能换发，或错耗语义额度；候选字段反馈未到达新生成；分段选择破坏旧冻结任务；新增可选字段改写旧恢复记录。失败后保存结果并按当前源码修正文稿，不改产品。记录器探针另查当前修复用途错分，以及已知SSE解析缺口是否真的影响可选预算。

[驱动](.chapter23-replay-20261007/run_checks.py)和[完整命令与退出状态](.chapter23-replay-20261007/checks.json)保存实际执行。直接选择当前仓库已有测试；TEMP、TMP及类型生成目录指向本章临时材料。

| 实际执行范围 | 原始结果 | 结论 |
| --- | --- | --- |
| Core三份文件的六个用例 | [完整名称](.chapter23-replay-20261007/core-selected.txt)、[原始输出](.chapter23-replay-20261007/core-tests.log)：6通过、91未选中；Vitest 38.78秒 | 四个执行器用例、一个Driver同invocation换发、一个分段章节选择；实际产品函数、合成任务、临时文件和受控时间 |
| Runtime历史裁剪用例 | [名称](.chapter23-replay-20261007/runtime-selected.txt)、[原始输出](.chapter23-replay-20261007/runtime-tests.log)：1通过、0失败、0忽略、460未选中；测试过程0.00秒 | ex13_redaction_preserves_pre_new_object_history_bytes确认裁剪不向旧调用增加null字段，不代表整套检查点验收 |
| 记录器两项本地重放 | [重放程序](.chapter23-replay-20261007/replay_recorder.mjs)、[输出](.chapter23-replay-20261007/recorder-boundaries.log)、[结构结果](.chapter23-replay-20261007/recorder-boundaries.json)：两项预期问题均复现 | 读取生产v4签名后错分为unclassified；回环端口固定SSE中usage=7、tokenLimit=1，第二次仍返回200，两个null、0次预算拒绝 |

现有产品用例合计7项通过，另有2项问题复现。固定SSE原字节被保留，不含真实模型、网络服务或凭据。驱动含启动耗时分别40.812、2.392、0.279秒，仅作运行记录，不解释模型速度和产品延迟。既存ts-rs属性解析警告保留在Runtime日志。

本轮未运行真实模型、Embedding、浏览器、整套产品测试或前章Server矩阵。来源修复、网络准备、保存、发布及预览按当前实际路径回读，前章已成立用例保留原日期和边界；不能计入本轮七项。

书稿检查针对新增链接和锚点、实现及测试符号、四段源码摘录、历史数字与计算口径，以及第22章仅导航和其他已成稿章节保持原文。发现不符就回读并修正文稿；验证程序与结果分别存于[书稿核对程序](.chapter23-replay-20261007/verify.py)和[核对结果](.chapter23-replay-20261007/verification.json)。检查不生成哈希或校验和；Mermaid仅核对时序含义和结构，不声称浏览器渲染验收。

实际核对135处新增或变更范围内的链接（含4处锚点）、94个实现符号、7个测试名称、4段连续源码摘录，历史数字29处及五组计算口径。23份无关书稿与写作前原文逐字节一致，第22章仅相邻导航变化；检查点41行，产品跟踪文件无差异。首轮链接、摘录、计算和实现符号均通过，仅参数化Driver测试名被核对脚本误报：源码使用%s，实际运行名称展开为same；补入明确映射后单项核对通过，并只补验新增验证说明的链接与统计。[首轮结果](.chapter23-replay-20261007/verification-first.json)保留；没有重复运行产品测试。

本轮完成第23章五节、已知问题和七组面试回答，同步README进度为23/24、第22章相邻导航和简短续写记录。目录与第1—21章不改，保留第16—22章全部验证材料；第24章只写下一步，个人贡献留待真实参与依据。

## 第 24 章续写验证记录

本轮只运行书稿检查：检测新增链接或锚点失效、实现符号与摘录不符、历史分母串用、教学容量计算错误、相邻导航遗漏，以及无关章节被改写。失败时回读对应原文并修订书稿，不据此修改产品或重复前章行为测试。

核对程序与结构化结果保存在[书稿核对程序](.chapter24-replay-20261007/verify.py)和[核对结果](.chapter24-replay-20261007/verification.json)。新增正文全量核对，README、SOURCES、OUTLINE、检查点与第23章只检查本次新增或变化部分。写作前保留28份直接副本，用于核对已完成章节与目录的改动范围；未新增哈希或校验和文件。Mermaid按职责与结构核对，未运行浏览器渲染。

两段摘录分别取自当前来源匹配和网络私人状态端口。教学计算核对30与60次调用需求、360与720许可秒、480许可秒供给、75%与150%负载比，以及理想40次调用/分钟；并核对当前默认槽位为2，避免把假设8写成配置事实。LA7、LA9-v2、EV7与EX13.6的历史数字按各自原始记录核对。

实际首轮检查通过：新增或变化范围内109处链接（含5处锚点，52个不同目标）、57个实现符号、2段连续源码摘录、18处历史标记，以及两组容量条件与理想调用量计算。导读与第1—22章共23份正文逐字节保持原文；第23章仅相邻导航，OUTLINE仅上述两处主题调整。检查点35行，产品跟踪文件无差异。记录结果后只补核本段数字与结构化输出的一致性，不重跑已通过检查。

本轮没有运行产品测试、真实模型、Embedding、浏览器或新的容量与费用实验，也没有重新采样历史成绩。第23章6项Core、1项Runtime及两项记录器问题重放仅被引用，未算作本轮验证。产品代码、产品测试和第1—22章正文不改；第23章仅相邻导航，OUTLINE仅第24章中心问题与24.5主题变化。

## 附录与全书导航校订验证记录

2026-10-08的检查分为两个范围：全书现有Markdown的本地链接、标题锚点、代码围栏、连续章节与附录入口；新增附录的实现符号、两段源码摘录、四个JSON形状、坐标算例、默认配置和脚本入口。前者针对跨章组合后读者无法继续查阅的问题，后者针对新索引错指实现或将单位、历史状态写错的问题。发现失败时修正文稿或验证程序的具体误报，不重跑无关产品测试。

程序及结果见[本批核对程序](.book-reference-20261008/verify.py)与[核对结果](.book-reference-20261008/verification.json)。写作前保存29份书稿直接副本，用于确认原章节的实际改动范围；不新增哈希或校验和。保留第16—24章验证材料。源码链接使用相对路径，所有历史运行仍以原始日志及当时条件解释。

重点术语回查确认：正文已经把旧十二轮、前端待定与EV7待实施标为历史背景，source.present采用当前quote接口；没有因搜索命中这些词就改写历史。第3章四行表的数量表述作了编辑修正。Mermaid只检查围栏与导航，没有重新渲染旧图；本轮没有产品测试、真实模型、Embedding、浏览器、费用或容量实验。

首轮检查35份Markdown中的2133处本地链接（含43处锚点、467个不同目标），均可达；新增内容的69个符号中有1个被附录E指到了错误文件，另发现附录A缺返回入口。该用例实际位于presentation_author.rs，已更正链接与核对映射，并补上A的返回链接；只补验两项修正及本段新增记录，结果通过。[首轮结果](.book-reference-20261008/verification-first.json)与[单项补验](.book-reference-20261008/verification-corrections.json)分别保存，没有重跑已通过项目。

两段连续源码摘录、四个JSON示例、10个默认配置字段、7项脚本映射及坐标算例通过。第1—24章中22份未涉及的正文逐字节保持原文；第3章只修正表格引导，导读与第24章只增加附录导航，原章节目录不变。检查点31行，产品跟踪文件无差异。新增链接在补验记录中单列，不将首轮链接数量写成补验后的全量重跑结果。

## 连续阅读版导出验证记录

2026-10-08完成连续阅读版[Markdown](exports/深入UnderstandBook-连续阅读版.md)、[EPUB](exports/深入UnderstandBook-连续阅读版.epub)及[Markdown含插图压缩包](exports/深入UnderstandBook-连续阅读版-Markdown含插图.zip)。合并输入为chapters的导读与第1—24章，以及appendices的A—F，共31份源稿。只编排既有书稿，原章节与附录保持原文；生成过程以读取时的原始字节确认输入未被改动，没有用相同Git提交替代文件核对。各章事实读取日期和历史验证范围保留原口径。

导出程序、排版规则与阅读说明见[导出说明](exports/README.md)、[生成程序](exports/tools/build.py)、[图表与公式渲染](exports/tools/render-assets.cjs)和[电子书样式](exports/tools/reading.css)。Markdown保留原公式标记并使用26幅流程图附件；EPUB内嵌26幅图、9处独立公式及14处行内公式，共49张图片。部分过宽的横向流程图调整为纵向，仅改变布局。源码引用在Markdown中继续采用相对路径，在EPUB中转到340个文件或段落出处条目；电子书不打包产品源码。

检查前明确了失败处理：合并后丢字、代码摘录变化或链接错位时修正转换；公式或图表不能渲染时修正导出处理；EPUB结构不合规或窄屏裁切时修正标记与样式。本次检查不解释产品行为，也不扩大前章验收。转换器会将正文少量泛型参数误认成HTML，导出解析已经保留其可见文字。首轮EPUBCheck发现旧Pandoc输出的代码高亮嵌套不合规，改为不带高亮的代码块，内容不变；[首轮诊断](exports/tools/work/epubcheck-first.json)保留，[修正后结果](exports/tools/work/epubcheck.json)为零错误、零警告。只重新生成受影响的EPUB，没有重跑图表渲染。

[内容与引用核对](exports/tools/verify.py)的[实际结果](exports/tools/work/verification.json)通过：31份源稿、17071个正文或行内代码片段、两种格式的150段完整摘录、Markdown的23处公式，以及522个节锚点。140张表和49张图片完整；Markdown的1210处本地引用与EPUB包内1392处资源或锚点引用均可达。这些计数是各自格式中的引用出现次数，不能互相当作唯一文件数。EPUB共35个XHTML及35个spine阅读项，包括正文、附录、扉页、编排说明、目录与出处索引；目录列编排说明、31份源稿和出处索引，共33项。

[版面程序](exports/tools/preview.cjs)实际使用Playwright驱动无头Edge检查390与900像素宽度下的34个内容页面，共68次，包含图片加载和横向溢出；[结果](exports/tools/work/layout-verification.json)通过。另人工查看6张截图，覆盖手机流程图、表格和公式，以及桌面源码、公式和扉页。图片完整，代码与表格可换行，行内公式尺寸正常。这里运行的是导出内容渲染，不是产品浏览器验收；未测试实体墨水屏或第三方EPUB阅读器。长时序图的窄屏细节需要放大，阅读器可能采用不同分页和字体。

本轮只新增exports及更新书稿入口、来源记录与续写点，未改产品或产品测试，未运行真实模型、Embedding或新的性能、费用、容量实验；既有.chapter16-replay-20261007至.chapter24-replay-20261007及.book-reference-20261008材料全部保留。个人贡献表达继续暂缓。

## 当前实现与历史材料的处理

ADR-0136 仍包含十二轮上限、Tutor 当时未实现等背景。当前停止规则采用 ADR-0150 与 OuterConfig；当前 Tutor、工作计划状态需要结合现有实现、ADR-0141、ADR-0155 和架构中的后续记录。旧背景可用于演进章节。

ADR-0127 的早期历史与恢复安排需要结合 JSONL、网络准入和当前运行提交代码理解。正文以当前会话事实说明保存；各类 queued、claimed 与本地 pending 的恢复行为留给第 16、19 章逐项展开。

当前 goal_turn_delivered 对 Content 分支返回 true，对 ReaderAction 检查 effects 非空，对 PresentationDelivery 核对目标结果中存在相应呈现引用。样章据此说明通用检查的粒度，不能把它写成任意自然语言任务都已被程序逐项证明完成。

ADR-0021 的“前端待定”属于历史状态，当前已有 Vue 和 Tauri 实现。ADR-0146 的 U1—U2 已完成，但 U3—U11 尚未实施；新章节区分内容版本约定与完整增量复用、迁移能力。ReaderWorkspace 的本地 publication 可为空，网络发布的确切引用不能无条件套到所有本地入口。

第 4—5 章同时区分 ADR-0008 的句级后续设计与当前源块切分，ADR-0062 的旧 PDF 来源假设与 ADR-0063 的规范正文，以及 ADR-0010 的全量目录方案与现行候选式、可选 Pass2。构建期锚点结构接纳与第 9 章读时已观察证据是不同环节，不能因为节点有 LID 就推为当前运行已经读过对应文字。

现有 README 中的早期产品对照和后续同环消融具有不同实验条件。写评测章时保留其时间、样本、成功定义和成本口径；单题修复结果只支持对应问题的验证。
