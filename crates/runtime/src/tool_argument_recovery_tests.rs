// Preserve the real music run's sequence: two host intent rejections, then a
// complete-call retry that accidentally drops the active Tutor participation.
// A real default keeps session_default in the corrected sampled schema; the
// neutral proposal still disagrees with that default and reaches host validation.
struct ArgumentRecoveryTeachingPort<'a> {
    inner: TeachingPort<'a>,
}

impl ResidentStatePort for ArgumentRecoveryTeachingPort<'_> {
    fn tutor_step_preflight(&self, request: &serde_json::Value, _: &[SourceBinding], _: &[read_tools::EvidenceRange]) -> Result<serde_json::Value, crate::run_context::TeachingSelectionPreflightError> {
        if request["move"]["interaction_intent"] == "direct_explanation" && request["move"]["participation"]["choice"] == "invite" {
            return Err(crate::run_context::TeachingSelectionPreflightError {
                error: memory::teaching::invalid("用户选择直接讲解时不要夹带新的作答任务"), missing_source_read: None,
            });
        }
        if request["move"]["intent_origin"] == "session_default" {
            return Err(crate::run_context::TeachingSelectionPreflightError {
                error: memory::teaching::invalid("教学意图需匹配当前原话或会话默认；直接讲解应采用解释动作"),
                missing_source_read: None,
            });
        }
        Ok(request.clone())
    }
    fn tutor_step(&mut self, request: serde_json::Value, evidence: &[SourceBinding], ranges: &[read_tools::EvidenceRange]) -> Result<serde_json::Value, ToolError> {
        self.inner.tutor_step(request, evidence, ranges)
    }
    fn tutor_record_review(&mut self, record: serde_json::Value) -> Result<serde_json::Value, ToolError> {
        self.inner.tutor_record_review(record)
    }
    fn submit_private<R>(&mut self, operation: impl FnOnce(&mut MemoryStore) -> R) -> Result<R, ToolError> { self.inner.submit_private(operation) }
    fn read_live_reader<R>(&mut self, operation: impl FnOnce(&Reader) -> R) -> Result<R, ToolError> { self.inner.read_live_reader(operation) }
    fn apply_reader<R>(&mut self, operation: impl FnOnce(&mut MemoryStore, &mut Reader) -> R) -> Result<R, ToolError> { self.inner.apply_reader(operation) }
    fn reader_input(&mut self, book: &Book, question: &str) -> crate::run_context::ReaderInputSnapshot { self.inner.reader_input(book, question) }
}

fn assert_tool_argument_recovery(case: &str) {
    let b = book();
    let mut store = MemoryStore::open(tmp(&format!("tool-argument-recovery-{case}"))).unwrap();
    let mut reader = Reader::new(&b, DEFAULT_RADIUS);
    let snapshot = default_profile_snapshot(&b, &store, "t0");
    let answer = "The retained catalog keeps this card even when the current route omits it.";
    let mut chats = Vec::new();
    for id in ["invalid-intent-1", "invalid-intent-2"] {
        let mut selection = selected_body(id, answer);
        selection["move"]["intent_origin"] = serde_json::json!("session_default");
        chats.push(turn_calls(vec![call(id, "tutor.step", &selection.to_string())]));
    }
    let mut missing = selected_body("missing-participation", answer);
    missing["move"].as_object_mut().unwrap().remove("participation");
    chats.push(turn_calls(vec![call("missing-participation", "tutor.step", &missing.to_string())]));
    if case == "repeat" {
        // Changing the move id must not turn another schema rejection into
        // progress or repeatedly grant the same correction opportunity.
        missing["move"]["move_id"] = serde_json::json!("still-missing-participation");
        chats.push(turn_calls(vec![call("still-missing-participation", "tutor.step", &missing.to_string())]));
    } else {
        chats.push(turn_calls(vec![call("corrected", "tutor.step", &selected_body("corrected", answer).to_string())]));
    }
    chats.push(turn_final("Use the accepted delivery."));
    let adapter = ReviewedTeachingAdapter::new(chats, if case == "success" { vec![accepted_review()] } else { vec![] });
    let mut context = RunContext::new(new_session(), OuterConfig {
        max_turns: Some(if case == "budget" { 3 } else { 8 }), ..Default::default()
    }, adapter.model_runtime_profile());
    context.tutor = Some(serde_json::json!({"status":"active", "default_teaching_intent":"guided_inquiry",
        "current_design":{"design_ref":"design:current","design":design()}, "learner_context":{}}));
    let mut port = ArgumentRecoveryTeachingPort { inner: TeachingPort {
        inner: BorrowedResidentState { store: &mut store, reader: &mut reader },
        accepted: Vec::new(), selections: Default::default(), reviews: Vec::new(),
    }};
    let outcome = run_context(&b, &mut port, &adapter, &mut context, &snapshot, &ResidentTurnResources::default(), None,
        &mut EphemeralCompactionCheckpointSink::default(), "Explain this relationship.", "t0");
    for id in ["invalid-intent-1", "invalid-intent-2"] {
        assert_eq!(tool_result_error_code(&tool_result(&context.messages, id)).as_deref(), Some("TEACHING_INVALID"));
    }
    let rejected: serde_json::Value = serde_json::from_str(&tool_result(&context.messages, "missing-participation")).unwrap();
    assert_eq!(rejected["error_code"], "TOOL_ARGUMENTS_INVALID");
    assert!(rejected["message"].as_str().unwrap().contains("$.move.participation"));
    let plans = adapter.inner.0.seen_plans.borrow();
    for index in [1,2] {
        let sampled=plans[index].ordered_messages();
        let current=sampled.iter().filter_map(|message|message.content.as_deref())
            .rfind(|text|text.starts_with("runtime_state_snapshot.v1")).unwrap();
        assert!(current.contains("agent.teaching_revision") && current.contains(answer),
            "host preflight rejection must keep the original draft available for the next author call");
    }
    if case != "budget" {
        let sampled=plans[3].ordered_messages();
        let feedback = serde_json::to_string(&sampled).unwrap();
        assert!(feedback.contains("TOOL_ARGUMENTS_INVALID") && feedback.contains("$.move.participation"),
            "the next sampling must consume the actual schema feedback before deciding how to retry");
        let current=sampled.iter().filter_map(|message|message.content.as_deref())
            .rfind(|text|text.starts_with("runtime_state_snapshot.v1")).unwrap();
        assert!(current.contains("agent.teaching_revision") && current.contains(answer),
            "schema rejection must preserve the actual draft so the author can fill the missing field");
        assert!(current.contains("missing-participation"));
    }
    let public_calls=serde_json::to_string(&context.messages.iter().flat_map(|message|message.tool_calls.iter()).collect::<Vec<_>>()).unwrap();
    assert!(!public_calls.contains(answer),"private revision recovery must not publish candidate text in tool history");
    if case == "success" {
        let outcome = outcome.unwrap();
        assert!(!outcome.incomplete);
        assert_eq!(outcome.answer.as_deref(), Some(answer));
        assert_eq!(outcome.teaching_delivery_move_refs, ["selected:corrected"]);
        assert_eq!(port.inner.accepted, ["select"]);
        assert!(port.inner.reviews.is_empty(), "host acceptance must not fabricate a review");
        assert_eq!(plans.len(), 5);
        let sampled=plans[4].ordered_messages();
        let current=sampled.iter().filter_map(|message|message.content.as_deref())
            .rfind(|text|text.starts_with("runtime_state_snapshot.v1")).unwrap();
        assert!(!current.contains("agent.teaching_revision"),"acceptance clears the private repair state");
    } else {
        if case == "budget" {
            let error = outcome.unwrap_err();
            assert_eq!(error.error_code, TURN_LIMIT_EXCEEDED);
            assert!(error.message.contains("$.move.participation"));
        } else {
            let outcome = outcome.unwrap();
            assert!(outcome.incomplete);
            assert_eq!(outcome.warning.as_deref(), Some("AGENT_NO_PROGRESS"));
        }
        assert!(port.inner.accepted.is_empty() && port.inner.reviews.is_empty());
        assert!(context.teaching_delivery_draft.is_none());
        assert_eq!(plans.len(), if case == "budget" { 3 } else { 5 },
            "argument correction cannot bypass the turn limit or renew a repeated schema failure");
        if case == "repeat" {
            assert_eq!(tool_result_error_code(&tool_result(&context.messages, "still-missing-participation")).as_deref(), Some("TOOL_ARGUMENTS_INVALID"));
        }
    }
}

#[test]
fn correction_guard_does_not_renew_cycles_or_changed_json_offsets() {
    let mut guard = ProgressPhaseGuard::default();
    guard.observe_correction_error("tutor.step", "TOOL_ARGUMENTS_INVALID", "tool arguments are not valid JSON: trailing characters at line 1 column 20");
    guard.recovery_used = true;
    guard.observe_correction_error("tutor.step", "TOOL_ARGUMENTS_INVALID", "tool arguments are not valid JSON: trailing characters at line 1 column 900");
    assert!(!guard.recovery_available());
    guard.observe_correction_error("tutor.step", "TOOL_ARGUMENTS_INVALID", "$.move.delivery_text is required\nCurrent draft: 1");
    assert!(guard.recovery_available());
    guard.recovery_used = true;
    guard.observe_correction_error("tutor.step", "TOOL_ARGUMENTS_INVALID", "tool arguments are not valid JSON: trailing characters at line 1 column 5");
    assert!(!guard.recovery_available(), "returning to a previously rejected shape is not a new failure");
    guard.observe_correction_error("tutor.step", "TOOL_ARGUMENTS_INVALID", "$.move.delivery_text is required\nCurrent draft: 2");
    assert!(!guard.recovery_available(), "new draft/repair instructions do not change the failed field");
}

#[test]
fn tool_argument_recovery_after_semantic_rejections_delivers_corrected_explain() {
    assert_tool_argument_recovery("success");
}

#[test]
fn tool_argument_recovery_does_not_renew_repeated_schema_errors() {
    assert_tool_argument_recovery("repeat");
}

#[test]
fn tool_argument_recovery_does_not_bypass_turn_budget() {
    assert_tool_argument_recovery("budget");
}

#[test]
fn distinct_argument_feedback_after_syntax_and_semantic_repair_reaches_delivery() {
    let b = book();
    let mut store = MemoryStore::open(tmp("successive-argument-repair")).unwrap();
    let mut reader = Reader::new(&b, DEFAULT_RADIUS);
    let snapshot = default_profile_snapshot(&b, &store, "t0");
    let answer = "The retained catalog keeps the card available for another route.";
    let mut semantic = selected_body("semantic", answer);
    semantic["move"]["intent_origin"] = serde_json::json!("session_default");
    let mut missing = selected_body("missing-body", answer);
    missing["move"].as_object_mut().unwrap().remove("delivery_text");
    let mut corrected = selected_body("corrected", answer);
    corrected["finish"] = serde_json::json!(true);
    let adapter = ReviewedTeachingAdapter::new(vec![
        turn_calls(vec![call("syntax", "tutor.step", r#"{"move":{}},"operation":"select"}"#)]),
        turn_calls(vec![call("semantic", "tutor.step", &semantic.to_string())]),
        turn_calls(vec![call("missing-body", "tutor.step", &missing.to_string())]),
        turn_calls(vec![call("corrected", "tutor.step", &corrected.to_string())]),
    ], vec![]);
    let mut context = RunContext::new(new_session(), OuterConfig::default(), adapter.model_runtime_profile());
    context.tutor = Some(serde_json::json!({"status":"active", "default_teaching_intent":"guided_inquiry",
        "current_design":{"design_ref":"design:current","design":design()}, "learner_context":{}}));
    let mut port = ArgumentRecoveryTeachingPort { inner: TeachingPort {
        inner: BorrowedResidentState { store: &mut store, reader: &mut reader },
        accepted: vec![], selections: Default::default(), reviews: vec![],
    }};
    let outcome = run_context(&b, &mut port, &adapter, &mut context, &snapshot, &ResidentTurnResources::default(), None,
        &mut EphemeralCompactionCheckpointSink::default(), "Explain this relationship.", "t0").unwrap();
    assert!(!outcome.incomplete);
    assert_eq!(outcome.answer.as_deref(), Some(answer));
    assert_eq!(outcome.turns, 4);
    let plans = adapter.inner.0.seen_plans.borrow();
    assert_eq!(plans.len(), 4, "new missing-field feedback must reach the next sampling");
    assert!(serde_json::to_string(&plans[3].ordered_messages()).unwrap().contains("$.move.delivery_text"));
    assert_repair_request_prefixes(&plans);
}

fn assert_repair_request_prefixes(plans: &[crate::model_runtime::AgentRequestPlan]) {
    use crate::deepseek_messages::{request_projection, MessagesCapabilities, ToolUpdateMode};
    let bodies: Vec<_> = plans.iter().map(|plan| request_projection("fixture", plan,
        MessagesCapabilities { tool_update: ToolUpdateMode::InHistory, system_in_history: true }).unwrap().0).collect();
    for pair in bodies.windows(2) {
        assert_eq!(pair[0]["system"], pair[1]["system"], "repair must not rewrite the system prefix");
        assert_eq!(pair[0]["tools"], pair[1]["tools"], "repair must not re-declare tools");
        let before = pair[0]["messages"].as_array().unwrap();
        let after = pair[1]["messages"].as_array().unwrap();
        assert_eq!(&after[..before.len()], before.as_slice(), "repair appends to the exact sent history");
    }
}

#[test]
fn tutor_repair_contract_measurement() {
    let context = serde_json::json!({"status":"active"});
    let spec = crate::tutor::model_spec_for_context(&context).unwrap();
    let report = serde_json::json!({"schema_chars":spec.parameters.to_string().chars().count(),
        "schema_utf8_bytes":spec.parameters.to_string().len(), "description_chars":spec.description.chars().count(),
        "examples":spec.parameters["examples"].as_array().unwrap().len()});
    if let Some(path) = std::env::var_os("TUTOR_REPAIR_CONTRACT_REPORT") {
        std::fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    }
    println!("{report}");
}
