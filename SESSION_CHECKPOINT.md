# SESSION_CHECKPOINT — 2026-10-11 12:11（Asia/Hong_Kong）

## 新鲜度自检
- 写入时最新 commit：`de6376d feat: integrate reading notes RN1-RN4 and sharing RS1-RS8`。
- 读入时对比 `git log -3`；若不同，以实际 Git 和工作树为准。
- 工作树：`E:/allwork/download/agent/understand-book-reading-share`；分支 `codex/reading-share`。MCP0–MCP9 尚未提交／发布。

## 当前在做什么
**MCP9「集成与新 Agent 验收」已完成，下一片是 MCP10「发布与运行单」。**

双账号、真实 HTTP 购包／回执恢复、来源到 Reader 的 UTF-16 解析、并发消费与实际客户端阅读已验收。生产模块和 schema 12 未变；本轮新增验收路径，并修正本机 MCP 过时 nullable schema 测试。[MCP9 实现](docs/内测MCP-MCP9实现.md)、[证据与命令](docs/performance/beta-mcp/README.md)。

## 下一步（可直接接手）
1. 按冷启动读序接手；保留 MCP0–MCP9 未提交成果，读取切片方案 MCP10 与 MCP9 已知限制。
2. 编写 `docs/内测MCP-发布运行单.md`，基于 `ControlStore::open`、schema 12 和既有 Linux 脚本写成套备份／升级／恢复步骤。
3. 在临时目录验证备份恢复及旧服务对 schema 12 的行为，核对旧账号、材料和模型费用；不得覆盖丢失新收款／消费。
4. 在目标 Linux／HTTPS 代理验证 `/mcp` 的认证／协议头及明确赠送次数的实际扣次；按 4 个活跃读取任务起步复测容量。
5. 完成 MCP10 发布验收后同步切片表与上线状态；本次仍是未发布状态。

## 未提交 / 未完成
- MCP9 已测待提交：`mcp_acceptance_tests.rs` 与测试模块接线、`mcp-source.integration.test.ts`、两种客户端启动脚本及引用／账本校验脚本、验收记录；`mcp.rs` 仅更新 `granularity.anyOf` 枚举及 null 断言。
- 最终自动验证：2 项新增集成／并发用例、87 项既有 Server 受影响回归通过；Web 13 文件 126 项、Admin 6 文件 30 项及两端类型检查／生产构建通过。首轮夹具重复登录触发限流，改复用 Cookie 后通过，产品断言未放宽。
- 真实 Agent：Codex CLI `0.162.0-alpha.2`（桌面自带）、用户配置 `gpt-6-astra`、协议 `2025-11-25`；两个无项目背景的新进程。alice 获赠 12 次，选材／检索／两页读取成功，2 条引用解析正确，SQLite 3 条消费、剩 9 次；bob 从免费目录的零余额停止，没有内容消费。[原始回合与账本](docs/performance/beta-mcp/README.md#真实-agent-结果)。
- 用户已明确允许**仅本次临时验收进程**预批准测试 MCP 的 11 工具；未修改全局配置。测试宿主均已退出，临时凭证配置移除、临时库关闭。资料保留于 `D:/codex-build-cache/understand-book-reading-share-mcp9/agent-approved/`，可复跑校验脚本。
- 客户端限制：Claude Code `2.1.196` 初始化和 11 工具发现成功，但模型服务凭证 401，完整阅读未验；系统 Codex `0.147.0` 不支持当前模型。可用客户端证据与上述失败分别记录，不等同于 HTTPS 上线。
- 小规模测量：1／4／8 同时读取均成功，8 请求最慢 37.725 ms，每请求约 50,950 字节；单个小型缓存材料、本机 TCP，不推算生产 QPS。MCP10 在目标环境复测。
- 既有 MU4 10 项本轮复现仍在 `mu4_tests.rs:76` 旧 snapshot 写 JSONL 时触发 `SESSION_EVENT_WRITE_REQUIRED`，业务断言未运行；需另片将样本改为事件写入，保留门禁。本机 MCP 旧 schema 失败已修复并通过。
- MCP0–MCP8 成果保留：凭证、获准读取、免费／付费次数、Streamable HTTP、Agent 自述、读者接入与来源定位、运营后台购包／赠送／更正。[实施入口](docs/内测MCP-实施入口.md)。账号 MCP 查询仍仅支持启用账号；金额更正不代表外部退款。
- 构建沿 `CARGO_INCREMENTAL=0`、`CARGO_PROFILE_TEST_DEBUG=0`、D 盘 TEMP、单并发 `cargo rustc -j 1 -p server --lib --profile test -- -C link-arg=/DEBUG:NONE`。本轮将 `target/debug/incremental/runtime-2eogrjxmh0z0p` 移到 `D:/codex-build-cache/understand-book-reading-share-mcp9/preserved/` 保留，未删除；其他历史缓存位置见 MCP4–MCP6 验证记录。
- MCP10 未实施；实际 Nginx／HTTPS、Linux 发布、备份恢复未验。本机无可用 Nginx／WSL；精确 `/mcp` 代理配置已保留凭证／协议／Origin 头。Rust ts-rs 属性警告、Web／Admin 大 chunk 提示保留。
- RN1–RN4、RS1–RS8 已集成于 HEAD；RN4 实体平台及 RS3 原生保存、安装后资源／Release 待验；Windows Debug NSIS 候选未安装／发布。[RS3](docs/performance/reading-share-rs3-20261010/README.md)、[RN4](docs/performance/reading-notes-rn4-20261010/README.md)。
- Computer Use 保持用户停止状态；本轮无实体浏览器视觉验收。原 `tmp/`、`packages/web/test-results/` 保留。真实书籍样本在忽略目录 `.understand-book/quantification-essence`。
- MU10 百回合恢复 queued 0／预期 100 为先前独立失败，本轮未复验；Tutor T19 真实体验待验。
- 其他工作线：[EX](checkpoint_ex.md)、[JSONL](SESSION_CHECKPOINT_JSONL.md)、[DSH](SESSION_CHECKPOINT_DSH.md)、[运营后台](SESSION_CHECKPOINT_ADMIN.md)、[ADM](checkpoint_ADM.md)、[INV](checkpoint_INV.md)、[工具契约](SESSION_CHECKPOINT_TOOL_CONTRACTS.md)、[LangSmith](SESSION_CHECKPOINT_LANGSMITH_FULL.md)。

## 冷启动读序
1. [MCP 实施入口](docs/内测MCP-实施入口.md)、[产品约定](docs/内测MCP-产品约定.md)、[ADR-0162](docs/adr/0162-beta-remote-mcp-and-prepaid-call-credits.md)；`CONTEXT.md` 开头五项 MCP 术语、ManualReceipt、ReaderAdmin 与 PublishedBookRef。
2. [切片方案](docs/切片方案-内测MCP与调用额度.md) §3–§7、MCP9／MCP10 与第 9–10 节；[MCP 接入](docs/MCP接入.md)全文。
3. [MCP9 实现](docs/内测MCP-MCP9实现.md)、[验收记录](docs/performance/beta-mcp/README.md)全文；[MCP4](docs/内测MCP-MCP4实现.md)持久化／更正合同与 [MCP5](docs/内测MCP-MCP5实现.md)HTTP／代理合同。
4. `crates/server/src/control_store.rs:ControlStore::open`、`mcp_access.sql`、`mcp_usage.sql`、`mcp_packs.sql`；`scripts/linux/nginx-multi-reader.conf.example` 及拟沿用的 Linux 备份／发布脚本。
5. [架构](docs/架构.md) MCP4–MCP8 与 [代码链路](docs/代码链路.md) MCP9；需要复跑时读 `mcp_acceptance_tests.rs`、`scripts/mcp9-codex-acceptance.mjs` 与 `scripts/verify-mcp9-agent.mjs`。

## 本会话决策摘要
- MCP9 落实既有 ADR-0162；新增测试与客户端证据，生产模块／schema 保持原合同。首批按 4 个活跃读取任务规划，实际 Linux 容量在 MCP10 复测。[验收记录](docs/performance/beta-mcp/README.md)。
