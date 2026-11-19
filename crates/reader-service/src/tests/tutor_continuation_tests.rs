//! TC0: real storage/admission baselines and opt-in red contracts for TC1–TC6.
//! Run ignored tests explicitly; an assertion failure is the recorded missing behavior.
use super::*;
use memory::teaching::TeachingFact;
use std::sync::atomic::{AtomicUsize, Ordering};

#[path = "tool_usability_trials.rs"]
pub mod to14;

#[path = "selection_intent_tests.rs"]
pub mod selection_intent;

pub const SOURCE: &str = include_str!("fixtures/tutor-continuation/source.md");
pub fn scenario() -> Value {
    serde_json::from_str(include_str!("fixtures/tutor-continuation/scenario.json")).unwrap()
}

pub fn tc3_move(state: &AppState) -> Value {
    json!({"move_id":"explain","object_ids":[],"capability":"explanation","kind":"explain",
        "interaction_intent":"neutral","intent_origin":"neutral","prompt":"Use total distance divided by total time.",
        "target":{"learning_focus":"average speed","expected_performance":"Explain the denominator","capability":"explanation",
            "source_bindings":[{"source_id":state.workspace.book.base.book_id,"source_revision":state.workspace.book.source_fingerprint(),
                "start_lid":"1.1","end_lid":"1.1"}],"object_refs":[]},"continuation":scenario()["continuation"]})
}
pub fn tc3_select(state: &AppState, turn: &AgentTurnRef, mut movement: Value) -> Result<Value, read_tools::ToolError> {
    if movement["interaction_intent"].is_null() { movement["interaction_intent"]=json!("neutral"); movement["intent_origin"]=json!("current_action"); }
    super::tutor_loop_tests::step_with_design(&state.private_context(),turn,json!({"operation":"select","move":movement}),&[],
        &[EvidenceRange { start_lid:"1.1".into(),end_lid:"1.1".into(),ranges:vec![] }])
}
pub fn tc3_views(state: &mut AppState, turn: &AgentTurnRef) -> Value {
    post_ok(state,"/tutor/activities",json!({"session_id":turn.session_id,"turn_id":turn.turn_id}))
}

pub fn tc4_offer(state: &mut AppState) -> (AgentTurnRef, Value) {
    enable(state);
    let run = super::tutor_loop_tests::prepare_practice_chat(state, &json!({"message":"Explain average speed", "question_quote":quote()}).to_string(), "1000").unwrap_or_else(|r|panic!("{}",r.body));
    tc3_select(state, &run.turn_ref, tc3_move(state)).unwrap();
    super::tutor_loop_tests::finish(state, &run.turn_ref, &super::tutor_loop_tests::outcome(None));
    let view = tc3_views(state, &run.turn_ref)["activities"][0]["continuation"].clone();
    let input = json!({"message":"", "client_request_id":"choice", "session_id":run.turn_ref.session_id,
        "expected_chat_head":view["expected_chat_head"],
        "teaching_choice":{"delivery_ref":view["delivery_ref"],"option_id":view["continuation"]["recommended_option_id"]}});
    (run.turn_ref, input)
}

#[test]
fn continuation_tc6_recent_interactions_are_not_cognitive_todos() {
    let (_root, mut state) = fixture(true);
    let (origin, input) = tc4_offer(&mut state);
    let displayed = post_ok(&mut state,"/tutor/continuation-displayed",json!({
        "delivery_ref":input["teaching_choice"]["delivery_ref"],"option_ids":["compare"],"recommended_option_id":"compare"}));
    let run = prepare(&mut state,input);
    let context = run.tutor.as_ref().unwrap();
    let session = context["binding"]["tutor_session_id"].as_str().unwrap();
    let facts = state.user.learning_store().unwrap().teaching_recent(session,20).unwrap();
    assert!(facts.iter().any(|f| f.kind == TeachingFact::ContinuationChosen));
    assert!(facts.iter().any(|f| f.kind == TeachingFact::ContinuationDisplayed));
    let pending = context["learner_context"]["uninterpreted_fact_refs"].as_array().unwrap();
    for f in &facts {
        if f.kind != TeachingFact::UsageObserved || f.payload["response"].as_str().is_none_or(|s|s.trim().is_empty()) {
            assert!(!pending.contains(&json!(f.event_id)),"non-learner fact became cognitive todo: {}",f.event_id);
        }
    }
    let recent = &context["learner_context"]["recent_facts"];
    assert!(recent.as_array().unwrap().iter().any(|f|f["kind"]=="continuation_chosen" && f["continuation"]["option_id"]=="compare"));
    assert!(recent.as_array().unwrap().iter().any(|f|f["event_id"]==displayed["event_id"]));
    let current = recent.as_array().unwrap().iter().find(|f|f["binding"]["turn_id"]==run.turn_ref.turn_id && f["usage"]["kind"]=="reading_action").unwrap();
    assert_eq!(current["interaction"]["action"]["origin"],"ui_choice");
    assert_eq!(current["interaction"]["answer_ref"]["turn_id"],origin.turn_id);
    assert_eq!(current["response_preview"]["text"],"");
}

#[test]
fn continuation_tc6_assistant_delivery_alone_cannot_diagnose_learner() {
    let (_root, mut state) = fixture(true);
    let (origin, input) = tc4_offer(&mut state);
    let run = prepare(&mut state,input);
    let session = run.tutor.as_ref().unwrap()["binding"]["tutor_session_id"].as_str().unwrap();
    for fact in [format!("delivery:{}:explain",origin.turn_id),format!("usage:{session}:assistant:{}",origin.turn_id)] {
        let result = super::tutor_loop_tests::step_with_design(&state.private_context(),&run.turn_ref,json!({"operation":"evidence","nature":"hypothesis",
            "operation_id":format!("unsupported-{fact}"),"target":tc3_move(&state)["target"],"fact_refs":[fact],
            "interpretation":"The reader cannot calculate.","teaching_implication":"Teach arithmetic prerequisites."}),&[],
            &[EvidenceRange {start_lid:"1.1".into(),end_lid:"1.1".into(),ranges:vec![]}]);
        assert!(result.is_err(),"assistant explanation alone became a learner diagnosis");
    }
}

#[test]
fn continuation_tc6_repeated_choices_and_reads_leave_learning_and_profile_unchanged() {
    let (_root, mut state) = fixture(true);
    let (mut origin, mut input) = tc4_offer(&mut state);
    let source = state.workspace.book.base.book_id.clone();
    let before = state.user.learning_store().unwrap().learner_projection(&source).unwrap();
    let profile_before = serde_json::to_value(state.user.store.profile_facts()).unwrap();
    for i in 0..3 {
        // Reads and no choice are observations only.
        for _ in 0..3 { tc3_views(&mut state,&origin); }
        input["client_request_id"] = json!(format!("repeat-{i}"));
        let run = prepare(&mut state,input.clone());
        assert_eq!(post_ok(&mut state,"/agent/reading-action-status",input.clone())["turn_id"],run.turn_ref.turn_id);
        let mut movement = tc3_move(&state);
        movement["kind"] = json!("compare");
        movement["interaction_intent"] = json!("neutral");
        movement["intent_origin"] = json!("current_action");
        tc3_select(&state,&run.turn_ref,movement).unwrap();
        super::tutor_loop_tests::finish(&mut state,&run.turn_ref,&super::tutor_loop_tests::outcome(None));
        origin = run.turn_ref;
        let view = tc3_views(&mut state,&origin)["activities"][0]["continuation"].clone();
        input["expected_chat_head"] = view["expected_chat_head"].clone();
        input["teaching_choice"]["delivery_ref"] = view["delivery_ref"].clone();
    }
    let store=state.user.learning_store().unwrap();
    assert_eq!(store.learner_projection(&source).unwrap(),before);
    assert!(store.evidence_page(&source,0,100).unwrap().is_empty());
    drop(store);
    assert_eq!(serde_json::to_value(state.user.store.profile_facts()).unwrap(),profile_before);
    let session=&state.user.agent_history.sessions[0];
    let job:memory::ReviewJob=serde_json::from_value(json!({"job_id":"tc6","session_id":session.id,"book_id":source,
        "from_turn_exclusive":0,"to_turn_inclusive":session.turns.len(),"status":"queued","attempts":0,"created_at":"0","updated_at":"0"})).unwrap();
    let review=host::copy_user_review_input(&state.user,&job,"technical_learning").unwrap();
    assert_eq!(review.turns.len(),4);
    for turn in &review.turns[1..] { assert_eq!(turn.user,""); assert!(turn.input_action.is_some()); }
    let pending=pending_review_context(&state.user,&source,state.workspace.learning_workspace_id.as_deref());
    assert_eq!(pending.len(),1); assert_eq!(pending[0].text,"Explain average speed");
}

#[test]
fn continuation_tc6_answer_actions_preserve_authorship_and_require_real_expression() {
    for action in ["clarify_answer","continue_answer"] {
        let (_root, mut state) = fixture(true);
        let (origin, _) = tc4_offer(&mut state);
        let input=json!({"message":"","client_request_id":"answer-action","session_id":origin.session_id,
            "expected_chat_head":origin.turn_id,"entry_action":action,"answer_ref":{"turn_id":origin.turn_id}});
        // Use the actual stored answer; the assistant excerpt is never user text.
        let run=prepare(&mut state,input);
        let context=run.tutor.as_ref().unwrap();
        assert_eq!(context["learner_context"]["current_action"]["answer_ref"]["turn_id"],origin.turn_id);
        assert_eq!(context["learner_context"]["current_request"],"");
        let fact=format!("usage:{}:user:{}",context["binding"]["tutor_session_id"].as_str().unwrap(),run.turn_ref.turn_id);
        assert!(!context["learner_context"]["uninterpreted_fact_refs"].as_array().unwrap().contains(&json!(fact)));
        let request=json!({"operation":"evidence","nature":"hypothesis","operation_id":"button-diagnosis",
            "target":tc3_move(&state)["target"],"fact_refs":[fact],"interpretation":"Missing arithmetic prerequisites.","teaching_implication":"Teach multiplication."});
        assert!(super::tutor_loop_tests::step_with_design(&state.private_context(),&run.turn_ref,request,&[],
            &[EvidenceRange{start_lid:"1.1".into(),end_lid:"1.1".into(),ranges:vec![]}]).is_err());
        super::tutor_loop_tests::finish(&mut state,&run.turn_ref,&super::tutor_loop_tests::outcome(None));
        let correction="不是不懂计算，是不懂停顿为什么也算在总时间里。";
        let next=prepare(&mut state,json!({"message":correction,"client_request_id":"real-correction","session_id":origin.session_id,
            "expected_chat_head":run.turn_ref.turn_id,"entry_action":action,"answer_ref":{"turn_id":origin.turn_id}}));
        let context=next.tutor.as_ref().unwrap();
        let fact=format!("usage:{}:user:{}",context["binding"]["tutor_session_id"].as_str().unwrap(),next.turn_ref.turn_id);
        assert!(context["learner_context"]["uninterpreted_fact_refs"].as_array().unwrap().contains(&json!(fact)));
        let request=json!({"operation":"evidence","nature":"hypothesis","operation_id":"real-correction",
            "target":tc3_move(&state)["target"],"fact_refs":[fact],"interpretation":"The question concerns elapsed time including stops.","teaching_implication":"Compare elapsed and moving time."});
        super::tutor_loop_tests::step_with_design(&state.private_context(),&next.turn_ref,request,&[],
            &[EvidenceRange{start_lid:"1.1".into(),end_lid:"1.1".into(),ranges:vec![]}]).unwrap();
        assert_eq!(state.user.learning_store().unwrap().evidence("interpretation:real-correction").unwrap().learner_quote,correction);
        let pending=pending_review_context(&state.user,&state.workspace.book.base.book_id,state.workspace.learning_workspace_id.as_deref());
        assert!(pending.iter().any(|p|p.text==correction));
        assert!(!pending.iter().any(|p|p.text.contains("操作：")));
    }
}

#[test]
fn continuation_tc6_source_change_excludes_old_interpretations() {
    let (_root,mut state)=fixture(true); enable(&mut state);
    let run=prepare(&mut state,json!({"message":"Why does stopped time count?"}));
    let context=run.tutor.as_ref().unwrap();
    let fact=format!("usage:{}:user:{}",context["binding"]["tutor_session_id"].as_str().unwrap(),run.turn_ref.turn_id);
    let target=tc3_move(&state)["target"].clone();
    let request=json!({"operation":"evidence","nature":"hypothesis","operation_id":"old-revision","target":target,
        "fact_refs":[fact],"interpretation":"The reader asks about stopped time.","teaching_implication":"Explain elapsed duration."});
    super::tutor_loop_tests::step_with_design(&state.private_context(),&run.turn_ref,request,&[],
        &[EvidenceRange{start_lid:"1.1".into(),end_lid:"1.1".into(),ranges:vec![]}]).unwrap();
    super::tutor_loop_tests::finish(&mut state,&run.turn_ref,&super::tutor_loop_tests::outcome(None));
    state.workspace.book=Arc::new(Book::new(state.workspace.book.base.clone(),&format!("{SOURCE}\nNew source revision.")));
    let next=prepare(&mut state,json!({"message":"Explain the current material"}));
    assert!(next.tutor.as_ref().unwrap()["learner_context"]["interpretations"].as_array().unwrap().is_empty());
    let mut movement=tc3_move(&state); movement["interpretation_refs"]=json!(["interpretation:old-revision"]);
    assert!(tc3_select(&state,&next.turn_ref,movement).is_err());
}

#[test]
#[ignore = "TC6 real model correction trajectory; set TC6_LIVE_OUTPUT and provider environment"]
fn continuation_tc6_live_correction_trajectory() {
    let directory=std::path::PathBuf::from(std::env::var("TC6_LIVE_OUTPUT").unwrap());
    assert!(!directory.exists()); std::fs::create_dir_all(&directory).unwrap();
    let config=ProviderConfig::from_env().unwrap();
    let (_root,mut state)=fixture(true); enable(&mut state);
    let original=prepare(&mut state,json!({"message":"我没看明白，平均速度是除法，为什么乘回总时间就能还原路程？","question_quote":quote()}));
    let context=original.tutor.as_ref().unwrap();
    let session=context["binding"]["tutor_session_id"].as_str().unwrap().to_owned();
    let old_ref="interpretation:tc6-prior";
    let prior=json!({"operation":"evidence","nature":"hypothesis","operation_id":"tc6-prior",
        "target":tc3_move(&state)["target"],"fact_refs":[format!("usage:{session}:user:{}",original.turn_ref.turn_id)],
        "interpretation":"读者当前在问除法定义与乘法回代的连接，可能需要拆开等式变形。",
        "teaching_implication":"先用除法与乘法互逆解释回代，再用一组数字演示。"});
    super::tutor_loop_tests::step_with_design(&state.private_context(),&original.turn_ref,prior.clone(),&[],
        &[EvidenceRange{start_lid:"1.1".into(),end_lid:"1.1".into(),ranges:vec![]}]).unwrap();
    let mut movement=tc3_move(&state); movement["interpretation_refs"]=json!([old_ref]);
    tc3_select(&state,&original.turn_ref,movement).unwrap();
    let original_text=std::fs::read_to_string("../../docs/performance/tutor-continuation-tc5/live/clarify/answer.md").unwrap();
    let mut result=super::tutor_loop_tests::outcome(None);
    result.answer=Some(original_text.clone()); result.answer_view.as_mut().unwrap().parts=vec![AgentAnswerPart::Markdown{text:original_text.clone()}];
    super::tutor_loop_tests::finish(&mut state,&original.turn_ref,&result);
    let input=json!({"message":"不是不懂计算；乘除互逆我已经明白了。我卡的是：停下来时没走路，为什么停顿还要算进分母？请只拆开这一点，不要再重复等式变形。",
        "client_request_id":"tc6-correction","session_id":original.turn_ref.session_id,"expected_chat_head":original.turn_ref.turn_id,
        "entry_action":"clarify_answer","answer_ref":{"turn_id":original.turn_ref.turn_id,"excerpt":"分母代表的时段，是不是正好覆盖了分子那段路程"}});
    std::fs::write(directory.join("configuration.json"),serde_json::to_vec_pretty(&json!({"model":config.model,"source":SOURCE,
        "prior_hypothesis":prior,"prior_answer":original_text,"seed":"deterministic prior hypothesis and replayed TC5 real answer",
        "reader":"scripted correction, no human participant"})).unwrap()).unwrap();
    std::fs::write(directory.join("input.json"),serde_json::to_vec_pretty(&input).unwrap()).unwrap();
    let source=state.workspace.book.base.book_id.clone();
    let before=state.user.learning_store().unwrap().learner_projection(&source).unwrap();
    let run=prepare(&mut state,input);
    std::fs::write(directory.join("learner-context.json"),serde_json::to_vec_pretty(&run.tutor).unwrap()).unwrap();
    let samples=Arc::new(std::sync::Mutex::new(Vec::new()));
    let adapter=TutorRequestRecorder{samples:samples.clone(),directory:Some(directory.clone()),
        inner:ProviderRegistry::adapter_from_config_with_timeout(config,Duration::from_secs(120))};
    let result=agent_run::execute_prepared(&agent_run::BorrowedAppPort(RefCell::new(&mut state)),&adapter,run,Default::default());
    let saved=state.user.agent_history.sessions[0].turns.last().unwrap();
    let store=state.user.learning_store().unwrap();
    let evidence=store.evidence_page(&source,0,100).unwrap();
    let projection=store.learner_projection(&source).unwrap();
    std::fs::write(directory.join("result.json"),serde_json::to_vec_pretty(&json!({"http_status":result.reply.status,
        "reply":serde_json::from_str::<Value>(&result.reply.body).unwrap(),"turn":saved,"provider_calls":samples.lock().unwrap().len(),
        "facts":store.teaching_events(&session,0,100).unwrap(),"evidence":evidence,"projection_before":before,"projection_after":projection})).unwrap()).unwrap();
    if let Some(answer)=saved.outcome.as_ref().and_then(|o|o.answer.as_ref()) {std::fs::write(directory.join("answer.md"),answer).unwrap();}
    println!("TC6_LIVE status={} samples={} superseded={}",result.reply.status,samples.lock().unwrap().len(),store.evidence_is_superseded(old_ref).unwrap());
    assert_eq!(saved.status,AgentAssistantStatus::Completed);
    assert!(!saved.outcome.as_ref().unwrap().incomplete);
    assert!(store.evidence_is_superseded(old_ref).unwrap(),"real correction must revise the previous hypothesis");
    let current=store.current_interpretations(&source,state.workspace.book.source_fingerprint(),0,20).unwrap();
    assert!(current.iter().all(|(_,e)|e.evidence_id!=old_ref));
    let deliveries=store.teaching_deliveries(&state.user.agent_history.sessions[0].id,&saved.turn_id).unwrap();
    assert!(deliveries.iter().any(|d| d.payload["move"]["interpretation_refs"].as_array().is_some_and(|refs|
        current.iter().any(|(_,e)|e.supersedes.as_deref()==Some(old_ref) && refs.contains(&json!(e.evidence_id))))));
    assert_eq!(projection.0.unwrap().rows,before.0.unwrap().rows);
}

#[test]
fn continuation_tc4_local_choice_is_consumed_once_and_keeps_goal() {
    let (_root, mut state) = fixture(true);
    let (origin, input) = tc4_offer(&mut state);
    assert_eq!(tc3_views(&mut state, &origin)["activities"][0]["continuation"]["availability"]["status"], "eligible");
    let old_goal = state.user.agent_history.sessions[0].turns[0].goal_ref.clone().unwrap();
    let run = prepare(&mut state, input.clone());
    assert_eq!(run.goal.as_ref().unwrap().related_goal_id.as_deref(), Some(old_goal.id.as_str()));
    let repeat = post_ok(&mut state, "/agent/chat", input.clone());
    assert_eq!(repeat["turn_id"], run.turn_ref.turn_id);
    let view = tc3_views(&mut state, &origin);
    assert_eq!(view["activities"][0]["continuation"]["availability"], json!({"status":"consumed","accepted_turn_id":run.turn_ref.turn_id}));
    let turn = &state.user.agent_history.sessions[0].turns[1];
    assert!(turn.teaching_ref.is_none());
    let binding = &run.tutor.as_ref().unwrap()["binding"]["tutor_session_id"];
    let events = state.user.learning_store().unwrap().teaching_turn_events(binding.as_str().unwrap(), &run.turn_ref.turn_id).unwrap();
    assert_eq!(events.iter().filter(|e| e.kind == TeachingFact::ContinuationChosen).count(), 1);
    assert!(!events.iter().any(|e| e.kind == TeachingFact::LearnerAction));
    let mut changed = input.clone(); changed["teaching_choice"]["option_id"] = json!("other");
    assert!(prepare_agent_chat(&mut state, &changed.to_string(), "1001").err().unwrap().body.contains("REQUEST_KEY_CONFLICT"));
    changed["client_request_id"] = json!("another-key");
    changed["teaching_choice"]["option_id"] = json!("example");
    assert!(prepare_agent_chat(&mut state, &changed.to_string(), "1001").is_err());
}

#[test]
fn continuation_tc5_display_records_visible_subset_once_without_consuming() {
    let (_root, mut state) = fixture(true);
    let (origin, choice) = tc4_offer(&mut state);
    let input = json!({"delivery_ref":choice["teaching_choice"]["delivery_ref"],
        "option_ids":["compare"],"recommended_option_id":"compare"});
    let first = post_ok(&mut state,"/tutor/continuation-displayed",input.clone());
    assert_eq!(first,post_ok(&mut state,"/tutor/continuation-displayed",input.clone()));
    let event = state.user.learning_store().unwrap().teaching_event(first["event_id"].as_str().unwrap()).unwrap();
    assert_eq!(event.kind,TeachingFact::ContinuationDisplayed);
    assert_eq!(event.payload["option_ids"],json!(["compare"]));
    for ids in [json!([]),json!(["compare","compare"]),json!(["unknown"])] {
        let mut bad = input.clone(); bad["option_ids"] = ids;
        assert_ne!(post(&mut state,"/tutor/continuation-displayed",&bad.to_string()).status,200);
    }
    assert_eq!(tc3_views(&mut state,&origin)["activities"][0]["continuation"]["availability"]["status"],"eligible");
    let run = prepare(&mut state,choice);
    assert!(run.turn_ref.turn_id != origin.turn_id);
}

#[test]
fn continuation_tc5_local_status_never_accepts_or_executes() {
    let (_root,mut state) = fixture(true);
    let (_,input) = tc4_offer(&mut state);
    let probe = Probe::default(); state.services.adapter = Box::new(probe.clone());
    assert_eq!(post_ok(&mut state,"/agent/reading-action-status",input.clone()),json!({"accepted":false}));
    assert_eq!(state.user.agent_history.sessions[0].turns.len(),1);
    let run = prepare(&mut state,input.clone());
    assert_eq!(post_ok(&mut state,"/agent/reading-action-status",input.clone())["turn_id"],run.turn_ref.turn_id);
    let mut changed=input; changed["teaching_choice"]["option_id"]=json!("example");
    assert!(post(&mut state,"/agent/reading-action-status",&changed.to_string()).body.contains("REQUEST_KEY_CONFLICT"));
    assert_eq!(probe.0.load(Ordering::SeqCst),0);
}

#[test]
#[ignore = "TC5 real material correction; set fresh TC5_LIVE_OUTPUT"]
fn continuation_tc5_live_correction() {
    let directory=std::path::PathBuf::from(std::env::var("TC5_LIVE_OUTPUT").unwrap());
    assert!(!directory.exists()); std::fs::create_dir_all(&directory).unwrap();
    let config=ProviderConfig::from_env().unwrap();
    let (_root,mut state)=fixture(true); enable(&mut state);
    let original=prepare(&mut state,json!({"message":"请讲清楚平均速度定义，重点解释总时间为什么在分母。","question_quote":quote()}));
    tc3_select(&state,&original.turn_ref,tc3_move(&state)).unwrap();
    let original_text=std::fs::read_to_string("../../docs/performance/tutor-continuation-tc3/live-v3/explain/answer.md").unwrap();
    let mut result=super::tutor_loop_tests::outcome(None);
    result.answer=Some(original_text.clone()); result.answer_view.as_mut().unwrap().parts=vec![AgentAnswerPart::Markdown {text:original_text.clone()}];
    super::tutor_loop_tests::finish(&mut state,&original.turn_ref,&result);
    std::fs::write(directory.join("configuration.json"),serde_json::to_vec_pretty(&json!({"model":config.model,"source":SOURCE,
        "prior_answer":original_text,"prior_continuation":scenario()["continuation"],"reader":"scripted correction, no human participant"})).unwrap()).unwrap();
    for (name,supplement,excerpt) in [("clarify","",Some("平均速度 × 总时间 应当还原出总路程")),
        ("redirect","不是不懂计算。我是不明白：停下来时根本没走路，为什么停下来的时间还要算进分母？请只拆开这一点。",None)] {
        let dir=directory.join(name); std::fs::create_dir_all(&dir).unwrap();
        let samples=Arc::new(std::sync::Mutex::new(Vec::new()));
        state.services.adapter=Box::new(TutorRequestRecorder {samples:samples.clone(),directory:Some(dir.clone()),
            inner:ProviderRegistry::adapter_from_config_with_timeout(config.clone(),Duration::from_secs(120))});
        let head=state.user.agent_history.sessions[0].turns.last().unwrap().turn_id.clone();
        let input=if name=="clarify" {json!({"message":supplement,"session_id":original.turn_ref.session_id,
            "client_request_id":name,"entry_action":"clarify_answer","answer_ref":{"turn_id":original.turn_ref.turn_id,"excerpt":excerpt},"expected_chat_head":head})}
            else {json!({"message":supplement})};
        std::fs::write(dir.join("input.json"),serde_json::to_vec_pretty(&input).unwrap()).unwrap();
        let reply=post(&mut state,"/agent/chat",&input.to_string());
        let saved=state.user.agent_history.sessions[0].turns.last().unwrap();
        std::fs::write(dir.join("result.json"),serde_json::to_vec_pretty(&json!({"http_status":reply.status,"reply":serde_json::from_str::<Value>(&reply.body).unwrap(),
            "turn":saved,"provider_calls":samples.lock().unwrap().len()})).unwrap()).unwrap();
        if let Some(answer)=saved.outcome.as_ref().and_then(|o|o.answer.as_ref()) {std::fs::write(dir.join("answer.md"),answer).unwrap();}
        println!("TC5_LIVE {name}: status={} samples={}",reply.status,samples.lock().unwrap().len());
    }
}

#[test]
fn continuation_tc4_local_two_tabs_accept_only_one_choice() {
    let (_root, mut state) = fixture(true);
    let (_, input) = tc4_offer(&mut state);
    let state = Arc::new(std::sync::Mutex::new(state));
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let handles: Vec<_> = ["compare","example"].into_iter().map(|option| {
        let state = state.clone(); let barrier = barrier.clone(); let mut input = input.clone();
        input["client_request_id"] = json!(option); input["teaching_choice"]["option_id"] = json!(option);
        std::thread::spawn(move || { barrier.wait(); prepare_agent_chat(&mut state.lock().unwrap(), &input.to_string(),"1000").is_ok() })
    }).collect();
    assert_eq!(handles.into_iter().map(|h| usize::from(h.join().unwrap())).sum::<usize>(),1);
    assert_eq!(state.lock().unwrap().user.agent_history.sessions[0].turns.len(),2);
}

#[test]
fn continuation_tc4_local_restarts_repair_facts_without_execution() {
    for action in ["choice", "explain_selection", "continue_answer", "clarify_answer"] {
        for point in ["accepted", "snapshot", "teaching_receipt", "prepared"] {
            let (_root, mut state) = fixture(true);
            let (origin, mut input) = tc4_offer(&mut state);
            if action == "explain_selection" { input = selection_input(&state, "action"); }
            else if action != "choice" {
                input.as_object_mut().unwrap().remove("teaching_choice");
                input["entry_action"] = json!(action);
                input["answer_ref"] = json!({"turn_id":origin.turn_id});
            }
            interaction_input::LOCAL_FAIL_AFTER.with(|p| p.set(Some(point)));
            let result = prepare_agent_chat(&mut state, &input.to_string(), "1000");
            interaction_input::LOCAL_FAIL_AFTER.with(|p| p.set(None));
            assert!(result.err().unwrap().body.contains("INJECTED_CRASH"), "{action}/{point}");
            let accepted = state.user.agent_history.sessions[0].turns[1].turn_id.clone();
            let (mut history, store) = session_store::load_chat_storage(&state.user.history_path).unwrap();
            let mut store = store.unwrap();
            session_runtime::recover(&mut store, &mut history, &BTreeSet::new()).unwrap();
            state.user.agent_history = history; state.user.session_store = Some(store);
            let probe = Probe::default(); state.services.adapter = Box::new(probe.clone());
            // Different live control and source cannot rewrite an already accepted input.
            let view = state.user.learning_store().unwrap().state().unwrap();
            post_ok(&mut state, "/tutor/mutate", json!({"operation_id":"pause","expected_revision":view.control.revision,
                "action":{"kind":"pause","session_id":view.control.current_tutor_session_id}}));
            for _ in 0..2 { assert_eq!(post_ok(&mut state, "/agent/chat", input.clone())["turn_id"], accepted); }
            assert_eq!(probe.0.load(Ordering::SeqCst), 0);
            let turn = &state.user.agent_history.sessions[0].turns[1];
            assert_eq!(turn.error.as_ref().unwrap().error_code, "INTERRUPTED");
            if point != "accepted" { assert!(turn.domain.local_preparation.is_some()); }
            let store=state.user.learning_store().unwrap();
            let owner=store.teaching_event(&format!("binding:{}",if action=="choice" {&origin.turn_id} else {&accepted}));
            let events=owner.map(|bound|store.teaching_turn_events(&bound.binding.tutor_session_id,&accepted).unwrap()).unwrap_or_default();
            assert_eq!(events.iter().filter(|e| e.kind == TeachingFact::ContinuationChosen).count(), usize::from(action == "choice"));
            assert!(!events.iter().any(|e| e.kind == TeachingFact::LearnerAction));
            println!("TC4_EVIDENCE {}", json!({"host":"local","action":action,"fault":point,"request":input,
                "accepted_turn":accepted,"chosen_events":events.iter().filter(|e| e.kind==TeachingFact::ContinuationChosen).collect::<Vec<_>>(),
                "execution_starts":0,"provider_calls":probe.0.load(Ordering::SeqCst),"terminal":"INTERRUPTED"}));
        }
    }
}

#[test]
fn continuation_tc4_local_choice_boundaries_and_historical_answer() {
    for boundary in ["source", "head", "wrong_head", "foreign_chat", "scene"] {
        let (_root, mut state) = fixture(true);
        let (origin, mut input) = tc4_offer(&mut state);
        let before = state.user.agent_history.sessions[0].turns.len();
        match boundary {
            "pause" => {
                let view = state.user.learning_store().unwrap().state().unwrap();
                post_ok(&mut state, "/tutor/mutate", json!({"operation_id":"pause", "expected_revision":view.control.revision,
                    "action":{"kind":"pause", "session_id":view.control.current_tutor_session_id}}));
            },
            "off" | "revision" => {
                let view = state.user.learning_store().unwrap().state().unwrap();
                super::tutor_loop_tests::legacy_control(&mut state,  json!({"operation_id":"off","expected_revision":view.control.revision,
                    "action":{"kind":"set_enabled","enabled":false}}));
                if boundary == "revision" { super::tutor_loop_tests::legacy_control(&mut state,  json!({"operation_id":"on","expected_revision":view.control.revision+1,
                    "action":{"kind":"set_enabled","enabled":true}})); }
            },
            "source" => state.workspace.book = Arc::new(Book::new(state.workspace.book.base.clone(), &format!("{SOURCE}\nchanged"))),
            "head" => { let run = prepare(&mut state, json!({"message":"Another question"}));
                super::tutor_loop_tests::finish(&mut state, &run.turn_ref, &super::tutor_loop_tests::outcome(None)); },
            "wrong_head" => input["expected_chat_head"] = json!("wrong"),
            "foreign_chat" => { route_agent_new(&mut state, "1001"); },
            "scene" => input["presentation_follow_up"] = json!({"session_id":origin.session_id,"turn_id":"other","reference":{}}),
            _ => unreachable!(),
        }
        assert!(prepare_agent_chat(&mut state, &input.to_string(), "1002").is_err(), "{boundary}");
        assert_eq!(state.user.agent_history.sessions[0].turns.len(), before + usize::from(boundary == "head"));
        if boundary == "head" {
            let head = state.user.agent_history.sessions[0].turns.last().unwrap().turn_id.clone();
            let request = json!({"message":"", "session_id":origin.session_id, "client_request_id":"historical",
                "entry_action":"continue_answer", "answer_ref":{"turn_id":origin.turn_id}, "expected_chat_head":head});
            let run = prepare(&mut state, request);
            assert_eq!(run.tutor.unwrap()["current_action"]["answer_ref"]["turn_id"], origin.turn_id);
        }
    }
}

pub struct Tc4CrashProbe(Probe);

pub fn accepted_design(plan:&AgentRequestPlan, call_id:&str) -> Value {
    let message=plan.input.iter().rev().find(|message|message.tool_call_id.as_deref()==Some(call_id))
        .expect("the next sampling must contain the design acceptance");
    let result:Value=serde_json::from_str(message.content.as_deref().unwrap()).unwrap();
    let accepted=result["model_body"].clone();
    assert!(accepted["design_ref"].is_string(),"design was not accepted: {result}");
    accepted
}

pub struct ContinuationDelivery {
    movement: Value,
    previous_design_ref: Option<String>,
    stage: std::cell::Cell<usize>,
}
impl ModelAdapter for ContinuationDelivery {
    fn complete(&self,_:CompletionRequest)->Result<ParsedResponse,AdapterError> {unreachable!()}
    fn complete_structured(&self, request: CompletionRequest) -> Result<Value, AdapterError> {
        if let Some(guidance)=super::tutor_loop_tests::fixture_coach_guidance(&request) { return Ok(guidance); }
        if let Some(review)=super::tutor_loop_tests::fixture_delivery_review(&request) { return Ok(review); }
        unreachable!()
    }
    fn chat(&self,plan:&AgentRequestPlan)->Result<AssistantTurn,AdapterError> {
        let stage=self.stage.get();self.stage.set(stage+1);
        let call=|id:&str,name:&str,arguments:Value|AssistantTurn {provider_continuation:None,text:None,usage_total_tokens:Some(1),
            tool_calls:vec![ToolCall {id:id.into(),name:name.into(),arguments:arguments.to_string()}]};
        Ok(match stage {
            0=>call("read","book.text",json!({"lid":"1.1","end_lid":"1.1"})),
            1=>{
                let mut design=super::teaching_design_tests::design_request("continuation-design",self.previous_design_ref.as_deref());
                design["design"]["target"]=self.movement["target"].clone();
                design["design"]["blocks"][0]["source_bindings"]=self.movement["target"]["source_bindings"].clone();
                call("design","tutor.step",design)
            },
            2=>{
                let mut movement=self.movement.clone();
                movement["design_ref"]=accepted_design(plan,"design")["design_ref"].clone();
                movement["block_refs"]=json!(["total-time"]);
                super::tutor_loop_tests::fixture_participation(&mut movement);
                call("select","tutor.step",json!({"operation":"select","move":movement}))
            },
            3=>AssistantTurn {provider_continuation:None,text:Some("Equal distances need not take equal times. Compare the journeys using total distance divided by total time.".into()),tool_calls:vec![],usage_total_tokens:Some(1)},
            _=>return Err(AdapterError {message:format!("Continuation fixture exhausted: {:?}",plan.input.last()),spend_stop:None}),
        })
    }
}
pub fn tc4_delivery(target:Value, previous_design_ref:Option<String>) -> ContinuationDelivery {
    ContinuationDelivery {movement:json!({"move_id":"chosen-comparison","object_ids":[],"kind":"compare",
        "interaction_intent":"neutral","intent_origin":"current_action","target":target,
        "prompt":"Compare equal-distance journeys using total distance and total time."}),previous_design_ref,stage:Default::default()}
}

impl ModelAdapter for Tc4CrashProbe {
    fn complete(&self, _:CompletionRequest) -> Result<ParsedResponse,AdapterError> { unreachable!() }
    fn complete_structured(&self, request:CompletionRequest) -> Result<Value,AdapterError> {
        if let Some(guidance)=super::tutor_loop_tests::fixture_coach_guidance(&request) { return Ok(guidance); }
        unreachable!()
    }
    fn chat(&self, _: &AgentRequestPlan) -> Result<AssistantTurn,AdapterError> {
        self.0.0.fetch_add(1,Ordering::SeqCst);
        panic!("TC4 host disappears after provider started");
    }
}

#[test]
fn continuation_tc4_local_started_execution_is_not_replayed() {
    for action in ["choice", "explain_selection", "continue_answer", "clarify_answer"] {
        let (_root, mut state) = fixture(true);
        let (origin, mut input) = tc4_offer(&mut state);
        if action == "explain_selection" { input = selection_input(&state,"started"); }
        else if action != "choice" {
            input.as_object_mut().unwrap().remove("teaching_choice");
            input["entry_action"] = json!(action); input["answer_ref"] = json!({"turn_id":origin.turn_id});
        }
        let probe = Probe::default(); state.services.adapter = Box::new(Tc4CrashProbe(probe.clone()));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| post(&mut state,"/agent/chat",&input.to_string())));
        assert!(result.is_err()); assert_eq!(probe.0.load(Ordering::SeqCst),1);
        let (mut history, store) = session_store::load_chat_storage(&state.user.history_path).unwrap();
        let mut store = store.unwrap(); session_runtime::recover(&mut store,&mut history,&BTreeSet::new()).unwrap();
        state.user.agent_history = history; state.user.session_store = Some(store);
        let accepted = state.user.agent_history.sessions[0].turns[1].turn_id.clone();
        state.services.adapter = Box::new(probe.clone());
        assert_eq!(post_ok(&mut state,"/agent/chat",input)["turn_id"], accepted);
        assert_eq!(probe.0.load(Ordering::SeqCst),1);
        println!("TC4_EVIDENCE {}", json!({"host":"local","action":action,"fault":"provider_started",
            "accepted_turn":accepted,"execution_starts":1,"provider_calls":1,"terminal":"INTERRUPTED"}));
    }
}

#[test]
fn continuation_tc4_local_answer_identity_and_open_goal() {
    let (_root, mut state) = fixture(true);
    let (origin, choice) = tc4_offer(&mut state);
    let mut goal = state.user.agent_history.sessions[0].goals[0].clone();
    goal.status = runtime::goal::GoalStatus::Open; goal.revision += 1;
    session_runtime::append(&mut state.user, &origin,"1000",session_event::EventBody::GoalUpdated { goal:goal.clone() }).unwrap();
    let run = prepare(&mut state, choice);
    assert_eq!(run.goal.as_ref().unwrap().id, goal.id);
    assert_eq!(run.tutor.as_ref().unwrap()["current_action"]["action"]["origin"], "ui_choice");
    assert_eq!(runtime::tutor::action_intent(run.tutor.as_ref().unwrap()),None,"a choice keeps its selected content but does not dictate teaching intent");
    let mut movement = tc3_move(&state);
    movement.as_object_mut().unwrap().remove("continuation"); movement["kind"] = json!("compare");
    movement["interaction_intent"]=json!("neutral"); movement["intent_origin"]=json!("current_action");
    tc3_select(&state,&run.turn_ref,movement).unwrap();
    super::tutor_loop_tests::finish(&mut state,&run.turn_ref,&super::tutor_loop_tests::outcome(None));
    let next = json!({"message":"", "session_id":origin.session_id, "client_request_id":"answer",
        "entry_action":"clarify_answer", "answer_ref":{"turn_id":origin.turn_id}, "expected_chat_head":run.turn_ref.turn_id});
    let accepted = prepare(&mut state,next.clone());
    for change in ["action","answer","excerpt","typed"] {
        let mut changed = next.clone();
        match change {
            "action" => changed["entry_action"] = json!("continue_answer"),
            "answer" => changed["answer_ref"]["turn_id"] = json!(run.turn_ref.turn_id),
            "typed" => { changed.as_object_mut().unwrap().remove("entry_action"); changed.as_object_mut().unwrap().remove("answer_ref"); changed["message"] = json!("A different free question"); },
            _ => changed["answer_ref"]["excerpt"] = json!("any excerpt"),
        }
        assert!(prepare_agent_chat(&mut state,&changed.to_string(),"1001").err().unwrap().body.contains("REQUEST_KEY_CONFLICT"));
    }
    assert_eq!(post_ok(&mut state,"/agent/chat",next)["turn_id"], accepted.turn_ref.turn_id);
}

#[test]
fn continuation_tc3_optional_set_delivers_once_and_reads_without_provider() {
    for options in 0..=2 {
        let (_root,mut state)=fixture(true); enable(&mut state);
        let probe=Probe::default(); state.services.adapter=Box::new(probe.clone());
        let run=prepare_practice(&mut state,json!({"message":"Explain average speed"}));
        let mut movement=tc3_move(&state);
        movement["hint"]=json!("PRIVATE_HELP"); movement["explanation"]=json!("PRIVATE_ANSWER");
        if options==0 { movement.as_object_mut().unwrap().remove("continuation"); }
        else { movement["continuation"]["options"].as_array_mut().unwrap().truncate(options); }
        let selected=tc3_select(&state,&run.turn_ref,movement.clone()).unwrap();
        assert_eq!(tc3_views(&mut state,&run.turn_ref)["activities"],json!([]));
        // Persist the answer without its teaching receipt: reads repair only that receipt.
        let mut result=super::tutor_loop_tests::outcome(None);
        result.teaching_delivery_move_refs=serde_json::from_value(selected["delivery_move_refs"].clone()).unwrap();
        finalize_agent_turn_completed(&mut state,&run.turn_ref,&result,&[],"now").unwrap();
        let first=tc3_views(&mut state,&run.turn_ref);
        assert_eq!(first,tc3_views(&mut state,&run.turn_ref));
        assert_eq!(first["activities"].as_array().unwrap().len(),1);
        let view=&first["activities"][0];
        if options==0 { assert!(view["continuation"].is_null()); }
        else {
            let public:runtime::tutor_continuation::TutorContinuationView=serde_json::from_value(view["continuation"].clone()).unwrap();
            assert_eq!(serde_json::to_value(public.continuation).unwrap(),movement["continuation"]);
            assert_eq!(public.origin_turn_id,run.turn_ref.turn_id);
            assert_eq!(public.expected_chat_head,run.turn_ref.turn_id);
            assert_eq!(public.availability,runtime::tutor_continuation::ContinuationAvailability::Eligible);
        }
        assert!(!first.to_string().contains("PRIVATE_"));
        for key in ["hint","explanation","assessment"] { assert!(view["move"].get(key).is_none()); }
        let store=state.user.learning_store().unwrap();
        let id=run.tutor.as_ref().unwrap()["binding"]["tutor_session_id"].as_str().unwrap().to_owned();
        let events=store.teaching_turn_events(&id,&run.turn_ref.turn_id).unwrap();
        assert_eq!(events.iter().filter(|e| e.kind==TeachingFact::MoveSelected).count(),1);
        assert_eq!(events.iter().filter(|e| e.kind==TeachingFact::MessageDelivered).count(),1);
        assert!(!events.iter().any(|e| matches!(e.kind,TeachingFact::LearnerAction|TeachingFact::ContinuationChosen)));
        assert_eq!(probe.0.load(Ordering::SeqCst),0);
    }
}

#[test]
fn continuation_tc3_rejects_second_set_unread_target_and_selection_without_design() {
    let (_root,mut state)=fixture(true); enable(&mut state);
    let run=prepare_practice(&mut state,json!({"message":"Explain average speed"}));
    let movement=tc3_move(&state);
    let mut missing=movement.clone(); missing.as_object_mut().unwrap().remove("target");
    assert!(teaching::step(&state.private_context(),&run.turn_ref,json!({"operation":"select","move":missing.clone()}),&[],
        &[EvidenceRange {start_lid:"1.1".into(),end_lid:"1.1".into(),ranges:vec![]}]).is_err());
    let mut unread=movement.clone(); unread["target"]["source_bindings"][0]["end_lid"]=json!("1.2");
    assert!(tc3_select(&state,&run.turn_ref,unread).is_err());
    for field in ["label","purpose"] {
        let mut invalid=movement.clone(); invalid["continuation"]["options"][0][field]=json!("Look at LID 1.2");
        assert!(tc3_select(&state,&run.turn_ref,invalid).is_err(),"public provenance: {field}");
    }
    let selected=tc3_select(&state,&run.turn_ref,movement.clone()).unwrap();
    missing["move_id"]=json!("inherited-target");
    missing["design_ref"]=selected["design_ref"].clone();
    missing["block_refs"]=json!(["total-time"]);
    missing.as_object_mut().unwrap().remove("continuation");
    super::tutor_loop_tests::fixture_participation(&mut missing);
    let inherited=teaching::step(&state.private_context(),&run.turn_ref,json!({"operation":"select","move":missing,
        "delivery_move_refs":selected["delivery_move_refs"]}),&[],&[EvidenceRange {start_lid:"1.1".into(),end_lid:"1.1".into(),ranges:vec![]}]).unwrap();
    let store=state.user.learning_store().unwrap();
    let mut inherited_target=store.teaching_event(inherited["selected"].as_str().unwrap()).unwrap().payload["move"]["target"].clone();
    assert!(inherited_target.as_object_mut().unwrap().remove("focus_ref").is_some());
    assert_eq!(inherited_target,movement["target"]);
    let mut second=movement; second["move_id"]=json!("second");
    assert!(tc3_select(&state,&run.turn_ref,second).unwrap_err().message.contains("一组"));
}

#[test]
fn continuation_tc3_failed_or_cancelled_delivery_keeps_candidates_private() {
    for status in [AgentAssistantStatus::Failed,AgentAssistantStatus::Cancelled] {
        let (_root,mut state)=fixture(true); enable(&mut state);
        let run=prepare_practice(&mut state,json!({"message":"Explain average speed"}));
        tc3_select(&state,&run.turn_ref,tc3_move(&state)).unwrap();
        let saved=state.user.agent_history.sessions[0].turns.last_mut().unwrap();
        saved.status=status; saved.outcome=Some(super::tutor_loop_tests::outcome(None));
        assert_eq!(tc3_views(&mut state,&run.turn_ref)["activities"],json!([]));
    }
}

#[test]
fn continuation_tc3_pending_formal_activity_suppresses_related_suggestions() {
    for object_only in [false,true] {
        let (_root,mut state)=fixture(true);
        let (_original,delivery)=activity(&mut state,object_only);
        let run=prepare_practice(&mut state,json!({"message":"Explain the denominator again"}));
        let movement=tc3_move(&state);
        let error=tc3_select(&state,&run.turn_ref,movement.clone()).unwrap_err();
        assert!(error.message.contains("正式活动"),"{error:?}");
        post_ok(&mut state,"/tutor/action",json!({"operation_id":"answer","delivery_ref":delivery,"action":"submit","response":"A"}));
        tc3_select(&state,&run.turn_ref,movement).unwrap();
    }
}

#[test]
fn continuation_tc3_current_assessment_and_unrelated_history_are_distinct() {
    let (_root,mut state)=fixture(true);
    activity(&mut state,false);
    let run=prepare_practice(&mut state,json!({"message":"Explain the bicycle colour"}));
    let mut movement=tc3_move(&state);
    movement["target"]["source_bindings"][0]["start_lid"]=json!("1.2");
    movement["target"]["source_bindings"][0]["end_lid"]=json!("1.2");
    movement["target"]["learning_focus"]=json!("bicycle colour");
    movement["continuation"]=json!({"recommended_option_id":"observe","options":[
        {"option_id":"observe","label":"Locate the colour word","purpose":"Connect the adjective to the bicycle","move_kind":"observe"}]});
    super::tutor_loop_tests::step_with_design(&state.private_context(),&run.turn_ref,json!({"operation":"select","move":movement}),&[],
        &[EvidenceRange{start_lid:"1.2".into(),end_lid:"1.2".into(),ranges:vec![]}]).unwrap();

    let (_root,mut state)=fixture(true); enable(&mut state);
    let run=prepare_practice(&mut state,json!({"message":"Explain average speed"}));
    let mut movement=tc3_move(&state);
    movement["actions"]=json!(["submit"]);
    movement["assessment"]=json!({"rule":{"kind":"choice_set","choices":["A","B"],"correct":["A"]},
        "source_lids":["1.1"],"source_sufficient":true,"feedback":"after_submit"});
    assert!(tc3_select(&state,&run.turn_ref,movement.clone()).unwrap_err().message.contains("正式活动"));
    // If another select opens an activity after the offer, delivery suppresses the offer.
    tc3_select(&state,&run.turn_ref,tc3_move(&state)).unwrap();
    movement.as_object_mut().unwrap().remove("continuation"); movement["move_id"]=json!("question");
    tc3_select(&state,&run.turn_ref,movement).unwrap();
    super::tutor_loop_tests::finish(&mut state,&run.turn_ref,&super::tutor_loop_tests::outcome(None));
    let views=tc3_views(&mut state,&run.turn_ref);
    assert!(views["activities"].as_array().unwrap().iter().all(|v| v["continuation"].is_null() && v["move"]["continuation"].is_null()));
}

#[test]
fn continuation_tc3_failed_delivery_stays_private_and_legacy_pause_does_not_block() {
    let (_root,mut state)=fixture(true); enable(&mut state);
    let run=prepare_practice(&mut state,json!({"message":"Explain average speed"}));
    tc3_select(&state,&run.turn_ref,tc3_move(&state)).unwrap();
    let mut result=super::tutor_loop_tests::outcome(None);
    result.delivery_diagnostics=Some(runtime::orchestrator::AnswerDeliveryDiagnostics {
                        fallback: None,
        initial:Default::default(),repair:Some(runtime::orchestrator::AnswerDeliveryAttemptDiagnostics { issues:vec![
            runtime::orchestrator::AnswerDeliveryIssue { error_code:"ANSWER_PROVENANCE".into(),start:None,end:None,
                trigger_value:None,match_form:"test".into(),source_channels:vec![] }] }) });
    finalize_agent_turn_completed(&mut state,&run.turn_ref,&result,&[],"now").unwrap();
    assert_eq!(tc3_views(&mut state,&run.turn_ref)["activities"],json!([]));

    let (_root,mut state)=fixture(true); enable(&mut state);
    let run=prepare_practice(&mut state,json!({"message":"Explain average speed"}));
    tc3_select(&state,&run.turn_ref,tc3_move(&state)).unwrap();
    let revision=state.user.learning_store().unwrap().state().unwrap().control.revision;
    super::tutor_loop_tests::legacy_control(&mut state,json!({"operation_id":"pause","expected_revision":revision,
        "action":{"kind":"set_enabled","enabled":false}}));
    super::tutor_loop_tests::finish(&mut state,&run.turn_ref,&super::tutor_loop_tests::outcome(None));
    assert_eq!(tc3_views(&mut state,&run.turn_ref)["activities"][0]["continuation"]["availability"]["status"],"eligible");
}

#[test]
fn continuation_tc3_fixture_supports_lexical_source_discovery() {
    let (_root,state)=fixture(true);
    let result=state.workspace.book.search_text(&serde_json::from_value(json!({"query":"Average speed"})).unwrap()).unwrap();
    assert!(serde_json::to_value(result).unwrap().to_string().contains("1.1"));
}

#[test]
fn continuation_tc3_incomplete_and_empty_answers_cannot_publish_suggestions() {
    for empty in [false,true] {
        let (_root,mut state)=fixture(true); enable(&mut state);
        let run=prepare_practice(&mut state,json!({"message":"Explain average speed"}));
        tc3_select(&state,&run.turn_ref,tc3_move(&state)).unwrap();
        let mut result=super::tutor_loop_tests::outcome(None); result.incomplete=true;
        if empty { result.answer=None; result.answer_view=None; }
        finalize_agent_turn_completed(&mut state,&run.turn_ref,&result,&[],"now").unwrap();
        let views=tc3_views(&mut state,&run.turn_ref);
        if empty { assert_eq!(views["activities"],json!([])); }
        else { assert!(views["activities"].as_array().unwrap().iter().all(|v| v["continuation"].is_null() && v["move"]["continuation"].is_null())); }
    }
}

pub fn fixture(ready: bool) -> (tempfile::TempDir, AppState) {
    let root = tempfile::tempdir().unwrap();
    let mut state = state_named(&format!("tc0-{}", uuid::Uuid::now_v7()));
    let mut base = sample_base();
    base.book_id = scenario()["book_id"].as_str().unwrap().into();
    base.graph_nodes.clear();
    base.graph_edges.clear();
    let end = SOURCE.find("\n\nA blue").unwrap();
    base.lid_nodes[0].span = Span { start: 0, end: SOURCE.len() };
    base.lid_nodes[0].children.push("1.2".into());
    // The first leaf owns the heading too; lexical search requires every
    // non-whitespace source character to belong to a leaf.
    base.lid_nodes[1].span = Span { start: 0, end };
    base.lid_nodes.push(LidNode { lid: "1.2".into(), path: vec![1, 2], kind: NodeKind::Paragraph,
        span: Span { start: end + 2, end: SOURCE.trim_end().len() }, children: vec![] });
    state.workspace.book = Arc::new(Book::new(base, SOURCE));
    state.workspace.reader = Reader::new(&state.workspace.book, DEFAULT_RADIUS);
    state.workspace.book_dir = root.path().into();
    state.user.store = MemoryStore::open(root.path().join("memory.json")).unwrap();
    state.user.history_path = Some(root.path().join("history.json"));
    let paths = crate::session_store::SessionPaths::from_history(state.user.history_path.as_ref().unwrap());
    std::fs::create_dir_all(&paths.root).unwrap();
    let (store, history) = crate::session_store::SessionStore::open(paths).unwrap();
    state.user.session_store = Some(store);
    state.user.agent_history = history;
    // Actual foundation files, without builder receipts or invented optional teaching assets.
    std::fs::write(root.path().join("source.txt"), SOURCE).unwrap();
    std::fs::write(root.path().join("base.json"), serde_json::to_vec(&state.workspace.book.base).unwrap()).unwrap();
    std::fs::write(root.path().join("discourse_index.json"), r#"{"items":[]}"#).unwrap();
    std::fs::write(root.path().join("formula_semantics.json"), "[]").unwrap();
    if ready {
        std::fs::write(root.path().join("book_structure.json"), json!({"header":{
            "book_id":scenario()["book_id"],"book_version":"v1","profile_id":"technical_learning",
            "profile_version":"1","core_schema_version":"core_v0","generated_at":"tc0"},
            "spine":[],"throughlines":[],"key_stops":[]}).to_string()).unwrap();
    }
    assert_eq!(route_agent_new(&mut state, "1000").status, 200);
    (root, state)
}

pub fn quote() -> Value {
    let text = scenario()["quote"].as_str().unwrap().to_owned();
    let start = SOURCE.find(&text).unwrap();
    json!({"lid":"1.1","quote":text,"status":"resolved","resolution_basis":"exact",
        "raw_quote":text,"resolved_quote":text,"ranges":[{"lid":"1.1","range":{"start":start,"end":start+text.len()}}]})
}
pub fn selection_input(state: &AppState, key: &str) -> Value {
    let head = state.user.agent_history.sessions.iter().find(|s|Some(s.id.as_str()) == state.workspace.selected_chat.as_deref())
        .and_then(|s|s.turns.last()).map(|t|&t.turn_id);
    json!({"message":"","session_id":state.workspace.selected_chat,"client_request_id":key,"entry_action":"explain_selection","question_quote":quote(),
        "source_id":state.workspace.book.base.book_id,"source_revision":state.workspace.book.source_fingerprint(),"expected_chat_head":head})
}

#[test]
fn continuation_tc2_selection_freezes_action_without_inventing_user_text_or_session() {
    for ready in [false,true] {
        let (_root, mut state) = fixture(ready);
        let input = selection_input(&state,"direct");
        let run = prepare(&mut state,input.clone());
        assert_eq!(run.message, "");
        assert!(run.agent_message.contains("帮我弄懂这段"));
        assert_eq!(run.tutor.as_ref().unwrap()["current_action"]["action"]["origin"], "ui_entry");
        assert_eq!(run.tutor.as_ref().unwrap()["current_request"], "");
        let view = state.user.learning_store().unwrap().state().unwrap();
        assert!(view.sessions.is_empty());
        let replay = prepare_agent_chat(&mut state,&input.to_string(),"1002").err().unwrap();
        assert_eq!(serde_json::from_str::<Value>(&replay.body).unwrap()["turn_id"],run.turn_ref.turn_id);
        let mut changed = input; changed["message"] = json!("different");
        let conflict = prepare_agent_chat(&mut state,&changed.to_string(),"1002").err().unwrap();
        assert!(conflict.body.contains("REQUEST_KEY_CONFLICT"));
        let (history,_) = session_store::load_chat_storage(&state.user.history_path).unwrap();
        assert!(history.sessions[0].turns[0].domain.local_action.is_some());
        let facts = state.user.learning_store().unwrap().teaching_recent(&format!("observation:{}",state.workspace.book.base.book_id),20).unwrap();
        assert!(facts.iter().any(|f|f.payload["usage"]["kind"] == "reading_action" && f.payload["response"] == ""));
    }
}

#[test]
fn continuation_tc2_ordinary_answers_continue_without_the_removed_switch() {
    let (_root,mut state)=fixture(true);let probe=Probe::default();state.services.adapter=Box::new(probe.clone());
    let input=selection_input(&state,"first");post_ok(&mut state,"/agent/chat",input);
    let original=state.user.agent_history.sessions[0].turns[0].turn_id.clone();
    let input=json!({"message":"","session_id":state.workspace.selected_chat,"client_request_id":"continue","entry_action":"continue_answer",
        "answer_ref":{"turn_id":original},"expected_chat_head":original});
    let run=prepare(&mut state,input.clone());
    assert_eq!(run.tutor.as_ref().unwrap()["status"],"observing");
    assert!(run.agent_message.contains(scenario()["answer"].as_str().unwrap()));
    assert!(state.user.learning_store().unwrap().state().unwrap().sessions.is_empty());
    assert!(!state.user.learning_store().unwrap().state().unwrap().control.enabled);
    let repeat=prepare_agent_chat(&mut state,&input.to_string(),"1002").err().unwrap();
    assert_eq!(serde_json::from_str::<Value>(&repeat.body).unwrap()["turn_id"],run.turn_ref.turn_id);
    assert_eq!(probe.0.load(Ordering::SeqCst),1);
}

#[test]
fn continuation_answer_practice_keeps_original_scope_and_enters_practice() {
    for pending_question in [false, true] {
        let (_root, mut state) = fixture(true);
        let original = if pending_question { activity(&mut state, false).0 } else {
            let input = selection_input(&state, "original");
            let run = prepare(&mut state, input);
            super::tutor_loop_tests::finish(&mut state, &run.turn_ref, &super::tutor_loop_tests::outcome(None));
            run.turn_ref
        };
        let scope = state.user.agent_history.sessions[0].turns[0].domain.interaction.clone().unwrap();
        let input = json!({"message":"", "entry_action":"continue_answer", "learning_purpose":"practice",
            "answer_ref":{"turn_id":original.turn_id,"excerpt":"total distance"},
            "session_id":original.session_id,"client_request_id":"answer-practice","expected_chat_head":original.turn_id,
            "source_id":state.workspace.book.base.book_id,"source_revision":state.workspace.book.source_fingerprint()});
        let run = prepare(&mut state, input);
        let context = run.tutor.as_ref().unwrap();
        let action = &context["current_action"];
        assert_eq!(action["action"]["entry_action"], "continue_answer");
        assert_eq!(action["answer_ref"]["turn_id"], original.turn_id);
        assert_eq!(action["answer_text"], "total distance");
        assert_eq!(action["source_bindings"], json!(scope.source_bindings));
        assert_eq!(action["source_excerpt"], json!(scope.source_excerpt));
        assert_eq!(action["target"], json!(scope.target));
        assert_eq!(action["accepted_request"], "根据这段回答给我一道应用题");
        assert_eq!(action["learning_purpose"], "practice");
        assert_eq!(context["learning_purpose"], "practice");
        assert_eq!(context["status"], "practice");
        assert!(!context["learner_context"]["responding_to_practice"].is_object());
        assert_ne!(context["outside_current_request"], true);
        assert_eq!(context["current_request"], "");
        assert!(run.agent_message.contains("根据这段回答给我一道应用题"));
        let stored = state.user.learning_store().unwrap().teaching_event(&format!("binding:{}", run.turn_ref.turn_id)).unwrap();
        assert_eq!(stored.payload["current_action"], *action);
    }
}

#[test]
fn continuation_answer_practice_rejects_unavailable_original_answers() {
    use crate::interaction_input::InteractionInput;
    for boundary in ["pending", "failed", "cancelled", "foreign_chat", "excerpt", "source", "unready"] {
        let (_root, mut state) = fixture(boundary != "unready");
        let original_input = selection_input(&state, "original");
        let original = prepare(&mut state, original_input).turn_ref;
        if boundary != "pending" {
            super::tutor_loop_tests::finish(&mut state, &original, &super::tutor_loop_tests::outcome(None));
        }
        let mut input = json!({"message":"", "entry_action":"continue_answer", "learning_purpose":"practice",
            "answer_ref":{"turn_id":original.turn_id},"session_id":original.session_id,
            "expected_chat_head":original.turn_id,"client_request_id":"answer-practice",
            "source_id":state.workspace.book.base.book_id,"source_revision":state.workspace.book.source_fingerprint()});
        match boundary {
            "failed" => state.user.agent_history.sessions[0].turns[0].status = AgentAssistantStatus::Failed,
            "cancelled" => state.user.agent_history.sessions[0].turns[0].status = AgentAssistantStatus::Cancelled,
            "foreign_chat" => { route_agent_new(&mut state, "1001"); },
            "excerpt" => input["answer_ref"]["excerpt"] = json!("not present in the delivered answer"),
            "source" => state.workspace.book = Arc::new(Book::new(state.workspace.book.base.clone(), &format!("{SOURCE}\nchanged"))),
            _ => {},
        }
        let error = InteractionInput::parse(&input).unwrap().resolve(&state.user, &state.workspace, &input, None)
            .err().expect("practice must retain original-answer validation");
        let expected = match boundary {
            "source" => "INTERACTION_STALE",
            "unready" => "INTERACTION_UNAVAILABLE",
            _ => "INTERACTION_INVALID",
        };
        assert_eq!(error.error_code, expected, "{boundary}");
    }
}

#[test]
fn continuation_tc2_rejects_stale_and_unresolved_source_before_acceptance() {
    let (_root,mut state)=fixture(true);
    for kind in ["revision","book","partial","legacy","head"] {
        let mut input=selection_input(&state,kind);
        match kind {
            "revision" => input["source_revision"]=json!("old"),
            "book" => input["source_id"]=json!("another-book"),
            "partial" => { input["question_quote"]["status"]=json!("partial");input["question_quote"].as_object_mut().unwrap().remove("resolution_basis"); },
            "legacy" => input["question_quote"]=json!({"lid":"1.1","quote":"unverified"}),
            _ => input["expected_chat_head"]=json!("old"),
        }
        assert!(prepare_agent_chat(&mut state,&input.to_string(),"1001").is_err(),"{kind}");
        assert!(state.user.agent_history.sessions[0].turns.is_empty());
    }
}

#[test]
fn continuation_tc2_ordinary_help_and_continuation_do_not_depend_on_a_legacy_goal() {
    for condition in ["unready","paused","ended","other"] {
        let (_root,mut state)=fixture(condition!="unready");enable(&mut state);
        let view=state.user.learning_store().unwrap().state().unwrap();
        if condition=="other" {
            post_ok(&mut state,"/tutor/mutate",json!({"operation_id":"other-goal","expected_revision":view.control.revision,
                "action":{"kind":"start","user_intent":"Another source","explicit_constraints":[],"material_scope":[{"source_id":"other-book","scope_refs":[],"role":"primary"}],"default_teaching_intent":null}}));
        } else if condition!="unready" {
            post_ok(&mut state,"/tutor/mutate",json!({"operation_id":"old-goal","expected_revision":view.control.revision,
                "action":{"kind":if condition=="paused" {"pause"} else {"end"},"session_id":view.control.current_tutor_session_id}}));
        }
        let before=state.user.learning_store().unwrap().state().unwrap();
        let input=selection_input(&state,"help");let run=prepare(&mut state,input);
        assert_eq!(run.tutor.as_ref().unwrap()["status"],"observing");
        super::tutor_loop_tests::finish(&mut state,&run.turn_ref,&super::tutor_loop_tests::outcome(None));
        let input=json!({"message":"","session_id":run.turn_ref.session_id,"client_request_id":"continue","entry_action":"continue_answer",
            "answer_ref":{"turn_id":run.turn_ref.turn_id},"expected_chat_head":run.turn_ref.turn_id});
        let next=prepare(&mut state,input);assert_eq!(next.tutor.as_ref().unwrap()["status"],"observing");
        assert_eq!(state.user.learning_store().unwrap().state().unwrap(),before);
    }
}

#[test]
fn continuation_tc2_current_question_routes_answer_and_source_without_recording_help() {
    for object_only in [false,true] { for enabled in [false,true] {
        let (_root,mut state)=fixture(true);
        let (turn,delivery)=activity(&mut state,object_only);
        if !enabled {
            let view=state.user.learning_store().unwrap().state().unwrap();
            super::tutor_loop_tests::legacy_control(&mut state,json!({"operation_id":"off","expected_revision":view.control.revision,"action":{"kind":"set_enabled","enabled":false}}));
        }
        let request=json!({"message":"","entry_action":"clarify_answer","answer_ref":{"turn_id":turn.turn_id},"expected_chat_head":turn.turn_id});
        let result=post_ok(&mut state,"/agent/chat",request);
        assert_eq!(result["kind"],"activity_help");
        assert_eq!(result["delivery_refs"],json!([delivery]));
        assert_eq!(result["activity"]["turn_id"],turn.turn_id);
        assert!(!state.user.learning_store().unwrap().teaching_activity_events(&delivery).unwrap().iter().any(|e|e.kind==TeachingFact::HelpDisplayed));
        // The unrelated paragraph remains ordinary help, including while off.
        let mut request=selection_input(&state,"unrelated");
        let text="A blue bicycle travels along the river.";
        request["question_quote"]=json!({"lid":"1.2","quote":text,"status":"resolved","raw_quote":text,"resolved_quote":text,
            "ranges":[{"lid":"1.2","range":{"start":0,"end":text.len()}}]});
        let run=prepare(&mut state,request);
        assert!(run.scope.question_quote.is_some());
    }}
}

#[test]
fn continuation_tc2_local_restart_marks_unknown_execution_without_replay() {
    let (_root,mut state)=fixture(true);
    let request=selection_input(&state,"interrupted-click");
    let run=prepare(&mut state,request.clone());
    let (mut history,store)=session_store::load_chat_storage(&state.user.history_path).unwrap();
    let mut store=store.unwrap();
    session_runtime::recover(&mut store,&mut history,&BTreeSet::new()).unwrap();
    state.user.agent_history=history; state.user.session_store=Some(store);
    let probe=Probe::default();state.services.adapter=Box::new(probe.clone());
    let result=post_ok(&mut state,"/agent/chat",request);
    assert_eq!(result["turn_id"],run.turn_ref.turn_id);
    assert_eq!(probe.0.load(Ordering::SeqCst),0);
    let turn=&state.user.agent_history.sessions[0].turns[0];
    assert_eq!(turn.status,AgentAssistantStatus::Failed);
    assert_eq!(turn.error.as_ref().unwrap().error_code,"INTERRUPTED");
    assert!(turn.domain.interaction.is_some());
}

#[test]
fn continuation_tc2_cross_lid_and_direct_help_require_full_source_before_select() {
    let (_root,mut state)=fixture(true);enable(&mut state);
    let mut request=selection_input(&state,"multi-lid");
    let ranges:Vec<SelectedRange>=serde_json::from_value(json!([
        {"lid":"1.1","range":{"start":0,"end":8}}, {"lid":"1.2","range":{"start":0,"end":6}}])).unwrap();
    let text=validate_and_rebuild_selection_quote(&state.workspace.book,&ranges,"test").unwrap();
    request["question_quote"]=json!({"lid":"1.1","quote":text,"raw_quote":text,"resolved_quote":text,"status":"resolved","ranges":ranges});
    let run=prepare(&mut state,request);
    assert!(run.agent_message.contains(&text));
    let bindings=run.tutor.as_ref().unwrap()["current_action"]["source_bindings"].clone();
    let movement=json!({"move_id":"direct","object_ids":[],"capability":"explanation","kind":"explain",
        "interaction_intent":"direct_explanation","intent_origin":"current_action","prompt":"Explain the selected relationship",
        "target":{"learning_focus":"speed","expected_performance":"explain","capability":"explanation","source_bindings":bindings,"object_refs":[]}});
    assert!(super::tutor_loop_tests::step_with_design(&state.private_context(),&run.turn_ref,json!({"operation":"select","move":movement}),&[],&[]).is_err());
    let mut wrong=movement.clone();wrong["kind"]=json!("question");wrong["interaction_intent"]=json!("neutral");wrong["intent_origin"]=json!("neutral");
    let reads=[EvidenceRange{start_lid:"1.1".into(),end_lid:"1.2".into(),ranges:vec![]}];
    assert!(super::tutor_loop_tests::step_with_design(&state.private_context(),&run.turn_ref,json!({"operation":"select","move":wrong}),&[],&reads).is_err());
    assert!(super::tutor_loop_tests::step_with_design(&state.private_context(),&run.turn_ref,json!({"operation":"select","move":movement}),&[],&reads).is_err());
    assert_eq!(run.tutor.as_ref().unwrap()["status"],"observing");
}
pub fn post_ok(state: &mut AppState, path: &str, input: Value) -> Value {
    let reply = post(state, path, &input.to_string());
    assert_eq!(reply.status, 200, "{path}: {}", reply.body);
    serde_json::from_str(&reply.body).unwrap()
}
pub fn enable(state: &mut AppState) {
    super::tutor_loop_tests::legacy_control(state,  json!({"operation_id":"tc0-enable","expected_revision":0,
        "action":{"kind":"set_enabled","enabled":true}}));
    teaching::start_request(&state.private_context(),"historical-goal").unwrap();
}
pub fn prepare(state: &mut AppState, input: Value) -> agent_run::PreparedAgentChat {
    prepare_agent_chat(state, &input.to_string(), "1000").unwrap_or_else(|reply| panic!("{}", reply.body))
}

pub fn prepare_practice(state:&mut AppState,input:Value)->agent_run::PreparedAgentChat {
    super::tutor_loop_tests::prepare_practice_chat(state,&input.to_string(),"1000").unwrap_or_else(|reply|panic!("{}",reply.body))
}

/// Both the source-only contract and historical object-only assessment remain supported.
pub fn activity(state: &mut AppState, object_only: bool) -> (AgentTurnRef, String) {
    enable(state);
    if object_only {
        let book = &state.workspace.book;
        std::fs::create_dir_all(state.workspace.book_dir.join("teaching/versions/v1")).unwrap();
        std::fs::write(state.workspace.book_dir.join("teaching/versions/v1/map.json"), json!({
            "source_id":book.base.book_id,"source_revision":book.source_fingerprint(),
            "objects":{"active_refs":[{"source_id":book.base.book_id,"object_id":"speed"}],"objects":[{
                "ref":{"source_id":book.base.book_id,"object_id":"speed"},"object_revision":1,"meaning":"average speed",
                "source_bindings":[{"source_id":book.base.book_id,"source_revision":book.source_fingerprint(),"lid":"1.1"}]}]},
            "cognitive_materials":{"materials":[]}}).to_string()).unwrap();
        std::fs::write(state.workspace.book_dir.join("teaching_readiness.json"), json!({"version":"teaching_readiness.v1",
            "status":"ready","source_id":book.base.book_id,"source_revision":book.source_fingerprint(),
            "teaching_map_revision":"v1","map_path":"teaching/versions/v1/map.json",
            "coverage":{"source":"complete","structure":"complete","objects":"complete","cognitive_materials":"complete","source_review":"passed"},"limitations":[]}).to_string()).unwrap();
    }
    let run = super::tutor_loop_tests::prepare_practice_chat(state, r#"{"message":"Practice the denominator"}"#, "1000").unwrap_or_else(|r|panic!("{}",r.body));
    let mut movement = json!({"move_id":"question","object_ids":[],"capability":"explanation","kind":"question",
        "interaction_intent":"neutral","intent_origin":"neutral","prompt":"Which quantity is the denominator? A total time; B colour.",
        "actions":["submit","revise","request_hint","reveal","skip"],"hint":"Look at the elapsed time.","explanation":"A: total time.",
        "target":{"learning_focus":"average speed","expected_performance":"Identify the denominator","capability":"explanation",
            "source_bindings":[{"source_id":state.workspace.book.base.book_id,"source_revision":state.workspace.book.source_fingerprint(),"start_lid":"1.1","end_lid":"1.1"}],"object_refs":[]},
        "assessment":{"rule":{"kind":"choice_set","choices":["A","B"],"correct":["A"]},"source_lids":["1.1"],"source_sufficient":true,"feedback":"after_submit"}});
    if object_only {
        movement.as_object_mut().unwrap().remove("target");
        movement["object_ids"] = json!(["speed"]);
        super::tutor_loop_tests::step_with_design(&state.private_context(), &run.turn_ref, json!({"operation":"material","object_id":"speed"}), &[], &[]).unwrap();
    }
    super::tutor_loop_tests::step_with_design(&state.private_context(), &run.turn_ref, json!({"operation":"select","move":movement}), &[],
        &[EvidenceRange { start_lid:"1.1".into(),end_lid:"1.1".into(),ranges:vec![] }]).unwrap();
    super::tutor_loop_tests::finish(state, &run.turn_ref, &super::tutor_loop_tests::outcome(None));
    let delivery = format!("delivery:{}:question", run.turn_ref.turn_id);
    post_ok(state, "/tutor/display", json!({"delivery_ref":delivery}));
    (run.turn_ref, delivery)
}

#[derive(Clone, Default)]
pub struct Probe(pub Arc<AtomicUsize>);
impl ModelAdapter for Probe {
    fn complete(&self, _: CompletionRequest) -> Result<ParsedResponse, AdapterError> { unreachable!() }
    fn complete_structured(&self, request:CompletionRequest) -> Result<Value,AdapterError> {
        if let Some(guidance)=super::tutor_loop_tests::fixture_coach_guidance(&request) { return Ok(guidance); }
        unreachable!()
    }
    fn chat(&self, _: &AgentRequestPlan) -> Result<AssistantTurn, AdapterError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(AssistantTurn { provider_continuation:None,text:Some(scenario()["answer"].as_str().unwrap().into()),
            tool_calls:vec![],usage_total_tokens:Some(1) })
    }
}

/// Inspect the actual sampled surface, not just the canonical registry.
pub struct ActionToolContract {
    inner: Box<dyn ModelAdapter + Send>,
    observations: Arc<std::sync::Mutex<Vec<bool>>>,
}
impl ModelAdapter for ActionToolContract {
    fn complete(&self, request: CompletionRequest) -> Result<ParsedResponse, AdapterError> { self.inner.complete(request) }
    fn complete_structured(&self, request: CompletionRequest) -> Result<Value, AdapterError> { self.inner.complete_structured(request) }
    fn chat(&self, request: &AgentRequestPlan) -> Result<AssistantTurn, AdapterError> {
        let tool = request.tools.iter().find(|tool| tool.name == "tutor.step");
        let movement = tool.map(|tool| &tool.parameters["properties"]["move"]).unwrap_or(&Value::Null);
        let active = movement.is_object();
        if active {
            assert!(movement["properties"].get("continuation").is_some());
            for field in ["interaction_intent","intent_origin","intent_quote","preference_ref","capability"] {
                assert!(super::tutor_loop_tests::tool_examples::sampled_execution_guidance(request)["move_forbidden"].as_array().unwrap()
                    .contains(&json!(field)), "the current action forbids duplicate input: {field}");
            }
        } else if let Some(tool) = tool {
            let ordinary:Vec<_>=tool.parameters["properties"]["operation"]["enum"].as_array().unwrap().iter().filter(|name|*name!=&json!("closed_book")).cloned().collect();
            assert_eq!(ordinary,json!(["trace","understanding","evidence","corrections"]).as_array().unwrap().clone());
            assert!(!tool.description.contains("Select"));
        }
        self.observations.lock().unwrap().push(active);
        self.inner.chat(request)
    }
}

#[test]
fn continuation_tc2_actual_tools_and_delivered_intent_follow_the_accepted_action() {
    action_contract_samples();
}

#[test]
fn tutor_to3_projected_ui_examples_execute_with_runtime_bound_fields() {
    use super::tutor_loop_tests::tool_examples::PublishedExample;
    let (_root, mut state) = fixture(true);
    enable(&mut state);
    let example = runtime::tutor::spec().parameters["examples"].as_array().unwrap().iter()
        .find(|e| e["operation"] == "understanding").unwrap().clone();
    for action in ["explain_selection", "continue_answer", "clarify_answer"] {
        let observed = Arc::new(std::sync::Mutex::new(Vec::new()));
        let previous_design_ref={
            let store=state.user.learning_store().unwrap();
            store.state().unwrap().control.current_tutor_session_id.as_deref().and_then(|session|
                store.current_teaching_design(session,&state.workspace.book.base.book_id,state.workspace.book.source_fingerprint()).unwrap().map(|event|event.event_id))
        };
        state.services.adapter = Box::new(PublishedExample { example:example.clone(), previous_design_ref, stage:AtomicUsize::new(0), observed:observed.clone() });
        let input = if action == "explain_selection" { selection_input(&state, "to3-selection") } else {
            let turn = &state.user.agent_history.sessions[0].turns.last().unwrap().turn_id;
            json!({"message":"","session_id":state.workspace.selected_chat,"client_request_id":format!("to3-{action}"),
                "entry_action":action,"answer_ref":{"turn_id":turn},"expected_chat_head":turn})
        };
        let result = post_ok(&mut state, "/agent/chat", input);
        assert_eq!(result["incomplete"], false, "{action}: {result}");
        let sent = observed.lock().unwrap();
        assert_eq!(sent[0]["operation"],"understanding");
        let chat=&state.user.agent_history.sessions[0];
        let turn=chat.turns.last().unwrap();
        let store=state.user.learning_store().unwrap();
        let bound=store.teaching_event(&format!("binding:{}",turn.turn_id)).unwrap();
        assert_eq!(bound.payload["status"],"observing");
        assert_eq!(bound.payload["current_action"]["source_bindings"][0]["source_revision"],state.workspace.book.source_fingerprint());
        assert!(store.teaching_deliveries(&chat.id,&turn.turn_id).unwrap().is_empty());

    }
}

pub fn action_contract_samples() -> Vec<Value> {
    let (_root,mut state) = fixture(true);
    let tool_call = |name: &str, arguments: Value| AssistantTurn {
        provider_continuation:None, text:None, usage_total_tokens:Some(1),
        tool_calls:vec![ToolCall { id:format!("call-{name}"), name:name.into(), arguments:arguments.to_string() }],
    };
    let answer = || AssistantTurn { provider_continuation:None, text:Some(scenario()["answer"].as_str().unwrap().into()),
        tool_calls:vec![], usage_total_tokens:Some(1) };
    let observations = Arc::new(std::sync::Mutex::new(Vec::new()));
    let samples = Arc::new(std::sync::Mutex::new(Vec::new()));
    state.services.adapter = Box::new(TutorRequestRecorder { samples:samples.clone(), directory:None, inner:Box::new(ActionToolContract {
        observations:observations.clone(), inner:Box::new(ChatStubAdapter::scripted(vec![
            tool_call("book.text",json!({"lid":"1.1","end_lid":"1.1"})), answer(),
        ])),
    }) });
    let input = selection_input(&state,"contract-first");
    post_ok(&mut state,"/agent/chat",input);
    let original = state.user.agent_history.sessions[0].turns[0].turn_id.clone();
    assert!(state.user.learning_store().unwrap().state().unwrap().sessions.is_empty());
    enable(&mut state);
    state.services.adapter=Box::new(TutorRequestRecorder {samples:samples.clone(),directory:None,inner:Box::new(ActionToolContract {
        observations:observations.clone(),inner:Box::new(ChatStubAdapter::scripted(vec![
            tool_call("book.text",json!({"lid":"1.1","end_lid":"1.1"})), answer(),
        ]))
    })});
    let input = json!({"message":"","session_id":state.workspace.selected_chat,"client_request_id":"contract-next",
        "entry_action":"continue_answer","answer_ref":{"turn_id":original},"expected_chat_head":original});
    post_ok(&mut state,"/agent/chat",input);
    assert_eq!(*observations.lock().unwrap(), vec![false,false,false,false]);
    let session = &state.user.agent_history.sessions[0];
    let turn = &session.turns[1];
    assert_eq!(turn.status,AgentAssistantStatus::Completed);
    let deliveries = state.user.learning_store().unwrap().teaching_deliveries(&session.id,&turn.turn_id).unwrap();
    assert!(deliveries.is_empty());
    let result = samples.lock().unwrap().clone();
    result
}

/// Record the actual provider-independent requests, including frozen fragments.
pub struct TutorRequestRecorder {
    pub inner: Box<dyn ModelAdapter + Send>,
    pub samples: Arc<std::sync::Mutex<Vec<Value>>>,
    pub directory: Option<std::path::PathBuf>,
}
impl ModelAdapter for TutorRequestRecorder {
    fn model_runtime_profile(&self) -> runtime::ModelRuntimeProfile { self.inner.model_runtime_profile() }
    fn complete(&self, request: CompletionRequest) -> Result<ParsedResponse, AdapterError> { self.inner.complete(request) }
    fn complete_structured(&self, request: CompletionRequest) -> Result<Value, AdapterError> {
        if !request.system.contains("Teaching delivery acceptance review.")
            && !request.system.contains("Teaching explanation acceptance review.") { return self.inner.complete_structured(request); }
        let value=json!({"kind":"structured_completion","instructions":request.system,"input":[{"role":runtime::Role::User,"content":request.user}],
            "tools":[],"images":request.images,"output_token_limit":request.output_token_limit,"reasoning_effort":request.reasoning_effort});
        let index={let mut samples=self.samples.lock().unwrap();let index=samples.len();samples.push(value.clone());index};
        if let Some(dir)=&self.directory {
            std::fs::write(dir.join(format!("request-{index:02}.json")),serde_json::to_vec_pretty(&value).unwrap()).unwrap();
        }
        // A recorder forwards to its actual inner adapter. In live provider
        // runs this is a genuine review call, never a fixture acceptance.
        let result=self.inner.complete_structured(request);
        if let Some(dir)=&self.directory {
            let output=match &result {Ok(value)=>value.clone(),Err(error)=>json!({"error":error.message})};
            std::fs::write(dir.join(format!("response-{index:02}.json")),serde_json::to_vec_pretty(&output).unwrap()).unwrap();
        }
        result
    }
    fn chat(&self, request: &AgentRequestPlan) -> Result<AssistantTurn, AdapterError> {
        let mut input = serde_json::to_value(&request.input).unwrap();
        for message in input.as_array_mut().into_iter().flatten() {
            if let Some(fields) = message.as_object_mut() { fields.remove("provider_continuation"); }
        }
        let value = json!({"instructions":request.instructions,"input":input,"tools":request.tools,
            "instruction_assets":request.instruction_assets,"runtime_profile":request.runtime_profile,
            "active_context":request.active_context,"output_token_limit":request.output_token_limit,
            "tool_choice":request.tool_choice,"parallel_tool_calls":request.parallel_tool_calls});
        let index = {
            let mut samples = self.samples.lock().unwrap();
            let index = samples.len(); samples.push(value.clone()); index
        };
        if let Some(dir) = &self.directory {
            std::fs::write(dir.join(format!("request-{index:02}.json")),serde_json::to_vec_pretty(&value).unwrap()).unwrap();
        }
        let result = self.inner.chat(request);
        if let Some(dir) = &self.directory {
            let output = match &result {
                Ok(turn) => json!({"text":turn.text,"tool_calls":turn.tool_calls,"usage_total_tokens":turn.usage_total_tokens}),
                Err(error) => json!({"error":error.message}),
            };
            std::fs::write(dir.join(format!("response-{index:02}.json")),serde_json::to_vec_pretty(&output).unwrap()).unwrap();
        }
        result
    }
}

#[test]
fn continuation_tc2a_selection_omits_navigation_workflow() {
    let samples = action_contract_samples();
    let first = samples[0].to_string();
    assert!(!first.contains("When establishing a new explicit guided-reading route"), "ordinary explanation carries the full navigation workflow");
    assert!(first.contains("resident-agent.policy.navigation-discovery"));
    assert!(!first.contains("resident-agent.tutor.understanding"));
    assert!(first.contains("current_action") && first.contains("source_bindings"));
}

#[test]
fn continuation_tc2a_continue_omits_assessment_workflow() {
    let samples = action_contract_samples();
    let continued = samples[2].to_string();
    assert!(!continued.contains("If a formal open response is pending"), "continuation without a formal response carries the assessment workflow");
    assert!(!continued.contains("Select validates and saves the exact delivery through the host"));
    for workflow in ["assessment", "understanding", "presentation"] {
        assert!(!continued.contains(&format!("resident-agent.tutor.{workflow}")));
    }
}

#[test]
fn continuation_tc2a_understanding_read_opens_update_workflow_without_enabling_teaching() {
    let (_root, mut state) = fixture(true);
    let samples = Arc::new(std::sync::Mutex::new(Vec::new()));
    let call = |name: &str, args: Value| AssistantTurn { provider_continuation:None,text:None,usage_total_tokens:Some(1),
        tool_calls:vec![ToolCall { id:name.into(),name:name.into(),arguments:args.to_string() }] };
    state.services.adapter = Box::new(TutorRequestRecorder { samples:samples.clone(),directory:None,
        inner:Box::new(ChatStubAdapter::scripted(vec![
            call("tutor.step",json!({"operation":"understanding"})),
            call("book.text",json!({"lid":"1.1"})),
            AssistantTurn { provider_continuation:None,text:Some("Use total distance over total time.".into()),usage_total_tokens:Some(1),tool_calls:vec![] },
        ])) });
    let input = selection_input(&state,"discover-understanding");
    let reply = post_ok(&mut state,"/agent/chat",input);
    assert!(!reply["incomplete"].as_bool().unwrap());
    let samples = samples.lock().unwrap();
    assert!(!samples[0].to_string().contains("resident-agent.tutor.understanding"));
    assert!(samples[1].to_string().contains("resident-agent.tutor.understanding"));
    assert_eq!(samples[0]["tools"],samples[1]["tools"],"opening guidance must not expand operation permissions");
    assert!(state.user.learning_store().unwrap().state().unwrap().sessions.is_empty());
}

#[test]
fn continuation_baseline_foundation_and_selection_are_shared_across_ready_states() {
    for ready in [true, false] {
        let (_root, mut state) = fixture(ready);
        let readiness = crate::tutor_api::tutor_source_readiness(&state.workspace.book, &state.workspace.book_dir);
        assert_eq!(readiness["status"], if ready { "ready" } else { "preparing" }, "{readiness}");
        if !ready { assert_eq!(readiness["blocked_stage"], "book_structure"); }
        enable(&mut state);
        let run = prepare(&mut state, json!({"message":"Explain average speed","question_quote":quote()}));
        assert_eq!(run.tutor.as_ref().unwrap()["status"], "observing");
        assert_eq!(run.scope.question_quote.as_ref().unwrap().quote, scenario()["quote"]);
        assert!(run.agent_message.contains(scenario()["quote"].as_str().unwrap()));
    }
}

#[test]
fn continuation_baseline_original_help_preserves_assisted_conditions() {
    for object_only in [false, true] {
        let (_root, mut state) = fixture(true);
        let (_, delivery) = activity(&mut state, object_only);
        let help = post_ok(&mut state, "/tutor/action", json!({"operation_id":"hint","delivery_ref":delivery,"action":"request_hint"}));
        post_ok(&mut state, "/tutor/help-displayed", json!({"help_ref":help["help"]["event_id"]}));
        let result = post_ok(&mut state, "/tutor/action", json!({"operation_id":"answer","delivery_ref":delivery,"action":"submit","response":"A"}));
        assert_eq!(result["assessment"], "correct");
        let store = state.user.learning_store().unwrap();
        let evidence = store.evidence(&format!("assessment:action:answer:{}", if object_only { "speed" } else { "source" })).unwrap();
        assert_eq!(evidence.assistance_refs.len(), 1);
        assert_eq!(evidence.attempt, 1);
        if object_only {
            let p = store.learner_projection(&state.workspace.book.base.book_id).unwrap().0.unwrap();
            assert_eq!((p.rows[0].independent_support, p.rows[0].assisted_support), (0, 1));
        }
    }
}

#[test]
fn continuation_baseline_local_jsonl_replays_delivered_answer_and_quote() {
    let (_root, mut state) = fixture(true);
    state.services.adapter = Box::new(Probe::default());
    post_ok(&mut state, "/agent/chat", json!({"message":"Explain this selection","question_quote":quote()}));
    let (history, _) = crate::session_store::load_chat_storage(&state.user.history_path).unwrap();
    let turn = &history.sessions[0].turns[0];
    assert_eq!(turn.status, AgentAssistantStatus::Completed);
    assert_eq!(turn.question_quote.as_ref().unwrap().quote, scenario()["quote"]);
    assert_eq!(turn.outcome.as_ref().unwrap().answer.as_deref(), scenario()["answer"].as_str());
}

pub fn incompatible_reference(clarify: bool) {
        let (_root, mut state) = fixture(true);
        let (turn, delivery) = activity(&mut state, false);
        let mut request = json!({"message":"Help with this step","teaching_ref":delivery});
        if clarify { request["entry_action"]=json!("clarify_answer"); request["answer_ref"]=json!({"turn_id":turn.turn_id}); }
        else { request["teaching_choice"]=json!({"delivery_ref":delivery,"option_id":"compare"}); }
        let result = prepare_agent_chat(&mut state, &request.to_string(), "1001");
        let facts = state.user.learning_store().unwrap().teaching_activity_events(&delivery).unwrap();
        let responses = facts.iter().filter(|e| e.kind == TeachingFact::LearnerAction && e.payload["action"] == "chat_response").count();
        assert_eq!(responses, 0, "new action mixed with teaching_ref must fail before recording a response; clarify={clarify}");
        assert!(result.is_err());
}
#[test]
fn continuation_red_choice_cannot_record_chat_response() { incompatible_reference(false); }
#[test]
fn continuation_red_clarification_cannot_record_chat_response() { incompatible_reference(true); }

#[test]
fn continuation_red_local_lost_result_does_not_execute_twice() {
    let (_root, mut state) = fixture(true);
    let probe = Probe::default();
    state.services.adapter = Box::new(probe.clone());
    let request = selection_input(&state, "same-click");
    post_ok(&mut state, "/agent/chat", request.clone());
    let (history, store) = crate::session_store::load_chat_storage(&state.user.history_path).unwrap();
    state.user.agent_history = history;
    state.user.session_store = store;
    post_ok(&mut state, "/agent/chat", request);
    assert_eq!(probe.0.load(Ordering::SeqCst), 1, "retry invoked the model twice");
}

#[test]
#[ignore = "TC4 red: paused Tutor must reject stale choice before accepting a turn (E03/E09)"]
fn continuation_red_paused_choice_is_not_an_ordinary_request() {
    let (_root, mut state) = fixture(true);
    let (_, delivery) = activity(&mut state, false);
    let view = state.user.learning_store().unwrap().state().unwrap();
    post_ok(&mut state, "/tutor/mutate", json!({"operation_id":"pause","expected_revision":view.control.revision,
        "action":{"kind":"pause","session_id":view.control.current_tutor_session_id}}));
    let result = prepare_agent_chat(&mut state, &json!({"message":"Compare the journeys",
        "teaching_choice":{"delivery_ref":delivery,"option_id":"compare"}}).to_string(), "1001");
    assert!(result.is_err(), "paused choice was accepted as an ordinary request");
}

pub fn source_help_then_correct(object_only: bool) {
    let (_root, mut state) = fixture(true);
    let (_, delivery) = activity(&mut state, object_only);
    // Existing explicit ordinary explanation remains allowed when Tutor is paused.
    let view = state.user.learning_store().unwrap().state().unwrap();
    post_ok(&mut state, "/tutor/mutate", json!({"operation_id":"pause","expected_revision":view.control.revision,
        "action":{"kind":"pause","session_id":view.control.current_tutor_session_id}}));
    state.services.adapter = Box::new(Probe::default());
    let turns_before: usize = state.user.agent_history.sessions.iter().map(|s|s.turns.len()).sum();
    let request = selection_input(&state, "question-help");
    let response = post_ok(&mut state, "/agent/chat", request);
    assert_eq!(response["kind"], "activity_help");
    let turns_after: usize = state.user.agent_history.sessions.iter().map(|s|s.turns.len()).sum();
    // When redirected to the original activity, the reader still has to request
    // and see its help. Merely locating an activity must never count as assistance.
    if turns_after == turns_before {
        let help = post_ok(&mut state, "/tutor/action", json!({"operation_id":"source-help","delivery_ref":delivery,"action":"request_hint"}));
        post_ok(&mut state, "/tutor/help-displayed", json!({"help_ref":help["help"]["event_id"]}));
    }
    let view = state.user.learning_store().unwrap().state().unwrap();
    post_ok(&mut state, "/tutor/mutate", json!({"operation_id":"resume","expected_revision":view.control.revision,
        "action":{"kind":"resume","session_id":view.control.current_tutor_session_id}}));
    let result = post_ok(&mut state, "/tutor/action", json!({"operation_id":"answer","delivery_ref":delivery,"action":"submit","response":"A"}));
    assert_eq!(result["assessment"], "correct");
    let store = state.user.learning_store().unwrap();
    let evidence = store.evidence(&format!("assessment:action:answer:{}", if object_only { "speed" } else { "source" })).unwrap();
    let independent = store.learner_projection(&state.workspace.book.base.book_id).unwrap().0.unwrap().rows.first().map(|r|r.independent_support);
    assert!(turns_after == turns_before && !evidence.assistance_refs.is_empty(),
        "source help bypassed original activity: object_only={object_only}, new_turns={}, assistance_refs={:?}, independent_support={independent:?}",
        turns_after - turns_before, evidence.assistance_refs);
}
#[test]
fn continuation_red_source_question_help_is_assisted() { source_help_then_correct(false); }
#[test]
fn continuation_red_object_question_help_is_assisted() { source_help_then_correct(true); }

#[test]
fn continuation_red_answer_excerpt_reaches_model_input() {
    let (_root, mut state) = fixture(true);
    let first = prepare(&mut state, json!({"message":"Explain average speed"}));
    let mut answer = super::tutor_loop_tests::outcome(None);
    answer.answer = Some(scenario()["answer"].as_str().unwrap().into());
    answer.answer_view = None;
    super::tutor_loop_tests::finish(&mut state, &first.turn_ref, &answer);
    let next = prepare(&mut state, json!({"message":"This step is unclear","entry_action":"clarify_answer",
        "session_id":first.turn_ref.session_id,"client_request_id":"clarify", "expected_chat_head":first.turn_ref.turn_id,
        "answer_ref":{"turn_id":first.turn_ref.turn_id,"excerpt":scenario()["answer_excerpt"]}}));
    assert!(next.agent_message.contains(scenario()["answer_excerpt"].as_str().unwrap()),
        "answer_ref was lost from the current request: {}", next.agent_message);
    assert!(!next.agent_message.contains("chat_response"));
}

#[test]
fn continuation_answer_excerpt_can_cross_a_source_citation() {
    let (_root, mut state) = fixture(true);
    let first = prepare(&mut state, json!({"message":"Explain average speed"}));
    let mut answer = super::tutor_loop_tests::outcome(None);
    answer.answer = Some("Distance divided by time gives average speed.".into());
    answer.answer_view = Some(AgentAnswerView { parts: vec![
        AgentAnswerPart::Markdown { text: "Distance divided by time".into() },
        AgentAnswerPart::Sources { source_ref_ids: vec![] },
        AgentAnswerPart::Markdown { text: " gives average speed.".into() },
    ], sources: vec![] });
    super::tutor_loop_tests::finish(&mut state, &first.turn_ref, &answer);
    let excerpt = "time gives average speed";
    let next = prepare(&mut state, json!({"message":"This step is unclear","entry_action":"clarify_answer",
        "session_id":first.turn_ref.session_id,"client_request_id":"clarify-citation","expected_chat_head":first.turn_ref.turn_id,
        "answer_ref":{"turn_id":first.turn_ref.turn_id,"excerpt":excerpt}}));
    assert!(next.agent_message.contains(excerpt));
}

#[test]
fn continuation_baseline_local_blank_chat_is_rejected() {
    let (_root, mut state) = fixture(true);
    assert!(prepare_agent_chat(&mut state, r#"{"message":"   "}"#, "1000").is_err(),
        "blank ordinary input must not create a turn");
    assert!(state.user.agent_history.sessions.iter().all(|session| session.turns.is_empty()));
}

#[test]
fn continuation_contract_input_shapes_and_normalized_identity() {
    use crate::interaction_input::InteractionInput;
    let selection = json!({"message":"","entry_action":"explain_selection","question_quote":quote()});
    let answer = json!({"message":"","entry_action":"clarify_answer","answer_ref":{"turn_id":"old","excerpt":"speed"},"expected_chat_head":"latest"});
    let choice = json!({"message":"","teaching_choice":{"delivery_ref":"d","option_id":"a"},"expected_chat_head":"offering"});
    let ordinary=run_input::normalize("w", &json!({"message":"ordinary"})).unwrap();
    for key in ["entry_action","answer_ref","teaching_choice"] { assert!(ordinary.get(key).is_none()); }
    assert_eq!(ordinary,run_input::normalize("w", &json!({"message":"ordinary","entry_action":null,"answer_ref":null,"teaching_choice":null})).unwrap());
    for valid in [selection.clone(),answer.clone(),choice.clone(),json!({"message":"ordinary"})] {
        assert!(InteractionInput::parse(&valid).is_ok(), "{valid}");
        assert_eq!(run_input::normalize("w", &valid).unwrap()["message"], valid["message"]);
    }
    for (key, value) in [("entry_action",json!("continue_answer")),("answer_ref",json!({"turn_id":"other","excerpt":"speed"})),
        ("answer_ref",json!({"turn_id":"old","excerpt":"distance"})),("message",json!("My actual words")),("expected_chat_head",json!("newer"))] {
        let mut changed = answer.clone(); changed[key] = value;
        assert_ne!(run_input::normalize("w", &answer).unwrap(),run_input::normalize("w", &changed).unwrap());
    }
    for (key,value) in [("teaching_ref",json!("activity")),("note_mem_id",json!("note")),("goal_action",json!("replace")),
        ("goal_id",json!("unrelated")),("retry_of",json!("old")),("resolved_interaction",json!({})),("current_action",json!({}))] {
        for mut invalid in [selection.clone(),answer.clone(),choice.clone()] {
            invalid[key] = value.clone();
            assert!(InteractionInput::parse(&invalid).is_err(), "{invalid}");
            assert!(run_input::normalize("w", &invalid).is_err());
        }
    }
    let mut invalids = vec![json!({"message":"   "}),json!({"message":"typed","answer_ref":{"turn_id":"old"}})];
    let mut v=selection.clone(); v["answer_ref"]=json!({"turn_id":"old"}); invalids.push(v);
    let mut v=selection.clone(); v.as_object_mut().unwrap().remove("question_quote"); invalids.push(v);
    let mut v=selection; v["entry_action"]=json!("unknown"); invalids.push(v);
    let mut v=answer.clone(); v["teaching_choice"]=choice["teaching_choice"].clone(); invalids.push(v);
    let mut v=answer.clone(); v["question_quote"]=quote(); invalids.push(v);
    let mut v=answer.clone(); v["answer_ref"]["excerpt"]=json!(" "); invalids.push(v);
    let mut v=answer.clone(); v["answer_ref"]["tool"]=json!("reader.note"); invalids.push(v);
    let mut v=answer.clone(); v["expected_chat_head"]=Value::Null; invalids.push(v);
    let mut v=answer; v.as_object_mut().unwrap().remove("answer_ref"); invalids.push(v);
    let mut v=choice.clone(); v["question_quote"]=quote(); invalids.push(v);
    let mut v=choice.clone(); v["teaching_choice"]["option_id"]=json!(" "); invalids.push(v);
    for invalid in invalids { assert!(InteractionInput::parse(&invalid).is_err(), "{invalid}"); }
    let mut with_scene=choice; with_scene["presentation_follow_up"]=json!({"saved_state_ref":"scene"});
    assert!(InteractionInput::parse(&with_scene).is_ok()); // Actual receipt authority belongs to the resolver.
    assert_eq!(run_input::normalize("w",&with_scene).unwrap()["presentation_follow_up"],with_scene["presentation_follow_up"]);
}

#[test]
fn continuation_answer_practice_contract_is_explicit_and_exclusive() {
    use crate::interaction_input::InteractionInput;
    let input = json!({"message":"","entry_action":"continue_answer","learning_purpose":"practice",
        "answer_ref":{"turn_id":"original"},"expected_chat_head":"latest"});
    assert!(matches!(InteractionInput::parse(&input).unwrap(), InteractionInput::Answer { .. }));
    let normalized = run_input::normalize("w", &input).unwrap();
    assert_eq!(normalized["learning_purpose"], "practice");
    let mut ordinary = input.clone();
    ordinary.as_object_mut().unwrap().remove("learning_purpose");
    assert!(InteractionInput::parse(&ordinary).is_ok());
    assert_ne!(normalized, run_input::normalize("w", &ordinary).unwrap());
    for (key, value) in [("learning_purpose", json!("understand")), ("learning_purpose", json!("discover")),
        ("learning_purpose", json!("exam")), ("learning_purpose", json!("explore")),
        ("entry_action", json!("clarify_answer")), ("entry_action", json!("learn_with_purpose")),
        ("answer_ref", Value::Null), ("expected_chat_head", Value::Null), ("chapter_lid", json!("1")),
        ("question_quote", quote()), ("question_anchor_lid", json!("1.1")),
        ("teaching_choice", json!({"delivery_ref":"delivery","option_id":"next"}))] {
        let mut invalid = input.clone();
        invalid[key] = value;
        assert_eq!(InteractionInput::parse(&invalid).unwrap_err().error_code, "INTERACTION_INVALID", "{invalid}");
    }
}

#[test]
fn continuation_contract_references_reject_invalid_clarification() {
    let (_root,mut state)=fixture(true);
    let probe=Probe::default(); state.services.adapter=Box::new(probe.clone());
    post_ok(&mut state,"/agent/chat",json!({"message":"Explain this selection","question_quote":quote()}));
    let turn=state.user.agent_history.sessions[0].turns[0].turn_id.clone();
    let count=state.user.agent_history.sessions[0].turns.len();
    let input=json!({"message":"","entry_action":"clarify_answer","answer_ref":{"turn_id":turn,"excerpt":scenario()["answer_excerpt"]},"expected_chat_head":turn});
    let error_code = |state: &mut AppState,input: &Value| {
        let reply=prepare_agent_chat(state,&input.to_string(),"1001").err().expect("TC1 must not open execution");
        serde_json::from_str::<Value>(&reply.body).unwrap()["error_code"].as_str().unwrap().to_owned()
    };
    assert_eq!(error_code(&mut state,&input),"INTERACTION_INVALID"); // missing durable request identity
    let mut bad=input.clone(); bad["answer_ref"]["excerpt"]=json!("not delivered");
    assert_eq!(error_code(&mut state,&bad),"INTERACTION_INVALID");
    bad["answer_ref"]["turn_id"]=json!("another-chat-turn");
    assert_eq!(error_code(&mut state,&bad),"INTERACTION_INVALID");
    let mut bad=json!({"message":"","entry_action":"explain_selection","question_quote":quote()});
    // Extended selection authority comes from verified ranges, not the legacy
    // display quote alone. Make the actual range invalid.
    bad["question_quote"]["ranges"][0]["range"]["end"]=json!(999999);
    assert_ne!(error_code(&mut state,&bad),"INTERACTION_UNAVAILABLE");
    assert_eq!(state.user.agent_history.sessions[0].turns.len(),count);
    assert_eq!(probe.0.load(Ordering::SeqCst),1);
}

pub fn resolved_action(state: &AppState) -> runtime::tutor_continuation::ResolvedInteraction {
    serde_json::from_value(json!({"action":{"origin":"ui_entry","entry_action":"explain_selection"},
        "user_supplement":"","accepted_request":"帮我弄懂这段","answer_ref":null,"answer_text":null,
        "source_excerpt":scenario()["quote"],"source_bindings":[{"source_id":state.workspace.book.base.book_id,
            "source_revision":state.workspace.book.source_fingerprint(),"start_lid":"1.1","end_lid":"1.1"}],
        "target":null,"expected_chat_head":null,"presentation_follow_up":null})).unwrap()
}

#[test]
fn continuation_contract_snapshot_survives_jsonl_and_drives_current_action() {
    let (_root,mut state)=fixture(true); enable(&mut state);
    let snapshot=resolved_action(&state);
    let mut session=state.user.agent_history.sessions[0].clone();
    let turn=append_pending_agent_turn(&mut session,"操作：帮我弄懂这段".into(),None,Some(serde_json::from_value(quote()).unwrap()),
        None,None,None,None,None,None,false,"1001").unwrap();
    session.turns.last_mut().unwrap().domain.interaction=Some(snapshot.clone());
    state.user.session_store.as_mut().unwrap().accept(&mut state.user.agent_history,session,"1001").unwrap();
    let (history,store)=crate::session_store::load_chat_storage(&state.user.history_path).unwrap();
    state.user.agent_history=history; state.user.session_store=store;
    assert_eq!(state.user.agent_history.sessions[0].turns[0].domain.interaction,Some(snapshot.clone()));
    let context=teaching::prepare(&state.private_context(),&turn,&json!({"message":"实际补充"}),"1001").unwrap().unwrap();
    assert_eq!(context["current_action"],json!(snapshot));
    assert_eq!(context["current_request"], "");
    assert_eq!(context["learner_context"]["current_action"]["action"]["origin"],"ui_entry");
    let movement=json!({"move_id":"explain","object_ids":[],"capability":"explanation","kind":"explain",
        "prompt":"平均速度是总路程除以总时间。",
        "target":{"learning_focus":"速度","expected_performance":"解释时间","capability":"explanation",
            "source_bindings":snapshot.source_bindings,"object_refs":[]}});
    let ranges=[EvidenceRange { start_lid:"1.1".into(),end_lid:"1.1".into(),ranges:vec![] }];
    let mut bad=movement.clone(); bad["interaction_intent"]=json!("guided_inquiry"); bad["kind"]=json!("question");
    assert!(super::tutor_loop_tests::step_with_design(&state.private_context(),&turn,json!({"operation":"select","move":bad}),&[],&ranges).is_err());
    assert_eq!(context["status"],"observing");
    assert!(context["current_design"].is_null());
    assert!(!state.user.learning_store().unwrap().teaching_turn_events(context["binding"]["tutor_session_id"].as_str().unwrap(),&turn.turn_id).unwrap()
        .iter().any(|event|matches!(event.kind,TeachingFact::DesignRevised|TeachingFact::MoveSelected)));
}

#[test]
fn continuation_contract_no_action_cannot_claim_current_action() {
    let (_root,mut state)=fixture(true); enable(&mut state);
    let run=prepare(&mut state,json!({"message":"Explain average speed"}));
    let movement=json!({"move_id":"explain","object_ids":[],"capability":"explanation","kind":"explain",
        "interaction_intent":"direct_explanation","intent_origin":"current_action","prompt":"average speed",
        "target":{"learning_focus":"speed","expected_performance":"explain time","capability":"explanation",
            "source_bindings":resolved_action(&state).source_bindings,"object_refs":[]}});
    let error=super::tutor_loop_tests::step_with_design(&state.private_context(),&run.turn_ref,json!({"operation":"select","move":movement}),&[],
        &[EvidenceRange {start_lid:"1.1".into(),end_lid:"1.1".into(),ranges:vec![]}]).unwrap_err();
    assert!(error.message.contains("明确的练习请求"),"{}",error.message);
    let saved=&state.user.agent_history.sessions[0].turns[0];
    let mut old=serde_json::to_value(saved).unwrap(); old.as_object_mut().unwrap().remove("domain");
    let reread: AgentChatTurn=serde_json::from_value(old).unwrap();
    assert!(reread.domain.interaction.is_none());
    assert!(serde_json::to_value(reread).unwrap()["domain"].get("interaction").is_none());
}

#[test]
fn continuation_contract_candidate_snapshot_freezes_before_chat_acceptance() {
    let (_root,state)=fixture(true);
    let snapshot=resolved_action(&state);
    let mut candidate=state.user.agent_history.sessions[0].clone();
    let turn=append_pending_agent_turn(&mut candidate,"操作：帮我弄懂这段".into(),None,None,
        None,None,None,None,None,None,false,"1001").unwrap();
    assert!(state.user.agent_history.sessions[0].turns.is_empty());
    let frozen=teaching::freeze_prepare(&state.private_context(),&turn,&json!({"message":""}),Some(&snapshot),"1001").unwrap();
    let context=frozen.context.unwrap();
    assert_eq!(context["current_action"],json!(snapshot));
    assert_eq!(context["learner_context"]["current_action"],json!(snapshot));
}

#[test]
#[ignore = "configured real model; two-action TC2 first-help and continuation acceptance"]
fn continuation_tc2_live_first_help_enable_then_continue() {
    let (_root,mut state)=fixture(true);
    let config=ProviderConfig::from_env().unwrap();
    println!("TC2_LIVE_MODEL {}",config.model);
    let observations=Arc::new(std::sync::Mutex::new(Vec::new()));
    state.services.adapter=Box::new(ActionToolContract {
        inner:ProviderRegistry::adapter_from_config_with_timeout(config,Duration::from_secs(90)),
        observations:observations.clone(),
    });
    let input=selection_input(&state,"tc2-live-first");
    let first=post_ok(&mut state,"/agent/chat",input);
    println!("TC2_LIVE_FIRST {}",first);
    let original=state.user.agent_history.sessions[0].turns[0].turn_id.clone();
    assert_eq!(state.user.agent_history.sessions[0].turns[0].status,AgentAssistantStatus::Completed);
    assert!(state.user.learning_store().unwrap().state().unwrap().sessions.is_empty());
    enable(&mut state);
    assert_eq!(state.user.agent_history.sessions[0].turns.len(),1);
    let next=json!({"message":"","session_id":state.workspace.selected_chat,
        "client_request_id":"tc2-live-next","entry_action":"continue_answer",
        "answer_ref":{"turn_id":original},"expected_chat_head":original});
    let second=post_ok(&mut state,"/agent/chat",next);
    println!("TC2_LIVE_CONTINUE {}",second);
    let turn=&state.user.agent_history.sessions[0].turns[1];
    assert_eq!(turn.status,AgentAssistantStatus::Completed);
    assert!(!state.user.learning_store().unwrap().teaching_deliveries(&state.user.agent_history.sessions[0].id,&turn.turn_id).unwrap().is_empty());
    assert_eq!(state.user.learning_store().unwrap().state().unwrap().sessions.len(),1);
    let observations=observations.lock().unwrap();
    println!("TC2_LIVE_TOOL_ACTIVE {}",json!(*observations));
    assert!(observations.contains(&false) && observations.contains(&true));
}

#[test]
#[ignore = "TC2A fixed-input real-model comparison: three continuations; set TC2A_OUTPUT"]
fn continuation_tc2a_live_fixed_input_comparison() {
    let directory = std::path::PathBuf::from(std::env::var("TC2A_OUTPUT").expect("set a fresh output directory"));
    assert!(!directory.exists(), "preserve previous comparison output");
    std::fs::create_dir_all(&directory).unwrap();
    let original: Value = serde_json::from_slice(&std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/performance/tutor-continuation-tc2a/baseline-assets/original-answer.json")).unwrap()).unwrap();
    let original_answer = original["first"]["answer"].as_str().unwrap();
    let config = ProviderConfig::from_env().unwrap();
    std::fs::write(directory.join("configuration.json"), serde_json::to_vec_pretty(&json!({
        "model":config.model,"provider":format!("{:?}",config.mode),"base_url":config.base_url,
        "temperature":0,"repetitions":3,"source":SOURCE,"original_answer":original_answer,
        "entry_action":"continue_answer","user_supplement":"",
        "selection_timing":"production select persists before public final delivery; unchanged in both groups"
    })).unwrap()).unwrap();
    for repetition in 1..=3 {
        let dir = directory.join(format!("run-{repetition}"));
        std::fs::create_dir_all(&dir).unwrap();
        let (_root,mut state) = fixture(true);
        state.services.adapter = Box::new(ChatStubAdapter::scripted(vec![
            AssistantTurn { provider_continuation:None,text:None,usage_total_tokens:Some(0),tool_calls:vec![ToolCall {
                id:"fixed-source".into(),name:"book.text".into(),arguments:json!({"lid":"1.1","end_lid":"1.1"}).to_string() }] },
            AssistantTurn { provider_continuation:None,text:Some(original_answer.into()),usage_total_tokens:Some(0),tool_calls:vec![] },
        ]));
        let input = selection_input(&state,"fixed-first");
        post_ok(&mut state,"/agent/chat",input);
        let original = state.user.agent_history.sessions[0].turns[0].turn_id.clone();
        enable(&mut state);
        state.services.adapter = Box::new(TutorRequestRecorder {
            samples:Default::default(),directory:Some(dir.clone()),
            inner:ProviderRegistry::adapter_from_config_with_timeout(config.clone(),Duration::from_secs(120)),
        });
        let input = json!({"message":"","session_id":state.workspace.selected_chat,"client_request_id":"fixed-next",
            "entry_action":"continue_answer","answer_ref":{"turn_id":original},"expected_chat_head":original});
        let reply = post(&mut state,"/agent/chat",&input.to_string());
        let session = &state.user.agent_history.sessions[0];
        let turn = session.turns.last().unwrap();
        let deliveries = state.user.learning_store().unwrap().teaching_deliveries(&session.id,&turn.turn_id).unwrap();
        let store = state.user.learning_store().unwrap();
        let teaching_session = store.state().unwrap().control.current_tutor_session_id;
        let events = teaching_session.map(|id|store.teaching_turn_events(&id,&turn.turn_id).unwrap()).unwrap_or_default();
        let result = json!({"http_status":reply.status,"reply":serde_json::from_str::<Value>(&reply.body).unwrap_or(json!(reply.body)),
            "turn_status":turn.status,"deliveries":deliveries,"events":events});
        std::fs::write(dir.join("result.json"),serde_json::to_vec_pretty(&result).unwrap()).unwrap();
        println!("TC2A repetition={repetition} status={} output={}",reply.status,dir.display());
    }
}

#[test]
#[ignore = "TC3 fixed two-scenario model observation; set fresh TC3_OUTPUT"]
fn continuation_tc3_live_answer_and_optional_suggestions() {
    let directory=std::path::PathBuf::from(std::env::var("TC3_OUTPUT").expect("set a fresh output directory"));
    assert!(!directory.exists()); std::fs::create_dir_all(&directory).unwrap();
    let config=ProviderConfig::from_env().unwrap();
    let only=std::env::var("TC3_SCENARIO").ok();
    let scenarios=[("explain","请讲清楚这段平均速度定义，先帮助我理解总时间为什么在分母。"),
        ("answer-only","请只用一句话说明平均速度的定义。这次回答到此为止，不要后续建议或练习。")];
    std::fs::write(directory.join("configuration.json"),serde_json::to_vec_pretty(&json!({"model":config.model,
        "temperature":0,"source":SOURCE,"scenarios":scenarios,"only_scenario":only,"question_quote":quote(),
        "recording":"provider-private continuation omitted; public task input and tool results retained"})).unwrap()).unwrap();
    for (name,message) in scenarios {
        if only.as_deref().is_some_and(|selected| selected != name) { continue; }
        let dir=directory.join(name); std::fs::create_dir_all(&dir).unwrap();
        let (_root,mut state)=fixture(true); enable(&mut state);
        let samples=Arc::new(std::sync::Mutex::new(Vec::new()));
        state.services.adapter=Box::new(TutorRequestRecorder { samples:samples.clone(),directory:Some(dir.clone()),
            inner:ProviderRegistry::adapter_from_config_with_timeout(config.clone(),Duration::from_secs(120)) });
        let reply=post(&mut state,"/agent/chat",&json!({"message":message,"question_quote":quote()}).to_string());
        let session=&state.user.agent_history.sessions[0];
        let saved=session.turns.last().unwrap();
        let turn=AgentTurnRef { session_id:session.id.clone(),turn_id:saved.turn_id.clone(),user_turn_ordinal:0 };
        let store=state.user.learning_store().unwrap();
        let id=store.state().unwrap().control.current_tutor_session_id.unwrap();
        let events=store.teaching_turn_events(&id,&turn.turn_id).unwrap(); drop(store);
        let before=samples.lock().unwrap().len();
        let views=tc3_views(&mut state,&turn); assert_eq!(views,tc3_views(&mut state,&turn));
        assert_eq!(before,samples.lock().unwrap().len());
        std::fs::write(dir.join("result.json"),serde_json::to_vec_pretty(&json!({"http_status":reply.status,
            "reply":serde_json::from_str::<Value>(&reply.body).unwrap_or(json!(reply.body)),"events":events,
            "public_projection":views,"provider_calls":before,"read_provider_calls":0})).unwrap()).unwrap();
        println!("TC3 {name}: status={} samples={before} output={}",reply.status,dir.display());
    }
}
