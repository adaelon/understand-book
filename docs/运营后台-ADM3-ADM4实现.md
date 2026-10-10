# ADM3–ADM4 实现合同

日期：2026-10-08。实现位于当前工作树，未提交、未部署。总方案见[运营后台与账号额度](切片方案-运营后台与账号额度.md)。

## ADM3 管理请求与事实

沿用管理员 Cookie、Origin、CSRF 和 `/api/admin/users/{user_id}/`。下面每个 POST 都必须带 `operation_id`；所有未列字段均拒绝。时间为 UTC Unix 秒，金额为整数。

| 路径尾段 | 业务字段 | 返回与约束 |
| --- | --- | --- |
| `allowance-periods` | `starts_at, expires_at` | 服务生成 `period_id`，初始 revision 0、额度 0；允许相邻期间，拒绝重叠 |
| `allowance-adjustments` | `period_id, revision, delta_micro_cny, reason` | 正负非零调整；原因必填，下调不能占用已结算、在途或待核算金额 |
| `allowance-validity` | `period_id, revision, starts_at, expires_at` | 显式修改全期有效范围；不能与其他未关闭期间重叠 |
| `receipts` | `period_id, revision, amount_fen, delta_micro_cny, paid_at, channel, external_ref?, note` | 收款分与授额分别为正数；同事务保存收款、授额、revision 和操作回执 |
| `receipt-corrections` | `receipt_id, period_id, revision, delta_fen, delta_micro_cny, reason` | 追加非零收款差额，调整后收款不得为负；额度差额可为零；必须关联原收款授额所在期 |

成功回执保留 ADM2 的公共字段，并增加 `allowance_period`（操作提交时的期间与余额快照）、`receipt_id`（无关联收款时为 null）。同操作人、操作号、目标、种类和规范化参数重试返回原结果，即使该 revision 已旧。原因、渠道、交易号、备注去掉两端空白后比较；无外部交易号可省略或为 null，空字符串拒绝。后续更正不改写原回执。

修改已存在期间的操作每次使 revision 加一。不同操作号持有同一旧 revision 时，只能一个提交；其余返回 `ALLOWANCE_REVISION_CONFLICT`（409）。操作号业务冲突为 `ADMIN_OPERATION_CONFLICT`（409），渠道交易号重复为 `RECEIPT_EXTERNAL_REF_CONFLICT`（409），期间重叠为 `ALLOWANCE_PERIOD_OVERLAP`（409），下调侵占已承诺额度为 `ALLOWANCE_ALREADY_COMMITTED`（409）。缺失或其他账号的期间、收款为 404。

账号详情增加 `current_allowance`，没有当前有效期时为 null。上述尾段中除 `allowance-validity` 外都支持 GET 分页，返回 `{items,total,limit,offset}`，默认 50、最多 100。期间项包含 revision 与实际余额；收款项同时显示 `amount_fen` 原额和 `effective_amount_fen` 更正后额；调整与更正列表保存操作人、操作号、原因和时间。

余额分列 `granted_micro_cny`、`debited_micro_cny`、`active_reserved_micro_cny`、`pending_micro_cny`、`available_micro_cny`。在途包括 reserved/sent；pending 独立占用；settled/released 按实际账号扣减。新一期初始额度为零，旧调用继续留在原期；到期判断为 `[starts_at, expires_at)`。

schema 6 新增 `receipt_corrections`，以复合外键关联原收款和同一账号的操作回执；1–5 顺序升级到 6。更正是账面追加事实，撤销到零保留原外部交易号，外部退款由运营者另行处理。赠送、收款更正和调用费用核算分别表达。

## ADM4 实际发送合同

`runtime::model_spend::ModelSpendPort` 的两个方法：

```text
before_send(SendIdentity, final_request) -> Result<(), SpendStop>
after_send(SendIdentity, SendOutcome) -> Result<(), SpendStop>
```

`SendIdentity` 包含独立 `call_id`、共享 `logical_call_id`、从 1 开始的 attempt、账号/任务归属、purpose、配置模型和 Provider 地址。Server 在 `RunAdmissions::run_one` 绑定 `ReaderRun {user_id, run_ref: turn_id}`；另提供显式 `ReaderTask` 与 `OperatorTask`，不按缺失账号推断运营任务。安装费用端口后缺失/空归属会在发送前拒绝。

Native 和 ReAct 的三种模型入口最终进入 `NativeAdapter::post_chat_completions`。它在每次 `send_chat_completions_once` 前调用 before，读取该次响应后调用 after；网络等待期间不持有费用端口锁。既有一次网络重试继续保留，每次独立 before/after。after 失败直接停止；before 失败不发请求。Request 观测事件仍然每逻辑调用一次，费用事实独立于观测开关。

同次发送的用量快照覆盖前帧；断流、取消、HTTP 错误中已经取得的用量仍交给 after。无响应的 I/O/协议中断为 `OutcomeUnknown`，DNS/连接建立失败为 `NotSent`；收到响应为 `Response`，但 Response 本身不证明费用为零。模型返回后的业务解析失败不改变已经报告的发送事实。费用端口接收的最终正文只供即时估算，账本保存元数据。

## 可达入口与归属

| 入口 | 实际发送链路 | 费用归属与目的 |
| --- | --- | --- |
| 短问答、持续任务、呈现源码生成 | `RunAdmissions::run_one` → `agent_run::execute_model` → Resident 外层 `chat_observed` | 同一个 ReaderRun；`outer` |
| 检索消歧、证据支持判断、原文综合 | `execute_book_query` / `query_run` / `synthesize` → `complete_structured` / `complete` | 同一 Run；`query` / `synthesize` |
| 来源修复与最终回答 | Resident repair / tools-disabled finalization → `chat_observed` | 同一 Run；`repair` / `outer` |
| 上下文压缩及其修订 | `maybe_auto_compact` → `compact_with_adapter` → `call_generator` → `complete_structured` | 同一 Run；`compaction` |
| Tutor 判定 | `tutor.step assess` → `tutor::evaluate` → `chat` | 同一 Run；`tutor_assessment` |
| 图片/呈现预览观察 | `append_preview_images` → 最终 messages 中的 image_url → 下一次外层采样 | 同一 Run；随外层请求逐图计量 |
| 画像/记忆后台复核 | 现有多人宿主只登记 review_jobs；执行器在单用户 `host.rs`，当前多人 capability 没有该模型入口 | 当前无多人后台发送；启用时须显式提供 ReaderTask |
| 视频原音频/视觉渐进处理 | 当前多人路由没有对应模型执行入口 | 后续接入时补 ReaderTask 或真实运营任务归属 |
| 共享书籍预构建 | 离线构建 Harness / 单用户 MCP，不经多人 RunAdmissions | 运营准备成本，不归给打开材料的读者 |

`ObservedAdapter` 从既有 RunEvents 作用域向底层传递 purpose；`CancellableAdapter` 和 `LimitedAdapter` 转发费用上下文，原并发、取消与材料授权入口保持。purpose 依赖运行作用域，不依赖 LangSmith 等外部观测是否开启。

## 费率、估算与计价

`server::model_rates::ModelRates::load/select` 从指定 JSON 路径读取 `{rates:[ModelRateSnapshot...]}`，校验版本、必要出处和区间。按 Provider、模型和服务器 UTC 时间精确选择 `[starts_at,expires_at)`；重叠/重复版本、缺失或区间外返回 `RateUnavailable`。每个高峰、低谷或节假日适用时段在配置中展开为绝对区间；返回值为独立快照，后续配置变化不修改历史。

快照字段：`version, provider, model, starts_at, expires_at`；`input_micro_cny_per_million, cached_input_micro_cny_per_million, output_micro_cny_per_million`；`usage_contract: deep_seek`、`image_meter: unsupported | deep_seek1024`；`source` 包含 `currency, original_prices, cny_conversion, checked_on, official_url`。单价为运营者按记录的换算依据配置的整数 micro_cny/百万 Token，源报价和换算依据原样保留。

首个已实现用量合同为 DeepSeek：输入总数减缓存命中得到未命中输入；输出已经包含推理子项。缺输入/输出/缓存细分、缓存创建费用、矛盾细分或成本溢出返回明确未知原因。计价以 u128 累加各项的 Token×费率分子，最后一次向上取整到 micro_cny；不逐项取整，不将推理再次相加。

`ModelRateSnapshot::estimate` 使用最终 messages、工具描述和结构化输出约束的字符估计；缓存按全未命中预留，输出使用最终请求的实际上限。DeepSeek 图片按每张 1024 Token 的公开上限预留，移除图片 URL/base64 后单独计量；不支持的媒体类型或缺输出上限返回配置原因。结果保存文字估量、图片量、输出上限、规则及预估成本，供 ADM5 同费率快照持久化并与真实用量比较。

官方依据（2026-10-08 核对）：[模型与价格](https://api-docs.deepseek.com/quick_start/pricing/)规定适用时段影响价格；[Vision Token Usage](https://api-docs.deepseek.com/guides/vision/#token-usage)说明图片逐张计量及每张 1024 Token 上限。具体销售价格、首批额度和投产费率配置归 ADM10。

## 已知限制与下一接点

- ADM4 提供计价、发送身份和费用端口。ADM5 已在 `RunAdmissions::run_one` 安装同库预占/结算实现，并在启动时装载费率与恢复费用状态，见 [ADM5 实现合同](运营后台-ADM5实现.md)。
- ADM6 已让适配器保留 SpendStop，并接通内外层停止、历史保存与显式继续，见 [ADM6 实现](运营后台-ADM6实现.md)。
- 文字预估不是严格 Token 上界，实际成本可超过预估；ADM5 已按完整真实用量结算，并支持未知费用及供应商实扣差异的人工核算。
- 真实 Provider 验收、后台页面、读者费用界面和发布均属于后续切片；本轮验证使用真实 SQLite 和本地可控 HTTP。
