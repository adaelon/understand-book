use super::*;
use memory::teaching::TeachingFact;
use runtime::presentation::*;

fn fixture() -> (tempfile::TempDir, AppState) {
    let root = tempfile::tempdir().unwrap();
    let mut state = state_named(&format!(
        "tutor-loop-{}",
        root.path().file_name().unwrap().to_string_lossy()
    ));
    let source = "Average speed is total distance divided by total time. Equal distances need not take equal times. The total time must be positive. ".repeat(3);
    state.workspace.book = Arc::new(Book::new(sample_base(), &source));
    state.workspace.reader = Reader::new(&state.workspace.book, DEFAULT_RADIUS);
    state.user.store = MemoryStore::open(root.path().join("memory.json")).unwrap();
    state.user.history_path = Some(root.path().join("history.json"));
    state.workspace.book_dir = root.path().to_path_buf();
    let map = json!({"source_id":state.workspace.book.base.book_id,"source_revision":state.workspace.book.source_fingerprint(),
        "objects":{"active_refs":[{"source_id":state.workspace.book.base.book_id,"object_id":"speed"}],
            "objects":[{"ref":{"source_id":state.workspace.book.base.book_id,"object_id":"speed"},"meaning":"average speed","object_revision":1,
                "source_bindings":[{"source_id":state.workspace.book.base.book_id,"source_revision":state.workspace.book.source_fingerprint(),"lid":"1.1"}]}]},
        "cognitive_materials":{"materials":[{"object_refs":[{"source_id":state.workspace.book.base.book_id,"object_id":"speed"}],"purpose":"Explain average speed using total distance and total time.","source_bindings":[{"lid":"1.1"}]}]}});
    std::fs::create_dir_all(root.path().join("teaching/versions/v1")).unwrap();
    std::fs::write(
        root.path().join("teaching/versions/v1/map.json"),
        map.to_string(),
    )
    .unwrap();
    std::fs::write(root.path().join("teaching_readiness.json"), json!({"version":"teaching_readiness.v1","status":"ready","source_id":state.workspace.book.base.book_id,
        "source_revision":state.workspace.book.source_fingerprint(),"teaching_map_revision":"v1","map_path":"teaching/versions/v1/map.json",
        "coverage":{"source":"complete","structure":"complete","objects":"complete","cognitive_materials":"complete","source_review":"passed"},"limitations":[]}).to_string()).unwrap();
    assert_eq!(post(&mut state,"/tutor/mutate",r#"{"operation_id":"on","expected_revision":0,"action":{"kind":"set_enabled","enabled":true}}"#).status,200);
    assert!(teaching::start_request(&state.private_context(), "now").unwrap()["started"] == true);
    (root, state)
}
fn prepare(state: &mut AppState) -> agent_run::PreparedAgentChat {
    prepare_agent_chat(state, r#"{"message":"Explain average speed"}"#, "now")
        .unwrap_or_else(|r| panic!("{}", r.body))
}
fn range() -> EvidenceRange {
    EvidenceRange {
        start_lid: "1.1".into(),
        end_lid: "1.1".into(),
        ranges: vec![],
    }
}
pub(super) fn select(state: &AppState, turn: &AgentTurnRef, id: &str, reference: Option<&PresentationRef>) {
    teaching::step(&state.private_context(),
        turn,
        json!({"operation":"material","object_id":"speed"}),
        &[],
        &[],
    )
    .unwrap();
    teaching::step(&state.private_context(),turn,json!({"operation":"select","move":{"move_id":id,"object_ids":["speed"],"capability":"explanation","kind":"observe","interaction_intent":"neutral","intent_origin":"neutral","prompt":format!("Observe {id}"),
        "actions":["submit","revise","request_hint","reveal","skip","self_report"],"hint":"Compare the two times.","explanation":"Use total distance divided by total time.","presentation":reference}}),&[],&[range()]).unwrap();
}
pub(super) fn outcome(reference: Option<&PresentationRef>) -> OuterOutcome {
    let mut parts = vec![AgentAnswerPart::Markdown {
        text: "Compare total distance and total time.".into(),
    }];
    if let Some(r) = reference {
        parts.push(AgentAnswerPart::Presentation {
            presentation_id: r.presentation_id.clone(),
            revision: r.revision,
        });
    }
    OuterOutcome {
        answer: Some("Compare total distance and total time.".into()),
        answer_view: Some(AgentAnswerView {
            parts,
            sources: vec![],
        }),
        incomplete: false,
        warning: None,
        turns: 1,
        tokens_spent: 0,
        effects: vec![],
        trace: vec![],
        profile_usage: Default::default(),
        memory_updates: vec![],
        source_bindings: vec![],
        delivery_diagnostics: None,
        request_audit: Default::default(),
    }
}
pub(super) fn finish(state: &mut AppState, turn: &AgentTurnRef, result: &OuterOutcome) {
    finalize_agent_turn_completed(state, turn, result, &state.workspace.messages.clone(), "now").unwrap();
    teaching::record_delivery(&state.private_context(), turn, "now").unwrap();
}
fn json_post(state: &mut AppState, path: &str, body: Value) -> Value {
    let reply = post(state, path, &body.to_string());
    assert_eq!(reply.status, 200, "{}", reply.body);
    serde_json::from_str(&reply.body).unwrap()
}

fn assessed_activity(
    state: &mut AppState,
    rule: Value,
    feedback: &str,
) -> (AgentTurnRef, String, Value) {
    let run = prepare(state);
    let turn = run.turn_ref;
    teaching::step(&state.private_context(),
        &turn,
        json!({"operation":"material","object_id":"speed"}),
        &[],
        &[],
    )
    .unwrap();
    let proposal = json!({"operation":"select","move":{"move_id":"scored","object_ids":["speed"],"capability":"explanation","kind":"question","interaction_intent":"neutral","intent_origin":"neutral",
        "prompt":"Choose the necessary quantities: A total distance; B total time; C colour. Submit choice tokens as a JSON array.","actions":["submit","revise","request_hint","reveal","skip","self_report"],
        "hint":"Consider the denominator.","explanation":"Use total distance and total time; time must be positive.",
        "assessment":{"rule":rule,"source_lids":["1.1"],"source_sufficient":true,"feedback":feedback}}});
    teaching::step(&state.private_context(), &turn, proposal.clone(), &[], &[range()]).unwrap();
    finish(state, &turn, &outcome(None));
    let id = format!("delivery:{}:scored", turn.turn_id);
    json_post(state, "/tutor/display", json!({"delivery_ref":id}));
    (turn, id, proposal)
}

#[test]
fn tutor_frozen_assessment_uses_original_key_and_retains_attempt_conditions() {
    let (_root, mut state) = fixture();
    let (turn, id, mut proposal) = assessed_activity(
        &mut state,
        json!({"kind":"choice_set","choices":["A","B","C"],"correct":["A","B"]}),
        "on_reveal",
    );
    proposal["move"]["assessment"]["rule"]["correct"] = json!(["C"]);
    assert!(teaching::step(&state.private_context(), &turn, proposal, &[], &[range()]).is_err());
    let public = json_post(
        &mut state,
        "/tutor/activities",
        json!({"session_id":turn.session_id,"turn_id":turn.turn_id}),
    );
    assert!(!public.to_string().contains("choice_set"));
    assert!(!public.to_string().contains("scoring_sources"));
    let first =
        json!({"operation_id":"grade-first","delivery_ref":id,"action":"submit","response":"A"});
    let result = json_post(&mut state, "/tutor/action", first.clone());
    assert_eq!(result["assessment"], "partial");
    assert_eq!(
        json_post(&mut state, "/tutor/action", first.clone()),
        result
    );
    let help = json_post(
        &mut state,
        "/tutor/action",
        json!({"operation_id":"grade-hint","delivery_ref":id,"action":"request_hint"}),
    );
    json_post(
        &mut state,
        "/tutor/help-displayed",
        json!({"help_ref":help["help"]["event_id"]}),
    );
    assert_eq!(json_post(&mut state, "/tutor/action", first), result);
    for (operation, response, expected) in [
        ("wrong", "C", "incorrect"),
        ("right", r#"[" b ","a"]"#, "correct"),
        ("technical", "[broken", "unassessed"),
    ] {
        let r = json_post(
            &mut state,
            "/tutor/action",
            json!({"operation_id":operation,"delivery_ref":id,"action":"revise","response":response}),
        );
        assert_eq!(r["assessment"], expected);
    }
    let facts = state
        .user.learning_store()
        .unwrap()
        .teaching_activity_events(&id)
        .unwrap();
    let attempts: Vec<_> = facts
        .iter()
        .filter(|e| matches!(e.payload["action"].as_str(), Some("submit" | "revise")))
        .collect();
    assert_eq!(attempts.len(), 4);
    assert_eq!(attempts[0].payload["assistance_refs"], json!([]));
    assert_eq!(
        attempts[2].payload["assistance_refs"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(attempts[2].payload["attempt"], 3);
    let skipped = json_post(
        &mut state,
        "/tutor/action",
        json!({"operation_id":"skip","delivery_ref":id,"action":"skip"}),
    );
    assert_eq!(skipped["assessment"], "unassessed");
    assert!(state
        .user.learning_store()
        .unwrap()
        .assessment("action:technical")
        .unwrap()
        .is_none());
}

fn open_rule() -> Value {
    json!({"kind":"open","items":[{"id":"distance","criterion":"Numerator is total distance; accept equivalent wording","source_lids":["1.1"]},{"id":"time","criterion":"Denominator is total time; accept equivalent wording","source_lids":["1.1"]}]})
}
fn open_items() -> Value {
    json!([
        {"id":"distance","verdict":"supported","response_quote":"all distance","sources":[{"lid":"1.1","quote":"total distance"}],"reason":"The response names the total distance."},
        {"id":"time","verdict":"supported","response_quote":"all time","sources":[{"lid":"1.1","quote":"total time"}],"reason":"The response names total elapsed time."}
    ])
}

#[test]
fn tutor_open_assessment_is_isolated_budgeted_and_contract_checked() {
    let (_root, mut state) = fixture();
    let (_, id, _) = assessed_activity(&mut state, open_rule(), "on_reveal");
    let action = json_post(
        &mut state,
        "/tutor/action",
        json!({"operation_id":"open","delivery_ref":id,"action":"submit","response":"all distance over all time"}),
    );
    assert_eq!(action["assessment"], "unassessed");
    state.services.adapter = Box::new(ChatStubAdapter::scripted(vec![
        call(
            "tutor.step",
            json!({"operation":"assess","action_ref":action["event_id"]}),
        ),
        answer(&json!({"items":open_items()}).to_string()),
        call("tutor.step", json!({"operation":"outside"})),
        answer("Your response meets the criteria."),
    ]));
    let result = json_post(
        &mut state,
        "/agent/chat",
        json!({"message":"Please evaluate my response","teaching_ref":action["event_id"]}),
    );
    assert_eq!(result["tokens_spent"], 20);
    let assessment = state
        .user.learning_store()
        .unwrap()
        .assessment("action:open")
        .unwrap()
        .unwrap();
    assert_eq!(
        assessment.status,
        memory::assessment::AssessmentStatus::Correct
    );
    let run = prepare(&mut state);
    assert_eq!(
        teaching::assessment_input(&state.private_context(), &run.turn_ref, "action:open").unwrap()["existing"],
        "correct"
    );
    let revised = json_post(
        &mut state,
        "/tutor/action",
        json!({"operation_id":"open-revised","delivery_ref":id,"action":"revise","response":"all distance over all time"}),
    );
    assert!(teaching::assessment_accept(&state.private_context(),
        &run.turn_ref,
        revised["event_id"].as_str().unwrap(),
        json!([])
    )
    .is_err());
    let mut wrong = open_items();
    wrong[0]["sources"][0]["lid"] = json!("2.1");
    assert!(
        teaching::assessment_accept(&state.private_context(), &run.turn_ref, "action:open-revised", wrong).is_err()
    );
    assert!(state
        .user.learning_store()
        .unwrap()
        .assessment("action:open-revised")
        .unwrap()
        .is_none());
}

#[test]
fn tutor_understanding_correction_changes_next_context_without_changing_public_map() {
    let (root, mut state) = fixture();
    let map_path = root.path().join("teaching/versions/v1/map.json");
    let original = std::fs::read(&map_path).unwrap();
    let unknown = json_post(&mut state, "/tutor/understanding", json!({}));
    assert_eq!(unknown["rows"][0]["state"], "unknown");
    let (_, id, _) = assessed_activity(
        &mut state,
        json!({"kind":"choice_set","choices":["A","B","C"],"correct":["A","B"]}),
        "after_submit",
    );
    let action = json_post(
        &mut state,
        "/tutor/action",
        json!({"operation_id":"knowledge","delivery_ref":id,"action":"submit","response":r#"["A","B"]"#}),
    );
    let view = json_post(&mut state, "/tutor/understanding", json!({}));
    assert_eq!(view["rows"][0]["independent_support"], 1);
    assert_eq!(view["stale"], false);
    let evidence = view["rows"][0]["evidence_refs"][0].clone();
    let detail = json_post(
        &mut state,
        "/tutor/evidence",
        json!({"evidence_ref":evidence}),
    );
    assert_eq!(detail["learner_quote"], r#"["A","B"]"#);
    let corrected = json_post(
        &mut state,
        "/tutor/correct",
        json!({"operation_id":"correction-ui","evidence_ref":evidence,"text":"I copied an earlier worked example"}),
    );
    let new_view = json_post(&mut state, "/tutor/understanding", json!({}));
    assert_eq!(new_view["rows"][0]["state"], "unknown");
    assert_eq!(new_view["rows"][0]["independent_support"], 0);
    let next = prepare_agent_chat(
        &mut state,
        &json!({"message":"Directly explain this time","teaching_ref":action["event_id"]})
            .to_string(),
        "later",
    )
    .unwrap_or_else(|r| panic!("{}", r.body));
    let context = next.tutor.unwrap();
    assert_eq!(
        context["learner_context"]["entries"][0]["latest_evidence_ref"],
        corrected["evidence_ref"]
    );
    assert!(context["learner_context"]
        .to_string()
        .contains("copied an earlier worked example"));
    assert_eq!(std::fs::read(&map_path).unwrap(), original);
    let replay = json_post(&mut state, "/tutor/understanding", json!({"rebuild":true}));
    assert_eq!(replay, new_view);
}

#[test]
fn tutor_ungraded_discrimination_is_referenced_and_self_report_remains_subjective() {
    let (_root, mut state) = fixture();
    let run = prepare(&mut state);
    let turn = &run.turn_ref;
    teaching::step(&state.private_context(),
        turn,
        json!({"operation":"material","object_id":"speed"}),
        &[],
        &[],
    )
    .unwrap();
    teaching::step(&state.private_context(),turn,json!({"operation":"select","move":{"move_id":"discriminate","object_ids":["speed"],"capability":"evaluation","kind":"source_focus","interaction_intent":"neutral","intent_origin":"neutral","prompt":"Which quantities does this source support?","actions":["submit","revise","self_report"]}}),&[],&[range()]).unwrap();
    finish(&mut state, turn, &outcome(None));
    let id = format!("delivery:{}:discriminate", turn.turn_id);
    json_post(&mut state, "/tutor/display", json!({"delivery_ref":id}));
    assert_eq!(
        json_post(&mut state, "/tutor/understanding", json!({}))["rows"][0]["state"],
        "unknown"
    );
    json_post(
        &mut state,
        "/tutor/action",
        json!({"operation_id":"discrimination","delivery_ref":id,"action":"submit","response":"The source supports total distance; colour is not supported."}),
    );
    let accepted=teaching::step(&state.private_context(),turn,json!({"operation":"evidence","operation_id":"discrimination-evidence","action_ref":"action:discrimination","object_id":"speed","response_quote":"The source supports total distance","sources":[{"lid":"1.1","quote":"total distance"}],"interpretation":"The learner distinguished a source-supported quantity from an unsupported property."}),&[],&[]).unwrap();
    let detail = json_post(
        &mut state,
        "/tutor/evidence",
        json!({"evidence_ref":accepted["evidence_ref"]}),
    );
    assert_eq!(detail["status"], "uncertain");
    assert!(detail["interpretation"]
        .as_str()
        .unwrap()
        .contains("distinguished"));
    json_post(
        &mut state,
        "/tutor/action",
        json!({"operation_id":"self","delivery_ref":id,"action":"self_report","response":"I think I understand now"}),
    );
    let view = json_post(&mut state, "/tutor/understanding", json!({}));
    assert_eq!(view["rows"][0]["state"], "unknown");
    assert_eq!(view["rows"][0]["uncertain"], 2);
}

#[test]
fn tutor_scoring_keys_stay_out_of_history_and_public_trace() {
    let (_root, mut state) = fixture();
    state.services.adapter = Box::new(ChatStubAdapter::scripted(vec![
        call(
            "tutor.step",
            json!({"operation":"material","object_id":"speed"}),
        ),
        call("book.text", json!({"lid":"1.1","end_lid":"1.1"})),
        call(
            "tutor.step",
            json!({"operation":"select","move":{"move_id":"private","object_ids":["speed"],"capability":"explanation","kind":"question","interaction_intent":"neutral","intent_origin":"neutral","prompt":"Choose A or B","actions":["submit"],"hint":"PRIVATE_HINT","assessment":{"rule":{"kind":"choice_set","choices":["A","B"],"correct":["A"]},"source_lids":["1.1"],"source_sufficient":true,"feedback":"on_reveal"}}}),
        ),
        answer("Choose A or B"),
    ]));
    let result = json_post(
        &mut state,
        "/agent/chat",
        json!({"message":"Teach average speed"}),
    );
    for text in [
        result.to_string(),
        serde_json::to_string(&state.workspace.messages).unwrap(),
    ] {
        assert!(!text.contains("PRIVATE_HINT"));
        assert!(!text.contains("choice_set"));
    }
    let chat = state.user.agent_history.sessions.last().unwrap();
    let turn = chat.turns.last().unwrap();
    let candidates = state
        .user.learning_store()
        .unwrap()
        .teaching_turn_events("tutor-begin-1", &turn.turn_id)
        .unwrap();
    assert!(candidates
        .iter()
        .any(|e| e.kind == TeachingFact::MoveSelected
            && e.payload["move"]["assessment"]["rule"]["correct"] == json!(["A"])));
}

#[test]
fn tutor_malformed_evaluator_output_keeps_response_unassessed() {
    let (_root, mut state) = fixture();
    let (_, id, _) = assessed_activity(&mut state, open_rule(), "after_submit");
    json_post(
        &mut state,
        "/tutor/action",
        json!({"operation_id":"malformed","delivery_ref":id,"action":"submit","response":"all distance over all time"}),
    );
    state.services.adapter = Box::new(ChatStubAdapter::scripted(vec![
        call(
            "tutor.step",
            json!({"operation":"assess","action_ref":"action:malformed"}),
        ),
        answer("{unfinished"),
        call("tutor.step", json!({"operation":"outside"})),
        answer("The evaluation did not finish."),
    ]));
    json_post(
        &mut state,
        "/agent/chat",
        json!({"message":"Evaluate this response","teaching_ref":"action:malformed"}),
    );
    assert!(state
        .user.learning_store()
        .unwrap()
        .assessment("action:malformed")
        .unwrap()
        .is_none());
    assert!(state
        .user.learning_store()
        .unwrap()
        .evidence_page(&state.workspace.book.base.book_id, 0, 100)
        .unwrap()
        .is_empty());
}

#[test]
fn tutor_visible_rubric_feedback_is_help_for_the_next_attempt() {
    let (_root, mut state) = fixture();
    let (_, id, _) = assessed_activity(&mut state, open_rule(), "after_submit");
    json_post(
        &mut state,
        "/tutor/action",
        json!({"operation_id":"feedback-first","delivery_ref":id,"action":"submit","response":"all distance over all time"}),
    );
    let run = prepare(&mut state);
    teaching::assessment_accept(&state.private_context(), &run.turn_ref, "action:feedback-first", open_items())
        .unwrap();
    let before = state
        .user.learning_store()
        .unwrap()
        .teaching_activity_events(&id)
        .unwrap();
    assert!(!before.iter().any(|e| e.kind == TeachingFact::HelpDisplayed));
    json_post(
        &mut state,
        "/tutor/feedback-displayed",
        json!({"action_ref":"action:feedback-first"}),
    );
    json_post(
        &mut state,
        "/tutor/feedback-displayed",
        json!({"action_ref":"action:feedback-first"}),
    );
    json_post(
        &mut state,
        "/tutor/action",
        json!({"operation_id":"feedback-revise","delivery_ref":id,"action":"revise","response":"all distance over all time"}),
    );
    let action = state
        .user.learning_store()
        .unwrap()
        .teaching_event("action:feedback-revise")
        .unwrap();
    assert_eq!(
        action.payload["assistance_refs"].as_array().unwrap().len(),
        1
    );
}

#[test]
fn tutor_old_activity_and_object_revision_remain_bound_after_new_map() {
    let (root, mut state) = fixture();
    let (_, old_id, _) = assessed_activity(
        &mut state,
        json!({"kind":"choice_set","choices":["A","B","C"],"correct":["A","B"]}),
        "after_submit",
    );
    let mut map: Value = serde_json::from_slice(
        &std::fs::read(root.path().join("teaching/versions/v1/map.json")).unwrap(),
    )
    .unwrap();
    map["objects"]["objects"][0]["object_revision"] = json!(2);
    std::fs::create_dir_all(root.path().join("teaching/versions/v2")).unwrap();
    std::fs::write(
        root.path().join("teaching/versions/v2/map.json"),
        map.to_string(),
    )
    .unwrap();
    let ready_path = root.path().join("teaching_readiness.json");
    let mut ready: Value = serde_json::from_slice(&std::fs::read(&ready_path).unwrap()).unwrap();
    ready["teaching_map_revision"] = json!("v2");
    ready["map_path"] = json!("teaching/versions/v2/map.json");
    std::fs::write(ready_path, ready.to_string()).unwrap();
    let (_, new_id, _) = assessed_activity(
        &mut state,
        json!({"kind":"choice_set","choices":["A","B","C"],"correct":["C"]}),
        "after_submit",
    );
    assert_eq!(
        json_post(
            &mut state,
            "/tutor/action",
            json!({"operation_id":"old-version","delivery_ref":old_id,"action":"submit","response":r#"["A","B"]"#})
        )["assessment"],
        "correct"
    );
    assert_eq!(
        json_post(
            &mut state,
            "/tutor/action",
            json!({"operation_id":"new-version","delivery_ref":new_id,"action":"submit","response":r#"["A","B"]"#})
        )["assessment"],
        "incorrect"
    );
    let view = json_post(&mut state, "/tutor/understanding", json!({}));
    assert_eq!(view["rows"].as_array().unwrap().len(), 2);
    assert_eq!(view["rows"][0]["historical"], true);
    let next = prepare(&mut state).tutor.unwrap();
    let rows = next["learner_context"]["entries"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["object_revision"], 2);
    assert_eq!(rows[0]["independent_support"], 0);
}

#[test]
#[ignore = "configured provider; three independently specified source-grounded assessment cases"]
fn tutor_live_open_rubric_assessment() {
    use memory::assessment::*;
    let adapter = ProviderRegistry::adapter_from_config(ProviderConfig::from_env().unwrap());
    let contract:AssessmentContract=serde_json::from_value(json!({"rule":open_rule(),"source_lids":["1.1"],"source_sufficient":true,"feedback":"after_submit"})).unwrap();
    let sources = BTreeMap::from([(
        "1.1".to_string(),
        "Average speed is total distance divided by total time. Total time must be positive."
            .to_string(),
    )]);
    // Expected outcomes are established from source before any model output.
    for (response, expected) in [
        (
            "all distance over all elapsed time",
            AssessmentStatus::Correct,
        ),
        (
            "It uses total distance; I do not know what to divide by",
            AssessmentStatus::Partial,
        ),
        ("It is the colour of the road", AssessmentStatus::Incorrect),
    ] {
        let (result, usage) = runtime::tutor::evaluate(
            adapter.as_ref(),
            adapter.model_runtime_profile(),
            json!({"contract":contract,"response":response,"sources":sources}),
        )
        .unwrap();
        let parsed: Value = serde_json::from_str(result["text"].as_str().unwrap()).unwrap();
        let items: Vec<ItemAssessment> = serde_json::from_value(parsed["items"].clone()).unwrap();
        let status = accept_open(&contract, response, &sources, &items).unwrap();
        println!(
            "TUTOR_ASSESSMENT_LIVE {}",
            json!({"response":response,"status":status,"items":items,"usage":usage})
        );
        assert_eq!(status, expected);
    }
}
#[test]
fn tutor_candidate_failed_and_unmounted_deliveries_are_distinct() {
    let (_root, mut state) = fixture();
    let first = prepare(&mut state);
    select(&state, &first.turn_ref, "failed", None);
    assert!(state
        .user.learning_store()
        .unwrap()
        .teaching_deliveries(&first.turn_ref.session_id, &first.turn_ref.turn_id)
        .unwrap()
        .is_empty());
    finalize_agent_turn(
        &mut state,
        &first.turn_ref,
        AgentAssistantStatus::Failed,
        None,
        Some(AgentTurnError {
            error_code: "FAILED".into(),
            category: "provider".into(),
            message: "failed".into(),
        }),
        None,
        &[],
        "now",
    )
    .unwrap();
    teaching::record_delivery(&state.private_context(), &first.turn_ref, "now").unwrap();
    assert!(state
        .user.learning_store()
        .unwrap()
        .teaching_deliveries(&first.turn_ref.session_id, &first.turn_ref.turn_id)
        .unwrap()
        .is_empty());
    let second = prepare(&mut state);
    select(&state, &second.turn_ref, "text", None);
    finish(&mut state, &second.turn_ref, &outcome(None));
    let rows = state
        .user.learning_store()
        .unwrap()
        .teaching_deliveries(&second.turn_ref.session_id, &second.turn_ref.turn_id)
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert!(state
        .user.learning_store()
        .unwrap()
        .teaching_activity_events(&rows[0].event_id)
        .unwrap()
        .is_empty());
    teaching::record_delivery(&state.private_context(), &second.turn_ref, "retry").unwrap();
    assert_eq!(
        state
            .user.learning_store()
            .unwrap()
            .teaching_deliveries(&second.turn_ref.session_id, &second.turn_ref.turn_id)
            .unwrap()
            .len(),
        1
    );
    let recovery = prepare(&mut state);
    select(&state, &recovery.turn_ref, "recovery", None);
    finalize_agent_turn_completed(&mut state, &recovery.turn_ref, &outcome(None), &[], "now")
        .unwrap();
    let revision = state
        .user.learning_store()
        .unwrap()
        .state()
        .unwrap()
        .control
        .revision;
    json_post(
        &mut state,
        "/tutor/mutate",
        json!({"operation_id":"recovery-off","expected_revision":revision,"action":{"kind":"set_enabled","enabled":false}}),
    );
    teaching::record_delivery(&state.private_context(), &recovery.turn_ref, "recovery").unwrap();
    assert_eq!(
        state
            .user.learning_store()
            .unwrap()
            .teaching_deliveries(&recovery.turn_ref.session_id, &recovery.turn_ref.turn_id)
            .unwrap()
            .len(),
        1
    );
    let bad=post(&mut state,"/tutor/action",&json!({"operation_id":"unseen","delivery_ref":rows[0].event_id,"action":"submit","response":"yes"}).to_string());
    assert_ne!(bad.status, 200);
    assert!(bad.body.contains("尚未实际展示"));
}

#[test]
fn tutor_two_activities_keep_exact_scene_help_and_cross_chat_ownership() {
    let (root, mut state) = fixture();
    let run = prepare(&mut state);
    let turn = &run.turn_ref;
    let candidate = state.private_context().create_presentation_candidate(
            &turn.session_id,
            &turn.turn_id,
            None,
            PresentationContent {
                animation_assets: Default::default(),
                title: "Speed".into(),
                content_files: BTreeMap::from([(
                    "index.html".into(),
                    "<p>Speed experiment</p>".into(),
                )]),
                entrypoint: "index.html".into(),
                readable_content: "Speed experiment".into(),
                source_bindings: vec![],
                assumptions: vec![],
                state_contract: json!({}),
                initial_state: json!({"speed":2}),
            },
        )
        .unwrap();
    let reference = state.private_context().persist_presentation_candidate(&turn.session_id, &turn.turn_id, &candidate.candidate_id)
        .unwrap();
    select(&state, turn, "one", Some(&reference));
    select(&state, turn, "two", Some(&reference));
    finish(&mut state, turn, &outcome(Some(&reference)));
    let scene = state.private_context().save_presentation_state(
            &turn.session_id,
            &turn.turn_id,
            &reference,
            PresentationState {
                values: json!({"speed":2}),
                visible_step: Some("original".into()),
                observed_result: "The page says correct".into(),
                source_ref_ids: vec![],
            },
        )
        .unwrap();
    let mut requested_reference = serde_json::to_value(&reference).unwrap();
    requested_reference["kind"] = json!("presentation");
    let listing = json_post(
        &mut state,
        "/tutor/activities",
        json!({"session_id":turn.session_id,"turn_id":turn.turn_id,"reference":requested_reference}),
    );
    assert_eq!(listing["activities"].as_array().unwrap().len(), 2);
    assert!(!listing.to_string().contains("Compare the two times"));
    let first = listing["activities"][0]["delivery_ref"]
        .as_str()
        .unwrap()
        .to_string();
    let second = listing["activities"][1]["delivery_ref"]
        .as_str()
        .unwrap()
        .to_string();
    for id in [&first, &second] {
        json_post(
            &mut state,
            "/tutor/display",
            json!({"delivery_ref":id,"scene":scene}),
        );
    }
    let action = json!({"operation_id":"submit","delivery_ref":first,"action":"submit","response":"distance/time","scene":scene});
    let submitted = json_post(&mut state, "/tutor/action", action.clone());
    assert_eq!(submitted["assessment"], "unassessed");
    assert_eq!(json_post(&mut state, "/tutor/action", action), submitted);
    let hint = json_post(
        &mut state,
        "/tutor/action",
        json!({"operation_id":"hint","delivery_ref":first,"action":"request_hint","scene":scene}),
    );
    let store = state.user.learning_store().unwrap();
    assert!(!store
        .teaching_activity_events(&first)
        .unwrap()
        .iter()
        .any(|e| e.kind == TeachingFact::HelpDisplayed));
    assert_ne!(
        post(
            &mut state,
            "/tutor/help-displayed",
            r#"{"help_ref":"missing"}"#
        )
        .status,
        200
    );
    json_post(
        &mut state,
        "/tutor/help-displayed",
        json!({"help_ref":hint["help"]["event_id"]}),
    );
    assert!(!state
        .user.learning_store()
        .unwrap()
        .teaching_activity_events(&second)
        .unwrap()
        .iter()
        .any(|e| e.kind == TeachingFact::LearnerAction));
    let revision = state
        .user.learning_store()
        .unwrap()
        .state()
        .unwrap()
        .control
        .revision;
    json_post(
        &mut state,
        "/tutor/mutate",
        json!({"operation_id":"off","expected_revision":revision,"action":{"kind":"set_enabled","enabled":false}}),
    );
    let reveal = json_post(
        &mut state,
        "/tutor/action",
        json!({"operation_id":"reveal","delivery_ref":first,"action":"reveal","scene":scene}),
    );
    json_post(
        &mut state,
        "/tutor/help-displayed",
        json!({"help_ref":reveal["help"]["event_id"]}),
    );
    let rejected=post(&mut state,"/tutor/action",&json!({"operation_id":"paused","delivery_ref":first,"action":"revise","response":"2","scene":scene}).to_string());
    assert_ne!(rejected.status, 200);
    post(&mut state, "/agent/new", "{}");
    assert!(presentation_api::follow_up_context(&state.private_context(), &scene).is_err());
    let context =
        teaching::reference_context(&state.private_context(), submitted["event_id"].as_str().unwrap()).unwrap();
    assert!(context.contains("original"));
    assert!(context.contains(&turn.session_id));
    for chat in &mut state.user.agent_history.sessions {
        chat.messages.clear();
        chat.compaction_checkpoint = None;
    }
    state.user.learning_store().unwrap().rebuild().unwrap();
    state.user.store = MemoryStore::open(root.path().join("memory.json")).unwrap();
    assert_eq!(
        teaching::reference_context(&state.private_context(), submitted["event_id"].as_str().unwrap()).unwrap(),
        context
    );
    assert_eq!(
        state
            .user.learning_store()
            .unwrap()
            .teaching_activity_events(&first)
            .unwrap()
            .iter()
            .filter(|e| e.kind == TeachingFact::HelpDisplayed)
            .count(),
        2
    );
    let revision = state
        .user.learning_store()
        .unwrap()
        .state()
        .unwrap()
        .control
        .revision;
    json_post(
        &mut state,
        "/tutor/mutate",
        json!({"operation_id":"again","expected_revision":revision,"action":{"kind":"set_enabled","enabled":true}}),
    );
    teaching::start_request(&state.private_context(), "restart").unwrap();
    assert_eq!(
        state
            .user.learning_store()
            .unwrap()
            .teaching_activity_events(&first)
            .unwrap()
            .iter()
            .filter(|e| e.kind == TeachingFact::HelpDisplayed)
            .count(),
        2
    );
    let mut forged = scene.clone();
    forged.reference.revision = 99;
    assert_ne!(
        post(
            &mut state,
            "/tutor/display",
            &json!({"delivery_ref":first,"scene":forged}).to_string()
        )
        .status,
        200
    );
    std::fs::remove_file(root.path().join("teaching/versions/v1/map.json")).unwrap();
    assert!(teaching::reference_context(&state.private_context(), &first)
        .unwrap_err()
        .message
        .contains("无法读取"));
}

fn call(name: &str, arguments: Value) -> AssistantTurn {
    AssistantTurn {
        provider_continuation: None,
        text: None,
        tool_calls: vec![ToolCall {
            id: format!("call-{name}"),
            name: name.into(),
            arguments: arguments.to_string(),
        }],
        usage_total_tokens: Some(5),
    }
}
fn answer(text: &str) -> AssistantTurn {
    AssistantTurn {
        provider_continuation: None,
        text: Some(text.into()),
        tool_calls: vec![],
        usage_total_tokens: Some(5),
    }
}
fn script(kind: &str) -> ChatStubAdapter {
    ChatStubAdapter::scripted(vec![
        call(
            "tutor.step",
            json!({"operation":"material","object_id":"speed"}),
        ),
        call("book.text", json!({"lid":"1.1","end_lid":"1.1"})),
        call(
            "tutor.step",
            json!({"operation":"select","move":{"move_id":"next","object_ids":["speed"],"capability":"explanation","kind":kind,"interaction_intent":if kind=="explain" {"direct_explanation"} else {"neutral"},"intent_origin":if kind=="explain" {"current_request"} else {"neutral"},"intent_quote":if kind=="explain" {Some("directly explain this time")} else {None},"prompt":"Compare total distance with total time.","actions":[]}}),
        ),
        answer("Average speed uses total distance divided by total time."),
    ])
}
#[test]
fn tutor_resident_reads_sources_then_adapts_next_turn_and_stops_when_disabled() {
    let (_root, mut state) = fixture();
    state.services.adapter = Box::new(script("observe"));
    let first = json_post(
        &mut state,
        "/agent/chat",
        json!({"message":"Teach me average speed"}),
    );
    assert!(first["trace"]
        .as_array()
        .unwrap()
        .iter()
        .any(|t| t["tool"] == "book.text"));
    let session = state.user.agent_history.sessions.last().unwrap();
    let turn = session.turns.last().unwrap();
    let rows = state
        .user.learning_store()
        .unwrap()
        .teaching_deliveries(&session.id, &turn.turn_id)
        .unwrap();
    assert_eq!(rows.len(), 1, "{first}");
    let old_session = rows[0].binding.tutor_session_id.clone();
    json_post(
        &mut state,
        "/tutor/display",
        json!({"delivery_ref":rows[0].event_id}),
    );
    let revision = state
        .user.learning_store()
        .unwrap()
        .state()
        .unwrap()
        .control
        .revision;
    json_post(
        &mut state,
        "/tutor/mutate",
        json!({"operation_id":"mode","expected_revision":revision,"action":{"kind":"set_mode","session_id":old_session,"mode":"guided_inquiry"}}),
    );
    post(&mut state, "/agent/new", "{}");
    state.services.adapter = Box::new(script("explain"));
    let second = json_post(
        &mut state,
        "/agent/chat",
        json!({"message":"I am confused; directly explain this time","teaching_ref":rows[0].event_id}),
    );
    assert!(second["answer"].as_str().unwrap().contains("total time"));
    assert_eq!(
        state
            .user.learning_store()
            .unwrap()
            .state()
            .unwrap()
            .control
            .current_tutor_session_id
            .as_deref(),
        Some(old_session.as_str())
    );
    assert_eq!(
        state
            .user.learning_store()
            .unwrap()
            .state()
            .unwrap()
            .sessions[&old_session]
            .default_teaching_intent,
        Some(memory::learning::TutorSessionMode::GuidedInquiry)
    );
    let revision = state
        .user.learning_store()
        .unwrap()
        .state()
        .unwrap()
        .control
        .revision;
    json_post(
        &mut state,
        "/tutor/mutate",
        json!({"operation_id":"off","expected_revision":revision,"action":{"kind":"set_enabled","enabled":false}}),
    );
    state.services.adapter = Box::new(ChatStubAdapter::scripted(vec![answer(
        "Ordinary explanation.",
    )]));
    let prepared = prepare(&mut state);
    assert!(prepared.tutor.is_none());
    let report = agent_run::execute_prepared(
        &agent_run::BorrowedAppPort(RefCell::new(&mut state)),
        &ChatStubAdapter::scripted(vec![answer("Ordinary explanation.")]),
        prepared,
        Default::default(),
    );
    assert_eq!(report.reply.status, 200);
}

#[test]
fn tutor_ordinary_reply_keeps_help_facts_and_unready_never_exposes_formal_moves() {
    let (root, mut state) = fixture();
    let first = prepare(&mut state);
    select(&state, &first.turn_ref, "original", None);
    finish(&mut state, &first.turn_ref, &outcome(None));
    let delivery = format!("delivery:{}:original", first.turn_ref.turn_id);
    let revision = state
        .user.learning_store()
        .unwrap()
        .state()
        .unwrap()
        .control
        .revision;
    json_post(
        &mut state,
        "/tutor/mutate",
        json!({"operation_id":"off","expected_revision":revision,"action":{"kind":"set_enabled","enabled":false}}),
    );
    post(&mut state, "/agent/new", "{}");
    state.services.adapter = Box::new(ChatStubAdapter::scripted(vec![answer(
        "Use total distance divided by total time.",
    )]));
    json_post(
        &mut state,
        "/agent/chat",
        json!({"message":"Directly explain the original activity","teaching_ref":delivery}),
    );
    let session = state.user.agent_history.sessions.last().unwrap();
    let turn = session.turns.last().unwrap();
    assert_eq!(turn.teaching_ref.as_deref(), Some(delivery.as_str()));
    let ordinary_ref = AgentTurnRef {
        session_id: session.id.clone(),
        turn_id: turn.turn_id.clone(),
        user_turn_ordinal: 0,
    };
    let reread = teaching::step(&state.private_context(),
        &ordinary_ref,
        json!({"operation":"trace","event_id":delivery}),
        &[],
        &[],
    )
    .unwrap();
    assert!(reread["chunk"]
        .as_str()
        .unwrap()
        .contains("Compare total distance"));
    assert!(teaching::step(&state.private_context(),
        &ordinary_ref,
        json!({"operation":"material","object_id":"speed"}),
        &[],
        &[]
    )
    .is_err());
    let reply_ref = format!("followup:{}", turn.turn_id);
    assert!(!state
        .user.learning_store()
        .unwrap()
        .teaching_activity_events(&delivery)
        .unwrap()
        .iter()
        .any(|e| e.kind == TeachingFact::HelpDisplayed));
    json_post(
        &mut state,
        "/tutor/display",
        json!({"delivery_ref":reply_ref}),
    );
    assert_eq!(
        state
            .user.learning_store()
            .unwrap()
            .teaching_activity_events(&delivery)
            .unwrap()
            .iter()
            .filter(|e| e.kind == TeachingFact::HelpDisplayed)
            .count(),
        1
    );
    let revision = state
        .user.learning_store()
        .unwrap()
        .state()
        .unwrap()
        .control
        .revision;
    json_post(
        &mut state,
        "/tutor/mutate",
        json!({"operation_id":"on2","expected_revision":revision,"action":{"kind":"set_enabled","enabled":true}}),
    );
    teaching::start_request(&state.private_context(), "now").unwrap();
    std::fs::remove_file(root.path().join("teaching_readiness.json")).unwrap();
    let unready = prepare(&mut state);
    assert_eq!(unready.tutor.unwrap()["status"], "preparing");
    assert!(teaching::step(&state.private_context(),
        &unready.turn_ref,
        json!({"operation":"material","object_id":"speed"}),
        &[],
        &[]
    )
    .is_err());
}

#[test]
#[ignore = "requires configured real model; bounded two-turn Tutor acceptance"]
fn tutor_live_model_source_and_move_acceptance() {
    let (_root, mut state) = fixture();
    state.services.adapter = ProviderRegistry::adapter_from_config(ProviderConfig::from_env().unwrap());
    for message in [
        "Teach me the meaning of average speed from this source. Use one short observation.",
        "I mixed up distance and time; this time directly explain it.",
    ] {
        let result = json_post(&mut state, "/agent/chat", json!({"message":message}));
        let session = state.user.agent_history.sessions.last().unwrap();
        let turn = session.turns.last().unwrap();
        let rows = state
            .user.learning_store()
            .unwrap()
            .teaching_deliveries(&session.id, &turn.turn_id)
            .unwrap();
        assert!(!rows.is_empty(), "No grounded move delivered: {result}");
        println!(
            "TUTOR_LIVE_RESULT {}",
            json!({"answer":result["answer"],"tools":result["trace"].as_array().unwrap().iter().map(|r| &r["tool"]).collect::<Vec<_>>(),"move":rows[0].payload["move"]})
        );
    }
}

#[test]
#[ignore = "bounded real Server host for tutor-activities.spec.ts"]
fn tutor_browser_host() {
    tutor_browser_host_impl(false);
}

#[test]
#[ignore = "bounded real Server host for tutor-assessment.spec.ts"]
fn tutor_assessment_browser_host() {
    tutor_browser_host_impl(true);
}

fn tutor_browser_host_impl(scored: bool) {
    let (root, mut state) = fixture();
    let run = prepare(&mut state);
    let turn = &run.turn_ref;
    let html = r#"<h1>Average speed</h1><input id="speed" type="range" min="1" max="5" value="2"><p id="result">2</p><script>
const control=document.getElementById('speed'), result=document.getElementById('result');
control.oninput=()=>{result.textContent=control.value;window.presentation.commitState()};
window.presentation.registerStateReader(()=>({values:{speed:Number(control.value)}}));
window.presentation.registerStateRestorer(s=>{control.value=String(s.values.page?.speed ?? 2);result.textContent=control.value});
</script>"#;
    let candidate = state.private_context().create_presentation_candidate(
            &turn.session_id,
            &turn.turn_id,
            None,
            PresentationContent {
                animation_assets: Default::default(),
                title: "Average speed".into(),
                content_files: BTreeMap::from([("index.html".into(), html.into())]),
                entrypoint: "index.html".into(),
                readable_content: "Average speed experiment".into(),
                source_bindings: vec![],
                assumptions: vec![],
                state_contract: json!({}),
                initial_state: json!({"speed":2}),
            },
        )
        .unwrap();
    let reference = state.private_context().persist_presentation_candidate(&turn.session_id, &turn.turn_id, &candidate.candidate_id)
        .unwrap();
    if scored {
        teaching::step(&state.private_context(),
            turn,
            json!({"operation":"material","object_id":"speed"}),
            &[],
            &[],
        )
        .unwrap();
        teaching::step(&state.private_context(),turn,json!({"operation":"select","move":{"move_id":"one","object_ids":["speed"],"capability":"explanation","kind":"question","interaction_intent":"neutral","intent_origin":"neutral","prompt":"Observe one: A total distance; B total time; C colour.","actions":["submit","revise","request_hint","reveal","skip","self_report"],"hint":"Compare the two times.","explanation":"Use total distance divided by total time.","presentation":reference,"assessment":{"rule":{"kind":"choice_set","choices":["A","B","C"],"correct":["A","B"]},"source_lids":["1.1"],"source_sufficient":true,"feedback":"after_submit"}}}),&[],&[range()]).unwrap();
    } else {
        select(&state, turn, "one", Some(&reference));
    }
    if scored {
        teaching::step(&state.private_context(),turn,json!({"operation":"select","move":{"move_id":"two","object_ids":["speed"],"capability":"explanation","kind":"question","interaction_intent":"neutral","intent_origin":"neutral","prompt":"Observe two: Explain average speed.","actions":["submit","revise"],"presentation":reference,"assessment":{"rule":open_rule(),"source_lids":["1.1"],"source_sufficient":true,"feedback":"after_submit"}}}),&[],&[range()]).unwrap();
    } else {
        select(&state, turn, "two", Some(&reference));
    }
    let result = outcome(Some(&reference));
    finish(&mut state, turn, &result);
    let teaching_ref = format!("delivery:{}:one", turn.turn_id);
    let server = tiny_http::Server::http("127.0.0.1:4175").unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(300);
    println!("TUTOR_BROWSER_READY");
    while std::time::Instant::now() < deadline {
        let Some(mut request) = server.recv_timeout(Duration::from_secs(1)).unwrap() else {
            continue;
        };
        let path = request.url().to_string();
        let reply = match path.as_str() {
            "/fixture" => ok_json(
                &json!({"session_id":turn.session_id,"turn_id":turn.turn_id,"reference":reference,"outcome":result,"teaching_ref":teaching_ref}),
            ),
            "/facts" => ok_json(
                &state
                    .user.learning_store()
                    .unwrap()
                    .teaching_events(&format!("tutor-begin-1"), 0, 100)
                    .unwrap(),
            ),
            "/reopen" => {
                state.user.store = MemoryStore::open(root.path().join("memory.json")).unwrap();
                ok_json(&json!({"ok":true}))
            }
            "/evaluate" if scored => {
                let mut body = String::new();
                request.as_reader().read_to_string(&mut body).unwrap();
                let body: Value = serde_json::from_str(&body).unwrap();
                ok_json(
                    &teaching::assessment_accept(&state.private_context(),
                        turn,
                        body["action_ref"].as_str().unwrap(),
                        open_items(),
                    )
                    .unwrap(),
                )
            }
            "/stop" => {
                request
                    .respond(tiny_http::Response::from_string("stopped"))
                    .unwrap();
                break;
            }
            _ => {
                let mut body = String::new();
                request.as_reader().read_to_string(&mut body).unwrap();
                route(
                    &mut state,
                    Req {
                        method: request.method().as_str(),
                        url: &path,
                        body: &body,
                        now: "browser",
                    },
                )
            }
        };
        request
            .respond(
                tiny_http::Response::from_string(reply.body)
                    .with_status_code(reply.status)
                    .with_header(
                        tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap(),
                    ),
            )
            .unwrap();
    }
}

#[test]
fn mu1c_tutor_material_keeps_original_binding_after_workspace_replacement() {
    use crate::agent_run::{AppStatePort, BorrowedAppPort, RuntimeStatePort};
    use runtime::run_context::ResidentStatePort;
    let (_root, mut state) = fixture();
    let prepared = prepare(&mut state);
    let original_anchor = prepared.scope.reader_input.state.viewport.anchor_lid.clone();
    let other = write_multi_leaf_book("mu1c-tutor-other", "mu1c-tutor-other", 4);
    assert_eq!(route_open_book(&mut state, &json!({"dir":other}).to_string(), "later").status, 200);
    let app = BorrowedAppPort(RefCell::new(&mut state));
    let mut port = RuntimeStatePort { port:&app, turn_ref:&prepared.turn_ref, scope:&prepared.scope,
        previewed:Default::default(), animations:Default::default(), plots:Default::default() };
    assert_eq!(port.read_live_reader(|r| r.state()).unwrap_err().error_code, "WORKSPACE_STALE");
    assert_eq!(port.reader_input(&prepared.scope.book, "ignored").state.viewport.anchor_lid, original_anchor);
    let material = port.tutor_step(json!({"operation":"material","object_id":"speed"}), &[], &[]).unwrap();
    assert!(material.to_string().contains("total distance"), "{material}");
    app.with_app(|state| {
        let revision = state.user.learning_store().unwrap().state().unwrap().control.revision;
        assert_eq!(post(state, "/tutor/mutate", &json!({"operation_id":"mu1c-off","expected_revision":revision,"action":{"kind":"set_enabled","enabled":false}}).to_string()).status, 200);
    });
    assert!(!port.tutor_active().unwrap());
}

#[test]
fn jl5_teaching_receipts_keep_original_revision_across_chats_and_restart() {
    let (_root, mut state) = fixture();
    super::session_runtime_tests::enable_jsonl(&mut state);
    assert_eq!(route_agent_new(&mut state, "t0").status, 200);
    let first = prepare(&mut state);
    select(&state, &first.turn_ref, "first", None);
    finish(&mut state, &first.turn_ref, &outcome(None));
    crate::session_runtime::link_teaching(&mut state.user, &first.turn_ref, "t1").unwrap();
    let first_chat = state.user.agent_history.sessions.iter().find(|s| s.id == first.turn_ref.session_id).unwrap().clone();
    let tutor = first_chat.turns[0].domain.teaching[0].session_id.clone();
    let revision = first_chat.turns[0].domain.teaching[0].revision;
    assert!(first_chat.turns[0].domain.teaching.iter().flat_map(|l| &l.receipt_ids).any(|id| id.starts_with("delivery:")));
    assert_eq!(route_agent_new(&mut state, "t2").status, 200);
    let second = prepare(&mut state);
    assert_ne!(first.turn_ref.session_id, second.turn_ref.session_id);
    finish(&mut state, &second.turn_ref, &outcome(None));
    crate::session_runtime::link_teaching(&mut state.user, &second.turn_ref, "t3").unwrap();
    let learning_before = state.user.learning_store().unwrap().state().unwrap();
    let history = crate::session_store::load_chat_storage(&state.user.history_path).unwrap().0;
    for session in &history.sessions {
        let turn = &session.turns[0];
        assert_eq!(turn.domain.teaching[0].session_id, tutor);
        assert_eq!(turn.domain.teaching[0].revision, revision);
        assert_eq!(session.goals[0].origin_turn_id, turn.turn_id);
        for link in &turn.domain.teaching {
            for id in &link.receipt_ids {
                let receipt = state.user.learning_store().unwrap().teaching_event(id).unwrap();
                assert_eq!(receipt.binding.chat_session_id, session.id);
            }
        }
    }
    assert_eq!(json!(learning_before), json!(state.user.learning_store().unwrap().state().unwrap()));
}
