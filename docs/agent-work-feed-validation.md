# Agent 工作过程与停止交互验证 — 2026-10-22

## 实现

常驻 Agent 通过 `agent.progress` 与实际工作同批发送面向用户的简短说明。`runtime::agent_progress` 校验公开文本，`orchestrator` 将成功说明写为 `commentary` 活动；步骤序号、SSE、快照和持久历史复用原链路。普通模型文本和未审核教学草稿维持原交付规则，进展说明不增加证据、交付成果或防空转进展。

`AgentActivities` 运行时按顺序显示说明与顶层工具，结束后可展开回看；诊断视图保留模型及嵌套步骤。`useAgentRun` 订阅说明事件并提供停止请求状态；`RightRail` 在过程、输入区和演示工具栏提供停止入口，防止重复点击，保留输入草稿，连续回答片段也触发滚动跟随，用户上滚时不抢位置。

演示准备允许与进展说明同批，仍不允许与写入等实际工作混批，保留准备后进入下一次采样的约束。

## 验证结果

| 验证 | 结果与覆盖 |
| --- | --- |
| `vitest run src/agent-run-state.test.ts src/useAgentRun.test.ts` | 16 项通过；说明事件顺序、去重、快照及取消保留；取消重复点击、请求失败重试、旧取消响应不覆盖终态、网络取消响应不清空已观察内容。 |
| `AgentActivities.test.ts`、`RightRail.test.ts` | 72 项通过；运行时过程、结束折叠、诊断信息、停止禁用与恢复、连续草稿跟随、手机作答隐藏输入区及演示讨论折叠时的停止入口。 |
| Runtime `agent_progress`、`tool_exposure` 及三项精确复验 | 52 项通过；公开说明校验、普通答案隔离、取消后不发布、无进展规则、native/ReAct 请求暴露、工具映射和请求清单、演示准备同批与实际工作混批边界。日志：[runtime](../tmp/agent-progress-runtime-tests.log)。 |
| Server `resident_public_progress_survives_snapshot_and_cancelled_history` | 通过；真实受控 HTTP 模型响应中同批播报与读取原文，SSE/快照恢复进展，取消后不执行返回的高亮工具，磁盘历史保留说明。 |
| Server `resident_cancel_keeps_completed_note_and_blocks_returned_tools_and_later_sampling` | 通过；已保存笔记保留，取消后的工具与后续模型采样被阻止。 |
| `pnpm --filter @understand-book/web build` | 类型检查及生产构建通过。 |
| 工具参考导出 | Server 契约导出及 Tutor schema 导出各通过一项；使用仓库既有生成函数更新工具参考与 JSON。 |
| 实际浏览器操作 | 桌面与 390px 手机预览检查运行、停止中、停止后及完成状态；过程可读，停止后草稿保留，已完成过程可展开。 |

预览使用真实 `RightRail` 组件和示例内容：[桌面截图](screenshots/agent-work-desktop.png)、[手机截图](screenshots/agent-work-mobile.png)。开发入口为 `packages/web/agent-work-visual.html`，支持运行、完成和模拟停止。

## 已知问题与验证范围

- 停止沿用协作式取消。同步网络请求正在阻塞读取时，仍需等待该请求返回或超时；界面显示正在停止。此次支持停止本轮后继续提问。
- 当前工作树有两条较早的契约断言不一致，未在本轮修改：`tool_registry::contract_tests::advertised_examples_survive_provider_projection_and_real_validation`（`tool_contract_tests.rs:178`，实际 69／预期 67，本轮未新增示例）；`agent_prompt::tests::cr10_evidence_policy_forbids_derived_lids_and_failed_source_retries`（`agent_prompt.rs:328`，实际 v7／预期 v6，本轮未修改该版本）。不将这两项计为通过。
- 模型链路使用受控响应验证，尚未进行真实提供商的完整长任务行为验收。改动在本地工作树，尚未部署。
