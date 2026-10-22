# OM1：语义现场贯通保存与追问

日期：2026-10-20。状态：完成。代码基线：`33c19c7fa8aec9a4f1031122bc177743b234b9fd` 加本轮工作树增量；工作树同时含其他工作线的既有修改。本片没有提交、推送或部署。

合同见[切片方案 §3、§6 OM1](../../../切片方案-交互教学与学习接续.md)，决策见 [ADR-0172](../../../adr/0172-interactive-teaching-state-and-learning-continuity.md)。

## 实现与入口

- [`crates/runtime/src/presentation.rs`](../../../../crates/runtime/src/presentation.rs)：新增 `PresentationObservation { summary, state, rendered? }`，`PresentationState.observation` 可缺省。旧快照继续由同一类型读取；前端类型由 Rust 的 ts-rs 导出。
- [`presentation-bridge.js::capture`](../../../../packages/web/src/presentation-bridge.js)：每次同步调用 reader，将语义投影与当次控件、可见 DOM 文本一起发送。reader 抛错时仍采集当前 DOM 与控件，省略不可读的自定义状态和语义，不保留先前观察。
- [`AgentPresentation.vue::captureScene/snapshotScene/followUp`](../../../../packages/web/src/components/AgentPresentation.vue) 与 [`presentation_store.rs`](../../../../crates/server/src/presentation_store.rs) 原有实现完整传递并保存新增字段，无需另建存储或改写队列。
- [`presentation_api.rs::save_state/follow_up_context`](../../../../crates/server/src/presentation_api.rs)：沿确切回执取原快照；追问说明区分输入、显示和宿主文本，语义缺失时不得猜测已计算结果。网络工作区的 `presentation/save/read/restore` 复用同一合同。
- [`state 作者指导`](../../../../skills/presentation/references/state.md)：要求同一模型投影、只读 reader、输入与 applied 参数分别保存、取消计算不推进显示、重开暂停，以及即时页省略 `rendered`。

## 固定场景与观察

验收页为 [`presentation-apply-state.html`](../../../../packages/web/playwright/fixtures/presentation-apply-state.html)，载入元数据为 [content.json](content.json)。A 固定 1 m/s，B 初始 1 m/s，共同时间 10 秒。

| 操作 | 当次快照与显示 |
| --- | --- |
| B 输入改为 2，尚未应用 | `values.page` 保存 input=2、applied=1；语义 `state.speed_b_m_s=2`，`rendered.distance_b_m=10`，宿主文本显示 10 米 |
| 保存、重开并回到原聊天 | 输入仍为 2；画面仍按 1 m/s 显示 10 米；不会自动应用 |
| Reader 发送“为什么没有变” | 保存回执及传入模型适配器的用户消息同时包含新输入、旧显示与宿主文本 |
| 点击应用，异步计算完成 | B 在 10 秒走 20 米，applied=2，与实际 DOM 一致 |
| 最新现场已是 20 米，再读取原追问回执 | 仍取得提问时的 input=2、applied=1、10 米 |
| 取消计算、暂停重开、重置 | 取消保持旧结果；暂停后的时间及结果原样恢复；重置同步恢复默认参数与显示 |
| reader 抛错或省略观察 | 读取当次控件与 DOM，快照中没有旧 `observation` |

同步案例复用 `compare-one-factor/v2/example/index.html`，仅在验收文档中为原计算模型增加 reader 投影，已发布版本保持原样。改速度会按该作品原语义将时间归零；定位到 10 秒后 B=3 m/s 对应 30 米。快照省略 `rendered`，读取和逐次 input 不增加自动保存。

真实 Reader 使用原 `agent-presentation-visual.html → RightRail → AgentPresentation`、真实 Rust HTTP 路由和临时私人文件存储。保存与重开由 Rust 宿主完成；没有用前端假回执代替存储。结构化记录在 [reader-evidence.json](reader-evidence.json)，截图为[待应用重开](pending-reopened.png)与[应用完成](applied.png)。

## 执行结果

下列命令从仓库根目录执行；浏览器命令经 `pnpm --filter @understand-book/web exec` 进入 Web 包。

| 检查 | 具体检测目标 | 结果 |
| --- | --- | --- |
| `pnpm --filter @understand-book/web exec vitest run src/presentation-host.test.ts src/components/AgentPresentation.test.ts` | 字段传递、保存完成才发送、页面前进不替换本次追问回执 | 23 passed |
| `cargo test -p runtime --lib presentation::export_bindings` | Rust 合同可编译，生成的 TS 保留可选字段及对象类型 | 13 passed；[日志](runtime-types.log) |
| `cargo test -p server --lib presentation_store_tests` | 原状态、版本、笔记、追问及新增观察的持久化回归 | 初次 31 passed / 1 新测试前提错误 / 5 ignored；[日志](server-store.log) |
| `cargo test -p server --lib -- om1_ mu5_presentation_linked_view_keeps_chat_and_late_restore_cannot_replace_user_state` | 修正重开前提后核对原始模型输入、缺省字段和网络原回执 | 3 passed；保存集合累计 32 项通过，网络 1 项通过；[日志](server-om1-network.log) |
| `pnpm --filter @understand-book/web exec playwright test playwright/presentation-observation.spec.ts playwright/presentation-bridge.spec.ts --grep-invert 'real Reader' --workers=1 --reporter=line` | 真实 iframe 的采集、取消、恢复、即时页与原 bridge 回归 | 初次 11 passed / 1 测试前提错误；[日志](browser-bridge.log) |
| `pnpm --filter @understand-book/web exec playwright test playwright/presentation-observation.spec.ts --grep 'existing single-factor' --workers=1 --reporter=line` | 按作品归零语义，再定位到共同时间检验即时结果 | 1 passed；浏览器集合累计 12 项通过；[日志](browser-immediate.log) |
| `pnpm --filter @understand-book/web exec playwright test playwright/presentation-observation.spec.ts --grep 'real Reader' --workers=1 --reporter=line` | Reader → 保存 → 重开 → 追问输入 → 应用 → 原回执回读 | 1 passed，无跳过；[日志](reader-browser.log) |
| `pnpm --filter @understand-book/web typecheck` | 状态新字段在 Web 消费链中的类型一致性 | 通过；[日志](web-typecheck.log) |

真实 Reader 运行前设置 `EX10_CONTENT` 为本目录 `content.json` 的绝对路径，`PRESENTATION_TEST_PORT=4189`，启动现有 `presentation_ex10_browser_host` ignored 用例。浏览器设置 `OM1_READER_HOST=http://127.0.0.1:4189`、`OM1_OUTPUT` 为本目录。宿主完成后调用其 `/stop`，记录在 [reader-host.log](reader-host.log)。5 个默认 ignored 用例不计入保存集合通过数；本次显式启动了上述验收宿主。

交付检查：本轮涉及的已跟踪文件通过 `git diff --check`；本记录与 checkpoint 的 31 个本地链接有效，checkpoint 为 49 行。

## 过程中的修正

1. 首次 Rust 编译复用了与源码不一致的 read-tools 产物，报告源码中已存在的数学索引方法缺失。更新该源文件的修改时间以触发重新编译后通过，文件内容未改动。
2. 单因素作品原有改参操作会将时钟归零。测试补充归零断言，再通过原 `seek` 定位到 10 秒；没有修改作品行为或降低距离断言。
3. 新服务端测试加载历史后遗漏选择原聊天，被 `PRESENTATION_SESSION_MISMATCH` 正确拒绝。补上真实 `route_agent_history_select` 后通过，浏览器重开走相同选择路径。
4. 读 O1 固定源码时发现索引的异步结果定位应为 L82，已修正该链接。

## 已知限制

模型输入由现有 `ChatRecordingAdapter` 截获；本片没有调用外部模型或评价教学回答质量，实际模型行为对照属于 OM6。实时教师动作、证据接续和帮助归属的新增能力仍分别属于 OM4、OM2、OM5。

语义内容由页面作者按自身模型给出；本片证明采集、恢复与传递链，`observation` 不成为已核验计算或学习者掌握的证据。
