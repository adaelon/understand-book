# WB1：知识画板的持久内容闭环

日期：2026-10-25。范围依据：[知识画板与持续讲解设计方案 §13](设计方案-知识画板与持续讲解.md#13-分阶段实现与交付)。

状态：WB1 已实现；分层测试与真实完整 Reader 的四类内容保存、原文往返、重开和原聊天删除验收已通过。跨媒体来源预览补充验证、桌面与手机视觉检查、类型检查和生产构建均已通过。WB2–WB4 尚未实施。

## 1. 已交付行为

读者可从顶栏“画板”打开当前材料的私人画板列表，新建、重命名或继续已有画板；也可从完整回答、互动演示或“我的理解”中的具体记录使用“加入画板”。添加内容面板按四种类型读取实际已有内容，支持分页及对已加载候选的标题、内容筛选。

画板以卡片承载简短预览，选中后在相邻阅读区展示完整内容、来源和演示。卡片可拖动、用方向键移动；空白处可平移，工具提供缩放和显示全部。标题、卡片位置、视角与选择会保存。画板具有稳定地址，打开链接时保留画板、资料发布和学习工作区身份。

| 内容种类 | 实际保留内容 | 阅读行为 |
| --- | --- | --- |
| 预构建知识 | 正式对象的 source_id、object_id、object_revision、map_revision 和当时对象快照，连同原文绑定 | 查看加入时版本的知识内容及依据 |
| Agent 回答 | 原 session_id、turn_id、问题、完整正文和 answer_parts | 全文正常排版；回答中的精确演示版本同时留为独立条目，可从正文入口打开 |
| 互动演示 | 原版本与加入时现场的 NotePresentationReceipt，以及后续保存的当前现场 | 使用原 AgentPresentation 操作；保存当前状态，或明确恢复加入时状态 |
| 我的理解 | evidence_id、原资料与对象版本、用户原话、系统解释及证据快照 | 原话与解释分区呈现，保留身份和已公开依据 |

导入请求只提交已有对象引用。服务端从正式记录生成留存内容，不接受客户端提供的替代正文或证据快照。卡片预览的截断不影响完整正文；代码、公式在预览中简化，全文仍按原内容渲染。

移动端提供“画板／阅读全文”切换；已经访问的演示面板保留实例，切换卡片或阅读视图不主动重建其 iframe。对话框支持键盘焦点约束与关闭后返回触发处，英文类目可换行。

## 2. 身份、保存与生命周期

`BoardDocument.board_id` 是独立稳定身份，不使用笔记 mem_id 或临时 Reader workspace_id。它归属于当前用户的私有根目录及 learning_workspace_id，并固定 book_id、publication_id、source_revision。无学习工作区的本地场景使用 unscoped 存储。

保存结构由 [BoardStore](../crates/memory/src/boards.rs) 管理：

```text
<private_root>/boards/workspaces/<learning_workspace_id>/<board_id>/
  head.json
  revisions/<revision>.json
  view.json

<private_root>/boards/unscoped/<board_id>/
  （相同文件结构）
```

内容修订先写完整不可变快照，再替换当前 head。标题、条目与位置属于内容修订；平移、缩放、选中项和当前演示现场属于 BoardView，不推进文档 revision。普通位置保存不能改写既有条目的正文、目标或来源。视角保存合并保留服务端持有的演示现场，旧视角请求不会覆盖刚保存的新现场。

更新、导入、删除均核对当前内容版本。前端的 expected_revision 与网络请求的 board_revision 保持含义明确，独立于 Reader 窗口版本。多窗口提交过期版本时返回 `BOARD_REVISION_CONFLICT`（HTTP 409）；界面保留未保存标题和位置，显示失败原因，允许重试，或由读者明确选择“放弃未保存修改并重新载入”。保存失败时不会把关闭动作伪装成已完成。

回答的完整正文与来源绑定保存在画板条目中；演示以真实留存关系复用原 PresentationStore 中的确切版本和状态。删除原聊天后，已经留存的回答、内嵌演示、当前现场和来源仍可读取。加入时的 receipt 固定不变，后续操作状态单独保存在 `BoardView.presentation_states[item_id]`。删除后的旧聊天不能再作为新增导入的来源。

从画板移除条目不修改原聊天、原文或学习记录；删除画板删除其私人文档与留存关系。画板查看、摆放和演示浏览不写入新的能力判断。

## 3. 原文与阅读返回

来源读取从 `board_id + item_id + source_ref_id` 解析该条目已保存的绑定，保留确切出版物、来源版本、段落或媒体位置。来源失效时显示原有标签与预览，并报告无法跳转，不把历史来源悄悄重绑到当前资料。

已验证的正文路径由 [App.vue](../packages/web/src/App.vue) 的 `openBoardSource` 接入现有 Reader 导航。打开正文时保存画板返回点，原文中的“返回画板”恢复原 board_id、选中条目及仍挂载的画板现场。地址读写由 [board-route.ts](../packages/web/src/board-route.ts) 保存 board_id，以及可用的确切资料发布和学习工作区参数。

补充的 [BoardSourcePreview.vue](../packages/web/src/components/BoardSourcePreview.vue) 复用现有视频、音乐和正文来源组件。当前 Reader 已打开同一媒体发布时可定位原播放器；跨媒体或不可直接跳转的来源在画板旁预览，并明确显示引用快照与不可用原因。关闭预览暂停媒体并返回画板焦点。跨视频来源不再被当成成功跳转后静默收起画板；音乐来源即使不能切入当前 Reader，仍能显示其原录音和位置。

## 4. 实现入口

| 文件与符号 | 职责 |
| --- | --- |
| [memory/boards.rs](../crates/memory/src/boards.rs)：BoardDocument、BoardTarget、BoardStore | 四类权威模型、独立存储、修订和视图 |
| [server/board_api.rs](../crates/server/src/board_api.rs)：route、import_item、retained、resolve_source | 创建、列表、候选分页、导入、保存、来源解析及演示留存 |
| [server/presentation_store.rs](../crates/server/src/presentation_store.rs)：read_retained_state、save_retained_state | 原聊天删除后读取和保存留存演示现场 |
| [BoardWorkspace.vue](../packages/web/src/components/BoardWorkspace.vue)：saveChanges、flush、discardAndReload | 内容管理、顺序保存、失败与冲突恢复、移动视图 |
| [BoardCanvas.vue](../packages/web/src/components/BoardCanvas.vue)、[board-workspace.ts](../packages/web/src/board-workspace.ts) | 卡片坐标、拖动、缩放、适应视图、预览 |
| [BoardReadingPane.vue](../packages/web/src/components/BoardReadingPane.vue) | 全文、理解原话与解释、嵌入演示及来源 |
| [BoardSourcePreview.vue](../packages/web/src/components/BoardSourcePreview.vue) | 跨媒体来源、不可用来源与保留快照的预览承接 |
| [AgentPresentation.vue](../packages/web/src/components/AgentPresentation.vue) | 加入时现场、当前状态保存与恢复、留存版本宿主 |
| [TopBar.vue](../packages/web/src/components/TopBar.vue)、[RightRail.vue](../packages/web/src/components/RightRail.vue)、[UnderstandingSpace.vue](../packages/web/src/components/UnderstandingSpace.vue) | 画板入口和真实内容加入动作 |
| [App.vue](../packages/web/src/App.vue)、[board-route.ts](../packages/web/src/board-route.ts) | Reader 归属、直接地址、来源导航与画板返回 |
| [en.json](../packages/i18n/src/en.json) | 画板与相关入口的英文文案，包含动态类别和内容说明 |

## 5. 验证证据

以下是本轮已取得的结果。测试覆盖各自边界，实际 Reader 验收另列；数量不代表 WB2–WB4 已实现。

| 层次 | 结果 | 检测的实际失败 |
| --- | --- | --- |
| memory 画板 | 14 通过 | 独立内容与来源重开、历史快照、范围隔离、过期更新/删除、视图与演示现场、提交失败恢复 |
| memory 持久化回归 | 9 通过 | 原子写入和既有私有存储回归 |
| server WB1 | 6/6 通过 | 四类真实来源、完整回答与内嵌演示留存、删除聊天后读取及现场恢复、来源范围、跨窗口和私有范围 |
| 既有演示服务端回归 | 41 通过，10 个专用用例默认忽略 | 原版本、现场、留存关系和原聊天行为；38 项首次通过，3 项补齐测试替身后定向通过 |
| 画板前端 | 10 通过 | 保存/重开、失败阻止关闭、显式重载、布局与演示实例连续性 |
| 来源预览组件 | 4/4 通过 | 陈旧快照、跨媒体来源分流、关闭暂停、焦点恢复 |
| AgentPresentation / RightRail | 104 通过 | 演示留存与状态、完整回答入口及既有呈现行为 |
| TopBar / UnderstandingSpace | 27 通过 | 入口可用性、移动菜单关闭、当前真实 evidence_id 与纠正后新依据 |
| 网络导航相关 | 26 通过 | Reader 原文导航与现场版本行为 |
| board API / 路由 / 连续性 | 7 通过 | 画板修订与 Reader 版本分离、留存读取、稳定地址与返回 |
| TypeScript / Vue 类型检查 | 通过 | 权威类型与组件、事件和接口一致性 |
| Web 生产构建 | 通过 | 完整 App 和新增来源预览可生成生产产物；日志见 build.log |
| 英文目录 | 1/1 通过 | 静态源键覆盖、插值参数一致；动态画板类别/说明也已补齐 |

最终服务端重编成功；新产物定向运行此前三个演示回归及 WB1 六项，共 9/9 通过。旧测试的失败位于回答完成后的教学指导调用：测试替身仅支持聊天调用，缺少当前结构化指导响应。EX13 的本地 RecordingAdapter 已补齐与现有 fixture 一致的结构化响应委托，没有改变生产行为。

### 真实完整 App 与持久后端

[board-live.spec.ts](../packages/web/playwright/board-live.spec.ts) 包含两项均已通过的场景。第一项加载实际 App，画板请求经过真实服务端路由并落盘；测试内容由隔离 fixture 提供，不调用模型。

“平均速度”场景完成：新建画板，导入预构建、完整回答及其内嵌演示、理解记录；核对四类条目与完整正文；修改标题和卡片位置；打开正文并返回画板；调整演示回程速度并展开结论；关闭画板；删除原聊天并重新打开存储；通过直接地址重开；确认标题、四类内容、全文、原文返回、演示速度和已揭示结论仍保留；在 390px 切换全文与画板。页面脚本错误为空。

第二项使用真实画板存储与完整 App，按服务端既有跨视频、跨音乐合同控制来源响应，验证当前材料不会被错误替换、画板保持可见、原声定位到引用起点、Escape 返回和 390px 预览布局。音频由测试内的六秒静音 WAV 提供，支持实际 HTTP Range；该项是媒体来源合同的 UI 集成验证，不代表真实云端录音访问验收。修复后定向重跑通过。

结果与已直接查看的截图：

- [结果 JSON](../tmp/board-wb1/browser/result.json)：四类内容、原聊天已删除、errors 为空。
- [桌面演示](../tmp/board-wb1/browser/desktop.png)、[桌面理解记录](../tmp/board-wb1/browser/desktop-understanding.png)。
- [390px 全文](../tmp/board-wb1/browser/mobile-reading.png)、[390px 画板](../tmp/board-wb1/browser/mobile-canvas.png)。
- [390px 跨资料来源预览](../tmp/board-wb1/browser/mobile-source-preview.png)、[生产构建日志](../tmp/board-wb1/build.log)。

独立组件的 mock 浏览器用例另有 2/2 通过，产物位于 [packages/web/.wb1-ui-results](../packages/web/.wb1-ui-results)。这些证据用于布局、交互与英文类目检查，不计作真实后端验收。

## 6. 可复用验证方法

在仓库根运行相关存储与服务端测试。只在改动对应路径、出现新失败或仍有具体疑点时重跑：

```powershell
cargo test -p memory boards::
cargo test -p memory persist
cargo test -p server --lib wb1_
```

前端的最小相关覆盖与类型检查：

```powershell
pnpm --filter @understand-book/web test src/board-workspace.test.ts src/board-source-preview.test.ts src/board-api.test.ts src/board-route.test.ts src/components/AgentPresentation.board.test.ts src/components/TopBar.test.ts src/components/UnderstandingSpace.test.ts src/i18n-catalog.test.ts
pnpm --filter @understand-book/web typecheck
```

真实浏览器使用 [board_browser_host.rs](../crates/server/src/tests/board_browser_host.rs) 的专用测试宿主。它建立独立临时私有目录及平均速度资料、已完成回答、演示和真实学习证据；不连接生产用户存储。测试专用 `/test/fixture`、`/test/reopen`、`/test/shutdown` 仅存在于此 ignored 测试函数，不是产品接口。`/test/reopen` 从磁盘重读，供验收保存结果。

终端一启动测试宿主，等出现 `WB1_BROWSER_READY`：

```powershell
cargo test -p server --lib board_browser_host -- --ignored --nocapture
```

终端二启动实际 Reader 开发服务并指向该后端：

```powershell
$env:UNDERSTAND_BOOK_ADDR = '127.0.0.1:4218'
pnpm --filter @understand-book/web dev --host 127.0.0.1 --port 4219 --strictPort
```

终端三执行浏览器验收，然后关闭测试宿主：

```powershell
pnpm --filter @understand-book/web exec playwright test --config playwright/board-live.config.ts
Invoke-RestMethod -Method Post -Uri 'http://127.0.0.1:4218/test/shutdown'
```

测试宿主固定监听本机 4218，默认 Reader 为 4219；每轮需要新的测试宿主，因为验收会删除 fixture 原聊天。成功后读取 result.json，并直接检查桌面与手机截图。浏览器失败会保留 failure.png 与 failure.txt，依据失败步骤修正对应保存、导航或布局路径。

## 7. 已知问题与未实现范围

- 生产构建通过，现有打包器仍提示 Reader 大体积 chunk、部分动态与静态导入重合及 lottie 的 eval 警告；本轮没有扩展为打包性能改造。
- WB2 的画板选中对象追问、正式回答归板与全文检索，WB3 的段落—图元映射、语义连线、逐笔播放和教师跨聊天操作，WB4 的正式局部提交与实时补充均尚未实现。现有添加面板筛选只查已加载候选，不能当作画板全文检索。
- 本轮浏览器证明实际 Reader、产品 API 与隔离持久存储的内容闭环，未覆盖正式云端登录、订阅、发布部署或实际模型生成。多人私有范围与授权由服务端相关测试覆盖。
- 尚未进行大型画板容量或交互性能基准，不报告未测量的规模和性能收益。
