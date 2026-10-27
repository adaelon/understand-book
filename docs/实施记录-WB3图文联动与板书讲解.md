# WB3：图文联动与板书讲解

日期：2026-10-25。状态：WB3 已实现并验收。分层回归、生产构建及真实网络页面完整流程通过；桌面 1500 × 1050 与窄屏 390 × 844 的实际截图已检查。WB4 的运行中正式局部交付尚未实施。

本记录承接 [WB2 同板追问](实施记录-WB2画板追问.md)，范围依据[完整设计 §13](设计方案-知识画板与持续讲解.md#13-分阶段实现与交付)。WB3 将正式保留的回答、图元与演示区域建立双向对应，并用可保存的讲解路线组织逐步绘制、回退与重开。生成过程中的正式局部交付仍由 WB4 单独实现。

## 1. 本轮实现范围

读者可从一段解释定位到它对应的图元，也可从图元回到完整解释。讲解按已保存步骤展开，支持暂停、上一步、下一步与全图；用户手动探索后保持当前步骤，显式继续跟随。演示继续保留确切版本和实验现场，放大、收起和跨聊天操作沿原演示执行与回执链路完成。

Agent 新制作的原生板书通过 `board.author` 制作、观察和暂存，最终成功回答提交时进入画板。图示中的用户原话、系统理解判断、预构建条件和 Agent 解释保留各自来源身份。画板留存目标也已接入 `presentation.interact`，当前聊天可以操作另一聊天留下的确切演示版本。

## 2. 已实现的持久合同

权威类型位于 [memory/boards.rs](../crates/memory/src/boards.rs)，Web 类型通过既有 Rust → TypeScript 导出流程生成到 [generated](../packages/web/src/generated)。

| 模型 | 保存的内容 |
| --- | --- |
| `BoardExplanationBlock` | 稳定 `block_id`、Markdown 正文及完整 `answer_parts` |
| `BoardContentRef` | 条目、可选解释块及确切 `content_revision` |
| `BoardTarget::Drawing` | 原生 `graphic`、产生它的 session/turn 及 `based_on` 来源引用 |
| `BoardGraphic` | 文字、带局部坐标点列的路径、矩形或椭圆形状及文字 |
| `BoardVisualTarget` | 条目、连线，或确切演示版本的声明区域 |
| `BoardLink` | 稳定连线身份、两端目标、关系文字及来源 |
| `BoardExplanationMapping` | 稳定映射身份、解释引用与视觉目标 |
| `BoardRoute` / `BoardRouteStep` | 稳定路线与步骤身份、解释、揭示目标、焦点目标、过渡方式和时长 |
| `BoardPlaybackState` | 路线、当前步骤、是否跟随及是否显示全图 |

`BoardDocument` 新增 `links`、`mappings`、`routes`；`BoardItem` 新增 `explanation_blocks`。已有条目的整答 `block_id` 保留，WB1、WB2 的聊天入口、深链和历史回执继续引用它。细块由内容制作时明确提供稳定身份，数组顺序仅表示阅读顺序。

`resolve_explanation_block` 同时解析整答和细块。旧回答缺少显式整答块身份时，仍根据稳定条目身份补足原有地址；旧文档缺少新增集合时按空集合读取。没有对旧正文重新拆段，也没有用 Markdown 字符位置或 DOM 顺序生成长期地址。

路径点位位于条目内部坐标系，条目的 `position/size` 继续表示画板中的位置与尺寸。图文映射引用内容身份，移动条目不会改写解释来源。

## 3. 提交、冲突与删除

[runtime/board_author.rs](../crates/runtime/src/board_author.rs) 定义 `board.author` 合同，[server/board_author.rs](../crates/server/src/board_author.rs) 在当前已受理的画板追问范围内执行。候选随原回合的 domain 保存，不由浏览器另行导入。

| 操作 | 实际行为 |
| --- | --- |
| `read` | 读取当前画板、保留来源和候选；返回预留答案身份、建议位置、本轮真实理解条目、确切理解引用及正式条件引用 |
| `prepare` | 比较基准修订；用原回答编译器编译完整解释及各个细块；构造完整画板投影并校验来源、图文引用和声明区域；保存新的候选修订 |
| `preview` | 使用真实浏览器加载 Reader 的 `BoardCanvas`，返回截图、实际几何和布局问题；成功观察绑定确切候选修订 |
| `deliver` | 仅将已观察的候选标记为待交付；最终正式回答成功后才提交画板内容 |

`read` 的大结果按完整记录分页，并返回可实际调用的 `next_calls`。`collection=layout` 单独提供条目身份、位置和尺寸，长回答不会挡住已有图元的布局；条目正文、连线、映射、路线及候选内容可分别读取。分页绑定 `board_revision`，候选分页还绑定候选修订；用户在中途改变内容时重新读取，不能拼接不同版本。这个入口修复了真实九项画板被通用结果投影裁成五项、却没有继续读取办法的问题。

每次重新 `prepare` 都使原观察和待交付资格失效。运行时要求预览图像进入后续模型采样后才可交付；被图像预算省略的观察不算已看见。已有教学选择和已采纳教学正文同样约束原生解释，旧候选不能替代后来改变的教学目标或绕过正文检查。最终已编译正文与候选解释不一致时，候选不发布。

[board_preview.rs](../crates/server/src/board_preview.rs) 复用现有真实浏览器观察能力；[board-preview.ts](../packages/web/src/board-preview.ts) 直接挂载正式 [BoardCanvas.vue](../packages/web/src/components/BoardCanvas.vue)。预览检查本次新增图元的渲染尺寸、文字裁切、路径越界和内容重叠，并提供截图供模型检查。Reader 构建产出 `board-preview/renderer.js` 与样式，桌面使用打包资源，网络服务可通过 `UNDERSTAND_BOOK_WEB_DIST` 指定构建目录；缺少这些资源时明确返回预览不可用。

`BoardStore::append_authored_delivery` 接收已解析的回答、图元、连线、映射和路线。它先按原 session/turn 查找既有交付回执；同次重试返回原结果。新提交严格比较预览使用的画板基准修订，保留传入的已预览位置，不重新排版用户已有内容。

正式内容和 `BoardDelivery` 写入同一个不可变修订快照，再更新当前修订指针。用户移动过条目或其他内容已更新时，旧候选返回冲突，制作流程需要基于当前画板重新投影与观察。

普通 WB2 回答追加保留原行为。布局重放只有在中间修订未改变原有内容、布局和图文结构时成立；旧布局请求不能覆盖后来新增的连线、映射或路线。普通移动与改标题不能改写已留存的细块、图元正文或关系文字。

`validate_candidate` 检查稳定身份、确切历史内容引用、解释块和视觉目标、演示版本，以及路径的有限坐标和正线宽。服务端另外读取对应演示版本，确认映射区域来自正式声明。

移除条目会清理失去端点的连线、相关映射和路线目标，并清理无剩余内容的步骤与路线。来自历史快照的 `based_on` 引用继续作为来源保留，交付回执也继续保留；重试不会重建用户已经移除的内容。

## 4. 图文定位、播放与重开

[BoardWorkspace.vue](../packages/web/src/components/BoardWorkspace.vue) 根据持久映射组织双向定位。点击图元或连线时，找到对应解释；有多条解释时列出供选择。点击解释块的定位入口时，高亮对应图元并定位画面；涉及演示区域时转交已打开的演示实例。画布所选对象与正在阅读的解释分别保存在 `selected_item_id` 和 `reading_item_id`，避免图文来回定位时互相覆盖。

[BoardReadingPane.vue](../packages/web/src/components/BoardReadingPane.vue) 按显式块身份呈现完整正文与来源，保留整答入口。播放驱动的文字定位不会反复抢走键盘焦点。`AgentPresentation` 的区域桥接只接受当前确切版本声明的区域；区域点击可回到对应解释，解释也可定位区域。区域聚焦与实验参数操作分别执行，定位不会恢复提问时的旧实验现场。

[board-workspace.ts](../packages/web/src/board-workspace.ts) 的 `projectBoardRoute` 从路线开始到当前步骤计算已揭示集合；上一步和跳转步骤不需要重放一串有副作用的操作。原生路径、语义连线和短文字按步骤在本地显示，暂停、继续、上一步、下一步和全图均不调用模型；减少动态效果设置直接呈现步骤完成态。

用户平移、缩放、拖动图元或主动定位内容时，停止自动播放与镜头跟随，保留当前步骤。“继续跟随”回到该步骤的讲解焦点。显式定位按目标范围与可视区域选择阅读缩放：单个目标尽量以原尺寸呈现，较大的目标缩小以完整适配；全图查看后不把过小的缩放沿用到段落定位。普通选择、高亮和手动探索不自动改变缩放。显示全图只改变展示状态，不删除路线，也不改写布局。已打开的演示实例在面板切换中保留，演示输入和已保存现场沿原宿主管理。

`BoardView` 新增 `selected_block_id`、`reading_item_id` 和 `playback`。这些字段与镜头、演示现场一起保存在独立视图文件中，保存步骤不增加内容修订。

播放状态只保存稳定步骤，不保存逐帧时间和运行中的定时器。重开停在该步骤完成态，由用户继续播放。相机或演示现场保存合并已有视图字段，不将未发送的阅读位置或播放状态清空。

读取视图时会移除已经不存在的条目、解释块或路线选择。全文搜索命中显式细块时返回其稳定 `block_id`；旧整答入口仍可读取完整正文。理解原话和系统解释继续沿原有分字段检索合同处理。

## 5. 操作画板留存的演示

`presentation.interact` 的 `observe/act` 新增 `board_target`，包含画板、条目和确切画板修订。服务端从该引用解析原演示 owner 与 `PresentationRef`，不要求当前聊天就是原内容聊天，也不要求原聊天记录仍存在。当前回合负责本次运行和教师操作事实，原演示继续拥有内容及实验现场。

[presentation_interact.rs](../crates/server/src/presentation_interact.rs) 复用原命令投递、参数冲突、取消和完成回执机制。Agent 先观察当前真正打开的页面，再使用该观察的 `saved_state_ref` 执行声明过的动作。只有 `applied` 与已保存回执确认操作完成；冲突、超时或取消后重新观察，不自动重试操作。现场保存更新对应画板的演示回执，教师帮助仍写入原教学事实链，不计作用户答案或独立掌握证据。

目标解析检查私人作用域、条目存在和确切内容版本。移除目标、切换为其他版本或使用其他学习工作区的引用均不能作用到替代页面。这个入口授予原版本的声明动作执行能力，不授予跨聊天改写演示内容的能力。

## 6. 本轮理解、跨轮纠正与正式条件

[board_understanding.rs](../crates/server/src/board_understanding.rs) 从已有 LearningEvidence 和原始教学表达事实生成理解条目，不接受模型编写的“用户原话”。它以实际表达的聊天 session/turn 确认本轮归属；接口操作类回答不作为学习表达，评分反馈的隐藏边界沿用现有 evidence view。

本轮也可以纠正以前一轮留下的理解。系统在接受解释时通过 [teaching.rs](../crates/server/src/teaching.rs) 保存 `EvidenceInterpreted` 事实，绑定当前回合及接受的 evidence。画板只在这个系统事实与来源、Tutor 会话和证据身份相符时纳入旧表达的新解释。原话仍保留原发生时间和身份，`supersedes` 表达解释之间的纠正关系；旧解释记录保持不变，不把系统的新判断写成用户刚刚说过的话。

理解条目按 `target.evidence_id` 去重。用户先手动导入同一证据后再触发自动归板时，复用已存在的实际条目身份。`board.author.read` 返回的 `understanding_refs` 因而区分已有内容的当前修订与待新增内容的下一修订，后续映射不用猜测自动生成的 item ID。

正式条件通过 `formal_condition_refs` 单独引用预构建对象，核对来源、来源版本、对象身份与修订、教学图版本。画板可将用户表达、系统解释与该条件并列比较，但理解判断不会改写正式知识关系，图形本身也不产生掌握结论。

普通问答的 observing 模式可依据本轮实际原文和用户表达形成 source-only hypothesis；没有正式对象引用时，不自动推断 `formal_condition_refs`。Agent 仍可读取画板上已保留的预构建对象，以两份独立的精确 `based_on` 引用制作对照。保留快照不因此获得当前 Tutor material 的权限。

## 7. 维护入口

| 文件 | 职责 |
| --- | --- |
| [memory/boards.rs](../crates/memory/src/boards.rs) | 权威持久类型、原子内容提交、引用校验、细块搜索与视图状态 |
| [runtime/board_author.rs](../crates/runtime/src/board_author.rs)、[server/board_author.rs](../crates/server/src/board_author.rs) | 原生制作合同、编译、候选保存、观察和最终交付条件 |
| [board_preview.rs](../crates/server/src/board_preview.rs)、[board-preview.ts](../packages/web/src/board-preview.ts) | 同一 BoardCanvas 的真实浏览器观察与资源装配 |
| [board_api.rs](../crates/server/src/board_api.rs)、[board_delivery.rs](../crates/server/src/board_delivery.rs) | 冻结目标、正式答案归板、留存演示解析及交付恢复 |
| [board_understanding.rs](../crates/server/src/board_understanding.rs)、[teaching.rs](../crates/server/src/teaching.rs) | 真实理解证据、跨轮解释接受事实、正式条件与去重 |
| [presentation_interact.rs](../crates/server/src/presentation_interact.rs) | 画板演示 observe/act、当前运行归属和真实保存回执 |
| [BoardWorkspace.vue](../packages/web/src/components/BoardWorkspace.vue)、[BoardReadingPane.vue](../packages/web/src/components/BoardReadingPane.vue)、[BoardCanvas.vue](../packages/web/src/components/BoardCanvas.vue) | 图文定位、阅读、路线播放、用户探索与渲染 |

## 8. 当前验证证据

| 层次 | 当前结果 | 已验证范围 |
| --- | --- | --- |
| memory 画板 | **47/47 通过**：23 项行为测试、24 项类型导出测试 | WB1/WB2 存储回归和 WB3 持久合同 |
| WB3 新增存储行为 | **7/7 通过**，包含在上述总数中 | 见下表 |
| server 最终筛选 | **41 通过、0 失败、4 个长驻宿主 ignored** | 制作候选、布局分页、小数路径保存、正式提交、留存演示、真实理解、跨轮纠正及 WB1/WB2 回归 |
| 前端定向测试 | **8 个文件、203/203 通过** | App、画板渲染与工作区、原生文字换行、显式定位可读缩放、演示现场和相关接线 |
| runtime interact | **6/6 通过** | 演示操作合同及相关类型导出 |
| runtime 制作交付约束 | **2/2 通过** | 教学选择变化后的旧候选失效、采纳正文与原生交付一致 |
| runtime 小数几何与完整分页 | **2/2 通过**；与上述运行时检查合计 10/10 | 实际 JSON 小数几何；真实结果预算投影后追回全部九条记录、长回答中的完整布局，以及版本一致性 |
| Web 生产构建 | **通过** | Vue/TypeScript 检查、Reader 和原生预览资源构建 |
| 真实浏览器与视觉验收 | **完整流程 1/1 通过，约 1.2 分钟** | 两次真实候选观察、图文与演示区域联动、播放与重开、删除原聊天后的操作、新理解对照及桌面和窄屏渲染 |

新增测试位于 [boards/wb3_tests.rs](../crates/memory/src/boards/wb3_tests.rs)：

| 场景 | 检查的结果 |
| --- | --- |
| 图文与播放重开 | 图文内容、整答地址、细块地址、播放步骤和全文命中均保持 |
| 预览之后用户移动 | 旧候选拒绝提交；以新修订提交时保留用户位置 |
| 移除解释和图元 | 连线、映射、路线及失效选择清理，回执重试不重建内容 |
| 删除原来源条目 | 当前连线清理，历史来源仍能从原修订读取 |
| 非法引用和路径 | 错误块、内容修订、视觉目标或非有限路径不发布部分内容 |
| 普通布局编辑 | 不能改写细块和关系；正常移动保留图文合同 |
| WB1/WB2 旧文件 | 缺少新增字段时仍能读取整答、空图文集合和默认播放状态 |

存储执行入口为 `cargo test -p memory boards:: --no-fail-fast`。最终使用 `cargo test -p server -p runtime -p memory --lib --no-run` 统一编译，再运行对应测试二进制中的定向用例。本轮使用工作区磁盘上的隔离临时目录。已有分层证据见[存储测试](../tmp/board-wb3/memory-tests-accepted.log)、[前端测试](../tmp/board-wb3/web-tests-accepted.log)、[服务端筛选](../tmp/board-wb3/server-tests-accepted.log)、[运行时测试](../tmp/board-wb3/runtime-tests-accepted.log)及[生产构建](../tmp/board-wb3/web-build-accepted.log)。

真实请求曾暴露小数坐标的解析失败：工作区启用 `serde_json/arbitrary_precision` 后，内部标记枚举的暂存层不能直接交给普通 `f64` 读取。修复沿用现有演示时间字段的 `Value → as_f64` 做法，覆盖画板的七个几何数字字段。回归保留小数输入，验证实际 `prepare`、私有候选重读和正式快照重开；没有把夹具改成整数绕过问题。新增跨轮纠正测试也已通过。

最终 203 项前端检查包含卡片来源标记显示和显式定位缩放修正。390px 窄屏中，290px 图元从全图的 25% 恢复为可读的 100%；普通选择、高亮不移动镜头，手动平移保持用户缩放，超大目标按可视区域适配。

实际浏览器入口为 [board-wb3.spec.ts](../packages/web/playwright/board-wb3.spec.ts) 与 [board_wb3_browser_host.rs](../crates/server/src/tests/board_wb3_browser_host.rs)。最终[运行日志](../tmp/board-wb3/browser-validation.log)与[结构化结果](../tmp/board-wb3/browser/result.json)保存以下完整流程证据：

- 原生讲解和理解对照两次候选均由真实 BoardCanvas 预览，返回 `preview_ready_for_inspection`、零布局问题；紧接着的模型采样各收到一张实际截图。正式内容在成功最终回答后提交，内容修订从 5 经原生交付变为 6，经跨聊天回答与理解对照变为 8，最终保留 15 个条目。
- 三个真实回合分别完整读回 5、9、10 项布局；后两轮均包含此前全部三个原生图元，后续绘图依据已读布局和答案范围放置。
- 文字与图元双向定位、逐步播放、暂停、前进与回退、手动探索后继续跟随、全图和重开均通过；本地播放期间模型请求数与内容修订保持不变。
- 演示参数改为 1.5 后，展开、收起与切换面板保留原 iframe 和输入。通过实际历史菜单删除原聊天，再在新聊天中观察留存演示、将参数改为 2 并执行比较，页面显示 2.40 m/s；`observed`、`applied` 回执保留原演示 owner，新回答归属当前聊天。
- 本轮新表达经实际教学事实链形成 `interpretation:wb3-new-understanding` 的 hypothesis，再与精确预构建条件共同引用，用户原话与系统解释分别保留。新理解的浏览器验证与跨轮纠正的服务端回归分别记录，不将两者混为同一场景。
- 等待阅读项和解释块保存后，移除地址中的条目与块参数重开，再刷新，恢复同一个理解对照回答和正式条件块，且未产生重复条目。
- 桌面与窄屏截图已人工检查。窄屏主动定位的目标宽度至少 280px、95% 以上位于视域内；演示结果区域在上方窗口完整可见，对应解释出现在下方，无页面横向溢出。

实际页面的 JavaScript 错误为 0，非预期 HTTP 失败为 0。夹具缺省的可选 `source_manifest` 有 9 次已分类的 404，不影响上述来源解析与流程。截图见[桌面理解对照](../tmp/board-wb3/browser/desktop-understanding-comparison.png)、[桌面图文联动](../tmp/board-wb3/browser/desktop-linking.png)、[窄屏定位](../tmp/board-wb3/browser/mobile-canvas-focused.png)、[窄屏阅读](../tmp/board-wb3/browser/mobile-reading.png)及[窄屏演示区域联动](../tmp/board-wb3/browser/mobile-presentation-linked.png)。

旧 WB1/WB2 浏览器夹具已补齐新增默认字段，保留原四类导入内容。模拟视图保存合并已存在的字段，避免未发送的播放或阅读位置在夹具中被清空。

## 9. 已知限制

WB4 的运行中正式局部提交与实时补充尚未实现。当前候选在最终成功回答时正式归板，已保存路线在本地逐步播放。

浏览器验收使用可控模型适配器驱动真实工具、网络、教学事实、保存和页面链路；不据此宣称外部模型已经稳定生成高质量布局或教学内容。

工具读取仍受单次结果预算约束：单条完整内容大于预算时明确标为未观察，不截断正文后冒充完整证据；其身份与几何仍可从 `layout` 获取。Reader 中保存的完整正文不受该工具投影限制。任意旧 HTML 演示只有已经声明的区域与动作参加细粒度联动。
