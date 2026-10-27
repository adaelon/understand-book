# Harness 缓存机制接入与验证，2026-10-25

本次将工具声明、活动模型历史、状态增量和压力压缩接入现有 Runtime／SessionLog。目标是减少已发送前缀的无必要变化和重复新增内容，同时保持执行权限、来源证据、审核和恢复语义。已于 2026-10-26 合并上线；发布时补齐 Messages 计价预估的续接格式支持，并完成真实两轮验证，见[合并发布记录](../combined-release-20261026/README.md)。

参考源码固定为官方 [DeepSeek Harness `22ddee3792402c3e70ec5f86a63c3e3e895a2f60`](https://github.com/deepseek-ai/deepseek-harness/tree/22ddee3792402c3e70ec5f86a63c3e3e895a2f60)，提交时间 2026-10-23。前置机制分析分别见[工具声明](../../../output/deepseek-harness-mechanisms-20261025/tools.md)、[窗口与压缩](../../../output/deepseek-harness-mechanisms-20261025/context.md)、[纠错与动态状态](../../../output/deepseek-harness-mechanisms-20261025/correction.md)。研究阶段的“尚未验证接口能力”结论由本报告的认证回执和三次真实探测更新。

## 修改前基线

原统计窗口为 2026-10-18 15:48:50 至 2026-10-25 15:48:50，Asia/Hong_Kong，LangSmith 项目 `understand-book-dev`。

| 口径 | 原始结果 |
|---|---:|
| 完整产品模型 usage | 1,042 条 |
| 输入 token | 164,947,470 |
| 缓存输入 token | 129,988,224 |
| 输入 token 加权命中率 | 78.8058% |
| 已有单次命中率 ≥99% 的调用 | 22 条 |
| 该窗口的模型压缩调用 | 未观察到 |

另有 32 条不完整模型 span，不能按零 token／费用处理。最近 24 小时“消息追加且工具／设置稳定”组为 90.57%，“消息追加但工具／设置变化”组为 16.58%；分组差异用于定位，不能直接作为改造收益。原始统计与相邻请求依据见[缓存分析](../../../output/langsmith-cache-20261025/cache-analysis.md)。

## 实际修改机制

### 模型声明与执行约束分开

`tutor::model_spec_for_context → ToolExposurePlan::build_with_model_specs` 在同一 `active / observing / reference` 权限模式中保持 schema、字段和例子稳定，并用实际模型声明计算 schema 预算。`previous_ref` 始终可见，首次设计省略，后续修订的真实前驱仍必须与最新设计一致；`target / capability / intent` 的条件通过当前 `agent.tutor_execution` 给出。

执行仍使用 `tutor::spec_for_context → validate_arguments → server teaching preflight`。当前动作禁止的字段、当前必填 target、kind／意图限制以及 operation 精确重试约束保持。稳定的模型词汇不构成额外执行权限。

### 工具定义在历史尾部激活

`NativeAdapter` 获取实际路线的 `/models` 能力；`deepseek_messages::capabilities_for_route` 仅为具备能力的路线选择 `/anthropic/v1/messages`。明确列出的模型能力优先；官方域名上的旧别名按官方别名合同和本次真实探测解析，第三方服务不继承官方别名能力。

`ToolDeclarationHistory` 保存首次 schema 与消息索引。初始工具立即声明；新增工具以 `defer_loading` 提交，在消息尾部的 `tool_addition / tool_reference` 位置激活。重复发现不重复注入。同名定义、模型／指令或旧消息前缀改变时重建序列；压缩切掉激活位置时，不留下无法解释的延迟声明。

当前官方路线为 `addition-only`：新增可尾部激活，撤回仍从声明集合删除，因此撤回是可预期的缓存边界。`in-history` 路线才使用保留声明加 removal 的方式。两种路线都由当次 `ToolExposurePlan` 验证可执行集合，历史声明不能恢复被撤回的权限。

Messages 适配同时处理文本、thinking 与签名／原块顺序、工具调用、图片、usage、结束原因和流式错误；供应商续接信息仅在匹配的模型与协议中回放。取消、输出重试、费用结算和审计仍通过现有包装链。

LangSmith 输入与 `RequestTracker` 使用归一化后实际发送的请求体，审核尾部诊断也进入观测。现有诊断字段同时识别 Messages 的直接图片、工具结果内图片、thinking／redacted_thinking，以及 tool_use 的 input／id／name 变化；保留无正文的计数和变化归因。

### 工作与纠错按记录追加

`refresh_tutor_working_fragments` 将原 `agent.working_state` 拆成 `agent.work.<turn_id>`、`agent.correction.<review_ref>` 和简短选择清单。记录由 current 转 previous 不换 key、不重发正文；一次修订仅追加变化记录。沿原最多 4 轮／8 条纠错范围过期，`active_keys` 明确列出仍有效的记录。

`ContextFragmentLedger` 首次采样总发布状态、指导和决策消息三类替换边界，空状态也清除恢复历史中的旧状态；后续相同内容不重复追加。压力清理会重新发布完整有效状态，避免保留了引用却丢失原正文。

`PreparedDeliveryReview` 冻结已投影的审核材料、图片选择与原指令。格式或引文失败时，`CompletionRequest.followup_user` 将实际错误稿及诊断作为原文字、图片说明和图片块之后的新 user 消息；Native Chat Completions 与 Messages 都保留原消息数组及字节，不把诊断插入图片之前。原审核结果不会被本地改写成通过。完整修复输入再次进行上下文准入，最多一次格式修复，超限不通过减少原图片来补救。

### 破坏前缀的回收集中到压力边界

`SavedAuthorCall / ActiveToolResultLedger` 的普通成功采样只记录“已读取”及真实候选后继关系。达到压力后，`retire_sampled_bodies` 一次回收可读回的正文、大参数和已被后继替代的 edits；未读取结果、最新编辑依据和工具调用／结果配对继续保留。音乐场景也沿同一动态增量与压力回收路径，不再每步退休全部快照。即使压力回收后无需生成语义 checkpoint，也同步清除 active 和 sampled 模型投影，避免随后取消时再次保存旧窗口。

`ContextTokenMeter` 使用成功请求的实际 input/output usage 校准相同路线、相同头部且消息连续追加的窗口，实际 input 已包括 cached token。已丢弃输出的重试汇总、无法回放的隐藏推理不建立窗口锚点；头部或历史变化回到完整估算。估算下界保持保守，原 `ActiveContextBudget` 高水位与输出预留未按 Harness 的数字直接替换。

### 摘要生成复用原请求前缀

`PreparedCompaction::bind_generation_prefix → prefix_generation_plan → complete_structured_in_context` 在字面历史可以对应、无临时图片且预算容纳时，复用原 system、工具声明和旧消息，只在尾部加入压缩指令、来源位置映射和证据白名单。`complete_structured_in_context` 是调用最外层 `adapter.chat` 的 crate helper，继续穿过 Observed、取消和预算包装链。生成内容仍为原 `CompactionDraft`，原来源覆盖、引用、pending effects、修复上限和安装校验继续执行。

长 turn 的 `completed_step_prefix` 可选择近期 token 尾部之前已闭合的工具步骤，平行工具批次整体选择；本轮用户要求、最新闭合步骤和未闭合批次保留原文。不能复用的历史或图片请求继续用已有 typed receipt／分层生成路径，不能把重建出来的内容宣称为原始暖前缀。

### 跨轮次持久化活动模型投影

`ActiveModelProjection::capture_request / finish → session_runtime::model_projection → ModelProjectionUpdated` 将实际已接纳的主对话文本模型输入与工具声明历史保存在私有会话日志，业务历史继续保存精简回执。独立兜底和审核请求不替代主对话投影。日志按已有前缀位置记录变化后缀；恢复时按明确的业务消息计数和本轮片段数量拼接，不靠用户问句文本寻找边界。

供应商、模型配置、原指令或 checkpoint 边界不一致时重建投影。图片继续是临时附件，不持久化为声称可以精确复原的多模态请求。模型输入恢复不恢复本轮来源读取、工具发现授权、演示候选句柄或可交付资格。

## 真实状态数据的代码路径测量

测试读取本地私有导出中的相邻调用 `01a171fa-ecb1-7df3-91ac-7c989c7dce90 → 01a171fb-cc17-7811-bf73-b3ddbf88f6c2`，提取两次实际工作状态。旧方案使用原单 key 构造，候选调用当前生产 `refresh_tutor_working_fragments`；两者共用真实 `ContextFragmentLedger`。私有正文未复制到测试或报告。

| 指标 | 实测 |
|---|---:|
| 旧方案追加的状态消息正文 | 32,450 字节 |
| 当前方案追加的状态消息正文 | 12,507 字节 |
| 减少 | 19,943 字节，61.4576% |
| 有效记录 | 6 条 |
| 没有重发的未变记录 | 4 条，原 JSON 合计 19,903 字节 |

以下断言均通过：旧请求前缀逐字一致；6 条有效记录与原状态精确一致；未变记录正文不重发；压力重建后 6 条记录全部存在。[聚合结果](../../../output/harness-cache-implementation-20261025/state-delta-measurement.json)、[执行日志](../../../output/harness-cache-implementation-20261025/state-delta-measurement.log)。

重放入口为 `cache_state_measurement_tests::actual_langsmith_working_state_delta_reuses_unchanged_records`，默认 ignored；`CACHE_STATE_PAIR_INPUT` 指向已有私有导出，`CACHE_STATE_MEASUREMENT_OUTPUT` 指向聚合 JSON。输出只含字节数、数量及布尔验证结果。此测量是 UTF-8 状态正文大小，不是供应商 token、缓存率、时延或费用测量。

## 官方接口能力与真实协议探测

认证 `/models` 回执中 `deepseek-flash` 声明 `system_prompt_update=in-history`、`tool_update=addition-only`；`deepseek-v4-pro` 为 `leading-only`。配置的 `deepseek-v4-flash` 未作为独立 ID 返回，本次按官方域名的别名合同，使用该实际配置名称执行三次 Messages 请求确认可达。[能力回执](../../../output/deepseek-harness-mechanisms-20261025/models-capabilities.json)、[完整探测请求与响应](../../../output/deepseek-harness-mechanisms-20261025/protocol-probe/results.json)。

| 次序 | 探测内容 | 输入 token | 缓存输入 | 输出 token |
|---|---|---:|---:|---:|
| 1 | 初始工具声明并调用 A | 295 | 0 | 39 |
| 2 | 保留历史，延迟声明并尾部激活 B | 427 | 256 | 39 |
| 3 | 回传 B 的结果，`tool_choice:none` 停用工具并结束 | 181 | 0 | 2 |

三次均成功；第二、三次携带工具更新 beta header。探测确认 Messages、工具结果续接、延迟工具激活及归一化 usage 的实际协议链路。第三次 `tool_choice:none` 停用工具，是工具可用性变化的冷边界，输入降至 181、cached 为 0；它不作为稳定前缀收益样本。三次很小的协议样本不证明业务任务平均命中率已经提高，也没有证明 99% 的持续命中率。

## 工程验证

Runtime 与 Server 编译通过，本次必要验证已完成。以下数量按最终通过的不同测试累计；全量或受影响集合执行后，仅对修正过测试基准的失败项定向复测，没有将一次失败日志记作全量通过。

| 范围 | 最终结果 | 执行依据 |
|---|---|---|
| Runtime library | 920 项通过；15 项默认忽略 | 全量执行 919 通过、1 失败、15 忽略；修正 Manim 测试按原始消息位置匹配压缩来源的基准后，该项定向复测通过，期间无 Runtime 产品代码变更 |
| Server 受影响集合 | 140 项通过；1 项手工 Linux LangSmith canary 忽略 | 集合执行 138 通过、2 失败、1 忽略；修正主对话／独立兜底请求对照及协议工具名基准后，两项定向复测通过，期间无 Server 产品代码变更 |
| 真实状态与接口 | 状态重放及三次官方 Messages 探测通过 | 状态重放保留原前缀、6 条有效记录及压力重建；真实接口覆盖延迟工具激活与结果续接 |

Runtime 日志：[全量执行](../../../output/harness-cache-implementation-20261025/runtime-suite-current.log)、[最后一项复测](../../../output/harness-cache-implementation-20261025/runtime-manim-verified.log)。Server 日志：[受影响集合](../../../output/harness-cache-implementation-20261025/server-affected-ready.log)、[最后两项复测](../../../output/harness-cache-implementation-20261025/server-final-two.log)、[最终编译](../../../output/harness-cache-implementation-20261025/server-compile-final-two.log)。Server 更早的广泛执行发现失败后已停止；最终验证覆盖 agent run、teaching coach、ADM6、session、model projection、service limits、observability 和 compaction checkpoint，不宣称整个 Server library 全量通过。

保存重试验证发现并修复了实际顺序问题：`run_admission` 先用 `SessionStore::settle` 确认上次结果不明的终态追加，再判断 `already_saved`，避免投影保存确认旧终态后继续追加仅允许 Pending 状态的进度。跨轮次权限测试还实际尝试调用前一轮已发现、当前未授权的 `memory_recall`，确认恢复模型投影后仍返回 `TOOL_NOT_EXPOSED`。

定向覆盖包括：同模式模型 schema 稳定但执行限制有效；新增工具只激活一次、撤回后不可执行、schema／压缩边界重建；同一请求的文本／工具／推理续接及流式结束；空状态恢复的三类清空边界；工作记录增量与压力重建；审核诊断尾部及图片预算；同前缀摘要、工具步骤闭合与来源校验；跨轮次投影在业务历史精简、重启和 checkpoint 变化时的恢复行为。

## 限制与后续度量

- 实现已于 2026-10-26 合并上线。三次协议探测不用于推算生产整体费用，也未证明达到 99%。历史 78.81% 与已有 22 次 ≥99% 调用均是修改前基线。
- 审核修复在同次审核内保留原文字与图片请求前缀；跨轮次精确模型投影只持久化没有临时图片的请求，不扩展为多模态历史持久化。
- `addition-only` 撤回、同名 schema 真正变化、模型／指令变化及语义压缩都是实际缓存边界。服务端缓存持久化、淘汰和请求新增内容量仍决定可获得的命中率。
- 原窗口没有模型压缩调用，不能报告历史压缩费用下降。上线后应按稳定工具、新增、撤回、同名变更与压缩分组，对同类任务统计 input/cached/output、实际费用、完成轮次和回答质量。
