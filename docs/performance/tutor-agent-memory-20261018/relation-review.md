# 关系审核 v11：第一轮与第三轮重审

本文保留 v11 的冻结实验结果。后续主教师与审核共用写作规约的实现及实测见 [技术讲解写作规约](plain-writing.md)。

2026-10-18。已把审核改为对照用户所问关系与实际回答，并用原模型各重审一次 Q1、Q3。两例仍为 `accept / answered`，`findings` 为空；本次没有改善此前指出问题的发现结果。

## 实现变化

唯一运行时规则为 [tutor_review_rules.md](../../../crates/runtime/src/tutor_review_rules.md)，版本 `teaching-delivery.v11`。R1 要求从用户原话确定关系、指代、比较基准与范围；R2 要求依据实际回答说明该关系如何得到解释，不能以相关对象、接口、阶段、条件和结果的罗列替代连接，也不能替正文补入教材或审核自身知道但回答没有表达的内容。准确回答窄问题即可接受，不要求扩展成完整机制课。

`request_resolution` 新增三项：

- `requested_relation`：本次用户要厘清的关系及范围。
- `delivered_relation`：实际回答对同一关系作了怎样的解释。
- `answer_quotes`：支撑上述判断的交付原句。

`tutor.rs::normalize_delivery_review` 要求两项关系描述非空；`answered / bounded_answer` 至少有一条真实回答引文。引文只可来自 `proposed_delivery.text` 或实际页面观察中的 `dom.text`、`dom.visible_text`、`reading.text`；教材、设计、作者语义说明 `readable_content` 和隐藏内容均不能作为已交付原句。`unresolved` 可无引文，但所引内容同样须真实。未解决时给主教师的修订意见同时带上两侧关系。

这些字段随原有 `request_resolution` 保存到工作记忆，无新增存储系统。原有 unresolved／具体缺陷退回规则不变。单一规则文件没有写入 Embedding、LID 或这两题的具体答案。

## 固定输入与调用方法

Q1 使用 run-5 的 `request-029.json`，Q3 使用 `request-049.json`。完整稿件、来源、页面材料、已有 v9 审阅与工作记忆及模型参数均保留，只替换 system。未加入上轮 v10 的新评判、已知错误清单或期望答案。规则在调用前冻结；两例各一次，没有重采样、纠错反馈或应用记忆写入。

请求模型为 `deepseek-v4-flash`，响应模型名为 `deepseek-flash`。输入见 [inputs.json](../../../tmp/tutor-three-turn-20261018/relation-review-probe/inputs.json)，冻结规则见 [rules-used.md](../../../tmp/tutor-three-turn-20261018/relation-review-probe/rules-used.md)，公开原始输出见 [results.json](../../../tmp/tutor-three-turn-20261018/relation-review-probe/results.json)。

## 实际结果

| 病例 | 模型决定 | 关系处置 | 引文数量 | 具体缺陷 | Rust 处理后决定 |
| --- | --- | --- | ---: | --- | --- |
| Q1：Embedding 在哪里、怎么用，并给演示页 | accept | answered | 5 | 空 | accept |
| Q3：确认输入是预构建中间产物 | accept | answered | 5 | 空 | accept |

### Q1：仍把角色与相关流程视为解释充分

审核将所问关系定义为：“Embedding 在本书第 6 章 BookStructure 候选召回中的使用位置与调用方式：它在哪里被调用、在召回中承担哪一步，以及按要求给出的演示页。”

它把实际解释概括为 Core 准备阶段的调用、短投影、按相似度补充候选、与词法合并去重、额度及演示切换。五条引用包括正文的候选角色、调用阶段、合并流程，以及页面的操作说明与候选数量。

公开理由仍据这些内容认定“位置与用法都已得到解释”。它没有提出查询文本与条目文本两侧如何形成向量、经过比较得到候选排序这一必要连接，也没有指出页面“也不判断相关性”的过强表述。`findings` 和 `open_questions` 均为空。原返回见 [Q1 审核输出](../../../tmp/tutor-three-turn-20261018/relation-review-probe/q1-final-response.json)。

### Q3：看到了有问题的句子，但把它们作为通过依据

审核准确识别用户要确认“进入 embedding 的内容是否不是原文本身，而是准备阶段预构建出来的中间产物，并厘清这种中间产物与原文的关系”。

它的回答引文直接包含：“它更像「投影」，不是「改写」”和“所以它和原文的差别主要在「短」和「只留指定字段」”。它仍认为选料、截取、时点的区分已经讲清关系，公开理由以“未发现影响该确认关系的缺口”结束。

因此本例不是没读到问题句：审核已经引用了这些句子，却没有识别“原文形成预构建字段”与“字段再取短投影”的层次区别，以及对用户未提出的改写主张的收窄。它还通过真实用户原句识别出这是一个确认问句，但这一判断没有纠正接受结论。`findings` 和 `open_questions` 均为空。原返回见 [Q3 审核输出](../../../tmp/tutor-three-turn-20261018/relation-review-probe/q3-final-response.json)。

## 验证与用量

45 项相关验证全部通过：审核合同 8 项、教学流程 36 项、保存的真实输出校验 1 项。真实输出校验对上述两例执行生产的 `normalize_delivery_review → validate_review_grounding → validate_review_work`，两例均有效，accept 不变。具体结果见 [runtime-validation.json](../../../tmp/tutor-three-turn-20261018/relation-review-probe/runtime-validation.json)。

测试入口为 `cargo test -p runtime --lib review_resolution`、`cargo test -p runtime --lib teaching_`。设置 `TUTOR_REVIEW_PROBE_DIR` 为本次产物目录后，以 `cargo test -p runtime --lib recorded_reviews_follow_runtime_contract -- --ignored` 核验保存结果；它不会请求模型。日志保存在产物目录 `test-*.log`。

| 病例 | 输入 tokens | 输出 tokens | 总 tokens | 请求耗时 |
| --- | ---: | ---: | ---: | ---: |
| Q1 | 42,492 | 12,002 | 54,494 | 54.410 秒 |
| Q3 | 22,564 | 14,421 | 36,985 | 60.856 秒 |
| 合计 | 65,056 | 26,423 | 91,479 | 并行墙钟 60.918 秒 |

两例均 HTTP 200、`finish_reason=stop`，未重试。保存公开返回和用量，不保存内部推理文本或凭据。

## 已知限制

引文真实性和合同校验只保证判断依据可追溯，不证明解释正确。此次观测表明审核能填写两侧关系并引用实际回答，但仍把相关流程或有问题的解释认作充分；没有证据说明 v11 在这两例上改善了语义判断。

本次是冻结最终稿的定向重审，没有让主教师接收新意见再改稿，没有重做页面交互，也没有更换运行中演示服务的 v9 二进制。历史 v10 试验保留在 [审核规章与四例实验](review-rules.md)，不覆盖其结果。
