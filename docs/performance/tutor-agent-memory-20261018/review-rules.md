# 教学审核规章：来源与接入边界

本文保留 v10 规章的设计与实测记录。后续 v11 将审核改为所问关系与实际解释的对照，见 [关系审核重审记录](relation-review.md)；各次实际使用的规章随实验产物冻结保存。

2026-10-18。新的审核规章采用六个稳定规则编号，唯一运行时文本为 [tutor_review_rules.md](../../../crates/runtime/src/tutor_review_rules.md)，版本 `teaching-delivery.v10`。本文件记录设计依据、程序接入和四个固定病例的真实模型结果。

## 借鉴的 Socratopia 本地实现

依据为本机发布包的既有 REA 静态还原，没有重新请求 Socratopia 云端，也没有运行同样三问作产品对照。

| 本地可确认的做法 | 证据位置 | 本项目的采用方式 |
| --- | --- | --- |
| 教练区分学习者主动追问与教师重复；学习者已解释并完成迁移后，教师换例重复同一判断归于教师。新变量或改变支架不自动算重复。 | [module-48130.js:12](E:/Socratopia-REA-Analysis/2026-10-18/readable-teaching/module-48130.js:12) | R6 保留归因及继续回答主动追问的原则；不强制复问或练习。 |
| `companionIssue` 与 `learnerNeed` 分开。教师问题允许 `none`；没有会改变下一轮教法的具体需要时，`learnerNeed` 留空。 | [module-48130.js:17](E:/Socratopia-REA-Analysis/2026-10-18/readable-teaching/module-48130.js:17) | R4 将教师错误留在教师工作记录，未知理解保持未知；不为填字段制造学习者缺陷。 |
| 只有影响后续理解的具体未决关节才深化；相关类比、假说和反例不直接算跑题。 | [module-48130.js:16](E:/Socratopia-REA-Analysis/2026-10-18/readable-teaching/module-48130.js:16) | R2 只要求本问题必要的连接，R6 不按表面话题变化拒绝交付。 |
| 固定指导优先回应最新消息；空泛回答应由教材事实、推理或例子补实。 | [module-48130.js:191](E:/Socratopia-REA-Analysis/2026-10-18/readable-teaching/module-48130.js:191)、[module-48130.js:218](E:/Socratopia-REA-Analysis/2026-10-18/readable-teaching/module-48130.js:218) | R1 以当前请求为判断对象，R2 要求可理解的关系，旧偏好不能改写直接回答意图。 |
| 判为教师导致重复后，程序把下一步覆盖成 `advance`，并省略可能冲突的行动理由。 | [module-48130.js:307](E:/Socratopia-REA-Analysis/2026-10-18/readable-teaching/module-48130.js:307) | 借鉴“判断结果影响控制流”，新增本项目的未解决请求／具体缺陷与接受决定之间的程序一致性约束。没有照搬自动推进。 |
| 课堂摘要区分教师讲过与学习者掌握，保留教师误称、学习者纠正的角色；缺少证据记尚未验证，新证据可以替代旧状态。 | [endclass-memory.json:1583](E:/Socratopia-REA-Analysis/2026-10-18/prompt-extraction/endclass-memory.json:1583)、[endclass-memory.json:1643](E:/Socratopia-REA-Analysis/2026-10-18/prompt-extraction/endclass-memory.json:1643) | R4 保真归属，R6 接续真实修订；agent 工作记忆与学习者证据分开。 |

Socratopia 的教练分析最近已完成的对话，为下一轮提供方向。请求设置为最多 600 tokens；教师完成后启动，下一轮读取结果。对应 [教练调用](E:/Socratopia-REA-Analysis/2026-10-18/readable-teaching/module-9716.js:547)、[下一轮读取](E:/Socratopia-REA-Analysis/2026-10-18/readable-teaching/module-9716.js:1097)、[本轮结束后启动](E:/Socratopia-REA-Analysis/2026-10-18/readable-teaching/module-9716.js:1227)。这不是本项目交付前的接受／修订流程。

## 本项目自行设计的六条规章

| 规则 | 具体判断对象 | 需要修订的情形 |
| --- | --- | --- |
| R1 回答当前问题 | 用户原话、实际正文和页面 | 必要问题未答，邻近知识代替回答，或将必要补查留作未来工作却声称完成。 |
| R2 必要连接与层次 | 本次问题要求的对象、操作和结果关系 | 只堆术语和结果，跳过必要前提／连接，或提前揭晓拟留给学习者的推断。 |
| R3 来源范围与条件 | 实际已读原文与关键结论 | 局部事实被推广、可能性被加强、推论冒充原文，或本问题必要依据尚缺。 |
| R4 学习者归因 | 原始表达、已给帮助、教师主张 | 教师错误被记成用户缺陷，准确确认被扭成纠正未表达的强说法，或无依据声称掌握。 |
| R5 实际页面与观察 | 保存版本、可读内容、实际预览和用户场景 | 交付内容与观察矛盾，或以标题／设计意图冒充已经发生的页面行为。 |
| R6 纠错接续与停止 | 历史意见、实际变化、当前问题 | 已解决意见反复提出，真实旧错误仍影响当前交付，或为配额增加问题和练习。 |

六条规章由当前任务需要设计，不能说是从 Socratopia 复制出的事实验收机制。v9 已有多数语义要求，包括当前意图、来源条件、理解依据、实际页面和后续复核；v10 将它们收拢成固定编号和可保存的缺陷记录，替换原长提示词。新增行为是要求显式回答“本次请求是否解决”，并让未解决状态或已确认缺陷阻止接受。

输出合同新增 `request_resolution:{status,question_quote,reason}` 与 `findings:[{rule_id,location,evidence,required_change}]`。`status` 为 `answered`、`unresolved` 或 `bounded_answer`；`question_quote` 必须来自冻结当前用户问题。`unresolved` 或非空 `findings` 由程序归一为 `revise`，即使模型同时写了 `accept`。若模型写 `revise`，却没有具体缺陷且不标为未解决，则作为无效审核要求补齐，不能静默放行。`bounded_answer` 只用于证据确实不可得且说明界限已经回答请求；其语义仍由模型判断。历史未决事项本身不阻塞不相关的新问题。

保留 `learner_claims` 的原话依据及 `agent_work` 的可靠结论／未决工作。可靠结论的每条来源继续核对 `source_id`、`source_revision`、`lid` 和精确原文引文；这些字段验证出处，不证明语义推论正确。没有新增独立审核 agent 循环或固定第二次完成调用。

## 已知限制

Socratopia 本地证据支持的是教学过程归因、记忆保真与下一轮方向控制，不能证明它能发现本项目的事实或交互错误；云端规则也不在这些还原材料中。本项目新规章的有效性需要实际模型输出和修订结果，不能用规则数量、JSON 字段完整或程序归一代替内容判断。

此前独立两阶段探针在同一 Q1 主稿上仍返回接受，没有改善已知遗漏，因此没有接入固定生产调用；见 [原回放报告](replay.md) 和 [探针结果](../../../tmp/tutor-three-turn-20261018/independent-review-probe/results.json)。以下是随后独立执行的新规则探针，与该阴性结果分开记录。

## 新规则的四个固定实际探针

2026-10-18 13:34:31–13:35:23 UTC，以原真实 DeepSeek 配置并行提交四次请求，每例仅一次，没有根据结果重采样。前三例直接采用 run-5 最终 Q1 的 `request-029.json`、最终 Q2 的 `request-043.json`、最终 Q3 的 `request-049.json`；保留原始 packet、已读原文、页面材料、历史审核／工作记忆和模型参数，只将 system 替换为冻结的新规章。没有向审核额外提供已知错误清单或期望答案。输入清单与具体请求见 [inputs.json](../../../tmp/tutor-three-turn-20261018/rules-review-probe/inputs.json)，实际使用规则见 [rules-used.md](../../../tmp/tutor-three-turn-20261018/rules-review-probe/rules-used.md)。

第四例是明确标注的异领域合成正确对照：使用既有 `teaching_design_tests.rs` 的完整平均速度原文 fixture、原问题 “So elapsed time includes the stop, right?” 和既有正确解释，说明出发到到达的总时间包含停车。没有页面或学习者缺陷诊断。其目的仅是检查新规则会不会无差别拒绝一个清楚正确的回答，见 [对照输入](../../../tmp/tutor-three-turn-20261018/rules-review-probe/correct-average-speed-source-request.json)。

本次是对三份既有最终稿的定向审核，不是新建课堂或完整三轮重放。探针没有修改应用、页面或学习记忆，也没有将审核修订意见反馈给主教师。因此“要求修订”只能证明审核提出了意见，不能证明主教师随后会正确补查和改稿。

| 固定病例 | 模型原始决定 | 当前请求处置 | 具体缺陷记录 | Rust 实际归一后的决定 |
| --- | --- | --- | --- | --- |
| Q1：在哪里用、怎么用，并给演示页 | `accept` | `answered` | 空 | `accept` |
| Q2：准备阶段粒度是否按 LID 树切分 | `revise` | `unresolved` | 一条 R1 | `revise` |
| Q3：确认进入向量的是预构建中间产物 | `accept` | `answered` | 空 | `accept` |
| 正确平均速度对照 | `accept` | `answered` | 空 | `accept` |

所有公开原始输出、解析结果、用量和结束状态均保留在 [results.json](../../../tmp/tutor-three-turn-20261018/rules-review-probe/results.json) 以及各例 `*-response.json` 中。没有保存模型内部推理文本或凭据。随后把这四份公开结果送入真实 Rust `normalize_delivery_review → validate_review_grounding → validate_review_work`，全部通过；归一结果见 [runtime-validation.json](../../../tmp/tutor-three-turn-20261018/rules-review-probe/runtime-validation.json)。本次没有出现“原始 accept 却同时记录 unresolved 或具体 finding”的矛盾组合，因此程序没有改变这四个原始决定。

### 本次实际发现了什么

**Q2 的必要问题终于被作为未解决事项阻止接受。** 新审核指出，正文讲了短投影、去重、重算和批量调用，却仍没有回答“是否按 LID 树切分”；不能仅以所读几段未提 LID 树就算完成。它给出的 R1 修订要求是补查 BookStructure／LID 定义及视图构造段，确认每章、每候选与逐节点之间的关系，再回答原问题。这比原 run-5 最终审核的接受更符合当前任务。公开 `revision` 和 `findings.required_change` 都写出了补查动作，见 [Q2 实际输出](../../../tmp/tutor-three-turn-20261018/rules-review-probe/q2-final-response.json)。尚未执行这项补查，不能把这个结果写成 Q2 已经改对；审核也没有证明“查不到”这一退路何时才真正成立。

**Q1 的必要机制仍然漏检。** 新审核接受的理由列举了 Core 准备阶段、文档与查询接口、检索视图、相似度候选合并、额度、截断、缓存和演示切换。这些是原稿确实包含的相关信息，但仍没有要求把具体查询文本与条目文本分别如何变成向量、两侧如何比较并得到候选排序这条关系讲通。它也没有指出页面“既不写答案，也不判断相关性”这一过强表达。Q1 的 `findings` 和 `open_questions` 均为空，见 [Q1 实际输出](../../../tmp/tutor-three-turn-20261018/rules-review-probe/q1-final-response.json)。这说明固定规则编号和显式请求处置没有自动补足该次语义判断。

**Q3 的归因和层次问题仍然漏检。** 审核认为正文已经确认短文本／投影，并认可“选料、截取、复用”的分段，因此标为已回答。它没有指出开头“不是改写”在纠正用户未表达的强说法，也没有指出“与原文的差别主要是短和指定字段”把“原文形成预构建字段”与“字段再取短投影”混在一起。返回 `learner_claims:[]` 只代表没有另记学习者判断，不能消除正文里的误纠正。该例保留了旧 Q2 的粒度未决事项，却仍接受 Q3；历史未决事项并未被无条件当作新的阻塞项，见 [Q3 实际输出](../../../tmp/tutor-three-turn-20261018/rules-review-probe/q3-final-response.json)。

**正确对照被接受。** 审核直接认可总时间包含停车的解释，未制造缺陷或学习者诊断，来源引用来自所给完整 fixture，见 [正确对照实际输出](../../../tmp/tutor-three-turn-20261018/rules-review-probe/correct-average-speed-response.json)。这一个对照没有出现无差别拒绝，但不足以证明规则对所有正确回答都不会误拒。

### 实际调用与用量

请求模型为 `deepseek-v4-flash`，响应记录返回模型名 `deepseek-flash`。四次均 HTTP 200、`finish_reason=stop`，SSE 收到完成标记，没有超时、截断或自动重试。

| 病例 | 输入 tokens | 输出 tokens | 总 tokens | 请求耗时 |
| --- | ---: | ---: | ---: | ---: |
| Q1 | 42,026 | 6,557 | 48,583 | 29.558 秒 |
| Q2 | 26,996 | 8,547 | 35,543 | 38.247 秒 |
| Q3 | 22,098 | 11,991 | 34,089 | 52.400 秒 |
| 正确对照 | 1,804 | 1,174 | 2,978 | 6.394 秒 |
| 合计 | 92,924 | 28,269 | 121,193 | 并行墙钟 52.439 秒 |

输入缓存命中 1,280 tokens，未命中 91,644 tokens；四次请求各自耗时相加为 126.599 秒，与并行墙钟不是同一指标。这些调用独立于 run-5 的正式 51 次调用和此前两阶段阴性探针，不合并成一次完整课堂的成本。

本次结果支持一个有限结论：新规章在这份固定 Q2 稿件上提出了必要补查和直接回答的修订要求，正确对照被接受；Q1 和 Q3 的已知实质问题仍被放行。不能据四例宣称通用成功率，也不能将新规则或程序合同称为已经解决“同能力审核能发现主教师全部错误”。

## 程序验证

相关验证共 49 项通过：审核处置合同 6 项、教学流程 35 项、工作记忆来源合同 5 项、持久工作记忆 2 项，以及读取上述四份真实输出的 Rust 校验 1 项。教学流程覆盖模型原报 accept 但标记 unresolved 时，在 host 选择前退回并保存 revise；后续修订稿接受后，只交付修订后的正文。记忆验证覆盖重开后仍可读到规则编号、原问题处置与实际修订变化，并保持学习者证据独立。

执行入口为 `cargo test -p runtime --lib review_resolution`、`cargo test -p runtime --lib teaching_`、`cargo test -p runtime --lib review_work`、`cargo test -p memory --lib agent_work`。真实输出校验通过 `TUTOR_REVIEW_PROBE_DIR` 指向上述产物目录，再运行 `cargo test -p runtime --lib recorded_reviews_follow_runtime_contract -- --ignored`；该测试只读取保存的公开输出，不请求模型。日志保存在同目录 `test-*.log`。

本次修改已进入源码并经上述运行时路径验证。运行中的演示服务仍是 run-5 的 v9 二进制，页面保留原三轮结果；本次未重启服务、未执行主教师改稿，也未重做页面交互。
