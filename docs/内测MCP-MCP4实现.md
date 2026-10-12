# MCP4 调用包与人工收款

日期：2026-10-11。状态：MCP4 已完成；未提交／发布。

依据：[切片方案 §5／§6／MCP4](切片方案-内测MCP与调用额度.md)、[ADR-0162 §4／§5](adr/0162-beta-remote-mcp-and-prepaid-call-credits.md)。

## 实施进度

- [x] schema 12、购包批次、明确消费来源、免费与到期优先扣除。
- [x] 管理员购包／更正 API、接入既有当月赠送、两类收款投影。
- [x] 真实 SQLite 定向测试和受影响收款／schema 回归。
- [x] 更新切片表、架构、代码链路和 checkpoint。

## 管理接口

所有请求经既有 Host／Origin、网页登录、写请求 CSRF 和 ReaderAdmin 权限检查，位于私人 Reader 现场装载之前。

| 请求 | 输入／输出 |
| --- | --- |
| `GET /api/admin/users/{user_id}/mcp` | 当前香港月份、免费余量、有效已购余量、各批次余额与期限 |
| `POST /api/admin/users/{user_id}/mcp/packs` | `{operation_id, pack_count, paid_at, channel, external_ref?, note}`；返回操作回执、`receipt_id` 和 `call_pack` |
| `POST /api/admin/users/{user_id}/mcp/gifts` | `{operation_id, units, reason}`；复用 `McpUsage::gift`，返回原月份赠送回执 |
| `POST /api/admin/users/{user_id}/mcp/corrections` | `{operation_id, receipt_id, revision, delta_fen, delta_units, reason}`；返回操作回执和更正后的 `call_pack` |

金额与次数由正整数 `pack_count` 计算：每包 1,000 分／1,000 次，接口不接受覆盖单价或发放数。一次收款购买的同批包共享开通时间与期限，作为一条批次保存 `pack_count`、`unit_price_fen`、`units_per_pack`、合计价格和原始总次数。收款时间 `paid_at` 不决定有效期；开通时间取服务端管理操作时间。

有效区间为 `[opened_at, expires_at)`；香港时间增加 12 个日历月，目标月份无对应日则取月末。例如 2028-02-29 23:30 开通，到 2029-02-28 23:30 到期。数据库使用 Unix 秒，包及余额响应时间采用带 `+08:00` 的 RFC 3339 字符串。

## 事务与扣除

```text
购包 → AdminStore::apply → BEGIN IMMEDIATE → 比较原操作业务字段
     → admin_operations + manual_receipts + mcp_call_packs → COMMIT
成功内容准备 → McpUsage::consume_success → BEGIN IMMEDIATE → 取业务时间
     → 当月免费扣 1；免费耗尽则最早到期的有效包扣 1
     → mcp_usage 保存确切来源 → 读取扣后余额 → COMMIT → 交付内容
```

同一管理员的操作号、账号、操作种类及规范化业务字段相同，返回原回执；跨月重试也不重发。字段变化返回 `ADMIN_OPERATION_CONFLICT`。渠道和交易编号沿用两类收款共用的唯一约束，重复登记返回 `RECEIPT_EXTERNAL_REF_CONFLICT`。任何写入失败均回滚整笔操作。

免费次数优先；付费按 `expires_at, opened_at, pack_seq` 排序，同一秒开通的批次按数据库登记顺序。过期批次保留历史与未用次数，但可用次数为零。消费回执 `charged_from` 为 `{kind:"free_month", month_hk}` 或 `{kind:"call_pack", pack_id}`。同账号所有凭证共用余额。

`Usage` 延续免费字段，增加 `paid_remaining`、`paid_next_expires_at` 和 `call_packs`。批次区分 `total_units`、`adjusted_units`、`used_units`、`remaining_units` 与当前 `available_units`；下次到期时间只取仍有可用次数的批次。新月份免费次数恢复，已购余量保持。

## 更正与收款投影

更正必须指定原 MCP 收款和当前批次 `revision`。`delta_fen` 调整收款登记，`delta_units` 调整次数，两者至少一项非零；金额、次数分别追加 `receipt_corrections`、`mcp_pack_adjustments`，与操作回执及批次修订号同事务提交。正数次数更正可恢复或补足误录次数，保留原购买条款与独立调整事实；它不延长有效期。负数最多扣回尚未消费次数。

已消费次数及消费事实保持不变。金额更正后不得为负，次数更正后授予总数不得低于已用数。旧修订号返回 `MCP_PACK_REVISION_CONFLICT`，扣回已消费次数返回 `MCP_PACK_ALREADY_CONSUMED`；同号重放先于修订号检查。两类收款不能借用对方的更正入口。

既有 `GET .../receipts` 与 `GET .../receipt-corrections` 增加 `entitlement_kind`（`model_allowance` 或 `mcp_call_pack`）和可空 `pack_id`。收款中的 `period_id` 对 MCP 为 null，对模型额度仍指原授予期。原始和有效金额分别呈现。MCP 购包、更正、赠送均不写模型额度期、额度调整或模型消费。

## 持久化

`ControlStore::open` 新库安装及旧库顺序升级到 schema 12；新增 `mcp_call_packs`、`mcp_pack_adjustments`，重建 `mcp_usage` 以容纳两类来源并复制原消费事实。消费的来源引用通过按类型生成的外键列约束到同一账号的确切月份或批次。迁移与版本号在同一事务提交。

## 验证

| 运行 | 结果 |
| --- | --- |
| MCP4 定向 `mcp_pack` | 8 通过、0 失败、0 忽略；[日志](performance/mcp4-20261011/mcp-pack.log) |
| 免费次数、控制库、MCP1／MCP2、ADM2／ADM3 回归 | 38 通过、0 失败、0 忽略；[日志](performance/mcp4-20261011/regression.log) |

真实临时 SQLite 覆盖：购包同号重试／跨月重放、字段冲突、两类收款共用交易编号去重、失败回滚与释放句柄后重启；免费优先、跨月余量、月末／闰年期限、到期排除与同到期批次顺序；金额与次数独立更正、修订号冲突、不可扣回已消费次数、消费事实保留；两连接争最后一次；schema 11 消费保值升级、失败原子回滚和来源外键；经真实 Host 分发的混合收款、两类更正、赠送复用、权限与参数拒绝。

本机默认构建遇到磁盘／PDB 环境错误；最终以相同 Rust 测试程序关闭调试输出运行，构建命令、测试命令与环境处理见 [验证记录](performance/mcp4-20261011/README.md)。

## 已知限制

- 远程内容协议和读取／扣次衔接沿 MCP5；读者和管理员页面沿 MCP7／MCP8。
- 金额更正仅登记业务事实，不代表外部退款已完成。
- 未提交／发布，线上数据库未变更；旧二进制不能读取 schema 12。
