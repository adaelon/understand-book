# 章节导读切片 A 实现与验证

日期：2026-10-17。切片 A 接通章节入口、章范围持久化和第一个问题的选择。真实首轮使用项目书第六章完整原文，三个读者起点分别产生了有来源、与所选问题相符的交付。完整方案见[章节导读设计方案](../../设计方案-章节导读的问题选择与理解进展.md)。

## 入口与章节范围

Reader 顶栏显示规范目录中的实际章名。Tutor 关闭时，按钮明确显示“开启学习并带我读这一章”；点击后依次开启学习、开始章会话并提交一次请求。同章 active 会话复用，同章 paused 会话 Resume，换章使用已有 Start。共用的 `startTutorRequest` 承接启动、刷新和唯一提交；生命周期失败时保留现有错误反馈。

`GuideChapter` 请求携带 canonical `chapter_lid` 和来源版本。服务端从原文解析实际章名，生成公开请求“带我读《实际章名》这一章”。章容器只作为定位范围，实际教学目标和证据由本轮原文读取建立。章入口不会被同章的待答活动误认成选区求助。

`TutorSession.material_scope[].scope_refs` 保存规范章 LID，`freeze_prepare` 在 `TurnBound` 前确定会话；冻结上下文提供 `material_scope` 和派生的 `chapter_scope`。已开启 Tutor 且会话未暂停时，自然语言“带我读这一章”优先从本次请求已有的阅读位置确定章节，缺失时才回退到冻结的 Reader 视口。关闭或暂停时，普通章请求继续作为普通导读。之后移动视口再说“继续”，仍使用原章范围。独立导入的第六章其规范 LID 是 `1`，显示章号 `6` 不参与寻址。

浏览器实际阅读位置与服务端阅读窗口中心可以不同。例如短章跳转后，窗口因已到末尾不能再向下移动，但浏览器已定位第二章。新回归保留这两个不同位置，验证带入场位置时选第二章、缺失时按窗口位置选择第一章。

本轮回归还发现并修复了真实续接缺口：首个问题完成会关闭对应 ResidentGoal，而章 TutorSession 仍 Active。原来的精确“继续”因此可能返回 `GOAL_NOT_FOUND`。现在，同一章会话、上一轮有同一教学绑定且当前没有未完成 Goal 时，沿既有 Completed Goal 分支创建关联的新 Goal。普通已完成任务、不同会话，以及用户显式指定的目标保持原有受理规则。

实现接点：[App.vue](../../../packages/web/src/App.vue)、[TopBar.vue](../../../packages/web/src/components/TopBar.vue)、[interaction_input.rs](../../../crates/server/src/interaction_input.rs)、[teaching.rs](../../../crates/server/src/teaching.rs)、[lib.rs](../../../crates/server/src/lib.rs)、[tutor_continuation.rs](../../../crates/runtime/src/tutor_continuation.rs)。

## 选题怎样进入实际交付

同一 Resident 根据读者用途、已有连接和具体困惑，选择在该章有来源支撑、当前可以推进的问题。结构和 CognitiveMaterials 提供定位；确定设计前读取原文。未知背景允许从具体、可修正的入口开始。

问题及其此刻的用途进入现有 `organizing_model`；关系块的 `purpose` 表达学会它能做什么，`focus_block_refs` 和局部 `expected_performance` 限定本轮工作，`basis_refs` 指向实际读者表达。适用的设计和关系块继续复用。章级定位与本轮已读的局部 `source_bindings` 分别保留。

SELECT 要求问题实际决定讲解关系、例子和停止点。现有独立内容接纳调用同时收到章范围、已接纳设计和拟交付正文，可以退回“设计选了一个问题，正文却铺开无关主题”的具体不符。最终仍只交付获接纳的正文。章节入口保持 neutral，教师按需要选择 support、invite 或 conclude。

导航指导与这条路径对齐：全书路线中的候选需按冻结章节选择；对当前关系补充支持或结束时无需额外跳转。若选择新的停点，仍先读准确 LID，再执行真实跳转。未浏览的前置章节不被当作理解缺失。

实现接点：[tutor.rs](../../../crates/runtime/src/tutor.rs)、[agent_prompt.rs](../../../crates/runtime/src/agent_prompt.rs)、[orchestrator.rs](../../../crates/runtime/src/orchestrator.rs)。本轮指导版本为 `teaching-design.v7`、`teaching-delivery.v4`、navigation `v6`。

## 工程回归

验证针对实际失败路径：入口重复发请求、生命周期失败后仍提交、章身份错误、视口变化或重开使范围漂移、完成首问后无法继续、未接纳正文泄漏，以及新增入口改变既有回答操作。

| 检查 | 结果 |
| --- | --- |
| 新增前端章节入口场景 | 7 通过，覆盖 off、active、paused、换章、三类失败及连续点击 |
| App.startup 与 TopBar | 84 通过，1 个原有来源定位失败，见已知问题 |
| 前端 typecheck | 通过 |
| 实际浏览器桌面与窄屏 | 2 通过，1440×900 与 390×900 |
| Runtime teaching_ | 17 通过 |
| Runtime tutor_continuation_tests | 6 通过 |
| Runtime guided_read | 11 通过，1 个 opt-in 实验忽略 |
| Runtime agent_prompt | 18 通过 |
| Server teaching_design_tests | 19 通过，含 5 项新增章节回归 |
| Server tutor_continuation_tests | 62 通过，8 个 opt-in 实验忽略 |
| Server 原有显式继续与目标保持回归 | 1 通过 |

Runtime 的退回修订回归确认内容接纳收到选题、块用途与章节范围，拒绝的正文不会保存或进入公开流，最终只交付修订后的选择。章节动作合同覆盖 neutral 以及用户补充文字优先。

服务端 5 项章节回归覆盖章身份与版本、范围持久化及数据库重开、视口移动后继续、明确换章与同章复用、关闭及暂停状态、普通任务不被章续接接管、浏览器入场位置优先。邻近设计与回答操作回归也全部通过。可复现命令和结果保存在[regression-results.json](regression-results.json)。

浏览器使用实际 App 和现有受控接口，确认按钮完整显示实际章名、可点击、一次启用及 Start 后只有一个 `guide_chapter` 请求，并保留来源和对话身份。截图已实际查看：[桌面入口](ui-entry-1440.png)、[窄屏入口](ui-entry-390.png)、[完整 UI 记录](ui-results.json)。

## 真实章节首轮

材料为[项目书第六章完整原文](../../project-book/chapters/06-全书结构与语义候选召回.md)，通过正式 Markdown 导入和分段流程生成 102 个规范节点。每个场景使用独立新会话，三个教师均为 gpt-6.1-sol；另一个独立 gpt-6.1-sol 执行真实 structured completion，包括现有内容接纳。工具执行、设计消费、来源检查、持久化和最终交付经过真实服务路径。

| 读者起点 | 已接纳的选题 | 最终交付如何兑现 | 参与方式 |
| --- | --- | --- | --- |
| 首次接触，只知道系统会做摘要 | 既然能做摘要，为什么还保留候选 | 使用原文三个缓存学习点，对照全部保留与路线只突出两个，解释省略条件为何仍要可找回 | support |
| 已区分保留与路线，想知道全书结构的用途 | 已有目录和概念图，全书结构增加什么阅读判断 | 连接位置、关系和学习作用，用定义、失效条件、应用三个位置解释为何选择下一处，并留一次阅读目标变化后的选择 | invite |
| 卡在未被章节接受的候选与主题召回 | 章节未选为何仍可被主题选中，检索命中是否等于支持主题 | 沿同一个假设候选，区分目录保留、检索预览、完整材料交付和有依据的主题选择 | support |

三个最终回答均有一份已保存教学交付、三组公开来源绑定，且其设计引用与内容接纳包中的已接纳设计一致。实际首答分别见[首次接触](first-contact/answer-01.md)、[已有基础](existing-basis/answer-01.md)、[具体困惑](specific-confusion/answer-01.md)。`answer-01.md` 保存正文；完整来源部件和绑定在各场景的 `turn-01.json`、`result.json`。

内容与原文核对的依据分别为：摘要逐层丢失条件和缓存选择表；目录、概念图与学习角色的分工；搜索预览与后续交付记录的区别，以及主题从完整候选目录补入重点的机制。三份交付都围绕所选关系停下。已有基础场景末尾的小应用可以收集下一次表达；它尚未收到读者回答，也不能单独承担独立迁移测量。

| 场景 | Resident 调用 | structured 调用 | 其中内容接纳 | 总调用 | 本地首答墙钟 |
| --- | ---: | ---: | ---: | ---: | ---: |
| 首次接触 | 9 | 3 | 1 | 12 | 390.450 秒 |
| 已有基础 | 7 | 1 | 1 | 8 | 288.744 秒 |
| 具体困惑 | 14 | 3 | 1 | 17 | 382.333 秒 |

三个内容接纳均一次接受。首次接触和具体困惑另外各有两次 `book.query` 内部 structured 调用。完整用量字段见[live-results.json](live-results.json)；[summarize.mjs](summarize.mjs)从实际接纳包和保存交付提取结果，[protocol.md](protocol.md)保存复现方式。

## 已知问题与验证边界

前端原有 `MCP7 opens exact UTF-16 evidence in canonical text without saving a highlight` 失败，显示“来源定位未完成，请重试”。使用本轮保存的改动前 App 和测试副本复现了相同失败。此次章入口和按钮回归均通过。

三轮输入是预设读者起点，使用真实生成模型作答；目前没有真人回应、后续应用或保持数据。它们支持“问题选择进入设计并改变交付”的结论。理解进展仍需按原方案保留读者原先的解释、实际帮助和之后未被代答的判断。

真实首轮使用无 BookStructure sidecar 的原文局部导读路径；已有结构路线由工程回归覆盖。浏览器验证使用受控接口，教学内容由独立真实服务实验验证。

墙钟包含文件通道和子代理轮询等待，不能当作线上直接 API 首答延迟。适配器未提供 provider tokens；保存的 195332、146678、369896 是主机估算值，不能据此计算实际费用。一次全包测试编译因 Windows 页面文件不足失败，后续验证限定相关库并串行编译。
