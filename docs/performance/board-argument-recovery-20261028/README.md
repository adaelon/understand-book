# 画板参数纠错与交付恢复

2026-10-28，本地源码修复，基于 `d791eea5` 及现有工作区改动。入口为用户截图中的 `$.responds_to is not allowed 本轮画板尚未保存，任务未完成。`。本次没有取得原失败轮次的完整参数；使用公开接口和实际运行循环固定同类输入，未改写历史失败记录。

## 原因与处理

`responds_to` 属于 `tutor.step select` 的 `move`，包括原生画板的精简选择接口。根层提交被执行前校验拒绝；原提示没有指出正确位置。`ToolRegistration::append_argument_recovery` 根据当前动态接口指出所有误放字段的正确路径，附当前 move 的允许和必填字段、当前操作的根层字段。提示覆盖教学、画板及演示工具，保留原错误。Tutor 工具说明也明确教学字段与交付控制字段的层级。完整纠正提示进入下一次模型采样；最终画板失败信息只保留原始原因，不展开内部接口定义。

相邻流程检查发现并修复三处问题：

- 原生画板和演示页共用待观察集合，设置回合上限的调用在预算边界却只允许演示工具收尾。按候选类型分配一次受限交付机会；教学开启的画板仍需选择、正常审核和暂存，仅保留一次审核及其既有的一次格式纠正额度。审核需要修改时停止，不再延长创作轮次。
- 原生画板的成功预览只按候选版本记进展。增加规范化预览参数，让不同视口的实际观察成为新进展；相同输入的重复检查仍受停滞限制。
- 预览图片因上下文预算省略后，重预览和补齐观察不能解除旧失败记录。复用当前回合的进展集合记录首次省略和实际完整观察，并使用既有有限恢复处理演示图片未观察的错误。

## 验证

58 项不同的相关测试最终通过，未调用外部模型。运行循环测试使用脚本模型与宿主测试替身，覆盖实际校验、调度、审核、图片预算与交付暂存路径。

- 参数纠错首组 7 项通过，见 [argument-recovery.log](argument-recovery.log)。
- 运行循环及相邻路径 25 项通过；新增审核拒绝场景的首次期望未考虑已有纯文字降级，故同轮保留 1 项失败，见 [related-regressions-final.log](related-regressions-final.log)。最终测试要求降级回复保持 `incomplete`、不得保存候选、不得重新开放工具，符合既有流程。
- 修正上述测试期望后，该运行循环场景及工具接口/Provider 投影共 33 项通过，见 [selection-and-contracts.log](selection-and-contracts.log)。与前述 25 项合计 58 项，包含误放回应字段、同时误放参与/邀请字段、受限收尾的接受/拒绝/错误工具等 16 种教学画板情形。
- 相邻预览回归覆盖六种非教学状态、四个不同视口、重复相同视口，以及原生画板和演示页各自的“首次 256 张图片触发预算省略 → 原交付被拒绝 → 同参数重预览 → 实际观察 → 同参数交付成功”。交付次数均为一次。

复跑命令：

```text
cargo test -p runtime --lib -- argument_recovery native_board board_recovery_paths contract_tests preview_at_turn_limit ex11_distinct_preview_interactions agent_tool_policy_general_no_progress
```

初次完整构建因 C 盘仅剩约 20 MB，链接器无法写入临时文件；失败日志保留在 [related-regressions.log](related-regressions.log)。将本次进程的 `TEMP`/`TMP` 指向工作区 `.tmp/board-argument-recovery-temp` 后构建与回归完成。未清理其他文件。

入口和回归位置：

- `crates/runtime/src/tool_registry.rs`：当前操作的参数纠错提示及动态字段测试。
- `crates/runtime/src/teaching_design_loop_tests.rs`：原生画板误放回应/邀请字段、纠正后审核并暂存一次、预算边界的教学接受/拒绝/错误工具。
- `crates/runtime/src/orchestrator.rs`：预览交付收尾、有限恢复和实际观察进展。
- `crates/runtime/src/board_recovery_paths_tests.rs`：六种非教学状态的受限收尾、不同/相同视口、实际图片预算省略后的同参数重试。

正式写盘失败已有 `retry_save` 路径，保留同一份完成结果并重试保存，不重跑模型。本次沿用；已有覆盖见 `crates/server/src/tests/board_follow_up_run_tests.rs:board_storage_failure_stays_unsaved_until_retry_publishes_the_same_answer`。

## 已知限制

本次验证针对确定性的工具调用、校验、恢复和交付行为，不证明模型永远能一次生成正确参数。没有调用生产模型、部署线上服务或恢复截图中的原失败草稿。正式写盘重试路径未改动，本轮只核对其既有实现与回归覆盖，没有重新执行该 Server 集成测试。

## 2026-10-28 上线补记

20:28 已随 `editable-mind-map-20261028` 合并发布，20:33 公网复核通过，原有记录保留。先前未上线描述指当时的本地验证。见[完整发布记录](../combined-release-20261028/README.md)。
