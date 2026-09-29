# Resident Goal G0–G3 验收记录（2026-09-25）

## G0 冻结基线

来源是本机 `chat_1790313507677_60` 的第二回合，只抽取最小事实到 `crates/runtime/src/testdata/goal-g0-chapter-presentation.json`。自然请求固定为“可以把这一章的内容富文本演示给我看吗，我想看一下整体”。轨迹有 39 次工具调用：正文 19、搜索 5、来源呈现 13、结构 1、能力发现 1；发现了 `presentation.author`，制作、预览、交付为零。终态为 `TURN_LIMIT_EXCEEDED`、`incomplete=true`、纯 Markdown 回答。这是失败基线，不是新 Goal 的自然模型收益证明。

后续 G6 使用同一书、上述原话、同一 Provider/模型运行配置和十二次工具循环上限。短答须一次正常采样完成且不调用 `goal.update`；复杂任务的目标更新计入原十二次上限，不增加监督模型或自动续跑。逐次记录模型采样数和 token 用量，报告相对基线的差额。私有历史未保存可移植的模型标识与用量，因此真实对照运行前要从当前宿主配置读取并固定模型标识，不能从此夹具推断旧模型成本。

合同对照：`g0_chapter_presentation_failure_baseline_stays_portable` 重放无页面却有文字的超限路径；`provider_equivalence_error_and_stop_fixtures_share_runtime_semantics` 固定 native/react 的超限语义；`presentation_preview_at_turn_limit_gets_one_deliver_only_sampling` 和 `presentation_versions_reopen_with_original_history_and_explicit_edit_base` 固定交付引用路径；`resident_cancel_closes_unexecuted_batch_receipts_and_can_continue_session` 固定取消 Run 的终态；Server Goal 测试固定短答无工具调用。真实浏览器交付测试在本机因缺少 Chromium/Edge 被标记为 ignored，G6 需在目标宿主补验。

## G1–G3 实施结果

G1：`ResidentGoal` 与回合 `goal_ref` 随 `AgentChatSession` 一起原子保存。旧历史缺字段时可读取；模型进行中的 Goal 更新会立即保存，失败提交保持旧文件与可见状态。取消、替换、已完成产物的新修改、续接及多对象歧义由 Server 预提交处理。Run 取消或超限不把 Goal 标为 completed。

G2：当前用户请求先绑定 Goal；主 Agent 可以直接短答，也可调用本轮直接可见的 `goal.update` 细化用户要求或调整工作焦点。要求修订必须引用当前用户原文；工具不能写入已交付或已完成事实。native/react 共用注册、schema 与执行路径。明确选择由请求的 `goal_id` 表达，`goal_action` 支持 cancel/replace；多个开放 Goal 的裸“继续”返回 `GOAL_AMBIGUOUS`。

G3：每次采样按当前 Goal revision 和本轮事实投影 `agent.resident_goal`，展示正文观察数、未交付候选、实际交付引用和页面缺口。压缩后的旧摘要不覆盖持久 Goal；候选及工具激活只在本轮内有效，重启只恢复持久结果引用。重复相同工作更新不增加 revision，也不计入既有进展守卫。

## 验证与后续边界

- `cargo test -p runtime --lib`：368 通过，3 个既有 ignored。
- `cargo test -p server --lib`：310 通过，10 个既有 ignored；涵盖 Goal 续接/取消、运行中更新、已完成任务修改/替换、提交失败重启恢复与页面版本保存。
- G4 尚未实现页面完成检查、有限修正及 Goal completed 的统一终态提交。因此 G0–G3 的 Goal 会保留 open；不能把有文字回答或单独保存的页面解释成任务已完成。
- G5 Reader 任务状态与操作、G6 自然模型和各宿主验收仍待实施。
