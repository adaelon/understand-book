pub use reader_service::tests::video_draft_tests::*;
use super::*;
use crate::video_draft;


#[test]
fn video_board_questions_deliver_and_reopen_without_consuming_player_drafts() {
    use super::mu5_tests::Fixture;
    let f = Fixture::new();
    let input_dir = f.root.path().join("video-board-input");
    fixture(&input_dir);
    let reference = f.access.library.lock().unwrap().publish(&input_dir).unwrap().reference;
    f.access.library.lock().unwrap().grant("A", &reference).unwrap();
    let calls = Arc::new(Mutex::new(Vec::<String>::new()));
    #[derive(Clone)]
    struct Answer(Arc<Mutex<Vec<String>>>);
    impl ModelAdapter for Answer {
        fn complete(&self, _: CompletionRequest) -> Result<ParsedResponse, AdapterError> { unreachable!() }
        fn chat(&self, request: &AgentRequestPlan) -> Result<AssistantTurn, AdapterError> {
            self.0.lock().unwrap().push(json!(request.ordered_messages()).to_string());
            Ok(AssistantTurn { provider_continuation: None, text: Some("缓存保留已经取得的数据，重复请求可以直接复用。".into()),
                tool_calls: vec![], usage_total_tokens: Some(1) })
        }
    }
    let recorded = calls.clone();
    f.access.runs.configure_fake(move || Box::new(Answer(recorded.clone())));
    let w = f.create(&f.a, &reference, "board");
    let w = f.action(&f.a, &w, "board", "chat/new", json!({}));
    let w = f.action(&f.a, &w, "board", "boards/create", json!({"title":"为什么需要缓存呢"}));
    let document = w["result"].clone();
    let p = f.access.library.lock().unwrap().load("A", &reference).unwrap();
    let mut player = reader_workspace::ReaderWorkspace::local(p.directory().into(), p.book.clone(),
        Reader::new(&p.book, DEFAULT_RADIUS), vec![], None);
    player.publication = Some(p);
    let player_draft = draft(&player, "unrelated-player-draft", 492000.0);
    let mut w = f.action(&f.a, &w, "board", "reader/video_draft",
        json!({"window_id":"player","action":"save","draft":player_draft}));
    let url = format!("/api/workspaces/{}/agent/runs", w["workspace_id"].as_str().unwrap());
    let mut request = Fixture::stamp(&w, "board");
    request["session_id"] = w["selected_chat"].clone();
    request["expected_chat_head"] = Value::Null;
    request["client_request_id"] = json!("video-player-without-draft");
    request["message"] = json!("为什么需要缓存呢");
    let (status, rejected) = f.call(&f.a, "POST", &url, request.clone());
    assert_eq!(status, 400, "ordinary video questions still require their input-time frame: {rejected}");
    assert_eq!(rejected["error_code"], "VIDEO_DRAFT_INVALID");
    assert!(calls.lock().unwrap().is_empty());

    request["client_request_id"] = json!("video-board-root");
    request["board_follow_up"] = json!({"board_id":document["board_id"],"board_revision":document["revision"]});
    let mut missing = request.clone();
    missing["client_request_id"] = json!("video-board-missing");
    missing["board_follow_up"]["board_id"] = json!("missing-board");
    assert_ne!(f.call(&f.a, "POST", &url, missing).0, 202, "a board must resolve before bypassing player context");

    let (status, accepted) = f.call(&f.a, "POST", &url, request.clone());
    assert_eq!(status, 202, "a board question has its own context and no player draft: {accepted}");
    assert!(f.access.runs.run_one(&f.access).unwrap());
    w = f.get(&f.a, &w);
    let history = f.action(&f.a, &w, "board", "chat/history", json!({}));
    let turn = &history["result"]["current"]["turns"][0];
    assert_eq!(turn["status"], "completed", "{turn}");
    assert!(turn["video_question"].is_null());
    assert_eq!(turn["board_follow_up"], request["board_follow_up"]);
    let saved = f.action(&f.a, &w, "board", "boards/read", json!({"board_id":document["board_id"]}));
    let saved = &saved["result"]["document"];
    assert_eq!(saved["items"].as_array().unwrap().len(), 1);
    assert_eq!(saved["items"][0]["target"]["turn_id"], accepted["turn_id"]);
    let retained = f.action(&f.a, &w, "board", "reader/video_draft", json!({"window_id":"player","action":"get"}));
    assert_eq!(retained["result"]["draft"], player_draft, "board discussion must not consume the player's unfinished question");

    let mut follow_up = Fixture::stamp(&w, "board");
    follow_up["session_id"] = w["selected_chat"].clone();
    follow_up["expected_chat_head"] = accepted["turn_id"].clone();
    follow_up["client_request_id"] = json!("video-board-follow-up");
    follow_up["message"] = json!("那缓存有什么代价？");
    follow_up["board_follow_up"] = json!({"board_id":saved["board_id"],"board_revision":saved["revision"],
        "item_id":saved["items"][0]["item_id"]});
    let (status, next) = f.call(&f.a, "POST", &url, follow_up);
    assert_eq!(status, 202, "{next}");
    assert!(f.access.runs.run_one(&f.access).unwrap());
    let reopened = f.create(&f.a, &reference, "reopened");
    let discussion = f.action(&f.a, &reopened, "reopened", "boards/conversation", json!({"board_id":document["board_id"]}));
    let turns = discussion["result"]["sessions"][0]["turns"].as_array().unwrap();
    assert_eq!(turns.len(), 2, "{discussion}");
    assert!(turns.iter().all(|turn| turn["status"] == "completed" && turn["video_question"].is_null()));
    let reopened = f.action(&f.a, &reopened, "reopened", "boards/read", json!({"board_id":document["board_id"]}));
    assert_eq!(reopened["result"]["document"]["items"].as_array().unwrap().len(), 2);
    let calls = calls.lock().unwrap();
    assert_eq!(calls.len(), 2);
    assert!(calls.iter().all(|request| !request.contains("unrelated-player-draft")));
}

#[test]
fn video_draft_network_queue_reconnect_cancel_and_two_windows() {
    use super::mu5_tests::Fixture;
    let f = Fixture::new(); let input_dir=f.root.path().join("video-input"); fixture(&input_dir);
    let reference = f.access.library.lock().unwrap().publish(&input_dir).unwrap().reference;
    f.access.library.lock().unwrap().grant("A",&reference).unwrap();
    #[derive(Clone)]
    struct Recording(Arc<Mutex<Vec<String>>>);
    impl ModelAdapter for Recording {
        fn complete(&self, _: CompletionRequest) -> Result<ParsedResponse, AdapterError> { Err(AdapterError { spend_stop:None, message:"fixture".into() }) }
        fn chat(&self, p: &AgentRequestPlan) -> Result<AssistantTurn, AdapterError> {
            self.0.lock().unwrap().push(json!({"messages":p.ordered_messages(),"question_image":p.question_image}).to_string());
            Ok(AssistantTurn { provider_continuation:None, text:Some("你好！".into()),tool_calls:vec![],usage_total_tokens:Some(1) })
        }
    }
    let calls = Arc::new(Mutex::new(vec![])); let recorder = Recording(calls.clone());
    f.access.runs.configure_fake(move ||Box::new(recorder.clone()));
    let mut a=f.create(&f.a,&reference,"a"); a=f.action(&f.a,&a,"a","chat/new",json!({}));
    let mut b=f.create(&f.a,&reference,"b"); b=f.action(&f.a,&b,"b","chat/new",json!({}));
    let make = |id: &str,time:f64| {
        let p=f.access.library.lock().unwrap().load("A",&reference).unwrap();
        let mut w=reader_workspace::ReaderWorkspace::local(p.directory().into(),p.book.clone(),Reader::new(&p.book,DEFAULT_RADIUS),vec![],None);
        w.publication=Some(p);
        let mut d = draft(&w,id,time);
        d["context"]["image"] = question_image();
        d["context"]["range"] = json!({"video":d["context"]["material"]["video"],"start_ms":400000,"end_ms":509000});
        d
    };
    a=f.action(&f.a,&a,"a","reader/video_draft",json!({"window_id":"window","action":"save","draft":make("a-draft",492000.0)}));
    b=f.action(&f.a,&b,"b","reader/video_draft",json!({"window_id":"window","action":"save","draft":make("b-draft",507000.0)}));
    // The same browser window key in two distinct workspaces must not cross-bind.
    let mut input=Fixture::stamp(&a,"a");
    input["session_id"]=a["selected_chat"].clone(); input["client_request_id"]=json!("video-request");
    input["message"]=json!("Explain this code");input["video_window_id"]=json!("window");input["video_draft_id"]=json!("a-draft");
    let url=format!("/api/workspaces/{}/agent/runs",a["workspace_id"].as_str().unwrap());
    let (status,accepted)=f.call(&f.a,"POST",&url,input.clone()); assert_eq!(status,202,"{accepted}");
    assert_eq!(f.get(&f.a,&a)["revision"], a["revision"], "draft handoff must not supersede the accepted Run's reader effects");
    assert_eq!(f.call(&f.a,"POST",&url,input).1["turn_id"],accepted["turn_id"]);
    // Reload registry from its durable checkpoint, with accepted turn held in the session log.
    *f.access.workspaces.lock().unwrap()=crate::workspace_registry::WorkspaceRegistry::new(crate::control_store::ControlStore::open(f.access.users.lock().unwrap().writer()).unwrap());
    let a=f.get(&f.a,&a); let a=f.action(&f.a,&a,"reconnected","attach",json!({}));
    let restored=f.action(&f.a,&a,"reconnected","reader/video_draft",json!({"window_id":"window","action":"get"}));
    assert!(restored["result"]["draft"].is_null());
    let b=f.get(&f.a,&b);let b=f.action(&f.a,&b,"b","attach",json!({}));
    let restored=f.action(&f.a,&b,"b","reader/video_draft",json!({"window_id":"window","action":"get"}));
    assert_eq!(restored["result"]["draft"]["context"]["frame"]["actual_pts_ms"],507000.0);
    assert_eq!(restored["result"]["draft"]["context"]["range"]["start_ms"],400000.0);
    let cancelled=f.call(&f.a,"POST",&format!("/api/agent/runs/{}/cancel",accepted["turn_id"].as_str().unwrap()),json!({}));
    assert!(cancelled.0<300,"{:?}",cancelled);
    f.access.runs.run_one(&f.access).unwrap();
    let history=f.action(&f.a,&f.get(&f.a,&a),"reconnected","chat/history",json!({}));
    let turn=&history["result"]["current"]["turns"][0];
    assert_eq!(turn["video_question"]["frame"]["actual_pts_ms"],492000.0);
    assert_eq!(turn["video_question"]["publication"],json!(reference));
    assert_eq!(turn["video_question"]["range"]["end_ms"],509000.0);
    assert_eq!(turn["status"],"cancelled");
    let b=f.get(&f.a,&b); let mut input=Fixture::stamp(&b,"b");
    input["session_id"]=b["selected_chat"].clone(); input["client_request_id"]=json!("video-request-b");
    input["message"]=json!("你好"); input["video_window_id"]=json!("window");input["video_draft_id"]=json!("b-draft");
    let result=f.call(&f.a,"POST",&format!("/api/workspaces/{}/agent/runs",b["workspace_id"].as_str().unwrap()),input);
    assert_eq!(result.0,202,"{:?}",result);
    f.access.runs.recover(&f.access).unwrap();
    assert!(f.access.runs.run_one(&f.access).unwrap());
    let captured=calls.lock().unwrap(); assert!(!captured.is_empty());
    assert!(captured[0].contains("507000")); assert!(captured[0].contains("b-draft"));
    assert!(captured[0].contains("400000")); assert!(captured[0].contains("509000"));
    assert!(!captured[0].contains("a-draft"));
}