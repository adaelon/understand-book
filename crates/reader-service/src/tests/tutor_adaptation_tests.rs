//! TB3 authored public fixtures and explicit-input historical model replays.
use super::*;
use crate::tests::tutor_loop_tests::prepare_activity_chat as prepare_agent_chat;
use memory::teaching::TeachingFact;

pub fn directory() -> PathBuf {
    PathBuf::from(std::env::var_os("TB3_TRIAL_DIR")
        .expect("set TB3_TRIAL_DIR to the trial input/output directory; frozen replay needs scenarios.json and sol/"))
}
pub fn read(path: impl AsRef<Path>) -> Value {
    let path = path.as_ref();
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("cannot read Tutor replay input {}: {error}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|error| panic!("invalid Tutor replay JSON {}: {error}", path.display()))
}
pub fn save(path: impl AsRef<Path>, value: &Value) {
    std::fs::create_dir_all(path.as_ref().parent().unwrap()).unwrap();
    std::fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}
pub fn range() -> EvidenceRange { EvidenceRange { start_lid:"1.1".into(), end_lid:"1.1".into(), ranges:vec![] } }
pub fn cases() -> Vec<Value> {
    let fixture: Value = serde_json::from_str(include_str!("../../tests/fixtures/tutor-borrowing/adaptation.json")).unwrap();
    fixture["cases"].as_array().unwrap().clone()
}

pub struct Replay {
    output: Value,
    sources: Vec<Value>,
    source_references: RefCell<Vec<(String,String)>>,
    references: Vec<(String, String)>,
    previous_design_ref: Option<String>,
    requires_capability: bool,
    stage: std::cell::Cell<usize>,
}
impl Replay {
    fn rebound(&self, plan: &AgentRequestPlan) -> Value {
        let mut text = self.output.to_string();
        for (old, new) in &self.references { text = text.replace(old, new); }
        if self.source_references.borrow().is_empty() {
            for (index, source) in self.sources.iter().enumerate() {
                let id = format!("source-{index}");
                let result = plan.input.iter().rev().find(|m|m.tool_call_id.as_deref()==Some(&id)).unwrap();
                let body: Value = serde_json::from_str(result.content.as_deref().unwrap()).unwrap();
                let actual=body["model_body"]["source_ref_id"].as_str().unwrap_or_else(||panic!("current source was not accepted: {body}"));
                self.source_references.borrow_mut().push((source["source_ref_id"].as_str().unwrap().into(),actual.into()));
            }
        }
        for (old,new) in self.source_references.borrow().iter() { text=text.replace(old,new); }
        serde_json::from_str(&text).unwrap()
    }
}
impl ModelAdapter for Replay {
    fn complete(&self, _: CompletionRequest) -> Result<ParsedResponse, AdapterError> { unreachable!("local replay has no model helpers") }
    fn complete_structured(&self, request: CompletionRequest) -> Result<Value, AdapterError> {
        if let Some(guidance)=super::tutor_loop_tests::fixture_coach_guidance(&request) { return Ok(guidance); }
        if let Some(review)=super::tutor_loop_tests::fixture_delivery_review(&request) { return Ok(review); }
        unreachable!("local replay has no other model helpers")
    }
    fn chat(&self, plan: &AgentRequestPlan) -> Result<AssistantTurn, AdapterError> {
        let stage = self.stage.get(); self.stage.set(stage+1);
        let call = |id: &str, name: &str, arguments: Value| {
            let capability = arguments["move"]["capability"].clone();
            let mut input = super::tutor_loop_tests::fixture_model_input(arguments);
            if input["operation"] == "select" && self.requires_capability {
                input["move"]["capability"] = capability;
            }
            ToolCall { id:id.into(), name:name.into(), arguments:input.to_string() }
        };
        let has_evidence = self.output["evidence"].is_object();
        let (text, tool_calls) = match stage {
            0 => (None, vec![call("read", "book.text", json!({"lid":"1.1","end_lid":"1.1"}))]),
            1 => (None, self.sources.iter().enumerate().map(|(i,s)|call(&format!("source-{i}"), "source.present", json!({"quote":s["quote"]}))).collect()),
            2 if has_evidence => (None, vec![call("interpret", "tutor.step", self.rebound(plan)["evidence"].clone())]),
            n if n == 2+usize::from(has_evidence) => {
                let output=self.rebound(plan);
                let target=&output["select"]["move"]["target"];
                (None,vec![call("design","tutor.step",json!({"operation":"design","operation_id":"borrowed-access","design":{
                    "target":target,"organizing_model":"One owned value and a borrowed access path remain distinct; identify the expression's type before tracking ownership.",
                    "representation":{"kind":"text","reason":"The expression's type and borrowed access can be compared directly in the short code example."},
                    "blocks":[{"block_id":"borrowed-access","relation":"A shared reference allows reading the owned value without transferring ownership.",
                        "purpose":target["expected_performance"],"source_bindings":target["source_bindings"]}],
                    "connections":[],"focus_block_refs":["borrowed-access"],"basis_refs":output["select"]["move"].get("interpretation_refs").cloned().unwrap_or(json!([])),
                    "open_question_refs":[],"previous_ref":self.previous_design_ref,"revision_reason":"Connect this learner response to the distinction between ownership and access."}}))])
            },
            n if n == 3+usize::from(has_evidence) => {
                let output=self.rebound(plan);
                let mut request=output["select"].clone();
                request["move"]["design_ref"]=super::tutor_continuation_tests::accepted_design(plan,"design")["design_ref"].clone();
                request["move"]["block_refs"]=json!(["borrowed-access"]);
                request["move"]["delivery_text"]=output["answer"].clone();
                super::tutor_loop_tests::fixture_participation(&mut request["move"]);
                (None,vec![call("select","tutor.step",request)])
            },
            n if n == 4+usize::from(has_evidence) => {
                let selected=plan.input.iter().rev().find(|message|message.tool_call_id.as_deref()==Some("select")).unwrap();
                let receipt:Value=serde_json::from_str(selected.content.as_deref().unwrap()).unwrap();
                assert!(receipt["model_body"]["selected"].is_string(),"frozen selection was not accepted: {receipt}");
                (Some(self.rebound(plan)["answer"].as_str().unwrap().into()), vec![])
            },
            _ => return Err(AdapterError {message:format!("Frozen output requires a repair: {:?}",plan.input.last()),spend_stop:None}),
        };
        Ok(AssistantTurn {provider_continuation:None,text,tool_calls,usage_total_tokens:None})
    }
}

pub fn seed(case: &Value) -> (tempfile::TempDir, AppState, Value, Value) {
    let (variant, case_id) = case["prior"].as_str().unwrap().split_once('/').unwrap();
    let (packet, output) = super::tutor_building_blocks_tests::public_replay(variant, case_id);
    seed_samples(case, super::tutor_building_blocks_tests::SOURCE, packet, output)
}

pub fn recorded_seed(case: &Value) -> (tempfile::TempDir, AppState, Value, Value) {
    let source_path = PathBuf::from(std::env::var_os("TB3_SOURCE")
        .expect("set TB3_SOURCE to the source.txt used by the frozen model replay"));
    let source = std::fs::read_to_string(&source_path)
        .unwrap_or_else(|error| panic!("cannot read TB3_SOURCE {}: {error}", source_path.display()));
    let prior_dir = PathBuf::from(std::env::var_os("TB3_PRIOR_DIR")
        .expect("set TB3_PRIOR_DIR to the prior TB2 replay directory containing before/, after/ and prerequisite-repair/"))
        .join(case["prior"].as_str().unwrap());
    seed_samples(case, &source, read(prior_dir.join("input.json")), read(prior_dir.join("output.json")))
}

fn seed_samples(case: &Value, source: &str, packet: Value, output: Value) -> (tempfile::TempDir, AppState, Value, Value) {
    let (root, mut state) = super::tutor_building_blocks_tests::fixture_with_source(source);
    super::session_runtime_tests::enable_jsonl(&mut state);
    assert_eq!(route_agent_new(&mut state, "tb3").status, 200);
    let run = prepare_agent_chat(&mut state, &json!({"message":packet["user_request"]}).to_string(), "tb3").unwrap_or_else(|r|panic!("{}",r.body));
    let turn = run.turn_ref.clone();
    let tutor_session = run.tutor.as_ref().unwrap()["binding"]["tutor_session_id"].as_str().unwrap().to_string();
    let adapter = Replay {output:output.clone(),sources:packet["presented_sources"].as_array().unwrap().clone(),source_references:Default::default(),references:vec![],
        requires_capability:!run.tutor.as_ref().unwrap()["current_action"].is_object(),
        previous_design_ref:run.tutor.as_ref().unwrap()["current_design"]["design_ref"].as_str().map(str::to_owned),stage:Default::default()};
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

pub fn context_packet(state: &AppState, run: &agent_run::PreparedAgentChat, delivery: &Value) -> Value {
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
    let (packet,mut output)=super::tutor_building_blocks_tests::public_replay("after","starting-ownership");
    output["select"]["move"]["actions"]=json!(["request_hint"]);
    output["select"]["move"]["hint"]=json!("先判断右侧是 Foo 还是 &Foo，再追踪所有者。");
    let run=prepare_agent_chat(&mut state,&json!({"message":packet["user_request"]}).to_string(),"tb3").unwrap_or_else(|r|panic!("{}",r.body));
    let turn=run.turn_ref.clone();
    let adapter=Replay {output:output.clone(),sources:packet["presented_sources"].as_array().unwrap().clone(),source_references:Default::default(),references:vec![],
        requires_capability:!run.tutor.as_ref().unwrap()["current_action"].is_object(),
        previous_design_ref:run.tutor.as_ref().unwrap()["current_design"]["design_ref"].as_str().map(str::to_owned),stage:Default::default()};
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
    assert_eq!(stored_fact.binding.session_revision,delivery.binding.session_revision,"legacy mode changes do not rebind a practice response or erase its help");
    assert_eq!(store.teaching_event(&evidence.assistance_refs[0]).unwrap().payload["text"],output["select"]["move"]["hint"]);
    assert!(store.learner_projection(&state.workspace.book.base.book_id).unwrap().0.unwrap().rows.is_empty());
}

#[test]
fn om3_assisted_expression_carries_original_help_into_the_next_unreviewed_turn() {
    let (_root, mut state) = super::teaching_design_tests::fixture();
    let first = prepare_agent_chat(&mut state, r#"{"message":"请解释为什么停车时间也在分母里。"}"#, "om3").unwrap();
    let design = super::teaching_design_tests::design_request("om3-initial", None);
    let mut support = super::teaching_design_tests::select_request("om3-show-time", "", false);
    support["move"]["delivery_text"] = json!("同一行程从出发计时到到达，行驶 10 分钟、停车 5 分钟，总时间就是 15 分钟。停车虽然不增加距离，仍在这个时间区间里。");
    let first_report = super::tutor_building_blocks_tests::om3_replay(&mut state, first, design, support, "assisted-setup");
    let delivery = first_report["delivery"]["event_id"].as_str().unwrap().to_owned();
    let design_ref = first_report["accepted_design"]["event_id"].as_str().unwrap().to_owned();
    let displayed = post(&mut state, "/tutor/display", &json!({"delivery_ref":delivery}).to_string());
    assert_eq!(displayed.status, 200, "{}", displayed.body);
    let words = "看了例子我明白：总时间包括停车，所以这里是 15 分钟。";
    let expression = prepare_agent_chat(&mut state, &json!({"message":words}).to_string(), "om3").unwrap();
    let fact = format!("usage:{}:user:{}", expression.tutor.as_ref().unwrap()["binding"]["tutor_session_id"].as_str().unwrap(), expression.turn_ref.turn_id);
    let evidence = teaching::step(&state.private_context(), &expression.turn_ref, json!({
        "operation":"evidence","nature":"hypothesis","operation_id":"om3-assisted",
        "target":super::teaching_design_tests::target(),"design_ref":design_ref,"block_refs":["total-time"],
        "fact_refs":[fact],"interpretation":"用户看过行程例子后复述总时间包括停车；独立应用未知。",
        "teaching_implication":"沿用原行程连接新条件；仅当当前目标需要且请求允许时检验未展示的应用。"
    }), &[], &[super::teaching_design_tests::range()]).unwrap();
    super::tutor_loop_tests::finish(&mut state, &expression.turn_ref, &super::tutor_loop_tests::outcome(None));
    let original = state.user.learning_store().unwrap().evidence(evidence["evidence_ref"].as_str().unwrap()).unwrap();
    let request = "继续，沿用上次例子，直接解释如果停车更久会怎样。";
    let next = prepare_agent_chat(&mut state, &json!({"message":request}).to_string(), "om3").unwrap();
    let continuity = &next.tutor.as_ref().unwrap()["learner_context"]["learning_opportunity"]["continuity"];
    assert!(continuity["previous_work"].as_array().unwrap().iter().any(|work| work["fact_refs"].as_array()
        .is_some_and(|refs| refs.contains(&json!(fact)))));
    let mut revised = super::teaching_design_tests::design_request("om3-new-condition", Some(&design_ref));
    revised["design"]["basis_refs"] = json!([evidence["evidence_ref"]]);
    revised["design"]["revision_reason"] = json!("已有受助表达可连接新条件；当前要求沿用例子直接讲解，暂不邀请独立应用。");
    let mut selection = super::teaching_design_tests::select_request("om3-longer-stop", "", false);
    selection["move"]["kind"] = json!("explain");
    selection["move"]["interaction_intent"] = json!("direct_explanation");
    selection["move"]["intent_origin"] = json!("current_request");
    selection["move"]["intent_quote"] = json!("沿用上次例子，直接解释如果停车更久会怎样");
    selection["move"]["interpretation_refs"] = json!([evidence["evidence_ref"]]);
    selection["move"]["participation"] = json!({"choice":"conclude","reason":"沿用受助例子直接回应新条件；独立应用仍未知。"});
    selection["move"]["delivery_text"] = json!("沿用刚才的行程，路程与行驶时间保持不变。停车从 5 分钟增加到 8 分钟，总时间就从 15 分钟增加到 18 分钟。平均速度用同一路程除以总时间；分子不变、正的分母增大，所以平均速度降低。");
    let report = super::tutor_building_blocks_tests::om3_replay(&mut state, next, revised, selection, "assisted-next-question");
    let main_input = report["main_input"].to_string();
    assert!(main_input.contains(evidence["evidence_ref"].as_str().unwrap()),
        "the original evidence reference must reach the actual main-model request");
    assert!(main_input.contains(words), "the original assisted expression must remain available");
    assert!(main_input.contains(&delivery), "the actual prior support must remain identifiable");
    assert!(report["prepare"]["learner_context"]["learning_opportunity"]["continuity"]["teaching_implications"]
        .as_array().unwrap().iter().any(|item| item["evidence_ref"] == evidence["evidence_ref"]));
    assert!(report["delivery"]["payload"]["move"]["response_expectation"].is_null());
    let store = state.user.learning_store().unwrap();
    assert_eq!(serde_json::to_value(store.evidence(evidence["evidence_ref"].as_str().unwrap()).unwrap()).unwrap(),
        serde_json::to_value(original).unwrap(), "new support preserves the original interpretation and help conditions");
    assert!(store.learner_projection(&state.workspace.book.base.book_id).unwrap().0.unwrap().rows.is_empty());
}

pub fn references(old: &Value, current: &Value) -> Vec<(String,String)> {
    let mut result=Vec::new();
    // Turn identities change between isolated fixtures; match immutable actual
    // learner expressions/deliveries, never model interpretation text.
    for old_fact in old["read_facts"].as_array().unwrap() {
        if let Some(new_fact)=current["read_facts"].as_array().unwrap().iter().find(|new_fact|
            new_fact["kind"]==old_fact["kind"]
            && new_fact["content"]["response"]==old_fact["content"]["response"]
            && new_fact["content"]["answer"]==old_fact["content"]["answer"])
        {
            result.push((old_fact["event_id"].as_str().unwrap().into(),new_fact["event_id"].as_str().unwrap().into()));
        }
    }
    result.push((old["prior_delivery"]["event_id"].as_str().unwrap().into(),current["prior_delivery"]["event_id"].as_str().unwrap().into()));
    result
}

pub fn replay_turn(state: &mut AppState, run: agent_run::PreparedAgentChat, packet: &Value, mut output: Value, current: &Value, capture_failure: bool) -> Value {
    let turn=run.turn_ref.clone();
    let session=run.tutor.as_ref().unwrap()["binding"]["tutor_session_id"].as_str().unwrap().to_string();
    let expected_evidence=output["evidence"]["operation_id"].as_str().map(|id|format!("interpretation:{id}"));
    let expected_refs=output["select"]["move"]["interpretation_refs"].clone();
    let references=references(packet,current);
    if let Some(fact_refs)=output.get_mut("evidence").and_then(|evidence|evidence.get_mut("fact_refs")).and_then(Value::as_array_mut) {
        let store=state.user.learning_store().unwrap();
        // Older experiment packets mix delivered help into fact_refs. Keep the
        // original interpretation, but adopt only actual expressions as its basis.
        fact_refs.retain(|reference| {
            let original=reference.as_str().unwrap();
            let actual=references.iter().find(|(old,_)|old==original).map(|(_,new)|new.as_str()).unwrap_or(original);
            store.teaching_event(actual).unwrap_or_else(|error| panic!("unmapped frozen fact {original}: {error:?}; references={references:?}")).learner_expression().is_some()
        });
        assert!(!fact_refs.is_empty(),"a frozen hypothesis still needs actual learner words");
    }
    let samples=Arc::new(std::sync::Mutex::new(Vec::new()));
    let adapter=super::tutor_continuation_tests::TutorRequestRecorder {samples:samples.clone(),directory:None,
        inner:Box::new(Replay {output,sources:packet["presented_sources"].as_array().unwrap().clone(),source_references:Default::default(),references,
            requires_capability:!run.tutor.as_ref().unwrap()["current_action"].is_object(),
            previous_design_ref:run.tutor.as_ref().unwrap()["current_design"]["design_ref"].as_str().map(str::to_owned),stage:Default::default()})};
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
#[ignore = "frozen model replay needs TB3_TRIAL_DIR, TB3_SOURCE and TB3_PRIOR_DIR; no Provider"]
fn tutor_tb3_sol_replay_and_reopen() {
    let phase=std::env::var("TB3_PHASE").unwrap_or_else(|_|"response".into());
    let reopening=phase.starts_with("reopen");
    for case in read(directory().join("scenarios.json"))["cases"].as_array().unwrap() {
        for variant in if reopening {vec!["after"]} else {vec!["before","after"]} {
            let (root,mut state,delivery,prior)=recorded_seed(case);
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
