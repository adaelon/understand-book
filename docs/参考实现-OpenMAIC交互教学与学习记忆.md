# OpenMAIC 交互教学与学习记忆：源码索引

日期：2026-10-20。用途：为 [ADR-0172](adr/0172-interactive-teaching-state-and-learning-continuity.md) 与 [OM 切片方案](切片方案-交互教学与学习接续.md)提供可回读的外部实现依据。本页记录源码事实及采用判断；实施进度以切片方案为准。

## 1. 固定来源

- 原仓库：[THU-MAIC/OpenMAIC](https://github.com/THU-MAIC/OpenMAIC)。
- 本次分析提交：[aaca61742cad1287944edd24a2674c2949b9bcf0](https://github.com/THU-MAIC/OpenMAIC/tree/aaca61742cad1287944edd24a2674c2949b9bcf0)。
- 提交时间：2026-10-19T16:21:41+08:00；提交标题：`feat(editor): derive current course issues and thumbnail markers (#1903)`。
- 下文全部源码链接固定到该提交，行号与符号共同定位。后续上游变化须另记提交与差异，不能把本页结论直接移到新版本。
- 调研方式：读取源码、模板、调用链及相关测试；未启动 OpenMAIC，未执行其测试或真实教学实验。

本项目接入点依据 2026-10-20 工作树，HEAD 为 `33c19c7fa8aec9a4f1031122bc177743b234b9fd`，包含此前已有未提交修改。因此本项目位置以相对路径和符号定位，外部实现以固定提交永久链接定位。

## 2. 五点与采用范围

| 已接受方向 | 外部实现确实提供了什么 | 本项目采用方式 | 切片 |
| --- | --- | --- | --- |
| P1 教具报告真实当前状态 | 以 `summary/state/rendered` 表达课堂语义；发送聊天时重新采集当前场景 | 扩展既有 state reader 与 `PresentationState`，保留宿主可见文本和保存回执 | OM1 |
| P2 教师在同一教具上连续演示 | 从真实元素清单生成高亮、改状态、注释、揭示动作并播放 | 借鉴小动作与元素清单；自行补齐实时工具、执行回执和帮助归属 | OM4、OM5 |
| P3 区分提供支撑与要求判断 | 区分开放／封闭任务，提供操作方法和类比例子，结合当前任务理解短回复 | 改进现有教学指导、设计与审阅案例 | OM3 |
| P4 作品和评价进入学习接续 | 当前任务简报带入最新提交、评价与其他任务的既有作品 | 从 LearningMemory／TeachingDesign 重建有出处的接续上下文 | OM2、OM5 |
| P5 记忆调整支撑，明确请求优先 | 观察进入难度调整；显式难度请求有直接处理路径；模板与私人运行态分离 | 使用任务范围内的教学含义和用户请求；沿用现有私人存储边界 | OM3、OM5 |

下面编号 O、A、T、M、D 是本文的证据位置，供切片引用；它们不是拟新增的产品协议。

## 3. O：当前状态的语义观察

| 证据 | 固定源码与定位 | 实现事实 |
| --- | --- | --- |
| O1 | [interactive-observation.md L1–33](https://github.com/THU-MAIC/OpenMAIC/blob/aaca61742cad1287944edd24a2674c2949b9bcf0/packages/%40openmaic/generation/snippets/interactive-observation.md#L1-L33)，[异步结果 L82](https://github.com/THU-MAIC/OpenMAIC/blob/aaca61742cad1287944edd24a2674c2949b9bcf0/packages/%40openmaic/generation/snippets/interactive-observation.md#L82) | 生成页发布一份不可执行 JSON；`summary` 描述现场，`state` 表达学科对象和参数，只有输入与显示可能滞后时使用 `rendered`。未知值明确为未知；读状态不触发重算。 |
| O2 | [observation-bridge.ts L35–107](https://github.com/THU-MAIC/OpenMAIC/blob/aaca61742cad1287944edd24a2674c2949b9bcf0/lib/interactive/observation-bridge.ts#L35-L107)，`installObservationResponder`；[会话采集 L110–223](https://github.com/THU-MAIC/OpenMAIC/blob/aaca61742cad1287944edd24a2674c2949b9bcf0/lib/interactive/observation-bridge.ts#L110-L223) | 平台收集页面发布的数据，附上场景身份与请求时间；观察协议与页面自身交互代码分工。 |
| O3 | [use-chat-sessions.ts L349–394](https://github.com/THU-MAIC/OpenMAIC/blob/aaca61742cad1287944edd24a2674c2949b9bcf0/components/chat/use-chat-sessions.ts#L349-L394)，`runPiSingleRequest`；[interactive-state-evidence.ts L124–186](https://github.com/THU-MAIC/OpenMAIC/blob/aaca61742cad1287944edd24a2674c2949b9bcf0/lib/chat/pi/interactive-state-evidence.ts#L124-L186)，`attachInteractiveState` | 每次发送重新采集当前场景。状态随聊天请求进入模型证据；页面源代码与当前状态的身份和含义分开。该注入受课件引用配置控制。 |

采用的是“现场应表达什么”，不是其 DOM JSON 采集协议。本项目已有同步 `registerStateReader`、宿主 DOM 读取和持久回执，OM1 直接扩展这条链路。

## 4. A：小动作与同一教具演示

| 证据 | 固定源码与定位 | 实现事实 |
| --- | --- | --- |
| A1 | [interactive-actions/system.md L1–101](https://github.com/THU-MAIC/OpenMAIC/blob/aaca61742cad1287944edd24a2674c2949b9bcf0/packages/%40openmaic/generation/templates/interactive-actions/system.md#L1-L101) | 生成交替的讲解与动作序列，动作包括 `widget_highlight`、`widget_setState`、`widget_annotation`、`widget_reveal`；要求使用给定元素清单，教师稿不代写学生回答。 |
| A2 | [scene-generator.ts L1431–1533](https://github.com/THU-MAIC/OpenMAIC/blob/aaca61742cad1287944edd24a2674c2949b9bcf0/packages/%40openmaic/generation/src/scene-generator.ts#L1431-L1533)，`extractInteractiveElements`；[调用 L1825–1855](https://github.com/THU-MAIC/OpenMAIC/blob/aaca61742cad1287944edd24a2674c2949b9bcf0/packages/%40openmaic/generation/src/scene-generator.ts#L1825-L1855) | 从生成后的 HTML 提取真实元素标识、标签与控件信息，再交给动作生成；避免模型凭空猜测目标。 |
| A3 | [engine.ts L866–904](https://github.com/THU-MAIC/OpenMAIC/blob/aaca61742cad1287944edd24a2674c2949b9bcf0/lib/action/engine.ts#L866-L904)，`sendWidgetMessage` 与四个 `executeWidget*`；[action.ts L207–228](https://github.com/THU-MAIC/OpenMAIC/blob/aaca61742cad1287944edd24a2674c2949b9bcf0/packages/%40openmaic/dsl/src/action.ts#L207-L228) | 播放器把动作发往 widget，再等待短暂时延。该段没有“浏览器实际完成并保存结果”的返回合同。 |
| A4 | [classroom-actions.ts L270–345](https://github.com/THU-MAIC/OpenMAIC/blob/aaca61742cad1287944edd24a2674c2949b9bcf0/lib/chat/pi/tools/classroom-actions.ts#L270-L345)，`buildChildActionTools`；[tool-schemas.ts](https://github.com/THU-MAIC/OpenMAIC/blob/aaca61742cad1287944edd24a2674c2949b9bcf0/lib/chat/pi/tool-schemas.ts) | 实时 Pi 聊天工具另有白板、聚焦等动作；不能把预生成 widget 播放动作等同于实时聊天工具。 |

可以复用的思想是“一个小动作显现一个关系，继续使用同一个对象”。本项目的实时观察、执行、回执、继续讲解是新增设计，由 OM4／OM5 完成。

## 5. T：教学支撑与问题质量

| 证据 | 固定源码与定位 | 实现事实 |
| --- | --- | --- |
| T1 | [instructor-base-rules.md L55–76](https://github.com/THU-MAIC/OpenMAIC/blob/aaca61742cad1287944edd24a2674c2949b9bcf0/lib/pbl/v2/prompts/instructor-base-rules.md#L55-L76)，[短回复 L122–146](https://github.com/THU-MAIC/OpenMAIC/blob/aaca61742cad1287944edd24a2674c2949b9bcf0/lib/pbl/v2/prompts/instructor-base-rules.md#L122-L146) | 可运行代码／计算等任务与分析／写作／设计等任务采用不同支撑；短回复优先结合当前任务解释。 |
| T2 | [问题完整性与推进 L195–236](https://github.com/THU-MAIC/OpenMAIC/blob/aaca61742cad1287944edd24a2674c2949b9bcf0/lib/pbl/v2/prompts/instructor-base-rules.md#L195-L236) | 避免把多个都成立的角度设成二选一；教学聊天、提交、评价和平台推进各有职责。 |
| T3 | [tier-guidance.ts L36–63](https://github.com/THU-MAIC/OpenMAIC/blob/aaca61742cad1287944edd24a2674c2949b9bcf0/lib/pbl/v2/agents/tier-guidance.ts#L36-L63)；[instructor.ts L649–663](https://github.com/THU-MAIC/OpenMAIC/blob/aaca61742cad1287944edd24a2674c2949b9bcf0/lib/pbl/v2/agents/instructor.ts#L649-L663) | 提供“怎么做、在哪里做”和类比例子；阶段综合提问可选，已有回答不重复追问。部分分层规则还使用固定次数和固定节奏。 |
| T4 | [instructor-question-integrity.test.ts](https://github.com/THU-MAIC/OpenMAIC/blob/aaca61742cad1287944edd24a2674c2949b9bcf0/tests/pbl/v2/instructor-question-integrity.test.ts) | 测试约束了提示词中应包含的规则；这类文本断言不能单独证明模型遵守规则或学习效果改善。 |

本项目保留 ADR-0164 的情境判断；采用这些具体教学案例，不引入按出错次数机械释放答案的教学状态机。

## 6. M：作品、评价与对话接续

| 证据 | 固定源码与定位 | 实现事实 |
| --- | --- | --- |
| M1 | [instructor.ts L197–231](https://github.com/THU-MAIC/OpenMAIC/blob/aaca61742cad1287944edd24a2674c2949b9bcf0/lib/pbl/v2/agents/instructor.ts#L197-L231)，`buildTaskEvaluationBlock`、`buildSubmissionContextBlock` | 当前最新提交与评价一起进入上下文；评价早于最新提交时，不当作该提交已评价。 |
| M2 | [instructor.ts L234–321](https://github.com/THU-MAIC/OpenMAIC/blob/aaca61742cad1287944edd24a2674c2949b9bcf0/lib/pbl/v2/agents/instructor.ts#L234-L321)，`buildPriorSubmissionsBlock` | 其他任务每个取最近作品，保留作品摘要或正文片段及评价，按总预算裁剪并注明截断。 |
| M3 | [instructor.ts L498–539](https://github.com/THU-MAIC/OpenMAIC/blob/aaca61742cad1287944edd24a2674c2949b9bcf0/lib/pbl/v2/agents/instructor.ts#L498-L539)，`buildInstructorRuntimeBrief`；[注入 L803–875](https://github.com/THU-MAIC/OpenMAIC/blob/aaca61742cad1287944edd24a2674c2949b9bcf0/lib/pbl/v2/agents/instructor.ts#L803-L875) | 当前目标、位置、任务、提交评价和此前作品组织成当前教学简报，而非仅依赖聊天窗口。 |
| M4 | [instructor-memory.ts L1–138](https://github.com/THU-MAIC/OpenMAIC/blob/aaca61742cad1287944edd24a2674c2949b9bcf0/lib/pbl/v2/agents/instructor-memory.ts#L1-L138)，`compressIfNeeded`；[历史输入 L882–902](https://github.com/THU-MAIC/OpenMAIC/blob/aaca61742cad1287944edd24a2674c2949b9bcf0/lib/pbl/v2/agents/instructor.ts#L882-L902) | 压缩器从文本模式提取环境、偏好、困难和进展片段，保留有界摘要；教师输入使用可选旧摘要和最近 24 条消息。 |
| M5 | [压缩调用 L1732–1745](https://github.com/THU-MAIC/OpenMAIC/blob/aaca61742cad1287944edd24a2674c2949b9bcf0/lib/pbl/v2/agents/instructor.ts#L1732-L1745)；[route.ts L37–76](https://github.com/THU-MAIC/OpenMAIC/blob/aaca61742cad1287944edd24a2674c2949b9bcf0/app/api/pbl/v2/instructor/route.ts#L37-L76)；[use-instructor-stream.ts L137–172](https://github.com/THU-MAIC/OpenMAIC/blob/aaca61742cad1287944edd24a2674c2949b9bcf0/components/scene-renderers/pbl/v2/use-instructor-stream.ts#L137-L172)；[learner-state.ts L36–42](https://github.com/THU-MAIC/OpenMAIC/blob/aaca61742cad1287944edd24a2674c2949b9bcf0/lib/pbl/v2/runtime/learner-state.ts#L36-L42) | 压缩发生在一次服务端请求的局部对象上。源码明确注明尚无持久 reducer／patch 回写线程截断和 `earlierSummary`。 |

OM2 主要采用 M1–M3 的上下文组织方式。持久化、修订、原话和帮助依据继续使用本项目 LearningMemory；M4 的规则摘要不承担新的事实权威。

## 7. D：状态影响支撑与私人归属

| 证据 | 固定源码与定位 | 实现事实 |
| --- | --- | --- |
| D1 | [instructor.ts L1416–1483](https://github.com/THU-MAIC/OpenMAIC/blob/aaca61742cad1287944edd24a2674c2949b9bcf0/lib/pbl/v2/agents/instructor.ts#L1416-L1483)，`record_observation`、`adjust_difficulty`；[dynamic-signals.ts L171–223](https://github.com/THU-MAIC/OpenMAIC/blob/aaca61742cad1287944edd24a2674c2949b9bcf0/lib/pbl/v2/operations/runtime/dynamic-signals.ts#L171-L223)，`applyProficiencyDirective` | 教学观察可改变支撑状态；用户明确提出难度方向时走直接调整，包括重置相关计数。 |
| D2 | [proficiency.ts L484–520](https://github.com/THU-MAIC/OpenMAIC/blob/aaca61742cad1287944edd24a2674c2949b9bcf0/packages/%40openmaic/generation/src/pbl/operations/kernel/proficiency.ts#L484-L520)，`signalFromObservation`；[schemas.ts L62–91](https://github.com/THU-MAIC/OpenMAIC/blob/aaca61742cad1287944edd24a2674c2949b9bcf0/lib/pbl/v2/operations/runtime/schemas.ts#L62-L91) | `question` 被映射为负向 proficiency 信号；当前观察工具接受 `error/struggle/question`。 |
| D3 | [learner-state.ts L84–177](https://github.com/THU-MAIC/OpenMAIC/blob/aaca61742cad1287944edd24a2674c2949b9bcf0/lib/pbl/v2/runtime/learner-state.ts#L84-L177)，`extractLearnerState`、`stripToDesignTemplate`；[回填 L180–245](https://github.com/THU-MAIC/OpenMAIC/blob/aaca61742cad1287944edd24a2674c2949b9bcf0/lib/pbl/v2/runtime/learner-state.ts#L180-L245) | 提交、评价、线程、阶段状态和 proficiency 从教学模板剥离，恢复时重新组合私人运行态。 |
| D4 | [drain.ts](https://github.com/THU-MAIC/OpenMAIC/blob/aaca61742cad1287944edd24a2674c2949b9bcf0/lib/pbl/v2/runtime/drain.ts)；[fold.ts](https://github.com/THU-MAIC/OpenMAIC/blob/aaca61742cad1287944edd24a2674c2949b9bcf0/lib/pbl/v2/runtime/fold.ts)；[hydration.ts L234–334](https://github.com/THU-MAIC/OpenMAIC/blob/aaca61742cad1287944edd24a2674c2949b9bcf0/lib/pbl/v2/runtime/hydration.ts#L234-L334)，`hydratePBLProjectFromRuntime`；[storage/runtime/types.ts](https://github.com/THU-MAIC/OpenMAIC/blob/aaca61742cad1287944edd24a2674c2949b9bcf0/packages/%40openmaic/storage/src/runtime/types.ts) | 运行事件按课堂／场景／学习者写入运行存储，再折叠成可恢复状态；这是与 M5 的特定摘要缺口不同的一条已存在持久化链路。 |

本项目采用“记忆必须影响接下来的支撑”，不把提问次数转换成整体能力等级。公共编排／私人学习工作区的分离由 ADR-0170 已经确定。

## 8. 已知问题与参考限制

1. **预生成动作与实时操作是不同链路。** A1–A3 能证明动作生成与播放，不能证明实时教师已具备“观察 → 操作 → 确认 → 再观察”的完整工具循环。OM4／OM5 的回执是本项目新增要求。
2. **对话压缩尚不能保证跨请求接续。** M4–M5 的调用顺序和状态回写缺口意味着，最近消息窗口以外的自由对话不能仅凭该摘要机制保证再次进入模型上下文。原运行记录仍可能保存着这些消息；不能据此声称所有历史已丢失。
3. **提问与能力弱不等价。** D2 确实把 `question` 作为负向信号；我们的设计只从具体表达、帮助和结果推导任务内的教学含义。
4. **部分教学提示与工具存在不一致。** T3 的 beginner 指导第 6 条要求记录 `concept_unlocked` 并 advance；D1／D2 的当前观察工具枚举不包含它，常规教学回合的推进规则也另有约束。采用教学原则时必须重新对齐实际工具。
5. **源码机制不能证明教学收益。** 上游提示词测试、代码中的恢复路径以及本次静态分析，不构成真实学习效果或本项目集成完成的证据。OM6 分别记录工程结果、实际模型行为和后续真人使用证据。

## 9. 后续 Agent 的回读顺序

先读 ADR-0172 与切片方案中本片合同；随后按 P1→O、P2→A、P3→T、P4→M、P5→D 找原实现。涉及持久化时同时读 M5 与 D3–D4，涉及实时动作时同时读 A3–A4，避免只读一个功能函数就推断整条链路成立。

需要在本地阅读时，使用仓库的独立参考目录并检出上述确切提交；`git show aaca61742cad1287944edd24a2674c2949b9bcf0:<仓库相对路径>` 可读取指定版本。临时调研目录不是交付依赖，以上永久链接和路径足以重新取得证据。
