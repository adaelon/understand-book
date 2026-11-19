use super::*;
use crate::agent_run::RuntimeStatePort;
use runtime::presentation_interact::*;
use runtime::run_context::{CancellationToken, ResidentStatePort};
use crate::agent_stream::{RunDescriptor, RunStream};

#[path = "presentation_demonstration_tests.rs"]
pub mod om5;

pub fn interactive_fixture() -> (tempfile::TempDir, AppState, AgentTurnRef, PresentationRef) {
    let (root, mut state, delivered) = fixture();
    let mut draft = content(&state, "速度对照");
    draft.source_bindings.clear();
    draft.assumptions.clear();
    draft.readable_content = "A 每秒走 1 米；B 速度改变后需要应用，两车使用共同时间。".into();
    draft.content_files.insert("index.html".into(), include_str!("../../../../packages/web/playwright/fixtures/presentation-apply-state.html").into());
    draft.action_contract = serde_json::from_value(json!([
        {"kind":"set_parameters","parameters":["input_speed_b_m_s"]},
        {"kind":"invoke","action_id":"apply","label":"应用"},
        {"kind":"invoke","action_id":"reset","label":"重置"},
        {"kind":"highlight","region_id":"result"},
        {"kind":"reveal","region_id":"relation"},
        {"kind":"annotate","region_id":"annotation"}
    ])).unwrap();
    draft.initial_state = json!({"input_speed_b_m_s":1,"applied_speed_b_m_s":1,"time_s":10,"phase":"ready","revealed":false,"annotated":false});
    draft.state_contract = json!({"input_speed_b_m_s":"B input speed in m/s, 0..3","applied_speed_b_m_s":"B displayed speed in m/s, 0..3",
        "time_s":"Shared time in seconds, 0..10","phase":"ready, awaiting_apply or cancelled","revealed":"relation shown, boolean","annotated":"annotation shown, boolean"});
    let candidate = state.private_context().create_presentation_candidate(&delivered.session_id, &delivered.turn_id, None, draft).unwrap();
    let reference = persist(&mut state, &delivered, &candidate);
    finish(&mut state, &delivered, &reference).unwrap();
    (root, state, delivered, reference)
}
pub fn start_stream(state: &mut AppState, turn: &AgentTurnRef) -> Arc<RunStream> {
    let stream = RunStream::new(RunDescriptor { book_id:state.workspace.book.base.book_id.clone(), session_id:turn.session_id.clone(), turn_id:turn.turn_id.clone() });
    state.workspace.active_agent_stream = Some(Arc::downgrade(&stream));
    stream
}
pub fn wait_command(stream: &RunStream) -> PresentationCommand {
    let until = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(command) = stream.snapshot().presentation_command { return command; }
        assert!(std::time::Instant::now() < until, "command was not published");
        std::thread::sleep(Duration::from_millis(5));
    }
}
pub fn run_command(state: Arc<Mutex<AppState>>, turn: AgentTurnRef, request: InteractRequest, token: CancellationToken)
    -> std::thread::JoinHandle<Result<Value, ToolError>> {
    std::thread::spawn(move || {
        let scope = state.with_app(|s| crate::run_scope::RunScope::capture(s, &turn, "operate", None, None));
        let mut port = RuntimeStatePort { port:&state, scope:&scope, turn_ref:&turn, previewed:Default::default(), animations:Default::default(), plots:Default::default() };
        port.interact_presentation(request, &token)
    })
}

#[test]
fn om4_local_waits_for_saved_result_and_cancelled_commands_cannot_replay() {
    let (_root, mut state, delivered, reference) = interactive_fixture();
    let turn = next_turn(&mut state);
    let stream = start_stream(&mut state, &turn);
    let state = Arc::new(Mutex::new(state));
    let token = CancellationToken::default();
    let task = run_command(state.clone(), turn.clone(), InteractRequest::Observe { board_target:None, reference:reference.clone() }, token.clone());
    let command = wait_command(&stream);
    let claim = json!({"operation":"claim","command_id":command.command_id,"run_turn_id":turn.turn_id,"page_id":"page-1"});
    let response = state.with_app(|s| post(s,"/agent/presentation.interaction",&claim.to_string()));
    assert!(response.body.contains("\"claimed\":true"), "{}", response.body);
    assert!(!task.is_finished(), "sending/claiming must not count as completion");
    let mut complete = json!({"operation":"complete","command_id":command.command_id,"run_turn_id":turn.turn_id,"page_id":"page-1","status":"observed",
        "receipt":{"session_id":delivered.session_id,"turn_id":delivered.turn_id,"reference":reference,"state_revision":1,"saved_state_ref":"invented"}});
    assert_ne!(state.with_app(|s| post(s,"/agent/presentation.interaction",&complete.to_string())).status,200);
    let scene = json!({"values":{"page":{"input_speed_b_m_s":1,"applied_speed_b_m_s":1,"time_s":10}},"visible_step":"ready","observed_result":"B 走 10 米","source_ref_ids":[]});
    let saved = state.with_app(|s| post(s,"/agent/presentation.state.save",&json!({"session_id":delivered.session_id,"turn_id":delivered.turn_id,"reference":reference,"state":scene}).to_string()));
    assert_eq!(saved.status,200,"{}",saved.body);
    complete["receipt"] = serde_json::from_str(&saved.body).unwrap();
    assert_eq!(state.with_app(|s| post(s,"/agent/presentation.interaction",&complete.to_string())).status,200);
    let result = task.join().unwrap().unwrap();
    assert_eq!(result["state"],scene);
    assert_eq!(result["reference"],json!(reference));
    assert_eq!(result["page_id"],"page-1");
    assert!(result["state_contract"]["input_speed_b_m_s"].as_str().unwrap().contains("0..3"));
    assert!(stream.snapshot().presentation_command.is_none());
    let based_on = result["saved_state_ref"].as_str().unwrap().to_string();
    let action = InteractRequest::Act { board_target:None, reference:reference.clone(), based_on:based_on.clone(), action:PresentationAction::SetParameters {values:BTreeMap::from([("input_speed_b_m_s".into(),json!(2))])} };
    let task = run_command(state.clone(), turn.clone(), action, token.clone());
    let command = wait_command(&stream);
    assert_eq!(command.expected_values["input_speed_b_m_s"],1);
    assert_eq!(command.page_id.as_deref(),Some("page-1"));
    let claim = json!({"operation":"claim","command_id":command.command_id,"run_turn_id":turn.turn_id,"page_id":"page-1"});
    assert!(state.with_app(|s| post(s,"/agent/presentation.interaction",&claim.to_string())).body.contains("\"claimed\":true"));
    assert!(state.with_app(|s| post(s,"/agent/presentation.interaction",&claim.to_string())).body.contains("\"claimed\":false"));
    token.cancel(); stream.cancelling();
    assert_eq!(task.join().unwrap().unwrap_err().error_code,"AGENT_RUN_CANCELLED");
    complete["command_id"]=json!(command.command_id); complete["status"]=json!("applied");
    assert_ne!(state.with_app(|s| post(s,"/agent/presentation.interaction",&complete.to_string())).status,200);
    assert_eq!(state.with_app(|s| s.private_context().read_presentation(&delivered.session_id,&reference).unwrap().reference.revision),1);
}

#[test]
fn om4_missing_page_and_switching_workspace_end_with_actual_unavailability() {
    let (_root, mut state, _, reference) = interactive_fixture();
    let turn = next_turn(&mut state);
    let stream = start_stream(&mut state,&turn);
    let state=Arc::new(Mutex::new(state));
    let task=run_command(state.clone(),turn.clone(),InteractRequest::Observe { board_target:None, reference:reference.clone() },CancellationToken::default());
    let command=wait_command(&stream);
    let result=state.with_app(|s|post(s,"/agent/presentation.interaction",&json!({"operation":"claim","command_id":command.command_id,"run_turn_id":turn.turn_id,"page_id":null}).to_string()));
    assert_eq!(result.status,200);
    assert_eq!(task.join().unwrap().unwrap()["status"],"unavailable");
    let task=run_command(state.clone(),turn,InteractRequest::Observe {board_target:None, reference},CancellationToken::default());
    wait_command(&stream);
    state.with_app(|s|s.workspace.generation+=1);
    assert_eq!(task.join().unwrap().unwrap_err().error_code,"WORKSPACE_STALE");
}

#[test]
fn om4_author_previews_declared_actions_and_rejects_missing_handler() {
    use runtime::presentation_author::AuthorRequest;
    let (_root, mut state, delivered, reference) = interactive_fixture();
    let content = state.private_context().read_presentation(&delivered.session_id, &reference).unwrap().content;
    let turn = next_turn(&mut state);
    let app = crate::agent_run::BorrowedAppPort(std::cell::RefCell::new(&mut state));
    let scope = app.with_app(|s|crate::run_scope::RunScope::capture(s,&turn,"preview",None,None));
    let mut port=RuntimeStatePort {port:&app,scope:&scope,turn_ref:&turn,previewed:Default::default(),animations:Default::default(),plots:Default::default()};
    let write=json!({"operation":"write","new_object":true,"title":content.title,"html":content.content_files["index.html"],
        "readable_content":content.readable_content,"state_contract":content.state_contract,"initial_state":content.initial_state,"action_contract":content.action_contract});
    let written=port.author_presentation(serde_json::from_value(write.clone()).unwrap(),&[],&[],&CancellationToken::default()).unwrap();
    let mut reports=vec![];
    for (_,viewport) in runtime::presentation_preview::REQUIRED_PREVIEW_ENVIRONMENTS {
        let request=json!({"operation":"preview","candidate_id":written.body["candidate_id"],"viewport":viewport,
            "read_selector":"#result","actions":[
                {"kind":"interact","action":{"kind":"set_parameters","values":{"input_speed_b_m_s":2}}},
                {"kind":"interact","action":{"kind":"invoke","action_id":"apply"}},
                {"kind":"interact","action":{"kind":"reveal","region_id":"relation"}},
                {"kind":"interact","action":{"kind":"annotate","region_id":"annotation"}}]});
        let result=port.author_presentation(serde_json::from_value(request).unwrap(),&[],&[],&CancellationToken::default()).unwrap();
        assert!(result.body["error_code"].is_null(),"{}",result.body);
        assert!(result.body["reading"]["text"].as_str().unwrap().contains("B 走 20.0 米"));
        reports.push(result.body);
    }
    let mut broken=write;
    broken["html"]=json!(content.content_files["index.html"].replace("registerActionHandler?.","missingActionHandler?."));
    let written=port.author_presentation(serde_json::from_value(broken).unwrap(),&[],&[],&CancellationToken::default()).unwrap();
    let result=port.author_presentation(serde_json::from_value::<AuthorRequest>(json!({"operation":"preview","candidate_id":written.body["candidate_id"]})).unwrap(),
        &[],&[],&CancellationToken::default()).unwrap();
    assert_eq!(result.body["status"],"preview_failed");
    assert!(result.body.to_string().contains("registerActionHandler"));
    if let Ok(path)=std::env::var("OM4_OUTPUT") {
        std::fs::write(std::path::Path::new(&path).join("author-preview.json"),serde_json::to_vec_pretty(&json!({"valid":reports,"missing_handler":result.body})).unwrap()).unwrap();
    }
}

#[test]
#[ignore = "OM4 real Reader/browser host; run with OM4_READER_HOST"]
fn om4_browser_host() {
    let (_root, mut state, delivered, reference)=interactive_fixture();
    let turn=next_turn(&mut state);
    let stream=start_stream(&mut state,&turn);
    let descriptor=json!(stream.snapshot().descriptor);
    let state=Arc::new(Mutex::new(state));
    let token=CancellationToken::default();
    let result=Arc::new(Mutex::new(Value::Null));
    let server=tiny_http::Server::http("127.0.0.1:4190").unwrap();
    println!("OM4_BROWSER_READY");
    for mut request in server.incoming_requests() {
        let path=request.url().to_owned();
        if path.contains("/events") {
            let after=path.split("?after=").nth(1).and_then(|s|s.parse().ok());
            crate::agent_stream::serve(request,stream.clone(),after); continue;
        }
        let mut body=String::new();request.as_reader().read_to_string(&mut body).unwrap();
        let reply=if path=="/fixture" {
            ok_json(&json!({"session_id":delivered.session_id,"turn_id":delivered.turn_id,"reference":reference,"outcome":outcome(&reference),"run_descriptor":descriptor}))
        } else if path=="/om4/command" {
            assert!(stream.snapshot().presentation_command.is_none());
            *result.lock().unwrap()=Value::Null;
            let input:InteractRequest=serde_json::from_str(&body).unwrap();
            let task=run_command(state.clone(),turn.clone(),input,token.clone());
            let result=result.clone();
            std::thread::spawn(move||*result.lock().unwrap()=match task.join().unwrap(){Ok(v)=>v,Err(e)=>json!(e)});
            ok_json(&json!({"started":true}))
        } else if path=="/om4/result" { ok_json(&*result.lock().unwrap())
        } else if path.ends_with("/cancel") {
            token.cancel();stream.cancelling();ok_json(&json!(stream.snapshot()))
        } else if path.starts_with("/agent/runs/") {ok_json(&json!(stream.snapshot()))
        } else if path=="/stop" {
            request.respond(tiny_http::Response::from_string("stopped")).unwrap();break;
        } else {
            state.with_app(|s|route(s,Req {method:request.method().as_str(),url:&path,body:&body,now:"om4-browser"}))
        };
        request.respond(tiny_http::Response::from_string(reply.body).with_status_code(reply.status)
            .with_header(tiny_http::Header::from_bytes("Content-Type","application/json").unwrap())).unwrap();
    }
}
