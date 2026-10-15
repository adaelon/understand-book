# 内测 MCP 实施入口

更新时间：2026-10-12。状态：**MCP0–MCP10 已完成并上线**，包含集成、新 Agent、正式 HTTPS、账本及恢复验收。发布入口见 [MCP10 运行单](内测MCP-发布运行单.md)，与 Tutor 工作线的合并见 [整合记录](performance/integration-mcp-tutor-20261012.md)。

## 工作位置与基线

- MCP 原工作树：`E:/allwork/download/agent/understand-book-reading-share`；分支 `codex/reading-share`，成果提交 `8e7e5fe`。
- 后续集成基线：`E:/allwork/download/agent/understand-book` 的 main；原 MCP 工作树保留在独立成果提交。
- 准备时基线：`de6376d`，已集成阅读笔记 RN1–RN4 与阅读分享 RS1–RS8。
- MCP 进度：MCP0–MCP10 已完成并上线；应用发布提交 `8497f29`。

MCP 产品约定、ADR-0162、切片方案、五个 MCP 术语及实现已与 main 的 Tutor 和阅读器改进整合并发布。后续从最新 main 开始；原 MCP 工作树保留在独立成果提交，继续使用前先同步 main。

## 接手读序

1. [产品约定](内测MCP-产品约定.md)：使用资格、按次计量与预付调用包。
2. [ADR-0162](adr/0162-beta-remote-mcp-and-prepaid-call-credits.md)：能力范围、记次和接入决策。
3. [术语表](../CONTEXT.md)：内测 Book MCP、免费使用额度、付费调用包、调用记次、接入凭证五项。
4. [切片方案](切片方案-内测MCP与调用额度.md)：先读第 3–7 节、MCP9／MCP10 与第 9–10 节；再读 [MCP4](内测MCP-MCP4实现.md)、[MCP5](内测MCP-MCP5实现.md)、[MCP6](内测MCP-MCP6实现.md)、[MCP7](内测MCP-MCP7实现.md)、[MCP8](内测MCP-MCP8实现.md)和 [MCP 接入](MCP接入.md)。凭证细节查 [MCP1](内测MCP-MCP1实现.md)，来源读取细节查 [MCP2](内测MCP-MCP2实现.md)。

## MCP1 已完成

`mcp_access.rs` 提供本人凭证创建、列表、撤销及 Bearer 身份解析；`mcp_access.sql` 沿原控制库升级到 schema 10。账号 API 经原 Host/Origin、Cookie/CSRF，在装载私人 Reader 状态之前完成。完整值只在创建时返回；撤销、停用、改密和旧 epoch 拒绝认证，网页登录退出不撤销 MCP 凭证。

最终 6 项 MCP1 测试通过（含真实 HTTP）；原认证／密码／控制库 22 项回归通过。旧 MU4 10 项用例在会话样本准备阶段触发 `SESSION_EVENT_WRITE_REQUIRED`；接口、日志和限制见 [MCP1 实现](内测MCP-MCP1实现.md)。

## MCP2 已完成

`McpRead::call` 提供获准目录和九个纯读取工具，复用 PublishedLibrary，续页重新检查账号和确切发布授权。原文直接从源范围最多读取 8,000 个 Unicode 字符，坐标为 UTF-16；结构、关系和论文投影提供明确的集合分片与继续调用。目录不装载正文，内容调用不创建私人 Reader 现场。

新增 7 项测试通过；两份真实书籍相关回归在补齐样本后通过。原本机 MCP 的一个 schema 形状断言过时，仍记录为失败。完整合同和日志见 [MCP2 实现](内测MCP-MCP2实现.md)。

## MCP3 已完成

`McpUsage` 提供账号月份余额、管理员当月赠送和短事务成功消费，MCP3 引入 schema 11，当前已由 MCP4 升到 schema 12。每月首次使用获得 1,000 次；赠送复用管理员操作号，消费携带确切扣除来源，事务取时决定香港月份。8 项新增测试与 30 项相关回归通过，见 [MCP3 实现](内测MCP-MCP3实现.md)。

## MCP4 已完成

`mcp_pack.rs` 提供同笔收款的购包批次与追加更正，`McpUsage` 先扣免费次数，再扣有效且最早到期批次。控制库当前 schema 12，保留原消费事实；管理 API 提供查询／购包／赠送／更正，收款列表分别表达两类权益。8 项新增与 38 项相关回归通过，见 [MCP4 实现](内测MCP-MCP4实现.md)。

## MCP5 已完成

`remote_mcp.rs` 由专用 `/mcp` 路由接入独立 Bearer、可选 Origin 和 `2025-11-25` Streamable HTTP JSON 响应。参数与材料授权先于余额检查，内容和编码准备先于原子消费，消费失败不交付正文。Nginx 精确入口保留凭证和协议头。新增真实 HTTP／SQLite 6 项、相关回归 46 项通过，见 [MCP5 实现](内测MCP-MCP5实现.md)。

## MCP6 已完成

初始化说明与 11 个工具自述已给出选材、定位、读证据、引用、分页／计次和恢复规则。[MCP 接入](MCP接入.md)提供接入设置与实际字段说明。定向 5 项（4 项新增）和读取／协议回归 10 项通过，0 失败／0 忽略，见 [MCP6 实现](内测MCP-MCP6实现.md)。

## MCP7 已完成

账号菜单与 `/?account=mcp` 接入凭证、本人次数、各包期限和人工购买说明；来源链接登录后打开确切发布，在规范文本中按 UTF-16 范围临时标记。Web 125 项、真实 HTTP／SQLite 16 项与类型检查通过，见 [MCP7](内测MCP-MCP7实现.md)。

## MCP8 已完成

账号详情提供 MCP 查询、购包、当月赠送及更正；混合收款分别进入模型／MCP 表单，原操作号支持超时和刷新恢复。后台 30 项、服务端 18 项回归及 Admin 类型检查通过，见 [MCP8](内测MCP-MCP8实现.md)。

## MCP9 已完成

Codex CLI 0.162.0-alpha.2 通过无背景阅读及耗尽停止验收，2 条引用和 3 条消费事实核对通过。补齐双账号／HTTP 购包恢复、真实来源解析和小规模并发；87 项既有 Server 回归、2 项新增用例、Web 126 项、Admin 30 项及构建通过。旧 MU4 样本失败与客户端限制见 [MCP9](内测MCP-MCP9实现.md)、[验收记录](performance/beta-mcp/README.md)。

## MCP10 已完成

Linux 版本 `mcp10-20261012` 于 2026-10-12 08:53 HKT 上线，schema 12。独立 schema 9 升级、schema 12 成套恢复、旧二进制拒绝新 schema 与旧快照恢复可读均通过。最新生产数据保留 4 个账号、10 个发布、39 条授权、265 个私人文件及 280 条模型费用。

正式 HTTPS 的 11 工具、来源坐标、赠送／消费账本和 1／4／8 路读取通过；8,000 字符页每次 37,738 字节，最大耗时约 2.16 秒，首批仍按 4 个活跃任务规划。两轮赠送／消费各 28 次，原有余额保持，测试凭证已撤销。见 [运行单](内测MCP-发布运行单.md)和 [证据](performance/mcp10-20261012/README.md)。

## 后续衔接

MCP1–MCP10 已接通并验收凭证、读取、次数、远程协议、Agent 自述、读者界面、运营后台与实际 HTTPS 部署。后续按真实客户端或运营问题开新切片。完整依赖以[切片方案执行顺序](切片方案-内测MCP与调用额度.md#8-切片执行顺序)为准。

原文来源打开已由 MCP7 对接 Reader；凭证和服务端材料访问不依赖私人笔记或分享草稿。早期 MCP 实现记录中的“未发布”是历史状态，当前以本入口和发布运行单为准。
