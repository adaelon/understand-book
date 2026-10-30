// A failed host preflight retains the exact draft for a source repair or local
// edit. There is no synchronous content reviewer in this path.
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
        context.tutor = Some(serde_json::json!({"status":"active",
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
