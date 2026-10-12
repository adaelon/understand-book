# MCP3 月度免费次数

日期：2026-10-11。状态：MCP3 已完成，未提交／发布。

依据：[切片方案 §5／§6／MCP3](切片方案-内测MCP与调用额度.md)、[ADR-0162 §3／§5](adr/0162-beta-remote-mcp-and-prepaid-call-credits.md)。沿用已确认的账号月份、当月赠送和成功记次合同。

MCP4 已扩展付费来源、购包管理与完整余额字段，当前合同见 [MCP4 实现](内测MCP-MCP4实现.md)；本页记录免费次数规则与 MCP3 验证。

## 服务入口

```text
McpUsage::new(ControlStore)
  .status(owner) → Usage
  .consume_success(owner, tool, PublishedBookRef) → Consumption
  .gift(Principal, operation_id, owner, units, reason) → 管理员操作回执
```

`status` 和 `consume_success` 是受信任的服务端入口，账号由调用方从本次认证身份取得。服务会检查账号仍启用；凭证、工具参数与材料授权由 MCP1／MCP2 处理。消费只接受 MCP2 的九个内容工具和非空材料引用，目录、额度查询及模型工具不能进入消费入口。

每月第一次查询、赠送或消费时，按 `(user_id, month_hk)` 建立唯一月份记录，授予完整 1,000 次。账号的全部凭证共用同一记录；首次月中使用、重启和重新开通凭证均不改变当月基数。免费次数与赠送在当月合并可用，跨月不会结转。

```text
Usage = { month_hk, free_granted, free_used, free_remaining, free_resets_at }
Consumption = { call_id, charged_units: 1,
                charged_from: { kind: "free_month", month_hk },
                completed_at, usage: Usage }
```

`free_resets_at` 为香港时区的 RFC 3339 时间，例如 `2026-11-01T00:00:00+08:00`；账本及 `completed_at` 使用 Unix 秒。生产实例使用服务端系统时钟；测试替换实例内时钟，以确定性输入覆盖跨月、跨年和闰年。

## 成功消费

```text
调用方完成身份／参数／材料授权 → status 查询余量
→ 在事务外准备成功内容与可编码响应
→ consume_success：BEGIN IMMEDIATE → 读取当前业务时间
  → 当前香港月份 → used_units < granted_units 时扣 1
  → 插入消费事实 → 读取扣后余额 → COMMIT
→ 调用方取得成功回执后交付内容
```

时间在取得写事务之后读取；月末查询后跨月完成的调用归入完成时的新月份。前置余量只是提示，最终是否可消费由事务判定。两个独立连接竞争最后一次时，只有一个消费成功。

消费 ID 由服务端为每次消费生成，不使用客户端 JSON-RPC 请求号去重。内容重读、分页和正常空检索均由调用方按一次成功调用提交。账本不存检索词或返回原文，只保留账号、工具、材料版本、一个单位、扣除来源与完成时间。事务中任一步失败都会回滚扣次和消费记录。

## 当月赠送

`gift` 接受正整数次数和非空原因，去除原因首尾空白。事务内重新验证网页 Principal 与当前管理员角色；普通用户、失效会话或被撤销的管理员不能赠送。目标账号必须存在且启用。

赠送复用 `admin_operations` 和原有业务字段比较逻辑；同一管理员的同一操作号、目标账号、次数和规范化原因返回原回执，改变业务字段返回冲突。月份由首次成功事务决定，重试跨月仍返回原月份回执。增加余额、操作回执和独立赠送事实同事务提交；赠送不创建收款、模型额度或模型消费。

## 持久化与升级

`ControlStore::open` 升级到 schema 11；新库直接安装，schema 1–10 按原升级链追加本片 SQL，新增表／索引与版本号在同一事务提交。

| 表 | 内容 |
| --- | --- |
| `mcp_free_months` | 账号月份唯一、累计授予与已用次数；约束已用不超过授予 |
| `mcp_free_gifts` | 管理员操作号、目标账号月份、次数、原因和时间；关联原操作回执 |
| `mcp_usage` | 每次成功消费及 `free_month` 来源；引用确切账号月份 |

已存在的账号、凭证、阅读现场、模型费用和收款事实不迁写。消费记录中的材料版本作为历史引用保存，不依赖材料文件持续存在。

## 错误

| 错误码 | 含义 |
| --- | --- |
| `MCP_QUOTA_EXHAUSTED` | 当月次数耗尽；消息带免费恢复时间，要求停止重复内容请求 |
| `MCP_USAGE_INPUT_INVALID` | 非内容工具、缺材料标识、赠送参数无效或次数溢出 |
| `MCP_USAGE_STORAGE_UNAVAILABLE` | 月份、赠送或消费未能读取／提交 |
| `USER_UNAVAILABLE` | 目标账号不存在或已停用 |
| `ADMIN_REQUIRED`／`AUTH_REQUIRED` | 无当前管理员身份／有效会话 |
| `ADMIN_OPERATION_CONFLICT` | 同一操作号具有不同业务字段 |

## 验证

| 运行 | 结果与日志 |
| --- | --- |
| `cargo test -p server --lib mcp_usage` | 8 通过、0 失败、0 忽略；[日志](performance/mcp3-20261011/mcp-usage.log) |
| `cargo test -p server --lib -- control_store:: mcp_access:: mcp_read:: tests::adm2_tests tests::adm3_tests` | 30 通过、0 失败、0 忽略；[日志](performance/mcp3-20261011/regression.log) |

新增用例使用真实临时 SQLite：首次完整额度、账号隔离与最小消费事实；香港月界／年界／闰年；前置查询后跨月，且时间取值发生在写事务内；两个独立连接同时通过前置余量后竞争最后一次；释放全部服务句柄再重启、替换凭证保持余额；非内容工具、停用账号和消费写入失败无消费／半笔月份；管理员角色、会话、赠送同号重试／字段冲突／跨月重放及失败回滚；schema 10→11 保留凭证与阅读现场且升级失败原子回滚。

相关回归验证旧 schema 1–9 升级保值、schema 5–8 与新库结构一致、原凭证和材料读取、管理员权限／重复操作、原收款／额度／更正和故障回滚。MCP1 升级测试的最终版本断言改用当前版本，账号 schema 用例的固定当前版本更新为 11，历史版本断言保持原值。

## 已知限制

- 本片交付服务端库入口。读取失败不调用消费入口、可编码内容准备、最终响应交付与扣次的连接由 MCP5 实施，当前未开放远程 HTTP；管理员赠送 API 已由 MCP4 接入。
- 付费包、免费优先与包到期排序已由 MCP4 实施，完整额度字段见 MCP4。购买页面和远程错误投影由后续接入层补齐。
- 成功消费提交后响应丢失仍保留记次，重新发起的成功调用再次消费。
- 未提交或发布，线上数据库未变更；旧 schema 10 二进制不能打开 schema 11。
