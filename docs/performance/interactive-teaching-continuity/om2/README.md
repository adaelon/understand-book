# OM2：从原证据组织接续上下文

日期：2026-10-20。状态：完成。代码基线：`33c19c7fa8aec9a4f1031122bc177743b234b9fd` 加本片工作树增量；其他工作线的既有修改保留。本片没有提交、推送或部署。

合同见[切片方案 §5.1、§6 OM2](../../../切片方案-交互教学与学习接续.md)，决策见 [ADR-0172](../../../adr/0172-interactive-teaching-state-and-learning-continuity.md)。

## 实现与入口

- [`teaching.rs::freeze_prepare/learner_context`](../../../../crates/server/src/teaching.rs) 在 `learning_opportunity.continuity` 返回当前工作区、目标版本、来源版本、设计与焦点；接纳设计、证据和评价后使用同一投影。当前输入中的实际事实参与 prepare，不依赖聊天摘要。
- [`teaching/continuity.rs::context`](../../../../crates/server/src/teaching/continuity.rs) 组织最近相关作品、原话、输入时帮助、确切作答评价、当前未决问题和有效教学含义。`action_ref`、`fact_refs`、`evidence_refs`、`assessment_ref` 与教具原 `saved_state_ref` 保留各自身份。普通表达保持 `unassessed`，没有对应评价的提交或修订为 `pending`。
- [`LearningStore::teaching_continuity_facts`](../../../../crates/memory/src/teaching.rs) 先按工作区、阶段、来源版本、设计谱系和焦点引用选择，再取最近 13 条候选；当前回合和显式依据也可进入。现有 `understanding_basis`、`teaching_expression_interpretations` 过滤被替代解释；后者同时支持按 `action_ref` 查询。
- [`workspace_record_context.rs::selected_records`](../../../../crates/server/src/workspace_record_context.rs) 为接续投影和 `workspace.records.read` 共用原提交前缀、原聊天与因果／修订链读取。选入作品保留原工作区、提交位置、目标、出版身份和帮助，标记为历史；读取不会复制、改写原证据。
- `teaching.rs::readable_fact` 在作答 trace 中附该 `action_ref` 的原始评价；作品的评价引用可直接回读。教师工作记忆仍由原 `agent_working_state` 提供。

作品内容使用 4,000 字符预算，未决问题与教学含义共用 2,000 字符预算；每组最多六项，正文片段最多 600 字符。这样较多解释记录不会耗尽作品预览。历史最多展开三份选入聊天；帮助正文每件作品最多展开两条。片段和省略分别由 `truncated`、`help_truncated` 表达，完整引用和 `read_more` 指向原 `tutor.step`／`workspace.records.read`。回读定位信息不计入内容片段预算。

## 场景与证据

| 场景 | 验证内容 |
| --- | --- |
| 受助表达后关闭重开 | 丢弃 AppState，从真实聊天文件和 SQLite 恢复；下一轮仍保留原话、当时帮助及用户纠正，后给的帮助不会倒灌 |
| 同一工作区换聊天 | 重启服务、重新连接阅读现场、新建聊天后，正式 prepare 中仍有该工作区先前作品 |
| 旧评价与新修订 | 原 `correct` 评价只指向旧作答，新修订保持 `pending`；trace 可读取原评价 |
| 相关作品较旧 | 插入 24 条不属于当前关系的后来表达，较早的相关作答仍被选中 |
| 用户纠正 | 仅当前纠正进入教学含义和作品证据引用，原解释与原话仍可回读且保持不变 |
| 选入另一个聊天的前缀 | 原出版身份、选定回合和提交位置保持；后续未选入作答不出现，纠正链可以更新 |
| 目标改向 | 下一轮采用新目标版本和新阶段，不把旧阶段作品重绑定为新目标事实 |
| 同工作区多资料、来源更新 | 其他书籍的回合和旧来源版本不会被回填成当前资料的作品 |
| 教具现场前进 | 当前页面已有新状态／新版本，接续作品仍引用表达时的旧保存回执和帮助 |

实际 prepare 投影：[重开现场](reopened-context.json)、[作答与评价](attempts-context.json)、[选入历史](linked-context.json)。测试中的原文、帮助与评价回读断言同时检查对应持久记录；这些文件是测试输出，非手写示例。

## 运行与结果

从仓库根目录执行。设置 `OM2_OUTPUT` 为本目录绝对路径时，相关测试保存上述三份实际投影。

```text
cargo test -p server --lib -- teaching_design_tests tutor_understanding_view_tests w6_learning_records_tests w4_workspace_goal_tests tutor_loop_tests
cargo test -p server --lib -- om2_continuity tutor_t17a_ordinary_messages tutor_ordinary_reply w3_ordinary_understanding w4_goal_edit_stops
cargo test -p memory --lib -- teaching:: learning_evidence:: learning_records::
```

| 检查 | 检测目标 | 结果 |
| --- | --- | --- |
| Server 相邻回归 | 教学设计、原理解视图、历史选入、工作区目标和原工具循环未回退 | 75 passed、0 failed、6 ignored；[日志](server-regression.log) |
| Server 最后定向验证 | 来源版本修正、同工作区多资料、重开、选入前缀、原使用事实及目标改向 | 9 passed、0 failed；[日志](server-final.log) |
| Memory 证据／历史／教学回归 | 查询范围、原记录不可变、纠正替代、评价引用与 SQLite 重开 | 22 passed、0 failed；[日志](memory-regression.log) |

最后定向验证发生在多资料和来源版本过滤调整后，覆盖修改路径；其中八项与前一回归重叠，不重复累加。六项 ignored 为原集合中的浏览器或手动宿主用例，本片未启动。没有 Web 类型或前端合同变更。

交付检查：本片涉及的已跟踪文件通过 `git diff --check`；本记录与 checkpoint 的 31 个本地链接均可解析，checkpoint 为 49 行。

## 过程中发现与修正

1. 选入前缀测试发现 `usage_facts` 原先只按书籍回填聊天，其他工作区同书的未选入新回合也会成为当前事实。现按工作区、每回合的原书籍、目标版本及已有来源绑定筛选，并保留原目标及出版身份；目标改向、选入历史、多资料和来源版本测试覆盖这个修复。
2. 初次重开夹具只设置旧历史路径，没有初始化正式聊天文件和选定聊天。测试改为从正式新建聊天入口开始，随后销毁 AppState 再加载持久状态。
3. 网络服务重启后需要重新连接阅读现场。换聊天测试补上现有 `attach` 操作后再继续，保留原 `WORKSPACE_STALE` 行为。
4. 目标版本测试改用现有真实工作区改向场景；无工作区的旧式本地夹具不能调用 `sync_workspace_goal`。

首次结果保留在 [初轮](server-om2.log)、[重开与选入场景](server-om2-scenarios.log)、[范围修正后](server-om2-scoped.log)。最终结果以回归日志为准。

## 已知限制

本片验证服务端 prepare、工具回读、持久化、范围选择和已有教具回执链。网络测试使用既有确定性模型替身，没有外部模型调用；下一轮实际教学是否采用合适支撑由 OM3／OM6 验收。

接续是有界事实视图，未展开记录通过原工具回读。原历史仍属于原目标与帮助条件，投影不生成新的掌握结论。
