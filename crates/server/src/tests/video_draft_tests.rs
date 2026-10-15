use super::*;
use crate::video_draft;

pub(super) fn fixture(dir: &Path) -> Book {
    std::fs::create_dir_all(dir.join("media")).unwrap();
    let video = json!({"media_id":"v","media_revision":"1"});
    let transcript = json!({"transcript_id":"t","transcript_revision":"1","video":video});
    let location = json!({"video":video,"start_ms":0,"end_ms":510000});
    let base = json!({"book_id":"vi6","lid_nodes":[{"lid":"1","path":[1],"kind":"paragraph","span":{"start":0,"end":8},"children":[]}],"graph_nodes":[],"graph_edges":[]});
    std::fs::write(dir.join("base.json"), base.to_string()).unwrap();
    std::fs::write(dir.join("source.txt"), "A frame.").unwrap();
    std::fs::write(dir.join("media/original.mp4"), b"fixture media").unwrap();
    std::fs::write(dir.join("raw.json"), "{}").unwrap();
    std::fs::write(dir.join("video_source_manifest.json"),json!({"version":"video_source_manifest.v1","material":{"book_id":"vi6","video":video,"transcript":transcript},"source":{"video":video,"imported_path":"media/original.mp4","original_path":"fixture.mp4","duration_ms":510000,"width":1920,"height":1080},"canonical_text_path":"source.txt","transcript_map_path":"transcript_map.json","coverage":[{"capability":"transcript","accepted":[location],"pending":[],"failed":[]}],"previous_book_id":null}).to_string()).unwrap();
    std::fs::write(dir.join("transcript_map.json"),json!({"transcript":transcript,"raw_result_path":"raw.json","machine_transcription":{"engine":"fixture","model":"fixture","language":"en"},"segments":[{"segment_id":"s","text":"A frame.","location":location,"text_range":{"lid":"1","range":{"start":0,"end":8}},"words":[]}]}).to_string()).unwrap();
    std::fs::write(dir.join("publication.json"),json!({"version":"publication.v1","reference":{"book_id":"vi6","publication_id":"original"},"source_fingerprint":"fixture-existing-contract","files":{},"capabilities":{},"artifact_versions":{},"teaching_readiness":{},"build_readiness":{},"resident_bytes":0}).to_string()).unwrap();
    Book::load(dir.to_str().unwrap()).unwrap()
}
fn draft(w: &reader_workspace::ReaderWorkspace, id: &str, time: f64) -> Value {
    let (m, _) = w.book.load_video_source(&w.book_dir).unwrap();
    json!({"message":"Explain this code", "context":{"draft_id":id,"publication":video_draft::publication(w).unwrap(),"material":m.material,
        "frame":{"video":m.source.video,"actual_pts_ms":time,"width":1920,"height":1080},"region":null,"range":null}})
}
#[test]
fn video_draft_local_failure_restore_handoff_and_history() {
    let root = tempfile::tempdir().unwrap(); let book = fixture(root.path());
    let mut state = state_named("vi6-local");
    state.workspace.book = Arc::new(book); state.workspace.book_dir = root.path().into();
    state.workspace.reader = Reader::new(&state.workspace.book, DEFAULT_RADIUS);
    state.workspace.session_path = Some(root.path().join("session.json"));
    state.workspace.restore_chat(&mut state.user.agent_history, "t0");
    let d = draft(&state.workspace, "d1", 492000.0);
    let command = json!({"window_id":"a","action":"save","draft":d});
    let reply = route(&mut state, Req { method:"POST", url:"/reader/video_draft",body:&command.to_string(),now:"t0" });
    assert_eq!(reply.status,200,"{}",reply.body);
    assert_eq!(video_draft::load(&state.workspace.session_path)["a"].context.frame.actual_pts_ms,492000.0);
    let other = draft(&state.workspace,"d2",507000.0);
    video_draft::command(&mut state.workspace,&state.user,&json!({"window_id":"b","action":"save","draft":other})).unwrap();
    let input = json!({"message":"Explain this code","video_window_id":"a","video_draft_id":"d1"});
    let mut invalid = input.clone(); invalid["video_draft_id"]=json!("missing");
    assert!(prepare_agent_chat(&mut state,&invalid.to_string(),"t1").is_err());
    assert_eq!(state.workspace.video_drafts["a"].context.frame.actual_pts_ms,492000.0);
    let prepared = prepare_agent_chat(&mut state,&input.to_string(),"t2").unwrap_or_else(|r|panic!("{}",r.body));
    assert_eq!(prepared.scope.reader_input.video_question.as_ref().unwrap().frame.actual_pts_ms,492000.0);
    assert_eq!(prepared.scope.reader_input.video_question.as_ref().unwrap().publication.publication_id,"original");
    let session = state.user.agent_history.sessions.iter().find(|s|s.id==prepared.turn_ref.session_id).unwrap();
    let restored: AgentChatSession = serde_json::from_str(&serde_json::to_string(session).unwrap()).unwrap();
    assert_eq!(turn_view(&prepared.scope.book,&restored.turns[0]).video_question.as_ref().unwrap().frame.actual_pts_ms,492000.0);
    assert_eq!(restored.turns[0].published_book_ref.as_ref().unwrap().publication_id,"original");
    assert!(video_draft::command(&mut state.workspace,&state.user,&json!({"window_id":"a","action":"get"})).unwrap()["draft"].is_null());
    assert!(!video_draft::load(&state.workspace.session_path).contains_key("a"));
    assert_eq!(state.workspace.video_drafts["b"].context.frame.actual_pts_ms,507000.0);
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
            self.0.lock().unwrap().push(serde_json::to_string(&p.ordered_messages()).unwrap());
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

#[test]
fn video_draft_range_edits_restore_and_handoff_independent_region() {
    let root = tempfile::tempdir().unwrap(); let book = fixture(root.path());
    let mut state = state_named("vi8b-local");
    state.workspace.book = Arc::new(book); state.workspace.book_dir = root.path().into();
    state.workspace.reader = Reader::new(&state.workspace.book, DEFAULT_RADIUS);
    state.workspace.session_path = Some(root.path().join("session.json"));
    state.workspace.restore_chat(&mut state.user.agent_history, "t0");
    let mut d = draft(&state.workspace, "range", 492000.0);
    d["message"] = json!("");
    d["context"]["range"] = json!({"video":d["context"]["material"]["video"],"start_ms":400000,"end_ms":510000});
    let save = |state: &mut AppState, d: &Value| route(state, Req { method:"POST", url:"/reader/video_draft",
        body: &json!({"window_id":"a","action":"save","draft":d}).to_string(), now:"t0" });
    assert_eq!(save(&mut state, &d).status, 200, "range-only is content; it can exceed one read batch");
    let region = json!({"frame":d["context"]["frame"],"rect_normalized":[0.125,0.25,0.5,0.25]});
    d["context"]["region"] = region.clone(); d["context"]["range"]["start_ms"] = json!(410000);
    assert_eq!(save(&mut state, &d).status, 200);
    d["context"]["range"] = Value::Null;
    assert_eq!(save(&mut state, &d).status, 200, "region keeps binding on range cancellation");
    d["context"]["range"] = json!({"video":d["context"]["material"]["video"],"start_ms":0,"end_ms":90000});
    assert_eq!(save(&mut state, &d).status, 200, "range may be independent of focused frame");
    for (start, end) in [(90000, 90000), (90000, 0), (-1, 90000), (0, 510001)] {
        let mut invalid = d.clone(); invalid["context"]["range"]["start_ms"] = json!(start); invalid["context"]["range"]["end_ms"] = json!(end);
        assert_ne!(save(&mut state, &invalid).status, 200);
    }
    let mut invalid = d.clone(); invalid["context"]["range"]["video"]["media_id"] = json!("other");
    assert_ne!(save(&mut state, &invalid).status, 200);
    let restored = video_draft::load(&state.workspace.session_path);
    assert_eq!(restored["a"].context.range.as_ref().unwrap().end_ms, 90000.0);
    assert_eq!(serde_json::to_value(&restored["a"].context.region).unwrap(), region);
    let input = json!({"message":"Explain process","video_window_id":"a","video_draft_id":"range"});
    let prepared = prepare_agent_chat(&mut state, &input.to_string(), "t1").unwrap_or_else(|r| panic!("{}", r.body));
    let context = prepared.scope.reader_input.video_question.as_ref().unwrap();
    assert_eq!(context.range, restored["a"].context.range); assert_eq!(context.frame.actual_pts_ms, 492000.0);
    assert!(video_draft::load(&state.workspace.session_path).is_empty());
}

#[test]
fn video_draft_region_edit_preserves_binding_and_handoff() {
    let root = tempfile::tempdir().unwrap(); let book = fixture(root.path());
    let mut state = state_named("vi8a-local");
    state.workspace.book = Arc::new(book); state.workspace.book_dir = root.path().into();
    state.workspace.reader = Reader::new(&state.workspace.book, DEFAULT_RADIUS);
    state.workspace.session_path = Some(root.path().join("session.json"));
    state.workspace.restore_chat(&mut state.user.agent_history, "t0");
    let mut d = draft(&state.workspace, "selected", 492000.0);
    let save = |state: &mut AppState, d: &Value| route(state, Req { method:"POST", url:"/reader/video_draft",
        body: &json!({"window_id":"a","action":"save","draft":d}).to_string(), now:"t0" });
    assert_eq!(save(&mut state, &d).status, 200);
    let region = json!({"frame":d["context"]["frame"],"rect_normalized":[0.125,0.25,0.5,0.25]});
    d["context"]["region"] = region.clone();
    assert_eq!(save(&mut state, &d).status, 200);
    d["context"]["region"] = Value::Null;
    assert_eq!(save(&mut state, &d).status, 200, "text-only draft keeps its frame");
    d["message"] = json!("");
    assert_ne!(save(&mut state, &d).status, 200, "empty draft must use release");
    d["context"]["region"] = region.clone();
    assert_eq!(save(&mut state, &d).status, 200, "selection-only draft is content");
    let restored = video_draft::load(&state.workspace.session_path);
    assert_eq!(serde_json::to_value(&restored["a"].context.region).unwrap(), region);
    for field in ["draft_id", "publication", "material", "frame"] {
        let mut changed = d.clone();
        match field {
            "draft_id" => changed["context"][field] = json!("new-draft"),
            "publication" => changed["context"][field]["publication_id"] = json!("other"),
            "material" => changed["context"][field]["transcript"]["transcript_revision"] = json!("2"),
            "frame" => {
                changed["context"]["frame"]["actual_pts_ms"] = json!(507000.0);
                changed["context"]["region"]["frame"] = changed["context"]["frame"].clone();
            },
            _ => changed["context"][field] = json!({"video":d["context"]["frame"]["video"],"start_ms":0,"end_ms":1000}),
        }
        assert_ne!(save(&mut state, &changed).status, 200, "must not rebind {field}");
    }
    let mut changed = d.clone(); changed["context"]["region"]["rect_normalized"] = json!([0.9,0.0,0.5,0.5]);
    assert_ne!(save(&mut state, &changed).status, 200);
    let input = json!({"message":"Explain selected code","video_window_id":"a","video_draft_id":"selected"});
    let prepared = prepare_agent_chat(&mut state, &input.to_string(), "t1").unwrap_or_else(|r| panic!("{}", r.body));
    assert_eq!(serde_json::to_value(&prepared.scope.reader_input.video_question.as_ref().unwrap().region).unwrap(), region);
    assert!(video_draft::load(&state.workspace.session_path).is_empty());
}
