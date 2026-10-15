# 教学问答的阅读布局与行动入口

后续更新：全屏大屏布局已于当天 10:09 调整为左右侧栏，见 [三栏布局与发布记录](../conversation-sidebars-20261013/README.md)。下文保留 09:38 首版的布局及验收记录。

状态：**已上线**。2026-10-13 **09:38:32（Asia/Hong_Kong）** 切换静态入口，09:39:13 完成正式 HTTPS 产物复核；桌面与手机登录页正常加载，无脚本错误或失败资源。

## 呈现与交互

全屏讲解与学习目标共用最大 52rem 的阅读区域。目标压缩成轻量的上下文提示；正文通过段落间距、标题、引用层次组织阅读，用户提问与讲解分别呈现。

输入区上方保留当前回答的学习操作。存在可提交或修订的活动时显示“回答问题”，点击滚动并聚焦原题输入框；有后续建议时显示“查看下一步”；其余情况进入原有提问输入框。“这一步没跟上”绑定最新已交付回合，发送中禁用，新回合未交付时收起。定位操作保留未发送草稿。

教学内容与题目紧随回答，随后才显示接续建议和回答操作。题目使用“轮到你了”提示、独立输入区域及明确的提交按钮；接续建议呈现完整可点击卡片，区分推荐方向与其他选择。连续教学的全局作用域说明保留在开关的可访问描述和悬浮提示中。

历史回合只有 `domain.teaching` 记录、没有 `tutor.step` trace 时，原先不会挂载 `TutorActivities`。现在与已有教学呈现和接续建议统一使用 `hasTeaching`，恢复后的题目也能显示并被定位。

## 验证

- 五个相关组件测试文件共 76 项通过，覆盖目标编辑、教学开关、接续选择、活动及回答操作。
- 四个相关浏览器测试文件共 26 项通过，覆盖桌面与手机、历史目标、暂停与重试、原文引用、草稿保留及回答提交。
- 最后调整题目顺序和历史回合入口后，重跑受影响的 62 项组件测试与 6 项浏览器测试，全部通过。
- `pnpm --filter @understand-book/web build` 对应的包内 `pnpm build` 完成类型检查及生产构建。

新增浏览器用例使用真实 App 与本地接口夹具：长讲解从顶部阅读时两个操作保持可见，回答入口定位原题并获得焦点，只有提交按钮才产生作答请求，主输入草稿始终保留。最后一轮同时验证仅带教学记录的历史回合。

## 页面预览

- [桌面阅读](reading-1440.png)
- [桌面作答](responding-1440.png)
- [手机阅读](reading-390.png)
- [手机作答](responding-390.png)

## 已知限制

教学预览采用测试内容；教学交互由前述 App 夹具验证，正式站点验收覆盖新资源及桌面、手机登录页加载。

## 生产版本与恢复点

- 前端应用提交：`18aae9e93bb4003661b111345d748da6039eb1b6`，main 已推送。
- 静态目录：`/opt/understand-book/releases/to-tb-20261012/packages/web/dist-multi`；前端源码同步到同一 release。
- 使用 `VITE_MULTI_USER=1 pnpm --filter @understand-book/web build --outDir dist-multi` 构建；上传 `reader-DJnGri3V.js` 与 `reader-CNKIi8WI.css`，复用 267 个既有资源。
- 恢复点：`/opt/understand-book/backups/conversation-learning-design-20261013T013827Z`，保存旧入口及前端源码基线；旧静态资源保留。
- 发布前 461 个前端基线文件与上次应用提交逐文件比较一致。正式首页、Reader 入口及直接引用的 JS／CSS 与此次产物逐字节一致。
- 服务 PID `642476` 保持运行，后端仍为 `55e767f`，数据根仍为 `/opt/understand-book/data/multi-reader-to-tb-20261012`。

证据：[部署回执](deployment.json)、[正式资源复核](public-verification.json)、[桌面／手机浏览器结果](production-browser.json)、[网站构建日志](build-multi.log)。发布脚本：主工作区 `tmp/conversation-learning-design-20261013/publish.py`。
