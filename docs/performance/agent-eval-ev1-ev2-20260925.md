# EV1–EV2：显式任务合同与分离判分

状态：EV1、EV2 实施完成，2026-09-25。评测协议与报告使用本地 LA7 历史运行验证，不代表当前版本 Agent 成绩；三案人工标签仍未确认。

## 交付

- `evals/semantic/task-spec.mjs`：32 道冻结旧题的显式 `EvalTaskSpec`，包含层级、交互条件、必要要求、证据替代、同链 family 和仅用户材料的输入投影。`cross-01` 的跨章并列事实仍为 L1；`restart-01` 为 L1 且 `cross_session/new_chat_after_restart`。没有解释要求的分类题不因关键词变化获得隐藏要求。
- `evals/semantic/agent-run.mjs --task-protocol`：新运行版本入口。问答与恢复调用 `productInput`；恢复设置阶段不投影未来追问。旧运行入口继续使用原协议。
- `evals/semantic/task-quality.mjs`、`task-quality-run.mjs`：新质量协议从不可变产品运行准备样本；绝对评分每答案独立请求，配对偏好仅对双方实际交付的答案双顺序请求。逐要求结果保留 `met/unmet/unknown/not_applicable`、`valid/not_run/invalid/needs_review` 的评估状态及答案、来源、效果引用。任务结果为 `pass/fail/unresolved`；确认失败不因别的未决项或 Judge 未运行而消失。
- `evals/semantic/quality-run.mjs --protocol task-v1`：显式选择新协议并在新目录生成 `bundle.json`、`jobs.json`、原始评分记录、`report.json` 与 `report.md`。旧 `historical-v2` 入口和旧成绩保持可回读。EV3 之前新协议只允许探索性模型评分。

## 固定历史包

本地忽略目录：`evals/semantic/quality-results/2026-09-25-ev1-ev2-la7-final/`。只读 LA7 `run.json` 准备 32 个任务、56 个系统样本（Chunk 24、Agent 32）、79 个可选评分请求。报告始终保留完整分母。未运行评委时，Agent 有 6 个产品交付失败，Chunk 有 1 个；四个重启任务的已记录动作和持久状态通过，其余已交付答案保持未决。

对三案各运行一次探索性绝对评分。最终包在核对目标作业输入与原始输入逐字段相同后重放三条原始评分记录；记录中的 `replayed_from` 保留来源作业 ID，没有再次调用模型。原始尝试留在本地：

| 案例 | 新协议观察 | 边界 |
| --- | --- | --- |
| `contrast-03` | 模型判必要内容、解释与来源均满足；暂定 `pass` | 旧摘句标点误杀不再由逐字摘句比较决定；尚无独立人审 |
| `contrast-04` | 模型判内容满足、来源未满足；`fail` | 无实际有效交付来源，公共参考原文没有替答案补引用 |
| `cross-02` | 评委返回的 JSON 不符合已冻结输出结构；`invalid_output`，任务 `unresolved` | 保留原始输出与用量，不能视作产品内容失败或成功 |

探索性调用 3 次，其中 2 次格式有效、1 次无效。未运行 76 个预备评分请求；配对偏好没有实测结论。当前报告 Agent `pass 5/fail 7/unresolved 20` 含四个已确认的重启状态和上述两个有效探索性评分；这些数仅验证协议的分母与状态表示，不是经人工校准的性能结果。旧 Agent 15/24 保持原样。

## 定向验证

- `pnpm eval:semantic:test`：30/30 通过，确认旧评分和报告路径未退化。
- `pnpm eval:quality:test`：16/16 通过，包含新协议任务分类、输入投影、修复后实际交付、来源/动作失败、必要项失败与另一项 unknown、JSON 无效、配对单次失败及双顺序分歧。
- `node evals/semantic/agent-run.mjs --validate --task-protocol`：32 题、8 类通过语料校验，不调用模型。
- `quality-run.mjs prepare/report --protocol task-v1`：对 LA7 原始运行生成可复评分报告；旧 `run.json`、`summary.json`、`report.md` 未修改。

## 已知限制

- 此处没有当前版本产品重跑，也没有 EV3 人工校准。模型暂定通过不构成正式评分结论。
- `cross-02` 的新输出格式无效，报告保留未决；不能从其原始内容推断自动通过。
- 新协议报告中的绝对判定与配对偏好仍待人工按用途分别校准；未运行的评分请求明确记 `not_run`。
