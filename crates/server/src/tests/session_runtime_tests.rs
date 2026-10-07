use super::*;
use crate::session_event::EventBody;
use crate::session_store::{SessionPaths, SessionStore};

pub(super) fn setup() -> (tempfile::TempDir, AppState) {
    let dir = tempfile::tempdir().unwrap();
    let mut state = state_named(&format!("jl4-local-{}", uuid::Uuid::now_v7()));
    let path = dir.path().join("agent-history.json");
    let paths = SessionPaths::from_history(&path);
    std::fs::create_dir_all(&paths.root).unwrap();
    let (store, history) = SessionStore::open(paths).unwrap();
    state.user.history_path = Some(path);
    state.user.session_store = Some(store);
    state.user.agent_history = history;
    assert_eq!(route_agent_new(&mut state, "t0").status, 200);
    (dir, state)
}

struct Hello;
impl ModelAdapter for Hello {
    fn complete(&self, _: CompletionRequest) -> Result<ParsedResponse, AdapterError> {
        unreachable!()
    }
    fn chat(&self, _: &AgentRequestPlan) -> Result<AssistantTurn, AdapterError> {
        Ok(AssistantTurn {
            provider_continuation: Some(runtime::ProviderContinuation {
                model: "fixture".into(),
                reasoning_content: "private continuation".into(),
            }),
            text: Some("你好！".into()),
            tool_calls: vec![],
            usage_total_tokens: Some(3),
        })
    }
}

#[test]
fn jl4_local_run_reopens_and_public_history_hides_continuation() {
    let (_dir, mut state) = setup();
    state.services.adapter = Box::new(Hello);
    let reply = route_agent_chat(&mut state, r#"{"message":"你好"}"#, "t1");
    assert_eq!(reply.status, 200, "{}", reply.body);
    assert!(!reply.body.contains("private continuation"));
    let (history, _) = crate::session_store::load_chat_storage(&state.user.history_path).unwrap();
    let session = &history.sessions[0];
    assert_eq!(session.turns[0].status, AgentAssistantStatus::Completed);
    assert!(session
        .messages
        .iter()
        .any(|m| m.provider_continuation.is_some()));
    assert_eq!(
        serde_json::to_value(session).unwrap(),
        serde_json::to_value(&state.user.agent_history.sessions[0]).unwrap()
    );
    let log = &state.user.session_store.as_ref().unwrap().logs[&session.id];
    let kinds = std::fs::read_to_string(&log.path)
        .unwrap()
        .lines()
        .map(|s| {
            serde_json::from_str::<Value>(s).unwrap()["kind"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        kinds
            .iter()
            .filter(|s| s.as_str() == "message.appended")
            .count(),
        2
    );
    assert!(kinds.iter().any(|s| s == "activity.recorded"));
    assert_eq!(kinds.last().unwrap(), "turn.finished");
    assert!(!state.user.history_path.as_ref().unwrap().exists());
}

#[test]
fn jl3_local_book_switch_keeps_prepared_run_in_original_chat() {
    let (_dir, mut state) = setup();
    let prepared = prepare_agent_chat(&mut state, r#"{"message":"你好"}"#, "t1")
        .ok()
        .unwrap();
    let original = prepared.turn_ref.session_id.clone();
    let next = write_multi_leaf_book("jl3-book-switch", "jl3-other", 3);
    let reply = route_open_book(&mut state, &json!({"dir":next}).to_string(), "t2");
    assert_eq!(reply.status, 200, "{}", reply.body);
    let selected = state.workspace.selected_chat.clone();
    let result = agent_run::execute_prepared(
        &agent_run::BorrowedAppPort(std::cell::RefCell::new(&mut state)),
        &Hello,
        prepared,
        runtime::run_context::CancellationToken::default(),
    );
    assert_eq!(result.reply.status, 200, "{}", result.reply.body);
    assert_eq!(state.workspace.selected_chat, selected);
    let history = crate::session_store::load_chat_storage(&state.user.history_path)
        .unwrap()
        .0;
    assert_eq!(
        history
            .sessions
            .iter()
            .find(|s| s.id == original)
            .unwrap()
            .turns[0]
            .status,
        AgentAssistantStatus::Completed
    );
    assert!(history
        .sessions
        .iter()
        .find(|s| Some(&s.id) == selected.as_ref())
        .unwrap()
        .turns
        .is_empty());
}

#[test]
fn jl4_pre_turn_failure_keeps_previous_answer_and_accepted_question() {
    let (_dir, mut state) = setup();
    state.services.adapter = Box::new(Hello);
    assert_eq!(
        route_agent_chat(&mut state, r#"{"message":"你好"}"#, "t1").status,
        200
    );
    let prepared = prepare_agent_chat(&mut state, r#"{"message":"next question"}"#, "t2")
        .ok()
        .unwrap();
    let old = prepared.messages.clone();
    let result = agent_run::execute_model(
        &Hello,
        &prepared,
        Default::default(),
        None,
        None,
        std::time::Instant::now(),
        |_, _, _| {
            Err(ToolError {
                error_code: "COMPACTION_FAILED".into(),
                category: "internal".into(),
                message: "before user append".into(),
            })
        },
    );
    assert!(result.messages.starts_with(&old));
    assert_eq!(result.messages.len(), old.len() + 1);
    assert_eq!(
        result.messages.last().unwrap().content.as_deref(),
        Some("next question")
    );
}

#[test]
fn jl4_redaction_checkpoint_and_failure_truncation_share_persistable_history() {
    let (_dir, mut state) = setup();
    let book = state.workspace.book.base.book_id.clone();
    let reference =
        precommit_agent_turn(&mut state, &book, "current".into(), None, None, None, "t1").unwrap();
    let mut messages = vec![
        Message::system("base"),
        Message::user("old objective ".repeat(1200)),
    ];
    let mut answer = Message::user("old answer ".repeat(1200));
    answer.role = runtime::Role::Assistant;
    messages.push(answer);
    messages.push(Message::user("current"));
    let mut call = Message::user("");
    call.role = runtime::Role::Assistant;
    call.tool_calls.push(runtime::ToolCall {
        id: "author".into(),
        name: "presentation.author".into(),
        arguments: json!({"operation":"write","html":"FORBIDDEN_HTML"}).to_string(),
    });
    messages.push(call);
    let mut result=Message::user(json!({"version":"tool_search_result.v2","task":"find","matches":[{"name":"presentation.author","description":"FORBIDDEN_DESCRIPTION"}]}).to_string());
    result.role = runtime::Role::Tool;
    result.tool_call_id = Some("author".into());
    messages.push(result);
    crate::session_runtime::progress(&mut state.user, &reference, &messages, &[]).unwrap();
    let port = agent_run::BorrowedAppPort(std::cell::RefCell::new(&mut state));
    let mut sink = agent_run::RunCheckpointSink {
        port: &port,
        turn_ref: &reference,
    };
    sink.prepare_persisted_messages(&mut messages);
    let prepared = runtime::prepare_compaction(
        runtime::CompactionPhase::MidTurn,
        &messages,
        &messages,
        vec![],
        vec![],
        vec![],
        BTreeMap::new(),
    )
    .unwrap();
    let ids = prepared
        .request()
        .eligible_items
        .iter()
        .map(|s| s.source_item_id.clone())
        .collect::<Vec<_>>();
    let generator = CompactionDraftAdapter {
        output: RefCell::new(Some(json!({
            "active_goal":[{"item_id":"item.goal","text":"Continue objective","source_item_ids":ids,"evidence_refs":[]}],
            "progress":[],"decisions":[],"user_constraints":[],"open_obligations":[],"unresolved_ambiguities":[],"critical_facts":[],"critical_examples":[],"next_steps":[],
            "source_coverage":ids.iter().map(|id|json!({"source_item_id":id,"disposition":"compacted","target_item_ids":["item.goal"]})).collect::<Vec<_>>()
        }))),
    };
    let profile =
        runtime::ModelRuntimeProfile::fallback("fixture", runtime::ProviderToolProtocol::Native);
    let checkpoint = runtime::compact_with_adapter(
        &generator,
        &profile,
        &prepared,
        runtime::CompactionLimits {
            generation_input_limit_tokens: 100_000,
            target_active_tokens: 20_000,
        },
    )
    .unwrap();
    sink.install(&checkpoint, &messages).unwrap();
    drop(sink);
    drop(port);
    let paths = state.user.session_store.as_ref().unwrap().paths.clone();
    let (store, history) = SessionStore::open(paths.clone()).unwrap();
    assert_eq!(history.sessions[0].compaction_checkpoint, Some(checkpoint));
    assert_eq!(history.sessions[0].messages, messages);
    state.user.session_store = Some(store);
    state.user.agent_history = history;
    messages.truncate(4);
    finalize_agent_turn(
        &mut state,
        &reference,
        AgentAssistantStatus::Failed,
        None,
        Some(AgentTurnError {
            error_code: "PROVIDER_ERROR".into(),
            category: "provider".into(),
            message: "failed".into(),
        }),
        None,
        &messages,
        "t2",
    )
    .unwrap();
    let (_, history) = SessionStore::open(paths.clone()).unwrap();
    assert!(history.sessions[0].compaction_checkpoint.is_none());
    assert_eq!(history.sessions[0].messages, messages);
    let bytes = std::fs::read_to_string(paths.session(&reference.session_id)).unwrap();
    assert!(!bytes.contains("FORBIDDEN_HTML") && !bytes.contains("FORBIDDEN_DESCRIPTION"));
    let last: Value = serde_json::from_str(bytes.lines().last().unwrap()).unwrap();
    assert_eq!(last["kind"], "turn.finished");
    assert_eq!(last["payload"]["history_revision"]["from"], 4);
    let goal = history.sessions[0].goals[0].id.clone();
    assert_eq!(
        route_agent_goal_cancel(&mut state, &json!({"goal_id":goal}).to_string(), "t3").status,
        200
    );
    assert_eq!(
        SessionStore::open(paths).unwrap().1.sessions[0].goals[0].status,
        runtime::goal::GoalStatus::Cancelled
    );
}

#[test]
fn jl4_local_pending_recovery_closes_saved_tool_call_once() {
    let (_dir, mut state) = setup();
    let book = state.workspace.book.base.book_id.clone();
    let reference =
        precommit_agent_turn(&mut state, &book, "pending".into(), None, None, None, "t1").unwrap();
    let mut call = Message::user("");
    call.role = runtime::Role::Assistant;
    call.tool_calls.push(runtime::ToolCall {
        id: "started".into(),
        name: "reader.note".into(),
        arguments: "{}".into(),
    });
    crate::session_runtime::append(
        &mut state.user,
        &reference,
        "t2",
        EventBody::MessageAppended {
            messages: vec![Message::user("pending"), call],
        },
    )
    .unwrap();
    let paths = state.user.session_store.as_ref().unwrap().paths.clone();
    let (mut store, mut history) = SessionStore::open(paths.clone()).unwrap();
    crate::session_runtime::recover(&mut store, &mut history, &Default::default()).unwrap();
    let bytes = std::fs::read(paths.session(&reference.session_id)).unwrap();
    crate::session_runtime::recover(&mut store, &mut history, &Default::default()).unwrap();
    assert_eq!(
        std::fs::read(paths.session(&reference.session_id)).unwrap(),
        bytes
    );
    assert_eq!(
        history.sessions[0].turns[0]
            .error
            .as_ref()
            .unwrap()
            .error_code,
        "INTERRUPTED"
    );
    assert_eq!(
        history.sessions[0]
            .messages
            .iter()
            .filter(|m| m.tool_call_id.as_deref() == Some("started"))
            .count(),
        1
    );
}

/// Start a fresh persistent test chat using the production startup path.
pub(super) fn enable_jsonl(state: &mut AppState) {
    let (history, store) = crate::session_store::load_chat_storage(&state.user.history_path).unwrap();
    state.user.session_store = store; state.user.agent_history = history;
    state.workspace.selected_chat = None;
    state.workspace.messages.clear();
}

#[test]
fn jl5_bound_sources_survive_compaction_and_reopen() {
    let (_dir, mut state) = setup();
    let path = state.user.history_path.clone().unwrap();
    let (turn, source) = install_source_bound_turn(&mut state, path);
    let session = state.user.agent_history.sessions[0].clone();
    let reference = AgentTurnRef { session_id: session.id.clone(), turn_id: turn.clone(), user_turn_ordinal: 1 };
    let mut answer = Message::user("Bound answer and its evidence ".repeat(1000));
    answer.role = runtime::Role::Assistant;
    let messages = vec![Message::system("base"), Message::user("Original question ".repeat(1000)), answer, Message::user("next")];
    crate::session_runtime::append(&mut state.user, &reference, "revised", EventBody::HistoryRevised(
        crate::session_event::MessageRevision { from: 0, suffix: messages.clone() })).unwrap();
    let prepared = runtime::prepare_compaction(runtime::CompactionPhase::MidTurn, &messages, &messages,
        vec![], vec![], vec![], BTreeMap::new()).unwrap();
    let ids = prepared.request().eligible_items.iter().map(|s| s.source_item_id.clone()).collect::<Vec<_>>();
    let generator = CompactionDraftAdapter { output: RefCell::new(Some(json!({
        "active_goal":[{"item_id":"source.goal","text":"Continue the bound question","source_item_ids":ids,"evidence_refs":[]}],
        "progress":[],"decisions":[],"user_constraints":[],"open_obligations":[],"unresolved_ambiguities":[],"critical_facts":[],"critical_examples":[],"next_steps":[],
        "source_coverage":ids.iter().map(|id| json!({"source_item_id":id,"disposition":"compacted","target_item_ids":["source.goal"]})).collect::<Vec<_>>()
    }))) };
    let profile = runtime::ModelRuntimeProfile::fallback("fixture", runtime::ProviderToolProtocol::Native);
    let checkpoint = runtime::compact_with_adapter(&generator, &profile, &prepared,
        runtime::CompactionLimits { generation_input_limit_tokens: 100_000, target_active_tokens: 20_000 }).unwrap();
    install_agent_compaction_checkpoint(&mut state, &reference.session_id, checkpoint).unwrap();
    let (history, store) = crate::session_store::load_chat_storage(&state.user.history_path).unwrap();
    assert!(history.sessions[0].compaction_checkpoint.is_some());
    state.user.agent_history = history; state.user.session_store = store;
    let actual = &state.user.agent_history.sessions[0].turns[0];
    assert_eq!(actual.source_bindings, session.turns[0].source_bindings);
    let request = json!({"turn_id":turn,"source_ref_id":source});
    assert_eq!(post(&mut state, "/agent/source.resolve", &request.to_string()).status, 200);
    assert_ne!(post(&mut state, "/agent/source.resolve", &json!({"turn_id":turn,"source_ref_id":"retrieved-candidate-only"}).to_string()).status, 200);
    let path = &state.user.session_store.as_ref().unwrap().logs[&reference.session_id].path;
    assert!(std::fs::read_to_string(path).unwrap().contains("sources.bound"));
    let recap = crate::session_recap::local(&state, &HashMap::from([("session_id".into(), reference.session_id)]), "recap").unwrap();
    let recap = json!(recap);
    assert_eq!(recap["sources"].as_array().unwrap().len(), 1);
    assert_eq!(recap["sources"][0]["source_ref_id"], source);
    assert!(recap["sources"][0]["unavailable_reason"].is_null());
}
