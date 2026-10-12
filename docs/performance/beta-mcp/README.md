# MCP9 验收记录

日期：2026-10-11。当前实施状态见 [MCP9](../../内测MCP-MCP9实现.md)。

## 运行目的

- 集成测试检测双账号材料／余额串用、购包恢复重复发包、撤权后续页返回正文；失败修复账号、回执或读取接点。
- 来源验收以真实 HTTP 输出调用 Web 的来源解析函数，检测版本、范围或 UTF-16 坐标不能还原原文；失败修复来源生成或 Reader 解析。
- 受影响回归检测远程入口损坏认证、材料、收款及本机 MCP；Web／Admin 检测账号接入、来源恢复和操作回执；构建检测类型或生产资源打包失败。
- 新 Agent 仅拿接入说明和问题，引用必须能解析到实际读过的原文；消费数以 SQLite 事实核对，不采用模型自评。
- 1／4／8 并发测量响应耗时与字节量，并逐请求断言成功与记账数；用于给首批试运行设置保守并发范围，不充当 Linux 容量承诺。

## 运行环境

Windows 本机，真实 TCP HTTP、临时 SQLite schema 12、已发布测试文本。测试账号和收款均为夹具。外部 Agent 的 alice 获赠 12 次，bob 模拟耗尽；没有消耗正式 MCP 付费次数。

沿此前构建设置：`CARGO_INCREMENTAL=0`、`CARGO_PROFILE_TEST_DEBUG=0`、`TEMP/TMP=D:/codex-build-cache/understand-book-reading-share-mcp4/temp`。因 E 盘空间不足，核对路径后将 `target/debug/incremental/runtime-2eogrjxmh0z0p` 移到 `D:/codex-build-cache/understand-book-reading-share-mcp9/preserved/` 保留，未删除。

## 自动化结果

| 检查 | 结果与证据 |
| --- | --- |
| Rust 库测试构建 | 通过；[最终构建](build-final.log) |
| 双账号／购包恢复／来源输出 | 修正夹具后 1 通过；[最终日志](integration-final.log) |
| 1／4／8 并发及消费账本 | 1 通过；[测量日志](integration-initial.log) |
| Server 既有受影响回归 | 87 项全部通过；[日志](server-regression.log) |
| Web 账号／来源／Reader／网络回归 | 13 文件，126 项通过；[日志](web-tests.log) |
| Admin 全部测试 | 6 文件，30 项通过；[日志](admin-tests.log) |
| Web／Admin 类型检查与生产构建 | 两者退出码 0；[Web](web-build.log)、[Admin](admin-build.log) |

Server 回归命令中的 `mcp::tests::` 子串同时匹配了新增 `remote_mcp::tests::mcp_acceptance`：因此原日志总数为 88 通过、1 失败、1 忽略。其中新增并发用例通过，新增双账号用例触发前述夹具限流，手动外部客户端夹具按设计忽略。双账号修正后的独立重跑通过；未重复运行已经通过的 87 项既有回归。下方重跑命令已排除这项重复匹配。

## 小规模并发观察

| 同步请求数 | 总耗时 ms | 请求中位 ms | 最慢 ms | HTTP JSON 正文字节总数 |
| --- | ---: | ---: | ---: | ---: |
| 1（首次装载） | 64.720 | 62.419 | 62.419 | 50,950 |
| 4（已缓存） | 30.935 | 19.684 | 21.813 | 203,800 |
| 8（已缓存） | 39.553 | 28.415 | 37.725 | 407,600 |

每次原文为 8,000 字符，HTTP 返回同时含文本 JSON 和结构化结果；13 次请求全部成功，对应 13 条消费事实。这里只测一个小型发布、两个账号和本机缓存读取，未测多大书混合工作集或长期吞吐。首批试运行先按 **4 个同时活跃读取任务**规划，在 MCP10 的目标 Linux／HTTPS 环境复测后再提高；不以本机突发测试推算注册用户数或生产 QPS。

## 已观察到的首轮失败

- [旧测试复现](legacy-initial.log)：本机 `mcp_tool_characterization` 仍断言 `granularity.enum`；共享合同已有 `anyOf[0].enum` 与 null 分支，已更新测试并验证通过。
- 同一日志中 MU4 10 项在 `mu4_tests.rs:76` 使用旧 snapshot 写入 JSONL 会话，触发 `SESSION_EVENT_WRITE_REQUIRED`，尚未改动。未放宽事件写入门禁。
- [新增集成首轮](integration-initial.log)：测试辅助函数每次请求重新登录，触发 `LOGIN_RATE_LIMITED`；已改为整个用例复用同一管理员 Cookie，保留实际限流。
- [客户端首轮](agent-client-initial.log)：隔离 `setting-sources` 后没有继承原用户配置中的认证环境。修正启动脚本仅继承原 `env` 和模型选择，仍不加载项目背景、插件、hooks 或历史。
- Claude Code 2.1.196 随后成功连接并发现 11 工具，但模型服务拒绝现有 token（401）；[连接证据](claude-compatibility.json)。未记任何 MCP 内容消费。
- Codex CLI 0.147.0 被模型服务要求升级；改用本机桌面自带 0.162.0-alpha.2，保持原模型选择。其首次目录调用要求审批；用户明确授权本次临时验收的 11 工具后，以进程级配置重跑，未修改全局设置。

## 真实 Agent 结果

| 项目 | 结果 |
| --- | --- |
| 客户端／模型 | codex-cli 0.162.0-alpha.2／用户配置的 gpt-6-astra；[元数据](agent-client.json) |
| 协议／传输 | 2025-11-25／Streamable HTTP，本机真实 TCP |
| 正常账号 | 自主选材、检索和两页原文，正确回答 18 升及南池未参与；[原始回合](agent-reader.jsonl) |
| 耗尽账号 | 免费目录返回零余额后停止；准确说明恢复时间及购买入口；[原始回合](agent-exhausted.jsonl) |
| 引用 | 2 条链接均指向实际读过的确切发布与 UTF-16 范围；[回答与校验摘要](agent-verification.json) |
| 账本 | 3 条免费消费，赠送 12 次剩 9 次，bob 零消费；[原文与账本](agent-evidence.json) |
| 确定性判定 | [verify-mcp9-agent.mjs](../../../scripts/verify-mcp9-agent.mjs) 退出码 0；[日志](agent-verification.log) |

验收输入仅为 [接入说明](../../MCP接入.md)及“查清北池减少多少升水，说明南池是否参与，附来源和短引文”的问题；耗尽账号使用独立进程提出相同主题的问题。已禁用项目说明、额外 MCP、插件、技能、shell、网络搜索与会话持久化。使用 [Codex 连接配置](https://learn.chatgpt.com/docs/extend/mcp?surface=cli)中的 URL、Bearer 环境变量和本次进程的工具审批设置；[配置项说明](https://learn.chatgpt.com/docs/config-file/config-reference)。

## 重跑入口

```powershell
$env:TEMP='D:\codex-build-cache\understand-book-reading-share-mcp4\temp'
$env:TMP=$env:TEMP
$env:CARGO_INCREMENTAL='0'
$env:CARGO_PROFILE_TEST_DEBUG='0'
cargo rustc -j 1 -p server --lib --profile test -- -C link-arg=/DEBUG:NONE
$env:MCP9_SOURCE_EVIDENCE=(Join-Path (Get-Location) 'docs/performance/beta-mcp/source-evidence.json')
& 'target/debug/deps/server-6b365f9831000c84.exe' mcp9_two_accounts mcp9_small --nocapture --test-threads=1
& 'target/debug/deps/server-6b365f9831000c84.exe' remote_mcp::tests::remote_mcp remote_mcp::tests::mcp_contract mcp_access:: mcp_read:: mcp_usage:: mcp_pack:: auth::tests:: auth_password:: control_store:: tests::mcp_pack_api_tests tests::adm2_tests tests::adm3_tests tests::mu3_tests mcp::tests:: --skip mcp_acceptance --test-threads=2
pnpm --filter @understand-book/admin test
pnpm --filter @understand-book/admin build
pnpm --filter @understand-book/web build
```

测试产物路径以后续 Cargo 输出为准。外部 Agent 需在单独进程以全新 `MCP9_AGENT_DIR` 启动 `mcp9_external_agent_fixture --ignored --nocapture`，READY 出现后运行 `node scripts/mcp9-agent-acceptance.mjs <fixture目录> <Claude可执行文件>`；脚本结束创建 STOP，宿主输出无凭证的 evidence.json 并关闭。

本次通过的客户端入口为 `node scripts/mcp9-codex-acceptance.mjs <fixture目录> <Codex可执行文件>`；该脚本只用于已获授权的隔离夹具。宿主结束后运行 `node scripts/verify-mcp9-agent.mjs <fixture目录>`，核对引用和 SQLite 消费。提交的证据文件加了 `agent-` 前缀以与 HTTP 来源样本区分。

Web 回归命令：`pnpm --filter @understand-book/web exec vitest run src/mcp-source.integration.test.ts src/components/AccountMcp.test.ts src/mcp-source.test.ts src/NetworkApp.mcp.test.ts src/App.startup.test.ts src/NetworkApp.recovery.test.ts src/NetworkApp.resumption.test.ts src/NetworkApp.chat.test.ts src/NetworkApp.recap.test.ts src/network-client.test.ts src/components/AccountSettings.test.ts src/components/ReaderPane.test.ts src/reader-navigation.test.ts`。

## 已知限制

本机无实际 HTTPS 代理验收；Linux 发布、备份恢复与代理在 MCP10 完成。Computer Use 保持停止，本轮没有实体浏览器视觉验收。Web／Admin 构建保留既有大 chunk 提示，Rust 保留既有 ts-rs 属性警告。
