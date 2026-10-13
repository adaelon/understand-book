# SESSION_CHECKPOINT — 2026-10-13 09:40（教学布局与作答入口已上线）

## 新鲜度自检
- 主工作区：E:/allwork/download/agent/understand-book；分支 main。
- 本页写入时最新提交：18aae9e feat(web): improve teaching layout and response entry；发布回执与本页随后以 ops 提交保存。
- 读入时对比 git log -3。生产前端源码为 18aae9e93bb4003661b111345d748da6039eb1b6；后端仍为 55e767f，后续文档提交不改变部署源码。

## 当前在做什么
教学问答改用较窄的阅读区域与轻量目标提示；输入区固定显示当前回答的帮助及作答入口，点击定位原题并保留草稿。题目接在讲解后，接续建议采用完整可点击卡片。历史教学记录也能挂载活动组件。已提交、推送并在 09:38:32 上线，09:39:13 完成正式资源复核。

## 下一步（可直接接手）
1. 读 docs/performance/conversation-learning-design-20261013/README.md 获取最新前端版本、验证和恢复点；完整后端发布见 to-tb-release-20261012/README.md。
2. 如继续 EXISTING 笔记修复，读 docs/performance/tool-usability/to5/README.md 已知限制，再定位 Goal reader_action 与 reader.note 回执。
3. 如启动 TB6，从 TB5 理解视图和 TB4 教具接续，固定跨材料与真人验收入口。
4. TC7／TC8 与 TC2 剩余质量验收沿原 TC 方案。

## 未提交 / 未完成
- 本轮应用代码无未提交修改；部署证据和本页以随后 ops 提交保存。
- TB6、TC7／TC8、TC2 剩余真实模型质量与读者验收未完成，没有学习收益结论。
- EXISTING 笔记可能仍无法满足 Goal reader_action 完成判断，见 TO5；TO14 原始样本保留原结论。
- 既有 Core BookStructure 路线用例失败仍保留，见 integration-mcp-tutor-20261012.md。
- 本地安装态插件与 Windows 安装包未更新；原有本地书籍、临时文件和其他未跟踪输出保留。

## 冷启动读序
1. docs/performance/conversation-learning-design-20261013/README.md — 最新前端修改、验收与恢复点。
2. docs/performance/to-tb-release-20261012/README.md — 后端、数据根与完整发布恢复操作。
3. docs/performance/integration-to-tb-20261012/README.md — TO／TB 合并依据和集成验证。
4. docs/切片方案-Tutor使命与认知积木.md 的 TB3–TB6；docs/切片方案-工具契约与Agent可用性优化.md 的已知限制与交接。

## 生产环境与验证
- Release：/opt/understand-book/releases/to-tb-20261012；数据根：/opt/understand-book/data/multi-reader-to-tb-20261012；schema 12。
- 服务：understand-book-multi.service，127.0.0.1:8788；正式 https://understandbook.top/；MCP /mcp。
- 最新前端备份：/opt/understand-book/backups/conversation-learning-design-20261013T013827Z。恢复旧页面入口即可回退本次静态修改，旧资源保留。
- 完整发布备份：/opt/understand-book/backups/to-tb-20261012；开放流量后的新增事实必须保留，不能用旧快照覆盖当前数据根。
- 本轮：76 项组件测试、26 项浏览器用例通过，最终调整后重跑受影响的 62 项组件与 6 项浏览器测试通过；类型及多用户生产构建通过。
- 线上 461 个前端基线文件比较通过；正式入口与直接引用 JS／CSS 匹配，1440px／390px 登录页无脚本错误或资源失败，后端 PID 642476 不变。
- 本轮无新增模型请求；教学交互由本地确定性夹具验证，正式站点验收覆盖入口、资源与登录页加载。
