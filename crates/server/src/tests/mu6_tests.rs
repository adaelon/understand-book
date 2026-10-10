use super::mu5_tests::Fixture;
use super::*;
use crate::{
    authorization::{Authorization, AuthorizedContext},
    control_store::ControlStore,
    multi_user_host::{self, Site},
    user_registry::UserRegistry,
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Condvar,
};

#[path = "cache_observation_tests.rs"]
mod cache_observation_tests;

#[derive(Clone, Default)]
struct Probe {
    calls: Arc<AtomicUsize>,
    messages: Arc<Mutex<Vec<Vec<Message>>>>,
    block: Option<Arc<(Mutex<bool>, Condvar)>>,
    note: bool,
    finish_note: bool,
    tokens_per_call: u32,
}
impl ModelAdapter for Probe {
    fn complete(&self, _: CompletionRequest) -> Result<ParsedResponse, AdapterError> {
        Err(AdapterError {
            spend_stop: None,
            message: "fixture".into(),
        })
    }
    fn chat(&self, request: &AgentRequestPlan) -> Result<AssistantTurn, AdapterError> {
        let n = self.calls.fetch_add(1, Ordering::SeqCst);
        self.messages
            .lock()
            .unwrap()
            .push(request.ordered_messages());
        if self.note && n == 0 {
            return Ok(AssistantTurn {
                provider_continuation: None,
                text: None,
                tool_calls: vec![runtime::ToolCall {
                    id: "note".into(),
                    name: "reader.note".into(),
                    arguments: json!({"lid":"1.1","text":"mu6 durable note"}).to_string(),
                }],
                usage_total_tokens: Some(self.tokens_per_call.max(1)),
            });
        }
        if let Some(gate) = &self.block {
            let (lock, cv) = &**gate;
            let guard = lock.lock().unwrap();
            let (_guard, timeout) = cv
                .wait_timeout_while(guard, Duration::from_secs(15), |open| !*open)
                .unwrap();
            assert!(!timeout.timed_out(), "test did not release model");
        }
        if self.note && !self.finish_note {
            return Err(AdapterError {
                spend_stop: None,
                message: "fixture exit".into(),
            });
        }
        Ok(AssistantTurn {
            provider_continuation: None,
            text: Some("你好！".into()),
            tool_calls: vec![],
            usage_total_tokens: Some(self.tokens_per_call.max(1)),
        })
    }
}
fn configure(f: &Fixture, probe: &Probe) {
    let probe = probe.clone();
    f.access
        .runs
        .configure_fake(move || Box::new(probe.clone()));
}
fn context(f: &Fixture, token: &str) -> AuthorizedContext {
    f.access
        .context(
            f.access
                .auth
                .authenticate(token, multi_user_host::now())
                .unwrap(),
            multi_user_host::now(),
        )
        .unwrap()
}
fn setup() -> (Fixture, Value, Value, Probe) {
    let f = Fixture::new();
    let probe = Probe::default();
    configure(&f, &probe);
    let w = f.create(&f.a, &f.x, "page");
    let w = f.action(&f.a, &w, "page", "chat/new", json!({}));
    let mut input = Fixture::stamp(&w, "page");
    input["session_id"] = w["selected_chat"].clone();
    input["client_request_id"] = json!("key");
    input["message"] = json!("你好");
    (f, w, input, probe)
}
fn jsonl_path(f: &Fixture) -> PathBuf {
    let handle = f.access.users.lock().unwrap().get("A", "now").unwrap();
    let user = handle.lock().unwrap();
    assert!(user.session_store.is_some());
    user.history_path.clone().unwrap()
}

#[test]
fn jl7_old_active_chat_and_pending_admissions_close_without_execution() {
    for stage in ["queued", "claimed"] {
        let (f, w, input, probe) = setup();
        assert_eq!(admit(&f, &w, &input).0, 202);
        if stage == "claimed" {
            *f.access.runs.fail_after.lock().unwrap() = Some("claimed");
            f.access.runs.run_one(&f.access).unwrap_err();
        }
        let path = jsonl_path(&f);
        let bytes = {
            let handle = f.access.users.lock().unwrap().get("A", "now").unwrap();
            let user = handle.lock().unwrap();
            serde_json::to_vec(&user.agent_history).unwrap()
        };
        std::fs::write(&path, &bytes).unwrap();
        // Simulate a pre-JSONL installation with only an old snapshot and its
        // control-plane references. No new-log facts exist for that old run.
        let paths = crate::session_store::SessionPaths::from_history(&path);
        std::fs::remove_file(paths.session(input["session_id"].as_str().unwrap())).unwrap();
        let f = reopen(f, &probe);
        assert_eq!(row(&f, "key")["stage"], "admission_failed");
        assert_eq!(row(&f, "key")["unsaved"], false);
        assert_eq!(admit(&f, &w, &input).1["error_code"], "REQUEST_KEY_CLOSED");
        assert!(!f.access.runs.run_one(&f.access).unwrap());
        let current = f.action(&f.a, &w, "new-page", "attach", json!({}));
        assert!(current["selected_chat"].is_null());
        let fresh = f.action(&f.a, &current, "new-page", "chat/new", json!({}));
        assert_ne!(fresh["selected_chat"], input["session_id"]);
        assert_eq!(probe.calls.load(Ordering::SeqCst), 0);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
}

#[test]
fn jl3_admission_windows_recover_and_repeat_original_request() {
    for point in ["preparing", "pending", "prepared", "queued", "claimed", "terminal"] {
        let (f, w, input, probe) = setup();
        let path = jsonl_path(&f);
        let original = std::fs::read(&path).ok();
        if matches!(point, "claimed" | "terminal") {
            assert_eq!(admit(&f, &w, &input).0, 202);
            *f.access.runs.fail_after.lock().unwrap() = Some(point);
            assert_eq!(f.access.runs.run_one(&f.access).unwrap_err().error_code, "INJECTED_CRASH");
        } else {
            *f.access.runs.fail_after.lock().unwrap() = Some(point);
            assert_eq!(admit(&f, &w, &input).0, 503, "{point}");
        }
        let id = row(&f, "key")["turn"].clone();
        let calls = probe.calls.load(Ordering::SeqCst);
        let f = reopen(f, &probe);
        if point == "preparing" {
            assert_eq!(admit(&f, &w, &input).1["error_code"], "REQUEST_KEY_CLOSED");
        } else {
            assert_eq!(admit(&f, &w, &input).1["turn_id"], id);
            let mut different = input.clone(); different["message"] = json!("changed");
            assert_eq!(admit(&f, &w, &different).1["error_code"], "REQUEST_KEY_CONFLICT");
            if matches!(point, "claimed" | "terminal") {
                assert!(!f.access.runs.run_one(&f.access).unwrap());
                assert_eq!(probe.calls.load(Ordering::SeqCst), calls);
            } else {
                assert_eq!(row(&f, "key")["stage"], "queued");
                assert!(f.access.runs.run_one(&f.access).unwrap());
            }
            let handle = f.access.users.lock().unwrap().get("A", "now").unwrap();
            let user = handle.lock().unwrap();
            assert_eq!(user.agent_history.sessions[0].turns.len(), 1);
            let log = &user.session_store.as_ref().unwrap().logs[input["session_id"].as_str().unwrap()];
            let events = std::fs::read_to_string(&log.path).unwrap();
            let accepted: Value = events.lines().map(|s| serde_json::from_str::<Value>(s).unwrap()).find(|e| e["kind"] == "turn.accepted").unwrap();
            assert!(accepted["payload"]["input"]["messages"].is_object());
            assert!(accepted["payload"]["turn"]["admission_input"].is_null());
            let mut reopened = crate::session_log::SessionLog::open(log.path.clone()).unwrap();
            reopened.append(serde_json::from_value(accepted).unwrap()).unwrap();
            assert_eq!(std::fs::read_to_string(&log.path).unwrap(),events);
        }
        assert_eq!(std::fs::read(&path).ok(), original);
    }
}

#[test]
fn jl3_queued_input_keeps_committed_reference() {
    let (f, w, input, probe) = setup();
    assert_eq!(admit(&f, &w, &input).0, 202);
    jsonl_path(&f);
    let f = reopen(f, &probe);
    assert_eq!(admit(&f, &w, &input).0, 202);
    assert!(f.access.runs.run_one(&f.access).unwrap());
    let handle = f.access.users.lock().unwrap().get("A", "now").unwrap();
    let user = handle.lock().unwrap();
    let log = &user.session_store.as_ref().unwrap().logs[input["session_id"].as_str().unwrap()];
    assert!(matches!(log.projection.inputs.values().next().unwrap().messages,
        crate::session_event::HistoryPosition::Committed { history_through_seq: 1 }));
    assert_eq!(std::fs::read_to_string(&log.path).unwrap().lines().filter(|l| l.contains("\"normalized\"")).count(), 1);
}

#[test]
fn jl3_accepted_append_size_does_not_repeat_old_messages_or_other_chats() {
    // Understanding observation now contributes a fixed frozen context. Compare
    // equal admissions with and without a large history, rather than its old byte cap.
    let baseline_delta = {
        let (f,w,input,_) = setup();
        let paths = crate::session_store::SessionPaths::from_history(&jsonl_path(&f));
        let path = paths.session(input["session_id"].as_str().unwrap());
        let before = std::fs::metadata(&path).unwrap().len();
        assert_eq!(admit(&f,&w,&input).0,202);
        std::fs::metadata(&path).unwrap().len() - before
    };
    let (f,w,input,_)=setup();
    {
        let handle=f.access.users.lock().unwrap().get("A","now").unwrap();
        let mut user=handle.lock().unwrap();
        let user = &mut *user;
        let chat = input["session_id"].as_str().unwrap();
        let mut messages = user.agent_history.sessions[0].messages.clone();
        messages.push(Message::user("OLD_HISTORY_MARKER ".repeat(5000)));
        let mut answer=Message::user("old answer"); answer.role=runtime::Role::Assistant; messages.push(answer);
        user.session_store.as_mut().unwrap().append(&mut user.agent_history, chat, "old", None,
            crate::session_event::EventBody::HistoryRevised(crate::session_event::MessageRevision { from: 0, suffix: messages })).unwrap();
        let mut other=new_agent_session(&f.x.book_id,"other",1);
        other.id="unrelated-chat".into();
        user.session_store.as_mut().unwrap().create(&mut user.agent_history, other).unwrap();
    }
    let path=jsonl_path(&f);
    let paths=crate::session_store::SessionPaths::from_history(&path);
    let chat_path=paths.session(input["session_id"].as_str().unwrap());
    let before=std::fs::read(&chat_path).unwrap();
    let other=std::fs::read(paths.session("unrelated-chat")).unwrap();
    assert_eq!(admit(&f,&w,&input).0,202);
    let after=std::fs::read(&chat_path).unwrap();
    assert!(after.starts_with(&before));
    assert!((after.len()-before.len()) as u64 <= baseline_delta + 1024);
    assert!(!std::str::from_utf8(&after[before.len()..]).unwrap().contains("OLD_HISTORY_MARKER"));
    assert_eq!(std::fs::read(paths.session("unrelated-chat")).unwrap(),other);
}

#[test]
fn jl3_uncertain_accept_is_settled_before_closing_control_admission() {
    for full in [false,true] {
        let (f,w,input,probe)=setup();jsonl_path(&f);
        {
            let handle=f.access.users.lock().unwrap().get("A","now").unwrap();
            let mut user=handle.lock().unwrap();
            user.session_store.as_mut().unwrap().logs.get_mut(input["session_id"].as_str().unwrap()).unwrap().fail_write=Some(full);
        }
        assert_eq!(admit(&f,&w,&input).0,503);
        assert_eq!(row(&f,"key")["stage"],"admission_failed");
        assert_eq!(probe.calls.load(Ordering::SeqCst),0);
        let turn=row(&f,"key")["turn"].clone();
        let f=reopen(f,&probe);
        assert_eq!(admit(&f,&w,&input).1["turn_id"],turn);
        let current=f.action(&f.a,&w,"new-page","attach",json!({}));
        let mut next=Fixture::stamp(&current,"new-page");
        next["session_id"]=input["session_id"].clone();next["message"]=json!("你好");next["client_request_id"]=json!("next");
        assert_eq!(admit(&f,&current,&next).0,202);
        assert!(f.access.runs.run_one(&f.access).unwrap());
        let handle=f.access.users.lock().unwrap().get("A","now").unwrap();
        let user=handle.lock().unwrap();
        assert_eq!(user.agent_history.sessions[0].turns.len(),2);
        assert_eq!(user.agent_history.sessions[0].turns[0].error.as_ref().unwrap().error_code,"ADMISSION_FAILED");
    }
}

#[test]
#[ignore = "JL4 child process for durable run boundaries"]
fn jl4_kill_child() {
    let (f, w, mut input, mut probe) = setup();
    let path = jsonl_path(&f);
    probe.note = true; probe.finish_note = true;
    configure(&f, &probe);
    input["message"] = json!("请在 1.1 保存笔记");
    std::fs::write(std::env::var_os("JL4_MANIFEST").unwrap(),
        json!({"root":f.root.path(),"history":path,"chat":input["session_id"]}).to_string()).unwrap();
    assert_eq!(admit(&f, &w, &input).0, 202);
    if std::env::var("JL4_KILL_POINT").as_deref() == Ok("checkpoint") {
        // Hold the same real claimed admission while installing a checkpoint.
        *f.access.runs.fail_after.lock().unwrap() = Some("claimed");
        f.access.runs.run_one(&f.access).unwrap_err();
        let handle = f.access.users.lock().unwrap().get("A", "now").unwrap();
        let mut user = handle.lock().unwrap();
        let session = &user.agent_history.sessions[0];
        let reference = AgentTurnRef { session_id:session.id.clone(), turn_id:session.turns[0].turn_id.clone(), user_turn_ordinal:1 };
        let mut messages = session.messages.clone();
        messages.push(Message::user("old objective ".repeat(1200)));
        let mut answer = Message::user("old answer ".repeat(1200)); answer.role = runtime::Role::Assistant;
        messages.push(answer); messages.push(Message::user("current question"));
        let prepared = runtime::prepare_compaction(runtime::CompactionPhase::MidTurn, &messages, &messages,
            vec![],vec![],vec![],BTreeMap::new()).unwrap();
        let ids = prepared.request().eligible_items.iter().map(|s|s.source_item_id.clone()).collect::<Vec<_>>();
        let generator = CompactionDraftAdapter { output: RefCell::new(Some(json!({
            "active_goal":[{"item_id":"item.goal","text":"Continue objective","source_item_ids":ids,"evidence_refs":[]}],
            "progress":[],"decisions":[],"user_constraints":[],"open_obligations":[],"unresolved_ambiguities":[],"critical_facts":[],"critical_examples":[],"next_steps":[],
            "source_coverage":ids.iter().map(|id|json!({"source_item_id":id,"disposition":"compacted","target_item_ids":["item.goal"]})).collect::<Vec<_>>()
        }))) };
        let profile = runtime::ModelRuntimeProfile::fallback("fixture", runtime::ProviderToolProtocol::Native);
        let checkpoint = runtime::compact_with_adapter(&generator,&profile,&prepared,
            runtime::CompactionLimits { generation_input_limit_tokens:100_000,target_active_tokens:20_000 }).unwrap();
        crate::session_runtime::checkpoint(&mut user,&reference,&checkpoint,&messages).unwrap();
    } else {
        f.access.runs.run_one(&f.access).unwrap();
    }
    panic!("durable boundary was not reached");
}

#[test]
fn jl4_process_kill_preserves_messages_and_never_reexecutes_tools() {
    struct Child(std::process::Child);
    impl Drop for Child { fn drop(&mut self) { let _=self.0.kill(); let _=self.0.wait(); } }
    for point in ["tool_start", "tool_result", "checkpoint", "before_terminal"] {
        let dir = tempfile::tempdir().unwrap();
        let manifest = dir.path().join("manifest.json");
        let ready = dir.path().join("ready");
        let output = std::fs::File::create(dir.path().join("child.log")).unwrap();
        let mut child = Child(std::process::Command::new(std::env::current_exe().unwrap())
            .args(["jl4_kill_child","--ignored","--nocapture"])
            .env("JL4_KILL_POINT",point).env("MU10_READY",&ready).env("JL4_MANIFEST",&manifest)
            .env("TEMP",dir.path()).env("TMP",dir.path())
            .stdout(output.try_clone().unwrap()).stderr(output).spawn().unwrap());
        let deadline = std::time::Instant::now()+Duration::from_secs(40);
        while !ready.exists() {
            assert!(child.0.try_wait().unwrap().is_none() && std::time::Instant::now()<deadline,
                "{point}: {}",std::fs::read_to_string(dir.path().join("child.log")).unwrap());
            std::thread::sleep(Duration::from_millis(20));
        }
        child.0.kill().unwrap(); child.0.wait().unwrap();
        let manifest: Value = serde_json::from_slice(&std::fs::read(&manifest).unwrap()).unwrap();
        let users = UserRegistry::open(Path::new(manifest["root"].as_str().unwrap())).unwrap();
        let access = Arc::new(Authorization::new(users).unwrap());
        let probe = Probe::default(); let adapter = probe.clone();
        access.runs.configure_fake(move || Box::new(adapter.clone()));
        access.runs.recover(&access).unwrap();
        assert!(!access.runs.run_one(&access).unwrap());
        assert_eq!(probe.calls.load(Ordering::SeqCst),0);
        let handle=access.users.lock().unwrap().get("A","now").unwrap();
        let user=handle.lock().unwrap();
        let session=&user.agent_history.sessions[0];
        assert_eq!(session.turns[0].error.as_ref().unwrap().error_code,"INTERRUPTED");
        let messages=&session.messages;
        for call in messages.iter().flat_map(|m| &m.tool_calls) {
            assert_eq!(messages.iter().filter(|m|m.tool_call_id.as_ref()==Some(&call.id)).count(),1);
        }
        if matches!(point,"tool_result"|"before_terminal") {
            assert!(messages.iter().any(|m|m.tool_call_id.as_deref()==Some("note") && !m.content.as_deref().unwrap_or("").contains("INTERRUPTED")));
        }
        if point=="tool_start" {
            assert!(messages.iter().any(|m|m.tool_call_id.as_deref()==Some("note") && m.content.as_deref().unwrap_or("").contains("INTERRUPTED")));
        }
        if point=="checkpoint" { assert!(session.compaction_checkpoint.is_some()); }
    }
}

#[test]
fn jl4_log_write_failure_stops_tools_and_retry_save_does_not_execute() {
    #[derive(Clone)]
    struct FailingLog { probe: Probe, path: PathBuf, fail_on: usize }
    impl ModelAdapter for FailingLog {
        fn complete(&self, _:CompletionRequest)->Result<ParsedResponse,AdapterError>{unreachable!()}
        fn chat(&self, request:&AgentRequestPlan)->Result<AssistantTurn,AdapterError>{
            let n=self.probe.calls.load(Ordering::SeqCst);
            let result=self.probe.chat(request);
            if n==self.fail_on {
                std::fs::rename(&self.path,self.path.with_extension("held")).unwrap();
                std::fs::create_dir(&self.path).unwrap();
            }
            result
        }
    }
    for fail_on in [0,1] {
        let (f,w,mut input,mut probe)=setup();
        jsonl_path(&f);
        probe.note=true;probe.finish_note=true;
        let path={
            let handle=f.access.users.lock().unwrap().get("A","now").unwrap();
            let user=handle.lock().unwrap();
            user.session_store.as_ref().unwrap().paths.session(input["session_id"].as_str().unwrap())
        };
        let adapter=FailingLog{probe:probe.clone(),path:path.clone(),fail_on};
        f.access.runs.configure_fake(move ||Box::new(adapter.clone()));
        input["message"]=json!("请在 1.1 保存笔记");
        assert_eq!(admit(&f,&w,&input).0,202);
        f.access.runs.run_one(&f.access).unwrap();
        assert_eq!(row(&f,"key")["unsaved"],true);
        assert_eq!(probe.calls.load(Ordering::SeqCst),fail_on+1);
        let handle=f.access.users.lock().unwrap().get("A","now").unwrap();
        let notes=handle.lock().unwrap().store.recall(&memory::RecallQuery{text:Some("mu6 durable note".into()),..Default::default()});
        assert_eq!(notes.is_empty(),fail_on==0);
        std::fs::remove_dir(&path).unwrap();
        std::fs::rename(path.with_extension("held"),&path).unwrap();
        let id=row(&f,"key")["turn"].as_str().unwrap().to_owned();
        f.access.runs.retry_save(&context(&f,&f.a),&id).unwrap();
        assert_eq!(row(&f,"key")["stage"],"settled");
        assert_eq!(probe.calls.load(Ordering::SeqCst),fail_on+1);
    }
}
fn admit(f: &Fixture, w: &Value, input: &Value) -> (u16, Value) {
    f.call(
        &f.a,
        "POST",
        &format!(
            "/api/workspaces/{}/agent/runs",
            w["workspace_id"].as_str().unwrap()
        ),
        input.clone(),
    )
}
fn row(f: &Fixture, key: &str) -> Value {
    f.control.connection.query_row("SELECT turn_id,dispatch_state,cancel_requested,key_closed,unsaved,attempt FROM run_admissions WHERE owner_user_id='A' AND client_request_id=?",[key],|r|Ok(json!({"turn":r.get::<_,String>(0)?,"stage":r.get::<_,String>(1)?,"cancel":r.get::<_,bool>(2)?,"closed":r.get::<_,bool>(3)?,"unsaved":r.get::<_,bool>(4)?,"attempt":r.get::<_,u64>(5)?}))).unwrap()
}
fn reopen(f: Fixture, probe: &Probe) -> Fixture {
    let Fixture {
        root,
        access,
        control,
        x,
        y,
        a,
        b,
    } = f;
    drop(access);
    drop(control);
    let users = UserRegistry::open(root.path()).unwrap();
    let control = ControlStore::open(users.writer()).unwrap();
    let f = Fixture {
        root,
        access: Arc::new(Authorization::new(users).unwrap()),
        control,
        x,
        y,
        a,
        b,
    };
    configure(&f, probe);
    f.access.runs.recover(&f.access).unwrap();
    f
}
fn wait_calls(probe: &Probe, n: usize) {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while probe.calls.load(Ordering::SeqCst) < n {
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn release(probe: &Probe) {
    let gate = probe.block.as_ref().unwrap();
    *gate.0.lock().unwrap() = true;
    gate.1.notify_all();
}

#[test]
fn mu6a_same_key_compares_frozen_fields_and_occupies_chat_once() {
    let (f, w, input, probe) = setup();
    let (status, accepted) = admit(&f, &w, &input);
    assert_eq!(status, 202, "{accepted}");
    let again = admit(&f, &w, &input);
    assert_eq!(again.0, 202);
    assert_eq!(again.1["turn_id"], accepted["turn_id"]);
    for (key, value) in [
        ("message", json!("different")),
        ("question_anchor_lid", json!("1.2")),
        ("goal_action", json!("replace")),
        ("generation", json!(99)),
        ("teaching_ref", json!("different")),
        ("question_quote", json!({"lid":"1.1","quote":"different"})),
    ] {
        let mut changed = input.clone();
        changed[key] = value;
        assert_eq!(
            admit(&f, &w, &changed).1["error_code"],
            "REQUEST_KEY_CONFLICT",
            "{key}"
        );
    }
    let mut other = input.clone();
    other["client_request_id"] = json!("second");
    assert_eq!(admit(&f, &w, &other).1["error_code"], "CHAT_BUSY");
    let user = f.access.users.lock().unwrap().get("A", "now").unwrap();
    assert_eq!(
        user.lock().unwrap().agent_history.sessions[0].turns.len(),
        1
    );
    drop(user);
    f.access.runs.run_one(&f.access).unwrap();
    assert_eq!(row(&f, "key")["stage"], "settled");
    assert!(probe.calls.load(Ordering::SeqCst) > 0);
    let calls = probe.calls.load(Ordering::SeqCst);
    assert_eq!(admit(&f, &w, &input).1["turn_id"], accepted["turn_id"]);
    assert!(!f.access.runs.run_one(&f.access).unwrap());
    assert_eq!(probe.calls.load(Ordering::SeqCst), calls);
    let status = f.ok(
        &f.a,
        "GET",
        &format!("/api/agent/runs/{}", accepted["turn_id"].as_str().unwrap()),
        json!({}),
    );
    assert_eq!(status["turn"]["status"], "completed");
}

#[test]
fn mu9_t64_restored_old_chat_stays_archived_and_jsonl_queued_recovers() {
    let (f,w,input,probe)=setup();
    let accepted=admit(&f,&w,&input);
    assert_eq!(accepted.0,202);
    {
        let handle=f.access.users.lock().unwrap().get("A","now").unwrap();
        let user=handle.lock().unwrap();
        let mut legacy=new_agent_session(&f.x.book_id,"legacy",2);
        legacy.id="legacy-imported-chat".into();
        legacy.turns.push(serde_json::from_value(json!({"turn_id":"legacy-imported-turn","user_turn_ordinal":1,"user":"旧问题","status":"pending_assistant","question_anchor_lid":null,"question_quote":null})).unwrap());
        std::fs::write(user.history_path.as_ref().unwrap(), serde_json::to_vec(&AgentHistory { sessions: vec![legacy], ..Default::default() }).unwrap()).unwrap();
    }
    let Fixture {root:original,access,control,x,y,a,b}=f;
    drop(access);drop(control);
    let backups=tempfile::tempdir().unwrap();
    assert!(crate::reader_maintenance::export_user(original.path(),"A",&backups.path().join("export")).unwrap_err().to_string().contains("accepted runs"));
    let snapshot=backups.path().join("snapshot");
    crate::reader_maintenance::backup_service(original.path(),&snapshot).unwrap();
    let root=tempfile::tempdir().unwrap();
    crate::reader_maintenance::restore_service(&snapshot,root.path()).unwrap();
    let users=UserRegistry::open(root.path()).unwrap();
    let control=ControlStore::open(users.writer()).unwrap();
    let f=Fixture {root,access:Arc::new(Authorization::new(users).unwrap()),control,x,y,a,b};
    configure(&f,&probe);
    f.access.runs.recover(&f.access).unwrap();
    let c=context(&f,&f.a);
    assert!(c.turn("legacy-imported-turn").is_err());
    assert_eq!(row(&f,"key")["stage"],"queued");
    assert_eq!(probe.calls.load(Ordering::SeqCst),0);
    assert!(f.access.runs.run_one(&f.access).unwrap());
    assert_eq!(row(&f,"key")["attempt"],1);
    assert!(!f.access.runs.run_one(&f.access).unwrap());
    assert_eq!(admit(&f,&w,&input).1["turn_id"],accepted.1["turn_id"]);
}

#[test]
fn mu6a_each_preclaim_commit_recovers_without_duplicate_question_or_model() {
    for point in ["preparing", "pending", "prepared", "queued"] {
        let (f, w, input, probe) = setup();
        *f.access.runs.fail_after.lock().unwrap() = Some(point);
        assert_eq!(admit(&f, &w, &input).0, 503, "{point}");
        assert_eq!(probe.calls.load(Ordering::SeqCst), 0);
        if point == "preparing" {
            assert_eq!(admit(&f, &w, &input).1["error_code"], "ADMISSION_PREPARING");
        }
        assert_eq!(
            f.call(
                &f.a,
                "DELETE",
                &format!("/api/me/chats/{}", input["session_id"].as_str().unwrap()),
                json!({})
            )
            .1["error_code"],
            "CHAT_BUSY"
        );
        let turn = row(&f, "key")["turn"].clone();
        let f = reopen(f, &probe);
        if point == "preparing" {
            assert_eq!(row(&f, "key")["stage"], "admission_failed");
            assert_eq!(admit(&f, &w, &input).1["error_code"], "REQUEST_KEY_CLOSED");
            assert!(!f.access.runs.run_one(&f.access).unwrap());
            let current = f.action(&f.a, &w, "new-page", "attach", json!({}));
            let mut next = Fixture::stamp(&current, "new-page");
            next["session_id"] = input["session_id"].clone();
            next["client_request_id"] = json!("after-closed-key");
            next["message"] = json!("你好");
            let (status, accepted) = admit(&f, &current, &next);
            assert_eq!(status, 202, "{accepted}");
            assert_ne!(accepted["turn_id"], turn);
            assert!(f.access.runs.run_one(&f.access).unwrap());
            assert_eq!(admit(&f, &w, &input).1["error_code"], "REQUEST_KEY_CLOSED");
        } else {
            assert_eq!(row(&f, "key")["stage"], "queued");
            assert_eq!(admit(&f, &w, &input).1["turn_id"], turn);
            assert!(f.access.runs.run_one(&f.access).unwrap());
            assert_eq!(row(&f, "key")["attempt"], 1);
            let user = f.access.users.lock().unwrap().get("A", "now").unwrap();
            assert_eq!(
                user.lock().unwrap().agent_history.sessions[0].turns.len(),
                1
            );
        }
    }
}

#[test]
fn mu6a_claimed_never_replays_and_saved_terminal_only_repairs_index() {
    for point in ["claimed", "terminal"] {
        let (f, w, input, probe) = setup();
        assert_eq!(admit(&f, &w, &input).0, 202);
        *f.access.runs.fail_after.lock().unwrap() = Some(point);
        assert_eq!(
            f.access.runs.run_one(&f.access).unwrap_err().error_code,
            "INJECTED_CRASH"
        );
        assert_eq!(row(&f, "key")["stage"], "claimed");
        let count = probe.calls.load(Ordering::SeqCst);
        let f = reopen(f, &probe);
        assert_eq!(row(&f, "key")["stage"], "settled");
        assert!(!f.access.runs.run_one(&f.access).unwrap());
        assert_eq!(probe.calls.load(Ordering::SeqCst), count);
        let t = context(&f, &f.a)
            .turn(row(&f, "key")["turn"].as_str().unwrap())
            .unwrap();
        if point == "claimed" {
            assert_eq!(t["turn"]["error"]["error_code"], "INTERRUPTED");
            assert_eq!(count, 0);
        } else {
            assert_eq!(t["turn"]["status"], "completed");
        }
    }
}

#[test]
fn session_event_queued_claimed_keep_original_frozen_input() {
    for claimed in [false, true] {
        let (f, w, input, probe) = setup();
        assert_eq!(admit(&f, &w, &input).0, 202);
        if claimed {
            *f.access.runs.fail_after.lock().unwrap() = Some("claimed");
            f.access.runs.run_one(&f.access).unwrap_err();
        }
        let handle = f.access.users.lock().unwrap().get("A", "now").unwrap();
        let user = handle.lock().unwrap();
        let session = &user.agent_history.sessions[0];
        assert!(session.turns[0].admission_input.is_none());
        let log = &user.session_store.as_ref().unwrap().logs[&session.id];
        let input = &log.projection.inputs[&session.turns[0].turn_id];
        let messages = log.frozen_messages(&input.messages).unwrap();
        let loaded = crate::session_log::SessionLog::open(log.path.clone()).unwrap();
        assert_eq!(loaded.frozen_messages(&input.messages).unwrap(), messages);
        assert_eq!(json!(loaded.projection.inputs[&session.turns[0].turn_id]), json!(input));
        assert_eq!(probe.calls.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn session_event_real_note_result_survives_reopen_without_reexecution() {
    let (f, w, mut input, mut probe) = setup();
    probe.note = true; probe.finish_note = true;
    configure(&f, &probe);
    input["message"] = json!("请在 1.1 保存笔记");
    assert_eq!(admit(&f, &w, &input).0, 202);
    assert!(f.access.runs.run_one(&f.access).unwrap());
    let handle = f.access.users.lock().unwrap().get("A", "now").unwrap();
    let user = handle.lock().unwrap();
    let session = &user.agent_history.sessions[0];
    assert!(session.messages.iter().any(|m| m.role == runtime::Role::Tool && m.tool_call_id.as_deref() == Some("note")));
    assert!(!session.turns[0].outcome.as_ref().unwrap().effects.is_empty());
    let calls = probe.calls.load(Ordering::SeqCst);
    let log = &user.session_store.as_ref().unwrap().logs[&session.id];
    let loaded = crate::session_log::SessionLog::open(log.path.clone()).unwrap();
    assert_eq!(json!(loaded.projection.session.unwrap()), json!(session));
    assert_eq!(probe.calls.load(Ordering::SeqCst), calls);
}

#[test]
fn session_management_network_create_select_delete_and_reload_use_only_jsonl() {
    let (f, w, input, probe) = setup();
    let original = input["session_id"].as_str().unwrap().to_owned();
    let history_path = jsonl_path(&f);
    let old_bytes = std::fs::read(&history_path).ok();
    let w = f.action(&f.a, &w, "page", "chat/new", json!({}));
    let newer = w["selected_chat"].as_str().unwrap().to_owned();
    assert_ne!(newer, original);
    let w = f.action(&f.a, &w, "page", "chat/select", json!({"session_id":original}));
    assert_eq!(w["selected_chat"], original);
    assert_eq!(f.call(&f.b, "POST", "/api/agent/history/delete", json!({"session_id":original})).0, 404);
    assert_eq!(f.call(&f.a, "POST", "/api/agent/history/delete", json!({"session_id":original})).0, 200);
    assert!(f.get(&f.a, &w)["selected_chat"].is_null());
    assert_eq!(std::fs::read(&history_path).ok(), old_bytes);
    let f = reopen(f, &probe);
    let handle = f.access.users.lock().unwrap().get("A", "now").unwrap();
    let user = handle.lock().unwrap();
    assert!(user.session_store.is_some());
    assert_eq!(user.agent_history.sessions.len(), 1);
    assert_eq!(user.agent_history.sessions[0].id, newer);
    assert_eq!(probe.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn mu6b_queued_cancel_and_busy_delete_are_owner_scoped_and_keys_stay_closed() {
    let (f, w, input, probe) = setup();
    let accepted = admit(&f, &w, &input).1;
    let turn = accepted["turn_id"].as_str().unwrap();
    assert_eq!(
        f.call(
            &f.a,
            "POST",
            "/api/agent/history/delete",
            json!({"session_id":input["session_id"]})
        )
        .1["error_code"],
        "CHAT_BUSY"
    );
    assert_eq!(
        f.call(
            &f.b,
            "POST",
            &format!("/api/agent/runs/{turn}/cancel"),
            json!({})
        )
        .0,
        404
    );
    assert_eq!(
        f.call(
            &f.b,
            "DELETE",
            &format!("/api/me/chats/{}", input["session_id"].as_str().unwrap()),
            json!({})
        )
        .0,
        404
    );
    let fork = f.action(&f.a, &w, "fork", "fork", json!({}));
    assert_eq!(
        f.ok(
            &f.a,
            "POST",
            &format!("/api/agent/runs/{turn}/cancel"),
            json!({})
        )["turn"]["status"],
        "cancelled"
    );
    assert!(!f.access.runs.run_one(&f.access).unwrap());
    assert_eq!(probe.calls.load(Ordering::SeqCst), 0);
    f.ok(
        &f.a,
        "DELETE",
        &format!("/api/me/chats/{}", input["session_id"].as_str().unwrap()),
        json!({}),
    );
    for scene in [&w, &fork] {
        let current = f.get(&f.a, scene);
        assert!(current["selected_chat"].is_null());
        assert!(current["generation"].as_u64() > scene["generation"].as_u64());
    }
    assert_eq!(admit(&f, &w, &input).1["error_code"], "REQUEST_KEY_CLOSED");
}

#[test]
fn mu6b_active_cancel_preserves_note_and_blocks_delete_until_execution_exits() {
    let (f, w, mut input, _) = setup();
    let probe = Probe {
        block: Some(Arc::new((Mutex::new(false), Condvar::new()))),
        note: true,
        ..Default::default()
    };
    configure(&f, &probe);
    input["message"] = json!("请在 1.1 保存笔记");
    let accepted = admit(&f, &w, &input).1;
    let turn = accepted["turn_id"].as_str().unwrap();
    let access = f.access.clone();
    let worker = std::thread::spawn(move || access.runs.run_one(&access));
    wait_calls(&probe, 2);
    assert_eq!(
        f.call(
            &f.a,
            "DELETE",
            &format!("/api/me/chats/{}", input["session_id"].as_str().unwrap()),
            json!({})
        )
        .1["error_code"],
        "CHAT_BUSY"
    );
    f.ok(
        &f.a,
        "POST",
        &format!("/api/agent/runs/{turn}/cancel"),
        json!({}),
    );
    assert_eq!(row(&f, "key")["stage"], "claimed");
    release(&probe);
    worker.join().unwrap().unwrap();
    assert_eq!(
        context(&f, &f.a).turn(turn).unwrap()["turn"]["status"],
        "cancelled"
    );
    assert_eq!(probe.calls.load(Ordering::SeqCst), 2);
    let user = f.access.users.lock().unwrap().get("A", "now").unwrap();
    assert!(!user
        .lock()
        .unwrap()
        .store
        .recall(&memory::RecallQuery {
            text: Some("mu6 durable note".into()),
            ..Default::default()
        })
        .is_empty());
}

#[test]
fn mu6b_unsaved_pins_chat_and_retries_exact_result_without_model() {
    let (f, w, input, _) = setup();
    let probe = Probe {
        block: Some(Arc::new((Mutex::new(false), Condvar::new()))),
        ..Default::default()
    };
    configure(&f, &probe);
    let accepted = admit(&f, &w, &input).1;
    let turn = accepted["turn_id"].as_str().unwrap();
    let observation = f.access.runs.stream(&context(&f, &f.a), turn).unwrap();
    let access = f.access.clone();
    let worker = std::thread::spawn(move || access.runs.run_one(&access));
    wait_calls(&probe, 1);
    {
        let handle = f.access.users.lock().unwrap().get("A", "now").unwrap();
        let mut user = handle.lock().unwrap();
        let log = user.session_store.as_mut().unwrap().logs.get_mut(input["session_id"].as_str().unwrap()).unwrap();
        log.fail_write = Some(false);
        log.fail_terminal_only = true;
    }
    release(&probe);
    worker.join().unwrap().unwrap();
    assert!(row(&f, "key")["unsaved"].as_bool().unwrap());
    assert_eq!(observation.snapshot().persistence_state, "failed");
    assert_eq!(
        observation.snapshot().error.unwrap()["error_code"],
        "TURN_UNSAVED"
    );
    assert_eq!(
        context(&f, &f.a).turn(turn).unwrap()["turn"]["status"],
        "pending_assistant"
    );
    assert_eq!(
        f.call(
            &f.a,
            "DELETE",
            &format!("/api/me/chats/{}", input["session_id"].as_str().unwrap()),
            json!({})
        )
        .1["error_code"],
        "CHAT_BUSY"
    );
    let count = probe.calls.load(Ordering::SeqCst);
    let saved = f.ok(
        &f.a,
        "POST",
        &format!("/api/agent/runs/{turn}/retry-save"),
        json!({}),
    );
    assert_eq!(saved["turn"]["status"], "completed");
    assert_eq!(observation.snapshot().persistence_state, "saved");
    assert_eq!(saved["dispatch_state"], "settled");
    assert_eq!(probe.calls.load(Ordering::SeqCst), count);
}
#[test]
fn mu6a_real_http_concurrent_keys_old_chat_and_other_reader_share_the_core() {
    let (f, w, input, probe) = setup();
    let server = multi_user_host::start_with_access(
        "127.0.0.1:0".parse().unwrap(),
        Arc::new(Site::new("https://reader.example").unwrap()),
        f.access.clone(),
    )
    .unwrap();
    let post = |path: &str, input: Value| {
        ureq::post(&format!("{}{path}", server.url))
            .set("Host", "reader.example")
            .set("Origin", "https://reader.example")
            .set("Cookie", &format!("{}={}", crate::auth::COOKIE, f.a))
            .set("X-CSRF-Token", &crate::auth::csrf(&f.a))
            .send_json(&input)
            .unwrap()
            .into_json::<Value>()
            .unwrap()
    };
    let url = format!(
        "{}/api/workspaces/{}/agent/runs",
        server.url,
        w["workspace_id"].as_str().unwrap()
    );
    let mut jobs = vec![];
    for _ in 0..2 {
        let (url, input, token) = (url.clone(), input.clone(), f.a.clone());
        jobs.push(std::thread::spawn(move || {
            ureq::post(&url)
                .set("Host", "reader.example")
                .set("Origin", "https://reader.example")
                .set("Cookie", &format!("{}={token}", crate::auth::COOKIE))
                .set("X-CSRF-Token", &crate::auth::csrf(&token))
                .send_json(&input)
                .unwrap_or_else(|e| match e {
                    ureq::Error::Status(409, response) => response,
                    other => panic!("{other}"),
                })
                .into_json::<Value>()
                .unwrap()
        }));
    }
    let mut replies: Vec<_> = jobs.into_iter().map(|j| j.join().unwrap()).collect();
    for reply in &mut replies {
        if reply["error_code"] == "ADMISSION_PREPARING" {
            *reply = post(
                &format!(
                    "/api/workspaces/{}/agent/runs",
                    w["workspace_id"].as_str().unwrap()
                ),
                input.clone(),
            );
        }
    }
    assert_eq!(replies[0]["turn_id"], replies[1]["turn_id"]);
    let mut legacy = input.clone();
    legacy["workspace_id"] = w["workspace_id"].clone();
    let outcome = post("/api/agent/chat", legacy);
    assert_eq!(outcome["answer"], "你好！");
    assert_eq!(probe.calls.load(Ordering::SeqCst), 1);
    assert_eq!(row(&f, "key")["attempt"], 1);
    assert_eq!(
        f.call(
            &f.b,
            "GET",
            &format!(
                "/api/agent/runs/{}",
                replies[0]["turn_id"].as_str().unwrap()
            ),
            json!({})
        )
        .0,
        404
    );
    server.shutdown();
}

#[test]
fn mu6a_storage_failure_never_returns_acceptance_and_keeps_failed_input() {
    for stage in ["insert", "queued"] {
        let (f, w, input, probe) = setup();
        let sql = if stage == "insert" {
            "CREATE TRIGGER fail_admission BEFORE INSERT ON run_admissions BEGIN SELECT RAISE(ABORT,'fixture'); END;"
        } else {
            "CREATE TRIGGER fail_admission BEFORE UPDATE OF dispatch_state ON run_admissions WHEN NEW.dispatch_state='queued' BEGIN SELECT RAISE(ABORT,'fixture'); END;"
        };
        f.control.connection.execute_batch(sql).unwrap();
        assert_eq!(admit(&f, &w, &input).0, 503);
        assert_eq!(probe.calls.load(Ordering::SeqCst), 0);
        f.control
            .connection
            .execute_batch("DROP TRIGGER fail_admission;")
            .unwrap();
        if stage == "insert" {
            assert_eq!(admit(&f, &w, &input).0, 202);
        } else {
            assert_eq!(row(&f, "key")["stage"], "admission_failed");
            assert_eq!(admit(&f, &w, &input).1["error_code"], "ADMISSION_FAILED");
            let user = f.access.users.lock().unwrap().get("A", "now").unwrap();
            assert_eq!(
                user.lock().unwrap().agent_history.sessions[0].turns[0].status,
                AgentAssistantStatus::Failed
            );
        }
    }
}

#[test]
fn mu6a_teaching_partial_receipt_reuses_frozen_attempt_and_original_binding() {
    teaching_partial_receipt(false);
}

#[test]
fn jl3_teaching_receipt_reuses_frozen_attempt() {
    teaching_partial_receipt(true);
}

fn teaching_partial_receipt(jsonl: bool) {
    use memory::teaching::{TeachingBinding, TeachingEvent, TeachingFact};
    let (f, w, mut input, probe) = setup();
    f.ok(&f.a,"POST","/api/tutor/mutate",json!({"operation_id":"on","expected_revision":0,"action":{"kind":"set_enabled","enabled":true}}));
    let binding;
    {
        let handle = f.access.users.lock().unwrap().get("A", "now").unwrap();
        let user = handle.lock().unwrap();
        let publication = f.access.library.lock().unwrap().load("A", &f.x).unwrap();
        let private = PrivateBookContext {
            user: &user,
            book: &publication.book,
            book_dir: publication.directory(),
            messages: &[],
            selected_chat: w["selected_chat"].as_str(),
        };
        teaching::start_request(&private, "original").unwrap();
        let learning = user.learning_store().unwrap();
        let state = learning.state().unwrap();
        let session = state
            .sessions
            .get(state.control.current_tutor_session_id.as_ref().unwrap())
            .unwrap();
        binding = TeachingBinding {
            tutor_session_id: session.id.clone(),
            session_revision: session.revision,
            control_revision: state.control.revision,
            source_id: f.x.book_id.clone(),
            source_revision: publication.book.source_fingerprint().into(),
            map_revision: Some("v1".into()),
            chat_session_id: "original-chat".into(),
            turn_id: "original-turn".into(),
        };
        user.learning_store().unwrap().append_teaching(&TeachingEvent{event_id:"delivery".into(),binding:binding.clone(),kind:TeachingFact::MessageDelivered,causal_refs:vec![],payload:json!({"move":{"move_id":"question","prompt":"Explain speed","object_ids":["speed"]}}),occurred_at:"original".into()}).unwrap();
    }
    if jsonl { jsonl_path(&f); }
    input["teaching_ref"] = json!("delivery");
    input["message"] = json!("I think speed is distance over time");
    *f.access.runs.fail_after.lock().unwrap() = Some("teaching_receipt");
    assert_eq!(admit(&f, &w, &input).0, 503);
    let turn = row(&f, "key")["turn"].as_str().unwrap().to_owned();
    let original;
    {
        let handle = f.access.users.lock().unwrap().get("A", "now").unwrap();
        let user = handle.lock().unwrap();
        let mut learning = user.learning_store().unwrap();
        original = learning
            .teaching_event(&format!("response:{turn}"))
            .unwrap();
        assert_eq!(original.payload["attempt"], 1);
        learning.append_teaching(&TeachingEvent{event_id:"later-action".into(),binding:binding.clone(),kind:TeachingFact::LearnerAction,causal_refs:vec!["delivery".into()],payload:json!({"delivery_ref":"delivery","action":"submit","response":"later","attempt":2}),occurred_at:"later".into()}).unwrap();
        let state = learning.state().unwrap();
        learning
            .mutate(
                &memory::learning::TutorMutation {
                    operation_id: "off".into(),
                    expected_revision: state.control.revision,
                    action: memory::learning::TutorAction::SetEnabled { enabled: false },
                },
                "later",
            )
            .unwrap();
    }
    let f = reopen(f, &probe);
    assert_eq!(row(&f, "key")["stage"], "queued");
    let handle = f.access.users.lock().unwrap().get("A", "now").unwrap();
    let user = handle.lock().unwrap();
    let learning = user.learning_store().unwrap();
    assert_eq!(
        learning
            .teaching_event(&format!("response:{turn}"))
            .unwrap(),
        original
    );
    let bound = learning.teaching_event(&format!("binding:{turn}")).unwrap();
    assert_eq!(bound.binding.control_revision, binding.control_revision);
    assert_eq!(probe.calls.load(Ordering::SeqCst), 0);
    let log = &user.session_store.as_ref().unwrap().logs[input["session_id"].as_str().unwrap()];
    let bytes = std::fs::read_to_string(&log.path).unwrap();
    assert_eq!(bytes.lines().map(|l|serde_json::from_str::<Value>(l).unwrap()).filter(|v|v["kind"]=="turn.prepared").count(),1);
    drop(learning);
    drop(user);
    drop(handle);
    let learning_path = f.access.users.lock().unwrap().paths("A").unwrap().learning;
    let db = rusqlite::Connection::open(&learning_path).unwrap();
    db.execute_batch("CREATE TRIGGER fail_delivery BEFORE INSERT ON teaching_trace WHEN json_extract(NEW.event,'$.kind')='message_delivered' BEGIN SELECT RAISE(ABORT,'fixture'); END;").unwrap();
    f.access.runs.run_one(&f.access).unwrap();
    assert_eq!(row(&f, "key")["unsaved"], true);
    assert_eq!(
        context(&f, &f.a).turn(&turn).unwrap()["turn"]["status"],
        "completed"
    );
    db.execute_batch("DROP TRIGGER fail_delivery;").unwrap();
    drop(db);
    let count = probe.calls.load(Ordering::SeqCst);
    let f = reopen(f, &probe);
    let handle = f.access.users.lock().unwrap().get("A", "now").unwrap();
    let user = handle.lock().unwrap();
    assert_eq!(
        user.learning_store()
            .unwrap()
            .teaching_event(&format!("followup:{turn}"))
            .unwrap()
            .kind,
        TeachingFact::MessageDelivered
    );
    assert_eq!(row(&f, "key")["stage"], "settled");
    assert_eq!(probe.calls.load(Ordering::SeqCst), count);
}

#[test]
fn mu6a_confirmation_dependency_is_not_persisted_and_restart_requires_reconfirmation() {
    let (f, w, mut input, probe) = setup();
    let mut local = state_named("mu6-sensitive-seed");
    let reply = post_profile(
        &mut local,
        0,
        json!({"kind":"remember","operation_id":"private-op","evidence_text":"Remember my medical preference","fact":profile_fact_draft("book","health","SENSITIVE_MU6_CANDIDATE","normal")}),
    );
    assert_eq!(reply.status, 200);
    assert!(reply.body.contains("needs_sensitive_confirmation"));
    let pending = local
        .user
        .agent_history
        .pending_governance_mutations
        .values()
        .next()
        .unwrap()
        .clone();
    {
        let handle = f.access.users.lock().unwrap().get("A", "now").unwrap();
        let mut user = handle.lock().unwrap();
        let chat = w["selected_chat"].as_str().unwrap();
        user.agent_history
            .pending_governance_mutations
            .insert(chat.into(), pending);
        user.agent_history.arm_confirmation(chat);
    }
    input["message"] = json!("确认保存");
    assert_eq!(admit(&f, &w, &input).0, 202);
    let path = crate::session_store::SessionPaths::from_history(&jsonl_path(&f)).session(input["session_id"].as_str().unwrap());
    assert!(!std::fs::read_to_string(path)
        .unwrap()
        .contains("SENSITIVE_MU6_CANDIDATE"));
    let f = reopen(f, &probe);
    assert_eq!(row(&f, "key")["stage"], "settled");
    let value = context(&f, &f.a)
        .turn(row(&f, "key")["turn"].as_str().unwrap())
        .unwrap();
    assert_eq!(
        value["turn"]["error"]["error_code"],
        "SENSITIVE_CONFIRMATION_EXPIRED"
    );
    assert_eq!(probe.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn mu6a_queued_uses_original_book_and_snapshot_after_workspace_switch() {
    queued_original_snapshot(false);
}

#[test]
fn jl3_frozen_position_survives_later_append_and_workspace_switch() {
    queued_original_snapshot(true);
}

fn queued_original_snapshot(jsonl: bool) {
    let (f, w, mut input, _) = setup();
    let probe = Probe {
        note: true,
        ..Default::default()
    };
    configure(&f, &probe);
    input["message"] = json!("请在 1.1 保存笔记");
    if jsonl { jsonl_path(&f); }
    assert_eq!(admit(&f, &w, &input).0, 202);
    if jsonl {
        let handle = f.access.users.lock().unwrap().get("A", "now").unwrap();
        let mut user = handle.lock().unwrap();
        let session = &user.agent_history.sessions[0];
        let reference = AgentTurnRef { session_id: session.id.clone(), turn_id: session.turns[0].turn_id.clone(), user_turn_ordinal: 1 };
        crate::session_runtime::append(&mut user, &reference, "later", crate::session_event::EventBody::MessageAppended {
            messages: vec![Message::user("LATER_MESSAGE_MUST_NOT_ENTER_FROZEN_INPUT")],
        }).unwrap();
    }
    let changed = f.action(
        &f.a,
        &w,
        "page",
        "book/open",
        json!({"published_book_ref":f.y}),
    );
    assert_eq!(admit(&f, &w, &input).0, 202);
    f.access.runs.run_one(&f.access).unwrap();
    assert_eq!(f.get(&f.a, &changed)["reader"], changed["reader"]);
    let handle = f.access.users.lock().unwrap().get("A", "now").unwrap();
    let user = handle.lock().unwrap();
    let notes = user.store.recall(&memory::RecallQuery {
        text: Some("mu6 durable note".into()),
        ..Default::default()
    });
    assert!(!notes.is_empty());
    let saved = &user.agent_history.sessions[0].turns[0];
    assert_eq!(saved.published_book_ref.as_ref(), Some(&f.x));
    assert_eq!(saved.status, AgentAssistantStatus::Failed);
    if jsonl {
        assert!(!probe.messages.lock().unwrap().iter().flatten().any(|m| m.content.as_deref().unwrap_or("").contains("LATER_MESSAGE_MUST_NOT_ENTER_FROZEN_INPUT")));
    }
}

#[test]
fn mu6b_delete_commit_crash_repairs_all_workspace_selections_and_closes_old_key() {
    let (f, w, input, probe) = setup();
    let accepted = admit(&f, &w, &input).1;
    f.access
        .runs
        .cancel(&context(&f, &f.a), accepted["turn_id"].as_str().unwrap())
        .unwrap();
    let fork = f.action(&f.a, &w, "fork", "fork", json!({}));
    *f.access.runs.fail_after.lock().unwrap() = Some("chat_deleted");
    assert_eq!(
        f.call(
            &f.a,
            "DELETE",
            &format!("/api/me/chats/{}", input["session_id"].as_str().unwrap()),
            json!({})
        )
        .0,
        503
    );
    let f = reopen(f, &probe);
    for scene in [&w, &fork] {
        let current = f.get(&f.a, scene);
        assert!(current["selected_chat"].is_null());
        assert!(current["generation"].as_u64() > scene["generation"].as_u64());
    }
    assert_eq!(admit(&f, &w, &input).1["error_code"], "REQUEST_KEY_CLOSED");
    let mut new = input.clone();
    new["client_request_id"] = json!("new-key");
    assert_ne!(admit(&f, &w, &new).0, 202);
}

#[test]
fn mu6b_cancel_racing_claim_has_one_winner_and_never_replays() {
    let (f, mut w, _, probe) = setup();
    for n in 0..4 {
        let mut input = Fixture::stamp(&w, "page");
        input["session_id"] = w["selected_chat"].clone();
        input["client_request_id"] = json!(format!("race-{n}"));
        input["message"] = json!("你好");
        let accepted = admit(&f, &w, &input).1;
        let turn = accepted["turn_id"].as_str().unwrap().to_owned();
        let gate = Arc::new(std::sync::Barrier::new(2));
        let a = f.access.clone();
        let g = gate.clone();
        let worker = std::thread::spawn(move || {
            g.wait();
            a.runs.run_one(&a)
        });
        let c = context(&f, &f.a);
        gate.wait();
        f.access.runs.cancel(&c, &turn).unwrap();
        worker.join().unwrap().unwrap();
        let r = row(&f, &format!("race-{n}"));
        assert_eq!(r["stage"], "settled");
        assert!(r["attempt"].as_u64().unwrap() <= 1);
        assert!(matches!(
            c.turn(&turn).unwrap()["turn"]["status"].as_str(),
            Some("completed" | "cancelled")
        ));
        w = f.action(&f.a, &w, "page", "chat/new", json!({}));
    }
    assert!(probe.calls.load(Ordering::SeqCst) <= 4);
}
#[test]
fn mu6a_disabled_or_revoked_queued_requests_do_not_start_provider() {
    for disabled in [true, false] {
        let (mut f, w, input, probe) = setup();
        assert_eq!(admit(&f, &w, &input).0, 202);
        if disabled {
            f.control.set_user_disabled("A", true).unwrap();
        } else {
            f.access.library.lock().unwrap().revoke("A", &f.x).unwrap();
        }
        f.access.runs.run_one(&f.access).unwrap();
        assert_eq!(row(&f, "key")["stage"], "settled");
        assert_eq!(probe.calls.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn adm2_disable_cancels_queued_work_across_reenable_and_restart() {
    let (mut f, w, input, probe) = setup();
    f.control.set_reader_admin("B", true).unwrap();
    assert_eq!(admit(&f, &w, &input).0, 202);
    let receipt = f.ok(&f.b,"POST","/api/admin/users/A/status",json!({"operation_id":"disable-A","disabled":true}));
    assert_eq!(receipt["cancel_requested_runs"],1);
    assert_eq!(row(&f,"key")["cancel"],true);
    f.ok(&f.b,"POST","/api/admin/users/A/status",json!({"operation_id":"enable-A","disabled":false}));
    let f = reopen(f,&probe);
    assert_eq!(row(&f,"key")["stage"],"settled");
    assert!(!f.access.runs.run_one(&f.access).unwrap());
    assert_eq!(probe.calls.load(Ordering::SeqCst),0);
    let handle=f.access.users.lock().unwrap().get_for_recovery("A","now").unwrap();
    let user=handle.lock().unwrap();
    assert_eq!(user.agent_history.sessions[0].turns[0].status,AgentAssistantStatus::Cancelled);
}

#[test]
fn adm2_disable_active_run_preserves_effects_and_uses_existing_cancel_save() {
    let (mut f,w,mut input,_) = setup();
    f.control.set_reader_admin("B",true).unwrap();
    let probe=Probe { block:Some(Arc::new((Mutex::new(false),Condvar::new()))),note:true,..Default::default() };
    configure(&f,&probe);
    input["message"]=json!("请在 1.1 保存笔记");
    assert_eq!(admit(&f,&w,&input).0,202);
    let access=f.access.clone();
    let worker=std::thread::spawn(move || access.runs.run_one(&access));
    wait_calls(&probe,2);
    let reply=f.call(&f.b,"POST","/api/admin/users/A/status",json!({"operation_id":"disable-active","disabled":true}));
    release(&probe);
    worker.join().unwrap().unwrap();
    assert_eq!(reply.0,200,"{}",reply.1);
    assert_eq!(row(&f,"key")["stage"],"settled");
    assert_eq!(row(&f,"key")["unsaved"],false);
    assert_eq!(f.call(&f.a,"GET","/api/auth/me",json!({})).0,401);
    assert_eq!(probe.calls.load(Ordering::SeqCst),2);
    let handle=f.access.users.lock().unwrap().get_for_recovery("A","now").unwrap();
    let user=handle.lock().unwrap();
    assert_eq!(user.agent_history.sessions[0].turns[0].status,AgentAssistantStatus::Cancelled);
    assert!(!user.store.recall(&memory::RecallQuery { text:Some("mu6 durable note".into()),..Default::default() }).is_empty());
}

#[test]
fn mu6b_unsaved_restart_reports_interruption_and_old_pending_stays_archived() {
    let (f, w, input, _) = setup();
    let probe = Probe {
        block: Some(Arc::new((Mutex::new(false), Condvar::new()))),
        ..Default::default()
    };
    configure(&f, &probe);
    assert_eq!(admit(&f, &w, &input).0, 202);
    let access = f.access.clone();
    let worker = std::thread::spawn(move || access.runs.run_one(&access));
    wait_calls(&probe, 1);
    {
        let handle = f.access.users.lock().unwrap().get("A", "now").unwrap();
        let mut user = handle.lock().unwrap();
        let log = user.session_store.as_mut().unwrap().logs.get_mut(input["session_id"].as_str().unwrap()).unwrap();
        log.fail_write = Some(false);
        log.fail_terminal_only = true;
    }
    release(&probe);
    worker.join().unwrap().unwrap();
    assert_eq!(row(&f, "key")["unsaved"], true);
    {
        let handle = f.access.users.lock().unwrap().get("A", "now").unwrap();
        let user = handle.lock().unwrap();
        let mut legacy = new_agent_session(&f.x.book_id, "legacy", 2);
        legacy.id = "legacy".into();
        legacy.turns.push(serde_json::from_value(json!({"turn_id":"legacy-turn","user_turn_ordinal":1,"user":"old","status":"pending_assistant","question_anchor_lid":null,"question_quote":null})).unwrap());
        std::fs::write(user.history_path.as_ref().unwrap(), serde_json::to_vec(&AgentHistory { sessions: vec![legacy], ..Default::default() }).unwrap()).unwrap();
    }
    let count = probe.calls.load(Ordering::SeqCst);
    let f = reopen(f, &probe);
    let c = context(&f, &f.a);
    assert!(c.turn("legacy-turn").is_err());
    for turn in [row(&f, "key")["turn"].as_str().unwrap()] {
        assert_eq!(
            c.turn(turn).unwrap()["turn"]["error"]["error_code"],
            "INTERRUPTED"
        );
    }
    assert!(!f.access.runs.run_one(&f.access).unwrap());
    assert_eq!(probe.calls.load(Ordering::SeqCst), count);
}

#[test]
fn mu6a_real_provider_transport_and_request_key_lookup_survive_lost_accept_response() {
    let (mut f, w, mut input, _) = setup();
    let fake = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let address = fake.server_addr().to_ip().unwrap();
    super::adm5_tests::seed(&mut f, &format!("http://{address}"), 1_000_000);
    let provider = std::thread::spawn(move || {
        let mut request = fake.recv_timeout(Duration::from_secs(10)).unwrap().unwrap();
        let mut text = String::new();
        request.as_reader().read_to_string(&mut text).unwrap();
        assert!(text.contains("你好"));
        request.respond(tiny_http::Response::from_string(json!({"choices":[{"message":{"role":"assistant","content":"你好！"},"finish_reason":"stop"}],"usage":{"total_tokens":1}}).to_string()).with_header(tiny_http::Header::from_bytes("Content-Type","application/json").unwrap())).unwrap();
    });
    f.access
        .runs
        .configure(
            ProviderConfig::from_values(
                "native",
                "fixture-secret-not-in-history",
                format!("http://{address}"),
                "fixture-model",
            )
            .unwrap(),
        )
        .unwrap();
    let server = multi_user_host::start_with_access(
        "127.0.0.1:0".parse().unwrap(),
        Arc::new(Site::new("https://reader.example").unwrap()),
        f.access.clone(),
    )
    .unwrap();
    input["workspace_id"] = w["workspace_id"].clone();
    let response = ureq::post(&format!("{}/api/agent/chat", server.url))
        .set("Host", "reader.example")
        .set("Origin", "https://reader.example")
        .set("Cookie", &format!("{}={}", crate::auth::COOKIE, f.a))
        .set("X-CSRF-Token", &crate::auth::csrf(&f.a))
        .send_json(&input)
        .unwrap()
        .into_json::<Value>()
        .unwrap();
    assert_eq!(response["answer"], "你好！");
    let found = f.ok(
        &f.a,
        "GET",
        "/api/agent/runs?client_request_id=key",
        json!({}),
    );
    assert_eq!(found["dispatch_state"], "settled");
    assert_eq!(found["turn_id"], row(&f, "key")["turn"]);
    let path = crate::session_store::SessionPaths::from_history(&jsonl_path(&f)).session(input["session_id"].as_str().unwrap());
    assert!(!std::fs::read_to_string(path)
        .unwrap()
        .contains("fixture-secret-not-in-history"));
    provider.join().unwrap();
    server.shutdown();
}

#[test]
fn mu6a_provider_change_does_not_rebind_already_accepted_turn() {
    let (f, w, input, probe) = setup();
    assert_eq!(admit(&f, &w, &input).0, 202);
    f.access
        .runs
        .configure(
            ProviderConfig::from_values(
                "native",
                "unused-fixture-key",
                "http://127.0.0.1:1",
                "different-model",
            )
            .unwrap(),
        )
        .unwrap();
    f.access.runs.run_one(&f.access).unwrap();
    assert_eq!(probe.calls.load(Ordering::SeqCst), 0);
    let t = context(&f, &f.a)
        .turn(row(&f, "key")["turn"].as_str().unwrap())
        .unwrap();
    assert_eq!(
        t["turn"]["error"]["error_code"],
        "PROVIDER_BINDING_UNAVAILABLE"
    );
}

#[test]
fn mu6b_queued_cancel_save_failure_stays_busy_and_retry_never_calls_model() {
    let (f, w, input, probe) = setup();
    let turn = admit(&f, &w, &input).1["turn_id"]
        .as_str()
        .unwrap()
        .to_owned();
    {
        let handle = f.access.users.lock().unwrap().get("A", "now").unwrap();
        let mut user = handle.lock().unwrap();
        let log = user.session_store.as_mut().unwrap().logs.get_mut(input["session_id"].as_str().unwrap()).unwrap();
        log.fail_write = Some(false);
        log.fail_terminal_only = true;
    }
    assert_eq!(
        f.call(
            &f.a,
            "POST",
            &format!("/api/agent/runs/{turn}/cancel"),
            json!({})
        )
        .0,
        503
    );
    assert_eq!(row(&f, "key")["unsaved"], true);
    assert!(!f.access.runs.run_one(&f.access).unwrap());
    assert_eq!(
        f.call(
            &f.a,
            "DELETE",
            &format!("/api/me/chats/{}", input["session_id"].as_str().unwrap()),
            json!({})
        )
        .1["error_code"],
        "CHAT_BUSY"
    );
    let saved = f.ok(
        &f.a,
        "POST",
        &format!("/api/agent/runs/{turn}/retry-save"),
        json!({}),
    );
    assert_eq!(saved["turn"]["status"], "cancelled");
    assert_eq!(saved["dispatch_state"], "settled");
    assert_eq!(probe.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn mu6a_claimed_recovery_discards_run_local_context_and_restores_frozen_prefix() {
    let (f, w, input, probe) = setup();
    assert_eq!(admit(&f, &w, &input).0, 202);
    *f.access.runs.fail_after.lock().unwrap() = Some("claimed");
    f.access.runs.run_one(&f.access).unwrap_err();
    {
        let handle = f.access.users.lock().unwrap().get("A", "now").unwrap();
        let mut user = handle.lock().unwrap();
        user.agent_history.sessions[0].messages =
            vec![Message::user("run-local compacted context")];
        // Execution-local changes have no durable record and must disappear on reopen.
    }
    let f = reopen(f, &probe);
    let handle = f.access.users.lock().unwrap().get("A", "now").unwrap();
    let user = handle.lock().unwrap();
    let messages = serde_json::to_string(&user.agent_history.sessions[0].messages).unwrap();
    assert!(messages.contains("你好"));
    assert!(!messages.contains("run-local compacted context"));
    assert_eq!(probe.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn mu6a_active_slot_does_not_block_other_users_reading_and_shutdown_stops_claims() {
    let (f, w, input, _) = setup();
    let probe = Probe {
        block: Some(Arc::new((Mutex::new(false), Condvar::new()))),
        ..Default::default()
    };
    configure(&f, &probe);
    let b = f.create(&f.b, &f.x, "b-page");
    let b = f.action(&f.b, &b, "b-page", "chat/new", json!({}));
    let mut b_input = Fixture::stamp(&b, "b-page");
    b_input["session_id"] = b["selected_chat"].clone();
    b_input["client_request_id"] = json!("b-key");
    b_input["message"] = json!("你好");
    assert_eq!(admit(&f, &w, &input).0, 202);
    let access = f.access.clone();
    let worker = std::thread::spawn(move || access.runs.run_one(&access));
    wait_calls(&probe, 1);
    assert_eq!(
        f.call(
            &f.b,
            "POST",
            &format!(
                "/api/workspaces/{}/agent/runs",
                b["workspace_id"].as_str().unwrap()
            ),
            b_input
        )
        .0,
        202
    );
    assert_eq!(f.get(&f.b, &b)["selected_chat"], b["selected_chat"]);
    let access = f.access.clone();
    let second = std::thread::spawn(move || access.runs.run_one(&access));
    wait_calls(&probe, 2);
    f.access.runs.stop();
    release(&probe);
    worker.join().unwrap().unwrap();
    second.join().unwrap().unwrap();
    assert!(!f.access.runs.run_one(&f.access).unwrap());
    let f = reopen(f, &probe);
    assert!(!f.access.runs.run_one(&f.access).unwrap());
    assert_eq!(probe.calls.load(Ordering::SeqCst), 2);
}

#[test]
fn mu6c_user_share_caps_windows_and_rotation_serves_b_before_a_second() {
    let (f, w, input, mut probe) = setup();
    probe.block = Some(Arc::new((Mutex::new(false), Condvar::new())));
    configure(&f, &probe);
    let a2 = f.create(&f.a, &f.x, "a2");
    let a2 = f.action(&f.a, &a2, "a2", "chat/new", json!({}));
    let mut i2 = Fixture::stamp(&a2, "a2");
    i2["session_id"] = a2["selected_chat"].clone();
    i2["client_request_id"] = json!("a2");
    i2["message"] = json!("你好");
    assert_eq!(admit(&f, &w, &input).0, 202);
    assert_eq!(admit(&f, &a2, &i2).0, 202);
    let access = f.access.clone();
    let worker = std::thread::spawn(move || access.runs.run_one(&access));
    wait_calls(&probe, 1);
    assert!(
        !f.access.runs.run_one(&f.access).unwrap(),
        "same user must not take a second share"
    );
    let b = f.create(&f.b, &f.x, "b");
    let b = f.action(&f.b, &b, "b", "chat/new", json!({}));
    let mut ib = Fixture::stamp(&b, "b");
    ib["session_id"] = b["selected_chat"].clone();
    ib["client_request_id"] = json!("b");
    ib["message"] = json!("你好");
    assert_eq!(
        f.call(
            &f.b,
            "POST",
            &format!(
                "/api/workspaces/{}/agent/runs",
                b["workspace_id"].as_str().unwrap()
            ),
            ib
        )
        .0,
        202
    );
    release(&probe);
    worker.join().unwrap().unwrap();
    assert!(f.access.runs.run_one(&f.access).unwrap());
    assert_eq!(
        row(&f, "a2")["stage"],
        "queued",
        "rotation must visit B before A again"
    );
    assert!(f.access.runs.run_one(&f.access).unwrap());
    assert_eq!(row(&f, "a2")["stage"], "settled");
    let status = f
        .access
        .runs
        .status(&context(&f, &f.a), row(&f, "a2")["turn"].as_str().unwrap())
        .unwrap();
    assert_eq!(status["turn"]["usage"]["known_total_tokens"], 1);
}

#[test]
fn mu6c_four_sync_waiters_do_not_occupy_normal_http_workers() {
    let (f, w, input, mut probe) = setup();
    probe.block = Some(Arc::new((Mutex::new(false), Condvar::new())));
    configure(&f, &probe);
    assert_eq!(admit(&f, &w, &input).0, 202);
    let server = multi_user_host::start_with_access(
        "127.0.0.1:0".parse().unwrap(),
        Arc::new(Site::new("https://reader.example").unwrap()),
        f.access.clone(),
    )
    .unwrap();
    wait_calls(&probe, 1);
    let mut jobs = vec![];
    for _ in 0..4 {
        let (url, token, mut input) = (server.url.clone(), f.a.clone(), input.clone());
        input["workspace_id"] = w["workspace_id"].clone();
        jobs.push(std::thread::spawn(move || {
            ureq::post(&format!("{url}/api/agent/chat"))
                .set("Host", "reader.example")
                .set("Origin", "https://reader.example")
                .set("Cookie", &format!("{}={token}", crate::auth::COOKIE))
                .set("X-CSRF-Token", &crate::auth::csrf(&token))
                .send_json(&input)
                .unwrap()
                .into_json::<Value>()
                .unwrap()
        }));
    }
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        if f.access
            .resources
            .try_acquire(crate::service_limits::Resource::SyncWait, "A")
            .is_none()
        {
            break;
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
    let started = std::time::Instant::now();
    let response = ureq::get(&format!("{}/api/me/chats", server.url))
        .timeout(Duration::from_secs(2))
        .set("Host", "reader.example")
        .set("Cookie", &format!("{}={}", crate::auth::COOKIE, f.b))
        .call()
        .unwrap();
    assert_eq!(response.status(), 200);
    assert!(started.elapsed() < Duration::from_secs(2));
    let mut overflow = input;
    overflow["workspace_id"] = w["workspace_id"].clone();
    let accepted = ureq::post(&format!("{}/api/agent/chat", server.url))
        .set("Host", "reader.example")
        .set("Origin", "https://reader.example")
        .set("Cookie", &format!("{}={}", crate::auth::COOKIE, f.a))
        .set("X-CSRF-Token", &crate::auth::csrf(&f.a))
        .send_json(overflow)
        .unwrap();
    assert_eq!(accepted.status(), 202);
    assert_eq!(
        accepted.into_json::<Value>().unwrap()["turn_id"],
        row(&f, "key")["turn"]
    );
    release(&probe);
    for job in jobs {
        assert_eq!(job.join().unwrap()["answer"], "你好！");
    }
    assert_eq!(probe.calls.load(Ordering::SeqCst), 1);
    server.shutdown();
}

#[test]
fn mu6d_queue_observation_tracks_execution_and_restart_discards_old_cursor() {
    let (f, w, input, probe) = setup();
    let accepted = admit(&f, &w, &input).1;
    let turn = accepted["turn_id"].as_str().unwrap();
    let ctx = context(&f, &f.a);
    let stream = f.access.runs.stream(&ctx, turn).unwrap();
    let (events, terminal) = stream.read_after(None, Duration::ZERO);
    assert!(!terminal);
    assert_eq!(events[0].payload["execution_state"], "queued");
    let frame = stream.event_frame(&events[0]);
    let cursor = frame
        .lines()
        .next()
        .unwrap()
        .strip_prefix("id: ")
        .unwrap()
        .to_owned();
    assert_eq!(stream.cursor(Some(&cursor)).unwrap(), Some(0));
    assert!(frame.contains("\"live_buffer_reset\":true"));
    assert!(f.access.runs.run_one(&f.access).unwrap());
    let (events, terminal) = stream.read_after(Some(0), Duration::ZERO);
    assert!(terminal);
    assert!(events.iter().any(|e| e.event_type == "run.started"));
    assert_eq!(events.last().unwrap().payload["persistence_state"], "saved");
    assert_eq!(probe.calls.load(Ordering::SeqCst), 1);
    drop(ctx);
    drop(stream);
    let f = reopen(f, &probe);
    let server = multi_user_host::start_with_access(
        "127.0.0.1:0".parse().unwrap(),
        Arc::new(Site::new("https://reader.example").unwrap()),
        f.access.clone(),
    )
    .unwrap();
    let response = ureq::get(&format!("{}/api/agent/runs/{turn}/events", server.url))
        .set("Host", "reader.example")
        .set("Cookie", &format!("{}={}", crate::auth::COOKIE, f.a))
        .set("Last-Event-ID", &cursor)
        .call()
        .unwrap()
        .into_string()
        .unwrap();
    assert!(response.contains("event: run.snapshot"));
    assert!(response.contains("\"live_buffer_reset\":true"));
    assert!(!response.contains(&format!("id: {cursor}\n")));
    assert!(response.contains("\"persistence_state\":\"saved\""));
    assert_eq!(probe.calls.load(Ordering::SeqCst), 1);
    server.shutdown();
}

#[test]
fn mu6d_observer_capacity_is_owner_scoped_and_logout_only_revokes_its_session() {
    let (f, w, input, _) = setup();
    let accepted = admit(&f, &w, &input).1;
    let turn = accepted["turn_id"].as_str().unwrap();
    let a = context(&f, &f.a);
    let b = context(&f, &f.b);
    assert!(f.access.observation(&b, turn).is_err());
    let mut permits = vec![];
    for _ in 0..f.access.resources.limits.user_sse_connections {
        permits.push(f.access.observation(&a, turn).unwrap());
    }
    assert_eq!(
        f.access.observation(&a, turn).err().unwrap().error_code,
        "OBSERVATION_CAPACITY"
    );
    let descriptor = f
        .access
        .runs
        .stream(&a, turn)
        .unwrap()
        .snapshot()
        .descriptor;
    assert!(permits[0].allows(&descriptor));
    permits.pop();
    assert!(f.access.observation(&a, turn).is_ok());
    assert_eq!(f.call(&f.a, "POST", "/api/auth/logout", json!({})).0, 200);
    assert!(!permits[0].allows(&descriptor));
    assert_eq!(f.call(&f.b, "GET", "/api/me/chats", json!({})).0, 200);
    assert_eq!(
        row(&f, "key")["stage"],
        "queued",
        "logout does not cancel accepted execution"
    );
}

#[test]
fn mu6d_queued_cancel_notifies_existing_observer() {
    let (f, w, input, _) = setup();
    let accepted = admit(&f, &w, &input).1;
    let turn = accepted["turn_id"].as_str().unwrap();
    let a = context(&f, &f.a);
    let stream = f.access.runs.stream(&a, turn).unwrap();
    f.access.runs.cancel(&a, turn).unwrap();
    let (events, terminal) = stream.read_after(None, Duration::ZERO);
    assert!(terminal);
    assert_eq!(events[0].payload["execution_state"], "cancelled");
    assert_eq!(
        events[0].payload["final_view"]["error"]["error_code"],
        "AGENT_RUN_CANCELLED"
    );
}

#[test]
fn mu6d_preclaim_capacity_retry_keeps_the_queued_observation() {
    let (f, w, input, probe) = setup();
    let accepted = admit(&f, &w, &input).1;
    let turn = accepted["turn_id"].as_str().unwrap();
    let a = context(&f, &f.a);
    let original = f.access.runs.stream(&a, turn).unwrap();
    let writer = f.access.users.lock().unwrap().writer();
    *f.access.library.lock().unwrap() = crate::published_library::PublishedLibrary::with_budget(
        ControlStore::open(writer.clone()).unwrap(),
        0,
        0,
    );
    assert!(f.access.runs.run_one(&f.access).is_err());
    assert_eq!(row(&f, "key")["stage"], "queued");
    assert_eq!(probe.calls.load(Ordering::SeqCst), 0);
    assert!(Arc::ptr_eq(
        &original,
        &f.access.runs.stream(&a, turn).unwrap()
    ));
    *f.access.library.lock().unwrap() =
        crate::published_library::PublishedLibrary::new(ControlStore::open(writer).unwrap());
    assert!(f.access.runs.run_one(&f.access).unwrap());
    assert_eq!(original.snapshot().persistence_state, "saved");
}

#[test]
fn mu6d_real_http_disconnect_does_not_repost_and_live_connections_are_bounded() {
    use std::io::BufRead;
    let (f, w, input, mut probe) = setup();
    probe.block = Some(Arc::new((Mutex::new(false), Condvar::new())));
    configure(&f, &probe);
    let accepted = admit(&f, &w, &input).1;
    let turn = accepted["turn_id"].as_str().unwrap();
    let server = multi_user_host::start_with_access(
        "127.0.0.1:0".parse().unwrap(),
        Arc::new(Site::new("https://reader.example").unwrap()),
        f.access.clone(),
    )
    .unwrap();
    wait_calls(&probe, 1);
    let url = format!("{}/api/agent/runs/{turn}/events", server.url);
    let get = || {
        ureq::get(&url)
            .set("Host", "reader.example")
            .set("Cookie", &format!("{}={}", crate::auth::COOKIE, f.a))
    };
    let mut readers = vec![];
    let mut cursor = String::new();
    for _ in 0..4 {
        let mut reader = std::io::BufReader::new(get().call().unwrap().into_reader());
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        assert!(line.starts_with("id: "));
        cursor = line.trim().strip_prefix("id: ").unwrap().to_owned();
        readers.push(reader);
    }
    match get().call() {
        Err(ureq::Error::Status(429, r)) => assert_eq!(
            r.into_json::<Value>().unwrap()["error_code"],
            "OBSERVATION_CAPACITY"
        ),
        _ => panic!("expected bounded observations"),
    }
    drop(readers);
    assert_eq!(probe.calls.load(Ordering::SeqCst), 1);
    release(&probe);
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        if row(&f, "key")["stage"] == "settled" {
            break;
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
    let response = loop {
        match get().set("Last-Event-ID", &cursor).call() {
            Ok(r) => break r.into_string().unwrap(),
            Err(ureq::Error::Status(429, _)) => {
                assert!(std::time::Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(e) => panic!("{e}"),
        }
    };
    assert!(response.contains("event: run.snapshot"));
    assert!(response.contains("\"persistence_state\":\"saved\""));
    assert_eq!(probe.calls.load(Ordering::SeqCst), 1);
    assert_eq!(row(&f, "key")["attempt"], 1);
    server.shutdown();
}

#[test]
fn mu6c_large_usage_continues_nested_round_and_saves_committed_note() {
    let (f, w, input, mut probe) = setup();
    probe.note = true;
    probe.finish_note = true;
    probe.tokens_per_call = 120_001;
    std::fs::write(
        f.root.path().join("service-limits.json"),
        br#"{"model_slots":1,"active_runs":1,"user_workspaces":1}"#,
    )
    .unwrap();
    let f = reopen(f, &probe);
    let w = f.get(&f.a, &w);
    let w = f.action(&f.a, &w, "page", "attach", json!({}));
    let mut input = input;
    input["message"] = json!("请在 1.1 保存笔记");
    let stamp = Fixture::stamp(&w, "page");
    for key in ["generation", "expected_revision", "attachment_id"] {
        input[key] = stamp[key].clone();
    }
    let (status, accepted) = admit(&f, &w, &input);
    assert_eq!(status, 202, "{accepted}");
    let turn = accepted["turn_id"].as_str().unwrap();
    assert!(f.access.runs.run_one(&f.access).unwrap());
    let value = f.access.runs.status(&context(&f, &f.a), turn).unwrap();
    assert_eq!(value["turn"]["status"], "completed", "{value}");
    assert_eq!(value["turn"]["usage"]["calls"].as_array().unwrap().len(), 2);
    assert_eq!(value["turn"]["usage"]["known_total_tokens"], 240_002);
    assert_eq!(probe.calls.load(Ordering::SeqCst), 2);
    assert!(!context(&f, &f.a)
        .user
        .lock()
        .unwrap()
        .store
        .recall(&memory::RecallQuery {
            text: Some("mu6 durable note".into()),
            ..Default::default()
        })
        .is_empty());
    assert!(f
        .access
        .resources
        .try_acquire(crate::service_limits::Resource::Model, "A")
        .is_some());
    assert_eq!(
        f.call(
            &f.a,
            "POST",
            "/api/workspaces",
            json!({"attachment_id":"second","published_book_ref":f.x})
        )
        .1["error_code"],
        "WORKSPACE_CAPACITY"
    );
}

#[test]
fn mu6d_preparing_observation_advances_with_original_preparation_receipt() {
    let (f, w, input, probe) = setup();
    *f.access.runs.fail_after.lock().unwrap() = Some("pending");
    assert_eq!(admit(&f, &w, &input).0, 503);
    let row = row(&f, "key");
    let turn = row["turn"].as_str().unwrap();
    let a = context(&f, &f.a);
    let stream = f.access.runs.stream(&a, turn).unwrap();
    assert_eq!(stream.snapshot().execution_state, "preparing");
    *f.access.runs.fail_after.lock().unwrap() = None;
    f.access.runs.recover(&f.access).unwrap();
    assert_eq!(stream.snapshot().execution_state, "queued");
    assert!(Arc::ptr_eq(
        &stream,
        &f.access.runs.stream(&a, turn).unwrap()
    ));
    assert!(f.access.runs.run_one(&f.access).unwrap());
    assert_eq!(stream.snapshot().persistence_state, "saved");
    assert_eq!(probe.calls.load(Ordering::SeqCst), 1);
}

#[test]
fn jl6_network_original_effect_is_private_and_disposition_survives_restart() {
    let (f, w, mut input, _) = setup();
    jsonl_path(&f);
    let probe = Probe { note: true, finish_note: true, ..Default::default() };
    configure(&f, &probe);
    input["message"] = json!("请在 1.1 保存笔记");
    assert_eq!(admit(&f, &w, &input).0, 202);
    f.access.runs.run_one(&f.access).unwrap();
    let history = context(&f, &f.a).history(w["selected_chat"].as_str()).unwrap();
    let turn = &history["turns"][0];
    let effect = &turn["domain"]["effects"][0];
    assert_eq!(effect["effect"]["effect"]["kind"], "Note");
    let command = json!({"session_id":w["selected_chat"],"turn_id":turn["turn_id"],"effect_id":effect["effect_id"],"action":"keep"});
    let current = f.get(&f.a, &w);
    let saved = f.action(&f.a, &current, "page", "agent/effect/dispose", command.clone());
    assert!(saved["result"]["receipt"]["error"].is_null());
    assert_eq!(saved["result"]["receipt"]["result_object_id"], effect["effect"]["effect"]["mem_id"]);
    let f = reopen(f, &probe);
    let restored = context(&f, &f.a).history(w["selected_chat"].as_str()).unwrap();
    assert_eq!(restored["turns"][0]["domain"]["effects"][0]["disposition"], saved["result"]);
    assert!(context(&f, &f.b).history(w["selected_chat"].as_str()).is_err());
    assert_eq!(probe.calls.load(Ordering::SeqCst), 2);
}

#[test]
fn jl8_network_recap_keeps_private_owner_and_original_publication_without_execution() {
    let (f, w, mut input, _) = setup();
    let probe = Probe { note: true, finish_note: true, ..Default::default() };
    configure(&f, &probe);
    input["message"] = json!("请在 1.1 保存笔记");
    assert_eq!(admit(&f, &w, &input).0, 202);
    f.access.runs.run_one(&f.access).unwrap();
    let url = format!("/api/agent/history/recap?session_id={}", w["selected_chat"].as_str().unwrap());
    let before_calls = probe.calls.load(Ordering::SeqCst);
    let handle = context(&f, &f.a).user;
    let publication = f.access.library.lock().unwrap().load("A", &f.x).unwrap();
    let range = EvidenceRange { start_lid: "1.1".into(), end_lid: "1.1".into(), ranges: vec![] };
    let resolved = publication.book.resolve_source(&range, "zh-CN", None).unwrap();
    {
        let mut user = handle.lock().unwrap();
        let session = &user.agent_history.sessions[0];
        let turn = &session.turns[0];
        let reference = AgentTurnRef { session_id: session.id.clone(), turn_id: turn.turn_id.clone(), user_turn_ordinal: turn.user_turn_ordinal };
        crate::session_runtime::append(&mut user, &reference, "source", crate::session_event::EventBody::SourcesBound { bindings: vec![SourceBinding {
            source_ref_id: "original-source".into(), book_id: f.x.book_id.clone(), evidence_range: range,
            evidence_text_digest: resolved.evidence_text_digest, label_snapshot: resolved.label, preview_snapshot: resolved.preview,
        }] }).unwrap();
    }
    let learning = json!(handle.lock().unwrap().learning_store().unwrap().state().unwrap());
    let memory = json!(handle.lock().unwrap().store.recall(&Default::default()));
    let first = f.ok(&f.a, "GET", &url, json!({}));
    assert_eq!(first["effects"][0]["published_book_ref"], json!(f.x));
    assert!(first["effects"][0]["unavailable_reason"].is_null());
    assert!(first["sources"][0]["unavailable_reason"].is_null());
    let current = f.get(&f.a, &w);
    f.action(&f.a, &current, "page", "book/open", json!({"published_book_ref":f.y}));
    let switched = f.ok(&f.a, "GET", &url, json!({}));
    assert_eq!(switched["sources"], first["sources"]);
    assert_eq!(f.call(&f.b, "GET", &url, json!({})).0, 404);
    f.access.library.lock().unwrap().revoke("A", &f.x).unwrap();
    let unavailable = f.ok(&f.a, "GET", &url, json!({}));
    assert_eq!(unavailable["effects"][0]["status"], first["effects"][0]["status"]);
    assert!(unavailable["effects"][0]["unavailable_reason"].is_string());
    assert!(unavailable["sources"][0]["unavailable_reason"].is_string());
    assert_eq!(unavailable["through_seq"], first["through_seq"]);
    assert_eq!(probe.calls.load(Ordering::SeqCst), before_calls);
    assert_eq!(json!(handle.lock().unwrap().learning_store().unwrap().state().unwrap()), learning);
    assert_eq!(json!(handle.lock().unwrap().store.recall(&Default::default())), memory);
}

#[test]
fn jl7_backup_restores_new_chats_checkpoint_and_original_domain_objects() {
    use crate::session_event::{EventBody, MessageRevision};
    use super::{presentation_store_tests as presentation, tutor_loop_tests as tutor};
    let (f, w, _, probe) = setup();
    let mut state = state_named(&format!("jl7-domain-{}", uuid::Uuid::now_v7()));
    let publication = f.access.library.lock().unwrap().load("A", &f.x).unwrap();
    state.workspace.book = publication.book.clone();
    state.workspace.book_dir = publication.directory.clone();
    state.workspace.reader = Reader::new(&state.workspace.book, DEFAULT_RADIUS);
    state.workspace.publication = Some(publication);
    state.workspace.selected_chat = w["selected_chat"].as_str().map(str::to_owned);
    state.workspace.session_path = None;
    let handle = f.access.users.lock().unwrap().get("A", "now").unwrap();
    // Borrow the real service user's stores for the existing local command core.
    std::mem::swap(&mut state.user, &mut *handle.lock().unwrap());
    state.workspace.messages = state.user.agent_history.sessions[0].messages.clone();
    assert_eq!(post(&mut state, "/tutor/mutate", r#"{"operation_id":"on","expected_revision":0,"action":{"kind":"set_enabled","enabled":true}}"#).status, 200);
    teaching::start_request(&state.private_context(), "now").unwrap();
    let prepared = prepare_agent_chat(&mut state, r#"{"message":"Explain speed"}"#, "now")
        .unwrap_or_else(|r| panic!("{}", r.body));
    let turn = prepared.turn_ref;
    let candidate = presentation::create(&mut state, &turn, None, "Original explanation");
    let reference = presentation::persist(&mut state, &turn, &candidate);
    tutor::select(&state, &turn, "observe", Some(&reference));
    let note = state.user.store.save(SaveInput {
        mem_id: None, mem_type: "note".into(), layer: "session".into(), book_id: f.x.book_id.clone(),
        anchor: Anchor { lid: Some("1.1".into()), concept: None }, content: "Original note".into(),
        range: None, selection_context: None, note_placement: None, citations: None,
        source_session_id: Some(turn.session_id.clone()),
    }, "now").unwrap();
    let effect = AgentEffect::Note { mem_id: note.mem_id.clone(), lid: "1.1".into(), text: note.content };
    let mut outcome = tutor::outcome(Some(&reference));
    outcome.effects.push(effect.clone());
    outcome.source_bindings = candidate.content.source_bindings.clone();
    tutor::finish(&mut state, &turn, &outcome);
    crate::session_runtime::link_teaching(&mut state.user, &turn, "linked").unwrap();
    let receipt = crate::effect_disposition::route(&mut state,
        &json!({"session_id":turn.session_id,"turn_id":turn.turn_id,"effect_id":runtime::orchestrator::effect_id(&effect),"action":"keep"}), "kept").unwrap();
    assert!(receipt.receipt.as_ref().unwrap().error.is_none());
    let next = precommit_agent_turn(&mut state, &f.x.book_id, "Failed follow-up".into(), None, None, None, "later").unwrap();
    let messages = state.workspace.messages.clone();
    finalize_agent_turn(&mut state, &next, AgentAssistantStatus::Failed, None,
        Some(AgentTurnError { error_code: "PROVIDER_ERROR".into(), category: "provider".into(), message: "interrupted follow-up".into() }), None, &messages, "failed").unwrap();

    let mut answer = Message::user("Original explanation ".repeat(1000));
    answer.role = runtime::Role::Assistant;
    let messages = vec![Message::system("base"), Message::user("Original question ".repeat(1000)), answer, Message::user("Failed follow-up")];
    crate::session_runtime::append(&mut state.user, &next, "history", EventBody::HistoryRevised(MessageRevision { from: 0, suffix: messages.clone() })).unwrap();
    let compact = runtime::prepare_compaction(runtime::CompactionPhase::MidTurn, &messages, &messages, vec![], vec![], vec![], BTreeMap::new()).unwrap();
    let ids = compact.request().eligible_items.iter().map(|s| s.source_item_id.clone()).collect::<Vec<_>>();
    let generator = CompactionDraftAdapter { output: RefCell::new(Some(json!({
        "active_goal":[{"item_id":"goal","text":"Continue original question","source_item_ids":ids,"evidence_refs":[]}],
        "progress":[],"decisions":[],"user_constraints":[],"open_obligations":[],"unresolved_ambiguities":[],"critical_facts":[],"critical_examples":[],"next_steps":[],
        "source_coverage":ids.iter().map(|id|json!({"source_item_id":id,"disposition":"compacted","target_item_ids":["goal"]})).collect::<Vec<_>>()
    }))) };
    let checkpoint = runtime::compact_with_adapter(&generator,
        &runtime::ModelRuntimeProfile::fallback("fixture", runtime::ProviderToolProtocol::Native), &compact,
        runtime::CompactionLimits { generation_input_limit_tokens: 100_000, target_active_tokens: 20_000 }).unwrap();
    install_agent_compaction_checkpoint(&mut state, &turn.session_id, checkpoint.clone()).unwrap();
    assert_eq!(route_agent_new(&mut state, "newer").status, 200);
    assert_eq!(route_agent_history_select(&mut state, &json!({"session_id":turn.session_id}).to_string()).status, 200);
    let expected = json!(state.user.agent_history);
    let history_path = state.user.history_path.clone().unwrap();
    let old = b"archived old JSON is never read or rewritten";
    std::fs::write(&history_path, old).unwrap();
    let paths = state.user.session_store.as_ref().unwrap().paths.clone();
    let log_bytes = std::fs::read(paths.session(&turn.session_id)).unwrap();
    let selection_bytes = std::fs::read(&paths.selection).unwrap();
    let learning_before = json!(state.user.learning_store().unwrap().state().unwrap());
    drop(prepared.scope);
    drop(state); drop(handle);
    let Fixture { root, access, control, x, .. } = f;
    drop(access); drop(control);
    let backups = tempfile::tempdir().unwrap();
    let snapshot = backups.path().join("snapshot");
    crate::reader_maintenance::backup_service(root.path(), &snapshot).unwrap();
    let restored = backups.path().join("restored");
    crate::reader_maintenance::restore_service(&snapshot, &restored).unwrap();
    let mut users = UserRegistry::open(&restored).unwrap();
    let control = ControlStore::open(users.writer()).unwrap();
    let mut library = crate::published_library::PublishedLibrary::new(control);
    let publication = library.load("A", &x).unwrap();
    let handle = users.get("A", "now").unwrap();
    let user = handle.lock().unwrap();
    assert_eq!(json!(user.agent_history), expected);
    let paths = &user.session_store.as_ref().unwrap().paths;
    assert_eq!(std::fs::read(paths.session(&turn.session_id)).unwrap(), log_bytes);
    assert_eq!(std::fs::read(&paths.selection).unwrap(), selection_bytes);
    assert_eq!(std::fs::read(user.history_path.as_ref().unwrap()).unwrap(), old);
    let session = user.agent_history.sessions.iter().find(|s| s.id == turn.session_id).unwrap();
    assert_eq!(session.compaction_checkpoint, Some(checkpoint));
    assert_eq!(session.turns[1].status, AgentAssistantStatus::Failed);
    assert_eq!(session.turns[0].published_book_ref.as_ref(), Some(&x));
    let private = PrivateBookContext { user: &user, book: &publication.book, book_dir: publication.directory(), messages: &session.messages, selected_chat: Some(&turn.session_id) };
    assert_eq!(private.read_presentation(&turn.session_id, &reference).unwrap().content, candidate.content);
    assert!(!session.turns[0].source_bindings.is_empty());
    for source in &session.turns[0].source_bindings {
        assert_eq!(publication.book.resolve_source(&source.evidence_range, "zh-CN", None).unwrap().evidence_text_digest, source.evidence_text_digest);
    }
    assert_eq!(user.store.recall(&RecallQuery { layer: Some("long_term".into()), ..Default::default() })[0].mem_id, note.mem_id);
    let learning = user.learning_store().unwrap();
    assert_eq!(json!(learning.state().unwrap()), learning_before);
    assert!(!session.turns[0].domain.teaching.is_empty());
    for link in &session.turns[0].domain.teaching {
        for id in &link.receipt_ids { assert_eq!(learning.teaching_event(id).unwrap().binding.chat_session_id, turn.session_id); }
    }
    assert_eq!(probe.calls.load(Ordering::SeqCst), 0);
}
