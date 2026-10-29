#[test]
fn local_followup_finish_delivers_reviewed_body_without_copy_sampling() {
    let b = book();
    let mut store = MemoryStore::open(tmp("local-followup-finish")).unwrap();
    let mut reader = Reader::new(&b, DEFAULT_RADIUS);
    let snapshot = default_profile_snapshot(&b, &store, "t0");
    let mut selection = selected_body("local", "The catalog retains B even when this route selects only A.");
    selection["finish"] = serde_json::json!(true);
    let adapter = ReviewedTeachingAdapter::new(vec![
        turn_calls(vec![call("finish", "tutor.step", &selection.to_string())]),
    ], vec![accepted_review()]);
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
    assert_eq!(outcome.turns, 3, "one author and two independent reviewers");
    assert_eq!(outcome.tokens_spent, 10 + 2 * 171);
    assert_eq!(outcome.teaching_delivery_move_refs, ["selected:local"]);
    assert_eq!(port.reviews.len(), 1);
    assert_eq!(adapter.seen_explanation_reviews.borrow().len(), 1);
    assert_eq!(port.reviews[0]["independent_reviews"]["factual"]["review"]["decision"], "accept");
    assert_eq!(port.reviews[0]["independent_reviews"]["explanation"]["review"]["decision"], "accept");
    assert!(!sink.0.lock().unwrap().is_empty(), "reviewed delivery must reach the stream");
    assert_eq!(context.messages.last().unwrap().content, outcome.answer);
}

#[test]
fn local_followup_review_repairs_its_quote_without_another_author_turn() {
    let b = book();
    let mut store = MemoryStore::open(tmp("local-followup-review-format")).unwrap();
    let mut reader = Reader::new(&b, DEFAULT_RADIUS);
    let snapshot = default_profile_snapshot(&b, &store, "t0");
    let question = "Does the omitted card remain?";
    let mut selection = selected_body("local-format", "The omitted card remains in the catalog.");
    selection["finish"] = serde_json::json!(true);
    let bad_quote = serde_json::json!({"decision":"accept","revision":null,"learner_claims":[],
        "request_resolution":{"status":"answered","question_quote":question,"answer_quotes":["An invented paraphrase."],
            "reason":"The catalog retains the card."}});
    let adapter = ReviewedTeachingAdapter::new(vec![
        turn_calls(vec![call("finish", "tutor.step", &selection.to_string())]),
    ], vec![Ok(bad_quote), accepted_review()]);
    let mut context = RunContext::new(new_session(), OuterConfig::default(), adapter.model_runtime_profile());
    context.tutor = Some(serde_json::json!({"status":"active","current_design":{"design_ref":"design:current","design":design()},"learner_context":{}}));
    let mut port = TeachingPort {inner:BorrowedResidentState {store:&mut store,reader:&mut reader},
        accepted:vec![], selections:Default::default(), reviews:vec![]};
    let outcome = run_context(&b, &mut port, &adapter, &mut context, &snapshot, &ResidentTurnResources::default(), None,
        &mut EphemeralCompactionCheckpointSink::default(), question, "t0").unwrap();
    assert!(!outcome.incomplete);
    assert_eq!(adapter.inner.0.seen_plans.borrow().len(), 1);
    assert_eq!(outcome.turns, 4);
    assert_eq!(outcome.tokens_spent, 10 + 3 * 171, "repair and both review usages must be charged exactly once");
    let packets = adapter.seen_reviews.borrow();
    assert_eq!(packets.len(), 2);
    assert_eq!(packets[0]["proposed_delivery"], packets[1]["proposed_delivery"]);
    assert!(packets[1]["review_format_repair"]["validation_error"].as_str().unwrap().contains("answer_quotes"));
    assert_eq!(port.reviews.len(), 1, "invalid verdict must not be saved as a content review");
    assert!(port.reviews[0]["review_format_repair"].is_object());
    let requests = adapter.seen_review_requests.borrow();
    assert_eq!(requests[1].user, requests[0].user);
    assert!(requests[1].followup_user.as_deref().unwrap().starts_with("review_format_repair:\n"));
    assert_eq!(requests[0].system, requests[1].system);
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
    ], vec![accepted_review()]);
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

#[test]
fn local_followup_independent_reviews_both_gate_delivery_and_keep_factual_work() {
    for (factual_rejects, explanation_rejects) in [(false, true), (true, false), (true, true)] {
        let b = book();
        let mut store = MemoryStore::open(tmp(&format!("independent-review-{factual_rejects}-{explanation_rejects}"))).unwrap();
        let mut reader = Reader::new(&b, DEFAULT_RADIUS);
        let snapshot = default_profile_snapshot(&b, &store, "t0");
        let mut first = selected_body("first", "UNACCEPTED_TEXT_WITH_A_MISSING_CONNECTION");
        first["finish"] = serde_json::json!(true);
        let mut repaired = selected_body("repaired", "B remains in the catalog, so a later route can select B even when this route selects only A.");
        repaired["finish"] = serde_json::json!(true);
        let mut factual_repair = selected_body("factual-repair", "B remains in the catalog.");
        factual_repair["finish"] = serde_json::json!(true);
        let work = serde_json::json!({"understanding":[{"statement":"The observed passage contains X.",
            "sources":[{"source_id":b.base.book_id,"source_revision":b.source_fingerprint(),"lid":"1.1","quote":"XXXXXXXX"}]}],
            "open_questions":["Explain how a later route can select the retained item."]});
        let mut factual = if factual_rejects {
            serde_json::json!({"decision":"revise","revision":"FACTUAL_GAP: distinguish the stored catalog from this route.","learner_claims":[]})
        } else { accepted_review().unwrap() };
        factual["agent_work"] = work.clone();
        let explanation = if explanation_rejects {
            serde_json::json!({"decision":"revise","revision":"EXPLANATION_GAP: follow B from the catalog into a later route.","learner_claims":[]})
        } else { accepted_review().unwrap() };
        let mut author_turns = vec![
            turn_calls(vec![call("source", "book.text", r#"{"lid":"1.1"}"#)]),
            turn_calls(vec![call("first", "tutor.step", &first.to_string())]),
        ];
        let mut factual_reviews = vec![Ok(factual.clone()), accepted_review()];
        if factual_rejects && explanation_rejects {
            author_turns.push(turn_calls(vec![call("factual-repair", "tutor.step", &factual_repair.to_string())]));
            factual_reviews.push(accepted_review());
        }
        author_turns.push(turn_calls(vec![call("repaired", "tutor.step", &repaired.to_string())]));
        let adapter = ReviewedTeachingAdapter::new(author_turns, factual_reviews);
        if explanation_rejects { adapter.explanation_reviews.borrow_mut().push_back(Ok(explanation)); }
        adapter.explanation_reviews.borrow_mut().push_back(accepted_review());
        let mut context = RunContext::new(new_session(), OuterConfig {max_turns:Some(12), ..Default::default()}, adapter.model_runtime_profile());
        context.tool_exposure_state.activate("book.text");
        context.tutor = Some(serde_json::json!({"status":"active","current_design":{"design_ref":"design:current","design":design()},"learner_context":{}}));
        let mut port = TeachingPort {inner:BorrowedResidentState {store:&mut store,reader:&mut reader},
            accepted:vec![], selections:Default::default(), reviews:vec![]};
        let outcome = run_context(&b, &mut port, &adapter, &mut context, &snapshot, &ResidentTurnResources::default(), None,
            &mut EphemeralCompactionCheckpointSink::default(), "Why can another route still select B?", "t0").unwrap();
        assert!(!outcome.incomplete);
        assert_eq!(outcome.answer.as_deref(), repaired["move"]["delivery_text"].as_str());
        assert_eq!(port.accepted, ["select"]);
        assert!(!port.selections.contains_key("selected:first"));
        assert!(!port.selections.contains_key("selected:factual-repair"));
        let factual_count = 2 + usize::from(factual_rejects && explanation_rejects);
        let explanation_count = 1 + usize::from(explanation_rejects);
        let author_count = factual_count + 1;
        assert_eq!(adapter.seen_reviews.borrow().len(), factual_count);
        assert_eq!(adapter.seen_explanation_reviews.borrow().len(), explanation_count,
            "only factually accepted drafts receive an independent explanation judgment");
        assert_eq!(outcome.turns, author_count + factual_count + explanation_count);
        assert_eq!(outcome.tokens_spent, author_count as u32 * 10 + (factual_count + explanation_count) as u32 * 171);
        assert_eq!(port.reviews.len(), factual_count);
        let rejected = &port.reviews[0];
        assert_eq!(rejected["review"]["decision"], "revise");
        assert_eq!(rejected["review"]["agent_work"], work,
            "the isolated explanation judgment must not erase the source-grounded factual work");
        assert_eq!(rejected["independent_reviews"]["factual"]["raw"]["agent_work"], work);
        assert_eq!(rejected["independent_reviews"]["factual"]["review"]["decision"],
            if factual_rejects {"revise"} else {"accept"});
        if factual_rejects {
            assert!(rejected["review"].to_string().contains("FACTUAL_GAP"));
            assert_eq!(rejected["independent_reviews"]["explanation"],
                serde_json::json!({"status":"skipped","reason":"factual_revision_required"}));
            assert!(!rejected["review"].to_string().contains("EXPLANATION_GAP"),
                "a skipped reviewer must contribute neither a verdict nor a fabricated finding");
        }
        if explanation_rejects {
            let explanation_rejection = &port.reviews[usize::from(factual_rejects)];
            assert_eq!(explanation_rejection["independent_reviews"]["factual"]["review"]["decision"], "accept");
            assert_eq!(explanation_rejection["independent_reviews"]["explanation"]["review"]["decision"], "revise");
            assert_eq!(explanation_rejection["review"]["decision"], "revise");
            assert!(explanation_rejection["review"].to_string().contains("EXPLANATION_GAP"));
        }
        let accepted = port.reviews.last().unwrap();
        assert_eq!(accepted["independent_reviews"]["factual"]["review"]["decision"], "accept");
        assert_eq!(accepted["independent_reviews"]["explanation"]["review"]["decision"], "accept");
        let purpose = adapter.seen_purposes.borrow();
        assert_eq!(purpose.iter().filter(|p|p.as_str()=="tutor_delivery_review").count(), factual_count);
        assert_eq!(purpose.iter().filter(|p|p.as_str()=="tutor_explanation_review").count(), explanation_count);
        let explanation_input = &adapter.seen_explanation_reviews.borrow()[0];
        assert_eq!(explanation_input["current_request"], "Why can another route still select B?");
        let first_explained = if factual_rejects {
            if explanation_rejects { &factual_repair } else { &repaired }
        } else { &first };
        assert_eq!(explanation_input["current_delivery"], first_explained["move"]["delivery_text"]);
        assert!(explanation_input.get("current_source_evidence").is_none());
        assert!(explanation_input.get("current_design").is_none());
        assert!(explanation_input.get("agent_working_state").is_none());
        assert!(!explanation_input.to_string().contains("FACTUAL_GAP"),
            "the factual verdict must not steer the isolated explanation judgment");
    }
}

#[test]
fn local_followup_explanation_quote_repair_keeps_the_same_draft_and_one_factual_review() {
    let b = book();
    let mut store = MemoryStore::open(tmp("independent-explanation-format")).unwrap();
    let mut reader = Reader::new(&b, DEFAULT_RADIUS);
    let snapshot = default_profile_snapshot(&b, &store, "t0");
    let question = "Does B remain available?";
    let mut selection = selected_body("explanation-format", "B remains in the catalog, so another route can still select B.");
    selection["finish"] = serde_json::json!(true);
    let adapter = ReviewedTeachingAdapter::new(vec![
        turn_calls(vec![call("select", "tutor.step", &selection.to_string())]),
    ], vec![accepted_review()]);
    adapter.explanation_reviews.borrow_mut().extend([
        Ok(serde_json::json!({"decision":"accept","revision":null,"learner_claims":[],
            "request_resolution":{"status":"answered","question_quote":question,
                "answer_quotes":["This quote is not in the current answer."],"reason":"The fixture relation is supplied."}})),
        accepted_review(),
    ]);
    let mut context = RunContext::new(new_session(), OuterConfig::default(), adapter.model_runtime_profile());
    context.tutor = Some(serde_json::json!({"status":"active","current_design":{"design_ref":"design:current","design":design()},"learner_context":{}}));
    let mut port = TeachingPort {inner:BorrowedResidentState {store:&mut store,reader:&mut reader},
        accepted:vec![], selections:Default::default(), reviews:vec![]};
    let outcome = run_context(&b, &mut port, &adapter, &mut context, &snapshot, &ResidentTurnResources::default(), None,
        &mut EphemeralCompactionCheckpointSink::default(), question, "t0").unwrap();
    assert!(!outcome.incomplete);
    assert_eq!(outcome.answer.as_deref(), selection["move"]["delivery_text"].as_str());
    assert_eq!(adapter.inner.0.seen_plans.borrow().len(), 1);
    assert_eq!(adapter.seen_reviews.borrow().len(), 1);
    let explanations = adapter.seen_explanation_reviews.borrow();
    assert_eq!(explanations.len(), 2);
    assert_eq!(explanations[0]["current_delivery"], explanations[1]["current_delivery"]);
    assert!(explanations[1]["review_format_repair"]["validation_error"].as_str().unwrap().contains("answer_quotes"));
    assert_eq!(outcome.turns, 4);
    assert_eq!(outcome.tokens_spent, 10 + 3 * 171);
    assert_eq!(port.reviews.len(), 1);
    assert!(port.reviews[0]["independent_reviews"]["explanation"]["format_repair"].is_object());
    let requests = adapter.seen_review_requests.borrow();
    assert_eq!(requests[2].user, requests[1].user);
    assert!(requests[2].followup_user.as_deref().unwrap().starts_with("review_format_repair:\n"));
    assert_eq!(requests[1].system, requests[2].system);
}
