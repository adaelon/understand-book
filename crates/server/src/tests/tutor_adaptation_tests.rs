//! TB3 scripted learner fixtures, real tools/storage, and frozen model delivery.
use super::*;
use memory::teaching::TeachingFact;

fn directory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/performance/tutor-building-blocks-tb3")
}
fn read(path: impl AsRef<Path>) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}
fn save(path: impl AsRef<Path>, value: &Value) {
    std::fs::create_dir_all(path.as_ref().parent().unwrap()).unwrap();
    std::fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}
fn range() -> EvidenceRange { EvidenceRange { start_lid:"1.1".into(), end_lid:"1.1".into(), ranges:vec![] } }
fn cases() -> Vec<Value> { read(directory().join("scenarios.json"))["cases"].as_array().unwrap().clone() }

struct Replay {
    output: Value,
    sources: Vec<Value>,
    references: Vec<(String, String)>,
    stage: std::cell::Cell<usize>,
}
impl Replay {
    fn rebound(&self, plan: &AgentRequestPlan) -> Value {
        let mut text = self.output.to_string();
        for (old, new) in &self.references { text = text.replace(old, new); }
        for (index, source) in self.sources.iter().enumerate() {
            let id = format!("source-{index}");
            let result = plan.input.iter().find(|m|m.tool_call_id.as_deref()==Some(&id)).unwrap();
            let body: Value = serde_json::from_str(result.content.as_deref().unwrap()).unwrap();
            text = text.replace(source["source_ref_id"].as_str().unwrap(), body["model_body"]["source_ref_id"].as_str().unwrap());
        }
        serde_json::from_str(&text).unwrap()
    }
}
impl ModelAdapter for Replay {
    fn complete(&self, _: CompletionRequest) -> Result<ParsedResponse, AdapterError> { unreachable!("local replay has no model helpers") }
    fn chat(&self, plan: &AgentRequestPlan) -> Result<AssistantTurn, AdapterError> {
        let stage = self.stage.get(); self.stage.set(stage+1);
        let call = |id: &str, name: &str, arguments: Value| ToolCall { id:id.into(), name:name.into(), arguments:arguments.to_string() };
        let has_evidence = self.output["evidence"].is_object();
        let (text, tool_calls) = match stage {
            0 => (None, vec![call("read", "book.text", json!({"lid":"1.1","end_lid":"1.1"}))]),
            1 => (None, self.sources.iter().enumerate().map(|(i,s)|call(&format!("source-{i}"), "source.present", json!({"quote":s["quote"]}))).collect()),
            2 if has_evidence => (None, vec![call("interpret", "tutor.step", self.rebound(plan)["evidence"].clone())]),
            n if n == 2+usize::from(has_evidence) => (None, vec![call("select", "tutor.step", self.rebound(plan)["select"].clone())]),
            n if n == 3+usize::from(has_evidence) => (Some(self.rebound(plan)["answer"].as_str().unwrap().into()), vec![]),
            _ => return Err(AdapterError {message:format!("Frozen output requires a repair: {:?}",plan.input.last()),spend_stop:None}),
        };
        Ok(AssistantTurn {provider_continuation:None,text,tool_calls,usage_total_tokens:None})
    }
}

pub(super) fn seed(case: &Value) -> (tempfile::TempDir, AppState, Value, Value) {
    let (root, mut state) = super::tutor_building_blocks_tests::fixture();
    super::session_runtime_tests::enable_jsonl(&mut state);
    assert_eq!(route_agent_new(&mut state, "tb3").status, 200);
    let prior_dir = directory().join("../tutor-building-blocks-tb2/sol").join(case["prior"].as_str().unwrap());
    let packet = read(prior_dir.join("input.json"));
    let output = read(prior_dir.join("output.json"));
    let run = prepare_agent_chat(&mut state, &json!({"message":packet["user_request"]}).to_string(), "tb3").unwrap_or_else(|r|panic!("{}",r.body));
    let turn = run.turn_ref.clone();
    let tutor_session = run.tutor.as_ref().unwrap()["binding"]["tutor_session_id"].as_str().unwrap().to_string();
    let adapter = Replay {output:output.clone(),sources:packet["presented_sources"].as_array().unwrap().clone(),references:vec![],stage:Default::default()};
    let result = agent_run::execute_prepared(&agent_run::BorrowedAppPort(RefCell::new(&mut state)), &adapter, run, Default::default());
    assert_eq!(result.reply.status,200,"{}",result.reply.body);
    let delivery = state.user.learning_store().unwrap().teaching_deliveries(&turn.session_id,&turn.turn_id).unwrap().into_iter().next().unwrap();
    if case["id"] == "support-mismatch" {
        teaching::step(&state.private_context(),&turn,json!({"operation":"evidence","nature":"hypothesis","operation_id":"tb3-prior-copy",
            "target":output["select"]["move"]["target"],"fact_refs":[format!("usage:{tutor_session}:user:{}",turn.turn_id)],
            "interpretation":"用户原话将引用理解为复制 Foo；是否已修正尚未知。","teaching_implication":"用一份值与引用的关系图澄清复制推断，再听用户解释。"}),&[],&[range()]).unwrap();
    }
    (root,state,serde_json::to_value(delivery).unwrap(),packet)
}

pub(super) fn context_packet(state: &AppState, run: &agent_run::PreparedAgentChat, delivery: &Value) -> Value {
    let context = run.tutor.as_ref().unwrap();
    let mut facts = Vec::new();
    for fact in context["learner_context"]["recent_facts"].as_array().unwrap() {
        let id = fact["event_id"].as_str().unwrap();
        if matches!(fact["kind"].as_str(),Some("move_selected"|"turn_bound"|"help_delivered")) {continue;}
        let mut after = 0;
        let mut text = String::new();
        loop {
            let value = teaching::step(&state.private_context(),&run.turn_ref,json!({"operation":"trace","event_id":id,"after":after}),&[],&[]).unwrap();
            text.push_str(value["chunk"].as_str().unwrap());
            let Some(next) = value["next"].as_u64() else {break}; after=next;
        }
        facts.push(serde_json::from_str::<Value>(&text).unwrap());
    }
    json!({"context":context,"read_facts":facts,"prior_delivery":delivery,
        "understanding":teaching::step(&state.private_context(),&run.turn_ref,json!({"operation":"understanding"}),&[],&[]).unwrap()})
}

#[test]
#[ignore = "exports fixed local experiment inputs; no external model calls"]
fn tutor_tb3_export_response_contexts() {
    for case in cases() {
        let (_root,mut state,delivery,prior) = seed(&case);
        let run = prepare_agent_chat(&mut state,&json!({"message":case["response"]}).to_string(),"tb3").unwrap_or_else(|r|panic!("{}",r.body));
        let mut packet = context_packet(&state,&run,&delivery);
        packet["presented_sources"] = prior["presented_sources"].clone();
        save(directory().join("contexts").join(case["id"].as_str().unwrap()).join("response.json"),&packet);
    }
}

#[test]
fn tutor_tb3_understanding_pages_preserve_older_revisions_and_long_facts() {
    let case = cases().remove(0);
    let (_root,mut state,delivery,_) = seed(&case);
    let message = format!("{} 最后才说明：我借助了上一次示范，还不能独立解释。", "这里是逐行思考。".repeat(1300));
    let run = prepare_agent_chat(&mut state,&json!({"message":message}).to_string(),"tb3").unwrap_or_else(|r|panic!("{}",r.body));
    let context = run.tutor.as_ref().unwrap();
    let fact = format!("usage:{}:user:{}",context["binding"]["tutor_session_id"].as_str().unwrap(),run.turn_ref.turn_id);
    let summary = context["learner_context"]["recent_facts"].as_array().unwrap().iter().find(|f|f["event_id"]==fact).unwrap();
    assert_eq!(summary["response_preview"]["truncated"],true);
    let mut after=0; let mut exact=String::new(); let mut chunks=0;
    loop {
        let page=teaching::step(&state.private_context(),&run.turn_ref,json!({"operation":"trace","event_id":fact,"after":after}),&[],&[]).unwrap();
        exact.push_str(page["chunk"].as_str().unwrap()); chunks+=1;
        let Some(next)=page["next"].as_u64() else {break}; after=next;
    }
    assert!(chunks>1);
    assert_eq!(serde_json::from_str::<Value>(&exact).unwrap()["content"]["response"],message);
    for index in 0..25 {
        let mut request=json!({"operation":"evidence","nature":"hypothesis","operation_id":format!("page-{index}"),
            "target":delivery["payload"]["move"]["target"],"fact_refs":[fact],"interpretation":format!("Observation {index}"),"teaching_implication":"Keep the demonstrated support; independent explanation remains unknown."});
        if index==24 {request["supersedes"]=json!("interpretation:page-1");}
        teaching::step(&state.private_context(),&run.turn_ref,request,&[],&[range()]).unwrap();
    }
    let mut after=0; let mut ids=Vec::new();
    loop {
        let page=teaching::step(&state.private_context(),&run.turn_ref,json!({"operation":"understanding","after":after}),&[],&[]).unwrap();
        ids.extend(page["interpretations"].as_array().unwrap().iter().map(|e|e["evidence_id"].as_str().unwrap().to_string()));
        let Some(next)=page["next"].as_u64() else {break}; assert!(next>after); after=next;
    }
    assert_eq!(ids.len(),24,"all current source-only interpretations must be discoverable");
    assert!(ids.contains(&"interpretation:page-0".into()));
    assert!(!ids.contains(&"interpretation:page-1".into()));
    let first=teaching::step(&state.private_context(),&run.turn_ref,json!({"operation":"understanding"}),&[],&[]).unwrap();
    assert_eq!(first["interpretations"][0]["learner_quote_truncated"],true);
    assert_eq!(first["interpretations"][0]["fact_refs"],json!([fact]));
    let mut after=0; let mut exact=String::new();
    loop {
        let page=teaching::step(&state.private_context(),&run.turn_ref,json!({"operation":"understanding","evidence_ref":"interpretation:page-24","after":after}),&[],&[]).unwrap();
        exact.push_str(page["chunk"].as_str().unwrap());
        let Some(next)=page["next"].as_u64() else {break}; after=next;
    }
    assert_eq!(serde_json::from_str::<Value>(&exact).unwrap()["learner_quote"],message);
}

#[test]
fn tutor_tb3_referenced_chat_keeps_displayed_help_on_usage_hypothesis() {
    let (root,mut state)=super::tutor_building_blocks_tests::fixture();
    let prior_dir=directory().join("../tutor-building-blocks-tb2/sol/after/starting-ownership");
    let packet=read(prior_dir.join("input.json"));
    let mut output=read(prior_dir.join("output.json"));
    output["select"]["move"]["actions"]=json!(["request_hint"]);
    output["select"]["move"]["hint"]=json!("先判断右侧是 Foo 还是 &Foo，再追踪所有者。");
    let run=prepare_agent_chat(&mut state,&json!({"message":packet["user_request"]}).to_string(),"tb3").unwrap_or_else(|r|panic!("{}",r.body));
    let turn=run.turn_ref.clone();
    let adapter=Replay {output:output.clone(),sources:packet["presented_sources"].as_array().unwrap().clone(),references:vec![],stage:Default::default()};
    let result=agent_run::execute_prepared(&agent_run::BorrowedAppPort(RefCell::new(&mut state)),&adapter,run,Default::default());
    assert_eq!(result.reply.status,200);
    let delivery=state.user.learning_store().unwrap().teaching_deliveries(&turn.session_id,&turn.turn_id).unwrap().remove(0);
    let post_json=|state:&mut AppState,path:&str,value:Value| {
        let reply=post(state,path,&value.to_string()); assert_eq!(reply.status,200,"{}",reply.body);
        serde_json::from_str::<Value>(&reply.body).unwrap()
    };
    post_json(&mut state,"/tutor/display",json!({"delivery_ref":delivery.event_id}));
    let help=post_json(&mut state,"/tutor/action",json!({"operation_id":"tb3-hint","delivery_ref":delivery.event_id,"action":"request_hint"}));
    post_json(&mut state,"/tutor/help-displayed",json!({"help_ref":help["help"]["event_id"]}));
    let control=state.user.learning_store().unwrap().state().unwrap().control;
    post_json(&mut state,"/tutor/mutate",json!({"operation_id":"tb3-after-help-mode","expected_revision":control.revision,
        "action":{"kind":"set_mode","session_id":control.current_tutor_session_id,"mode":"direct_explanation"}}));
    let message="照着提示，我先看出了 &foo 的类型是 &Foo，所有权仍在 foo。";
    let run=prepare_agent_chat(&mut state,&json!({"message":message,"teaching_ref":delivery.event_id}).to_string(),"tb3").unwrap_or_else(|r|panic!("{}",r.body));
    let fact=format!("usage:{}:user:{}",delivery.binding.tutor_session_id,run.turn_ref.turn_id);
    teaching::step(&state.private_context(),&run.turn_ref,json!({"operation":"evidence","nature":"hypothesis","operation_id":"help-observation",
        "target":delivery.payload["move"]["target"],"fact_refs":[fact],"interpretation":"借助已展示提示完成了类型判断，独立解释未知。","teaching_implication":"保留右侧类型作为支撑，让用户连接到打印。"}),&[],&[range()]).unwrap();
    let store=memory::learning::LearningStore::open(&root.path().join("learning.db"),false).unwrap();
    let evidence=store.evidence("interpretation:help-observation").unwrap();
    assert_eq!(evidence.learner_quote,message);
    assert_eq!(evidence.assistance_refs,vec!["seen:help:tb3-hint"]);
    let stored_fact=store.teaching_event(&fact).unwrap();
    assert_ne!(stored_fact.binding.session_revision,delivery.binding.session_revision,"mode changes do not erase earlier help");
    assert_eq!(store.teaching_event(&evidence.assistance_refs[0]).unwrap().payload["text"],output["select"]["move"]["hint"]);
    assert!(store.learner_projection(&state.workspace.book.base.book_id).unwrap().0.unwrap().rows.is_empty());
}

fn references(old: &Value, current: &Value) -> Vec<(String,String)> {
    let mut result=Vec::new();
    // Turn identities change between isolated fixtures; match immutable actual
    // learner expressions/deliveries, never model interpretation text.
    for old_fact in old["read_facts"].as_array().unwrap() {
        if let Some(new_fact)=current["read_facts"].as_array().unwrap().iter().find(|new_fact|
            new_fact["kind"]==old_fact["kind"] && new_fact["content"]["usage"]==old_fact["content"]["usage"]
            && new_fact["content"]["response"]==old_fact["content"]["response"]
            && new_fact["content"]["answer"]==old_fact["content"]["answer"])
        {
            result.push((old_fact["event_id"].as_str().unwrap().into(),new_fact["event_id"].as_str().unwrap().into()));
        }
    }
    result.push((old["prior_delivery"]["event_id"].as_str().unwrap().into(),current["prior_delivery"]["event_id"].as_str().unwrap().into()));
    result
}

pub(super) fn replay_turn(state: &mut AppState, run: agent_run::PreparedAgentChat, packet: &Value, output: Value, current: &Value, capture_failure: bool) -> Value {
    let turn=run.turn_ref.clone();
    let session=run.tutor.as_ref().unwrap()["binding"]["tutor_session_id"].as_str().unwrap().to_string();
    let expected_evidence=output["evidence"]["operation_id"].as_str().map(|id|format!("interpretation:{id}"));
    let expected_refs=output["select"]["move"]["interpretation_refs"].clone();
    let samples=Arc::new(std::sync::Mutex::new(Vec::new()));
    let adapter=super::tutor_continuation_tests::TutorRequestRecorder {samples:samples.clone(),directory:None,
        inner:Box::new(Replay {output,sources:packet["presented_sources"].as_array().unwrap().clone(),references:references(packet,current),stage:Default::default()})};
    let result=agent_run::execute_prepared(&agent_run::BorrowedAppPort(RefCell::new(state)),&adapter,run,Default::default());
    if capture_failure {
        let store=state.user.learning_store().unwrap();
        let facts=store.teaching_turn_events(&session,&turn.turn_id).unwrap();
        let saved=state.user.agent_history.sessions.iter().find(|s|s.id==turn.session_id).unwrap().turns.last().unwrap();
        return json!({"http_status":result.reply.status,"reply":result.reply.body,"outcome":saved.outcome,"facts":facts,
            "evidence":expected_evidence.and_then(|id|store.evidence(&id).ok()),"requests":*samples.lock().unwrap(),"external_provider_calls":0});
    }
    assert_eq!(result.reply.status,200,"{}",result.reply.body);
    let saved=state.user.agent_history.sessions.iter().find(|s|s.id==turn.session_id).unwrap().turns.last().unwrap();
    let outcome=saved.outcome.as_ref().unwrap();
    assert!(!outcome.incomplete,"{}",result.reply.body);
    let store=state.user.learning_store().unwrap();
    let facts=store.teaching_turn_events(&session,&turn.turn_id).unwrap();
    let deliveries:Vec<_>=facts.iter().filter(|f|f.kind==TeachingFact::MessageDelivered).collect();
    assert_eq!(deliveries.len(),1);
    assert_eq!(deliveries[0].payload["move"]["interpretation_refs"],if expected_refs.is_null() {json!([])} else {expected_refs});
    let evidence=expected_evidence.map(|id|store.evidence(&id).unwrap());
    if let Some(evidence)=&evidence {
        assert_eq!(evidence.nature,memory::learning_evidence::EvidenceNature::Hypothesis);
        assert_eq!(evidence.status,memory::assessment::AssessmentStatus::Unassessed);
        assert!(evidence.assessment_ref.is_none());
        assert!(evidence.learner_quote.len()>0);
    }
    assert!(store.learner_projection(&state.workspace.book.base.book_id).unwrap().0.unwrap().rows.is_empty());
    json!({"outcome":outcome,"facts":facts,"evidence":evidence,"requests":*samples.lock().unwrap(),"external_provider_calls":0})
}

#[test]
#[ignore = "replays completed frozen Sol outputs and exports reopened contexts; no Provider"]
fn tutor_tb3_sol_replay_and_reopen() {
    let phase=std::env::var("TB3_PHASE").unwrap_or_else(|_|"response".into());
    let reopening=phase.starts_with("reopen");
    for case in cases() {
        for variant in if reopening {vec!["after"]} else {vec!["before","after"]} {
            let (root,mut state,delivery,prior)=seed(&case);
            let run=prepare_agent_chat(&mut state,&json!({"message":case["response"]}).to_string(),"tb3").unwrap_or_else(|r|panic!("{}",r.body));
            let current=context_packet(&state,&run,&delivery);
            let variant=if phase!="initial" && variant=="after" && matches!(case["id"].as_str(),Some("support-mismatch"|"independent-step")) {"repair"} else {variant};
            let dir=directory().join("sol").join(variant).join(case["id"].as_str().unwrap());
            let packet=read(dir.join("input.json"));
            let output=read(dir.join("output.json"));
            let result=replay_turn(&mut state,run,&packet,output,&current,phase=="initial" || variant=="before");
            if !reopening {save(dir.join("replay.json"),&result);}
            if phase=="initial" || variant=="before" {continue;}
            // Reopen private memory and history, then start a new chat while the
            // same TutorSession remains active. No old model conversation is used.
            state.user.store=MemoryStore::open(root.path().join("memory.json")).unwrap();
            let (history,storage)=crate::session_store::load_chat_storage(&state.user.history_path).unwrap();
            state.user.agent_history=history; state.user.session_store=storage;
            assert_eq!(route_agent_new(&mut state,"tb3-reopen").status,200);
            let run=prepare_agent_chat(&mut state,&json!({"message":case["reopen_request"]}).to_string(),"tb3-reopen").unwrap_or_else(|r|panic!("{}",r.body));
            assert_eq!(run.tutor.as_ref().unwrap()["binding"]["tutor_session_id"],current["context"]["binding"]["tutor_session_id"]);
            let mut reopened=context_packet(&state,&run,&delivery);
            reopened["presented_sources"]=prior["presented_sources"].clone();
            if reopening {
                let variant=if phase=="reopen" && matches!(case["id"].as_str(),Some("cannot-start"|"independent-step")) {"reopen-repair"} else {"reopen"};
                let dir=directory().join("sol").join(variant).join(case["id"].as_str().unwrap());
                let result=replay_turn(&mut state,run,&read(dir.join("input.json")),read(dir.join("output.json")),&reopened,phase=="reopen-initial");
                save(dir.join("replay.json"),&result);
            } else {
                save(directory().join("contexts").join(case["id"].as_str().unwrap()).join("reopen.json"),&reopened);
            }
        }
    }
}
