//! OM6 bounded model packets and frozen-output replay through the real Tutor loop.
use super::*;

const BORROW_REQUEST: &str = "上次看了你的例子，我说“bar 是 &Foo，foo 仍拥有值”。这次直接逐行解释为什么两个 println! 都能读到值，顺带说明如果把 &foo 改为 foo 会怎样，不用先出题。";
const OPEN_REQUEST: &str = "我之前倾向少停站，但必要休息必须保留。这次请深入分析同一路线缩短总时间的几种合理方案，可以减少非必要停留，也可以在安全条件下提高行驶速度；直接给公式、比较方法和取舍，不要让我先选一个唯一答案。";
const SPEED_REQUEST: &str = "我把 B 的速度从 1 调到 2 米/秒了，为什么图上还是 10 米？先解释现在的画面，再告诉我怎么让它显示新结果。";

fn directory() -> PathBuf {
    std::env::var("OM6_OUTPUT").map(PathBuf::from).unwrap_or_else(|_| {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/performance/interactive-teaching-continuity/om6")
    })
}

fn save(name: &str, value: &Value) {
    let path = directory().join(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

fn reopen(root: &Path, state: AppState) -> AppState {
    let book = state.workspace.book.clone();
    let session = state.user.agent_history.sessions[0].id.clone();
    drop(state);
    let mut state = state_named("om6-reopened");
    state.workspace.book = book;
    state.workspace.reader = Reader::new(&state.workspace.book, DEFAULT_RADIUS);
    state.workspace.book_dir = root.to_path_buf();
    state.user.store = MemoryStore::open(root.join("memory.json")).unwrap();
    state.user.history_path = Some(root.join("history.json"));
    let (history, storage) = crate::session_store::load_chat_storage(&state.user.history_path).unwrap();
    state.user.agent_history = history;
    state.user.session_store = storage;
    assert_eq!(route_agent_history_select(&mut state, &json!({"session_id":session}).to_string()).status, 200);
    state
}

fn chat_storage(state: &mut AppState) {
    let (history, storage) = crate::session_store::load_chat_storage(&state.user.history_path).unwrap();
    state.user.agent_history = history;
    state.user.session_store = storage;
    assert_eq!(route_agent_new(state, "om6-new").status, 200);
}

fn seed_scenario(name: &str) -> (tempfile::TempDir, AppState, Value, Value) {
    if name == "speed" {
        let (root, mut state, delivered, reference, _) = teaching_fixture();
        let receipt = state.private_context().save_presentation_state(
            &delivered.session_id, &delivered.turn_id, &reference,
            serde_json::from_value(scene(2, 1, false)).unwrap(),
        ).unwrap();
        state = reopen(root.path(), state);
        return (root, state, json!({"message":SPEED_REQUEST,"presentation_follow_up":receipt}),
            json!({"reference":reference,"state":scene(2,1,false),"receipt":receipt,
                "available_actions":[{"kind":"invoke","action_id":"apply","label":"应用当前参数"}]}));
    }
    let (root, mut state) = if name == "borrowing" {
        crate::tests::tutor_building_blocks_tests::fixture()
    } else { design::fixture() };
    chat_storage(&mut state);
    let initial_request = if name == "borrowing" {"请用一小段代码示范共享借用，先给我帮助。"}
        else {"平时请简单一些，少推导公式。我倾向于减少停站。"};
    let first = prepare_agent_chat(&mut state, &json!({"message":initial_request}).to_string(), "om6-first").unwrap();
    if name == "open-design" {
        use memory::{Applicability, CreateProfileFact, FactSource, PreferenceClaim, ProfileScope, Sensitivity};
        state.user.store.create_profile_fact(CreateProfileFact {
            scope:ProfileScope::Book {book_id:state.workspace.book.base.book_id.clone()},
            applicability:Applicability::Any,
            payload:ProfilePayload::ExplanationPreference(PreferenceClaim {key:"depth".into(),value:initial_request.into()}),
            source:FactSource::UserStated,
            evidence:vec![memory::EvidenceRef::Turn {session_id:first.turn_ref.session_id.clone(),turn_id:first.turn_ref.turn_id.clone()}],
            confidence:None,sensitivity:Sensitivity::Normal,valid_until:None,
        }, "2026-10-20T08:00:00Z").unwrap();
    }
    let mut plan = design::design_request("om6-prior", None);
    let (relation, help, words) = if name == "borrowing" {
        ("共享借用允许读取值，所有权仍留在 foo。",
         "示例：#[derive(Debug)] struct Foo(u32); let foo = Foo(0); let bar = &foo; println!(\"{:?}\", foo); println!(\"{:?}\", bar); bar 的类型是 &Foo，foo 仍拥有值；两个打印都只读取值。",
         "bar 是 &Foo，foo 仍拥有值。")
    } else {
        ("总时间等于行驶时间加停留时间。",
         "相同路程下，行驶速度和停留时间都会影响总时间；必要休息需要保留。",
         "我倾向减少停站，但必要的休息要保留。")
    };
    if name == "borrowing" {
        plan["design"]["target"]["learning_focus"] = json!("共享借用与所有权");
        plan["design"]["target"]["expected_performance"] = json!("解释 foo 和 bar 的类型、访问与所有权");
        plan["design"]["blocks"][0]["block_id"] = json!("borrowed-access");
        plan["design"]["focus_block_refs"] = json!(["borrowed-access"]);
    }
    plan["design"]["organizing_model"] = json!(relation);
    plan["design"]["blocks"][0]["relation"] = json!(relation);
    plan["design"]["blocks"][0]["purpose"] = json!(relation);
    let accepted = teaching::step(&state.private_context(), &first.turn_ref, plan, &[], &[design::range()]).unwrap();
    let mut support = design::select_request("om6-help", accepted["design_ref"].as_str().unwrap(), false);
    // The accepted design is the authority; the response keeps its exact target.
    let current = state.user.learning_store().unwrap().teaching_event(accepted["design_ref"].as_str().unwrap()).unwrap();
    support["move"]["target"] = current.payload["design"]["target"].clone();
    support["move"]["block_refs"] = current.payload["design"]["focus_block_refs"].clone();
    support["move"]["prompt"] = json!(help);
    support["move"]["delivery_text"] = json!(help);
    let selected = teaching::step(&state.private_context(), &first.turn_ref, support, &[], &[design::range()]).unwrap();
    let mut outcome = tutor::outcome(None);
    outcome.answer = Some(help.into());
    outcome.answer_view.as_mut().unwrap().parts = vec![AgentAnswerPart::Markdown {text:help.into()}];
    outcome.teaching_delivery_move_refs = serde_json::from_value(selected["delivery_move_refs"].clone()).unwrap();
    tutor::finish(&mut state, &first.turn_ref, &outcome);
    let delivered = state.user.learning_store().unwrap().teaching_deliveries(&first.turn_ref.session_id, &first.turn_ref.turn_id).unwrap().remove(0);
    let displayed = TeachingEvent {event_id:format!("om6-seen:{}", first.turn_ref.turn_id),
        binding:delivered.binding.clone(),kind:TeachingFact::HelpDisplayed,causal_refs:vec![delivered.event_id.clone()],
        payload:json!({"delivery_ref":delivered.event_id,"text":help,"actor":"teacher"}),occurred_at:"before-response".into()};
    state.user.learning_store().unwrap().append_teaching(&displayed).unwrap();
    let response = prepare_agent_chat(&mut state, &json!({"message":words}).to_string(), "om6-response").unwrap();
    let response_ref = format!("usage:{}:user:{}", delivered.binding.tutor_session_id, response.turn_ref.turn_id);
    let fact = state.user.learning_store().unwrap().teaching_event(&response_ref).unwrap();
    assert!(fact.payload["assistance_refs"].as_array().unwrap().contains(&json!(displayed.event_id)));
    tutor::finish(&mut state, &response.turn_ref, &tutor::outcome(None));
    state = reopen(root.path(), state);
    let request = if name == "borrowing" {BORROW_REQUEST} else {OPEN_REQUEST};
    (root,state,json!({"message":request}),json!({"original_expression":fact,"displayed_help":displayed}))
}

fn scenario(name: &str) -> (tempfile::TempDir, AppState, agent_run::PreparedAgentChat, Value) {
    let (root,mut state,input,evidence)=seed_scenario(name);
    let run=prepare_agent_chat(&mut state,&input.to_string(),"om6-next").unwrap();
    let packet=packet(&state,&run,evidence);
    let continuity = &packet["context"]["learner_context"]["learning_opportunity"]["continuity"];
    if name!="speed" {
        assert!(continuity.to_string().contains(packet["evidence"]["original_expression"]["payload"]["response"].as_str().unwrap()), "{continuity}");
        assert!(continuity.to_string().contains(packet["evidence"]["displayed_help"]["event_id"].as_str().unwrap()), "{continuity}");
    }
    (root, state, run, packet)
}

fn packet(state: &AppState, run: &agent_run::PreparedAgentChat, evidence: Value) -> Value {
    json!({"source":state.workspace.book.text("1.1",None).unwrap(),"context":run.tutor,
        "request":run.tutor.as_ref().unwrap()["current_request"],"evidence":evidence,
        "tool_contract":runtime::tutor::spec(),"design_example":design::design_request("om6-next",None)})
}

#[test]
fn om6_three_scenarios_export_persisted_input_conditions() {
    for name in ["speed","borrowing","open-design"] {
        let (_root, _state, _run, packet) = scenario(name);
        if std::env::var_os("OM6_OUTPUT").is_some() {
            save(&format!("contexts/{name}.json"), &packet);
        }
    }
}

// Rebind only exact runtime reference values after recreating the same fixture.
// Learner words and model teaching text are never edited.
fn reference_pairs(old: &Value, new: &Value, pairs: &mut BTreeMap<String,String>) {
    match (old,new) {
        (Value::Object(a),Value::Object(b)) => for (key,value) in a {
            if let Some(next) = b.get(key) { reference_pairs(value,next,pairs); }
        },
        (Value::Array(a),Value::Array(b)) => for (a,b) in a.iter().zip(b) { reference_pairs(a,b,pairs); },
        (Value::String(a),Value::String(b)) if a != b && !a.chars().any(char::is_whitespace)
            && ["usage:","design:","delivery:","om6-seen:","turn_","session_","tutor_"].iter().any(|prefix|a.starts_with(prefix)) => {
                pairs.insert(a.clone(),b.clone());
            },
        _ => {}
    }
}
fn rebind(value: &mut Value, pairs: &BTreeMap<String,String>) {
    match value {
        Value::Object(fields) => for value in fields.values_mut() { rebind(value,pairs); },
        Value::Array(values) => for value in values { rebind(value,pairs); },
        Value::String(s) => if let Some(new) = pairs.get(s) { *s = new.clone(); },
        _ => {}
    }
}

fn replay(state:&mut AppState,run:agent_run::PreparedAgentChat,current:&Value,name:&str,variant:&str)->Value {
    let dir=directory().join(variant).join(name);
    let original:Value=serde_json::from_slice(&std::fs::read(dir.join("input.json")).unwrap()).unwrap();
    let mut output:Value=serde_json::from_slice(&std::fs::read(dir.join("output.json")).unwrap()).unwrap();
    let mut pairs=BTreeMap::new();
    reference_pairs(&original["packet"],current,&mut pairs);
    let next:Value=serde_json::from_slice(&std::fs::read(dir.join("stage2-input.json")).unwrap()).unwrap();
    reference_pairs(&next["packet"],current,&mut pairs);
    rebind(&mut output,&pairs);
    let mut plan=output["design"].clone();
    plan["operation"]=json!("design");plan["operation_id"]=json!("om6-model");
    plan["design"]["previous_ref"]=current["context"]["current_design"]["design_ref"].clone();
    let mut selection=output["select"].clone();
    selection["move"]["delivery_text"]=output["answer"].clone();
    let turn=run.turn_ref.clone();
    selection["move"]["design_ref"]=json!(format!("design:{}:om6-model",turn.turn_id));
    let body=output["answer"].as_str().unwrap().to_owned();
    let samples=Arc::new(Mutex::new(vec![]));
    let call=|id:&str,name:&str,arguments:Value|AssistantTurn {provider_continuation:None,text:None,
        tool_calls:vec![ToolCall{id:id.into(),name:name.into(),arguments:arguments.to_string()}],usage_total_tokens:None};
    let adapter=crate::tests::tutor_continuation_tests::TutorRequestRecorder {directory:None,samples:samples.clone(),
        inner:Box::new(ChatStubAdapter::scripted(vec![
            call("read","book.text",json!({"lid":"1.1","end_lid":"1.1"})),
            call("design","tutor.step",plan),call("select","tutor.step",selection),
            AssistantTurn {provider_continuation:None,text:Some(body),tool_calls:vec![],usage_total_tokens:None},
        ]))};
    let result=agent_run::execute_prepared(&crate::agent_run::BorrowedAppPort(std::cell::RefCell::new(&mut *state)),
        &adapter,run,CancellationToken::default());
    let outcome:Value=serde_json::from_str(&result.reply.body).unwrap();
    let store=state.user.learning_store().unwrap();
    let delivery=store.teaching_deliveries(&turn.session_id,&turn.turn_id).unwrap().into_iter().next();
    let selected=delivery.as_ref().map(|d|store.teaching_event(&d.causal_refs[0]).unwrap());
    let samples=samples.lock().unwrap();
    let review=samples.iter().find(|sample|sample["kind"]=="structured_completion")
        .map(|sample|serde_json::from_str::<Value>(sample["input"][0]["content"].as_str().unwrap()).unwrap());
    let report=json!({"prepare":current["context"],"http_status":result.reply.status,"outcome":outcome,
        "delivery":delivery,"selection":selected,"samples":*samples,"review_input":review,
        "replay_model":"frozen real output; deterministic acceptance only; no provider calls"});
    save(&format!("{variant}/{name}/replay.json"),&report);
    if variant=="after" {
        assert_eq!(report["outcome"]["incomplete"],false,"{report}");
        assert_eq!(report["delivery"]["payload"]["answer"],output["answer"]);
    }
    report
}

#[test]
#[ignore = "Accept frozen real model designs and export their actual receipts; no provider"]
fn om6_accept_model_designs() {
    for variant in ["before","after"] {
        for name in ["speed","borrowing","open-design"] {
            if std::env::var("OM6_CASE").ok().is_some_and(|only|only!=name)
                || std::env::var("OM6_VARIANT").ok().is_some_and(|only|only!=variant) {continue;}
            let dir=directory().join(variant).join(name);
            let original:Value=serde_json::from_slice(&std::fs::read(dir.join("input.json")).unwrap()).unwrap();
            let raw:Value=serde_json::from_slice(&std::fs::read(dir.join("design-output.json")).unwrap()).unwrap();
            let mut plan=json!({"operation":raw["operation"],"operation_id":raw["operation_id"],"design":raw["design"]});
            let (_root,state,run,mut current)=scenario(name);
            let mut pairs=BTreeMap::new();
            reference_pairs(&original["packet"],&current,&mut pairs);
            rebind(&mut plan,&pairs);
            plan["design"]["previous_ref"]=current["context"]["current_design"]["design_ref"].clone();
            let receipt=teaching::step(&state.private_context(),&run.turn_ref,plan.clone(),&[],&[design::range()])
                .unwrap_or_else(|error|panic!("{variant}/{name}: {error:?}"));
            current["context"]["learner_context"]=receipt["learner_context_update"].clone();
            let accepted=state.user.learning_store().unwrap().teaching_event(receipt["design_ref"].as_str().unwrap()).unwrap();
            current["context"]["current_design"]=json!({"design_ref":receipt["design_ref"],"design":accepted.payload["design"]});
            save(&format!("{variant}/{name}/design-receipt.json"),&receipt);
            save(&format!("{variant}/{name}/stage2-input.json"),&json!({"packet":current,"accepted_design":plan,"receipt":receipt}));
        }
    }
}

#[test]
#[ignore = "OM6 frozen real model outputs; no provider calls"]
fn om6_replay_real_outputs_through_delivery() {
    for variant in ["before","after"] {
        for name in ["speed","borrowing","open-design"] {
            let (_root,mut state,run,current) = scenario(name);
            let report=replay(&mut state,run,&current,name,variant);
            save(&format!("{variant}/{name}/replay.json"),&report);
        }
    }
}

#[test]
#[ignore = "OM6 local Reader host with frozen real model outputs"]
fn om6_local_browser_host() {
    let mut name="speed".to_owned();
    let (mut root,mut state,mut input,mut evidence)=seed_scenario(&name);
    let mut reports=Vec::new();
    let server=tiny_http::Server::http("127.0.0.1:4194").unwrap();
    println!("OM6_LOCAL_READY");
    for mut request in server.incoming_requests() {
        let path=request.url().to_owned();
        let mut body=String::new();request.as_reader().read_to_string(&mut body).unwrap();
        let reply=if path.starts_with("/om6/case/") {
            name=path.trim_start_matches("/om6/case/").to_owned();
            assert!(["speed","borrowing"].contains(&name.as_str()));
            (root,state,input,evidence)=seed_scenario(&name);
            reports.clear();
            ok_json(&json!({"ready":true}))
        } else if path=="/fixture" {
            let session=&state.user.agent_history.sessions[0];
            let turn=if name=="speed" {&session.turns[0]} else {session.turns.last().unwrap()};
            ok_json(&json!({"session_id":session.id,"turn_id":turn.turn_id,"outcome":turn.outcome,
                "user_request":turn.user_authored_text(),"next_request":input["message"]}))
        } else if path=="/agent/chat" {
            let received:Value=serde_json::from_str(&body).unwrap();
            assert_eq!(received["message"],input["message"]);
            let run=prepare_agent_chat(&mut state,&body,"om6-browser").unwrap();
            let current=packet(&state,&run,evidence.clone());
            let report=replay(&mut state,run,&current,&name,"after");
            let outcome=report["outcome"].clone();reports.push(report);
            ok_json(&outcome)
        } else if path=="/om6/reopen" {
            state=reopen(root.path(),state);
            ok_json(&json!({"reopened":true}))
        } else if path=="/om6/evidence" {
            let session=&state.user.agent_history.sessions[0];
            let store=state.user.learning_store().unwrap();
            let binding=store.teaching_event(&format!("binding:{}",session.turns.last().unwrap().turn_id)).unwrap().binding;
            let value=json!({"reports":reports,"history":session,"events":store.teaching_events(&binding.tutor_session_id,0,100).unwrap()});
            save(&format!("local-{name}-evidence.json"),&value);
            ok_json(&value)
        } else if path=="/stop" {
            request.respond(tiny_http::Response::from_string("stopped")).unwrap();break;
        } else {route(&mut state,Req{method:request.method().as_str(),url:&path,body:&body,now:"om6-browser"})};
        request.respond(tiny_http::Response::from_string(reply.body).with_status_code(reply.status)
            .with_header(tiny_http::Header::from_bytes("Content-Type","application/json").unwrap())).unwrap();
    }
}
