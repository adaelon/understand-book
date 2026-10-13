# SESSION_CHECKPOINT — 2026-10-13 10:11（全屏问答三栏布局已上线）

## 新鲜度自检
- 主工作区：E:/allwork/download/agent/understand-book；分支 main。
- 本页写入时最新提交：7b94889 feat(web): move learning controls into fullscreen sidebars；发布回执与本页随后以 ops 提交保存。
- 读入时对比 git log -3。生产前端源码为 7b9488991fb3c00e29828b96c21f286c53b4e9a5；后端仍为 55e767f，后续文档提交不改变部署源码。

## 当前在做什么
根据用户“像 Codex 一样把内容放到两侧”的反馈，全屏问答在 1280 CSS 像素以上改为三栏。左侧放学习目标、会话与教学开关，右侧放帮助、作答及“我的理解”，中间取消 52rem 上限并只留下 69px 高的输入区。同一份控件移动位置，宽窄屏切换保留目标、提问和作答草稿。已提交、推送并在 10:09:54 上线，10:10:31 完成正式资源复核。

## 下一步（可直接接手）
1. 读 docs/performance/conversation-sidebars-20261013/README.md 获取最新前端版本、验证和恢复点；完整后端发布见 to-tb-release-20261012/README.md。
2. 如继续 EXISTING 笔记修复，读 docs/performance/tool-usability/to5/README.md 已知限制，再定位 Goal reader_action 与 reader.note 回执。
3. 如启动 TB6，从 TB5 理解视图和 TB4 教具接续，固定跨材料与真人验收入口。
4. TC7／TC8 与 TC2 剩余质量验收沿原 TC 方案。

## 未提交 / 未完成
- 本轮应用代码无未提交修改；部署证据和本页以随后 ops 提交保存。
- 非全屏手机 390×669 的正文高度为 361px，未满足已有用例要求的 401.4px。修改前 00f57ea 同样失败；本轮保留手机布局，待进一步压缩底部操作区，证据见最新发布记录。
- TB6、TC7／TC8、TC2 剩余真实模型质量与读者验收未完成，没有学习收益结论。
- EXISTING 笔记可能仍无法满足 Goal reader_action 完成判断，见 TO5；TO14 原始样本保留原结论。
- 既有 Core BookStructure 路线用例失败仍保留，见 integration-mcp-tutor-20261012.md。
- 本地安装态插件与 Windows 安装包未更新；原有本地书籍、临时文件和其他未跟踪输出保留。

## 冷启动读序
1. docs/performance/conversation-sidebars-20261013/README.md — 最新前端修改、验收与恢复点。
2. docs/performance/to-tb-release-20261012/README.md — 后端、数据根与完整发布恢复操作。
3. docs/performance/integration-to-tb-20261012/README.md — TO／TB 合并依据和集成验证。
4. docs/切片方案-Tutor使命与认知积木.md 的 TB3–TB6；docs/切片方案-工具契约与Agent可用性优化.md 的已知限制与交接。

## 生产环境与验证
- Release：/opt/understand-book/releases/to-tb-20261012；数据根：/opt/understand-book/data/multi-reader-to-tb-20261012；schema 12。
- 服务：understand-book-multi.service，127.0.0.1:8788；正式 https://understandbook.top/；MCP /mcp。
- 最新前端备份：/opt/understand-book/backups/conversation-sidebars-20261013T020948Z。恢复旧页面入口即可回退本次静态修改，旧资源保留。
- 完整发布备份：/opt/understand-book/backups/to-tb-20261012；开放流量后的新增事实必须保留，不能用旧快照覆盖当前数据根。
- 本轮：53 项组件测试、4 项布局／作答浏览器用例、5 项学习入口及 3 项手机交互用例通过；手机阅读高度用例保留原有失败；类型及多用户生产构建通过。
- 线上 461 个前端基线文件比较通过；正式入口与直接引用 JS／CSS 匹配，1440px／390px 登录页无脚本错误或资源失败，后端 PID 642476 不变。
- 本轮无新增模型请求；教学交互由本地确定性夹具验证，正式站点验收覆盖入口、资源与登录页加载。
