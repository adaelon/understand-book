# CONTEXT —— 术语表

## 书籍封面 (book cover)
选书时用于识别阅读材料的图像。自动来源为 EPUB 声明的原书封面或 PDF 首页；没有可用图像时显示书名封面。状态：NEW（2026-10-01，用户确认先做系统默认提取）。

## 项目代码只读访问
供架构分析者查阅本项目当前获准代码与文档的访问能力，包含未提交修改和未跟踪的新文件；不授予文件修改、构建执行或 Reader 操作权限。它不同于面向书籍内容的 Book MCP。状态：NEW（2026-09-18，用户已确认范围与内容传输边界；见 `docs/代码只读MCP.md`）。

## 获准项目文件
项目维护者明确开放给代码分析者的源码、测试、设计文档和必要配置。未纳入 Git 不影响其资格；临时运行资料、私有配置、书稿和构建产物不因位于项目目录内而获得资格。状态：NEW（2026-09-18，见 `docs/代码只读MCP.md`）。

## Resident 运行 (Resident Run)
一条读者消息触发的完整住户 Agent 工作过程，归属于提问时的书籍与会话，可以包含多次模型请求和工具调用。它以回答交付、失败或取消结束；运行结束与问题得到完整回答是两个判断。状态:EXISTING（见 [ADR-0127](docs/adr/0127-resident-agent-streaming-and-runtime-activity.md)）。

## Resident 运行活动 (Resident Activity)
住户 Agent 一次运行中已经发生的模型请求、工具执行、阅读动作及其结果的可见记录。活动的先后与包含关系来自实际执行，准备调用、正在执行和执行完成具有不同含义。状态:EXISTING（见 [ADR-0127](docs/adr/0127-resident-agent-streaming-and-runtime-activity.md)）。

## Agent 回答草稿 (Agent Answer Draft)
当前运行中已经通过公开展示规则、仍可能随后续生成或交付修复而变化的回答内容。草稿尚未成为持久的完成回答，其中的来源仍须来自本轮已验证且已绑定的证据。状态:EXISTING（见 [ADR-0127](docs/adr/0127-resident-agent-streaming-and-runtime-activity.md)）。

## 当前待办与历史派发计划
当前待办是经过当前输入、策略与成果有效性判定后仍需执行的工作。历史派发计划保存批次组织和顺序，不代表其中每一项仍未完成。状态:EXISTING（见 [ADR-0126](docs/adr/0126-dispatch-refill-uses-current-pending-work.md)）。

## 候选纠错反馈 (candidate retry feedback)
同一工作单元、输入和策略范围内，最近一次 writer 字段格式失败的有界诊断。由持久失败记录生成，在专用执行器的下一次 `GENERATE.retry_feedback` 交付字段位置与期望格式；不改变产物 schema、语义输入或策略身份。自动三次尝试耗尽后，用户每次确认只追加一次纠错机会。状态:EXISTING(见 [ADR-0125](docs/adr/0125-candidate-validation-feedback-and-bounded-retry.md))。

> 纯术语表。只定义"词是什么意思",不含实现细节、不含决策(决策见 `docs/adr/`)。
> 凡此处定义与代码/对话冲突,以此处为准,冲突即点破。

## LID (Location ID)
书的语义单元 URI 方案。深度可变的有序路径(层级随书真实结构而定),硬保证:① 全局唯一 ② 同级有序(可计算前后/邻接)③ 可双向跳原文。下游不得假设固定段数。
状态:EXISTING(源自 需求文档-V2 §3.1)。

## 用户可见来源引用 (user-visible source reference)
普通用户在 Agent 对话和来源交互中看到的证据指代。它在系统内部绑定真实 LID,对外只表达可理解的来源标签、内容预览与精确跳转能力;标签必须由真实书树、节点类型与原文确定性派生,不由 LLM 自由命名。Agent 可以不产生来源;一旦产生,普通界面必须只消费 opaque ref 和用户标签。原始 LID 不属于普通用户界面,只可在显式诊断上下文中查看。来源呈现是受强制的边界契约,不依赖 Agent 自觉隐藏 LID。状态:NEW(见 [ADR-0086](docs/adr/0086-runtime-owned-user-visible-source-references.md))。

## 本轮证据账本 (turn evidence ledger)
Resident Agent 在当前用户回合内实际观察且通过现有结构闸的连续证据段集合。条目只来自用户已验证选区或证据型读取结果;视口、导航候选、错误恢复提示等仅含 LID 的状态不是证据。用户可见来源引用只能绑定本账本中的条目;账本随回合结束,不构成新的持久真相源。状态:NEW(见 [ADR-0086](docs/adr/0086-runtime-owned-user-visible-source-references.md))。

## 回答来源属性 (answer provenance)
普通 Agent 回答中某段信息来自公开文本还是内部结构通道的类型化属性。用户可见对话文本与已验证规范证据正文属于公开文本来源,可跨回合沿用;`lid/start_lid/end_lid/anchor_lid/citation_candidate_lids` 等结构字段属于内部位置来源,即使自然化为章节措辞也不改变属性。明示 `LID`/节点号的措辞始终属于内部位置;仅字面相同且有公开文本来源时不构成 LID 泄露。状态:NEW(见 [ADR-0087](docs/adr/0087-provenance-aware-answer-delivery-and-compact-provider-history.md))。

## 历史 Tool 回执 (historical Tool receipt)
已完成回合的 Tool 调用在 Provider 上下文中的类型化摘要。它保留工具名、定位型参数、成功状态或错误码、证据账本实际接受的范围、生成的 source ref 与不含正文片段的 opaque result digest,但不包含 Tool result body,不复用轨迹中截断正文的 `result_digest`,也不把结果中出现的所有 LID 当成证据。状态:NEW(见 [ADR-0087](docs/adr/0087-provenance-aware-answer-delivery-and-compact-provider-history.md))。

## Provider 历史投影 (provider history projection)
从持久对话消息生成本次模型请求上下文的只读投影。已完成历史 Tool 消息变为历史 Tool 回执,当前活动回合工具结果保持完整;投影追溯适用于旧会话但不改写持久历史、公开历史 View 或轨迹 UI。状态:NEW(见 [ADR-0087](docs/adr/0087-provenance-aware-answer-delivery-and-compact-provider-history.md))。

## 模型运行时配置 (model runtime profile)
某一模型在 Resident Agent 中适用的版本化能力与约束集合,包括基础指令、上下文窗口、输出预留、原生工具/continuation 能力、工具 schema 预算、结果截断策略以及压缩生成/消费资产。它在一个用户回合开始时解析并冻结,不是由 Provider adapter 临时猜测的参数集合。状态:NEW(见 [ADR-0091](docs/adr/0091-model-aware-agent-request-tool-exposure-and-active-context-budget.md))。

## Agent 请求计划 (agent request plan)
一次模型采样所需的 Provider 无关结构,分别承载基础指令、结构化消息、模型可见工具、工具选择模式和活动上下文状态。Provider adapter 只能把该计划映射为线协议,不得自行增删业务指令或工具。状态:NEW(见 [ADR-0091](docs/adr/0091-model-aware-agent-request-tool-exposure-and-active-context-budget.md))。

## 上下文片段账本 (context fragment ledger)
以稳定 key、内容 revision、生命周期 scope 和角色管理模型可见动态上下文的回合级账本。相同 key 在一个请求投影中最多保留一个活动 revision;敏感的回合冻结片段可参与请求但不因此进入持久会话。状态:NEW(见 [ADR-0091](docs/adr/0091-model-aware-agent-request-tool-exposure-and-active-context-budget.md))。

## 工具暴露计划 (tool exposure plan)
从完整工具注册表按模型能力、内容 profile、权限、证据状态和本回合激活集派生出的模型可见工具集合。本回合激活集既可由显式用户意图在首轮前确定性建立,也可由延迟工具发现扩展;两条路径都只改变可见能力,不执行工具或 Reader 副作用。状态:BOUNDARY_CHANGE(见 [ADR-0091](docs/adr/0091-model-aware-agent-request-tool-exposure-and-active-context-budget.md)、[ADR-0104](docs/adr/0104-intent-seeded-guided-reading-tool-exposure-and-resident-navigation-policy.md))。

## 延迟工具发现 (deferred tool discovery)
模型通过一个只读元数据检索工具查找当前未直接暴露的工具,命中项只在后续采样中加入本回合工具暴露计划。发现动作不执行目标工具、不提升权限,也不让 hidden 工具变为可见。状态:NEW(见 [ADR-0091](docs/adr/0091-model-aware-agent-request-tool-exposure-and-active-context-budget.md))。

## 活动上下文预算 (active context budget)
一次模型请求中可实际容纳的输入预算,由模型上下文窗口扣除输出预留和安全余量得到。它约束当前请求投影并在高水位触发自动上下文压缩;跨请求累计的 Provider token 只表示成本,不属于活动上下文预算。状态:BOUNDARY_CHANGE(见 [ADR-0091](docs/adr/0091-model-aware-agent-request-tool-exposure-and-active-context-budget.md),修订 [ADR-0026](docs/adr/0026-外层E编排loop-原生toolcalling-adapter扩chat-双重停机usage口径-memory独立json落盘.md))。

## 自动上下文压缩 (automatic context compaction)
活动上下文达到模型高水位时,系统把旧历史转换为一个可验证且带来源覆盖的语义压缩检查点,以该检查点原子替换模型请求中的旧历史并在同一用户回合继续。它保留原始持久消息;当前用户原文、已验证选区和未完成工具链不经摘要。旧消息只有在检查点覆盖后才能退出活动投影,不得通过无语义替代的删除或截断制造空间。状态:NEW(见 [ADR-0091](docs/adr/0091-model-aware-agent-request-tool-exposure-and-active-context-budget.md))。

## 压缩检查点 (compaction checkpoint)
从一段旧活动历史派生的、带来源覆盖的类型化任务交接状态。它保存继续任务所需的当前目标、已完成进展、关键决定、用户约束、未决义务、未解歧义、关键事实、关键示例和下一步,但不保存思维过程,不成为新的事实源,也不包含由运行时账本重新注入的敏感上下文。检查点中的语义条目只能引用输入中已存在的历史 item 和证据 ref;未通过来源覆盖与引用校验时不得替代旧活动历史。状态:NEW(见 [ADR-0091](docs/adr/0091-model-aware-agent-request-tool-exposure-and-active-context-budget.md))。

## 回答交付诊断 (answer delivery diagnostic)
来源边界校验在初次回答和唯一一次修复后产生的服务端结构记录,只含错误码、触发值、匹配形态与来源通道。它不保存回答候选全文或思维链,不进入 Provider 历史、公开历史 View 或轨迹 UI。状态:NEW(见 [ADR-0087](docs/adr/0087-provenance-aware-answer-delivery-and-compact-provider-history.md))。

## 锚定 (anchor)
节点 / 边 / 引用必须绑定到**真实存在的 LID**。基数按类型分裂 `[ADR-0010]`:**实体 / 概念锚定一个或多个 LID**(`occurrences`,贯穿全书的身份),**断言 / 引用锚定单个 LID**(`source_lid`)。未锚定或锚定到不存在 LID 的对象,一律由确定性闸丢弃。`citations[]` 只放真 LID 即此义。

## occurrences(出现锚点集)
实体 / 概念节点的多锚点字段:该身份在书中出现过的全部 LID 列表。跨窗口同一实体靠 `id = entity:{normalized-name}` 归并为一个节点,各窗口贡献的 LID 并入 occurrences。直接兑现 `book.concept` 的"出现 LID 列表"。状态:NEW(详见 [docs/adr/0010])。

## 图谱节点展示标签 (graph node display label)
`GraphNode.name` 中面向人类阅读的节点名称。它表达节点在图谱中的语义内容,可以是“模型与脚手架的消长关系”这类描述性短语;它不是用户必须准确复述的公开寻址键,也不承担自然语言指代解析职责。状态:NEW(2026-07-31 `book.concept` 可发现性 Grill 共识)。

## 概念查询候选集 (concept query candidate set)
根据用户的自然语言表述召回的、带稳定节点身份与正文证据的相关概念 / 实体节点集合。候选之间可以是同义对象,也可以是同一主题的不同侧面;该集合用于后续按问题意图选择证据,不要求预先收敛成唯一节点,也不等同于最终回答。状态:NEW(2026-07-31 `book.concept` 可发现性 Grill 共识)。

## 概念候选选择权 (concept candidate selection ownership)
概念查询候选的语义取舍属于看到用户完整问题的调用方 Agent。`book.concept` 只做确定性候选召回与证据回传,不在工具内部调用 LLM 决定候选是否回答了用户意图;调用方可选择一个或多个候选,再读取其正文证据。状态:NEW(2026-07-31 `book.concept` 可发现性 Grill 共识)。

## 概念候选正文召回 (concept occurrence-text recall)
概念查询除节点稳定身份与展示标签外,还可用节点 occurrences 所锚定的正文做确定性候选召回。正文命中说明“查询词出现在该节点的证据位置”,召回强度低于节点标签直接命中,且不自动证明该节点符合用户最终意图。状态:NEW(2026-07-31 `book.concept` 可发现性 Grill 共识)。

## 全书概念召回 (global concept recall)
概念查询的候选全集来自整本书的概念 / 实体图谱,不受当前 Reader 视口或某个 anchor 附近范围限制。可选位置只能在匹配强度相同的候选之间参与排序,不得过滤候选或使附近节点覆盖远处更准确的节点;位置相关扩展属于 `book.context`。状态:NEW(2026-07-31 `book.concept` 可发现性 Grill 共识)。

## Pass1 / Pass2(语义边两遍抽取)
构建期抽语义边的两个串行阶段 `[ADR-0010]`:**Pass1**(`pass1-local-extractor`)逐窗口抽实体/断言节点 + 局部边,merge 后确定性投影出全局目录;**Pass2**(`pass2-longrange-linker`)带全量全局目录逐窗口抽长程边、不产节点。两遍之间是**硬串行屏障**(全部 Pass1 完成→沉淀完整目录→才开 Pass2),因长程边两端可跨全书任意远窗口、Pass2 须见全量目录。各遍内 5 并发。状态:NEW(详见 [docs/adr/0010])。

## 边 scope(local / long_range)
语义边的来源标记:Pass1 产的同窗口内边记 `local`,Pass2 产的跨窗口边记 `long_range`。供读时质量度量、调试、按局部/长程分别统计召回。状态:NEW(详见 [docs/adr/0010])。

## 语义边 (semantic edge)
知识图谱中连接**实体 / 概念 / 断言**节点的边(如 builds_on / contradicts / exemplifies / cites)。区别于结构边(contains / LID 层级):语义边**无法被确定性解析**,由 LLM 在构建期读出,再过确定性图谱闸校验。
**读时角色** `[ADR-0011]`:边在读时是**召回路标**——其存在决定"捞哪些 LID 原文进证据集";`edge.type` **不当 LLM 推理的强先验**(关系让 LLM 从捞回原文现判),type 退居召回提示 / UI 展示 / 图谱导航。⇒ 错边碰不到引用真实性(结构红线焊死),最坏只污染检索精度。

## 确定性图谱闸 (deterministic graph gate)
图谱固化前的确定性校验闸 `[ADR-0011]`,对锚点真实性做**二元判定**:边端 / 节点锚点 LID 查 LID 全集(由切分层 [docs/adr/0008] 确立),存在则保留、不存在(LLM 幻觉)则**确定性丢弃且绝不重建**。最小连坐:边端节点缺失只丢边;断言 source_lid 悬空丢断言+其边;实体/概念部分 occurrence 悬空只剔该锚、全悬空才丢节点。**不自产"低置信"档**(置信是切分层属性,见下)。区别于切分层的**分区不变式闸**(后者管 LID 自身合法,是本闸前置)。状态:NEW(详见 [docs/adr/0011])。

## 局部边 / 长程边 (local / long-range edge)
- **局部边**:两端落在同一 LID 窗口内、读单个窗口即可抽出的边。
- **长程边**:两端跨窗口、读任一窗口都看不全、必须借「全局目录」才能连上的边(如"第2章的断言反驳第9章的断言")。

## 全局目录 (global catalog)
构建期第一遍抽取后沉淀的**扁平文本索引**(实体索引 + 断言索引,每条带锚定 LID + 一行摘要)。作用是充当"人造 wikilink":第二遍把目录连同窗口一起喂给 LLM,使其能把当前窗口的节点连到窗口外的远处节点。是我们替代"书没有 import / 没有 wikilink"的关键造物。

## 语境胶囊 (context capsule)
**已降格为 `book.context` 的读时确定性投影**,不再是物化产物 `[ADR-0012]`。原 V3 §3.5 把它列为模块A预组装产物(每 LID 预存"前置背景 LID + 同主题关联 LID + 概念锚点");工程层裁决砍掉物化:`book.context(lid, granularity)` 读时现场从最终图([docs/adr/0011])+ LID 物化路径树([docs/adr/0008])纯函数投影 near/mid/far,复用 `scope=auto` 同一套图谱遍历。理由:图谱遍历已毫秒级([docs/adr/0002]),物化是冗余抽象层 + 一致性风险源。**一切基础是图谱,不叠派生抽象层。** near/mid/far 三档 = 图谱具名遍历(near=树邻接+local边 / mid=经概念二跳的其他 occurrences / far=long_range边另一端),**累积半径 + 分层标注**(`far` 返回 near∪mid∪far,每条标 `{lid,layer,via}`)+ 每档**确定性 top-K 截断**(near 按 LID 距离 / mid 按共享概念数 / far 按边 weight,K 留实测),全量某概念走 `book.concept`。详见 [docs/adr/0013]。状态:NEW(详见 [docs/adr/0012] 降格 + [docs/adr/0013] 三档规则)。

## 窗口 (window)
构建期喂给 LLM 做一次抽取的一段连续 LID 跨度。**以章/节子树为基本单元**(超预算则子树内细分、过小则合并相邻同级,不跨卷);细分切点吸附 LID 边界,不腰斩句。批次的基本单元。详见 [docs/adr/0009]。

## 窗口预算 (window budget)
界定一个窗口能装多少的双约束:**输入硬闸**(正文 token ≤ 上下文窗口 × 安全系数 − 指令/目录/输出预留,放不下必拆)+ **输出软闸**(单窗口预期节点/边软上限,超则再拆)。具体数字留切片0实测。详见 [docs/adr/0009]。

## 预算可路由性 (budget routability)
计划生成的每个最小模型工作单元在进入执行器前,必然不超过其版本化输入硬闸。它约束的是单次模型输入,不同于 BuildPlan 总预算或 dispatch 聚合上限;LID 继续作为证据坐标,不得为适配模型窗口而改写。状态:BOUNDARY_CHANGE(见 [ADR-0100](docs/adr/0100-budget-routable-model-work-units-and-truthful-build-recovery.md))。

## 模型输入片 (model input slice)
从 canonical source 中一个或多个 LID 的受校验区间确定性派生、只供单次模型执行的内部输入单位。它不创建段落或 LID,不改变 EPUB/Markdown/PDF 对齐与引用身份;同一 LID 的 core 区间须完整且不重不漏,可见 overlap 不计覆盖,语义结果最终仍归回原 LID。状态:BOUNDARY_CHANGE(见 [ADR-0100](docs/adr/0100-budget-routable-model-work-units-and-truthful-build-recovery.md))。

## 融合批次 (fused batch)
把多个相邻小窗口打包进一次 subagent 抽取调用以省开销;产出仍按各原窗口的 LID 区间拆回。借自 U-A 的 fused-batch。状态:NEW(详见 [docs/adr/0009])。

## 边界重叠 (boundary overlap)
仅在"超大子树内部 token 细分"产生的**人工切点**处,让相邻窗口共享一小段 LID,使紧邻细关系被某窗口完整看到,确定性闸去重。子树语义边界处零重叠(跨边界关系=长程边,走 Pass2)。状态:NEW(详见 [docs/adr/0009])。

## 预构建期 / 读时 (build-time / read-time)
- **预构建期**:跑在 agent harness(Claude Code / Codex 等)里,一次性产出 `.understand-book` 产物。
- **读时**:阅读器运行中,与任何 harness 脱钩,独立产品。

## 规范化 Markdown 正文 (canonical Markdown source)
PDF 论文 MVP 的正文真相:用户用外部 OCR / PDF-to-Markdown 工具得到并清洗后的 Markdown 文件。构建管线只以该 Markdown 生成 `source/source.txt`、`SourceBlock[]`、`LID/span/book.text`;原版 PDF 不参与正文切分。状态:NEW(详见 [docs/adr/0046])。

## PDF 旁路附件 (PDF sidecar attachment)
与规范化 Markdown 正文配对保存的原版 PDF,用于旁路预览、人工核对和未来回跳。它不是正文真相,也不是 citation anchor;图谱节点、断言、引用、FormulaSemantics 证据仍必须锚定真实 LID。状态:NEW(详见 [docs/adr/0046])。

## PDF source map
可选的 Markdown 到 PDF 版面映射 sidecar,形如 `md_span -> page/bbox`。只有存在该映射时,阅读器才能从某个 LID 近似/精确回跳原 PDF 页框;没有 source map 时只提供 PDF 旁路打开/人工核对。source map 只做 provenance,不得替代 LID 成为证据锚。状态:NEW(详见 [docs/adr/0046])。

## paper profile
英语学术论文的 content profile / extraction rule pack:把输入按论文体裁理解为 abstract、introduction、method、results、discussion、references 等论文结构和论证功能的组合,用于约束后续抽取内容与抽取规则。它不是新的存储基座或锚定体系;所有证据、引用和读时命令仍复用 LID/source/book.text。状态:NEW(详见 [docs/adr/0046], [docs/adr/0048])。

## paper_subtype
`content_profile=paper` 内部的论文体裁分型,用于在 paper base rules 之上叠加局部抽取规则。它不是新的顶层 content profile,也不得覆盖 LID/source/book.text/citation anchor、PDF 旁路、paper_metadata、paper_lexicon、单篇 MCP 边界等 paper base 契约。状态:NEW(详见 [docs/adr/0059])。

## PaperSubtypeOverlay
paper_subtype 对 paper base rules 的局部覆盖层。它只能 patch `detect_rules`、`section_classification_rules`、`metadata_extra_fields`、`argument_shape`、`graph_edge_rules`、`book_structure_rules`、`reading_guide_rules`、`validators` 等固定 slot;不得新增 Core schema、独立 pipeline 或 subtype 专属持久 truth。状态:NEW(详见 [docs/adr/0059])。

## survey subtype
综述类论文在 `paper` 规则包下的 subtype。其论证形状以领域范围、分类轴、文献簇、比较维度、综合判断、共识/分歧和未来空白为核心,而不是强套普通研究论文的 problem/method/experiment/result 链。状态:NEW(详见 [docs/adr/0059])。

## paper claim source
paper 规则包中声明事实来源层级的标记。`review_says` 表示本综述作者的转述或综合;`original_paper_verified` 表示已通过接入原始论文 MCP 或等价证据验证一手论文事实。综述中对原文献的描述默认只能标为 `review_says`。状态:NEW(详见 [docs/adr/0059])。

## PaperArgumentLayer
paper 规则包预构建抽取的论证链层:围绕 `problem -> research_question -> method -> evidence -> claim -> limitation` 组织本篇论文的可验证理解。它不是独立 `paper_argument.json`;具体落在共享 BookStructure(`spine/throughlines/key_stops`)、graph(`entity/concept/claim/edges`) 与 discourse sidecar(段落功能/语篇关系)中,再由 MCP/读时投影视图组合出来。服务单篇读懂、论文 MCP 自我说明和跨论文比较时的证据回应;每个判断必须带真实 LID evidence。状态:BOUNDARY_CHANGE(详见 [docs/adr/0049], [docs/adr/0053])。

## PaperMetadataLayer
paper 规则包预构建抽取的书目/上下文元数据层:记录 title、authors、affiliations、venue、year/date、DOI/arXiv/URL、keywords、field/topic labels、references、dataset/code/funding links 等单篇公开事实。它服务未来多论文 MCP 编排的候选对齐,不直接生成跨论文关系。所有字段必须使用 MetadataField envelope 表达 value/source/evidence/confidence;来自正文的字段必须带 LID evidence;来自用户或外部 resolver 的字段必须标 source,不得伪装成正文证据。该层作为独立 profile sidecar `paper_metadata.json` 物化,不塞进 BookStructure 或 graph。状态:BOUNDARY_CHANGE(详见 [docs/adr/0049], [docs/adr/0050], [docs/adr/0051])。

## paper_metadata.json
paper 规则包的独立 profile artifact,承载 PaperMetadataLayer。它是单篇论文的公开书目/上下文事实 sidecar,带 profile/version 头;其业务字段统一使用 MetadataField envelope,供 MCP projection 和多论文 MCP 编排读取。它不是 BookStructure,不参与 LID 树/图谱 schema,也不直接生成跨论文关系。状态:BOUNDARY_CHANGE(详见 [docs/adr/0050], [docs/adr/0051])。

## PaperMetadata MVP 字段集
paper_metadata.json 的 MVP 字段边界:只覆盖跨论文 MCP 编排最常用的对齐键,包括 title、authors、affiliations、venue、year、identifiers(DOI/arXiv/URL)、keywords、field_labels、references、datasets、code_links、funding。MVP 不做 citation style normalization、author disambiguation、institution canonicalization、BibTeX/CSL 完整兼容或 reference graph normalization。状态:NEW(详见 [docs/adr/0052])。

## MetadataField
paper metadata 字段的统一来源信封:`{value, source, evidence_lids?, confidence?}`。`source` 标记字段来自 front_matter、paper_text、user_supplied、filename、external_resolver 等来源;`evidence_lids` 只在字段可由正文 LID 证明时填写;`confidence` 表示抽取/解析置信度,不得替代 LID evidence。状态:NEW(详见 [docs/adr/0051])。

## BilingualAidLayer
面向“中文母语用户阅读英文论文”的双语辅助层。英文 `source/book.text` 仍是唯一正文真相和 citation source;中文只作为解释、释义、术语说明、句法拆解和学习辅助,不得替代原文或成为证据锚。预构建只抽关键术语/缩写/高价值短语的 paper_lexicon;普通单词、短语和句子理解走读时按需解释 + 用户 memory。状态:NEW(详见 [docs/adr/0054])。

## paper_lexicon.json
paper 规则包的双语术语 profile artifact,本质是论文术语索引而非预生成中文讲义。它承载本论文公共的关键英文词项:论文自定义术语、方法名、缩写、领域术语、数据集/指标/模型名、影响论证理解的高价值学术短语等。MVP 只抽“理解本论文必需”的词项,不抽普通英语生词。每项优先保存 term、term_type、`occurrences_lids`、`defined_at_lid?`、aliases、acronym_expansion 等索引信息;可选短中文 gloss,但深度中文解释、句法拆解和面向用户水平的讲解留给读时按上下文生成。它不保存全文翻译,也不保存读者私人不会的词。状态:BOUNDARY_CHANGE(详见 [docs/adr/0054], [docs/adr/0055], [docs/adr/0056], [docs/adr/0057])。

## PaperReadingGuide projection
paper 规则包的读时/MCP 投影视图,把单篇论文的 BookStructure、graph、discourse、paper_metadata、paper_lexicon 组合成“如何读这篇论文”的任务面。它支持速读/精读/研读层次、消极/积极/批判/创造性阶段、论文十问、Codebook 解码和摘要中文理解辅助;不新增 Core schema,不复制持久 truth。状态:NEW(详见 [docs/adr/0058])。

## PaperReadingMode
PaperReadingGuide 的阅读层次: `skim`(速读:标题/摘要/引言/贡献/是否值得读)、`close`(精读:问题/假设/方法/实验/证据/局限)、`deep`(研读:复现/公式细节/实现路径/后续研究)。状态:NEW(详见 [docs/adr/0058])。

## PaperReadingStage
PaperReadingGuide 的阅读阶段: `passive`(知道论文是什么)、`active`(知道它有什么用)、`critical`(质疑假设/实验/证据/局限)、`creative`(提出改进点和后续研究方向)。状态:NEW(详见 [docs/adr/0058])。

## 论文十问 (paper reading questions)
paper 规则包的标准 MCP/读时问答面:围绕问题/input-output、问题性质、hypothesis、相关研究/关键人物、核心贡献、实验设计、数据集、结果是否支撑假设、贡献总结、下一步工作十类问题组织回答。回答必须回到真实 LID evidence 或明确标注为 model_supplement / user reflection。状态:NEW(详见 [docs/adr/0058])。

## PaperCodebook
帮助读者“解码”论文的组合视图,不是单独产物。由 `paper_lexicon.json`(术语/缩写/方法名)、`paper_metadata.json`(作者/领域/时间/引用上下文)、BookStructure(展开逻辑)、discourse sidecar(段落功能)和 graph(claim/evidence/limitation)共同构成。状态:NEW(详见 [docs/adr/0058])。

## AbstractReadingAid
BilingualAidLayer 在论文摘要上的特化投影:围绕 abstract 的英文原文、关键术语、短中文释义、逐句理解检查和用户中文复述来暴露理解漏洞。它不做全文预翻译,也不把中文复述当 citation evidence。状态:NEW(详见 [docs/adr/0058])。

## paper 消费层
paper profile 预构建产物与读时用户任务之间的产品入口层。第一版主入口是 Web reader 单篇交互阅读:围绕单篇论文的 PaperReadingGuide、PaperCodebook、AbstractReadingAid、metadata/lexicon 与 LID 原文证据帮助读者读懂本篇;不负责跨论文综合,不把读者私有理解写回公共 paper truth。状态:NEW(2026-07-05 §0.5 paper 消费层 grill)。

## content profile / extraction rule pack
挂在 Core build pipeline 固定插槽上的可插拔抽取规则包。它决定 Pass1 抽取关注点、profile-sidecar 语篇/公式规则、Pass2 edge contracts、BookStructure key_stop/throughline 选择策略、MCP/读时投影视图;但不得改变 LID、source/book.text、citation anchor、确定性闸或 Core 命令面。状态:NEW(详见 [docs/adr/0048])。

## Profile Plugin Framework
让 `content_profile` 同时驱动预构建、后端读时/agent、前端消费的插件框架。后端 registry 管 build rules、runtime policy、projection contracts、evidence gate 和 profile manifest;前端 registry 管 profile 组件槽位、布局渲染和交互 affordance。二者通过共享 profile contract / version 对齐;前端不得拥有 paper/book truth、plan truth 或 citation policy。状态:NEW(详见 [docs/adr/0061])。

## technical_learning rule pack
当前已有抽取规则的正式规则包名,覆盖工具书、教材、技术书、数学、金融、科学、管理等说明型学习材料。现有 `pass1-local-extractor`、`profile-sidecar-extractor`、`pass2-longrange-linker`、`book-structure-extractor` 的规则应收敛为该规则包的默认实现,而不是项目全局真理。状态:BOUNDARY_CHANGE(详见 [docs/adr/0033], [docs/adr/0048])。

## 单篇论文 MCP (single-paper MCP)
一篇已构建论文对外暴露的 MCP 服务/工具面:只负责回答、检索、解释、对照本论文内部的 LID 证据与 paper profile 产物。它不拥有跨论文全局关系,也不写全局 corpus graph。状态:NEW(详见 [docs/adr/0047])。

## 多论文 MCP 编排 (multi-paper MCP orchestration)
外部 MCP 客户端(如 Claude / Codex)同时连接多个单篇论文 MCP,在运行时按用户任务询问、比较、挑战和综合多篇论文,从而建立临时跨论文关系。跨论文关系默认是会话级综合;只有用户显式保存时才进入 memory/note。状态:NEW(详见 [docs/adr/0047])。

## 阅读器 (reader)
第三层(消费)的独立阅读产品,集成确定性导航(②)+ LLM 问答(③),消费已就绪材料并维护当前读者的阅读状态。“阅读器本体”包含读时 Agent 与读者私有数据;读时与预构建宿主脱钩,支持用户自有设备或自有服务器上的使用。状态:BOUNDARY_CHANGE(见 [ADR-0122](docs/adr/0122-linux-reader-host-and-source-deployment.md))。

## LLM 后端 (LLM backend)
读时 ③ 用的大模型提供方,**用户自选**(Anthropic / OpenAI / 本地 Ollama 等),由阅读器后端的 provider 抽象接入。区别于预构建期由 harness 提供的 LLM。

## 书 agent / 模块 E (book agent)
读时"这本书的 agent"。承接用户对本书任何问题的**单一入口**,带 **memory** + **skills**,架在确定性导航(②)+ LLM 问答(③)之上,跑用户自选后端。运行时由我们在阅读器后端**自建**(最小工具调用 loop + 记忆;弱后端走 ReAct 兜底)。详见 [docs/adr/0005]。

## 记忆层 (memory layer)
E 的记忆所在。**独立于只读基座、用户私有、可变、跨书**。两层:会话工作记忆(临时:当前对话+阅读位置)+ 长期记忆(持久:旅程/问答/兴趣/卡点/笔记)。book agent 读写但不拥有。详见 [docs/adr/0006]。
**记录模型** `[ADR-0015]`(参考 Codex `codex-rs/memories`):结构信封 + 散文 content + **记忆引用锚定**(见下)。命令面 `memory.save/recall/delete`(议题6 定);Codex 式两阶段后台 consolidation(Phase1 抽取阅读会话 / Phase2 合并+遗忘+usage 剪枝)+ 分层渐进披露产物留议题7。

## 异步已读账本 (asynchronous read ledger)
`reader.scroll/goto` 与 GUI `reader.position.observe` 产生的确定性已读事实先进入进程内待持久集合，命令成功不确认该批事实已具备崩溃耐久性。Agent/headless 导航可登记命令窗口，GUI 原生滚动只登记防抖后实际观察到的当前 LID；待持久集合合并同一 LID 的触达次数与最近时间，失败保留待重试，切书或有序退出前冲刷。Note、Highlight、QA 等用户显式记忆仍保持同步原子提交。状态:BOUNDARY_CHANGE(见 [ADR-0106](docs/adr/0106-asynchronous-coalesced-read-ledger-persistence.md)、[ADR-0109](docs/adr/0109-browser-owned-native-prose-flow.md))。

## Memory selection context
选区创建的 Note 可选携带的结构化来源上下文:保存 `resolved/partial` 状态、用户实际选择的 `raw_quote`、可验证的 `resolved_quote` 与按阅读顺序排列的完整 LID ranges。用户内容可保留 raw quote,但 citations 与精确投影只能使用 resolved quote/ranges。Note 的 `anchor.lid` 仍取首个 resolved LID用于语义定位和排序,PDF 行内标记取末 range 作为显示锚,citations 由 ranges 中的 LID 去重派生;普通旧 Note 无此字段且保持兼容。状态:BOUNDARY_CHANGE。

## Memory replace
Note 内容编辑使用的原子替换命令:验证旧 `mem_id` 后只更新 content,默认继承 anchor、selection context、note placement、citations 与 layer;写入失败时旧记录保持不变。带 quote source 的重新定位必须走显式“重新选择”并提交新的 selection context;无引用来源 Note 的正文迁移走独立 Note 原子重锚,两者都不得伪装成内容编辑。状态:BOUNDARY_CHANGE(见 [docs/adr/0074]、[docs/adr/0083])。

## 无引用来源 Note (unquoted-source Note)
不带结构化 `selection_context` 的 Note;正文中的 blockquote (`>`) 只是内容展示,不证明引用来源也不决定记录类型。新记录必须带显式 Note 正文放置,二者皆无只允许作为未知旧类型兼容读取。状态:BOUNDARY_CHANGE(见 [docs/adr/0083])。

## Note 放置草稿 (NotePlacementDraft)
用户从 Agent 回答截取无引用来源内容后形成、尚未写入 memory 的单个短生命周期草稿,绑定当前书、阅读表面与来源指纹。有效目标提交成功后转成 Note;取消、新草稿、切书或关闭 Reader 时丢弃,写入失败或结果待确认时暂留。状态:BOUNDARY_CHANGE(见 [docs/adr/0083])。

## Note 放置会话 (Note placement session)
用户把 Note 放置草稿首次绑定或把已有无引用来源 Note 移到正文目标的单一临时交互控制器。首版仅由 Pointer Events 点选真实目标;Markdown 与 PDF 会话严格隔离,提交前最新操作可抢占,提交后不可取消或替换。状态:BOUNDARY_CHANGE(见 [docs/adr/0083])。

## Note 正文放置 (Note body placement)
用户把无引用来源 Note 显式绑定到当前 canonical source 中真实正文目标的持久语义,分为 Markdown `lid_block` 与 PDF `pdf_region` 两种不可跨格式迁移的类型。正文放置连同 `source_fingerprint` 是唯一位置权威,`anchor.lid`、citations 与内容寻址 `mem_id` 都从它派生。状态:BOUNDARY_CHANGE(见 [docs/adr/0083])。

## PDF Note region placement
无引用来源 Note 在 PDF 正文中的显式对象位置,由 `source_fingerprint + lid + source_map_version + source_map_config_hash + page_index + region_id` 标识。渲染时必须复验同一来源与映射身份;任一失效时只保留 Notes 列表,不得回退到 primary region、整 LID bbox 或邻近区域。状态:BOUNDARY_CHANGE(见 [docs/adr/0083])。

## Note 原子重锚 (Note atomic reanchor)
把已有无引用来源 Note 移到另一正文放置的一次性 memory mutation:placement、`anchor.lid`、citations 与内容寻址 `mem_id` 原子改变,content、layer、generated_at、usage 与 source session 保持。成功后旧 ID 失效,碰撞或失败时原记录不变;当前不设稳定 `note_id`,带 quote source 的 Note 仍走显式重新选择。状态:BOUNDARY_CHANGE(见 [docs/adr/0083])。

## 记忆引用锚定 (memory citation)
记忆记录回溯到源位置的锚 `[ADR-0015]`,借自 Codex memory 的 `MemoryCitationEntry{path, line_range, note}`——把 Codex 的 `path:行号区间` 换成本项目的 **LID**:`citations:[{lid, book_id, note}]`。使 `memory.recall` 返回的每条记忆**可验证、可跳原文**,是引用红线([docs/adr/0004])在记忆层的延伸。区别于 book 的 `citations[]`(那是问答证据;此为记忆溯源)。状态:NEW(详见 [docs/adr/0015])。

## effect 返回 / 错误分类+recovery
- **effect 返回** `[ADR-0015]`:`reader.*` 变更命令返回"变更后的相关状态"(gotoLid→viewport、note→{id}…)而非裸 ack,使 agent 能确认动作、闭环、re-sync(配 `reader.state()` 只读会话态)。
- **错误分类 + recovery** `[ADR-0015]`:统一错误信封 `{error_code, category∈{validation,not_found,provider,budget,internal}, message, recovery?}`。category 分流瞬时(provider/budget,可重试)vs 永久(not_found/validation,改输入)错;`recovery`(nearest_valid_lid / suggestions[] / {retriable,after_ms})**仅供 agent 自纠、系统永不自动套用**(守禁宽松降级,体检 §14)。状态:NEW(详见 [docs/adr/0015])。

## 命令面 (command surface)
阅读器的命令可寻址引擎。人类能做的每个阅读器动作都暴露为具名、可参数化、agent 可调用的命令;GUI 是其上一层渲染。E 及任意外部 agent 与人类走**同一命令面**(人机对称、无特供),agent-CLI 普适。**一套面、三命名空间**:`book.*`(只读内容查询 ②③)/ `reader.*`(可变 UI 控制)/ `memory.*`(记忆层读写)。硬边界:reader/memory 不得写只读基座。详见 [docs/adr/0007]。
**命令面分层** `[ADR-0014]`:命令面即 agent 的 tool 集,按是否调 LLM 分两层——**确定性命令 = 叶子工具**(见下),**LLM 命令 = 自建最小运行时被无状态调一次的暴露**(`book.query/synthesize`,本身即 agent loop)。

## BookToolContractRegistry
Book 共同命令跨 Resident、REST、MCP 的版本化契约单一真相源:统一拥有 logical tool ID、typed input、JSON Schema、required/enum/default/validation、结果契约引用、surface aliases 与 capability tags。各表面可使用 `book.text`/`book_text`/`/book/text` 等不同 transport 名称,但只能投影同一逻辑契约;MCP 过滤 reader/memory/private 工具属于能力边界,不是 schema 漂移。完整住户 prompt 与外部 MCP 编排策略仍可不同。状态:NEW(设计已接受,实现见 [ADR-0088](docs/adr/0088-deterministic-text-occurrence-search-and-canonical-book-tool-contracts.md))。

## Reader UI Control Plane
阅读器页面布局与面板状态的可命令化控制面,属于 `reader.*` 可变 UI 会话态。agent 不直接操作 DOM,只能发受控 `ReaderLayoutAction`(如 open/close/focus slot、切换 layout preset、pin evidence),由后端 session layout state 校验并产出可撤销 effect,前端按 profile registry 渲染同步状态。它不写 book truth、paper truth、BookStructure 或 memory。状态:NEW(详见 [docs/adr/0060])。

## 阅读排版偏好（reader typography preferences）
归属于当前读者在当前设备上的正文文字外观与排布选择，包括字体、字号、行距、字距和版心宽度；同一设备上的不同读者分别保有自己的选择。它区别于书籍内容、学习画像、逻辑工作区布局与阅读位置。状态：EXISTING，RE1–RE2 已实现（[ADR-0148](docs/adr/0148-reader-typography-annotations-and-motion.md)）。

## 正文批注标记（body annotation marker）
正文中指向已有笔记或高亮记录的轻量可见入口，用于保持批注与原文的对应关系；它是记录的呈现，不是新的笔记、来源锚点或证据。状态：EXISTING，RE3 已实现（[ADR-0148](docs/adr/0148-reader-typography-annotations-and-motion.md)）。

## 叶子工具 (leaf tool)
确定性命令在"命令面即 agent tool 集"分层中的角色 `[ADR-0014]`:无 LLM、毫秒级、可组合的 primitive,agent loop 直接调它们捞素材。包括 `book.manifest`(确定性拓扑)、`book.context`(纯指针 near/mid/far,`{lid,layer,via}`)、`book.text`(按 LID/区间取真原文)、`book.concept`(概念全量 occurrences)与 `book.search_text`(正文 occurrence 完整定位)。区别于 LLM 命令(运行时暴露)。状态:BOUNDARY_CHANGE(基础定义见 [docs/adr/0014],全文定位扩展见 [ADR-0088](docs/adr/0088-deterministic-text-occurrence-search-and-canonical-book-tool-contracts.md))。

## TextOccurrence (正文匹配 occurrence)
`book.search_text` 在规范 `source.txt` 中找到的一次字面匹配,身份是一个非空全局 UTF-16 source range 及其按叶子分区拆出的连续 LID ranges。同一 LID 内重复出现分别计数,跨叶匹配仍只算一次,父 container 与子 leaf 不得重复计数。它不同于 graph concept 的 `occurrences:[LID...]`:前者是可证明完备的正文匹配实例,后者是构建期语义锚点集合。状态:NEW(已实现,见 [ADR-0088](docs/adr/0088-deterministic-text-occurrence-search-and-canonical-book-tool-contracts.md))。

## book.search_text
确定性全文定位叶子工具:在给定 query、exact/版本化 normalized mode 与可选 LID/相对 scope 内以两遍扫描计算完整 totals、再只物化当前 `TextOccurrence` 页面,返回全集总数、section counts、规范文档序页面和绑定 source revision 的稳定 cursor。`page_size` 只控制单页载荷,不把“全部”退化为 top-K;first/previous/nearest/all 都是全集上的投影。其 `exhaustive=true` 只保证 lexical exhaustive,不保证找齐语义改写或隐含讨论;语义解释继续用 `book.text/context/query/synthesize`。状态:NEW(已实现,见 [ADR-0088](docs/adr/0088-deterministic-text-occurrence-search-and-canonical-book-tool-contracts.md))。

## 段 (paragraph / 叶子块)
切分的中间叶子单元 = 源格式的一个块级标记(md 段/列表项/引用块/代码块;epub p/li/blockquote/br)。**忠实映射源块的结构类型,不检测文学体裁**;诗行、对话轮靠块边界天然落位(非按体裁识别),裸 txt 退化到空行。切片0 的最深 LID 层。注:`code/table/image/formula` 四类块级标记是带类型的一等 **asset 叶子**(见下),非普通 paragraph;其中 formula 另有公式语义剖面。状态:NEW(详见 [docs/adr/0008/0029])。

## asset 叶子 (asset leaf)
代码块 / 表 / 图 / 公式四类**带类型的一等 LID 叶子** `[ADR-0029]`(`NodeKind ∈ {Code,Table,Image,Formula}`,闭集可扩展)。仍是 LID 树叶子:**占 source span、进分区不变式划分、用 LID 单一寻址**(不引第二套 asset_id)。**原文 = 源标记的确定性序列化**(image=`![alt](src)`、code 保留换行缩进、table 保留表文本、formula 保留 LaTeX/MathML 源标记),`book.text(asset_lid)` 返回它,确定性+忠实;`book.manifest` 节点带 `kind` 暴露可见性(零新基础命令,agent 过滤 kind 定位)。Code/Table/Image 作为普通可锚 LID 进入图谱;Formula 额外要求公式语义剖面(见下),让 agent 交互时能拿到参数、组合含义和上下文关系。多模态图理解(描述图)留切片1+、走"标注来源"旁路、**不进只读基座**。源自 `参考.md` 文档世界状态「让图/表/代码不从链路消失」并扩展到技术/数学书的公式状态。状态:NEW(详见 [docs/adr/0029])。

## 行内公式 LID (inline formula LID)
源段落内部的 `$...$` 或 EPUB 段内 MathML 公式也是 Formula 叶子 `[ADR-0029 执行回填]`:预构建切分时将其从所在文本段拆出为独立 `NodeKind=Formula` LID,公式前后的文字保留为相邻 paragraph LID。它不是段内 occurrence/span sidecar,不引入第二套公式 id;`book.text(formula_lid)` 返回公式源标记,PB6 `formula_lids`、`formula_semantics.json`、前端点击/Formula tab 均复用真实 formula LID 路径。状态:BOUNDARY_CHANGE(2026-07-03)。

## 公式语义剖面 (FormulaSemantics)
Formula 叶子的高优先级读时语义对象 `[ADR-0029]`。除 `book.text(formula_lid)` 返回公式原文外,构建期还要固化 `parameters[]`(每个符号/参数的名称、含义、单位或取值域若原文给出、定义来源 LID)、`composition`(公式整体表达什么、各项如何组合)、`context_links[]`(公式与前后段落、概念、断言、图/表/代码的关系)。每条解释必须带 `source_lid` 或 `evidence_lids`,且这些 LID 由确定性闸校验真实存在。agent 解释公式、回答公式相关问题、生成追问时应优先把 FormulaSemantics 连同公式原文放入上下文;无证据的模型常识只能作为 model_supplement,不得伪装成书内事实。状态:NEW(详见 [docs/adr/0029])。

## 句 (sentence)
段下更深一层 LID 节点,由确定性句切(句末标点 + 引号/括号配对保护)从叶子块文本切出,每块 1..N 句。citation 与语义边的最细锚点。切片1+ 长出。状态:NEW(详见 [docs/adr/0008])。

## 物化路径 (materialized path)
LID 的字符串编码 = 各级序号点分串(如 `3.2.5.2`)。比较用**逐段数值**(非字典序),据此算同级有序 / 前后邻接 / 窗口范围切片。状态:NEW(详见 [docs/adr/0008])。

## 分区不变式 (partition invariant)
切分自检闸的核心断言:全部叶子 span 构成对原文内容的一次**划分**(全覆盖 + 无重叠),父 span ⊇ 子并集,同级数值递增,LID 全局唯一。破则打回 / 标低置信。状态:NEW(详见 [docs/adr/0008])。

## 最小 agent loop / 自建运行时 (minimal agent loop)
模块 E 的心脏:本地查询服务内置的 agent 运行时,U-A 没有、本项目净新自建 `[ADR-0005][ADR-0016]`。**双层嵌套**:
- **外层 = E 编排 loop**(会话级,有状态 messages+memory):LLM 自主调命令面,管多跳编排 + 会话态 + 主动策略。默认不限模型采样轮数,以实际任务交付结束;上下文压缩后继续,取消、无进展或无法恢复的失败保留未完成状态。调用方显式设置的运行限制独立于上下文容量。状态:BOUNDARY_CHANGE([ADR-0150](docs/adr/0150-resident-unbounded-tool-loop-and-progress-stops.md))。
- **内层 = `book.query` 自含 mini-loop**(无状态,被外层调一次,裸调即完整):外层提交自含问题、显式 referents 与回答义务;内层先从本地 ReferentCatalog 产候选并冻结唯一 binding,再围绕该 binding 回读来源 LID、由 LLM 判断开放语义支持度,最后由结构硬闸校验义务覆盖与 citations。anchor 仅是同级排序先验,不再定义检索边界。
状态:BOUNDARY_CHANGE(基础双层 loop 承 [docs/adr/0016],query 内层由 [docs/adr/0077] 修订)。

## ModelAdapter (provider 适配层)
统一 loop 骨架与具体后端之间的薄序列化层 `[ADR-0016]`。loop 控制(请求校验、候选/证据预算、停机、状态聚合、citations 校验)**provider 无关恒定**,只通过 `complete(messages, tools?, schema?) → ParsedResponse` 跟模型打交道;两实现:**NativeAdapter**(原生 tools API + JSON mode,直接拿结构化)/ **ReActAdapter**(无原生 tool-calling 的弱后端:注入 ReAct/JSON 模板 + 解析文本回填成同一 ParsedResponse)。弱后端只改输出解析方式;开放语义相关性由 LLM 判断,结构红线([docs/adr/0004])仍由确定性 citations 过滤。状态:BOUNDARY_CHANGE(详见 [docs/adr/0016][docs/adr/0077])。

## book.query / book.synthesize(LLM 命令分工)
两个 LLM 命令按**证据所有权**分工:`book.query(query,intent,targets,obligations,anchor_lid)` 只处理显式 referent 的语义问答,系统先解析 target、冻结唯一 referent,再围绕它隐式取证并回答;`anchor_lid` 只作弱排序先验。`book.synthesize(lids:[LID...], task?)` 由调用方拥有证据选择权,系统只在给定离散 LID 内综合、无检索、无外扩。章节主旨/论文贡献先用 `book.structure`/`book.guide_path`(technical book)或 `book.paper_reading_guide`(paper)选 LID 再 synthesize;当前 passage 优先 `book.text`/`book.context` 或已知 LID 的 synthesize;query 不承担文档级搜索,也不在答后重复调用 synthesize。两者 citations 都只能指向各自允许的来源 LID。状态:BOUNDARY_CHANGE(原分工承 [docs/adr/0017],query 边界由 [docs/adr/0077] 修订)。

## Query referent / frozen referent binding
**Query referent** 是外层 Agent 在 `book.query` 请求中显式声明、需要内层回答其语义问题的自然语言逻辑对象;它不是用户原话、anchor LID 或附近主题。内层 Resolver 必须把每个 target 映射到唯一 catalog candidate 后形成 **frozen referent binding**,取证期间不得再被 anchor 邻文或后来出现的候选替换。多义未消除则返回 ambiguous,无可接受候选则 unresolved,均不得生成语义答案。状态:NEW(详见 [docs/adr/0077])。

## ReferentCatalog
读时本地 referent 路由索引的统一名称:technical book 以 graph Concept/Entity 为目录,paper 以 paper lexicon 为主、graph Concept/Entity 为兜底。它通过名称、显式 alias/acronym 与自身 occurrence 文本产生候选,不依赖向量服务,也不要求构建期穷举所有别名。catalog、候选摘录、gloss 和图谱边都只是 routing artifact;只有候选冻结后回读的真实来源 LID 才能成为 query evidence。状态:NEW(详见 [docs/adr/0077])。

## Query obligation / PlanGate
**Query obligation** 是外层 Agent 随自含 query 一并声明的原子回答要求,只表达“这次必须回答什么”,不枚举定义、因果、类比、机制等关系逻辑,也不预先规定哪些来源可推出哪些关系。**PlanGate** 是内层对 query、targets 与 obligations 是否无损一致的语义 veto;缺项时只返回 invalid_plan 交外层修正,不得静默补题或缩题。状态:NEW(详见 [docs/adr/0077])。

## Query structural gate / semantic support assessment
`book.query` 的两类判定边界:**semantic support assessment** 由 LLM 阅读 frozen referent 的来源证据后开放判断每项 obligation 为 supported/uncertain/unsupported;程序不试图穷举知识关系。**Query structural gate** 只确定性检查请求合法、binding 唯一且冻结、每项 obligation 有 assessment、citation 位于证据包且 quote 来自对应真实 LID,再聚合 complete/partial/insufficient。排序分数不能补救结构闸失败。状态:NEW(详见 [docs/adr/0077])。

## QueryAudit
`book.query` 每次运行产生的旁路结构化审计:记录请求、候选、逐候选 fit、Resolver 结果、frozen bindings 及 selected round/rank、证据选择/预算/扩展/overflow、模型调用数、obligation assessments、citations 与结构闸结果。它供用户和开发者检查并随 resident Agent 回合历史持久化,但不进入 tool result、messages 或后续模型上下文,不充当来源证据,也不保存隐藏思维链。状态:NEW(详见 [docs/adr/0077])。

## scope-granularity 同轴半径 (legacy query retrieval)
`book.context` 的 near/mid/far 仍是从树邻接、local 边、概念二跳与 long_range 边确定性投影的累积半径 `[ADR-0013]`。`book.query` 原 local/chapter/cross_chapter/global anchor-scope 阶梯曾与它同轴,但该 query 用法已被 referent-first 取证取代:query 只能在 frozen referent 之后复用这些图谱/结构原语扩展证据,不得再以 anchor 为中心扫章或全书。状态:BOUNDARY_CHANGE([docs/adr/0077] 修订 [docs/adr/0016] 的 query 检索部分)。

## 构建侧增量
以旧版成果作为计算缓存、生成独立新版基座的构建方式；是否复用由完整任务输入、抽取规则与确定的引用对应决定，工作区位置不决定成果有效性。新版与旧版分别拥有真实内容与引用归属，下游按当前实际依赖确定变化范围。状态：BOUNDARY_CHANGE（[ADR-0146](docs/adr/0146-portable-build-workspaces-and-incremental-book-updates.md)，已接受设计、待实施，承接并修订 ADR-0019）。

## 书籍版本
某一份确定内容形成的独立可读基座，以 book_id 区分其原文、附件及公开成果的归属；搬迁保持身份，内容改版形成关联前一版本的新基座。状态：EXISTING（[ADR-0146](docs/adr/0146-portable-build-workspaces-and-incremental-book-updates.md)，明确 ADR-0019 的版本边界）。

## 新旧段落对应
指定两版来源之间，由原文、结构与上下文确定的段落关联，表达原文等价、内容修改、删除或对应未确定，并识别新版新增内容。它服务成果重绑与历史引用投影，不把相同位置编号或同名概念视为内容等价。状态：NEW（[ADR-0146](docs/adr/0146-portable-build-workspaces-and-incremental-book-updates.md)，已接受设计、待实施）。

## 记忆迁移(v1→v2 跨基座 citation 重锚)
保留历史私人记录及其原始引用，在新版阅读时通过确定的来源对应投影可用定位：原文等价可延续，原文变化标需复核，删除或对应未确定保留旧版入口。原会话与阅读位置保持原版归属；教学证据的跨版消费还需正式学习对象的显式对应，不能仅由段落映射推导。状态：BOUNDARY_CHANGE（[ADR-0146](docs/adr/0146-portable-build-workspaces-and-incremental-book-updates.md)，已接受设计、待实施，保留并扩展 ADR-0020 的历史事实与读时投影原则）。

## 命令面 REST 投影 (command surface REST projection)
读时 localhost 服务把**冻结命令面**(V3 §4)投影成 HTTP 的形式 `[ADR-0028]`:`book.*` 只读 → `GET`、`reader.*`/`memory.*` 可变 → `POST`,**端点名 = 命令名**,错误**原样透传** §4.4 分类信封。是命令面的网络面、非另立的第二套 API;前端 / agent / 人看同一张面([docs/adr/0007] 人机同命令面无特供)。状态:NEW(详见 [docs/adr/0028])。

## 连续正文渲染 / LID 隐形 (continuous prose rendering)
前端把视口 `visible_lids`(叶序滑动窗口)渲染成**一整列连续流动的正文**:不画每段框/分隔/LID 标号,阅读单位是连续文章而非 LID 片段。**LID 是隐形接缝**,只在 citation、跳转或高亮锚等需要时显形。状态:BOUNDARY_CHANGE(基础见 [ADR-0028](docs/adr/0028-前端切片架构-vue-localhost-server-crate-tinyhttp同步-rest命令面1对1投影-不引epub框架-连续正文lid隐形-无页码寻址.md),实现基线恢复见 [ADR-0112](docs/adr/0112-pre-phr-reader-body-path-rollback.md))。

## 区间视口 (interval viewport)
阅读器连续滚动模型中的后端视口:`top_lid` 是当前阅读区间起点,`bottom_lid` 是区间终点,`visible_lids` 是 `[top..top+width)` 叶序区间,`width` 是区间宽度。`anchor_lid` 表示区间中段叶,用于 agent/state 共享；`scroll(delta)` 移动 `top_lid`,`goto(lid)` 让目标叶成为 `top_lid`。状态:EXISTING(见 [ADR-0043](docs/adr/0043-reader连续滚动视口-后端区间窗口-前端虚拟流-note-overlay.md))。
## 虚拟阅读流 (virtual reader buffer)
前端连续阅读的渲染形态:浏览器原生滚动驱动一条按叶序排列的长流,到缓冲边缘时调用 Reader 区间视口命令补入新叶。它是 `visible_lids` 的展示策略,不引入页码,不改变 LID 作为唯一引用锚；当前实现恢复为 `e64d5f0` 的追加路径,不沿用 PHR 有界回收或未提交原生正文流。状态:BOUNDARY_CHANGE(基础见 [ADR-0043](docs/adr/0043-reader连续滚动视口-后端区间窗口-前端虚拟流-note-overlay.md),回退见 [ADR-0112](docs/adr/0112-pre-phr-reader-body-path-rollback.md))。
## note overlay
note 卡片的展示层 `[ADR-0043]`:note 内容仍来自 memory,overlay 只负责按当前可见 LID 过滤、定位和展示,避免 note 卡嵌入正文段落循环后污染虚拟列表的段高估算。highlight 仍是段内 `<mark>`。状态:NEW(详见 [docs/adr/0043])。
## 读位感 (reading position sense)
替代"页码"的位置参照:**章节定位**(从当前 anchor 上溯容器 LID)+ **进度%**(`anchor_idx` / 叶总数)。它只做导航与显示,**不做引用锚**。本项目无一等页码；印刷版 page-list 仅在源携带时作 LID 展示标签,citation 恒为 LID。状态:EXISTING(见 [ADR-0028](docs/adr/0028-前端切片架构-vue-localhost-server-crate-tinyhttp同步-rest命令面1对1投影-不引epub框架-连续正文lid隐形-无页码寻址.md))。

## MemoryDocument
本地单用户、跨书的读者私人内容记录与稳定画像事实账本,统一容纳其审核和治理状态,与只读书基座物理隔离。它是 `LearningMemory` 内稳定用户上下文的权威来源,不拥有绑定 `LearningObjectRef` 的学习证据或对象级知识状态。旧的裸记录数组是其待迁移前身,不是并列真相源。状态:BOUNDARY_CHANGE(详见 [docs/adr/0075]、`grill.md` Q81)。

## ProfileFact
一条带来源、证据、信任状态和生命周期的结构化稳定用户上下文事实,描述背景、目标、讲解偏好、约束或用户明示的通用能力背景。`scope` 表示事实属于全局还是某本书,`applicability` 表示它适用于全部内容、某个 `content_profile`、论文 subtype 或领域;两者正交。它不得保存或推导绑定 `LearningObjectRef` 的对象级表现或掌握判断。状态:BOUNDARY_CHANGE(详见 [docs/adr/0075]、`grill.md` Q81)。

## ExplanationPreference
`ProfileFact` 中表达用户通常偏好怎样讲解的长期上下文，只在教学语境已成立、当前回合显式意图与有效 `TutorSessionMode` 都未给出更强指示时作为弱默认。只有已确认偏好可以参与解析；它不得建立教学语境、开启 `GuidedInquiryPolicy`，也不得覆盖当前请求或临时会话默认。状态:BOUNDARY_CHANGE(见 `grill.md` Q86)。

## ProfileFactCapture (画像事实采集方式)
记录画像事实是在当前交互中采集,还是从用户选定的历史对话中回填;它与事实 `source` 正交。`HistoricalBackfill` 保留原始来源,但确认前始终为 `Pending`。状态:NEW(2026-07-14 M4 §0.5)。

## CollectionRule (画像采集规则)
用户对未来画像采集边界的持久规则,以画像事实类别、可选语义键与 `scope`/`applicability` 确定性限定哪些信息不得自动或回填进入画像。当前显式 `remember` 可作单次例外且不解除规则;已有事实只有经显式 `forget` 才清除。状态:NEW(2026-07-14 M4 §0.5)。

## ProfileScopeChange (画像作用域变更)
用户把既有画像事实显式重述到新 `scope` 的治理动作。它产生 `UserStated + Confirmed` 后继事实并 supersede 旧事实,不原地改写事实权威或审计链。状态:NEW(2026-07-14 M4 §0.5)。

## 显式画像证据记录 (explicit profile evidence record)
用户明确 remember 或 correct 画像时授权保留的来源原话,供 `ProfileFact` 审计与来源检查;它不是普通 context memory、不进入 snapshot,执行 forget 时必须与事实值一起物理删除。状态:NEW(详见 [docs/adr/0075])。

## 画像隐私分类 (profile privacy class)
画像写入前的三档确定性分类:`Normal` 可按显式意图保存,`Sensitive` 只允许用户明示并二次确认本地明文风险,`Secret` 在任何情况下都拒绝保存;分类 validator 可升级风险但不得降级。状态:NEW(详见 [docs/adr/0075])。

## ReaderPrivateStorageGate (读者私人存储闸)
保证本地明文读者记忆只对当前 OS 用户可读写的隐私边界。权限无法收紧或验证时必须停止私人 memory 读写并暴露可诊断状态,但不阻断不依赖私人记忆的普通阅读。状态:NEW(2026-07-14 M4 §0.5)。

## GlobalReaderProfile
从有效 `ProfileFact` 投影出的跨书稳定读者画像,只包含相关背景、通用能力背景、长期目标、稳定讲解偏好和长期约束。它不吸收单本阅读反应、临时情绪、普通 context memory 或绑定 `LearningObjectRef` 的对象级知识状态。状态:BOUNDARY_CHANGE(详见 [docs/adr/0075]、`grill.md` Q81)。

## BookReadingState
某一本书的读者私人阅读状态,由已读 LID、提问/笔记/高亮等原始行为信号、单本事实和当前 `MemoryPolicy` 投影组成。旧代码级 `ReaderProfile {read_lids, focus_lids, puzzle_heat}` 此后归入本术语;行为信号本身不证明困惑或掌握。状态:BOUNDARY_CHANGE(详见 [docs/adr/0075])。

## ReaderProfileSnapshot
runtime 在读者回合开始前从 `GlobalReaderProfile`、当前 `BookReadingState` 和适用事实组装的有界画像视图。它是 `LearnerContext` 的稳定用户上下文输入,不承载对象级 `LearnerKnowledgeState`;可按账本重建,不是第二真相源,也不写入对话历史。状态:BOUNDARY_CHANGE(详见 [docs/adr/0075]、`grill.md` Q81)。

## MemoryIntentGate
住户用户消息进入主 agent 前的显式记忆意图入口:结构化 UI 动作直接生成记忆操作,明确的“记住/纠正/忘记”表达才触发前台结构化抽取。未命中的普通表达不增加当前回答延迟,由后台审核兜底。状态:NEW(详见 [docs/adr/0075])。

## MemoryOp
由结构化 UI 动作或 `MemoryIntentGate` 前台抽取产生的 typed 画像记忆操作,统一表达 remember、correct 和 forget;它必须经过 source、scope、sensitivity 与 schema 校验后才能原子修改 `MemoryDocument`,不等同于普通 `memory.save(type=context)`。状态:NEW(详见 [docs/adr/0075])。

## ProfileGovernanceMutation (画像治理变更)
用户通过治理面发起的 typed 画像变更,由 `operation_id` 标识意图并以 `expected_document_revision` 保护并发状态。已成功的同 ID 同内容重放返回原结果,同 ID 异内容或未见请求的 stale revision 均冲突。状态:NEW(2026-07-14 M4 §0.5)。

## ProfileGovernanceSurface (画像治理面)
住户读者查看画像来源、集中审核并主动纠错的控制面,不是画像产生的必经流程。普通低风险更新保持自动且只给非阻塞通知;全局 Pending、敏感信息、不可逆 forget 与显式历史回填才要求用户主动动作。状态:NEW(2026-07-14 M4 §0.5)。

## ReviewJob / 记忆 consolidation
`ReviewJob` 是对住户会话未审核回合做增量事实抽取的持久任务;全局 consolidation 只消费已落账事实与跨书独立证据,生成待确认的全局候选。二者都由 runtime 调度并以 watermark 保证可恢复,不依赖主 agent 自愿调用 `memory.save/recall`。状态:BOUNDARY_CHANGE(详见 [docs/adr/0075])。

## HistoricalBackfillJob (历史画像回填任务)
用户显式发起、用于预览历史语义画像候选的持久任务,范围冻结为选定的住户会话及其当时的用户回合上界。其候选均以 `HistoricalBackfill + Pending` 进入集中审核;失败或取消保留已完成部分与进度,重试只补剩余范围,清除只移除任务与未确认候选,已确认事实只能经显式 `forget` 删除。状态:NEW(2026-07-14 M4 §0.5)。

## MemoryPolicy
`content_profile` 为读者记忆提供的语义解释策略:定义哪些私人信号值得提取、如何派生 profile-specific state、以及如何给快照候选排序。它不得改变 MemoryDocument、信任/删除规则、LID/citation 红线或直接生成自由文本提示;未知 profile 使用中性策略。状态:NEW(详见 [docs/adr/0075])。

## ProfileUsageTrace
一次住户回答对画像的可检查使用轨迹,区分 runtime 确定性注入的事实与模型声明实际参考的事实。模型声明不是客观因果证明,只能作为弱统计和用户解释入口。状态:NEW(详见 [docs/adr/0075])。

## ProfileMarkdownProjection (画像 Markdown 投影)
`reader-profile.md` 与 `reading-handbook.md` 是从画像真相源单向覆写的可检查派生视图,不接受反向写入。其 `current/stale/missing/unreadable` 状态由文件标记的 `projection_revision` 与当前真相源对比得出,不成为新的持久真相。状态:BOUNDARY_CHANGE(2026-07-14 M4 §0.5)。

## qa 提问活动 (qa activity)
读者在某 LID 留下的不同 qa 记录数,只证明发生过提问并构成私人关注信号,不单独证明困惑、难度或掌握程度。`technical_learning` 可将其用于回看排序,`paper` 等其他 MemoryPolicy 按自己的语义解释;`puzzle_heat` 仅是旧实现字段名。状态:BOUNDARY_CHANGE(详见 [docs/adr/0041], [docs/adr/0075])。

## agent 可撤销提议 (agent reversible proposal)
读时 E agent 在阅读器中对视图/标注的修改**是可撤销的提议、用户终裁** `[ADR-0030]`。agent **真执行**命令(守人机对称 [docs/adr/0007],命令面无特供),可撤销落**前端交互层**:用 effect 返回([docs/adr/0015])的反向命令 undo(goto 回原 anchor / `memory.delete(id)`)。**提议单元 = 一次对话回合**(`/agent/chat` 一次调用的全部副作用)= 事务性 undo。**agent 提议态 = 标注落 `layer=session`(临时),用户「保留」才升 `long_term`**(复用 memory 两层,零新字段;未处置走人则不污染长期记忆)。`orchestrator` 的 `OuterOutcome` 加 `effects[]`(副作用清单)+ `trace[]`(查询踪迹,对用户可见)承载之。状态:NEW(详见 [docs/adr/0030])。

## 读时会话边界 (reading session boundary)
对话会话的切分仍由用户显式控制 `[ADR-0030]`;idle 只触发 ReviewJob,不自动创建新对话。新对话、切书和 context compression 是记忆审核边界,新回合以自动 `ReaderProfileSnapshot` 冷启动而非依赖 agent 主动 recall。状态:BOUNDARY_CHANGE(详见 [docs/adr/0030], [docs/adr/0075])。

## agent 对话历史 (agent chat history)
住户 agent 的本地、读者私有、按 `book_id` 分组的可恢复对话会话列表 `[ADR-0030]`。它不是 memory 真相,但其中的用户回合可作为 ReviewJob 的画像证据引用;删除原会话不等同于执行 memory“忘记”,且历史绝不暴露给 visitor/MCP。状态:BOUNDARY_CHANGE(详见 [docs/adr/0030], [docs/adr/0075])。

## route(导航原语)
图谱上的**确定性多跳导航原语** `[ADR-0034]`,零 LLM,只保证返回的 LID/边真实("确定性 LID 由 route 保证")。两形态:`route_from(at)` = **前沿式内核**(站在当前 LID 返回可走的下一步);`route_to(from, target)` = 同批边上跑 BFS 的确定性组合(派生)。区别于 `book.context`(单跳"相关点"):route 是把 context 链起来 + 按边语义排序成"可导航下一步"的多跳找路。route 内核是 Core(架在 book.context 上);教学性排序/过滤属 technical_learning policy。状态:NEW(详见 [docs/adr/0034])。

## 前沿 (frontier) / 导航类别 (navigation category)
`route_from` 的返回形状 `[ADR-0034]`:不是一条平铺 ranked list,而是按导航语义分的 **5 个类别**——`back`(前置/背景)/ `forward`(深入/承接)/ `concretize`(例证/具体)/ `cross`(关联/跨章)/ `continue`(顺读)。`edge_type → 类别` 是固定确定性映射表(Core),组内按 weight×距离 排序。意图直接落到类别("没懂"→back,"给例子"→concretize)。状态:NEW(详见 [docs/adr/0034])。

## 住户 / 访客 (resident / visitor)
读时两类 agent 的本质区别 `[ADR-0034][ADR-0035]`:**住户**携带当前位置与自动注入的 `ReaderProfileSnapshot`,和读者有持续关系;**访客**只带外来意图和临时会话,不得读取、生成或修改读者私人记忆。可共享的是 route/世界模型,不可让渡的是读者私人层。状态:BOUNDARY_CHANGE(详见 [docs/adr/0034], [docs/adr/0035], [docs/adr/0075])。

## 三类记忆 (three memory classes)
读时记忆按可见性分三类 `[ADR-0035]`:**① 世界模型**(公共,可借:route/book.text/citation gate)/ **② 读者私人记忆**(durable + 读者所有:MemoryDocument/GlobalReaderProfile/BookReadingState)/ **③ 访客会话记忆**(ephemeral + 访客交互所有)。③ 不得成为②的证据来源。状态:BOUNDARY_CHANGE(详见 [docs/adr/0035], [docs/adr/0075])。

## 访客会话 (visitor session)
外部 agent 经 MCP 连接我们时的 ephemeral 会话 `[ADR-0035]`,**TCP 式握手/挥手**维护:握手发 `session_id`、传输期迭代引导(支持"不对"refine)、挥手即焚 + 超时 GC。内容 = `transcript`(交互记录)+ 临时**游标** `cursor{at_lid, last_frontier}`(访客自己的位置,≠ 读者 viewport)。绝不写入 ② 的 durable store。修订了 P7 原"无状态"假设(见 [docs/adr/0035])。状态:NEW(详见 [docs/adr/0035])。

## book_guide(访客向导命令)
外部 agent 投影的只读 LLM 命令 `[ADR-0035]`:`book_guide(intent, anchor?)` 返回 `意图→入口节点→route 路线(每步理由+证据 LID)`。是 `book_query` 的姊妹——**query 返答案,guide 返路线**。配访客会话态可跨调用 refine。返回全是真 LID/真边,外部可独立验证。状态:NEW(详见 [docs/adr/0035])。

## 反馈信号 (feedback signal)
带读 loop 中调整下一步的转向输入 `[ADR-0036]`。**唯一主信号 = 用户在停靠点的开放 NL 提问**(非闭集 token);viewport 偏离仅作弱旁路、`ReaderProfileSnapshot` 作慢先验,当前明确指令始终优先。状态:BOUNDARY_CHANGE(详见 [docs/adr/0036], [docs/adr/0075])。

## 导航轴 / 讲法轴 (navigation axis / explanation axis)
反馈意图的两个正交轴 `[ADR-0036]`:**导航轴**(去哪)落 `route_from` 的导航类别;**讲法轴**(怎么讲/多细/重讲)由当前 `MemoryPolicy` 消费 `BookReadingState` 与适用全局事实,不改变 route Core。裸“没懂”仍先使用确定性未读前置,不得把画像假设当成用户当前指令。状态:BOUNDARY_CHANGE(详见 [docs/adr/0036], [docs/adr/0075])。

## TechnicalLearningAgentPolicy(带读教学整形)
`technical_learning` 对 `route_from` 前沿和讲解方式的 profile policy `[ADR-0034][ADR-0037]`。它可消费已读与 qa/note/highlight 等原始活动及证据化学习假设,但不得从行为自动断言 novice/expert、掌握或卡点,也不得让 profile 偏见进入 route Core。状态:BOUNDARY_CHANGE(详见 [docs/adr/0037], [docs/adr/0075])。

## BookStructure(书籍结构地图)
content profile / extraction rule pack 驱动的预构建结构 sidecar `[ADR-0044/0048]`:描述一本书/论文的公共结构理解,用于“带我读”先讲总体框架/当前位置意义,再进入逐停靠点 route。它不是读时临时摘要、不是 `ReaderProfileSnapshot`、不是 memory;输入只来自公共书基座与 profile artifacts,不得混入用户私人层。状态:BOUNDARY_CHANGE(详见 [docs/adr/0044], [docs/adr/0048])。

## spine / throughline / key_stop
BookStructure 的三层骨架 `[ADR-0044]`:
- **spine**:书的教学展开主干,按结构单元表达“这本书如何展开”(如 setup/foundation/method/application/synthesis),保留阅读顺序和依赖。
- **throughline**:贯穿多个章节/单元的主题线,表达“一个问题如何跨书发展”,由 graph long_range、discourse、formula/pass2 evidence 支撑。
- **key_stop**:带读时值得停下讲的锚点(定义、核心公式、反直觉论断、转折、例子、总结段等),可被 spine 和 throughline 共同引用。状态:NEW(详见 [docs/adr/0044])。

## structure unit card
BookStructure 构建中对一个章或节的有来源理解，表达该单元的问题、角色、摘要及所选重点，为全书主题和阅读依赖提供局部依据。章节重点与宏观路线选点可以不同；选择较短路线不删除已确认的章节内容。状态：BOUNDARY_CHANGE，BSR1/2 引用合同及 Codex 真实章节局部验收完成，BSR5 正式章节路由与发布已接入（[ADR-0151](docs/adr/0151-book-structure-global-outline-and-semantic-retrieval.md)，承接 ADR-0044）。

## BookStructure 全书框架草案
根据材料的真实目录、前言及章级概述形成的暂定整体理解，表达各章要回答的问题、展开阶段和可能贯穿全书的主题。正文可以修订它；形成框架不代表完成正文覆盖。状态：EXISTING，BSR2 框架合同及真实局部生成已验证（[ADR-0151 §1](docs/adr/0151-book-structure-global-outline-and-semantic-retrieval.md#1-全书框架)）。

## BookStructure 候选重点
从公共材料中发现、具有来源依据和讲解价值的内容候选，保留其含义、成立条件及所在章节。经章节或主题取舍后成为正式重点；未入选宏观路线不表示候选失效，也不表示读者已掌握。状态：EXISTING，BSR1 候选保留与引用物化已实现（[ADR-0151 §2](docs/adr/0151-book-structure-global-outline-and-semantic-retrieval.md#2-候选重点与章节取舍)）。

## BookStructure 主题工作集
围绕一个跨章问题组织的相关章节、候选重点与依据，用于说明这个问题在后续内容中加入了哪些条件、发生了哪些判断变化。它的成员可以是不同机制；共同主题不等于对象同一或阅读前置依赖。状态：EXISTING，BSR4 合同及真实主题局部对照完成，BSR5 正式调度与发布已接入（[ADR-0151 §3](docs/adr/0151-book-structure-global-outline-and-semantic-retrieval.md#3-跨章主题与依赖)、[验收](docs/performance/book-structure-bsr3-bsr4.md)）。

## 结构投影 (structure projection)
BookStructure 在读时围绕某个 LID 投影出的结构解释 `[ADR-0045]`:回答“当前位置在全书/当前 spine/throughline/key_stop 中意味着什么”。它只消费公共 BookStructure sidecar 与真实 LID,不消费 `ReaderProfileSnapshot`、MemoryDocument 或读者 viewport。状态:BOUNDARY_CHANGE(详见 [docs/adr/0045], [docs/adr/0075])。

## 宏观带读路线 (guide path)
BookStructure 在读时提供的全书级带读路线 `[ADR-0045]`:按 spine 分段展开 key_stops,用于“先把这本书的重要地方过一遍”。它区别于 `guided_route_from`:guide path 是宏观路线,`guided_route_from` 是站在当前 LID 的局部转向前沿。状态:NEW(详见 [docs/adr/0045])。

## 带读目标自检 (guide target self-check)
非机械带读跳转的证据校验步骤 `[ADR-0045]`:LLM 可根据用户反馈选择候选 LID,但必须先读取该 LID 的真实原文与近邻上下文,判断是否满足目标,通过后才执行跳转。机械“继续/下一段”不触发此术语。状态:NEW(详见 [docs/adr/0045])。

## 构建工作区 (build workspace)
一份书籍版本的已导入来源、公开成果与续建进度所在位置；其位置可变，已完成成果的有效性由内容和抽取规则决定。预构建私有进度与阅读期公开成果保持各自所有权。状态：EXISTING（[ADR-0146](docs/adr/0146-portable-build-workspaces-and-incremental-book-updates.md)，U1–U2 可搬迁能力已实现；修订 ADR-0042 初期仅指中间产物目录的定义）。

## 跨会话续建 (cross-session build resume)
Claude 在环驱动预构建时,**会话 token / 上下文耗尽后由新会话接着建**的机制 `[ADR-0042]`——真书数十窗 × Pass1 subagent 抽取一个会话跑不完,**跨会话是常态路径非异常**(承软工准则 A4 防上下文断裂)。物理前提 = **逐窗原子落盘**(每抽完一窗即写 `pass1/<id>.json`,旧"手工拼单一 outputs.json"会话死则全丢)。续建判定 = **存在性 + content-hash 校验,位置 id 键**(`content_hash = sha256(buildPass1Input(window).text)`;新会话重算窗口逐窗比对,在且一致 = done,缺失/不一致 = pending);**无状态位 / watermark / lock**(承 [docs/adr/0038][docs/adr/0039] 砍单机过度工程),中断 = 没文件 = pending(二值)。冷启动靠 **agent 续建契约**(写进 `skills/build/SKILL.md`,与 SESSION_CHECKPOINT C4/C5 同招):新 Claude `status <book>` 拿 pending → 逐窗 `emit-input` + subagent 抽取 + 原子写 → 全 done 跑 `pass1-batch` 收口(pending 默认拒绝收口)。区别于跨版本增量构建([docs/adr/0019],书改了复用旧基座 + LID 重锚)与内容寻址复用(留 [docs/adr/0042] 何时回头),二者本刀不做。状态:NEW(详见 [docs/adr/0042])。
## Hybrid paper source
Paper profile 的双源输入模型:Markdown 与原版 PDF 共同进入 source reconciliation,生成可信 `source.txt`;PDF-first reader 再用 map artifact 把 LID/range 投影回 PDF 页面。状态:BOUNDARY_CHANGE(见 [docs/adr/0063])。

## Source reconciliation
PDF-first paper build 中把 `paper.md + paper.pdf` 对齐、修复版面/编码差异、阻断内容冲突,并只在确定性门禁通过后产出可信 `source.txt/base.json` 的阶段。状态:NEW(见 [docs/adr/0063])。

## Paper Markdown canonicalization
Source reconciliation 对 draft `paper.md` 中不承载内容的表示噪声执行确定性规范化，所得文本是对齐 span、人工复核和可信 `source.txt` 的共同正文基准；原始输入只保留 provenance。未知或承载结构的 HTML 不得被静默删除。状态:NEW(见 [docs/adr/0081])。

## Source reconciliation overload
Source reconciliation 的派生诊断：未解决项的绝对数量或密度已表明系统性对齐失配，不应继续作为逐项人工或批量 LLM 复核队列。它只暂停高成本复核并要求先修复、重跑确定性对齐器；不删除 unresolved 诊断，不产生可信 `source.txt`，也不改变 Manual source override 契约。状态:NEW(见 `docs/修复方案-来源对齐复核过载.md`)。

## Source review decisions
Source reconciliation unresolved block 的用户复核记录,写入 `.build/source-reconciliation/review-decisions.json`。它只表达用户选择和备注,供后续 source reconciliation rerun/gate 消费;自身不生成可信 `source.txt`,也不能替代 content equivalence 与 realignment gate。状态:NEW(见 `docs/切片方案-paper-pdf-first-hybrid.md` PH16)。

## Source review page group
把共享同一 `pdf_page_index` 的 Source reconciliation unresolved blocks 投影为一次页面级复核操作的临时分组；无页码 block 必须各自成组。分组不合并、删除或信任原子诊断，用户页面级决定仍逐条持久化为 Source review decisions。状态:NEW(见 `docs/修复方案-来源对齐复核过载.md`)。

## Bulk LLM source review decisions
用户在 Source reconciliation review surface 上一次明确选择,授权系统逐项请求 LLM 修订并把高置信、非 uncertain 的有效结果记录为 Source review decisions。批量操作允许部分成功:成功项保留决策,技术失败、无效输出、低置信或 uncertain 项继续留给用户逐项复核;它不是全有或全无事务,也不直接生成可信 `source.txt`。状态:BOUNDARY_CHANGE(见 `docs/切片方案-paper-pdf-first-hybrid.md` PH16)。

## Manual source override
Source review decisions 全部齐备后,系统只执行一次确定性 source reconciliation 重跑;若 reviewed draft 仍有 residual unresolved,用户终裁优先于这一次验证结果,该 reviewed draft 以显式 `manual_override` provenance 进入后续构建。残余报告必须保留用于审计,但不得再次生成同一轮来源复核或自动重跑。它不允许跳过首轮逐项复核,也不允许直接采用尚未持久化的 LLM suggestion;它终结的是来源文本决定,不证明 PDF 页、行、字符或 bbox 映射正确。状态:BOUNDARY_CHANGE(见 [docs/adr/0065]、[docs/adr/0082])。

## Hybrid alignment unit
混合阅读基座定位 PDF 时使用的有序、互不重叠来源区间,把正文与相邻公式上下文作为一个定位单元;定位成功后再把证据投影到该区间内的子 LID。它不改变 SourceBlock/LID 分区,也不是 citation anchor。状态:NEW(见 [docs/adr/0082])。

## Source alignment evidence
Source reconciliation 为 Hybrid alignment unit 持久化的输入指纹绑定证据,记录来源区间、PDF 页/行区间与 `verified | format_equivalent | reviewed_hint | unmapped` 状态。只有 `verified`/`format_equivalent` 可作为确定性定位种子;`reviewed_hint` 只能缩小搜索范围并须重新验证,不能证明几何正确。状态:NEW(见 [docs/adr/0082])。

## PDF projection precision
每个子 LID 的 PDF 投影等级:`char_exact` 可支持字符级引用、复制和标注,`region_exact` 只支持导航或公式对象标记,`partial` 只允许显式受限操作,`unmapped` 只保留 PDF 原生选择/复制。单元级 bbox 不得冒充子 LID 的字符几何。状态:NEW(见 [docs/adr/0082])。

## PDF selection resolution basis
PDF 原生选区在 `resolved | partial | unresolved` 能力状态之外的正交解析依据。`exact` 表示所选字符均有已证实字符映射;`recovered` 表示唯一、单调的选区只存在版本化白名单内的表示差异,用户仍按已定位使用,但诊断不得冒充逐字符 exact。v1 白名单含布局空白与连字符类表示差异;v2 增加完整简单公式的排版表示差异,且必须同时满足确定性 source-display 投影、唯一 PDF glyph 序列和完整公式选择。新增类别仍须有真实失败样本、反例测试和确定性回归证据,不得按覆盖率阈值自动放宽。状态:BOUNDARY_CHANGE(见 [ADR-0090](docs/adr/0090-pdf-selection-recovered-resolution-and-versioned-discrepancy-policy.md))。

## Hybrid foundation integrity gate
混合阅读基座的二元安全门禁,校验输入指纹、artifact/hash/schema、LID/页身份、bbox 边界、单调顺序和无重复绑定;任一失败都使阶段失败且不得替换旧产物。它不包含覆盖率或质量阈值。状态:NEW(见 [docs/adr/0082])。

## PDF alignment quality tier
Hybrid foundation integrity gate 通过后的映射质量分级:`full` 达到版本化质量策略目标,`degraded` 未达目标但仍可进入 Reader。分级由 `unit_location_ratio`、按来源区间加权的 `exact_text_span_ratio`、`exact_formula_ratio` 与 `heading_location_ratio` 决定,不改变可信 `source.txt`。状态:NEW(见 [docs/adr/0082])。

## Versioned alignment benchmark
以版本号、输入哈希和人工真值冻结的 PDF 对齐回归集,包含可入库的许可小样本与哈希引用的本地真实论文样本;算法变更必须由确定性 runner 报告错误页、重复绑定和各质量指标,LLM 不承担通过判定。状态:NEW(见 [docs/adr/0082])。

## PDF visual source map
`pdf_source_map.json` 中从可信 LID/source span 到 PDF `pageIndex + bbox` 区域的轻量运行时映射。v2 为每个子 LID 标注 PDF projection precision;只有 `char_exact` 可承担字符级 citation/note/highlight 投影,`region_exact` 仅作导航或对象标记。它不替代 LID,也不是 citation anchor。状态:BOUNDARY_CHANGE(见 [docs/adr/0063]、[docs/adr/0082])。

## PDF selection map
后端私有的 char-level、按页分片 PDF 反解 artifact,用于把 PDF 选区或语义 LID/range 转成可保存或可显示的 LID range / PDF rect。v2 只有 `char_exact` 或 `partial` 中已证实的 exact 字符子区间可写入规范 LID/source span;`region_exact/unmapped` 不得伪造字符映射。状态:BOUNDARY_CHANGE(见 [docs/adr/0063]、[docs/adr/0082])。

## PDF 临时选区快照 (PDF selection draft)
PDF 原生拖选经 selection map 反解后形成的前端会话态:冻结规范 LID ranges、引用文本与工具条位置,只供用户显式选择高亮、笔记或 Ask AI;它不自动改变 reader 位置、不打开来源正文,在动作执行、取消或新选区替换时销毁。状态:NEW。

## PDF 选区翻译 (PDF selection translation)
BilingualAidLayer 对 PDF 临时选区提供的读时按需辅助:用户显式触发后,基于选中英文、规范 LID ranges 与论文术语上下文生成忠实中文译文。英文原文仍是唯一正文真相和 citation source;译文不进入 Agent 对话、不写 memory、不持久化。`paper_lexicon` 只在后台约束术语用词,不作为译文或独立展示内容。状态:NEW(2026-07-16 §0.5 PDF 选区翻译 Grill)。

## PDF 选区翻译浮层 (PDF selection translation surface)
锚在当前 PDF 选区旁、只显示中文译文的临时只读 surface。它不替换 PDF 正文,不承载术语讲解,关闭或产生新选区后销毁。状态:NEW(2026-07-16 §0.5 PDF 选区翻译 Grill)。

## PDF 用户标注投影 (PDF user annotation projection)
把用户主动保存的 Highlight 与 Note 从 memory 精确投影回原版 PDF 的可变展示层;它不显示自动 source-map regions,不改变 PDF/LID 真相,投影失败时不得用整段 bbox 猜测位置。状态:NEW。

## PDF 行内 Note 标记 (PDF inline note marker)
锚在 Note 持久化选区最后一个精确字符之后的小型交互标记;点击后打开与 Markdown Note 卡片一致的内容、编辑和删除界面。跨 LID/跨页选区以最后一个 range 作为显示锚,完整 ranges 保留为引用上下文;无法精确反向投影时只在 Notes 列表显示无法定位状态。状态:NEW。

## Ask AI 选区上下文 (Ask AI selection context)
Markdown 与 PDF 共用的临时引用草稿:保留现有首 LID 与完整 quote 展示,同时携带全部规范 ranges 和 `resolved/partial` 状态;它只在右栏等待用户输入问题,不自动发送、不写 memory、不改变 reader 位置,发送、清除或切换会话后销毁。状态:BOUNDARY_CHANGE。

## PDF-first reader surface
paper profile 的主阅读表面:中心区域在当前书具备可用 PDF capability 时渲染原版 PDF 页面,并通过 PDF visual source map 显示 LID 高亮、跳转和证据位置。Markdown/结构化正文仅作为调试、降级和未映射 LID 的 fallback 视图。状态:NEW(见 [docs/adr/0063])。

## Agentic paper minimap
paper reader 中常驻的辅助型全局导航:以 PDF 页/章节顺序为稳定坐标,默认只显示区域、当前位置和地标点,用户展开后才显示模式图层与局部论证关系。它不是摘要、阅读指南或第二主阅读面,agent 只能通过受控命令调整可撤销视图。状态:BOUNDARY_CHANGE(见 [docs/adr/0072])。

## Paper minimap base projection
由可信 LID tree、PDF visual source map、BookStructure、discourse 与 graph/Pass2 确定性组合的只读地图基座,包含区域、地标和证据化关系,不新增持久 paper truth。基础拓扑不可用时整图 unavailable;语义图层缺失时独立 degraded。状态:NEW(见 [docs/adr/0072])。

## Paper minimap mode lens
小地图的 `skim / abstract / deep` 展示口径:只改变地标筛选、强调和当前位置局部投影,不得重排 PDF/章节坐标,也不等同于 ReaderLayoutPreset。用户拥有模式控制权;agent 推断出的模式切换必须以 proposal 交由用户确认。状态:NEW(见 [docs/adr/0072])。

## Paper minimap position triad
小地图中相互独立的三个位置状态:`viewport_position` 表示用户实际看到的 PDF 范围,`selected_lid` 表示用户选择,`map_focus` 表示 agent 或用户当前强调。agent 改 map_focus 默认不触发正文导航。状态:NEW(见 [docs/adr/0072])。

## Paper minimap overlay
叠加在只读地图基座上的读者私有可变层:SessionOverlay 承载本会话焦点、强调和图层显隐,SavedUserOverlay 承载用户确认后跨会话保留的地标与覆盖。个人覆盖必须保留 provenance,不得改写派生地标或存入论文 workspace。状态:NEW(见 [docs/adr/0072])。

## Paper minimap action
人类与 resident agent 共用的 typed reader command,用于聚焦区域/地标、选择合法局部投影、调整图层和临时固定地标。后端 reducer 是最终权威:直接视图变化返回可撤销 effect,agent 推断的模式切换或长期状态变更返回 revision-bound proposal。状态:NEW(见 [docs/adr/0072])。

## 论文地图中文显示层 (paper minimap Chinese display layer)
面向中文母语读者的小地图可失效展示投影:首次加载时可由 LLM 把章节标题与地标描述批量转成中文,关系类型使用固定中文词表,模型、方法、数据集、指标和缩写等专有名词保留英文;英文正文、LID、citation、地图坐标与证据关系仍是唯一权威。结果按 book version 与 base fingerprint 缓存,滚动和跳转不得触发 LLM;Provider 不可用或输出无效时退回确定性中文类别与原始标签。状态:BOUNDARY_CHANGE(见 [docs/adr/0073])。

## Clean-room PDF extractor
不复制 AGPL 代码或资产的 PDF 抽取器实现边界。可以参考外部项目的架构分层和处理顺序,但不得复制源码、测试、模型、bundle、私有协议或受限资产;公共契约由本项目自定义。状态:NEW(见 [docs/adr/0063])。

## Alignment repair
PDF/Markdown 对齐失败时的可选修复机制。LLM 只能提出格式修复候选;是否进入可信 `source.txt` 必须由 content equivalence、确定性 aligner 和阈值决定,不得由 LLM 直接判断内容正确性或 page/bbox 正确性。状态:NEW(见 [docs/adr/0063])。

## Build Workbench
可信 `source.txt/base.json` 尚不存在、构建未完成或 source reconciliation 需要用户决策时使用的独立 build-mode 控制台。它承载 paper 输入上传/选择、draft workspace 创建、job 创建/续跑、server-side executor 启动、用户决策、执行权限审批、阶段 DAG、事件日志和 token/cost 观察;但不取代 `.build/<stage>` artifact 真相,也不把 job 状态当作 reader trust。状态:BOUNDARY_CHANGE(见 [docs/adr/0063] 与 `docs/切片方案-paper-pdf-first-hybrid.md` PH12-PH18)。

## Reader surface selection
paper workspace 的主界面由来源信任与 Hybrid foundation integrity gate 决定:来源未可信或完整性失败时显示 Build Workbench;来源可信且完整性通过后立即进入 Reader,PDF alignment quality tier 为 `degraded` 不得阻断。历史 job 状态与界面偏好不能改变该路由;可信书只允许用户从 Reader 主动打开 Workbench 诊断。状态:BOUNDARY_CHANGE(见 [docs/adr/0071]、[docs/adr/0082],取代 [docs/adr/0066])。

## Build controller
Build Workbench 背后的服务端控制层:负责把上传的 `paper.md/paper.pdf` 固化为未信任输入 manifest,按 fingerprint 创建/复用 `.build/jobs/<job_id>.json`,启动 Codex/opencode/Claude/manual 等 executor adapter,把 stdout/stderr/heartbeat/权限请求/用户决策写成 job events,并在每次阶段结束后重新运行确定性 artifact gate。它只能驱动构建,不能绕过 `.build/<stage>` artifact/hash/schema gate。状态:NEW(见 `docs/切片方案-paper-pdf-first-hybrid.md` PH12-PH18)。

## 一键预构建 (guided automatic prebuild)
用户用一个命令授权 build orchestrator 执行已确认 `BuildPlan` 的预构建方式。paper 的来源对齐与立即可读基础层仍由 Build Workbench 完成,非 paper 可从 Markdown/EPUB 原始输入生成基础层;尚无计划时不得把导入或打开一本书解释为全量语义构建授权。`standard_deep` 是当前完整产物的默认配方选项,只有用户显式选择或显式调用兼容命令时才进入确认计划。orchestrator 只在计划确认、预算漂移、额外权限或自动修复耗尽时暂停;普通阶段、执行器和恢复细节不暴露为用户工作。阶段完成仍只由 artifact、语义复用身份与 schema gate 判定。状态:BOUNDARY_CHANGE(见 [ADR-0093](docs/adr/0093-intent-confirmed-progressive-prebuild-and-reader-private-goal-artifacts.md),修订 [ADR-0067](docs/adr/0067-codex-plugin-one-command-prebuild.md) 的“全预构建”默认范围)。

## 立即可读基础层 (immediate readable foundation)
书籍导入后、第一次昂贵语义模型调用前形成的公共确定性基座。它至少包含可信 canonical source、LID、`book.text` 所需基座与词法检索输入;paper 还要求 source reconciliation 与 Hybrid foundation integrity gate 通过。Reader 是否可进入只依赖此层的信任与完整性,不得等待 Pass1、Pass2、BookStructure 或目标产物。状态:NEW(见 [ADR-0093](docs/adr/0093-intent-confirmed-progressive-prebuild-and-reader-private-goal-artifacts.md))。

## 标准语义配方 (standard semantic recipe)
以 `content_profile` 固定规则生成可跨阅读目标复用的公共语义层。`technical_learning` 当前包含 Pass1、profile sidecar 与 BookStructure,并由用户选择是否加入 Pass2 长程边增强;`paper` 另含 paper metadata、paper lexicon 与 PaperReadingGuide。Pass2 的选择必须进入已确认 `BuildPlan` 的依赖闭包,不由对话记忆或执行器临时推断。其配方 id 为 `standard_deep`,是产品提供的默认深读模板,不是导入后的默认执行授权;输入不得包含 `BuildIntent` 或 reader-private memory。状态:BOUNDARY_CHANGE(见 [ADR-0093](docs/adr/0093-intent-confirmed-progressive-prebuild-and-reader-private-goal-artifacts.md)、[ADR-0098](docs/adr/0098-optional-pass2-enrichment-for-book-structure.md))。

## BuildIntent
用户围绕当前书显式陈述并确认的版本化构建目标,记录目标、作用范围、期望输出、使用期限、来源 fingerprint 与隐私级别。自然语言原文只能作为 reader-private draft 输入,不得直接改变 source/LID/content profile、公共 Pass1/Pass2 prompt、BookStructure 或标准语义 artifact freshness;未确认、已取代或来源过期的 intent 不得授权模型构建。它属于 reader-private 状态,不是 Book truth、ReaderProfile 推断或 memory 自动结论;Codex 入口由当前 Codex 基于 `BuildPlanningContext` 形成 `BuildPlanningCandidate`,Reader UI 可由 fallback planner 形成同形候选,两者均须经 Core 编译后才成为 intent/plan draft。状态:BOUNDARY_CHANGE(见 [ADR-0093](docs/adr/0093-intent-confirmed-progressive-prebuild-and-reader-private-goal-artifacts.md)、[ADR-0094](docs/adr/0094-codex-designed-artifact-blueprints-and-versioned-registry.md)、[ADR-0096](docs/adr/0096-codex-authored-build-planning-reader-deterministic-authority.md))。

## BuildPlan
由 Core 根据当前 source/profile、`BuildIntent`、已验证 `BuildPlanningCandidate` 或用户显式选择的 `standard_deep` 编译出的版本化构建决策 artifact,列出目标产物、所用 `ArtifactBlueprint` 的 id/version、依赖闭包、可复用与新增工作、明确不生成项、token/墙钟估计及预算上限。Codex 或 Reader fallback planner 只能产生未授权 candidate;确定性 validator 必须校验 context freshness、blueprint、profile capability、依赖、LID scope 与隐私,用户一次确认精确 `plan_id + plan_revision` 后 orchestrator 才能执行昂贵工作。`plan_revision` 是 Core 所有、单调递增的计划代际,不再用小型计划对象的摘要充当修订号。它不是 semantic truth,预算、依赖或 blueprint 漂移超过已确认边界时必须回到 `needs_user`。状态:BOUNDARY_CHANGE(见 [ADR-0093](docs/adr/0093-intent-confirmed-progressive-prebuild-and-reader-private-goal-artifacts.md)、[ADR-0094](docs/adr/0094-codex-designed-artifact-blueprints-and-versioned-registry.md)、[ADR-0096](docs/adr/0096-codex-authored-build-planning-reader-deterministic-authority.md)、[ADR-0115](docs/adr/0115-root-shared-executor-mcp-and-subagent-inheritance.md))。

## BuildPlanningContext
Reader 为一个可信 current workspace 投影的有界只读规划上下文,包含 book/source fingerprint、content profile、scope catalog 摘要、可用 Blueprint 摘要、候选合同版本与规模上限,并以 Core 所有的 `context_id + context_revision` 绑定。它不包含 raw goal、私有 intent/plan/artifact 正文、API key 或 filesystem path,调用不得创建 intent、plan、usage、task 或 lease;source/profile/Registry/合同上限漂移后旧 revision 必须拒绝提交。状态:BOUNDARY_CHANGE(见 [ADR-0096](docs/adr/0096-codex-authored-build-planning-reader-deterministic-authority.md)、[ADR-0115](docs/adr/0115-root-shared-executor-mcp-and-subagent-inheritance.md))。

## BuildPlanningCandidate
Codex 或 Reader UI fallback planner 根据显式目标与 `BuildPlanningContext` 产生的严格、未授权语义候选,声明 goal kind、source scope、usage horizon 及 0..N 个系统/私有/one-off `ArtifactBlueprint` 选择。它不是 `BuildIntent`、`BuildPlan` 或执行授权,不得自带可信 plan identity;Reader 必须按 `context_id + context_revision` 重读 current context,Core 必须重新解析 Blueprint 并编译唯一 reader-private draft。Codex 入口不得在失败时静默改由 Reader 模型二次规划。状态:BOUNDARY_CHANGE(见 [ADR-0096](docs/adr/0096-codex-authored-build-planning-reader-deterministic-authority.md)、[ADR-0115](docs/adr/0115-root-shared-executor-mcp-and-subagent-inheritance.md))。

## ArtifactBlueprint
Codex 为一个目标产物选择或设计的版本化数据元合同,冻结用途、通用展示形态、受限 record/relation schema、可搜索与摘要字段、Routing Card、逐记录 LID 证据策略及规模上限。它不携带书中内容、任意可执行代码或自定义渲染器;`blueprint_id + blueprint_version` 直接进入 `BuildPlan` 身份,Runtime 只按受限 DSL 与确定性 gate 验收。现有 timeline、concept map、comparison table、argument map 降为内置 Blueprint,不再构成封闭类型枚举。状态:BOUNDARY_CHANGE(见 [ADR-0094](docs/adr/0094-codex-designed-artifact-blueprints-and-versioned-registry.md)、[ADR-0115](docs/adr/0115-root-shared-executor-mcp-and-subagent-inheritance.md))。

## ArtifactBlueprint Registry
按稳定 identity、显式版本和状态管理 `ArtifactBlueprint` 的注册表；同一 `blueprint_id + blueprint_version` 的 schema 直接比较,不得再以小型 schema 摘要代替版本所有权。系统层随软件发布内置通用 Blueprint;用户私有层只保存 Codex 基于真实目标形成的 schema 候选及使用记录,不保存书中内容,可跨相似目标复用;候选只有经人工评审才可晋升为系统预设。Registry 为空不阻塞 Codex 设计一次性 Blueprint。状态:BOUNDARY_CHANGE(见 [ADR-0094](docs/adr/0094-codex-designed-artifact-blueprints-and-versioned-registry.md)、[ADR-0115](docs/adr/0115-root-shared-executor-mcp-and-subagent-inheritance.md))。

## ArtifactRoutingCard
语义字段随 `ArtifactBlueprint` 确认并冻结、实例计数由 accepted payload 确定性补全的有界路由卡,描述产物的 title、purpose、`use_when`、`avoid_when`、covered topics、scope label、可搜索字段和记录数,用于判断当前问题是否值得调用产物工具。它不是书源证据,不得证明书中事实,也不得包含原始用户目标、BuildIntent 或 BuildPlan 正文。状态:NEW(见 [ADR-0095](docs/adr/0095-active-artifact-read-surface-and-book-mcp-boundary.md))。

## ArtifactAccessSnapshot
Resident Agent 在一个用户回合开始时冻结的 active + accepted 目标产物读取视图,包含 overlay revision、Routing Cards、blueprint/payload identity 和只读记录访问能力。该回合内 `artifact.search/read` 必须保持同一 revision;目标 replan、换源、失效或删除不能让一次工具循环跨 overlay 混读。Book MCP 每次工具调用独立解析同等门禁的当前 snapshot。状态:NEW(见 [ADR-0095](docs/adr/0095-active-artifact-read-surface-and-book-mcp-boundary.md))。

## Codex 构建意图入口 (Codex build-intent entry)
Codex plugin 面向同一 reader-private `BuildIntent/BuildPlan` 权威提供的对话入口:用户在 Codex 陈述目标后,Desktop-owned stdin controller 先为显式目标 workspace 返回 `BuildPlanningContext`;当前 Codex 直接选择/设计 Blueprint 并提交绑定 `context_id + context_revision` 的 `BuildPlanningCandidate`,Reader/Core 确定性编译和持久化可审阅 draft。Codex 或 Reader 只能按当前 `plan_id + plan_revision` 显式确认,随后由一键预构建执行公共依赖闭包与私有目标产物。它不调用 Reader provider 二次解释 Codex goal,不复制私有 store,不经 Visitor/Book MCP 暴露 intent,不把原始目标或 candidate 写入 argv、公共 stdout/stderr、临时公共文件或书目录。状态:BOUNDARY_CHANGE(见 [ADR-0093](docs/adr/0093-intent-confirmed-progressive-prebuild-and-reader-private-goal-artifacts.md)、[ADR-0096](docs/adr/0096-codex-authored-build-planning-reader-deterministic-authority.md)、[ADR-0115](docs/adr/0115-root-shared-executor-mcp-and-subagent-inheritance.md))。

## Codex 会话确认 (Codex conversation confirmation)
用户在 Codex 对话中审阅当前计划摘要后,对精确 `plan_id + plan_revision` 作出的显式构建授权,持久化来源为 `codex_conversation`。它与 `reader_ui` 具有相同计划门禁,不能预先授权未知计划、复用旧 revision 或由 agent 代替用户确认;旧完整构建兼容来源仍为 `explicit_legacy_command`。状态:BOUNDARY_CHANGE(见 [ADR-0093](docs/adr/0093-intent-confirmed-progressive-prebuild-and-reader-private-goal-artifacts.md)、[ADR-0115](docs/adr/0115-root-shared-executor-mcp-and-subagent-inheritance.md))。

## 目标产物层 (intent artifact overlay)
围绕单个已确认 `BuildIntent` 生成、物理上位于 reader-private store 的数据型产物集合。每个产物必须绑定 source fingerprint、`intent_id + intent_revision`、`plan_id + plan_revision`、`blueprint_id + blueprint_version`、受限输出 schema 与真实 LID evidence,并与公共 graph、profile sidecar、BookStructure 和书目录根 artifact 物理隔离;目标改变时只取代对应 overlay,不得使仍 fresh 的公共基础层或标准语义层失效。Resident Reader/Agent 与绑定该书的 Book MCP 只可读取当前 active + accepted snapshot;Intent、Plan、candidate、failure、历史 overlay、原始目标和执行记录仍不可见。状态:BOUNDARY_CHANGE(见 [ADR-0093](docs/adr/0093-intent-confirmed-progressive-prebuild-and-reader-private-goal-artifacts.md)、[ADR-0095](docs/adr/0095-active-artifact-read-surface-and-book-mcp-boundary.md)、[ADR-0115](docs/adr/0115-root-shared-executor-mcp-and-subagent-inheritance.md))。

## 分阶段任务租约 (phase-aware task lease)
一键预构建中语义任务所有权的限时凭证,区分等待专用 executor 接管的 `reserved` 阶段与已经开始处理输入的 `running` 阶段,两阶段拥有独立截止时间。租约只控制任务执行权和中断恢复,不证明语义产物完成;正常任务必须依靠保守运行期限即可完成,heartbeat 只延长仍匹配 target/source/policy/owner 的活动运行租约。状态:BOUNDARY_CHANGE(见 [ADR-0092](docs/adr/0092-phase-aware-automatic-build-leases-and-executor-dispatch-bundles.md))。

## 语义尝试 (semantic attempt)
同一 policy-bound work unit 为生成有效语义候选而进行的一轮模型推理。只有 schema、evidence、provider 或语义输出失败并需要重新生成候选时才递增;executor 未接管、租约过期或相同候选的编码重传不得伪装成新的语义尝试。状态:BOUNDARY_CHANGE(见 [ADR-0092](docs/adr/0092-phase-aware-automatic-build-leases-and-executor-dispatch-bundles.md))。

## 租约世代 (lease epoch)
同一语义尝试因 owner、进程、heartbeat 或调度中断而重新领取时递增的执行所有权世代。每个世代有独立 token 与阶段时间线;过期世代永久不能 submit,但不消耗语义尝试上限。状态:NEW(见 [ADR-0092](docs/adr/0092-phase-aware-automatic-build-leases-and-executor-dispatch-bundles.md))。

## 恢复代际身份 (recovery generation identity)
同一 Executor 调度运行中当前可启动一次的控制面恢复身份,由 `dispatch_id + dispatch_run_id + current_work_unit_id + semantic_attempt + lease_epoch` 确定。相同恢复代际必须返回同一 opaque handoff ref 并至多启动一个专用 Executor;当前工作单元、语义尝试或租约世代变化时必须形成新 ref。它不进入 task binding、attempt scope、artifact path 或语义复用身份,不得使仍 fresh 的 accepted artifact 失效。状态:NEW(见 [ADR-0117](docs/adr/0117-recovery-generation-handoff-and-semantic-result-reuse.md))。

零调用启动失败后，恢复身份还区分启动重试代际（bootstrap epoch）。它记录同一工作单元、语义尝试与租约世代内已结束的启动尝试，使重试获得新 handoff，同时保持 dispatch slot 和语义复用身份不变。重复上报同一次失败不推进代际。见 [启动恢复修复](docs/修复-executor-bootstrap-20260907.md)。

## Executor 调度槽身份 (executor dispatch slot identity)
Root 在同一 `dispatch_id + dispatch_run_id` 内限制 live child 数量的不透明控制键。它跨 work unit、semantic attempt 与 lease epoch 保持稳定,只用于保证一个调度运行同时最多有一个专用 Executor;Root 不把它传给 child,它也不进入 handoff 恢复身份、task binding、attempt scope 或 artifact freshness。状态:NEW(见 [ADR-0117](docs/adr/0117-recovery-generation-handoff-and-semantic-result-reuse.md))。

## 提交修订 (submit revision)
同一活动租约内对同一语义候选执行确定性编码修复或幂等重传的序号。它不得改变候选语义、input hash 或 policy identity;一旦需要重新推理,必须进入新的语义尝试。状态:NEW(见 [ADR-0092](docs/adr/0092-phase-aware-automatic-build-leases-and-executor-dispatch-bundles.md))。

## Executor 调度执行包 (executor dispatch bundle)
Build Engine 把多个既有、同 target/stage/policy/kind 的 work unit 临时编组的调度信封。每个 work unit 保留独立任务租约、input、candidate mailbox、receipt、失败恢复和 artifact identity；执行包不是新的语义 work unit，也不等同于某个专用 child 或连接的生命周期，不把多个输入合成一次模型判断。见 [ADR-0092](docs/adr/0092-phase-aware-automatic-build-leases-and-executor-dispatch-bundles.md)、[ADR-0117](docs/adr/0117-recovery-generation-handoff-and-semantic-result-reuse.md)、[ADR-0121](docs/adr/0121-bounded-build-state-reads-and-executor-lifecycle.md)。

## Executor 调度运行 (executor dispatch run)
同一确定性 `dispatch_id` 的一次有界执行周期,由独立 `dispatch_run_id` 标识。Planner identity、work unit 和 artifact freshness 不随运行重试改变;semantic failure、executor interruption 或恢复重排后可为相同 manifest 创建新的 run-scoped mailbox,旧 run 的 progress/receipt 保持 append-only 且不得吞掉新 run。状态:NEW(见 [ADR-0092](docs/adr/0092-phase-aware-automatic-build-leases-and-executor-dispatch-bundles.md))。

## Opaque handoff ref
由确定性预构建代码签发、供 root Codex 原样转交给专用 executor 的有界 ASCII locator。它不等同于文件路径,不暴露 workspace、dispatch、attempt、hash 或 mailbox identity；其真实绑定只由代码私下持久化并在消费点重验。状态:NEW(见 [ADR-0101](docs/adr/0101-deterministic-prebuild-protocol-ownership-and-codex-semantic-boundary.md))。

## 候选传输合同 (candidate transport contract)
生成侧在产生候选前可见的基础设施准入边界,分别公开候选值与完整序列化提交请求的 token/UTF-8 byte 上限。它只决定一次候选能否进入 Executor transport；合同版本和预算不属于 policy generation、semantic contract 或 artifact freshness。状态:BOUNDARY_CHANGE(见 [ADR-0117](docs/adr/0117-recovery-generation-handoff-and-semantic-result-reuse.md))。

## Executor open
专用 executor 用 opaque handoff ref 开启语义执行会话的原子消费边界。它在返回任何语义输入或取得任务执行权前,由代码完成 ref、路径、handoff、prompt、manifest、dispatch identity 与当前终态的重验；无效或漂移输入必须失败关闭且不得创建语义尝试。状态:NEW(见 [ADR-0101](docs/adr/0101-deterministic-prebuild-protocol-ownership-and-codex-semantic-boundary.md))。

## Executor bootstrap contract
专用 executor 在消费 opaque handoff ref 之前必须已经获得的版本化角色指令,精确定义 Build Engine 解析、`executor.open/executor.session` 循环、action 分支、candidate 私有边界与有界终态。它不是 semantic prompt、root build skill 或一句启动提示,不得依赖当前工作目录发现源码后再推断。状态:NEW(见 [ADR-0102](docs/adr/0102-dedicated-executor-bootstrap-role-isolation-and-distribution.md))。

## Executor role registration
把版本化 executor custom-agent 模板显式安装到个人或项目 Codex agent 配置、使其成为可选 `agent_type` 的所有权边界。注册以规范化全文直接相等保证幂等；同名内容不同时失败关闭且不覆盖,已知前代只在用户显式迁移并先按原始字节备份后替换。插件携带 TOML asset 不等于宿主已自动注册该角色。状态:BOUNDARY_CHANGE(见 [ADR-0102](docs/adr/0102-dedicated-executor-bootstrap-role-isolation-and-distribution.md)、[ADR-0115](docs/adr/0115-root-shared-executor-mcp-and-subagent-inheritance.md))。

## HERO 哈希用途门 (HERO hash-utility gate)
预构建与 Executor 控制面采用的范围门:H 禁止为小配置、budget proof、版本、receipt/evidence 或已在手中的正文再造摘要,只有内容身份替代复制大型 source/model input/published handoff/candidate/accepted artifact,且比较会改变昂贵结果复用、fresh/stale、snapshot 或幂等 replay 动作时才保留；E 不为本机合作者范围虚构攻击者、账号或部署加固；R 每项检查必须对应一个具体失败和不同的后续动作,不得重复审计或设置永不放行的门；O 不允许以上一层守卫为理由再叠校验层。Opaque ref 的派生值只作为有界持久 locator,不得当完整性或 caller-role 证明。状态:NEW(见 [ADR-0115](docs/adr/0115-root-shared-executor-mcp-and-subagent-inheritance.md))。

## 语义复用身份 (semantic reuse identity)
决定既有昂贵模型结果、accepted artifact 或大型 candidate/handoff 正文能否复用的最小身份:可信 source/base 内容身份、work-unit 实际 input hash、一次 prompt 内容身份、显式 semantic contract version,以及确有 snapshot/freshness/idempotent replay 消费者的 artifact/payload/candidate 内容 hash。Estimator、reserve、transport profile、lease、budget proof、policy-set 包装、receipt 与运行时间属于执行证据,不得使仍匹配上述身份的语义 artifact 失效；它们在下一次 claim/execution 时按原字段重算或直接比较。状态:NEW(见 [ADR-0115](docs/adr/0115-root-shared-executor-mcp-and-subagent-inheritance.md))。

## 发布接管 (release takeover)
前 1.0 本机持久合同换代时保留旧历史与仍满足语义复用身份的 accepted artifact,不把旧控制记录原地改写成新合同；未完成工作由用户重新确认的当前计划与当前协议接管。派生控制结果从当前事实重算,旧 attempt/session/receipt 继续只读保留。状态:BOUNDARY_CHANGE(见 [ADR-0115](docs/adr/0115-root-shared-executor-mcp-and-subagent-inheritance.md))。

## 共享 Executor MCP 注册 (root-shared Executor MCP registration)
把 `understand_book_build_executor` MCP 登记到 Codex parent/root 会话的插件配置层,使受有界角色指令约束的 `understand_book_executor` subagent 可从 parent 继承 `executor.open`、`executor.input.next`、`executor.generation.start` 与 `executor.submit_candidate`。Root 与其他继承 parent 配置的 subagent 因此也可能在宿主能力上调用这四个工具,但协议只允许专用 Executor child 调用;正常语义流仍只得进入该 child 交互和代码所有的 candidate mailbox。该边界不再满足 `root_toolset ∩ executor_tools = ∅`;非 Executor 调用禁令由指令合同与安装态 trace 审计约束,Build Engine 继续保护 ref、phase、lease、schema、evidence 与写入完整性,但不提供 caller-role 硬隔离。状态:BOUNDARY_CHANGE(见 [ADR-0115](docs/adr/0115-root-shared-executor-mcp-and-subagent-inheritance.md))。

## Codex plugin
以 `.codex-plugin/plugin.json` 打包、由 Codex 安装和加载的本地预构建 harness 外壳。它通过 `$understand-book-build` 驱动现有确定性脚本与专用语义抽取契约,并可经 Codex 构建意图入口读写 Desktop-owned 的同一 reader-private 计划;正常条件下一次已确认执行完成、外部中断后按磁盘产物幂等续跑。paper 只消费 Build Workbench 已可信的混合阅读基座;非 paper 可从 Markdown/EPUB 原始输入开始。它不是 Web 内置模型 worker,不嵌入 Codex app-server,不经 Visitor/Book MCP 读取私有计划,也不能绕过 plan、artifact、语义复用身份与 schema gate。状态:BOUNDARY_CHANGE(2026-07-26 IP11)。

## Book MCP Sidecar
随 Windows Setup 安装、由 Codex plugin 声明和启动的只读本地 MCP 可执行程序。一个进程在启动时绑定一本可信 Book workspace,静态投影 canonical Book tools 与通用 artifact list/search/read 工具;artifact 调用只能读取同一 OS 用户、同一本书的 current active + accepted snapshot。它不读取 BuildIntent、BuildPlan、candidate、历史 overlay、resident memory、画像、Agent 历史或 Provider 设置。无显式书目录时可读取 Reader session 的当前书路由指针,但该指针不成为新的工具面。状态:BOUNDARY_CHANGE(详见 [docs/adr/0089]、[ADR-0095](docs/adr/0095-active-artifact-read-surface-and-book-mcp-boundary.md))。

## current-book MCP binding
Book MCP 进程级的单书绑定:优先使用显式 CLI 目录,其次 `UNDERSTAND_BOOK_DIR`,最后使用 Reader 最近持久化且仍可加载的当前书。绑定在进程生命周期内不随 Reader 切书漂移;新 MCP 进程才重新解析。状态:NEW(详见 [docs/adr/0089])。

## Desktop Reader App
把现有 Vue reader 与 Rust localhost REST host 封装为 Windows 桌面应用的产品表面。第一版使用 Tauri 2 + WebView2,保留同源 REST 契约,支持无当前书启动、默认书库和外部 `.understand-book/<book_id>` workspace 注册且不复制产物。状态:BOUNDARY_CHANGE(详见 [docs/adr/0068])。

## 用户书库根目录 (user library root)
Desktop Reader App 中由当前用户选择、跨启动保持的唯一书库根路径,决定书库扫描范围与新建书写入位置。用户可选择普通父目录或现有 `.understand-book`;前者解析为其下的 `.understand-book`,后者直接复用。切换不迁移、不复制、不删除旧书库,当前已打开的书保持不变。状态:NEW(详见 [docs/adr/0069])。

## 桌面 Provider 设置 (desktop provider settings)
Desktop Reader App 当前用户为读时 LLM adapter 提供的模式、Base URL、Model 与 API Key 配置。保存成功后立即替换当前 server 的 adapter,不重启、不改变当前书与会话;保存只做确定性格式校验,不自动产生模型请求。API Key 暂按用户明确接受的风险明文保存在用户级设置中,界面不回显已存值。状态:NEW(详见 [docs/adr/0070])。

## Build Engine Sidecar
随 Desktop Reader App 安装、由 Codex plugin 调用但不直接展示的确定性预构建可执行程序 `understand-book-build.exe`。它承载 resolve/next/input/write/close/gate 与 protocol doctor,不执行语义模型;用户不需要安装 Node、pnpm 或 Cargo。状态:NEW(详见 [docs/adr/0068])。

## Windows Installer
用户首次运行的 per-user NSIS `UnderstandBookSetup.exe`:安装桌面阅读器与 Build Engine Sidecar,经明确授权后从公开 Git marketplace 安装 Codex plugin。阅读器安装与插件安装分事务;Codex/网络失败只产生可重试的待安装状态。状态:NEW(详见 [docs/adr/0068])。

## Plugin Installation Receipt
Windows Installer 对 Codex plugin 安装所有权的用户级记录。只有 `installed_by_reader_setup=true` 的插件才由阅读器卸载器默认提议移除;书库、`.understand-book` 与阅读记忆默认不删除。状态:NEW(详见 [docs/adr/0068])。

## Codex marketplace source migration
Understand Book 将同名 Codex plugin marketplace 从旧 source 切换到当前发布 source 的受控更新。只有 Plugin Installation Receipt 同时匹配 plugin/marketplace 名且声明该 marketplace 由 Setup 创建时,Reader 才可删除旧 plugin/marketplace 后重新添加;无回执、名称不匹配或外部拥有的 marketplace 必须保持不变并返回人工处理提示。同一 source 的正常更新只刷新 marketplace snapshot。状态:BOUNDARY_CHANGE(详见 [docs/adr/0068])。

## Draft paper workspace
Build Workbench 接收 `paper.md + paper.pdf` 后形成的未信任论文构建目录。它可以保存 canonical 输入副本和 `.build/input/manifest.json`,但在 source reconciliation 与 artifact gate 通过前不得被 normal reader 当作可信书。状态:NEW(见 `docs/切片方案-paper-pdf-first-hybrid.md` PH12)。

## Workbench input manifest
Draft paper workspace 中记录原始 `paper.md/paper.pdf` 路径、sha256、profile、display title、config hash 与 current input fingerprint 的未信任 manifest。它只用于构建编排和 stale 判断,不替代 `.build/<stage>` accepted artifact。状态:NEW(见 `docs/切片方案-paper-pdf-first-hybrid.md` PH12)。

## Executor run contract
Build controller 为一次 server-side executor run 写出的未信任执行契约,包含 stage、scoped workdir、prompt、input manifest、允许输出路径、command summary 和权限策略。它只约束 executor 行为,不证明 stage artifact 可信。状态:NEW(见 `docs/切片方案-paper-pdf-first-hybrid.md` PH14)。

## BuildDecisionRequest
Build Workbench 中影响构建方向或阶段 readiness 的用户选择请求,区别于 Codex/opencode/Claude 等 executor 的工具权限请求。其答案必须写入 job event,若影响构建结果还要写入 stage decision artifact。状态:NEW(见 [docs/adr/0063])。

## Agent 原生学习环境
Agent 可理解、可观察、可行动并获得真实反馈的学习现场，由已有公共教学资产、私人学习状态、当前阅读与交互状态、对象关系和操作能力共同构成；每回合自动提供稳定环境说明和精简当前现场，相关细节按需读取，表现判断须取得对应的实际呈现、帮助条件和用户回应。系统维护真实状态、对象语义和动作结果，Agent 结合目标与环境作具体教学选择；环境保留事实与判断的区别，不新增独立的可编辑真相。状态:NEW(见 `grill.md` Q90/Q91、[ADR-0119](docs/adr/0119-agent-native-learning-environment-and-teaching-agency.md))。

## 正式学习就绪
整本书或整篇论文的可信来源、全局结构、正式对象与关系、重点认知素材共同满足覆盖与来源要求的状态，是启动正式 TutorLoop 的前置条件。它与原文可读及用户是否开启 Tutor 分别表达；已如实保留的来源缺口限制依赖该缺口的教学判断，尚未完成的必需构建工作仍阻止就绪。状态:BOUNDARY_CHANGE（[ADR-0120](docs/adr/0120-whole-source-prebuild-gate-for-formal-learning.md)、[ADR-0142](docs/adr/0142-grounded-teaching-map-and-whole-source-readiness.md)）。

## TeachingMap
公共、版本化的教学资产边界，以可信原文及其可复用公共语义产物为依据，描述“可以教什么”以及可复用的认知推进素材；正式对象与关系承接经过语义整理和来源确认的内容，教学前置依赖注明能力目标与成立条件。`KnowledgeSpace`、`ReasoningPath`、`ReasoningPattern` 与后续教学步骤结构均是其内部数据或投影；它不保存任何用户私人学习状态。状态:NEW(见 `grill.md` Q72/Q85/Q99/Q100)。

## 教学前置依赖
`TeachingMap` 内注明目标对象及能力、所需前置对象及能力、适用条件和依据的公共依赖关系，描述完成相应目标所需的内容条件。读时 Agent 结合当前目标、私人学习证据与可提供的帮助决定实际教学顺序；关系本身不表示某个用户已具备或欠缺前置能力，也不等同材料的出现顺序。状态:NEW(见 `grill.md` Q100)。

## LearningMemory
统一拥有读者私人稳定上下文、全局 Tutor 控制、教学会话与动态学习证据的逻辑边界。`ProfileFact` 表达稳定背景与偏好，`TutorControl` 表达显式教学开关，独立 `TutorSession` 表达本次学习意图、焦点、材料范围与默认教法；对象级理解判断由 `InteractionTrace → LearningEvidence → LearnerKnowledgeState` 证据链表达，路径进度和用户理解空间是可重建的私人投影。状态:BOUNDARY_CHANGE（[ADR-0141](docs/adr/0141-global-tutor-control-and-session-ownership.md)、[ADR-0143](docs/adr/0143-teaching-trace-assessment-and-learning-evidence.md)）。

## 全局 Tutor 模式（TutorControl）
用户对整个应用是否启用持续教学的显式控制，跨阅读、聊天、演示页与应用重启延续；关闭会暂停当前教学并保留学习进展，开启后依据有效会话、当前材料与正式学习就绪状态继续或开始教学。它由 LearningMemory 拥有，与会话默认教法、结束某次学习及本回合直接讲解分别表达。状态:NEW（[ADR-0141](docs/adr/0141-global-tutor-control-and-session-ownership.md)）。

## User Understanding Space（用户理解空间）
`LearningMemory` 内以公共正式学习对象及可学习关键关系为共同参照的私人理解视图，结合实际表现、用户个人解释与有证据的可修正理解假设。它由既有 `InteractionTrace`、`LearningEvidence` 和 `LearnerKnowledgeState` 的相关内容投影，区分用户原话与系统推断，无证据保持未知，不独立拥有可编辑学习状态或改写公共 `TeachingMap`。状态:NEW(见 `grill.md` Q97)。

## LearnerContext
`TutorLoop` 在一个读者回合开始时冻结的有界私人上下文，由相关稳定画像、有效 TutorSession 的用户意图、当前学习焦点与默认教法、当前 `PathProgress`、相关 `User Understanding Space` 片段与必要的新鲜度状态合成，是 Agent 原生学习环境中的读者状态视图。它只服务本回合教学决策，不是完整环境或持久真相，不写入对话历史，也不得把画像事实、会话默认或交互意图改写成对象级掌握判断。状态:BOUNDARY_CHANGE（[ADR-0143](docs/adr/0143-teaching-trace-assessment-and-learning-evidence.md)）。

## ResolvedInteractionIntent
`TutorLoop` 对当前回合实际采用的有界交互意图，按“本回合显式需求 → 用户显式建立的 `TutorSessionMode` → 已确认 `ExplanationPreference` → 中性兜底”解析，并保留生效来源及有效教学会话的 revision 引用。它只支配本回合 `TeachingMove` 选择，可临时覆盖教法而不改写会话默认，不是 `TaskNeed`、长期偏好、会话合同或能力证据。状态:BOUNDARY_CHANGE(见 `grill.md` Q86/Q87)。

## ResidentGoal（住户任务目标）
用户在当前 Resident 聊天内要求完成的一项任务，保留用户意图、范围、明确交付要求以及可修订的工作判断，并关联实际成果与尚未完成的义务；一次运行停止不等于任务完成。它与长期读者目标、构建目标及教学会话学习意图分别表达，完成交付不等于读者掌握。状态:BOUNDARY_CHANGE（任务保存、调整、投影和实际交付检查已实现，见 [ADR-0136](docs/adr/0136-resident-goal-lifecycle-and-delivery-completion.md)；工作计划扩展已实现，见 [ADR-0155](docs/adr/0155-goal-work-plan-and-version-centered-presentation-context.md)）。

## Goal 工作计划
ResidentGoal 内由 Agent 维护、随实际观察修订的工作安排，用工作项及待做、进行中、已完成状态表达推进顺序与当前进展。工作项是达成用户要求的方法，可以重排、拆合或放弃；它不自行改变用户要求，不构成独立任务，也不能以全部勾选代替实际交付或内容完整性判断。状态:NEW（[ADR-0155](docs/adr/0155-goal-work-plan-and-version-centered-presentation-context.md)，EX13.3 已实现，2026-10-02）。

## TutorSession
`LearningMemory` 独立持有的可回放私人教学会话，分别表达用户学习意图与约束、Agent 当前学习焦点、参考材料范围及默认教法，并引用私人路径与进度。首版仅有一个当前会话，可跨聊天和重启延续，历史会话可显式恢复；生命周期由用户动作决定，翻页、换材料、聊天压缩或一次表现不会自行切换目标或结束学习。状态:BOUNDARY_CHANGE（[ADR-0118](docs/adr/0118-learning-memory-owned-replayable-tutor-session.md)、[ADR-0141](docs/adr/0141-global-tutor-control-and-session-ownership.md)）。

## 会话学习意图
用户希望在 TutorSession 中完成的事及其明确约束，允许最初表达模糊，与 Agent 当前采用的暂定学习目标及参考材料范围分别表达。改变最终目标、要求达到的能力深度或新增持续学习任务，由用户表达或接受方向变更；局部教学焦点与参考材料调整不覆写它，也不等同于支配本回合交互方式的 `ResolvedInteractionIntent`。状态:NEW(见 `grill.md` Q93)。

## 暂定学习目标
Agent 基于当前材料、相关历史与会话学习意图采用的当前学习焦点，可随实际回应细化，用于组织起步及接下来的教学动作。它保留 Agent 解释的身份，不覆写用户已表达的会话学习意图，不是已确认的长期目标或能力证据，完整画像与细化目标不成为开始学习的前置条件。状态:NEW(见 `grill.md` Q92/Q93)。

## TutorSessionMode
由 `TutorSession` 持有、经用户显式动作建立或变更的会话默认教法，仅在全局 Tutor 开启、该会话有效且请求属于其范围时参与本回合交互意图的解析。当前明确要求直接讲解可以覆盖它，Agent 的局部教学选择不改变它；它不表示整个应用是否启用 Tutor。状态:BOUNDARY_CHANGE（[ADR-0141](docs/adr/0141-global-tutor-control-and-session-ownership.md)）。

## TutorLoop
Agent 在原生学习环境中感知、选择、行动并利用反馈的读时教学循环，以全局 Tutor 开启、有效教学会话与正式学习就绪为前提。它从当前明确请求、会话教法及已确认偏好解析交互意图，结合相关私人证据和公共素材选择一个有界 TeachingMove，根据实际交付与显式用户行为适配下一步；系统维护真实状态与证据，具体教学选择由 Agent 作出，PathPlanner、ContextBuilder 与 Estimator 是其内部职责。状态:BOUNDARY_CHANGE（[ADR-0143](docs/adr/0143-teaching-trace-assessment-and-learning-evidence.md)）。

## TeachingMove
`TutorLoop` 围绕当前交互意图、稳定 `LearningObjectRef`、学习对象、目标能力、Learner 状态与来源条件选择或合成的一个有界认知动作。它可以是问题、检索线索、例子/反例、对比、来源聚焦、关系支架、程序动作或局部解释，不以问句为本体、不默认要求判题，也不允许用完整答案或连续题目替代 Learner 本应执行的目标认知活动；来源推论未闭合时，可围绕证据缺口推进理解，补充解释须保留假设身份。状态:BOUNDARY_CHANGE(见 `grill.md` Q85/Q88)。

## GuidedInquiryPolicy
`TutorLoop` 在 `ResolvedInteractionIntent` 为引导学习且当前对象、能力目标与 Learner 前置状态适合主动生成、辨析、预测或重建时，优先让 Learner 先执行目标认知活动的 `TeachingMove` 选择策略。它不是全局“全程提问”流程、content profile 或评分模式；本回合明确要求直接答案、总结、翻译或完整解释时不得启用。状态:BOUNDARY_CHANGE(见 `grill.md` Q85/Q86)。

## ReasoningEpisode
`TeachingMap` 内围绕一个目标问题、claim 或公式形成的公共、源锚定作者论证片段，由有序推理步骤、步骤间关系、来源层、Evidence LID 与显式缺口组成。其范围是支撑目标所必需的最小完整论证链：缺失连接显式留洞并停止扩张，不按固定 LID 窗口或整章截取。它描述作者如何从前提走向目标，不决定针对某个读者如何教学，也不是整本书的唯一完整思考路径。状态:NEW(见 `grill.md` Q73/Q75)。

## ReasoningPattern
`TeachingMap` 内可跨段落、问题、案例或资源复用的关系与认知组织模式，例如定义边界、分类轴、因果机制、证据—主张连接、比较框架、程序依赖、数量关系或问题求解约束。它可以由一个或多个源锚定 `ReasoningEpisode` 或其他教学资产支撑，是可选的教学素材而非所有内容的统一解释模板；它不等于按题号记答案、按章节背摘要，也不保存某个 Learner 是否掌握。状态:BOUNDARY_CHANGE(见 `grill.md` Q69/Q85)。

## ReasoningTarget
构造一个 `ReasoningEpisode` 时要解释的明确入口。`BookStructure.key_stop` 是默认预构建目标；key_stop 之外的源锚定 claim、公式或当前问题可在被明确请求时成为按需目标。它只指定“从哪里开始解释”，不预先规定 Episode 的完整范围。状态:NEW(见 `grill.md` Q74)。

## LearningObject（正式学习对象）
`TeachingMap` 拥有的有来源依据、可独立讨论的内容单位，按需要分别追踪的不同理解内容确定粒度，允许复合对象与子对象并存。其公共身份独立于用户表现、教法、具体活动和帮助条件；同一内容的识别、解释、应用等由能力维度分别表达，组织或包含关系本身不推导掌握。承载独立理解内容的关键关系也可以成为正式学习对象。状态:NEW(见 `grill.md` Q79/Q97/Q98)。

## LearningObjectRef
`TeachingMap` 为经语义整理与来源确认的正式学习对象分配的稳定引用，内容修订与对象身份分别表达；可学习关系由同一引用统一拥有具体含义、参与对象、角色、条件与依据，图中连线引用它。候选图节点或名称不能充当正式身份；跨材料对应、对象拆分或合并须显式表达，已有私人证据保留原引用。状态:BOUNDARY_CHANGE（[ADR-0142](docs/adr/0142-grounded-teaching-map-and-whole-source-readiness.md)）。

## InteractionTrace
`LearningMemory` 内独立于 Agent 对话历史和低层工具轨迹的用户私有、追加式语义事件流。它以实际发生的可观察动作或结果为原子记录，用因果引用连接 `TutorSession` 的显式生命周期、跨聊天回合的 `ResolvedInteractionIntent` 及其生效来源、有效 `TutorSessionMode`、`TutorPresentation`、Learner 行为、被采用的运行时路径/下一步决策及最终处置；它记录“用户接触了什么、双方做了什么”，不直接断言“学会了什么”，也不保存模型隐藏推理。状态:BOUNDARY_CHANGE(见 `grill.md` Q69/Q71/Q82/Q86/Q87)。

## TutorPresentation
`TutorLoop` 把已采用 `TeachingMove` 实际交付到用户界面后形成的教学内容与交互条件，包括 Agent 生成的问题、提示、解释、反馈，所选择的书内片段、图示或其他材料，用户当时可用的回答方式，以及回答前已获得的 assistance；采用 AgentPresentation 时关联其实际交付版本与交互现场。它记录 Learner 行为发生前真正看到了什么，不包含未采用候选；书源与 Agent 生成内容保持来源差异，内容已交付不证明用户看完、理解或掌握。状态:BOUNDARY_CHANGE(见 `grill.md` Q82/Q83/Q85/Q102、[ADR-0130](docs/adr/0130-agent-rich-presentation-and-read-time-authoring.md))。

## PresentationFrame
`TutorLoop` 将已采用 `TeachingMove` 渲染为 `TutorPresentation` 时使用的可选体验框架，可表达沟通风格、Tutor voice、人物设定、关系语境与交付模态。它只能改变呈现方式，不得改变目标认知活动、来源主张、assistance 暴露、`AssessmentContract` 或 `LearningEvidence`；人物关注、好感、失望及参与时长也不得成为对象级能力证据或学习奖惩。状态:NEW(见 `grill.md` Q85)。

## TutorInteraction
`TutorPresentation` 中确有必要使用专门用户界面时的结构化教学交互，承载自由回答、单选、多选、排序、匹配、证据选择或自评等有界、版本化行为；其视觉呈现可采用现场生成的 AgentPresentation，实际题面、材料、来源、回答条件、assistance 及提交、改答、跳过、请求提示仍与对应呈现保持类型化关联。它服务于稀疏诊断、回忆或明确需要结构化作答的场景，不是 `TutorLoop` 的默认教学外壳或 Reader 副作用；视觉自由不授予新的判题或学习状态语义，普通思考引导与解释继续自动记录实际呈现。状态:BOUNDARY_CHANGE(见 `grill.md` Q83/Q85/Q102、[ADR-0130](docs/adr/0130-agent-rich-presentation-and-read-time-authoring.md))。

## Agent 呈现内容 (AgentPresentation)
Agent 围绕当前阅读问题交付、与对话关联且可继续修改的内容对象，可承载富排版回答、交互讲解、可运行教具和持续更新的资料。它保留内容版本、来源与模型补充的区别；普通呈现不等同于正式教学活动或公共书源。状态:NEW（[ADR-0130](docs/adr/0130-agent-rich-presentation-and-read-time-authoring.md)，已接受设计，2026-09-16）。

## 演示全局框架
围绕当前读者问题组织一篇 AgentPresentation 的可修订设计摘要，说明读者最终应能辨认、解释或完成的事情、关键关系、理解所需的展开顺序、各局部的作用和贯穿全文的对象约定。它表达当前解释的设计意图；任务要求和工作进度由 ResidentGoal 维持。它不等同于书籍结构、正式教学路线、已交付内容或读者理解证据。状态:BOUNDARY_CHANGE（[ADR-0154](docs/adr/0154-presentation-global-framework-and-staged-authoring.md)、[ADR-0155](docs/adr/0155-goal-work-plan-and-version-centered-presentation-context.md)，职责分工已接受，2026-10-02）。

## 演示制作阶段
Agent 制作一篇 AgentPresentation 时当前承担的设计职责，包括组织全局框架、制作局部解释和串读整篇成品；局部观察可以促使其返回全局修订。阶段只说明当前工作重点，不表示工程验收、内容交付或学习目标已经完成。状态:NEW（[ADR-0154](docs/adr/0154-presentation-global-framework-and-staged-authoring.md)，已接受设计，2026-10-02）。

## 呈现现场 (PresentationState)
某一 AgentPresentation 版本在实际使用中的可观察状态，包括当前参数、选项、步骤和显示结果，是继续解释或修改该内容的共同参照。它记录发生了什么，不直接判断读者是否理解；用于正式教学时关联相应实际呈现条件。状态:NEW（[ADR-0130](docs/adr/0130-agent-rich-presentation-and-read-time-authoring.md)，已接受设计，2026-09-16）。

## 演示工作区
让读者在同一处操作 AgentPresentation 并与 Agent 继续原对话的阅读交互空间；提问关联发送时的内容版本与现场，回复、新版本及来源可在其中查看。它沿用原对话和全局 Tutor 状态，打开或收起工作区不另建教学会话。状态:NEW（[ADR-0144](docs/adr/0144-shared-presentation-conversation-workspace.md)）。

## 交互教具
AgentPresentation 用于支持观察、比较、试验、推理或构造理解的教学用途，可由当前问题现场形成并继续调整。它具有可操作对象和可观察反馈，是否形成正式学习证据由相应教学活动与用户表现决定。状态:NEW（[ADR-0130](docs/adr/0130-agent-rich-presentation-and-read-time-authoring.md)，已接受设计，2026-09-16）。

## 局部动态演示
AgentPresentation 正文中围绕一个关系、动作或变化过程展开的动态插图，与附近文字共同构成解释。它可由实时图形或预渲染短片段承载，播放、暂停与定位服务于观察该过程；预渲染片段的时间定位不意味着能够实时改变模型参数。现场沿用 PresentationState，观看完成不直接形成学习效果结论。状态:NEW（[ADR-0139 §14](docs/adr/0139-explorable-explanation.md#14-通用制作与局部动态演示)，已接受设计，2026-09-28）。

## AssessmentContract
可判题 `TutorInteraction` 在 Learner 作答前冻结的版本化评分合同，与确切题面版本、目标 `LearningObjectRef`、能力轴、判定方式和版本、答案键或原子 rubric、允许来源/规则、部分正确与不确定策略及反馈/答案揭示策略绑定。Agent 可以在出题时提议该合同，但看见回答后不得修改；合同本身也不得在揭示条件满足前泄露答案。状态:NEW(见 `grill.md` Q84)。

## ResponseAssessment
某次 Learner 提交在对应冻结 `AssessmentContract` 下形成的题目级判定，区分 correct、partial、incorrect、uncertain，以及因判定未执行或技术失败产生的 unassessed，并保存逐项判断、来源与评估器版本。它只回答“这次回答是否满足这道题的既定标准”，不是长期掌握结论，也无权直接改写 `LearnerKnowledgeState`。状态:NEW(见 `grill.md` Q84)。

## LearningEvidence
由 `TutorLoop` 从 `InteractionTrace` 与可用的题目级 `ResponseAssessment` 中提取、带 `LearningObjectRef`、能力轴、assistance、attempts、context、来源、版本和精确 Trace 引用的持久解释性证据。它解释一次表现对学习状态提供什么方向、什么强度的依据；能力轴由学习对象和目标能力共同约束，可区分 recognition、recall、explanation、relation、application、evaluation、production 或 transfer 等不同表现，但不预设所有内容共享一套训练轴，也不能把局部能力差异压成统一对错。来源推论未闭合时，依赖该推论的正确性与掌握判断暂停；有独立评价依据的证据辨析等表现仍可形成 Evidence。人物关系、情绪反应、停留时长或参与度本身不得生成对象级能力证据；它是对象级学习状态的唯一判断输入，不是观察事实、具体答题判定或来源引用使用的“本轮证据账本”。状态:BOUNDARY_CHANGE(见 `grill.md` Q71/Q78/Q79/Q81/Q84/Q85/Q88)。

## PathProgress
当前私人 `PathInstance` 上“已经推进到哪里、在什么帮助下完成或停滞”的短期可重建投影。它消费相关 `LearningEvidence`,服务连续教学,但不得把一次路径内表现直接提升为跨问题、跨资源的长期掌握。状态:NEW(见 `grill.md` Q70/Q81)。

## LearnerKnowledgeState
按稳定 `LearningObjectRef` 与能力轴组织的长期动态状态投影,由带版本和 watermark 的 `LearningEvidence` 可回放重建。它回答当前证据支持怎样的对象级判断,但不拥有或删除原始 Trace/Evidence,也不得被学习计划或 `ProfileFact` 覆盖。状态:NEW(见 `grill.md` Q70/Q78/Q79/Q81)。

## TaskNeed（任务需求）
开放自然语言在 Resident 工具发现边界中的有界结构化需求。模型只提出语义维度（scope、operation 与所需 capability）；Runtime 以本轮真实证据状态、content profile、权限和经确认的副作用意图补全并校验状态维度。它是单回合工具路由输入，不是 `ResolvedInteractionIntent`、`TutorSessionMode`、持久用户意图、证据或权限授予。状态:BOUNDARY_CHANGE(见 [ADR-0113](docs/adr/0113-open-natural-language-capability-routing-and-runtime-evidence-topology.md)、`grill.md` Q86)。

## Resident Tool Routing Card（住户工具能力卡）
ToolRegistry 为每个 Resident 工具拥有的结构化发现合同，声明 provides、scopes、operations、use_when、avoid_when、effects、preconditions、content_profiles 与 relative_cost。它用于能力解析和发现排序，不是书内证据、工具执行结果或权限来源。状态:NEW(见 [ADR-0113](docs/adr/0113-open-natural-language-capability-routing-and-runtime-evidence-topology.md))。

## StructuralIndex（结构索引能力）
只读地提供章节或文档结构角色、主线、关键停靠点及证据规划入口的 Resident 工具能力。它可以被概述和导航流程共同消费，但本身不改变 Reader 状态，也不等同于 `NavigationPlan` 或 `ReaderWrite`。状态:NEW(见 [ADR-0113](docs/adr/0113-open-natural-language-capability-routing-and-runtime-evidence-topology.md))。

## TurnLocatorLedger（本轮定位来源账本）
Runtime 在一个用户回合内维护的 LID 来源集合，记录某个定位是否来自用户明确输入、已验证选区、当前 Reader anchor、结构/搜索/查询/context 工具结果或先前已验证证据。它只决定某个 LID 能否作为后续读取参数，不证明该位置正文已经被模型观察；后者仍由“本轮证据账本”负责。状态:NEW(见 [ADR-0113](docs/adr/0113-open-natural-language-capability-routing-and-runtime-evidence-topology.md))。

## ProgressPhase（进展阶段）
Runtime 对当前 Resident 用户回合所处任务阶段的瞬时权威分类，按 `UNLOCATED -> LOCATED -> EVIDENCE_READY -> SYNTHESIZED -> FINAL` 单向推进。只有新的合法 locator、正文 evidence、capability、Reader/memory effect 或最终综合结果能证明进展；仅更换工具参数不构成阶段推进。它不是 Provider 自报状态、工具调用计数或持久任务进度。状态:NEW(见 [ADR-0113](docs/adr/0113-open-natural-language-capability-routing-and-runtime-evidence-topology.md))。

## 最终收敛采样 (finalization sampling)
Resident 模型—工具循环用尽最后一个合法工具批次后，由 Runtime 保留的一次 tools-disabled 终答机会。它只能利用本轮已取得的证据形成最终回答；任何工具调用输出都是协议错误，不得重新进入工具循环。状态:NEW(见 [ADR-0113](docs/adr/0113-open-natural-language-capability-routing-and-runtime-evidence-topology.md))。

## BookStructure 构建引用范围
由实际交付的章节摘录、带出处的卡片或子级结果派生的构建期引用范围，区分结构单元身份、正文证据和依赖目标。章节任务生成本章内容，汇总任务在两端资料可见时形成跨章依赖与主题线；它不等同于 Resident 回合证据账本。状态：EXISTING（ADR-0124，2026-09-12）。

## Executor 调用纠错
主任务根据所属子代理的实际失败调用，指出控制参数或调用顺序错误，并指导恢复同一工作目标。它不等同于候选内容修订、语义重试或执行器重新注册。状态：NEW（[ADR-0128](docs/adr/0128-root-guided-executor-call-correction.md)，已接受设计，2026-09-14）。

## 构建诊断读取范围
构建主任务为解释子代理失败可读取的调用事实，包括操作、控制参数、工具诊断和顺序；不包括书稿正文、语义候选与隐藏推理。它扩展主任务的诊断职责，不授予语义生成或提交权威。状态：BOUNDARY_CHANGE（[ADR-0128](docs/adr/0128-root-guided-executor-call-correction.md)，已接受设计，2026-09-14）。

## 局部结构贡献
一份已接受局部拼接成果对 BookStructure 阅读骨架、重点位置和局部主题的有来源贡献。重复使用同一成果不产生新的贡献，后续主题整合保留其来源。状态：NEW（[ADR-0129](docs/adr/0129-append-book-structure-and-reconcile-relations.md)，已接受设计，2026-09-14）。

## 结构关系增量
基于已交付结构条目及证据形成的主题新建、扩展、合并或阅读依赖补充。它表达局部结构之间新判断的关系，不是三个完整清单的替换结果。状态：NEW（[ADR-0129](docs/adr/0129-append-book-structure-and-reconcile-relations.md)，已接受设计，2026-09-14）。

## 设备侧呈现投影（device-local workspace projection）
同一逻辑阅读工作区在当前设备可用空间中的呈现方式。它表达区域的摆放与可见方式，不改变书籍、证据、会话归属或后端逻辑布局的所有权。状态：NEW（见 [ADR-0131](docs/adr/0131-mobile-reading-workspace-and-viewport-projection.md)）。

## 阅读返回点（reading return point）
读者离开当前阅读或解释现场时保留的可返回位置，关联原文位置及发起动作的消息或内容对象。它不是新的书籍锚点或学习证据。状态：NEW（见 [ADR-0131](docs/adr/0131-mobile-reading-workspace-and-viewport-projection.md)）。

## 冻结选区（frozen reading selection）
读者对选中内容发起动作时固定的引用范围与文本上下文，不依赖之后浏览器原生选择是否仍然存在；证据有效性仍由既有范围和来源规则决定。状态：NEW（见 [ADR-0131](docs/adr/0131-mobile-reading-workspace-and-viewport-projection.md)）。

## 阅读评测任务合同（EvalTaskSpec）
评测侧在执行前冻结的一次阅读任务及其用户输入、初始条件、必要要求、核验材料和适用交付条件；它规定如何判断 Agent 的实际结果，不拥有生产任务状态，也不等同于判 Learner 作答的 AssessmentContract。状态：NEW，已接受设计（见 [ADR-0137](docs/adr/0137-stratified-reading-evals-and-diagnostic-improvement.md)）。

## 阅读任务层级（ReadingTaskLevel）
按完成任务所必需的理解操作划分的 L1 定位提取、L2 解释与局部推导、L3 多证据整合、L4 情境诊断与迁移评价；它描述任务结构，不表示工具数量、篇幅或某版本的实测难度。状态：NEW，已接受设计（见 [ADR-0137](docs/adr/0137-stratified-reading-evals-and-diagnostic-improvement.md)）。

## 阅读交互条件（ReadingInteractionCondition）
评测任务对单轮、多轮承接或跨会话状态恢复的要求，与阅读任务层级独立；跨会话须区分同聊天重启与新聊天恢复，不扩展生产 Goal 的既有归属。状态：NEW，已接受设计（见 [ADR-0137](docs/adr/0137-stratified-reading-evals-and-diagnostic-improvement.md)）。

## 原文支持（阅读答案评测）
答案具体论断能否由该答案实际交付的有效来源原文推出，包含必要条件、互补前提和推论边界。材料采用已保存的完整来源视图，并区分高亮与前后文；公共参考只用于内容核验，不能补作交付来源。评委须标出论断与支持片段；支持成立本身不证明定位准确、检索高效或理解能力提高。状态：NEW，task-v2 已实现、正式校准待人审（见 [ADR-0137 §6](docs/adr/0137-stratified-reading-evals-and-diagnostic-improvement.md#6-原文支持与引用定位)）。

## 引用定位（阅读答案评测）
答案所附引用是否清楚指出支持该具体论断的原文，使读者无需重新搜索依据。高亮过窄、错段、只指标题、宽泛范围掩埋依据属于交付问题；必要的完整段落或多段前提不因长度受罚。与原文支持分别判定，必要性由任务合同声明。状态：NEW，task-v2 已实现、正式校准待人审（见 [ADR-0137 §6](docs/adr/0137-stratified-reading-evals-and-diagnostic-improvement.md#6-原文支持与引用定位)）。

## 已发现构建工作量
当前依赖和有效成果已经允许展开、由构建引擎实际规划出的工作量。它区分需模型处理、已接纳、未接纳和确定性跳过；未接纳包含正在执行的工作。后续依赖就绪或归并展开可改变数量，因此它不代表整本书的固定任务总数，规划批组数也不等于子代理启动次数。状态：已确认，第一切片已实现（见 [ADR-0140](docs/adr/0140-codex-native-subagent-refill-boundary.md)）。

## 构建宿主（Build Harness）
为预构建提供模型调用、Agent 循环、子代理创建与取消的外部运行环境。Build Engine 保持计划、预算、候选接纳、发布与恢复的权威。Codex 与 DeepSeek Harness 属于不同宿主。状态：已对齐（[ADR-0138](docs/adr/0138-multi-harness-prebuild-and-deepseek-harness-adapter.md)）。

## 构建执行配置（Build Execution Profile）
一次构建运行在开始前确定、执行期间保持不变的宿主、模型路由、模型预算与传输能力组合。更换宿主后的运行重新评估该组合，历史记录保留原义。状态：已接入 Codex 旧分支与 DSH 公共标准构建源码路径；模型快照、确认记录与传输 profile 由 invocation 绑定（[ADR-0138 §6](docs/adr/0138-multi-harness-prebuild-and-deepseek-harness-adapter.md#6-执行配置显式绑定协议扩展不伪装成字节不变)）。

## 页面结果读取（preview reading）
`presentation.author.preview(read_selector)` 在最后一个动作后，同次读取一个可见区域的完整文字、实际页面参数、原生控件和可用场景位置；candidate、environment 与 action_step 标明观察来源。action_step 是预览动作序号，数学步号来自页面状态/读数。投影整体保留结果与上下文，预算不足时整条省略并要求缩小范围。状态：已确认并实现（[结果读取记录](docs/performance/explorable-explanation-ex2-result-read-20260927.md)）。

## Provider 续接状态与采样状态快照

Provider 续接状态是绑定 assistant 消息与原模型的私有协议字段 `provider_continuation`，覆盖工具回复和终答，随私有历史持久化；不作为答案、记忆或压缩语义素材。缺字段的旧记录通过保留全文的历史上下文投影进入新协议边界，不生成替代推理。采样状态快照是每次模型决策前提供的完整动态运行状态，最后一份权威，按完成的消息组追加，仅在当前运行内保留。均为既有模型请求与上下文片段机制的具体约束，见 [调用成本计划](docs/计划-调用成本与DeepSeek适配.md)。

## 视频伴读（video companion reading）
围绕用户正在观看的视频及其提问现场，依据材料帮助解决理解卡点、并让用户继续观看的交互方式。它以原视频为主要讲授顺序，区别于围绕学习目标重新组织教学的 Tutor。状态：NEW，已确认设计（[视频 Grill VQ01](docs/grill-video.md#vq01-首版视频伴读)）。

## 媒体定位（media location）
一份确定原视频中的实际时刻或区间，表达可回看的来源位置，独立于对画面的语义分组。它是原始媒体的位置，区别于文本 LID 和可重新划分的视觉事件。状态：NEW，已确认设计（[ADR-0145](docs/adr/0145-video-media-locations-and-revisable-visual-units.md)）。

## 视觉事件单元（Visual Unit，VU）
根据实际观察画面识别出的、有意义的画面状态或变化单元，关联相应媒体位置及相关转写。它属于可修订的语义组织，同一原视频重新分析后可以拆分或合并。状态：NEW，已确认设计（[视频 Grill VQ05](docs/grill-video.md#vq05-视觉事件单元)、[ADR-0145](docs/adr/0145-video-media-locations-and-revisable-visual-units.md)）。

## 视觉索引（visual index）
以视觉事件及其画面文字、外观或变化线索组织的可检索材料目录，关联候选媒体位置和已处理范围。索引命中表示找到候选位置，区别于实际读取原始媒体取得的观察证据。状态：NEW，已确认设计（[视频 Grill VQ04](docs/grill-video.md#vq04-渐进视觉索引与问题驱动精读)）。

## 局部联合语义抽取（local joint semantic extraction）
在一段有界材料内，结合相关转写、真实画面与必要前后文形成知识对象和关系候选的语义理解方式。共同理解保持各来源可分别追溯，纯语言或纯画面内容也可独立形成语义候选。状态：NEW，已确认设计（[视频 Grill VQ09](docs/grill-video.md#vq09-局部联合语义抽取)）。

## 联合证据集合（joint evidence set）
共同支持某个具体判断的语言证据与视觉证据集合，各部分的来源及其所支持的信息可以分别说明。两类素材仅在时间上接近或同时被读取，不足以构成该判断的联合依据。状态：NEW，已确认设计（[视频 Grill VQ09](docs/grill-video.md#vq09-局部联合语义抽取)）。

## 视频讲解结构（video discourse structure）
原视频中定义、解释、示例、反例及条件限定等讲解角色和片段间关系的有来源组织。它描述材料自身的讲解安排，与视觉事件划分可以多对多关联，区别于针对某位用户制定的教学顺序。状态：NEW，已确认设计（[视频 Grill VQ10](docs/grill-video.md#vq10-原材料的讲解结构)）。

## 忠实转写正文（faithful transcript source）
保持讲者原意和表达顺序的可读转写文本，可以整理断句、标点并依据材料纠正识别错误，同时保留机器转写的来源属性。画面中的信息、指代解释与补充讲解分别表达其来源。状态：NEW，已确认设计（[视频 Grill VQ14](docs/grill-video.md#vq14-忠实转写正文2026-09-30)）。

## 视频双语对照（video bilingual transcript）
与整讲英文转写及其媒体位置对应的中文辅助译文和英文转写的对照内容，供观看时连续参照。英文转写及原视频保留来源身份，中文译文依附于对应的转写版本。状态：NEW，已确认设计（[视频 Grill VQ23](docs/grill-video.md#vq23-整讲中英对照2026-09-30)）。

## 转写校正（transcript correction）
对机器转写中的识别错误进行的有依据文字修订，可以由用户编辑或外部 Harness 的模型辅助完成。校正以保持讲者原意和表达顺序为目标，区别于整理讲稿或补充知识解释。状态：NEW，已确认设计（[视频 Grill VQ16](docs/grill-video.md#vq16-人工与模型辅助的转写校正2026-09-30)）。

## 转写校正草稿（transcript correction draft）
模型针对现有转写提出的待采纳文字修订，包含修改前后内容及简短理由。用户可以整批或部分采纳，已采纳内容构成正文修订与后续构建的输入。状态：NEW，已确认设计（[视频 Grill VQ17](docs/grill-video.md#vq17-模型校正草稿的采纳方式2026-09-30)）。

## 原音频复核（source audio verification）
针对转写疑点重新读取相应原音频片段，并结合语言上下文和相关画面核对讲者实际措辞的活动，可发生在转写校正或日常伴读提问中。复核可为当前回答或校正建议提供依据，无法确定的内容仍保留为疑点，修改已保存转写须经用户采纳校正草稿。状态：NEW，已确认设计（[视频 Grill VQ18](docs/grill-video.md#vq18-按疑点复核原音频2026-09-30)、[视频 Grill VQ29](docs/grill-video.md#vq29-日常伴读按需复核原音频2026-10-01)）。

## 视频提问现场（video question context）
一条视频问题所绑定的原视频位置、画面与提问时材料版本，默认在开始输入时绑定，也可包含用户明确选定的帧及关注区域或视频起止范围，后续播放与后台成果变化不改写已绑定内容。它表达问题所指的现场，区别于 Agent 实际读取材料后取得的媒体证据。状态：NEW，已确认设计（[视频 Grill VQ19](docs/grill-video.md#vq19-输入问题时暂停并绑定现场2026-09-30)、[视频 Grill VQ25](docs/grill-video.md#vq25-暂停画面框选提问2026-09-30)、[视频 Grill VQ26](docs/grill-video.md#vq26-指定视频时间范围后提问2026-09-30)、[视频 Grill VQ36](docs/grill-video.md#vq36-同源补齐成果自动接入2026-10-02)）。

## 视频引用预览（video source preview）
用户从回答中的来源引用打开、在回答旁观看的原视频片段，保留主播放器原有观看位置。用户可进一步进入对应原视频位置查看完整上下文。状态：NEW，已确认设计（[视频 Grill VQ20](docs/grill-video.md#vq20-回答旁的视频引用预览2026-09-30)）。

## 视频渐进成果接入（progressive video enrichment adoption）
同一原视频与同一忠实转写版本下，当前观看在提问间隙自动采用已完成译文、视觉索引和语义成果的过程。它保持当前提问的材料与历史来源归属，区别于原视频或转写正文的改版。状态：NEW，已确认设计（[视频 Grill VQ36](docs/grill-video.md#vq36-同源补齐成果自动接入2026-10-02)）。

## 学习伙伴形象（learning companion character）
在阅读、观看视频与提问中呈现用户所交流的学习伙伴身份的角色形象，是日常学习交互的一部分，也可用于品牌展示。其核心气质为安静、机灵、有好奇心，最鲜明的小执念是用户卡住时一定要陪其弄明白，幽默来自有点过头的认真。状态：NEW，已确认设计（[视频 Grill VQ31](docs/grill-video.md#vq31-日常学习伙伴形象2026-10-01)、[视频 Grill VQ32](docs/grill-video.md#vq32-学习伙伴气质与多邻国参考2026-10-01)、[视频 Grill VQ33](docs/grill-video.md#vq33-学习伙伴的鲜明小执念2026-10-01)）。

## 用户运行空间（UserRuntime）
一个读者的私人记忆、聊天、学习记录、私人演示和成果的共同归属；多个阅读现场使用同一份私人权威状态。它区别于当前视口、聊天选择、单次 Resident Run 和服务运维配置。状态：EXISTING（[ADR-0147 §1](docs/adr/0147-linux-multi-reader-service-without-redis.md)，MU1a 本地身份与 MU2 显式服务用户）。

## 阅读现场（ReaderWorkspace）
一个独立阅读窗口当前使用的材料、阅读位置、聊天选择和现场代次的共同归属；多个现场共享同一用户的私人记录，现场变化不会替换另一现场的选择。独立页面的挂接冲突通过分叉或显式接管解决；受控演示附属页沿用原现场和聊天。状态：EXISTING（[ADR-0147 §1/§5](docs/adr/0147-linux-multi-reader-service-without-redis.md)，MU1b/MU5）。

## 运行归属与提问现场（RunScope / ReaderInputSnapshot）
Resident Run 开始时固定的用户、原聊天、原材料与原现场代次，以及回答原问题所需的已验证阅读输入。现场换代使实时读写失效，不改变原问题、私人记录和历史回答的归属。状态：EXISTING（[ADR-0147 §5](docs/adr/0147-linux-multi-reader-service-without-redis.md)）。


## 已发布材料引用（PublishedBookRef）
一本内容材料的某次不可变发布身份，由 book_id 与 publication_id 共同确定；同内容补齐能力形成新的发布，正文或实际来源附件改版使用新 book_id。运行与历史保存确切发布，普通现场沿用已绑定发布，视频伴读可按渐进成果接入约定采用同源的新发布。状态：BOUNDARY_CHANGE，MU3 已实现，视频扩展已确认设计、待实现（[ADR-0147 §3](docs/adr/0147-linux-multi-reader-service-without-redis.md)、[视频 Grill VQ36](docs/grill-video.md#vq36-同源补齐成果自动接入2026-10-02)）。

## 应用身份与登录会话（Principal / AuthSession）
服务器验证登录会话后确认的读者身份，以及该次登录的有效期与撤销范围。身份不由请求正文、模型参数或代理用户头声明；账号禁用、改密或会话撤销后不再有效。它与私人聊天、Tutor 教学会话和阅读现场分别表达。状态：EXISTING（[ADR-0147 §2](docs/adr/0147-linux-multi-reader-service-without-redis.md)，MU4）。

## 授权访问上下文（AuthorizedContext）
已确认身份对自身私人对象和获授权材料的访问范围；每个外部对象引用仍须在此范围内解析，成功登录不等于获准访问任意对象。材料权限和观察连接随撤销失效，已保存的私人历史保持原归属。状态：EXISTING（[多人方案 §4](docs/切片方案-Linux原生多人阅读与无Redis首版.md)，MU4）。

## Agent 会话顺序日志（Resident session log）
一个住户聊天所发生事件的持久顺序记录，保存原提问现场、对话、来源关联、实际成果及任务状态，供恢复当前聊天和回看阅读活动。阅读现场、教学事实和私人记忆继续按既有领域合同持有。状态：BOUNDARY_CHANGE，JL0–JL7 会话、运行、领域关联、成果处置与新日志启用已实现（[ADR-0152](docs/adr/0152-resident-linear-jsonl-session-log.md)）。

## 阅读成果处置回执（Reading effect disposition receipt）
读者对助手产生的阅读成果执行保留、撤销或忽略后，由实际处理方确认的结果，关联原成果及处理后成果；它表达发生过的处理，不保证成果此后仍然存在。状态：NEW，JL6 已实现；未能确认的实际结果保留待核对状态（[ADR-0152 §4](docs/adr/0152-resident-linear-jsonl-session-log.md)）。

## 本次阅读回顾（Session reading recap）
当前聊天在明确记录范围内的阅读活动回顾，列出讨论过的问题、引用的原文、留下的成果和待继续事项，并关联原回合与实际来源。它表达阅读过程中发生过什么，读者的理解程度和学习结论由既有教学证据定义。状态：EXISTING，JL8–JL9 已实现并验证（[ADR-0153](docs/adr/0153-session-reading-recap.md)）。
