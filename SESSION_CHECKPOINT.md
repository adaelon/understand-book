# SESSION_CHECKPOINT — 2026-10-12 23:34（教学格式与首屏合并读取已上线）

## 新鲜度自检
- 主工作区：E:/allwork/download/agent/understand-book；分支 main。
- 本页写入时最新提交：3ee9e35 fix(tutor): render activity formatting and avoid duplicate answers；发布回执与本页随后以 ops 提交保存。
- 读入时对比 git log -3。生产前端源码为 3ee9e351e2e3d211ab02245fc20b8c72def332af；后端仍为 55e767f，后续文档提交不改变部署源码。

## 当前在做什么
教学活动正文、帮助和反馈已接入 Markdown 与来源按钮；相同的无操作讲解使用原回答的实际可见性记账，避免重复展示。首屏／连续正文窗口已合并相邻段落读取。两项修复已提交、推送并在 23:31:35 上线，23:33:22 完成正式资源复核。

## 下一步（可直接接手）
1. 读 docs/performance/tutor-format-window-read-20261012/README.md 获取最新前端版本、验证和静态回滚点；完整后端发布见 to-tb-release-20261012/README.md。
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
1. docs/performance/tutor-format-window-read-20261012/README.md — 最新前端修改、验收与恢复点。
2. docs/performance/to-tb-release-20261012/README.md — 后端、数据根与完整发布恢复操作。
3. docs/performance/integration-to-tb-20261012/README.md — TO／TB 合并依据和集成验证。
4. docs/切片方案-Tutor使命与认知积木.md 的 TB3–TB6；docs/切片方案-工具契约与Agent可用性优化.md 的已知限制与交接。

## 生产环境与验证
- Release：/opt/understand-book/releases/to-tb-20261012；数据根：/opt/understand-book/data/multi-reader-to-tb-20261012；schema 12。
- 服务：understand-book-multi.service，127.0.0.1:8788；正式 https://understandbook.top/；MCP /mcp。
- 最新前端备份：/opt/understand-book/backups/tutor-format-window-read-20261012T153130Z。恢复旧页面入口即可回退本次静态修复，旧资源保留。
- 完整发布备份：/opt/understand-book/backups/to-tb-20261012；开放流量后的新增事实必须保留，不能用旧快照覆盖当前数据根。
- 本轮：136 项单元／组件测试、2 项桌面／手机浏览器用例通过，前端类型和多用户生产构建通过。
- 线上 458 个前端基线文件比较通过；正式入口与直接引用 JS／CSS 匹配，登录浏览器无脚本或资源失败，后端 PID 642476 不变。
- 本轮无模型请求；教学交互与首屏行为由本地确定性夹具验证，正式站点未新建模型回合。
