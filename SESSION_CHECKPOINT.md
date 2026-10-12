# SESSION_CHECKPOINT — 2026-10-12 22:39（TO／TB 已上线）

## 新鲜度自检
- 主工作区：E:/allwork/download/agent/understand-book；分支 main。
- 本页写入时最新提交：55e767f docs: close TO-TB integration checkpoint；发布记录与本页随后以 ops 提交收口。
- 读入时对比 git log -3。生产应用源码固定为 55e767fbfc5dded3b07479a0fbe93b8f5a20a7cf；后续文档提交不改变部署源码。

## 当前在做什么
TO0–TO14、选区保存修复、TB1–TB5 和交互学习首页已上线 understandbook.top。22:36:57 完成切换，22:38:58 完成正式验收；维护窗口约 14.6 秒。应用 main 已推送。

## 下一步（可直接接手）
1. 阅读 docs/performance/to-tb-release-20261012/README.md，获取现网路径、验收与恢复点。
2. 如继续修复 EXISTING 笔记，读 docs/performance/tool-usability/to5/README.md 已知限制，再定位 Goal reader_action 与 reader.note 回执；选区保存授权问题已修复并上线。
3. 如启动 TB6，从 TB5 理解视图与 TB4 已交付教具接续，固定跨材料与真实读者验收入口。
4. TC7／TC8 与 TC2 剩余质量验收沿原 TC 方案；发布完成不代表这些待办结清。

## 未提交 / 未完成
- 本轮应用代码已提交、推送、部署；本页和发布记录以随后 ops 提交保存。
- TB6、TC7／TC8、TC2 剩余真实模型质量与读者验收未完成，没有学习收益结论。
- EXISTING 笔记可能仍无法满足 Goal 的 reader_action 完成判断，见 TO5。
- TO14 原始样本仍为 13 通过、2 笔记失败、1 执行器未决；选区保存修复有独立红绿证据。
- 既有 Core BookStructure 路线用例失败仍保留，见 integration-mcp-tutor-20261012.md。
- 本次未更新本地安装态插件或 Windows 安装包；本地书籍、临时文件和其他未跟踪输出保留。

## 冷启动读序
1. docs/performance/to-tb-release-20261012/README.md — 当前生产状态与恢复操作。
2. docs/performance/integration-to-tb-20261012/README.md — 合并依据和本地集成验证。
3. docs/切片方案-Tutor使命与认知积木.md 的 TB3–TB6；docs/切片方案-工具契约与Agent可用性优化.md 的已知限制与交接。

## 生产环境与验证
- Release：/opt/understand-book/releases/to-tb-20261012；数据根：/opt/understand-book/data/multi-reader-to-tb-20261012；schema 12。
- 服务：understand-book-multi.service，127.0.0.1:8788；正式 https://understandbook.top/；MCP /mcp。
- 备份：/opt/understand-book/backups/to-tb-20261012；执行记录：/opt/understand-book/acceptance/to-tb-20261012。
- 保留原 MCP10 release、原数据根和旧静态资产；开放流量后的新增事实必须保留，不能用旧快照覆盖当前根。
- Linux Reader、Admin 和五个 Rust 程序构建通过；Rust release 9 分 57 秒。Rust 1.98.1、Node 24.20.0、pnpm 10.34.2。
- 停写后完整备份、独立恢复和比较通过：4 个账号、10 个发布、68 个工作区、1,738 个文件，其中 265 个私人文件。
- 正式新资源、Reader 登录、搜索、Tutor 就绪、PDF Range、MCP 11 工具新合同和一次实际读取通过；原业务表保持。
- 验收消耗 1 次免费 MCP 读取，付费 0、模型费用 0；凭证已撤销，会话已退出。
- 本地集成验证沿已有证据：Server 117、Runtime 149、Memory 1、Core 90、Web 20；类型／构建、TB5 浏览器和首页浏览器通过。
