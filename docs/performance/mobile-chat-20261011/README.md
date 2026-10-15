# 手机问答布局与回答选区 — 2026-10-11

手机问答把目标压为一行；“调整”直接编辑目标，标题栏显示待办数量，输入框旁保留“我的理解”。“学习详情”由会话菜单进入，演示展开时保留独立入口。连续教学的常规说明不再占用手机端固定行，异常、准备中和暂停等状态仍可见。

回答选区通过 `selectionchange` 识别所属已交付回答，沿用 `answerNoteExcerpt` 校验摘录；操作条默认在选区下方，空间不足时移至上方，滚动及视口变化后重新定位。按钮由 `click` 执行，`mousedown.prevent` 只阻止鼠标破坏选区。WebKit 中取消 `pointerdown` 会压掉触屏后续点击，本次回归已检出并改正。

## 空间与交互验证

- 390×669、长回答＋学习目标＋未完成任务＋连续教学：修改前正文 **250.83 px**；修改后 WebKit 正文 **427 px（63.83%）**。
- 320×568、同一组合：正文 **326 px（57.39%）**，无横向溢出，输入框可见。测量见 [layout.json](layout.json)。
- 4 个新浏览器用例分别覆盖触屏选区保存、滚动与退出清理、直接入口及目标编辑、窄屏和键盘激活；Chromium 与 WebKit 均通过。
- 既有 Chromium 浏览器回归 16 项中，14 项直接通过，菜单焦点返回和目标保存失败重试两项在修复后定向复跑通过；对应 WebKit 13 项均通过。
- 组件与摘录测试共 **67 项通过**；目标编辑组件在最后的焦点调整后另行复跑通过。生产构建含类型检查，结果见 [build.log](build.log)。

入口：`packages/web/playwright/mobile-chat-interactions.spec.ts`、`conversation-learning-entry.spec.ts`、`mobile-workspace.spec.ts`、`reading-assistant-space.spec.ts`。WebKit 回归覆盖前三组。

## 实际页面

- [选区旁记笔记](selection-webkit.png)
- [390 px 对话布局](conversation-390-webkit.png)
- [320 px 对话布局](conversation-320-webkit.png)

## 已知限制

浏览器验证使用程序创建原生选区，再执行触屏点击；未在实体 iPhone 上验证系统长按菜单与拖动选区手柄。本次完成本地代码与构建，未发布线上站点。
