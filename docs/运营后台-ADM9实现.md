# ADM9 运营汇总与回访

日期：2026-10-08。已实现并通过本地候选验收；工作树未提交、未部署。依据 [ADR-0156](adr/0156-reader-admin-and-account-allowance.md) 与[切片方案第 7–9 节](切片方案-运营后台与账号额度.md)。

## 使用事实与交付判定

schema 8 追加 `admin_usage_events(user_id, kind, event_ref, occurred_at)`；原 1–7 顺序升级保留。表仅保存账号、事件种类、稳定引用及 UTC Unix 秒，账号/日期摘要由查询重建，独立于私人聊天删除；升级不回填历史活动。

- `read`：成功创建材料工作区，或成功 `attach/takeover/fork/book/open`。书库列表、登录、后台查询、工作区状态轮询和失败打开均不记录阅读。
- `question`：有效输入完成私人历史准备后，与 `run_admissions` 的 preparing → queued 在同一 SQLite 事务记录；引用为原 turn_id，时间为原接单时间。请求重放与准备恢复不会添加重复提问。
- `completed`：私人终态、教学交付/关联和 review job 保存成功后，检查已保存回合。状态必须 completed、无错误/交付修复失败，并满足既有 `goal_turn_delivered`（无 incomplete/warning、已交付回答及 Goal 要求）；没有 Goal 的回合采用相同基础回答条件。引用为原 turn_id，按首次成功记录的保存时间归日。失败、取消、费用停止、交付失败及未保存均不计完成；retry-save/启动时既有 repair_saved 可补记，重复修复只记一次。

最近阅读/提问分别取 read/question 最大时间。活跃日为发生任一类使用的香港自然日；同日多次操作只计一天。账号回访天数为活跃日数减一；所选期间回访账号数为期间内使用且在更早香港自然日已有使用的去重账号数，首次使用可以发生在期间外。completed 不构成一次读者回访。

## 查询与金额口径

所有入口沿用原 Cookie 与服务端当前管理员授权，不创建目标账号的阅读现场，不读取聊天正文。

| 接口 | 合同 |
| --- | --- |
| `GET /api/admin/usage` | `from/to=YYYY-MM-DD`（含起止日，默认香港今天），可选确切 `user_id`，`limit/offset`；返回 today、summary、分页 tasks、as_of、today_date 和时区 |
| `GET /api/admin/users/{id}/usage` | 同一日期/分页合同，账号由路径固定；缺失账号 404 |
| `GET /api/admin/users`、`/{id}` | 增加 activity：first_used_at、last_read_at、last_question_at、active_days、return_days、completed_runs；无历史时间为 null |
| `GET /api/admin/charges` | 保留既有参数，追加 from/to 与互斥 run_ref/task_ref，供任务聚合钻取；明细与总数在同一读取事务取得 |

区间转换为 `[香港开始日零点, 香港结束日次日零点)` 的 UTC 秒。费用按 **created_at** 纳入所选期间，展示查询时的核算结果；迟到结算和人工核算仍更新原调用所在期间。按 `(user_id, run/task 种类, 引用)` 分组，按已确认成本、扣减降序后稳定分页。金额合计在服务端以整数计算，前端只格式化分/micro_cny，不从当前页加总。

| 汇总字段 | 口径 |
| --- | --- |
| confirmed_cost_micro_cny | 当前已确认的供应商成本之和；未知记录另外计数 |
| account_debit_micro_cny | 当前已确认账号扣减之和，免扣为零 |
| active_reserved_micro_cny / active_requests | reserved、sent 的当前预留和条数 |
| pending_micro_cny | pending 状态仍占用的预留金额 |
| pending_requests | pending、免扣但供应商成本未知、needs_reconciliation 的记录并集；单条只计一次 |
| unknown_cost_requests | provider_cost_status=pending 的调用数，含在途和免扣未知 |
| request_count | 逐次调用记录数，含预占、未发送释放及每次传输重试 |
| receipt_original_fen / receipt_correction_fen / receipt_net_fen | 按原付款 paid_at 归组的原额、全部后续更正及净额；分别于费用与额度展示 |

一次汇总请求在同一读取事务内取得今日、所选期间和任务分页。收款更正属于原收款日，报表展示当前核对结果；它不是按更正发生时间编制的资金流水。任务明细继续提供原费用详情与账号核算入口。

## 后台页面

`/admin/usage` 提供今日/所选期间卡片、香港日期和确切账号筛选、刷新、任务成本分页及逐次调用分页。成本未知有独立数量，免扣后即使占用为零仍可识别待核算。账号列表显示最近阅读和提问；详情显示活跃/回访天数、完成数和个人期间汇总。

页面复用现有 AdminSession、查询缓存与身份 epoch；退出、换账号及角色变化清空旧查询。任务钻取保持账号、日期和 run/task 归属。金额使用既有整数格式化，保留六位精度。

## 验证与复跑

| 验证 | 结果与覆盖 |
| --- | --- |
| Server 专项与受影响回归 | **94 通过、0 失败、3 忽略**；覆盖 ADM1/2/3/5/6/7/9 与 MU5/MU6，包含升级、身份/材料、费用、接单/取消、保存及恢复。忽略项为 ADM7/ADM8 候选宿主与既有 JL4 子进程入口 |
| ADM9 最终专项 | **4 通过、0 失败、1 忽略**；补充迟到核算和同名 run/task 归属后复测，忽略项为随后显式启动的 ADM9 候选宿主 |
| Admin 单元测试 | **10 通过**；身份、操作回执、精确金额、香港日期与任务钻取参数 |
| Admin 类型检查 / 生产构建 | 通过；手机任务编号列宽修订后再构建通过 |
| 真实 HTTPS 浏览器 | **6 组通过，脚本错误 0**；深链接刷新、分列金额与 API 核对、21 个任务/23 条单任务调用两层分页、日期/账号筛选、香港跨日、账号摘要、390px 手机可读性和换账号清理 |


专项覆盖 schema 7→8 安装失败回滚/重开、香港午夜跨日、重放去重、成功打开材料接点、普通账号拒绝、真实费用端口和 SQLite 汇总/分页/未知免扣、迟到核算、收款更正、真实本地 HTTP 保存失败与 retry-save、删除聊天保留元数据。

```powershell
$env:CARGO_INCREMENTAL='0'; cargo test -p server --lib -j 1 -- adm1 adm2 adm3 adm5 adm6 adm7 adm9 tests::mu5_tests tests::mu6_tests --test-threads=1
pnpm -C apps/admin test
pnpm -C apps/admin build
```

浏览器使用显式 `adm9_candidate_host -- --ignored --nocapture`；设置绝对 `ADM9_CANDIDATE_INFO` 指向候选目录 info.json。真实 Rust/SQLite 夹具在 `127.0.0.1:18788` 启动，以 B 管理员、A 普通账号及 `fixture-only-password` 登录。夹具通过真实费用端口准备 43 条调用、21 个任务、香港跨日使用与收款；不调用外部 Provider。

HTTPS/Nginx、证书及 Admin 装配沿 [ADM7 候选合同](运营后台-ADM7实现.md#验证与复跑)；运行 `node scripts/linux/smoke-admin-usage.mjs`。`ADM9_URL/ADM9_EVIDENCE` 可指定地址/证据目录，默认 `https://localhost:18443` 与 `tmp/adm9-candidate`。创建 info 同名 `.stop` 文件结束临时宿主。

日志：`tmp/adm9-rust-regression.log`、`tmp/adm9-rust-final.log`、`tmp/adm9-admin-tests.log`、`tmp/adm9-admin-build-final.log`。浏览器结果与截图：`tmp/adm9-candidate/result.json`、`usage-desktop.png`、`usage-mobile.png`；临时宿主和 Nginx 已停止。

## 已知限制

- 活跃记录从启用时开始，未观察到的过去回访不补造；一项完成数代表一次满足交付条件且保存成功的 Run，不代表学习效果。
- 若私人终态已保存而进程在写运营完成事实前退出，既有未结清运行恢复会补记；归日采用补记时刻。跨日边界处不承诺恢复实际保存的原秒数。
- 正式 Linux 同版发布、旧库迁入、运营费率与真实 Provider 核对属于 ADM10。
- Admin 生产构建保留既有主脚本体积提示（约 542 kB、gzip 170 kB）；Rust 保留既有 ts-rs 属性解析警告。
