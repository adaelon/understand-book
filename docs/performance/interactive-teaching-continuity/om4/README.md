# OM4：同一教具的单动作闭环

日期：2026-10-20。状态：完成。代码基线：`33c19c7fa8aec9a4f1031122bc177743b234b9fd` 加本片工作树增量；保留其他工作线与 OM0–OM3 的既有修改。本片未提交、推送或部署。

合同见[切片方案 §4、§6 OM4](../../../切片方案-交互教学与学习接续.md)，决策见 [ADR-0172](../../../adr/0172-interactive-teaching-state-and-learning-continuity.md)。

## 实现与入口

- [`runtime/presentation_interact.rs`](../../../../crates/runtime/src/presentation_interact.rs) 定义 `observe/act`、版本动作声明和五种动作；沿 registry、exposure、schema 与 orchestrator 实际分发。工具通过 `presentation_interaction` 能力发现；`act.based_on` 必须来自本轮观察的确切保存回执。运行结果有 `actor=teacher`，不写入学习者作答。
- `PresentationContent.action_contract` 与内容版本一起保存、读取和修订；[`presentation_author.rs`](../../../../crates/server/src/presentation_author.rs) 与 [`presentation_preview.rs`](../../../../crates/server/src/presentation_preview.rs) 通过实际浏览器检查回调、reader、控件及区域。预览支持 `{kind:"interact",action:...}`，共用 Reader 的动作执行器；三个规定尺寸都执行了改参、异步应用、显现和既有注释。缺失 handler 会使预览失败。
- [`server/presentation_interact.rs`](../../../../crates/server/src/presentation_interact.rs) 的 `PresentationCommands` 由活跃 `RunStream` 持有。`RuntimeStatePort` 和 `NetworkRunPort` 在原现场／授权范围准备命令，释放共享锁后等待；`presentation.command` 通过原运行事件与快照传递。结果沿本地 `/agent/presentation.interaction` 或网络 `presentation/interaction` 工作区 action 返回。
- [`presentation-interaction.ts`](../../../../packages/web/src/presentation-interaction.ts) 在 `useAgentRun` 中领取一次命令；[`AgentPresentation.vue`](../../../../packages/web/src/components/AgentPresentation.vue) 只接受当前可见且准备好的确切页面。iframe 回调完成后，沿原保存队列写入状态；服务端回读保存文件，才把状态、回执、页面身份、参数定义和动作清单交给工具调用。
- [`presentation-actions.js`](../../../../packages/web/src/presentation-actions.js) 连接声明与实际 native 控件／区域，等待原 handler，核对改参读回、取值范围与显现结果。`set_parameters` 可留下待应用输入；`invoke(apply)` 等待真实结果。`highlight` 定位既有区域，`reveal/annotate` 要求页面更新可恢复状态并显示已有内容。作者合同见 [state.md](../../../../skills/presentation/references/state.md)。

## 生命周期与冲突

命令绑定 run、当前现场、页面实例与 `PresentationRef`。领取或发出消息都不是完成；成功回传必须引用同版、原交付回合的真实不可变状态。原内容修订号保持不变。

`bridge` 记录受支持控件中相关参数的人为编辑次数，保存在 `values.parameter_edits`。改参只比较目标键，调用比较声明的可操作参数；值变化且有人为编辑时返回 `conflict` 和当前保存快照。计时／播放自然推进不触发这种冲突。动作错误可保存当时实际现场供继续判断。

前端在断线、运行取消、页面换代或卸载时停止等待并传递取消信号；同一实例不再领取旧命令，服务端也拒绝重复领取和迟到完成。重开页不能使用旧实例的 `based_on` 执行动作。服务端等待最多 30 秒，宿主动作等待最多 25 秒；超时后的下一步是重新观察，动作没有自动重试。

网络路径继续核对授权和工作区现场；领取／完成不推进阅读状态修订，真实状态保存沿原工作区保存命令。教师操作不会重新取得已被用户导航改变的效果应用权威。

## 已完成验证

| 检查 | 具体检测目标 | 结果与证据 |
| --- | --- | --- |
| Runtime 工具注册、合同、示例与实际循环 | 新工具可被发现和分发；缺少 based_on、未声明参数及注入注释文本被拒绝；已保存结果进入下一次模型采样 | 累计 28 项通过，含 4 项类型导出；[初轮合同与循环](runtime-contracts.log)、[最终定向验证](rust-final.log) |
| Server 保存、事件流及网络相邻回归 | 原教具状态、旧回执、恢复、网络现场与流式快照不回退；等待释放锁，未落盘和迟到结果不能报成功 | 累计 40 项通过；初轮 39 passed、1 fixture 失败、6 ignored，修正后 OM4 四项全部通过；[集合日志](server-regression.log)、[最终日志](rust-final.log) |
| 回执元数据 | 返回原参数定义和页面实例；保持已保存状态及取消行为 | 1 项定向通过，与上述集合重叠；[日志](result-metadata.log) |
| 真实作者预览 | 三个规定尺寸的异步计算实际变为 20 米；无回调作品拒绝交付预览 | 包含在 Server 集合；[预览输出](author-preview.json) |
| Web 消息、保存、运行与恢复 | 动作结果通过宿主过滤，保存完成才回传；断线／取消不重放，相关人为编辑与自然推进分别处理 | 累计 50 项通过；[首轮相邻集合](web-regression.log)、[最后变更涉及的 30 项](web-final.log) |
| Web 类型合同 | Rust 生成的动作标签、命令、元数据与实际消费者一致 | 通过；[日志](web-types.log) |
| 原 browser bridge 与 OM1 场景 | 改参未应用、取消、重置、暂停重开、来源及原帮助回执保持 | 12 项通过；[日志](browser-regression.log) |
| 真实 Reader 与实际 SSE | 五种动作、参数错误、冲突、重开、旧页面拒绝；暂停完成回传再取消，不重放且旧回执可回读 | 1 项通过；[日志](reader-browser.log)、[实际命令结果与原回执](reader-evidence.json)、[重开截图](reader-applied.png)、[宿主日志](reader-host.log) |

通过数按不同测试计，不累计重跑。六项默认 ignored 含原来五项手动用例及本片浏览器宿主；宿主在下面的 Reader 验收中显式启动。

`pnpm docs:tools` 已从原合同生成 `docs/工具契约参考.md` 和 `docs/tool-contracts.json`：Resident 44、Local MCP 16、Remote MCP 11、Executor 4 项工具，Tutor 状态 17。导出和原状态示例合同通过，见[生成日志](tool-reference.log)。

交付检查：涉及的已跟踪文件通过 `git diff --check`；本记录、checkpoint、ADR 和切片方案的 94 个本地链接目标均存在。checkpoint 已刷新为 47 行，下一片入口为 OM5。

Reader 首先观察 1 m/s／10 米，改为 2 m/s 时保持旧显示，应用后得到 20 米。4 m/s 被原控件范围拒绝，返回 2 m/s 的实际现场。显现关系和注释后，用户改为 3 m/s；基于旧观察的改参返回 conflict，继续应用得到 30 米。重开恢复该现场和已显现内容，旧实例的动作返回 unavailable。重新观察后改回 2 m/s 并应用，在页面和状态保存都已完成但工具完成回传被暂停时取消，工具返回 `AGENT_RUN_CANCELLED`；放行迟到回传再重开，仍为 20 米，没有再次应用。整个场景保留同一版本 1，早先应用的保存回执仍读出原来的 20 米状态。

## 复现入口

从仓库根目录运行受影响集合：

```text
cargo test -p runtime -p server --lib -- om4_ presentation_interact::export_bindings presentation_store_tests agent_stream::tests tool_registry::contract_tests mu5_presentation_linked_view_keeps_chat_and_late_restore_cannot_replace_user_state
pnpm --filter @understand-book/web test -- src/presentation-actions.test.ts src/presentation-interaction.test.ts src/components/AgentPresentation.test.ts src/agent-run-state.test.ts src/useAgentRun.test.ts src/network-run.test.ts src/presentation-document.test.ts src/presentation-host.test.ts
pnpm --filter @understand-book/web typecheck
pnpm docs:tools
```

设置 `OM4_OUTPUT` 为本目录绝对路径，作者预览测试会写出真实执行结果。Reader 验收使用 `om4_browser_host`（显式 `--ignored --nocapture`），监听 `127.0.0.1:4190`；Vite 设置 `UNDERSTAND_BOOK_ADDR=127.0.0.1:4190`，`OM4_READER_HOST=http://127.0.0.1:4190`。运行 `playwright/presentation-interaction.spec.ts`，完成后调用宿主 `/stop`。

本次使用临时 Playwright 配置将前端监听设为 4191，并通过原 Vite `/api` 代理传递真实 SSE；没有把事件流替换为测试回放。原 bridge／observation 集合使用同一配置并排除另有独立宿主的 OM1 `real Reader` 用例。浏览器测试源码沿仓库原测试入口保存。

## 修正记录

组件测试发现宿主消息过滤器尚未接纳 `interaction-result`，页面已有完成结果却无法进入保存队列。已在原消息合同中接通；保存完成前不能产生成功回执的断言保留。

网络 fixture 最初将首轮设为 completed，却漏了正常终态的 outcome，原会话一致性检查因此拒绝启动下一轮。补上原 outcome 后，真实网络领取、保存、完成与第二个动作均通过；没有改动终态校验。

新枚举的 serde 属性最初合并书写，ts-rs 未解析动作标签。拆开属性后由 Rust 重新导出；新测试中的延迟完成回调也改为真实 `PageOperationResult` 类型。类型检查通过后保留生成结果，不手写另一份动作合同。

## 已知限制

本片验证单动作执行、保存及生命周期。连续演示的教学组织、HelpDelivered／HelpDisplayed／usage 归属和用户接手由 OM5 接续；完整教学场景与真实模型行为对照属于 OM6。本片工具循环使用确定性模型替身，没有外部模型调用。

页面回调复用作者的计算逻辑；语义观察和可见文本不成为独立核验计算或学习者掌握的证据。异步页面实现须遵守原作者合同响应取消信号。
