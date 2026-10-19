# SE5：读者反馈入口

2026-10-16。状态：完成。基于 HEAD `402d9be` 的现有工作树，沿 SE4 的确切版本、当前选择和反馈修订号合同。下一入口 SE6。

## 实施步骤

- [x] 接通四项反馈 API 与网络工作区映射，在演示宿主实现一键选择、改选/撤回、选填原因/补充和保存回执。
- [x] 接通可选现场与主动追问，隔离版本切换、改选/撤回和迟到回执；失败重试保留原输入。
- [x] 运行组件、传输和浏览器验收，记录三环境、键盘、现场保持、失败/迟到及无自动模型调用的证据。
- [x] 更新方案、代码链路、架构和主/专线 checkpoint，下一入口 SE6。

## 交互与调用合同

`AgentPresentation` 的宿主工具区提供“喜欢”“不喜欢”，按钮使用原生键盘操作与 `aria-pressed` 表达已保存选择。同项再次点击撤回，另一项改选。首次选择不要求原因；“补充原因”提供对应的多选项和可选文字，保存失败保留草稿及原请求供重试。保存/撤回状态、服务端时间与现场状态独立展示。

`api.presentationFeedbackRead/Set/Clear/AttachState` 使用 SE4 生成的类型。本地使用 `/agent/presentation.feedback.*`，网络经 `network-client` 映射为工作区 `presentation/feedback/*` 并解包 `result`，复用当前用户、出版、attachment、generation 和 revision。原对话、展开工作区、附属窗口共用同一宿主，展开和反馈不重建 iframe。

选择路径：`read → set/clear → 成功回执更新选择 → set 有效且尚无现场时尝试 captureScene → state.save → attach_state(原反馈修订号,原现场回执)`。读取/保存回执按内容代际限制回写，现场附加另绑定本次选择；改选、撤回、切内容或版本后取消旧快照等待，迟到保存/附加结果不覆盖当前选择。

快照超时或页面未准备好时保留票并显示“无现场”。已捕获的状态保存失败时，重试保存原状态；已取得现场回执而附加失败时，重试原回执。没有捕获到现场就不展示“重试附加原现场”，避免把后来操作的结果冒充反馈时现场。

“带入追问”仅把已保存的选择、原因和补充加入原输入草稿；未保存的补充须先保存。主动发送时优先使用反馈附带的确切现场，否则沿现有追问采集当前现场。改选/撤回或切版本移除本功能插入的反馈文字，保留读者原草稿。单纯投票、填原因、保存现场与准备草稿都不启动模型。

## 代码落点

- [AgentPresentation.vue](../../../../packages/web/src/components/AgentPresentation.vue)：`loadFeedback`、`saveFeedback`、`attachFeedbackScene`、`prepareFeedbackQuestion` 与宿主反馈区；现有追问复用 `captureScene/snapshotScene`。
- [api.ts](../../../../packages/web/src/api.ts)：四项类型化调用；[network-client.ts](../../../../packages/web/src/network-client.ts)：工作区与附属窗口传输映射。
- [组件测试](../../../../packages/web/src/components/AgentPresentation.test.ts)、[网络测试](../../../../packages/web/src/network-client.test.ts)：异步回执、失败和传输归属。
- [浏览器测试](../../../../packages/web/playwright/presentation-feedback.spec.ts)：真实组件、沙盒 iframe、键盘与窄/短屏交互，受控 HTTP 响应。

## 验证结果

| 检查 | 结果与证据 |
|---|---|
| 组件与相关回归 | 7 文件、113 项通过：[日志](unit-regression.log)。含 SE5 新增组件 8 项、网络 2 项，及原组件、RightRail、教学控件、文档与桥接测试 |
| 选择与失败 | 回执前不显示选中；不填原因可保存；改选/撤回；失败请求原样重试；补充草稿保留；读取失败可重试 |
| 确切现场 | 快照超时仍留票；状态保存失败重试同一状态；附加失败重试同一回执；迟到读取、选择回执和现场附加不串版本；撤回后不附加旧现场 |
| 三入口与浏览器回归 | SE5 6 项、桥接 7 项、工作区布局 3 项最终通过。[首轮三入口](browser-initial.log)、[组运行](browser-regression.log)、[追问夹具修正后单项](browser-follow-up-retry.log)、[最终 SE5 组](browser-final.log) |
| 布局与现场 | 原对话 960×720、工作区 1536×800、附属窗口 390×844、手机 320×420、短窗口 640×240；键盘 Enter/Space 投票、选填表单、现场保持、iframe 仅加载一次；320 宽无水平溢出 |
| 追问 | “带入追问”仅产生草稿；明确发送使用原反馈现场；编辑补充后的单行草稿仍绑定原现场；普通追问现场失败不发送、保留问题并可重试；操作期间无自动模型请求 |
| 类型检查 | `pnpm --filter @understand-book/web typecheck` 退出码 0：[日志](typecheck.log) |

截图：[原对话](feedback-inline.png)、[工作区](feedback-workspace.png)、[附属窗口](feedback-linked.png)、[320 窄屏](feedback-mobile.png)、[640×240 短窗口](feedback-short.png)。

## 复现

在仓库根目录运行：

~~~powershell
pnpm --filter @understand-book/web test -- src/components/AgentPresentation.test.ts src/network-client.test.ts src/presentation-host.test.ts src/presentation-document.test.ts src/components/RightRail.test.ts src/components/TutorActivities.test.ts src/components/TutorContinuation.test.ts
pnpm --filter @understand-book/web typecheck
pnpm --filter @understand-book/web exec playwright test playwright/presentation-feedback.spec.ts playwright/presentation-workspace-layout.spec.ts playwright/presentation-bridge.spec.ts --workers=1 --reporter=line
~~~

## 已知限制与失败记录

- 浏览器与网络单元验收使用受控 HTTP 回执，真实组件、传输代码、iframe 和快照桥接运行；后端真实私有存储与权限合同继续由 [SE4](../se4/README.md)提供证据。本片未新增 Rust 逻辑，未运行付费模型、发布或部署，反馈数据均为工程夹具。
- 首轮新增组件用例因 happy-dom 不创建 srcdoc 窗口而有 5 项失败；显式提供测试窗口并按实际 channel/origin 驱动消息后通过。测试中的 `findLast` 不在现有 TypeScript lib 范围，改用反转后查找；未修改项目编译配置。修正后的最早 28 项日志见 [unit-tests.log](unit-tests.log)。
- 浏览器组运行的普通追问失败注入曾被滑块自己的异步自动保存先消费，15 项通过、1 项失败；夹具改为等待该自动保存回执后再注入追问失败，单项重跑通过。产品无需为此改变保存行为，原日志保留。
- 收口时补充了“带入含原因/评论的反馈 → 改动现场 → 编辑单行追问 → 发送”回归，复现浏览器移除换行导致原现场绑定丢失：[失败日志](draft-edit-red.log)。反馈摘要改为单行，原反馈记录保持原文；[浏览器重跑](draft-edit-green.log)确认仍发送原回执，[组件回归](draft-component-regression.log)10 项通过。
- 原 `presentation-document.test.ts` 夹具输出 localhost:3000/style.css 连接失败，测试断言通过；既有 KaTeX quirks 与 Playwright 颜色环境警告保留在日志。
- 首版独立笔记回看沿原入口，不显示反馈控件；跨窗口并行改票没有实时推送，新打开或重新读取时获取当前选择。真实用户案例与方法归因待 SE6，正式方法采用待 SE9。
