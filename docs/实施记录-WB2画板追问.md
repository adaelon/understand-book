# WB2：同板追问与可找回的回答

日期：2026-10-25。状态：WB2 已实现并验收。分层测试、真实网络 Reader 流程、桌面与手机视觉检查及最终生产构建均通过。

本记录承接 [WB1 持久内容闭环](实施记录-WB1知识画板.md)，范围依据[完整设计 §13](设计方案-知识画板与持续讲解.md#13-分阶段实现与交付)。已接通提交、恢复、保存重试、理解与聊天定位，以及条目和解释块深链。各层验证结果见第 6 节，范围与限制集中在第 7 节。

## 1. 本轮目标与当前实现

读者选中画板上的知识、回答、演示或理解记录后，在同一画板提出问题。问题固定发出时所指的内容与演示现场；之后切换选择、移动视角或继续操作演示，不改变该次问题。通过正式交付的完整回答与来源保留到原画板，并与原对象建立明确的后续解释关联。

| 能力 | 已接入行为 |
| --- | --- |
| 选中对象追问 | 底部输入、对象提示、提交与受理状态，演示现场捕获，取消、恢复和失败重试 |
| 冻结对象与来源 | 读取发问时确切修订与真实演示回执，后续改选、改布局不重绑该次问题 |
| 正式回答归板 | 完整回答块、来源和内嵌演示留存，父条目关联，保存重试与交付去重 |
| 回答与父对象往返 | 卡片标识、父子连线、全文“接着解释”入口、聊天定位及条目/解释块深链 |
| 画板全文检索 | 当前私人材料范围内跨画板查标题、完整内容和理解文字，定位实际全文命中 |
| 从知识或理解找回画板 | 预构建精确对象或理解证据查关联根条目，并找到后续解释回答的稳定块 |

## 2. 冻结追问合同

权威类型定义在 [memory/boards.rs](../crates/memory/src/boards.rs)，Web 类型沿原 Rust → TypeScript 生成流程生成。

`BoardFollowUp` 包含 `board_id`、`board_revision`、`item_id`，以及可选 `block_id` 和 `presentation` 现场回执。前端先捕获用户按下发送时所选对象；演示先取得真实现场，再等待其他必要保存。保存失败或画板已切换时不把该次问题发给另一个对象。

[board_api.rs](../crates/server/src/board_api.rs) 的 `freeze_follow_up` 从请求引用解析确切画板修订、留存条目、来源与演示回执，产生 `FrozenBoardFollowUp`。该数据沿 [run_admission.rs](../crates/server/src/run_admission.rs) 进入原聊天回合的 domain 快照，保持当前运行与原内容身份分别可辨。它不是重新从稍后可见的画板选择读取上下文。

冻结内容保留原来源身份；它不会把历史 source_ref_id 自动变成本轮有效引用。新回答需要据原书形成论断时，继续通过当前轮读取与 source.present 取得实际来源。

问题草稿按 board_id 保留；仅在收到受理确认后清除相应草稿。失败后可恢复原问题。输入区的运行对象提示使用原提交目标，用户当前选中项仍可继续变化。

App 使用同一 `client_request_id` 关联提交与受理恢复。本地提交响应丢失时先恢复已接纳的回合，避免把同一点击变成第二次模型运行。已有正式回答仅归板保存失败时，“重试保存”续做原回合的保存与关联，不重新请求模型生成答案。

## 3. 完整回答的正式归板

`BoardDelivery` 保存 `board_id`、提交前后版本、`item_ids`、`block_ids`、原 `origin_session_id/origin_turn_id` 及 `parent_item_id`。新增回答通过 `reply_to_item_id` 连接所问对象；回答自身保留完整正文、问题与来源，内嵌演示沿 WB1 的真实留存路径处理。

[board_delivery.rs](../crates/server/src/board_delivery.rs) 的 `deliverable` 只接纳已完成且满足交付条件的回合；`ensure` 把已受理的冻结对象与实际正式回答交给 `deliver_follow_up`，再记录会话交付关联。`repair` 为尚无成功或失败结果的已冻结追问继续完成持久关联。普通草稿、取消后的未交付内容或不合格交付不应被计作正式画板回答。

BoardStore 以原会话和回合识别既有交付：同次归板重试返回原回执，不新增第二组卡片，也不恢复用户后来已经移除的条目。内容保存和会话关联中断时，恢复路径仍引用同一正式回答。父对象或画板已删除时不新建替代目标。

用户运行期间产生的布局保存可跨过“只有正式归板追加”的连续修订：`replay_layout_over_deliveries` 先确认中间版本没有改变既有条目和标题，再保留新回答并合入布局。另一个窗口改过标题、卡片或其他内容时仍返回版本冲突；这不是通用自动合并。

首版稳定解释块以完整回答为单位：`block_id` 由稳定条目身份确定，WB1 已有回答读取时补足相同稳定块身份。它提供回执和导航定位，不把正文段落下标当成持久身份。语义段落与图元的细粒度映射仍属于 WB3。

新回答依据父对象放置并记录关系。到达通知提供“定位新回答”，不自动抢走读者当前镜头、选择或已打开的演示。父子连线只表达“此回答接着解释该对象”，不冒充正式知识图谱关系。

## 4. 搜索与相关对象查找

`BoardStore.search` 在当前用户、学习工作区、材料与确切发布范围内查询多个画板，覆盖画板标题、条目完整文字及理解记录。理解原话和系统解释保持分字段检索，不拼成用户从未说过的一句话。`BoardSearchHit` 返回画板修订、条目和可选稳定块身份，附类型、标题与命中摘要。

搜索结果打开对应画板，选择卡片并定位其全文块；有搜索词时，在该块的实际呈现段落中滚动到匹配位置。被移除的内容或不存在的块会明确提示重新搜索。新增全文检索与 WB1 添加面板的已加载候选筛选分别使用。

相关画板 API 正式请求使用 `{ source }`。服务端从 `BoardImportSource` 解析内部 `BoardLookupTarget`：预构建按确切来源、对象与修订匹配；理解记录按 evidence_id，并结合该证据的实际对象引用寻找相关根条目。`find_related` 在同一材料范围查找，再沿 `reply_to_item_id` 展开后代回答，返回其稳定 item_id 与 block_id。UnderstandingSpace 可以直接打开相关解释，不需要先找原聊天。

聊天中的“回到画板提问对象”和“在画板中查看”分别定位原目标与实际交付结果。直接地址除保留资料发布和学习工作区外，增加 `board_item_id` 与 `board_block_id`；重新打开同一地址仍按稳定身份定位。

## 5. 代码入口

| 文件 / 符号 | 职责 |
| --- | --- |
| [memory/boards.rs](../crates/memory/src/boards.rs)：BoardFollowUp、BoardDelivery、BoardLookupTarget、BoardSearchHit | 权威合同、完整回答块、交付去重、搜索和确切对象查找 |
| [server/board_api.rs](../crates/server/src/board_api.rs)：freeze_follow_up、deliver_follow_up、related | 解析并冻结目标、生成归板条目、相关对象 API |
| [server/board_delivery.rs](../crates/server/src/board_delivery.rs)：deliverable、ensure、repair | 正式回合归板、会话关联与恢复 |
| [server/run_admission.rs](../crates/server/src/run_admission.rs)、[session_event.rs](../crates/server/src/session_event.rs) | 追问受理及持久领域快照 |
| [BoardWorkspace.vue](../packages/web/src/components/BoardWorkspace.vue)：askQuestion、refreshDelivery、locate、searchBoards | 追问、草稿与状态、交付后刷新和搜索定位 |
| [BoardReadingPane.vue](../packages/web/src/components/BoardReadingPane.vue)、[BoardCanvas.vue](../packages/web/src/components/BoardCanvas.vue) | 稳定块与实际文字定位、父子关系显示、原对象返回 |
| [UnderstandingSpace.vue](../packages/web/src/components/UnderstandingSpace.vue)、[RightRail.vue](../packages/web/src/components/RightRail.vue) | 从理解与聊天找回原画板对象及正式回答 |
| [App.vue](../packages/web/src/App.vue)、[api.ts](../packages/web/src/api.ts) | 与实际运行、Reader 归属、画板返回点和 API 的主线接入 |

## 6. 验证结果

| 层次 | 本轮结果 | 覆盖内容 |
| --- | --- | --- |
| memory 画板 | 28/28 通过 | 内容留存、交付去重、父项删除、布局重放、完整文字检索和相关对象 |
| server 的 board 筛选 | 24 个有效测试通过（筛选运行 23 项、精确复跑 1 项），2 个长驻测试宿主按预期 ignored；最终 no-run 通过 | 画板 API、网络归属、正式交付和恢复 |
| App.startup | 115/115 通过 | 实际 App 入口、追问提交/恢复、保存重试与定位接线 |
| network-client | 25/25 通过 | 请求与运行恢复 |
| board-api | 4/4 通过 | 画板请求合同与本地丢响应后的受理恢复 |
| board-route / local-reading-action | 2/2、3/3 通过 | 画板条目/块深链与本地阅读动作 |
| NetworkApp opening / resumption | 6/6、16/16 通过 | 网络 Reader 打开和续接 |
| UnderstandingSpace / RightRail | 25/25、75/75 通过 | 相关画板、聊天中原目标与交付回答定位 |
| BoardWorkspace / AgentPresentation 板现场 | 16/16、9/9 通过 | 画板追问、搜索、父子关系和演示现场 |
| 最终 UI 定向复验 | 2/2 通过 | 全文起点与搜索摘要显示 |
| UI mock 浏览器 | WB1 2/2、WB2 2/2 通过 | 组件页面布局、状态与交互；与真实后端验收分开统计 |
| 真实网络 Reader 浏览器 | 1/1 通过，29.5 秒；7 张截图已检查 | 实际 App、服务端、私人磁盘存储及运行恢复链路；JS 错误和非预期 HTTP 错误均为 0 |
| 英文目录 | 1/1 通过 | 组件与 App 源键、插值参数及英文文案 |
| 最终生产构建 | `pnpm run build` 通过，24.79 秒 | Vue/TypeScript 类型检查及 Vite 产物；仅出现既有 chunk、eval 和 import 警告 |

真实网络页面由 [board-follow-up.spec.ts](../packages/web/playwright/board-follow-up.spec.ts) 配合 [board_follow_up_browser_host.rs](../crates/server/src/tests/board_follow_up_browser_host.rs) 验收。它使用实际 Reader、多人服务端路由与私有磁盘存储，模型回复由可控测试适配器提供。最终链路断言全部通过；4 次纯文本材料的可选 source_manifest 404 单独记录，不计作非预期 HTTP 错误。验收结束后，隔离宿主与 Reader 开发服务均已停止。

本地证据包括[运行结果](../tmp/board-wb2/browser/result.json)与[最终构建日志](../tmp/board-wb2/web-build.log)。视觉验收检查了[完整回答](../tmp/board-wb2/browser/desktop-answer.png)、[留存画板全图](../tmp/board-wb2/browser/desktop-retained-board.png)、[全文搜索](../tmp/board-wb2/browser/desktop-search.png)、[理解关联解释](../tmp/board-wb2/browser/desktop-understanding-related.png)、[手机全文阅读](../tmp/board-wb2/browser/mobile-reading.png)、[手机画板](../tmp/board-wb2/browser/mobile-canvas.png)及[等待期间切换选择](../tmp/board-wb2/browser/desktop-waiting-selection.png)。完整回答从标题开始呈现，搜索摘要去除 Markdown 标记。

存储与服务端的相关复验入口为 `cargo test -p memory boards::` 和 `cargo test -p server --lib board`。前端按上述文件运行对应 Vitest 测试；文案修改后运行 `src/i18n-catalog.test.ts`。

真实页面使用独立 `board_follow_up_browser_host` 测试宿主：产品后端为本机 4220，夹具与模型控制为 4221，Vite Reader 为 4222。在仓库根目录的独立终端启动宿主，使用位于工作区磁盘的隔离临时目录：

```powershell
$wb2Temp = (New-Item -ItemType Directory -Force -Path 'tmp/board-wb2/temp').FullName
$env:TEMP = $wb2Temp
$env:TMP = $wb2Temp
cargo test -p server --lib board_follow_up_browser_host -- --ignored --nocapture
```

在另一个终端启动 Reader：

```powershell
$env:UNDERSTAND_BOOK_ADDR = '127.0.0.1:4220'
pnpm --filter @understand-book/web dev --host 127.0.0.1 --port 4222 --strictPort
```

宿主输出 `WB2_BROWSER_READY` 且 Reader 就绪后，在第三个终端执行：

```powershell
pnpm --filter @understand-book/web exec playwright test --config playwright/board-follow-up.config.ts
```

测试保存实际请求、归板记录和桌面/手机截图。结束后向 `http://127.0.0.1:4221/stop` 发送 POST 请求关闭隔离宿主，再停止 Reader 开发服务。

## 7. 范围与当前限制

- 可控模型用于检查真实运行与留存链路，不构成外部模型生成质量的验收。
- 当前全文搜索限定当前学习工作区中的同一材料与发布，不是跨所有私人材料或跨用户搜索。首版解释块为整段完整回答，不宣称已有精细语义段落—图元映射。
- WB3 的图文双向联动、逐笔播放、镜头跟随及教师通过画板留存目标操作旧演示，WB4 的正式局部提交与实时补充均未在 WB2 实现。
