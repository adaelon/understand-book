# SESSION_CHECKPOINT — 2026-10-10 09:37（Asia/Hong_Kong）

## 新鲜度自检
- 写入时最新 commit：`4e0b68e feat: integrate book structure, tutoring and multi-user reader`。
- 读入时对比 `git log -3`；若不同，以实际 Git 和工作树为准。
- RS0 设计文档已落档、未提交；RS1–RS8 待实施。工作区有其他工作线的未提交修改，不能按 HEAD 覆盖当前文件。

## 当前在做什么
**阅读分享与安静呈现：RS0 已完成，下一刀 RS1「笔记分享闭环」。**

从一条短篇笔记进入分享预览，提供摘录卡／理解卡两种排版、标题与感想编辑、暖纸珊瑚／奶油蓝配色，并下载真实 PNG。关闭后保持原笔记、读位和聊天草稿。首个完整发布范围为 RS1–RS3，当前先执行 RS1。

## 下一步（可直接接手）
1. 按下列冷启动读序读取 ADR、方案及现有笔记／现场入口，检查本次将改动文件的工作区差异。
2. 以当前 MemoryRecord 准备有来源、无来源两条短篇笔记样本，确定来源标签；作者未知时显示“阅读笔记”。
3. 在 NoteCard 的操作区接入“生成分享图”，由 App 沿原用户与现场创建页面内草稿；新增 ShareImagePanel.vue 与 reading-share.ts 承担本片预览和导出。
4. 用实际中文字体完成预览和 PNG 输出，接好标题／感想编辑、配色、重试及关闭返回；换用户、材料或聊天时沿既有清理入口结束旧草稿。
5. 运行相关组件与真实浏览器下载检查，解码图片核对文字、来源、换行及非空内容，并验证关闭后原笔记与读位不变；通过后记录 RS1 结果、更新方案和代码链路，再刷新 checkpoint 指向 RS2。

## 未提交 / 未完成
- RS0：[ADR-0160](docs/adr/0160-reading-share-images-and-calm-surfaces.md)与[切片方案](docs/切片方案-阅读分享与安静呈现.md)为新增未跟踪文档；CONTEXT、架构索引及代码链路已有对应修改，均未提交。
- RS0 文档检查通过：30 个相对链接及锚点、ADR 编号唯一性、切片状态与依赖、术语和决策长度；这不是功能验收。
- RS1 尚未开始。导出依赖及真实中文字体输出须在本片用实际样本确定；ShareImagePanel.vue、reading-share.ts 是拟新增接点。
- RS1 范围为短篇笔记完整闭环；长文分图、原文／高亮入口与复杂内容格式归 RS2，首版跨平台验收归 RS3。
- 当前 MemoryRecord 没有统一作者字段，source_session_id 本身不证明作者；分享草稿仅编辑本次标题、感想与排版，所选正文及来源沿原对象。
- App.vue、NetworkApp.vue、network-context.ts、样式及其他模块已有在途改动；采用当前代码的身份、现场和清理入口，保留既有成果，避免整文件回退。
- 原 Tutor 主线 T19 真实体验验收仍待完成，入口为 [Tutor 方案](docs/切片方案-Tutor预构建不阻塞教学.md) 的 T19；最新基础成品准入发布状态见 [上线记录](docs/performance/tutor-artifacts-deployment-20261009/README.md)。
- 其他任务入口：[EX](checkpoint_ex.md)、[JSONL](SESSION_CHECKPOINT_JSONL.md)、[DSH](SESSION_CHECKPOINT_DSH.md)、[运营后台](SESSION_CHECKPOINT_ADMIN.md)、[ADM](checkpoint_ADM.md)、[INV](checkpoint_INV.md)、[工具契约](SESSION_CHECKPOINT_TOOL_CONTRACTS.md)、[LangSmith](SESSION_CHECKPOINT_LANGSMITH_FULL.md)。
- 视频工作继续以[视频方案](docs/切片方案-视频理解与渐进伴读.md)和[验收记录](docs/验收-视频理解.md)为准。

## 冷启动读序
1. [ADR-0160](docs/adr/0160-reading-share-images-and-calm-surfaces.md)：全文，内容归属、来源身份、预览／导出与局部呈现规则。
2. [RS 切片方案](docs/切片方案-阅读分享与安静呈现.md)：§1–5、RS1、§6、§8；按首版范围执行，后续切片只作边界参考。
3. [CONTEXT](CONTEXT.md)：阅读分享图、分享草稿、阅读接续卡、阅读现场、应用身份与登录会话、授权访问上下文。
4. [NoteCard.vue](packages/web/src/components/NoteCard.vue)：全文；[api.ts](packages/web/src/api.ts)：MemoryRecord、SelectionContext 与相关笔记接口；[App.vue](packages/web/src/App.vue)：笔记展示／来源跳转、编辑保存、workspaceContextKey 及相关清理逻辑。
5. [network-context.ts](packages/web/src/network-context.ts)：全文，sceneKey、readerKey、forgetNetwork、bindSceneApi；[style.css](packages/web/src/style.css)：现有颜色与字体变量；[reader-typography.css](packages/web/src/reader-typography.css)：本地字体与正文样式；[Web package.json](packages/web/package.json)：现有依赖和验证入口。
6. [代码链路](docs/代码链路.md)：RS0 条目；[阅读排版 ADR](docs/adr/0148-reader-typography-annotations-and-motion.md)：§1–4，阅读偏好、批注与读位连续性。

## 本会话决策摘要
- 分享沿用读者明确选定的已有内容，在本机排版下载；页面内草稿不改写原笔记或教学记录，见 ADR-0160 §2。
- 原文、阅读笔记、助手解释保留实际身份，来源按原对象解析；未知作者使用中性标签，见 ADR-0160 §3。
- 预览和导出使用同一排版结果；RS1 先做短笔记，RS2 完成长内容分图及格式覆盖，见 ADR-0160 §4 与方案 RS1–RS2。
- 视觉延续现有暖纸与珊瑚色，奶油蓝用于分享样式；页面主线为分享 → 阅读回顾 → 书架接续，见方案 §2、§5。
