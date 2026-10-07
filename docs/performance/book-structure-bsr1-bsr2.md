# BSR1–BSR2 候选保留与章节引用选择

日期：2026-10-01。BSR1、BSR2 完成。独立 Codex subagent 已完成真实章节对照，内容验收通过，成本观测及限制见[对照报告](book-structure-bsr2-codex.md)。设计：[ADR-0151](../adr/0151-book-structure-global-outline-and-semantic-retrieval.md)。

## 实现

`book-structure-candidates.ts` 保存候选的贡献派生引用、所在章节／小节、含义、条件、教学理由及原证据。旧产物没有独立含义／条件字段时，完整保存其原理由，不编造条件。相同贡献重复读入保持幂等；同 LID 的不同候选保留区分；归并层只有显式引用原候选且内容一致才消除重复。

`readStructureCandidateCatalog` 经现有 generation reader 重读当前有效任务的卡片与片段。局部入口复用第 8 章最终卡与 BSR0 点名的五个片段，完整接入这些贡献的 **31 个候选**，包含 BSR0 的 9 个章卡候选和 9 个片段补回候选。其余 13 个也保留，未用验收清单过滤输入。

章节选择使用 `accepted_stop_refs` 与其子集 `macro_stop_refs`。`materializeStructureChapterSelections` 从完整目录携带正文和证据；宏观路线只控制 spine 引用，不删除章内已选点。未选候选继续留在目录。LID 不改写，标题从 canonical Markdown heading paragraph/source span 提取，交付卡片、拼接和关系输入。模型不能写入 Core 的标题元数据。

`book-structure-planning.ts` 提供可修订全书框架与章节交互。框架读取真实目录、已接纳概述及前言；章节完整浏览候选索引，单次最多展开 6 个候选或 3 个原文范围。索引／框架不授予新章节摘要的证据范围，实际 inspect/read 才记录交付。新摘要按本章实际交付证据接纳，宏观选择必须是章内选择子集。

## 验证

累计 15 个测试文件共 73 项通过，其中 BSR0 保留 1 项 `it.fails` 的 BSR4 语义反例。Core `tsc --noEmit` 通过。

| 验证对象 | 结果 |
| --- | --- |
| BSR0 9→2 丢点反例 | 已转绿：9 个显式接受引用全部物化，宏观路线独立 |
| 封面／前言后的 LID 10 | 已转绿：展示第 8 章《推理优化》 |
| BSR0 指定的 18 个候选 | 全部保留，含连续批处理、分块 prefill、写时复制、复用条件、快照、压缩和卸载 |
| 两章完成顺序／分组／重复应用 | 内容、身份和证据一致；条件进入公开重点理由 |
| 无效引用／外章引用／冲突来源 | 拒绝，不产生部分物化结果 |
| 宏观路线与章内内容 | 2 点宏观路线不删除 18 个已选章内点；未选候选留存 |
| 框架与章节新解释 | 非本章、未交付、仅预览的证据均拒绝；索引未浏览完整时拒绝完成 |
| 既有路径 | append production、来源接纳、分片预算、复用失效、paper prompt 与 optional Pass2 回归通过 |

新增测试为 `book-structure-candidates.test.ts`（6 项）、`book-structure-planning.test.ts`（4 项）；BSR0 两项断言接到新引用接口，保留历史真实 payload 仍只有 2 点的事实。append production 测试在重新构造模型候选时剥离新增的 Core-owned `unit_titles`，与其原先剥离来源元数据的目的相同。

## 真实对照结果

用户明确选择用 subagent 后，以 `evals/book-structure/chapter-subagent.ts` 驱动独立 Codex 生成者，复用同一输入准备、prompt、Core 接纳与物化路径。生成者不继承父聊天，不接收验收答案及旧成品。结果保存于 `book-structure-bsr2-codex-20261001/`，原 DeepSeek 计划保持未执行草案，不再是本片的确认门槛。

31 个候选全部浏览及展开，12 处原文全部读取；14 个目录单元形成框架及 4 个暂定主题。模型选择 27 个章内重点，其中 15 个进入宏观路线。预定 9 类机制经原文对照全部保留，4 个未选候选继续存在。旧发布只有显存和有效吞吐 2 点的问题在本次章节结果中消除。

共 13 次语义提交、0 次拒绝，425,446 ms。序列化输入／输出估算合计 88,375／3,107 tokens，实际调用数及 tokens 不可得；外部生成 API 与 embedding 调用均为 0。逐步重放的输入、响应接纳、状态与物化报告一致。适配器另有 2 项测试覆盖拒绝后状态保留、pending 重复读取、重复提交、输出预算及完整框架→章节流程。[完整内容与成本记录](book-structure-bsr2-codex.md)。

## 已知限制

- Codex 实际模型用量不可得；输入／输出估算不包含工具轮次、思考和保留上下文。本次已通过局部内容验收，不能据估算声称严格成本下降。
- 31 个候选只覆盖此次复用的卡片和五个片段，不代表全章前段发现已完整；全章发现替换留 BSR6。
- 正式 Engine 的新任务路由、policy/recovery 和发布接入留 BSR5；本片提供 Core 合同与局部消费者。
- 旧 702 项任务的实际 tokens 仍未知，历史估算与新调用实际用量分列。主题及阅读前置语义留 BSR4。
