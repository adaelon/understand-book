use super::*;
use crate::tests::{teaching_design_tests as design, tutor_loop_tests as tutor};
use memory::teaching::{TeachingEvent, TeachingFact};

#[path = "interactive_teaching_om6_tests.rs"]
mod om6;

fn teaching_fixture() -> (tempfile::TempDir, AppState, AgentTurnRef, PresentationRef, TeachingEvent) {
    let (root, mut state) = design::fixture();
    let (history, store) = crate::session_store::load_chat_storage(&state.user.history_path).unwrap();
    state.user.agent_history = history; state.user.session_store = store;
    assert_eq!(route_agent_new(&mut state, "om5-start").status, 200);
    let first = prepare_agent_chat(&mut state, r#"{"message":"用同一页面演示速度与距离的关系。"}"#, "first").unwrap();
    let mut plan = design::design_request("om5", None);
    plan["design"]["representation"] = json!({"kind":"presentation","reason":"同一时刻对照距离","observation":"同一时间速度翻倍，距离翻倍"});
    plan["design"]["blocks"] = json!([{"block_id":"distance-time","relation":"距离等于速度乘时间",
        "purpose":"固定共同时间比较两车距离","source_bindings":[{"start_lid":"1.1","end_lid":"1.1"}]}]);
    plan["design"]["focus_block_refs"] = json!(["distance-time"]);
    let accepted = teaching::step(&state.private_context(), &first.turn_ref, plan, &[], &[design::range()]).unwrap();
    let metadata: Value = serde_json::from_str(include_str!("../../../../skills/presentation/methods/compare-one-factor/v3/example/author.json")).unwrap();
    let mut page = content(&state, "快一倍，会远一倍吗？");
    page.source_bindings.clear();
    page.readable_content = metadata["readable_content"].as_str().unwrap().into();
    page.assumptions = serde_json::from_value(metadata["assumptions"].clone()).unwrap();
    page.initial_state = metadata["initial_state"].clone();
    page.state_contract = metadata["state_contract"].clone();
    page.action_contract = serde_json::from_value(metadata["action_contract"].clone()).unwrap();
    let mut binding = metadata["teaching_binding"].clone();
    binding["design_ref"] = accepted["design_ref"].clone();
    page.teaching_binding = Some(serde_json::from_value(binding).unwrap());
    page.content_files.insert("index.html".into(), include_str!("../../../../skills/presentation/methods/compare-one-factor/v3/example/index.html").into());
    let candidate = state.private_context().create_presentation_candidate(&first.turn_ref.session_id, &first.turn_ref.turn_id, None, page).unwrap();
    let reference = persist(&mut state, &first.turn_ref, &candidate);
    let mut movement = design::select_request("compare-speed", accepted["design_ref"].as_str().unwrap(), false);
    movement["move"]["block_refs"] = json!(["distance-time"]);
    movement["move"]["prompt"] = json!("共同时间保持 10 秒，先看 A、B 各走 10 米，再改变 B。");
    movement["move"]["delivery_text"] = movement["move"]["prompt"].clone();
    movement["move"]["presentation"] = json!(reference);
    let selected = teaching::step(&state.private_context(), &first.turn_ref, movement, &[], &[design::range()]).unwrap();
    let mut output = tutor::outcome(Some(&reference));
    output.teaching_delivery_move_refs = serde_json::from_value(selected["delivery_move_refs"].clone()).unwrap();
    tutor::finish(&mut state, &first.turn_ref, &output);
    let delivery = state.user.learning_store().unwrap().teaching_deliveries(&first.turn_ref.session_id, &first.turn_ref.turn_id).unwrap().remove(0);
    (root, state, first.turn_ref, reference, delivery)
}

fn scene(input: u32, applied: u32, revealed: bool) -> Value {
    json!({"values":{"page":{"input_speed":input,"applied_speed":applied,"time_s":10,"revealed":revealed,"annotated":false}},
        "visible_step":if input==applied {"applied"} else {"awaiting_apply"},
        "observed_result":format!("A 走 10 米，B 走 {} 米。{}",applied*10,if revealed {"同一时间速度翻倍，距离翻倍。"} else {""}),
        "source_ref_ids":[],"observation":{"summary":"共同时间 10 秒","state":{"speed_b_m_s":input},
            "rendered":{"speed_b_m_s":applied,"distance_a_m":10,"distance_b_m":applied*10}}})
}

fn saved_command(
    state: &Arc<Mutex<AppState>>, stream: &RunStream, turn: &AgentTurnRef,
    request: InteractRequest, snapshot: Value, support: Value, complete: bool,
) -> (Value, CancellationToken, std::thread::JoinHandle<Result<Value, ToolError>>) {
    let status = if matches!(&request, InteractRequest::Observe {..}) {"observed"} else {"applied"};
    let token = CancellationToken::default();
    let task = run_command(state.clone(), turn.clone(), request, token.clone());
    let command = wait_command(stream);
    let claim = json!({"operation":"claim","command_id":command.command_id,"run_turn_id":turn.turn_id,"page_id":"om5-page"});
    assert_eq!(state.with_app(|s|post(s,"/agent/presentation.interaction",&claim.to_string())).status,200);
    let mut save = json!({"session_id":command.session_id,"turn_id":command.delivery_turn_id,"reference":command.request.reference(),"state":snapshot,
        "teacher_interaction":{"command_id":command.command_id,"run_turn_id":command.run_turn_id,"page_id":"om5-page","status":status,"support":support}});
    // Reader passes its answer reference, which also carries the presentation kind.
    save["reference"]["kind"] = json!("presentation");
    let saved = state.with_app(|s|post(s,"/agent/presentation.state.save",&save.to_string()));
    assert_eq!(saved.status,200,"{}",saved.body);
    let receipt:Value=serde_json::from_str(&saved.body).unwrap();
    // Repeated transport receipt does not create a second state or help.
    assert_eq!(state.with_app(|s|post(s,"/agent/presentation.state.save",&save.to_string())).body,saved.body);
    if complete {
        let reply = state.with_app(|s|post(s,"/agent/presentation.interaction",&json!({"operation":"complete",
            "command_id":command.command_id,"run_turn_id":turn.turn_id,"page_id":"om5-page","status":status,"receipt":receipt}).to_string()));
        assert_eq!(reply.status,200,"{}",reply.body);
    }
    (receipt,token,task)
}

#[test]
fn om5_teacher_help_is_ordered_frozen_and_survives_cancel_and_reopen() {
    let (root, mut state, delivered, reference, delivery) = teaching_fixture();
    let run=prepare_agent_chat(&mut state,r#"{"message":"为什么没有变，请演示。"}"#,"demo").unwrap();
    let turn=run.turn_ref;
    let stream=start_stream(&mut state,&turn);
    let state=Arc::new(Mutex::new(state));
    let (_,_,task)=saved_command(&state,&stream,&turn,InteractRequest::Observe {reference:reference.clone()},scene(1,1,false),json!([]),true);
    let mut current=task.join().unwrap().unwrap();
    let mut act=|action:PresentationAction,snapshot:Value,support:Value| {
        let (_,_,task)=saved_command(&state,&stream,&turn,InteractRequest::Act {reference:reference.clone(),
            based_on:current["saved_state_ref"].as_str().unwrap().into(),action},snapshot,support,true);
        current=task.join().unwrap().unwrap();current.clone()
    };
    let pending=act(PresentationAction::SetParameters {values:BTreeMap::from([("input_speed".into(),json!(2))])},scene(2,1,false),json!([]));
    assert_eq!(pending["teaching"]["assistance_refs"],json!([]));
    let applied=act(PresentationAction::Invoke {action_id:"apply".into()},scene(2,2,false),json!([{"region_id":"result","text":"A 走 10 米，B 走 20 米。"}]));
    let revealed=act(PresentationAction::Reveal {region_id:"relation".into()},scene(2,2,true),json!([{"region_id":"relation","text":"同一时间速度翻倍，距离翻倍。"}]));
    let expected=json!([applied["teaching"]["assistance_refs"][0],revealed["teaching"]["assistance_refs"][0]]);
    state.with_app(|s|tutor::finish(s,&turn,&tutor::outcome(None)));
    let words="速度翻倍，所以同一时间距离翻倍。";
    let reply=state.with_app(|s|prepare_agent_chat(s,&json!({"message":words,"presentation_follow_up":revealed["receipt"]}).to_string(),"response").unwrap());
    let fact_ref=format!("usage:{}:user:{}",delivery.binding.tutor_session_id,reply.turn_ref.turn_id);
    state.with_app(|s| {
        let fact=s.user.learning_store().unwrap().teaching_event(&fact_ref).unwrap();
        assert_eq!(fact.payload["assistance_refs"],expected);
        assert_eq!(fact.learner_expression(),Some(words));
        assert!(!s.user.learning_store().unwrap().teaching_activity_events(&delivery.event_id).unwrap().iter().any(|e|e.kind==TeachingFact::LearnerAction));
        tutor::finish(s,&reply.turn_ref,&tutor::outcome(None));
    });
    // A new teacher result has been seen/saved, but its final tool reply is held.
    let later=state.with_app(|s|prepare_agent_chat(s,r#"{"message":"再演示一次。"}"#,"later").unwrap().turn_ref);
    let later_stream=state.with_app(|s|start_stream(s,&later));
    let (_,_,task)=saved_command(&state,&later_stream,&later,InteractRequest::Observe{reference:reference.clone()},scene(3,2,true),json!([]),true);
    let observed=task.join().unwrap().unwrap();
    let (last,token,task)=saved_command(&state,&later_stream,&later,InteractRequest::Act{reference:reference.clone(),based_on:observed["saved_state_ref"].as_str().unwrap().into(),
        action:PresentationAction::Invoke{action_id:"apply".into()}},scene(3,3,true),json!([{"region_id":"result","text":"A 走 10 米，B 走 30 米。"}]),false);
    token.cancel();later_stream.cancelling();
    assert_eq!(task.join().unwrap().unwrap_err().error_code,"AGENT_RUN_CANCELLED");
    let book=state.with_app(|s| {
        let store=s.user.learning_store().unwrap();
        assert_eq!(store.teaching_event(&fact_ref).unwrap().payload["assistance_refs"],expected);
        assert_eq!(store.teaching_activity_events(&delivery.event_id).unwrap().iter().filter(|e|e.kind==TeachingFact::HelpDisplayed).count(),3);
        drop(store);
        tutor::finish(s,&later,&tutor::outcome(None));s.workspace.book.clone()
    });
    drop(state);
    let mut reopened=state_named("om5-reopened");
    reopened.workspace.book=book;
    reopened.workspace.reader=Reader::new(&reopened.workspace.book,DEFAULT_RADIUS);
    reopened.workspace.book_dir=root.path().to_path_buf();
    reopened.user.store=MemoryStore::open(root.path().join("memory.json")).unwrap();
    reopened.user.history_path=Some(root.path().join("history.json"));
    let (history,store)=crate::session_store::load_chat_storage(&reopened.user.history_path).unwrap();
    reopened.user.agent_history=history;reopened.user.session_store=store;
    assert_eq!(route_agent_history_select(&mut reopened,&json!({"session_id":delivered.session_id}).to_string()).status,200);
    let next=prepare_agent_chat(&mut reopened,&json!({"message":"这次直接讲公式。","presentation_follow_up":last}).to_string(),"next").unwrap();
    let learner=&next.tutor.as_ref().unwrap()["learner_context"];
    if let Ok(output)=std::env::var("OM5_OUTPUT") {
        std::fs::write(Path::new(&output).join("reopened-context.json"),serde_json::to_vec_pretty(learner).unwrap()).unwrap();
    }
    let work=learner["learning_opportunity"]["continuity"]["previous_work"].as_array().unwrap().iter()
        .find(|row|row["fact_refs"]==json!([fact_ref])).unwrap_or_else(||panic!("missing work: {}",learner["learning_opportunity"]["continuity"]));
    assert_eq!(work["assistance_refs"],expected);
    assert_eq!(work["assessment_status"],"unassessed");
    assert!(work["help"].as_array().unwrap().iter().any(|h|h["actor"]=="teacher"));
    let trace=teaching::step(&reopened.private_context(),&next.turn_ref,json!({"operation":"trace","event_id":expected[0]}),&[],&[]).unwrap();
    assert!(trace.to_string().contains("20"));
    if let Ok(output)=std::env::var("OM5_OUTPUT") {
        std::fs::write(Path::new(&output).join("reopened-context.json"),serde_json::to_vec_pretty(learner).unwrap()).unwrap();
    }
}

#[test]
fn om5_method_previews_apply_and_reveal_at_three_sizes() {
    let (_root,mut state,delivered,reference,_)=teaching_fixture();
    let content=state.private_context().read_presentation(&delivered.session_id,&reference).unwrap().content;
    let turn=prepare_agent_chat(&mut state,r#"{"message":"继续演示。"}"#,"preview").unwrap().turn_ref;
    let app=crate::agent_run::BorrowedAppPort(std::cell::RefCell::new(&mut state));
    let scope=app.with_app(|s|crate::run_scope::RunScope::capture(s,&turn,"preview",None,None));
    let mut port=RuntimeStatePort {port:&app,scope:&scope,turn_ref:&turn,previewed:Default::default(),animations:Default::default(),plots:Default::default()};
    let write=json!({"operation":"write","new_object":true,"title":content.title,"html":content.content_files["index.html"],
        "readable_content":content.readable_content,"state_contract":content.state_contract,"initial_state":content.initial_state,
        "action_contract":content.action_contract,"teaching_binding":content.teaching_binding});
    let written=port.author_presentation(serde_json::from_value(write).unwrap(),&[],&[],&CancellationToken::default()).unwrap();
    let mut reports=vec![];
    for (_,viewport) in runtime::presentation_preview::REQUIRED_PREVIEW_ENVIRONMENTS {
        let request=json!({"operation":"preview","candidate_id":written.body["candidate_id"],"viewport":viewport,"read_selector":"#reading","actions":[
            {"kind":"interact","action":{"kind":"set_parameters","values":{"input_speed":2}}},
            {"kind":"interact","action":{"kind":"invoke","action_id":"apply"}},
            {"kind":"interact","action":{"kind":"reveal","region_id":"relation"}}]});
        let result=port.author_presentation(serde_json::from_value(request).unwrap(),&[],&[],&CancellationToken::default()).unwrap();
        assert!(result.body["error_code"].is_null(),"{}",result.body);
        assert!(result.body["reading"]["text"].as_str().unwrap().contains("B 走 20 米"));
        reports.push(result.body);
    }
    if let Ok(output)=std::env::var("OM5_OUTPUT") {
        std::fs::write(Path::new(&output).join("author-preview.json"),serde_json::to_vec_pretty(&reports).unwrap()).unwrap();
    }
}

fn scripted_reply(state:&mut AppState,body:&str,delivery:&TeachingEvent)->Value {
    let prepared=prepare_agent_chat(state,body,"learner-reply").unwrap_or_else(|e|panic!("{}",e.body));
    let context=prepared.tutor.clone().unwrap();
    let message:Value=serde_json::from_str(body).unwrap();
    let text=if message["message"].as_str().unwrap().contains("公式") {
        "你在演示后说过“速度翻倍，所以同一时间距离翻倍”，后来又自己改为 3 米/秒并应用。公式是 d = v × t；同一时间下 dB / dA = vB / vA。此前看过的演示仍是这些表达的帮助条件。"
    } else {
        "这句话与当前画面一致。保留此前演示的帮助条件，你可以继续改变速度或共同时间。"
    };
    let mut selection=design::select_request("continue-comparison",delivery.payload["move"]["design_ref"].as_str().unwrap(),false);
    selection["move"]["block_refs"]=json!(["distance-time"]);
    selection["move"]["prompt"]=json!(text);selection["move"]["delivery_text"]=json!(text);
    selection["move"]["responds_to"]=json!({"fact_refs":[format!("usage:{}:user:{}",delivery.binding.tutor_session_id,prepared.turn_ref.turn_id)],"disposition":"addressed"});
    if message["message"].as_str().unwrap().contains("公式") {
        selection["move"]["kind"]=json!("explain");
        selection["move"]["interaction_intent"]=json!("direct_explanation");
        selection["move"]["intent_origin"]=json!("current_request");
        selection["move"]["intent_quote"]=message["message"].clone();
    }
    let call=|id:&str,name:&str,args:Value|AssistantTurn {provider_continuation:None,text:None,tool_calls:vec![ToolCall{id:id.into(),name:name.into(),arguments:args.to_string()}],usage_total_tokens:Some(5)};
    let samples=Arc::new(Mutex::new(vec![]));
    let adapter=crate::tests::tutor_continuation_tests::TutorRequestRecorder {directory:None,samples:samples.clone(),inner:Box::new(ChatStubAdapter::scripted(vec![
        call("source","book.text",json!({"lid":"1.1","end_lid":"1.1"})),
        call("support","tutor.step",selection.clone()),
        AssistantTurn{provider_continuation:None,text:Some(text.into()),tool_calls:vec![],usage_total_tokens:Some(5)},
    ]))};
    let result=agent_run::execute_prepared(&crate::agent_run::BorrowedAppPort(std::cell::RefCell::new(state)),&adapter,prepared,CancellationToken::default());
    assert_eq!(result.reply.status,200,"{}",result.reply.body);
    json!({"context":context,"selection":selection,"samples":*samples.lock().unwrap(),"outcome":serde_json::from_str::<Value>(&result.reply.body).unwrap()})
}

#[test]
#[ignore = "OM5 real Reader host; start explicitly with OM5_READER_HOST"]
fn om5_browser_host() {
    let (root,mut initial,delivered,reference,delivery)=teaching_fixture();
    let mut turn=Some(prepare_agent_chat(&mut initial,r#"{"message":"为什么没有变，请在同页演示。"}"#,"demo").unwrap().turn_ref);
    let mut stream=start_stream(&mut initial,turn.as_ref().unwrap());
    let state=Arc::new(Mutex::new(initial));
    let mut token=CancellationToken::default();
    let result=Arc::new(Mutex::new(Value::Null));
    let mut replies=Vec::<Value>::new();
    let server=tiny_http::Server::http("127.0.0.1:4192").unwrap();
    println!("OM5_BROWSER_READY");
    for mut request in server.incoming_requests() {
        let path=request.url().to_owned();
        if path.contains("/events") {
            let after=path.split("?after=").nth(1).and_then(|s|s.parse().ok());
            crate::agent_stream::serve(request,stream.clone(),after);continue;
        }
        let mut body=String::new();request.as_reader().read_to_string(&mut body).unwrap();
        let reply=if path=="/fixture" {
            ok_json(&json!({"session_id":delivered.session_id,"turn_id":delivered.turn_id,"reference":reference,
                "outcome":outcome(&reference),"run_descriptor":turn.as_ref().map(|_|stream.snapshot().descriptor),"teaching_ref":delivery.event_id}))
        } else if path=="/om5/command" {
            *result.lock().unwrap()=Value::Null;
            let task=run_command(state.clone(),turn.clone().unwrap(),serde_json::from_str(&body).unwrap(),token.clone());
            let result=result.clone();
            std::thread::spawn(move||*result.lock().unwrap()=match task.join().unwrap(){Ok(v)=>v,Err(e)=>json!(e)});
            ok_json(&json!({"started":true}))
        } else if path=="/om5/result" {ok_json(&*result.lock().unwrap())
        } else if path=="/om5/evidence" {
            let events=state.with_app(|s|s.user.learning_store().unwrap().teaching_events(&delivery.binding.tutor_session_id,0,100).unwrap());
            ok_json(&json!({"events":events,"replies":replies}))
        } else if path=="/agent/chat" {
            if let Some(active)=turn.take() {
                state.with_app(|s|tutor::finish(s,&active,&tutor::outcome(None)));stream.finish(None,None);
            }
            let reply=state.with_app(|s|scripted_reply(s,&body,&delivery));
            let outcome=reply["outcome"].clone();replies.push(reply);ok_json(&outcome)
        } else if path=="/om5/start" {
            turn=Some(state.with_app(|s|prepare_agent_chat(s,r#"{"message":"再演示一次。"}"#,"demo-again").unwrap().turn_ref));
            stream=state.with_app(|s|start_stream(s,turn.as_ref().unwrap()));token=CancellationToken::default();
            ok_json(&json!({"started":true}))
        } else if path.ends_with("/cancel") {
            token.cancel();stream.cancelling();ok_json(&json!(stream.snapshot()))
        } else if path=="/om5/reopen" {
            if let Some(active)=turn.take() {state.with_app(|s|tutor::finish(s,&active,&tutor::outcome(None)));}
            state.with_app(|s|{
                let mut reopened=state_named("om5-browser-reopened");
                reopened.workspace.book=s.workspace.book.clone();
                reopened.workspace.reader=Reader::new(&reopened.workspace.book,DEFAULT_RADIUS);
                reopened.workspace.book_dir=root.path().to_path_buf();
                reopened.user.store=MemoryStore::open(root.path().join("memory.json")).unwrap();
                reopened.user.history_path=Some(root.path().join("history.json"));
                let (history,storage)=crate::session_store::load_chat_storage(&reopened.user.history_path).unwrap();
                reopened.user.agent_history=history;reopened.user.session_store=storage;
                assert_eq!(route_agent_history_select(&mut reopened,&json!({"session_id":delivered.session_id}).to_string()).status,200);
                *s=reopened;
            });
            ok_json(&json!({"reopened":true}))
        } else if path.starts_with("/agent/runs/") {ok_json(&json!(stream.snapshot()))
        } else if path=="/stop" {request.respond(tiny_http::Response::from_string("stopped")).unwrap();break;
        } else {state.with_app(|s|route(s,Req{method:request.method().as_str(),url:&path,body:&body,now:"om5-browser"}))};
        request.respond(tiny_http::Response::from_string(reply.body).with_status_code(reply.status)
            .with_header(tiny_http::Header::from_bytes("Content-Type","application/json").unwrap())).unwrap();
    }
}
