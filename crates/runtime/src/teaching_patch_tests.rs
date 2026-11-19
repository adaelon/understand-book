// A failed host preflight retains the exact draft for a source repair or local
// edit. There is no synchronous content reviewer in this path.
#[test]
fn allocation_patch_reuses_long_body_sources_finish_and_cached_prefix() {
    let b = book();
    let mut store = MemoryStore::open(tmp("allocation-patch")).unwrap();
    let mut reader = Reader::new(&b, DEFAULT_RADIUS);
    let snapshot = default_profile_snapshot(&b, &store, "t0");
    let answer = "The catalog retains the card while a route selects which cards it uses. ".repeat(40);
    let mut initial = selected_body("retained", &answer);
    initial["finish"] = serde_json::json!(true);
    initial["move"]["interaction_intent"] = serde_json::json!("direct_explanation");
    initial["move"]["participation"] = serde_json::json!({"choice":"invite","reason":"Ask for an application",
        "support":"The catalog retains cards.","learner_inference":"Apply this distinction to another route."});
    initial["move"]["response_expectation"] = serde_json::json!({"public_prompt":"What does another route keep?", "block_refs":["relation"]});
    let still_invalid = serde_json::json!({"operation":"revise","draft_ref":"teaching-draft:1","edits":[],
        "move_changes":{"participation":{"choice":"invite","reason":"Still proposing an application",
            "support":"The catalog retains cards.","learner_inference":"Apply this distinction to another route."}}});
    let revision = serde_json::json!({"operation":"revise","draft_ref":"teaching-draft:2","edits":[],
        "move_changes":{"participation":{"choice":"conclude","reason":"The requested direct explanation is complete."}, "response_expectation":null}});
    let adapter = ReviewedTeachingAdapter::new(vec![
        turn_calls(vec![call("initial", "tutor.step", &initial.to_string())]),
        turn_calls(vec![call("still-invalid", "tutor.step", &still_invalid.to_string())]),
        turn_calls(vec![call("revised", "tutor.step", &revision.to_string())]),
    ], vec![]);
    let mut context = RunContext::new(new_session(), OuterConfig::default(), adapter.model_runtime_profile());
    context.tutor = Some(serde_json::json!({"status":"practice",
        "current_design":{"design_ref":"design:current","design":design()},"learner_context":{}}));
    let mut port = ArgumentRecoveryTeachingPort { inner: TeachingPort {
        inner:BorrowedResidentState {store:&mut store,reader:&mut reader}, accepted:vec![],selections:Default::default(),reviews:vec![],
    }};
    let outcome = run_context(&b,&mut port,&adapter,&mut context,&snapshot,&ResidentTurnResources::default(),None,
        &mut EphemeralCompactionCheckpointSink::default(),"Explain the catalog directly.","t0").unwrap();
    assert!(!outcome.incomplete);
    assert_eq!(outcome.answer.as_deref(), Some(answer.as_str()));
    assert_eq!(outcome.turns, 3);
    assert!(tool_result(&context.messages, "still-invalid").contains("TEACHING_INVALID"),
        "metadata-only revisions must still pass the complete host preflight");
    assert_eq!(port.inner.accepted, ["select"]);
    let selected = &port.inner.selections["selected:retained"];
    assert_eq!(selected["participation"]["choice"], "conclude");
    assert!(selected.get("response_expectation").is_none_or(|value| value.is_null()));
    assert_eq!(selected["target"], initial["move"]["target"]);
    assert_eq!(selected["block_refs"], initial["move"]["block_refs"]);
    assert_repair_request_prefixes(&adapter.inner.0.seen_plans.borrow());
    let public = crate::tutor::public_arguments(&revision.to_string());
    assert!(!public.contains("move_changes") && !public.contains("requested direct explanation"));
    println!("allocation repair: complete_select_bytes={}, revision_bytes={}, original_body_bytes={}",
        initial.to_string().len(),revision.to_string().len(),answer.len());
}

#[test]
fn teaching_patch_after_source_repair_preserves_long_body_participation_and_finish() {
    for edit_body in [false, true] {
        let b = book();
        let mut store = MemoryStore::open(tmp(&format!("host-patch-{edit_body}"))).unwrap();
        let mut reader = Reader::new(&b, DEFAULT_RADIUS);
        let snapshot = default_profile_snapshot(&b, &store, "t0");
        let prefix = "The catalog keeps the same card for a later route. ".repeat(35);
        let original = format!("{prefix}\n\nThe route omits B.\n\nThe retained card remains reusable.");
        let replacement = "The current route omits B while another route can select the same retained B.";
        let mut selection = selected_body("patched", &original);
        selection["finish"] = serde_json::json!(true);
        let edits = if edit_body { serde_json::json!([
            {"field":"delivery_text","paragraph":2,"new_text":replacement}
        ]) } else { serde_json::json!([]) };
        let revision = serde_json::json!({"operation":"revise","draft_ref":"teaching-draft:1","edits":edits});
        let adapter = ReviewedTeachingAdapter::new(vec![
            turn_calls(vec![call("unread", "tutor.step", &selection.to_string())]),
            turn_calls(vec![call("source", "book.text", r#"{"lid":"1.1"}"#)]),
            turn_calls(vec![call("repaired", "tutor.step", &revision.to_string())]),
        ], vec![]);
        let mut context = RunContext::new(new_session(), OuterConfig::default(), adapter.model_runtime_profile());
        context.tutor = Some(serde_json::json!({"status":"practice",
            "current_design":{"design_ref":"design:current","design":design()},"learner_context":{}}));
        let mut port = SourcePreflightTeachingPort {
            inner:TeachingPort {inner:BorrowedResidentState {store:&mut store,reader:&mut reader},
                accepted:vec![],selections:Default::default(),reviews:vec![]},
            observed_reads:RefCell::new(vec![]),
            required:read_tools::EvidenceRange {start_lid:"1.1".into(),end_lid:"1.1".into(),ranges:vec![]},
        };
        let outcome = run_context(&b,&mut port,&adapter,&mut context,&snapshot,&ResidentTurnResources::default(),None,
            &mut EphemeralCompactionCheckpointSink::default(),"Explain how the same retained card can be reused.","t0").unwrap();
        let expected = if edit_body { original.replace("The route omits B.",replacement) } else { original.clone() };
        assert!(!outcome.incomplete);
        assert_eq!(outcome.answer.as_deref(),Some(expected.as_str()));
        assert_eq!(outcome.turns,3,"source recovery and patch must not add review or copy-only calls");
        assert_eq!(port.inner.accepted,["select"]);
        assert_eq!(port.inner.selections["selected:patched"]["participation"],selection["move"]["participation"]);
        assert!(port.inner.reviews.is_empty() && adapter.seen_reviews.borrow().is_empty());
        assert!(adapter.seen_explanation_reviews.borrow().is_empty());
        let plans=adapter.inner.0.seen_plans.borrow();
        let repair_context=serde_json::to_string(&plans[1].ordered_messages()).unwrap();
        assert!(repair_context.contains("teaching-draft:1") && repair_context.contains("editable_paragraphs"));
        assert!(!repair_context.contains("review_feedback"));
    }
}
