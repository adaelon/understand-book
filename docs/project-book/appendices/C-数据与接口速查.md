# 附录 C 数据与接口速查

本附录围绕“拿到一个值以后应当怎样解释它”组织合同。字段表只列理解全书所需的部分，完整定义以链接中的当前类型为准。示例中的LID、文字和候选ID是教学输入，实际调用仍需要存在的材料、已观察证据或有效候选。

## 身份与坐标

| 值或字段 | 作用域 | 使用时需要同时知道什么 |
| --- | --- | --- |
| book_id | 内容基座 | 它对应的原文与定位成果 |
| publication_id | 一次不可变发布 | 所属book_id |
| lid | 材料内部层级位置 | 所属材料，且按数值路径比较顺序 |
| workspace_id、workspace_generation | 阅读现场及其代际 | 原Run固定的现场归属和当前有效性 |
| chat_session_id、turn_id | 聊天与被接纳回合 | 原用户和材料 |
| work_unit_id、semantic_attempt、lease_epoch | 构建工作、语义尝试与执行代次 | 相应输入与策略范围 |
| source_ref_id | Runtime分配的来源身份 | 已保存或本轮有效的来源绑定 |
| presentation_id、revision | 确切演示版本 | 用户、材料及相应访问入口 |
| SessionEvent.seq | 某聊天日志中的顺序 | 所属聊天，不是SSE游标 |

来源分别是[RunScope](../../../crates/server/src/run_scope.rs)、[构建执行身份](../../../packages/core/src/automatic-build-task-store.ts)、[来源与回答类型](../../../crates/runtime/src/orchestrator.rs)、[演示类型](../../../crates/runtime/src/presentation.rs)和[会话事件](../../../crates/server/src/session_event.rs)。

确切发布的实际定义来自[PublishedBookRef](../../../crates/server/src/published_library.rs)：

```rust
pub struct PublishedBookRef {
    pub book_id: String,
    pub publication_id: String,
}
```

在本地Reader中，RunScope.publication可以为空；网络发布入口的确切发布引用不能直接套到所有本地材料打开方式。

[Span](../../../crates/base-schema/src/lib.rs)的实际定义很短：

```rust
pub struct Span {
    pub start: usize,
    pub end: usize,
}
```

字段形状之外，还必须保留坐标的参照系：

| 范围 | 单位与原点 | 解释 |
| --- | --- | --- |
| LidNode.span | 全部规范原文的UTF-16半开区间 | 将一个节点映射回source.txt |
| SourceSelectedRange.range | 指定LID正文内部的UTF-16半开区间 | 表达节点中的选中部分 |
| SourceExcerpt.highlight | 返回excerpt.text内部的UTF-16半开区间 | 让界面高亮本次展示片段 |
| 模型Token估算 | 相应估算器的输入或输出计量 | 用于预算，不作为字符串下标 |

三种原文范围可在[read-tools](../../../crates/read-tools/src/lib.rs)的match_source_quote与resolve_source相关路径中对照。教学字符串`A中B`有3个UTF-16单位、5个UTF-8字节；`中`在前一种坐标中为[1, 2)，在后一种中为[1, 4)。若该段全局Span从120开始，局部UTF-16的[1, 2)对应全局[121, 122)。这些是坐标算例，不是书中某个真实节点的数据。

## 公开材料与私人事实

| 对象 | 主要内容 | 权威与读取入口 |
| --- | --- | --- |
| source.txt、base.json | 规范原文、book_id、lid_nodes、graph_nodes、graph_edges | [ReadOnlyBase](../../../crates/base-schema/src/lib.rs)、[Book::load](../../../crates/read-tools/src/lib.rs) |
| book_structure.json等sidecar | 随材料提供的结构或专项阅读能力 | Book::load分别解析；各类缺失与无效处理按相应合同 |
| 构建候选及发布回执 | 待接纳生成结果、正式成果发布事实 | [执行器会话](../../../packages/core/src/automatic-build-executor-session.ts)、[发布](../../../packages/core/src/automatic-build-publication.ts) |
| 会话JSONL | 接纳、消息、来源、效果、目标及终态等顺序事件 | [SessionEvent](../../../crates/server/src/session_event.rs)、[SessionLog](../../../crates/server/src/session_log.rs) |
| 私人笔记、画像与学习记录 | 原用户的持续状态 | [Memory](../../../crates/memory/src/lib.rs)及Server的私人状态端口 |
| 演示版本与现场 | 已保存版本及其运行、继续修订所需信息 | [presentation_sandbox.rs](../../../crates/server/src/presentation_sandbox.rs)与演示合同 |

当前SessionEvent的封套包含version、seq、at、可选turn_id，以及展开后的kind和payload。`sources.bound`保存来源绑定，`effect.delivered`保存相应效果关联，`turn.finished`保存终态；具体顺序由提交路径决定，不能手工拼出一个“看似完整”的日志代替业务操作。

## Book工具：先辨认入口，再看参数

[book-tool-contracts](../../../crates/book-tool-contracts/src/lib.rs)为共同Book能力维护类型和不同表面的别名。以Text为例：

| 表面 | 名称 | 责任 |
| --- | --- | --- |
| Resident | book.text | 内置Agent读取已定位原文 |
| MCP | book_text | 外部调用方读取共同Book能力 |
| REST分派别名 | text | 由相应HTTP路由映射到合同 |

这里的REST别名不是完整URL。本地与多人宿主仍有各自路由、材料选择和权限上下文。外部MCP调用也不会因此取得Resident私人状态写入入口。

TextInput接收一个起点lid与可选end_lid。下面分别表示一个位置和一个包含终点的LID范围：

```json
{"lid":"1.2"}
```

```json
{"lid":"1.2","end_lid":"1.4"}
```

LID范围的“包含终点”与字符串Span的“半开区间”针对不同对象。调用前要确认材料里确实存在这些位置；当前反向Text范围的边界见[第10章](../chapters/10-工具与模型调用边界.md)。

## 来源交付：引文是输入，来源身份是结果

当前SourcePresentArgs只声明quote，并拒绝未知字段。模型提交的形状是：

```json
{"quote":"这里填写本轮已经观察到的连续原文"}
```

[TurnEvidenceLedger::prepare_present](../../../crates/runtime/src/orchestrator.rs)先处理参数，再让Book在已观察区间匹配；成功后产生source_ref_id、label和preview。模型不能通过额外填写lid或source_ref_id来指定一份尚未观察的来源。

回答的结构化展示由AgentAnswerView连接：

| 对象 | 当前关键字段或变体 | 用途 |
| --- | --- | --- |
| SourceBinding | source_ref_id、book_id、evidence_range、label_snapshot、preview_snapshot等 | 保留原文绑定与展示依据 |
| AgentAnswerView | parts、sources | 组织公开回答 |
| AgentAnswerPart::Markdown | text | 展示正文 |
| AgentAnswerPart::Sources | source_ref_ids | 展示已有来源引用 |
| AgentAnswerPart::Presentation | presentation_id、revision | 关联确切演示版本 |

来源身份正确以后，仍需判断它是否支持主张，以及任务要求是否全部交付。相关失败与评分口径见[第12章](../chapters/12-来源与流式交付.md)、[第21章](../chapters/21-可观测性成本与效果评测.md)和[第23章](../chapters/23-真实失败与工程改进.md)。

## 演示：制作操作与版本身份

[AuthorRequest](../../../crates/runtime/src/presentation_author.rs)按operation选择变体并拒绝未知字段。下表是字段选读，不能替代完整请求定义：

| operation | 关键输入 | 对应阶段 |
| --- | --- | --- |
| prepare | phase、needs及可选framework、focus | 组织本轮制作指导 |
| write | title、html、readable_content及相应资源、来源、状态字段 | 建立或修订候选 |
| read、search | reference或candidate_id，以及文件、范围或查询 | 回读版本或候选内容 |
| patch | reference或candidate_id、edits，以及合同允许的元数据 | 对已有内容作局部修订 |
| preview | candidate_id及相应视口、动作 | 获取候选的工程预览 |
| deliver | candidate_id | 在交付条件满足后形成确切版本引用 |

最短的交付请求形状是：

```json
{"operation":"deliver","candidate_id":"candidate-from-this-run"}
```

这个对象可解析，不代表候选存在或交付条件成立。当前Patch变体没有source_ref_ids字段，补充完整来源列表需要按Write合同处理；第17章记录的错误提示与字段合同不一致问题仍应保留。

## 默认配置与单位

以下值来自2026-10-08回读的[ServiceLimits::default](../../../crates/server/src/service_limits.rs)，用于解释默认约束，不表示实测容量：

| 字段 | 默认值 | 约束对象 |
| --- | ---: | --- |
| active_runs / user_active_runs | 4 / 1 | 服务 / 单用户活动任务 |
| queued_runs / user_queued_runs | 20 / 4 | 服务 / 单用户排队任务 |
| model_slots / user_model_slots | 2 / 1 | 服务 / 单用户模型调用许可 |
| preview_slots / plot_slots / animation_slots | 1 / 1 / 1 | 三类制作资源各自的许可 |
| event_bytes | 256 × 1024 | 事件缓冲的字节预算 |

普通Resident的[OuterConfig](../../../crates/runtime/src/orchestrator.rs)默认max_turns=None，旧token_budget字段不再充当活动上下文停机闸。活动容量见[ActiveContextBudget::from_plan](../../../crates/runtime/src/auto_compaction.rs)，累计用量与费用口径见第21章。

## 合同边界

本附录核对类型、当前字段与示例形状，没有执行真实HTTP、MCP、模型或浏览器调用。字段接纳、业务授权、语义正确和持久成功分别由各自路径处理；会话JSONL、准入SQLite和领域存储没有共同事务。完整调用分析回到相应正文，运行与验证入口见[附录E](E-运行验证与实验入口.md)。

返回[全书入口](../README.md)。
