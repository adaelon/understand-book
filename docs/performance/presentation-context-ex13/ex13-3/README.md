# EX13.3 Goal 工作计划

2026-10-02。**实现及确定性验证完成**。Runtime 447 项通过、3 忽略；Server 相关用例合计 10 项通过、1 忽略；Web 类型检查与任务面板 27 项通过。新增真实模型调用 0。合同见 [EX13 第 4 节](../../../切片方案-EX13-Goal工作计划与演示修订上下文.md#4-goal-工作计划的最小合同)与 [ADR-0155 §2](../../../adr/0155-goal-work-plan-and-version-centered-presentation-context.md#2-工作计划所有权)。

## 更新与所有权

`GoalWorkItem` 只含 `id / description / status`，状态为 `pending / in_progress / completed`。按列表顺序表达工作安排，使用同一 Goal 内的非空唯一 id。

`goal.update(operation=working)` 提供 items 时先验证整份列表，再替换；省略保留，`[]` 清空。空白 id、空白描述、重复 id、非法状态或缺少必要字段被拒绝，失败不写入部分工作状态。相同更新不增加 revision；重排、删项、改描述或重新打开 completed 项均可表达。旧 Goal 缺 items 默认空列表，缺整个 working 也仍可读取。

`focus / open_questions / next_move` 保留各自含义，采样投影现在同时包含未决问题与工作项。`requirements`、任务解释、用户依据及交付检查继续各自持有原职责。工作项全部 completed 后，缺少实际页面的任务仍保持 open / incomplete；Server 仍要求当前 Goal 的交付版本挂入最终回答。

## 保存、恢复与制作指导

```text
goal.update → ResidentGoal::apply_update → persist_goal → GoalUpdated
  → SessionLog 重放 / 同聊天新运行 → RunContext.goal
  → 每次 agent.resident_goal 投影 → Native / ReAct
最终回合提交 → TurnFinished 中的同一 Goal 和 revision
```

现有 `changed_goals` 比较完整 Goal，无需额外持久层。测试在真实临时磁盘分别重开 GoalUpdated 和 TurnFinished，并检查旧日志缺字段、API 返回工作状态及同聊天“继续”的首个请求。运行未结束时工作项已保存；后续新运行保留原要求与计划，不恢复旧候选资格。

中途压缩用例安装实际 CompactionCheckpoint 后，采样仍含三个工作项及完整要求；修改运行内 framework 也不改写 Goal。另一用例将过时 active_goal 摘要与新 Goal 一起装入新运行，确认最新工作项仍被投影且候选为空。

制作指导更新为 **ex13.v1**：用户要求留在 interpretation / requirements，多步进展留在 working.items，解释设计留在 framework。简单任务可无计划；小修订按需选择阶段。review 同时核对用户要求、材料与当前成果，保留真实剩余缺口。

## 请求与验证

[Native 请求](requests/native.json)与 [ReAct 请求](requests/react.json)记录同一受控输入的最终序列化：先更新 completed 工作项，再 refine 要求，下一次采样保留工作项及页面交付缺口，终答保持 incomplete。工具 schema 暴露完整更新语义。两份记录均 `usage=null`，未发送到外部提供方。

| 验证 | 结果 | 检出的具体失败范围 |
| --- | --- | --- |
| `cargo test -p runtime --lib` | 447 通过，0 失败，3 忽略 | 原子更新、同值 revision、默认值、来源/预算/交付缺口、压缩及制作指导回归 |
| `cargo test -p server --lib goal` | 首次 9 通过，1 失败，1 忽略；失败项隔离重跑通过 | pending 时落盘、GoalUpdated / TurnFinished 重放、新运行接续、API 工作项与真实完成合同 |
| `pnpm --filter @understand-book/web typecheck` | 通过 | Rust 新字段对应的前端类型与调用位置 |
| `pnpm --filter @understand-book/web test -- src/components/RightRail.test.ts` | 27 通过 | completed 工作项不隐藏仍 open 的任务，原继续/取消入口保持有效 |

Runtime 新增四项 `goal::tests::ex13_*`；加强 `g2_goal_update_is_direct_in_native_and_react_and_g3_projection_keeps_gap`、`g3_new_run_projects_durable_goal_over_old_summary_without_old_candidate` 与 `ex12_current_framework_survives_mid_turn_compaction`。Server 新增 `ex13_session_log_old_goal_records_default_to_an_empty_work_plan`，并加强日志提交、pending 保存及交付用例。日志在 `tmp/ex13-3-runtime.log`、`tmp/ex13-3-server.log`、`tmp/ex13-3-server-recap.log`、`tmp/ex13-3-web-types.log`、`tmp/ex13-3-web.log`。

请求重放：设置 `EX13_GOAL_RECORDING_DIR` 为输出目录后，运行 `cargo test -p runtime --lib g2_goal_update_is_direct_in_native_and_react_and_g3_projection_keeps_gap`；完成后清除该环境变量。原 EX13.0–EX13.2 录制保持冻结。

## 已知限制

首次 Server 测试中的会话回顾用例在进入 Goal 断言前，因公共临时目录的旧 `learning.db` 失败：`user_version=1`、`application_id=0`，当前 LearningStore 要求 `0x55424c4e`。将该次进程的 TEMP/TMP 指向新建独立目录，仅重跑失败用例后通过，未修改旧数据库或产品存储规则。之后运行此类测试宜使用独立临时目录。

工作项表达 Agent 的判断，不证明语义完整性；本片没有自然模型行为、缓存命中或费用结论。未提交、未部署，EX13.4–EX13.6 继续待实施；EX12.4 原生验收仍受提供方 402 阻塞，EX12.5 未开始。
