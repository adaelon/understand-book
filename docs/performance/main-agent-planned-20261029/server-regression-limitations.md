# 服务端回归的失败归因

全量首轮为 1192 通过、27 失败、112 忽略。13 项属于已改职责的旧测试预期，更新为无教师内容审核、历史身份和当前执行合同后通过。其余 14 项在冻结 HEAD 源码重新构建的测试程序上逐项重放；12 项仍失败，2 项超时相关测试新旧单独运行均通过。候选累计 1207 项通过，保留 12 项原有失败、112 项原忽略；不是全量绿色。

冻结源码构建：`tmp/main-agent-planned-20261029/baseline-attribution-build.log`；逐项原始结果：`baseline-attribution-results.json`。失败消息与本轮全量一致，未为这些旧问题修改生产代码。

- `host::agent_run_tests::resident_goals_persist_continue_select_and_cancel_without_resurrection`：旧源码同样失败，原日志 `baseline-attribution-2.log`。
- `tests::ambiguous_forget_reaches_main_agent_as_ephemeral_clarification_only`：旧源码同样失败，原日志 `baseline-attribution-3.log`。
- `tests::finalization_protocol_violation_persists_incomplete_fallback_without_effects`：旧源码同样失败，原日志 `baseline-attribution-4.log`。
- `tests::mu6_tests::cache_observation_tests::formal_admission_exports_usage_diagnostics_and_terminal_states`：旧源码同样失败，原日志 `baseline-attribution-5.log`。
- `tests::mu6_tests::e1_workspace_integration_tests::e1_d_existing_period_and_legacy_chat_upgrade_select_once_and_preserve_evidence`：旧源码同样失败，原日志 `baseline-attribution-6.log`。
- `tests::non_confirmation_cancels_pending_sensitive_memory`：旧源码同样失败，原日志 `baseline-attribution-8.log`。
- `tests::presentation_author_tests::ex12::ex12_archived_profile_keeps_catalog_authoring_and_compaction_budgets`：旧源码同样失败，原日志 `baseline-attribution-9.log`。
- `tests::presentation_author_tests::se2::se2_actual_provider_inputs_contain_only_requested_method_source`：旧源码同样失败，原日志 `baseline-attribution-10.log`。
- `tests::presentation_author_tests::se3::se3_provider_records_actual_inputs_and_candidate_binding`：旧源码同样失败，原日志 `baseline-attribution-11.log`。
- `tests::profile_snapshot_explicit_remember_commits_before_projection_and_survives_new_chat`：旧源码同样失败，原日志 `baseline-attribution-12.log`。
- `tests::provider_failure_persists_stable_incomplete_turn_before_restart`：旧源码同样失败，原日志 `baseline-attribution-13.log`。
- `tests::sensitive_memory_waits_for_exact_next_message_without_second_extraction`：旧源码同样失败，原日志 `baseline-attribution-14.log`。

这些失败包括旧记忆夹具计数、过时输出预算和诊断目的预期、遗留会话升级计数，以及辅助请求／模拟网络调用等；它们仍需各自原场景处理。本轮保留独立结果，不将其归结为移除教师审核。
