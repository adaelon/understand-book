# 随时记录入口 · 实现与发布

2026-10-30 17:49:12 HKT 已上线 [understandbook.top](https://understandbook.top/index.html)。按用户确认的“随时随地记笔记、减少记录阻力”方案，移除前一版为入口预留的底部整行空间，恢复阅读、问答的完整高度。用户查看实际组件预览后确认发布；沿用实际线上 `all-pending-20261030`，服务 PID 524591 保持。

## 行为与代码链路

- `ApplicationShell.vue → application-content` 恢复全高内容区；保留实际内容视口变量，避免手机键盘的偏移被计算两次。
- `PersonalNotes.vue → useNoteLauncher` 将全局入口收成单个 96×44px 的“记一下”按钮。默认靠右，避开问答、个人讨论、画板的输入区和手机阅读底部导航。内容区变化、输入展开、视口变化触发位置更新；不会持续轮询。
- `note-launcher.ts → placeNoteLauncher` 在可见视口内选取离原位置最近的可用位置；覆盖在后台的输入区不参与避让。拖动、Alt+方向键可移动入口，按使用者保存位置；键盘临时避让不改写保存的位置。
- `PersonalNotes.vue:capture → openThoughtCapture` 沿用原有草稿、来源和保存逻辑。一点击中正文，无需先请求列表；收起后再次点击继续原草稿。已保存内容从浮窗标题栏的“我的想法”按钮进入。
- 原有浮窗移动、缩放、草稿持久保存、就地笔记和演示现场记录继续保留。主导航继续使用上一版的“问答 / 画像 / 工具”。

## 实际预览

本地入口：`http://127.0.0.1:4192/note-launcher-preview.html`。

`note-launcher-preview.html → playwright/fixtures/note-launcher-preview.ts → NoteLauncherPreview.vue` 使用实际 ApplicationShell、ReaderWorkspace、ReaderPane、RightRail、PersonalNotes 组件，配合本地示例内容。截图不是静态重绘。可以拖动入口、记录、收起续写、保存及查看列表；演示数据保存在此浏览器本地，不会发起线上模型调用。此预览入口不包含在正式构建输入中。

[桌面默认状态](preview-1440.png) · [桌面展开输入](preview-input-1440.png) · [桌面记录](preview-writing-1440.png) · [手机入口](preview-390.png) · [手机记录](preview-writing-390.png)。

## 验证

- 23 个不同的浏览器场景通过。首轮 19 项通过，两个演示用例因全局入口与原有演示入口同名而需要限定定位范围；调整定位后定向 6 项全部通过，包括这两个演示场景、两个新增的手机导航与画板场景，以及发送、键盘的再次验证。
- 发送检查覆盖 1440×900、1024×715、783×715、390×844、320×568、1024×440、长草稿、打开并收起笔记后再次发送。使用普通点击检测实际遮挡，并断言内容区没有底部预留高度。
- 鼠标拖动、触摸拖动、拖动不误打开、关闭和刷新后恢复位置、草稿恢复、原文滚动、画板切换和演示现场记录通过。
- 手机阅读导航、画板输入区可命中，隐藏在画板背后的对话不会推移入口。全屏问答覆盖可视高度 460px、顶部偏移 40px 的模拟键盘。
- 19 项位置、视口与笔记组件检查通过。类型检查及多用户生产构建通过。
- 1920px、1440px、390px 实际预览验证：发送可命中、一点击入正文、收起续写、保存后查看列表均通过；页面脚本错误 0。已逐张查看桌面和手机截图。

证据：[浏览器首轮](browser.log) · [定向验证](browser-final.log) · [组件检查](unit.log) · [生产构建](build-final.log) · [预览操作结果](preview-checks.json) · [相对上一版上线包的源码差异](source.patch)。

## 发布与恢复

- 发布前逐文件比对线上构建输入：既有文件仅 `ApplicationShell.vue`、`PersonalNotes.vue` 与候选不同；新增生产模块仅 `note-launcher.ts`。实际预览与构建产物来自同一候选，其他线上功能保持。
- 本次只替换 `index.html` 及相关源码，先安装 4 份新静态资源。首页、后台页面、旧资源保留；无服务重启或数据迁移。
- 入口资源：`reader-B1AEu3rH.js` / `reader-8fUPLhOE.css`。17:50 HKT 公网 5 个入口引用资源与发布文件逐字节相同，普通读者登录和 18 本授权材料读取通过。
- 17:52 HKT 公网 Edge 1440px / 390px 验收通过：内容区为完整视口高度，长草稿下发送按钮可命中；一点击入笔记正文、收起续写正常。桌面拖动不误打开，画像切换并返回问答保留草稿。页面脚本错误 0；已查看两种尺寸的实际线上截图。没有调用模型或创建线上笔记，临时登录会话已注销、临时认证文件已清理。
- 恢复点：`/opt/understand-book/backups/note-launcher-20261030T094912Z`，保存变更前源码与 Reader HTML。恢复时还原备份文件，并按其中 `added-files.json` 移除本次新增的源码文件；旧静态资源保留，业务数据无需恢复。
- [发布回执](deployment.json) · [构建输入差异](candidate-comparison.json) · [公网资源验证](public-verification.json)。
- [公网交互验证](public-browser.json) · [桌面实际效果](public-send-1440.png) · [手机实际效果](public-send-390.png) · [线上记录窗口](public-note-1440.png)。

## 已知限制

手机键盘通过浏览器模拟可视视口验证，未使用实体手机。本次不改动后端、模型和数据格式，也不宣称完整仓库测试通过。
