//! TB4: public practice fixtures and explicit-input frozen browser authoring.
use super::*;
use crate::tests::tutor_loop_tests::prepare_activity_chat as prepare_agent_chat;
use runtime::presentation::PresentationRef;

pub fn directory() -> PathBuf {
    PathBuf::from(std::env::var_os("TB4_TRIAL_DIR")
        .expect("set TB4_TRIAL_DIR to the frozen authoring trial directory containing sol/ and artifact.json"))
}
pub fn read(path: impl AsRef<Path>) -> Value {
    let path = path.as_ref();
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("cannot read TB4 trial input {}: {error}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|error| panic!("invalid TB4 trial JSON {}: {error}", path.display()))
}
pub fn save(path: impl AsRef<Path>, value: &Value) {
    std::fs::create_dir_all(path.as_ref().parent().unwrap()).unwrap();
    std::fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

pub fn borrowing_practice(path: impl AsRef<Path>) -> Value {
    practice_output(read(path))
}

fn practice_output(mut output: Value) -> Value {
    let question = output["answer"].clone();
    let movement = &mut output["select"]["move"];
    // These historical packets asked an ungraded dialogue question. The fixture
    // now enters explicit practice, so supply the private contract for that same
    // unanswered question without changing its visible explanation or support.
    movement["prompt"] = question;
    movement["actions"] = json!(["submit", "revise", "request_hint", "reveal", "skip"]);
    movement["explanation"] = json!("b 的类型是 &Foo，提供共享访问。Foo 实现 Copy 只改变值复制的规则，不会把共享引用变成可变引用，因此 b.0 = 8 不能编译。");
    movement["assessment"] = json!({"rule":{"kind":"open","items":[{"id":"shared-access",
        "criterion":"判断 b.0 = 8 不能编译，并说明 Copy 不会改变 &Foo 的共享访问权限。","source_lids":["1.1"]}]},
        "source_lids":["1.1"],"source_sufficient":true,"feedback":"after_submit"});
    movement["practice"] = json!({"entry":"warmup","mode":"discriminate","distance":"adjacent",
        "hints":[{"kind":"structure","text":"分别检查 Foo 的性质与 b 的类型。"}]});
    output
}

pub fn seed() -> (tempfile::TempDir, AppState, Value) {
    let case = super::tutor_adaptation_tests::cases().into_iter()
        .find(|case| case["id"] == "independent-step").unwrap();
    let (root, mut state, delivery, prior) = super::tutor_adaptation_tests::seed(&case);
    let run = prepare_agent_chat(&mut state, &json!({"message":case["response"]}).to_string(), "tb4-seed").unwrap_or_else(|r|panic!("{}",r.body));
    let mut packet = super::tutor_adaptation_tests::context_packet(&state,&run,&delivery);
    let learner_fact = packet["read_facts"].as_array().unwrap().iter()
        .find(|fact| fact["content"]["response"] == case["response"]).unwrap()["event_id"].as_str().unwrap();
    let output: Value = serde_json::from_str(&include_str!("../../tests/fixtures/tutor-borrowing/practice-followup.json")
        .replace("$LEARNER_FACT", learner_fact)).unwrap();
    packet["presented_sources"] = prior["presented_sources"].clone();
    let replay = super::tutor_adaptation_tests::replay_turn(&mut state,run,&packet,practice_output(output),&packet,false);
    let delivery = replay["facts"].as_array().unwrap().iter().find(|f|f["kind"]=="message_delivered").unwrap().clone();
    assert_eq!(route_agent_new(&mut state,"tb4-seed-reopen").status,200);
    (root,state,delivery)
}

fn recorded_seed() -> (tempfile::TempDir, AppState, Value) {
    let tb3 = super::tutor_adaptation_tests::directory();
    let case = read(tb3.join("scenarios.json"))["cases"].as_array().unwrap().iter()
        .find(|c| c["id"] == "independent-step").unwrap().clone();
    let (root, mut state, delivery, _) = super::tutor_adaptation_tests::recorded_seed(&case);
    let run = prepare_agent_chat(&mut state, &json!({"message":case["response"]}).to_string(), "tb4-seed").unwrap_or_else(|r|panic!("{}",r.body));
    let current = super::tutor_adaptation_tests::context_packet(&state,&run,&delivery);
    let dir = tb3.join("sol/repair/independent-step");
    let replay = super::tutor_adaptation_tests::replay_turn(&mut state,run,&read(dir.join("input.json")),borrowing_practice(dir.join("output.json")),&current,false);
    let delivery = replay["facts"].as_array().unwrap().iter().find(|f|f["kind"]=="message_delivered").unwrap().clone();
    // Reopen the unanswered activity by its original delivery instead of
    // creating a duplicate practice question with a new identity.
    assert_eq!(route_agent_new(&mut state,"tb4-seed-reopen").status,200);
    (root,state,delivery)
}

#[test]
fn tutor_tb4_keeps_tb3_help_and_unanswered_judgment() {
    let (_root,mut state,delivery) = seed();
    let run = prepare_agent_chat(&mut state,&json!({"message":"让我用操作检验上次的判断，先别给答案。","teaching_ref":delivery["event_id"]}).to_string(),"tb4").unwrap_or_else(|r|panic!("{}",r.body));
    let current = super::tutor_adaptation_tests::context_packet(&state,&run,&delivery);
    let interpretation = &current["context"]["learner_context"]["interpretations"][0];
    assert_eq!(interpretation["evidence_ref"],"interpretation:tb3-independent-copy-condition");
    assert_eq!(interpretation["nature"],"hypothesis");
    assert_eq!(interpretation["status"],"unassessed");
    assert!(interpretation["interpretation"]["text"].as_str().unwrap().contains("完整讲解"));
    assert!(delivery["payload"]["answer"].as_str().unwrap().contains("b.0 = 8"));
    assert!(state.user.learning_store().unwrap().learner_projection(&state.workspace.book.base.book_id).unwrap().0.unwrap().rows.is_empty());
}

pub struct AuthorReplay {
    packet: Value,
    output: Value,
    stage: std::cell::Cell<usize>,
    sources: RefCell<Vec<(String,String)>>,
    receipts: RefCell<BTreeMap<String,Value>>,
}
impl AuthorReplay {
    fn rebound(&self) -> Value {
        let mut text = self.output.to_string();
        for (old,new) in self.sources.borrow().iter() { text = text.replace(old,new); }
        serde_json::from_str(&text).unwrap()
    }
}
impl ModelAdapter for AuthorReplay {
    fn complete(&self, _: CompletionRequest) -> Result<ParsedResponse,AdapterError> { unreachable!("local replay has no Provider") }
    fn complete_structured(&self, request: CompletionRequest) -> Result<Value, AdapterError> {
        if let Some(review)=super::tutor_loop_tests::fixture_delivery_review(&request) { return Ok(review); }
        unreachable!("local replay has no other Provider helpers")
    }
    fn chat(&self, plan: &AgentRequestPlan) -> Result<AssistantTurn,AdapterError> {
        use base64::Engine as _;
        let n = self.stage.get(); self.stage.set(n+1);
        for message in &plan.input {
            if let (Some(id),Some(content)) = (&message.tool_call_id,&message.content) {
                if let Ok(value) = serde_json::from_str::<Value>(content) {
                    self.receipts.borrow_mut().insert(id.clone(),value["model_body"].clone());
                }
            }
        }
        save(directory().join(format!("replay/request-{n:02}.json")),&json!({"messages":plan.ordered_messages(),"tools":plan.tools,"assets":plan.instruction_assets}));
        save(directory().join("replay/receipts.json"),&json!(*self.receipts.borrow()));
        for (index,image) in plan.preview_images.iter().enumerate() {
            std::fs::write(directory().join(format!("replay/preview-{n:02}-{index}.png")),base64::engine::general_purpose::STANDARD.decode(&image.png_base64).unwrap()).unwrap();
        }
        if n == 2 {
            for (index,source) in self.packet["presented_sources"].as_array().unwrap().iter().enumerate() {
                let receipt = self.receipts.borrow()[&format!("source-{index}")].clone();
                self.sources.borrow_mut().push((source["source_ref_id"].as_str().unwrap().into(),receipt["source_ref_id"].as_str().unwrap().into()));
            }
        }
        let out = self.rebound();
        // The captured author supplied preview batches (actions plus reading
        // selector). Run its comparison in all required environments, then its
        // remaining comparisons on desktop. Every preview also captures initial state.
        let batches = out["preview_actions"].as_array().unwrap();
        let delivery_stage = 7 + 3 + batches.len().saturating_sub(2);
        let call = |id: &str,name: &str,args: Value| ToolCall{id:id.into(),name:name.into(),arguments:args.to_string()};
        let author = |id: &str,args: Value|call(id,"presentation.author",args);
        let calls = match n {
            0 => vec![call("read","book.text",json!({"lid":"1.1","end_lid":"1.1"}))],
            1 => self.packet["presented_sources"].as_array().unwrap().iter().enumerate().map(|(i,s)|call(&format!("source-{i}"),"source.present",json!({"quote":s["quote"]}))).collect(),
            2 => vec![call("discover","tool.search",json!({"task":"build interactive borrowing comparison","required_capabilities":["presentation_authoring"],"scope":"passage","operation":"explain","effect_mode":"read_only","max_results":1}))],
            3 => vec![author("global",json!({"operation":"prepare","phase":"global","framework":out["framework"],"focus":out["focus"],"needs":[]}))],
            4 => vec![author("local",json!({"operation":"prepare","phase":"local","framework":out["framework"],"focus":out["focus"],"needs":["state"]}))],
            5 => vec![author("write",out["write"].clone())],
            6 => vec![author("review",json!({"operation":"prepare","phase":"review","framework":out["framework"],"focus":out["focus"],"needs":["state"]}))],
            stage if (7..delivery_stage).contains(&stage) => {
                let candidate = self.receipts.borrow()["write"]["candidate_id"].clone();
                let index = stage-7;
                let (_,viewport) = runtime::presentation_preview::REQUIRED_PREVIEW_ENVIRONMENTS[index.min(2)];
                let batch = &batches[if index<3 {1.min(batches.len()-1)} else {index-1}];
                vec![author(&format!("preview-{index}"),json!({"operation":"preview","candidate_id":candidate,"viewport":viewport,"actions":batch["actions"],"read_selector":batch["read_selector"]}))]
            },
            stage if stage==delivery_stage => {
                let candidate = self.receipts.borrow()["write"]["candidate_id"].clone();
                save(directory().join("replay/awaiting-inspection.json"),&json!({"candidate_id":candidate,"instruction":"Inspect actual preview images and receipts, then save accepted-preview.json with this candidate_id."}));
                let deadline = std::time::Instant::now()+Duration::from_secs(900);
                loop {
                    let accepted = directory().join("replay/accepted-preview.json");
                    if accepted.exists() && read(&accepted)["candidate_id"] == candidate {break;}
                    assert!(std::time::Instant::now()<deadline,"actual preview inspection not completed");
                    std::thread::sleep(Duration::from_millis(250));
                }
                vec![author("deliver",json!({"operation":"deliver","candidate_id":candidate}))]
            },
            stage if stage==delivery_stage+1 => {
                let mut selected = out["select"].clone();
                selected["move"]["presentation"] = self.receipts.borrow()["deliver"]["reference"].clone();
                assert!(selected["move"]["presentation"].is_object(),"{:?}",self.receipts.borrow());
                vec![call("select","tutor.step",selected)]
            },
            stage if stage==delivery_stage+2 => vec![],
            _ => return Err(AdapterError{message:"Frozen TB4 author output failed; no automatic repair sampling".into(),spend_stop:None}),
        };
        Ok(AssistantTurn{provider_continuation:None,text:(n==delivery_stage+2).then(||out["answer"].as_str().unwrap().into()),tool_calls:calls,usage_total_tokens:None})
    }
}

#[test]
#[ignore = "real browser preview needs TB4_TRIAL_DIR, TB3_TRIAL_DIR, TB3_SOURCE and TB3_PRIOR_DIR; serves on 4185"]
fn tutor_tb4_author_and_browser_host() {
    let (root,mut state,prior_delivery) = recorded_seed();
    let packet = read(directory().join("sol/author/input.json"));
    let output = read(directory().join("artifact.json"));
    let run = prepare_agent_chat(&mut state,&json!({"message":packet["user_request"],"teaching_ref":prior_delivery["event_id"]}).to_string(),"tb4-author").unwrap_or_else(|r|panic!("{}",r.body));
    let turn = run.turn_ref.clone();
    let session = run.tutor.as_ref().unwrap()["binding"]["tutor_session_id"].as_str().unwrap().to_string();
    save(directory().join("replay/author-context.json"),&super::tutor_adaptation_tests::context_packet(&state,&run,&prior_delivery));
    let adapter = AuthorReplay{packet,output,stage:Default::default(),sources:Default::default(),receipts:Default::default()};
    let report = agent_run::execute_prepared(&agent_run::BorrowedAppPort(RefCell::new(&mut state)),&adapter,run,Default::default());
    save(directory().join("replay/author-result.json"),&json!({"status":report.reply.status,"body":report.reply.body}));
    assert_eq!(report.reply.status,200,"{}",report.reply.body);
    let outcome: OuterOutcome = serde_json::from_str(&report.reply.body).unwrap();
    assert!(!outcome.incomplete,"{}",report.reply.body);
    let reference: PresentationRef = serde_json::from_value(adapter.receipts.borrow()["deliver"]["reference"].clone()).unwrap();
    let version = state.private_context().read_presentation(&turn.session_id,&reference).unwrap();
    save(directory().join("replay/delivered-version.json"),&json!(version));
    let delivery = state.user.learning_store().unwrap().teaching_deliveries(&turn.session_id,&turn.turn_id).unwrap().remove(0);
    assert_eq!(delivery.payload["presentation"]["reference"],json!(reference));
    assert_eq!(delivery.payload["move"]["interpretation_refs"],json!(["interpretation:tb3-independent-copy-condition"]));
    let fixture = json!({"session_id":turn.session_id,"turn_id":turn.turn_id,"reference":reference,"outcome":outcome,"user_request":adapter.packet["user_request"]});
    save(directory().join("browser/fixture.json"),&fixture);
    let server = tiny_http::Server::http("127.0.0.1:4185").unwrap();
    println!("TB4_BROWSER_READY");
    let deadline = std::time::Instant::now()+Duration::from_secs(1800);
    while std::time::Instant::now()<deadline {
        let Some(mut request) = server.recv_timeout(Duration::from_secs(1)).unwrap() else {continue;};
        let path = request.url().to_string();
        let mut body = String::new(); request.as_reader().read_to_string(&mut body).unwrap();
        let reply = match path.as_str() {
            "/fixture" => ok_json(&fixture),
            "/reopen" => {
                state.user.store = MemoryStore::open(root.path().join("memory.json")).unwrap();
                let (history,storage)=crate::session_store::load_chat_storage(&state.user.history_path).unwrap();
                state.user.agent_history=history; state.user.session_store=storage;
                ok_json(&json!({"ok":true}))
            },
            "/agent/chat" => {
                let input: Value=serde_json::from_str(&body).unwrap();
                let receipt=serde_json::from_value(input["presentation_follow_up"].clone()).unwrap();
                let scene=state.private_context().read_presentation_state(&receipt).unwrap();
                assert_eq!(scene.receipt.reference,reference);
                assert_eq!(scene.receipt.session_id,turn.session_id);
                let run=prepare_agent_chat(&mut state,&body,"tb4-followup").unwrap_or_else(|r|panic!("{}",r.body));
                assert_eq!(run.turn_ref.session_id,turn.session_id);
                assert_eq!(run.tutor.as_ref().unwrap()["binding"]["tutor_session_id"],session);
                let mut current=super::tutor_adaptation_tests::context_packet(&state,&run,&json!(delivery));
                current["scene"]=json!(scene); current["delivered_version"]=json!(version);
                current["agent_message"]=json!(run.agent_message);
                save(directory().join("browser/followup-context.json"),&current);
                let output_path=directory().join("followup-delivery.json");
                let stop=std::time::Instant::now()+Duration::from_secs(900);
                while !output_path.exists() {
                    assert!(std::time::Instant::now()<stop,"authorized follow-up output not supplied");
                    std::thread::sleep(Duration::from_millis(250));
                }
                let mut packet=read(directory().join("sol/followup/input.json"));
                // TB3 replay rebinding consumes immutable facts from this same captured turn.
                packet["read_facts"]=packet["current"]["read_facts"].clone();
                packet["prior_delivery"]=packet["current"]["prior_delivery"].clone();
                let mut output=read(output_path);
                // Save batching can change the receipt counter on a replay of
                // identical controls/results. Rebind only that runtime identity.
                assert_eq!(packet["current"]["scene"]["state"]["values"]["page"],current["scene"]["state"]["values"]["page"]);
                assert_eq!(packet["current"]["scene"]["state"]["observed_result"],current["scene"]["state"]["observed_result"]);
                let label=|receipt: &Value|format!("revision {}、state_revision {}",receipt["reference"]["revision"],receipt["state_revision"]);
                let old=label(&packet["current"]["scene"]["receipt"]);
                let new=label(&current["scene"]["receipt"]);
                let interpretation=output["evidence"]["interpretation"].as_str().unwrap();
                assert!(interpretation.contains(&old));
                output["evidence"]["interpretation"]=json!(interpretation.replace(&old,&new));
                save(directory().join("browser/scene-rebinding.json"),&json!({"old":old,"new":new,"same_conditions_and_results":true,"changed_field":"evidence.interpretation","answer_changed":false}));
                let result=super::tutor_adaptation_tests::replay_turn(&mut state,run,&packet,output,&current,false);
                save(directory().join("browser/followup-result.json"),&result);
                assert!(result["facts"].as_array().unwrap().iter().all(|f|f["kind"]!="response_assessed"));
                let delivered=result["facts"].as_array().unwrap().iter().find(|f|f["kind"]=="message_delivered").unwrap();
                assert!(delivered["payload"]["move"]["interpretation_refs"].as_array().unwrap().contains(&result["evidence"]["evidence_id"]));
                ok_json(&result["outcome"])
            },
            "/stop" => { request.respond(tiny_http::Response::from_string("stopped")).unwrap(); break; },
            _ => route(&mut state,Req{method:request.method().as_str(),url:&path,body:&body,now:"tb4-browser"}),
        };
        request.respond(tiny_http::Response::from_string(reply.body).with_status_code(reply.status)
            .with_header(tiny_http::Header::from_bytes("Content-Type","application/json").unwrap())).unwrap();
    }
}
