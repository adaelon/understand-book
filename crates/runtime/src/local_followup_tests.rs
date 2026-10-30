#[test]
fn local_followup_finish_delivers_exact_body_without_review_or_copy_sampling() {
    let b = book();
    let mut store = MemoryStore::open(tmp("local-followup-finish")).unwrap();
    let mut reader = Reader::new(&b, DEFAULT_RADIUS);
    let snapshot = default_profile_snapshot(&b, &store, "t0");
    let mut selection = selected_body("local", "The catalog retains B even when this route selects only A.");
    selection["finish"] = serde_json::json!(true);
    let adapter = ReviewedTeachingAdapter::new(vec![
        turn_calls(vec![call("finish", "tutor.step", &selection.to_string())]),
    ], vec![]);
    let sink = std::sync::Arc::new(TeachingStreamSink::default());
    let mut context = RunContext::new(new_session(), OuterConfig::default(), adapter.model_runtime_profile());
    context.events = crate::run_events::RunEvents::new(Some(sink.clone()));
    context.tutor = Some(serde_json::json!({"status":"active", "current_design":{"design_ref":"design:current","design":design()},
        "learner_context":{"current_request":"B is still available, right?"}}));
    let mut port = TeachingPort {inner:BorrowedResidentState {store:&mut store,reader:&mut reader},
        accepted:vec![], selections:Default::default(), reviews:vec![]};
    let outcome = run_context(&b, &mut port, &adapter, &mut context, &snapshot, &ResidentTurnResources::default(), None,
        &mut EphemeralCompactionCheckpointSink::default(), "B is still available, right?", "t0").unwrap();
    assert!(!outcome.incomplete);
    assert_eq!(outcome.answer.as_deref(), selection["move"]["delivery_text"].as_str());
    assert_eq!(adapter.inner.0.seen_plans.borrow().len(), 1, "accepted text needs no copy-only sample");
    assert_eq!(outcome.turns, 1, "one author completes the exact selected body");
    assert_eq!(outcome.tokens_spent, 10);
    assert_eq!(outcome.teaching_delivery_move_refs, ["selected:local"]);
    assert!(port.reviews.is_empty());
    assert!(adapter.seen_reviews.borrow().is_empty() && adapter.seen_explanation_reviews.borrow().is_empty());
    assert!(!sink.0.lock().unwrap().is_empty(), "selected delivery must reach the stream");
    assert_eq!(context.messages.last().unwrap().content, outcome.answer);
}

#[test]
fn local_followup_saved_target_authorizes_read_but_never_reuses_old_evidence() {
    let b = book_leaves(3);
    let target = serde_json::json!({"source_bindings":[{"source_id":b.base.book_id,
        "source_revision":b.source_fingerprint(),"start_lid":"1.2","end_lid":"1.3"}]});
    let mut ledger = TurnLocatorLedger::from_turn(&b, "Explain the previous relationship.", &[], "1.1");
    assert!(authorize_book_text(r#"{"lid":"1.2","end_lid":"1.3"}"#, &ledger).is_err());
    ledger.observe_teaching_targets(&serde_json::json!({"current_design":{"design":{"target":target}}}), &b);
    assert!(authorize_book_text(r#"{"lid":"1.2","end_lid":"1.3"}"#, &ledger).is_ok());
    assert!(ledger.origins("1.2").unwrap().contains(&LocatorOrigin::TeachingTarget));
    assert!(TurnEvidenceLedger::default().teaching_source_ranges().is_empty());
    let mut stale = target.clone();
    stale["source_bindings"][0]["source_revision"] = serde_json::json!("different-version");
    let mut stale_ledger = TurnLocatorLedger::from_turn(&b, "Explain it.", &[], "1.1");
    stale_ledger.observe_teaching_targets(&serde_json::json!({"current_design":{"design":{"target":stale}}}), &b);
    assert!(authorize_book_text(r#"{"lid":"1.2","end_lid":"1.3"}"#, &stale_ledger).is_err());
}

#[test]
fn local_followup_finish_preserves_unfinished_requested_reader_operation() {
    let b = book();
    let mut store = MemoryStore::open(tmp("local-followup-unfinished-operation")).unwrap();
    let mut reader = Reader::new(&b, DEFAULT_RADIUS);
    let snapshot = default_profile_snapshot(&b, &store, "t0");
    let mut selection = selected_body("local-operation", "The retained catalog still contains B.");
    selection["finish"] = serde_json::json!(true);
    let adapter = ReviewedTeachingAdapter::new(vec![
        turn_calls(vec![call("select", "tutor.step", &selection.to_string())]),
        turn_calls(vec![call("read", "book.text", r#"{"lid":"1.1"}"#)]),
        turn_final("Use the selected body."),
        turn_final("Use the selected body."),
    ], vec![]);
    let mut context = RunContext::new(new_session(), OuterConfig { max_turns:Some(5), ..Default::default() }, adapter.model_runtime_profile());
    context.goal = Some(crate::goal::ResidentGoal::new("reader-goal".into(), "t1".into(), "解释后跳转到原文".into()));
    context.goal.as_mut().unwrap().requirements = vec![crate::goal::GoalRequirement {
        id:"goto".into(), description:"Jump to the original passage".into(), basis_turn_id:"t1".into(),
        verification:crate::goal::GoalVerification::ReaderAction }];
    context.tutor = Some(serde_json::json!({"status":"active","current_design":{"design_ref":"design:current","design":design()},"learner_context":{}}));
    let mut port = TeachingPort {inner:BorrowedResidentState {store:&mut store,reader:&mut reader},
        accepted:vec![], selections:Default::default(), reviews:vec![]};
    let outcome = run_context(&b, &mut port, &adapter, &mut context, &snapshot, &ResidentTurnResources::default(), None,
        &mut EphemeralCompactionCheckpointSink::default(), "Explain and then jump to the original passage.", "t0").unwrap();
    assert!(adapter.inner.0.seen_plans.borrow().len() > 1, "a declaration cannot bypass an unperformed operation");
    assert!(outcome.incomplete);
    assert!(outcome.teaching_delivery_move_refs.is_empty());
}
