# 全屏问答的三栏布局

状态：**已上线**。2026-10-13 **10:09:54（Asia/Hong_Kong）** 切换静态入口，10:10:31 完成正式 HTTPS 资源复核；桌面与手机登录页加载正常，无脚本错误或失败资源。

## 页面变化

全屏问答在窗口宽度达到 1280 CSS 像素时使用三栏。左侧放学习目标、目标调整、会话操作及连续教学开关；右侧放当前回答的帮助、作答入口和“我的理解”；中间承接讲解、活动及提问输入框。正文取消原先的 52rem 上限，跟随两侧栏之间的空间伸展，输入区只保留输入框与发送操作。

两侧控件使用原有组件与动作。通过 Vue Teleport 移动同一份控件，宽窄屏切换保留目标编辑草稿、主输入草稿和活动回答；退出全屏、窗口收窄及演示工作区沿用原有布局。会话历史关闭后，焦点回到实际可见的历史按钮。

## 实测空间

单位均为 CSS 像素。阅读内容宽度扣除了中央滚动区域左右内边距；阅读区域高度不含顶部标题栏及输入区。

| 窗口 | 阅读内容宽度 | 阅读区域高度 | 输入区高度 |
| --- | ---: | ---: | ---: |
| 1907 × 879 | 1331 | 758 | 69 |
| 1526 × 703 | 1004 | 582 | 69 |
| 1440 × 900 | 944 | 779 | 69 |

1526 × 703 对应 1907 × 879 窗口在约 125% 缩放下的有效尺寸；这一用例使用长篇教学活动，覆盖用户截图中的长题目卡片。三种桌面尺寸的正文区域均占窗口高度 80% 以上。

测量依据：[布局记录](layout-metrics.json)。

## 验证

- `RightRail.test.ts`：53 项组件测试通过。三个浅渲染用例改为使用真实 Teleport，继续验证原来的插槽与输入行为。
- `conversation-learning-design.spec.ts`：4 项浏览器测试通过，覆盖三种桌面尺寸与 390px 手机；断言两侧控件位置、正文宽高、操作可见、定位原题、真实提交动作及草稿保留。
- `conversation-learning-entry.spec.ts`：5 项浏览器测试通过，覆盖真实 App 中的目标、暂停、保存失败重试及手机入口。
- `mobile-chat-interactions.spec.ts`：选中文字记笔记、滚动跟随和键盘操作 3 项通过；阅读高度用例的已有失败见下文。
- 类型检查及 `VITE_MULTI_USER=1` 的多用户生产构建通过。

测试报告：[布局测试](layout-tests.json)、[生产构建](build-multi.log)。

## 页面预览

- [桌面三栏阅读](reading-1907.png)
- [125% 缩放对应尺寸下的长教学活动](reading-1526.png)
- [从右侧进入作答](responding-1526.png)
- [1440px 桌面](reading-1440.png)
- [手机阅读](reading-390.png)

## 已知问题与验收边界

390 × 669 的非全屏手机用例要求正文高度超过 401.4px，实际为 361px。使用修改前的 `00f57ea` 工作区基线重跑同一用例，结果同为 361px，见 [基线检查](mobile-baseline.log)。此前新增的底部学习操作仍占用一行，这次桌面三栏调整保留该手机布局；该问题尚未修复。

页面预览与教学动作验证采用本地确定性内容，没有新增模型请求。正式站点验收覆盖入口、直接引用的资源和桌面／手机登录页加载。

## 发布

- 前端应用提交：`7b9488991fb3c00e29828b96c21f286c53b4e9a5`，main 已推送。
- 静态目录：`/opt/understand-book/releases/to-tb-20261012/packages/web/dist-multi`；前端源码同步到同一 release。
- 使用 `VITE_MULTI_USER=1 pnpm --filter @understand-book/web build --outDir dist-multi` 构建，上传 `reader-DO2hmBKd.js` 与 `reader-D5Rm1WOT.css`，复用 267 个既有资源。
- 恢复点：`/opt/understand-book/backups/conversation-sidebars-20261013T020948Z`，保存旧页面入口及前端源码基线；旧静态资源保留。
- 发布前 461 个前端基线文件与上一版 `18aae9e` 逐文件比较一致；正式首页、Reader 入口及直接引用的 JS／CSS 与此次产物逐字节一致。
- 后端进程 PID `642476`、数据根 `/opt/understand-book/data/multi-reader-to-tb-20261012` 保持运行；后端源码仍为 `55e767f`。

证据：[部署回执](deployment.json)、[正式资源复核](public-verification.json)、[桌面／手机浏览器结果](production-browser.json)。发布脚本为主工作区 `tmp/conversation-sidebars-20261013/publish.py`。
