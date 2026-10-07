# 会话 JSONL 与本次阅读回顾：实施切片

状态：JL0–JL9 已实现并验证，桌面与服务启动直接使用新日志并提供阅读回顾；JL10 集成与发布验收待实施。更新：2026-10-02。

决策依据：[ADR-0152 会话顺序日志](adr/0152-resident-linear-jsonl-session-log.md)、[ADR-0153 本次阅读回顾](adr/0153-session-reading-recap.md)。

## 1. 交付目标与术语对齐

将 Resident 聊天改为每会话一个线性 JSONL，恢复原有聊天能力，并保存 Understand Book 的提问现场、回答来源、实际成果及教学关联；在此基础上提供可追溯的“本次阅读回顾”。本文件是实施合同与任务表，各切片完成前不视为功能已上线。

会话保持线性顺序。聊天日志负责对话事实，教学事实仍由原领域持有；读取日志只恢复状态，重新执行阅读操作通过既有显式命令进行。

| 术语 | 状态 | 本方案中的含义与归属 |
| --- | --- | --- |
| agent 对话历史 / AgentChatSession | EXISTING | 读者私有、按材料组织、由用户显式新建的聊天；保持原 session / turn 身份 |
| Agent 会话顺序日志 | BOUNDARY_CHANGE | 聊天事实的持久顺序记录；当前会话状态由它派生 |
| RunScope / ReaderInputSnapshot | EXISTING | 原用户、原聊天、原材料发布及提问现场；现场换代不改写旧问题 |
| 回答来源绑定 | EXISTING | 回答关联的原文证据；检索命中、读取和回答接受依据分别表达 |
| ResidentGoal | EXISTING | 当前聊天的任务目标及完成状态 |
| TutorSession / 教学事实 | EXISTING | 跨聊天教学状态；仍由 learning.db 持有，聊天只保存当时引用与修订 |
| Presentation / PresentationFollowUp | EXISTING | 不可变内容版本与发送时的交互状态引用；内容仍在原存储 |
| 阅读成果处置回执 | NEW | 实际保留、撤销或忽略操作的结果及原／结果对象关联 |
| 本次阅读回顾 | NEW | 当前聊天明确范围内的事实投影，不产生掌握判定 |

领域定义见 [CONTEXT.md](../CONTEXT.md)。本项属于聊天存储边界变更与阅读回顾模型扩展。

## 2. 当前实现与必须保留的行为

本节依据 2026-10-01 工作区代码；路径和符号是后续切片入口，不表示这些文件已改造。

| 现有入口 | 当前行为 | 本次处理 |
| --- | --- | --- |
| [server/lib.rs](../crates/server/src/lib.rs)：AgentHistory、AgentChatSession、precommit_agent_turn_with_goal、finalize_agent_turn | 全部会话保存为 agent-history.json；提交／结束回合会复制并重写历史 | 改为当前会话增量提交；保留历史 API 的用户语义 |
| [run_admission.rs](../crates/server/src/run_admission.rs)：FrozenTurn、准备与恢复链路 | 冻结输入内含整段 messages；准入与控制库共同决定恢复 | 用日志位置重建冻结输入；保留请求幂等、原 turn_id、教学准备回执及准入规则 |
| [runtime/lib.rs](../crates/runtime/src/lib.rs)、[orchestrator.rs](../crates/runtime/src/orchestrator.rs)、[compaction.rs](../crates/runtime/src/compaction.rs) | 维护消息、私有协议续接字段、工具结果与压缩投影 | 记录必要增量；压缩不删除旧记录，日志读取不调用 provider 或工具 |
| [agent_run.rs](../crates/server/src/agent_run.rs)、[agent_stream.rs](../crates/server/src/agent_stream.rs) | 最终清理工具历史；运行摘要支持重连，SSE 有自己的序号 | 清理移到持久化边界之前；保留最终合法修订，区分日志序号与 SSE 序号 |
| [run_scope.rs](../crates/server/src/run_scope.rs)、[presentation_store.rs](../crates/server/src/presentation_store.rs) | 原问题归属固定；Presentation 内容与状态单独保存 | 日志关联已有身份与版本，不复制整份成果内容 |
| [user_runtime.rs](../crates/server/src/user_runtime.rs)、[user_storage_paths.rs](../crates/server/src/user_storage_paths.rs) | 用户历史路径独立；presentation_root 由 history_path 派生 | 明确解析各自路径，保留已有 agent-history.presentations 目录 |
| [App.vue](../packages/web/src/App.vue)：keepEffect、undoEffect | 执行真实命令后，仅在前端 handled 中标记处置；保留 highlight 可能生成新对象 | 增加服务端处置回执，关联新旧对象，刷新后恢复处理结果 |
| [reader_maintenance.rs](../crates/server/src/reader_maintenance.rs) | 维护工具管理私人数据备份与恢复 | 将新日志与选择元数据纳入现有备份恢复清单 |

继续遵守 [ADR-0147](adr/0147-linux-multi-reader-service-without-redis.md) 的单服务单写者、每聊天准入互斥、已认领运行不自动重跑、私人数据隔离及离线维护合同；继续遵守 [ADR-0141](adr/0141-global-tutor-control-and-session-ownership.md)、[ADR-0143](adr/0143-teaching-trace-assessment-and-learning-evidence.md)、[ADR-0144](adr/0144-shared-presentation-conversation-workspace.md) 的领域归属。

## 3. 存储布局与事件合同

每个读者现有私人 memory 目录新增 `agent-sessions/`；每聊天一个 `s_<base64url(session_id UTF-8，无填充)>.jsonl`。逻辑 session_id 保持原值；现有 ID 中可能包含 Windows 文件名不接受的冒号，文件名使用可逆编码。新日志不改变材料、笔记、learning.db 或 Presentation 的位置。

JSONL 聊天的本地／默认选择信息保存于小型 `agent-chat-selection.json`；它只引用新日志中的聊天，可在丢失后选择最近聊天重建。服务端各阅读现场的选择仍归控制库。标题、创建时间等会话事实存在日志中，选择文件不承载会话正文或第二份事实。

首版启动时逐文件折叠，形成内存中的列表及会话状态；随后按已提交事件增量更新。历史列表请求不重新扫描全部文件，单轮提交不复制全部 AgentHistory。不新增持久检索索引；确有启动或内存负担后再按测量结果选择索引或懒加载。

事件信封的预定形状如下，JL0 将其落实为有类型的结构：

```json
{"version":1,"seq":1,"kind":"session.created","at":"2026-10-01T09:00:00+08:00","payload":{"session_id":"chat_…","book_id":"book_…","title":"…"}}
{"version":1,"seq":2,"kind":"turn.accepted","at":"2026-10-01T09:01:00+08:00","turn_id":"turn_…","payload":{"history_through_seq":1,"user":"…","reading_context":{}}}
```

示例只展示信封与关联，不是完整业务 payload。`seq` 在当前文件中从 1 连续递增；`at` 用于显示，恢复顺序以 `seq` 为准。无 parentId 或 active leaf；执行活动自身的 parent_step_id 仍可表示工具步骤嵌套。

| 事件族 | 保存内容 | 折叠作用 |
| --- | --- | --- |
| session.created / session.updated | 会话身份、材料、标题等元数据 | 建立会话并更新列表信息 |
| turn.accepted / turn.prepared | 用户原消息、原回合序号、冻结现场、provider 绑定、历史位置；已有准备回执 | 恢复已接收的回合与可核对的准入输入 |
| message.appended / activity.recorded | 完整消息与工具调用／结果；必要的工具开始、完成、错误和 usage | 恢复合法消息序列与运行观察，完整活动复用现有类型 |
| history.revised / checkpoint.installed | 受影响的旧消息位置与合法替换后缀；既有压缩检查点 | 表达当前真实存在的结束清理、失败截断与模型上下文投影 |
| goal.updated / teaching.linked | 聊天目标状态；教学 session、revision 与原领域回执引用 | 更新聊天任务并保留当时教学关联 |
| sources.bound / effect.delivered | 回答来源绑定；实际交付的成果引用 | 支持历史来源跳转及成果状态恢复 |
| effect.disposition_started / effect.disposed | 用户处置身份、目标、操作；实际执行结果回执 | 区分待执行／待核对与已经完成的处置 |
| turn.finished | 回合完成／失败／中断、摘要、结果及错误 | 确立回合终态及恢复后的观察快照 |

属于一次不可分更新的字段写在同一事件里，例如终态与该终态必需的结果；不得把恢复必须共同出现的状态拆成几个可能缺一的事件。事件 payload 复用现有领域结构，只为缺失的关联与回执新增类型。

新回合的 `FrozenTurn.messages` 改为已提交的 `history_through_seq`，按该位置恢复消息及当时压缩投影，再应用该回合冻结的输入；原本单独冻结的规范化请求、provider、现场及教学准备资料继续保存。后续消息或目标变化不能改变已准入回合的输入。

压缩检查点继续引用原始历史与保留项；`history.revised` 触发既有失效规则。事件保存增量，每 token 展示补丁留在既有流式通道，不落成持久事件。

## 4. 写入、恢复与副作用边界

顺序为“构造允许持久化的事件 → 追加完整行并完成约定的刷盘 → 推进内存投影 → 对外确认持久状态”。持久化边界覆盖准入确认、完整消息／工具结果、检查点和终态；界面可以先显示临时流，但不能据此宣称已保存。复用既有单写者和聊天互斥，不新增第二套调度系统。

追加失败时不推进已提交序号。继续写之前重新核对末尾，利用原 turn_id、工具 call_id 及记录位置识别已完整写入的结果，不能将不确定的同一次提交再次追加。实现需明确 flush／sync 的调用边界；一行是一条逻辑记录，不意味着文件写操作天然原子。

| 恢复时看到的情况 | 处理 |
| --- | --- |
| 末尾只有半行 | 折叠前面的完整记录；重新取得写入权后截去不完整尾部，再继续追加 |
| 中部无效 JSON、序号断裂或不支持的格式版本 | 报出文件及位置，停止恢复该会话；保留文件，交由既有维护入口处理 |
| 已准入、未认领 queued，冻结输入及准备回执齐全 | 按控制库与既有恢复规则继续该运行 |
| 已认领／开始执行而没有终态 | 记录中断或待核对；工具与 provider 不自动重跑 |
| 成果存储成功，日志回执保存失败 | 通过已知对象／操作身份核对；能确证则补回执，不能确证则保留待核对状态，不再次执行副作用 |

原始内存消息须先经过既有工具暴露／Presentation 清理规则再追加。只有允许保存的模型协议续接字段进入私人记录，公开历史与回顾不输出这些字段。若某工具内容只能在完成后确定可保存范围，先保存调用身份和阶段，待可清理时再提交消息；`history.revised` 不能用来抹去已经落盘的禁存内容。

`history.revised` 仅覆盖当前代码已经需要的最终消息清理和失败截断，不提供编辑旧回合后重新分叉的产品能力。折叠日志不修改 reader session、memory、learning.db 或 Presentation Store；新执行继续使用原命令与原领域写入路径。

## 5. Understand Book 的领域记录

| 内容 | 必须留下的关联 | 首版判定规则 |
| --- | --- | --- |
| 提问现场 | 原 PublishedBookRef、位置／已验证引文、RunScope 归属；如有演示则带 PresentationFollowUp | 使用发送时现场，后续翻页、切书或拖动演示不改写它 |
| 回答来源 | 原 publication、原文 locator／range、现有证据标识、所属回合及来源绑定 | “引用的原文”来自回答已接受并绑定的依据；检索候选不会自动进入该区 |
| 笔记与标注 | 实际创建对象 ID、所属材料／位置、临时或长期层；处置操作的原／结果对象 ID | 创建成功不等于用户已保留；保留 highlight 生成新对象时保留对应关系 |
| 导航与布局 | 实际操作回执、应用前后必要引用；提议与实际应用分别记录 | 提议未应用时显示待处理，已应用后按真实结果显示 |
| Presentation | 实际交付的 presentation_id、revision、状态引用及所属回合 | 卡片草稿或工具意图不算交付；回看旧版不自动替换当前画布 |
| 聊天任务／教学 | ResidentGoal 的真实状态；原 TutorSession 与准备／教学事实引用 | 回答结束不代表任务完成；助手讲解、回顾或压缩不构成学习掌握证据 |

成果处置使用稳定的 `{turn_id, effect_id}` 关联，不以界面数组位置作为身份。能复用现有稳定 effect 身份时直接复用；创建回执固定该身份，之后的流式展示和最终结果保持一致。

前端“保留／撤销／忽略”经服务端处置入口处理：先保存该次处置的身份、目标与操作，再调用现有业务操作，成功后保存回执并返回结果。回执包括实际结果、原对象与结果对象；历史读取恢复处理状态，重复提交同一已完成处置返回原回执。若开始记录没有结果，先按已有对象／操作身份核对，无法确证时显示待核对，不能直接重做。不同处置的合法顺序沿用现有交互能力，不借此扩展任意撤销／重做系统。

原成果后来被删除，不会改写“当时曾保存”的事实；条目在访问时再报告当前对象是否可用。历史删除沿用既有私人历史合同，与长期记忆的“忘记”分别处理。

## 6. 阅读回顾的可验收界面

在当前聊天历史区域提供“本次阅读回顾”入口，打开只读面板。首版范围固定为该聊天起点至打开时最后一个已提交事件；显示“当前对话 · 截至……”和刷新操作。进行中的回合以已保存事实呈现，刷新才吸收后续记录。

| 区域 | 数据来源与显示内容 | 跳转 |
| --- | --- | --- |
| 讨论过的问题 | 用户回合原文的简短展示及完整文本，按原顺序；附已回答／进行中／中断等运行状态 | 定位该回合 |
| 引用的原文 | 回答绑定的材料、章节／位置及可用引文；同一来源可合并并保留所有关联回合 | 通过原发布与 locator 打开原文 |
| 留下的成果 | 笔记、标注、已交付演示及实际阅读操作；显示已生成、已保留、已撤销、已忽略、待处理或待核对 | 定位对象、原版本或原回合 |
| 待继续事项 | 尚未完成的 ResidentGoal，以及失败／中断等明确未完成的回合 | 定位目标或回合，后续操作走既有聊天入口 |

“已回答”只表达回合结果。普通问题在已有回答后是否仍未理解，首版不推断；没有现成结构化读者标记时也不新增标记体系。空区域显示简洁空态，尚未处置的成果显示待处理，已开始但没有结果的处置显示待核对。

预定读取接口为 `GET /agent/history/recap?session_id=…&through_seq=…`；省略位置时由服务端固定当前已提交末尾，返回 `session_id`、`through_seq`、`generated_at` 与四组条目。每项携带 `{turn_id, event_seq}` 证据引用以及按需的来源／成果引用。接口名称可在 JL8 配合现有路由风格调整，范围与只读合同保持不变。

回顾按事件折叠确定性生成，不保存独立总结文件，不调用模型，不产生新回合或教学更新。原文、成果及历史仍通过既有身份与授权访问；资料不可用时显示原因，不能跳到另一发布来代替原引用。点击回顾条目才触发明确跳转，打开面板本身不移动阅读现场。

## 7. 新日志启用与备份恢复

正式启用时，桌面与服务模式都直接创建或打开该读者的 `agent-sessions/`，聊天列表与回顾只读取该目录的事件。旧 `agent-history.json` 原样留在原处；书籍、笔记、标注、学习数据库及 Presentation 继续使用原存储和路径。

1. 首次使用从新聊天开始，选择元数据只引用新日志中的会话；服务阅读现场中失效的旧聊天选择回到新聊天入口。
2. 旧聊天遗留的未完成运行按既有中断／结束流程收口，不恢复成新的运行；新 JSONL 的 queued／claimed 仍按 §4 的规则恢复。
3. 备份与恢复覆盖新日志、选择元数据及既有领域存储；恢复后验证新聊天列表、来源、演示和教学关联。
4. 新日志启用与恢复验收通过后进入发布；桌面与服务模式共用同一事件合同。

JL7 已移除早期方案中的导入事件、Imported 位置及冻结片段读取分支；相关测试通过正常创建／追加事件准备会话。备份递归清单包含新日志、选择文件与选择恢复备份，维护文件分类已识别这些路径。

## 8. 切片任务表

每个切片以文件中的合同、代码和验收结果交接；已完成部分与验证记录如下，其余按前置依赖实施。

| 切片 | 交付 | 前置 | 状态 |
| --- | --- | --- | --- |
| JL0 | 类型合同、投影不变量与现有行为样本 | 本方案 | 已完成；事件合同 5 项、压缩回归 2 项通过 |
| JL1 | 单会话追加、折叠与末尾恢复 | JL0 | 已完成；日志读写恢复 6 项、压缩位置回归通过 |
| JL2 | 路径、会话列表与新建／选择／删除 | JL1 | 已完成；管理／路径 8 项、本地历史与 MU6 回归通过 |
| JL3 | 准入、冻结输入引用与运行恢复 | JL2 | 已完成；六个保存窗口、请求幂等与原位置 queued 恢复通过 |
| JL4 | 消息／工具／终态／压缩的增量持久化 | JL3 | 已完成；四个强杀窗口、写盘失败、终态精确补存、清理／压缩及原运行回归通过 |
| JL5 | 阅读现场、来源、演示及教学关联 | JL4 | 已完成；冻结演示、来源压缩／重启、跨聊天教学原回执测试通过 |
| JL6 | 成果处置服务端回执 | JL5 | 已完成；真实对象保留／撤销／忽略、失败窗口、界面恢复及多人归属测试通过 |
| JL7 | 新日志启用、导入分支清理与备份恢复 | JL5、JL6 | 已完成；4 项专项验收，相关后端回归去重 201 项最终通过 |
| JL8 | 阅读回顾投影与读取接口 | JL5、JL6 | 已完成；固定截点、连续保留／撤销、只读及原用户／publication 授权通过 |
| JL9 | 阅读回顾面板与原处跳转 | JL8 | 已完成；四区、刷新、原处跳转、聊天切换及 390px／1440px 浏览器验证通过 |
| JL10 | 桌面／服务集成与发布记录 | JL7、JL9 | 待实施 |

## 9. JL0：固定合同和样本

输入：本方案及 §2 现有链路。产出：server 中的具体事件类型、会话投影规则和只覆盖当前受支持流程的测试样本；记录旧字段到事件的映射。

范围：普通问答、一次真实工具结果、失败截断、压缩后继续、queued／claimed 恢复、带教学及 Presentation 引用的历史。对原有类型字段以代码为准，不建立通用事件框架。

验收：相同已有事实映射后的会话视图一致；列明 FrozenTurn.messages、私有协议字段、最终历史修订及现有 ReviewJob 水位如何保留。若样本暴露未映射字段，补映射后才进入存储切片。

JL0 字段映射（2026-10-01）：

| 旧字段 | 事件／投影归宿 |
| --- | --- |
| AgentChatSession.id / book_id / title / created_at / updated_at | created 建立会话；title 由 updated／accepted 更新；后续事件 at 推进 updated_at |
| turns[].turn_id / user_turn_ordinal / user / question_anchor_lid / question_quote | accepted.turn；沿用旧身份和严格递增 ordinal，ReviewJob 的 session／turn／ordinal 水位不换成日志 seq |
| published_book_ref / presentation_follow_up / teaching_ref / goal_ref | accepted.turn 原类型；teaching.linked 补充原教学 revision／receipt_ids；终态可更新 goal_ref |
| status / outcome / error / run_summary / source_bindings / delivery_diagnostics | finished 原子提交；sources.bound 可提前绑定；运行活动以 activity.recorded 保留，summary.last_seq 仍为 SSE 水位 |
| messages / provider_continuation / tool_calls / tool_call_id | message.appended；保留允许持久化的私人协议字段和工具配对，公开历史仍只消费既有 turn/session view |
| compaction_checkpoint | checkpoint.installed；history.revised 指明原 messages 下标和替换后缀，实际改写原前缀使 checkpoint 失效，纯追加保留 |
| goals | goal.updated；accepted／finished 同时携带该次发生变化的目标，保持目标引用与回合结果共同提交 |
| FrozenTurn（messages 之外全部字段） | 新日志使用 FrozenTurn<HistoryPosition>；其余字段及 teaching 准备资料保持原样 |
| FrozenTurn.messages | Committed{history_through_seq} 重建当时消息和压缩投影 |
| active_by_book | 独立选择元数据；pending_confirmations / pending_memory_ops / pending_governance_mutations 继续仅驻内存 |

`session_event::SessionProjection` 是纯折叠器，不触达 provider、工具或领域存储。`turn.finished.history_revision` 将失败截断与终态放在同一记录；event.seq 与活动 step_id、SSE last_seq 分别保留。上表为当前事件合同；早期导入分支已由 JL7 清理，此前验证结果保留为历史记录。

## 10. JL1：追加与折叠

输入：JL0 事件类型。产出：拟建 `crates/server/src/session_log.rs`，直接实现会话日志读写、序号分配和状态折叠。

范围：完整记录提交、末尾半行恢复、消息／目标／终态更新和 checkpoint 失效；用临时目录及可控写失败验证，不接入 provider 或工具。

验收：重开文件还原相同状态；半行末尾可恢复并继续追加，中部损坏明确报错；写失败不推进已提交状态，恢复后重试不重复提交。读取日志时工具／provider 调用次数为零。失败则修读写边界或 reducer，不以忽略损坏通过。

## 11. JL2：会话管理与路径

输入：JL1。产出：`user_storage_paths.rs`、`user_runtime.rs`、`host.rs` 与历史路由接入新存储的会话管理；明确保留 Presentation 原路径。

范围：列表、新建、选择、删除和用户运行时加载／释放；选择元数据与正文分别保存。删除继续受既有运行准入互斥约束。

验收：带冒号的旧 session_id 在 Windows 可用且逻辑身份不变；两个用户的同名会话独立；选择后重开仍可定位，删除不留下失效的活动选择；原演示可读取。若引用或目录改变，修路径解析，不重建用户成果。

当前实现：`session_store::SessionPaths/SessionStore` 管理每读者目录与选择文件；本地历史路由、服务 `workspace_registry` 的 chat/new/select、`RunAdmissions::delete_chat` 均沿原准入边界接入。`load_chat_storage` 已由 JL7 调整为直接创建或打开 `agent-sessions/`，不读取旧快照；旧快照保存入口返回 `SESSION_EVENT_WRITE_REQUIRED`。JL3/JL4 已接运行写入，本地切书与 workbench 交接调用 `ensure_book`。

选择保存只写小型元数据；选择缺失／失效时按现存聊天时间恢复，识别旧 `server-start` 占位及 RFC3339 时区。删除日志成功但选择保存失败时，内存立即移除会话并清除现场选择；重启仍按目录事实修复选择，不复活已删除会话。用户释放不重写整份历史，Presentation 继续解析原 `agent-history.presentations`。

验证（Windows，独立 TEMP/TMP）：`cargo test -p server session_ --lib` 26 通过；`cargo test -p server compaction_checkpoint --lib` 2 通过；`cargo test -p server agent_history --lib` 6 通过；`cargo test -p server mu6 --lib` 39 通过。`session_store` 4 项补验覆盖时间恢复修正。分组之间有重复样本，不合计为独立测试数。真实笔记工具结果、queued/claimed 原冻结输入、压缩位置及实际 Presentation 内容均使用现有运行／存储样本验证。

## 12. JL3：冻结输入与准入恢复

输入：JL2 会话位置引用。产出：`run_admission.rs`、`agent_run.rs` 的准入写入与恢复使用新日志；FrozenTurn 从指定位置恢复 messages。

范围：保持现有请求身份、202 接受前的保存边界、教学准备和控制库对账；目标更新使用当前会话事件，不复制所有聊天。

验收：冻结后追加新消息、切聊天或换现场，原回合输入仍一致；同一已接受请求不产生第二回合；未认领且输入完整的 queued 可恢复，claimed／已开始运行只呈现中断或待核对。检查实际写入 payload，确保没有每轮内嵌完整历史；发现重复保存则改为位置引用。

实现：`SessionStore::accept` 提交位置型输入，`turn.prepared` 在原教学回执后提交；`RunAdmissions::frozen_input` 按原位置加载 raw messages，runtime 从同一位置取得 checkpoint，统一执行模型投影。内存会话不重复保存整段 admission_input。恢复仍由既有控制库决定 queued／claimed，claimed 保留已提交消息与活动，不调用 provider／工具。本地未完成回合由 `session_runtime::recover` 终结；早期导入分支已由 JL7 移除，现只从 Committed 位置读取。

## 13. JL4：运行消息与压缩

输入：JL3。产出：runtime 与 server 之间必要的持久事件接入，替换 precommit／finalize 与压缩保存的整份历史提交路径。

范围：完整 assistant 消息、工具调用／结果、必要活动、usage、终态、历史修订及 checkpoint。沿用现有流式事件与控制逻辑，补齐持久边界，不另造运行循环。

验收：在工具开始后、结果已保存后、checkpoint 安装后、终态前分别中断进程，重开仍形成合法工具配对或明确中断；不会重复执行。带既有清理规则的工具 payload 在文件中不出现禁存内容；最终失败截断与 checkpoint 失效正确。长回答只增加消息／阶段记录，不逐 token 写文件。

实现：现有 `CompactionCheckpointSink` 增加完整消息／活动持久边界，server 的 `RunCheckpointSink` 将增量写入 `session_runtime`；完整 assistant 调用先保存，再保存工具开始，工具结果保存后才继续下一步。活动包含既有 usage 与阶段信息；SSE token 补丁不调用日志写入。工具正文在追加前清理，checkpoint 在清理后的源消息上生成。终态和最小必要消息修订共同提交；不确定写入先按原事件补存，终态重试不重复追加。任务目标更新、本地切书及 workbench 交接也使用当前会话日志。

验证（Windows，独立 TEMP/TMP）：server `session_` 32 项、`mu6` 47 项、`compaction_checkpoint` 2 项、`agent_history` 6 项通过；runtime `compaction` 19 项通过。MU6 的一个 ignored 项是由父测试显式启动的强杀子进程入口，四个退出窗口均已执行。分组有交集，不合计为独立测试数。新增覆盖原现场／聊天归属、导入冻结片段、丢失接受响应、教学回执复用、大历史追加量、部分写入／完整写后失败、实际工具结果、禁存内容、checkpoint 重开与失败截断。日志：`tmp/jl34-sessions.log`、`tmp/jl34-mu6.log`、`tmp/jl34-checkpoints.log`、`tmp/jl34-history.log`、`tmp/jl34-runtime.log`。

精确重试收尾：`cargo test -p server session_log --lib` 7 项和 `cargo test -p server jl3_admission_windows --lib` 1 组通过，分别见 `tmp/jl34-log-final.log`、`tmp/jl34-admission-final.log`。冻结现场中的映射允许解码后改变序列化字段顺序；事件身份按结构内容判断，不确定写入保留原事件与原始字节，重试不会改写成另一行。

## 14. JL5：阅读现场与领域关联

输入：JL4。产出：`RunScope`、来源绑定、Presentation 及教学准备结果在日志中的关联和历史读取投影。

范围：§5 已存在的事实与对象身份；复用原类型，保持 `memory.json`、`learning.db`、Presentation Store 的写入归属。

验收：提问后移动阅读位置或改变演示状态，旧回合仍指向发送时版本；检索命中但未进入回答绑定的片段不显示为回答来源；压缩及重启后来源仍可定位；原聊天目标与跨聊天 TutorSession 分别恢复。失败则修冻结／引用链，不能用当前现场补旧资料。

实现：`turn.accepted` 保存原 publication、冻结输入和 `domain.scene`；教学准备／实际交付后，`session_runtime::link_teaching` 读取 learning.db 的原 turn 回执，按 TutorSession 与 revision 追加 `teaching.linked`。运行实际 reader effects 经 `RunCheckpointSink::persist_effects` 追加 `effect.delivered`；回答终态追加已接纳的 `sources.bound` 与有效 Presentation 版本引用。原始对象正文仍由 Memory／Presentation／Learning Store 持有。`TurnDomain` 和事件证据在历史公开投影与 JSONL 重开时恢复，来源事件保留原 seq 供 JL8 使用。

验证：`jl5_bound_sources_survive_compaction_and_reopen` 实际安装压缩 checkpoint 后重开并解析原来源，拒绝候选引用；`jl5_presentation_delivery_and_frozen_followup_survive_scene_change` 区分草稿与实际交付，验证换现场／演示版本后旧问题仍读取发送时内容；`jl5_teaching_receipts_keep_original_revision_across_chats_and_restart` 验证原教学 revision／回执和各聊天目标归属，重开不产生新的学习判断。

## 15. JL6：成果处置回执

输入：JL5 稳定成果引用。产出：服务端处置命令及日志回执，`api.ts`、`App.vue`、对应成果控件改用服务端返回状态。

范围：沿用现有 keep／undo／dismiss 能力；保留 highlight 产生新对象时记录映射，布局与导航只在原合同允许的现场执行。

验收：成功保留／撤销后刷新或重启，状态仍在；同一处置重试不会多建标注；业务操作失败不显示成功；成果成功但回执失败能以真实身份核对或显示待核对，不能盲目重做。覆盖提议尚未应用即忽略的路径，不能把它记成已应用后撤销。

实现：`POST /agent/effect/dispose`（服务模式经原 workspace stamped 路由）接收 `{session_id, turn_id, effect_id, action}`，先提交 `effect.disposition_started`，执行原 memory／Reader 命令，再提交 `effect.disposed`。流式事件、终态和按钮共用稳定的原成果身份；highlight 保留预分配结果对象 ID 并保留原锚定／引用；布局与 minimap 回执引用实际操作结果。历史读取和 `App.vue` 从服务端回执恢复“已保留／已应用／已撤销／已忽略／处理失败／待核对”，不再以数组位置保存状态。相同请求返回原回执，不重复操作。

验证：六项 `effect_disposition_tests::jl6_*` 覆盖真实笔记／高亮对象、撤销及失败、部分／完整回执写失败、已保存对象对账、未执行意图、布局与 minimap 的真实提议和结果、导航位置与现场约束。`jl6_network_original_effect_is_private_and_disposition_survives_restart` 经真实运行交付和多人处置路由验证重启、私人归属与 provider 不重跑。前端六文件 53 项通过，含 App 按钮接线／重挂载恢复、stamped 请求和流式同 ID 更新；类型检查与生产构建通过。JL 定向组 24 项通过，强杀子进程入口由父测试执行；日志见 `tmp/jl56-focused.log`、`tmp/jl56-web-tests.log`、`tmp/jl56-web-build.log`。

相关回归：server 会话／MU6／Presentation／Tutor／stream／来源与历史分组共 118 项通过，runtime 压缩 19 项及成果／导航 10 项通过，分组与 JL 定向组有交集，不合计。server 9 个 ignored 项中，强杀子进程由父测试执行，其余为浏览器宿主或真实模型环境测试。记录：`tmp/jl56-regression.log`、`tmp/jl56-runtime.log`、`tmp/jl56-runtime-effects.log`。

限制：业务存储与会话日志没有跨存储事务。业务成功但回执未确认时，仅能以预分配结果对象或原笔记已提升状态证明成功；其余保留待核对状态，不自动重新执行。正式启动和导入分支／测试夹具清理由 JL7 完成，见 §16。此次未执行真实模型及浏览器端到端环境测试；界面接线通过组件集成测试验证。构建仍报告已有的大块资源提示。

## 16. JL7：新日志启用与备份恢复

输入：JL5／JL6 完整投影及新建 JSONL 会话样本。产出：启动路径直接创建或打开新日志目录，移除旧导入分支，现有 `reader_maintenance.rs` 支持新日志的备份恢复。

范围：清理 `session_event`／projection、`session_log` 及相关调用中的导入事件、Imported 位置和冻结片段读取；测试通过正式创建／追加事件准备会话。新日志为唯一聊天来源，旧文件保持原样，其他领域存储路径保持稳定。

验收：首次启动可直接创建 JSONL 聊天；已有新日志可重开；旁边存在旧 JSON 时，其内容不变且旧对话不进入新列表，旧活动引用按 §7 收口；新建会话覆盖压缩、失败回合、教学、演示和成果处置，备份还原后列表及对象链接可用。保留原准入／中断恢复回归，清理后不再生成或读取旧导入事件。


实现：`SessionStore::open` 创建私人日志目录，`load_chat_storage` 仅折叠 JSONL；桌面启动直接创建首个聊天，旧 JSON 及旧备份不参与恢复。服务现场沿既有 `WorkspaceRegistry` 清除失效聊天／演示选择，`RunAdmissions::recover` 关闭没有新日志回合的旧请求；删除聊天后缺失的冻结输入也返回原请求键关闭结果。新日志 queued／claimed 恢复规则保持不变。

维护：`reader_maintenance` 的递归快照原样覆盖日志与选择文件，SQLite 继续通过原备份 API 保存；新格式路径已纳入维护分类。恢复到独立根后，实际读取原发布来源、Presentation 版本、learning.db 教学回执及长期笔记，并比对聊天、失败回合、压缩 checkpoint、处置回执和选择文件。旧 JSON 字节保持不变，恢复读取不调用 provider、不增加学习判断。

测试夹具：去掉导入事件及 Imported 位置，使用 typed `session.created`／accepted／finished 等事件；桌面和 MU2／MU5–MU8 的故障注入与历史读取改用实际日志路径。保留原准入窗口、四个强杀窗口、终态补存及删除中断后的请求幂等覆盖。

验证（Windows，独立 TEMP/TMP）：`cargo test -p server --lib --no-run` 编译通过；生成的测试二进制执行 `session_ mu2 mu3 mu5 mu6 mu9 jl compaction_checkpoint agent_history agent_run_tests presentation_store tutor_loop`，随后复验修正的夹具与共用其入口的 MU7／MU8。按测试名称取最终结果，201 项通过、无未解决失败；明细 `tmp/jl7-results.json`，原日志 `tmp/jl7-regression.log`、`tmp/jl7-repair.log`、`tmp/jl7-workspace-final.log`。4 项 JL7 专项验收另见 `tmp/jl7-focused.log`，计入上述去重结果。

限制：10 个 ignored 入口中，JL4 强杀子进程已由父测试执行四个窗口；其他为浏览器宿主或真实模型环境测试。此次未执行 Linux 发布验收；桌面／服务整体发布仍按 JL10 推进。构建保留已有 ts-rs 属性解析警告。

## 17. JL8：回顾投影与接口

输入：JL5／JL6 的 JSONL 会话与成果回执。产出：拟建 `session_recap.rs` reducer 与只读路由，实现 §6 数据合同；可在 JL7 正式启用前用新建会话样本验证。

范围：当前聊天起点至 through_seq 的四组事实，所有条目带回合／事件证据引用；只消费既有记录，不新增模型总结或学习写入。

验收：同一范围产生相同事实条目；指定末尾之后新增的结果不提前出现；保留后撤销按选定范围正确显示；未处置显示待处理，仅有处置开始记录时显示待核对；未绑定检索结果不进入来源区；跨用户访问沿用历史权限拒绝。测试同时断言无 provider／工具调用及学习存储写入，失败则修 reducer 或路由职责。

实现：`session_recap::snapshot/project/resolve` 从 `SessionLog.at` 读取固定前缀，四区条目携带原 turn／event_seq，返回 `session_id/through_seq/through_at/generated_at`。读取不补存不确定事件；公开投影只选择问题、绑定来源、实际成果、目标与运行状态。当前可用性沿原用户和 publication 解析，后来删除或权限撤销不改变范围内的历史事实。本地与多人共用 `GET /agent/history/recap`。

处置补齐：为支持本节的连续处置验收，笔记／标注允许“成功保留 → 撤销”；撤销使用保留回执中的真实结果对象 ID。开始／完成事件各自保存，重复撤销返回原回执，失败或未确认保留不能进入该后继。回顾保留整条处置证据，在各截点分别显示待处理、待核对、已保留、已撤销。

验证：`session_recap_tests::jl8_*` 与 `mu6_tests::jl8_network_recap_keeps_private_owner_and_original_publication_without_execution` 覆盖固定范围、保留后撤销的原／结果对象、失败与忽略、目标、重开、不确定写入、私人隔离和权限撤销；JL5 来源压缩／重开及原演示版本样本已增加回顾断言。读取前后比对日志、Memory、Learning 状态与 Reader 现场，并核对 provider 调用次数。

## 18. JL9：回顾面板

输入：JL8。产出：拟建 `packages/web/src/components/SessionRecap.vue`，在现有聊天区域接入入口、面板、刷新及条目跳转。

范围：四区内容、空态、运行中状态、来源／成果不可用提示；定位使用原回合与原发布，Presentation 使用明确版本。

验收：在带笔记、原文引用、演示和未完成任务的会话中逐项跳转正确；打开面板不移动原文或演示；新记录出现后刷新才更新范围；切换聊天不残留上个聊天内容；窄屏可读取和返回聊天。失败则修界面状态与引用，不改变历史事实。

实现：`SessionRecap.vue` 提供聊天内入口、四区内容、全文展开、截点／更新时间、刷新、空态及不可用提示。`RightRail` 在用户点击后复核相同截点的当前可用性，定位原回合、来源、保留后的对象和明确演示版本；跨发布由 `NetworkApp` 打开原发布并选择原聊天，`App` 等历史加载后继续定位。切换聊天或卸载会丢弃旧请求响应，运行新活动不关闭从回顾打开的来源。

演示中断修复：交付事件成功、终态保存中断时，原 `presentation_api` 仅认可终态答案而拒绝已交付版本。失败测试已复现；读取和来源解析现同时认可真实 `effect.delivered`，保留候选不可读、原版本归属及公开内容校验。没有终态卡片的交付也可从回顾打开。

验证（Windows）：后端相关回归按最终测试名称去重 97 项通过，无未解决失败；前端六文件共 53 项通过；Web 类型检查与生产构建通过。Playwright 验证 390px／1440px 无水平溢出、四区可滚动阅读、手动刷新、返回聊天后草稿与焦点保留，2 项通过。日志：`tmp/jl89-regression.log`、`tmp/jl89-presentation-green.log`、`tmp/jl89-keep-undo-green.log`、`tmp/jl89-keep-undo-final.log`、`tmp/jl89-web-final.log`、`tmp/jl89-effect-web.log`、`tmp/jl89-web-build.log`、`tmp/jl89-browser.log`；截图 `tmp/jl89-recap-390.png`、`tmp/jl89-recap-1440.png`。

限制：后端回归中的 9 个 ignored 入口含由父测试实际执行的 JL4 强杀子进程，其余为浏览器宿主、演示运行环境或真实模型专项；此次浏览器使用既有移动工作区夹具。Linux 服务与 Windows 桌面的真实整体验收仍归 JL10。构建保留已有 ts-rs／KaTeX 提示及 Web 大块资源提示。

## 19. JL10：集成与发布

2026-10-02 Linux 发布进展：`ex13-jl-20261002` 已上线，两读者私有隔离、选定演示及回顾、重启与原生备份还原通过；一次真实 B=8 现场追问新增 19,280 字节，原日志前缀和其他聊天未重写。公网登录、9 本书、阅读刷新与 PDF 正常。详见 [发布步骤、证据及已知限制](Linux上线-EX12-EX13-JL.md)。Windows 桌面整体验收、不同长度历史矩阵与完整连续场景仍待收口，本片整体不标完成。

输入：JL7／JL9。产出：Windows 桌面与 Linux 多读者服务的集成记录、正式切换步骤和更新后的架构／代码链路文档。

验收围绕真实失败展开：直接新建 JSONL 聊天，发起含工具与演示的回合，保留一条成果，重启后回看，再备份还原；两名读者的记录、回顾及引用各自独立。验证准入、停止、重连与聊天历史交互，确认日志没有改变既有运行终止规则。

存储写入验收使用固定问答与不同长度的 JSONL 历史：记录这次追加的字节数及涉及文件，确认普通新消息提交没有写回旧消息或无关会话；允许 checkpoint 和明确的后缀修订具有各自大小。发现随已有历史重复写入时回到 JL3／JL4 修正，不以改名 JSONL 视为完成。

执行对应 Rust 单元／集成测试及 web 的定向 Vitest、typecheck 和 build；实际命令、结果及平台在完成切片时填写。既有部署放行规则仍按[多人实施方案](切片方案-Linux原生多人阅读与无Redis首版.md)执行，本任务表不能替代尚未完成的发布验收。

## 20. 已知限制

- 新版聊天列表与阅读回顾只覆盖 JSONL 中的会话，旧 JSON 对话原文件仍在原处。
- JSONL 与学习、记忆、成果存储之间没有统一事务；跨存储失败依靠原身份核对，无法确证时保留待核对状态。
- 首版回顾覆盖当前聊天，不提供跨聊天专题汇总、可编辑总结或掌握评分；普通问题是否真正理解仍由读者及现有教学证据表达。
- 首版启动需要折叠日志，长期使用后可能产生可测量的加载成本；持久索引和历史归档留待真实数据决定。
