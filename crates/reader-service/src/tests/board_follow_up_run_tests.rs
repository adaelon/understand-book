use super::*;
use crate::agent_run::BorrowedAppPort;
use runtime::run_context::CancellationToken;

pub const NOW: &str = "2026-10-25T09:00:00Z";

pub fn board(state: &AppState, operation: &str, value: Value) -> Value {
    crate::board_api::route(&state.user, &state.workspace, operation, &value, NOW).unwrap()
}

pub fn setup() -> (tempfile::TempDir, AppState, Value, Value) {
    let (root, state, sources) = super::board_browser_host::fixture();
    let mut document = board(&state, "create", json!({"title":"持续追问"}));
    for kind in ["prebuild", "understanding"] {
        document = board(&state, "import", json!({"board_id":document["board_id"],
            "board_revision":document["revision"],"source":sources["sources"][kind]}));
    }
    (root, state, document, sources)
}

pub fn input(state: &AppState, document: &Value, item: usize, key: &str) -> Value {
    let session = state.user.agent_history.sessions.iter()
        .find(|session| Some(session.id.as_str()) == state.workspace.selected_chat.as_deref()).unwrap();
    json!({"message":"请继续说明我选中的这张卡片。","client_request_id":key,
        "session_id":session.id,"expected_chat_head":session.turns.last().map(|turn| &turn.turn_id),
        "board_follow_up":{"board_id":document["board_id"],"board_revision":document["revision"],
            "item_id":document["items"][item]["item_id"]}})
}

pub struct AnswerAdapter {
    requests: Arc<Mutex<Vec<Value>>>,
    fail: bool,
}
impl ModelAdapter for AnswerAdapter {
    fn complete(&self, _: CompletionRequest) -> Result<ParsedResponse, AdapterError> { unreachable!() }
    fn complete_structured(&self, request: CompletionRequest) -> Result<Value, AdapterError> {
        if let Some(value) = super::tutor_loop_tests::fixture_coach_guidance(&request) { return Ok(value); }
        if let Some(value) = super::tutor_loop_tests::fixture_delivery_review(&request) { return Ok(value); }
        unreachable!("unexpected structured request")
    }
    fn chat(&self, request: &AgentRequestPlan) -> Result<AssistantTurn, AdapterError> {
        self.requests.lock().unwrap().push(json!(request.ordered_messages()));
        if self.fail {
            // A provider error intentionally produces a compiled, completed fallback.
            // A spend-storage stop is a real failed run and cannot fall back.
            return Err(AdapterError { spend_stop: Some(runtime::model_spend::SpendStop::StorageUnavailable),
                message:"WB2 model spending unavailable".into() });
        }
        Ok(AssistantTurn { provider_continuation: None, text: Some("选中的内容已保留，我们可以沿着这张卡片继续讨论。".into()),
            tool_calls:vec![],usage_total_tokens:None })
    }
}

#[test]
fn wb2_local_run_keeps_sent_target_and_repairs_replay_without_duplicate_cards() {
    let (_root, mut state, document, _) = setup();
    let request = input(&state, &document, 0, "wb2-local-fixed");
    let prepared = prepare_agent_chat(&mut state, &request.to_string(), NOW).unwrap_or_else(|r| panic!("{}", r.body));
    let accepted = prepared.turn_ref.clone();
    assert_eq!(prepared.scope.board_context.as_ref().unwrap().follow_up.item_id.as_deref(), document["items"][0]["item_id"].as_str());
    let changed = board(&state, "update", json!({"board_id":document["board_id"],"board_revision":document["revision"],
        "positions":[{"item_id":document["items"][0]["item_id"],"position":{"x":950,"y":720}}]}));
    board(&state, "view.save", json!({"board_id":document["board_id"],"view":{
        "x":0,"y":0,"zoom":1,"selected_item_id":document["items"][1]["item_id"]}}));
    let requests = Arc::new(Mutex::new(Vec::new()));
    let adapter = AnswerAdapter { requests:requests.clone(),fail:false };
    let report = crate::agent_run::execute_prepared(&BorrowedAppPort(RefCell::new(&mut state)), &adapter,
        prepared, CancellationToken::default());
    assert_eq!(report.reply.status, 200, "{}", report.reply.body);
    let result:Value = serde_json::from_str(&report.reply.body).unwrap();
    assert_eq!(result["board_delivery"]["parent_item_id"], document["items"][0]["item_id"], "{}", report.reply.body);
    let read = board(&state, "read", json!({"board_id":document["board_id"]}));
    assert_eq!(read["document"]["items"].as_array().unwrap().len(), 3);
    assert_eq!(read["document"]["items"][0]["position"], changed["items"][0]["position"]);
    assert_eq!(read["view"]["selected_item_id"], document["items"][1]["item_id"]);
    let answer = &read["document"]["items"][2];
    assert_eq!(answer["reply_to_item_id"], document["items"][0]["item_id"]);
    assert_eq!(answer["target"]["turn_id"], accepted.turn_id);
    assert!(answer["block_id"].as_str().is_some_and(|id| !id.is_empty()));
    assert!(requests.lock().unwrap()[0].to_string().contains(document["items"][0]["item_id"].as_str().unwrap()));
    let replay = match prepare_agent_chat(&mut state, &request.to_string(), NOW) { Err(reply) => reply, Ok(_) => panic!("replay must not execute") };
    assert_eq!(replay.status, 200, "{}", replay.body);
    let replay:Value = serde_json::from_str(&replay.body).unwrap();
    assert_eq!(replay["turn_id"], accepted.turn_id);
    assert_eq!(board(&state,"read",json!({"board_id":document["board_id"]}))["document"], read["document"]);
    let mut conflict = request;
    conflict["board_follow_up"]["item_id"] = document["items"][1]["item_id"].clone();
    let rejected = match prepare_agent_chat(&mut state,&conflict.to_string(),NOW) { Err(reply)=>reply,Ok(_)=>panic!("changed replay accepted") };
    assert_eq!(rejected.status,409);
    let calls = requests.lock().unwrap().len();
    let state = Arc::new(Mutex::new(state));
    let coordinator = crate::agent_run::RunCoordinator::new(state.clone(),
        Arc::new(std::sync::atomic::AtomicBool::new(false)), crate::observability::ObservabilityRuntime::disabled(None));
    let retried = coordinator.retry_save(&accepted.turn_id,NOW).unwrap();
    assert_eq!(retried.final_view.unwrap()["board_delivery"],result["board_delivery"]);
    assert_eq!(requests.lock().unwrap().len(),calls,"save retry must not ask the model again");
    assert_eq!(board(&state.lock().unwrap(),"read",json!({"board_id":document["board_id"]}))["document"],read["document"]);
}

#[test]
fn board_storage_failure_stays_unsaved_until_retry_publishes_the_same_answer() {
    let (root, state, document, _) = setup();
    let request = input(&state, &document, 0, "board-storage-failure");
    let blocker = root.path().join("boards/unscoped")
        .join(document["board_id"].as_str().unwrap()).join("head.replace.tmp");
    assert!(blocker.starts_with(root.path()));
    std::fs::create_dir(&blocker).unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let adapter = AnswerAdapter { requests: requests.clone(), fail: false };
    let state = Arc::new(Mutex::new(state));
    let coordinator = crate::agent_run::RunCoordinator::new(state.clone(),
        Arc::new(std::sync::atomic::AtomicBool::new(false)),
        crate::observability::ObservabilityRuntime::disabled(None));
    let reply = coordinator.run(&request.to_string(), NOW, &adapter, || {}, || {});
    assert_ne!(reply.status, 200, "an answer is not complete while its board write failed: {}", reply.body);
    let failed = coordinator.unsaved_run().expect("the existing save-retry path must retain the result");
    let stream = coordinator.stream(&failed.turn_id).unwrap();
    assert_eq!(stream.snapshot().persistence_state, "failed");
    assert_eq!(stream.snapshot().execution_state, "ended");
    let (events, terminal) = stream.read_after(Some(0), Duration::ZERO);
    assert!(terminal);
    assert!(!events.iter().any(|event| event.event_type == "run.completed"));
    let (answer, messages, turn_count) = {
        let state = state.lock().unwrap();
        let session = state.user.agent_history.sessions.iter().find(|session| session.id == failed.session_id).unwrap();
        let turn = session.turns.iter().find(|turn| turn.turn_id == failed.turn_id).unwrap();
        assert_eq!(turn.status, AgentAssistantStatus::Completed, "the model result remains retryable");
        assert_eq!(turn.board_delivery_error.as_ref().unwrap().error_code, "INTERNAL_ERROR");
        assert_eq!(board(&state, "read", json!({"board_id": document["board_id"]}))["document"], document);
        (json!(turn.outcome), session.messages.clone(), session.turns.len())
    };
    let calls = requests.lock().unwrap().len();
    assert!(coordinator.retry_save(&failed.turn_id, NOW).is_err());
    assert_eq!(stream.snapshot().persistence_state, "failed");
    std::fs::remove_dir(&blocker).unwrap();
    let saved = coordinator.retry_save(&failed.turn_id, NOW).unwrap();
    assert_eq!(saved.persistence_state, "saved");
    assert_eq!(stream.snapshot().execution_state, "completed");
    assert_eq!(stream.snapshot().persistence_state, "saved");
    assert!(coordinator.unsaved_run().is_none());
    let persisted = {
        let state = state.lock().unwrap();
        let session = state.user.agent_history.sessions.iter().find(|session| session.id == failed.session_id).unwrap();
        let turn = session.turns.iter().find(|turn| turn.turn_id == failed.turn_id).unwrap();
        assert_eq!(json!(turn.outcome), answer);
        assert_eq!(session.messages, messages);
        assert_eq!(session.turns.len(), turn_count);
        assert!(turn.board_delivery.is_some());
        assert!(turn.board_delivery_error.is_none());
        board(&state, "read", json!({"board_id": document["board_id"]}))["document"].clone()
    };
    coordinator.retry_save(&failed.turn_id, NOW).unwrap();
    assert_eq!(board(&state.lock().unwrap(), "read", json!({"board_id": document["board_id"]}))["document"], persisted);
    assert_eq!(requests.lock().unwrap().len(), calls, "save retries must reuse the finished model result");
}

#[test]
fn board_root_conversation_delivers_reopens_and_survives_chat_switch() {
    let (_root, mut state, sources) = super::board_browser_host::fixture();
    let document = board(&state, "create", json!({"title":"从问题开始理解"}));
    assert!(document["items"].as_array().unwrap().is_empty());
    let request = json!({"message":"从整张空白画板开始说明平均速度。", "client_request_id":"board-root-conversation",
        "session_id":sources["session_id"], "expected_chat_head":sources["turn_id"],
        "board_follow_up":{"board_id":document["board_id"],"board_revision":document["revision"]}});
    let prepared = prepare_agent_chat(&mut state, &request.to_string(), NOW)
        .unwrap_or_else(|reply| panic!("{}", reply.body));
    let accepted = prepared.turn_ref.clone();
    let context = prepared.scope.board_context.as_ref().unwrap();
    assert!(context.follow_up.item_id.is_none());
    assert!(context.selected_item.is_none());
    assert_eq!(board(&state,"read",json!({"board_id":document["board_id"]}))["document"],document,
        "accepting an empty board must not create a placeholder root");
    let requests = Arc::new(Mutex::new(Vec::new()));
    let adapter = AnswerAdapter { requests:requests.clone(),fail:false };
    let report = crate::agent_run::execute_prepared(&BorrowedAppPort(RefCell::new(&mut state)), &adapter,
        prepared, CancellationToken::default());
    assert_eq!(report.reply.status,200,"{}",report.reply.body);
    let result: Value = serde_json::from_str(&report.reply.body).unwrap();
    assert_eq!(result["board_delivery"]["board_id"],document["board_id"]);
    assert!(result["board_delivery"]["parent_item_id"].is_null());
    let saved = board(&state,"read",json!({"board_id":document["board_id"]}));
    let items = saved["document"]["items"].as_array().unwrap();
    assert_eq!(items.len(),1,"only the delivered answer belongs on the empty board");
    assert_eq!(items[0]["target"]["kind"],"answer");
    assert_eq!(items[0]["target"]["turn_id"],accepted.turn_id);
    assert!(items[0]["reply_to_item_id"].is_null());
    assert!(!items[0]["readable_content"].as_str().unwrap().is_empty());
    assert_eq!(saved["document"]["deliveries"].as_array().unwrap().len(),1);
    let calls = requests.lock().unwrap().len();
    let replay = match prepare_agent_chat(&mut state,&request.to_string(),NOW) {
        Err(reply)=>reply, Ok(_)=>panic!("the accepted root request must replay without another run"),
    };
    assert_eq!(replay.status,200,"{}",replay.body);
    assert_eq!(serde_json::from_str::<Value>(&replay.body).unwrap()["turn_id"],accepted.turn_id);
    assert_eq!(requests.lock().unwrap().len(),calls);
    assert_eq!(route_agent_new(&mut state,NOW).status,200);
    assert_ne!(state.workspace.selected_chat.as_deref(),Some(accepted.session_id.as_str()));
    let discussion = board(&state,"conversation",json!({"board_id":document["board_id"]}));
    assert_eq!(discussion["sessions"].as_array().unwrap().len(),1);
    assert_eq!(discussion["sessions"][0]["session_id"],accepted.session_id);
    assert_eq!(discussion["sessions"][0]["turns"].as_array().unwrap().len(),1);
    assert_eq!(discussion["sessions"][0]["turns"][0]["turn_id"],accepted.turn_id);
    assert_eq!(discussion["sessions"][0]["turns"][0]["user"],request["message"]);
    assert_eq!(discussion["sessions"][0]["turns"][0]["status"],"completed");
    // Reopen the fixture's actual snapshot history and BoardStore, then replay
    // the delivery repair path without changing its content revision or items.
    assert!(state.user.session_store.is_none());
    state.user.agent_history = load_agent_history(&state.user.history_path).unwrap();
    crate::board_delivery::repair(&mut state.user,NOW).unwrap();
    assert_eq!(board(&state,"read",json!({"board_id":document["board_id"]})),saved);
    assert_eq!(board(&state,"conversation",json!({"board_id":document["board_id"]})),discussion);
    assert_eq!(requests.lock().unwrap().len(),calls);
}

#[test]
fn wb2_local_run_deleted_parent_is_reported_without_resurrection() {
    let (_root, mut state, document, _) = setup();
    let request = input(&state,&document,0,"wb2-deleted");
    let prepared = prepare_agent_chat(&mut state,&request.to_string(),NOW);
    let prepared = prepared.unwrap_or_else(|r|panic!("{}",r.body));
    let reference = prepared.turn_ref.clone();
    board(&state,"update",json!({"board_id":document["board_id"],"board_revision":document["revision"],
        "remove_item_ids":[document["items"][0]["item_id"]]}));
    let adapter=AnswerAdapter { requests:Arc::default(),fail:false };
    let report=crate::agent_run::execute_prepared(&BorrowedAppPort(RefCell::new(&mut state)),&adapter,prepared,CancellationToken::default());
    assert_ne!(report.reply.status,200,"{}",report.reply.body);
    let result:Value=serde_json::from_str(&report.reply.body).unwrap();
    assert_eq!(result["error_code"], "BOARD_DELIVERY_UNSAVED");
    let turn = state.user.agent_history.sessions.iter().find(|session| session.id == reference.session_id).unwrap()
        .turns.iter().find(|turn| turn.turn_id == reference.turn_id).unwrap();
    assert!(turn.board_delivery.is_none());
    assert!(turn.board_delivery_error.is_some());
    assert_eq!(board(&state,"read",json!({"board_id":document["board_id"]}))["document"]["items"].as_array().unwrap().len(),1);
}

#[test]
fn wb2_local_cancelled_and_failed_runs_do_not_deliver() {
    for cancel in [true,false] {
        let (_root,mut state,document,_) = setup();
        let request=input(&state,&document,0,if cancel {"wb2-cancel"} else {"wb2-fail"});
        let prepared=prepare_agent_chat(&mut state,&request.to_string(),NOW).unwrap_or_else(|r|panic!("{}",r.body));
        let turn_ref=prepared.turn_ref.clone();
        let token=CancellationToken::default();
        if cancel { token.cancel(); }
        let adapter=AnswerAdapter {requests:Arc::default(),fail:!cancel};
        let report=crate::agent_run::execute_prepared(&BorrowedAppPort(RefCell::new(&mut state)),&adapter,prepared,token);
        assert_ne!(report.reply.status,200);
        let turn=state.user.agent_history.sessions.iter().find(|s|s.id==turn_ref.session_id).unwrap().turns.iter().find(|t|t.turn_id==turn_ref.turn_id).unwrap();
        assert_eq!(turn.status,if cancel {AgentAssistantStatus::Cancelled} else {AgentAssistantStatus::Failed});
        assert!(turn.board_delivery.is_none());
        crate::board_delivery::repair(&mut state.user,NOW).unwrap();
        assert_eq!(board(&state,"read",json!({"board_id":document["board_id"]}))["document"],document);
    }
}

#[test]
fn wb2_normalized_request_includes_exact_nested_receipt_and_preserves_ordinary_input() {
    let plain=json!({"message":"same"});
    let normalized=crate::run_input::normalize("workspace",&plain).unwrap();
    assert!(normalized.get("board_follow_up").is_none());
    let mut board=plain;
    board["board_follow_up"]=json!({"board_id":"b","board_revision":3,"item_id":"a",
        "presentation":{"session_id":"old","turn_id":"turn","presentation_id":"scene","revision":2,"state_revision":4}});
    let first=crate::run_input::normalize("workspace",&board).unwrap();
    assert_eq!(first["board_follow_up"],board["board_follow_up"]);
    board["board_follow_up"]["presentation"]["state_revision"]=json!(5);
    assert_ne!(first,crate::run_input::normalize("workspace",&board).unwrap());
}
