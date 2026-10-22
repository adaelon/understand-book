# OM5：连续演示与学习者接手

日期：2026-10-20。状态：完成。代码基线：`33c19c7fa8aec9a4f1031122bc177743b234b9fd` 加本片工作树增量；保留其他工作线与 OM0–OM4 的既有修改。本片未提交、推送或部署。

合同见[切片方案 §4.3、§5、§6 OM5](../../../切片方案-交互教学与学习接续.md)，决策见 [ADR-0172](../../../adr/0172-interactive-teaching-state-and-learning-continuity.md)。

## 实现与入口

- [compare-one-factor@3](../../../../skills/presentation/methods/compare-one-factor/v3/card.md) 从 @2 派生，保留已发布版本。输入速度、已应用速度、共同时间、关系显现与注释分别保存；用户控件与教师回调共用函数。目录 revision 5 登记 9 个方法、10 个版本。改参可以保持待应用现场，应用后继续同页对照，再把可操作页面留给用户。
- [presentation-bridge.js](../../../../packages/web/src/presentation-bridge.js) 按版本的 `teaching_binding.regions` 读取动作实际显示的文字。`set_parameters/invoke` 只采集显示语义字段改变的区域；`reveal/annotate` 采集目标区域。单独改变待应用输入和 `highlight` 不新增帮助。
- [AgentPresentation.vue](../../../../packages/web/src/components/AgentPresentation.vue) 沿原保存队列传递命令身份、现场与支撑。服务端 [presentation_interact.rs](../../../../crates/server/src/presentation_interact.rs) 将保存绑定到已领取命令、原交付、当前页面和前后回执，本地／网络路由共用。重复保存同一命令返回原回执；最终完成回传必须使用该次保存结果。
- [teaching/page_help.rs](../../../../crates/server/src/teaching/page_help.rs) 把教师操作记录为 `UsageObserved`，保留 `actor=teacher`、动作及前后现场；实际可见支撑写入原 `HelpDelivered/HelpDisplayed`，关联原教学交付。工具结果带回 `action_ref/delivery_refs/assistance_refs`。教师动作不写 `LearnerAction`。
- [teaching/design.rs](../../../../crates/server/src/teaching/design.rs) 接受原话时冻结当前聊天和确切追问页面的已见帮助。先前页面即使已有后续聊天讲解，原帮助仍可接续；后来提供的帮助不回填旧表达。`support_context/grounding` 保留教师身份和动作回执，[continuity.rs](../../../../crates/server/src/teaching/continuity.rs) 在原预算内优先保留此前作品及帮助引用，完整内容通过原工具回读。
- [Tutor 指导](../../../../crates/runtime/src/tutor.rs) 更新为 `teaching-design.v13`；`presentation.interact` 指导使用上一条真实回执继续演示，按需要等待回应，并采用当前直接讲解请求。[状态指导](../../../../skills/presentation/references/state.md) 说明实际显示的帮助区域与教师／手动帮助回调的用法。

状态保存完成即记录已见帮助，最终工具回传随后发生。因此在页面已经应用、保存已完成而最终回传被暂停时取消，帮助仍留在原存储；重开只恢复现场，不重播动作。

## 验证与证据

| 检查 | 具体检测目标 | 结果与证据 |
| --- | --- | --- |
| Server 保存、教学与接续回归 | 帮助顺序、输入时冻结、旧页面追问、教师身份、取消后持久化、重开和网络保存；原教学合同保持 | 79 passed、0 failed、10 ignored；[最终日志](server-final.log) |
| 三尺寸真实作者预览 | 回调得到取消信号；同一页面改速、应用、揭示后实际读到 20 米 | 包含在 Server 集合；[实际预览输出](author-preview.json) |
| Runtime 目录、合同及教学规则装配 | 新旧版本按确切版本可读，工具示例和主模型／审阅规则一致 | 26 passed；[日志](runtime-contracts.log) |
| Web 组件与消息流程 | 状态和帮助保存后才能完成；保存失败返回失败；原保存、追问和帮助回执保持 | 30 passed；[日志](web-regression.log) |
| Web 类型 | 保存参数、命令结果与消费者一致 | 通过；[日志](web-types-final.log) |
| 原 browser bridge／OM1 | 恢复、输入与显示、取消、手动帮助、原快照不回退 | 12 passed；[日志](browser-regression.log) |
| 真实 Reader 三路径与重开接续 | 教师揭示、用户接手、取消、帮助时间与归属、下一轮公式交付、旧回执稳定 | 1 passed；[日志](reader-browser.log)、[实际命令、原记录与模型输入](reader-evidence.json)、[重开截图](reader-resumed.png)、[宿主日志](reader-host.log) |

10 项默认 ignored 包含两个 OM4／OM5 Reader 宿主、五个既有演示手动用例和三个教学回放用例。OM5 宿主单独显式启动；ignored 不计入通过数。前期定向验证与最终集合重叠，不累计重跑。

`pnpm docs:tools` 沿原生成器更新工具参考及 JSON：Resident 44、Local MCP 16、Remote MCP 11、Executor 4，Tutor 状态 17。导出与状态示例验证通过，见[生成日志](tool-reference.log)。

交付检查：本片涉及的已跟踪文件通过 `git diff --check`；checkpoint、ADR、切片方案、本记录及新方法说明的 115 个本地链接均存在。checkpoint 为 48 行，下一片入口为 OM6。

真实 Reader 场景使用 [presentation-demonstration.spec.ts](../../../../packages/web/playwright/presentation-demonstration.spec.ts) 与 [服务端宿主](../../../../crates/server/src/tests/presentation_demonstration_tests.rs)，通过原 SSE、iframe、状态保存、追问入口及真实 LearningStore 执行：

1. 用户将 B 输入改为 2，观察仍为 10 米；教师定位不新增帮助，应用得到 20 米并揭示关系，形成两条帮助。用户提交原话“速度翻倍，所以同一时间距离翻倍”，保存这两条帮助引用。
2. 用户自行改为 3 并应用，画面变为 30 米，再提交自己的观察。没有新增教师操作或帮助；此前已经看过的帮助仍随表达保留。
3. 新运行观察原页面，教师改回 1 并应用。页面和保存完成后暂停最终回传并取消，得到第三条已见帮助；两个旧表达保持原样。销毁并重建 AppState，从原聊天存储和 LearningStore 重开，恢复 10 米及已显现关系。
4. 用户从页面追问“这次直接讲公式”。实际 prepare、主模型与 select 审阅收到原表达和帮助条件，确定性工具循环采用 `direct_explanation/current_request` 交付公式，没有新作答邀请；早先 20 米的不可变回执仍可读。

## 复现入口

从仓库根目录运行：

```text
cargo test -p server --lib -- presentation_store_tests teaching_design_tests tutor_adaptation_tests tutor_response_fit_tests om2_continuity om3_ om4_network_command
cargo test -p runtime --lib -- presentation_methods::tests tool_registry::contract_tests teaching_writing_rules_reach teaching_review_receives
pnpm --filter @understand-book/web test -- src/components/AgentPresentation.test.ts src/presentation-interaction.test.ts src/presentation-host.test.ts
pnpm --filter @understand-book/web typecheck
pnpm docs:tools
```

设置 `OM5_OUTPUT` 为本目录绝对路径，服务端 OM5 用例写出实际预览和重开上下文。Reader 宿主运行：

```text
cargo test -p server --lib tests::presentation_store_tests::om4::om5::om5_browser_host -- --exact --ignored --nocapture
```

宿主监听 `127.0.0.1:4192`。Vite 设置 `UNDERSTAND_BOOK_ADDR=127.0.0.1:4192`，浏览器测试设置 `OM5_READER_HOST=http://127.0.0.1:4192` 和 `OM5_OUTPUT`，执行 `presentation-demonstration.spec.ts`；结束后调用宿主 `/stop`。本次使用临时 Playwright 配置把前端设为 4193，原 `/api` 代理传递实际 SSE。

原浏览器回归运行 `presentation-bridge.spec.ts presentation-observation.spec.ts --grep-invert "real Reader" --workers=1`；被排除的是需要另一个独立宿主的 OM1 场景。本片的 Reader 全流程单独运行。

## 修正记录

- 首轮组件断言发现普通保存多带了一个 `undefined` 参数；保持原四参数调用，教师保存才携带新增元数据。动作保存失败的等待分支同时处理拒绝，避免派生 promise 的未处理拒绝；完成回传仍为失败。[首轮日志](web-initial.log)、[最终日志](web-regression.log)。
- 连续帮助使当前请求的重复投影挤掉此前作品。`continuity` 优先保留原作品；帮助正文超预算时注明缩略，保留完整引用与回读入口。[首次失败](server-initial.log)、[重开上下文](reopened-context.json)。
- v3 回调使用取消信号时，真实作者预览暴露原执行入口没有传入信号。预览现在和 Reader 一样提供 `AbortSignal`。[失败日志](server-om5.log)、[三环境结果](author-preview.json)。
- 真实 Reader 的演示引用带 `kind=presentation`；新增保存入口原先按完整 JSON 比较，误拒有效回执。改为沿现有合同解析 `PresentationRef` 后比较，服务端用实际引用格式覆盖。[失败请求](failed-save.json)、[诊断日志](reader-browser-save-diagnostic.log)。
- 公式接续脚本原先将教学动作标为 `compare`，被现有直接讲解合同拒绝。脚本改为 `explain`；原校验保持。[失败日志](reader-browser-reply-failure.log)、[当时实际记录与输入](reader-before-reply-fix.json)。

## 已知限制

本片没有外部模型调用。Reader 的实际主模型／审阅输入由 `TutorRequestRecorder` 记录，回答和动作选择使用 `ChatStubAdapter` 确定性替身；这证明执行、归属和接续链路，真实模型是否自然选择合适演示、采用帮助条件及响应明确请求由 OM6 对照。

普通追问保留原话和帮助条件，没有自动创造独立掌握或学习评价。页面观察继续以作者计算状态和实际可见文字为依据；本片没有真人学习收益结论。
