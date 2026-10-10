# RS3 首版平台与连续性验收

2026-10-10。功能与 Web 验收完成；Windows 原生保存和实体手机验收待补。接续 RN1–RN4、RS1–RS2 未提交工作树，未提交、未发布。

## 已交付

分享图默认奶油蓝：原文纸页居中，灰绿圆角底、柔和光带、微斜纸页和蓝色手写装饰形成层次；标题移到主体下方，感想与来源依次成组。摘录卡居中，理解卡标题／感想左对齐且纸页倾角较小。暖纸珊瑚沿用构图。正文、公式和表格继续使用真实字体排版；固定手写装饰使用 SVG 路径，不依赖接收设备字体，无额外配图或在线资源。短文来源紧跟正文，多页保留页序。

用户确认[无配图样张](visual-direction.png)方向，并确认实际 quickstart-demo 输出“效果可以，继续收尾”。随后按弹窗反馈精简编辑／预览分区：原文和出处默认折叠，排版／配色并排，预览使用独立浅灰绿底，保存操作固定在底部；外层不滚动，内部需要滚动时使用细、低对比度滚动条。移动窄屏沿单列滚动，底部操作持续可达。

材料没有 paper metadata、且书架尚未打开时，分享仍从真实 bookLibrary 读取名称；作用域变化或关闭后，晚到名称不会重开或覆盖新草稿。修复前已用失败测试复现“已有材料却显示材料名称暂不可用”。

关闭、身份／材料／发布／聊天变化清理分享草稿、图片与晚到结果；预览与逐页保存复用同一 PNG Blob。保存取消可重试，失败保留编辑；关闭回到原读位和触发入口，聊天未发送草稿保留。

原生宿主写入毫秒时间，RN4 原解析误乘 1000，显示五万多年后的年份。本轮已修复毫秒／旧秒字符串／ISO 的统一显示与排序，见 [RN4 验收](../reading-notes-rn4-20261010/README.md)。

## 验证

| 检查 | 实际结果 |
| --- | --- |
| 分享／来源／作用域单测 | 4 文件 60 项通过；面板精简后 App 与面板 53 项再次通过；`unit-composition-final.log`、`panel-unit.log` |
| RN4 原生时间 | 先失败再通过；3 项验证秒、毫秒、ISO 混合排序与年份；`note-time-before.log`、`note-time-after.log` |
| Chromium | 19 项相关用例分轮通过；四个宽度 320／390／768／1440 px、逐页下载字节一致、缩小键盘视口、打开图片、关闭读位／聊天草稿、RN3／RN4 回归 |
| 最终弹窗 | RS1 两项和 RS3 四宽度共 6 项通过；`panel-final.log` |
| 富文本与成图 | 长文／列表／代码／公式／表格保全、过宽拒绝、页头绘制一致、两排版两配色实际 PNG；`composition.log`、`colored-ink.log` |
| Web 类型检查／生产构建 | 最终代码通过；保留既有 PDF 混合导入／包体积提示；`web-build-panel.log` |
| Windows 构建 | 沿原 Tauri 配置成功构建 Debug NSIS 候选，包含 sidecar／Book MCP smoke 和 web-dist；未用 TAURI_CONFIG 排除资源；`windows-build-final.log` |
| Windows 实际窗口 | WebView2 154.0.4258.62、隔离 quickstart-demo 和记忆目录，最终界面生成 7 页 Markdown／公式／表格预览；原生保存对话框尚未验收 |

早期成图测试有两项断言随版式调整：页头比对范围改为实际固定署名区域，标题已移至纸页下方；像素检查从仅统计近黑改为统计低亮度墨色，覆盖蓝色与珊瑚字。内容完整、来源、像素非空及下载字节一致断言保留，修正后定向复验通过。失败与后续通过日志均保留。

实际图片：[摘录／奶油蓝](rs3-excerpt-blue.png)、[摘录／暖纸](rs3-excerpt-paper.png)、[理解／奶油蓝](rs3-understanding-blue.png)、[理解／暖纸](rs3-understanding-paper.png)。

界面：[精简前](panel-before.png)、[精简后](panel-after.png)、[320](rs3-320.png)、[390](rs3-390.png)、[768](rs3-768.png)、[1440](rs3-1440.png)。逐页样例见 `rs3-1440-page-*.png`，实际笔记样例见 `sample.md`。

复现入口：

```text
pnpm --filter @understand-book/web exec vitest run src/reading-share.test.ts src/reading-share-save.test.ts src/components/ShareImagePanel.test.ts src/App.startup.test.ts src/reading-notes.test.ts --maxWorkers=2
pnpm --filter @understand-book/web exec playwright test playwright/pdf-selection-actions.spec.ts --grep "RS1|RS2|RS3|RN4|RN3|resolved real PDF selection performs" --workers=1
pnpm --filter @understand-book/web build
pnpm -C apps/desktop tauri build --debug --bundles nsis
```

候选安装包：`target/debug/bundle/nsis/Understand Book_0.2.0_x64-setup.exe`。它是本轮调试构建产物，不代表已安装或发布。

## 已知限制与接续

用户已决定直接接续 RS4；以下平台验收延期保留，不作为后续实施门槛。

- 用户已确认无可连接实体手机；实际下载／长按保存与 RN4 手机连续性保留待办，模拟视口不代替实体设备。
- 用户按物理 Esc 停止 Computer Use，本轮已停止桌面控制。Windows 七页预览已出现，原生对话框逐页保存、取消／重试、切现场待补；RS3 平台关闭条件尚未全部满足。
- 本轮未安装候选包或覆盖已有安装，运行的是 Debug 程序，Web 内容来自工作区生产 dist；安装后的资源路径与 Release 行为未实测。
- 隔离桌面测试窗口保留供用户查看；进程 PID 记录在 `tmp/rs3-desktop.pid`，副本在 `tmp/rs3-desktop-run`，`LOCALAPPDATA` 位于 `tmp/rs3-desktop-profile`，`UNDERSTAND_BOOK_MEMORY_DIR` 位于 `tmp/rs3-memory`，WebView profile 位于 `tmp/rs3-webview`，CDP 为本机 9229。重启时必须保留隔离变量，避免写入用户真实记忆目录；仅在用户不再使用窗口后清理这些测试资源。
- 图片／视频等内嵌资源、过宽表格／公式及无法按行分割的过高内容仍沿 RS2 明确报错；RS3 不引入新的内容转译。
