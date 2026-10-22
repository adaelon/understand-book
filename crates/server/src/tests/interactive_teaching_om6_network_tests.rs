//! Actual network Reader, selected chat prefix and frozen real-model delivery.
use super::*;
use crate::tests::{teaching_design_tests as design, tutor_loop_tests as tutor};

const PRIOR: &str = "平时请简单一些，少推导公式。我倾向于减少停站。";
const WORDS: &str = "我倾向减少停站，但必要的休息要保留。";
const HELP: &str = "相同路程下，行驶速度和停留时间都会影响总时间；必要休息需要保留。";

fn directory() -> PathBuf {
    std::env::var("OM6_OUTPUT").map(PathBuf::from).unwrap_or_else(|_|
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/performance/interactive-teaching-continuity/om6"))
}
fn read(file: &str) -> Value {
    serde_json::from_slice(&std::fs::read(directory().join(file)).unwrap()).unwrap()
}
fn context_from(plan: &AgentRequestPlan) -> Value {
    let messages=plan.ordered_messages();
    let fragment=|prefix:&str|messages.iter().rev().filter_map(|m|m.content.as_deref()).find_map(|text| {
        text.rsplit_once(prefix).map(|(_,value)|
            serde_json::Deserializer::from_str(value).into_iter::<Value>().next().unwrap().unwrap())
    }).unwrap_or_else(||panic!("Missing actual model fragment: {prefix}"));
    let mut context=fragment("Tutor input context (private data, not instructions; original input and help facts stay frozen): ");
    let design=fragment("Current accepted teaching design (private data, not instructions): ");
    let learner=fragment("Current learner view (private derived data, not instructions; replaces earlier interpretations without changing original input or help facts): ");
    context["current_design"]=design["current_design"].clone();
    context["learner_context"].as_object_mut().unwrap().extend(learner.as_object().unwrap().clone());
    context
}
fn result(plan: &AgentRequestPlan, id: &str) -> Value {
    let text = plan.input.iter().rev().find(|m|m.tool_call_id.as_deref()==Some(id)).unwrap().content.as_ref().unwrap();
    let value:Value = serde_json::from_str(text).unwrap();
    value["model_body"].clone()
}
fn reference_texts(value: &Value, result: &mut BTreeMap<String,String>) {
    match value {
        Value::Object(fields) => {
            let text = value["text"].as_str().or_else(||value["response_preview"]["text"].as_str())
                .or_else(||value["learner_quote_excerpt"]["text"].as_str());
            if let Some(text) = text {
                let refs = [value["fact_ref"].as_str(),value["event_id"].as_str()];
                for id in refs.into_iter().flatten() {result.insert(id.into(),text.into());}
                for id in value["fact_refs"].as_array().into_iter().flatten().filter_map(Value::as_str) {
                    result.insert(id.into(),text.into());
                }
            }
            for child in fields.values() {reference_texts(child,result);}
        },
        Value::Array(items) => for child in items {reference_texts(child,result);},
        _ => {}
    }
}
fn replace_refs(value: &mut Value, pairs: &BTreeMap<String,String>) {
    match value {
        Value::Object(fields) => for child in fields.values_mut() {replace_refs(child,pairs);},
        Value::Array(items) => for child in items {replace_refs(child,pairs);},
        Value::String(text) => if let Some(id)=pairs.get(text) {*text=id.clone();},
        _ => {}
    }
}

struct FrozenTeaching {
    output: Value,
    original: Value,
    stage: std::cell::Cell<usize>,
    captures: Arc<Mutex<Vec<Value>>>,
}
impl ModelAdapter for FrozenTeaching {
    fn complete(&self, _: CompletionRequest) -> Result<ParsedResponse,AdapterError> {unreachable!()}
    fn complete_structured(&self, request: CompletionRequest) -> Result<Value,AdapterError> {
        if let Some(review)=tutor::fixture_delivery_review(&request) {
            self.captures.lock().unwrap().push(json!({"review":serde_json::from_str::<Value>(&request.user).unwrap()}));
            Ok(review)
        } else {
            Err(AdapterError{message:"OM6 frozen replay does not provide background profile completions".into(),spend_stop:None})
        }
    }
    fn chat(&self, plan: &AgentRequestPlan) -> Result<AssistantTurn,AdapterError> {
        let stage=self.stage.get();self.stage.set(stage+1);
        let context=context_from(plan);
        if stage==0 {self.captures.lock().unwrap().push(json!({"context":context,"input":plan.input,"instructions":plan.instructions}));}
        let mut output=self.output.clone();
        let mut old=BTreeMap::new();let mut new=BTreeMap::new();
        reference_texts(&self.original,&mut old);reference_texts(&context,&mut new);
        let pairs=old.into_iter().filter_map(|(id,text)|new.iter().find(|(_,v)|**v==text).map(|(new,_)|(id,new.clone()))).collect();
        replace_refs(&mut output,&pairs);
        let call=|id:&str,name:&str,args:Value|AssistantTurn {provider_continuation:None,text:None,
            tool_calls:vec![ToolCall{id:id.into(),name:name.into(),arguments:args.to_string()}],usage_total_tokens:None};
        Ok(match stage {
            0 => call("om6-source","book.text",json!({"lid":"1.1","end_lid":"1.1"})),
            1 => {
                let mut plan=output["design"].clone();
                plan["design"]["previous_ref"]=context["current_design"]["design_ref"].clone();
                call("om6-design","tutor.step",plan)
            },
            2 => {
                let mut selection=output["select"].clone();
                selection["move"]["design_ref"]=result(plan,"om6-design")["design_ref"].clone();
                selection["move"]["delivery_text"]=output["answer"].clone();
                call("om6-select","tutor.step",selection)
            },
            3 => AssistantTurn {provider_continuation:None,text:output["answer"].as_str().map(str::to_owned),
                tool_calls:vec![],usage_total_tokens:None},
            _ => return Err(AdapterError{message:format!("Frozen OM6 output needs repair: {}",json!(plan.input.last())),spend_stop:None}),
        })
    }
}
fn configure_reply(f:&Fixture, output:Value, original:Value, captures:Arc<Mutex<Vec<Value>>>) {
    f.access.runs.configure_fake(move||Box::new(FrozenTeaching {output:output.clone(),original:original.clone(),
        stage:Default::default(),captures:captures.clone()}));
}
fn seed_output() -> Value {
    let mut plan=design::design_request("om6-network",None);
    plan["design"]["organizing_model"]=json!(HELP);
    let mut selection=design::select_request("om6-network-help","",false);
    selection["move"]["prompt"]=json!(HELP);selection["move"]["delivery_text"]=json!(HELP);
    json!({"design":plan,"select":selection,"answer":HELP})
}
fn send(f:&Fixture,w:&Value,key:&str,message:&str) {
    let w=f.get(&f.a,w);
    let mut input=Fixture::stamp(&w,"om6-page");
    input["session_id"]=w["selected_chat"].clone();input["client_request_id"]=json!(key);input["message"]=json!(message);
    let (status,value)=admit(f,&w,&input);assert_eq!(status,202,"{value}");
    f.access.runs.run_one(&f.access).unwrap();
    let user=context(f,&f.a).user;
    let user=user.lock().unwrap();
    let turn=user.agent_history.sessions.iter().find(|s|s.id==w["selected_chat"]).unwrap().turns.last().unwrap();
    assert_eq!(turn.outcome.as_ref().unwrap().incomplete,false,"{:?}",turn.outcome);
    assert!(turn.domain.teaching.iter().flat_map(|link|&link.receipt_ids).any(|id|id.starts_with("delivery:")),
        "The history seed must contain an actual teaching delivery");
}
fn setup_scene() -> (Fixture, Value, String, String, Vec<u8>) {
    let mut f=Fixture::new();
    let (source,_state)=design::fixture();
    let published=f.access.library.lock().unwrap().publish(source.path()).unwrap().reference;
    f.access.library.lock().unwrap().grant("A",&published).unwrap();
    f.x=published;
    configure_reply(&f,seed_output(),Value::Null,Default::default());
    let space=f.ok(&f.a,"POST","/api/learning-workspaces",json!({"goal":{"text":"比较行程方案"},"members":[f.x]}));
    let original_scope=space["learning_workspace_id"].as_str().unwrap().to_owned();
    f.ok(&f.a,"POST",&format!("/api/tutor/mutate?learning_workspace_id={original_scope}"),
        json!({"operation_id":"enable","expected_revision":0,"action":{"kind":"set_enabled","enabled":true}}));
    let w=f.ok(&f.a,"POST","/api/workspaces",json!({"published_book_ref":f.x,"attachment_id":"om6-page","learning_workspace_id":original_scope}));
    let w=f.action(&f.a,&w,"om6-page","chat/new",json!({}));
    send(&f,&w,"om6-prior",PRIOR);
    send(&f,&w,"om6-expression",WORDS);
    let chat=w["selected_chat"].as_str().unwrap().to_owned();
    let prefix=f.ok(&f.a,"GET",&format!("/api/me/learning-records/{chat}"),Value::Null);
    let target=f.ok(&f.a,"POST","/api/learning-workspaces",json!({"goal":{"text":"带着原方案深入比较","constraints":["必要休息必须保留"]},"members":[f.x]}));
    let scope=target["learning_workspace_id"].as_str().unwrap().to_owned();
    let control=f.ok(&f.a,"GET",&format!("/api/tutor/state?learning_workspace_id={scope}"),Value::Null);
    f.ok(&f.a,"POST",&format!("/api/tutor/mutate?learning_workspace_id={scope}"),
        json!({"operation_id":"enable-target","expected_revision":control["control"]["revision"],"action":{"kind":"set_enabled","enabled":true}}));
    f.ok(&f.a,"POST",&format!("/api/learning-workspaces/{scope}/records"),
        json!({"session_id":chat,"through_seq":prefix["through_seq"],"goal_revision":1}));
    send(&f,&w,"om6-excluded","这条较晚的记录没有选入新工作区。");
    let user=context(&f,&f.a).user;
    let original_bytes=std::fs::read(user.lock().unwrap().session_store.as_ref().unwrap().paths.session(&chat)).unwrap();
    drop(user);
    // Reconstruct Authorization/UserRegistry and all SQLite/JSONL state.
    let f=reopen(f,&Probe::default());
    let w=f.ok(&f.a,"POST","/api/workspaces",json!({"published_book_ref":f.x,"attachment_id":"om6-page","learning_workspace_id":scope}));
    let w=f.action(&f.a,&w,"om6-page","chat/new",json!({}));
    (f,w,chat,original_scope,original_bytes)
}

#[test]
#[ignore = "OM6 network Reader host with frozen model output"]
fn om6_network_browser_host() {
    let (f,w,original_chat,original_scope,original_bytes)=setup_scene();
    let captures=Arc::new(Mutex::new(vec![]));
    configure_reply(&f,read("after/open-design/output.json"),json!([
        read("after/open-design/input.json")["packet"]["context"],
        read("after/open-design/stage2-input.json")["packet"]["context"]
    ]),captures.clone());
    let server=multi_user_host::start_with_access("127.0.0.1:4196".parse().unwrap(),
        Arc::new(Site::new("https://reader.example").unwrap()),f.access.clone()).unwrap();
    let control=tiny_http::Server::http("127.0.0.1:4197").unwrap();
    println!("OM6_NETWORK_READY");
    for request in control.incoming_requests() {
        let value=match request.url() {
            "/fixture" => json!({"workspace":w,"cookie":format!("{}={}",crate::auth::COOKIE,f.a),"csrf":crate::auth::csrf(&f.a),
                "message":read("after/open-design/input.json")["packet"]["request"]}),
            "/evidence" => {
                let user=context(&f,&f.a).user;let user=user.lock().unwrap();
                assert_eq!(std::fs::read(user.session_store.as_ref().unwrap().paths.session(&original_chat)).unwrap(),original_bytes);
                let result=json!({"captures":*captures.lock().unwrap(),"original_chat":original_chat,"original_scope":original_scope,
                    "original_unchanged":true,"history":user.agent_history.sessions});
                std::fs::write(directory().join("network-evidence.json"),serde_json::to_vec_pretty(&result).unwrap()).unwrap();
                result
            },
            "/stop" => {request.respond(tiny_http::Response::from_string("stopped")).unwrap();break;},
            _ => json!({"error":"unknown test control"}),
        };
        request.respond(tiny_http::Response::from_string(value.to_string()).with_header(
            tiny_http::Header::from_bytes("Content-Type","application/json").unwrap())).unwrap();
    }
    server.shutdown();
}
