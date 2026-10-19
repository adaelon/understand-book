# 主教师与审核共用的技术讲解写作规约

后续已用同一冻结规则完成第三轮原稿、改写稿的独立盲审与审核驱动修订，见 [第三轮两稿盲审与审核驱动修订](blind-q3-review.md)。该实验修正了开场表达，但保留了一处来源不能支持的比较，复审仍然接受；不能据此认为核心问题已解决。

2026-10-18。新增 [tutor_writing_rules.md](../../../crates/runtime/src/tutor_writing_rules.md)，版本 `plain-explanation.v1`。教学正文、页面解释与审核意见采用同一份文本。规则禁止用标签对立开场定调或代替解释，要求稳定命名、具体动作、清楚指代和信息保真。

## 来源与采用范围

ASD-STE100 起源于 1986 年的指南；当前 Issue 9 发布于 2025-01-15。官方介绍列出 53 条规则及约 900 个获准通用词，并允许领域技术名词、技术动词。我们借鉴固定术语、直接动作和清楚句法，没有把英语词典或 20／25 个英文词的句长限制转成中文字数限制。[官方介绍](https://www.asd-ste100.org/about_STE.html)、[官方 FAQ](https://www.asd-ste100.org/STE_faq.html)。

视频作者的实验指标是启发式检查器发现的违规数／100 个英文词。六类英语文案的数据不能证明中文教学的事实正确、关系完整或学习效果。其 GPT-5.5 结果中 Orwell 为 1.69、STE 为 1.76，禁词表也有改善；不能据此声称 STE 在所有模型上最好，或禁词完全无效。[作者实验说明](https://www.chele.bi/videos/the-cure-for-ai-slop)。

本项目规约是中文教学的自行适配，不是 STE 认证工具。禁止无依据的标签对立和纠正用户未说的主张，来自本项目的用户要求与教学边界。没有安装作者插件、改用户全局配置或引入英文评分器。

## 执行方式

- 同一对象用同一名称；引入专业词要说明所指，区分不同处理阶段。
- 用具体主语和动词说明对象怎样产生结果；术语、阶段、比喻与标签不代替操作。
- 禁止“更像 X，不是 Y”等标签对立开场或代替解释。确需比较时说明各自作用、条件及差别，只纠正用户实际表达的主张。
- 每句一个主要判断、每段一个关系；拆句时保留因果、条件和时间连接。
- 删除无事实内容的评价和套话，保留不确定性、范围、例外与数值。
- 按整段是否回答用户所问关系来修订，不以禁词计数或风格分数宣布合格。

`tutor::instruction_modules` 在 active、observing、reference 状态下注入一次 `resident-agent.tutor.writing`，关闭或暂停时不注入。`tutor::review_instructions()` 将审核规则 v12、两个换行和共享写作规约组合一次；预算计算与 provider 请求共用该完整字符串。沿用已有关系判定、交付引文校验及纠错记忆，没有新增评分或第二套记忆。

## 一次重写与一次独立审核

使用与前几次相同的 `deepseek-v4-flash` 配置。作者输入包含 Q3 原问题、原已读的四段完整来源、来源引用映射和明确标为待重写草稿的原正文；没有加入外部评判、标准答案或旧审核意见。作者 system 为共享规约加通用改写与 JSON 输出合同。

作者仅调用一次。随后复制原 Q3 审核包，只替换 `proposed_delivery.text` 及唯一 move 的 `delivery_text`，其余来源、历史、原设计和原计划保持不变。审核使用与生产完全一致的 v12＋共享规约，仅调用一次。没有重采样、应用写入或给审核补充人工诊断。

新稿开头：

> 对。进入向量计算的是准备阶段生成的短文本投影；完整词法字段和原材料位置另外保留。你说的“中间产物”在这里对应两类检索视图：章节视图取问题与概览，候选视图取含义、条件和别名。

随后明确写“投影会从这些字段里再截取开头”，再解释截取长度、信息损失与复用条件。原稿中的“更像投影，不是改写”与“和原文的差别主要在短和只留指定字段”均被移除。完整结果见 [rewritten-answer.md](../../../tmp/tutor-three-turn-20261018/plain-writing-probe/rewritten-answer.md)。

独立审核返回 `accept / answered`、`findings:[]`，引用新稿五句，认为准备阶段、检索视图字段与短投影的对应关系已说明。完整公开结果见 [review-response.json](../../../tmp/tutor-three-turn-20261018/plain-writing-probe/review-response.json)。

| 阶段 | 输入 tokens | 输出 tokens | 合计 | 请求耗时 |
| --- | ---: | ---: | ---: | ---: |
| 作者 | 2,209 | 1,672 | 3,881 | 8.868 秒 |
| 审核 | 22,689 | 8,121 | 30,810 | 35.797 秒 |
| 合计 | 24,898 | 9,793 | 34,691 | 请求耗时相加 44.665 秒 |

两次均 HTTP 200、`finish_reason=stop`。请求、公开响应、冻结的两份 system 和实际修改路径保存在 [plain-writing-probe](../../../tmp/tutor-three-turn-20261018/plain-writing-probe/)；内部推理文本与凭据不保存。

## 程序验证

38 项教学路径测试通过，包括主教师实际请求只注入一次共享全文，以及审核实际请求与预算使用统一文本。另有 1 项保存输出验证通过：核对真实审核 system 与生产组合完全一致，然后执行 `normalize_delivery_review → validate_review_grounding → validate_review_work`，结果有效且仍为 accept。见 [runtime-validation.json](../../../tmp/tutor-three-turn-20261018/plain-writing-probe/runtime-validation.json) 和同目录 `test-*.log`。

测试命令为 `cargo test -p runtime --lib teaching_`。保存结果验证使用 `TUTOR_REVIEW_PROBE_DIR` 指向本次目录，再执行 `cargo test -p runtime --lib recorded_reviews_follow_runtime_contract -- --ignored`。响应文件中的 `verdict` 是对未改动 `public_output` 的 JSON 解析副本，供现有验证入口读取。

## 已知限制

新稿仍混用“含义／meaning”“查看／inspect”等名称，审核没有提出这项写作问题。原来源引用标记也没有保留；本次作者是抽离的正文改写调用，没有完整主教师工具与来源交付流程，因此新稿不能直接当作应用内已验收成品。

这一例支持“共享规约下能够移除指定的空泛对立，并把字段与截取关系表达得更直接”，不能证明所有写作规则均被遵守，也不能证明事实或层次问题已普遍解决。程序核验保证规则接入与引文出处，关系和写作语义仍由模型判断。

本次没有重新生成演示页、完整回放三轮或重启运行中的演示服务。此前 v11 的两例失败结果保留在 [关系审核报告](relation-review.md)。
