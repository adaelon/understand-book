# 目标恢复与历史演示读取修复

2026-10-24 08:00:15 HKT 已部署到 understandbook.top，release `goal-presentation-20261024`。发布以当时实际线上源码为基线收拢本次修复，见 [发布与线上验收](release-20261024/README.md)。

## 行为

- 单独读书按资料来源恢复最近明确开始或继续的学习阶段；工作区内按工作区和资料来源恢复。工作区总目标独立保留，同书在不同工作区互不覆盖。
- 旧阶段从既有事件回放恢复，不新增数据库表或复制学习记录。显式跨资料阶段沿用原材料范围；没有阶段的书显示空态，仍可直接提问。
- 恢复目标保留原暂停、结束和教学开关状态。切换资料不会调用模型，未保存编辑不会进入另一份资料，旧异步操作不继续启动新书教学。
- 同一工作区的历史演示按原回合出版读取，保留当前阅读资料；读取、观察、状态保存与恢复、反馈和独立演示窗使用同一来源归属。原出版授权仍须有效。
- 首次演示读取接受同一阅读现场的 revision 推进，attachment/generation 变化仍使请求失效。组件随实际场景变化重新绑定读取，旧响应不能覆盖新内容。
- 读取失败结束加载提示，显示原因并提供真正的“重新读取”；已加载 iframe 的运行错误继续使用本地重新加载。

## 原因与代码

原目标模型只记录工作区单一当前阶段，书库所有资料共用空工作区范围；前端不查找该书保存的阶段。`crates/memory/src/learning.rs:TutorProjection/with_source` 从 Start/Resume 回放来源选择，旧投影缺少索引时从事件重建。`user_runtime.rs:learning_store_for_source`、`tutor_api.rs`、`multi_user_host.rs`、`teaching.rs`、`interaction_input.rs` 和 `teaching_coach.rs` 对齐状态显示、教学准备和交互选择。跨资料共用聊天的首问操作身份加入来源，避免两本书的首问相互冲突。

`packages/web/src/network-client.ts` 同时传递来源与工作区；`useTutorControl.ts` 消费后端确定的当前阶段。`App.vue` 统一目标栏、详情和继续入口，切换资料重置本地状态和编辑界面，异步启动检查阅读上下文。`ConversationLearningGoal.vue` 为无目标资料提供轻量设定入口。

原历史演示通过 `require_turn_publication` 要求当前资料与原回合出版完全相同，导致 AI Infra 现场无法打开原 CS336 回合的演示。`workspace_registry.rs:with_turn_presentation` 改为校验选中对话、工作区归属和用户原出版授权后构造演示上下文；仍保留当前运行接纳的出版校验。共享工作区的旧演示原文引用补全保存回合出版，再走既有跨资料引用路径。

`packages/web/src/components/AgentPresentation.vue:loadPresentation` 每次重新绑定当前场景，以 generation 丢弃旧结果，并提供首次读取失败重试。没有添加无限自动重试。

## 验证

| 检查 | 结果与覆盖 |
| --- | --- |
| `cargo test -p memory learning::tests -- --nocapture` | 11 通过；旧历史回放、重开、A/B 来源、工作区、共同阶段、总目标改版。两条新增回归在修复前实际失败 |
| `cargo test -p server --lib tutor_tests:: -- --nocapture` | 10 通过 |
| `cargo test -p server --lib tutor_loop_tests:: -- --nocapture` | 36 通过，5 个既有真实模型/浏览器夹具忽略；含界面目标到教学绑定一致及跨资料聊天首问 |
| `cargo test -p server --lib w4_workspace_goal_tests:: -- --nocapture` | 4 通过；HTTP 来源选择、工作区隔离、总目标改版 |
| 演示 `mu8_`、`mu5_presentation_linked`、`mu5_tests::om4`、`presentation_store_tests::se4` | 14 通过，1 个既有浏览器夹具忽略；覆盖跨资料完整演示流程、原文引用、授权撤销、读取 revision/generation 边界 |
| 前端目标组件与集成回归 | 目标 A→B→新书→A、同书跨工作区、共同阶段、目标栏和详情一致、保存失败、旧响应隔离；切换仅发 GET |
| `App.startup.test.ts`、`network-client.test.ts` | 分别 110、24 通过；含真实入口启用请求未返回时换书，不误恢复旧阶段。两条媒体测试保留旧目标历史、将当前来源选择改为 null，与新接口合同一致 |
| `AgentPresentation.test.ts` 与 `AgentPresentation.loading.test.ts` | 25 通过；首次读取重试、同源 publication 更新、旧请求成功/失败隔离、iframe 重建 |
| Web 类型检查与 `pnpm --filter @understand-book/web build` | 通过；产物位于 `packages/web/dist`，构建仍提示既有 PDF 导入与大体积 chunk 警告 |
| 浏览器实际组件交互 | 1280×900、390×844、320×568：切换恢复、无目标设定后再恢复、错误提示、重新读取及 iframe 滑杆响应通过；无横向溢出或页面异常 |

浏览器使用实际 Vue 组件和受控 API 数据，验证显示与交互；持久化和权限由上述真实服务端请求测试覆盖。临时预览未纳入产品入口。浏览器脚本、数据与截图在 `tmp/goal-presentation-fix-20261023/`，演示后端日志在 `tmp/presentation-backend-*.log`。

## 已知边界

- 上述测试表为实现期本地证据；线上发布包与部署后验证另见发布记录。
- 独立单书模式下，同书不同出版的原文引用仍遵守既有规则，要求打开原出版；演示本身可以回放旧出版。本次补齐的引用恢复覆盖共享工作区跨资料场景。
