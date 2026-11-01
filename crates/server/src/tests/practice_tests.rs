//! Retrieval practice on the existing formal-activity path (ADR-0178, P1).
use super::*;
use super::tutor_loop_tests::{finish, fixture_with_book, outcome, step_with_design};
use memory::teaching::TeachingFact;

fn fixture() -> (tempfile::TempDir, AppState) {
    let source = "Average speed is total distance divided by total time. Equal distances need not take equal times. The total time must be positive. ".repeat(3);
    fixture_with_book(true, Book::new(sample_base(), &source), &source)
}

fn range() -> EvidenceRange {
    EvidenceRange { start_lid: "1.1".into(), end_lid: "1.1".into(), ranges: vec![] }
}

fn json_post(state: &mut AppState, path: &str, body: Value) -> Value {
    let reply = post(state, path, &body.to_string());
    assert_eq!(reply.status, 200, "{}", reply.body);
    serde_json::from_str(&reply.body).unwrap()
}

const PROMPT: &str = "A cyclist rides 12 km in 30 minutes, rests 10 minutes, then rides 8 km in 20 minutes. Which quantities decide the figure for the whole outing? Submit choice tokens as a JSON array: A 20 km; B 60 minutes; C 50 minutes.";

fn hints() -> Value {
    json!([{"kind":"in_problem","text":"Look again at the ten-minute rest."},
        {"kind":"source","text":"Read how the book builds the figure from its two quantities.","source_lids":["1.1"]},
        {"kind":"worked_step","text":"Add every leg and the rest: 30 + 10 + 20 minutes."}])
}

fn practice_move(prompt: &str, practice: Value) -> Value {
    json!({"operation":"select","move":{"move_id":"practice","object_ids":["speed"],"capability":"explanation","kind":"question",
        "interaction_intent":"neutral","intent_origin":"neutral","prompt":prompt,"actions":["submit","revise","request_hint","reveal","skip"],
        "explanation":"The whole outing covers 12 + 8 = 20 km over 30 + 10 + 20 = 60 minutes, rest included: tokens A and B.",
        "assessment":{"rule":{"kind":"choice_set","choices":["A","B","C"],"correct":["A","B"]},"source_lids":["1.1"],"source_sufficient":true,"feedback":"on_reveal"},
        "practice":practice}})
}

fn warmup(hints: Value) -> Value {
    json!({"entry":"warmup","mode":"recall","distance":"adjacent","hints":hints})
}

fn select(state: &AppState, turn: &AgentTurnRef, proposal: Value) -> Result<Value, read_tools::ToolError> {
    step_with_design(&state.private_context(), turn, json!({"operation":"material","object_id":"speed"}), &[], &[]).unwrap();
    step_with_design(&state.private_context(), turn, proposal, &[], &[range()])
}

fn delivered(state: &mut AppState, practice: Value) -> String {
    let turn = prepare_agent_chat(state, r#"{"message":"Explain average speed"}"#, "now").unwrap_or_else(|r| panic!("{}", r.body)).turn_ref;
    select(state, &turn, practice_move(PROMPT, practice)).unwrap();
    finish(state, &turn, &outcome(None));
    let id = format!("delivery:{}:practice", turn.turn_id);
    json_post(state, "/tutor/display", json!({"delivery_ref":id}));
    id
}

fn hint(state: &mut AppState, id: &str, operation: &str) -> Value {
    let help = json_post(state, "/tutor/action", json!({"operation_id":operation,"delivery_ref":id,"action":"request_hint"}))["help"].clone();
    json_post(state, "/tutor/help-displayed", json!({"help_ref":help["event_id"]}));
    help
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

fn speed_row(state: &mut AppState) -> Value {
    let view = json_post(state, "/tutor/understanding", json!({}));
    view["rows"].as_array().unwrap().iter().find(|row| row["object_id"] == "speed").cloned()
        .unwrap_or_else(|| panic!("no projected row: {view}"))
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
    let answered = json_post(&mut state, "/tutor/action", json!({"operation_id":"a1","delivery_ref":id,"action":"submit","response":"A"}));
    let next = prepare_agent_chat(&mut state, &json!({"message":"A","teaching_ref":answered["event_id"]}).to_string(), "later")
        .unwrap_or_else(|r| panic!("{}", r.body));
    let context = next.tutor.unwrap();
    let practice = &context["learner_context"]["responding_to_practice"];
    assert_eq!((practice["delivery_ref"].as_str(), practice["practice"]["hint_count"].as_u64()), (Some(id.as_str()), Some(3)), "{context}");
    assert!(!context.to_string().contains("ten-minute rest"), "private hints stay out of the answer turn");
    assert_eq!(context["learner_context"]["practice_history"][0]["outcome"], "missed", "a partial answer that was not corrected");
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
    let binding = state.user.learning_store().unwrap().teaching_event(&id).unwrap().binding;
    let activities = json_post(&mut state, "/tutor/activities", json!({"session_id":binding.chat_session_id,"turn_id":binding.turn_id}));
    assert_eq!(activities["activities"][0]["practice_check"], "questionable");
    let next = prepare_agent_chat(&mut state, &json!({"message":"Was I right?","teaching_ref":"action:a1"}).to_string(), "later")
        .unwrap_or_else(|r| panic!("{}", r.body));
    assert_eq!(next.tutor.unwrap()["learner_context"]["responding_to_practice"]["check"], "questionable");
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
fn two_relationships() -> (tempfile::TempDir, AppState) {
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

fn both() -> EvidenceRange { EvidenceRange { start_lid: "1.1".into(), end_lid: "1.2".into(), ranges: vec![] } }

/// Pose one practice problem on paragraph `lid` in a new turn and deliver it.
fn pose(state: &mut AppState, lid: &str, prompt: &str, mode: &str, distance: &str) -> Result<String, read_tools::ToolError> {
    pose_focus(state, lid, &format!("Relationship in paragraph {lid}"), prompt, mode, distance)
}

fn pose_focus(state: &mut AppState, lid: &str, focus: &str, prompt: &str, mode: &str, distance: &str) -> Result<String, read_tools::ToolError> {
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

fn answer(state: &mut AppState, id: &str, operation: &str, response: &str) -> Value {
    json_post(state, "/tutor/action", json!({"operation_id":operation,"delivery_ref":id,"action":"submit","response":response}))
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
    assert_eq!(answer(&mut state, &id, "a1", r#"["B","A"]"#)["assessment"], "correct");
}

#[test]
fn a_teaching_aid_goal_needs_a_delivered_page_and_an_open_rubric() {
    let (_root, mut state) = fixture();
    let turn = prepare_agent_chat(&mut state, r#"{"message":"Let me practice"}"#, "now").unwrap_or_else(|r| panic!("{}", r.body)).turn_ref;
    let page = practice_move(PROMPT, json!({"entry":"presentation","mode":"compose","distance":"adjacent","hints":hints()}));
    assert!(select(&state, &turn, page).unwrap_err().message.contains("演示"));
}
