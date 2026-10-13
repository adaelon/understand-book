# YY7 真实任务的一次完成反馈（2026-10-13）

> 发布状态更新：2026-10-13，用户已授权整体上线；本项已随主标识与近期品牌版本发布。详见[正式发布记录](../logo-brand-release-20261013/README.md)。下文保留原实施阶段的验证记录。

YY7 已在本地完成。当前任务成功保存并回读确认完成后，当前可见的身份头像轻点头一次；原回答、任务文字、草稿和操作继续承担内容。范围和验收依据：[YY7 实施合同](../../切片方案-页页品牌融入与动效.md#yy7-真实任务的一次完成反馈)。YY0–YY7 已接受的品牌融入切片全部完成本地实现。

## 实现

- [yeye-feedback.ts](../../../packages/web/src/yeye-feedback.ts)：当前阅读作用域内保存已观察到的 Goal `id / revision / status`。每次历史回读消费新事实；只有同作用域、已保存的成功运行、回读的完成回合、已有 open 基线和带实际 `result_refs` 的 completed 变化同时满足时，返回一次反馈。重复或较旧 revision 保持静态，同次多个完成合并为一次。
- [App.vue](../../../packages/web/src/App.vue)：`finishResidentRun` 在终态时取得当前可见头像的一次回调，沿原保存后历史回读调用 `applyAgentHistory(history, snapshot)`；回读前后保留作用域核对。`workspaceContextKey` 变化立即清除比较基线；普通历史、首次加载与重新挂载只建立事实基线。
- [RightRail.vue](../../../packages/web/src/components/RightRail.vue)：`prepareCompletionFeedback` 选择普通侧栏 24px、桌面全屏 32px 或手机 24px 的现有身份位；宿主、辅助区、历史弹层及手机工具面板控制当前头像资格。
- [YeyeMascot.vue](../../../packages/web/src/components/YeyeMascot.vue)：复用宿主、页面可见性、视口和减少动态效果条件，使用 720ms CSS 轻点头。回调只能消费一次；保存回读期间隐藏或卸载使回调永久失效。播放中隐藏取消动作和计时；返回静态，没有等待队列。此路径直接使用专用头像。

## 验证

| 验证 | 具体失败及对应修正入口 | 结果 |
| --- | --- | --- |
| [状态与组件测试](component-tests.log) | 历史／重复回读误播、失败或无结果被当作完成、隐藏后补播、卸载残留计时、原待机和书架／助手接入回归 | 111 项通过：反馈状态 22、角色组件 20、RightRail 58、接续 11 |
| [生产构建](build.log) | TypeScript／Vue 接口不匹配、CSS 或资源无法打包 | 多用户生产构建与类型检查通过；测试参数的只读数组类型已修正 |
| [首轮浏览器验证](browser-tests.log)、[定向验证](browser-followup.log) | 实际保存回读未触发或误触发、身份位选择错误、焦点／草稿丢失、隐藏及跨聊天回读补播 | 24 个不同场景全部通过：17 个 YY7、7 个 YY6 回归；首轮 21／22 通过，历史测试同步修正后定向 4 项通过（含 2 个新增场景） |

浏览器使用多用户生产构建，经过真实 `NetworkApp → App → RightRail → YeyeMascot`。接口返回固定 Run／Goal 事实，可控 EventSource 触发运行终态。测试观测实际 CSS 动画名称、变换矩阵和单次开始次数，断言 720ms 后恢复静态，重复事件不再启动；任务回答与输入草稿、焦点均保留。

覆盖普通回复、缺少结果、失败、取消、未保存、同回合新建即完成，书架覆盖、后台、减少动态效果、手机返回正文、保存回读期间隐藏再返回，换聊天、重建组件、重载页面、跨聊天迟到回读及历史回读失败。YY6 的首页按需加载、视口、书架／专注往返、真实等待、流式回答、停止／重连／保存失败继续通过。

换聊天测试现等待历史响应完成和历史按钮恢复可用后，才执行下一次切换；先前测试只等待弹层消失，可能在旧组件仍执行异步切换时再次打开菜单。该项修正了测试同步，产品链路无需调整。

实际截图已查看：点头限制在原身份区，回答和输入没有被遮挡。静态截图记录布局，动作时序由组件测试和浏览器事件／矩阵断言记录。

- 普通侧栏：[截图](completion-tab.png)、[动作采样](completion-tab.json)。
- 桌面全屏：[截图](completion-fullscreen.png)、[动作采样](completion-fullscreen.json)。
- 手机 390px：[截图](completion-mobile.png)、[动作采样](completion-mobile.json)。

从仓库根目录复现；[独立配置](playwright.config.ts) 自动管理 4177 生产预览并将本次证据写入此目录：

```powershell
pnpm --filter @understand-book/web test -- src/yeye-feedback.test.ts src/components/YeyeMascot.test.ts src/components/RightRail.test.ts src/NetworkApp.resumption.test.ts
$env:VITE_MULTI_USER = '1'
pnpm --filter @understand-book/web build
pnpm --filter @understand-book/web test:e2e --config ../../docs/performance/yeye-brand-yy7-20261013/playwright.config.ts
```

## 已知限制

- 本轮为本地实现与验证，未提交、推送或发布。实体手机与原生 WebView 待验；手机、后台及接口事实均为 Chromium 中的受控模拟。
- 首轮覆盖已观察到 open→completed 的当前任务；历史快照、单回合新建即完成、完成时隐藏及未成功回读的任务静态呈现。任务交付不推断读者掌握。
- 全身 success Lottie 仍仅保留在制作资产，生产完成反馈使用现有头像。既有 PDF 混合导入、大 chunk 及 lottie-web 表达式实现的 eval 构建提示保持。
