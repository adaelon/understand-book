# MCP5 远程 MCP 完整路径

日期：2026-10-11。状态：MCP5 已实现，未提交／发布。

依据：[切片方案 §3–§5／MCP5](切片方案-内测MCP与调用额度.md)、[ADR-0162](adr/0162-beta-remote-mcp-and-prepaid-call-credits.md)。

## 实施进度

- [x] `/mcp` 专用路由、2025-11-25 Streamable HTTP 请求／响应、身份与工具调用。
- [x] 参数／授权 → 余额 → 读取与编码 → 原子消费 → 内容交付；精确 Nginx 入口。
- [x] 真实 HTTP／SQLite 验证及受影响回归。
- [x] 更新切片表、架构、代码链路和 checkpoint。

## HTTP 与协议合同

`multi_user_host::dispatch` 将精确 `/mcp` 路径交给 `remote_mcp::dispatch`。站点 Host 必须匹配；Origin 缺省可接入，提供时必须匹配配置。每次请求重新调用 `McpAccess::authenticate`，使用独立 Bearer 身份。全部响应沿宿主的 `Cache-Control: no-store`。

| 请求 | 响应 |
| --- | --- |
| POST `initialize` | 返回实际支持版本 `2025-11-25`、tools 能力及服务标识；客户端提出其他版本时仍返回该支持版本 |
| POST `notifications/initialized` 等通知、客户端响应 | 202，空正文；通知不执行内容工具 |
| POST `ping` | JSON-RPC 空结果 |
| POST `tools/list` | 一次返回 11 个工具：目录、九个纯读取工具、额度查询 |
| POST `tools/call` | HTTP 200，MCP `content`、`structuredContent`、`isError` |
| GET／DELETE／其他方法 | 405，`Allow: POST` |
| 缺失／无效 Bearer | 401，Bearer challenge |
| 非本站 Host／Origin | 403 |
| 非 JSON Content-Type／未声明两个 Accept 类型 | 415／406 |
| 无效 JSON／JSON-RPC／参数／协议版本 | 400 和协议错误；未知方法／工具为 JSON-RPC 错误 |

POST 使用 `Content-Type: application/json` 与 `Accept: application/json, text/event-stream`。初始化后请求携带 `MCP-Protocol-Version: 2025-11-25`。服务不分配 Session ID；缺少后续版本头时无法关联初始化版本，按传输合同的旧版本默认值处理并拒绝，不默认为当前版本。

协议依据：[2025-11-25 传输规范](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports)、[生命周期规范](https://modelcontextprotocol.io/specification/2025-11-25/basic/lifecycle)。工具参数／来源／分页继续沿 [MCP2 合同](内测MCP-MCP2实现.md)。

## 读取与消费顺序

```text
Host／Origin → 本次 Bearer 身份 → JSON-RPC 与工具选择
目录／额度 → 免费查询 → MCP 结果
内容 → McpRead.prepare（参数、确切发布授权，不装载正文）
     → McpUsage.status（无额度即停止）
     → McpRead.read_prepared（load 再授权、原文／投影与来源）
     → 准备 JSON Value 与文本内容
     → McpUsage.consume_success（短事务扣次、消费事实、扣后余额、提交）
     → 合并扣后余额 → 返回 MCP 结果
```

`McpRead::call` 保留原库入口，复用相同准备及读取步骤。读取和编码准备均在消费事务之前；内容已是 `serde_json::Value`，提交后只有 JSON 值和类型化消费回执的组装，没有文件读取、工具执行或自定义内容序列化。事务失败丢弃准备结果，失败响应没有正文或来源。

成功内容返回的 `usage` 包括 MCP4 的完整余额，加 `charged_units: 1`、`charged_from`；免费目录和额度查询为 `charged_units: 0`、`charged_from: null`。JSON-RPC id 只用于响应关联，重读、续页和成功空检索分别消费一次；两个凭证共用账号余额。最终消费事务裁定并发最后一次的归属。

## 错误与下一步

工具业务错误在 `structuredContent` 和文本结果中返回同一对象：`error_code`、原始稳定 `cause_code`、`message`、`retryable`、`next_step`、`charged_units: 0`。正常结果的文本与结构化投影也一致。

| error_code | 含义／行为 |
| --- | --- |
| `unauthenticated` | 检查或重建凭证 |
| `invalid_arguments` | 按 schema 或返回的续页参数修正 |
| `material_unavailable` | 查询目录，改选获准且有该能力的发布 |
| `quota_exhausted` | 停止内容重试；返回当前免费／付费余额和免费恢复时间 |
| `service_busy` | 等待返回的 `retry_after_seconds` 后重试 |
| `service_unavailable` | 读取或记账失败，稍后重试；本次不扣次 |

## 代理入口

`scripts/linux/nginx-multi-reader.conf.example` 的 `location = /mcp` 显式保留 Authorization、MCP-Protocol-Version、MCP-Session-Id 和可选 Origin，固定后端 Host，关闭代理缓存和缓冲。普通 `/api/` 继续清除 Authorization。公开 HTTPS 部署验证沿 MCP10。

## 已知限制

- 购买界面沿 MCP7；当前 `top_up_url: null`，`top_up_instruction` 明确联系内测管理员购包。MCP7 交付实际入口后填入该字段。
- Agent 自述已由 [MCP6](内测MCP-MCP6实现.md)补齐，真实 Agent 客户端版本验收沿 MCP9。来源链接的 Reader 打开与登录恢复沿 MCP7。
- 消费提交后连接中断仍保留消费事实，重新成功调用再次计次。
- 尚未提交／发布；本机没有可用的 Nginx／WSL 环境，未执行 HTTPS 代理运行验收。

## 验证

| 运行 | 结果 |
| --- | --- |
| MCP5 `remote_mcp` 定向 | 6 通过，0 失败，0 忽略；[日志](performance/mcp5-20261011/remote-mcp.log) |
| MCP1–MCP4／控制库／ADM2–ADM3 受影响回归 | 46 通过，0 失败，0 忽略；[日志](performance/mcp5-20261011/regression.log) |

`remote_mcp_tests.rs` 使用真实 TCP HTTP 和临时 SQLite，覆盖初始化版本协商、通知空响应、11 个工具发现、两个账号目录、两凭证共用次数、原文与续页坐标、文本与结构化投影一致、正常空检索和同 id 重读记次、免费转付费、耗尽后购包恢复；协议／身份／Host／Origin 拒绝、缓存撤权、错误参数、无能力、无效 LID、记账触发器故障回滚且不交付正文、两个 HTTP 客户端竞争最后一次、零额度不装载正文，以及私人现场／模型账本零写入。

原读取层四项回归通过，保留嵌套投影拼接与授权分页语义；原网页凭证 API 的 Cookie／CSRF 边界和两类人工收款回归通过。构建命令、缓存保留位置及验证范围见 [验证记录](performance/mcp5-20261011/README.md)。
