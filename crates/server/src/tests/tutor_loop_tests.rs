use super::*;
use memory::teaching::TeachingFact;
use runtime::presentation::*;

fn fixture() -> (tempfile::TempDir, AppState) {
    fixture_with_assets(true)
}

#[test]
fn tutor_ordinary_chat_records_understanding_without_enabling_teaching() {
    let (_root, mut state) = fixture_with_assets(false);
    let private = tempfile::tempdir().unwrap();
    state.user.store = MemoryStore::open(private.path().join("memory.json")).unwrap();
    let original = source_move(&state);
    let run = prepare_agent_chat(&mut state, r#"{"message":"Why total time? I do not understand the denominator."}"#, "now").unwrap_or_else(|r|panic!("{}",r.body));
    let context = run.tutor.as_ref().expect("ordinary questions must receive understanding context");
    assert_eq!(context["status"], "observing");
    let fact = format!("usage:{}:user:{}",context["binding"]["tutor_session_id"].as_str().unwrap(),run.turn_ref.turn_id);
    let request = json!({"operation":"evidence","nature":"hypothesis","operation_id":"ordinary","target":original["move"]["target"],
        "fact_refs":[fact],"interpretation":"The denominator connection is unclear.","teaching_implication":"Explain the total elapsed time first."});
    assert!(teaching::step(&state.private_context(),&run.turn_ref,original.clone(),&[],&[range()]).is_err(), "observation cannot select a teaching move");
    let adapter = ChatStubAdapter::scripted(vec![call("book.text",json!({"lid":"1.1","end_lid":"1.1"})),call("tutor.step",request),answer("Use total time.")]);
    let result = agent_run::execute_prepared(&agent_run::BorrowedAppPort(RefCell::new(&mut state)), &adapter, run, Default::default());
    assert_eq!(result.reply.status,200,"{}",result.reply.body);
    let store = state.user.learning_store().unwrap();
    assert_eq!(store.evidence("interpretation:ordinary").unwrap().learner_quote,"Why total time? I do not understand the denominator.");
    assert!(!store.state().unwrap().control.enabled);
    assert!(store.state().unwrap().sessions.is_empty());
    drop(store);
    // The interpretation survives reopening and can be revised after explicit teaching starts.
    state.user.store = MemoryStore::open(private.path().join("memory.json")).unwrap();
    json_post(&mut state,"/tutor/mutate",json!({"operation_id":"ordinary-on","expected_revision":0,"action":{"kind":"set_enabled","enabled":true}}));
    post(&mut state,"/agent/new","{}");
    let run = prepare_agent_chat(&mut state,r#"{"message":"I understand the denominator now; explain the conditions."}"#,"now").unwrap_or_else(|r|panic!("{}",r.body));
    let context = run.tutor.as_ref().unwrap();
    assert_eq!(context["learner_context"]["interpretations"][0]["evidence_ref"],"interpretation:ordinary");
    let fact = format!("usage:{}:user:{}",context["binding"]["tutor_session_id"].as_str().unwrap(),run.turn_ref.turn_id);
    let changed = json!({"operation":"evidence","nature":"hypothesis","operation_id":"ordinary-revised","target":original["move"]["target"],
        "fact_refs":[fact],"supersedes":"interpretation:ordinary","interpretation":"The question is now about validity conditions.","teaching_implication":"Explain positive total time."});
    let accepted = teaching::step(&state.private_context(),&run.turn_ref,changed,&[],&[range()]).unwrap();
    assert_eq!(accepted["evidence_ref"],"interpretation:ordinary-revised");
    assert!(state.user.learning_store().unwrap().evidence_is_superseded("interpretation:ordinary").unwrap());
}

#[test]
fn tutor_first_question_becomes_starting_intent_when_enabled() {
    let (_root, mut state) = fixture_with_assets(false);
    let private = tempfile::tempdir().unwrap();
    state.user.store = MemoryStore::open(private.path().join("memory.json")).unwrap();
    json_post(&mut state,"/tutor/mutate",json!({"operation_id":"enable","expected_revision":0,"action":{"kind":"set_enabled","enabled":true}}));
    let run = prepare_agent_chat(&mut state, r#"{"message":"Help me understand average speed."}"#, "now").unwrap_or_else(|r|panic!("{}",r.body));
    let context = run.tutor.unwrap();
    assert_eq!(context["status"],"active");
    assert_eq!(context["user_intent"],"Help me understand average speed.");
    assert_eq!(state.user.learning_store().unwrap().state().unwrap().sessions.len(),1);
}
fn fixture_with_assets(published: bool) -> (tempfile::TempDir, AppState) {
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
    super::tutor_tests::t15_foundation(root.path(), &state.workspace.book);
    std::fs::write(root.path().join("source.txt"), &source).unwrap();
    super::tutor_tests::t15_close(root.path(), &state.workspace.book, "pass1", &["base.json", "source.txt"]);
    if published {
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
    }
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
    assert!(teaching::reference_context(&state.private_context(), &first).is_ok());
    state.workspace.book = Arc::new(Book::new(sample_base(), "changed source"));
    assert!(teaching::reference_context(&state.private_context(), &first).is_err());
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

fn source_move(state: &AppState) -> Value {
    json!({"operation":"select","move":{"move_id":"source","object_ids":[],
        "target":{"learning_focus":"Average speed","expected_performance":"Explain why total time is the denominator",
            "capability":"explanation","source_bindings":[{"source_id":state.workspace.book.base.book_id,
                "source_revision":state.workspace.book.source_fingerprint(),"start_lid":"1.1","end_lid":"1.1"}],"object_refs":[]},
        "capability":"explanation","kind":"explain","interaction_intent":"neutral","intent_origin":"neutral",
        "prompt":"Average speed uses total distance divided by total time.","actions":[]}})
}

#[test]
fn tutor_t17a_ordinary_messages_and_closed_reading_survive_reopen() {
    let (root, mut state) = fixture_with_assets(false);
    let revision = state.user.learning_store().unwrap().state().unwrap().control.revision;
    json_post(&mut state, "/tutor/mutate", json!({"operation_id":"off-usage","expected_revision":revision,"action":{"kind":"set_enabled","enabled":false}}));
    state.workspace.reader.goto_lid(&state.workspace.book, &mut state.user.store, "1.1", "read-while-off").unwrap();
    state.user.store.flush_pending_reads().unwrap();
    state.services.adapter = Box::new(ChatStubAdapter::scripted(vec![answer("Use total time.")]));
    json_post(&mut state, "/agent/chat", json!({"message":"Why total time?"}));
    assert!(!state.user.learning_store().unwrap().state().unwrap().control.enabled);
    state.user.store = MemoryStore::open(root.path().join("memory.json")).unwrap();
    let revision = state.user.learning_store().unwrap().state().unwrap().control.revision;
    json_post(&mut state, "/tutor/mutate", json!({"operation_id":"on-usage","expected_revision":revision,"action":{"kind":"set_enabled","enabled":true}}));
    teaching::start_request(&state.private_context(), "now").unwrap();
    post(&mut state, "/agent/new", "{}");
    let run = prepare_agent_chat(&mut state, r#"{"message":"I understand time; I mean when the denominator is valid."}"#, "now").unwrap_or_else(|r|panic!("{}",r.body));
    let context = run.tutor.unwrap();
    let session = context["binding"]["tutor_session_id"].as_str().unwrap();
    let store = memory::learning::LearningStore::open(&root.path().join("learning.db"), false).unwrap();
    let facts: Vec<_> = store.teaching_events(session, 0, 100).unwrap().into_iter().map(|(_,e)|e).filter(|e| e.kind==TeachingFact::UsageObserved).collect();
    assert!(facts.iter().any(|e|e.payload["usage"]["kind"]=="reading_contact"));
    let question = facts.iter().find(|e|e.payload["response"]=="Why total time?").unwrap();
    let reply = facts.iter().find(|e|e.payload["answer"]=="Use total time.").unwrap();
    assert_eq!(question.payload["original_ref"], reply.payload["original_ref"]);
    assert!(reply.causal_refs.contains(&question.event_id));
    assert!(facts.iter().any(|e|e.payload["response"]=="I understand time; I mean when the denominator is valid."));
    assert_eq!(context["learner_context"]["entries"],json!([]));
    let read = teaching::step(&state.private_context(), &run.turn_ref,json!({"operation":"trace","event_id":question.event_id}),&[],&[]).unwrap();
    assert!(read["chunk"].as_str().unwrap().contains("Why total time?"));
    assert!(store.evidence_page(&state.workspace.book.base.book_id,0,100).unwrap().is_empty());
}

#[test]
fn tutor_t16_source_only_resident_delivers_and_continues_after_reopen() {
    let (root, mut state) = fixture_with_assets(false);
    let movement = source_move(&state);
    state.services.adapter = Box::new(ChatStubAdapter::scripted(vec![
        call("book.text", json!({"lid":"1.1","end_lid":"1.1"})),
        call("tutor.step", movement.clone()),
        answer("Average speed uses total distance divided by total time."),
    ]));
    let first = json_post(&mut state, "/agent/chat", json!({"message":"I just opened this book and have not read it. Explain average speed."}));
    assert_eq!(first["incomplete"], false, "{first}");
    let session = state.user.agent_history.sessions.last().unwrap();
    let turn = session.turns.last().unwrap();
    let store = memory::learning::LearningStore::open(&root.path().join("learning.db"), false).unwrap();
    let rows = store.teaching_deliveries(&session.id, &turn.turn_id).unwrap();
    assert_eq!(rows.len(), 1, "{first}");
    assert!(rows[0].binding.map_revision.is_none());
    assert_eq!(rows[0].payload["move"]["target"], movement["move"]["target"]);
    let binding = store.teaching_event(&format!("binding:{}", turn.turn_id)).unwrap();
    assert_eq!(binding.payload["learner_context"]["confirmed_background"], json!([]));
    assert!(binding.payload["learner_context"]["current_request"].as_str().unwrap().contains("have not read"));
    assert_eq!(binding.payload["learner_context"]["entries"], json!([]));
    drop(store);
    let activity_request = json!({"session_id":session.id,"turn_id":turn.turn_id});
    let activities = json_post(&mut state, "/tutor/activities", activity_request);
    assert_eq!(activities["activities"][0]["status"], "active", "{activities}");
    post(&mut state, "/agent/new", "{}");
    state.services.adapter = Box::new(ChatStubAdapter::scripted(vec![
        answer("An unfinished explanation before choosing a move."),
        call("book.text", json!({"lid":"1.1","end_lid":"1.1"})),
        call("tutor.step", json!({"operation":"trace","event_id":rows[0].event_id})),
        call("tutor.step", movement.clone()), answer("The denominator is the total time."),
    ]));
    let second = json_post(&mut state, "/agent/chat", json!({"message":"Why total time?","teaching_ref":rows[0].event_id}));
    assert_eq!(second["incomplete"], false, "{second}");
    let session = state.user.agent_history.sessions.last().unwrap();
    let turn = session.turns.last().unwrap();
    let binding = state.user.learning_store().unwrap().teaching_event(&format!("binding:{}", turn.turn_id)).unwrap();
    assert_eq!(binding.payload["learner_context"]["current_target"], movement["move"]["target"]);
    assert!(!binding.payload["learner_context"]["recent_facts"].as_array().unwrap().is_empty());
    let revision = state.user.learning_store().unwrap().state().unwrap().control.revision;
    json_post(&mut state, "/tutor/mutate", json!({"operation_id":"off","expected_revision":revision,"action":{"kind":"set_enabled","enabled":false}}));
    assert_eq!(prepare(&mut state).tutor.unwrap()["status"], "observing");
    assert!(teaching::reference_context(&state.private_context(), &rows[0].event_id).is_ok());
}

#[test]
fn tutor_t17b_resident_revises_and_adopts_ordinary_understanding() {
    let (root, mut state) = fixture_with_assets(false);
    super::session_runtime_tests::enable_jsonl(&mut state);
    assert_eq!(route_agent_new(&mut state, "now").status, 200);
    let original = source_move(&state);
    let mut previous = None;
    for (id, message, interpretation, implication, explanation) in [
        ("first", "Why total time? I still do not understand.", "The denominator connection may be missing.", "Use the whole journey as the example.", "Consider the whole journey and add all elapsed times."),
        ("corrected", "I understand the denominator; I mean when it is valid.", "The user understands the denominator and asks about its validity conditions.", "Explain positive total time and its boundary conditions.", "The denominator must be positive; now consider the zero-time boundary."),
    ] {
        let run = prepare_agent_chat(&mut state, &json!({"message":message}).to_string(), "now").unwrap_or_else(|r|panic!("{}",r.body));
        let binding = run.tutor.as_ref().unwrap()["binding"].clone();
        let fact = format!("usage:{}:user:{}",binding["tutor_session_id"].as_str().unwrap(),run.turn_ref.turn_id);
        let evidence_id = format!("interpretation:{id}");
        let mut movement = original.clone();
        movement["move"]["interpretation_refs"] = json!([evidence_id]);
        movement["move"]["prompt"] = json!(explanation);
        let mut request = json!({"operation":"evidence","nature":"hypothesis","operation_id":id,"target":original["move"]["target"],
            "fact_refs":[fact],"interpretation":interpretation,"teaching_implication":implication});
        if let Some(previous) = &previous { request["supersedes"] = json!(previous); }
        let adapter = ChatStubAdapter::scripted(vec![
            call("book.text",json!({"lid":"1.1","end_lid":"1.1"})), call("tutor.step",request),
            call("tutor.step",movement), answer(explanation),
        ]);
        let turn = run.turn_ref.clone();
        let result = agent_run::execute_prepared(&agent_run::BorrowedAppPort(RefCell::new(&mut state)), &adapter, run, Default::default());
        assert_eq!(result.reply.status,200,"{}",result.reply.body);
        let store = memory::learning::LearningStore::open(&root.path().join("learning.db"),false).unwrap();
        let accepted = store.evidence(&evidence_id).unwrap_or_else(|e|panic!("{e:?}; {}",result.reply.body));
        assert_eq!(accepted.learner_quote,message);
        assert!(accepted.assessment_ref.is_none());
        let delivery = store.teaching_event(&format!("delivery:{}:source",turn.turn_id)).unwrap();
        assert_eq!(delivery.payload["move"]["interpretation_refs"],json!([evidence_id]));
        assert_eq!(delivery.payload["answer"],explanation);
        assert!(store.learner_projection(&state.workspace.book.base.book_id).unwrap().0.unwrap().rows.is_empty());
        previous = Some(evidence_id);
        post(&mut state,"/agent/new","{}");
    }
    let (history, session_store) = crate::session_store::load_chat_storage(&state.user.history_path).unwrap();
    state.user.agent_history = history;
    state.user.session_store = session_store;
    let run = prepare(&mut state);
    let interpretations = &run.tutor.as_ref().unwrap()["learner_context"]["interpretations"];
    assert_eq!(interpretations.as_array().unwrap().len(),1);
    assert_eq!(interpretations[0]["evidence_ref"],"interpretation:corrected");
    let mut stale = original.clone(); stale["move"]["interpretation_refs"]=json!(["interpretation:first"]);
    assert!(teaching::step(&state.private_context(),&run.turn_ref,stale,&[],&[range()]).is_err());
    let bad = json!({"operation":"evidence","nature":"hypothesis","operation_id":"bad","target":original["move"]["target"],
        "fact_refs":[format!("binding:{}",run.turn_ref.turn_id)],"interpretation":"invented","teaching_implication":"invented"});
    assert!(teaching::step(&state.private_context(),&run.turn_ref,bad,&[],&[range()]).is_err());
    let view = json_post(&mut state,"/tutor/understanding",json!({}));
    assert_eq!(view["interpretations"][0]["evidence_id"],"interpretation:corrected");
    let corrected = json_post(&mut state,"/tutor/correct",json!({"evidence_ref":"interpretation:corrected","operation_id":"user-correction","text":"I also know the positive-time condition."}));
    finish(&mut state,&run.turn_ref,&outcome(None));
    let next = prepare(&mut state).tutor.unwrap();
    assert_eq!(next["learner_context"]["interpretations"][0]["evidence_ref"],corrected["evidence_ref"]);
    assert_eq!(next["learner_context"]["interpretations"][0]["correction"],"I also know the positive-time condition.");
}

#[test]
fn tutor_t17a_usage_trace_reads_only_committed_presentations_and_exact_scene() {
    let (_root,mut state)=fixture_with_assets(false);
    let run=prepare(&mut state);
    let candidate=state.private_context().create_presentation_candidate(&run.turn_ref.session_id,&run.turn_ref.turn_id,None,
        PresentationContent { animation_assets:Default::default(),title:"Journey".into(),
            content_files:BTreeMap::from([("index.html".into(),"<p>Journey</p>".into())]),entrypoint:"index.html".into(),
            readable_content:"Watch the total elapsed time.".into(),source_bindings:vec![],assumptions:vec![],state_contract:json!({}),initial_state:json!({"time":2}) }).unwrap();
    let reference=state.private_context().persist_presentation_candidate(&run.turn_ref.session_id,&run.turn_ref.turn_id,&candidate.candidate_id).unwrap();
    let mut movement=source_move(&state); movement["move"]["presentation"]=json!(reference);
    teaching::step(&state.private_context(),&run.turn_ref,movement,&[],&[range()]).unwrap();
    assert!(teaching::step(&state.private_context(),&run.turn_ref,json!({"operation":"trace","event_id":format!("move:{}:source",run.turn_ref.turn_id)}),&[],&[]).is_err());
    let binding=run.tutor.as_ref().unwrap()["binding"].clone();
    let usage=format!("usage:{}:assistant:{}",binding["tutor_session_id"].as_str().unwrap(),run.turn_ref.turn_id);
    assert!(state.user.learning_store().unwrap().teaching_event(&usage).is_err());
    finish(&mut state,&run.turn_ref,&outcome(Some(&reference)));
    let read=teaching::step(&state.private_context(),&run.turn_ref,json!({"operation":"trace","event_id":usage}),&[],&[]).unwrap();
    assert!(read["chunk"].as_str().unwrap().contains("Watch the total elapsed time."));
    assert!(!read["chunk"].as_str().unwrap().contains("content_files"));
    let scene=state.private_context().save_presentation_state(&run.turn_ref.session_id,&run.turn_ref.turn_id,&reference,
        PresentationState{values:json!({"time":4}),visible_step:Some("waiting".into()),observed_result:"four seconds".into(),source_ref_ids:vec![]}).unwrap();
    let next=prepare_agent_chat(&mut state,&json!({"message":"Why four seconds?","presentation_follow_up":scene}).to_string(),"later").unwrap_or_else(|r|panic!("{}",r.body));
    let usage=format!("usage:{}:user:{}",binding["tutor_session_id"].as_str().unwrap(),next.turn_ref.turn_id);
    let read=teaching::step(&state.private_context(),&next.turn_ref,json!({"operation":"trace","event_id":usage}),&[],&[]).unwrap();
    assert!(read["chunk"].as_str().unwrap().contains("four seconds"));
    assert!(read["chunk"].as_str().unwrap().contains("waiting"));
}

#[test]
fn tutor_t17c_source_only_assessments_keep_help_and_reopen() {
    let (root, mut state) = fixture_with_assets(false);
    let run = prepare(&mut state);
    let mut movement = source_move(&state);
    movement["move"]["kind"]=json!("question");
    movement["move"]["actions"]=json!(["submit","revise","request_hint"]);
    movement["move"]["hint"]=json!("Think about all elapsed time.");
    movement["move"]["assessment"]=json!({"rule":{"kind":"choice_set","choices":["A","B"],"correct":["A"]},"source_lids":["1.1"],"source_sufficient":true,"feedback":"after_submit"});
    teaching::step(&state.private_context(),&run.turn_ref,movement,&[],&[range()]).unwrap();
    finish(&mut state,&run.turn_ref,&outcome(None));
    let id=format!("delivery:{}:source",run.turn_ref.turn_id);
    json_post(&mut state,"/tutor/display",json!({"delivery_ref":id}));
    let first=json_post(&mut state,"/tutor/action",json!({"operation_id":"source-first","delivery_ref":id,"action":"submit","response":"A"}));
    assert_eq!(first["assessment"],"correct");
    let help=json_post(&mut state,"/tutor/action",json!({"operation_id":"source-hint","delivery_ref":id,"action":"request_hint"}));
    json_post(&mut state,"/tutor/help-displayed",json!({"help_ref":help["help"]["event_id"]}));
    json_post(&mut state,"/tutor/action",json!({"operation_id":"source-revise","delivery_ref":id,"action":"revise","response":"A"}));
    let store=memory::learning::LearningStore::open(&root.path().join("learning.db"),false).unwrap();
    let first=store.evidence("assessment:action:source-first:source").unwrap();
    let revised=store.evidence("assessment:action:source-revise:source").unwrap();
    assert!(first.object_id.is_none() && first.map_revision.is_none());
    assert!(first.assistance_refs.is_empty()); assert_eq!(first.attempt,1);
    assert_eq!(revised.assistance_refs.len(),1); assert_eq!(revised.attempt,2);
    assert_eq!(first.target,revised.target);
    assert!(store.learner_projection(&state.workspace.book.base.book_id).unwrap().0.unwrap().rows.is_empty());
    drop(store);
    let next=prepare(&mut state).tutor.unwrap();
    assert_eq!(next["learner_context"]["interpretations"].as_array().unwrap().len(),2);
    // Open rubric must also run without a teaching map, and failed evaluation retains the response.
    let run=prepare(&mut state);
    let mut movement=source_move(&state);
    movement["move"]["actions"]=json!(["submit"]);
    movement["move"]["assessment"]=json!({"rule":open_rule(),"source_lids":["1.1"],"source_sufficient":true,"feedback":"after_submit"});
    teaching::step(&state.private_context(),&run.turn_ref,movement,&[],&[range()]).unwrap(); finish(&mut state,&run.turn_ref,&outcome(None));
    let id=format!("delivery:{}:source",run.turn_ref.turn_id);
    json_post(&mut state,"/tutor/display",json!({"delivery_ref":id}));
    json_post(&mut state,"/tutor/action",json!({"operation_id":"source-open","delivery_ref":id,"action":"submit","response":"all distance over all time"}));
    let packet=teaching::assessment_input(&state.private_context(),&run.turn_ref,"action:source-open").unwrap();
    assert_eq!(packet["response"],"all distance over all time");
    assert!(teaching::assessment_accept(&state.private_context(),&run.turn_ref,"action:source-open",json!([])).is_err());
    assert!(state.user.learning_store().unwrap().assessment("action:source-open").unwrap().is_none());
    let accepted=teaching::assessment_accept(&state.private_context(),&run.turn_ref,"action:source-open",open_items()).unwrap();
    assert_eq!(accepted["assessment"],"correct");
}

#[test]
fn tutor_t18_publications_add_materials_without_rebinding_prior_activities() {
    use crate::{control_store::{ControlStore, ServiceWriter}, published_library::PublishedLibrary};
    let (root, mut state) = fixture();
    let service = tempfile::tempdir().unwrap();
    let mut control = ControlStore::open(ServiceWriter::acquire(service.path()).unwrap()).unwrap();
    control.create_user("reader").unwrap();
    let mut library = PublishedLibrary::new(control);
    let mut receipt: Value = serde_json::from_slice(&std::fs::read(root.path().join("teaching_readiness.json")).unwrap()).unwrap();
    let mut map: Value = serde_json::from_slice(&std::fs::read(root.path().join("teaching/versions/v1/map.json")).unwrap()).unwrap();
    std::fs::remove_file(root.path().join("teaching_readiness.json")).unwrap();
    let session_before = state.user.learning_store().unwrap().state().unwrap();
    let mut deliveries = Vec::new();
    for revision in 0..3 {
        if revision > 0 {
            let version = format!("v{revision}");
            let path = format!("teaching/versions/{version}/map.json");
            receipt["teaching_map_revision"] = json!(version);
            receipt["map_path"] = json!(path);
            map["objects"]["objects"][0]["object_revision"] = json!(revision);
            map["cognitive_materials"]["materials"][0]["purpose"] = json!(format!("Material revision {revision}"));
            std::fs::create_dir_all(root.path().join(format!("teaching/versions/{version}"))).unwrap();
            std::fs::write(root.path().join(path), map.to_string()).unwrap();
            std::fs::write(root.path().join("teaching_readiness.json"), receipt.to_string()).unwrap();
        }
        let publication = library.publish(root.path()).unwrap();
        assert!(library.load("reader", &publication.reference).is_err());
        library.grant("reader", &publication.reference).unwrap();
        let loaded = library.load("reader", &publication.reference).unwrap();
        state.workspace.bind_publication(loaded);
        let ready = crate::tutor_api::readiness_view(&state.workspace.book, &state.workspace.book_dir);
        assert_eq!(ready["status"], "ready");
        assert!(ready.get("required_stages").is_none());
        let run = prepare(&mut state);
        let expected_map = (revision > 0).then(||format!("v{revision}"));
        assert_eq!(run.tutor.as_ref().unwrap()["binding"]["map_revision"], json!(expected_map));
        if revision > 0 {
            let material = teaching::step(&state.private_context(), &run.turn_ref, json!({"operation":"material","object_id":"speed"}), &[], &[]).unwrap();
            assert!(material.to_string().contains(&format!("Material revision {revision}")));
        }
        let mut movement = source_move(&state);
        movement["move"]["actions"] = json!(["submit","revise","request_hint"]);
        movement["move"]["hint"] = json!("Use total time.");
        movement["move"]["assessment"] = json!({"rule":{"kind":"choice_set","choices":["A","B"],"correct":["A"]},"source_lids":["1.1"],"source_sufficient":true,"feedback":"after_submit"});
        if revision > 0 {
            movement["move"]["object_ids"] = json!(["speed"]);
            movement["move"]["target"]["object_refs"] = json!([{"source_id":state.workspace.book.base.book_id,"object_id":"speed","object_revision":revision}]);
        }
        teaching::step(&state.private_context(), &run.turn_ref, movement, &[], &[range()]).unwrap();
        finish(&mut state, &run.turn_ref, &outcome(None));
        let delivery = state.user.learning_store().unwrap().teaching_event(&format!("delivery:{}:source",run.turn_ref.turn_id)).unwrap();
        json_post(&mut state, "/tutor/display", json!({"delivery_ref":delivery.event_id}));
        deliveries.push(delivery);
    }
    assert!(!state.workspace.book_dir.join("teaching/versions/v1/map.json").exists());
    for (index, saved) in deliveries.iter().enumerate() {
        let response = json_post(&mut state, "/tutor/action", json!({"operation_id":format!("old-{index}"),"delivery_ref":saved.event_id,"action":"submit","response":"A"}));
        assert_eq!(response["assessment"], "correct");
        assert_eq!(state.user.learning_store().unwrap().teaching_event(&saved.event_id).unwrap(), *saved);
        let evidence = state.user.learning_store().unwrap().evidence(&format!("assessment:action:old-{index}:{}",if index==0 {"source"} else {"speed"})).unwrap();
        assert_eq!(evidence.map_revision, saved.binding.map_revision);
        assert_eq!(serde_json::to_value(evidence.target).unwrap(), saved.payload["move"]["target"]);
    }
    assert_eq!(state.user.learning_store().unwrap().state().unwrap(), session_before);
    let corrected = json_post(&mut state, "/tutor/correct", json!({"evidence_ref":"assessment:action:old-0:source","operation_id":"t18-correction","text":"I understand the denominator; explain the conditions."}));
    let session_id = session_before.control.current_tutor_session_id.clone().unwrap();
    json_post(&mut state, "/tutor/mutate", json!({"operation_id":"t18-pause","expected_revision":session_before.control.revision,"action":{"kind":"pause","session_id":session_id}}));
    let paused = prepare(&mut state);
    assert_eq!(paused.tutor.as_ref().unwrap()["status"], "observing");
    finish(&mut state, &paused.turn_ref, &outcome(None));
    let original_book = state.workspace.book.clone();
    let mut other = sample_base(); other.book_id = "other-book".into();
    state.workspace.book = Arc::new(Book::new(other, "other source"));
    let control_before = state.user.learning_store().unwrap().state().unwrap();
    assert_eq!(teaching::start_request(&state.private_context(), "now").unwrap()["started"], false);
    assert_eq!(state.user.learning_store().unwrap().state().unwrap(), control_before);
    state.workspace.book = original_book;
    assert_eq!(teaching::start_request(&state.private_context(), "now").unwrap()["started"], true);
    post(&mut state, "/agent/new", "{}");
    state.user.store = MemoryStore::open(root.path().join("memory.json")).unwrap();
    let next = prepare(&mut state).tutor.unwrap();
    assert_eq!(next["binding"]["tutor_session_id"], session_id);
    assert!(next["learner_context"]["interpretations"].as_array().unwrap().iter().any(|i|
        i["evidence_ref"] == corrected["evidence_ref"] && i["correction"] == "I understand the denominator; explain the conditions."));
}

#[test]
fn tutor_t16_target_requires_exact_current_turn_source_and_object_identity() {
    let (_root, mut state) = fixture_with_assets(false);
    let run = prepare(&mut state);
    let movement = source_move(&state);
    assert!(teaching::step(&state.private_context(), &run.turn_ref, movement.clone(), &[], &[]).is_err());
    let mut invalid_move = movement.clone();
    invalid_move["move"]["target"]["source_bindings"][0]["end_lid"] = json!("1.2");
    assert!(teaching::step(&state.private_context(), &run.turn_ref, invalid_move, &[], &[range()]).is_err());
    let mut invalid_move = movement.clone();
    invalid_move["move"]["target"]["source_bindings"][0]["source_revision"] = json!("other");
    assert!(teaching::step(&state.private_context(), &run.turn_ref, invalid_move, &[], &[range()]).is_err());
    let mut invalid_move = movement.clone();
    invalid_move["move"]["target"]["object_refs"] = json!([{"source_id":state.workspace.book.base.book_id,"object_id":"invented","object_revision":1}]);
    assert!(teaching::step(&state.private_context(), &run.turn_ref, invalid_move, &[], &[range()]).is_err());
    let mut partial = range();
    partial.ranges = vec![read_tools::SourceSelectedRange { lid:"1.1".into(), range:read_tools::SourceTextRange { start:0, end:5 } }];
    assert!(teaching::step(&state.private_context(), &run.turn_ref, movement.clone(), &[], &[partial]).is_err());
    teaching::step(&state.private_context(), &run.turn_ref, movement, &[], &[range()]).unwrap();
}

#[test]
fn tutor_t16_accepted_objects_without_materials_stay_frozen_and_deliver() {
    let (root, mut state) = fixture();
    let map: Value = serde_json::from_slice(&std::fs::read(root.path().join("teaching/versions/v1/map.json")).unwrap()).unwrap();
    std::fs::remove_file(root.path().join("teaching_readiness.json")).unwrap();
    let mut objects = map["objects"].clone();
    objects["version"] = json!("formal_objects.v1");
    objects["source_id"] = json!(state.workspace.book.base.book_id);
    objects["source_revision"] = json!(state.workspace.book.source_fingerprint());
    objects["revision"] = json!(1);
    std::fs::write(root.path().join("formal_objects.json"), objects.to_string()).unwrap();
    super::tutor_tests::t15_close(root.path(), &state.workspace.book, "formal_objects", &["formal_objects.json"]);
    let run = prepare(&mut state);
    assert_eq!(run.tutor.as_ref().unwrap()["object_count"], 1);
    assert!(run.tutor.as_ref().unwrap()["binding"]["map_revision"].is_null());
    let mut movement = source_move(&state);
    movement["move"]["object_ids"] = json!(["speed"]);
    movement["move"]["target"]["object_refs"] = json!([{"source_id":state.workspace.book.base.book_id,"object_id":"speed","object_revision":1}]);
    assert!(teaching::step(&state.private_context(), &run.turn_ref, movement.clone(), &[], &[range()]).is_err());
    // A supported builder publication replaces the mutable standalone artifact.
    objects["revision"] = json!(2);
    objects["objects"][0]["object_revision"] = json!(2);
    std::fs::write(root.path().join("formal_objects.json"), objects.to_string()).unwrap();
    super::tutor_tests::t15_close(root.path(), &state.workspace.book, "formal_objects", &["formal_objects.json"]);
    let material = teaching::step(&state.private_context(), &run.turn_ref, json!({"operation":"material","object_id":"speed"}), &[], &[]).unwrap();
    assert_eq!(material["object"]["object_revision"], 1);
    assert_eq!(material["materials"], json!([]));
    let mut wrong_revision = movement.clone();
    wrong_revision["move"]["target"]["object_refs"][0]["object_revision"] = json!(2);
    assert!(teaching::step(&state.private_context(), &run.turn_ref, wrong_revision, &[], &[range()]).is_err());
    teaching::step(&state.private_context(), &run.turn_ref, movement, &[], &[range()]).unwrap();
    finish(&mut state, &run.turn_ref, &outcome(None));
    let rows = state.user.learning_store().unwrap().teaching_deliveries(&run.turn_ref.session_id, &run.turn_ref.turn_id).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].payload["object_versions"][0]["object_revision"], 1);
    let next = prepare(&mut state);
    let material = teaching::step(&state.private_context(), &next.turn_ref, json!({"operation":"material","object_id":"speed"}), &[], &[]).unwrap();
    assert_eq!(material["object"]["object_revision"], 2);
    // T17: accepted standalone objects remain current in the understanding view.
    let mut scored=source_move(&state);
    scored["move"]["object_ids"]=json!(["speed"]);
    scored["move"]["target"]["object_refs"]=json!([{"source_id":state.workspace.book.base.book_id,"object_id":"speed","object_revision":2}]);
    scored["move"]["actions"]=json!(["submit"]);
    scored["move"]["assessment"]=json!({"rule":{"kind":"choice_set","choices":["A","B"],"correct":["A"]},"source_lids":["1.1"],"source_sufficient":true,"feedback":"after_submit"});
    teaching::step(&state.private_context(),&next.turn_ref,scored,&[],&[range()]).unwrap();
    finish(&mut state,&next.turn_ref,&outcome(None));
    let delivery=format!("delivery:{}:source",next.turn_ref.turn_id);
    json_post(&mut state,"/tutor/display",json!({"delivery_ref":delivery}));
    json_post(&mut state,"/tutor/action",json!({"operation_id":"standalone-score","delivery_ref":delivery,"action":"submit","response":"A"}));
    let view=json_post(&mut state,"/tutor/understanding",json!({}));
    assert!(view["rows"].as_array().unwrap().iter().any(|r|r["object_id"]=="speed" && r["object_revision"]==2 && r["historical"]==false && r["independent_support"]==1),"{view}");
}

#[test]
fn tutor_t16_context_keeps_confirmed_background_without_inventing_knowledge() {
    use memory::{Applicability, BackgroundClaim, CreateProfileFact, FactSource, ProfileScope, Sensitivity};
    let (_root, mut state) = fixture_with_assets(false);
    let stated = prepare_agent_chat(&mut state, r#"{"message":"I know distance but have not studied average speed"}"#, "2026-10-09T10:00:00Z")
        .unwrap_or_else(|r| panic!("{}", r.body));
    let fact = state.user.store.create_profile_fact(CreateProfileFact {
        scope: ProfileScope::Book { book_id:state.workspace.book.base.book_id.clone() },
        applicability: Applicability::Any,
        payload: ProfilePayload::Background(BackgroundClaim { key:"physics".into(), value:"I know distance but have not studied average speed".into() }),
        source: FactSource::UserStated, evidence: vec![memory::EvidenceRef::Turn {
            session_id:stated.turn_ref.session_id, turn_id:stated.turn_ref.turn_id,
        }], confidence:None,
        sensitivity:Sensitivity::Normal, valid_until:None,
    }, "2026-10-09T10:00:00Z").unwrap();
    let context = prepare(&mut state).tutor.unwrap();
    assert_eq!(context["learner_context"]["confirmed_background"][0]["fact_id"], fact.fact_id);
    assert!(context["learner_context"]["confirmed_background"].to_string().contains("not studied average speed"));
    assert_eq!(context["learner_context"]["entries"], json!([]));
    assert_eq!(context["learner_context"]["unknown_when_absent"], true);
    assert_eq!(context["learner_context"]["user_intent"], context["user_intent"]);
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
    assert_eq!(prepared.tutor.as_ref().unwrap()["status"], "observing");
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
    assert_eq!(prepare(&mut state).tutor.unwrap()["status"], "active");
    std::fs::remove_file(root.path().join("book_structure.json")).unwrap();
    let unready = prepare(&mut state);
    assert_eq!(unready.tutor.unwrap()["status"], "observing");
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
    tutor_browser_host_impl(false, false);
}

#[test]
#[ignore = "bounded real Server host for tutor-assessment.spec.ts"]
fn tutor_assessment_browser_host() {
    tutor_browser_host_impl(true, false);
}

#[test]
#[ignore = "bounded real Server host for tutor-continuity.spec.ts"]
fn tutor_t18_browser_host() {
    tutor_browser_host_impl(false, true);
}

fn tutor_browser_host_impl(scored: bool, source_only: bool) {
    let (root, mut state) = fixture_with_assets(!source_only);
    if source_only {
        std::fs::write(root.path().join("teaching_readiness.json"), r#"{"status":"failed"}"#).unwrap();
    }
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
    if source_only {
        let mut movement = source_move(&state);
        movement["move"]["move_id"] = json!("one");
        movement["move"]["prompt"] = json!("Choose A for total distance divided by total time.");
        movement["move"]["presentation"] = json!(reference);
        movement["move"]["actions"] = json!(["submit","revise"]);
        movement["move"]["assessment"] = json!({"rule":{"kind":"choice_set","choices":["A","B"],"correct":["A"]},"source_lids":["1.1"],"source_sufficient":true,"feedback":"after_submit"});
        teaching::step(&state.private_context(), turn, movement, &[], &[range()]).unwrap();
    } else if scored {
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
    } else if !source_only {
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
