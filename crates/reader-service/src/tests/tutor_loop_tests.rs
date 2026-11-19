use super::*;
use memory::teaching::TeachingFact;
use runtime::presentation::*;

#[path = "closed_book_tests.rs"]
pub mod closed_book_tests;

#[path = "tutor_example_tests.rs"]
pub mod tool_examples;
#[path = "teacher_corrections_tests.rs"]
pub mod teacher_corrections_tests;

pub fn fixture() -> (tempfile::TempDir, AppState) {
    fixture_with_assets(true)
}

#[test]
fn unified_ordinary_answers_ignore_legacy_controls_and_keep_real_learning_records() {
    for enabled in [false, true] {
        let (_root, mut state) = fixture_with_assets(false);
        let before = state.user.learning_store().unwrap().state().unwrap();
        state.user.learning_store().unwrap().mutate(&serde_json::from_value(json!({
            "operation_id":"legacy-control","expected_revision":before.control.revision,
            "action":{"kind":"set_enabled","enabled":enabled}})).unwrap(), "before-upgrade").unwrap();
        let run = prepare_agent_chat(&mut state, r#"{"message":"Why does the stopped time count?"}"#, "now")
            .unwrap_or_else(|reply| panic!("{}", reply.body));
        let context = run.tutor.as_ref().unwrap();
        assert_eq!(context["status"], "observing");
        assert!(context["current_design"].is_null());
        let binding = context["binding"]["tutor_session_id"].as_str().unwrap().to_owned();
        let turn = run.turn_ref.clone();
        let answer_text = "Elapsed time includes the stop: the clock keeps running even when distance does not increase.";
        let adapter = ChatStubAdapter::scripted(vec![answer(answer_text)]);
        let result = agent_run::execute_prepared(&agent_run::BorrowedAppPort(RefCell::new(&mut state)), &adapter, run, Default::default());
        assert_eq!(result.reply.status, 200, "{}", result.reply.body);
        let store = state.user.learning_store().unwrap();
        let facts = store.teaching_turn_events(&binding, &turn.turn_id).unwrap();
        assert!(!facts.iter().any(|event| matches!(event.kind, TeachingFact::DesignRevised | TeachingFact::MoveSelected)));
        assert!(facts.iter().any(|event| event.learner_expression() == Some("Why does the stopped time count?")));
        assert!(facts.iter().any(|event| event.payload.to_string().contains(answer_text)));
        assert_eq!(store.state().unwrap().sessions.len(), before.sessions.len());
    }
}

#[test]
fn unified_continuation_and_learning_purposes_work_with_a_disabled_legacy_control() {
    let (_root, mut state) = fixture_with_assets(false);
    let view = state.user.learning_store().unwrap().state().unwrap();
    state.user.learning_store().unwrap().mutate(&serde_json::from_value(json!({
        "operation_id":"old-off","expected_revision":view.control.revision,
        "action":{"kind":"set_enabled","enabled":false}})).unwrap(), "before-upgrade").unwrap();
    post(&mut state, "/agent/new", "{}");
    let input = json!({"client_request_id":"understand-off", "session_id":state.workspace.selected_chat, "expected_chat_head":null, "message":"Explain the denominator", "entry_action":"learn_with_purpose", "learning_purpose":"understand",
        "source_id":state.workspace.book.base.book_id,"source_revision":state.workspace.book.source_fingerprint()});
    let run = prepare_agent_chat(&mut state, &input.to_string(), "now").unwrap_or_else(|reply| panic!("{}", reply.body));
    assert_eq!(run.tutor.as_ref().unwrap()["status"], "observing");
    let original = run.turn_ref;
    let mut delivered = outcome(None);
    delivered.answer = Some("The elapsed time includes every part of the outing.".into());
    delivered.answer_view.as_mut().unwrap().parts = vec![AgentAnswerPart::Markdown { text: delivered.answer.clone().unwrap() }];
    finish(&mut state, &original, &delivered);
    let next = json!({"message":"", "entry_action":"continue_answer", "answer_ref":{"turn_id":original.turn_id},
        "session_id":original.session_id,"expected_chat_head":original.turn_id,"client_request_id":"continue-off"});
    let continued = prepare_agent_chat(&mut state, &next.to_string(), "later").unwrap_or_else(|reply| panic!("{}", reply.body));
    assert_eq!(continued.tutor.as_ref().unwrap()["status"], "observing");
    assert_eq!(continued.tutor.as_ref().unwrap()["current_action"]["answer_text"], delivered.answer.unwrap());
    assert!(!continued.tutor.as_ref().unwrap()["current_action"]["source_bindings"].as_array().unwrap().is_empty());
}

#[test]
fn unified_api_rejects_the_removed_global_switch() {
    let (_root, mut state) = fixture_with_assets(false);
    let view = state.user.learning_store().unwrap().state().unwrap();
    let reply = post(&mut state, "/tutor/mutate", &json!({"operation_id":"obsolete-switch", "expected_revision":view.control.revision,
        "action":{"kind":"set_enabled","enabled":false}}).to_string());
    assert_eq!(reply.status, 400);
    assert_eq!(state.user.learning_store().unwrap().state().unwrap(), view);
}

#[test]
fn socratic_dialogue_follows_only_the_question_delivered_in_this_chat() {
    let (_root, mut state) = fixture_with_assets(false);
    let private = tempfile::tempdir().unwrap();
    state.user.store = MemoryStore::open(private.path().join("memory.json")).unwrap();
    let ask = |state: &mut AppState, message: &str| prepare_agent_chat(state,
        &json!({"message":message}).to_string(), "now").unwrap_or_else(|reply| panic!("{}", reply.body));
    let deliver = |state: &mut AppState, run, text| {
        let adapter = ChatStubAdapter::scripted(vec![answer(text)]);
        let result = agent_run::execute_prepared(&agent_run::BorrowedAppPort(RefCell::new(state)), &adapter, run, Default::default());
        assert_eq!(result.reply.status, 200, "{}", result.reply.body);
    };
    let first = ask(&mut state, "Explain elapsed time.");
    assert_eq!(first.tutor.as_ref().unwrap()["status"], "observing");
    let purpose=json!({"focus":"elapsed time","reason":"continue the learner's comparison","learner_inference":"a longer stop increases total time","evidence_to_notice":"count time without extra distance, with the supplied explanation","basis_refs":[]});
    let body=format!("The whole journey includes its stop.\n\n[[socratic]] {}",json!({"question":"What changes if the stop is longer?","purpose":purpose}));
    deliver(&mut state, first, &body);
    // History round-trip uses the persisted answer-view contract.
    state.user.agent_history=serde_json::from_value(serde_json::to_value(&state.user.agent_history).unwrap()).unwrap();
    let session = state.user.agent_history.sessions.last().unwrap();
    let question = "What changes if the stop is longer?";
    assert_eq!(session.turns.last().unwrap().socratic_question(), Some(question));
    let next = prepare_agent_chat(&mut state,&json!({"message":"继续","socratic_dialogue":{"question":"forged question","purpose":{"focus":"forged purpose"}}}).to_string(),"now").unwrap_or_else(|reply|panic!("{}",reply.body));
    assert_eq!(next.tutor.as_ref().unwrap()["socratic_dialogue"]["question"], question);
    assert_eq!(next.tutor.as_ref().unwrap()["socratic_dialogue"]["purpose"],purpose);
    let help=next.tutor.as_ref().unwrap()["socratic_dialogue"]["prior_help"]["answer"].as_str().unwrap();
    assert!(help.contains("The whole journey includes its stop."));
    assert!(!help.contains("learner_inference") && !help.contains("[[socratic]]"));
    assert_eq!(next.message, "继续");
    let next_turn=next.turn_ref.clone();
    let tutor_session=next.tutor.as_ref().unwrap()["binding"]["tutor_session_id"].as_str().unwrap().to_owned();
    deliver(&mut state, next, "Consider the time between leaving and arriving.\n\n[[socratic]] Which interval is longer now?");
    let job=state.user.learning_store().unwrap().coach_job_for_source(&tutor_session,memory::tutor_coach::CoachJobOrigin::AfterTurn,&next_turn.turn_id).unwrap().unwrap();
    assert_eq!(job.input.context["teaching_context"]["socratic_dialogue"]["purpose"],purpose);
    assert_eq!(job.input.context["teaching_context"]["socratic_dialogue"]["question"],question);
    assert!(job.input.context["actual_delivery"]["answer"].as_str().unwrap().contains("Which interval is longer now?"));
    let stop = ask(&mut state, "I included the stop. Let's leave it there.");
    assert_eq!(stop.tutor.as_ref().unwrap()["socratic_dialogue"]["question"], "Which interval is longer now?");
    assert!(stop.tutor.as_ref().unwrap()["socratic_dialogue"]["purpose"].is_null(),"legacy questions must not inherit another question's purpose");
    deliver(&mut state, stop, "You counted the stop as part of the elapsed time.");
    let fresh = ask(&mut state, "Explain a different topic.");
    assert!(fresh.tutor.as_ref().unwrap()["socratic_dialogue"].is_null());
    deliver(&mut state, fresh, "Here is a different comparison.\n\n[[socratic]] What would you compare?");
    post(&mut state, "/agent/new", "{}");
    let other = ask(&mut state, "A new conversation.");
    assert!(other.tutor.unwrap()["socratic_dialogue"].is_null());
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
    assert!(step_with_design(&state.private_context(),&run.turn_ref,original.clone(),&[],&[range()]).is_err(), "observation cannot select a teaching move");
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
    legacy_control(&mut state,json!({"operation_id":"ordinary-on","expected_revision":0,"action":{"kind":"set_enabled","enabled":true}}));
    post(&mut state,"/agent/new","{}");
    let run = prepare_agent_chat(&mut state,r#"{"message":"I understand the denominator now; explain the conditions."}"#,"now").unwrap_or_else(|r|panic!("{}",r.body));
    let context = run.tutor.as_ref().unwrap();
    assert_eq!(context["learner_context"]["interpretations"][0]["evidence_ref"],"interpretation:ordinary");
    let fact = format!("usage:{}:user:{}",context["binding"]["tutor_session_id"].as_str().unwrap(),run.turn_ref.turn_id);
    let changed = json!({"operation":"evidence","nature":"hypothesis","operation_id":"ordinary-revised","target":original["move"]["target"],
        "fact_refs":[fact],"supersedes":"interpretation:ordinary","interpretation":"The question is now about validity conditions.","teaching_implication":"Explain positive total time."});
    let accepted = step_with_design(&state.private_context(),&run.turn_ref,changed,&[],&[range()]).unwrap();
    assert_eq!(accepted["evidence_ref"],"interpretation:ordinary-revised");
    assert!(state.user.learning_store().unwrap().evidence_is_superseded("interpretation:ordinary").unwrap());
}

#[test]
fn tutor_first_question_keeps_the_current_request_without_an_automatic_stage() {
    let (_root, mut state) = fixture_with_assets(false);
    let private = tempfile::tempdir().unwrap();
    state.user.store = MemoryStore::open(private.path().join("memory.json")).unwrap();
    legacy_control(&mut state,json!({"operation_id":"enable","expected_revision":0,"action":{"kind":"set_enabled","enabled":true}}));
    let run = prepare_agent_chat(&mut state, r#"{"message":"Help me understand average speed."}"#, "now").unwrap_or_else(|r|panic!("{}",r.body));
    let context = run.tutor.unwrap();
    assert_eq!(context["status"],"observing");
    assert_eq!(context["current_request"],"Help me understand average speed.");
    assert!(state.user.learning_store().unwrap().state().unwrap().sessions.is_empty());
}

#[test]
fn tutor_source_restoration_drives_the_same_saved_stage_in_ui_and_teaching() {
    let (_root, mut state) = fixture_with_assets(false);
    let source = state.workspace.book.base.book_id.clone();
    let original = state.user.learning_store_for_source(None, &source).unwrap().state().unwrap();
    let original_id = original.control.current_tutor_session_id.clone().unwrap();
    json_post(&mut state, "/tutor/mutate", json!({"operation_id":"other-source", "expected_revision":original.control.revision,
        "action":{"kind":"start", "user_intent":"另一份资料的考试复习", "explicit_constraints":[],
            "material_scope":[{"source_id":"other-book", "scope_refs":[], "role":"primary"}], "default_teaching_intent":null}}));
    let read = route(&mut state, Req { method:"GET", url:"/tutor/state", body:"", now:"now" });
    assert_eq!(read.status, 200);
    let read: Value = serde_json::from_str(&read.body).unwrap();
    assert_eq!(read["control"]["current_tutor_session_id"], original_id);
    assert_eq!(read["sessions"][&original_id]["status"], "paused");
    let continued = teaching::start_request(&state.private_context(), "later").unwrap();
    assert_eq!(continued["started"], true);
    assert_eq!(continued["state"]["control"]["current_tutor_session_id"], original_id);
    let run = prepare(&mut state);
    let teaching = run.tutor.unwrap();
    assert_eq!(teaching["status"], "practice");
    assert_ne!(teaching["binding"]["tutor_session_id"], original_id);
    assert_eq!(teaching["user_intent"], original.sessions[&original_id].user_intent);
    let other = state.user.learning_store_for_source(None, "other-book").unwrap().state().unwrap();
    assert_eq!(other.control.current_tutor_session_id.as_deref(), Some("tutor-other-source"));
}

#[test]
fn tutor_source_first_questions_in_one_cross_material_chat_get_distinct_observation_scopes() {
    let (_root, mut first) = fixture_with_assets(false);
    let mut second_base = sample_base();
    second_base.book_id = "other-source".into();
    let source = "A second source about computing resources. ".repeat(12);
    let (_second_root, second) = fixture_with_book(false, Book::new(second_base, &source), &source);
    let private = tempfile::tempdir().unwrap();
    first.user.store = MemoryStore::open(private.path().join("memory.json")).unwrap();
    legacy_control(&mut first,  json!({"operation_id":"on", "expected_revision":0,
        "action":{"kind":"set_enabled", "enabled":true}}));
    let mut stages = Vec::new();
    for (index, workspace) in [&first.workspace, &second.workspace].into_iter().enumerate() {
        let context = PrivateBookContext { user:&first.user, book:&workspace.book, book_dir:&workspace.book_dir,
            learning_workspace_id:None, messages:&[], selected_chat:None };
        let turn = AgentTurnRef { session_id:"one-cross-material-chat".into(),
            turn_id:format!("turn-{index}"), user_turn_ordinal:index as u64 };
        let prepared = teaching::freeze_prepare(&context, &turn,
            &json!({"message":format!("Explain source {index}")}), None, "now").unwrap();
        let teaching = prepared.context.unwrap();
        assert_eq!(teaching["status"], "observing");
        assert_eq!(teaching["current_request"], format!("Explain source {index}"));
        stages.push(teaching["binding"]["tutor_session_id"].clone());
    }
    assert_ne!(stages[0], stages[1]);
}
pub fn fixture_with_assets(published: bool) -> (tempfile::TempDir, AppState) {
    let source = "Average speed is total distance divided by total time. Equal distances need not take equal times. The total time must be positive. ".repeat(3);
    fixture_with_book(published, Book::new(sample_base(), &source), &source)
}
pub fn fixture_with_book(published: bool, book: Book, source: &str) -> (tempfile::TempDir, AppState) {
    let root = tempfile::tempdir().unwrap();
    let mut state = state_named(&format!(
        "tutor-loop-{}",
        root.path().file_name().unwrap().to_string_lossy()
    ));
    state.workspace.book = Arc::new(book);
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
    state.user.learning_store().unwrap().mutate(&serde_json::from_value(json!({"operation_id":"on","expected_revision":0,"action":{"kind":"set_enabled","enabled":true}})).unwrap(), "now").unwrap();
    assert!(teaching::start_request(&state.private_context(), "now").unwrap()["started"] == true);
    (root, state)
}
pub fn prepare(state: &mut AppState) -> agent_run::PreparedAgentChat {
    prepare_activity_chat(state, r#"{"message":"Practice average speed"}"#, "now")
        .unwrap_or_else(|r| panic!("{}", r.body))
}

/// Formal activity fixtures enter through the same explicit practice action as
/// the reader. Ordinary-question regressions call prepare_agent_chat directly.
pub fn prepare_practice_chat(state: &mut AppState, body: &str, now: &str) -> Result<agent_run::PreparedAgentChat, Reply> {
    let mut input: Value = serde_json::from_str(body).unwrap();
    if input["teaching_ref"].is_null() && input["entry_action"].is_null() {
        if state.workspace.selected_chat.is_none() { post(state, "/agent/new", "{}"); }
        input["client_request_id"] = json!(uuid::Uuid::now_v7().to_string());
        input["session_id"] = json!(state.workspace.selected_chat);
        input["expected_chat_head"] = json!(state.user.agent_history.sessions.iter()
            .find(|session| Some(&session.id) == state.workspace.selected_chat.as_ref()).and_then(|session| session.turns.last()).map(|turn| &turn.turn_id));
        input["entry_action"] = json!("learn_with_purpose");
        input["learning_purpose"] = json!("practice");
        input["source_id"] = json!(state.workspace.book.base.book_id);
        input["source_revision"] = json!(state.workspace.book.source_fingerprint());
    }
    prepare_agent_chat(state, &input.to_string(), now)
}

/// Activity contract tests explicitly request practice on each new turn. The
/// conversation retains its practice design without fabricating a question reply.
pub fn prepare_activity_chat(state: &mut AppState, body: &str, now: &str) -> Result<agent_run::PreparedAgentChat, Reply> {
    prepare_practice_chat(state, body, now)
}

/// Replay the stored setting from a pre-upgrade learner database. The removed
/// public API is tested separately and must continue rejecting this action.
pub fn legacy_control(state: &mut AppState, body: Value) -> Value {
    serde_json::to_value(state.user.learning_store().unwrap()
        .mutate(&serde_json::from_value(body).unwrap(), "before-upgrade").unwrap()).unwrap()
}
pub fn range() -> EvidenceRange {
    EvidenceRange {
        start_lid: "1.1".into(),
        end_lid: "1.1".into(),
        ranges: vec![],
    }
}

/// Existing storage/assessment fixtures explicitly describe their intended participation.
pub fn fixture_participation(movement:&mut Value) {
    if movement["delivery_text"].is_null() { movement["delivery_text"]=movement["prompt"].clone(); }
    if !movement["participation"].is_null() { return; }
    let invite=movement["response_expectation"].is_object() || movement["assessment"].is_object()
        || movement["actions"].as_array().into_iter().flatten().any(|action|action=="submit");
    movement["participation"]=if invite {json!({"choice":"invite","reason":"The fixture exercises the learner response to this activity.",
        "learner_inference":"Apply the displayed relationship to the requested comparison.","support":"The displayed source and activity supply the comparison quantities."})}
        else {json!({"choice":"support","reason":"The fixture delivers an explanation without requesting a new response.",
            "support":"The complete journey includes stopped time as well as moving time.","learner_inference":null})};
}

/// The automatic coach must not consume the Resident's scripted chat turns.
/// This fixture leaves source knowledge unknown and supplies no learner diagnosis.
pub fn fixture_coach_guidance(request:&CompletionRequest) -> Option<Value> {
    if !request.system.starts_with("你整理 Understand Book 普通对话中已经发生的理解变化。") { return None; }
    let input:Value=serde_json::from_str(&request.user).unwrap();
    Some(json!({"observations":input["observation_facts"].as_array().into_iter().flatten()
        .filter(|fact|fact["already_interpreted"]!=true).map(|fact|json!({"fact_ref":fact["fact_ref"],
            "decision":"no_new_information","reason":"This fixture tests dispatch, not learner interpretation."})).collect::<Vec<_>>(),
        "reason":"This scripted fixture supplies no source or learner-understanding judgment."}))
}

/// Scripted storage/dispatch fixtures do not evaluate teaching content. The
/// acceptance gate itself is exercised with rejecting/failing runtime adapters;
/// live teaching adapters continue to send this request to the real model.
pub fn fixture_delivery_review(request:&CompletionRequest) -> Option<Value> {
    let explanation = request.system.contains("Teaching explanation acceptance review.");
    if !explanation && !request.system.contains("Teaching delivery acceptance review.") { return None; }
    let packet:Value=serde_json::from_str(&request.user).unwrap();
    let question=if explanation { packet["current_request"].as_str().unwrap() } else {
        [&packet["frozen_input"]["current_request"],&packet["frozen_input"]["learner_request"],
            &packet["frozen_input"]["current_action"]["accepted_request"],&packet["original_request"]]
            .into_iter().filter_map(Value::as_str).find(|text| !text.trim().is_empty()).unwrap()
    };
    let delivered=if explanation { packet["current_delivery"].as_str().unwrap() }
        else { packet["proposed_delivery"]["text"].as_str().unwrap() };
    // These deterministic fixtures exercise dispatch and persistence; live
    // adapters still produce their own review through TutorRequestRecorder.
    Some(json!({"decision":"accept","revision":null,"learner_claims":[],"findings":[],
        "request_resolution":{"status":"answered","question_quote":question,
            "requested_relation":format!("Fixture request: {question}"),"delivered_relation":delivered,
            "answer_quotes":[delivered],"reason":"The fixture supplies its requested comparison."},
        "agent_work":{"understanding":[],"open_questions":[]}}))
}

/// Existing storage/assessment fixtures establish the real design precondition.
pub fn step_with_design(state:&PrivateBookContext<'_>,turn:&AgentTurnRef,mut request:Value,evidence:&[SourceBinding],ranges:&[EvidenceRange]) -> Result<Value,read_tools::ToolError> {
    if request["operation"] == "select" && request["move"]["kind"] == "question"
        && request["move"]["assessment"].is_object() && request["move"]["practice"].is_null() {
        let binding = state.user.learning_store()?.teaching_event(&format!("binding:{}",turn.turn_id))?;
        if serde_json::from_value::<runtime::tutor_continuation::ResolvedInteraction>(binding.payload["current_action"].clone())
            .is_ok_and(|action| action.requests_practice()) {
            let hint = request["move"].as_object_mut().unwrap().remove("hint")
                .unwrap_or_else(|| json!("Compare the quantities in the problem."));
            request["move"]["practice"] = json!({"entry":"warmup","mode":"recall","distance":"adjacent",
                "hints":[{"kind":"structure","text":hint}]});
        }
    }
    if request["operation"]=="select" { fixture_participation(&mut request["move"]); }
    if request["operation"]=="select" && request["move"]["design_ref"].is_null() {
        let store=state.user.learning_store()?;
        let binding=store.teaching_event(&format!("binding:{}",turn.turn_id))?.binding;
        let current=store.current_teaching_design(&binding.tutor_session_id,&binding.source_id,&binding.source_revision)?;
        let previous_selected=store.teaching_turn_events(&binding.tutor_session_id,&turn.turn_id)?.into_iter().rev().find(|e|e.kind==TeachingFact::MoveSelected);
        if request["move"]["target"].is_null() {
            let mut target=super::teaching_design_tests::target();
            target["capability"]=request["move"]["capability"].clone();
            let mut objects=Vec::new();
            for id in request["move"]["object_ids"].as_array().into_iter().flatten().filter_map(Value::as_str) {
                let material=store.teaching_event(&format!("material:{}:{id}",turn.turn_id))?;
                objects.push(json!({"source_id":binding.source_id,"object_id":id,"object_revision":material.payload["object"]["object_revision"]}));
            }
            target["object_refs"]=json!(objects);
            request["move"]["target"]=target;
        }
        drop(store);
        let design_ref=if let Some(current)=current {current.event_id} else {
            let mut design=super::teaching_design_tests::design_request("fixture-design",None);
            if request["move"]["target"].is_object() {design["design"]["target"]=request["move"]["target"].clone();}
            design["design"]["blocks"][0]["source_bindings"]=design["design"]["target"]["source_bindings"].clone();
            teaching::step(state,turn,design,evidence,ranges)?["design_ref"].as_str().unwrap().to_owned()
        };
        request["move"]["design_ref"]=json!(design_ref);
        request["move"]["block_refs"]=json!(["total-time"]);
        if let Some(previous)=previous_selected {request["delivery_move_refs"]=previous.payload["delivery_move_refs"].clone();}
    }
    teaching::step(state,turn,request,evidence,ranges)
}
pub fn select(state: &AppState, turn: &AgentTurnRef, id: &str, reference: Option<&PresentationRef>) {
    step_with_design(&state.private_context(),
        turn,
        json!({"operation":"material","object_id":"speed"}),
        &[],
        &[],
    )
    .unwrap();
    step_with_design(&state.private_context(),turn,json!({"operation":"select","move":{"move_id":id,"object_ids":["speed"],"capability":"explanation","kind":"observe","interaction_intent":"neutral","intent_origin":"neutral","prompt":format!("Observe {id}"),
        "actions":["submit","revise","request_hint","reveal","skip","self_report"],"hint":"Compare the two times.","explanation":"Use total distance divided by total time.","presentation":reference}}),&[],&[range()]).unwrap();
}
pub fn outcome(reference: Option<&PresentationRef>) -> OuterOutcome {
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
        teaching_delivery_move_refs: vec![],
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
pub fn finish(state: &mut AppState, turn: &AgentTurnRef, result: &OuterOutcome) {
    let mut result=result.clone();
    if result.teaching_delivery_move_refs.is_empty() {
        let store=state.user.learning_store().unwrap();
        if let Ok(bind)=store.teaching_event(&format!("binding:{}",turn.turn_id)) {
            if let Some(selected)=store.teaching_turn_events(&bind.binding.tutor_session_id,&turn.turn_id).unwrap().into_iter().rev().find(|e|e.kind==TeachingFact::MoveSelected) {
                result.teaching_delivery_move_refs=serde_json::from_value(selected.payload["delivery_move_refs"].clone()).unwrap_or_default();
            }
        }
    }
    finalize_agent_turn_completed(state, turn, &result, &state.workspace.messages.clone(), "now").unwrap();
    teaching::record_delivery(&state.private_context(), turn, "now").unwrap();
}
pub fn json_post(state: &mut AppState, path: &str, body: Value) -> Value {
    let reply = post(state, path, &body.to_string());
    assert_eq!(reply.status, 200, "{}", reply.body);
    serde_json::from_str(&reply.body).unwrap()
}

pub fn assessed_activity(
    state: &mut AppState,
    rule: Value,
    feedback: &str,
) -> (AgentTurnRef, String, Value) {
    let run = prepare(state);
    let turn = run.turn_ref;
    step_with_design(&state.private_context(),
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
    step_with_design(&state.private_context(), &turn, proposal.clone(), &[], &[range()]).unwrap();
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
    assert!(step_with_design(&state.private_context(), &turn, proposal, &[], &[range()]).is_err());
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

pub fn open_rule() -> Value {
    json!({"kind":"open","items":[{"id":"distance","criterion":"Numerator is total distance; accept equivalent wording","source_lids":["1.1"]},{"id":"time","criterion":"Denominator is total time; accept equivalent wording","source_lids":["1.1"]}]})
}
pub fn open_items() -> Value {
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
    let samples = Arc::new(std::sync::Mutex::new(Vec::new()));
    state.services.adapter = Box::new(super::tutor_continuation_tests::TutorRequestRecorder {
        samples:samples.clone(), directory:None, inner:Box::new(ChatStubAdapter::scripted(vec![
        call(
            "tutor.step",
            json!({"operation":"assess","action_ref":action["event_id"]}),
        ),
        answer(&json!({"items":open_items()}).to_string()),
        call("tutor.step", json!({"operation":"outside"})),
        answer("Your response meets the criteria."),
    ])) });
    let result = json_post(
        &mut state,
        "/agent/chat",
        json!({"message":"Please evaluate my response","teaching_ref":action["event_id"]}),
    );
    assert_eq!(result["tokens_spent"], 20);
    let sampled = samples.lock().unwrap();
    assert!(sampled[0]["instruction_assets"].as_array().unwrap().iter().any(|a| a["asset_id"] == "resident-agent.tutor.assessment"));
    assert!(sampled[0].to_string().contains("action:open"));
    assert!(!sampled[2]["instruction_assets"].as_array().unwrap().iter().any(|a| a["asset_id"] == "resident-agent.tutor.assessment"), "completed assessment expires at the next sampling");
    let examples = |index: usize| sampled[index]["tools"].as_array().unwrap().iter()
        .find(|t| t["name"] == "tutor.step").unwrap()["parameters"]["examples"].as_array().unwrap();
    assert!(!examples(0).iter().any(|e| e["operation"] == "assess"), "fact-dependent examples do not change the stable tool schema");
    let learner:Value=sampled[0]["input"].as_array().unwrap().iter()
        .filter_map(|message|message["content"].as_str()).flat_map(str::lines)
        .find_map(|line|line.starts_with("Current learner view (")
            .then(||serde_json::from_str(line.split_once(": ").unwrap().1).unwrap()))
        .expect("the current learner view must supply the pending response identity");
    assert!(learner["pending_assessment_refs"].as_array().unwrap().iter().any(|id|id=="action:open"));
    assert!(learner["tool_input_examples"]["examples"].as_array().unwrap().iter()
        .any(|e| e["operation"] == "assess" && e["action_ref"] == "<pending_assessment_action_ref>"));
    assert!(!examples(2).iter().any(|e| e["operation"] == "assess"), "completed assessment example expires with its instruction");
    drop(sampled);
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
    let expressions=context["learner_context"]["learner_grounding"]["expressions"].as_array().unwrap();
    let correction=expressions.iter().find(|expression|expression["evidence_ref"]==corrected["evidence_ref"]).unwrap();
    assert_eq!(correction["kind"],"learner_correction");
    assert_eq!(correction["text"],"I copied an earlier worked example");
    assert!(correction.get("fact_ref").is_none(),"a correction is original user text but not a teaching-event id");
    assert_eq!(std::fs::read(&map_path).unwrap(), original);
    let replay = json_post(&mut state, "/tutor/understanding", json!({"rebuild":true}));
    assert_eq!(replay, new_view);
}

#[test]
fn tutor_ungraded_discrimination_is_referenced_and_self_report_remains_subjective() {
    let (_root, mut state) = fixture();
    let run = prepare(&mut state);
    let turn = &run.turn_ref;
    step_with_design(&state.private_context(),
        turn,
        json!({"operation":"material","object_id":"speed"}),
        &[],
        &[],
    )
    .unwrap();
    step_with_design(&state.private_context(),turn,json!({"operation":"select","move":{"move_id":"discriminate","object_ids":["speed"],"capability":"evaluation","kind":"source_focus","interaction_intent":"neutral","intent_origin":"neutral","prompt":"Which quantities does this source support?","actions":["submit","revise","self_report"]}}),&[],&[range()]).unwrap();
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
    let accepted=step_with_design(&state.private_context(),turn,json!({"operation":"evidence","operation_id":"discrimination-evidence","action_ref":"action:discrimination","object_id":"speed","response_quote":"The source supports total distance","sources":[{"lid":"1.1","quote":"total distance"}],"interpretation":"The learner distinguished a source-supported quantity from an unsupported property."}),&[],&[]).unwrap();
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
    let run=prepare_activity_chat(&mut state,r#"{"message":"Teach average speed"}"#,"now").unwrap_or_else(|e|panic!("{}",e.body));
    let owner=run.tutor.as_ref().unwrap()["binding"]["tutor_session_id"].as_str().unwrap().to_owned();
    let (design,movement)=planned_move(&run,json!({"operation":"select","move":{"move_id":"private","object_ids":["speed"],"capability":"explanation","kind":"question","interaction_intent":"neutral","intent_origin":"neutral","prompt":"Choose A or B","actions":["submit","revise","request_hint","reveal","skip"],"explanation":"A is the required quantity.","practice":{"entry":"warmup","mode":"recall","distance":"adjacent","hints":[{"kind":"structure","text":"PRIVATE_HINT"}]},"assessment":{"rule":{"kind":"choice_set","choices":["A","B"],"correct":["A"]},"source_lids":["1.1"],"source_sufficient":true,"feedback":"on_reveal"}}}));
    let result=execute_script(&mut state,run,vec![
        call(
            "tutor.step",
            json!({"operation":"material","object_id":"speed"}),
        ),
        call("book.text", json!({"lid":"1.1","end_lid":"1.1"})),
        design,call("tutor.step",movement),
        answer("Choose A or B"),
    ]);
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
        .teaching_turn_events(&owner, &turn.turn_id)
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
    teaching::assessment_accept(&state.private_context(), &run.turn_ref, "action:feedback-revise", open_items())
        .unwrap();
    let timeline=json_post(&mut state,"/tutor/timeline",json!({"evidence_ref":"assessment:action:feedback-revise:speed"}));
    let revised=timeline["entries"].as_array().unwrap().iter()
        .find(|entry|entry["action_ref"]=="action:feedback-revise").unwrap();
    assert_eq!(revised["assistance_count"],1);
    assert_eq!(revised["assistance"][0]["fact_ref"],"feedback-seen:action:feedback-first");
    assert_eq!(revised["assistance"][0]["kind"],"help_displayed");
    assert_eq!(revised["assistance"][0]["text"],
        "The response names the total distance.\nThe response names total elapsed time.",
        "the timeline resolves the exact feedback shown before this revision, rather than claiming its text was not retained");
    let first=timeline["entries"].as_array().unwrap().iter()
        .find(|entry|entry["action_ref"]=="action:feedback-first").unwrap();
    assert!(first["assistance"].as_array().unwrap().is_empty(),
        "feedback shown later must not become help received before the original response");
}

#[test]
fn tutor_old_activity_and_object_revision_remain_bound_after_new_map() {
    let (root, mut state) = fixture();
    let (_, old_id, mut proposal) = assessed_activity(
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
    post(&mut state, "/agent/new", "{}");
    let new=prepare(&mut state);
    step_with_design(&state.private_context(),&new.turn_ref,json!({"operation":"material","object_id":"speed"}),&[],&[]).unwrap();
    proposal["move"]["prompt"]=json!("Which duration is valid for this calculation: A zero; B negative; C positive?");
    proposal["move"]["explanation"]=json!("The elapsed duration must be positive.");
    proposal["move"]["assessment"]["rule"]["correct"]=json!(["C"]);
    let mut target=super::teaching_design_tests::target();
    target["learning_focus"]=json!("positive duration condition");
    target["expected_performance"]=json!("Distinguish a valid positive duration from zero or negative duration.");
    target["object_refs"]=json!([{"source_id":state.workspace.book.base.book_id,"object_id":"speed","object_revision":2}]);
    proposal["move"]["target"]=target;
    step_with_design(&state.private_context(),&new.turn_ref,proposal,&[],&[range()]).unwrap();
    finish(&mut state,&new.turn_ref,&outcome(None));
    let new_id=format!("delivery:{}:scored",new.turn_ref.turn_id);
    json_post(&mut state,"/tutor/display",json!({"delivery_ref":new_id}));
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
            vec![],
        )
        .unwrap();
        let parsed: Value = serde_json::from_str(result["text"].as_str().unwrap()).unwrap();
        let items: Vec<ItemAssessment> = serde_json::from_value(parsed["items"].clone()).unwrap();
        let status = accept_open(&contract, response, &sources, &BTreeMap::new(), &items).unwrap();
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
    let mut recovery_outcome=outcome(None);
    recovery_outcome.teaching_delivery_move_refs=vec![format!("move:{}:recovery",recovery.turn_ref.turn_id)];
    finalize_agent_turn_completed(&mut state, &recovery.turn_ref, &recovery_outcome, &[], "now")
        .unwrap();
    let revision = state
        .user.learning_store()
        .unwrap()
        .state()
        .unwrap()
        .control
        .revision;
    legacy_control(
        &mut state,
        
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
            PresentationContent { action_contract: vec![], teaching_binding: None,
                method_refs: None, method_observations: Default::default(),
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
                observation: None,
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
    legacy_control(
        &mut state,
        
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
    assert_eq!(rejected.status, 200, "{}", rejected.body);
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
    legacy_control(
        &mut state,
        
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

pub fn fixture_model_input(mut arguments:Value)->Value {
    if arguments["operation"] == "select" {
        arguments["move"].as_object_mut().unwrap().remove("capability");
        if arguments["move"]["intent_origin"] == "session_default" {
            arguments["move"]["interaction_intent"]=json!("neutral");
            arguments["move"]["intent_origin"]=json!("neutral");
            arguments["move"].as_object_mut().unwrap().remove("intent_quote");
        }
    }
    arguments
}
pub fn call(name: &str, arguments: Value) -> AssistantTurn {
    AssistantTurn {
        provider_continuation: None,
        text: None,
        tool_calls: vec![ToolCall {
            id: format!("call-{name}"),
            name: name.into(),
            arguments: fixture_model_input(arguments).to_string(),
        }],
        usage_total_tokens: Some(5),
    }
}
pub fn answer(text: &str) -> AssistantTurn {
    AssistantTurn {
        provider_continuation: None,
        text: Some(text.into()),
        tool_calls: vec![],
        usage_total_tokens: Some(5),
    }
}
pub fn planned_move(run:&agent_run::PreparedAgentChat,mut movement:Value) -> (AssistantTurn,Value) {
    fixture_participation(&mut movement["move"]);
    let context=run.tutor.as_ref().unwrap();
    let previous=context["current_design"]["design_ref"].as_str();
    let mut design=super::teaching_design_tests::design_request("script-design",previous);
    if movement["move"]["target"].is_null() {
        let mut target=super::teaching_design_tests::target();
        target["capability"]=movement["move"]["capability"].clone();
        target["object_refs"]=json!(movement["move"]["object_ids"].as_array().into_iter().flatten()
            .map(|id|json!({"source_id":context["binding"]["source_id"],"object_id":id,"object_revision":1})).collect::<Vec<_>>());
        movement["move"]["target"]=target;
    }
    design["design"]["target"]=movement["move"]["target"].clone();
    design["design"]["blocks"][0]["source_bindings"]=movement["move"]["target"]["source_bindings"].clone();
    movement["move"]["design_ref"]=json!(format!("design:{}:script-design",run.turn_ref.turn_id));
    movement["move"]["block_refs"]=json!(["total-time"]);
    (call("tutor.step",design),movement)
}
pub fn execute_script(state:&mut AppState,run:agent_run::PreparedAgentChat,script:Vec<AssistantTurn>) -> Value {
    let result=agent_run::execute_prepared(&agent_run::BorrowedAppPort(RefCell::new(state)),&ChatStubAdapter::scripted(script),run,Default::default());
    assert_eq!(result.reply.status,200,"{}",result.reply.body);
    serde_json::from_str(&result.reply.body).unwrap()
}
pub fn script(run:&agent_run::PreparedAgentChat,kind: &str) -> Vec<AssistantTurn> {
    let (design,movement)=planned_move(run,json!({"operation":"select","move":{"move_id":"next","object_ids":["speed"],"capability":"explanation","kind":kind,"interaction_intent":if kind=="explain" {"direct_explanation"} else {"neutral"},"intent_origin":if kind=="explain" {"current_request"} else {"neutral"},"intent_quote":if kind=="explain" {Some("directly explain this time")} else {None},"prompt":"Compare total distance with total time.","actions":[]}}));
    vec![
        call(
            "tutor.step",
            json!({"operation":"material","object_id":"speed"}),
        ),
        call("book.text", json!({"lid":"1.1","end_lid":"1.1"})),
        design,call("tutor.step",movement),
        answer("Average speed uses total distance divided by total time."),
    ]
}

pub fn source_move(state: &AppState) -> Value {
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
    legacy_control(&mut state,  json!({"operation_id":"off-usage","expected_revision":revision,"action":{"kind":"set_enabled","enabled":false}}));
    state.workspace.reader.goto_lid(&state.workspace.book, &mut state.user.store, "1.1", "read-while-off").unwrap();
    state.user.store.flush_pending_reads().unwrap();
    state.services.adapter = Box::new(ChatStubAdapter::scripted(vec![answer("Use total time.")]));
    json_post(&mut state, "/agent/chat", json!({"message":"Why total time?"}));
    assert!(!state.user.learning_store().unwrap().state().unwrap().control.enabled);
    state.user.store = MemoryStore::open(root.path().join("memory.json")).unwrap();
    let revision = state.user.learning_store().unwrap().state().unwrap().control.revision;
    legacy_control(&mut state,  json!({"operation_id":"on-usage","expected_revision":revision,"action":{"kind":"set_enabled","enabled":true}}));
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
    let read = step_with_design(&state.private_context(), &run.turn_ref,json!({"operation":"trace","event_id":question.event_id}),&[],&[]).unwrap();
    assert!(read["chunk"].as_str().unwrap().contains("Why total time?"));
    assert!(store.evidence_page(&state.workspace.book.base.book_id,0,100).unwrap().is_empty());
}

#[test]
fn tutor_t16_source_only_resident_delivers_and_continues_after_reopen() {
    let (root, mut state) = fixture_with_assets(false);
    let movement = source_move(&state);
    let run=prepare_activity_chat(&mut state,&json!({"message":"I just opened this book and have not read it. Explain average speed."}).to_string(),"now").unwrap_or_else(|e|panic!("{}",e.body));
    let (design,first_move)=planned_move(&run,movement.clone());
    let first=execute_script(&mut state,run,vec![
        call("book.text", json!({"lid":"1.1","end_lid":"1.1"})),
        design,call("tutor.step", first_move),
        answer("Average speed uses total distance divided by total time."),
    ]);
    assert_eq!(first["incomplete"], false, "{first}");
    let session = state.user.agent_history.sessions.last().unwrap();
    let turn = session.turns.last().unwrap();
    let store = memory::learning::LearningStore::open(&root.path().join("learning.db"), false).unwrap();
    let rows = store.teaching_deliveries(&session.id, &turn.turn_id).unwrap();
    assert_eq!(rows.len(), 1, "{first}");
    assert!(rows[0].binding.map_revision.is_none());
    let mut delivered_target=rows[0].payload["move"]["target"].clone();
    assert!(delivered_target.as_object_mut().unwrap().remove("focus_ref").is_some());
    assert_eq!(delivered_target, movement["move"]["target"]);
    let binding = store.teaching_event(&format!("binding:{}", turn.turn_id)).unwrap();
    assert_eq!(binding.payload["learner_context"]["confirmed_background"], json!([]));
    assert!(binding.payload["learner_context"]["current_request"].as_str().unwrap().contains("have not read"));
    assert_eq!(binding.payload["learner_context"]["entries"], json!([]));
    drop(store);
    let activity_request = json!({"session_id":session.id,"turn_id":turn.turn_id});
    let activities = json_post(&mut state, "/tutor/activities", activity_request);
    assert_eq!(activities["activities"][0]["status"], "active", "{activities}");
    post(&mut state, "/agent/new", "{}");
    let run=prepare_activity_chat(&mut state,&json!({"message":"Why total time?","teaching_ref":rows[0].event_id}).to_string(),"now").unwrap_or_else(|e|panic!("{}",e.body));
    let second=execute_script(&mut state,run,vec![
        call("book.text", json!({"lid":"1.1","end_lid":"1.1"})),
        call("tutor.step", json!({"operation":"trace","event_id":rows[0].event_id})),
        answer("The denominator is the total time."),
    ]);
    assert_eq!(second["incomplete"], false, "{second}");
    let session = state.user.agent_history.sessions.last().unwrap();
    let turn = session.turns.last().unwrap();
    let binding = state.user.learning_store().unwrap().teaching_event(&format!("binding:{}", turn.turn_id)).unwrap();
    assert_eq!(binding.payload["learner_context"]["current_target"], rows[0].payload["move"]["target"]);
    assert!(!binding.payload["learner_context"]["recent_facts"].as_array().unwrap().is_empty());
    let revision = state.user.learning_store().unwrap().state().unwrap().control.revision;
    legacy_control(&mut state,  json!({"operation_id":"off","expected_revision":revision,"action":{"kind":"set_enabled","enabled":false}}));
    assert_eq!(prepare_agent_chat(&mut state,r#"{"message":"Explain another topic"}"#,"later").unwrap().tutor.unwrap()["status"], "observing");
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
        let samples = Arc::new(std::sync::Mutex::new(Vec::new()));
        let adapter = super::tutor_continuation_tests::TutorRequestRecorder { samples:samples.clone(), directory:None,
            inner:Box::new(ChatStubAdapter::scripted(vec![
            call("book.text",json!({"lid":"1.1","end_lid":"1.1"})), call("tutor.step",request),
            answer(explanation),
        ])) };
        let turn = run.turn_ref.clone();
        let result = agent_run::execute_prepared(&agent_run::BorrowedAppPort(RefCell::new(&mut state)), &adapter, run, Default::default());
        assert_eq!(result.reply.status,200,"{}",result.reply.body);
        // Understanding guidance is loaded when the operation is requested;
        // ordinary first sampling no longer preloads the interpretation workflow.
        assert!(!samples.lock().unwrap()[0].to_string().contains("resident-agent.tutor.understanding"));
        assert!(samples.lock().unwrap().iter().skip(1).any(|sample| sample.to_string().contains("resident-agent.tutor.understanding")));
        let store = memory::learning::LearningStore::open(&root.path().join("learning.db"),false).unwrap();
        let accepted = store.evidence(&evidence_id).unwrap_or_else(|e|panic!("{e:?}; {}",result.reply.body));
        assert_eq!(accepted.learner_quote,message);
        assert!(accepted.assessment_ref.is_none());
        let facts=store.teaching_turn_events(binding["tutor_session_id"].as_str().unwrap(),&turn.turn_id).unwrap();
        assert!(facts.iter().any(|event|event.kind==TeachingFact::UsageObserved && event.payload.to_string().contains(explanation)));
        assert!(!facts.iter().any(|event|matches!(event.kind,TeachingFact::DesignRevised|TeachingFact::MoveSelected)));
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
    assert!(step_with_design(&state.private_context(),&run.turn_ref,stale,&[],&[range()]).is_err());
    let bad = json!({"operation":"evidence","nature":"hypothesis","operation_id":"bad","target":original["move"]["target"],
        "fact_refs":[format!("binding:{}",run.turn_ref.turn_id)],"interpretation":"invented","teaching_implication":"invented"});
    assert!(step_with_design(&state.private_context(),&run.turn_ref,bad,&[],&[range()]).is_err());
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
        PresentationContent { action_contract: vec![], teaching_binding: None, method_refs: None, method_observations: Default::default(), animation_assets:Default::default(),title:"Journey".into(),
            content_files:BTreeMap::from([("index.html".into(),"<p>Journey</p>".into())]),entrypoint:"index.html".into(),
            readable_content:"Watch the total elapsed time.".into(),source_bindings:vec![],assumptions:vec![],state_contract:json!({}),initial_state:json!({"time":2}) }).unwrap();
    let reference=state.private_context().persist_presentation_candidate(&run.turn_ref.session_id,&run.turn_ref.turn_id,&candidate.candidate_id).unwrap();
    let mut movement=source_move(&state); movement["move"]["presentation"]=json!(reference);
    step_with_design(&state.private_context(),&run.turn_ref,movement,&[],&[range()]).unwrap();
    assert!(step_with_design(&state.private_context(),&run.turn_ref,json!({"operation":"trace","event_id":format!("move:{}:source",run.turn_ref.turn_id)}),&[],&[]).is_err());
    let binding=run.tutor.as_ref().unwrap()["binding"].clone();
    let usage=format!("usage:{}:assistant:{}",binding["tutor_session_id"].as_str().unwrap(),run.turn_ref.turn_id);
    assert!(state.user.learning_store().unwrap().teaching_event(&usage).is_err());
    finish(&mut state,&run.turn_ref,&outcome(Some(&reference)));
    let read=step_with_design(&state.private_context(),&run.turn_ref,json!({"operation":"trace","event_id":usage}),&[],&[]).unwrap();
    assert!(read["chunk"].as_str().unwrap().contains("Watch the total elapsed time."));
    assert!(!read["chunk"].as_str().unwrap().contains("content_files"));
    let scene=state.private_context().save_presentation_state(&run.turn_ref.session_id,&run.turn_ref.turn_id,&reference,
        PresentationState{observation:None,values:json!({"time":4}),visible_step:Some("waiting".into()),observed_result:"four seconds".into(),source_ref_ids:vec![]}).unwrap();
    let next=prepare_agent_chat(&mut state,&json!({"message":"Why four seconds?","presentation_follow_up":scene}).to_string(),"later").unwrap_or_else(|r|panic!("{}",r.body));
    let usage=format!("usage:{}:user:{}",next.tutor.as_ref().unwrap()["binding"]["tutor_session_id"].as_str().unwrap(),next.turn_ref.turn_id);
    let read=step_with_design(&state.private_context(),&next.turn_ref,json!({"operation":"trace","event_id":usage}),&[],&[]).unwrap();
    assert!(read["chunk"].as_str().unwrap().contains("four seconds"));
    assert!(read["chunk"].as_str().unwrap().contains("waiting"));
}

#[test]
fn tutor_t17c_source_only_assessments_keep_help_and_reopen() {
    let (root, mut state) = fixture_with_assets(false);
    let run = prepare(&mut state);
    let mut movement = source_move(&state);
    movement["move"]["kind"]=json!("question");
    movement["move"]["actions"]=json!(["submit","revise","request_hint","reveal","skip"]);
    movement["move"]["prompt"]=json!("A traveler covers 10 km in 15 minutes. Which denominator includes the whole journey: A all elapsed time; B only moving time?");
    movement["move"]["explanation"]=json!("A includes every elapsed interval of the journey.");
    movement["move"]["hint"]=json!("Think about all elapsed time.");
    movement["move"]["assessment"]=json!({"rule":{"kind":"choice_set","choices":["A","B"],"correct":["A"]},"source_lids":["1.1"],"source_sufficient":true,"feedback":"after_submit"});
    step_with_design(&state.private_context(),&run.turn_ref,movement,&[],&[range()]).unwrap();
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
    let next_run=prepare(&mut state);
    let next=next_run.tutor.as_ref().unwrap();
    assert_eq!(next["learner_context"]["interpretations"].as_array().unwrap().len(),2);
    finish(&mut state,&next_run.turn_ref,&outcome(None));
    post(&mut state,"/agent/new","{}");
    // Open rubric must also run without a teaching map, and failed evaluation retains the response.
    let run=prepare(&mut state);
    let mut movement=source_move(&state);
    movement["move"]["actions"]=json!(["submit"]);
    movement["move"]["assessment"]=json!({"rule":open_rule(),"source_lids":["1.1"],"source_sufficient":true,"feedback":"after_submit"});
    step_with_design(&state.private_context(),&run.turn_ref,movement,&[],&[range()]).unwrap(); finish(&mut state,&run.turn_ref,&outcome(None));
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
fn tutor_target_coverage_accepts_split_paragraphs_but_rejects_unread_text() {
    for (gap, accepted) in [("\n\n", true), ("XY", false)] {
        let source = format!("{}{}{}", "A".repeat(48), gap, "B".repeat(50));
        let mut base = sample_base();
        base.lid_nodes[0].children.push("1.2".into());
        let mut second = base.lid_nodes[1].clone();
        second.lid = "1.2".into();
        second.path = vec![1, 2];
        second.span.start = 50;
        base.lid_nodes[1].span.end = 48;
        base.lid_nodes.push(second);
        let (_root, mut state) = fixture_with_book(false, Book::new(base, &source), &source);
        let run = prepare(&mut state);
        let mut movement = source_move(&state);
        movement["move"]["target"]["source_bindings"][0]["end_lid"] = json!("1.2");
        let reads = [range(), EvidenceRange { start_lid: "1.2".into(), end_lid: "1.2".into(), ranges: vec![] }];
        let result = step_with_design(&state.private_context(), &run.turn_ref, movement, &[], &reads);
        assert_eq!(result.is_ok(), accepted, "gap {gap:?}: {result:?}");
    }
}

#[test]
fn tutor_target_binds_current_source_without_model_copying_revision() {
    let (_root, mut state) = fixture_with_assets(false);
    let mut movement = source_move(&state);
    let source = movement["move"]["target"]["source_bindings"][0].as_object_mut().unwrap();
    source.remove("source_id");
    source.remove("source_revision");
    let run=prepare(&mut state);
    let (design,movement)=planned_move(&run,movement);
    let reply=execute_script(&mut state,run,vec![
        call("book.text", json!({"lid":"1.1"})),design,call("tutor.step", movement), answer("Use total time."),
    ]);
    assert_eq!(reply["incomplete"], false, "{reply}");
    let session = state.user.agent_history.sessions.last().unwrap();
    let turn = session.turns.last().unwrap();
    let deliveries = state.user.learning_store().unwrap().teaching_deliveries(&session.id, &turn.turn_id).unwrap();
    assert_eq!(deliveries.len(), 1, "{reply}");
    assert_eq!(deliveries[0].payload["move"]["target"]["source_bindings"][0]["source_revision"], state.workspace.book.source_fingerprint());
}

#[test]
fn tutor_target_reports_revision_mismatch_before_interpretation_scope() {
    let (_root, mut state) = fixture_with_assets(false);
    let run = prepare(&mut state);
    let mut movement = source_move(&state);
    movement["move"]["target"]["source_bindings"][0]["source_revision"] = json!("search-index-revision");
    movement["move"]["interpretation_refs"] = json!(["interpretation:unread"]);
    let error = step_with_design(&state.private_context(), &run.turn_ref, movement, &[], &[range()]).unwrap_err();
    assert!(error.message.contains("source_revision"), "{error:?}");
    assert!(error.message.contains("省略"), "{error:?}");
}

#[test]
fn tutor_validation_recovery_accepts_distinct_corrections_then_stops_repeated_errors() {
    for recover in [true, false] {
        let (_root, mut state) = fixture_with_assets(false);
        let movement = source_move(&state);
        let run=prepare(&mut state);
        let (design,movement)=planned_move(&run,movement);
        let mut bad_target = movement.clone();
        bad_target["move"]["target"]["source_bindings"][0]["source_revision"] = json!("wrong-revision");
        let mut missing_hint = movement.clone();
        missing_hint["move"]["actions"] = json!(["request_hint"]);
        let mut third = missing_hint.clone();
        if recover { third["move"]["hint"] = json!("Compare elapsed times."); }
        else { third["move"]["prompt"] = json!("Try another wording."); }
        let adapter=ChatStubAdapter::scripted(vec![
            call("tutor.step", json!({"operation":"trace"})),
            call("book.text", json!({"lid":"1.1"})),
            design,
            call("tutor.step", bad_target), call("tutor.step", missing_hint),
            call("tutor.step", third), answer("Use total time."),
        ]);
        let result=agent_run::execute_prepared(&agent_run::BorrowedAppPort(RefCell::new(&mut state)),&adapter,run,Default::default());
        assert_eq!(result.reply.status,200,"{}",result.reply.body);
        let reply:Value=serde_json::from_str(&result.reply.body).unwrap();
        let trace = reply["trace"].as_array().unwrap();
        for code in ["TEACHING_INVALID", "TOOL_ARGUMENTS_INVALID"] {
            assert!(trace.iter().any(|step| step["result_digest"].as_str().is_some_and(|result| result.contains(code))),
                "the correction scenario must actually hit {code}: {reply}");
        }
        assert_eq!(reply["incomplete"], !recover, "recover={recover}: {reply}");
        if !recover { assert_eq!(reply["warning"], "AGENT_NO_PROGRESS", "{reply}"); }
        let session = state.user.agent_history.sessions.last().unwrap();
        let turn = session.turns.last().unwrap();
        let deliveries = state.user.learning_store().unwrap().teaching_deliveries(&session.id, &turn.turn_id).unwrap();
        assert_eq!(deliveries.len(), usize::from(recover), "{reply}");
    }
}

#[test]
fn tutor_t16_target_requires_exact_current_turn_source_and_object_identity() {
    let (_root, mut state) = fixture_with_assets(false);
    let run = prepare(&mut state);
    let movement = source_move(&state);
    assert!(step_with_design(&state.private_context(), &run.turn_ref, movement.clone(), &[], &[]).is_err());
    let mut invalid_move = movement.clone();
    invalid_move["move"]["target"]["source_bindings"][0]["end_lid"] = json!("1.2");
    assert!(step_with_design(&state.private_context(), &run.turn_ref, invalid_move, &[], &[range()]).is_err());
    let mut invalid_move = movement.clone();
    invalid_move["move"]["target"]["source_bindings"][0]["source_revision"] = json!("other");
    assert!(step_with_design(&state.private_context(), &run.turn_ref, invalid_move, &[], &[range()]).is_err());
    let mut invalid_move = movement.clone();
    invalid_move["move"]["target"]["object_refs"] = json!([{"source_id":state.workspace.book.base.book_id,"object_id":"invented","object_revision":1}]);
    assert!(step_with_design(&state.private_context(), &run.turn_ref, invalid_move, &[], &[range()]).is_err());
    let mut partial = range();
    partial.ranges = vec![read_tools::SourceSelectedRange { lid:"1.1".into(), range:read_tools::SourceTextRange { start:0, end:5 } }];
    assert!(step_with_design(&state.private_context(), &run.turn_ref, movement.clone(), &[], &[partial]).is_err());
    step_with_design(&state.private_context(), &run.turn_ref, movement, &[], &[range()]).unwrap();
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
    assert!(step_with_design(&state.private_context(), &run.turn_ref, movement.clone(), &[], &[range()]).is_err());
    // A supported builder publication replaces the mutable standalone artifact.
    objects["revision"] = json!(2);
    objects["objects"][0]["object_revision"] = json!(2);
    std::fs::write(root.path().join("formal_objects.json"), objects.to_string()).unwrap();
    super::tutor_tests::t15_close(root.path(), &state.workspace.book, "formal_objects", &["formal_objects.json"]);
    let material = step_with_design(&state.private_context(), &run.turn_ref, json!({"operation":"material","object_id":"speed"}), &[], &[]).unwrap();
    assert_eq!(material["object"]["object_revision"], 1);
    assert_eq!(material["materials"], json!([]));
    let mut wrong_revision = movement.clone();
    wrong_revision["move"]["target"]["object_refs"][0]["object_revision"] = json!(2);
    assert!(step_with_design(&state.private_context(), &run.turn_ref, wrong_revision, &[], &[range()]).is_err());
    step_with_design(&state.private_context(), &run.turn_ref, movement, &[], &[range()]).unwrap();
    finish(&mut state, &run.turn_ref, &outcome(None));
    let rows = state.user.learning_store().unwrap().teaching_deliveries(&run.turn_ref.session_id, &run.turn_ref.turn_id).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].payload["object_versions"][0]["object_revision"], 1);
    let next = prepare(&mut state);
    let material = step_with_design(&state.private_context(), &next.turn_ref, json!({"operation":"material","object_id":"speed"}), &[], &[]).unwrap();
    assert_eq!(material["object"]["object_revision"], 2);
    // T17: accepted standalone objects remain current in the understanding view.
    let mut scored=source_move(&state);
    scored["move"]["object_ids"]=json!(["speed"]);
    scored["move"]["target"]["object_refs"]=json!([{"source_id":state.workspace.book.base.book_id,"object_id":"speed","object_revision":2}]);
    scored["move"]["actions"]=json!(["submit"]);
    scored["move"]["assessment"]=json!({"rule":{"kind":"choice_set","choices":["A","B"],"correct":["A"]},"source_lids":["1.1"],"source_sufficient":true,"feedback":"after_submit"});
    step_with_design(&state.private_context(),&next.turn_ref,scored,&[],&[range()]).unwrap();
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
            session_id:stated.turn_ref.session_id.clone(), turn_id:stated.turn_ref.turn_id.clone(),
        }], confidence:None,
        sensitivity:Sensitivity::Normal, valid_until:None,
    }, "2026-10-09T10:00:00Z").unwrap();
    finish(&mut state, &stated.turn_ref, &outcome(None));
    let context = prepare(&mut state).tutor.unwrap();
    assert_eq!(context["learner_context"]["confirmed_background"][0]["fact_id"], fact.fact_id);
    assert!(context["learner_context"]["confirmed_background"].to_string().contains("not studied average speed"));
    assert_eq!(context["learner_context"]["entries"], json!([]));
    assert_eq!(context["learner_context"]["unknown_when_absent"], true);
    assert_eq!(context["learner_context"]["user_intent"], context["user_intent"]);
}
#[test]
fn tutor_resident_preserves_practice_context_and_answers_ordinary_questions() {
    let (_root, mut state) = fixture();
    let run=prepare_activity_chat(&mut state,r#"{"message":"Teach me average speed"}"#,"now").unwrap_or_else(|e|panic!("{}",e.body));
    let turns=script(&run,"observe");
    let first=execute_script(&mut state,run,turns);
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
    post(&mut state, "/agent/new", "{}");
    let run=prepare_activity_chat(&mut state,&json!({"message":"I am confused; directly explain this time","teaching_ref":rows[0].event_id}).to_string(),"now").unwrap_or_else(|e|panic!("{}",e.body));
    let turns=script(&run,"explain");
    let second=execute_script(&mut state,run,turns);
    assert!(second["answer"].as_str().unwrap().contains("total time"));
    let current=state.user.agent_history.sessions.last().unwrap().turns.last().unwrap();
    let bound=state.user.learning_store().unwrap().teaching_event(&format!("binding:{}",current.turn_id)).unwrap();
    assert_eq!(bound.binding.tutor_session_id,old_session);
    let revision = state
        .user.learning_store()
        .unwrap()
        .state()
        .unwrap()
        .control
        .revision;
    legacy_control(
        &mut state,
        
        json!({"operation_id":"off","expected_revision":revision,"action":{"kind":"set_enabled","enabled":false}}),
    );
    state.services.adapter = Box::new(ChatStubAdapter::scripted(vec![answer(
        "Ordinary explanation.",
    )]));
    let prepared = prepare_agent_chat(&mut state,r#"{"message":"An ordinary explanation"}"#,"later").unwrap();
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
    legacy_control(
        &mut state,
        
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
    let reread = step_with_design(&state.private_context(),
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
    legacy_control(
        &mut state,
        
        json!({"operation_id":"on2","expected_revision":revision,"action":{"kind":"set_enabled","enabled":true}}),
    );
    teaching::start_request(&state.private_context(), "now").unwrap();
    std::fs::remove_file(root.path().join("teaching_readiness.json")).unwrap();
    let ordinary=prepare_agent_chat(&mut state,r#"{"message":"Explain the denominator"}"#,"later").unwrap();
    assert_eq!(ordinary.tutor.as_ref().unwrap()["status"], "observing");
    finish(&mut state,&ordinary.turn_ref,&outcome(None));
    std::fs::remove_file(root.path().join("book_structure.json")).unwrap();
    let unready = prepare_agent_chat(&mut state,r#"{"message":"Explain the denominator again"}"#,"later").unwrap();
    assert_eq!(unready.tutor.unwrap()["status"], "observing");
    assert!(step_with_design(&state.private_context(),
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

pub fn tutor_browser_host_impl(scored: bool, source_only: bool) {
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
            PresentationContent { action_contract: vec![], teaching_binding: None,
                method_refs: None, method_observations: Default::default(),
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
        step_with_design(&state.private_context(), turn, movement, &[], &[range()]).unwrap();
    } else if scored {
        step_with_design(&state.private_context(),
            turn,
            json!({"operation":"material","object_id":"speed"}),
            &[],
            &[],
        )
        .unwrap();
        step_with_design(&state.private_context(),turn,json!({"operation":"select","move":{"move_id":"one","object_ids":["speed"],"capability":"explanation","kind":"question","interaction_intent":"neutral","intent_origin":"neutral","prompt":"Observe one: A total distance; B total time; C colour.","actions":["submit","revise","request_hint","reveal","skip","self_report"],"hint":"Compare the two times.","explanation":"Use total distance divided by total time.","presentation":reference,"assessment":{"rule":{"kind":"choice_set","choices":["A","B","C"],"correct":["A","B"]},"source_lids":["1.1"],"source_sufficient":true,"feedback":"after_submit"}}}),&[],&[range()]).unwrap();
    } else {
        select(&state, turn, "one", Some(&reference));
    }
    if scored {
        step_with_design(&state.private_context(),turn,json!({"operation":"select","move":{"move_id":"two","object_ids":["speed"],"capability":"explanation","kind":"question","interaction_intent":"neutral","intent_origin":"neutral","prompt":"Observe two: Explain average speed.","actions":["submit","revise"],"presentation":reference,"assessment":{"rule":open_rule(),"source_lids":["1.1"],"source_sufficient":true,"feedback":"after_submit"}}}),&[],&[range()]).unwrap();
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
    let original_anchor = prepared.scope.reader_input.as_ref().unwrap().state.viewport.anchor_lid.clone();
    let other = write_multi_leaf_book("mu1c-tutor-other", "mu1c-tutor-other", 4);
    assert_eq!(route_open_book(&mut state, &json!({"dir":other}).to_string(), "later").status, 200);
    let app = BorrowedAppPort(RefCell::new(&mut state));
    let mut port = RuntimeStatePort { port:&app, turn_ref:&prepared.turn_ref, scope:&prepared.scope,
        previewed:Default::default(), animations:Default::default(), plots:Default::default() };
    assert_eq!(port.read_live_reader(|r| r.state()).unwrap_err().error_code, "WORKSPACE_STALE");
    assert_eq!(port.reader_input(&prepared.scope.reading_book(), "ignored").state.viewport.anchor_lid, original_anchor);
    let material = port.tutor_step(json!({"operation":"material","object_id":"speed"}), &[], &[]).unwrap();
    assert!(material.to_string().contains("total distance"), "{material}");
    app.with_app(|state| {
        let revision = state.user.learning_store().unwrap().state().unwrap().control.revision;
        legacy_control(state, json!({"operation_id":"mu1c-off","expected_revision":revision,"action":{"kind":"set_enabled","enabled":false}}));
    });
    assert!(port.tutor_active().unwrap(), "the original practice run is independent of the removed setting");
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
        assert_eq!(turn.domain.teaching[0].session_id, if session.id == first.turn_ref.session_id { tutor.clone() } else { second.tutor.as_ref().unwrap()["binding"]["tutor_session_id"].as_str().unwrap().to_owned() });
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


#[test]
fn w3_ordinary_understanding_and_frozen_preparation_stay_in_origin_workspace() {
    let (_root, mut state) = fixture_with_assets(false);
    let private = tempfile::tempdir().unwrap();
    state.user.store = MemoryStore::open(private.path().join("memory.json")).unwrap();
    state.workspace.learning_workspace_id = Some("workspace-a".into());
    let mut session = new_agent_session(&state.workspace.book.base.book_id, "1", state.user.agent_history.sessions.len());
    session.learning_workspace_id = Some("workspace-a".into());
    state.workspace.select_chat(session.id.clone(), session.messages.clone());
    state.user.agent_history.sessions.push(session);
    let original = source_move(&state);
    let run = prepare_agent_chat(&mut state, r#"{"message":"I do not understand the denominator."}"#, "1").unwrap_or_else(|r|panic!("{}",r.body));
    run.scope.check_user(&state.user).unwrap();
    assert_eq!(state.user.agent_history.sessions.iter().find(|s|s.id==run.turn_ref.session_id).unwrap().learning_workspace_id.as_deref(), Some("workspace-a"));
    let context = run.tutor.as_ref().unwrap();
    assert_eq!(context["status"], "observing");
    let fact = format!("usage:{}:user:{}",context["binding"]["tutor_session_id"].as_str().unwrap(),run.turn_ref.turn_id);
    let request = json!({"operation":"evidence","nature":"hypothesis","operation_id":"workspace-a-difficulty","target":original["move"]["target"],
        "fact_refs":[fact],"interpretation":"The denominator connection is unclear.","teaching_implication":"Explain total elapsed time."});
    // Admission has already persisted this preparation. Recovery replays that
    // exact snapshot, rather than recomputing a view after facts gained sequences.
    let original_store=state.user.learning_store_for_workspace(Some("workspace-a")).unwrap();
    let original_binding=original_store.teaching_event(&format!("binding:{}",run.turn_ref.turn_id)).unwrap();
    let frozen=teaching::FrozenTeachingPreparation {
        learning_workspace_id:Some("workspace-a".into()),
        context:Some(original_binding.payload.clone()),
        events:original_store.teaching_turn_events(&original_binding.binding.tutor_session_id,&run.turn_ref.turn_id).unwrap(),
    };
    drop(original_store);
    let frozen: teaching::FrozenTeachingPreparation = serde_json::from_value(serde_json::to_value(frozen).unwrap()).unwrap();
    let adapter = ChatStubAdapter::scripted(vec![call("book.text",json!({"lid":"1.1","end_lid":"1.1"})),call("tutor.step",request),answer("Use total time.")]);
    let result = agent_run::execute_prepared(&agent_run::BorrowedAppPort(RefCell::new(&mut state)), &adapter, run, Default::default());
    assert_eq!(result.reply.status,200,"{}",result.reply.body);
    state.workspace.learning_workspace_id = Some("workspace-b".into());
    frozen.resume(&state.user).unwrap();
    let a = state.user.learning_store_for_workspace(Some("workspace-a")).unwrap();
    assert_eq!(a.teaching_event(&original_binding.event_id).unwrap(),original_binding,
        "resuming the original preparation must not rewrite its input-time context after understanding extraction");
    assert_eq!(a.evidence("interpretation:workspace-a-difficulty").unwrap().learner_quote, "I do not understand the denominator.");
    assert!(!a.state().unwrap().control.enabled);
    let b = state.user.learning_store_for_workspace(Some("workspace-b")).unwrap();
    assert!(b.evidence("interpretation:workspace-a-difficulty").is_err());
    assert!(b.teaching_event(&fact).is_err());
    drop(a); drop(b);
    state.user.store = MemoryStore::open(private.path().join("memory.json")).unwrap();
    assert!(state.user.learning_store_for_workspace(Some("workspace-a")).unwrap().evidence("interpretation:workspace-a-difficulty").is_ok());
    assert!(state.user.learning_store_for_workspace(Some("workspace-b")).unwrap().evidence("interpretation:workspace-a-difficulty").is_err());
}
