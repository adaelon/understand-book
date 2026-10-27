fn unfinished_review() -> AdapterError {
    AdapterError { spend_stop: None, message: "Provider unfinished response: length".into() }
}

#[test]
fn teaching_review_usage_preserves_reported_failure_and_unknown_usage() {
    for reported_usage in [Some(58_016), None] {
        let mut adapter = ReviewedTeachingAdapter::new(vec![], vec![Err(unfinished_review())]);
        adapter.reported_review_usage = reported_usage;
        let failure = crate::tutor::review_delivery(&adapter, adapter.model_runtime_profile(),
            serde_json::json!({"original_request":"Explain the relation.",
                "proposed_delivery":{"text":"The catalog retains the card omitted from this route."}}), Vec::new()).unwrap_err();
        assert_eq!(failure.reported_usage, reported_usage, "unknown usage must remain unknown rather than become a measured zero");
        assert_eq!(failure.error.message, "Provider unfinished response: length");
        assert!(failure.error.spend_stop.is_none());
    }
}

#[test]
fn teaching_review_usage_counts_failed_attempt_and_successful_retry_once() {
    let b = book();
    let mut store = MemoryStore::open(tmp("teaching-review-usage-retry")).unwrap();
    let mut reader = Reader::new(&b, DEFAULT_RADIUS);
    let snapshot = default_profile_snapshot(&b, &store, "t0");
    let select = selected_body("usage-retry", "The retained catalog still contains the omitted card.");
    let adapter = ReviewedTeachingAdapter::new(vec![
        turn_calls(vec![call("first", "tutor.step", &select.to_string())]),
        turn_calls(vec![call("retry", "tutor.step", &select.to_string())]),
        turn_final("Use the accepted delivery."),
    ], vec![Err(unfinished_review()), accepted_review()]);
    let mut context = RunContext::new(new_session(), OuterConfig { max_turns: Some(8), ..Default::default() }, adapter.model_runtime_profile());
    context.tutor = Some(serde_json::json!({"status":"active",
        "current_design":{"design_ref":"design:current","design":design()},"learner_context":{}}));
    let mut port = TeachingPort { inner: BorrowedResidentState { store: &mut store, reader: &mut reader },
        accepted: Vec::new(), selections: Default::default(), reviews: Vec::new() };
    let outcome = run_context(&b, &mut port, &adapter, &mut context, &snapshot, &ResidentTurnResources::default(), None,
        &mut EphemeralCompactionCheckpointSink::default(), "Explain the relation.", "t0").unwrap();
    assert!(!outcome.incomplete);
    assert_eq!(outcome.answer.as_deref(), select["move"]["delivery_text"].as_str());
    assert_eq!(outcome.tokens_spent, 3 * 10 + 3 * 171,
        "both factual attempts and the independent explanation review must each count once");
    assert_eq!(context.tokens_spent, outcome.tokens_spent);
    assert_eq!(adapter.seen_reviews.borrow().len(), 2);
    assert_eq!(adapter.seen_explanation_reviews.borrow().len(), 1);
    assert_eq!(port.accepted, ["select"]);
    assert_eq!(port.reviews.len(), 1, "a provider failure has no verdict to persist");
}

#[test]
fn teaching_review_usage_keeps_turn_and_spend_stops_without_another_request() {
    for spend_stop in [None, Some(crate::model_spend::SpendStop::InsufficientAllowance)] {
        let b = book();
        let mut store = MemoryStore::open(tmp(&format!("teaching-review-usage-stop-{spend_stop:?}"))).unwrap();
        let mut reader = Reader::new(&b, DEFAULT_RADIUS);
        let snapshot = default_profile_snapshot(&b, &store, "t0");
        let select = selected_body("budget-stop", "UNACCEPTED_USAGE_DRAFT");
        let mut adapter = ReviewedTeachingAdapter::new(vec![
            turn_calls(vec![call("first", "tutor.step", &select.to_string())]),
        ], vec![Err(AdapterError { spend_stop, ..unfinished_review() })]);
        adapter.reported_review_usage = Some(58_016);
        let mut context = RunContext::new(new_session(), OuterConfig { max_turns: Some(2), ..Default::default() }, adapter.model_runtime_profile());
        context.tutor = Some(serde_json::json!({"status":"active",
            "current_design":{"design_ref":"design:current","design":design()},"learner_context":{}}));
        let mut port = TeachingPort { inner: BorrowedResidentState { store: &mut store, reader: &mut reader },
            accepted: Vec::new(), selections: Default::default(), reviews: Vec::new() };
        let error = if spend_stop.is_some() {
            // A real allowance stop also bypasses the outer fallback wrapper.
            run_context(&b, &mut port, &adapter, &mut context, &snapshot, &ResidentTurnResources::default(), None,
                &mut EphemeralCompactionCheckpointSink::default(), "Explain the relation.", "t0")
        } else {
            // The existing model-turn limit must stop this loop after one
            // outer request plus one review, even though the verdict failed.
            run_context_inner(&b, &mut port, &adapter, &mut context, &snapshot, &ResidentTurnResources::default(), None,
                &mut EphemeralCompactionCheckpointSink::default(), "Explain the relation.", "t0")
        }.unwrap_err();
        assert_eq!(context.tokens_spent, 10 + 58_016, "failed review usage must survive loop termination");
        assert_eq!(context.model_turns, 2);
        assert_eq!(adapter.inner.0.seen_plans.borrow().len(), 1, "no outer retry or fallback request after the relevant stop");
        assert_eq!(adapter.seen_reviews.borrow().len(), 1);
        assert!(port.accepted.is_empty() && port.reviews.is_empty());
        assert!(context.teaching_delivery_draft.is_none());
        if let Some(reason) = spend_stop {
            assert_eq!(error.error_code, reason.code());
            assert_eq!(error.category, "model_spend");
        } else {
            assert_eq!(error.error_code, "TEACHING_REVIEW_FAILED");
            assert!(error.message.contains("Provider unfinished response: length"));
        }
    }
}

#[test]
fn teaching_explanation_failure_or_cancel_preserves_usage_without_selecting_or_persisting() {
    for failure in ["provider", "unknown_usage", "cancel", "spend"] {
        let b = book();
        let mut store = MemoryStore::open(tmp(&format!("explanation-review-stop-{failure}"))).unwrap();
        let mut reader = Reader::new(&b, DEFAULT_RADIUS);
        let snapshot = default_profile_snapshot(&b, &store, "t0");
        let mut selection = selected_body("unaccepted-explanation", "UNACCEPTED_EXPLANATION_DRAFT");
        selection["finish"] = serde_json::json!(true);
        let mut adapter = ReviewedTeachingAdapter::new(vec![
            turn_calls(vec![call("select", "tutor.step", &selection.to_string())]),
        ], vec![accepted_review()]);
        adapter.reported_explanation_usage = if failure == "unknown_usage" {None} else {Some(58_016)};
        adapter.explanation_reviews.borrow_mut().push_back(if failure == "cancel" {
            accepted_review()
        } else {
            Err(AdapterError {spend_stop:if failure=="spend" {Some(crate::model_spend::SpendStop::InsufficientAllowance)} else {None},
                message:"Explanation review did not finish".into()})
        });
        let mut context = RunContext::new(new_session(), OuterConfig {max_turns:Some(3), ..Default::default()}, adapter.model_runtime_profile());
        if failure == "cancel" { adapter.cancel_after_explanation = Some(context.cancellation.clone()); }
        let sink = std::sync::Arc::new(TeachingStreamSink::default());
        context.events = crate::run_events::RunEvents::new(Some(sink.clone()));
        context.tutor = Some(serde_json::json!({"status":"active","current_design":{"design_ref":"design:current","design":design()},"learner_context":{}}));
        let mut port = TeachingPort {inner:BorrowedResidentState {store:&mut store,reader:&mut reader},
            accepted:vec![], selections:Default::default(), reviews:vec![]};
        let error = run_context_inner(&b, &mut port, &adapter, &mut context, &snapshot, &ResidentTurnResources::default(), None,
            &mut EphemeralCompactionCheckpointSink::default(), "Explain why B remains available.", "t0").unwrap_err();
        assert_eq!(context.model_turns, 3, "{failure}");
        assert_eq!(context.tokens_spent, 10 + 171 + adapter.reported_explanation_usage.unwrap_or(0), "{failure}");
        assert_eq!(adapter.inner.0.seen_plans.borrow().len(), 1, "{failure}: no outer retry after this stop");
        assert_eq!(adapter.seen_reviews.borrow().len(), 1);
        assert_eq!(adapter.seen_explanation_reviews.borrow().len(), 1);
        assert!(port.accepted.is_empty() && port.reviews.is_empty(),
            "{failure}: a factual accept cannot become a complete review or selected draft");
        assert!(context.teaching_delivery_draft.is_none());
        assert!(!serde_json::to_string(&*sink.0.lock().unwrap()).unwrap().contains("UNACCEPTED_EXPLANATION_DRAFT"));
        match failure {
            "cancel" => assert_eq!(error.error_code, "AGENT_RUN_CANCELLED"),
            "spend" => assert_eq!(error.error_code, crate::model_spend::SpendStop::InsufficientAllowance.code()),
            _ => assert!(error.message.contains("Explanation review did not finish")),
        }
    }
}

#[test]
fn teaching_independent_review_rechecks_turn_and_experimental_budgets_between_calls() {
    for stop in ["before_explanation", "before_explanation_repair"] {
        let b = book();
        let mut store = MemoryStore::open(tmp(&format!("independent-review-budget-{stop}"))).unwrap();
        let mut reader = Reader::new(&b, DEFAULT_RADIUS);
        let snapshot = default_profile_snapshot(&b, &store, "t0");
        let mut selection = selected_body("budget", "UNACCEPTED_BUDGET_DRAFT");
        selection["finish"] = serde_json::json!(true);
        let mut adapter = ReviewedTeachingAdapter::new(vec![
            turn_calls(vec![call("select", "tutor.step", &selection.to_string())]),
        ], vec![accepted_review()]);
        if stop == "before_explanation_repair" {
            adapter.explanation_reviews.borrow_mut().push_back(Ok(serde_json::json!({
                "decision":"accept","revision":null,"learner_claims":[],"request_resolution":{
                    "status":"answered","question_quote":"Explain why B remains available.",
                    "answer_quotes":["A fabricated quote"],"reason":"The fixture claims an answer."}})));
        }
        let limit = Some(if stop == "before_explanation" { 2 } else { 3 });
        let mut context = RunContext::new(new_session(), OuterConfig {max_turns:limit, ..Default::default()}, adapter.model_runtime_profile());
        context.tutor = Some(serde_json::json!({"status":"active","current_design":{"design_ref":"design:current","design":design()},"learner_context":{}}));
        let mut port = TeachingPort {inner:BorrowedResidentState {store:&mut store,reader:&mut reader},
            accepted:vec![], selections:Default::default(), reviews:vec![]};
        let _error = run_context_inner(&b, &mut port, &adapter, &mut context, &snapshot, &ResidentTurnResources::default(), None,
            &mut EphemeralCompactionCheckpointSink::default(), "Explain why B remains available.", "t0").unwrap_err();
        let explanation_calls = usize::from(stop == "before_explanation_repair");
        assert_eq!(adapter.inner.0.seen_plans.borrow().len(), 1, "{stop}: no unbudgeted fallback or author request");
        assert_eq!(adapter.seen_reviews.borrow().len(), 1);
        assert_eq!(adapter.seen_explanation_reviews.borrow().len(), explanation_calls,
            "{stop}: a prior call's budget check cannot authorize the next call");
        assert_eq!(context.model_turns, 2 + explanation_calls);
        assert_eq!(context.tokens_spent, 10 + adapter.reported_review_usage.unwrap() + explanation_calls as u32 * 171);
        assert!(port.accepted.is_empty() && port.reviews.is_empty(), "{stop}");
        assert!(context.teaching_delivery_draft.is_none());
    }
}

#[test]
fn teaching_review_candidate_rechecks_experimental_tokens_between_judgments() {
    // The reading experiment deliberately does not expose tutor.step. Exercise
    // the shared candidate gate directly instead of pretending that tool is
    // available in an experimental outer loop.
    let b = book();
    let mut store = MemoryStore::open(tmp("independent-review-experimental-budget")).unwrap();
    let mut reader = Reader::new(&b, DEFAULT_RADIUS);
    let mut adapter = ReviewedTeachingAdapter::new(vec![], vec![accepted_review()]);
    adapter.reported_review_usage = Some(99_990);
    let mut port = TeachingPort {inner:BorrowedResidentState {store:&mut store,reader:&mut reader},
        accepted:vec![], selections:Default::default(), reviews:vec![]};
    let packet = serde_json::json!({"original_request":"Explain why B remains available.",
        "proposed_delivery":{"text":"The retained catalog still contains B.",
            "moves":[{"target":{"source_bindings":[]}}]}});
    let mut turns = 1;
    let mut spent = 10;
    let error = review_delivery_candidate(&mut port, &adapter, &adapter.model_runtime_profile(),
        packet, vec![], &serde_json::json!({}), &mut TeachingDeliveryReviewHistory::default(),
        &crate::run_context::CancellationToken::default(), &mut turns, &mut spent, OuterConfig::default(), true).unwrap_err();
    assert_eq!(error.error_code, "TEACHING_REVIEW_BUDGET_EXHAUSTED");
    assert_eq!(adapter.seen_reviews.borrow().len(), 1);
    assert!(adapter.seen_explanation_reviews.borrow().is_empty());
    assert_eq!(turns, 2);
    assert_eq!(spent, 100_000);
    assert!(port.reviews.is_empty() && port.accepted.is_empty() && port.selections.is_empty());
}
