//! Retrieval practice on the existing formal-activity path (ADR-0178, P1).
use super::*;
use super::tutor_loop_tests::{finish, fixture_with_book, outcome, step_with_design};

use super::tutor_loop_tests::prepare_practice_chat as prepare_agent_chat;

use memory::teaching::TeachingFact;

pub const OPTIMIZATION_QUESTIONS: &str = include_str!("../../../../examples/optimization-practice/questions.json");

#[test]
fn question_formats_match_their_frozen_contracts() {
    for q in serde_json::from_str::<Vec<Value>>(OPTIMIZATION_QUESTIONS).unwrap() {
        let input: runtime::tutor::AnswerInput = serde_json::from_value(q["answer_input"].clone()).unwrap();
        let mut contract: memory::assessment::AssessmentContract = serde_json::from_value(json!({
            "rule":q["rule"],"source_lids":[q["section"]],"source_sufficient":true,"feedback":"after_submit"})).unwrap();
        assert!(input.matches_assessment(Some(&contract)));
        contract.rule = memory::assessment::ScoringRule::ChoiceSet { choices:vec!["X".into()],correct:vec!["X".into()] };
        assert!(!input.matches_assessment(Some(&contract)));
    }
}

/// Seed real question cards through the same teaching selection and delivery as the Reader.
/// Both the book path and the new, isolated learner directory are supplied explicitly.
#[test]
#[ignore = "requires a built optimization book and a fresh private directory for interactive browser acceptance"]
fn seed_optimization_question_walkthrough() {
    let book_dir = std::path::PathBuf::from(std::env::var_os("UB_QUESTION_BOOK").unwrap());
    let private = std::path::PathBuf::from(std::env::var_os("UB_QUESTION_DEMO_DIR").unwrap());
    std::fs::create_dir(&private).expect("Choose a new, empty learner directory");
    let book = Book::load(book_dir.to_str().unwrap()).unwrap();
    assert_eq!(book.base.book_id,"optimization-modeling-algorithms-theory");
    let mut state = state_named("optimization-question-demo");
    state.workspace.book = Arc::new(book);
    state.workspace.reader = Reader::new(&state.workspace.book, DEFAULT_RADIUS);
    state.workspace.book_dir = book_dir;
    state.user.store = MemoryStore::open(private.join("memory.json")).unwrap();
    state.user.history_path = Some(private.join("agent-history.json"));
    (state.user.agent_history, state.user.session_store) = crate::session_store::load_chat_storage(&state.user.history_path).unwrap();
    let session = crate::new_agent_session(&state.workspace.book.base.book_id, "optimization-review", 0);
    state.workspace.selected_chat = Some(session.id.clone());
    state.user.session_store.as_mut().unwrap().create(&mut state.user.agent_history, session).unwrap();
    json_post(&mut state, "/tutor/mutate", json!({"operation_id":"on","expected_revision":0,"action":{"kind":"set_enabled","enabled":true}}));
    let started = teaching::start_request(&state.private_context(), "now").unwrap();
    assert_eq!(started["started"], true, "{started}");
    let questions: Vec<Value> = serde_json::from_str(OPTIMIZATION_QUESTIONS).unwrap();
    let ranges: Vec<_> = ["1.11.7.17", "1.7.9.12"].into_iter().map(|lid| EvidenceRange{start_lid:lid.into(),end_lid:lid.into(),ranges:vec![]}).collect();
    let sources = json!(ranges.iter().map(|r| json!({"start_lid":r.start_lid,"end_lid":r.end_lid})).collect::<Vec<_>>());
    let target = json!({"learning_focus":"从可行性判断最优值","expected_performance":"能用题中给出的信息说明一个界或最优值的根据。",
        "capability":"explanation","object_refs":[],"source_bindings":sources});
    let design = json!({"operation":"design","operation_id":"optimization-review","design":{
        "target":target,"organizing_model":"原可行点提供候选值；下界与可行值相遇可以证明最优性。",
        "representation":{"kind":"text","reason":"用短题逐项练习。"},"blocks":[{"block_id":"bounds","relation":"把候选点放回原可行集比较。",
            "purpose":"用题面数据判断能成立的结论。","source_bindings":sources}],"connections":[],"focus_block_refs":["bounds"],
        "basis_refs":[],"open_question_refs":[],"revision_reason":"期末练习","previous_ref":null}});
    let mut design_ref = Value::Null;
    for (q, title) in questions.into_iter().zip(["对偶界 · 单选题","最优性证书 · 多选题","约束最小值 · 计算题"]) {
        let turn = prepare_agent_chat(&mut state, &json!({"message":format!("期末冲刺：{title}")}).to_string(), "now")
            .unwrap_or_else(|r|panic!("{}",r.body)).turn_ref;
        if design_ref.is_null() { design_ref = teaching::step(&state.private_context(), &turn, design.clone(), &[], &ranges).unwrap()["design_ref"].clone(); }
        let lid = &q["section"];
        let mut local_target = target.clone();
        local_target["learning_focus"] = q["focus"].clone();
        local_target["source_bindings"] = json!([{"start_lid":lid,"end_lid":lid}]);
        let mut request = json!({"operation":"select","move":{"move_id":q["id"],"design_ref":design_ref,
            "block_refs":["bounds"],"target":local_target,"object_ids":[],"capability":"explanation","kind":"question",
            "interaction_intent":"neutral","intent_origin":"neutral","prompt":q["prompt"],"answer_input":q["answer_input"],
            "actions":["submit","revise","request_hint","reveal","skip"],"explanation":q["solution"],
            "assessment":{"rule":q["rule"],"source_lids":[lid],"source_sufficient":true,
                "feedback":if q["answer_input"]["kind"] == "worked_solution" {"after_submit"} else {"on_reveal"}},
            "practice":{"entry":q["entry"],"mode":"discriminate","distance":"adjacent","hints":[{"kind":"structure","text":q["hint"]}]}}});
        super::tutor_loop_tests::fixture_participation(&mut request["move"]);
        teaching::step(&state.private_context(), &turn, request, &[], &ranges).unwrap();
        let mut result = outcome(None);
        result.answer = Some(title.into());
        result.answer_view.as_mut().unwrap().parts = vec![AgentAnswerPart::Markdown{text:title.into()}];
        finish(&mut state,&turn,&result);
        let list = json_post(&mut state,"/tutor/activities",json!({"session_id":turn.session_id,"turn_id":turn.turn_id}));
        assert_eq!(list["activities"].as_array().unwrap().len(),1, "{list}");
    }
}

pub fn fixture() -> (tempfile::TempDir, AppState) {
    let source = "Average speed is total distance divided by total time. Equal distances need not take equal times. The total time must be positive. ".repeat(3);
    fixture_with_book(true, Book::new(sample_base(), &source), &source)
}

pub fn range() -> EvidenceRange {
    EvidenceRange { start_lid: "1.1".into(), end_lid: "1.1".into(), ranges: vec![] }
}

pub fn json_post(state: &mut AppState, path: &str, body: Value) -> Value {
    let reply = post(state, path, &body.to_string());
    assert_eq!(reply.status, 200, "{}", reply.body);
    serde_json::from_str(&reply.body).unwrap()
}

pub const PROMPT: &str = "A cyclist rides 12 km in 30 minutes, rests 10 minutes, then rides 8 km in 20 minutes. Which quantities decide the figure for the whole outing? Submit choice tokens as a JSON array: A 20 km; B 60 minutes; C 50 minutes.";

pub fn hints() -> Value {
    json!([{"kind":"in_problem","text":"Look again at the ten-minute rest."},
        {"kind":"source","text":"Read how the book builds the figure from its two quantities.","source_lids":["1.1"]},
        {"kind":"worked_step","text":"Add every leg and the rest: 30 + 10 + 20 minutes."}])
}

pub fn practice_move(prompt: &str, practice: Value) -> Value {
    json!({"operation":"select","move":{"move_id":"practice","object_ids":["speed"],"capability":"explanation","kind":"question",
        "interaction_intent":"neutral","intent_origin":"neutral","prompt":prompt,"actions":["submit","revise","request_hint","reveal","skip"],
        "explanation":"The whole outing covers 12 + 8 = 20 km over 30 + 10 + 20 = 60 minutes, rest included: tokens A and B.",
        "assessment":{"rule":{"kind":"choice_set","choices":["A","B","C"],"correct":["A","B"]},"source_lids":["1.1"],"source_sufficient":true,"feedback":"on_reveal"},
        "practice":practice}})
}

pub fn warmup(hints: Value) -> Value {
    json!({"entry":"warmup","mode":"recall","distance":"adjacent","hints":hints})
}

pub fn select(state: &AppState, turn: &AgentTurnRef, proposal: Value) -> Result<Value, read_tools::ToolError> {
    step_with_design(&state.private_context(), turn, json!({"operation":"material","object_id":"speed"}), &[], &[]).unwrap();
    step_with_design(&state.private_context(), turn, proposal, &[], &[range()])
}

pub fn delivered(state: &mut AppState, practice: Value) -> String {
    let turn = prepare_agent_chat(state, r#"{"message":"Explain average speed"}"#, "now").unwrap_or_else(|r| panic!("{}", r.body)).turn_ref;
    select(state, &turn, practice_move(PROMPT, practice)).unwrap();
    finish(state, &turn, &outcome(None));
    let id = format!("delivery:{}:practice", turn.turn_id);
    json_post(state, "/tutor/display", json!({"delivery_ref":id}));
    id
}

pub fn hint(state: &mut AppState, id: &str, operation: &str) -> Value {
    let help = json_post(state, "/tutor/action", json!({"operation_id":operation,"delivery_ref":id,"action":"request_hint"}))["help"].clone();
    json_post(state, "/tutor/help-displayed", json!({"help_ref":help["event_id"]}));
    help
}

#[test]
fn unified_practice_keeps_questions_help_and_grading_with_legacy_control_off() {
    let (_root, mut state) = fixture();
    let view = state.user.learning_store().unwrap().state().unwrap();
    state.user.learning_store().unwrap().mutate(&serde_json::from_value(json!({
        "operation_id":"legacy-off", "expected_revision":view.control.revision,
        "action":{"kind":"set_enabled","enabled":false}})).unwrap(), "before-upgrade").unwrap();
    let id = delivered(&mut state, warmup(hints()));
    let original = state.user.learning_store().unwrap().teaching_event(&id).unwrap();
    let activities = json_post(&mut state, "/tutor/activities", json!({"session_id":original.binding.chat_session_id,"turn_id":original.binding.turn_id}));
    assert_eq!(activities["activities"][0]["status"], "active");
    assert!(!state.user.learning_store().unwrap().state().unwrap().control.enabled);

    let ordinary = super::prepare_agent_chat(&mut state, r#"{"message":"Explain why a rest counts as time."}"#, "now").unwrap_or_else(|reply| panic!("{}", reply.body));
    assert_eq!(ordinary.tutor.as_ref().unwrap()["status"], "observing", "a pending practice question must not take over ordinary conversation");
    finish(&mut state, &ordinary.turn_ref, &outcome(None));
    let help = json!({"message":"", "entry_action":"continue_answer", "answer_ref":{"turn_id":original.binding.turn_id},
        "client_request_id":"original-question-help", "session_id":original.binding.chat_session_id, "expected_chat_head":ordinary.turn_ref.turn_id});
    let reply = super::prepare_agent_chat(&mut state, &help.to_string(), "later").err().expect("continue the original unanswered question through its help");
    assert_eq!(reply.status, 200, "{}", reply.body);
    assert_eq!(serde_json::from_str::<Value>(&reply.body).unwrap()["kind"], "activity_help");
    assert_eq!(hint(&mut state, &id, "off-hint")["hint_kind"], "in_problem");
    let result = json_post(&mut state, "/tutor/action", json!({"operation_id":"off-answer", "delivery_ref":id,"action":"submit","response":"[\"A\",\"B\"]"}));
    assert_eq!(result["assessment"], "correct");
}

#[test]
fn unified_practice_continuity_is_scoped_to_the_chat_and_never_activates_ordinary_questions() {
    let (_root,mut state)=fixture();
    let id=delivered(&mut state,warmup(hints()));
    let delivery=state.user.learning_store().unwrap().teaching_event(&id).unwrap();
    let ordinary=crate::prepare_agent_chat(&mut state,r#"{"message":"Explain why stopped time belongs in elapsed time."}"#,"ordinary")
        .unwrap_or_else(|reply|panic!("{}",reply.body));
    assert_eq!(ordinary.tutor.as_ref().unwrap()["status"],"observing");
    assert!(ordinary.tutor.as_ref().unwrap()["current_design"].is_null());
    finish(&mut state,&ordinary.turn_ref,&outcome(None));
    let again=prepare_agent_chat(&mut state,r#"{"message":"Give me another practice problem."}"#,"again")
        .unwrap_or_else(|reply|panic!("{}",reply.body));
    let context=again.tutor.as_ref().unwrap();
    assert_eq!(context["status"],"practice");
    assert_eq!(context["binding"]["tutor_session_id"],delivery.binding.tutor_session_id);
    assert_eq!(context["current_design"]["design_ref"],delivery.payload["move"]["design_ref"]);
    finish(&mut state,&again.turn_ref,&outcome(None));
    post(&mut state,"/agent/new","{}");
    let separate=prepare_agent_chat(&mut state,r#"{"message":"Practice in a new conversation."}"#,"separate")
        .unwrap_or_else(|reply|panic!("{}",reply.body));
    assert_ne!(separate.tutor.as_ref().unwrap()["binding"]["tutor_session_id"],delivery.binding.tutor_session_id);
    assert!(separate.tutor.as_ref().unwrap()["current_design"].is_null());
}

#[test]
fn practice_request_crosses_tool_validation_and_host_selection() {
    let (_root, mut state) = fixture();
    let proposal = practice_move(PROMPT, warmup(hints()));
    let registry = runtime::tool_registry::ToolRegistry::try_new(runtime::orchestrator::tool_specs()).unwrap();
    registry.registration("tutor.step").unwrap().validate_arguments(&proposal.to_string()).unwrap();
    let turn = prepare_agent_chat(&mut state, r#"{"message":"Let me practice average speed."}"#, "now")
        .unwrap_or_else(|r| panic!("{}", r.body)).turn_ref;
    select(&state, &turn, proposal).unwrap();
    finish(&mut state, &turn, &outcome(None));
    let id = format!("delivery:{}:practice", turn.turn_id);
    json_post(&mut state, "/tutor/display", json!({"delivery_ref":id}));
    assert_eq!(hint(&mut state, &id, "registered-hint")["hint_kind"], "in_problem");
}

#[test]
fn practice_hints_escalate_and_only_pointer_help_counts_as_assistance() {
    let (_root, mut state) = fixture();
    let id = delivered(&mut state, warmup(hints()));
    let binding = state.user.learning_store().unwrap().teaching_event(&id).unwrap().binding;
    let public = json_post(&mut state, "/tutor/activities",
        json!({"session_id":binding.chat_session_id,"turn_id":binding.turn_id})).to_string();
    for private in ["ten-minute rest", "Add every leg", "choice_set", "rest included"] {
        assert!(!public.contains(private), "{private} leaked into the public activity");
    }
    assert!(public.contains("\"hint_count\":3"), "{public}");

    let first = hint(&mut state, &id, "h1");
    assert_eq!((first["hint_kind"].as_str(), first["hint_index"].as_u64()), (Some("in_problem"), Some(0)));
    // A retried request returns the hint it first released, never the next one.
    let retry = json_post(&mut state, "/tutor/action", json!({"operation_id":"h1","delivery_ref":id,"action":"request_hint"}));
    assert_eq!(retry["help"]["text"], first["text"]);

    let partial = json_post(&mut state, "/tutor/action", json!({"operation_id":"a1","delivery_ref":id,"action":"submit","response":"A"}));
    assert_eq!(partial["assessment"], "partial");
    let second = hint(&mut state, &id, "h2");
    assert_eq!((second["hint_kind"].as_str(), &second["source_lids"]), (Some("source"), &json!(["1.1"])));
    let correct = json_post(&mut state, "/tutor/action", json!({"operation_id":"a2","delivery_ref":id,"action":"revise","response":r#"["A","B"]"#}));
    assert_eq!(correct["assessment"], "correct");

    let facts = state.user.learning_store().unwrap().teaching_activity_events(&id).unwrap();
    let attempts: Vec<_> = facts.iter().filter(|e| matches!(e.payload["action"].as_str(), Some("submit" | "revise"))).collect();
    // Pointing at a given in the problem surrenders nothing; opening the source does.
    assert_eq!(attempts[0].payload["assistance_refs"], json!([]));
    assert_eq!(attempts[1].payload["assistance_refs"], json!([format!("seen:help:h2")]));

    // The projection counts the source-assisted success as assisted, not independent.
    let row = speed_row(&mut state);
    assert_eq!((row["partial"].as_u64(), row["assisted_support"].as_u64(), row["independent_support"].as_u64()), (Some(1), Some(1), Some(0)), "{row}");

    assert_eq!(hint(&mut state, &id, "h3")["hint_kind"], "worked_step");
    let exhausted = post(&mut state, "/tutor/action", &json!({"operation_id":"h4","delivery_ref":id,"action":"request_hint"}).to_string());
    assert_ne!(exhausted.status, 200);
    assert!(exhausted.body.contains("没有更多提示"), "{}", exhausted.body);
}

pub fn speed_row(state: &mut AppState) -> Value {
    let view = json_post(state, "/tutor/understanding", json!({}));
    view["rows"].as_array().unwrap().iter().find(|row| row["object_id"] == "speed").cloned()
        .unwrap_or_else(|| panic!("no projected row: {view}"))
}

#[test]
fn practice_choice_understanding_reads_frozen_options_and_each_attempts_reasoning() {
    let (_root, mut state) = fixture();
    json_post(&mut state, "/agent/new", json!({}));
    let introduction = super::prepare_agent_chat(&mut state, r#"{"message":"Walk me through the opening passage."}"#, "earlier")
        .unwrap_or_else(|reply| panic!("{}", reply.body)).turn_ref;
    finish(&mut state, &introduction, &outcome(None));
    let turn = prepare_agent_chat(&mut state, r#"{"message":"Explain average speed"}"#, "now")
        .unwrap_or_else(|reply| panic!("{}", reply.body)).turn_ref;
    let mut task = practice_move("Which elapsed time represents the whole outing?", warmup(hints()));
    task["move"]["answer_input"] = json!({"kind":"single_choice","options":[
        {"id":"A","text":"60 minutes including the rest"}, {"id":"B","text":"10 minutes at rest"},
        {"id":"C","text":"50 minutes in motion"}]});
    task["move"]["assessment"]["rule"]["correct"] = json!(["A"]);
    task["move"]["explanation"] = json!("The whole outing takes 60 minutes including the rest.");
    select(&state, &turn, task).unwrap();
    finish(&mut state, &turn, &outcome(None));
    let session = state.user.agent_history.sessions.iter().find(|session| session.id == turn.session_id).unwrap();
    let request = session.turns.iter().find(|entry| entry.turn_id == turn.turn_id).unwrap().user.clone();
    assert_ne!(request, session.title);
    let delivery = format!("delivery:{}:practice", turn.turn_id);
    json_post(&mut state, "/tutor/display", json!({"delivery_ref":delivery}));
    for (operation, action, choice, reason) in [
        ("first", "submit", "C", None),
        ("second", "revise", "C", Some("I counted only riding time.")),
        ("third", "revise", "A", Some("The clock keeps running during the rest.")),
    ] {
        json_post(&mut state, "/tutor/action", json!({"delivery_ref":delivery,"operation_id":operation,
            "action":action,"response":choice,"reasoning":reason}));
        let view = json_post(&mut state, "/tutor/understanding", json!({}));
        let item = &view["current_understanding"][0];
        assert_eq!(item["learner_quote"], choice);
        let displayed = if choice == "A" { "60 minutes including the rest" } else { "50 minutes in motion" };
        assert_eq!(item["choice_answer"], json!({"options":[{"id":choice,"text":displayed}],"reasoning":reason}));
        if action == "revise" {
            assert_eq!(item["previous"]["learner_quote"], "C");
            assert_eq!(item["previous"]["choice_answer"]["options"], json!([{"id":"C","text":"50 minutes in motion"}]));
            let previous_reason = if operation == "third" { Some("I counted only riding time.") } else { None };
            assert_eq!(item["previous"]["choice_answer"]["reasoning"], json!(previous_reason));
        }
        let detail = json_post(&mut state, "/tutor/evidence", json!({"evidence_ref":item["evidence_ref"]}));
        assert_eq!(detail["choice_answer"], item["choice_answer"]);
        assert_eq!(detail["learner_quote"], choice);
        assert_eq!(detail["activity_turn_request"], request);
    }
}

#[test]
fn practice_solved_after_a_problem_pointer_counts_as_independent() {
    let (_root, mut state) = fixture();
    let id = delivered(&mut state, warmup(hints()));
    assert_eq!(hint(&mut state, &id, "h1")["hint_kind"], "in_problem");
    let correct = json_post(&mut state, "/tutor/action", json!({"operation_id":"a1","delivery_ref":id,"action":"submit","response":r#"["A","B"]"#}));
    assert_eq!(correct["assessment"], "correct");
    let row = speed_row(&mut state);
    assert_eq!((row["independent_support"].as_u64(), row["assisted_support"].as_u64()), (Some(1), Some(0)), "{row}");
}

#[test]
fn an_answer_turn_keeps_the_practice_context_and_one_delivery_poses_one_problem() {
    let (_root, mut state) = fixture();
    let id = delivered(&mut state, warmup(hints()));
    let reasoning = "Why does the denominator include the stop?";
    let answered = json_post(&mut state, "/tutor/action", json!({"operation_id":"a1","delivery_ref":id,"action":"submit","response":"A","reasoning":reasoning}));
    let next = prepare_agent_chat(&mut state, &json!({"message":reasoning,"teaching_ref":answered["event_id"]}).to_string(), "later")
        .unwrap_or_else(|r| panic!("{}", r.body));
    let context = next.tutor.unwrap();
    let practice = &context["learner_context"]["responding_to_practice"];
    assert_eq!(practice["submitted_work"]["action_ref"], answered["event_id"]);
    assert_eq!(practice["submitted_work"]["answer"], "A");
    assert_eq!(practice["submitted_work"]["reasoning"], reasoning);
    assert_eq!(practice["submitted_work"]["question"], PROMPT);
    assert_eq!((practice["delivery_ref"].as_str(), practice["practice"]["hint_count"].as_u64()), (Some(id.as_str()), Some(3)), "{context}");
    assert!(!context.to_string().contains("ten-minute rest"), "private hints stay out of the answer turn");
    assert_eq!(context["learner_context"]["practice_history"][0]["outcome"], "partial", "a partial answer that was not corrected");
}

#[test]
fn practice_contract_rejects_named_targets_missing_solutions_and_unordered_hints() {
    let (_root, mut state) = fixture();
    let turn = prepare_agent_chat(&mut state, r#"{"message":"Explain average speed"}"#, "now").unwrap_or_else(|r| panic!("{}", r.body)).turn_ref;
    let named = practice_move("What is the average speed of the whole outing? Submit A, B or C.", warmup(hints()));
    assert!(select(&state, &turn, named).unwrap_err().message.contains("点名"));
    let mut unordered = hints();
    unordered.as_array_mut().unwrap().swap(0, 2);
    let mut missing_solution = practice_move(PROMPT, warmup(hints()));
    missing_solution["move"].as_object_mut().unwrap().remove("explanation");
    let mut single_hint = practice_move(PROMPT, warmup(hints()));
    single_hint["move"]["hint"] = json!("Consider the rest.");
    let mut no_assessment = practice_move(PROMPT, warmup(hints()));
    no_assessment["move"].as_object_mut().unwrap().remove("assessment");
    let missing_note = practice_move(PROMPT, warmup(json!([{"kind":"own_note","text":"Your note.","note_ref":{"note_id":"missing","revision":1}}])));
    let mut undismissable = practice_move(PROMPT, warmup(hints()));
    undismissable["move"]["actions"] = json!(["submit","revise","request_hint","reveal"]);
    let mut no_revision = practice_move(PROMPT, warmup(hints()));
    no_revision["move"]["actions"] = json!(["submit","request_hint","reveal","skip"]);
    assert!(select(&state, &turn, no_revision).unwrap_err().message.contains("改答"));
    for proposal in [practice_move(PROMPT, warmup(unordered)), missing_solution, single_hint, no_assessment, missing_note, undismissable] {
        assert!(select(&state, &turn, proposal.clone()).is_err(), "{proposal}");
    }
    select(&state, &turn, practice_move(PROMPT, warmup(hints()))).unwrap();
}

#[test]
fn note_revisit_quotes_the_saved_idea_and_can_point_back_to_it() {
    let (_root, mut state) = fixture();
    let idea = "休息的时间不算，只看骑车的时间吧？";
    let note = state.user.store.create_personal_note(
        serde_json::from_value(json!({"content":idea,"authorship":"user"})).unwrap(), "idea", "1").unwrap();
    let reference = json!({"note_id":note.note_id,"revision":1});
    let turn = prepare_agent_chat(&mut state, r#"{"message":"Explain average speed"}"#, "now").unwrap_or_else(|r| panic!("{}", r.body)).turn_ref;
    let practice = json!({"entry":"note_revisit","mode":"discriminate","distance":"adjacent","revisit_note":reference,
        "hints":[{"kind":"own_note","text":"Reread what you wrote about the rest.","note_ref":reference},
            {"kind":"worked_step","text":"The rest is part of the outing: 60 minutes."}]});
    assert!(select(&state, &turn, practice_move(PROMPT, practice.clone())).unwrap_err().message.contains("逐字引用"));
    let quoted = format!("You once wrote: “{idea}” {PROMPT}");
    select(&state, &turn, practice_move(&quoted, practice)).unwrap();
}

#[test]
fn note_revisit_entry_requires_an_exact_existing_saved_idea() {
    use crate::interaction_input::InteractionInput;
    let (_root, mut state) = fixture();
    let note = state.user.store.create_personal_note(
        serde_json::from_value(json!({"content":"时间翻倍距离也翻倍？","authorship":"user"})).unwrap(), "idea", "1").unwrap();
    let input = |reference: Value| json!({"message":"","entry_action":"revisit_note","note_ref":reference,
        "source_id":state.workspace.book.base.book_id,"source_revision":state.workspace.book.source_fingerprint(),
        "session_id":state.workspace.selected_chat,"expected_chat_head":null,"client_request_id":"revisit"});
    let exact = input(json!({"note_id":note.note_id,"revision":1}));
    for bad in [input(json!({"note_id":note.note_id})), input(json!({"note_id":"","revision":1})), {
        let mut stray = exact.clone(); stray["entry_action"] = json!("guide_chapter"); stray
    }] {
        assert_eq!(InteractionInput::parse(&bad).unwrap_err().error_code, "INTERACTION_INVALID", "{bad}");
    }
    let missing = input(json!({"note_id":"missing","revision":1}));
    assert!(InteractionInput::parse(&missing).unwrap().resolve(&state.user, &state.workspace, &missing, None).is_err());
    let Some(runtime::tutor_continuation::InteractionResolution::RunInput { interaction }) =
        InteractionInput::parse(&exact).unwrap().resolve(&state.user, &state.workspace, &exact, None).unwrap()
    else { panic!("a note revisit must start a run") };
    assert!(interaction.requests_practice());
    assert_eq!(interaction.accepted_request, "用我这条想法练一练");
    assert_eq!(interaction.note_ref.as_ref().map(|r| r.revision), Some(Some(1)));
    assert!(!interaction.source_bindings.is_empty(), "practice is grounded in the current material");
}

#[test]
fn a_correct_answer_releases_the_solution_and_later_revisions_count_it_as_help() {
    let (_root, mut state) = fixture();
    let id = delivered(&mut state, warmup(hints()));
    let correct = json_post(&mut state, "/tutor/action", json!({"operation_id":"a1","delivery_ref":id,"action":"submit","response":r#"["A","B"]"#}));
    assert_eq!((correct["assessment"].as_str(), correct["help"]["reason"].as_str()), (Some("correct"), Some("concluded")), "{correct}");
    assert!(correct["help"]["text"].as_str().unwrap().contains("rest included"));
    json_post(&mut state, "/tutor/help-displayed", json!({"help_ref":correct["help"]["event_id"]}));
    json_post(&mut state, "/tutor/action", json!({"operation_id":"a2","delivery_ref":id,"action":"revise","response":r#"["A","B"]"#}));
    let facts = state.user.learning_store().unwrap().teaching_activity_events(&id).unwrap();
    let revised = facts.iter().find(|e| e.event_id == "action:a2").unwrap();
    assert_eq!(revised.payload["assistance_refs"], json!(["seen:help:a1"]));
    // A partial answer does not end the problem or reveal anything.
    let (_other, mut state) = fixture();
    let id = delivered(&mut state, warmup(hints()));
    let partial = json_post(&mut state, "/tutor/action", json!({"operation_id":"b1","delivery_ref":id,"action":"submit","response":"A"}));
    assert!(partial["help"].is_null(), "{partial}");
}

#[test]
fn open_practice_releases_its_solution_after_assessment_and_restores_it_on_read() {
    let (_root, mut state) = fixture();
    let turn = prepare_agent_chat(&mut state, r#"{"message":"Let me practice."}"#, "now")
        .unwrap_or_else(|r| panic!("{}", r.body)).turn_ref;
    let mut proposal = practice_move(PROMPT, warmup(hints()));
    proposal["move"]["assessment"]["rule"] = json!({"kind":"open","items":[{
        "id":"time","criterion":"Uses all elapsed time, including the stop.","source_lids":["1.1"]}]});
    select(&state, &turn, proposal).unwrap();
    finish(&mut state, &turn, &outcome(None));
    let id = format!("delivery:{}:practice", turn.turn_id);
    json_post(&mut state, "/tutor/display", json!({"delivery_ref":id}));
    let submitted = json_post(&mut state, "/tutor/action", json!({
        "operation_id":"open-practice","delivery_ref":id,"action":"submit","response":"all time"}));
    assert_eq!(submitted["assessment"], "unassessed");
    assert!(submitted["help"].is_null());
    let read = |state: &mut AppState| json_post(state, "/tutor/activities",
        json!({"session_id":turn.session_id,"turn_id":turn.turn_id}))["activities"][0].clone();
    assert!(read(&mut state)["help"].is_null());
    let evaluation = prepare_agent_chat(&mut state, &json!({"message":"all time",
        "teaching_ref":submitted["event_id"]}).to_string(), "later").unwrap_or_else(|r| panic!("{}", r.body));
    let items = json!([{"id":"time","verdict":"supported","response_quote":"all time",
        "sources":[{"lid":"1.1","quote":"total time"}],"reason":"Includes every part of elapsed time."}]);
    for _ in 0..2 {
        let result = teaching::assessment_accept(&state.private_context(), &evaluation.turn_ref,
            submitted["event_id"].as_str().unwrap(), items.clone()).unwrap();
        assert_eq!(result["assessment"], "correct");
    }
    let restored = read(&mut state);
    assert_eq!(restored["help"]["reason"], "concluded");
    assert!(restored["help"]["text"].as_str().unwrap().contains("rest included"));
    let facts = state.user.learning_store().unwrap().teaching_activity_events(&id).unwrap();
    assert_eq!(facts.iter().filter(|event| event.kind == TeachingFact::HelpDelivered).count(), 1);
    json_post(&mut state, "/tutor/help-displayed", json!({"help_ref":restored["help"]["event_id"]}));
    json_post(&mut state, "/tutor/action", json!({"operation_id":"open-practice-revised",
        "delivery_ref":id,"action":"revise","response":"all time"}));
    let revised = state.user.learning_store().unwrap().teaching_event("action:open-practice-revised").unwrap();
    assert_eq!(revised.payload["assistance_refs"], json!(["seen:help:open-practice"]));
}

#[test]
fn a_structure_hint_sits_after_the_problem_pointer_and_counts_as_help() {
    let (_root, mut state) = fixture();
    let mut ladder = hints();
    ladder.as_array_mut().unwrap().insert(1, json!({"kind":"structure","text":"This is about one whole outing, not its separate legs."}));
    let id = delivered(&mut state, warmup(ladder));
    hint(&mut state, &id, "h1");
    assert_eq!(hint(&mut state, &id, "h2")["hint_kind"], "structure");
    json_post(&mut state, "/tutor/action", json!({"operation_id":"a1","delivery_ref":id,"action":"submit","response":r#"["A","B"]"#}));
    let row = speed_row(&mut state);
    assert_eq!((row["independent_support"].as_u64(), row["assisted_support"].as_u64()), (Some(0), Some(1)), "{row}");
}

#[test]
fn a_questionable_check_flags_the_problem_and_removes_its_results_from_the_projection() {
    let (_root, mut state) = fixture();
    let id = delivered(&mut state, warmup(hints()));
    json_post(&mut state, "/tutor/action", json!({"operation_id":"a1","delivery_ref":id,"action":"submit","response":r#"["A","B"]"#}));
    assert_eq!(speed_row(&mut state)["independent_support"], 1);
    let before = json_post(&mut state, "/tutor/understanding", json!({}));
    let grade = before["interpretations"][0]["evidence_id"].clone();
    assert_eq!(before["interpretations"][0]["status"], "correct");

    let pending = crate::practice_check::next(&state.user).unwrap().expect("delivered practice awaits its check");
    assert_eq!(pending.delivery.event_id, id);
    // The check sees how the problem was posed, never the learner's answer.
    assert_eq!(pending.packet["committed_solution"].as_str().map(|s| s.contains("rest included")), Some(true));
    assert!(pending.packet["frozen_sources"]["1.1"].is_string(), "{}", pending.packet);
    assert!(!pending.packet.to_string().contains(r#"[\"A\",\"B\"]"#));
    let verdict: runtime::practice::PracticeCheck = serde_json::from_value(json!({"verdict":"questionable","reason":"The key is wrong.",
        "issues":[{"kind":"wrong_key","detail":"C is also defensible."}]})).unwrap();
    crate::practice_check::record(&state.user, &pending, Ok(verdict)).unwrap();
    assert!(crate::practice_check::next(&state.user).unwrap().is_none());

    let view = json_post(&mut state, "/tutor/understanding", json!({}));
    assert!(view["rows"].as_array().unwrap().iter().all(|row| row["independent_support"] == 0), "{view}");
    assert_eq!(view["current_understanding"],json!([]));
    assert_eq!(view["interpretations"],json!([]));
    assert_eq!(json_post(&mut state,"/tutor/evidence",json!({"evidence_ref":grade}))["status"],"correct");
    let binding = state.user.learning_store().unwrap().teaching_event(&id).unwrap().binding;
    let activities = json_post(&mut state, "/tutor/activities", json!({"session_id":binding.chat_session_id,"turn_id":binding.turn_id}));
    assert_eq!(activities["activities"][0]["practice_check"], "questionable");
    let next = prepare_agent_chat(&mut state, &json!({"message":"Was I right?","teaching_ref":"action:a1"}).to_string(), "later")
        .unwrap_or_else(|r| panic!("{}", r.body));
    let learner = &next.tutor.unwrap()["learner_context"];
    assert_eq!(learner["responding_to_practice"]["check"], "questionable");
    assert_eq!(learner["interpretations"],json!([]));
}

#[test]
fn failed_checks_are_recorded_and_bounded() {
    let (_root, mut state) = fixture();
    delivered(&mut state, warmup(hints()));
    for _ in 0..3 {
        let pending = crate::practice_check::next(&state.user).unwrap().expect("still pending after a failure");
        crate::practice_check::record(&state.user, &pending, Err("provider unavailable".into())).unwrap();
    }
    assert!(crate::practice_check::next(&state.user).unwrap().is_none(), "three failures stop retrying");
    assert_eq!(speed_row(&mut state)["evidence_count"], 0, "failed checks are not learner evidence");
}

/// Two relationships in separate paragraphs, without a teaching map.
pub fn two_relationships() -> (tempfile::TempDir, AppState) {
    use base_schema::{LidNode, NodeKind, Span};
    let first = "Average speed is total distance divided by total time, stops included. ";
    let second = "A constant rate makes the amount proportional to the elapsed time. ";
    let source = format!("{first}{second}");
    let (a, b) = (first.encode_utf16().count(), source.encode_utf16().count());
    let mut base = sample_base();
    base.graph_nodes.clear();
    base.graph_edges.clear();
    base.lid_nodes = vec![
        LidNode { lid:"1".into(), path:vec![1], kind:NodeKind::Chapter, span:Span { start:0, end:b }, children:vec!["1.1".into(), "1.2".into()] },
        LidNode { lid:"1.1".into(), path:vec![1,1], kind:NodeKind::Paragraph, span:Span { start:0, end:a }, children:vec![] },
        LidNode { lid:"1.2".into(), path:vec![1,2], kind:NodeKind::Paragraph, span:Span { start:a, end:b }, children:vec![] },
    ];
    fixture_with_book(false, Book::new(base, &source), &source)
}

pub fn both() -> EvidenceRange { EvidenceRange { start_lid: "1.1".into(), end_lid: "1.2".into(), ranges: vec![] } }

/// Pose one practice problem on paragraph `lid` in a new turn and deliver it.
pub fn pose(state: &mut AppState, lid: &str, prompt: &str, mode: &str, distance: &str) -> Result<String, read_tools::ToolError> {
    pose_focus(state, lid, &format!("Relationship in paragraph {lid}"), prompt, mode, distance)
}

pub fn pose_focus(state: &mut AppState, lid: &str, focus: &str, prompt: &str, mode: &str, distance: &str) -> Result<String, read_tools::ToolError> {
    let turn = prepare_agent_chat(state, r#"{"message":"Let me practice"}"#, "now").unwrap_or_else(|r| panic!("{}", r.body)).turn_ref;
    let mut proposal = practice_move(prompt, json!({"entry":"warmup","mode":mode,"distance":distance,"hints":hints()}));
    proposal["move"]["object_ids"] = json!([]);
    proposal["move"]["target"] = json!({"learning_focus":focus,"expected_performance":"Use it in a new situation",
        "capability":"explanation","source_bindings":[{"start_lid":lid,"end_lid":lid}],"object_refs":[]});
    let result = step_with_design(&state.private_context(), &turn, proposal, &[], &[both()]);
    if let Err(error) = result {
        // Leave the chat free for the next turn without delivering anything.
        finish(state, &turn, &outcome(None));
        return Err(error);
    }
    finish(state, &turn, &outcome(None));
    let id = format!("delivery:{}:practice", turn.turn_id);
    json_post(state, "/tutor/display", json!({"delivery_ref":id}));
    Ok(id)
}

pub fn answer(state: &mut AppState, id: &str, operation: &str, response: &str) -> Value {
    json_post(state, "/tutor/action", json!({"operation_id":operation,"delivery_ref":id,"action":"submit","response":response}))
}

#[test]
fn skipping_after_two_correct_answers_keeps_their_grades() {
    let (_root, mut state) = two_relationships();
    let first = pose(&mut state,"1.1",PROMPT,"recall","adjacent").unwrap();
    assert_eq!(answer(&mut state,&first,"first",r#"["A","B"]"#)["assessment"], "correct");
    let second = pose(&mut state,"1.2","Which quantities determine pump output? A, B or C.","recall","adjacent").unwrap();
    assert_eq!(answer(&mut state,&second,"second",r#"["A","B"]"#)["assessment"], "correct");
    let third = pose(&mut state,"1.1","Which two readings should a warehouse robot combine? A, B or C.","recall","far").unwrap();
    let skip = json_post(&mut state,"/tutor/action",json!({"operation_id":"third","delivery_ref":third,"action":"skip"}));
    let run = prepare_agent_chat(&mut state,&json!({"message":"我跳过了这个活动，请调整接下来的讲法。","teaching_ref":skip["event_id"]}).to_string(),"after-skip")
        .unwrap_or_else(|r|panic!("{}",r.body));
    let tutor = run.tutor.as_ref().unwrap();
    assert_eq!(tutor["learner_context"]["responding_to_practice"]["action"],"skip");
    let history = tutor["learner_context"]["practice_history"].as_array().unwrap();
    for (id, outcome) in [(&first,"clean"),(&second,"clean"),(&third,"skipped")] {
        let fact = history.iter().find(|q| q["delivery_ref"] == id.as_str()).unwrap();
        assert_eq!(fact["outcome"],outcome);
        assert_eq!(fact["same_chat"],true);
        assert_eq!(fact["skipped"],outcome == "skipped");
    }
}

#[test]
fn a_revealed_solution_without_an_answer_does_not_count_as_a_wrong_attempt() {
    let (_root,mut state)=fixture();
    let id=delivered(&mut state,warmup(hints()));
    let help=json_post(&mut state,"/tutor/action",json!({"operation_id":"look","delivery_ref":id,"action":"reveal"}));
    json_post(&mut state,"/tutor/help-displayed",json!({"help_ref":help["help"]["event_id"]}));
    let run = prepare_agent_chat(&mut state,r#"{"message":"Another practice problem."}"#,"next").unwrap_or_else(|r|panic!("{}",r.body));
    let fact=&run.tutor.as_ref().unwrap()["learner_context"]["practice_history"][0];
    assert_eq!(fact["outcome"],"revealed"); assert_eq!(fact["solution_seen"],true);
    assert_eq!(fact["skipped"],false);
}

#[test]
fn course_application_uses_the_current_lesson_and_delivers_a_practice_card() {
    let (_root,mut state) = fixture();
    json_post(&mut state, "/agent/new", json!({}));
    let mut course: memory::course::InquiryCourse = serde_json::from_str(include_str!(
        "../../../../docs/examples/inquiry-course/ai-infra-resource-bounds.v1.course.json"
    )).unwrap();
    let lesson = &mut course.lessons[0];
    let source = &mut lesson.summary.source;
    source.publication.book_id = state.workspace.book.base.book_id.clone();
    source.publication.publication_id = "local".into();
    source.source_revision = state.workspace.book.source_fingerprint().into();
    source.start_lid = "1.1".into();
    source.end_lid = "1.1".into();
    let mut store = state.user.learning_store().unwrap();
    let goal = store.state().unwrap().workspace_goal_revision.unwrap_or(0);
    let initial = store.mutate_course(&serde_json::from_value(json!({
        "operation_id":"start","run_id":null,"expected_revision":0,"release_version":1,
        "action":{"kind":"start","lesson_id":lesson.summary.id}
    })).unwrap(), Some(("fixture",1,lesson)), goal, "now", None).unwrap();
    let request = json!({
        "message":"", "entry_action":"apply_lesson","learning_purpose":"practice","response_language":"en",
        "course_context":{"run_id":initial.progress.run_id,"revision":initial.progress.revision,"surface":"course"},
        "session_id":state.workspace.selected_chat,"expected_chat_head":null,"client_request_id":"course-application",
        "source_id":state.workspace.book.base.book_id,"source_revision":state.workspace.book.source_fingerprint()
    });
    let run = prepare_agent_chat(&mut state,&request.to_string(),"practice").unwrap_or_else(|r|panic!("{}",r.body));
    let tutor = run.tutor.as_ref().unwrap();
    assert_eq!(tutor["status"],"practice");
    assert_eq!(tutor["binding"]["tutor_session_id"],initial.progress.run_id);
    assert_eq!(tutor["application_requested"],true);
    assert_eq!(tutor["response_language"],"en");
    assert_eq!(tutor["current_action"]["source_bindings"][0]["start_lid"],"1.1");
    assert!(teaching::turn_active(&state.private_context(),&run.turn_ref.turn_id).unwrap());
    let mut application = practice_move(PROMPT,warmup(hints()));
    application["move"]["practice"]["application_situation"] = json!(PROMPT.split('.').next().unwrap());
    select(&state,&run.turn_ref,application).unwrap();
    finish(&mut state,&run.turn_ref,&outcome(None));
    let activities = json_post(&mut state,"/tutor/activities",json!({"session_id":run.turn_ref.session_id,"turn_id":run.turn_ref.turn_id}));
    assert_eq!(activities["activities"].as_array().unwrap().len(),1);
    assert_eq!(store.course_view(&initial.progress.run_id).unwrap().progress.stage,initial.progress.stage);
}

#[test]
fn application_practice_rejects_a_source_recall_and_accepts_a_new_case() {
    let (_root,mut state)=fixture();
    let first=prepare_agent_chat(&mut state,r#"{"message":"Explain average speed."}"#,"first").unwrap_or_else(|r|panic!("{}",r.body));
    finish(&mut state,&first.turn_ref,&outcome(None));
    let input=json!({"message":"","entry_action":"continue_answer","learning_purpose":"practice",
        "answer_ref":{"turn_id":first.turn_ref.turn_id},"session_id":first.turn_ref.session_id,"client_request_id":"apply",
        "expected_chat_head":first.turn_ref.turn_id,"source_id":state.workspace.book.base.book_id,"source_revision":state.workspace.book.source_fingerprint()});
    let run=prepare_agent_chat(&mut state,&input.to_string(),"apply").unwrap_or_else(|r|panic!("{}",r.body));
    assert_eq!(run.tutor.as_ref().unwrap()["application_requested"],true);
    let source=state.workspace.book.text("1.1",Some("1.1")).unwrap();
    let excerpt="total distance divided by total time";
    assert!(source.contains(excerpt));
    let mut reading=practice_move(&format!("{excerpt}. Which statement comes first in the source?"),warmup(hints()));
    assert!(select(&state,&run.turn_ref,reading.clone()).unwrap_err().message.contains("application_situation"));
    reading["move"]["practice"]["application_situation"]=json!(excerpt);
    assert!(select(&state,&run.turn_ref,reading).unwrap_err().message.contains("重复"));
    let mut application=practice_move(PROMPT,warmup(hints()));
    application["move"]["practice"]["application_situation"]=json!(PROMPT.split('.').next().unwrap());
    select(&state,&run.turn_ref,application).unwrap();
    finish(&mut state,&run.turn_ref,&outcome(None));
    let pending=crate::practice_check::next(&state.user).unwrap().unwrap();
    assert!(pending.packet["application"]["new_case"].as_str().unwrap().starts_with("A cyclist"));
    assert!(pending.packet["application"]["previous_answer"].as_str().unwrap().contains("total time"));
}

#[test]
fn forwarded_practice_attempt_keeps_one_current_interpretation_and_its_correction() {
    let (_root, mut state) = fixture();
    let delivery = delivered(&mut state, warmup(hints()));
    assert_eq!(hint(&mut state, &delivery, "given-condition")["hint_kind"], "in_problem");
    let submission = answer(&mut state, &delivery, "first", "A");
    let run = prepare_agent_chat(&mut state, &json!({"message":"A","teaching_ref":submission["event_id"]}).to_string(), "echo")
        .unwrap_or_else(|reply| panic!("{}",reply.body));
    let response_ref = format!("response:{}", run.turn_ref.turn_id);
    let fact = state.user.learning_store().unwrap().teaching_event(&response_ref).unwrap();
    assert_eq!(fact.payload["assistance_refs"], json!([]));
    assert_eq!(fact.payload["attempt"], 1);
    assert_eq!(fact.payload["submission_field"], "response");
    let usage = state.user.learning_store().unwrap().teaching_recent(&fact.binding.tutor_session_id, 12).unwrap()
        .into_iter().find(|f|f.kind==TeachingFact::UsageObserved && f.binding.turn_id==fact.binding.turn_id
            && f.learner_expression()==Some("A")).unwrap();
    assert_eq!(usage.payload["assistance_refs"],json!([]));
    assert_eq!(usage.payload["support_delivery_refs"],json!([]));
    let observe = |state:&AppState, turn:&AgentTurnRef, operation:&str| teaching::step(
        &state.private_context(), turn, json!({"operation":"evidence","nature":"hypothesis","operation_id":operation,
        "target":{"learning_focus":"choosing the distance","expected_performance":"identify the distance","capability":"explanation",
        "source_bindings":[{"start_lid":"1.1","end_lid":"1.1"}]},"fact_refs":[format!("response:{}",turn.turn_id)],
        "interpretation":"The learner identified distance.","teaching_implication":"Check their explanation of time."}), &[], &[range()]
    ).unwrap();
    let echo = observe(&state, &run.turn_ref, "echo");
    let original = state.user.learning_store().unwrap().evidence(echo["evidence_ref"].as_str().unwrap()).unwrap();
    assert!(original.assistance_refs.is_empty() && original.support_delivery_refs.is_empty());
    let view = json_post(&mut state, "/tutor/understanding", json!({}));
    assert_eq!(view["current_understanding"].as_array().unwrap().len(), 1);
    assert_eq!(view["interpretations"].as_array().unwrap().len(), 1);
    assert_eq!(view["interpretations"][0]["nature"], "performance");
    let grade = view["interpretations"][0]["evidence_id"].clone();
    // An older client can still open the retained conversational interpretation.
    let correction = json_post(&mut state, "/tutor/correct", json!({
        "operation_id":"clarify-guess","evidence_ref":echo["evidence_ref"],"text":"I guessed the option."
    }));
    let view = json_post(&mut state, "/tutor/understanding", json!({}));
    assert_eq!(view["current_understanding"].as_array().unwrap().len(),1);
    assert_eq!(view["current_understanding"][0]["evidence_ref"],correction["evidence_ref"]);
    assert_eq!(view["interpretations"][0]["correction"],"I guessed the option.");
    let row = view["rows"].as_array().unwrap().iter().find(|r|r["object_id"]=="speed").unwrap();
    assert_eq!(row["partial"],0);
    assert_eq!(row["uncertain"],1);
    assert_eq!(row["evidence_refs"][0],correction["evidence_ref"]);
    let history = json_post(&mut state,"/tutor/evidence",json!({"evidence_ref":grade}));
    assert_eq!(history["superseded"],true);
    assert_eq!(history["status"],"partial");
    assert!(state.user.learning_store().unwrap().assessment(submission["event_id"].as_str().unwrap()).unwrap().is_some());
    finish(&mut state, &run.turn_ref, &outcome(None));
    let revised = json_post(&mut state,"/tutor/action",json!({"operation_id":"second","delivery_ref":delivery,"action":"revise","response":"A","reasoning":"Why include the rest?"}));
    let question = prepare_agent_chat(&mut state, &json!({"message":"Why include the rest?","teaching_ref":revised["event_id"]}).to_string(), "question")
        .unwrap_or_else(|reply| panic!("{}",reply.body));
    let store = state.user.learning_store().unwrap();
    let fact = store.teaching_event(&format!("response:{}",question.turn_ref.turn_id)).unwrap();
    let action = store.teaching_event(revised["event_id"].as_str().unwrap()).unwrap();
    assert_eq!(fact.payload["submission_field"], "reasoning");
    assert_eq!(fact.payload["attempt"], 2);
    assert_eq!(fact.payload["assistance_refs"], action.payload["assistance_refs"]);
    let question_evidence = observe(&state, &question.turn_ref, "actual-question");
    assert!(store.evidence(question_evidence["evidence_ref"].as_str().unwrap()).unwrap().support_delivery_refs.is_empty());
    let view = json_post(&mut state, "/tutor/understanding", json!({}));
    let current = view["interpretations"].as_array().unwrap();
    assert_eq!(current.len(),3);
    assert!(current.iter().any(|e| e["learner_quote"]=="Why include the rest?"));
    assert!(current.iter().any(|e| e["nature"]=="performance" && e["attempt"]==2));
    assert!(current.iter().any(|e| e["evidence_id"]==correction["evidence_ref"]));
}

#[test]
fn consecutive_problems_interleave_relationships_and_retries_use_a_new_situation() {
    let (_root, mut state) = two_relationships();
    pose(&mut state, "1.1", "Trip one: which quantities decide the whole outing? A, B or C.", "recall", "adjacent").unwrap();
    let again = pose(&mut state, "1.1", "Trip two: which quantities decide the whole outing? A, B or C.", "recall", "adjacent").unwrap_err();
    assert!(again.message.contains("交错"), "{}", again.message);
    pose(&mut state, "1.2", "A pump runs at a fixed rate. Which choice follows? A, B or C.", "recall", "adjacent").unwrap();
    let repeated = pose(&mut state, "1.1", "Trip one: which quantities decide the whole outing? A, B or C.", "recall", "adjacent").unwrap_err();
    assert!(repeated.message.contains("新情境"), "{}", repeated.message);
    pose(&mut state, "1.1", "Trip three: which quantities decide the whole outing? A, B or C.", "recall", "adjacent").unwrap();
}

#[test]
fn different_relationships_in_the_same_chapter_interleave_without_sharing_progression() {
    let (_root, mut state) = two_relationships();
    let trip = "Total time of a journey";
    let pump = "Accumulation at a constant rate";
    let first = pose_focus(&mut state, "1", trip, "Trip one: choose A, B or C.", "recall", "adjacent").unwrap();
    assert_eq!(answer(&mut state, &first, "chapter-a1", r#"["A","B"]"#)["assessment"], "correct");
    // Both relationships cite the whole chapter, as the real reader entry does.
    pose_focus(&mut state, "1", pump, "Pump one: choose A, B or C.", "recall", "adjacent").unwrap();
    let repeat = pose_focus(&mut state, "1", " Accumulation  at a constant rate ", "Pump two: choose A, B or C.", "recall", "adjacent").unwrap_err();
    assert!(repeat.message.contains("交错"), "{}", repeat.message);
    let too_easy = pose_focus(&mut state, "1", trip, "Trip two: choose A, B or C.", "recall", "adjacent").unwrap_err();
    assert!(too_easy.message.contains("调高"), "{}", too_easy.message);
    pose_focus(&mut state, "1", trip, "Trip three: choose A, B or C.", "recall", "far").unwrap();
}

#[test]
fn renaming_a_formal_object_does_not_bypass_practice_interleaving() {
    let (_root, mut state) = fixture();
    let first = delivered(&mut state, warmup(hints()));
    let saved = state.user.learning_store().unwrap().teaching_event(&first).unwrap();
    let turn = prepare_agent_chat(&mut state, r#"{"message":"Let me practice again."}"#, "now")
        .unwrap_or_else(|r| panic!("{}", r.body)).turn_ref;
    let mut proposal = practice_move("Another journey: choose A, B or C.", warmup(hints()));
    proposal["move"]["target"] = saved.payload["move"]["target"].clone();
    proposal["move"]["target"]["learning_focus"] = json!("A new label for the same defined quantity");
    let error = select(&state, &turn, proposal).unwrap_err();
    assert!(error.message.contains("交错"), "{}", error.message);
}

#[test]
fn difficulty_follows_the_last_outcome_and_two_misses_switch_relationships() {
    let (_root, mut state) = two_relationships();
    let a = pose(&mut state, "1.1", "Trip one: which quantities? A, B or C.", "recall", "adjacent").unwrap();
    assert_eq!(answer(&mut state, &a, "a1", r#"["A","B"]"#)["assessment"], "correct");
    pose(&mut state, "1.2", "Pump one: which choice follows? A, B or C.", "recall", "adjacent").unwrap();
    // Clean success: the same level is refused, a farther situation is accepted.
    let same = pose(&mut state, "1.1", "Trip two: which quantities? A, B or C.", "recall", "adjacent").unwrap_err();
    assert!(same.message.contains("调高"), "{}", same.message);
    let far = pose(&mut state, "1.1", "Trip three, far: which quantities? A, B or C.", "recall", "far").unwrap();
    assert_eq!(answer(&mut state, &far, "a2", "C")["assessment"], "incorrect");
    pose(&mut state, "1.2", "Pump two: which choice follows? A, B or C.", "recall", "adjacent").unwrap();
    // A miss lowers difficulty.
    let harder = pose(&mut state, "1.1", "Trip four: which quantities? A, B or C.", "recall", "far").unwrap_err();
    assert!(harder.message.contains("调低"), "{}", harder.message);
    let easier = pose(&mut state, "1.1", "Trip five: which quantities? A, B or C.", "recall", "near").unwrap();
    assert_eq!(answer(&mut state, &easier, "a3", "C")["assessment"], "incorrect");
    pose(&mut state, "1.2", "Pump three: which choice follows? A, B or C.", "recall", "near").unwrap();
    let switched = pose(&mut state, "1.1", "Trip six: which quantities? A, B or C.", "recall", "near").unwrap_err();
    assert!(switched.message.contains("连续两次"), "{}", switched.message);
}

#[test]
fn one_delivery_poses_one_problem_and_practice_names_its_relationship() {
    let (_root, mut state) = two_relationships();
    let turn = prepare_agent_chat(&mut state, r#"{"message":"Let me practice"}"#, "now").unwrap_or_else(|r| panic!("{}", r.body)).turn_ref;
    let target = |lid: &str| json!({"learning_focus":format!("Relationship in paragraph {lid}"),"expected_performance":"Use it",
        "capability":"explanation","source_bindings":[{"start_lid":lid,"end_lid":lid}],"object_refs":[]});
    let mut first = practice_move("Trip: which quantities? A, B or C.", warmup(hints()));
    first["move"]["object_ids"] = json!([]);
    first["move"]["target"] = target("1.1");
    step_with_design(&state.private_context(), &turn, first.clone(), &[], &[both()]).unwrap();
    let mut second = first.clone();
    second["move"]["move_id"] = json!("practice-2");
    second["move"]["prompt"] = json!("Pump: which choice follows? A, B or C.");
    second["move"]["target"] = target("1.2");
    let refused = step_with_design(&state.private_context(), &turn, second, &[], &[both()]).unwrap_err();
    assert!(refused.message.contains("只出一道"), "{}", refused.message);
}


#[test]
fn board_assembly_is_graded_by_the_exact_premise_set_and_hides_card_roles() {
    let (_root, mut state) = fixture();
    let cards = json!([{"id":"A","text":"The outing covered 20 km.","role":"premise"},
        {"id":"B","text":"The outing took 60 minutes, rest included.","role":"premise"},
        {"id":"C","text":"The cyclist was moving for 50 minutes.","role":"distractor"}]);
    let board = |correct: Value| {
        let mut proposal = practice_move("Pick the cards that together decide the figure for the whole outing.",
            json!({"entry":"board","mode":"compose","distance":"adjacent","hints":hints(),"cards":cards}));
        proposal["move"]["assessment"]["rule"]["correct"] = correct;
        proposal
    };
    let turn = prepare_agent_chat(&mut state, r#"{"message":"Let me practice"}"#, "now").unwrap_or_else(|r| panic!("{}", r.body)).turn_ref;
    assert!(select(&state, &turn, board(json!(["A"]))).unwrap_err().message.contains("choice_set"));
    let mut open = board(json!(["A","B"]));
    open["move"]["assessment"]["rule"] = json!({"kind":"open","items":[{"id":"x","criterion":"Uses total time","source_lids":["1.1"]}]});
    assert!(select(&state, &turn, open).is_err());
    select(&state, &turn, board(json!(["A","B"]))).unwrap();
    finish(&mut state, &turn, &outcome(None));
    let id = format!("delivery:{}:practice", turn.turn_id);
    json_post(&mut state, "/tutor/display", json!({"delivery_ref":id}));
    let public = json_post(&mut state, "/tutor/activities", json!({"session_id":turn.session_id,"turn_id":turn.turn_id}));
    assert_eq!(public["activities"][0]["move"]["practice"]["cards"][2]["text"], "The cyclist was moving for 50 minutes.");
    assert!(!public.to_string().contains("distractor") && !public.to_string().contains("\"role\""), "{public}");
    let note = "Why does the time card include the stop?";
    let submission = json_post(&mut state, "/tutor/action", json!({"operation_id":"a1","delivery_ref":id,
        "action":"submit","response":r#"["B","A"]"#,"reasoning":note}));
    assert_eq!(submission["assessment"], "correct");
    let view = json_post(&mut state, "/tutor/understanding", json!({}));
    let summary = &view["current_understanding"][0];
    assert_eq!(summary["learner_quote"], r#"["B","A"]"#);
    assert_eq!(summary["choice_answer"]["options"], json!([
        {"id":"B","text":"The outing took 60 minutes, rest included."},
        {"id":"A","text":"The outing covered 20 km."}]));
    let next = prepare_agent_chat(&mut state, &json!({"message":note,"teaching_ref":submission["event_id"]}).to_string(), "later")
        .unwrap_or_else(|r| panic!("{}", r.body));
    let context = next.tutor.unwrap();
    let work = &context["learner_context"]["responding_to_practice"]["submitted_work"];
    assert_eq!(work["options"], public["activities"][0]["move"]["practice"]["cards"]);
    assert_eq!(work["reasoning"], note);
}

#[test]
fn a_teaching_aid_goal_needs_a_delivered_page_and_an_open_rubric() {
    let (_root, mut state) = fixture();
    let turn = prepare_agent_chat(&mut state, r#"{"message":"Let me practice"}"#, "now").unwrap_or_else(|r| panic!("{}", r.body)).turn_ref;
    let page = practice_move(PROMPT, json!({"entry":"presentation","mode":"compose","distance":"adjacent","hints":hints()}));
    assert!(select(&state, &turn, page).unwrap_err().message.contains("演示"));
}
