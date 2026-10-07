# 切片方案：BookStructure 全局框架与语义召回

状态：设计已接受，2026-10-01；BSR0–BSR6 已完成，含独立 Codex subagent 真实章节／主题内容验收、本地 MiniLM 对照及新书完整章节发现；BSR7 已完成（[交付记录](performance/book-structure-bsr7.md)）。对应 [ADR-0151](adr/0151-book-structure-global-outline-and-semantic-retrieval.md)。BSR0 证据见[真实样本与比较合同](performance/book-structure-bsr0.md)，内容结果见 [BSR3–BSR4](performance/book-structure-bsr3-bsr4.md)，正式接入见 [BSR5](performance/book-structure-bsr5.md)，前段发现见 [BSR6](performance/book-structure-bsr6.md)。

目标是保住有来源的重要机制，形成可解释的章节推进与跨章主题，并减少重复生成和总构建成本。首轮以 ai-infra 的第 8 章和一条跨章主题验证章节整理与 SR 召回，再接入正式路由和新书前段。最终公共产物继续使用 spine / throughlines / key_stops，沿用现有 BuildStage 与 Harness。

## 现状与问题基线

审阅对象为 `E:/allwork/download/agent/lifebook/.understand-book/ai-infra-book-complete` 的 2026-10-01 发布结果。仓库 `.understand-book` 下的同名旧副本不作为基线。构建完成见[恢复与发布记录](performance/build-control-recovery-20260930.md)。

| 观测 | 当前结果 | 对应改动 |
| --- | --- | --- |
| BookStructure 已接纳工作 | 410 片段、190 归并、1 单元、5 拼接、1 选择、95 关系，共 702 项 | 减少层层重写和两两展开 |
| 章节候选与最终停靠点 | 138 个章节候选，成品 34 个点 | 候选持久保留，章节选择由引用组装 |
| 拼接分组 | 前四组各含 3–4 个单元，产 6–7 个点；最后一组单章产 8 个点 | 内容取舍独立于拼接分组 |
| 跨章主题 | 53 条，其中 49 条只连接两个单元，存在近义重复 | 按完整主题整合并明确边界 |
| 章号 | 13 条主题摘要将 LID 单元序号写成章号 | 真实标题映射与引用式呈现 |
| 章节依赖 | 14 个结构单元只有 1 条显式依赖 | 结合两端材料判断实际前置 |

702 个已接纳任务的预算记录累计估算：任务正文 2,817,329 tokens，语义提示词 507,377 tokens，交付开销 655,467 tokens；候选输出上限均为 1,024 tokens。这些是程序估算，分别保留，不当作计费账单或全部模型调用量。622 次结构来源／依赖引用均可解析，引用完整性与内容组织质量分别判断。

BSR0 已重算上述数据并保存来源路径，形成[可重放基线](performance/book-structure-bsr0.md)。另查得 720 次历史尝试的实际 tokens 全部未知；原书和大体积历史构建目录不随代码复制。

## 已确认边界

- BookStructure 继续描述公共材料的结构；阅读目标和用户私人状态留在读时。
- 首轮替换 technical_learning 路径。paper 保留其既有结构规则，接入处验证不受影响；扩大覆盖另按真实材料决定。
- Pass1、profile sidecar 和已确认的 Pass2 选择保持。ai-infra 原计划 Pass2 disabled。
- 复用 SR 能力，不增加对 formal_objects、cognitive_materials 或 teaching_publish 的前置依赖。
- 现有来源、预算、policy/generation、冻结输入、接纳与发布权威继续由 Core/Engine 持有。
- 本方案定义实现和验证工作；真实模型实验沿用项目已有计划及预算确认入口。

## 数据流与覆盖

```text
可信正文 / LID tree / 真实标题 / 已有公共语义产物
    → 全书框架草案：章节问题、部分组织、暂定主题
    → 逐章逐节候选发现；超限时按自然边界和原 core_range 分块
    → 持久候选目录：章节来源 + 含义/条件/教学价值 + 原文定位
    → 章节选择：完整浏览本章索引，按需展开，输出候选引用
    → 全书主题工作集：看全部章节卡与主题目录，SR 定点补取材料
    → 主题发展 / 主题目录收敛 / 有证据的阅读依赖
    → 程序物化三清单 → 现有来源与完整性接纳 → 发布
```

框架由前言、目录、章级引言和小结产生，正文可以修订；它不证明正文已覆盖。已有受支持的空标题路径由 Core 从真实 heading 节点和 source span 提取标题，封面、前言不计入正文显示章号。

首轮复用的章节卡与片段成果须通过当前来源和依赖有效性判断；旧结果作为有来源输入进入新合同，不冒充新规则下已经完成的章节选择。若两个归并层重复包含同一候选，按原贡献来源和显式候选引用整理；相同 LID 上不同含义的候选保留区分。

新书路径延续 `BookStructureLeafCoverageManifestV1` 的完整 core 范围覆盖：每个正文叶节点属于一个已接纳的覆盖贡献；相邻上下文可重叠，core 不重叠、不缺失。完整节目录、语义摘要或 SR 命中不能单独代替这份覆盖。局部材料可复用已接受的来源投影；尚无有效语义贡献的范围仍须处理，超大节沿既有有界输入机制拆开。

覆盖说明哪些内容已处理；重点选择说明哪些内容值得讲。模型完整浏览本章候选索引，按真实内容作取舍，不设每节必须一个点的名额。SR 提供查询结果、命中理由和分页；排名和预览不进入证据覆盖。新增判断由已交付原文或已接纳的锚定子结果支持，缺少适用条件时定点回源。

## 内部数据与成品映射

以下是拟实现的最小字段形状，具体版本随 BSR1/BSR2 的合同一次确定；对象均属于 BookStructure 构建中间产物。

```text
StructureStopCandidate {
  ref, unit_lid, section_lid, lid, type,
  meaning, conditions, reason, evidence_lids, contribution_ref
}

StructureChapterSelection {
  unit_lid, role, summary,
  accepted_stop_refs[], macro_stop_refs[]
}

StructureThemeWork {
  ref, question, distinction, member_refs[], evidence_requests[]
}

select_chapter(outline, candidate_index, delivered_evidence)
    → StructureChapterSelection

resolve_theme(theme_directory, work, delivered_evidence)
    → theme_delta + grounded_dependencies

materialize(chapter_selections, candidates, theme_deltas)
    → BookStructureSidecar
```

- `ref` 由既有任务／贡献身份派生，复用或恢复同一贡献时保持；展示标题来自 canonical source。语义正文里需要章名时，交付真实标题；不要让模型按 LID 计算章号。
- `accepted_stop_refs` 表示章内已选重点；`macro_stop_refs` 是其中的宏观路线子集。未选候选留在内部目录，供补读或局部修订。
- `key_stops` 由全部章节已选重点和主题显式补选的有效候选物化；宏观路线只决定 `spine.key_stop_ids`。跨章任务引用已有点，不重新抄写或删减其正文。
- `throughlines` 仍使用现有名称、摘要、成员 LID 与重点引用；摘要明确各阶段增加的条件和判断变化。`depends_on` 只表达有依据的阅读前置。
- 候选正文与证据以显式引用传递；模型仅输出选择、次序、合并及新的解释。预算根据任务实际产出形状配置，沿现有执行预算机制核算；超限后分解具体工作，不机械重写摘要树。
- 当前 Reader/Book MCP 的 `structure(at)` 和 `guide_path(at)` 继续消费三清单。章级投影能取得该章详细点，宏观路线保持精炼；BSR5 用真实调用确认这两种取法。

## SR 接入

使用 SR 的 Provider、本地 adapter、混合排序、分页、向量缓存、获授权 preparation、计量和 action 重放机制。T5A 的对象身份、参与者角色与拆并规则留在原消费者。

BookStructure 投影将章节问题、候选含义、条件和原文已有别名变为检索文本；记录另外关联章节身份、候选引用与证据位置。显示章号、路径和构建身份不作为语义内容。语义别名若需生成，必须来自候选产生阶段，不额外为每项增加一次生成。初版索引章节卡与重点候选，原文、公式和论述产物作为可展开证据。

BSR3 已复用 record 级准备与排序接点，将消费者投影版本贯穿依赖比较、缺项估计、准备与排序。T5A 继续拥有 FormalObject 投影和原缓存；BookStructure 使用章卡／候选投影与独立 `book-structure` 缓存槽，完整词法字段及来源定位保留。

保留 T5A 现有缓存文件，BookStructure 在 `.build/semantic-retrieval/book-structure/` 使用独立缓存文件，复用同一算法。已有内容摘要仅用于跳过未变 embedding；相同文本可共享向量，但不合并候选身份。查询翻页和证据展开复用同一有效检索准备。

BuildPlan 的检索数据范围须覆盖实际消费者：BookStructure 候选投影与 query，以及计划同时包含 T5A 时的正式对象投影与 query。当前仅允许 formal-object 范围的确认不能直接扩大；BSR5 同步计划、配置投影和 host 接入。普通 snapshot、writer、remaining-work 保持不调用 Provider。

使用 lexical_only 可完成同一结构流程；semantic_required 缺少有效准备时沿现有失败／恢复路径处理，不把 Provider 失败当成没有相关内容。候选投影适配当前 adapter 的真实输入限制，保留完整词法字段和可展开正文；不将长章节全文放进 embedding 输入。

## 主题工作

全局规划读取全部章节卡和紧凑主题目录，形成有限的主题工作集。每组表达一个问题、与其他组的区分，以及初始候选成员；同一个重点可以参与不同问题。材料过多时按主题的发展阶段分页展开，保持同一主题身份。

```text
plan_themes(all_chapter_cards, candidate_index)
for work in planned_theme_work:
    search / browse → 当前有效候选页
    inspect / read  → 本次可用材料
    resolve         → 主题变化与证据化依赖
reconcile_theme_directory(final_names_and_summaries)
    → 一次有界的合并／改名／成员调整；按需读取实际证据
materialize()
```

不生成 `group × group` 的两两任务。每个主题任务携带完整的紧凑主题目录，便于遵守分工。目录收敛用于处理实际重复或边界冲突，只提交变化和引用；不要求重新输出三清单，不自动产生下一轮全表扫描。具体未解决问题保留待完成状态，沿原恢复入口继续。

例：围绕“状态复用如何改变容量、传输和恢复成本”，比较模型上下文状态、单机 KV 分配与复用、跨实例 KV 交接，以及有实际证据支持的恢复关系。训练 checkpoint 与 Agent 轮次提交可属于同一主题，仍保留各自机制与成立条件；主题相关不授予对象同一性。

## 切片依赖与完成状态

```text
BSR0 → BSR1 → BSR2 → BSR3 → BSR4 → BSR5 → BSR6 → BSR7
        候选    先取得章级      主题       正式     新书     全书
        保留    真实对照        对照       接入     前段     交付
```

BSR0–BSR6 已完成：章号及丢点反例由 BSR1 转绿，真实章节验收由 BSR2 完成，第二消费者与主题合同由 BSR3/4 实现；最后一个依赖语义反例已替换为真实 B/C 结果及重放校验，不再预期失败；BSR5 已接入正式路由、确认范围、动作重放和发布；BSR6 已替换技术书前段发现并通过完整第 8 章内容验收。BSR7 已完成（[交付记录](performance/book-structure-bsr7.md)）。每片独立保留输入、接纳结果和验证记录，后续只依赖文件状态；完成后更新本节及项目代码链路。

## BSR0 真实样本与比较合同

**状态**：2026-10-01 完成；[报告](performance/book-structure-bsr0.md)、[重放入口](../evals/book-structure/README.md)、[回归测试](../packages/core/test/book-structure-bsr0.test.ts)。来源重放一致，10 项定向测试通过（含 3 项预期失败合同），Core 类型检查通过。

**产出**：可重放的 ai-infra 基线、精简回归夹具，以及运行前固定的原文验收依据。

**主要位置**：`evals/book-structure/`、`packages/core/testdata/book-structure/` 与 `docs/performance/book-structure-bsr0.md`；复用当前 artifacts/task reader。

**工作**：

1. 只读提取当前书的 14 个章节卡、138 个候选、最终三清单、702 个任务的分类与预算估算；分开标注历史估算与实际 usage。
2. 保存第 8 章 9 个最终章节候选，并从已接纳片段取回连续批处理等已知丢失机制。记录引用和来源位置，不复制整书或给每条增加校验和。
3. 固定一个跨章主题的应比较材料、应保持区分的机制及原文依据。原文答案标签只进入验收材料，不进入模型输入。
4. 建立最小反例：封面／前言导致显示章号偏移；重分拼接批次导致已选重点遗失；主题候选相关却不构成前置依赖。以现有函数的真实行为定位红测接点。

**验收**：数据可由真实文件重算；来源范围、基线与引用能逐项对上。检测出的错位、丢点或误依赖分别交 BSR1/2/4 修复，不扩展为全库检查。

## BSR1 候选保留与确定性组装

**状态**：2026-10-01 完成。贡献派生候选目录、真实标题和引用物化已实现；章号／9→2 反例转绿，顺序、分组和重复应用保持内容与证据。[记录](performance/book-structure-bsr1-bsr2.md)。

**产出**：可引用的完整候选目录、真实标题映射和按引用组装的基础能力。

**主要位置**：[book-structure.ts](../packages/core/src/book-structure.ts)、[book-structure-generation.ts](../packages/core/src/book-structure-generation.ts)、[book-structure-materialization.ts](../packages/core/src/book-structure-materialization.ts)、[book-structure-evidence.ts](../packages/core/src/book-structure-evidence.ts)；拟新增 `book-structure-candidates.ts` 及对应测试。

**工作**：接入有效章节卡和片段贡献；保存候选语义、条件、来源和贡献引用；从 heading/source span 提取真实标题；实现引用解析、章节归属和原证据范围继承。内部候选目录保留未选条目，物化只消费显式已接纳选择。

**验收**：同一已接纳选择更换拼接分组或完成顺序，重点与证据不变；已知缺失机制仍在候选目录；重复应用不重复发布；无效引用拒绝。BSR0 的章号／丢点反例在对应层转绿。

## BSR2 全书框架与章节引用选择

**状态**：2026-10-01 完成。用户选择独立 Codex subagent 执行真实对照：31 候选→27 个章内重点／15 个宏观路线点，9 类机制原文验收通过；13 次语义提交、0 拒绝、约 7 分 5 秒。实际 tokens 不可得，估算与限制单列。[实现](performance/book-structure-bsr1-bsr2.md)、[真实对照](performance/book-structure-bsr2-codex.md)。

**产出**：全书框架草案、第 8 章章节结构，以及第一次真实内容／成本对照。

**主要位置**：`book-structure.ts`、`book-structure-generation.ts`、[model-input-renderer.ts](../packages/core/src/model-input-renderer.ts)、`book-structure-planning.ts`、章节／框架 prompt、`evals/book-structure/chapter.ts` 与 `chapter-subagent.ts`。

**工作**：以真实目录、前言和章级概述形成框架；本章完整索引可浏览，单次输入只展开需要的候选与证据。模型输出章节摘要、已选重点引用和宏观路线子集。引用只读现有内容，新增解释按交付证据接纳；明确输出预算，首轮运行只涉及第 8 章与必要全书概述。

**验收**：用原文核实显存需求、批处理、KV 管理、压缩／卸载、推测解码和有效产出的关系；按机制可追溯到实际选点，重点选择有取舍但不再次退化成仅容量和吞吐两个点。记录完整调用、拒绝、读取、tokens 和耗时，并将新结果与旧结果交付审阅。此片不调用 embedding。

## BSR3 SR 的第二个消费者

**状态**：2026-10-01 完成。消费者投影身份与缓存槽贯通，短投影通过本地真实 tokenizer；C 实验嵌入 216 个文档和 9 个查询，36 次调用。相关 5 文件 45 项验证通过。[实现与验收](performance/book-structure-bsr3-bsr4.md)。

**产出**：BookStructure 候选投影、混合检索准备及独立缓存槽。

**主要位置**：[semantic-retrieval.ts](../packages/core/src/semantic-retrieval.ts)、[semantic-retrieval-preparation.ts](../packages/core/src/semantic-retrieval-preparation.ts)、[semantic-retrieval-cache.ts](../packages/core/src/semantic-retrieval-cache.ts)、[embedding-provider.ts](../packages/core/src/embedding-provider.ts)、[book-structure-retrieval.ts](../packages/core/src/book-structure-retrieval.ts)。

**工作**：提取已存在的 record 级共享接点，参数化真实消费者的 projection identity；T5A 投影继续由原模块拥有。贯通准备依赖、缓存缺项计算与排序；保留 exact/alias、完整浏览、分页和语义截断说明。先用确定性测试验证接口，再在既有计划机制下执行小样本真实投影；正式 host 接入留 BSR5。

**验收**：两个消费者相互运行不清空对方缓存；含义或条件变化只重算实际变化项，翻页不重复请求；未变候选可复用。当前真实 tokenizer 不接受的输入明确处理，短投影仍能定位完整材料。运行相关 T5A 回归以发现接口改变造成的排序、准备或重放回退。

## BSR4 主题工作集与局部对照

**状态**：2026-10-01 完成。真实 B/C 共享 202 候选、四主题目录；最终各 4 主题、25 阶段、8 条有双端来源的依赖，45／46 次提交，均 0 拒绝。原文核对后只重开固定主题，每组 2 次动作补齐分页／共享容量和量化转移联系，其他主题不变；完整重放通过。C 未证明整体收益，保持 `lexical_only`。[内容与成本报告](performance/book-structure-bsr3-bsr4.md)。

**产出**：一条完整跨章主题、阅读依赖与词法／混合检索对照；随后覆盖本书主题目录。

**主要位置**：[book-structure-themes.ts](../packages/core/src/book-structure-themes.ts)、[book-structure-retrieval.ts](../packages/core/src/book-structure-retrieval.ts)、[主题 prompt](../agents/book-structure-themes.md)、[themes.ts](../evals/book-structure/themes.ts)、[compare-themes.ts](../evals/book-structure/compare-themes.ts)。正式旧关系路由替换仍在 BSR5。

**工作**：以主题工作集替换 `bookStructureSelectedPairs` 在新合同中的展开；支持按问题搜索、展开证据和提交主题变化；保存紧凑全局主题目录。新流程同一份候选分别运行 B=改进词法、C=词法加语义召回，生成模型、prompt、证据要求和预算一致。先跑 BSR0 固定主题，再扩大到整本主题。

**验收**：11 项组不会自动生成 55 个关系任务；应比较的跨章材料可到达；相近机制仍保留区别；依赖有双端依据。最终主题能说明发展过程，同一问题不拆成近义两章关系列表。SR 若只提高排名而未减少模型比较或改善结构，报告该结果并保持词法选择。

## BSR5 正式路由恢复与发布接入

状态：已完成，2026-10-01；[实现、验证与限制](performance/book-structure-bsr5.md)。

**产出**：正式 Engine 可调度、恢复和发布新结构流程，两种 Harness 使用同一 Core 权威。

**主要位置**：[build-orchestrator.ts](../packages/core/src/build-orchestrator.ts) 的 `routeBookStructureProductionStage`、[stage-work-unit.ts](../packages/core/src/stage-work-unit.ts)、[automatic-build-quality.ts](../packages/core/src/automatic-build-quality.ts)、[automatic-build-retrieval.ts](../packages/core/src/automatic-build-retrieval.ts)、[build-intent.ts](../packages/core/src/build-intent.ts)、[build-retrieval-config.ts](../packages/core/src/build-retrieval-config.ts)、[automatic-build-driver.ts](../skills/build/automatic-build-driver.ts)、[skills/build/automatic-build.ts](../skills/build/automatic-build.ts)。

**工作**：新工作单元和输出合同进入现有 policy/generation、writer 与预算路径；计划表达 BookStructure 单消费者及与 T5A 组合的数据范围。普通观察仅读取，异步准备才调用 Provider。先取得实际检索页和输入再决定动作复用，动作在当前候选目录应用；停止或失败保留已接纳工作。质量报告分别呈现覆盖、候选、章节、主题和发布状态。

保留旧任务和回执，旧成品在新流程完成前仍可读；不把旧合约产物标成新合约已完成。需要的更新范围由现有失效规则计算，Pass1/profile 与有效来源继续复用。新流程全部接纳后，按现有写入／发布机制物化三清单。

**验收**：缺准备、取消、来源更新和目录变化后续建正确；已交付引用范围不扩大；重放不覆盖当前未展示候选；重复提交不重复贡献。运行 Chapter/Theme 到发布的完整集成、`structure` 与 `guide_path` 投影、paper 与 Pass2 已有选择回归。进度分清已发现任务和实际完成单元。

## BSR6 新书的前段候选发现

**状态**：2026-10-01 完成；[实现、成本与已知限制](performance/book-structure-bsr6.md)。15 文件 76 项定向测试及 Core typecheck 通过；全书 14 单元原文框架先行，完整第 8 章 653 叶节点经 40 个发现任务保留 178 候选。独立 Codex subagent 共 42 次提交（接受 41、拒绝 1），58 分 52.291 秒内完成；九类机制原文验收、同 LID 不同含义及保存响应重放通过，无外部生成 API／embedding／原书发布。实际模型用量未知，估算单列；复用公式的五处误标留定点修正。

**产出**：从源材料建立完整候选目录，替换反复压缩候选正文的归并树。

**主要位置**：`book-structure-discovery.ts` 的完整原文、小节边界、框架与程序累计，`book-structure.ts` 的自然小节路由，`book-structure-generation.ts`、`build-orchestrator.ts`、候选目录与质量接点，`book-structure-discovery-extractor` prompt 和 `evals/book-structure/discovery.ts`。

**工作**：先建立框架，再按自然小节组织首遍覆盖；过长小节使用现有核心范围分块。局部任务产出可引用重点和有来源的概述，程序累计候选；章级整理消费完整索引和必要正文，不继续逐层重写候选。复用 discourse/formula 等公共成果时保持其出处和条件，不能因已有一句摘要就跳过未覆盖范围。

**验收**：长节拆分后 core 完整且唯一；换批次不丢已接纳候选；同一 LID 不同含义的点可区分；模型没有把标题当证据。以一个未复用旧 BookStructure 中间结果的完整章节验证真实前段，再进入 BSR7 全书运行。

## BSR7 全书对照与交付

**产出**：完整 ai-infra 新结构、可复查的质量与成本报告，以及正式入口的交付证据。

**主要位置**：`evals/book-structure/`、相关Core/ReadTools集成测试、构建协议同步与原安装打包入口；实际记录为 [BSR7报告](performance/book-structure-bsr7.md)及 `docs/performance/book-structure-bsr7-20261002/`。

**当前状态**：已完成。完整发现与章节选择的14单元946重点／345宏观通过来源验收；unit14正式修订排除5.47GB总读取误标，并在锚定摘要保留准确比较条件，最终主题未回流该误标。完整主题规划正式输入19304/20000。C/B生成及三个正式主题修订完成，两组最终来源验收全部通过、未解决问题为0；真实fresh重放、Engine发布、正式安装及新书Reader/MCP均已通过实际验收。

**主题修订**：`requestStructureThemeRevision/readStructureThemeRevisionRequests` 保存 `{id,theme_ref,issue}`，路由在全部原主题及一次初始目录收敛后顺序定点重开。旧pending、task、prompt、input保持，其他主题及seen/inspected/evidence保留；修订沿用原 `structure_theme` 合同、prompt及retrieval，不重新进行全目录收敛。`full-book.ts request-theme-revision` 保存事件，fresh重放在首次修订前恢复正式请求。

**阶段验证与成品**：planning 8项、writer 1项、主题12项、两模式full-book／fresh replay／compare／Engine集成2项及Core类型检查通过；新sidecar25资产Node/Bun parity、T7、plugin-release及release-config通过。插件版本 `0.1.0+codex.20261003012800`，完整NSIS构建与导出完成，`dist/UnderstandBookSetup-BSR7-20261002.exe` 为63177800字节。新三个exe／web已安装于 `E:/allwork/Understand Book`，插件实际installed／enabled；CLI缓存访问拒绝后采用完整published插件复制到新版本cache，旧版本及市场保留。

**真实对照**：共同前段806次提交；主题新增C39次、B46次均接纳，共891次，含历史7次拒绝。两组各14单元／11主线；C为956重点／49阶段／9依赖／10补点，B为948重点／47阶段／7依赖／2补点。C本地embedding为2650 documents／12 queries／344 calls、125159实际input tokens、15690.6121ms，B为0。主题序列化输入／输出估算C324899／10962、B371792／11568；主题wall时间C4945835ms、B4446630ms。完整比较记录为 `docs/performance/book-structure-bsr7-20261002/comparison.json`。

**发布与读取实证**：真实fresh重放C838个接纳步骤，B799个共享接纳步骤加46个主题步骤，共845；输入／候选／结构一致，full quality通过，9111叶节点无缺口，模型及Provider调用均0，两组Engine publication均closed。C主交付新书位于 `E:/allwork/download/agent/lifebook/versions/bsr7-20261002/.understand-book/ai-infra-book-complete`，原书保留；新位置installed quality／integrity通过。Reader已打开并保存新书，已安装MCP显式及Reader默认两入口均验证14单元／956重点／345宏观／11主线、unit12=117／44，模型调用0。生成在2026-10-03T02:56Z前结束，累计891／1200、884接纳／7拒绝，授权额度和时间未重置。

**工作**：保留旧发布作为对照，不为收集旧成绩重跑 702 项。先固定模型、材料、prompt、检索模式、预算和计量方式，再运行新流程。B/C 仅改变检索方式，逐章检查原文与实际产物；先前已知的章号错位、重点损失、近义主题与缺少有据前置必须有明确处置。

**验收**：全书发布由 Engine 完成状态确认；引用／覆盖／重复身份由程序验证；语义取舍以来源对照和实际章节／主题审阅判断，不以模型自评分证明。分别报告复用旧候选的整理成本与新书前段加整理成本，包含 embedding、模型输入/输出、重试、读取和总耗时。未完成或未知 usage 单列，不能报成零或承诺全书提速比例。

本轮按用户选择以来源、重放、发布及成品读取验收通过的embedding模式C作为主交付，词法B保留完整对照。两组独立查询及动作不能证明语义模式的因果成本／提速收益，产品默认检索模式保留；原书已有来源和基础成果保持。

## 已知限制与接手入口

- [SR 最新记录](performance/semantic-retrieval-20260930.md)中 SR1–SR5 及验收工具已实现，SR6 真实对照尚未达到发布要求、长材料待验。共享实现可复用，T5A 身份质量和 BookStructure 主题质量分别验收；当前默认词法模式不因本方案落档改变。
- 本书的 138 个候选也已经经过旧归并，有些机制只存在更早片段；BSR0/BSR1 必须保留并补入已确认的来源，不能把该数当完整知识上限。
- BSR1/2 确定性验证及 Codex 真实章节内容验收已完成；实际模型用量不可得，702 项历史估算与本次序列化估算无法构成同条件严格成本对照。后续实验额度按用户选择的执行方式确认，不沿用其他 SR 实验的剩余额度。
- BSR7发现历史保留含5.47GB误标的原候选；该候选已退出章节选择，准确条件保留于章节摘要，最终主题来源验收确认该误标未回流。
- 本轮实际模型usage为null；C/B使用独立生成者，查询内容不同。序列化估算、实际embedding计量和wall时间分别报告，不能据此归因提速或推算严格费用。
- 当前聊天既有Book MCP连接仍为 `unavailable`／`book_structure.json not attached`，旧连接未附着新书；新安装MCP的显式目录及Reader默认目录的新进程已通过完整验收。

交付证据见 [BSR7报告](performance/book-structure-bsr7.md)及[实验入口](../evals/book-structure/README.md)；历史设计依据见 [BSR6报告](performance/book-structure-bsr6.md)、[BSR5报告](performance/book-structure-bsr5.md)及[BSR3–BSR4内容报告](performance/book-structure-bsr3-bsr4.md)。本轮已收口；后续新生成需独立确认材料和额度，原书及本轮词法对照保留。
