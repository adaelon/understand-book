# MCP1 接入凭证实现

日期：2026-10-11。状态：MCP1 已完成，尚未提交或发布。

依据：[ADR-0162 §1／§7](adr/0162-beta-remote-mcp-and-prepaid-call-credits.md)、[切片方案 MCP1](切片方案-内测MCP与调用额度.md#mcp1--内测账号接入凭证)。

## 本人账号接口

三个接口沿 `Site::validate`、Cookie 认证及写请求 CSRF 检查，在 `Authorization::context` 之前分发；操作不创建 UserRuntime 或 ReaderWorkspace。身份取当前网页登录者，不接受指定账号。

| 请求 | 输入 | 成功响应 |
| --- | --- | --- |
| `GET /api/account/mcp` | 无查询参数、无正文 | `{ "credentials": [Credential], "usage": Usage, "mcp_url": "站点/mcp" }` |
| `POST /api/account/mcp/credentials` | `{ "label": "我的 Agent" }` | `{ "credential": Credential, "token": "<仅本次显示的完整值>" }` |
| `POST /api/account/mcp/credentials/{id}/revoke` | 无查询参数，正文为空或 `{}` | `{ "revoked": true }` |

```text
Credential = { credential_id, label, created_at, revoked_at, active }
```

MCP7 增加的 `usage` 为本人免费／已购次数与各包期限，包含购买入口且本次扣次为 0；完整响应见 [MCP7](内测MCP-MCP7实现.md)。

标签去除首尾空白后为 1–80 个字符。列表按创建时间倒序，同时间按凭证 ID 倒序；时间为 Unix 秒。列表包含已撤销和旧授权代次的凭证，`active` 表示当前仍有效。`revoked_at` 为单独撤销的首次时间，旧代次失效不会伪造撤销时间。

完整值只出现在创建成功响应中；列表不返回凭证值、校验数据、用户 ID 或授权代次。重复撤销本人的同一凭证成功并保留首次时间；他人凭证与不存在的 ID 均返回 404。错误方法返回 405，非法输入返回 400，未登录／失效会话返回 401，站点或 CSRF 拒绝返回 403。

## 身份与失效

`McpAccess::authenticate(authorization: Option<&str>) -> Result<McpIdentity, ToolError>` 接收 Bearer 请求头，返回只含账号和凭证 ID 的独立身份。每次查询当前凭证、账号启用状态和 `auth_epoch`，失败返回 `MCP_AUTH_REQUIRED`／authentication。数据库故障返回 `MCP_ACCESS_STORAGE_UNAVAILABLE`／unavailable。

凭证由 `ub_mcp_` 前缀和 32 个随机字节的 URL-safe Base64 值组成。校验沿现有 `auth_sessions` 约定使用 SHA-256 摘要，数据库不保存完整值。MCP 身份不能转换为网页 Principal，MCP 凭证不能用作网页 Cookie；网页登录令牌也不能作为 MCP 凭证。

单独退出或网页登录过期不撤销 MCP 凭证。停用、重新启用、改密／重置密码或撤销全部授权沿原账号写入路径递增 `auth_epoch`，旧 MCP 凭证失效。重新启用或重新登录不会恢复旧凭证；新登录后可另行创建。

创建和撤销在 immediate 事务内复查 Principal；列表在同一读事务内复查会话并投影有效状态。服务端不缓存凭证有效性。

## 存储与接续

`ControlStore::open` 的 schema 10 增加 `mcp_credentials`：凭证 ID、账号外键、签发时授权代次、唯一校验数据、标签、创建及撤销时间。新库直接安装；既有 schema 1–9 沿原升级链到 10，新增表／索引与版本号在同一事务提交。已有账号、会话和私人现场保持原值。

```text
账号请求 → Host/Origin + Cookie/CSRF → McpAccess.create/list/revoke → control.sqlite
Authorization Bearer → McpAccess.authenticate → 当前账号/凭证/epoch → McpIdentity
```

MCP5 可从 `Authorization.mcp_access` 每次解析远程身份，再经 PublishedLibrary 检查材料授权。MCP7 接入账号界面时使用上述接口；完整值只驻留创建结果，不提供回查。

## 验证

| 检查 | 结果 |
| --- | --- |
| `cargo test -p server --lib mcp_access` | 最终 6 项通过，0 忽略；[日志](performance/mcp1-20261011/mcp-access-final.log) |
| `cargo test -p server --lib -- mcp_access auth::tests tests::mu4_tests control_store:: tests::inv7_tests` | 27 通过、10 失败；其中原认证／密码／控制库 22 项通过，当时 MCP1 5 项通过，旧 MU4 10 项在测试准备阶段失败；[日志](performance/mcp1-20261011/server-tests.log) |

新增用例在真实临时 SQLite 中覆盖两账号隔离、创建与列表投影、完整值不落库、单独撤销和重复撤销、账号停用／重新启用、改密和撤销全部授权、过期／退出会话、重启持久性、schema 9→10 升级保值及失败回滚。现有控制库用例另验证 schema 1–8 升级保持原字段并与新库一致。

账号路由用例经过实际 Host 分发，验证 Host／Origin／Cookie／CSRF、请求方法、拒绝指定他人账号和错误输入，以及不装载私人目录／阅读现场。最后增加真实 TCP HTTP 用例，从网页登录取得 Cookie/CSRF，创建凭证、列出元数据并撤销；响应沿现有 `Cache-Control: no-store`，没有把 MCP 凭证写入网页 Cookie。

## 已知限制

- 旧 MU4 的 10 项用例都在 `tests/mu4_tests.rs:76` 的共用 `seed_private` 失败：`save_agent_history_path` 对已建立 JSONL 存储的会话返回 `SESSION_EVENT_WRITE_REQUIRED`，未进入认证／材料断言。该准备代码及 JSONL 写入门禁未由本片改动；修复应让旧测试样本使用现有事件写入路径。此轮没有将这 10 项记为通过或修改它们。
- 本片提供凭证服务及账号 API；读取和记次库入口已由 MCP2／MCP3 提供，购包管理由 MCP4 接入；远程 `/mcp` 协议沿 MCP5、账号界面沿 MCP7 实施。
- 创建响应丢失后无法回查完整凭证；用户可从列表撤销该条并重新创建。
- 未发布或修改线上数据库。上线时沿后续发布切片备份控制库；旧二进制不能打开 schema 10。
