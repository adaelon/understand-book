# BSR0 真实样本与比较合同

日期：2026-10-01。状态：完成。承接 [ADR-0151](../adr/0151-book-structure-global-outline-and-semantic-retrieval.md) 与 [BSR0 方案](../切片方案-BookStructure全局框架与语义召回.md#bsr0-真实样本与比较合同)。

本片固定可重放基线、第 8 章机制材料、跨章主题验收依据和后续红测接点。没有修改生产算法或已发布书籍，没有模型或 embedding 调用。

## 来源与重放

唯一输入为 `E:/allwork/download/agent/lifebook/.understand-book/ai-infra-book-complete`。
使用 `base.json` 的 LID 与字符范围、`source.txt`、`book_structure.json`、
`.build/automatic-build/v3/artifacts/book_structure/`、`v4/tasks/book_structure/` 和 `v2/tasks/book_structure/`。

[提取入口](../../evals/book-structure/extract.ts)调用现有 task/artifact reader；冻结原文摘录与当前 source span 逐项比较。
14 张最终章节卡与拼接输入一致，702 个产物对应已提交尝试；5 个局部贡献经现有 materializer 加 95 个关系增量，重放得到与发布文件相同的 spine / throughlines / key_stops。
生成结果可用 `node --import tsx evals/book-structure/extract.ts --check` 从实际目录重新比较。

[基线 JSON](../../packages/core/testdata/book-structure/baseline.json)保存每项的任务／产物路径、候选原 ID、贡献来源、预算和尝试记录。
`chapter_cards[].card.candidate_key_stops` 是 138 个章节候选；`final` 是发布结果；`references` 是 622 次实际来源／依赖引用及字段位置。
原书和历史任务输入全文不复制入仓库。

## 结构基线

| 项目 | 实测 |
| --- | ---: |
| 结构单元／最终章节卡 | 14（封面、前言、12 章） |
| 最终章节卡内候选 | 138 |
| spine / throughlines / key_stops | 14 / 53 / 34 |
| 双单元 throughline | 49 |
| 显式阅读依赖 | 1 |
| 可解析来源／依赖引用 | 622 / 622 |
| 摘要将 LID 写成章号 | 13 条 |

真实映射为 LID 1=封面、2=前言，LID 3–14=第 1–12 章。第 8 章《推理优化》是 LID 10。
`chapter_number_suspects` 保存全部 13 条原摘要和对应标题；逐条对照均将相关单元 LID 当作章号，例如 `relation-42-0` 将第 8、9 章写为第 10、11 章。
该定位规则是本书的嫌疑提取规则，判定依据是实际章节标题及原摘要语义。

| 局部拼接贡献 | 单元 LID | 最终重点数 |
| --- | --- | ---: |
| `stitch:fragment:0000` | 1、2、3、4 | 7 |
| `stitch:fragment:0001` | 5、6、7 | 7 |
| `stitch:fragment:0002` | 8、9、10 | 6 |
| `stitch:fragment:0003` | 11、12、13 | 6 |
| `stitch:fragment:0004` | 14 | 8 |

分组与保留数量有关联，但不能从这组历史数据推导任意重分组必然丢点。相同内容改换分组，现有 materializer 仍保留内容；实际丢失发生在模型重写局部拼接产物时。

## 历史预算与实际用量

下表为 702 个已接纳任务冻结的估算，不含失败重试和完整会话成本。正文、语义提示和交付开销分列，输出列为累计上限。

| 任务类别 | 数量 | 正文估算 tokens | 提示估算 tokens | 交付开销估算 tokens | 输出上限累计 |
| --- | ---: | ---: | ---: | ---: | ---: |
| 片段 | 410 | 1,943,675 | 297,250 | 439,955 | 419,840 |
| 归并 | 190 | 736,119 | 174,040 | 167,465 | 194,560 |
| 单元 | 1 | 389 | 1,078 | 363 | 1,024 |
| 拼接 | 5 | 17,103 | 4,345 | 4,589 | 5,120 |
| 关系选择 | 1 | 3,298 | 169 | 861 | 1,024 |
| 关系增量 | 95 | 116,745 | 30,495 | 42,234 | 97,280 |
| 合计 | 702 | 2,817,329 | 507,377 | 655,467 | 718,848 |

单任务候选输出上限均为 1,024 tokens。

任务存储记录 **720 次尝试**：702 个 committed、13 个 retryable_failure、5 个缺 metrics。
input / output / cached input tokens 的已知尝试均为 **0**，未知均为 **720**；已知 token 小计记为 `null`。
715 条记录的 executor 时间合计 **49,682,039 ms（约 13.80 小时）**，另外 5 次未知；该和包含并发任务与失败尝试，不能作墙钟耗时。
整体墙钟、货币成本和完整实际模型调用量未知。仅有尝试记录不能证明每次恰好一次模型调用。

## 第 8 章比较材料

[章节输入](../../packages/core/testdata/book-structure/chapter-8.input.json)保存原 9 个候选和旧发布的 2 个重点，另从已接纳片段补回 9 条候选记录。

| 补回内容 | 原贡献 | 原文位置 |
| --- | --- | --- |
| 连续批处理的迭代接纳、吞吐／间隔取舍、分块 prefill | `unit:10:fragment:0005` | `10.9.3.2`、`.8`、`.13` |
| 共享尾块写时复制 | `unit:10:fragment:0008` | `10.10.3.5` |
| 前缀复用条件、递推快照密度与重算 | `unit:10:fragment:0009` | `10.10.4.2`、`.10` |
| KV 压缩净时间收益、权重卸载 | `unit:10:fragment:0013` | `10.11.3.23`、`10.11.4.2` |
| 卸载的 PCIe 时间限制 | `unit:10:fragment:0014` | `10.11.4.11` |

连续批处理在片段中已有定义和算例，最终章卡只在摘要及 evidence 中提到它，9 个候选中已经缺失。
拼接后剩下 `10.8.3.14` 显存公式与 `10.13.3.11` 有效吞吐公式两个点，章卡中的分页、缓存价值、推测采样及准入等候选又被压掉。

`acceptance.json` 固定显存、合批、连续调度、分页与共享、前缀恢复、缓存价值、压缩卸载、推测解码和有效产出的原文依据。
每项均有输入内原文范围；数学公式被分成多个 LID 时保留连续范围，避免只把裸公式交给后续比较。

## 跨章主题合同

问题固定为“状态复用如何改变容量、传输和恢复成本”。[主题输入](../../packages/core/testdata/book-structure/state-reuse.input.json)包含第 2、8、9、10、11 章的卡片、有关片段候选和原文。

| 应比较的机制 | 原文范围 | 需要保留的新增条件 |
| --- | --- | --- |
| 模型上下文状态 | `4.19.3.3–28`、`4.20.5.37–40` | 因果与计算条件不变；头共享改变表示容量 |
| 单机 KV 分配与复用 | `10.10.2–4` 中的已选范围 | 分页、前缀共享、写时复制、快照位置各有作用 |
| 跨实例 KV 交接 | `11.8.3.14–18`、`.40–47` | 传输时间、两端驻留、完成标记和释放顺序 |
| 推理持久化与部分重算 | `11.11.4.2–7` | 读到页不等于形成可复用连续前缀 |
| 训练 checkpoint | `12.12.4.2–4`、`.17–18` | 同一进度的参数、优化器、随机与数据状态 |
| Agent 轮次提交 | `13.13.2.8–19` | 完整轮次、连续保存进度、外部操作与重做成本 |

验收答案独立保存在 [acceptance.json](../../packages/core/testdata/book-structure/acceptance.json)，不被提取器读取，也不进入 `.input.json`。
后续章节实验先在 BSR2 无 embedding 比较；BSR4 的 B/C 使用同候选、生成条件和预算，仅改变检索模式。
分别报告旧候选整理成本与新书发现加整理成本。

## 最小反例与真实函数接点

[回归测试](../../packages/core/test/book-structure-bsr0.test.ts)包含以下三个预期失败断言，使用 `it.fails` 保留为可执行待修合同。

| 反例 | 当前行为和接点 | 后续转绿位置 |
| --- | --- | --- |
| 封面／前言导致章号偏移 | `bookStructureRelationEntries` 生成 `unit:10` 的 name=`10`，真实标题为第 8 章 | BSR1 canonical 标题映射、BSR2 实际输入 |
| 章卡 9 点到拼接 2 点 | 真实局部 payload 已缺 7 点，`materializeBookStructureContributions` 无章内接受引用可恢复它们 | BSR1 候选目录与引用组装、BSR2 章节选择 |
| 主题相关却不构成前置 | 用训练状态 `12.12.4.3` 与 Agent 提交 `13.13.2.15` 提议 `13 depends_on 12`；现有 gate 接纳双端来源，apply 写入该提议 | BSR4 `resolve_theme` 的有据依赖输出 |

第三例是人为固定的错误语义提议，不是已发布书中的错误依赖；已发布结果只有 1 条显式依赖。
来源 gate 正确履行交付范围校验，不能据此断言语义成立。后续将该期望接到主题 resolver 输出，不能给 gate 加关键词规则来假装理解前置关系。
普通测试还断言相同保留内容重分组不丢点、重复贡献幂等，并独立验证反例确实到达目标函数，避免异常被 `it.fails` 掩盖。

## 验证

- 实际来源重放：`node --import tsx evals/book-structure/extract.ts --check`，检查提取内容与冻结夹具一致。
- 定向回归：`pnpm --filter @understand-book/core exec vitest run test/book-structure-bsr0.test.ts`，10 项，含 3 项预期失败合同。
- 类型检查：`pnpm --filter @understand-book/core typecheck`，覆盖由测试导入的提取器。

## 已知限制

- BSR1–BSR7 尚未实施；本片固定证据和接点，不宣称章号、丢点或主题质量已修复。
- 原 138 个候选已经过归并，9 条补回只是局部反例材料，不是完整候选上限。
- 真实 tokens、完整调用量和端到端成本仍未知；不能据历史估算宣称新算法节省比例。
- 本片无模型实验，主题语义质量须在后续原文对照中验收；默认检索保持 lexical_only。
