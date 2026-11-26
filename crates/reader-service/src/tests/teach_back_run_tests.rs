use crate::agent_run::{self, BorrowedAppPort, PreparedAgentChat};
use crate::agent_stream::{RunDescriptor, RunStream};
use crate::tests::tutor_continuation_tests::{
    explain_back_admission::{explanation, original_answer},
    fixture, prepare,
};
use crate::tests::{tutor_loop_tests::answer, ChatStubAdapter};
use crate::AppState;
use memory::teaching::TeachingFact;
use runtime::Role;
use serde_json::{json, Value};
use std::{cell::RefCell, sync::Arc, time::Duration};

fn execute(
    state: &mut AppState,
    run: PreparedAgentChat,
    samples: &[String],
) -> (crate::Reply, Arc<RunStream>) {
    let stream = RunStream::new(RunDescriptor {
        book_id: run.scope.book_id().into(),
        session_id: run.turn_ref.session_id.clone(),
        turn_id: run.turn_ref.turn_id.clone(),
    });
    let adapter = ChatStubAdapter::scripted(samples.iter().map(|sample| answer(sample)).collect());
    let report = agent_run::execute_prepared_with_stream_for_test(
        &BorrowedAppPort(RefCell::new(state)),
        &adapter,
        run,
        Default::default(),
        &stream,
    );
    (report.reply, stream)
}

fn student_claim(words: &str) -> Value {
    json!({"kind":"explanation","student_graph":{
        "nodes":[{"id":"s","label":"Stopped time counts","quotes":[words]}],"edges":[]}})
}

fn book_graph() -> Value {
    json!({"nodes":[{"id":"elapsed","label":"Full elapsed time"},{"id":"speed","label":"Average speed"}],
        "edges":[{"id":"b","from":"elapsed","to":"speed","relation_type":"defines",
            "evidence":[{"lid":"1.1","quote":"Average speed is total distance divided by total time."}]}]})
}

#[test]
fn teach_back_acp_uses_fresh_blind_sessions_and_repairs_only_the_book_comparison() {
    use crate::agent_run::RunCoordinator;
    use std::{collections::HashSet, path::Path, sync::{atomic::AtomicBool, Mutex}};
    let (root, mut state) = fixture(false);
    let origin = original_answer(&mut state);
    let mut request = explanation(&origin);
    request["local_agent_id"] = json!("teacher");
    let record = root.path().join("samples.jsonl");
    let student = student_claim(request["message"].as_str().unwrap());
    let corrected = json!({"source":{"source_id":state.workspace.book.base.book_id,
        "source_revision":state.workspace.book.source_fingerprint()},
        "book_graph":book_graph(),
        "rows":[{"row_id":"r","book_edge_ids":["b"],"book_node_ids":[],"student_edge_ids":[],"student_node_ids":["s"],
            "status":"consistent","summary":"Both include stopped time."}],"side_notes":[],"follow_up":null});
    let mut draft = corrected.clone();
    draft["book_graph"]["edges"][0]["evidence"][0]["quote"] = json!("invented");
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/tests/fixtures/acp_teaching.cjs");
    let config = root.path().join("turn.json");
    std::fs::write(&config, json!({"teach_back":true,"record":record,"student":student,
        "draft":draft,"corrected":corrected}).to_string()).unwrap();
    crate::local_acp::update(root.path(), &json!({"action":"save","agent":{
        "id":"teacher","name":"teacher","command":"node","args":[script,config]}})).unwrap();
    let state = Arc::new(Mutex::new(state));
    let mut coordinator = RunCoordinator::new(state.clone(), Arc::new(AtomicBool::new(false)),
        crate::observability::ObservabilityRuntime::disabled(None));
    coordinator.local_profile = Some(root.path().into());
    let result = coordinator.run(&request.to_string(), "1001", &ChatStubAdapter::scripted(vec![]), || {}, || {});
    assert_eq!(result.status, 200, "{}", result.body);
    let outcome: Value = serde_json::from_str(&result.body).unwrap();
    let samples: Vec<Value> = std::fs::read_to_string(record).unwrap().lines()
        .map(|line| serde_json::from_str(line).unwrap()).collect();
    assert_eq!(samples.len(), 3);
    assert_eq!(samples.iter().map(|s| s["cwd"].as_str().unwrap()).collect::<HashSet<_>>().len(), 3);
    assert_eq!(samples[0]["packet"].as_object().unwrap().len(), 3);
    assert_eq!(samples[0]["packet"]["expression"], request["message"]);
    assert_eq!(samples[0]["packet"]["original_question"], "Why include stopped time?");
    assert!(samples[0]["packet"]["topic_terms"].is_array());
    assert!(samples[1]["packet"]["evidence"]["passages"].is_array());
    assert_eq!(samples[2]["packet"]["input"], samples[1]["packet"]);
    let card = &outcome["answer_view"]["parts"][0]["comparison"];
    assert_eq!(card["book_graph"]["edges"], corrected["book_graph"]["edges"]);
    assert_eq!(samples[1]["packet"]["frozen_student_graph"], card["student_graph"]);
    assert_eq!(outcome["turns"], 3);
    assert_eq!(outcome["tokens_spent"], 45);
    let mut state = state.lock().unwrap();
    let turn = state.user.agent_history.sessions[0].turns.last().unwrap();
    assert_eq!(turn.domain.local_agent.as_ref().unwrap().id, "teacher");
    let round = card["round_id"].as_str().unwrap();
    assert_eq!(state.user.learning_store().unwrap().teaching_event(round).unwrap().payload["comparison"], *card);
    request.as_object_mut().unwrap().remove("local_agent_id");
    assert!(crate::interaction_input::local_repeat(&mut state, &request.to_string()).is_err());
    drop(state);

    // A pure help request goes back to the same ACP Agent with its normal reading tools.
    let question = "Please explain stopped time.";
    let help = "Elapsed time includes the time spent waiting.";
    request["expected_chat_head"] = card["student_graph"]["nodes"][0]["quotes"][0]["turn_id"].clone();
    request["local_agent_id"] = json!("teacher");
    request["client_request_id"] = json!("help-request");
    request["expression_id"] = json!("help-request");
    request["message"] = json!(question);
    std::fs::write(&config, json!({"teach_back":true,"record":root.path().join("help.jsonl"),
        "student":{"kind":"help"},"question":question,"help":help}).to_string()).unwrap();
    let result = coordinator.run(&request.to_string(), "1002", &ChatStubAdapter::scripted(vec![]), || {}, || {});
    assert_eq!(result.status, 200, "{}", result.body);
    let ordinary: Value = serde_json::from_str(&result.body).unwrap();
    assert_eq!(ordinary["answer"], help);
    assert_eq!(ordinary["turns"], 2);
    assert_eq!(ordinary["tokens_spent"], 35);
    assert!(ordinary["answer_view"]["parts"].as_array().unwrap().iter().all(|p|p["kind"] != "teach_back_comparison"));
}

#[test]
fn teach_back_failed_comparison_retries_once_then_delivers_one_atomic_card_on_explicit_retry() {
    let (_root, mut state) = fixture(false);
    let original = original_answer(&mut state);
    let mut request = explanation(&original);
    let run = prepare(&mut state, request.clone());
    let turn = run.turn_ref.clone();
    let round = format!("tb:{}:r1", turn.turn_id);
    let student = student_claim(request["message"].as_str().unwrap()).to_string();
    let draft = json!({"source":{"source_id":state.workspace.book.base.book_id,
            "source_revision":state.workspace.book.source_fingerprint()},
        "book_graph":book_graph(),
        "rows":[{"row_id":"row","book_edge_ids":["b"],"book_node_ids":[],"student_edge_ids":[],"student_node_ids":["s"],
            "status":"consistent","summary":"Both count the whole interval."}],
        "side_notes":[],"follow_up":null});
    let mut invalid = draft.clone();
    invalid["book_graph"]["edges"][0]["evidence"][0]["quote"] = json!("An invented passage.");
    let (failed, stream) = execute(
        &mut state,
        run,
        &[student.clone(), invalid.to_string(), invalid.to_string()],
    );
    assert!(
        failed.body.contains("TEACH_BACK_INVALID"),
        "{}",
        failed.body
    );
    let snapshot = stream.snapshot();
    assert!(snapshot.draft.is_none());
    assert!(snapshot
        .activities
        .iter()
        .any(|a| a.name == "compare_passages"
            && a.status == runtime::run_events::ActivityStatus::Failed));
    assert!(state
        .user
        .learning_store()
        .unwrap()
        .teaching_event_sequence(&round)
        .unwrap()
        .is_none());

    request["retry_of"] = json!(turn.turn_id);
    request["expected_chat_head"] = request["retry_of"].clone();
    request["operation_id"] = json!("retry-correct-quote");
    let run = prepare(&mut state, request.clone());
    let (completed, stream) = execute(&mut state, run, &[student, draft.to_string()]);
    assert_eq!(completed.status, 200, "{}", completed.body);
    let outcome: Value = serde_json::from_str(&completed.body).unwrap();
    let parts = outcome["answer_view"]["parts"].as_array().unwrap();
    assert_eq!(parts.len(), 1);
    let card = &parts[0]["comparison"];
    assert_eq!(card["round_id"], round);
    assert_eq!(
        card["student_graph"]["nodes"][0]["quotes"][0]["turn_id"],
        turn.turn_id
    );
    assert_eq!(
        card["book_graph"]["edges"][0]["evidence"],
        draft["book_graph"]["edges"][0]["evidence"]
    );
    assert!(stream.snapshot().draft.is_none());
    let (events, _) = stream.read_after(Some(0), Duration::ZERO);
    let stages: Vec<_> = events
        .iter()
        .filter(|event| event.event_type.starts_with("teach_back."))
        .map(|event| {
            (
                event.event_type.as_str(),
                event.payload["name"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        stages,
        vec![
            ("teach_back.started", "organize_expression"),
            ("teach_back.finished", "organize_expression"),
            ("teach_back.started", "compare_passages"),
            ("teach_back.finished", "compare_passages")
        ]
    );
    assert_eq!(outcome["turns"], 2);
    assert_eq!(outcome["tokens_spent"], 10);
    let (chat, _) = crate::session_store::load_chat_storage(&state.user.history_path).unwrap();
    assert_eq!(chat.sessions[0].turns.len(), 2);
    assert_eq!(
        chat.sessions[0]
            .messages
            .iter()
            .filter(|m| m.role == Role::User && m.content.as_deref() == request["message"].as_str())
            .count(),
        1
    );
    let store = state.user.learning_store().unwrap();
    let saved = store.teaching_event(&round).unwrap();
    assert_eq!(saved.kind, TeachingFact::TeachBackDelivered);
    assert_eq!(saved.payload["comparison"], *card);
    assert!(store
        .teaching_event_sequence(&format!("display:{round}"))
        .unwrap()
        .is_none());
}

#[test]
fn teach_back_help_routes_to_ordinary_question_without_comparison_or_learning_evidence() {
    let (_root, mut state) = fixture(false);
    let original = original_answer(&mut state);
    let mut request = explanation(&original);
    request["message"] = json!("Could you explain total time?");
    let run = prepare(&mut state, request);
    let turn = run.turn_ref.clone();
    let (reply, stream) = execute(
        &mut state,
        run,
        &[
            json!({"kind":"help"}).to_string(),
            "Total time includes each stop.".into(),
        ],
    );
    assert_eq!(reply.status, 200, "{}", reply.body);
    let outcome: Value = serde_json::from_str(&reply.body).unwrap();
    assert_eq!(outcome["answer"], "Total time includes each stop.");
    assert!(outcome["answer_view"]["parts"]
        .as_array()
        .unwrap()
        .iter()
        .all(|p| p["kind"] != "teach_back_comparison"));
    let stages: Vec<_> = stream
        .snapshot()
        .activities
        .into_iter()
        .filter(|a| a.kind == "teach_back")
        .collect();
    assert_eq!(stages.len(), 1);
    assert_eq!(stages[0].name, "organize_expression");
    let store = state.user.learning_store().unwrap();
    for id in [
        format!("tb:{}:r1", turn.turn_id),
        format!("teach-back-expression:{}", turn.turn_id),
    ] {
        assert!(store.teaching_event_sequence(&id).unwrap().is_none());
    }
    assert_eq!(
        store
            .learner_projection(&state.workspace.book.base.book_id)
            .unwrap()
            .1,
        0
    );
}

#[test]
fn teach_back_corrections_keep_the_first_expression_and_freeze_displayed_assistance() {
    use crate::tests::tutor_continuation_tests::post_ok;
    let (_root, mut state) = fixture(false);
    let origin = original_answer(&mut state);
    let initial = explanation(&origin);
    let first_text = initial["message"].as_str().unwrap();
    let student = |words: &str| student_claim(words).to_string();
    let mut draft = json!({"source":{"source_id":state.workspace.book.base.book_id,
            "source_revision":state.workspace.book.source_fingerprint()},
        "book_graph":book_graph(),
        "rows":[{"row_id":"r","book_edge_ids":["b"],"book_node_ids":[],"student_edge_ids":[],"student_node_ids":["s"],
            "status":"divergent","summary":"Does the timer include the stop?","divergence_condition":"Counting the stop changes the denominator."}],
        "side_notes":[],"follow_up":{"row_id":"r","question":"Does the timer stop?"}});
    let send = |state: &mut AppState, request: Value, samples: &[String]| {
        let run = prepare(state, request);
        let turn = run.turn_ref.clone();
        let (reply, _) = execute(state, run, samples);
        assert_eq!(reply.status, 200, "{}", reply.body);
        (
            turn,
            serde_json::from_str::<Value>(&reply.body).unwrap()["answer_view"]["parts"][0].clone(),
        )
    };
    let (first, part) = send(
        &mut state,
        initial.clone(),
        &[student(first_text), draft.to_string()],
    );
    let card = &part["comparison"];
    let first_round = card["round_id"].as_str().unwrap();
    let reply = |head: &crate::AgentTurnRef, parent: &str, kind: &str, key: &str, words: &str| {
        json!({"entry_action":"explain_back","message":words,"response_language":"en",
            "client_request_id":key,"operation_id":key,"expression_id":key,
            "session_id":origin.session_id,"answer_ref":{"turn_id":origin.turn_id},"expected_chat_head":head.turn_id,
            "teach_back_reply":{"kind":kind,"round_id":parent,"row_id":"r"}})
    };
    let display = |state: &mut AppState, turn: &crate::AgentTurnRef, round: &str| {
        post_ok(
            state,
            "/tutor/display",
            json!({"chat_session_id":turn.session_id,"turn_id":turn.turn_id,"round_id":round}),
        )
    };
    let new_reason = reply(
        &first,
        first_round,
        "dispute_interpretation",
        "new-reason",
        "The clock measures all rest periods separately.",
    );
    let admit = |state: &mut AppState, req: &Value| {
        crate::validate_agent_input(&mut state.user, &state.workspace, &req.to_string())
    };
    assert!(admit(&mut state, &new_reason).is_err());
    display(&mut state, &first, first_round);
    let mut wrong_row = new_reason.clone();
    wrong_row["teach_back_reply"]["row_id"] = json!("r-other");
    assert!(admit(&mut state, &wrong_row).is_err());
    let (redirect, part) = send(
        &mut state,
        new_reason,
        &[
            student(first_text),
            json!({"kind":"needs_revision"}).to_string(),
        ],
    );
    assert_eq!(part["kind"], "markdown");
    assert!(part["text"].as_str().unwrap().contains("Revise"));
    let store = state.user.learning_store().unwrap();
    assert!(store
        .teaching_event_sequence(&format!("teach-back-expression:{}", redirect.turn_id))
        .unwrap()
        .is_none());
    assert!(store
        .teaching_event_sequence(&format!("tb:{}:r1", redirect.turn_id))
        .unwrap()
        .is_none());
    let dispute = reply(
        &redirect,
        first_round,
        "dispute_interpretation",
        "listen-again",
        "I already said the stop counts.",
    );
    draft["rows"][0]["status"] = json!("consistent");
    draft["rows"][0]["summary"] = json!("The explanation explicitly counts the stop.");
    draft["rows"][0]
        .as_object_mut()
        .unwrap()
        .remove("divergence_condition");
    draft["book_graph"]["nodes"].as_array_mut().unwrap().push(json!({"id":"numerator","label":"Total distance"}));
    draft["book_graph"]["edges"].as_array_mut().unwrap().push(json!({
        "id":"distance","from":"numerator","to":"speed","relation_type":"defines",
        "evidence":[{"lid":"1.1","quote":"total distance divided by total time"}]
    }));
    draft["rows"].as_array_mut().unwrap().push(json!({
        "row_id":"distance","book_edge_ids":["distance"],"book_node_ids":[],"student_edge_ids":[],"student_node_ids":[],
        "status":"omitted","summary":"How distance enters the calculation remains unexpressed."
    }));
    draft["follow_up"] =
        json!({"row_id":"distance","question":"Which distance belongs in the numerator?"});
    let (corrected, part) = send(
        &mut state,
        dispute,
        &[student(first_text), draft.to_string()],
    );
    let corrected_card = &part["comparison"];
    let corrected_round = corrected_card["round_id"].as_str().unwrap();
    assert_eq!(
        corrected_card["student_graph"]["nodes"][0]["quotes"][0]["turn_id"],
        first.turn_id
    );
    assert_eq!(
        corrected_card["support_delivery_refs"],
        card["support_delivery_refs"]
    );
    assert_eq!(
        corrected_card["continuation"]["kind"],
        "dispute_interpretation"
    );
    assert_eq!(
        corrected_card["continuation"]["parent_round_id"],
        first_round
    );
    let first_evidence = store
        .evidence(&format!("interpretation:{first_round}"))
        .unwrap();
    let reread = store
        .evidence(&format!("interpretation:{corrected_round}"))
        .unwrap();
    assert_eq!(
        reread.supersedes.as_deref(),
        Some(first_evidence.evidence_id.as_str())
    );
    assert_eq!(reread.action_ref, first_evidence.action_ref);
    assert_eq!(reread.learner_quote, first_text);
    assert!(reread.interpretation.contains("explicitly counts the stop"));
    assert!(reread.interpretation.contains("remains unexpressed"));
    assert_eq!(
        reread.teaching_implication,
        "Which distance belongs in the numerator?"
    );
    assert_eq!(
        reread.support_delivery_refs,
        first_evidence.support_delivery_refs
    );
    assert!(store
        .teaching_event_sequence(&format!("teach-back-expression:{}", corrected.turn_id))
        .unwrap()
        .is_none());
    assert!(store
        .evidence_is_superseded(&first_evidence.evidence_id)
        .unwrap());
    // A history card still refers to the reader's original words. It can open a
    // new correction even if that expression also has a more recent interpretation.
    assert!(admit(
        &mut state,
        &reply(
            &corrected,
            first_round,
            "dispute_interpretation",
            "again",
            "Read it again"
        )
    )
    .is_ok());

    display(&mut state, &corrected, corrected_round);
    let changed = "Total time includes every break from start to finish.";
    draft["rows"][0]["status"] = json!("clarified");
    let revise = reply(
        &corrected,
        corrected_round,
        "revise_understanding",
        "new-expression",
        changed,
    );
    let (revised, part) = send(&mut state, revise, &[student(changed), draft.to_string()]);
    let revised_card = &part["comparison"];
    let revised_round = revised_card["round_id"].as_str().unwrap();
    let new_evidence = store
        .evidence(&format!("interpretation:{revised_round}"))
        .unwrap();
    assert_eq!(new_evidence.supersedes, None);
    assert_eq!(new_evidence.learner_quote, changed);
    assert_eq!(
        new_evidence.action_ref,
        format!("teach-back-expression:{}", revised.turn_id)
    );
    assert!(new_evidence
        .support_delivery_refs
        .contains(&corrected_round.to_owned()));
    assert_eq!(
        revised_card["support_delivery_refs"],
        json!(new_evidence.support_delivery_refs)
    );
    assert_eq!(revised_card["continuation"]["first_round_id"], first_round);
    assert_eq!(
        revised_card["continuation"]["parent_round_id"],
        corrected_round
    );
    assert_eq!(
        revised_card["student_graph"]["nodes"][0]["quotes"][0]["turn_id"],
        revised.turn_id
    );
    let (history, _) = crate::session_store::load_chat_storage(&state.user.history_path).unwrap();
    let restored = history
        .sessions
        .iter()
        .find(|s| s.id == origin.session_id)
        .unwrap();
    assert_eq!(restored.turns.len(), 5);
    assert_eq!(
        serde_json::to_value(&restored.turns[1].outcome).unwrap()["answer_view"]["parts"][0]
            ["comparison"],
        *card
    );
    assert_eq!(
        serde_json::to_value(&restored.turns[4].outcome).unwrap()["answer_view"]["parts"][0]
            ["comparison"],
        *revised_card
    );

    // While this historical reread is generating, the reader can also correct
    // the same interpretation in their understanding history.
    let request = reply(
        &revised,
        first_round,
        "dispute_interpretation",
        "reread-original",
        "I said the timer kept running.",
    );
    let run = prepare(&mut state, request);
    let reread_turn = run.turn_ref.clone();
    let manual = state
        .user
        .learning_store()
        .unwrap()
        .correct_evidence(
            &reread.evidence_id,
            "independent-correction",
            "I meant wall-clock time.",
        )
        .unwrap();
    draft["rows"][0]["status"] = json!("consistent");
    let (result, _) = execute(&mut state, run, &[student(first_text), draft.to_string()]);
    assert_eq!(result.status, 200, "{}", result.body);
    let latest_round = format!("tb:{}:r1", reread_turn.turn_id);
    let latest_id = format!("interpretation:{latest_round}");
    let latest = store.evidence(&latest_id).unwrap();
    assert_eq!(latest.supersedes, Some(manual.evidence_id));
    assert_eq!(latest.action_ref, first_evidence.action_ref);
    assert_eq!(
        latest.support_delivery_refs,
        first_evidence.support_delivery_refs
    );
    display(&mut state, &reread_turn, &latest_round);
    assert_eq!(store.evidence(&latest_id).unwrap(), latest);
}
