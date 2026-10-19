# Tutor Agent 记忆：原三问隔离回放

2026-10-18。**run-5 原三问均 completed/saved、incomplete=false，全部 51 次模型请求及后台审阅已经结束。结果是部分改善：审核实际纠正了额外自测要求和若干错误论证，工作与纠错记录真实进入后续轮次；它仍未自动发现并改正全部问题。** Q1 的向量比较连接和过强表述仍在，Q2 用保守边界代替必要回答，Q3 又把未表达的“改写”强说法带入解释。独立构建、定向测试与页面实际交互通过，不等于内容全部正确。本记录独立于前四次实验；原记录与失败结果保留在 [此前回放报告](../tutor-three-turn-20261018/replay.md)。

## 本轮目的与判定

本轮使用新构建、全新会话和原始三问，观察当前 Tutor Agent 记忆机制能否在没有人工纠错意见的情况下，保留当前学习者的问题、定位解释中的缺口，并在后续回答中自行纠正。实际观察到的收益与仍未解决的问题分别记录，不把单次回放当作成功率或机制因果对照。

结果按实际交付判断：第一问是否从具体输入到向量、相似度排序、候选和来源建立用途；第二问是否解释章节／候选条目及短字段投影，与 LID 定位树区分；第三问是否准确确认预构建派生输入，未把学习者没有提出的强说法当成错误纠正。页面同时需要真实交互与非默认状态恢复证据。

“自发纠错”需要实际调用和公开输出支持：说明哪个环节先发现了什么问题，后续哪些内容因此改变。仅有新记忆记录、审阅 accept、运行 completed 或页面可打开，均不能代替这项判断。若连续三问中没有出现可观察的错误与修正，只能记录未观察到纠错，不能据此推定机制有效或无效。

## 已实现的路径

Agent 自己的工作与纠错历史复用既有 `teaching_trace`，新增 `AgentWorkReviewed` 事件。每条事件保存本次原问题、当前设计、拟交付文本与页面、实际已读来源、完整有效审阅结果及相对上次的内容变化；完整记录可通过既有 `tutor.step trace` 和精确 `review_ref` 读取。没有另建记忆数据库或另起后台教练。实现见 [memory/agent_work.rs](../../../crates/memory/src/agent_work.rs) 的 `append_agent_work_review`、[memory/teaching.rs](../../../crates/memory/src/teaching.rs) 的 `TeachingFact::AgentWorkReviewed` 与 [server/teaching.rs](../../../crates/server/src/teaching.rs) 的 `record_agent_review`、`trace_summary`。

持久事件投影成独立的 `agent_working_state`，包含 `current_work`、`previous_work`、`corrections`。它保留有来源的理解、真实未决工作、修订意见、后续内容变化及实际交付引用；投影按工作区、Tutor 会话和来源版本隔离，最近四轮形成有界上下文，完整事件仍可按引用读取。该状态不进入学习者画像或学习者表现证据。`accepted_by_review` 只表示当次审阅接受，实际是否交付由 `delivered_refs` 单独表达。实现见 [memory/agent_work.rs](../../../crates/memory/src/agent_work.rs) 的 `agent_working_state`。

交付审核现在接收完整的实际已读 `segments`，保留各段的来源与 LID，也保留实际的部分读取范围；搜索命中或短引用预览不再替代原文证据。审核仍基于已提供材料，不会自动获得整本书。新增 `agent_work.understanding` 记录的每条依据必须同时匹配 `source_id`、`source_revision`、`lid`，且 `quote` 是该实际已读段落的精确子串；这验证依据的来源，语义是否支持结论仍由审核判断。实现见 [orchestrator.rs](../../../crates/runtime/src/orchestrator.rs) 的 `teaching_review_sources`／`append_teaching_review_source` 与 [tutor.rs](../../../crates/runtime/src/tutor.rs) 的 `validate_review_work`、`review_delivery`。

每次审阅先完成学习者归因与 Agent 工作来源验证，再将有效的 accept 或 revise 结果持久化，然后才继续接受交付或把修订意见返回主 Agent。同一轮立即刷新工作状态和采样上下文，下一轮从绑定的私有作用域加载；resume 入口也重新加载持久状态，避免旧 admission snapshot 覆盖后续纠错。实现见 [orchestrator.rs](../../../crates/runtime/src/orchestrator.rs) 的审阅处理、`refresh_tutor_working_fragments` 和 `run_context_inner`，以及 [server/teaching.rs](../../../crates/server/src/teaching.rs)、[server/agent_run.rs](../../../crates/server/src/agent_run.rs)、[server/workspace_registry.rs](../../../crates/server/src/workspace_registry.rs) 的宿主接线。

## 当前验证与构建状态

下表记录各执行者已返回的定向验证结果，没有把尚在执行的构建或真实回放记作通过。

| 范围 | 已得到的结果 | 具体覆盖 |
| --- | --- | --- |
| memory `agent_work` | 2 项通过 | 重开后保留修订与真实变化、精确重试复用记录、恢复轮次不覆盖历史，以及工作区／会话／来源版本隔离。 |
| runtime `teaching_` | 首轮 34 项通过、1 项旧断言失败；修正预期后该项针对重跑通过 | 旧断言仍要求已接受 presentation 设计后进入 Global；当前既有流程直接承接设计进入 Local，故更新预期，不修改产品流程来满足旧断言。见 [针对重跑日志](../../../tmp/tutor-three-turn-20261018/run-5/authoring-phase-test.log)。 |
| runtime `review_work` | 5 项通过 | 有据结论与未决事项、伪造引文、段落匹配、部分读取边界、跨书／跨版本 LID 重合，以及格式和学习者字段隔离。 |
| server 跨轮与重启 | 1 项通过 | 审阅事件重开进入下一轮；Agent 工作状态不进入 learner_context，保留交付引用，trace 可读取完整原始记录。 |
| provider recorder | 8 项通过 | 公开 SSE／JSON 输出和工具调用、用量、完成状态、凭据与 reasoning 不落盘及既有代理行为。 |

根代理通过 `cargo rustc -p server --bin server -- -o .../run-5/bin/server.exe` 完成实验专用二进制构建，耗时 56.09 秒，保留了旧 1783 服务。独立二进制于 12:13:51.893 UTC 写入，大小 62,627,328 字节；新服务于 12:14:04.821 UTC 就绪，URL 为 `http://127.0.0.1:10617`，PID 63552。证据：[构建日志](../../../tmp/tutor-three-turn-20261018/run-5/build.log)、[实际配置](../../../tmp/tutor-three-turn-20261018/run-5/configuration.json)、[连接信息](../../../tmp/tutor-three-turn-20261018/run-5/connection.json)。

构建后未再修改生产代码；本轮相关的 13 个实现与测试源文件已另存 [source-snapshot](../../../tmp/tutor-three-turn-20261018/run-5/source-snapshot/)，包含实际未提交变更，供复核本次构建条件。

## 输入与隔离

新目录为 [run-5](../../../tmp/tutor-three-turn-20261018/run-5/)，准备信息见 [plan.json](../../../tmp/tutor-three-turn-20261018/run-5/plan.json)。启动时不使用 `--resume`，由现有脚本新建 chat 与 Tutor，会话记忆写入 `run-5/memory`，私有产物写入 `run-5/private`。

仅从原始历史材料提取问题、anchor 与 quote。此前回答、旧页面、跨会话记忆、四处纠错反馈、repair 输入和 review probe 均不导入。原来确认的偏好和“理解第六章”目标保留，初始 Reader 位置仍为 `10.9.8.5`。第一问的费用相关选区属于原始输入，保持不变。

| 轮次 | 问题 | 输入条件 |
| --- | --- | --- |
| 1 | 那Embedding在哪里会被用到呢，怎么用呢，给我个演示页吧 | anchor `10.9.8.2`，原选区“Embedding 会产生计算、等待与可能的外部费用。”，范围 `0..27`。 |
| 2 | 准备阶段embedding的粒度是怎么切分的呢，是根据lid树来切分的吗 | anchor `11.3`；使用本次新页面实际操作并保存后的 receipt。 |
| 3 | 啊，那进入embedding的内容不是原文而是预构建的中间产物对吗 | anchor `11.3`；不额外附页面 receipt。 |

三问依次运行，同一 run 内前一问的新回答自然进入下一问上下文。不通过页面反馈按钮、修订脚本或模型请求注入已知错误清单。实际页面操作仅产生正常保存状态；发生缺陷时先保存证据，完整基线结束前不修订该页。

## 启动与收集

由根代理统一启动。准备时确认本地 `.env` 配置为 `deepseek-v4-flash`、`https://api.deepseek.com`，凭据存在但不记录。本轮通过 `SEMANTIC_EVAL_SERVER` 指向构建完成后的 run-5 独立二进制，并继续使用独立 `tmp/tutor-three-turn-20261018/web-dist`。启动器不会自动构建；未设置覆盖时会使用 `target/debug/server.exe`。`configuration.json` 会记录实际启动的二进制路径、修改时间和大小，以及 Git HEAD。共享工作区的未提交实现差异已列出对应路径，不能只以 HEAD 认定版本相同。

```powershell
$env:SEMANTIC_EVAL_SERVER = Join-Path (Get-Location) 'tmp/tutor-three-turn-20261018/run-5/bin/server.exe'
node tmp/tutor-three-turn-20261018/start-replay.mjs tmp/tutor-three-turn-20261018/run-5
```

启动脚本持续运行。服务器自动选择空闲端口，真实 UI URL 以新 `connection.json` 为准；旧 `http://127.0.0.1:1783` 保留的是 run-4 最终成品，不用于新实验。另一个执行会话中依次运行：

```powershell
node tmp/tutor-three-turn-20261018/run-turn.mjs tmp/tutor-three-turn-20261018/run-5 1
```

第一问完成后，在新 URL 的真实 Reader 打开并操作演示，保存默认与变化状态、刷新恢复及来源打开证据。取新 run 的 `memory/agent-history.presentations/states/<presentation_id>/<revision>/<state_revision>.json` 中实际 `.receipt`，保存为 `turn-02-page-receipt.json`，再提交：

```powershell
node tmp/tutor-three-turn-20261018/run-turn.mjs tmp/tutor-three-turn-20261018/run-5 2 tmp/tutor-three-turn-20261018/run-5/turn-02-page-receipt.json
node tmp/tutor-three-turn-20261018/run-turn.mjs tmp/tutor-three-turn-20261018/run-5 3
```

观察脚本超时不重发请求，使用 `capture-turn.mjs <run目录> <轮次> <已接受turn_id>` 回接。保存每轮 input、accepted、run、result、answer、history；完整调用记录在 `provider/request-*.json`，摘要在 `provider-progress.json`，持久会话事件与教学／读者记忆留在独立 memory 中，页面版本和真实状态也一并保留。

每轮完成后区分执行状态与内容判断，并跟踪调用记录里的后台记忆审阅。三问结束且最后的后台审阅已经返回后，再对本次目录写 `STOP`；若保留页面供交互，则保留服务并明确记录活动端口。不要停止仍在记录请求的服务来节省等待。

## 记录器修复与验证

此前记录器只把模型响应当 JSON 解析，遇到 SSE 时仅保存 `non_json_response=true`，因此丢失公开输出与 usage。本轮在现有 [provider-recorder.mjs](../../../evals/semantic/provider-recorder.mjs) 补上 SSE 事件解析，拼接公开 `content` 和分片 `tool_calls`，保留返回 model、usage、finish_reason、`stream_done` 与 `response_complete`。不保存响应 reasoning；请求副本也剔除 `reasoning_content`。HTTP 凭据不落盘，请求／响应原始字节仍原样转发。

验证仅针对这一具体记录故障及既有行为：新增完整 SSE 与未完成 SSE 两项本地 HTTP 回归，修改前两项均失败；修复后执行 `node --test evals/semantic/provider-recorder.test.mjs`，**8 项通过、0 失败**。覆盖公开中文内容、分片工具参数、usage、完成标记、截断流、无 key/reasoning 落盘，以及既有 JSON、额度、错误响应、drain、intercept 行为。本次验证没有真实模型请求。

旧 run 已漏记的响应不补造；旧实验的响应和 token 总量仍保持原有限制。新记录的 `purpose=unclassified` 表示分类器未命名，不代表该调用未记录。分析完整审核链时应读取实际 system／请求包和公开返回，不只按 purpose 筛选。

## 运行结果

正式回放共 51 次 Provider 请求：42 次主 Agent 调用、7 次交付审核调用、2 次后台 profile review。全部 HTTP 200，全部响应已结束并取得 usage；其中一次交付审核以 `length` 结束、无有效公开结果，另外六次形成有效审核，合计 **3 次 revise、3 次 accept**。对应持久 Agent 工作事件也是六条。第 51 次后台画像审阅已在约 13:13:23 UTC 完成，不能只以主回答完成时的 50 次计数作为最终总量。

| 阶段 | Provider 请求 | token 总量 | 首次请求至最后响应 |
| --- | --- | ---: | ---: |
| Q1 | 001–030，30 次 | 3,246,093 | 8 分 44.56 秒 |
| Q2 | 032–044，13 次 | 2,055,201 | 4 分 51.77 秒 |
| Q3 | 045–050，6 次 | 880,913 | 1 分 4.60 秒 |
| 后台画像 | 031、051，2 次 | 4,685 | 分别 4.24、5.16 秒 |
| 正式回放合计 | 51 次 | **6,186,892** | 三轮区间相加约 **14 分 40.94 秒** |

总输入为 5,993,848 tokens，总输出为 193,044；输入缓存命中 4,696,576、未命中 1,297,272。首次调用到最后后台响应的自然时间约 59 分 6 秒，包含真实 UI 操作和轮间停留；不能当作连续模型响应耗时。上述 token 是累计请求用量，包含重复上下文与缓存输入，不据此推算金额。证据：[全部调用摘要](../../../tmp/tutor-three-turn-20261018/run-5/provider-progress.json)、[最后后台调用](../../../tmp/tutor-three-turn-20261018/run-5/provider/request-051.json)。

### Q1：用途与演示

第一问 `turn_16476c7d3cc1a4f1` 已 `completed/saved`，`incomplete=false`，交付 `presentation-1792325857278515600-3` revision 2。实际主循环为 27 次模型调用，另有 3 次教学审核调用和 1 次后台 profile review；前三类记录分别在 request 001–030 与 031。公开请求／响应和 usage 均已记录。该阶段 31 次请求累计输入 3,131,514 tokens、输出 116,677 tokens，合计 3,248,191；其中输入缓存命中 2,753,408、未命中 378,106。这里是实际报告的 token 用量，不据此推算金额。主循环最后一次请求结束于约 12:23:02 UTC，后台 profile review 随后于约 12:24:06 UTC 返回。证据：[Q1 任务状态](../../../tmp/tutor-three-turn-20261018/run-5/turn-01-run.json)、[结果](../../../tmp/tutor-three-turn-20261018/run-5/turn-01-result.json)、[最终回答](../../../tmp/tutor-three-turn-20261018/run-5/turn-01-answer.md)、[调用摘要](../../../tmp/tutor-three-turn-20261018/run-5/provider-progress.json)。

三次审核不能合并为“三次有效判断”。request 021 返回 `finish_reason=length`，公开输出为空，模型使用了 32,768 个输出 tokens；运行时将它作为 `TEACHING_REVIEW_FAILED` 返回，没有有效审阅事件入库。重试的 request 023 返回 `revise`；request 029 在修改后返回 `accept`。`response_complete=true` 记录的是 Provider 响应已结束，不能把 `length` 解释为有效审核完成。证据：[第一次审核](../../../tmp/tutor-three-turn-20261018/run-5/provider/request-021.json)、[有效退回](../../../tmp/tutor-three-turn-20261018/run-5/provider/request-023.json)、[修改后接受](../../../tmp/tutor-three-turn-20261018/run-5/provider/request-029.json)。

**自动发现并改正的内容是多加了任务要求。** 原拟交付正文末尾要求读者根据“公开重点 +8”决定是否开启语义通道，并在 move 中加入 invitation 与 response expectation；页面顶部要求“先让你自己把这条通道开、关一次”。有效审核指出用户当前要的是直接解释和演示，要求移除额外回答任务并把操作说明改成可选。主 Agent 随后读取实际页面，把顶部改为“下面的切换与展开可按需要操作，没有需要完成的题目”，删除正文反问和对应 participation／response expectation，再交付 revision 2。revision 1 到 2 的 HTML 只有这一句操作说明变化，`readable_content` 未变，正文另有措辞和数据展开调整。这是没有人工输入纠错意见的审核驱动修改；它证明这次自动回路实际纠正了一项问题，尚不能将收益单独归因于新记忆。证据：[request 023](../../../tmp/tutor-three-turn-20261018/run-5/provider/request-023.json)、[request 025 页面修订](../../../tmp/tutor-three-turn-20261018/run-5/provider/request-025.json)、[revision 1](../../../tmp/tutor-three-turn-20261018/run-5/memory/agent-history.presentations/versions/presentation-1792325857278515600-3/1.json)、[revision 2](../../../tmp/tutor-three-turn-20261018/run-5/memory/agent-history.presentations/versions/presentation-1792325857278515600-3/2.json)。

**新工作记忆已真实落盘并进入同轮后续输入。** `run-5/memory/learning.db` 的 `teaching_trace` seq 13、14 分别是 `agent-work:turn_16476c7d3cc1a4f1:1` 与 `:2`，对应 revise 和 accept；两条都保存了六项有引文的理解，`open_questions=[]`，第二条还保存了正文与页面的实际变化。request 024 已包含第一条工作记录，request 029 审核包的 `agent_working_state.current_work` 和 `corrections` 也明确引用它。审核包包含 11 个实际已读段落，共 1,293 个字符，覆盖 `10.9.7.2`–`.5`、`10.9.8.1`–`.3`、`10.11.3`／`.4`、`10.12.4`、`10.13.13`。两次有效审核的 `learner_claims=[]`，没有把本次多加任务的教师问题记为学习者缺陷。证据：[持久记忆库](../../../tmp/tutor-three-turn-20261018/run-5/memory/learning.db)、[退回后的主循环输入](../../../tmp/tutor-three-turn-20261018/run-5/provider/request-024.json)、[第二次有效审核包](../../../tmp/tutor-three-turn-20261018/run-5/provider/request-029.json)。

**第一问仍有被放过的内容缺口。** 最终页面写“它既不写答案，也不判断相关性”，把“相似度不能代替原文是否支持主题关系的判断”扩大成“不判断相关性”。书中 `10.9.7.4`／`.5` 明确描述按相似度排列、召回措辞不同但可能相关的材料；该措辞需要区分召回相关性和最终来源支持。request 029 的实际 preview 文本已经包含这句话，审核仍接受，因此这项遗漏不能归因于审核没有看到文本。页面用明确标为构造示意的预设相似度，演示了词法／语义切换、合并和短字段截断；示意数据本身不构成错误，但正文与页面仍未把“查询向量如何与条目向量比较后得到这些分数”接起来，使用机制的解释停在相似度排序这一层。较长的费用实验说明有来源，但没有补上该连接。证据：[最终页面](../../../tmp/tutor-three-turn-20261018/run-5/memory/agent-history.presentations/versions/presentation-1792325857278515600-3/2.json)、[已接受审核包](../../../tmp/tutor-three-turn-20261018/run-5/provider/request-029.json)、原书 [source.txt](../../../.understand-book/understandbook/source.txt) 第 1042–1050 行。

根代理已在真实 Reader 验证：混合模式 6 条切换到仅词法 1 条；空查询显示 6 条且向量计算为 0；展开后显示含义 48/103、条件 24/32 个码点；`lexical|q2|open` 非默认状态刷新后恢复；正文来源可打开。证据：[默认状态](../../../tmp/tutor-three-turn-20261018/run-5/ui-default.txt)、[仅词法](../../../tmp/tutor-three-turn-20261018/run-5/ui-lexical.txt)、[空查询](../../../tmp/tutor-three-turn-20261018/run-5/ui-empty-query.txt)、[刷新恢复](../../../tmp/tutor-three-turn-20261018/run-5/ui-restored.txt)、[恢复截图](../../../tmp/tutor-three-turn-20261018/run-5/ui-restored.png)、[来源打开](../../../tmp/tutor-three-turn-20261018/run-5/ui-source-open.txt)。曾误点“记一下”并打开空笔记后放弃，没有提交额外文字或教学反馈。Q2 已使用真实 state revision 7 receipt 启动，这些操作未向模型输入上述内容判断。

### Q2：错误推理修正了，粒度问题仍未回答

Q2 `turn_1647697d3cc19fd8` 于 13:10:44 UTC completed/saved，`incomplete=false`，使用实际 `lexical|q2|open`、state revision 7 的 receipt。首请求 032 已带第一问的 `accepted_review_ref=agent-work:turn_16476c7d3cc1a4f1:2`、先前修订意见和后续变化；request 033 真实补读 `10.9.7.2`／`.3`／`.4`、`10.9.8.3` 与 `10.12.4`。跨轮记忆接线已有真实输入证据。证据：[Q2 首请求](../../../tmp/tutor-three-turn-20261018/run-5/provider/request-032.json)、[补读调用](../../../tmp/tutor-three-turn-20261018/run-5/provider/request-033.json)、[最终结果](../../../tmp/tutor-three-turn-20261018/run-5/turn-02-result.json)。

request 037 的第一次审核发现了具体错误：不能用缓存复用反证 LID 节点切分；不能把文档输入数／调用次数当成记录粒度证据；不能将 token 数与 Unicode 码点截断当成同单位旁证；不能把来源的“可能漏掉后半段条件”加强成“真正排除的那句话常常在后面”。request 040 又要求纠正“去重与重算都归身份”，补证或删除当前材料未支撑的查询调用关系，并修正“下面交付演示页”的错误指称。这些意见在最终稿得到相应修改：错误反证和 token／码点量级吻合论被删去，去重、重算、长度分开，频率表述改回“可能”，演示改称“上一轮”。因此本轮有真实事实论证纠错，并非只改语气。证据：[第一次退回](../../../tmp/tutor-three-turn-20261018/run-5/provider/request-037.json)、[第二次退回](../../../tmp/tutor-three-turn-20261018/run-5/provider/request-040.json)、[最终回答](../../../tmp/tutor-three-turn-20261018/run-5/turn-02-answer.md)。

但核心问题仍未解决。第一次退回后，主 Agent 仅再按“检索视图”搜索；第二次退回要求查明 BookStructure／LID 定义与视图构造后，没有进一步原文读取。最终仍说“一章一条、一候选一条，还是逐节点一条，这些段落区分不了”，把已读材料范围的限制作为答案，而没有完成用户当前询问的粒度关系。request 043 一边 accept，一边在 `open_questions` 保留同一个粒度问题，并将补查写成“若后续问题需要”。该问题本来就在本轮请求中，记录它不等于解决它。此前已查明的原书发现与检索路径说明：候选有独立含义／条件／来源身份，同一 LID 可以有多个候选，章节与候选分别形成检索视图；不能把暂未补齐的定位关系写成整书没有可回答的材料。证据：[唯一追加搜索](../../../tmp/tutor-three-turn-20261018/run-5/provider/request-038.json)、[最后接受及未决项](../../../tmp/tutor-three-turn-20261018/run-5/provider/request-043.json)、原书 [source.txt](../../../.understand-book/understandbook/source.txt) 第 1001–1007、1042–1046 行。

持久库 seq 23、24、25 分别是 Q2 的 `agent-work:turn_1647697d3cc19fd8:1`、`:2`、`:3`，保存 revise、revise、accept。每条含五项理解，未决项从两项、三项收敛为一项；保留下来的正是核心粒度问题。三次审核均 `learner_claims=[]`，这些错误和修订记录属于 Agent 工作，不是学习者误解。

### Q3：确认方向正确，仍加入了未表达的纠正

Q3 `turn_16476a7d3cc1a18b` 于 13:12:18 UTC completed/saved，`incomplete=false`。首请求 045 已包含 Q2 的已接受工作记录 `agent-work:turn_1647697d3cc19fd8:3`；request 046 读取 `10.9.7.2`、`.3`、`10.9.8.3`、`.5`；request 049 一次审核即接受。审核包仍含 Q1／Q2 的修订历史和 Q2 未决粒度问题，最终 Agent 工作事件 seq 32 为 `agent-work:turn_16476a7d3cc1a18b:1`，保存四项理解并继续保留该未决项。记忆接续成立，但没有带来这项补查。证据：[Q3 首请求](../../../tmp/tutor-three-turn-20261018/run-5/provider/request-045.json)、[读取调用](../../../tmp/tutor-three-turn-20261018/run-5/provider/request-046.json)、[审核接受](../../../tmp/tutor-three-turn-20261018/run-5/provider/request-049.json)、[最终回答](../../../tmp/tutor-three-turn-20261018/run-5/turn-03-answer.md)。

最终答复先说“对”，这部分正确；随后称“中间产物这个说法值得收窄……它更像投影，不是改写”，并解释“混在一起就容易得出错结论”。用户没有声称它是改写，也没有提出那三个概念混为一谈，这个收窄重新引入了未表达的强说法。更实质的问题在后文“它和原文的差别主要在短和只留指定字段”：本题存在“原文→预构建章节／候选语义字段→短文本投影”两个步骤，回答只描述第二步，把字段与原文的关系省略了。已有字段的截取没有再次改写，不等于这些字段就是原文字面摘录。审核仍接受，`learner_claims=[]` 只表明没有结构化掌握判断，不能证明公开措辞没有误导性纠正。Q1 页面也没有被后两问改写，revision 2 的过强相关性表述仍在。

### 画像与界面记录

Q1 后的后台 request 031 没有生成新的画像事实，只记录当前用途／演示请求意图。最后 request 051 根据 Q2／Q3 使用了 embedding 粒度、LID 树和预构建中间产物等术语，新增了一条 `technical_familiarity` 背景推断，标记为 `agent_inferred`、`provisional`、低置信度；原用户确认的讲解偏好仍是一条初始化事实。这不是把教师纠错写入学习者缺陷，也不是已确认掌握，但确实存在从术语使用推断先验熟悉程度的行为。UI 的“学习偏好与背景 · 11 条”没有被当成十一项已确认学习者能力，未为该显示数字继续扩大调查。证据：[Q1 后台结果](../../../tmp/tutor-three-turn-20261018/run-5/provider/request-031.json)、[最终后台结果](../../../tmp/tutor-three-turn-20261018/run-5/provider/request-051.json)、[读者画像投影](../../../tmp/tutor-three-turn-20261018/run-5/memory/reader-profile.md)。

最终三问界面已留存 [ui-three-turn-complete.txt](../../../tmp/tutor-three-turn-20261018/run-5/ui-three-turn-complete.txt)。下图是实际切到仅词法后只剩一条结果的页面状态；刷新恢复和来源打开证据见前文。

![真实 Reader 中仅词法模式显示一条结果](E:/allwork/download/agent/understand-book/tmp/tutor-three-turn-20261018/run-5/ui-lexical-result.png)

## 独立两阶段诊断：未采纳为固定额外调用

在正式 run-5 外，另做了恰好两次真实调用的输入隔离诊断：第一阶段只看原问题、选区与实际已读证据，先形成必要解释关系，不看主稿、设计、页面、旧审阅或工作记忆；第二阶段把该依据附给原 Q1 审核包。没有向 run-5 应用、记忆或页面写入任何结果，也没有重试或追逐提示词。

该诊断未改善当前缺口：独立依据仍没有建立查询向量与条目向量比较的连接，第二阶段仍 accept，没有指出“不判断相关性”的过强表述。两次合计 57,848 tokens，耗时 45.304 秒。**不把这种固定额外调用加入生产流程**，也不将其计入上面的 51 次正式回放、用量或纠错成功数据。证据：[实验条件](../../../tmp/tutor-three-turn-20261018/independent-review-probe/experiment.json)、[两阶段公开结果](../../../tmp/tutor-three-turn-20261018/independent-review-probe/results.json)、[统计与隔离核对](../../../tmp/tutor-three-turn-20261018/independent-review-probe/verification.json)。

## 已知限制

本轮是当前实现的单次独立连续回放，不是历史采样的逐字重现，也不产生泛化成功率。模型、教学审阅与记忆任务的机制贡献需要实际调用证据支持。界面内的教学示意计算与真实 DeepSeek 生成调用分别记录，不能把演示按钮的本地计算写成实际 embedding 服务调用。
