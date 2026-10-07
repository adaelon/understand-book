# BookStructure BSR0

只读固定已发布 ai-infra 的章节、任务预算、用量、局部拼接和最终三清单。输入为
`E:/allwork/download/agent/lifebook/.understand-book/ai-infra-book-complete`。

```powershell
node --import tsx evals/book-structure/extract.ts --check
pnpm --filter @understand-book/core exec vitest run test/book-structure-bsr0.test.ts
```

`--check [book-directory]` 从实际书目录重算，与已保存 JSON 按内容比较；成功退出表示来源重放一致。
`--write [book-directory]` 将提取结果写入本仓库 `packages/core/testdata/book-structure/` 的三个文件。
两种模式都不写输入书目录，不运行模型、embedding、路由构建或发布。

提取复用 `readBookStructureGenerationTask`、`readBookStructureGenerationArtifact` 与
`listAutomaticBuildAttemptDirectories`，再用现有 materializer/relation apply 重放发布三清单。
原有 reader 自带的身份验证照常执行；本入口不新建摘要、校验和或指纹。
冻结章节/片段输入摘录逐项与 `source.txt` 的 Core 字符偏移比较；702 个成果匹配对应已提交尝试。
旧副本、未接纳产物、错误来源或结果差异会使提取失败，需先定位输入，不能覆盖基线掩盖差异。

| 文件 | 用途 |
| --- | --- |
| `sample-locations.json` | 原文范围和已接纳片段的定位清单，无答案判断 |
| `baseline.json` | 14 张完整章节卡（内含 138 个候选）、发布三清单、局部贡献、702 个任务、720 次尝试、622 次引用的位置 |
| `chapter-8.input.json` | 第 8 章 9 个旧候选、9 个片段补回候选、原文与旧发布对照 |
| `state-reuse.input.json` | 固定主题的五章卡片、已接纳片段候选和六类机制原文 |
| `acceptance.json` | **仅验收**的答案、区分项、负依赖例和后续比较约束 |

后续模型输入只显式读取对应 `.input.json`；不把整个夹具目录打包交付，也不读取 `acceptance.json`。
原书、大体积构建历史和整章正文仍留在输入书目录。三份生成文件约 1 MiB，包含可复核定位。

## 实施状态

- BSR0 提取、样本与比较合同已完成，证据见[报告](../../docs/performance/book-structure-bsr0.md)。
- 原三个 `it.fails` 已全部转绿：章名和候选保留由 BSR1 实现；依赖反例由 BSR4 真实 B/C 产物、六组来源、补充锚点和完整重放验证。
- 章节语义取舍已由 BSR2 Codex subagent 真实局部对照验收；主题与依赖已由 BSR4 独立生成和原文核对验收。来源 gate 只验证交付，不负责判断 Adam 是否为 Agent 轮次提交的前置。
- 原程序组装在相同保留内容重分组时保持内容；实际 9→2 的丢失已发生在模型拼接输出。测试保留了这一区分。

## 已知限制

BSR0 的章号嫌疑定位仍为本书基线工具；BSR1 已实现通用 canonical 标题映射。
两组样本是运行前固定的局部比较材料，未声称包含所有主题或本章所有有效候选。
720 次历史尝试的实际 tokens 均不可得；已知 executor 时间相加不代表墙钟时间。
BSR0 没有真实生成对照，不证明新流程的质量或节省。

## BSR1–BSR2 局部入口

`chapter.ts` 使用当前 task/artifact reader 重读章节卡与相关片段，读取 canonical 标题和前言，保存候选目录及局部计划。重读 BSR0 指定片段的全部候选共得到 31 项，原样保留其中未被 BSR0 点名的内容。

以下为外部生成 API 的可选入口；本次用户选择下方 Codex subagent 方式完成对照，这份 DeepSeek 草案保持未执行。

```powershell
node --import tsx evals/book-structure/chapter.ts prepare docs/performance/book-structure-bsr2-20261001
# 审阅 review.json，用户明确确认其中 plan_id + revision 后：
node --import tsx evals/book-structure/chapter.ts confirm docs/performance/book-structure-bsr2-20261001
node --import tsx evals/book-structure/chapter.ts run docs/performance/book-structure-bsr2-20261001
node --import tsx evals/book-structure/chapter.ts report docs/performance/book-structure-bsr2-20261001
```

prepare 不发模型请求。run 复核原书和贡献未变，保留每次请求、响应、拒绝/失败、usage 与耗时；中断后按同目录继续，已用调用额度不重置。received 响应先接纳再调用，reserved 未知结果停下保留额度。最多连续三次拒绝后暂停排错。

框架读取全部已接纳章级概述和前言；章节工作完整浏览候选索引，最多展开 6 项候选或 3 个原文范围。引用旧候选携带原证据，新增摘要必须有实际展开依据。不同任务使用 5000/3500 输出预算，整体输入按提示词与反馈一起计量。此入口不接管正式构建、不调用 embedding、不写原书发布文件。

report 保留旧成品和新结构；comparison.json 只在结果已生成后读取 acceptance.json，映射实际已选引用到验收原文位置。该映射不能替代内容审阅；历史实际 tokens 未知，不据估算宣称严格成本下降。

## Codex subagent 局部对照

用户已于 2026-10-01 明确选择用 subagent 完成 BSR2 对照。实际运行目录为 `docs/performance/book-structure-bsr2-codex-20261001/`；原 DeepSeek 目录保留为未执行草案，不再作为本片完成的确认门槛。

`chapter-subagent.ts` 复用同一 `prepareChapterExperiment`、框架/章节 prompt、Core 动作接纳及物化函数；不调用外部生成 API。`init` 冻结实验材料，`next` 交付当前阶段 prompt/input，`submit DIR ORDINAL RESPONSE_FILE` 记录语义响应并接纳，`report` 写入程序物化与旧成品对照。

独立 subagent 不继承父聊天，只读取 next 返回的文件，只写自身 response 文件并通过 submit 递交。session/report、验收答案、旧成品与源码均不交付生成者。输入上限估算 12,000 tokens；输出保留框架 5,000、章节 3,500 tokens 的形状预算，超限接纳为拒绝以便缩小范围重试。该实验适配器不是正式 opaque executor session；正式 Engine 接入仍在 BSR5。

记录区分语义提交次数与实际模型调用量：前者由适配器记录；后者包含 Codex 工具轮次和上下文管理，当前接口不可得。序列化输入/输出估算、真实墙钟、拒绝及原文读取均保存；实际 tokens 未知，不当作零费用或严格成本下降证据。

本次完成 13 次语义提交、0 拒绝，31 个候选选出 27 个章内重点与 15 个宏观路线点；内容验收及成本观测见[报告](../../docs/performance/book-structure-bsr2-codex.md)。`node --import tsx evals/book-structure/replay-subagent.ts docs/performance/book-structure-bsr2-codex-20261001` 从冻结输入重放已保存动作，逐项比较每步交付、状态与最终报告，不调用模型。

## BSR3–BSR4 主题与检索对照

用户选择本轮 Codex subagent + 本地 MiniLM 后，`themes.ts` 复用实际已接纳章卡与 BSR0 主题贡献，冻结 14 张章卡、202 候选及 212 处原文范围。每个贡献的候选全部保留，`acceptance.json` 不进入生成输入。规划者产生共享主题目录，独立 B/C 生成者在相同 prompt、证据要求和预算下执行；B 只用改进词法，C 加真实本地 embedding。每个主题从同一问题查询开始。

```powershell
node --import tsx evals/book-structure/themes.ts prepare OUTPUT_DIR tmp/sr5/embedding.json
node --import tsx evals/book-structure/themes.ts plan-next OUTPUT_DIR
node --import tsx evals/book-structure/themes.ts plan-submit OUTPUT_DIR PLANNING_RESPONSE.json
node --import tsx evals/book-structure/themes.ts next OUTPUT_DIR B
node --import tsx evals/book-structure/themes.ts submit OUTPUT_DIR B ORDINAL RESPONSE.json
node --import tsx evals/book-structure/themes.ts report OUTPUT_DIR B
node --import tsx evals/book-structure/themes.ts replay OUTPUT_DIR B
node --import tsx evals/book-structure/compare-themes.ts OUTPUT_DIR
```

C 组使用相同命令，将 B 替换为 C。生成者仅读取 `next` 给出的 prompt/input 文件，不读取另一组、plan/session、旧成品或验收材料。`next` 复用 pending 交付；`submit` 经 Core 接纳，拒绝不改变已接纳目录；三次连续拒绝停止。`replay` 使用保存的真实排序诊断与已接纳动作核对输入、状态和物化，不调用模型或 Provider。

`prepare` 的已确认授权记录专用于本次用户选择的实验方式；复用该入口跑新的真实实验前须取得对应授权，不能把旧记录当成新授权。计划存在时拒绝覆盖。每组最多 80 个动作，输入估算 20,000、输出估算 6,500 tokens；本轮 local embedding 独立上限 600 documents／80 queries／160 calls，含失败和重试。只加载已安装模型，不下载权重、不请求外部生成 API、不发布原书。

真实执行记录在 [book-structure-bsr4-20261001](../../docs/performance/book-structure-bsr4-20261001/review.json)，内容和成本结论见 [BSR3–BSR4 报告](../../docs/performance/book-structure-bsr3-bsr4.md)。语义动作次数、序列化估算和真实 embedding usage 分列；Codex 实际 tokens 未知。

原文审阅发现具体遗漏时，先保存初始对照和同一份问题文件，再用 `node --import tsx evals/book-structure/themes.ts reopen OUTPUT_DIR B THEME_REF ISSUE_FILE`（C 同理）重开指定主题；继续原来的 next/submit。输入含原结果与问题，其他主题不变，不重置额度、不再次扫描目录；before-revision 快照和 session 的修订事件参与完整重放。`compare-themes.ts` 在两组完成后读取验收答案，保存 comparison 与两组 verification；来源命中只用于定位，实际内容仍须对照原文。

2026-10-01 实验已完成：B 45／C 46 次提交、均 0 拒绝，各 4 主题／8 条依赖。每组含 2 次定点修订；其余三个主题保持一致。C 使用 216 文档／9 查询、36 次本地 embedding 调用，实际 5,147 输入 tokens；未证明整体成本收益，保持词法默认。

## BSR5 正式入口

BSR5 已把上述章节／主题合同接入正式 Engine、BuildPlan 消费者范围、获授权检索准备和三清单发布。确定性集成证据及命令索引见 [BSR5 报告](../../docs/performance/book-structure-bsr5.md)。此目录保留 BSR2/4 局部内容与成本证据；BSR6 前段发现和BSR7实际全书验收、安装交付均已完成，凭据见下文。

## BSR6 完整章节的新发现

`discovery.ts prepare` 仅复制 canonical source、manifest、profile、base、discourse 和 formula 公共前置成果到隔离书目录；不复制旧 BookStructure 任务、章卡、候选或成品。全书框架读取实际标题、小节目录和少量原文；发现从完整第 8 章的 653 个叶节点开始，使用正式 Core 路由、预算和接纳。

```powershell
node --import tsx evals/book-structure/discovery.ts prepare OUTPUT_DIR
# 本次实验方式与额度获用户确认后：
node --import tsx evals/book-structure/discovery.ts confirm OUTPUT_DIR
node --import tsx evals/book-structure/discovery.ts next OUTPUT_DIR
node --import tsx evals/book-structure/discovery.ts submit OUTPUT_DIR RESPONSE.json
node --import tsx evals/book-structure/discovery.ts report OUTPUT_DIR
node --import tsx evals/book-structure/discovery.ts replay OUTPUT_DIR
```

2026-10-01 用户明确选择独立 Codex subagent，最多 80 次提交（含失败／重试）、从首个 next 起 60 分钟。生成者不继承父聊天，只读取 next 提供的 prompt/input，只写自身响应；不读取源码、旧成品、session/report 或验收答案。每次输入、响应、接纳／拒绝、耗时和序列化估算落盘；pending 重用，计时与额度不因恢复重置，连续三次拒绝停止排错。代码版本修正导致的失败同样计入原额度。

report 从当前有效成果重建完整候选目录，逐项验证保存输入和成果；replay 在新的隔离目录按接受顺序重新接纳保存响应，比较每步实际交付、候选身份／内容和所有锚定载荷，不调用模型或 Provider。已有 replay 记录不覆盖。实际 Codex 调用量和 tokens 无法从接口取得，保持未知；序列化估算与提交次数不当作实际模型用量。此入口不调用外部生成 API、embedding 或原书发布。记录位于 [本次计划](../../docs/performance/book-structure-bsr6-20261001/plan.json)，结论见 [BSR6 报告](../../docs/performance/book-structure-bsr6.md)。

本次完成 42 次提交（接受 41、拒绝 1），58 分 52.291 秒；完整覆盖 653 叶节点且无缺口／重复，40 个正文任务发现 178 候选。序列化输入／输出估算为 186,318 / 38,545 tokens；实际模型用量未知。[重放](../../docs/performance/book-structure-bsr6-20261001/verification.json)核对所有接纳交付、候选与载荷一致，[原文审阅](../../docs/performance/book-structure-bsr6-20261001/content-review.json)确认九类已知机制及同 LID 不同含义保留，并记录复用 formula 的五处语义误标。BSR7 全书发现加整理、成本对照与正式交付已以独立授权完成，未复用BSR6额度。
# BSR7 全书入口

`full-book.ts` 管理本次已授权的全书真实执行，Core 负责冻结、接纳、检索准备、质量与发布。当前默认实验模式为 `semantic_required`，使用已安装本地模型；实际 Codex usage 不可见。

```powershell
node --import tsx evals/book-structure/full-book.ts next OUTPUT_DIR
node --import tsx evals/book-structure/full-book.ts status OUTPUT_DIR
node --import tsx evals/book-structure/full-book.ts submit OUTPUT_DIR RESPONSE_FILE
# 已接受原文框架、主 session 空闲时注册剩余章节发现
node --import tsx evals/book-structure/full-book.ts discovery-units OUTPUT_DIR
node --import tsx evals/book-structure/full-book.ts next OUTPUT_DIR - UNIT_LID
node --import tsx evals/book-structure/full-book.ts submit OUTPUT_DIR RESPONSE_FILE UNIT_LID
# 每章 complete_unit 后结束该发现 session；全部发现完成再调用主 next
node --import tsx evals/book-structure/full-book.ts fork-lexical OUTPUT_DIR
node --import tsx evals/book-structure/full-book.ts report OUTPUT_DIR
node --import tsx evals/book-structure/verify-full-book.ts OUTPUT_DIR
node --import tsx evals/book-structure/compare-full-book.ts OUTPUT_DIR
node --import tsx evals/book-structure/full-book.ts close OUTPUT_DIR
node --import tsx evals/book-structure/verify-reader-full-book.ts WORKSPACE INSTALLED_BOOK_MCP_EXE READER_EVIDENCE_DIR
# 已安装 Reader 打开新书后，验证 Book MCP 的默认选书入口
node --import tsx evals/book-structure/verify-reader-full-book.ts WORKSPACE INSTALLED_BOOK_MCP_EXE ACTIVE_READER_EVIDENCE_DIR --active-reader
```

生成代理仅读对应 `prompt_file` 和 `input_file`，不读取 task 文件、实现、旧成果或验收材料。三条发现 lane 使用不同章节与响应文件；各 session 和词法派生共享主运行的开始时间及总提交上限，失败／重试照计。`fork-lexical` 只在接受主题计划、尚未生成主题时执行一次。详细合同、实际全书结果与交付凭据见 [BSR7 报告](../../docs/performance/book-structure-bsr7.md)。新一轮执行仍需自己的授权，不能沿用本轮实验额度。

本轮总 token 消耗不设上限，单次任务沿正式输入／输出与传输容量。章节候选页和章节／主题原文目录页各 64 项，按交付的 next_offset 翻页，read 使用稳定原文索引。重放入口须完整 report 为 ready 后执行，已有重放目录不覆盖；保存响应验收不调用模型或 embedding Provider。Reader 验收实际启动指定成品 Book MCP，核对章节顺序、宏观路线、全部章内重点与主题投影；显式入口隔离 memory/private，active-reader 从Reader真实保存的当前书解析并隔离private。输出按连续UTF-8流解码，失败前保存实际投影，避免多字节字符跨Buffer损坏验收证据。

BSR7累计891/1200提交，884接受、7拒绝。两组最终来源、真实fresh重放及Engine发布通过，主交付为embedding组14单元/956重点/345宏观/11主线，已安装程序的新目录完整质量和Reader双入口均通过。生成在授权截止前结束，安装交付凭据集中于 [实际交付报告](../../docs/performance/book-structure-bsr7.md)。

两组 ready 后，`compare-full-book.ts` 验证候选目录及共享章节选择相同，将主题生成／收敛成本单独比较；共同发现和章节前段、C 全书成本、B/C 合计提交分别列示。通过主题选择形成的 `depends_on` 可以不同，共享候选正文必须相同。时间区分主题首次交付至最后接纳、真实 embedding 调用耗时和完整 C 运行；目录差异不直接判定语义质量或成本收益。

## BSR7 正式章节定点修订

来源审阅发现已接纳章节的重要遗漏时，在共享主题规划开始前，从主 session 提交问题文件。请求包含稳定问题身份、目标章节、审阅问题与修订要求，以及最多六个该章候选定位引用；它是审阅输入，语义选择仍由生成代理通过原 `structure_chapter` prompt 和动作 schema 提交。

```json
{
  "id": "BSR7-U8-OMISSION-01",
  "unit_lid": "8",
  "issue": "填写来源审阅指出的具体遗漏及本次定点修订要求",
  "candidate_refs": [
    "unit:8:fragment:0049#f50-2",
    "unit:8:fragment:0049#f50-3"
  ]
}
```

```powershell
node --import tsx evals/book-structure/full-book.ts request-chapter-revision OUTPUT_DIR REQUEST.json
node --import tsx evals/book-structure/full-book.ts next OUTPUT_DIR
node --import tsx evals/book-structure/full-book.ts submit OUTPUT_DIR RESPONSE_FILE
```

已有 pending 交付优先复用并完成；下次 `next` 优先交付定点修订。Core 先重放原章初选，再以新任务身份重入该章，正式输入携带请求和 `previous_selection`，继承完整 seen、inspected 和 evidence 账本。修订入口清空临时笔记和展开范围；生成者可直接 inspect 已浏览但未选的定位候选或 read 原文，无需重新发现或浏览全章。接纳仍经过当前输入、冻结与 Core writer gate。初选响应保留，修订响应单独留存。

请求存入书工作区 `.build/automatic-build/book-structure-chapter-revisions.json`，实验目录 `chapter-revision-events.json` 保存请求内容及每次修订任务的 opened／accepted／rejected 事件。登记请求不消费语义提交；修订动作、失败及重试全部计入原主运行与 B/C 共用提交和时间额度，额度不重置。`verify-full-book.ts` 在新工作区首次重放每个修订任务前恢复其持久请求，逐项核对正式输入、保存响应与最终结构；重放不调用模型或 Provider。

## BSR7 长章节分段选择与故障恢复

完整引用选择超过已标定的 candidate 请求容量时，登记该未完成章节的选择接续。请求指定当前动作边界；已有 pending 交付先完成，接续从下一动作开始。边界之前的任务、提示词、输入与回执保持原合同。

```json
{
  "id": "BSR7-U12-CAPACITY-01",
  "unit_lid": "12",
  "from_action_ordinal": 22,
  "issue": "完整114个重点和44个宏观点的引用超过单次candidate容量，请分段交付完整选择。"
}
```

```powershell
node --import tsx evals/book-structure/full-book.ts request-chapter-selection OUTPUT_DIR REQUEST.json
# 连续三次拒绝且已完成具体修复时，登记修复说明后恢复
node --import tsx evals/book-structure/full-book.ts resume-after-fix OUTPUT_DIR "填写已经完成并验证的具体修复"
node --import tsx evals/book-structure/full-book.ts next OUTPUT_DIR
```

接续使用正式 `structure_chapter_selection` 提示词，每次 `select_stops` 最多提交48个完整候选引用；最终 `select` 提交章角色、摘要及宏观引用，Core 从已接纳草稿注入完整重点集合，再执行原选择与来源门禁。未选候选仍在完整目录。选择接续保存到 `.build/automatic-build/book-structure-chapter-selection-continuations.json`，实验事件保存在 `chapter-selection-events.json`；新工作区重放在首个接续任务前恢复请求。

`resume-after-fix` 记录停止点、具体修复说明和时间，保留全部失败响应、提交计数和原截止时间；之后再次连续三次拒绝仍停止。登记选择接续和修复说明不消费语义提交，实际分段动作、最终选择与失败重试计入共同额度。

## BSR7 正式主题来源修订

主题来源审阅发现具体语义问题时，在该组已接受主题结果之后登记有限请求。请求仅作用于该组，不重做共享发现、章节选择或其他主题。

```json
{"id":"BSR7-B-BUDGETS-ACCOUNTING-01","theme_ref":"theme:budgets","issue":"填写实际来源问题、定位和局部修订范围"}
```

```powershell
node --import tsx evals/book-structure/full-book.ts request-theme-revision OUTPUT_DIR REQUEST.json
node --import tsx evals/book-structure/full-book.ts next OUTPUT_DIR
node --import tsx evals/book-structure/full-book.ts submit OUTPUT_DIR RESPONSE_FILE
```

已有 pending 交付保持；Core 先按原合同重放全部初始主题和一次目录收敛，之后按请求顺序复用既有 `reopenStructureTheme`，仅重开目标主题。官方输入携带问题、原结果和已交付证据，沿原 `structure_theme` prompt、检索准备和 writer 接纳，主题名称及章节成员保持；其他主题不变，修订后不再全目录收敛。未知或已被合并移除的目标返回实际 blocked。

请求保存到工作区 `.build/automatic-build/book-structure-theme-revisions.json`，实验目录 `theme-revision-events.json` 保存 requested/opened/accepted/rejected。登记不计语义提交，修订、失败和重试共享原额度/截止；fresh 保存响应重放在第一个修订任务前恢复原请求，不调用模型或 Provider。

