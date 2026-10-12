# 历史答案质量复评

## task-v2：原文支持与引用定位

`task-v2` 从已保存的产品 `run.json` 只读准备新评分包；旧 `historical-v2` 与 `task-v1` 报告不改写。匿名来源保留高亮、前文、后文及 `complete/partial/unavailable`。Chunk 的实际交付整块是完整视图；旧 Resident 记录缺少 `resolved` 时只保留已有高亮并标为 `partial`，不从当前书库补造弹窗。

语义要求由评委直接给 `met/unmet/unknown`，数值精确要求由程序核对。来源要求拆为逐论断的 `source_support` 和 `source_location`；评委输出答案片段以及有效来源的 `source_id/part/quote`，part 为 highlight/before/after，quote 为指定部分内唯一的连续逐字原文。程序精确定位后计算 UTF-16 半开区间，供既有校验和报告使用；评委不再手填 start/end。缺失、歧义或自行填写位置的 basis 使评分无效；原始输出保存在 `raw_judgment`，历史数值区间记录保持可读且不改写。公共 R 参考只用于内容核验，不能作为交付来源。报告保留完整分母，并把内容错误、原文不支持、定位缺陷、评分未决分列，允许并存。

评分前检查所有有效 C 的高亮与前后文；公式在相邻后文也算已交付。支持与引用挂接分别判断：多个互补来源可共同建立支持，但答案仍须明确指向所需来源；一个来源已充分时不要求额外引用。总结只复述有来源的正文且承接清楚，可沿用引用；新增实质论断单独判断。全书否定不能由局部片段证明。`source-judgment-probes.mjs` 保存这几类受控诊断输入与开发预期，预期不会进入模型输入，也不等于人审校准批准。提示变更后须在新目录重新运行已有实际人审判例，旧协议校准结果不能直接批准新协议。

```powershell
node evals/semantic/quality-run.mjs prepare --protocol task-v2 --run evals/semantic/results/2026-09-25-ev4-current-v1/run.json --out evals/semantic/quality-results/my-task-v2
node evals/semantic/quality-run.mjs calibration-review --protocol task-v2 --out evals/semantic/quality-results/my-task-v2
```

私有目录中的 `calibration-review.md` 可读地列出判例、实际答案、交付来源与公共参考。`calibration.json` 的建议标签仍是候选：绝对判例记录实际人审的 `human`、`human_support`、`human_location`、`basis`；偏好判例记录 `human`、`basis`，再填写审核人、评委配置和适用范围。人审可直接在聊天中确认，由 Agent 按原话回执写入文件；有疑问的判例保持待审。`task-v1` 的批准不能继承。经人审和新协议校准后，在**新的**准备目录以 `judge --calibration-from <校准目录>` 评分；未获准的范围保持未决。校准与正式评分命令的模型、用途和范围门禁沿用下文。

task-v2 的审核规则可用 `case_ids` 明确选定已审核判例；省略时仍要求所申请范围内的全部判例完成审核。选择项必须存在、属于对应用途与范围；选中的空标签会阻断批准。`scope_results` 分别记录每个 family × 层级 × 交互范围的分歧和失败，仅通过的范围可进入正式评分。配对偏好独立批准。

```powershell
node evals/semantic/quality-run.mjs compare --protocol task-v2 --out evals/semantic/quality-results/my-task-v2 --old-report evals/semantic/quality-results/2026-09-25-ev4-current-v1-formal/report.json
node evals/semantic/task-v2-diagnostics.mjs --bundle evals/semantic/quality-results/my-task-v2/bundle.json --book .understand-book/quantification-essence --ev5 evals/semantic/quality-results/2026-09-25-ev5-target-l2-v3/ev5-run.json --ev6 evals/semantic/quality-results/2026-09-25-ev5-ev6-target-l2-v1/ev6-run.json --out evals/semantic/quality-results/my-v2-diagnostics
```

`compare` 对齐旧新 18 个产品样本并输出逐要求依据。诊断准备仅保存 EV5 一份与 EV6 两份已存在答案的新协议材料，不调用产品或评委；须经相同 v2 校准范围批准后才可形成正式诊断判定。

## EV5–EV6 诊断

当前 `target-l2` 的充分原文与共同回答器最小探针使用独立输出目录：

```powershell
node evals/semantic/diagnostic-run.mjs --out evals/semantic/quality-results/my-diagnostic
```

命令只读首轮 `run.json` 与正式评分包，EV5 在隔离 Reader 中经真实 `book.search_text` 和 `book.text` 预供原文，EV6 从自然运行的实际模型请求提取 canonical 原文交给共同回答器。前两次 EV5 工具选择由诊断脚本给出，后续为真实模型；产品、诊断、Judge 用量分开。输出包含原文和请求，必须留在 Git 忽略的 `quality-results`。`--ev5-only` 只运行充分原文探针；输入可用 `--run`、`--formal`、`--book` 指定。

评委事实槽若与其准确、完整、来源维度互相冲突，诊断标记评分无效，不把自由文本与冻结枚举不一致算作产品失败。对已保存的原始评委输出用 `diagnostic-reconcile.mjs --ev5 <目录> --ev6 <目录> --out <新文件>` 作无新模型调用的复核；旧报告保留。首个探针、实际结果与限制见 [EV5–EV6 记录](../../docs/performance/agent-eval-ev5-ev6-20260925.md)。

## EV3–EV4 校准与新题集

新题集为两个证据链 family 的 L1–L4 共 8 道单轮题及两个同聊天多轮变体。`--reading-protocol` 通过真实产品入口执行；`--validate` 只核对书源证据、题目身份与划分，不调用模型。私有审阅包含原文和候选答案，必须留在 Git 忽略的 `quality-results` 目录。

```powershell
node evals/semantic/agent-run.mjs --validate --reading-protocol
node evals/semantic/reading-review.mjs --out evals/semantic/quality-results/reading-review-v1.json
node evals/semantic/agent-run.mjs --reading-protocol --out evals/semantic/results/reading-v1-run
node evals/semantic/quality-run.mjs prepare --protocol task-v1 --run evals/semantic/results/reading-v1-run/run.json --out evals/semantic/quality-results/reading-v1-evaluation
```

`prepare` 同时生成冻结的 `calibration.json` 与 `calibration-jobs.json`。旧 EV1–EV2 历史包可用 `calibration-prepare` 补建，但不能覆盖已有包。`calibration.json` 中申请批准范围的每案 `human`、`basis` 由独立人审填写；绝对评分用 `pass/fail/unknown`，偏好用 `A/B/tie/unresolved`。审核人还需填写 `reviewer`、`reviewed_at`、`judge_model`，将 `status` 改为 `human_reviewed`，并为 `rules.absolute`、`rules.preference` 各填写 `minimum_cases`、`allowed_disagreements`、`scopes`。每个 scope 明确填写 `family_id`、`reading_level`、`interaction_condition`；允许范围必须有相同三元组合的实际判例。未申请范围可以保持候选。关键错误误放一律阻断绝对评分，不被普通分歧预算抵消。

```powershell
node evals/semantic/quality-run.mjs judge --protocol task-v1 --out evals/semantic/quality-results/reading-v1-evaluation --calibration --provider existing
node evals/semantic/quality-run.mjs calibrate --protocol task-v1 --out evals/semantic/quality-results/reading-v1-evaluation
```

正式评分使用另一份完整产品运行和**新的** `prepare` 目录，并以 `judge --calibration-from <已通过校准的目录>` 调用。程序核对相同评委配置、协议、获准用途与任务范围；未获准范围不运行评分。两种用途独立批准；未获准的绝对评分不妨碍已获准的偏好比较。历史探索性作业不得混入正式目录。2026-09-25 两份校准已通过，首轮当前版本正式评分见[EV3–EV4 记录](../../docs/performance/agent-eval-ev3-ev4-20260925.md)。

## EV1–EV2 显式任务协议

新协议由 `task-spec.mjs` 冻结 32 道旧题的必要要求、阅读层级与交互条件。`quality-run.mjs --protocol task-v1` 从历史或新协议运行只读准备独立目录；绝对判定按答案请求，配对偏好另作双顺序请求。产品交付、逐项任务结果、评分有效性和成本分列；旧 `historical-v2` 命令与报告保持原样。EV3 人工校准完成前只执行探索性判分。

```powershell
node evals/semantic/quality-run.mjs prepare --protocol task-v1 --out evals/semantic/quality-results/my-task-run
node evals/semantic/quality-run.mjs judge --protocol task-v1 --out evals/semantic/quality-results/my-task-run --provider existing --exploratory --only T15
node evals/semantic/quality-run.mjs report --protocol task-v1 --out evals/semantic/quality-results/my-task-run
```

新协议默认仍读取冻结 LA7 `run.json`；`prepare` 可用 `--run`、`--book` 指向同源运行与书库。输出含原文和答案，留在 Git 忽略目录。当前实施与三案探索性差异见[EV1–EV2 记录](../../docs/performance/agent-eval-ev1-ev2-20260925.md)。

## 范围与状态

用户已确认第一刀：复用 LA7 历史问答，准备匿名样本、质量标准、校准材料、独立评委与可复核报告。本轮不改变被测 Agent；原始 run.json 和旧报告只读。工作目录中已有其他开发改动，本切片只维护评测文件及自己的工作记录。

- [ ] 匿名样本与完整执行分母
- [ ] 质量标准与人工校准材料
- [ ] 独立评委、交换顺序与错误记录
- [ ] 逐题质量、旧分数差异与报告
- [ ] 定向测试与 LA7 实际复评

## 决策：完成、质量与判分状态分列

**决策**：保留所有任务，独立记录交付与判分。

**否决**：
- 删除未完成任务后发布整体胜率：只代表幸存答案。
- 把评委失败当作产品错误：会把判分故障归因给 Agent。
- 覆盖旧成绩：无法解释结果变化。

## 质量标准

四个维度为内容准确、任务完整、证据充分、解释有效。每项使用 met / partial / failed / unknown；不适用的解释维度使用 not_applicable。问题仅要求分类、数值或定位时，简短正确答案可以充分完成任务；不按篇幅、标题数量、工具数量加分。解释题需要回答原因、条件或明确要求的联系，不能因回答更长就判优。

评委同时抽取必要事实；程序核对冻结金标准、选中的真实答案片段和已交付的有效引用。引用存在性不能替代语义支持，语义支持仍是模型判断。额外的实质性错误也纳入准确性，不能只答对事实槽就放过。

拒绝题依据冻结题集的不可回答约定评价，不因没有引用扣分；参考摘录不足以核验额外说法时用 unknown，不将“摘录未覆盖”等同于“全书不存在”。普通可回答题的引用支持只允许使用该答案实际交付的来源，公共参考材料只用于核验，不能替答案补引用。

## 评委材料与执行

输入仅包括问题、任务标准、人工可读的参考原文与事实要求、匿名答案片段和匿名来源。旧分数、系统名、工具轨迹、请求次数、成本、来源 ID 前缀不进入评委输入。引用标记重编号，其余答案文字保持。匿名映射与旧成绩单独保留在本地审计包。

双方交付的题目交换 A/B 顺序各评一次；单方交付的题目评价已有答案，另一方保留产品失败。配对优劣只表示双方已交付子集，须同时报告覆盖题数和完整任务分母。顺序相反时的优劣或通过状态冲突列为待复核，不用平均分掩盖。

独立配置使用 QUALITY_JUDGE_BASE_URL、QUALITY_JUDGE_API_KEY、QUALITY_JUDGE_MODEL；只有显式选择 existing 配置时才复制既有运行配置。评委费用与历史产品成本分开，缺失 usage 为未知，传输/格式失败保留，不自动反复重试。

## 校准与结论边界

校准包保留真实答案、原文及历史已知问题；建议标签是 Agent 起草，不能称为人工金标。人工审核必须填写 reviewer 与确认的判定；空白标签不自动批准。未经人工校准的模型复评只输出探索性报告，不能据其宣称架构胜出。正式质量结论需要先完成校准，再使用固定协议重评。

## 验证目的

- 匿名输入测试检测系统名、旧分数、原始来源 ID 泄露；失败则修导出。
- 分母测试检测未完成任务或评委失败被丢弃；失败则修汇总。
- 引用/片段测试检测评委虚构编号被接受；失败则修确定性核对。
- 顺序测试检测位置偏差被误记为胜出；失败则修配对聚合。
- 实际 LA7 导出检测语料不匹配、缺题或来源映射丢失；失败则停止模型调用并修材料。

生成物包含原书摘录与回答，仅存放在 Git 忽略的 quality-results 目录。

## 运行

```powershell
# 新目录；只读取历史运行与指定书库，不调用模型。
pnpm eval:quality prepare --out evals/semantic/quality-results/la7-quality-new

# 先跑校准候选的模型试评。existing 显式使用已有模型服务；默认 quality 使用三个独立变量。
pnpm eval:quality judge --out evals/semantic/quality-results/la7-quality-new --provider existing --exploratory --only Q01,Q05,Q08,Q10,Q15,Q17 --concurrency 2

# 按原始答案和原文独立完成 calibration.json，再检测分歧。
pnpm eval:quality calibrate --out evals/semantic/quality-results/la7-quality-new

# 探索性全量复评会跳过已有记录，不自动重试失败请求。
pnpm eval:quality judge --out evals/semantic/quality-results/la7-quality-new --provider existing --exploratory --concurrency 2
pnpm eval:quality report --out evals/semantic/quality-results/la7-quality-new
pnpm eval:quality:test
```

prepare 默认采用 LA7 与 quantification-essence，也可传 --run / --book。此版只接受完整的冻结24题双系统运行。来源归一化只替换引用编号，不改变其他答句；引用是否属于已交付来源与是否为规范原文分别核对。完整历史分母48个系统样本，双答案34次请求和单答案7次请求，共41次，不重跑产品。

人工审核时，逐题将 judgments 填为以下形状（值须由实际审核决定）：

```json
{
  "A": {"facts": {"purpose": "absolute_return"}, "dimensions": {"accuracy": "met", "completeness": "met", "grounding": "met", "explanation": "not_applicable"}},
  "B": {"facts": {"purpose": "absolute_return"}, "dimensions": {"accuracy": "met", "completeness": "met", "grounding": "met", "explanation": "not_applicable"}},
  "preference": "tie"
}
```

单候选只有 A，preference=not_applicable。未回答的事实值填 null。最后将顶层 status 改为 human_reviewed，填写 reviewer/reviewed_at。calibrate 报告两次顺序下事实、维度和偏好的逐项分歧；未审核不会自动批准。分歧须人工判断来自评委、标准或标签，修订协议时另开版本，不能反复调用直到碰巧通过。

正式运行必须使用新的 prepare 目录，并传 `judge --calibration-from <已通过的试评目录>`。运行器核对相同评委配置与协议，记录校准依据，拒绝将原探索性结果直接改名为正式结果。正式评判是通过小型校准集的模型判断，不等于每份答案均经人工审核。

## task-v2 的材料不足与迁移诊断（CQ6）

内容错误需要可核验的矛盾、错误计算或无效推导。交付引用缺少某句话只能影响来源判定，不能据此断言全书没有该说法；内容核验材料本身不足时记 unknown。相关引文缺少必要比较量、条件或中间前提时，支持保持 unknown，即使来源视图已完整保存。来源明确反驳、指向不相干内容或未交付必要引用仍可判 unmet。已知依据仅在前后文而高亮只给词时，支持与定位分别判定。

情境迁移允许将已交付的一般原则应用到题设，不要求原书存在同名团队或同一天数；新增具体参数与实质性额外断言仍须核验。CQ6 的三个 L4 控制判例分别检验正确映射、错误替代、以及只改变高亮的交付缺陷，初始 human 字段均为空。建议标签经实际人审后才能进入批准规则。

修订后的提示全文冻结在新的 `protocol.json`，原目录不变；正式运行已有全文一致性检查会拒绝混用旧协议批准。先重新校准，再对 CQ4/CQ5 保存的六份答案作同规则绝对评分，保留全部未决与失败。不重跑产品挑选答案，不将绝对评分差异写成未经校准的配对偏好。
