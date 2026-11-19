# 笔记召回评测（M0）

[ADR-0178 §8](../../docs/adr/0178-in-context-retrieval-practice-and-learning-map.md) 与[切片方案 M0](../../docs/切片方案-情境练习与学习脉络.md)。用途：在建设学习脉络之前，确认 BM25 与 MiniLM 是否漏掉了真正相关的旧笔记。

## 标注（只能由笔记的主人完成）

1. 从自己的学习记录中选 20–30 个真实教学回合，至少涉及两本书。
2. 每个回合写一条用例：`query` 为该回合的原始提问；`relevant` 为在**完整笔记列表**中标出的全部相关笔记 `note_id`。标注时不看任何检索结果。
3. 可选 `book_id` 限定材料。

```json
{"cases":[{"id":"t1","query":"为什么平均速度要算上停车时间？","relevant":["note-id-1","note-id-2"]}]}
```

用例文件与报告包含你的私人笔记身份，请放在仓库之外。

## 运行

先启动本地服务（默认 `http://127.0.0.1:8787`，MiniLM 就绪后语义一路才有结果），然后：

```bash
node evals/note-recall/run.mjs ~/private/note-recall-cases.json ~/private/note-recall-report.json
```

报告给出各路 recall@5 / recall@10、两路都漏掉的相关笔记（目标集），以及 `gate`：目标集为空则按 ADR-0178 不进入 M2/M3 的召回呈现。

## 结构召回（M2）

1. 在阅读器的练习设置中开启“整理我的学习脉络”，等待后台把已交付的讲解整理进脉络（计入额度）。
2. 用例可加 `turn_id`（该教学回合的回合 id，作为结构召回的焦点；不填则以与提问最相近的脉络陈述为焦点）与 `learning_workspace_id`。
3. 加 `--structural` 运行：

```bash
node evals/note-recall/run.mjs ~/private/note-recall-cases.json ~/private/note-recall-report.json --structural
```

每个用例各跑一次两路与三路融合。报告的 `structural_new_hits` 是结构召回在目标集上的新增命中；`fusion.new_recall` / `fusion.false_recall` / `fusion.displaced_relevant` 分别统计三路融合在前 10 名中新进入的相关笔记、新进入的无关笔记、以及被挤出的相关笔记。`m3_gate` 为 `stop` 时按 ADR-0178 不进入 M3。

```bash
node --test evals/note-recall/core.test.mjs
```
