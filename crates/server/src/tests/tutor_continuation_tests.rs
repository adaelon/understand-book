pub use reader_service::tests::tutor_continuation_tests::*;
// TC0: real storage/admission baselines and opt-in red contracts for TC1–TC6.
// Run ignored tests explicitly; an assertion failure is the recorded missing behavior.
use super::*;
use memory::teaching::TeachingFact;
use std::sync::atomic::{AtomicUsize, Ordering};


#[test]
fn continuation_tc6_network_snapshot_and_review_keep_choice_separate() {
    let (f,w,input,_) = tc4_network_offer();
    let (status,accepted)=admit(&f,&w,input); assert_eq!(status,202);
    let handle=f.access.users.lock().unwrap().get_for_recovery("A","1000").unwrap();
    let user=handle.lock().unwrap();
    let context=user.learning_store().unwrap().teaching_event(&format!("binding:{}",accepted["turn_id"].as_str().unwrap())).unwrap().payload;
    assert!(context["learner_context"]["recent_facts"].as_array().unwrap().iter().any(|f|f["kind"]=="continuation_chosen"));
    let chosen=format!("chosen:{}",accepted["turn_id"].as_str().unwrap());
    assert!(!context["learner_context"]["uninterpreted_fact_refs"].as_array().unwrap().contains(&json!(chosen)));
    let session=&user.agent_history.sessions[0];
    let job:memory::ReviewJob=serde_json::from_value(json!({"job_id":"network-tc6","session_id":session.id,"book_id":session.book_id,
        "from_turn_exclusive":0,"to_turn_inclusive":2,"status":"queued","attempts":0,"created_at":"0","updated_at":"0"})).unwrap();
    let review=host::copy_user_review_input(&user,&job,"technical_learning").unwrap();
    assert_eq!(review.turns[1].user,"");
    assert!(review.turns[1].input_action.is_some());
}

#[test]
fn continuation_tc5_network_display_uses_original_workspace_authority() {
    let (f,w,choice,probe) = tc4_network_offer();
    let mut input = super::mu5_tests::Fixture::stamp(&w,"tc0-page");
    input["delivery_ref"]=choice["teaching_choice"]["delivery_ref"].clone();
    input["option_ids"]=json!(["compare"]); input["recommended_option_id"]=json!("compare");
    let (status,result)=f.call(&f.a,"POST",&format!("/api/workspaces/{}/tutor/continuation-displayed",w["workspace_id"].as_str().unwrap()),input);
    assert_eq!(status,200,"{result}"); assert!(result.to_string().contains("continuation-display:"));
    assert_eq!(probe.0.load(Ordering::SeqCst),0);
}

#[test]
#[ignore = "TC5 bounded real API browser fixture; set TC5_BROWSER_OUTPUT"]
fn continuation_tc5_browser_api() {
    use runtime::presentation::PresentationContent;
    let output=std::path::PathBuf::from(std::env::var("TC5_BROWSER_OUTPUT").unwrap());
    std::fs::create_dir_all(&output).unwrap();
    let server=tiny_http::Server::http("127.0.0.1:4196").unwrap();
    let mut retained=Vec::new();
    let (_root,initial)=fixture(true); retained.push(_root);
    let state=Arc::new(std::sync::Mutex::new(initial));
    let coordinator=Arc::new(agent_run::RunCoordinator::new(state.clone(),Arc::new(std::sync::atomic::AtomicBool::new(false)),observability::ObservabilityRuntime::disabled(None)));
    let samples=Arc::new(std::sync::Mutex::new(Vec::new()));
    let mut network_fixture: Option<(super::mu5_tests::Fixture, Value)> = None;
    let deadline=std::time::Instant::now()+Duration::from_secs(1200);
    println!("TC5_BROWSER_READY");
    while std::time::Instant::now()<deadline {
        let Some(mut request)=server.recv_timeout(Duration::from_secs(1)).unwrap() else {continue;};
        let path=request.url().to_string();
        if path=="/stop" { request.respond(tiny_http::Response::from_string("stopped")).unwrap(); break; }
        let mut body=String::new(); request.as_reader().read_to_string(&mut body).unwrap();
        let method=request.method().as_str().to_owned();
        let reply=if path=="/network-reset" {
            let (f,w,input,_)=tc4_network_offer();
            let target=tc3_move(&state.lock().unwrap())["target"].clone();
            let handle=f.access.users.lock().unwrap().get_for_recovery("A","1000").unwrap();
            let previous=handle.lock().unwrap().learning_store().unwrap().teaching_event(input["teaching_choice"]["delivery_ref"].as_str().unwrap()).unwrap()
                .payload["move"]["design_ref"].as_str().map(str::to_owned);
            let collected=samples.clone(); samples.lock().unwrap().clear();
            f.access.runs.configure_fake(move || Box::new(TutorRequestRecorder {samples:collected.clone(),directory:None,
                inner:Box::new(tc4_delivery(target.clone(),previous.clone())) }));
            let result=ok_json(&w); network_fixture=Some((f,w)); result
        } else if let Some(path)=path.strip_prefix("/network") {
            let (f,_)=network_fixture.as_ref().unwrap();
            let input=serde_json::from_str(&body).unwrap_or(json!({}));
            let (status,value)=f.call(&f.a,&method,path,input);
            if method=="POST" && path.ends_with("/agent/runs") && status==202 { f.access.runs.run_one(&f.access).unwrap(); }
            Reply {status,body:value.to_string()}
        } else if path.starts_with("/reset") {
            let (root,mut fresh)=fixture(true); retained.push(root); enable(&mut fresh);
            let run=prepare(&mut fresh,json!({"message":"Explain average speed","question_quote":quote()}));
            let mut movement=tc3_move(&fresh);
            let reference=if path.contains("presentation") {
                let candidate=fresh.private_context().create_presentation_candidate(&run.turn_ref.session_id,&run.turn_ref.turn_id,None,
                    PresentationContent { action_contract: vec![], teaching_binding: None, method_refs: None, method_observations: Default::default(), animation_assets:Default::default(),title:"平均速度：观察总时间".into(),
                        content_files:BTreeMap::from([("index.html".into(),"<h2>总路程固定为 120 公里</h2><label>总时间 <input id=hours type=range min=1 max=6 value=2></label><p>观察时间变长后的平均速度。</p>".into())]),
                        entrypoint:"index.html".into(),readable_content:"总路程固定；增加总时间时，每小时对应的路程减少。".into(),
                        source_bindings:vec![],assumptions:vec![],state_contract:json!({}),initial_state:json!({"hours":2}) }).unwrap();
                let reference=fresh.private_context().persist_presentation_candidate(&run.turn_ref.session_id,&run.turn_ref.turn_id,&candidate.candidate_id).unwrap();
                movement["presentation"]=json!(reference); Some(reference)
            } else {None};
            tc3_select(&fresh,&run.turn_ref,movement).unwrap();
            let mut result=super::tutor_loop_tests::outcome(reference.as_ref());
            result.trace=vec![runtime::orchestrator::TraceStep { tool:"tutor.step".into(),args:"{}".into(),result_digest:"selected".into(),model_tool_loop:None,query_audit:None }];
            super::tutor_loop_tests::finish(&mut fresh,&run.turn_ref,&result);
            samples.lock().unwrap().clear(); *state.lock().unwrap()=fresh;
            ok_json(&json!({"session_id":run.turn_ref.session_id,"turn_id":run.turn_ref.turn_id,"reference":reference}))
        } else if path=="/evidence" {
            let state=state.lock().unwrap(); let session=state.user.agent_history.sessions.last().unwrap();
            let events=state.user.learning_store().unwrap().teaching_turn_events(
                state.user.learning_store().unwrap().state().unwrap().control.current_tutor_session_id.as_deref().unwrap(),&session.turns.last().unwrap().turn_id).unwrap();
            let value=json!({"history":agent_history_response(&state.user.agent_history,&state.workspace.book,state.workspace.selected_chat.as_deref()).unwrap(),
                "events":events,"provider_calls":samples.lock().unwrap().len()});
            let file=if session.turns[0].outcome.as_ref().unwrap().answer_view.as_ref().unwrap().parts.len()>1 {"presentation"} else {"ordinary"};
            std::fs::write(output.join(format!("{file}.json")),serde_json::to_vec_pretty(&value).unwrap()).unwrap(); ok_json(&value)
        } else if path=="/agent/runs" {
            let target=tc3_move(&state.lock().unwrap())["target"].clone();
            let previous={let state=state.lock().unwrap();let store=state.user.learning_store().unwrap();
                store.state().unwrap().control.current_tutor_session_id.as_deref().and_then(|session|
                    store.current_teaching_design(session,&state.workspace.book.base.book_id,state.workspace.book.source_fingerprint()).unwrap().map(|event|event.event_id))};
            coordinator.start(&body,"1001",Box::new(TutorRequestRecorder {samples:samples.clone(),directory:None,
                inner:Box::new(tc4_delivery(target,previous)) }),||{},||{})
        } else if let Some(id)=path.strip_prefix("/agent/runs/") {
            let id=id.split('/').next().unwrap();
            match coordinator.stream(id) {Some(stream)=>ok_json(&json!(stream.snapshot())),None=>Reply {status:404,body:"{}".into()}}
        } else {
            route(&mut state.lock().unwrap(),Req {method:&method,url:&path,body:&body,now:"1001"})
        };
        request.respond(tiny_http::Response::from_string(reply.body).with_status_code(reply.status)
            .with_header(tiny_http::Header::from_bytes("Content-Type","application/json").unwrap())).unwrap();
    }
}

fn tc4_network_offer() -> (super::mu5_tests::Fixture, Value, Value, Probe) {
    let (f, w, mut input, probe) = network_fixture();
    let reference = serde_json::from_value(w["published_book_ref"].clone()).unwrap();
    let publication = f.access.library.lock().unwrap().load("A", &reference).unwrap();
    input["expected_chat_head"]=Value::Null;
    input["entry_action"]=json!("learn_with_purpose");input["learning_purpose"]=json!("practice");
    input["source_id"]=json!(publication.book.base.book_id);input["source_revision"]=json!(publication.book.source_fingerprint());
    let handle = f.access.users.lock().unwrap().get_for_recovery("A", "1000").unwrap();
    handle.lock().unwrap().learning_store().unwrap().mutate(&serde_json::from_value(json!({"operation_id":"enable",
        "expected_revision":0,"action":{"kind":"set_enabled","enabled":true}})).unwrap(), "1000").unwrap();
    let (status, accepted) = admit(&f, &w, input); assert_eq!(status, 202, "{accepted}");
    let reference = serde_json::from_value(w["published_book_ref"].clone()).unwrap();
    let publication = f.access.library.lock().unwrap().load("A", &reference).unwrap();
    {
        let mut user = handle.lock().unwrap();
        let session = &user.agent_history.sessions[0]; let turn = &session.turns[0];
        let private = PrivateBookContext { learning_workspace_id:None, user:&user, book:&publication.book, book_dir:publication.directory(),
            messages:&session.messages, selected_chat:Some(&session.id) };
        let (_root, state) = fixture(true);
        let movement = tc3_move(&state);
        let reference = AgentTurnRef { session_id:session.id.clone(), turn_id:turn.turn_id.clone(), user_turn_ordinal:turn.user_turn_ordinal };
        let selected=super::tutor_loop_tests::step_with_design(&private, &reference,
            json!({"operation":"select","move":movement}), &[], &[EvidenceRange { start_lid:"1.1".into(), end_lid:"1.1".into(), ranges:vec![] }]).unwrap();
        let mut result = super::tutor_loop_tests::outcome(None);
        result.teaching_delivery_move_refs=serde_json::from_value(selected["delivery_move_refs"].clone()).unwrap();
        finalize_user_agent_turn(&mut user, &reference, AgentAssistantStatus::Completed, Some(result), None, None, &[], "1000").unwrap();
    }
    f.access.runs.recover(&f.access).unwrap();
    let mut input = super::mu5_tests::Fixture::stamp(&w, "tc0-page");
    input["session_id"] = w["selected_chat"].clone(); input["client_request_id"] = json!("choice");
    input["message"] = json!(""); input["expected_chat_head"] = accepted["turn_id"].clone();
    input["teaching_choice"] = json!({"delivery_ref":format!("delivery:{}:explain", accepted["turn_id"].as_str().unwrap()), "option_id":"compare"});
    (f,w,input,probe)
}

#[test]
fn continuation_tc4_network_choice_recovers_preparation_boundaries() {
    for point in ["preparing", "pending", "chosen_receipt", "teaching_receipt", "prepared", "queued"] {
        let (f,w,input,probe) = tc4_network_offer();
        *f.access.runs.fail_after.lock().unwrap() = Some(point);
        let (status, error) = admit(&f, &w, input.clone()); assert_eq!(status,503,"{point}: {error}");
        *f.access.runs.fail_after.lock().unwrap() = None;
        f.access.runs.recover(&f.access).unwrap();
        let (status, accepted) = admit(&f, &w, input.clone());
        if point == "preparing" {
            assert_eq!(accepted["error_code"], "REQUEST_KEY_CLOSED");
            assert_eq!(probe.0.load(Ordering::SeqCst), 0); continue;
        }
        assert_eq!(status,202,"{point}: {accepted}");
        drive_network_coach(&f);
        assert!(f.access.runs.run_one(&f.access).unwrap());
        assert!(!f.access.runs.run_one(&f.access).unwrap());
        let calls = probe.0.load(Ordering::SeqCst);
        assert!(calls > 0);
        for _ in 0..2 { assert_eq!(admit(&f,&w,input.clone()).1["turn_id"], accepted["turn_id"]); }
        assert_eq!(probe.0.load(Ordering::SeqCst),calls);
        assert_eq!(f.control.connection.query_row("SELECT attempt FROM run_admissions WHERE owner_user_id='A' AND turn_id=?",
            [accepted["turn_id"].as_str().unwrap()], |r| r.get::<_,u64>(0)).unwrap(), 1);
        let handle = f.access.users.lock().unwrap().get_for_recovery("A", "1000").unwrap();
        let user = handle.lock().unwrap(); let turn = &user.agent_history.sessions[0].turns[1];
        assert!(turn.teaching_ref.is_none());
        let chosen = user.learning_store().unwrap().teaching_event(&format!("chosen:{}", turn.turn_id)).unwrap();
        assert_eq!(chosen.payload["accepted_turn_id"], accepted["turn_id"]);
        let events = user.learning_store().unwrap().teaching_turn_events(&chosen.binding.tutor_session_id, &turn.turn_id).unwrap();
        assert_eq!(events.iter().filter(|e| e.kind == TeachingFact::ContinuationChosen).count(),1);
        assert!(!events.iter().any(|e| e.kind == TeachingFact::LearnerAction));
        println!("TC4_EVIDENCE {}", json!({"host":"network","action":"choice","fault":point,"request":input,
            "accepted_turn":turn.turn_id,"chosen_event":chosen,"execution_starts":1,"provider_calls":calls,"terminal":turn.status}));
    }
}

#[test]
fn continuation_tc4_network_two_choices_conflicts_and_unknown_execution() {
    let (f,w,input,probe) = tc4_network_offer();
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let handles: Vec<_> = ["compare", "example"].into_iter().map(|option| {
        let access = f.access.clone(); let token = f.a.clone(); let mut input = input.clone();
        input["client_request_id"] = json!(option); input["teaching_choice"]["option_id"] = json!(option);
        let workspace = w["workspace_id"].as_str().unwrap().to_owned(); let barrier = barrier.clone();
        std::thread::spawn(move || {
            let principal = access.auth.authenticate(&token, multi_user_host::now()).unwrap();
            let context = access.context(principal, multi_user_host::now()).unwrap();
            barrier.wait();
            let result = access.runs.admit(&access, &context, &workspace, &input, "1000");
            (input, result)
        })
    }).collect();
    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|(_, r)| r.is_ok()).count(), 1);
    let (original, accepted) = results.iter().find(|(_, r)| r.is_ok()).unwrap();
    let accepted = accepted.as_ref().unwrap();
    let mut changed = original.clone(); changed["teaching_choice"]["option_id"] = json!("different");
    assert_eq!(admit(&f,&w,changed).1["error_code"], "REQUEST_KEY_CONFLICT");
    *f.access.runs.fail_after.lock().unwrap() = Some("claimed");
    drive_network_coach(&f);
    assert!(f.access.runs.run_one(&f.access).is_err());
    *f.access.runs.fail_after.lock().unwrap() = None;
    f.access.runs.recover(&f.access).unwrap();
    assert!(!f.access.runs.run_one(&f.access).unwrap());
    for _ in 0..3 { assert_eq!(admit(&f,&w,original.clone()).1["turn_id"], accepted["turn_id"]); }
    assert_eq!(probe.0.load(Ordering::SeqCst),0);
    let handle = f.access.users.lock().unwrap().get_for_recovery("A","1000").unwrap();
    let user = handle.lock().unwrap(); let turn = &user.agent_history.sessions[0].turns[1];
    assert_eq!(turn.error.as_ref().unwrap().error_code,"INTERRUPTED");
    assert!(user.learning_store().unwrap().teaching_event(&format!("chosen:{}",turn.turn_id)).is_ok());
    assert_eq!(f.control.connection.query_row("SELECT count(*) FROM admin_usage_events WHERE user_id='A' AND kind='question' AND event_ref=?",
        [&turn.turn_id], |r| r.get::<_,u64>(0)).unwrap(), 1);
}

#[test]
fn continuation_tc4_both_hosts_execute_chosen_action_to_delivery() {
    let (_root, mut state) = fixture(true);
    let (_origin, input) = tc4_offer(&mut state);
    let target = tc3_move(&state)["target"].clone();
    let previous=state.user.learning_store().unwrap().teaching_event(input["teaching_choice"]["delivery_ref"].as_str().unwrap()).unwrap()
        .payload["move"]["design_ref"].as_str().map(str::to_owned);
    let samples = Arc::new(std::sync::Mutex::new(Vec::new()));
    state.services.adapter = Box::new(TutorRequestRecorder { samples:samples.clone(),directory:None,
        inner:Box::new(tc4_delivery(target.clone(),previous)) });
    let answer = post_ok(&mut state,"/agent/chat",input.clone());
    assert_eq!(answer["incomplete"],false,"{answer}");
    let accepted = state.user.agent_history.sessions[0].turns[1].turn_id.clone();
    assert_eq!(post_ok(&mut state,"/agent/chat",input.clone())["turn_id"],accepted);
    assert_eq!(samples.lock().unwrap().len(),4,"four author samplings; no teacher-content reviewer");
    let chosen = state.user.learning_store().unwrap().teaching_event(&format!("chosen:{accepted}")).unwrap();
    assert_eq!(state.user.learning_store().unwrap().teaching_deliveries(state.workspace.selected_chat.as_deref().unwrap(),&accepted).unwrap().len(),1);
    println!("TC4_EVIDENCE {}",json!({"host":"local","action":"choice","request":input,"accepted_turn":accepted,
        "chosen_event":chosen,"execution_starts":1,"provider_calls":4,"terminal":"completed","incomplete":false}));

    let (f,w,input,_) = tc4_network_offer();
    let handle=f.access.users.lock().unwrap().get_for_recovery("A","1000").unwrap();
    let prior=handle.lock().unwrap().learning_store().unwrap().teaching_event(input["teaching_choice"]["delivery_ref"].as_str().unwrap()).unwrap();
    let previous=prior.payload["move"]["design_ref"].as_str().map(str::to_owned);
    let target=prior.payload["move"]["target"].clone();
    let samples = Arc::new(std::sync::Mutex::new(Vec::new())); let collected = samples.clone();
    f.access.runs.configure_fake(move || Box::new(TutorRequestRecorder { samples:collected.clone(),directory:None,
        inner:Box::new(tc4_delivery(target.clone(),previous.clone())) }));
    let (status,accepted) = admit(&f,&w,input.clone()); assert_eq!(status,202,"{accepted}");
    drive_network_coach(&f);
    assert!(f.access.runs.run_one(&f.access).unwrap());
    assert!(!f.access.runs.run_one(&f.access).unwrap());
    assert_eq!(admit(&f,&w,input.clone()).1["turn_id"],accepted["turn_id"]);
    assert_eq!(samples.lock().unwrap().len(),4,"four author samplings; no teacher-content reviewer");
    let handle = f.access.users.lock().unwrap().get_for_recovery("A","1000").unwrap(); let user = handle.lock().unwrap();
    let turn = &user.agent_history.sessions[0].turns[1];
    assert_eq!(turn.status,AgentAssistantStatus::Completed,"{:?}",turn.error);
    assert!(!turn.outcome.as_ref().unwrap().incomplete);
    let chosen = user.learning_store().unwrap().teaching_event(&format!("chosen:{}",turn.turn_id)).unwrap();
    assert_eq!(user.learning_store().unwrap().teaching_deliveries(w["selected_chat"].as_str().unwrap(),&turn.turn_id).unwrap().len(),1);
    println!("TC4_EVIDENCE {}",json!({"host":"network","action":"choice","request":input,"accepted_turn":turn.turn_id,
        "chosen_event":chosen,"execution_starts":1,"provider_calls":4,"terminal":"completed","incomplete":false}));
}

#[test]
fn continuation_tc4_network_restart_keeps_choice_snapshot_and_authorization() {
    let (f,w,input,probe) = tc4_network_offer();
    *f.access.runs.fail_after.lock().unwrap() = Some("pending");
    assert_eq!(admit(&f,&w,input.clone()).0,503);
    let mut changed = input.clone(); changed["teaching_choice"]["option_id"] = json!("example");
    assert_eq!(admit(&f,&w,changed).1["error_code"], "REQUEST_KEY_CONFLICT");
    let root = f.root.path().to_owned();
    let super::mu5_tests::Fixture { root:retained, access, control, a:token, .. } = f;
    drop(access); drop(control);
    let access = Arc::new(authorization::Authorization::new(user_registry::UserRegistry::open(&root).unwrap()).unwrap());
    let adapter = probe.clone(); access.runs.configure_fake(move || Box::new(adapter.clone()));
    access.runs.recover(&access).unwrap();
    let principal = access.auth.authenticate(&token,multi_user_host::now()).unwrap();
    let context = access.context(principal,multi_user_host::now()).unwrap();
    let accepted = access.runs.admit(&access,&context,w["workspace_id"].as_str().unwrap(),&input,"1000").unwrap();
    let user = context.user.lock().unwrap();
    let turn = &user.agent_history.sessions[0].turns[1];
    assert_eq!(turn.turn_id,accepted["turn_id"]);
    assert_eq!(turn.domain.interaction.as_ref().unwrap().accepted_request,"Compare two equal-distance journeys");
    let event = user.learning_store().unwrap().teaching_event(&format!("chosen:{}",turn.turn_id)).unwrap();
    assert_eq!(event.payload["option_id"],"compare"); drop(user);
    let reference = serde_json::from_value(w["published_book_ref"].clone()).unwrap();
    access.library.lock().unwrap().revoke("A",&reference).unwrap();
    assert!(access.runs.admit(&access,&context,w["workspace_id"].as_str().unwrap(),&input,"1000").is_err());
    // Revocation closes the queued request before model execution.
    access.runs.run_one(&access).unwrap();
    assert_eq!(probe.0.load(Ordering::SeqCst),0);
    let user = context.user.lock().unwrap();
    assert_eq!(user.agent_history.sessions[0].turns[1].error.as_ref().unwrap().error_code,"RUN_PERMISSION_REVOKED");
    drop(user); drop(context); drop(access); drop(retained);
}

#[test]
fn continuation_tc2_network_action_recovers_each_preparation_boundary() {
    for point in ["pending","teaching_receipt","prepared","queued"] {
        let (f,w,mut input,probe)=network_fixture();
        let context=f.access.users.lock().unwrap().get_for_recovery("A","1000").unwrap();
        let reference: published_library::PublishedBookRef=serde_json::from_value(w["published_book_ref"].clone()).unwrap();
        let publication=f.access.library.lock().unwrap().load("A",&reference).unwrap();
        input["message"]=json!(""); input["entry_action"]=json!("explain_selection");
        input["source_id"]=json!(publication.book.base.book_id);
        input["source_revision"]=json!(publication.book.source_fingerprint()); input["expected_chat_head"]=Value::Null;
        *f.access.runs.fail_after.lock().unwrap()=Some(point);
        assert_eq!(admit(&f,&w,input.clone()).0,503,"{point}");
        *f.access.runs.fail_after.lock().unwrap()=None;
        f.access.runs.recover(&f.access).unwrap();
        let (status,accepted)=admit(&f,&w,input.clone()); assert_eq!(status,202,"{point}: {accepted}");
        assert!(f.access.runs.run_one(&f.access).unwrap());
        assert!(!f.access.runs.run_one(&f.access).unwrap());
        assert_eq!(admit(&f,&w,input).1["turn_id"],accepted["turn_id"]);
        assert_eq!(probe.0.load(Ordering::SeqCst),1);
        let user=context.lock().unwrap();
        assert!(user.agent_history.sessions[0].turns[0].domain.interaction.is_some());
    }
}

#[test]
fn continuation_tc2_network_continue_uses_original_answer_and_same_key() {
    let (f,w,input,probe)=network_fixture();
    let (status,first)=admit(&f,&w,input); assert_eq!(status,202);
    assert!(f.access.runs.run_one(&f.access).unwrap());
    let context=f.access.users.lock().unwrap().get_for_recovery("A","1000").unwrap();
    context.lock().unwrap().learning_store().unwrap().mutate(&serde_json::from_value(json!({
        "operation_id":"enable","expected_revision":0,"action":{"kind":"set_enabled","enabled":true}})).unwrap(),"1001").unwrap();
    let mut next=super::mu5_tests::Fixture::stamp(&w,"tc0-page");
    next["session_id"]=w["selected_chat"].clone(); next["client_request_id"]=json!("next");
    next["message"]=json!("");next["entry_action"]=json!("continue_answer");
    next["answer_ref"]=json!({"turn_id":first["turn_id"]});next["expected_chat_head"]=first["turn_id"].clone();
    let (status,second)=admit(&f,&w,next.clone());assert_eq!(status,202,"{second}");
    assert_eq!(admit(&f,&w,next.clone()).1["turn_id"],second["turn_id"]);
    next["answer_ref"]["excerpt"]=scenario()["answer_excerpt"].clone();
    assert_eq!(admit(&f,&w,next).1["error_code"],"REQUEST_KEY_CONFLICT");
    assert_eq!(probe.0.load(Ordering::SeqCst),1); // admission itself never samples
    let user=context.lock().unwrap();
    let second=&user.agent_history.sessions[0].turns[1];
    let action=second.domain.interaction.as_ref().unwrap();
    assert_eq!(action.answer_ref.as_ref().unwrap().turn_id,first["turn_id"].as_str().unwrap());
    assert!(second.teaching_ref.is_none());
    assert!(user.learning_store().unwrap().state().unwrap().sessions.is_empty());
}

#[test]
fn continuation_tc2_network_question_help_returns_location_without_run() {
    let (_root,mut local)=fixture(true);
    let (_,delivery)=activity(&mut local,false);
    let local_session=local.user.learning_store().unwrap().teaching_event(&delivery).unwrap().binding.tutor_session_id;
    let rows=local.user.learning_store().unwrap().teaching_events(&local_session,0,100).unwrap();
    let (f,w,mut request,probe)=network_fixture();
    let context=f.access.users.lock().unwrap().get_for_recovery("A","1000").unwrap();
    let user=context.lock().unwrap();
    let mut store=user.learning_store().unwrap();
    store.mutate(&serde_json::from_value(json!({"operation_id":"enabled","expected_revision":0,
        "action":{"kind":"set_enabled","enabled":true}})).unwrap(),"1001").unwrap();
    let view=store.mutate(&serde_json::from_value(json!({"operation_id":"session","expected_revision":1,
        "action":{"kind":"start","user_intent":"speed","explicit_constraints":[],
            "material_scope":[{"source_id":scenario()["book_id"],"scope_refs":[],"role":"primary"}],"default_teaching_intent":null}})).unwrap(),"1001").unwrap();
    for (_,mut event) in rows {
        event.binding.tutor_session_id=view.control.current_tutor_session_id.clone().unwrap();
        event.binding.chat_session_id=w["selected_chat"].as_str().unwrap().into();
        store.append_teaching(&event).unwrap();
    }
    drop(store);drop(user);
    let reference:published_library::PublishedBookRef=serde_json::from_value(w["published_book_ref"].clone()).unwrap();
    let publication=f.access.library.lock().unwrap().load("A",&reference).unwrap();
    request["message"]=json!("");request["entry_action"]=json!("explain_selection");
    request["source_id"]=json!(publication.book.base.book_id);request["source_revision"]=json!(publication.book.source_fingerprint());
    request["expected_chat_head"]=Value::Null;
    let (status,result)=admit(&f,&w,request);
    assert_eq!(status,200,"{result}");assert_eq!(result["delivery_refs"],json!([delivery]));
    assert_eq!(result["activity"]["session_id"],w["selected_chat"]);
    assert!(!f.access.runs.run_one(&f.access).unwrap());
    assert_eq!(probe.0.load(Ordering::SeqCst),0);
    assert!(context.lock().unwrap().agent_history.sessions[0].turns.is_empty());
}

fn network_fixture() -> (super::mu5_tests::Fixture, Value, Value, Probe) {
    let f = super::mu5_tests::Fixture::new();
    let (root, _state) = fixture(true);
    let reference = f.access.library.lock().unwrap().publish(root.path()).unwrap().reference;
    f.access.library.lock().unwrap().grant("A", &reference).unwrap();
    let probe = Probe::default();
    let adapter = probe.clone();
    f.access.runs.configure_fake(move || Box::new(adapter.clone()));
    let w = f.create(&f.a, &reference, "tc0-page");
    let w = f.action(&f.a, &w, "tc0-page", "chat/new", json!({}));
    let mut input = super::mu5_tests::Fixture::stamp(&w, "tc0-page");
    input["session_id"] = w["selected_chat"].clone();
    input["client_request_id"] = json!("tc0-request");
    input["message"] = json!("Explain average speed");
    input["question_quote"] = quote();
    (f, w, input, probe)
}
fn admit(f: &super::mu5_tests::Fixture, w: &Value, input: Value) -> (u16, Value) {
    f.call(&f.a, "POST", &format!("/api/workspaces/{}/agent/runs", w["workspace_id"].as_str().unwrap()), input)
}

// These in-process fixtures do not spawn the host's background coach worker.
fn drive_network_coach(f: &super::mu5_tests::Fixture) {
    let handle=f.access.users.lock().unwrap().get_for_recovery("A","1000").unwrap();
    while crate::teaching_coach::tick_network(&f.access,&handle,&runtime::run_context::CancellationToken::default()).unwrap() {}
}

#[test]
fn continuation_baseline_network_lost_acceptance_recovers_one_execution() {
    let (f, w, input, probe) = network_fixture();
    *f.access.runs.fail_after.lock().unwrap() = Some("queued");
    assert_eq!(admit(&f, &w, input.clone()).0, 503);
    *f.access.runs.fail_after.lock().unwrap() = None;
    f.access.runs.recover(&f.access).unwrap();
    let (status, first) = admit(&f, &w, input.clone());
    assert_eq!(status, 202, "{first}");
    assert_eq!(admit(&f, &w, input.clone()).1["turn_id"], first["turn_id"]);
    assert!(f.access.runs.run_one(&f.access).unwrap());
    assert!(!f.access.runs.run_one(&f.access).unwrap());
    assert_eq!(admit(&f, &w, input).1["turn_id"], first["turn_id"]);
    assert_eq!(probe.0.load(Ordering::SeqCst), 1);
}

#[test]
fn continuation_contract_unknown_choice_is_rejected_before_admission() {
    let (f, w, mut input, _) = network_fixture();
    input.as_object_mut().unwrap().remove("question_quote");
    input["expected_chat_head"] = json!("old-turn");
    input["teaching_choice"] = json!({"delivery_ref":"tc0-delivery","option_id":scenario()["continuation"]["options"][0]["option_id"]});
    let first = run_input::normalize("workspace", &input).unwrap();
    input["teaching_choice"]["option_id"] = scenario()["continuation"]["options"][1]["option_id"].clone();
    assert_ne!(first, run_input::normalize("workspace", &input).unwrap());
    // An unknown delivery is not a legitimate choice fixture. Real consumption
    // and same-key conflict coverage starts with delivered TC3 suggestions.
    let (_root, mut state) = fixture(true);
    let error = prepare_agent_chat(&mut state, &input.to_string(), "1001").err().unwrap();
    assert_eq!(serde_json::from_str::<Value>(&error.body).unwrap()["error_code"], "INTERACTION_INVALID");
    input["expected_chat_head"] = Value::Null;
    let (status, response) = admit(&f, &w, input);
    assert_eq!(status, 400, "{response}");
    assert_eq!(response["error_code"], "INTERACTION_INVALID");
}