# Tutor 预构建不阻塞教学切片方案

发布记录（2026-10-09）：T15–T18 随 INV9 同版上线，原版本因缺少必要接纳回执阻塞旧书。随后按用户要求将 Tutor 准入改为直接检查已有基础成品；此修复已于 20:32:02 HKT 部署，旧书无需重建或重新导入，见 [上线记录](performance/tutor-artifacts-deployment-20261009/README.md)。T19 仍待验收。见 [INV9 发布运行单](邮箱账号-INV9发布运行单.md)。

日期：2026-10-09。状态：T14 文档、T15 就绪与目标绑定合同、T16 来源教学与首轮上下文、T17 使用观察与反馈适配已完成；T18 连续使用与界面已完成；T19 待实施。Server 启动、回合准备及本机/发布书库界面使用同一 Tutor 可用就绪。

决策：[ADR-0158](adr/0158-book-structure-ready-tutor-and-optional-teaching-assets.md)。承接：[原 Tutor 方案](切片方案-Tutor全局模式与教学闭环.md)、[ADR-0141](adr/0141-global-tutor-control-and-session-ownership.md)、[ADR-0142](adr/0142-grounded-teaching-map-and-whole-source-readiness.md)、[ADR-0143](adr/0143-teaching-trace-assessment-and-learning-evidence.md)。术语以 [CONTEXT](../CONTEXT.md) 为准。

## 0 范围与领域边界

Pass1 基座、discourse 与 BookStructure 成品可用后，用户可以使用同一个 Tutor 完成讲解、追问、演示、理解检查、反馈适配和连续学习。正式对象、认知素材与教学发布是可利用的预构建资料，已有的照常使用，其缺失不成为这些功能的准入条件。

Tutor 的用户建模贯穿正常使用：记录用户看过什么、怎样提问和回应，以及系统实际讲过什么、提供过哪些帮助；分析这些事实形成可修正的理解判断；下一次讲解、演示和学习接续使用这些判断。普通追问、复述、纠正与解释反馈均可推动更新，正式题目只是其中一种观察方式。

本轮修改启动条件、教学动作与私人记录对预构建的依赖，复用全局 TutorControl、TutorSession、Resident 循环、LearningMemory 和演示工作区。公共语义产物继续按原质量要求构建和发布，已确认 BuildPlan 的范围与预算由原构建入口持有。

| 术语 | 状态 | 本次确定的含义 |
| --- | --- | --- |
| Tutor 可用就绪 | BOUNDARY_CHANGE | 当前来源已有可读的 Pass1 基座、discourse 与整份 BookStructure，可开展读时教学 |
| 正式学习就绪 | BOUNDARY_CHANGE | 保留为整份公共教学资产完整状态，与 Tutor 可用就绪分别表达 |
| TeachingMap、LearningObjectRef、认知素材 | EXISTING | 可复用的公共教学资料及稳定正式身份 |
| TeachingMove、AssessmentContract | BOUNDARY_CHANGE | 当次学习目标与原文可独立承载教学和判题，正式对象关联可选 |
| InteractionTrace、LearningEvidence | BOUNDARY_CHANGE | 普通使用也形成可引用事实及解释，理解假设与能力证据分别消费 |
| 理解假设 | NEW | 有事实依据、限定适用范围、可被新表现与用户纠正修订的暂定理解判断 |
| LearnerContext、用户理解空间 | BOUNDARY_CHANGE | 组合相关使用事实、当前理解判断与反馈，影响本轮教学并支持后续接续 |
| TutorControl、TutorSession、AgentPresentation | EXISTING | 沿用既有控制、会话、实际交付与现场所有权 |

变更类型：边界重构与模型扩展。领域对齐已完成。启动、动作和记录必须共同解除强制对象依赖；这三层的变更属于同一交付范围。

## 1 当前实现中的依赖

| 当前入口 | 已有行为 | 实施职责 |
| --- | --- | --- |
| [tutor_api.rs](../crates/server/src/tutor_api.rs)：`source_readiness` | 要求 `teaching_readiness.json`、四类覆盖与地图发布全部完成 | 分别投影 Tutor 可用就绪与可用教学资料 |
| [teaching.rs](../crates/server/src/teaching.rs)：`start_request`、`freeze_prepare` | 启动与回合上下文依赖上述状态，并立即读取地图 | 基础条件满足即可建立会话和完整读者上下文 |
| [tutor.rs](../crates/runtime/src/tutor.rs)、[orchestrator.rs](../crates/runtime/src/orchestrator.rs) | 指导、动作合同与未完成动作修复要求先读正式材料 | 同一动作流程支持直接依据原文教学 |
| [teaching.rs](../crates/server/src/teaching.rs)：`step`、`activity_status` | `select` 要求非空对象、已读材料；活动可用性依赖地图 | 检查实际来源与交付条件，对象材料按使用情况检查 |
| [Memory teaching.rs](../crates/memory/src/teaching.rs)、[learning_evidence.rs](../crates/memory/src/learning_evidence.rs) | Binding、Evidence 与投影强制携带地图或对象身份 | 无地图也可保存、回放目标相关事实与反馈 |
| [Reader](../crates/reader/src/lib.rs)、[Memory](../crates/memory/src/lib.rs)、[session_log.rs](../crates/server/src/session_log.rs)、[presentation_store.rs](../crates/server/src/presentation_store.rs) | 已有阅读接触、对话和演示现场记录，未形成完整的普通使用分析链 | 按实际保存的事实建立学习引用，连接双方过程 |
| [teaching.rs](../crates/server/src/teaching.rs)：`accept_ungraded_evidence` | 未评分解释仍要求正式行为、对象，且仅支持来源辨析 | 接纳普通使用形成的理解假设，保持与能力证据的用途区别 |
| [useTutorControl.ts](../packages/web/src/useTutorControl.ts)、[TutorPanel.vue](../packages/web/src/components/TutorPanel.vue) | 以同一 readiness 显示准备中或开始学习 | 按 Tutor 可用就绪开放学习，持续显示真实控制状态 |

## 2 准入与运行合同

构建阶段继续按 [BUILD_STAGE_DAG](../packages/core/src/build-workbench.ts) 执行。Tutor 读时直接检查当前公开目录的 `base.json`、`source.txt`、`discourse_index.json` 和 `book_structure.json`；读取成品及其来源归属即可，不依赖私有构建过程或发布时保存的准入状态。Pass2 与其他旁支不成为前置，`formal_objects → cognitive_materials → teaching_publish` 属于可选教学资料构建。

```text
tutor_ready = current_source_and_pass1_base_are_available
              and discourse_is_readable
              and book_structure_is_readable_for_current_source

prepare_turn(request):
    read persisted TutorControl and applicable TutorSession
    if Tutor is disabled or request is outside the session:
        use the existing ordinary request path
    if not tutor_ready:
        report the actual missing prerequisite
        preserve enabled intent and existing reading access
    otherwise:
        load relevant usage facts, prior interpretations and current request
        freeze session, source revision and bounded LearnerContext
        collect compatible, accepted, reader-visible teaching assets
        read source needed for the question or performance interpretation
        use applicable objects/materials when present
        interpret new facts in the existing Resident decision
        persist supported new or revised learning interpretations when needed
        choose teaching depth, example and next step using current understanding
        compose the teaching action from verified source when materials are absent
        deliver through the existing chat/presentation workspace
        retain actual delivery, help and response facts for the next turn
```

Tutor 可用就绪取自当前公开成品的可读性及来源归属；本机与已发布来源使用相同判断。缺少 `publication.json.tutor_readiness`、私有构建目录或阶段回执均不阻塞；旧的 preparing/ready 快照不替代成品检查。缺少或无法读取的基础成品按具体阶段反馈。已有发布和来源清单中的版本标识继续用于发现来源不一致。

readiness 对外区分“现在能否使用 Tutor”与“有哪些教学资料可用”。前者决定开始学习和运行准入；后者提供地图/材料引用及实际可用状态。缺失、过时或失败的可选资料退出本轮候选，当前来源与 BookStructure 有效时教学继续。若用户正在回答已交付活动，依照该活动原绑定读取资料；原依据确实不可读时只暂停依赖它的判定。

资料仅从既有接纳与公共读取边界进入：当前读者能访问的适用对象照常使用；缺少对应认知素材时定向读取原文组织当前讲解。构建中的候选、待审阅片段和私人构建控制记录不因本决策获得读取资格。完整 TeachingMap 仍按既有整份覆盖与审阅规则发布。

已有构建任务完成并进入当前 Reader 可见版本后，后续回合重新取得可用引用；进行中的回合、活动与评分合同保持所用版本。Tutor 开启沿用已确认的构建范围，不隐含新增整本书预构建任务。

## 3 教学动作与记录合同

以下为语义合同，序列化沿现有类型最小扩展；来源身份复用项目已有版本标识。

```text
TeachingTarget = {
  learning_focus: 当前暂定学习目标及本次期望表现,
  capability: 本次检查或支持的能力,
  source_bindings: 非空的确切来源版本与已核对原文范围,
  object_refs: 可为空的正式对象及其修订引用
}

TeachingBinding = 原有会话、控制、聊天、回合与来源绑定
                  + 可选的实际教学地图修订

TeachingMove → 实际交付（冻结 TeachingTarget、内容、帮助和可用动作）
用户回应 → InteractionTrace
可判题活动 → 冻结 AssessmentContract → ResponseAssessment
普通提问、追问、复述、纠正与相关使用事实 → InteractionTrace
上述记录 → LearningEvidence（继承原目标、来源和条件，区分解释用途）
         → 下一回合 LearnerContext / 用户理解空间
满足能力证据条件且有正式对象关联的解释 → LearnerKnowledgeState
```

无正式对象时，以已有会话与实际动作/交付引用定位学习焦点，使用原文依据组织讲解和理解检查。来源范围必须由本轮原文读取支持；结构摘要用于导航，不能自行充当原文证据。有正式对象时保留对象身份、修订和实际读取的材料引用，证据不足时继续定向读原文。

选择题的答案键、开放题的原子 rubric 仍在交付前冻结。无对象不构成拒绝评分的理由；依据不充分、无评分合同或评估技术失败按既有规则产生 uncertain 或 unassessed，并保留回应和可继续教学的反馈。回答正确、提示后答对、改答和自评保持各自条件。

LearningEvidence 支持仅关联学习焦点与来源的记录，这些记录可用于后续讲解及会话恢复。普通使用产生的理解假设与实际表现证据保留不同用途，前者可调整教学，后者在满足能力证据条件且有有效正式引用时参与对象状态聚合。新对象出现时，正常教学无需等待旧记录关联；实际教学需要复用旧证据时，依据明确语义与来源对应追加关联解释，保留原事件、原评估及帮助条件。

现存含地图/对象的记录继续可读，不重写既有历史。缺少对象采用真实缺省语义；禁止以空字符串地图、伪造对象编号或把 LID 直接当正式对象规避合同。

## 4 学习起点与演示

每轮 LearnerContext 都包含当前明确请求、会话目标与约束、已确认背景和偏好、暂定学习焦点、相关实际回应及帮助条件，以及适用于当前问题的理解假设和新增使用事实。正式对象证据在可用时补充上述内容。阅读位置只提供定位，内容展示只提供接触事实；没有理解证据时保持未知。

用户明确说刚打开书、尚未阅读时，从理解当前问题所需的基本对象和关系开始。背景未知时采用可纠偏的解释起点，已有明确基础和真实表现支持加深或加快讲解。完整画像、摸底测试和教学模式配置均不是起步前置；本轮明确要求直接讲解继续优先。

验收种子来自“全书主线：把『模型在生成答案』还原成一次搬移”所暴露的使用场景：刚打开 AI Infra 书的用户请求了解全书主线并看演示。合理的起步应让用户先看清一次回答涉及哪些数据、数据所在位置、计算与等待的联系，再把这个过程连到全书主题和章节；相关主张需从实际书源取得依据。

演示沿用现有全局框架、局部制作与现场追问能力，把本轮期望用户能够解释或预测的事情带入制作上下文。可以通过改变数据量、带宽或复用条件观察结果；具体变量由原文和解释目标确定。章节列表、性能数字或交付报告本身不能证明用户已建立心智模型。

## 4A 持续观察与理解分析

产品循环为 `观察双方的实际过程 → 分析当前理解 → 调整教学 → 接收新的反馈 → 修订学习记忆`。学习记忆沿 LearningMemory 保存，用户理解空间呈现当前判断，LearnerContext 提供本轮相关部分；记录、解释与临时上下文各自保留原有所有权。

**观察范围。** 复用 Reader 的实际阅读记录、当前书籍相关对话、已交付内容、演示版本与现场、提示和用户回应。Tutor 关闭时，现有阅读与对话记录继续按原规则保存；重新开启后可读取与当前学习有关的既有事实，关闭期间的浏览不会自行恢复教学或创建学习目标。观察限于产品中实际保存的使用事实。

| 实际过程 | 可用于教学的解释 | 应产生的教学变化 |
| --- | --- | --- |
| 明确说刚打开书，想了解主线 | 本次需要建立整体认识；领域基础仍按已有信息判断 | 从基本对象和联系起步，逐渐连接各章 |
| 普通对话中连续追问同一因果关系 | 前一次解释可能未解决关键连接，形成暂定假设 | 查找缺失前提，换一个例子或演示该连接 |
| 用自己的话复述并给出新例子 | 保存原话与当时帮助，形成针对具体表现的解释 | 选择合适的下一步，按证据调整讲解深度 |
| 纠正系统对困难的判断 | 原假设需要修订 | 本轮采用纠正后的焦点，后续接续不再使用被替代判断 |
| 回到某段或改变演示参数 | 保存接触与现场事实，具体意图尚待其他表达支持 | 定位当前问题；必要时确认关注点 |

系统也记录自己实际讲过的内容、采用的例子、已揭示的提示，以及用户之后如何回应。由此形成“这个解释在当时帮助用户推进了哪一步”的有条件判断；不把一次顺利回应固化为长期学习偏好。

**持久化与解释合同。** 原对话、阅读和演示记录保持原归属；相关事实成为学习依据时，由 InteractionTrace 保存可回读的原消息、交付或行为引用及必要的当时条件。普通提问可独立进入这条链路，无须先对应一道题、一个正式对象或一次已评分作答。用户当前自述以其消息为依据；涉及书中内容的解释另外引用相应原文。

在既有 LearningEvidence 解释合同上扩展以下语义，不另设用户能力档案：

```text
learning_interpretation = {
  scope: 当前来源与适用学习目标,
  fact_refs: 实际消息、内容交付、帮助或使用事实的引用,
  nature: 理解假设 | 有实际表现支持的能力证据,
  interpretation: 简短、可检查的理解判断,
  teaching_implication: 对起点、深度、例子或下一步的影响,
  supersedes?: 被新依据或用户纠正替代的解释
}
```

`nature` 与题目评分结果分别表达；无评分合同的普通对话不为保存理解假设而生成虚构评分。事实引用与归属由系统核对，语义解释由当前 Resident 形成。未知保留未知；原始事实追加保存，解释通过新记录修订，用户纠正保留其原话。稳定背景与讲解偏好仍沿 ProfileFact 的既有采集和确认规则处理。TeachingMove 的选择记录关联实际采用的解释引用；本轮输入快照保持原样，新解释参与当前决策并供以后回合读取。

**分析节奏。** 使用中的有意义事实随原记录入口保存；下一次读者回合开始时，按当前来源和目标读取相关事实、既有解释及尚未纳入判断的增量。Resident 在同一回合先理解这些输入，按需要保存新解释或修订，再选择当前教学动作；已有判断本轮仍适用时直接复用。实际交付和后续回应回到事实记录，支持下一轮分析。每次滚动、每个动画帧或滑块变化不独立触发模型分析，也不要求用户等待另一套分析流程完成后才能交谈。

```text
实际使用 → 原有私人记录 → 相关事实引用
                               ↓
本轮请求 + 相关新增事实 + 当前理解视图
  → Resident 形成 / 修订理解判断
  → 保存解释及其依据
  → 本轮教学采用相关判断
  → 实际交付与用户回应 → 下一轮继续
```

新聊天、上下文压缩或应用重启后，相关事实与当前有效解释仍可读取。后台资料补齐只增加可用内容，不重新推断用户水平；另一来源的阅读记录只在明确相关时作为背景，不迁移成当前来源的能力结论。

**具体接续。** 用户连续问“为什么计算更快还不够”时，可形成“尚未把传输与等待联系起来”的暂定假设，并用有原文依据的过程图回应。若用户随后说明“搬运我懂，我想问它何时成为瓶颈”，本轮改讲成立条件，保存这次纠正；下次继续时从这些条件接上。验收应查看新旧判断、实际采用的讲法与原对话是否对应。

## 5 切片与交付顺序

| 切片 | 状态 | 可检查产出 | 直接依赖 |
| --- | --- | --- | --- |
| T14 | 已完成 | ADR-0158、术语、旧决策替代关系与本方案 | 已确认产品边界 |
| T15 | 已完成 | 独立可用就绪与无地图的目标/绑定合同 | T14 |
| T16 | 已完成 | 同一 Resident 的来源教学与首轮上下文 | T15 |
| T17 | 已完成 | 普通使用观察、理解分析、无对象检查与反馈适配 | T16 |
| T18 | 已完成 | 真实界面、发布资料接入及连续使用 | T17 |
| T19 | 待实施 | 真实新用户全书主线体验与资料补齐记录 | T18 |

T15–T18 共同组成首个可交付实现；中间提交按合同逐步接通，在这些路径完成前不单独上线新的启动条件。T17 按 T17A → T17B → T17C 分别完成使用事实、持续理解分析和评分活动，均完成后进入 T18。T5A 继续负责大材料预构建，其完成不再是本方案的依赖。原 T13 中的完整预构建质量验收仍归 T5A/T13；Tutor 使用体验由 T19 在无预构建资料及已有资料两种条件下验收。

## T15 就绪与目标绑定合同

**输入 → 产出**：可信来源、当前构建/发布回执、既有 TeachingBinding → 可独立判定的 Tutor 可用就绪及允许缺少正式地图的类型与持久化合同。

**主要位置**：[tutor_api.rs](../crates/server/src/tutor_api.rs)、[teaching.rs](../crates/memory/src/teaching.rs)、[tutor.rs](../crates/runtime/src/tutor.rs)、[build-workbench.ts](../packages/core/src/build-workbench.ts)、[published_library.rs](../crates/server/src/published_library.rs)。

**实施**：明确准入只读取 BookStructure 必要依赖；区分可选资料状态；以当前焦点和原文范围表示目标；地图引用可缺省，既有具名修订继续可读。复用已有构建接纳与书库发布信息，仅补缺失的必要投影。此切片先落共享合同和准入计算，外部启动路径在 T16 接入。

**完成判据**：缺少 BookStructure 或其必要接纳失败时正确拒绝新教学；必要依赖完成且全部 Tutor 预构建缺失时判为可用；缺失 Pass2 不影响该结论。真实 Memory 写入与重开分别恢复有地图、无地图绑定，当前已有记录保留原引用。

**实现记录（2026-10-09）**：`tutor_source_readiness` 独立计算必要依赖，沿用来源接纳规则并消费 Pass1、profile_sidecar、BookStructure 的现有 close/publication 回执；校验当前来源与正式产物，Pass2 和构建任务状态不进入准入。`teaching_assets` 分别投影正式地图、对象与素材的可用引用。`PublicationManifest.tutor_readiness` 在导入时保存接纳投影，发布目录不携带私有构建资料。`TeachingTarget` 保存焦点、期望表现、能力、确切来源范围及可选对象修订，`TeachingBinding.map_revision` 为可缺省引用；历史记录不迁移。外部启动、原文读取证明与实际无地图交付由 T16 接入。

**验证入口**：扩展 [tutor_tests.rs](../crates/server/src/tests/tutor_tests.rs) 的就绪用例与 Memory teaching 单元测试；涉及发布回执时补相应 Server 用例。失败时定位准入依赖或持久化字段，不以放宽来源条件修复。

## T16 来源教学与首轮上下文

**输入 → 产出**：T15 合同、用户请求、BookStructure 和原文 → 开启 Tutor 后能完成实际来源教学与交付，下一回合可以继续。

**主要位置**：[Server teaching.rs](../crates/server/src/teaching.rs) 的 `start_request`、`freeze_prepare`、`step`、`record_delivery`、`activity_status`、`learner_context`；[Runtime tutor.rs](../crates/runtime/src/tutor.rs)、[orchestrator.rs](../crates/runtime/src/orchestrator.rs)。

**实施**：启动和回合准备使用 Tutor 可用就绪；基础 LearnerContext 始终生成，包含已确认背景、当前请求和已有的相关实际事实，为 T17 的持续理解分析提供同一上下文入口；`select` 接纳有原文依据的目标，不强制非空对象或读取不存在的认知材料。按适用性复用现有对象资料，缺少材料时继续读原文。工具说明、动作完成修复和结束条件共同接受此路径，仍要求实际交付。

**完成判据**：无教学地图时完成开启、原文读取、动作选择、交付和第二轮追问；已有对象和材料时仍被正确读取；对象适用而认知素材缺少时仍能交付。新读者上下文保留背景未知或用户明确的零阅读起点，既有视口阅读记录不变成掌握。关闭 Tutor 后按原生命周期停止后续教学。

**验证入口**：在 [tutor_loop_tests.rs](../crates/server/src/tests/tutor_loop_tests.rs) 的 Resident 集成用例中增加无地图夹具，调整旧的“未发布即不暴露教学动作”预期；基座未就绪的拒绝用例继续存在。若循环仍索取不存在的对象，修正工具指导与完成合同后重跑该路径。

**实现记录（2026-10-09）**：`start_request` 与 `freeze_prepare` 使用必要接纳就绪；地图引用可缺省，已接纳的独立对象和素材保存为本轮私人资料快照。`material` 读取可用资料，`select` 核对 TeachingTarget 的来源版本、完整原文范围和实际使用的对象修订；范围按原文 UTF-16 区间与本轮读取比对。已交付目标通过原事件进入下一轮上下文，来源教学的活动、显式回读和关闭后的普通追问沿原生命周期运行。LearnerContext 始终提供当前请求、会话意图与约束、已确认背景/偏好、当前目标及近期真实交付/回应；未知不产生能力结论。Runtime 指导和未完成动作修复接受无对象教学，仍要求选定动作后实际交付。验证见 [T16 验证](performance/tutor-t16-20261009.md)。

## T17 使用观察、理解分析与反馈适配

**输入 → 产出**：普通阅读、对话、演示使用与正式活动 → 可引用的双方过程、持续修订的理解判断、实际受到这些判断影响的后续教学。

| 子切片 | 状态 | 输入与产出 | 直接依赖 |
| --- | --- | --- | --- |
| T17A | 已完成 | 现有私人使用记录 → 可回读、带归属和当时条件的学习事实引用 | T16 |
| T17B | 已完成 | 相关事实与既有解释 → 可修订判断、当前理解视图及教学适配 | T17A |
| T17C | 已完成 | 实际活动与用户作答 → 无对象评分、帮助条件与能力证据 | T17B |

**T17A 记录双方使用过程。** 主要位置为 [Reader](../crates/reader/src/lib.rs)、[Memory](../crates/memory/src/lib.rs)、[session_log.rs](../crates/server/src/session_log.rs)、[presentation_store.rs](../crates/server/src/presentation_store.rs)、[Memory teaching.rs](../crates/memory/src/teaching.rs) 与 [Server teaching.rs](../crates/server/src/teaching.rs)。串起相关原文接触、公开对话消息、实际解释/演示交付、提示和用户回应；普通问题没有 `teaching_ref` 时也能保留其真实来源与学习上下文，既有原始记录保持原归属。

完成判据：在无地图、无评分合同、无正式题目的连续对话中，能回读提问、解释、追问和纠正及其先后关系；尚未展示的演示候选不记成用户已看，已展示的提示可追溯。Tutor 关闭期间的既有阅读记录可在重新开启后按相关性读取，既不创建正式作答，也不自动开启教学。验证使用真实存储重开及现有会话日志、阅读与教学记录测试；若消息丢失或归属混淆，修正记录/引用入口。

**T17B 分析理解并用于教学。** 主要位置为 [learning_evidence.rs](../crates/memory/src/learning_evidence.rs)、[Server teaching.rs](../crates/server/src/teaching.rs) 的解释接纳、`learner_context` 与理解投影，以及 [Runtime tutor.rs](../crates/runtime/src/tutor.rs)、[orchestrator.rs](../crates/runtime/src/orchestrator.rs)。沿现有证据/理解入口扩展解释用途与修订合同；当前 Resident 根据新增相关事实形成简短理解判断，保存后用于本轮动作和以后回合，用户理解空间投影当前有效解释。引用可回读、条件真实和修订归属由系统检查。

完成判据：普通追问即可形成带事实依据的暂定假设，无需构造评分活动；本轮和下一轮的讲解深度、例子或焦点确实采用相关判断。用户纠正后使用新判断，聊天压缩、新聊天与重启仍能接续。仅浏览、重复曝光或暂定假设不增加正式能力支持计数；稳定偏好仍按原确认规则处理。存储与 scripted Resident 测试检查依据、修订和上下文消费，真实讲解效果留给 T19；事实没有进入决策或旧判断继续生效时修复对应加载/消费环节。

**T17C 理解检查与能力证据。** 主要位置为 [assessment.rs](../crates/memory/src/assessment.rs)、[learning_evidence.rs](../crates/memory/src/learning_evidence.rs)、[Server teaching.rs](../crates/server/src/teaching.rs) 的活动与评估入口，以及 [TutorActivities.vue](../packages/web/src/components/TutorActivities.vue)、[UnderstandingSpace.vue](../packages/web/src/components/UnderstandingSpace.vue)。评分与行为接纳从实际交付读取冻结目标；Source/Session 绑定足以记录无对象活动。保持假设用途与题目级判定的区别，有正式引用且满足能力证据条件的解释继续进入对象投影。

完成判据：无地图活动完成独立回答、提示后改答、判定、存储重开和下一回合适配，能区分帮助条件；有对象的旧路径继续通过；来源不充分或评估失败不被记为答错，资料补齐不自动把旧假设变成掌握。验证复用 Memory assessment/evidence 与 [tutor_loop_tests.rs](../crates/server/src/tests/tutor_loop_tests.rs) 的冻结合同、帮助、用户纠正和版本绑定用例；若出现事实丢失或错误聚合，修复对应消费层后重跑受影响场景。

**T17 实现记录（2026-10-09）**：

- T17A：`UsageObserved` 复用私人阅读记录、原聊天回合及已交付演示引用；回合准备冻结相关事实，回复成功提交后保存实际交付。普通提问不需要 teaching_ref，已展示帮助沿原回执保存；关闭期间不分析，重开后加载近期记录。演示正文与现场按原归属和确切修订回读。
- T17B：LearningEvidence 保存 `nature/target/fact_refs/teaching_implication`，在同一 Resident 内形成和修订理解假设。LearnerContext 读取当前解释和未解释事实；动作保存 `interpretation_refs`，已替代判断不能再采用。用户纠正、新聊天和 JSONL 重开后保持当前解释；冻结回合输入不被新解释改写。理解空间提供暂定理解、教学影响和纠正入口。
- T17C：无对象封闭题及开放题使用实际交付的冻结目标和来源，保留独立回答、展示提示及改答条件。LearningEvidence 的地图、对象、交付引用按实际缺省；对象投影跳过假设与无对象证据，已接纳历史证据不因新字段重写；已接纳独立对象在无地图时仍按当前修订展示。

验证见 [T17 验证](performance/tutor-t17-20261009.md)。真实学习效果和成本继续由 T19 验收。

## T18 连续使用与界面

**输入 → 产出**：T15–T17 的完整路径、本机及发布书库 → 用户能直接开始、持续互动，并在原会话中使用后来可见的教学资料。

**主要位置**：[useTutorControl.ts](../packages/web/src/useTutorControl.ts)、[TutorPanel.vue](../packages/web/src/components/TutorPanel.vue)、[App.vue](../packages/web/src/App.vue)、[published_library.rs](../crates/server/src/published_library.rs) 与现有发布版本加载入口、[Server teaching.rs](../crates/server/src/teaching.rs)。

**实施**：界面按 Tutor 可用就绪显示开始/继续学习；资料状态不替换用户的 enabled 意图。当前活动保持冻结绑定，下一回合从当前可见版本取得新资料。沿现有发布和加载机制传递可用引用，不另建材料轮询或全书构建调度。

**完成判据**：BookStructure 已就绪、教学资产缺失或构建失败时，开始按钮、实际发送与活动反馈均可工作；当前会话不中断。相同来源后续发布教学材料后，下一回合能读取新资料，旧活动仍按原题面和依据接收回答。切书、暂停/恢复和新聊天继续遵守原会话归属；相关理解判断随学习接续恢复，用户纠正后的视图保持生效。

**验证入口**：[useTutorControl.test.ts](../packages/web/src/useTutorControl.test.ts)、相关组件用例、Server 发布/回合集成用例及一个浏览器连续使用场景。界面与运行结果不一致时修正状态消费；新资料不可见时检查既有发布加载链，不改为读取构建候选。

**T18 实现记录（2026-10-09）**：本机 `/tutor/readiness` 与发布书库 `tutor_readiness` 提供同一读者投影，独立返回 `teaching_assets`，内部必要阶段回执不进入响应。网页请求绑定当前明确选择的发布版本；切换到后来可见的同源发布后，下一回合读取该版本资料，正在执行的回合保持原加载范围。界面显示可开始/继续、基础准备中、暂停和材料归属；缺少必要基座时可保存意图，可选资料状态不改写开关。

旧活动按已保存的交付、目标、对象修订、来源引文及评分合同响应，不再要求新发布目录含有旧地图文件；仍核对原文身份和版本。原地图读取继续用于绑定该地图的执行回合。发布、权限、旧活动和理解接续测试及浏览器记录见 [T18 验证](performance/tutor-t18-20261009.md)。

## T19 真实体验验收

**输入 → 产出**：T18、已完成 BookStructure 的真实书籍 → 一段可复查的新用户教学过程、实际使用的来源/资料/反馈记录与等待时间。

**场景**：使用 AI Infra 书或同一完整来源的本地验收副本，保留必要基础资产，起始无 Tutor 预构建产物。用户明确刚打开书，请求全书主线与演示；先通过普通追问、复述和对理解缺口的纠正完成连续教学，再按需要进入理解检查。使用现有适用教学产物补齐可见资料，继续同一会话；已有完整资料的来源另作对象教学回归。测试环境的操作不修改正式站用户书籍或历史。

**普通使用验收段**：没有 AssessmentContract 和正式作答的连续对话中，至少观察到一次“追问暴露缺口 → 保存有依据的理解假设 → 调整实际讲法”，以及一次“用户纠正 → 修订当前判断 → 新聊天或重启后从纠正后的焦点接续”。将采用的解释与实际回复/演示对应保存；仅有一条分析记录或模型声称已经适配不算完成。

**完成判据**：工具与事件记录证明不存在教学资产等待门槛，实际演示和回复从用户起点建立对象及因果关系，用户回应进入下一轮解释；补齐资料后确实被后续动作使用，历史活动依据保持不变。真实读者的复述或预测用于判断是否形成初步理解，页面交付成功、自动测试和模型自评不能单独充当学习效果证据；若缺少真实读者反馈，学习效果保留待验收。

**证据与成本**：在 `docs/performance/` 保存本切片的来源版本、实际教学过程、关键演示现场、资产前后状态、交付/反馈引用与理解解释的修订和采用记录；分别记录基础构建等待、Tutor 首次回应、理解分析和读时补充所用时间/用量。发现具体讲解断层后修复该环节，再复验相同理解障碍；不启动无边界的整本重建或全套视觉优化。

## 6 验证与接续

T14 文档验证（2026-10-09）：新增与修改处的本地链接、标题锚点、ADR-0158 编号唯一性、T14–T19 状态表与待实施切片详情对应检查通过。持续观察增补已检查链接与锚点、T17A–T17C 的依赖和详情对应、理解假设术语唯一性、ADR 决策格式及文档差异空白，均通过。

T14 为文档交付，未执行实现测试。T15 的验证及环境记录见 [T15 验证](performance/tutor-t15-20261009.md)。后续实施按改动职责选择现有入口：`cargo test -p server tutor_`、`cargo test -p memory <相关测试名>`、`cargo test -p runtime <相关测试名>`；Web 使用 `pnpm --filter @understand-book/web exec vitest run <相关文件>`；涉及构建投影时使用 core 对应文件测试。共享类型变化后运行受影响包的类型检查与现有类型生成校验。

T17A–T17C 已接通并通过定向验证，见 [T17 验证](performance/tutor-t17-20261009.md)。T18 已完成，验证结果见 [T18 验证](performance/tutor-t18-20261009.md)；下一刀 T19 按真实体验场景收集学习过程、读者反馈和成本。原方案已完成的 T0–T12 记录保持其历史范围。

## 7 已知限制

Pass1 基座、discourse 与 BookStructure 成品仍需可用；文件缺失或无法读取时会提示对应阶段。旧发布缺少构建回执无需重建或重新导入。没有适用预构建资料时，当前回合可能需要更多原文读取与组织，首轮时间和成本需实测。

源锚定的学习反馈能够支持持续教学；跨活动的正式对象聚合仍依赖确切身份与有依据的关联。资料不足只限制依赖它的具体判断，完整教学体验的质量由 T19 的实际过程和读者反馈验收。

使用分析只能利用产品内实际保存的事实，不能推断产品外是否学过某内容；隐含行为的含义可能不唯一，暂定假设需要随明确表达与后续表现修订。分析开销计入读时成本，普通使用记录增加并不保证教学效果，仍需检查它是否使实际讲解更贴合用户。
