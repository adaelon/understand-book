# Tutor 原始三问：实际模型与 Reader 演示回放

2026-10-18。本报告覆盖 `run-1` 至 `run-4` 四次回放尝试，以及针对实际失败做的机制修复和定点模型实验。run-4 原三问完整保留；收到明确复核反馈后，正文、演示页面和机器读取的状态说明均已修订，最终交付为同一页面的 **revision 9**。真实 Reader 操作、非默认状态刷新恢复和来源打开已验证，最终页面已打开供继续交互。第四、第五、第八轮失败记录保留，不计作成功。文中链接指向本地证据；生成结果、页面状态和实验结果均保留在 `tmp/tutor-three-turn-20261018/`。

## 结果与判断

最终交付是 `run-4` 的 `presentation-1792316129531409900-15` revision 9。第六轮已在正文与页面补齐查询向量和条目向量的相似度比较、排序与混合召回；认可召回相关性可以评价；把 48/24/12 明确为候选字段的码点前缀上限；区分 BookStructure 检索条目集合和 LID 定位树。原四处内容缺口经独立复核均已修正。后续将准备状态的界面文案与机器读取说明同步为独立场景设定，计算机制保持原样。实际操作验证了调用复用、两侧待准备、词法模式、Provider 缺失、空查询、候选额度、刷新恢复和来源展开。证据：[内容修订回答](../../../tmp/tutor-three-turn-20261018/run-4/turn-06-answer.md)、[最终同步回答](../../../tmp/tutor-three-turn-20261018/run-4/turn-09-answer.md)、[最终结果](../../../tmp/tutor-three-turn-20261018/run-4/turn-09-result.json)、[revision 9 页面](../../../tmp/tutor-three-turn-20261018/run-4/memory/agent-history.presentations/versions/presentation-1792316129531409900-15/9.json)、[最终五步路径截图](../../../tmp/tutor-three-turn-20261018/run-4/ui-final/final-five-step.png)。

原始三问的结果与收到反馈后的修订分别保留。原三问核心都已答到，但第一问缺少相似度比较到排序的桥接，且把“Embedding 准不准”排除为错误问题；第二问把码点上限说成固定等长，并把 LID 树与检索条目集合混同。第三问正确确认派生短投影，后半缓存版本扩展超出当前确认问的必要范围，不影响核心正确性。原 revision 5 的空查询、词法模式、Provider 缺失、候选额度、非默认状态恢复与来源打开均有实际证据。第六轮的成功属于复核反馈后的修订，不改写原始回答。证据：[run-4 Q1](../../../tmp/tutor-three-turn-20261018/run-4/turn-01-answer.md)、[Q2](../../../tmp/tutor-three-turn-20261018/run-4/turn-02-answer.md)、[Q3](../../../tmp/tutor-three-turn-20261018/run-4/turn-03-answer.md)、[revision 5 恢复页面](../../../tmp/tutor-three-turn-20261018/run-4/ui/restored-final.txt)、[原页面来源弹窗](../../../tmp/tutor-three-turn-20261018/run-4/ui/source-popover.txt)。

修订中保留三次真实失败。第四轮因整项页面工具被隐藏而得到 `TOOL_NOT_EXPOSED`；修复只读曝光后，第五轮成功读取、搜索既有页面，但写入时先收到字段不允许的错误，模型反复删字段重试，最终 `incomplete=true`、`PRESENTATION_ARGUMENTS_INVALID`，只交付文字草稿。进一步修正操作门禁与参数校验顺序后，第六轮先提交教学设计，再修改页面，并在两次交付审阅后完成。第八轮同步状态说明时，模型先漏用已有引用，又把元数据当成内容文件，最终错误声称用户没有提供对象；第九轮经真实 Reader 请求附保存状态后完成同步。证据：[第四轮失败](../../../tmp/tutor-three-turn-20261018/run-4/turn-04-run.json)、[第五轮 trace](../../../tmp/tutor-three-turn-20261018/run-4/turn-05-result.json)、[第六轮 trace](../../../tmp/tutor-three-turn-20261018/run-4/turn-06-result.json)、[第八轮结果](../../../tmp/tutor-three-turn-20261018/run-4/turn-08-result.json)、[第九轮原生请求](../../../tmp/tutor-three-turn-20261018/run-4/turn-09-ui-request.json)。

此前 `run-2` 的第二问讲清了检索条目粒度，第三问核心确认正确；`run-3` 后两问则漏读书中直接回答问题的段落，转去解释邻近话题。运行结束、页面可打开、审阅接受和实际答到问题需要分别判断。证据：[run-2 三问历史](../../../tmp/tutor-three-turn-20261018/run-2/turn-03-history.json)、[run-3 三问历史](../../../tmp/tutor-three-turn-20261018/run-3/turn-03-history.json)。

| 回放 | 实际交付结果 | 内容与交互判断 |
| --- | --- | --- |
| run-1 | 第一问 `incomplete=true`、`TEACHING_INVALID`，没有页面交付。 | 必需的来源补读被误判为无进展；已定位并修复相应机制。 |
| run-2 | 三问均 `incomplete=false`；第一问交付 revision 2，第二问带真实 UI receipt。 | 页面查询、词法变化、空查询及恢复均有实际证据；第二问核心正确，第三问核心确认正确但局部措辞需收紧。第一问仍缺具体输入对象及召回后用途。 |
| run-3 | 第一问 `incomplete=true`，保留可打开但未接纳的页面；第二、三问均 `incomplete=false`。 | 第一问来源错误信息不足；页面空查询、恢复存在错误。后两问因检索缺口没有回答原问题，实际审阅仍接受。 |
| run-4 | 原三问均 `incomplete=false`，交付 revision 5，Q2 附真实 state revision 15 receipt。反馈修订中第 4、5、8 轮失败，第 6、7、9 轮 `incomplete=false`；最终为 revision 9。 | 原三问核心答到，四处局部缺口在第六轮正文和页面中修正，后续准备状态文案与机器说明同步完成。实际控件、非默认恢复和来源打开通过；原三问与失败结果均保留。 |

原始结果：[run-1 Q1](../../../tmp/tutor-three-turn-20261018/run-1/turn-01-result.json)；[run-2 Q1](../../../tmp/tutor-three-turn-20261018/run-2/turn-01-result.json)、[Q2](../../../tmp/tutor-three-turn-20261018/run-2/turn-02-result.json)、[Q3](../../../tmp/tutor-three-turn-20261018/run-2/turn-03-result.json)；[run-3 Q1](../../../tmp/tutor-three-turn-20261018/run-3/turn-01-result.json)、[Q2](../../../tmp/tutor-three-turn-20261018/run-3/turn-02-result.json)、[Q3](../../../tmp/tutor-three-turn-20261018/run-3/turn-03-result.json)。

## 实际模型、问题与验证边界

每次完整回放都使用同一本地书 `.understand-book/understandbook`、当前原生 DeepSeek 接口和 `deepseek-v4-flash` 配置，调用实际工具。来源修订为 `8923305d6ab5be4122268f51583d1ab31fa1776f3852f70d6eca422a9f363e23`，与原始材料对应。目标“理解第六章”和已确认的讲解偏好通过正常 API 初始化。每个 run 使用独立 chat、Tutor 与私有记忆，同一次 run 内的新回答自然成为后续问题的上下文。证据：[配置](../../../tmp/tutor-three-turn-20261018/run-3/configuration.json)、[来源就绪结果](../../../tmp/tutor-three-turn-20261018/run-3/readiness.json)、[启动脚本](../../../tmp/tutor-three-turn-20261018/start-replay.mjs)。

| 轮次 | 原始问题 | 保留的输入条件 |
| --- | --- | --- |
| 1 | 那Embedding在哪里会被用到呢，怎么用呢，给我个演示页吧 | anchor `10.9.8.2`；精确选中“Embedding 会产生计算、等待与可能的外部费用。”，范围 `0..27`。 |
| 2 | 准备阶段embedding的粒度是怎么切分的呢，是根据lid树来切分的吗 | anchor `11.3`，无选中文本；附本次新页面经实际 UI 操作保存的 receipt。 |
| 3 | 啊，那进入embedding的内容不是原文而是预构建的中间产物对吗 | anchor `11.3`，无选中文本，不额外附页面 receipt。 |

证据：[原始问题](../../../tmp/tutor-three-turn-20261018/run-3/original-questions.json)、[Q1 实际输入](../../../tmp/tutor-three-turn-20261018/run-3/turn-01-input.json)、[Q2 实际输入](../../../tmp/tutor-three-turn-20261018/run-3/turn-02-input.json)、[Q3 实际输入](../../../tmp/tutor-three-turn-20261018/run-3/turn-03-input.json)。

原始线上会话共 226 条事件，三问 accepted 序号为 2、140、198。第一问之前没有其他公开用户轮次，但 Tutor 继承了更早跨会话事实与旧设计。本次没有导入这些旧状态，所以验证的是当前实现的独立连续回放，不是历史采样的逐字重放。演示中的简化向量也只是页面教学算法；真实 DeepSeek 用于生成、审阅和回答，并不意味着页面每次点击都调用真实 embedding 服务。证据：[原始会话](../../../tmp/answer-quality-20261018/replay-input/original-session.jsonl)、[配置中的历史差异](../../../tmp/tutor-three-turn-20261018/run-3/configuration.json)、[run-2 页面说明](../../../tmp/tutor-three-turn-20261018/run-2/ui/restored-isolated-ui.txt)、[run-3 页面说明](../../../tmp/tutor-three-turn-20261018/run-3/ui/initial.txt)。

### 这三问应讲清的关系

以下是人工核对后的参考讲解，不是模型原答：在本次 BookStructure 路径里，原书先形成预构建的章节/候选条目，再取检索短文本。章节视图使用问题与概览；候选视图分别取 meaning、条件串、aliases 串的前 48、24、12 个 Unicode 码点后拼接，长度不足时不会补成等长。每条短文本生成一个条目向量，查询另生成查询向量；两者比较相似度并排序，形成语义候选，再与精确、别名和词法结果合并，最后通过保留的位置读取完整材料、判断原文是否支持具体关系。LID 提供定位，这里不是直接按 LID 全文切块，也不是每个字段各算一条向量。预构建产物仍来自原书；召回相关性可以评价，但不能用相似度替代最终的来源判断。依据：原书 [source.txt](../../../.understand-book/understandbook/source.txt) 第 1001、1007、1042–1050、1056–1068 行。

### 本轮借鉴 Socratopia 的范围

借鉴的是已经核实的教学判断边界：`learnerNeed` 没有具体证据时可以为空，先处理学习者当前问题；教师自身的漂移、空泛应归给教师，不能反写成学习者缺陷；掌握情况未知不等于未掌握，教师说错、学习者纠正时不能倒置错误归属。依据：[module-48130.js](E:/Socratopia-REA-Analysis/2026-10-18/readable-teaching/module-48130.js) 第 18、191、210 行，以及 [lesson-summary 提示词提取](E:/Socratopia-REA-Analysis/2026-10-18/prompt-extraction/endclass-memory.json) 的课堂工作记忆规则。本轮没有移植六字段课后总结、新后台教练、人物日记/关系记忆或云端 rules。

## 已完成的机制修复

**让缺源补读成为有效进展，并在正确阶段开放页面编写。** run-1 的模型按提示补读 `10.9.8.3`，却在两个无进展批次后被拒绝为 `AGENT_NO_PROGRESS`。设计预检现在返回已有类型化 `missing_source_read`，由进展追踪记录该定位；没有扩大无进展预算。活跃 Tutor 的页面写入与 Global 指导也受设计阶段约束，读取已有页面的例外在后续修订中补齐。run-2 实际跨过补读并完成页面生成。实现：[teaching.rs](../../../crates/server/src/teaching.rs) 的 `preflight_step`、[orchestrator.rs](../../../crates/runtime/src/orchestrator.rs) 的缺源结果处理与采样曝光；回归入口：[teaching_design_loop_tests.rs](../../../crates/runtime/src/teaching_design_loop_tests.rs)、[teaching_design_tests.rs](../../../crates/server/src/tests/teaching_design_tests.rs)、[presentation_authoring_tests.rs](../../../crates/runtime/src/presentation_authoring_tests.rs)。已有验证为 runtime 相关 31 项通过、server 设计组 27 项通过且 2 项既有真实模型测试忽略、曝光回归 1 项通过，见 [CHECKPOINT.md](../../../tmp/tutor-three-turn-20261018/CHECKPOINT.md)。

**页面读取与页面写入分别执行设计门禁，并明确恢复步骤。** 第四轮修订暴露了上述初版曝光策略隐藏整项工具的缺口。现已让无设计或 text 设计时的 `read/search` 可见，并可实际调用宿主读取、搜索既有页面；`prepare/write/deliver` 仍受设计条件约束。已接受 presentation 设计和普通页面流程保持原有行为。第五轮实际读取成功，但暴露了额外字段校验先于操作门禁的问题。进一步修复后，先判断操作是否允许，再校验该操作的参数，并明确通过 `tutor.step design` 恢复写入的路径。最终 `cargo test -p runtime --lib teaching_presentation_` 的 5 项测试全部通过，server/worker 构建通过、耗时 39.38 秒；第六轮实际先提交 design，再完成页面修改、预览、保存与交付审阅。实现：[orchestrator.rs](../../../crates/runtime/src/orchestrator.rs)，覆盖：[presentation_authoring_tests.rs](../../../crates/runtime/src/presentation_authoring_tests.rs)，实际验证：[第五轮结果](../../../tmp/tutor-three-turn-20261018/run-4/turn-05-result.json)、[第六轮结果](../../../tmp/tutor-three-turn-20261018/run-4/turn-06-result.json)，构建与测试记录：[CHECKPOINT.md](../../../tmp/tutor-three-turn-20261018/CHECKPOINT.md)。

**用已接受的教学设计直接约束页面制作。** 活跃 Tutor 页面现在继承设计的 target、聚焦 blocks 与 observation，进入现有 Local 阶段；同设计的 Local/Review 任务及页面区域关联保留，普通页面和 outside 分支继续原流程。这样不再于教学设计之后另起通用 Global 内容规划。实现：[orchestrator.rs](../../../crates/runtime/src/orchestrator.rs) 的 `focused_teaching_authoring`，同时用于采样与 prepare；相关 `presentation_authoring` 30 项测试通过。run-3 已实际使用该路径，但其内容仍偏向调用成本，因此不能把链路修复等同于讲解质量保证。证据：[测试入口](../../../crates/runtime/src/presentation_authoring_tests.rs)、[run-3 Q1](../../../tmp/tutor-three-turn-20261018/run-3/turn-01-result.json)、[验证记录](../../../tmp/tutor-three-turn-20261018/CHECKPOINT.md)。

**避免把内部教学计划重复显示给读者。** `AgentPresentation` 将真实 iframe 内容宿主交给 `TutorActivities`；无行动要求且已由页面承载的内容不再重复渲染私有 `move.prompt`，曝光记录仍保留。两个组件测试文件 46 项通过，`vue-tsc --noEmit` 通过，run-2 重载后的实际 UI 也不再出现该内部计划块。证据：[TutorActivities.vue](../../../packages/web/src/components/TutorActivities.vue)、[AgentPresentation.vue](../../../packages/web/src/components/AgentPresentation.vue)、[对应测试](../../../packages/web/src/components/TutorActivities.test.ts)、[实际 UI](../../../tmp/tutor-three-turn-20261018/run-2/ui/restored-isolated-ui.txt)、[验证记录](../../../tmp/tutor-three-turn-20261018/CHECKPOINT.md)。

**保留能够修复失败的具体诊断。** run-3 Q1 先缺 `move.intent_quote`，随后得到笼统的“教学正文来源无效”。现在缺失字段会明确返回；来源编译失败保留现有 `AnswerDeliveryIssue`，并指向 `move.delivery_text`。来源与意图的校验语义不变。聚焦回归先失败、修复后 1 项通过，覆盖补字段、未知引用值与位置、预检/执行一致，以及修正后的接纳。实现：[teaching.rs](../../../crates/server/src/teaching.rs)，测试：[teaching_design_tests.rs](../../../crates/server/src/tests/teaching_design_tests.rs) 的 `teaching_select_validation_errors_identify_the_missing_field_and_invalid_source`。原 run-3 拟交付正文没有完整保留，无法确定当时究竟哪个引用失配；测试构造的错误引用不是原故障证据。

**让没有自定义状态回调的页面也接受恢复验证。** run-3 的业务状态只在闭包里，重载后控件值保留而结果丢失；原制作端恢复检查在没有 state reader 时直接跳过。现已用控件与可见结果形成后备快照，在新页面上下文恢复后比较实际可见结果。原 run-3 HTML 能复现失败，闭包状态、静态页面和输入驱动结果三个浏览器场景通过；没有改写已经交付的 run-3 页面。实现：[presentation_preview.rs](../../../crates/server/src/presentation_preview.rs)，测试：[presentation_author_tests.rs](../../../crates/server/src/tests/presentation_author_tests.rs)。run-4 新页面已完成的具体恢复场景见末节。

run-4 的实际预览继续挡住了恢复后词法/Provider 失败状态变成语义候选的错误，模型随后把结果改为从原生控件重算，再通过预览。制作期间还暴露了回调包装错误：直接返回 `S` 会使 `values.page` 为空，正确形式是 `{values:S}`。恢复错误现在给出准确包装示例，并在失败后自动展示已有状态指南，不改变 schema。runtime 回归覆盖两条失败路径，1 项通过；真实浏览器回归覆盖错误返回被阻止、正确包装被放行，1 项通过。实现：[orchestrator.rs](../../../crates/runtime/src/orchestrator.rs) 的恢复失败处理、[presentation_preview.rs](../../../crates/server/src/presentation_preview.rs) 的恢复诊断；测试：[presentation_authoring_tests.rs](../../../crates/runtime/src/presentation_authoring_tests.rs) 的 `presentation_restore_failure_loads_state_contract_without_replanning`、[presentation_author_tests.rs](../../../crates/server/src/tests/presentation_author_tests.rs) 的 `preview_state_reader_wrapper_preserves_the_saved_result`；实际链路：[run-4 Q1 trace](../../../tmp/tutor-three-turn-20261018/run-4/turn-01-result.json)。

进度显示也已把摘要标签明确为“最近步骤”、耗时明确为“本步”，避免把最后一步的秒数读成整轮生成时间。相关组件 6 项测试通过，独立前端重建后实际 Reader 已显示“最近步骤：生成回答”“本步 3.3 秒”。实现与覆盖：[AgentActivities.vue](../../../packages/web/src/components/AgentActivities.vue)、[AgentActivities.test.ts](../../../packages/web/src/components/AgentActivities.test.ts)；实际界面：[current-reader.txt](../../../tmp/tutor-three-turn-20261018/run-4/ui/current-reader.txt)。

## 各问的实际内容与页面结果

### 第一问：演示能动，但对象与用途还没有接全

run-2 页面同时展示查询、候选排序、词法变化和向量复用，且实际审阅挡下了四个具体错误：别名混算为词法命中；relabel 未改查询却重复计算查询向量；正文默认 15 条与页面 13 条不一致；直接讲解末尾擅自追加预测练习。修订后交付 revision 2。证据：[第一次拒绝](../../../tmp/tutor-three-turn-20261018/run-2/provider/request-028.json)、[第二次拒绝](../../../tmp/tutor-three-turn-20261018/run-2/provider/request-034.json)，均定位最后一个工具结果；[最终回答](../../../tmp/tutor-three-turn-20261018/run-2/turn-01-answer.md)。

run-3 页面更集中地说明“文档准备后缓存，查询单独计算”：嵌入 18 条后文档调用保持 5 次，两次不同查询改变前三名并只增加查询调用。这是有用的局部演示。但它的 18 条“文档”是作者编写的术语说明，没有连接本项目实际章节/候选条目的文本；前三条结果之后又未连接完整候选、原文位置和关系判断。两次回放都仍缺“本项目输入哪个对象”和“召回后如何使用它”这两个连接，而非必须补齐所有维度、截断长度和混合额度才算有效。证据：[run-2 页面](../../../tmp/tutor-three-turn-20261018/run-2/memory/agent-history.presentations/versions/presentation-1792313050528799700-7/2.json)、[run-3 页面](../../../tmp/tutor-three-turn-20261018/run-3/memory/agent-history.presentations/versions/presentation-1792314703736323100-5/1.json)、[查询一](../../../tmp/tutor-three-turn-20261018/run-3/ui/query-one.txt)、[查询二](../../../tmp/tutor-three-turn-20261018/run-3/ui/query-two.txt)。

真实 UI 结果如下：

| 操作 | run-2 | run-3 |
| --- | --- | --- |
| 改查询 | 首候选由 C01 变为 C08，查询累计增至 2，文档复用。 | 前三条随查询变化，文档调用保持 5，查询累计增至 2。 |
| 只改完整词法字段 | 13 条变 15 条；别名 1、词法 2，查询不重算。 | 本页没有该操作。 |
| 空查询 | 浏览完整目录，本次文档和查询向量计算均为 0。 | 错误地增加查询计数，返回三条 `0.000` 候选。 |
| 保存后重开 | 非默认查询、relabel、首候选 C08 保留；累计文档重算/复用/查询为 20/40/2。 | 输入框保留，但准备状态、计数与结果归零。 |

证据：run-2 的 [查询变化](../../../tmp/tutor-three-turn-20261018/run-2/ui/query-change.txt)、[词法变化](../../../tmp/tutor-three-turn-20261018/run-2/ui/lexical-change.txt)、[空查询](../../../tmp/tutor-three-turn-20261018/run-2/ui/empty-query.txt)、[恢复后](../../../tmp/tutor-three-turn-20261018/run-2/ui/restored-isolated-ui.txt)；run-3 的 [空查询](../../../tmp/tutor-three-turn-20261018/run-3/ui/empty-query.txt)、[重开前](../../../tmp/tutor-three-turn-20261018/run-3/ui/before-restore.txt)、[重开后](../../../tmp/tutor-three-turn-20261018/run-3/ui/restored.txt)。

第二问使用的是实际 UI 保存的 receipt：run-2 页面 revision 2/state revision 8；run-3 页面 revision 1/state revision 11，后者来自重开失败后重新准备并查询的现场。原问题与 anchor 保持不变，未用手写页面状态替代。证据：[run-2 状态](../../../tmp/tutor-three-turn-20261018/run-2/ui/selected-state.json)、[run-2 receipt](../../../tmp/tutor-three-turn-20261018/run-2/turn-02-page-receipt.json)、[run-3 状态](../../../tmp/tutor-three-turn-20261018/run-3/ui/selected-state.json)、[run-3 receipt](../../../tmp/tutor-three-turn-20261018/run-3/turn-02-page-receipt.json)。

### 第二问：核心粒度已有回答，需区分上限和集合身份

run-2 明确回答：向量输入跟随章节/候选检索条目，而非逐 LID 节点；候选条目内再取短投影，LID 保留为原文定位。这与书中“同一 LID 可包含多个候选”和检索视图字段相符。结尾“猜测方向正好相反”措辞过强，但核心解释成立。证据：[run-2 Q2](../../../tmp/tutor-three-turn-20261018/run-2/turn-02-answer.md)、原书 [source.txt](../../../.understand-book/understandbook/source.txt) 第 1001、1056、1058 行。

run-3 却说已读材料不能确认输入单元，转去解释“LID 位置与模型分块可以分开调整”，随后展开批量调用次数。它没有读到直接回答的 `10.9.7.2` / `10.9.7.3`：章节视图使用问题与概览，候选视图使用含义、条件和别名，再取短文本计算向量。这是检索不足造成未答原问，不是书源缺失，也不是工具被隐藏。证据：[run-3 Q2 回答](../../../tmp/tutor-three-turn-20261018/run-3/turn-02-answer.md)、[实际 trace](../../../tmp/tutor-three-turn-20261018/run-3/turn-02-result.json)、原书 [source.txt](../../../.understand-book/understandbook/source.txt) 第 1056、1058 行。

run-4 回到了直接依据，明确说明检索条目、字段与前缀拼接，核心回答正确。两处局部表述仍错：48/24/12 是各字段码点前缀的上限，短字段不会补成“码点数被写死的等长”；“树决定有哪几条东西可被检索”又把用户所问的 LID 树混同了章节/候选检索集合，代码实际分别遍历 chapters 与 catalog，同一个 LID 可以对应多个候选。应保留条目粒度的结论并修正这两处关系。证据：[run-4 Q2](../../../tmp/tutor-three-turn-20261018/run-4/turn-02-answer.md)、[结果](../../../tmp/tutor-three-turn-20261018/run-4/turn-02-result.json)、[book-structure-retrieval.ts](../../../packages/core/src/book-structure-retrieval.ts) 第 15、18、26、30 行、原书 [source.txt](../../../.understand-book/understandbook/source.txt) 第 1001 行。

### 第三问：run-2 核心确认正确，run-3 转向邻近问题

run-2 已确认输入是“目录条目 → 短投影”的派生文本，回答了用户“预构建中间产物”的确认问。此前把整题判断为失败过严，现以原问题和原书重新核对后的结论为准。局部需要收紧：应说“分别截取字段，再拼接”，不是“拼起来再截断”；末段将上游条目是否由模型生成，与本阶段是否额外改写摘要混在一起；“原文和位置根本没有参与”宜限定为不直接作为此处向量输入。这些不推翻核心确认。证据：[run-2 Q3](../../../tmp/tutor-three-turn-20261018/run-2/turn-03-answer.md)、[原问题](../../../tmp/tutor-three-turn-20261018/run-2/turn-03-input.json)、原书 [source.txt](../../../.understand-book/understandbook/source.txt) 第 1007、1056、1058 行。

run-3 的实际审阅曾拒绝一项没有原话依据的学习者判断，即把用户的确认扩展为“不再需要原文”。但最终回答仍称不能确认 embedding 输入，改讲 inspect 与 read 的交付差别，并建议把用户的问题改成另一句“更安全”的表述。它同样未补读 `10.9.7.2` / `.3`，故最终没有回答原问题；`incomplete=false` 和审阅接受不能弥补该缺口。证据：[run-3 Q3](../../../tmp/tutor-three-turn-20261018/run-3/turn-03-answer.md)、[实际 trace 与接受结果](../../../tmp/tutor-three-turn-20261018/run-3/turn-03-result.json)、原书 [source.txt](../../../.understand-book/understandbook/source.txt) 第 1056、1058 行。

run-4 第三问于 09:53:10 UTC 完成，`incomplete=false`，直接确认进入向量计算的是候选字段截取、拼接形成的派生短投影，并说明原文和完整字段没有被替换，核心回答正确。后半段扩展到投影版本、缓存依赖和失效，篇幅比当前确认问所需更广；这属于范围观察，不据此否定核心答案。证据：[run-4 Q3 回答](../../../tmp/tutor-three-turn-20261018/run-4/turn-03-answer.md)、[完成结果](../../../tmp/tutor-three-turn-20261018/run-4/turn-03-result.json)。

## 审阅输入修改的实际效果

审阅包已去除重复的 `recent_facts`、完整 learner view 和未公开的旧计划字段，保留一份 canonical learner grounding，以及实际交付文字、页面现场、反馈与关联。持久化数据和生成端上下文不变，8 项相关测试通过。实际 DeepSeek 定点对照中，原包加原答、精简包加原答、精简包加正确短答都返回 `accept`；原答审阅输入从 **38,401 降到 13,677 tokens**。这证明该样本输入减少，不能证明接受决策改变或局部表述被修正。不同请求缓存命中不同，不据耗时推导提速。证据：[三组结果](../../../tmp/tutor-three-turn-20261018/review-probe/results.json)、[实验条件](../../../tmp/tutor-three-turn-20261018/review-probe/experiment.json)、[验证记录](../../../tmp/tutor-three-turn-20261018/CHECKPOINT.md)。

针对 run-3 的缺源转题，曾将现有审阅说明的一段替换为“区分已检索材料与整书可提供的材料，检索不足时定位缺失来源并回答原问题”。没有新增审阅调用或 schema。真实定点实验中，原失败样本仍 `accept`，正确答案对照也 `accept`；追加三个真实搜索的参数、覆盖状态、返回 LID 和续页信息后，结果虽变为 `revise`，要求的仍是协调页面标签与回答措辞，继续容许“已读材料未交代”，没有要求补查 `10.9.7.2` / `.3`。因此 source-gap probe 未达到触发补读的目标，这段无已证收益的生产提示替换已回退。run-4 启动时使用了包含该替换的构建，其采样和结果按当时状态保留，不改写历史实验条件。当前实现：[tutor.rs](../../../crates/runtime/src/tutor.rs)；证据：[两组结果](../../../tmp/tutor-three-turn-20261018/source-gap-probe/results.json)、[coverage 实验条件](../../../tmp/tutor-three-turn-20261018/source-gap-probe/coverage-experiment.json)、[coverage 实际返回](../../../tmp/tutor-three-turn-20261018/source-gap-probe/coverage-response.json)。

## 已知问题与实验限制

最终页面是收到人工复核反馈后的修订成品，不代表原始三问首次回答没有缺口。演示将“文档是否按当前目录准备”“查询是否按当前查询准备”设为独立场景控件：切换目录或查询不会自动翻转准备状态，输入变化后的重算由对应 pending 选项表达。revision 8 已把这一语义写明，revision 9 又同步了机器读取说明。该交互用于理解准备和复用的关系，不是实际 embedding 服务的缓存实现。证据：[第七轮回答](../../../tmp/tutor-three-turn-20261018/run-4/turn-07-answer.md)、[最终状态说明](../../../tmp/tutor-three-turn-20261018/run-4/turn-09-answer.md)、[最终页面](../../../tmp/tutor-three-turn-20261018/run-4/memory/agent-history.presentations/versions/presentation-1792316129531409900-15/9.json)。

第八轮仍暴露了具体的模型工具使用失败：上下文已经给出对象引用，模型前两次未使用，第三次使用正确引用却把 `state_contract` 元数据当作内容文件读取，随后把失败错误归因于用户未提供对象。第九轮从 Reader 附带真实页面 receipt 后成功，不足以证明这类引用使用错误在其他请求中不会再发生。证据：[第八轮错误答复](../../../tmp/tutor-three-turn-20261018/run-4/turn-08-answer.md)、[会话持久事件](../../../tmp/tutor-three-turn-20261018/run-4/memory/agent-sessions/s_Y2hhdC0wMWExNGU1Ny02NTM0LTczYjItOGE0ZS03ZmM5YWM0NDlhZWE.jsonl)（seq 892）、[第九轮原生请求](../../../tmp/tutor-three-turn-20261018/run-4/turn-09-ui-request.json)。

run-3 的旧页面保留其空查询和恢复失败记录，不把新页面通过写成旧交付已被修复。旧页面还把累计 2650 个文档输入称作可等比缩小的候选目录规模，原书只给出累计处理量，并未给出相同数量的独立候选；该表述无来源依据。证据：[run-3 页面](../../../tmp/tutor-three-turn-20261018/run-3/memory/agent-history.presentations/versions/presentation-1792314703736323100-5/1.json)、原书 [source.txt](../../../.understand-book/understandbook/source.txt) 第 1117 行。

实验中有两件基础设施事件不计为产品失败：run-2 Q1 的同步 fetch 约五分钟后超时，服务中的同一 turn 仍继续运行，随后按 turn ID 取回结果，没有重复提交；后续改用异步 run 观察。另一次共享 `packages/web/dist` 被其他任务的 network build 覆盖，刷新出现登录页；改用本次独立 local assets 并保留原 run-2 记忆恢复。证据：[等待错误](../../../tmp/tutor-three-turn-20261018/run-2/turn-01-error.json)、[同任务最终结果](../../../tmp/tutor-three-turn-20261018/run-2/turn-01-run.json)、[capture-turn.mjs](../../../tmp/tutor-three-turn-20261018/capture-turn.mjs)、[独立 assets 启动配置](../../../tmp/tutor-three-turn-20261018/start-replay.mjs)、[事件记录](../../../tmp/tutor-three-turn-20261018/CHECKPOINT.md)。

完整回放的原记录器不能解析部分 SSE 响应，不能据它计算完整 provider 成本；定点 probe 另行解析了流式用量，以上 token 数来自其实际返回。run-3 Q1 的原始拟交付正文未保留，不能倒推具体失配引用。单次实验也不支持泛化成功率或稳定速度结论。证据：[完整回放记录器进度](../../../tmp/tutor-three-turn-20261018/run-3/provider-progress.json)、[定点 probe 结果](../../../tmp/tutor-three-turn-20261018/review-probe/results.json)。

## run-4：原三问与实际修订成品

第一问于 09:45:08 UTC 完成，耗时约 16 分 19 秒，`incomplete=false`，最终页面为 `presentation-1792316129531409900-15` revision 5。回答已说清章节/候选视图的文本字段、文档与查询两个接口、准备与复用、失败条件，以及向量与原文引用的区别，主体可用。局部不足是只列出“短文本 → 向量 → 补入候选”，没有明确说查询向量与各条目向量比较相似度后排序；末段“真正该问的不是 Embedding 准不准”也过强，向量召回相关性本就可以评价。这两点不等于整答失败。页面明确声明使用字符二元组余弦的示意算法，简化本身不构成问题。证据：[最终回答](../../../tmp/tutor-three-turn-20261018/run-4/turn-01-answer.md)、[完成结果](../../../tmp/tutor-three-turn-20261018/run-4/turn-01-result.json)、[任务记录](../../../tmp/tutor-three-turn-20261018/run-4/turn-01-run.json)、[最终页面](../../../tmp/tutor-three-turn-20261018/run-4/memory/agent-history.presentations/versions/presentation-1792316129531409900-15/5.json)。

真实 Reader 操作结果如下。计数和控件值以 host 保存快照中的 `state.observed_result`、`state.values.controls` 及最终完整 UI 为依据；多数中间 UI 文本文件仅保存了增量状态，不能单独复核完整画面。

| 操作 | 实际结果 | 保存的现场 |
| --- | --- | --- |
| 默认状态与改查询 | 默认文档/查询/合计调用为 0/1/1，精确或别名 2、语义 3；改为取消用量问题后候选变为空。 | [默认](../../../tmp/tutor-three-turn-20261018/run-4/memory/agent-history.presentations/states/presentation-1792316129531409900-15/5/1.json)、[改查询](../../../tmp/tutor-three-turn-20261018/run-4/memory/agent-history.presentations/states/presentation-1792316129531409900-15/5/2.json) |
| 清空查询 | 完整显示 6 条目录，两侧调用与合计均为 0。 | [空查询](../../../tmp/tutor-three-turn-20261018/run-4/memory/agent-history.presentations/states/presentation-1792316129531409900-15/5/3.json) |
| lexical_only，查询候选视图 | 精确命中 1 条，语义 0，调用 0/0/0。 | [词法模式](../../../tmp/tutor-three-turn-20261018/run-4/memory/agent-history.presentations/states/presentation-1792316129531409900-15/5/5.json) |
| semantic_required 缺 Provider，并重开 | 明确失败，五个计数全为 0、候选为空；重开后保持。 | [重开前](../../../tmp/tutor-three-turn-20261018/run-4/memory/agent-history.presentations/states/presentation-1792316129531409900-15/5/7.json)、[重开后](../../../tmp/tutor-three-turn-20261018/run-4/memory/agent-history.presentations/states/presentation-1792316129531409900-15/5/8.json) |
| 默认查询，top_k 从 3 改为 1 | 精确/别名仍为 2，语义从 3 变为 1。 | [top_k=3](../../../tmp/tutor-three-turn-20261018/run-4/memory/agent-history.presentations/states/presentation-1792316129531409900-15/5/10.json)、[top_k=1](../../../tmp/tutor-three-turn-20261018/run-4/memory/agent-history.presentations/states/presentation-1792316129531409900-15/5/11.json) |
| 最终非默认状态再重开 | 查询“候选视图使用含义条件和别名”，文档待准备、查询已准备、top_k=1；调用保持 1/0/1，精确 1、语义 1。 | [重开前](../../../tmp/tutor-three-turn-20261018/run-4/memory/agent-history.presentations/states/presentation-1792316129531409900-15/5/14.json)、[重开后](../../../tmp/tutor-three-turn-20261018/run-4/memory/agent-history.presentations/states/presentation-1792316129531409900-15/5/15.json)、[完整 UI](../../../tmp/tutor-three-turn-20261018/run-4/ui/restored-final.txt) |

第二问于 09:47:23 UTC 开始并已完成，携带上述真实页面 state revision 15 receipt；问题、anchor 与原始输入一致。最终状态由原生控件重算，`values.page=null` 不代表恢复失败，重开后实际结果已验证一致。第三问于 09:53:10 UTC 完成，原三问至此完整结束。证据：[选定状态](../../../tmp/tutor-three-turn-20261018/run-4/ui/selected-state.json)、[receipt](../../../tmp/tutor-three-turn-20261018/run-4/turn-02-page-receipt.json)、[第二问输入](../../../tmp/tutor-three-turn-20261018/run-4/turn-02-input.json)、[三问最终历史](../../../tmp/tutor-three-turn-20261018/run-4/turn-03-history.json)。

实际点击演示的正文来源 [1] 后，Reader 弹窗显示了检索视图、候选字段 48/24/12 码点投影，以及精确/别名、词法、语义合并的原文，覆盖 `10.9.7.2` / `.3` 等相关段落。来源入口因此已有实际点击与显示证据。证据：[完整来源弹窗](../../../tmp/tutor-three-turn-20261018/run-4/ui/source-popover.txt)。

原三问结束后的 server 与 presentation worker 构建已通过，耗时 48.80 秒。随后恢复同一 run-4 记忆，于 09:55:40 UTC 提交第四轮显式修订，输入明确列出人工复核确认的缺口，要求修订现有正文与页面。该轮属于收到反馈后的修订，不是原始学习者三问，也不能算首次回答通过。证据：[修订边界](../../../tmp/tutor-three-turn-20261018/run-4/repair-purpose.json)、[第四轮实际输入](../../../tmp/tutor-three-turn-20261018/run-4/turn-04-input.json)。

第四轮最终 `execution_state=failed`、`persistence_state=saved`，没有 outcome；`final_view.error.error_code=TOOL_NOT_EXPOSED`，错误为本次采样未曝光 `presentation.author`。模型因而无法 read/search 已交付演示，没有产出修订成品。原三问和 revision 5 页面完整保留。证据：[第四轮任务记录](../../../tmp/tutor-three-turn-20261018/run-4/turn-04-run.json)。

只读门禁修复完成并通过 5 项相关 runtime 测试后，server/worker 构建通过，耗时 45.09 秒。第五轮沿用同一 run-4 记忆和同一份反馈，于 10:27:20 UTC 开始、10:30:35 UTC 结束。真实 trace 中多次 `presentation.author search/read` 成功，随后写入依次收到 `$.assets is not allowed`、`$.based_on is not allowed`、`$.html is not allowed`；模型删字段重试，仍未得到页面写入所需的设计恢复指引。最终任务为 `completed/saved`，但 outcome 为 `incomplete=true`、`warning=PRESENTATION_ARGUMENTS_INVALID`。证据：[第五轮输入](../../../tmp/tutor-three-turn-20261018/run-4/turn-05-input.json)、[任务状态](../../../tmp/tutor-three-turn-20261018/run-4/turn-05-run.json)、[完整结果与 trace](../../../tmp/tutor-three-turn-20261018/run-4/turn-05-result.json)。

第五轮仅输出“页面步骤修订”和“三问收束段”的文字草稿，没有更新现有页面或交付新 revision。该草稿与第四、第五轮失败结果完整保留，不能作为页面更新成功的证据。证据：[第五轮文字草稿](../../../tmp/tutor-three-turn-20261018/run-4/turn-05-answer.md)。

### 第六轮：实际修订交付与交互复核

操作门禁与诊断顺序修复、5 项相关测试和 server/worker 构建通过后，第六轮沿用同一反馈与 run-4 记忆，于 10:34:58–10:39:09 UTC 完成，任务为 `completed/saved`，`incomplete=false`。模型先主动提交 `tutor.step design`，随后读取和修改既有页面，经预览与审阅交付同一 presentation 的 revision 7。原四处缺口已在正文和页面中修正：查询向量逐一与条目向量比较相似度形成排序；召回相关性可以评价；48/24/12 是候选字段前缀上限，分别截取后拼成一条短文本；检索条目来自 BookStructure 检索视图，LID 用于定位，二者不能混同。证据：[任务记录](../../../tmp/tutor-three-turn-20261018/run-4/turn-06-run.json)、[完整结果](../../../tmp/tutor-three-turn-20261018/run-4/turn-06-result.json)、[修订正文](../../../tmp/tutor-three-turn-20261018/run-4/turn-06-answer.md)、[revision 7 页面](../../../tmp/tutor-three-turn-20261018/run-4/memory/agent-history.presentations/versions/presentation-1792316129531409900-15/7.json)。

这次自动审阅还起到了具体纠错作用。revision 6 已改主要解释，但在“五步路径”、`readable_content` 和 `teaching_binding` 中把 48/24/12 写成“每条”的共同规则，会误套到使用问题与概览的章节条目上。第一次交付审阅据此退回，要求限定为“每条候选”，并同步修正对应摘要与观察。模型读取实际页面后修改、重新预览，revision 7 经第二次交付审阅接受。它证明本次审阅纠正了这一局部过度概括；不能据此把全部改善归因于某段提示词或 Socratopia 借鉴。证据：[退回的完整工具结果](../../../tmp/tutor-three-turn-20261018/run-4/provider-resume-1792319683788/request-013.json)（`tutor.step` 工具消息）、[两次 select 与页面修订 trace](../../../tmp/tutor-three-turn-20261018/run-4/turn-06-result.json)。

以下是 revision 7 的真实 Reader 操作结果。前三个计数依次为文档调用、查询调用、合计调用；后两个为精确/别名命中和语义候选。

| 操作 | 实际结果 | 现场证据 |
| --- | --- | --- |
| 默认查询，top_k 从 3 改为 1 | 精确/别名保持 2，语义候选从 3 减为 1。 | [top-k](../../../tmp/tutor-three-turn-20261018/run-4/ui-repair-6/topk-one.txt) |
| 查询已准备，两侧复用 | 调用为 0/0/0。 | [查询复用](../../../tmp/tutor-three-turn-20261018/run-4/ui-repair-6/query-reuse.txt) |
| 文档待准备、查询已准备 | 调用为 1/0/1。 | [文档待准备](../../../tmp/tutor-three-turn-20261018/run-4/ui-repair-6/document-pending.txt) |
| 文档与查询均待准备 | 调用为 1/1/2。 | [两侧待准备](../../../tmp/tutor-three-turn-20261018/run-4/ui-repair-6/both-pending.txt) |
| semantic_required 缺 Provider | 五个计数均为 0，没有候选，明确呈现失败状态。 | [Provider 缺失](../../../tmp/tutor-three-turn-20261018/run-4/ui-repair-6/provider-missing.txt) |
| lexical_only | 五个计数为 0/0/0/2/0，保留精确/别名结果。 | [词法模式](../../../tmp/tutor-three-turn-20261018/run-4/ui-repair-6/lexical-only.txt) |
| v2、缺 Provider、语义模式下清空查询 | 仍浏览完整 7 条目录，调用为 0。 | [空查询](../../../tmp/tutor-three-turn-20261018/run-4/ui-repair-6/empty-query.txt) |

### 第七至九轮：状态说明同步与最终成品

第七轮于 10:41:49–10:44:40 UTC 完成，`incomplete=false`，交付 revision 8。它把“v2（多一条，需重算）”改成“v2（多一条）”，明确两个准备控件分别针对当前目录和当前查询，切换输入不会自动翻转准备状态，并收紧 pending 说明。独立差异检查确认 HTML 只改文案，脚本只改两处提示字符串，计算路径保持不变；四处核心内容解释保留。证据：[输入与范围](../../../tmp/tutor-three-turn-20261018/run-4/turn-07-input.json)、[完成结果](../../../tmp/tutor-three-turn-20261018/run-4/turn-07-result.json)、[回答](../../../tmp/tutor-three-turn-20261018/run-4/turn-07-answer.md)、[revision 8](../../../tmp/tutor-three-turn-20261018/run-4/memory/agent-history.presentations/versions/presentation-1792316129531409900-15/8.json)。

revision 8 随后经过真实 Reader 操作：v2 标签和独立准备说明正确；查询设为“候选视图使用含义条件和别名”，目录 v2、文档待准备、查询已准备、top_k=1，刷新后保持调用 1/0/1、精确/别名 1、语义 1，以及相似度 0.29 的候选。点击来源打开 `10.9.7.2`、`.3` 与混合召回原文。证据：[版本与准备状态](../../../tmp/tutor-three-turn-20261018/run-4/ui-final/version-ready.txt)、[刷新前](../../../tmp/tutor-three-turn-20261018/run-4/ui-final/before-reload.txt)、[刷新后](../../../tmp/tutor-three-turn-20261018/run-4/ui-final/after-reload.txt)、[来源弹窗](../../../tmp/tutor-three-turn-20261018/run-4/ui-final/source-popover.txt)。

页面文案虽已准确，revision 8 的 `state_contract.catalog_version` 还保留“v2 需要重算”的旧描述。第八轮于 10:47:29–10:47:44 UTC 尝试只同步机器说明，最终 `incomplete=true`、`PRESENTATION_AUTHORING_FAILED`，没有交付新版本。前两次 read 没有目标引用；第三次已经带正确的 revision 8 引用，但指定 `file="state_contract"`，把默认 read 返回的元数据误当作 `content_files` 文件，导致 `Content file not found`。结果 trace 的公开参数投影省略了第三次的 reference/file，因此不能仅据 trace 判定三次都没引用。完整持久事件保留了真实调用，而当轮上下文本来就有已交付引用，最终“用户没有提供可读取对象”的断言不成立。证据：[第八轮边界](../../../tmp/tutor-three-turn-20261018/run-4/repair-purpose-8.json)、[完成结果](../../../tmp/tutor-three-turn-20261018/run-4/turn-08-result.json)、[错误答复](../../../tmp/tutor-three-turn-20261018/run-4/turn-08-answer.md)、[持久事件 seq 892](../../../tmp/tutor-three-turn-20261018/run-4/memory/agent-sessions/s_Y2hhdC0wMWExNGU1Ny02NTM0LTczYjItOGE0ZS03ZmM5YWM0NDlhZWE.jsonl)。

第九轮从真实 Reader UI 发送同一同步请求，原生 `presentation_follow_up` 携带同一对象 revision 8、state revision 8 和 `state-1792320594477316400-25` 保存凭据。该轮于 10:52:20 UTC 成功结束，`completed/saved`、`incomplete=false`，交付 revision 9。独立差异检查确认只修改 `state_contract` 的 `catalog_version`、`doc_prep`、`q_prep` 三项描述，HTML 与其余字段和 revision 8 逐字相同：目录选择不自动表示重算，两侧 prepared 分别表示按当前目录/查询已准备，pending 表示对应侧仍需准备。证据：[原生 UI 请求](../../../tmp/tutor-three-turn-20261018/run-4/turn-09-ui-request.json)、[任务状态](../../../tmp/tutor-three-turn-20261018/run-4/turn-09-run.json)、[结果](../../../tmp/tutor-three-turn-20261018/run-4/turn-09-result.json)、[回答](../../../tmp/tutor-three-turn-20261018/run-4/turn-09-answer.md)、[revision 9](../../../tmp/tutor-three-turn-20261018/run-4/memory/agent-history.presentations/versions/presentation-1792316129531409900-15/9.json)。

最终浏览器已真实打开 revision 9，并点击“五步路径”入口查看完整讲解。由于 HTML 和脚本与已验证的 revision 8 完全相同，前述刷新、来源和交互证据仍适用于最终页面，无需重复同一内容的全部操作。页面已标记为交付物，证据为 [最终界面全文](../../../tmp/tutor-three-turn-20261018/run-4/ui-final/final-revision-9.txt) 和 [五步路径截图](../../../tmp/tutor-three-turn-20261018/run-4/ui-final/final-five-step.png)。本次所有模型请求已结束，最后一项后台 profile review 也已返回成功；本地服务保留在 [http://127.0.0.1:1783](http://127.0.0.1:1783) 供用户继续操作。
