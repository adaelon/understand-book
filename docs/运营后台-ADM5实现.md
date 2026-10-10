# ADM5 预占、结算与恢复实现合同

日期：2026-10-08。实现位于当前工作树，未提交、未部署。设计依据为 [ADR-0156](adr/0156-reader-admin-and-account-allowance.md)、[切片方案](切片方案-运营后台与账号额度.md)及 [ADM3–ADM4 合同](运营后台-ADM3-ADM4实现.md)。

## 发送与持久化

`Authorization::new` 建立共享 `ModelSpendStore`，装载服务根下的 `model-rates.json`，并在工作线程启动前恢复费用状态。`RunAdmissions::run_one` 为 Native/ReAct 安装 `RunSpendPort`，绑定服务器持有的 ReaderRun、材料发布引用与取消令牌。每次实际发送及传输重试都重新检查账号/材料权限、有效额度期和余额；调用身份不能再次授权物理发送。共享离线预构建继续使用原运营路径。

```text
最终请求 → 选定费率/估价
  → BEGIN IMMEDIATE：权限、有效期、可用额、reserved → COMMIT
  → BEGIN IMMEDIATE：复查权限/有效期、sent → COMMIT
  → HTTP（无数据库锁、无用户锁）
  → BEGIN IMMEDIATE：最后用量、成本、扣减、发送回执 → COMMIT
  → 既有工具调度/历史保存
```

预占写入费率快照、估算方法和数量，保留原账号、额度期、run/task、目的与逐次身份；最终请求正文和图片不落入账本。两次提交之间退出留下 reserved，可证明尚未授权 HTTP；sent 提交后退出则不能证明未计费。第二次事务发现权限或有效期失效时，按未发送释放。

有完整计价用量时立即 settled，返回业务错误也照常入账；实际成本超过预占仍全额记入，余额可以为负。明确 NotSent 且无用量时 released，供应商成本与账号扣减均为零。缺失用量、缺少计价细分或发送不明时 pending，保留预占和未知原因。

`charge_reports` 保存收到的发送回执。相同完整回执直接返回原处理结果；pending 可被后续完整回执结算。终态的不同回执追加为 conflict，保留原成本与扣减，设置 `needs_reconciliation`，返回 `MODEL_CHARGE_RECONCILIATION_REQUIRED`。重复冲突不会追加或重复扣费。人工改账后迟到的原回执仍为幂等重试。

费用存储失败拒绝后续发送，已提交占用保留，重启恢复后再处理；读取原文、历史和个人额度不依赖费用发送准入。历史保存失败不会回滚或再次执行费用结算。

## 启动配置与恢复

费率路径固定为 `<service-root>/model-rates.json`，使用 ADM4 的 `{rates:[ModelRateSnapshot...]}` 格式。`provider` 精确匹配配置中的 Provider base URL，`model` 精确匹配托管模型。启动检查当前模型的适用费率；缺失、损坏、过期或无法估价时返回 `MODEL_RATE_UNAVAILABLE`，阅读服务继续运行。配置在进程启动时载入，修改后按既有停服流程重启；历史调用仍使用自己的快照。

恢复仅执行数据库事务：reserved → released（确认未发送、成本零）；sent → pending（未知且保持预占）；pending、settled、released 不变。恢复失败保持费用发送关闭，普通阅读继续可用。恢复方法只用于工作线程启动前，不在活动调用中执行；恢复不调用 Provider、不执行工具、不迁移旧调用的额度期。

schema 7 从 6 顺序升级：`model_call_charges` 新增 revision、needs_reconciliation、send_identity、send_outcome；新增 `charge_reports` 和 `charge_reconciliations`。升级失败回滚本次版本与结构变更，保留旧费用。原 1–5 顺序升级路径继续有效。

现有 `reader_maintenance::backup_service` 递归备份服务根并使用 SQLite backup API，包含新费用表及根目录中的费率文件；沿用现有备份与恢复入口。

## 人工核算接口

`POST /api/admin/charges/{call_id}/reconcile` 使用现有管理员 Cookie、Origin、CSRF 和操作回执。金额均为整数 micro_cny，时间为 UTC Unix 秒，拒绝未知字段。

```json
{
  "operation_id": "reconcile-call-001",
  "revision": 2,
  "provider_cost_micro_cny": 100000,
  "waive_account": false,
  "reason": "核对本次调用成本",
  "evidence": "供应商账单中的对应请求记录"
}
```

`revision` 必须等于费用当前版本。reserved/sent 返回 `CHARGE_IN_FLIGHT`（409）；版本变化返回 `CHARGE_REVISION_CONFLICT`（409）。同一操作号、操作人及规范化参数重试优先返回原回执；参数冲突为 `ADMIN_OPERATION_CONFLICT`（409）。reason/evidence 去掉两端空白后必须非空。

- `waive_account=false`：必须提供非负 provider_cost_micro_cny；确认供应商成本并按同额扣减账号。
- `waive_account=true`：账号扣减为零、释放占用。可以提供有依据的供应商成本；省略或 null 保留原供应商成本，原本未知的仍为 pending。
- 已确认记录可再次更正；保留每次 before/after、操作人、原因、依据、时间和账号扣减差额。前后成本都已知时保存供应商成本差额，否则该差额为 null。原始用量、费率、回执及原额度期保留。

费用投影、追加核算事实和 `admin_operations` 回执在同一事务提交。成功返回公共回执字段以及 `charge`、`account_delta_micro_cny`、`provider_delta_micro_cny`。`GET /api/admin/operations/{operation_id}` 可查询原结果。核算不更改收款或授予量。

## 查询接口

| 接口 | 返回与筛选 |
| --- | --- |
| `GET /api/admin/charges` | `{items,total,limit,offset}`；可按 user_id 筛选；pending=true 包含待结算、账号免扣但供应商成本未知、存在冲突待核对的记录 |
| `GET /api/admin/charges/{call_id}` | 当前完整费用记录，以及分别分页的 reports、reconciliations；核算项包含旧值、新值与依据 |
| `GET /api/account/allowance` | 当前登录账号的 `{current_allowance}`，无有效期为 null；余额分列沿用 ADM3 |
| `GET /api/account/usage` | 当前登录账号的分页调用记录，包含原期、run/task、目的、预占、状态和账号扣减；不返回运营费率、供应商回执或人工依据 |

分页默认 50、范围 1–100，offset 为非负整数；列表在一个读事务中取得明细与总数。个人接口不接收目标账号参数，也不创建阅读现场或加载私人历史。管理查询沿用服务器当前管理员授权。聚合运营报表由 ADM9 接入。

## 验证

ADM5 专项 **11 通过、0 失败**，覆盖真实 SQLite 独立连接并发预占、超预占据实扣减、完整/冲突回执幂等、未知与迟到用量、跨期结算、免扣及追加更正、revision 冲突、失败回滚、schema 6 升级、读者隔离、真实 HTTP 重试以及历史保存失败后的重启。

服务端受影响回归首轮 96 通过、1 失败、1 忽略；失败的 `formal_admission_exports_usage_diagnostics_and_terminal_states` 旧夹具没有配置费用准入。补齐测试费率/额度并保留原断言后，ADM5、该观测测试、真实 Provider 宿主测试及 schema 约束共 **14 项定向复测全部通过**；受影响集合的 97 个可执行测试均已通过。忽略项是既有 JL4 子进程专用入口。Runtime 受影响回归 **23 通过、0 失败**。完整命令见 [checkpoint_ADM](../checkpoint_ADM.md)。

新增文件与本次相关修改的空白检查、新实现合同的本地文档链接检查通过。没有执行真实 Provider 请求。

## 已知限制

- ADM6 已接通 Runtime 内外层类型化停止、未完成 Goal、保存与显式继续，见 [ADM6 实现](运营后台-ADM6实现.md)；后台页面与发布验收按后续切片完成。
- 预占字符估价不是严格 Token 上界，实际费用可能超过预占；供应商实际扣款仍须有依据的人工核对。
- 投产费率、售卖价格与真实 Provider 预算验收属于 ADM10；当前专项使用本地可控 HTTP，不产生真实模型费用。
