# 附录 A 术语与状态归属

读到一个术语时，先问它标识什么、由谁修改、能保留多久。本附录把全书中容易混用的词放回这些问题中；具体控制流仍由相应章节展开。项目领域定义见[CONTEXT](../../../CONTEXT.md)，实现依据和读取日期见[资料索引](../SOURCES.md)。

## 材料与位置

| 术语 | 本书中的含义 | 展开位置 |
| --- | --- | --- |
| 内容基座、Book | 原文定位与公开知识成果构成可消费材料；读时Book加载正文、基座及相应旁路成果 | [第2章](../chapters/02-构建与阅读如何分工.md) |
| book_id | 内容基座身份，搬迁目录与内容改版是不同问题 | [第3章](../chapters/03-数据身份与状态归属.md) |
| 发布、PublishedBookRef | 一次确切的不可变发布，由book_id和publication_id共同标识 | [第19章](../chapters/19-多用户阅读与调度.md) |
| LID | 材料内部有层级、有顺序的位置路径；与材料身份共同解释，深度不固定 | [第4章](../chapters/04-原文定位与LID.md) |
| Span、UTF-16区间 | 规范原文的半开区间[start, end)，下标单位为UTF-16 code unit | [第4章](../chapters/04-原文定位与LID.md) |
| 语义图 | 概念、实体、断言及其关系；其锚点帮助寻找原文 | [第5章](../chapters/05-语义抽取与来源锚定.md) |
| BookStructure | 全书阅读结构，公开spine、key_stops、throughlines等成果 | [第6章](../chapters/06-全书结构与语义候选召回.md) |
| 旁路成果、sidecar | 随材料提供的补充能力文件；其存在、有效性和发布就绪分别判断 | [第8章](../chapters/08-构建恢复与成果发布.md) |

原文坐标以[Span与LidNode](../../../crates/base-schema/src/lib.rs)为基础；确切发布见[PublishedBookRef](../../../crates/server/src/published_library.rs)。同名LID可以出现在不同材料中，当前位置也不能替代原问题使用的材料版本。

## 构建工作与执行身份

| 术语 | 它承担的责任 | 展开位置 |
| --- | --- | --- |
| 窗口、window | 为抽取组织的一段材料范围，保留原文层级与预算约束 | [第4章](../chapters/04-原文定位与LID.md) |
| 模型输入片、model input slice | 从规范原文派生的单次模型输入，区分core覆盖与上下文重叠，不创建新的LID | [第7章](../chapters/07-模型工作单元与执行调度.md) |
| 工作单元、work unit | 按阶段合同可独立派发、提交和接纳的工作 | [第7章](../chapters/07-模型工作单元与执行调度.md) |
| 派发、dispatch | 将当前有效待办组织给执行器；历史派发列表不自动等于当前待办 | [第7章](../chapters/07-模型工作单元与执行调度.md) |
| 交接、handoff | 把执行责任和可取得的输入交给执行器；交接建立与完整输入交付有先后 | [第23章](../chapters/23-真实失败与工程改进.md) |
| 候选、candidate | 模型提出的待接纳结果；writer仍需检验字段、范围与相应合同 | [第5章](../chapters/05-语义抽取与来源锚定.md) |
| semantic_attempt | 在工作单元、输入和策略范围内的一次语义生成尝试 | [第8章](../chapters/08-构建恢复与成果发布.md) |
| lease_epoch | 执行租约代次，区分同一语义尝试下的接管和继续 | [第8章](../chapters/08-构建恢复与成果发布.md) |
| 候选纠错反馈 | 同范围内可定位字段错误的有界反馈，帮助下一次生成纠正格式 | [第23章](../chapters/23-真实失败与工程改进.md) |
| 收口、发布 | 根据正式成果和阶段条件确认完成，再形成读者能够加载的文件与发布事实 | [第8章](../chapters/08-构建恢复与成果发布.md) |

实现入口分别是[模型输入片](../../../packages/core/src/model-input-slice.ts)、[执行器会话](../../../packages/core/src/automatic-build-executor-session.ts)、[尝试与执行身份](../../../packages/core/src/automatic-build-task-store.ts)和[成果发布](../../../packages/core/src/automatic-build-publication.ts)。排查构建停滞时，应先确定失败发生在这些责任中的哪一项。

## 读者、聊天、现场与运行

| 术语 | 所有者与生命周期 | 展开位置 |
| --- | --- | --- |
| 用户私人状态 | 归读者所有，包含相应书籍的聊天、笔记、学习事实及跨书画像 | [第3章](../chapters/03-数据身份与状态归属.md)、[第15章](../chapters/15-笔记记忆与学习事实.md) |
| 阅读现场、ReaderWorkspace | 一个窗口或现场的Book、Reader与选中聊天等状态；重新绑定会改变其代际 | [第18章](../chapters/18-Rust宿主与并发边界.md) |
| 聊天、chat session | 连续追问与目标、来源和成果关联的持久归属 | [第16章](../chapters/16-会话日志与恢复.md) |
| 回合、turn | 一条被接纳的用户输入及其消息、活动和终态事实 | [第12章](../chapters/12-来源与流式交付.md) |
| Resident Run | 一次读者消息触发的完整Agent过程，可包含多次模型请求与工具调用 | [第9章](../chapters/09-一次有证据的回答.md) |
| RunScope | 固定原用户、材料、聊天、回合、现场与输入的运行归属 | [第18章](../chapters/18-Rust宿主与并发边界.md) |
| Activity | 实际模型请求、工具或动作的执行记录；用于观察发生了什么 | [第21章](../chapters/21-可观测性成本与效果评测.md) |
| Effect | 系统整理的领域操作效果，用于结果展示与会话关联 | [第16章](../chapters/16-会话日志与恢复.md) |
| Goal | 跨回合保留的任务要求与工作判断；工作计划描述推进情况 | [第13章](../chapters/13-持续目标与教学过程.md) |
| TutorSession、学习事实 | 有依据的教学过程及评估、纠正等私人学习记录 | [第13章](../chapters/13-持续目标与教学过程.md)、[第15章](../chapters/15-笔记记忆与学习事实.md) |

[RunScope](../../../crates/server/src/run_scope.rs)的check_user与check_scene提供了具体分界：原私人数据归属和当前现场有效性需要分别判断。[会话事件](../../../crates/server/src/session_event.rs)则将这些事实留在原聊天。运行结束、工具成功、效果被记录和任务完整成功具有不同判据。

## 证据、来源与演示

| 术语 | 本书中的含义 | 展开位置 |
| --- | --- | --- |
| 召回线索、locator | 帮助决定去哪里读取的位置或候选 | [第6章](../chapters/06-全书结构与语义候选召回.md)、[第9章](../chapters/09-一次有证据的回答.md) |
| 已观察证据 | 本轮取得并进入证据账本的规范原文，包含合法选区与相应读取结果 | [第9章](../chapters/09-一次有证据的回答.md) |
| TurnEvidenceLedger | 管理本轮观察区间与来源绑定的运行内账本 | [第12章](../chapters/12-来源与流式交付.md) |
| source_ref_id、来源引用 | 运行时分配的来源身份，连接标签、预览与原文范围 | [第12章](../chapters/12-来源与流式交付.md) |
| 回答草稿 | 通过当前公开规则但仍可能变化的展示内容 | [第12章](../chapters/12-来源与流式交付.md) |
| 持久回答 | 已进入相应聊天终态保存路径的回答与关联事实 | [第16章](../chapters/16-会话日志与恢复.md) |
| 演示候选 | 制作过程中待预览、观察与交付的内容对象 | [第17章](../chapters/17-可探索解释与演示版本.md) |
| PresentationRef | presentation_id与revision组成的确切演示版本引用 | [第17章](../chapters/17-可探索解释与演示版本.md) |
| 预览回执与后续观察 | 预览报告工程结果，后续模型采样实际接收截图等观察；两者共同参与交付条件 | [第23章](../chapters/23-真实失败与工程改进.md) |

来源相关类型与实际调用位于[orchestrator.rs](../../../crates/runtime/src/orchestrator.rs)；演示制作合同位于[presentation_author.rs](../../../crates/runtime/src/presentation_author.rs)。同样叫candidate，构建候选与演示候选属于不同流程，引用时应带上所属任务。

## 容量、费用与恢复

| 术语 | 计量或恢复对象 | 展开位置 |
| --- | --- | --- |
| 活动上下文 | 本次Provider请求实际携带的输入投影 | [第11章](../chapters/11-上下文组织与持续执行.md) |
| 活动上下文预算 | 模型窗口扣除输出预留和安全余量后可容纳的输入 | [第11章](../chapters/11-上下文组织与持续执行.md) |
| 压缩检查点 | 用于继续任务的历史摘要与覆盖依据；原始持久消息继续保留 | [第11章](../chapters/11-上下文组织与持续执行.md) |
| 累计usage | 已发生请求的用量记录，属于调用与费用观察 | [第21章](../chapters/21-可观测性成本与效果评测.md) |
| 活动Run份额 | 同时推进的任务数量约束 | [第19章](../chapters/19-多用户阅读与调度.md) |
| 模型许可 | 每次Provider调用占用的资源机会，等待和占用分别计时 | [第19章](../chapters/19-多用户阅读与调度.md) |
| 保存重试 | 使用原FinishedRun或已保存终态补齐提交；不同于重新执行工具 | [第16章](../chapters/16-会话日志与恢复.md) |
| 观察恢复 | 用快照与后续事件接回当前运行展示；不拥有模型执行生命周期 | [第18章](../chapters/18-Rust宿主与并发边界.md) |

容量见[ActiveContextBudget](../../../crates/runtime/src/auto_compaction.rs)，许可见[service_limits.rs](../../../crates/server/src/service_limits.rs)，保存恢复见[run_admission.rs](../../../crates/server/src/run_admission.rs)。

## 使用边界

本附录于2026-10-08回读相关定义与入口，既有行为验证沿用各章原日期。Effect为空不总能推出没有私人副作用，第18章已经记录相应失败组合；合法来源、结构接纳、Goal工作项完成和预览通过也各有语义边界。具体问题集中在原章与[SOURCES](../SOURCES.md)，术语索引不重新裁定历史实验。

继续查找实际调用，可进入[附录B：源码阅读路线](B-源码阅读路线.md)；字段与输入形状见[附录C](C-数据与接口速查.md)。

返回[全书入口](../README.md)。
