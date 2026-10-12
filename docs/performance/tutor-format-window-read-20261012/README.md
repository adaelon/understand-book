# 教学活动格式与正文合并读取发布

状态：**已上线**。2026-10-12 **23:31:35（Asia/Hong_Kong）** 切换静态入口，23:33:22 完成正式 HTTPS 产物一致性验证；正式浏览器登录页正常，无脚本错误或失败资源。

目标：教学正文保留 Markdown 和可打开的来源，已在回答中展示的无操作讲解不重复显示，连续正文段落合并读取后保持原文和顺序。用户已授权两项修复一同上线。

## 步骤与验证目的

- [x] 教学展示：先用组件测试复现格式、来源和重复正文问题；修复后确认标题、公式、引用、来源点击及实际可见的展示回执。
- [x] 正文读取：运行段落拆分和首屏测试；若请求数或原文不符，修合并分组或坐标拆分。
- [x] 发布构建：前端类型与生产构建通过；浏览器确认桌面／手机的长讲解与来源，首屏由真实 App 受控接口测试覆盖。
- [x] 发布：确认正式站点当前静态目录，备份入口，先上传资源再原子切换入口，保留旧资源；校验正式 HTTPS 资源及服务状态。

## 范围

本次仅更新 Web 静态资源及对应源码。沿用当前后端和数据根；保留教学活动交付、实际展示、作答与帮助记录的语义。

## 实现与验证

- `answer-markdown.ts:answerDisplayParts` 提取原回答渲染器供教学正文、帮助和反馈共用；`sourceMarkedParts` 将本回答已发布的来源标记转为来源按钮，无法匹配的引用显示“来源不可用”。
- `TutorActivities.vue:usesAnswer/observeVisibleContent` 对与已展示回答相同且无操作的正文取消重复卡片，改为观察原回答的真实可见性；交互题目照常保留。
- `RightRail.vue` 与 `AgentPresentation.vue` 把已发布来源接入教学内容的原有来源弹窗。
- `reader-window-text.ts:readWindowText` 合并相邻叶段落，依照 manifest 的 UTF-16 span 拆回。非相邻、逆序、父节点请求分开，保留调用方顺序。

修复前新增三项教学组件测试均失败；最终五个文件共 **136 项通过**：教学活动 8、回答面板 52、演示 2、正文拆分 3、真实 App 启动 71。浏览器 **2 项通过**（1440px／390px），确认结构、公式、表格、来源弹窗、无横向溢出及两种正文的实际展示记录。类型检查及 `VITE_MULTI_USER=1 pnpm --filter @understand-book/web build --outDir dist-multi` 通过。

## 生产版本与恢复点

- 前端应用提交：`3ee9e351e2e3d211ab02245fc20b8c72def332af`；其中正文合并读取为 `3d8aebe`。main 已推送。
- 继续使用 `/opt/understand-book/releases/to-tb-20261012/packages/web/dist-multi`；前端源码同步到该 release。后端沿用 TO／TB 完整发布的 `55e767f`，PID `642476` 未变。
- 恢复点：`/opt/understand-book/backups/tutor-format-window-read-20261012T153130Z`，保存旧 `index.html`、`home.html` 与前端源码基线归档。旧静态资源保留，恢复入口无需改变数据根。
- 发布前将线上 458 个前端源码／配置／公开文件与基线逐文件比较，确认无额外生产改动。
- 正式 Reader 入口引用 `reader-BzZaGIPM.js`、`reader-og--y6a9.css`；首页、入口及直接引用的 JS／CSS 与构建产物逐字节相同。
- 数据根仍为 `/opt/understand-book/data/multi-reader-to-tb-20261012`；本次未执行数据库迁移、服务重启或模型请求。

证据：[部署回执](deployment.json)、[正式 HTTPS 比较](public-verification.json)、[正式浏览器结果](production-browser.json)、[手机格式](tutor-format-390.png)、[桌面来源](tutor-source-1440.png)。发布脚本为主工作区 `tmp/tutor-format-window-read-20261012/publish.py`。

## 验证范围

教学浏览器验证使用冻结的 API 夹具；首屏合并读取使用真实 App 组件和受控接口验证请求次数与正文。正式站点验证覆盖 HTTPS 产物一致性与登录页加载，没有新建模型回合，也没有宣称真实首屏耗时提升比例。构建保留原有 PDF 动静态导入和大包提示。
