# Tutor 教练、参与活动与目标点击入口：实施方案

2026-10-29 接续：后续主 Agent 交付改造按 [ADR-0176](adr/0176-main-agent-planned-teaching-and-coach-guidance.md) 与[新切片方案](切片方案-主Agent计划执行与教练指导.md)执行，当前尚未实施。逐轮教学教练的现有代码只注入已就绪指导，前台不等待；本文早期的等待设计与图示属于历史安排。制作前表达教练仍保留同步指导。本文其余实施和验证记录按原日期理解。

创建：2026-10-21；修订：2026-10-22。状态：目标点击入口、逐轮自动教练与下轮指导的代码链路已接通，服务端联调和真实双轮场景正在验证。教练负责学习节奏与参与活动设计；调用由系统固定调度，主 Agent 不决定是否调用。当前实施范围见第 9 节，验证状态见第 10 节；架构决定见 [ADR-0174](adr/0174-teaching-coach-and-participation-planning.md)。

源码基线：HEAD 33c19c7fa8aec9a4f1031122bc177743b234b9fd 的当前工作树，包含既有未提交修改。以下路径与符号用于定位接点，实施以当时工作树为准。正式网站和 Windows 客户端按 ADR-0173 共用云端核心；现有本地开发 Reader 复用同一 Runtime 与状态端口。

## 1. 交付目标

用户通过点击表达本次学习目的，系统组织一项可开始的学习活动。独立教练在每个教学回合结束后，依据工作区大目标、当前材料、用户真实表达及本轮实际交付，为下一轮安排支撑、参与或收束。下一轮主 Agent 自动收到指导，并结合用户最新输入落实为原文、题目、讲解、演示和回应入口。初次开始阶段由系统自动生成起步指导，重开后继续实际已交付的活动。

首版贯通三类场景：当前章理解、当前章复习、围绕一个问题查证。查证先可在单份材料内完成；已有跨资料授权和来源合同能承接时，再对照工作区内的第二份材料。快速概览沿普通问答提供直接帮助。

用户可见的一次活动包含：为什么现在做、当前做哪一步、已经准备好的材料、回答或操作入口，以及就地帮助。用户贡献可以是回忆、预测、解释、应用、查证、比较或提出问题；教练依据其教学价值选择。

## 2. 现有架构与本次责任变化

| 职责 | 当前实现 | 本次变化 |
| --- | --- | --- |
| 工作区大目标与版本 | [workspace_goal.rs](../crates/runtime/src/workspace_goal.rs)、[learning_workspace.rs](../crates/server/src/learning_workspace.rs) | 继续作为全局学习方向，教练读取其当前有效版本 |
| 阶段意图、范围与教法 | [learning.rs](../crates/memory/src/learning.rs)：TutorSession、TutorSessionMode | 增加可选 LearningPurpose，表达本次阶段目的；保留阶段与大目标的区别 |
| 点击输入与接纳 | [tutor_continuation.rs](../crates/runtime/src/tutor_continuation.rs)：EntryAction、ResolvedInteraction；[interaction_input.rs](../crates/server/src/interaction_input.rs) | 增加目的选择动作，冻结目的、范围和真实补充，接入原运行 |
| 教学设计 | [teaching_design.rs](../crates/memory/src/teaching_design.rs)：TeachingDesign；[design.rs](../crates/server/src/teaching/design.rs) | 教练给出方向与活动规格；主 Agent 结合当前输入和实际来源，沿原接纳流程具体化为 TeachingDesign |
| 教学动作 | [tutor.rs](../crates/runtime/src/tutor.rs)：TeachingParticipation、TeachingMove | 主 Agent 采用自动注入的指导，关联 coach_ref，并保存有依据的调整 |
| 回应与帮助 | [TutorActivities.vue](../packages/web/src/components/TutorActivities.vue)、ResponseExpectation、TeachingResponseLink | 复用作答、提示、直接讲解、改答和暂缓；在主路径呈现当前活动 |
| 理解与接续 | [teaching.rs](../crates/memory/src/teaching.rs)、[learning_evidence.rs](../crates/memory/src/learning_evidence.rs)、[teaching.rs](../crates/server/src/teaching.rs) | 教练消费现有事实和解释；反馈仍沿原帮助归属和证据链保存 |
| 演示与实际现场 | [presentation.rs](../crates/runtime/src/presentation.rs)、[AgentPresentation.vue](../packages/web/src/components/AgentPresentation.vue) | 参与活动引用现有版本、观察与执行回执 |
| 独立交付审核 | [orchestrator.rs](../crates/runtime/src/orchestrator.rs)：tutor.step select 审阅链 | 继续检查这份实际交付；使用教练规格核对有价值的推理是否被提前代答 |
| 回合结束与下一轮执行 | [agent_run.rs](../crates/server/src/agent_run.rs)、[run_admission.rs](../crates/server/src/run_admission.rs)、[session_runtime.rs](../crates/server/src/session_runtime.rs) | 完成教学交付后登记并调度教练任务；下一轮执行前自动等待和装配对应指导 |
| 后台模型任务与费用 | [multi_user_review.rs](../crates/server/src/multi_user_review.rs)、[service_limits.rs](../crates/server/src/service_limits.rs) | 复用后台任务的 adapter 与 ReaderTask 归属方式，新增教练专用的持久任务记录 |

主 Agent 当前兼任教学设计和实施。实施后，教练拥有全局节奏和参与活动的规划责任，主 Agent 拥有取证、制作和现场回应责任。教练指导来自上一轮已发生的事实，用户当前明确请求优先；主 Agent 据此选择指导中的适用安排，必要时有依据地调整。每轮结束后的教练都会看到实际采用、调整与交付，继续修订下一步。Tutor 是承载这两个角色和现有状态机制的教学系统。

## 3. 目标点击入口与阶段生命周期

### 3.1 入口行为

| 文案 | LearningPurpose | 点选后的动作 |
| --- | --- | --- |
| 先了解重点 | discover | 以当前选区、章节或材料为范围直接概览，保持现有 Tutor 控制状态；不创建新的连续教学阶段 |
| 带我读懂 | understand | 沿显式启动链进入当前范围的阶段，教练决定首个问题和起步支撑 |
| 复习备考 | exam | 读取已有考试目标、范围和日期；以当前范围开始一项短诊断或回忆活动 |
| 探索一个问题 | explore | 有明确问题就据此开始；否则给出少量有来源的问题候选，点选后进入第一项查证或比较活动 |

“还没想好，先带我看看”采用 discover；“自己描述目标……”展开现有自由输入。自由描述保留原话，LearningPurpose 可以为空，不强迫将用户意图归类。上述枚举表达当前目的，不决定固定教法，不成为学习偏好或能力证据。

首次打开且无当前活动时，在 Reader 对话空态展示目的选项；当前活动存在时，优先展示活动及继续入口。TutorPanel 复用同一目的组件，不维护另一套候选文案。回到工作区时，优先展示待回应的问题和“继续上次学习”，旁边保留换目标。

### 3.2 默认范围与开启

范围优先来自有效选区，其次当前规范章节，再次当前材料。跨资料范围只来自当前工作区已选成员及现有授权。默认范围以可修改的短文案显示；只有范围缺失确实改变下一步时才提问。

understand、exam、explore 沿现有 GuideChapter 的显式启动行为完成开启、阶段建立和一次请求。新阶段由系统自动生成一次起步指导，再让主 Agent 开始；后续采用回合结束后的指导。Tutor 关闭时，入口明确说明“开启连续教学并开始”；仍沿原 SetEnabled／Start／Resume，不通过后台状态修改隐式开启。开启的作用范围沿现有全局 Tutor 约定展示。

同一材料和范围内，目的相同且阶段有效才复用当前会话。目的改变时沿 Start 建立新阶段，原阶段按现有规则暂停并保留。App 与服务端两处同章复用判断都需加入 purpose，不能只比 scope_refs。暂停阶段需用户明确恢复；已结束阶段按原规则新建。

阶段目的变化保留 WorkspaceGoal。只有用户明确修改工作区大目标时才形成新 goal revision，并沿现有机制停止旧阶段的主动推进。已有考试冲刺设置仍由原 ExamSprint 功能持有，日期与考试范围读取其权威记录；点击 exam 不创建另一套考试计划数据。

起步教练只为沿原授权流程建立或明确恢复的教学阶段运行；修改 WorkspaceGoal 本身不单独开启 Tutor。

### 3.3 输入合同

在现有 EntryAction 增加 LearnWithPurpose，AgentChatMeta 增加 learning_purpose。沿用 chapter_lid、选区引用、当前资料身份、client_request_id 和 expected_chat_head，不新建开始学习端点。

ResolvedInteraction 保存 purpose、服务器核验的范围、accepted_request 和 user_supplement。点击文案作为界面请求，只有用户真实补充属于用户表达。purpose 要进入归一化请求和持久接纳快照，同一请求恢复时还原原目的。

已知来源范围只表示定位，教练看到的历史原文保留实际读取身份，下一轮活动的具体来源承诺仍经过原读取和设计接纳。新字段从 Rust 合同导出前端类型，旧阶段缺省为无 purpose，沿原意图继续。点选不发送、清空或拼接用户尚未提交的普通草稿；已有选区草稿保持原资料绑定。

App 的 startLearningPurpose(purpose, scope) 先捕获当前范围，持续教学入口必要时沿 tutor.act 显式开启，然后只提交一次带目的的原 agent run。阶段建立、恢复或复用由服务端 freeze_prepare 接纳，避免先发送 Start 请求后又追加首回合。setConversationLearningGoal 已保留当前范围、purpose、明确约束与默认教法。

discover 必须在服务端跳过现有“已启用但没有阶段时自动 Start”的分支，并按普通请求完成，即使当前已有活动也不替换或推进该活动。此入口使用普通问答的就绪条件；不能因连续教学准备未完成而禁止用户先了解材料。

## 4. 教练如何设计参与活动

### 4.1 输入

宿主从既有权威状态组织 CoachContext，主 Agent 不另写一份学习者画像供教练相信。

1. WorkspaceGoal 及版本、阶段意图／purpose、明确约束和源回合的实际请求。
2. 当前资料与章节结构、当前 TeachingDesign、主线和返回位置；全局结构用于选择方向。
3. 截至源回合结束真实读过的相关原文和来源回执；区分可引用材料和仍需读取的范围。
4. 当前待回应邀请、源回合的原始用户表达、作答条件、实际帮助、已交付内容及演示现场。
5. 与当前关系相关的有效 LearningEvidence、用户纠正、允许带入的历史；保留事实与解释身份。
6. 已有教师工作记忆，特别是重复解释、错误修订和未完成帮助。
7. 当前可执行能力：普通回应、正式作答、材料读取、来源跳转及可用演示能力。
8. 本轮主 Agent 实际采用的 coach_ref、采用方式、调整依据，以及计划与交付的差异。

教练输入固定截至源回合的已交付事实，不包含尚未到来的用户回答。需要展开的记录按现有引用回读。课程全局方向由 WorkspaceGoal、BookStructure 和阶段关系承接；当前 TeachingDesign 保存这一阶段的关系与焦点。下一轮新输入由主 Agent 当场处理，并在该轮结束后进入下一次教练调用。

### 4.2 决策

教练先判断此刻该提供支撑、邀请尝试还是收束，直接复用 TeachingParticipation。邀请成立时，再选择具体活动。

| 活动 | 给用户留下的贡献 | 合适的材料和反馈 |
| --- | --- | --- |
| recall 回忆 | 从记忆中提取定义、条件、步骤或关系 | 作答前不展开答案；之后按原文核对遗漏，保留提示条件 |
| apply 应用 | 在新情境中选择方法、推导或求解 | 题目条件完整，准备可核对的解答；根据用户步骤提供反馈 |
| test_conjecture 检验猜想 | 寻找支持、反驳或限制条件并修订判断 | 给出可查的资料入口，接受有依据的不同结论 |
| predict 预测 | 在观察结果前解释将发生什么 | 给必要初始条件，先保留演示结果，随后对照预测 |
| compare 比较 | 区分前提、解释、证据或适用范围 | 准备可对照材料，先要求一个明确比较维度 |
| formulate_question 提问 | 找出未解决关系并形成自己的问题 | 提供必要背景，帮助缩小范围和找到查证入口 |

一次推荐一个下一步动作；讲解、原文与演示可以共同支撑它。当前仍有待回应邀请时，先等待其真实回应，并为影响下一步的少量情况给出条件安排。条件描述用户可能表达什么，不声称它已经发生。用户需要先看示范时选 support；当前请求完成且没有值得继续的活动时选 conclude。每轮都运行教练，继续当前安排也是有效结果。

### 4.3 活动规格

新增私有 ParticipationActivity，描述任务本身；支撑和留给用户的推理复用 TeachingParticipation.support／learner_inference，目标关系与来源复用 TeachingDesign。

~~~text
ParticipationActivity
  kind                  recall | apply | test_conjecture | predict | compare | formulate_question
  learner_task          用户当前要完成的具体动作，保留关键条件
  response_mode         conversation | assessment
  material_use          如何使用 design 中的原文、题目或教具
  evidence_to_notice    哪种实际表达会影响下一步教学判断
  adjustment            卡住时怎样减小一步；充分时怎样推进或收束
~~~

conversation 通过原 ResponseExpectation 收集解释、预测和开放判断。assessment 用于明确需要正式判题的活动，主 Agent 仍须在交付前按现有 AssessmentContract 准备并冻结评分依据、答案和帮助。选择活动类型本身不产生评分或掌握记录。

例如“检验一个猜想”必须写清猜想、材料入口、用户要查证的关系与可接受的证据，不能只输出“引导用户思考”。如果希望用户练习检索，关键词和找材料的过程就是任务的一部分；如果希望比较论证，主 Agent 先准备材料，让用户比较。

教练输出与教师准备的参考答案属于私有教学内容。公开目标、题面和当前支撑保留必要条件；尚未请求的答案、评分规则和提示不进入公开 DTO。实际曝光继续通过已有帮助与交付事实记录。

## 5. 系统自动调度、指导接收与最新输入

### 5.1 教学回合结束后的固定动作

一个教学回合指：当前教学阶段中，一次已接纳的用户请求及主 Agent 对它完成的实际教学交付。结束点取助手消息与对应交付事实已提交成功的位置。工具子调用、内部审核、流式片段和教练自身输出不构成新回合。

每个正常完成的教学回合都由宿主登记并调度一次教练任务，包括讲解、提示、作答反馈和收束。是否出现新题、主 Agent 是否调用教学工具，不影响调度。教练可以判断“保持当前安排”；不能据此跳过调用。discover 和普通非教学问答沿原路径，取消或失败且没有完成交付的运行不冒充已完成回合。

主 Agent 的回答按原流程交给用户，教练随后在受管理的后台任务中运行。主 Agent 无教练调用工具，也没有调用与否的参数。任务完成只保存私有指导，不自行再发一条用户消息，不触发另一个教练任务。

~~~mermaid
sequenceDiagram
    participant U as 用户
    participant H as 系统
    participant A as 主 Agent
    participant C as 教练
    participant M as 学习存储
    A->>H: 完成本轮教学交付 Tn
    H->>M: 提交消息、实际交付与教练待办
    H->>U: 本轮回答完成
    H->>C: 自动分析截至 Tn 的真实教学过程
    C->>M: 保存指导 Gn，注明源回合 Tn
    U->>H: 下一轮最新输入 Un+1
    H->>M: 接纳输入并固定对应教练任务引用
    Note over H,M: 若 Gn 尚未完成，先等待对应任务
    H->>A: 自动注入 Un+1、Gn 和当前状态
    A->>H: 结合最新输入落实或调整指导
    H->>M: 提交实际交付 Tn+1 与下一项教练待办
    H->>C: 自动生成下一轮指导 Gn+1
~~~

用户下一条消息可以在教练完成前到达。系统照常接纳消息，下一轮主模型采样前再等待对应任务，处理见第 5.4 节。

### 5.2 阶段首次开始与指导内容

新阶段没有上一轮指导时，由阶段启动流程自动登记一次 stage_start 教练任务。输入为已接纳的目的、初始请求、章节结构、允许带入的历史和已有实际来源；这是起步指导，不伪造已完成回合。已有阶段恢复时优先使用其最后一个完成回合的适用指导；确无记录的既有阶段沿同一系统起步路径补齐一次。

仅有范围定位时，起步指导给出读取需求和起步支撑方向。实际原文足够时，可以给出具体活动建议。主 Agent 后续负责读取材料和落实来源合同。首轮完成后仍自动运行一次 after_turn 教练；其余正常回合仅在结束后运行，不再每轮增加一次前置规划。

宿主组织 CoachContext，独立模型输出 CoachGuidanceDraft：

~~~text
CoachGuidanceDraft
  next_step          推荐的下一步：participation、activity、目标关系与支撑
  alternatives       少量由实际用户回应触发的调整方向
  design_candidate   可选的 TeachingDesign 修订建议
  needs_read         可选的待读取范围与用途
  reason             采用这项安排的事实依据
~~~

next_step 与 alternatives 中的 invite 都使用第 4 节的 ParticipationActivity。若当前存在待回应邀请，指导先指出需要等待哪项贡献；例如“若能解释条件则做变式，若仍不清楚则比较两个情境”。条件未被实际表达支持时不能执行推进，也不能将条件写成理解证据。

指导保存时不修改当前 TeachingDesign，不生成新邀请或正式题目。design_candidate 是供下一轮结合最新输入具体化的建议。下一轮需要修订设计时，主 Agent 沿既有 tutor.step design 和 teaching::design::prepare／accept 接纳；目标、来源、表示方式与理解依据约束保持原样。

### 5.3 上一轮指导与用户最新输入的关系

下一轮主模型首次采样前，系统分别注入：当前用户原话与真实操作、上一轮指导及其源回合、当前目标与待回应邀请。指导是有时间边界的教学安排，不能替代当前请求，也不能被当成用户已经表达的内容。

处理优先级：当前用户明确意图与已发生事实优先；仍适用的教练目标和活动规格用于组织回应。主 Agent 依据当前表达选择预案，未被预案覆盖的需求由它现场回应，并保存有依据的调整。回合后的教练读取实际过程，决定后续如何恢复主线或修订方向。

| 上一轮指导 | 最新用户输入 | 本轮主 Agent 的处理 | 本轮结束后的教练 |
| --- | --- | --- | --- |
| 解释充分就进入应用，否则补一个对照 | 用户只有结论，没有原因 | 围绕缺少的关系补支撑，不假设已掌握 | 看真实回应及已交付支撑，安排后续 |
| 下一步安排回忆 | “先完整讲给我” | 先讲解，保留实际帮助条件 | 据新的帮助情况调整回忆或应用安排 |
| 等待当前预测 | 用户给出反例或不同预测 | 核对证据与条件，允许改变原判断 | 修订猜想、参与活动或解释方向 |
| 继续原问题 | 用户提出相关新问题 | 先回应新问题，保留返回位置 | 决定继续探索还是返回主线 |
| 复习当前章 | 用户明确换阶段目的或大目标 | 沿原流程切换；旧指导转历史，新阶段自动起步 | 为新阶段持续安排 |
| 推荐一个演示或来源 | 实际读取发现材料不足、能力不可用 | 补读或缩小任务，说明未满足的条件 | 使用执行结果修订后续活动 |

正式作答先按原冻结合同评估；同题提示、解释和改答沿原活动处理。TeachingResponseLink 保持邀请归属，不为补一次帮助再发重复问题。这些回合正常结束后也会调用教练。

主 Agent 将采用的指导关联到 TeachingMove.coach_ref。存在实质调整时附私有 coach_adjustment，记录 reason 与 fact_refs，例如最新请求、作答或工具执行结果；自然改写、布局和措辞不要求重复写说明。调整不需要再次调用教练；也不能只写“我认为更好”就忽略仍然适用的安排。

### 5.4 下轮到达时，教练尚未完成

网络入口的 run_admission::admit 已在 freeze_prepare 时冻结部分教学上下文，实际模型执行稍后才发生。因此接纳输入时只固定所依赖的 coach_job_ref：同阶段最近已完成教学回合的任务，或当前新阶段的起步任务。不能当时读到空结果后把“无指导”永久冻结。

在 queued→claimed 之前检查该任务。pending／running 时本轮保留排队，由宿主继续调度；不占用主模型执行许可长等，不阻塞其他会话。后台教练允许处理这个待答回合所依赖的任务，不能照搬 multi_user_review 中“有 PendingAssistant 就跳过”的条件，否则双方会互等。

ready 时，系统持久保存本轮采用的 coach_ref，再将指导作为独立私有上下文片段注入 orchestrator 的教学上下文装配。原接纳请求和帮助快照保持不变；指导片段在首次采样前确定，并在本轮保持 TurnFrozen。历史指导的 source_turn_id 与当前 turn_id 不同是正常关系，不要求相等。

failed／interrupted 或任务超时后，明确提供 coach_unavailable 及原因，不静默换成更旧指导。可用的普通直接帮助和现有活动帮助继续运行；此状态下不声称已有新教练安排。费用或权限拒绝仍按全局规则停止相应模型调用。暂停、关闭或取消当前请求及时解除等待，不能以“等待教练”阻止用户停止。

本轮开始采样后不再热替换指导。随后完成的迟到结果保留源回合身份；下轮选择自己前驱回合的任务，不能让较旧结果覆盖较新的指导。

### 5.5 实施与交付审核

主 Agent 负责来源读取、题目与参考解答、演示、公开表达及 ResponseExpectation／assessment。采用教练目标与活动规格，结合最新输入落实适用安排。目标关系或表现类型需要变化时，依据当前请求和来源修订既有 TeachingDesign；实际设计接纳后的下一采样边界继续生效。

系统保证每轮自动调度、下轮自动装配及采用事实可追踪。当前有效 guidance 注入后，教学 Move 关联它；校验采用引用是否属于本轮已经装配的指导，并核对阶段与目标适用性。指导的源回合和当前 TurnBound 分别保留，实际 TeachingDesign 仍在当前回合按原合同接纳。

现有独立交付审核同时看到本轮最新输入、已注入指导、调整依据与具体教学稿。它判断当前回应是否合适、活动是否可完成、有价值的推理是否被提前代答；不能只因用户要求导致偏离上轮建议就否决。不新增一次审核调用。工具指导和 PRESENTATION_DESIGN_REQUIRED 继续指向设计具体化流程，角色说明改为“采用指导并回应当前输入”。

普通表达继续沿原理解解释与证据链保存，正式作答沿冻结评分流程。教练的预案、教师讲解和目的点击均不产生学习者掌握事实。回合后教练同时读取原话、实际帮助和有效解释，不能用之后出现的帮助改写之前的作答条件。

## 6. 持久状态、公开投影与恢复

### 6.1 指导与任务分别记录

TeachingDesign 继续是已接纳的阶段关系与焦点。新增 CoachGuidance 保存教练依据与建议：

~~~text
CoachGuidance（宿主封装）
  coach_ref / coach_job_ref
  origin               stage_start | after_turn
  source_binding       工作区、阶段、目标版本、资料范围
  source_ref           阶段建立记录，或已完成的源回合
  delivery_refs        after_turn 对应的实际交付
  base_design_ref      可选的已有设计引用
  basis_refs           原话、帮助、交付与理解解释的引用
  next_step / alternatives / design_candidate / needs_read / reason
~~~

新增 TeachingFact::CoachGuided，payload 沿 TeachingEvent 的 Value 存储；具体类型与模型 schema 放在 runtime/tutor_coach.rs。memory 不反向依赖 runtime，不再定义第二套 TeachingParticipation。CoachGuidance 的依据可包含帮助和交付；TeachingDesign.basis_refs 仍只使用原合同允许的学习者表达与有效理解依据。

ReaderTask 是费用归属类型。每用户 LearningStore SQLite 的 schema 4 已新增 teaching_coach_jobs 和 teaching_coach_turns：前者保存阶段、origin、源记录、固定输入、目标与模型绑定、状态、尝试记录及结果／错误，后者保存每个已接纳回合的任务依赖和首次消费上下文。按“工作区＋阶段＋origin＋源记录”唯一识别任务，复用现有引用。

教学交付事实保存后在共同 record_user_delivery 路径确保 pending 待办，恢复路径补齐已保存交付缺失的任务；重复完成通知取回同一任务。成功时指导事件与 ready 状态在同一 LearningStore 事务保存。任务状态区分 pending、running、ready、failed、interrupted、cancelled，升级沿原 LearningStore 事务完成。

本轮采用关系由执行上下文保存的 coach_ref、TeachingMove.coach_ref 及实际 MessageDelivered 连接。没有采用指导的普通帮助保留明确的不可用状态，不伪造引用。建议、已接纳设计、已选择动作和实际交付分别存在；只有实际交付产生用户待回应活动。

### 6.2 公开内容

现有 Tutor 活动 API 只增加当前目的、活动说明、公开题面及既有回应／帮助入口。coach_ref、coach_adjustment 及原始 CoachGuidance payload 纳入现有公开投影和私有字段清理。“为什么现在做”来自审核后实际交付的自然语言。原文、演示和帮助沿原引用显示。

普通邀请的题面由实际回答承载，活动区承载输入与帮助，移动端保持一项当前动作可见。“帮我找个起点”“先完整讲给我”“暂缓”复用原操作。正常回合后教练运行不挡住已经交付的回答；用户快速提交下一条而需要等待时，使用现有运行状态展示简短的下一步准备状态。

TeachingContinuation 保持 explain／compare／observe 的既有合同，参与活动通过 ResponseExpectation／assessment 实施。探索入口无明确问题时，起步指导可建议先提供背景，再复用 TeachingContinuation／TeachingChoice 给出一个推荐问题与一个可选方向。用户下一次点选时，主 Agent 同时收到该选择和回合后指导，据此落实活动。选择本身不算已经回答问题。

### 6.3 中断、重试与阶段变化

新消息到达本身不取消前驱教练任务。后台使用源回合固定上下文，下一轮新原话由主 Agent 处理。暂停、关闭 Tutor、取消相关任务或阶段失效时停止适用任务；取消当前等待中的用户请求不改写先前已完成的教学交付。用户仅关闭页面不等于停止已在云端运行的任务。

服务重启后，pending 的已提交任务可继续；ready 直接复用；running 且调用结果不明的任务标记 interrupted，不自动当作未调用重放。持久层已有 retry_coach_job，复用任务身份并记录新尝试；专门的 Reader 教练重试入口尚未接入。完整结果已在内存而仅保存失败时，worker 先重试保存，不重新付费生成。已产生的模型用量按实际记录，不能保证外部模型调用在崩溃边界绝对只发生一次。

目标、阶段或来源范围改变后，旧指导保留历史身份，不成为当前安排；新阶段自动生成起步指导。旧任务迟到完成只保存其原身份，不能写入当前设计或覆盖新阶段指引。重开时从实际已交付活动恢复，后台建议不制造“上次尚未答完的题”。

## 7. 模型、费用和上下文装配

教练采用独立提示和结构化模型调用，首版复用服务端当前模型配置。origin 区分起步和回合后任务，模型用途统一为 tutor_coach。每次调用保留任务、源回合、工作区、账号、用量、耗时与结果的关联。

网络后台任务复用任务 adapter 与费用包装，按 ChargeScope::ReaderTask 记录每个 coach_job_ref／attempt，ChargeAttribution 关联教练任务和学习工作区，任务再指向源回合；ReaderRun 原账目在前台运行结束时正常结算。网络调用使用取消、权限、LimitedAdapter 和 ModelSpendPort；本地开发宿主复用同一指导与状态机制。任务持有独立取消与运行状态，网络调用前沿现有机制检查账号、工作区和额度。

请求沿 AgentRequestPlan::for_ad_hoc 设置预算；结构化解析失败或取消时，已发生用量仍按实际记录。任务执行用受管理的后台调度；复用账号发现和调用包装，不照搬其他后台任务的触发频率与 PendingAssistant 跳过规则。ReaderTask 费用与源回合关联，在用户费用汇总中可解释为“安排下一步学习”。

下轮上下文仅带当前选中的 CoachGuidance、相关设计及必要原始依据。任务在 admission 后才完成时，由执行准备层单独装配指导，不改写已冻结的用户输入；采用引用随本轮保存，重试沿用。指导一旦进入当前模型采样就保持不变，历史记录按引用回读。

指导模块明确职责和优先级：教练负责节奏与活动建议，主 Agent 对当前用户输入作实际回应，系统固定调度教练。指导不是用户原话，也不是必须照抄的下一句。主 Agent 的实际调整会在回合结束后自动进入教练上下文。

## 8. 典型端到端实例

### 8.1 读懂当前章：预测与条件辨析

用户点“带我读懂”。系统完成显式开始并自动生成起步指导，建议围绕速度、时间与路程的条件安排预测。主 Agent 读取实际原文，将任务具体化，给出条件和预测入口。

第 1 回合交付后，系统自动调用教练，得到指导：“等待用户预测；若忽略时间条件，比较时间不变与时间减半；若能说明条件，进入变式应用。”此时用户尚未回答，系统不记录其掌握情况。

第 2 回合用户说“速度翻倍，路程总会翻倍”。主 Agent 自动收到这句原话和第 1 回合指导，采用条件对照，要求用户指出差异。回合结束后，教练自动读到错误判断、实际对照以及尚待回答的问题，修订下一轮安排。

第 3 回合若用户说“先直接讲清楚”，主 Agent 立即解释，并记录此次对指导的调整及实际帮助。教练随后据此安排更小的应用；若用户说明“还要看用时”并给出理由，则按真实表现推进或收束。重开后显示实际已经交付且尚未完成的问题。

### 8.2 备考：回忆后决定补讲或应用

用户点“复习备考”，系统使用已有考试范围或当前章，自动起步指导建议回忆一个重要结论的条件。主 Agent 准备材料，作答前不展开答案；本轮结束后的教练保存按回答调整的方向。

下一轮用户漏掉关键条件，主 Agent 结合真实回答落实条件辨析；若回忆充分，则使用指导中的变式应用方向。正式评分仍使用原 AssessmentContract，提示后的回答保留帮助记录。每次反馈交付结束，系统都自动调用教练，包含只给提示或完整讲解的回合。

### 8.3 探索：由用户修订猜想

用户选择“探索一个问题”。主 Agent 按起步指导展示背景与问题候选，回合后教练据实际材料建议查证方向。用户下一轮点选某个问题，主 Agent 将选择与指导结合，提出可查证的猜想和具体证据寻找任务。练习比较时准备两段原文，练习检索时提供起点，由用户选择查找方法。

用户发现反例时，主 Agent 先核对适用条件，允许修订或否定猜想。需要第二份材料则沿工作区现有检索与授权读取；实际读到什么、用户怎样判断、老师如何回应，都会在回合后进入教练，调整后续探索。笔记可保留用户修订后的判断与未决问题。

## 9. 实施切片与依赖

本轮按用户要求先贯通“点击目的 → 自动起步 → 每个成功教学回合后的教练 → 下一轮自动收到指导”。P1–P4 的代码链路、P5 的实际反馈回流和 P6 的入口已接通；其余活动呈现细化、专门重试界面和真人使用观察分别保留以下状态。工程验证不代替模型教学质量或学习收益。

| 切片 | 工作与主要接点 | 完成判据 |
| --- | --- | --- |
| P1 目的输入与阶段接纳 | LearningPurpose、TutorSession、EntryAction／ResolvedInteraction、interaction_input、run_admission、App 同章复用 | 零手写 prompt 点选目的进入正确范围；同范围换目的建立正确阶段；重试保留原目的和草稿 |
| P2 指导合同与持久任务 | 新 runtime/tutor_coach.rs、TeachingFact::CoachGuided、LearningStore 的 teaching_coach_jobs、RunContext／ResidentStatePort | 源回合与未来指导身份清楚；任务、尝试、结果和设计分别保存；建议不等同交付 |
| P3 自动起步与每轮后台调度 | 共用 record_delivery、阶段启动、任务 worker、ReaderTask 费用包装、恢复 | 不依赖 Agent 工具也每轮运行；同回合不重复排队；起步、取消、重启与费用可追踪 |
| P4 下轮自动接收与现场实施 | admission 的任务依赖、执行前等待、TurnFrozen 指导片段、TeachingMove、原设计／审核链 | 用户输入先接纳，模型开始前拿到正确指导；按最新输入落实或调整；失败不伪造指导 |
| P5 反馈调整与阶段接续 | learning_opportunity／continuity、TeachingResponseLink、LearningEvidence、目标与阶段切换 | 不同真实回答导致不同下一步；每轮实际调整反馈给教练；切换和重开不误用旧指导 |
| P6 默认入口与完整界面 | 共享目的组件、RightRail、TutorPanel、ConversationLearningGoal、TutorActivities、NetworkApp | 首次点选到活动反馈可自行完成；当前活动、返回继续、手机界面与草稿保留连贯 |
| P7 真实模型、使用观察与成本 | 复用教学 live harness、浏览器场景和费用追踪 | 三类实例走通；验证逐轮自动调用与接收；记录后台耗时、下轮等待及实际教学变化 |

当前状态：P1 目的及范围接纳、P2 持久任务与指导合同、P3 宿主自动执行、P4 依赖等待与首次采样注入已实现。P5 已将实际交付、指导采用与调整放入下一次固定输入，并以真实双轮运行验证明确求助会改变后续安排；P6 已在默认入口、TutorPanel 与当前阶段入口复用 LearningPurposeEntry，原活动和帮助界面继续承接。P7 的本轮链路联调、真实双轮运行、修复回放和定向回归已完成；三类教学场景的完整观察和真人使用尚未完成。

### 9.1 现有文件的具体修改责任

| 文件／模块 | 修改责任 |
| --- | --- |
| packages/web/src/components/RightRail.vue、TutorPanel.vue | 默认目的选择、当前任务与自由输入衔接 |
| packages/web/src/App.vue | guideCurrentChapter／tutorAction／startTutorRequest 共用目的启动；同范围复用加 purpose |
| packages/web/src/api.ts、network-client.ts、generated/ | 目的元数据传输与生成合同；保留原请求恢复 |
| crates/runtime/src/tutor_continuation.rs | 新点击动作、输入来源与目的解析合同 |
| crates/server/src/interaction_input.rs、run_admission.rs | 核验 purpose 与范围；保存教练依赖；queued 任务未就绪时暂缓 claim；准备执行时装配指导 |
| crates/memory/src/learning.rs、tutor_coach.rs | 阶段目的；learning schema 4；教练任务、回合依赖、结果事务与恢复 |
| crates/runtime/src/tutor_coach.rs（新增） | 私有指导类型、条件预案、模型提示与结构化调用 |
| crates/runtime/src/tutor.rs、orchestrator.rs、run_context.rs | 自动指导上下文、采用与调整记录、原设计具体化和交付审核 |
| crates/server/src/teaching.rs、teaching/design.rs | 从真实交付冻结输入并确保教练待办；保留原来源、设计和活动校验 |
| crates/memory/src/teaching.rs | 教练指导事件、源回合与交付引用的读写 |
| crates/server/src/teaching_coach.rs（新增）、multi_user_host.rs | 教练后台执行和 host 生命周期；沿用户 LearningStore 发现待办，复用任务 adapter |
| crates/server/src/agent_run.rs、session_runtime.rs、workspace_registry.rs | 本地同一结束挂点、等待与取消；两个状态端口接指导读写及恢复 |
| crates/server/src/service_limits.rs、model_spend_store.rs、run_events.rs | 复用任务容量、权限与计费，教练用途和源回合关联 |
| packages/web/src/components/TutorActivities.vue、ConversationLearningGoal.vue | 当前动作、就地求助、暂停与恢复摘要 |

工具 schema、Rust 序列化和前端生成类型已随链路更新。架构、代码链路和 CONTEXT 记录当前职责，验证结果统一在第 10 节补记。

回合结束的共同接点为 teaching::record_user_delivery：它在 Completed、有实际交付且无 delivery failure 时保存交付事实，然后调用 teaching_coach::after_delivery。本地 agent_run 和网络 run_admission 共用这一接点；repair_saved 与 recover_user 补齐已保存交付缺失的任务。ensure_turn 为已接纳输入固定前驱任务，或登记 stage_start；prepare_execution 在主模型开始前冻结 ready／unavailable 上下文。

下一轮网络等待放在 RunAdmissions::run_one 的 queued→claimed 之前；本地放在 RunCoordinator 开始模型执行之前，等待可取消，不持有模型许可或 AppState 锁。后台账号发现与停止方式参考 multi_user_host、user_registry；只复用这些生命周期能力，教练任务有自己的触发合同。

## 10. 验证与会改变实施的失败

### 10.1 当前验证记录

本轮定向回归已全部通过。Runtime 覆盖一次结构化调用、缺活动的邀请拒绝、host 绑定、首次／后续采样指导唯一性与私有投影；Memory 覆盖持久状态、结果事务、未知调用恢复、消费冻结与恢复补登顺序。各组独立执行，筛选范围可能重叠，不合计为唯一测试数。

| 验证范围 | 结果 |
| --- | --- |
| Server `teaching_coach` | 12 passed：本地／网络两轮、等待、失败、阶段切换、恢复与保存重试 |
| Server `tutor_` | 125 passed，19 ignored |
| Server `purpose_` | 4 passed |
| Server `teaching_design_` | 36 passed，2 ignored |
| Server 章节标题来源回归 | 1 passed |
| Runtime `tutor_coach` | 9 passed；1 项真实输出回放另行显式运行 |
| Memory `tutor_coach` | 5 passed；此前相关存储升级与目的保存回归也通过 |
| 真实输出离线回放 | 1 passed，0 次网络调用 |
| Web | 类型检查与生产构建通过；相关组件 83、App 启动 97、网络 24 项通过；实际 400 修复的定向测试 12 项通过；桌面／移动端浏览器 4 项通过 |
| 最终服务端构建 | 全部代码修复与回归完成后，`cargo build -p server --bin server` 通过 |

默认忽略的其他测试未在本轮执行，不能视为通过。

[真实 run02 记录](../evals/semantic/quality-results/teaching-coach-20261022-02/review.md) 已完成本地 Host、真实浏览器与 Provider 的两轮教学。两次 after_turn 均达到 ready；第二轮在首轮教练完成前接纳输入，待其 ready 后才首次采样，消费精确的前驱 coach_ref。用户要求先讲解、暂不做题，主 Agent 将推荐 invite 改为 support 并实际交付对照解释；本轮结束后的教练据真实请求与交付改为 conclude。

run02 原始整体结果仍为 failed_checks：bootstrap 模型返回 conclude 分支中的空 support，导致首次指导失败；第二轮虽然正确调整教学，专门的 coach_adjustment 字段未保存；入口标题还暴露了把规范 LID 当章节序号的问题。原记录保持不变。相应代码修复为：只在教练输出边界清理 conclude 的空串／null 占位字段；选择方式改变且未填调整时，从该动作已有 reason 与事实引用复制调整；接纳标题与 UI 使用同一 manifest 标题来源，教练提示明确 LID 只是定位。另将最新 after_turn 的选择按源回合 binding 序列确定，防止恢复时补登旧任务覆盖较新指导。

原 provider 输出与第二轮真实动作的[离线回放](../evals/semantic/quality-results/teaching-coach-20261022-02/offline-runtime-replay.json) 已通过：实际 execute_coach 接收第 1、11、24 次保存的教练输出，实际 bind_move 将第二轮已有的调整理由、事实引用与 coach_ref 一同保存。回放未联网，未改写 run02 的失败状态。标题来源与任务补登顺序分别由新增回归验证；修复后的完整链路由本地／网络服务端两轮测试验证，没有再次付费执行整套真实模型运行。

run02 共 24 次模型请求、1,626,245 tokens；其中教练 3 次（含失败 bootstrap）、133,959 tokens、累计 41.70 秒。首轮 after_turn 用时 22.46 秒，用户提前提交的下一轮在它 ready 后才开始主模型；两轮完整响应约 185.7 秒与 206.1 秒，主要耗时还包含主 Agent 和交付审核。此次为本地验收，金额未知，未验证网络账号实扣。脚本输入是模拟学习者反馈，尚无真人学习收益结论。

### 10.2 失败与后续验证

以下覆盖用于定位本次职责变化造成的具体失败，已有通过项只在代码变化或新疑点出现时重跑。

| 要发现的具体失败 | 验证位置 | 失败后的动作 |
| --- | --- | --- |
| 点目的仍需手写 prompt、重复发送；同章换目的继续旧阶段 | 入口组件／interaction_input／阶段测试 | 修正单次发送、接纳和复用条件，再检查完整首次入口 |
| discover 在 Tutor 已启用但无阶段时自动启动教学 | 服务端输入接纳与阶段测试 | 跳过自动 Start，保持普通请求路径 |
| 纯讲解、提示或没有调用教学工具的教学回合未触发教练 | 共同 record_delivery 与本地／网络回合测试 | 依据完成的教学回合调度，移除 Agent 自行触发条件 |
| 同一交付重复通知引起重复模型调用；答案已保存但任务丢失 | LearningStore、repair_saved 和 worker 测试 | 同源回合唯一登记，恢复补齐明确缺失的待办 |
| 起步没有指导，或每轮意外多运行一次前置教练 | 阶段启动与多轮调用轨迹 | 保留一次系统起步，正常回合仅回合后调度 |
| 下一轮已入账，指导未就绪就启动模型；或双方永久互等 | admission 冻结、queued→claimed 和后台 PendingAssistant 场景 | 固定任务依赖，执行前装配，允许依赖任务运行 |
| 教练把未来答对当事实，或主 Agent 用旧建议覆盖最新求助 | 条件指导、最新输入与交付审核场景 | 修正时间身份、当前请求优先级及调整记录 |
| 教练没看到源回合的真实回答、已展示帮助和实际调整 | CoachContext 固定输入与多轮测试 | 在完成交付后固定事实，并带入采用／调整记录 |
| 教练计划替代实际交付，重开出现未曾发出的题 | LearningStore 与恢复测试 | 修正计划、MoveSelected、MessageDelivered 的投影 |
| 回忆答案提前公开，或开放猜想只接受模型结论 | 真实内容样例与已有审核 | 修改活动设计／材料准备和审核消费，不增设评分档案 |
| 看提示后被记为独立完成；点击被当理解 | 现有 assessment／learning_evidence 测试 | 修复帮助归属及输入身份传递 |
| 换阶段后使用旧指导；迟到结果替换正在采样的指导 | 阶段切换、指导冻结与恢复测试 | 按源回合和阶段选择，当前轮固定，新阶段自动起步 |
| 教练失败被当成成功，重启盲目重放不明调用 | 任务失败／interrupted／ready 恢复测试 | 保存真实结果状态，显式不可用，重试记录新尝试 |
| 后台漏计费用或写回已结算 Run，取消后仍应用指导 | ReaderTask 归属、额度、取消与追踪测试 | 使用任务 adapter 与独立费用记录，终止适用状态 |
| 新教练增加等待，却没有改变教学行为 | 同材料真实模型对照 | 缩减上下文、消除重复调用，或修正决策与实施连接 |

复用测试入口：Runtime 的 teaching_design_loop_tests、tutor_continuation_tests；Server 的 agent_run_tests、session_runtime_tests、tutor_tests、tutor_loop_tests、teaching_design_tests、tutor_adaptation_tests、tutor_continuation_tests、tutor_building_blocks_tests 及 w4_workspace_goal_tests，并扩展 run_admission 与任务计费的相邻测试；Web 的 chapter-reading-a、tutor-assessment、conversation-learning-entry、interactive-teaching-om6 场景。以当时存在的文件及过滤名运行，新增 coach 与 purpose 用例放在相邻测试中。

真实模型验证沿既有教学 live harness，使用第 8 节三类材料和少量固定反馈，保留每轮输入、自动任务、指导、采用或调整、实际交付。至少包含一次提示回合、一次明确改向及一次用户早于教练完成而提交消息的场景。已有未修改能力不重复扩大测试；模型场景结果与真人使用观察分别记录。

成本与等待记录：从用户提交到首份可用帮助的时间、交付到指导 ready 的耗时、下一轮实际等待指导的时间、起步和逐轮教练调用次数、ReaderRun 与 ReaderTask 合计的模型用量及费用、失败和重试。对照使用同材料、相同用户输入与同等模型配置，保留原路径基线。没有数据前不预设加速或学习收益百分比。

首轮真人观察沿现有章节方案的形成性工作，使用读者自己的卡点，从默认未开始状态进入。观察能否自行选择目的、开始尝试、获得有效帮助、改变路线与重开接续；根据每个实际失败修对应环节。后续独立应用或隔时回忆用于判断学习变化，点击量不代替它们。

## 11. 已知限制与后续条件

每个正常完成的教学回合增加一次独立教练调用，阶段起步另有一次。后台执行可以与用户阅读、思考重叠；用户很快回复时仍可能等待。实际教学收益、等待和费用在 P7 验证；优化上下文与执行开销时保持逐轮自动调用这一行为。

回合后指导天然没有见到下一句用户回答。它给出目标、活动和条件预案，主 Agent 对最新输入承担现场回应责任；教练在随后回合结束时看到真实结果。每轮参与不等于每轮换题，也不等于主 Agent 必须照搬预案。

教练失败时已交付回答保持成功，下一轮明确指导不可用并保留普通帮助与原停止入口。专门的教练重试界面尚未接入，持久层重试能力已具备。模型任务跨服务重启的未知结果沿第 6.3 节处理；每回合登记一次任务不等于外部服务在所有故障下恰好执行一次。

Socratopia 借鉴依据为 2026-10-18 本地发布包静态还原：教练分析最近对话、区分教师问题与学习者需要、给下一轮方向。其本地证据见 [既有记录](performance/tutor-agent-memory-20261018/review-rules.md)。本方案进一步使用 Understand Book 的大目标、章节结构、实际来源与证据接续，效果由本项目验证。

正式题目仍受可用来源和评分合同约束；来源不充分时先补材料或以普通查证活动继续。跨资料完整能力以当前工作区授权、目标来源与实际读取合同为准。未接入材料和不可执行的页面能力不能成为活动承诺。

后续按第 9 节补齐交互细化与观察；发布和真人学习收益分别按实际结果记录。
