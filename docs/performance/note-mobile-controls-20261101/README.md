# 笔记保存反馈与手机输入操作

2026-11-01 16:05:26 HKT 已上线 [understandbook.top](https://understandbook.top/index.html)，随后正式网站的手机页面交互验证通过。

## 行为与原因

- `PersonalNotes.vue:notice` 原来只在新建、选中等后续操作中清空，保存后停在原页面就会一直显示。现在成功反馈显示 3 秒后自动清空；新反馈重新计时，状态改变和组件卸载会取消旧计时。
- `note-launcher.ts:useNoteLauncher` 原来只避让列出的输入区域，手机键盘压缩可见区域时，悬浮入口仍可能覆盖按钮。现在在窄屏且键盘占用可见高度时隐藏“记一下”，关闭键盘后恢复；沿用原位置，不覆盖拖动偏好。通过缩放比例排除单纯捏合缩放。
- 提示保留短暂确认反馈，外层及提示不接收点击；隐藏后的入口也不占用点击位置。已打开的笔记窗口仍可编辑、保存。
- 320px 宽、260px 可见高度的实际渲染发现草稿状态行侵入操作区 18px。正文最小高度由 56px 调为 32px，短屏时正文内部滚动，状态行与保存区各自保持完整。

## 验证

- 25 项单元与组件检查通过，包含连续保存相同提示时的计时重置、保存失败重试、迟到保存、草稿恢复、窗口位置和键盘视口。
- Edge 覆盖 8 个不同场景：普通/长输入发送、拖动与重开、触摸拖动、失败重试，以及 4 个手机回归场景。
- WebKit 覆盖 4 个手机场景：键盘期间隐藏入口、关闭恢复且保存位置不变、捏合缩放不误隐藏；390px/320px 下直接保存笔记、反馈消失、继续发送；学习目标编辑保存。
- 新增状态行与操作区不重叠断言，在旧样式中报出 18px 重叠；修复后按两种手机尺寸复验。
- 完整前端类型检查与修正后生产构建通过。构建输出到独立目录 `packages/web/dist-note-mobile-controls`。

复验命令（仓库根目录）：

```powershell
pnpm --filter @understand-book/web exec vitest run src/components/PersonalNotes.test.ts src/note-launcher.test.ts src/note-window.test.ts src/app-viewport-height.test.ts
pnpm --filter @understand-book/web exec playwright test --config playwright/thought-window.config.ts --grep 'mobile keyboard|mobile note|thought entry leaves|dragging the recording entry|a failed save'
pnpm --filter @understand-book/web exec playwright test --config playwright/note-mobile.config.ts
pnpm --filter @understand-book/web typecheck
pnpm --filter @understand-book/web exec vite build --mode multi --outDir dist-note-mobile-controls
```

截图：[390px 保存](mobile-note-save-390.png)、[320px 保存](mobile-note-save-320.png)、[保存后继续发送](mobile-after-save-390.png)、[失败重试](keyboard-retry-390.png)、[桌面发送](send-1440x900.png)。手机截图截取模拟键盘弹出后剩余的实际可见区域。

## 发布与公网验证

- 以当前正式 `note-thinking-20261101` 为基线，生产源码差异仅 `PersonalNotes.vue` 与 `note-launcher.ts`；在独立候选中完成 Linux 类型检查、25 项相关单元/组件检查及多用户前端构建。
- 新增 4 个静态资源后原子替换 Reader HTML，首页、后台入口和旧资源保留。Reader 入口为 `reader-CD5KhATP.js` / `reader-CxfpjFGf.css`。服务 PID 前后均为 `70237`，没有服务重启或数据迁移。
- 公网 10 个页面入口、13 个直接引用资源与部署目录比对通过；候选 458 个 CSS 资源引用全部可读。
- 正式 HTTPS 登录后，390px / 320px 页面各创建一条临时验证笔记并正常保存。成功提示约 3 秒后消失（观测 2912ms / 3320ms），键盘期间入口隐藏，收起键盘后恢复。真实阅读页面的发送按钮通过普通点击命中检查（trial，不提交模型问题）；状态行与保存区分别留有 10px / 6px 间距。页面脚本错误 0；两条临时笔记均已删除，临时登录会话已注销。
- [发布回执](deployment.json)、[候选构建结果](build-status.json)、[正式网站交互结果](public-browser.json)、[390px 保存](public-save-390.png)、[320px 保存](public-save-320.png)、[手机发送按钮](public-send-390.png)。正式页面截图已经直接查看。
- 回退副本位于 `/opt/understand-book/backups/note-mobile-controls-20261101T080525Z`，包含替换前的 Reader HTML 和两份源码。恢复这些文件即可回退本次静态页面变更；用户数据无需恢复。独立候选与记录位于 `/opt/understand-book/acceptance/note-mobile-controls-20261101`。

## 已知限制

键盘通过 VisualViewport 变化模拟，已验证 Edge 和 WebKit，未使用实体 iPhone。正式验证使用独立临时笔记并已清理，没有提交模型问题或修改既有笔记。
