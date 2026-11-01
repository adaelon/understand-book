use super::*;

#[test]
fn board_provider_failure_before_first_tool_keeps_current_work_without_another_model_call() {
    for regeneration in [false,true] {
        let b=book();
        let adapter=RequestPlanRecordingAdapter::new(vec![turn_final("Unrelated music and presentation claims must never be sampled.")],vec![]);
        let mut context=RunContext::new(new_session(),OuterConfig::default(),adapter.model_runtime_profile());
        // Previous turns' actions and pending candidates are historical scope,
        // not new operations by this run.
        context.messages.push(Message{role:Role::Tool,content:Some(r#"{"status":"board_part_committed","candidate_id":"old"}"#.into()),tool_calls:vec![],tool_call_id:Some("historical-commit".into()),provider_continuation:None});
        context.board_request_context=Some(serde_json::json!({"board_id":"board","selected_item":{"item_id":"mutable","title":"人工保留的标题"},
            "regeneration":if regeneration {serde_json::json!({"source":{"candidate_id":"old-pending"},"original_request":"Add an example"})} else {serde_json::Value::Null}}));
        context.tutor=Some(serde_json::json!({"status":"active"}));
        let error=ToolError{error_code:"PROVIDER_ERROR".into(),category:"provider".into(),message:"HTTP 400; all tools cannot be deferred".into()};
        let result=delivery_fallback::deliver(&b,&adapter,&mut context,&ResidentTurnResources::default(),
            &mut EphemeralCompactionCheckpointSink::default(),"基于当前内容完成未应用的修改",&error,None).unwrap();
        let answer=result.answer.as_deref().unwrap();
        assert!(answer.contains("模型服务") && answer.contains("没有应用新的画板修改"));
        assert_eq!(answer.contains("原待处理建议都已保留"),regeneration);
        assert!(!answer.contains("音乐") && !answer.contains("saved_presentations") && !answer.contains("缺少"));
        assert!(adapter.seen_plans.borrow().is_empty());
        assert!(result.incomplete && result.warning.as_deref()==Some("PROVIDER_ERROR"));
        assert!(!context.tutor_resolved && result.teaching_delivery_move_refs.is_empty());
        assert!(context.board_candidate_id.is_none() && context.board_delivery_draft.is_none() && result.effects.is_empty());
        assert_eq!(result.delivery_diagnostics.unwrap().initial.issues[0].trigger_value.as_deref(),Some(error.message.as_str()),"the actual provider cause remains recorded");
    }
}

#[test]
fn board_provider_fallback_keeps_current_run_delivery_and_read_paths() {
    for scenario in ["accepted_board","saved_page","executed_tool"] {
        let b=book();
        let adapter=RequestPlanRecordingAdapter::new(vec![turn_final("The available explanation remains useful; further work is incomplete.")],vec![]);
        let mut context=RunContext::new(new_session(),OuterConfig::default(),adapter.model_runtime_profile());
        context.board_request_context=Some(serde_json::json!({"board_id":"board"}));
        if scenario=="accepted_board" {context.board_delivery_draft=Some("The exact accepted board explanation.".into());}
        if scenario=="saved_page" {context.delivered_presentations.push(crate::presentation::PresentationRef{presentation_id:"saved".into(),revision:1});}
        if scenario=="executed_tool" {context.trace.push(TraceStep{model_tool_loop:Some(1),tool:"board.author".into(),args:r#"{"operation":"read"}"#.into(),result_digest:"observed board".into(),query_audit:None});}
        let result=delivery_fallback::deliver(&b,&adapter,&mut context,&ResidentTurnResources::default(),
            &mut EphemeralCompactionCheckpointSink::default(),"Explain this board",&ToolError{error_code:"PROVIDER_ERROR".into(),category:"provider".into(),message:"offline".into()},None).unwrap();
        assert!(result.incomplete);
        assert!(!result.answer.as_deref().unwrap().contains("没有应用新的画板修改"),"only a failure before any current-run work has that status");
        if scenario=="accepted_board" {
            assert_eq!(result.answer.as_deref(),Some("The exact accepted board explanation."));
            assert!(adapter.seen_plans.borrow().is_empty());
        } else {
            assert_eq!(adapter.seen_plans.borrow().len(),1,"existing material-based fallback remains available");
        }
        if scenario=="saved_page" {assert!(result.answer_view.unwrap().parts.iter().any(|part|matches!(part,AgentAnswerPart::Presentation{presentation_id,..} if presentation_id=="saved")));}
    }
}

#[test]
fn available_answer_retains_ui_request_without_typed_text() {
    let b = book();
    let accepted = "Accepted reading action: practice the relationships already taught in this material.";
    for (typed, expected) in [("", accepted), ("  ", accepted), ("Use only the last section.", "Use only the last section.")] {
        let adapter = RequestPlanRecordingAdapter::new(vec![turn_final("The requested practice could not be completed.")], vec![]);
        let mut context = RunContext::new(new_session(), OuterConfig::default(), adapter.model_runtime_profile());
        context.current_user_message = Some(typed.into());
        delivery_fallback::deliver(&b, &adapter, &mut context, &ResidentTurnResources::default(),
            &mut EphemeralCompactionCheckpointSink::default(), accepted,
            &ToolError { error_code:"TEACHING_INVALID".into(), category:"validation".into(), message:"No valid problem was selected.".into() }, None).unwrap();
        let plans = adapter.seen_plans.borrow();
        assert_eq!(plans.len(), 1);
        let payload: serde_json::Value = serde_json::from_str(plans[0].ordered_messages()[1].content.as_deref().unwrap()).unwrap();
        assert_eq!(payload["question"], expected);
    }
}

#[test]
fn available_answer_uses_a_small_tools_disabled_request_after_empty_provider() {
    let b = book();
    let mut store = MemoryStore::open(tmp("available-answer-synthesis")).unwrap();
    let mut reader = Reader::new(&b, DEFAULT_RADIUS);
    let snapshot = default_profile_snapshot(&b, &store, "t0");
    let adapter = RequestPlanRecordingAdapter::new(vec![
        turn_calls(vec![call("read", "book.text", r#"{"lid":"1.1"}"#)]),
        turn_final(""), turn_final("Here is the available explanation. The requested presentation is still unfinished."),
    ], vec![]);
    let mut context = RunContext::new(new_session(), OuterConfig::default(), adapter.model_runtime_profile());
    let result = run_context(&b, &mut BorrowedResidentState {store:&mut store, reader:&mut reader},
        &adapter, &mut context, &snapshot, &ResidentTurnResources::default(), None,
        &mut EphemeralCompactionCheckpointSink::default(), "Explain 1.1 and show a demonstration", "t0").unwrap();
    assert!(result.incomplete);
    assert_eq!(result.warning.as_deref(), Some("PROVIDER_EMPTY_RESPONSE"));
    assert!(result.answer.unwrap().contains("available explanation"));
    assert!(result.teaching_delivery_move_refs.is_empty());
    assert_eq!(result.turns, 3);
    assert_eq!(result.trace.len(), 1);
    let plans = adapter.seen_plans.borrow();
    assert_eq!(plans.len(), 3);
    assert!(plans[2].tools.is_empty());
    assert_eq!(plans[2].ordered_messages().len(), 2);
    let payload: serde_json::Value = serde_json::from_str(plans[2].ordered_messages()[1].content.as_deref().unwrap()).unwrap();
    assert!(!payload["read_evidence"].as_array().unwrap().is_empty());
    assert!(result.delivery_diagnostics.unwrap().fallback.unwrap().issues.is_empty());
}

#[test]
fn available_answer_keeps_source_excerpts_when_provider_is_unavailable() {
    let b = book();
    let mut store = MemoryStore::open(tmp("available-answer-excerpts")).unwrap();
    let mut reader = Reader::new(&b, DEFAULT_RADIUS);
    let snapshot = default_profile_snapshot(&b, &store, "t0");
    let adapter = RequestPlanRecordingAdapter::new(vec![
        turn_calls(vec![call("read", "book.text", r#"{"lid":"1.1"}"#)]),
    ], vec![]);
    let mut context = RunContext::new(new_session(), OuterConfig::default(), adapter.model_runtime_profile());
    let result = run_context(&b, &mut BorrowedResidentState {store:&mut store,reader:&mut reader}, &adapter,
        &mut context, &snapshot, &ResidentTurnResources::default(), None,
        &mut EphemeralCompactionCheckpointSink::default(), "Explain 1.1", "t0").unwrap();
    assert!(result.incomplete);
    assert!(result.answer.as_ref().unwrap().contains("本轮已读到的原文"));
    assert!(!result.source_bindings.is_empty());
    assert!(!result.answer_view.as_ref().unwrap().sources.is_empty());
    assert_eq!(adapter.seen_plans.borrow().len(), 3, "only one fallback sampling after the failed main request");
    assert_eq!(context.messages.last().unwrap().content, result.answer);
    assert!(result.teaching_delivery_move_refs.is_empty());
}

#[test]
fn available_answer_source_repair_failure_keeps_prior_diagnostics_without_another_model_call() {
    let b = book();
    let mut context = RunContext::new(new_session(), OuterConfig::default(), ModelRuntimeProfile::fallback("fallback", ProviderToolProtocol::Native));
    context.evidence_ledger.observe(EvidenceRange { start_lid:"1.1".into(), end_lid:"1.1".into(), ranges:Vec::new() });
    let adapter = RequestPlanRecordingAdapter::new(vec![],vec![]);
    let diagnostics = AnswerDeliveryDiagnostics {
        initial:AnswerDeliveryAttemptDiagnostics {issues:vec![delivery_issue("RAW_LID_LEAK","test")]},
        repair:Some(AnswerDeliveryAttemptDiagnostics {issues:vec![delivery_issue("REPAIR_EMPTY_ANSWER","test")]}), fallback:None,
    };
    let previous = OuterOutcome {
        answer:Some(SOURCE_PRESENTATION_FAILURE_MESSAGE.into()), answer_view:None, incomplete:true, warning:None,
        turns:7,tokens_spent:420,effects:Vec::new(),trace:Vec::new(),profile_usage:Default::default(),memory_updates:Vec::new(),
        teaching_delivery_move_refs:Vec::new(),source_bindings:Vec::new(),delivery_diagnostics:Some(diagnostics.clone()),request_audit:Default::default(),
    };
    let result = delivery_fallback::deliver(&b,&adapter,&mut context,&ResidentTurnResources::default(),
        &mut EphemeralCompactionCheckpointSink::default(),"Explain this",&source_presentation_error("ANSWER_DELIVERY_INCOMPLETE","failed"),Some(previous)).unwrap();
    assert_eq!(result.turns,7);
    assert_eq!(result.tokens_spent,420);
    let updated = result.delivery_diagnostics.unwrap();
    assert_eq!(updated.initial,diagnostics.initial);
    assert_eq!(updated.repair,diagnostics.repair);
    assert!(updated.fallback.unwrap().issues.is_empty());
    assert!(adapter.seen_plans.borrow().is_empty());
    assert!(!result.source_bindings.is_empty());
}

#[test]
fn available_answer_does_not_retry_cancellation_spend_or_storage_failures() {
    for category in ["cancelled","model_spend","storage","authorization"] {
        assert!(!delivery_fallback::recoverable(&ToolError {error_code:"STOP".into(),category:category.into(),message:"stop".into()}));
    }
}

#[test]
fn available_answer_reports_explicit_provider_balance_failure_without_retrying() {
    for (language, with_source) in [
        (None, false),
        (Some(crate::response_language::ResponseLanguage::English), false),
        (None, true),
    ] {
        let b = book();
        let adapter = RequestPlanRecordingAdapter::new(vec![], vec![]);
        let mut context = RunContext::new(new_session(), OuterConfig::default(), adapter.model_runtime_profile());
        context.response_language = language;
        if with_source {
            context.evidence_ledger.observe(EvidenceRange { start_lid:"1.1".into(), end_lid:"1.1".into(), ranges:Vec::new() });
        }
        // Same status/body shape as the failed first teaching turn on 2026-10-25.
        let error = ToolError { error_code:"PROVIDER_ERROR".into(), category:"provider".into(),
            message:r#"HTTP 请求失败: status code 402; {"error":{"message":"Insufficient Balance (request_id: internal-request)","type":"insufficient_balance_error","param":null,"code":"invalid_request_error"}}"#.into() };
        let result = delivery_fallback::deliver(&b, &adapter, &mut context, &ResidentTurnResources::default(),
            &mut EphemeralCompactionCheckpointSink::default(), "Explain this chapter", &error, None).unwrap();
        let answer = result.answer.as_deref().unwrap();
        assert!(answer.contains(if language.is_some() { "insufficient balance" } else { "模型服务余额不足" }));
        assert!(!answer.contains("可核对材料") && !answer.contains("verified material"));
        assert!(!answer.contains("internal-request") && !answer.contains("status code") && !answer.contains("invalid_request_error"));
        assert!(adapter.seen_plans.borrow().is_empty(), "a second model request cannot replenish the service balance");
        assert_eq!(!result.source_bindings.is_empty(), with_source, "already read sources remain available");
        assert!(result.incomplete && result.teaching_delivery_move_refs.is_empty());
        assert_eq!(context.messages.last().unwrap().content, result.answer);
    }
}

#[test]
fn available_answer_distinguishes_provider_failure_from_missing_source_material() {
    for (error, expected, excluded) in [
        (ToolError { error_code:"PROVIDER_ERROR".into(), category:"provider".into(),
            message:"HTTP 请求失败: status code 402".into() }, "模型服务本次未能完成回复", "余额不足"),
        (ToolError { error_code:"TEACHING_EVIDENCE_REQUIRED".into(), category:"validation".into(),
            message:"No source text was read".into() }, "尚未取得足够的可核对材料", "模型服务"),
    ] {
        let b = book();
        let adapter = RequestPlanRecordingAdapter::new(vec![], vec![]);
        let mut context = RunContext::new(new_session(), OuterConfig::default(), adapter.model_runtime_profile());
        let result = delivery_fallback::deliver(&b, &adapter, &mut context, &ResidentTurnResources::default(),
            &mut EphemeralCompactionCheckpointSink::default(), "Explain this chapter", &error, None).unwrap();
        let answer = result.answer.unwrap();
        assert!(answer.contains(expected), "{answer}");
        assert!(!answer.contains(excluded), "{answer}");
        assert!(result.incomplete);
    }
}

#[test]
fn available_answer_preserves_accepted_and_saved_presentations_without_duplicates() {
    let b = book();
    let mut context = RunContext::new(new_session(), OuterConfig::default(), ModelRuntimeProfile::fallback("fallback", ProviderToolProtocol::Native));
    context.teaching_delivery_draft = Some(serde_json::json!({"text":"See the accepted demonstration below.",
        "moves":[{"presentation":{"presentation_id":"accepted-page","revision":2}}]}));
    context.delivered_presentations = vec![
        crate::presentation::PresentationRef {presentation_id:"accepted-page".into(),revision:2},
        crate::presentation::PresentationRef {presentation_id:"unselected-page".into(),revision:1},
    ];
    context.presentation_candidates.insert("unsaved-candidate".into());
    let adapter = RequestPlanRecordingAdapter::new(vec![],vec![]);
    let result = delivery_fallback::deliver(&b,&adapter,&mut context,&ResidentTurnResources::default(),
        &mut EphemeralCompactionCheckpointSink::default(),"Show a demo",
        &ToolError {error_code:"PROVIDER_ERROR".into(),category:"provider".into(),message:"offline".into()},None).unwrap();
    assert!(result.incomplete && result.teaching_delivery_move_refs.is_empty());
    let parts = result.answer_view.unwrap().parts;
    assert_eq!(parts.iter().filter(|part|matches!(part,AgentAnswerPart::Presentation {presentation_id,revision:2} if presentation_id=="accepted-page")).count(),1);
    assert!(parts.iter().any(|part|matches!(part,AgentAnswerPart::Presentation {presentation_id,revision:1} if presentation_id=="unselected-page")));
    assert_eq!(parts.iter().filter(|part|matches!(part,AgentAnswerPart::Presentation {..})).count(),2);
    assert!(adapter.seen_plans.borrow().is_empty(), "accepted content needs no additional sampling");
}

#[test]
fn available_answer_keeps_delivered_page_when_teaching_selection_was_rejected() {
    let b = book();
    let mut context = RunContext::new(new_session(), OuterConfig::default(), ModelRuntimeProfile::fallback("fallback", ProviderToolProtocol::Native));
    let reference = crate::presentation::PresentationRef {presentation_id:"saved-unselected".into(),revision:3};
    context.delivered_presentations.push(reference.clone());
    context.presentation_candidates.insert("unfinished-candidate".into());
    context.presentation_delivery_observations.insert(serde_json::to_string(&reference).unwrap(), vec![
        serde_json::json!({"environment_name":"desktop","status":"preview_ready_for_inspection"}),
        serde_json::json!({"environment_name":"mobile","status":"preview_environment_recorded"}),
    ]);
    let adapter = RequestPlanRecordingAdapter::new(vec![turn_final("The relationship can be explored in the saved page; the accompanying explanation still needs correction.")],vec![]);
    let error = ToolError {error_code:"TEACHING_REVIEW_FAILED".into(),category:"validation".into(),
        message:"The explanation says the feature appears at 3, but the observed page shows it at 2.".into()};
    let result = delivery_fallback::deliver(&b,&adapter,&mut context,&ResidentTurnResources::default(),
        &mut EphemeralCompactionCheckpointSink::default(),"Show a demo",&error,None).unwrap();
    assert!(result.incomplete);
    assert_eq!(result.warning.as_deref(),Some("TEACHING_REVIEW_FAILED"));
    assert!(result.teaching_delivery_move_refs.is_empty(), "fallback does not accept the rejected teaching selection");
    assert!(result.answer.as_ref().unwrap().contains("可直接打开"));
    assert!(result.answer.as_ref().unwrap().contains("页面与配套讲解的内容仍待进一步核对"));
    assert_eq!(result.answer_view.unwrap().parts.iter().filter(|part|
        matches!(part,AgentAnswerPart::Presentation {presentation_id,revision:3} if presentation_id=="saved-unselected")).count(),1);
    let plans = adapter.seen_plans.borrow();
    assert_eq!(plans.len(),1);
    assert!(plans[0].tools.is_empty());
    let messages = plans[0].ordered_messages();
    let payload: serde_json::Value = serde_json::from_str(messages[1].content.as_deref().unwrap()).unwrap();
    assert_eq!(payload["saved_presentations"].as_array().unwrap().len(),1);
    assert_eq!(payload["saved_presentations"][0]["reference"],serde_json::to_value(reference).unwrap());
    assert_eq!(payload["saved_presentations"][0]["saved"],true);
    assert_eq!(payload["saved_presentations"][0]["author_previews"].as_array().unwrap().len(),2);
    assert_eq!(payload["remaining_work"]["unresolved_issue"],error.message);
    let instructions = messages[0].content.as_deref().unwrap();
    assert!(instructions.contains("Do not claim these pages were not made or could not be run"));
    assert!(instructions.contains("Do not reproduce HTML"));
}

#[test]
fn available_answer_replaces_html_fallback_with_sources_and_the_saved_page() {
    for bad_answer in [
        "I cannot run the demo here. Save this file yourself:\n```html\n<div>Replacement demo</div>\n```",
        "<!DOCTYPE html><html><body><button>Replacement demo</button></body></html>",
        "```\n<div><button>Replacement demo</button></div>\n```",
    ] {
        let b = book();
        let mut context = RunContext::new(new_session(), OuterConfig::default(), ModelRuntimeProfile::fallback("fallback", ProviderToolProtocol::Native));
        context.evidence_ledger.observe(EvidenceRange {start_lid:"1.1".into(),end_lid:"1.1".into(),ranges:Vec::new()});
        context.delivered_presentations.push(crate::presentation::PresentationRef {presentation_id:"available-demo".into(),revision:2});
        let adapter = RequestPlanRecordingAdapter::new(vec![turn_final(bad_answer)],vec![]);
        let result = delivery_fallback::deliver(&b,&adapter,&mut context,&ResidentTurnResources::default(),
            &mut EphemeralCompactionCheckpointSink::default(),"Show a demo",
            &ToolError {error_code:"TEACHING_REVIEW_FAILED".into(),category:"validation".into(),message:"explanation mismatch".into()},None).unwrap();
        let answer = result.answer.as_ref().unwrap();
        assert!(answer.contains("本轮已读到的原文"));
        assert!(answer.contains("可直接打开"));
        assert!(!answer.contains("Replacement demo") && !answer.contains("cannot run") && !answer.contains("```"));
        let view = result.answer_view.unwrap();
        assert!(!view.sources.is_empty());
        assert!(view.parts.iter().any(|part|matches!(part,AgentAnswerPart::Presentation {presentation_id,revision:2} if presentation_id=="available-demo")));
        assert!(result.incomplete && result.teaching_delivery_move_refs.is_empty());
        assert_eq!(adapter.seen_plans.borrow().len(),1, "invalid HTML uses deterministic fallback without another model call");
        assert_eq!(context.messages.last().unwrap().content,result.answer);
    }
}

#[test]
fn available_answer_does_not_promote_candidates_or_previous_turn_pages() {
    let b = book();
    let mut messages = new_session();
    messages.push(Message {role:Role::Assistant,content:Some("Previous turn had a saved demo.".into()),tool_calls:vec![
        call("old-deliver","presentation.author",r#"{"operation":"deliver","candidate_id":"previous-candidate"}"#),
    ],tool_call_id:None,provider_continuation:None});
    messages.push(Message {role:Role::Tool,content:Some(r#"{"status":"delivered","reference":{"presentation_id":"previous-page","revision":1}}"#.into()),
        tool_calls:Vec::new(),tool_call_id:Some("old-deliver".into()),provider_continuation:None});
    let mut context = RunContext::new(messages, OuterConfig::default(), ModelRuntimeProfile::fallback("fallback", ProviderToolProtocol::Native));
    context.presentation_candidates.insert("current-unsaved-candidate".into());
    let adapter = RequestPlanRecordingAdapter::new(vec![turn_final("```html\n<p>Save this as a demo</p>\n```")],vec![]);
    let result = delivery_fallback::deliver(&b,&adapter,&mut context,&ResidentTurnResources::default(),
        &mut EphemeralCompactionCheckpointSink::default(),"Show a new demo",
        &ToolError {error_code:"PROVIDER_ERROR".into(),category:"provider".into(),message:"offline".into()},None).unwrap();
    let answer = result.answer.unwrap();
    assert!(answer.contains("模型服务本次未能完成回复"));
    assert!(!answer.contains("已保存的演示") && !answer.contains("```"));
    assert!(!result.answer_view.unwrap().parts.iter().any(|part|matches!(part,AgentAnswerPart::Presentation {..})));
    assert!(result.incomplete && result.teaching_delivery_move_refs.is_empty());
    let plans = adapter.seen_plans.borrow();
    let payload: serde_json::Value = serde_json::from_str(plans[0].ordered_messages()[1].content.as_deref().unwrap()).unwrap();
    assert!(payload["saved_presentations"].as_array().unwrap().is_empty());
}

#[test]
fn available_answer_preserves_the_users_video_question_image() {
    let b = book();
    let adapter = RequestPlanRecordingAdapter::new(vec![turn_final("The attached image shows the requested frame.")], vec![]);
    let mut context = RunContext::new(new_session(), OuterConfig::default(), adapter.model_runtime_profile());
    let image = base_schema::VideoQuestionImage { png_base64:"question-pixels".into(), width:576, height:432, origin:base_schema::VideoQuestionImageOrigin::Selection };
    context.question_image = Some(image.clone());
    let result = delivery_fallback::deliver(&b, &adapter, &mut context, &ResidentTurnResources::default(),
        &mut EphemeralCompactionCheckpointSink::default(), "Explain this frame", &ToolError {
            error_code:"PROVIDER_EMPTY_RESPONSE".into(), category:"provider".into(), message:"empty".into(),
        }, None).unwrap();
    assert!(result.answer.unwrap().contains("attached image"));
    let plans = adapter.seen_plans.borrow();
    assert_eq!(plans.len(), 1);
    assert_eq!(plans[0].question_image, Some(image));
    assert!(plans[0].tools.is_empty());
    assert!(context.media_observations.is_empty(), "A user attachment is not a verified video read");
}
