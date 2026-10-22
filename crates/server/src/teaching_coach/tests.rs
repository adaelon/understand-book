use super::*;
use crate::tests::tutor_loop_tests::{fixture_with_book, fixture_delivery_review, finish, outcome};
use crate::tests::teaching_design_tests::{design_request,select_request};
use memory::learning::{TutorAction, TutorMutation};

type Samples = Arc<Mutex<Vec<(String, Value)>>>;

#[derive(Default)]
struct RecordingAdapter { samples: Samples, step:std::sync::atomic::AtomicUsize }

impl Clone for RecordingAdapter {
    fn clone(&self) -> Self { Self {samples:self.samples.clone(),step:std::sync::atomic::AtomicUsize::new(0)} }
}

fn sample_fragment(plan: &AgentRequestPlan, prefix: &str) -> Value {
    plan.ordered_messages().iter().rev().filter_map(|message|message.content.as_deref())
        .find_map(|text|text.split_once(prefix).and_then(|(_,json)|
            serde_json::Deserializer::from_str(json).into_iter::<Value>().next().and_then(Result::ok)))
        .unwrap_or(Value::Null)
}

fn guidance() -> Value {
    json!({"next_step":{"focus":"总时间包含停留时间的原因",
        "participation":{"choice":"support","reason":"先回应当前困惑",
            "support":"对照出发到抵达的完整时间线","learner_inference":null}},
        "alternatives":[{"when":"用户明确要求先直接解释",
            "next_step":{"focus":"按最新请求直接解释条件","participation":{"choice":"support",
                "reason":"当前明确请求优先","support":"完整解释必要条件","learner_inference":null}}}],
        "needs_read":[],"reason":"PRIVATE-COACH-NEXT: 根据上一轮实际讲解安排下一步；尚未看到下一句回答"})
}

impl ModelAdapter for RecordingAdapter {
    fn complete(&self, _: CompletionRequest) -> Result<ParsedResponse, AdapterError> {
        Err(AdapterError { spend_stop:None, message:"unexpected inner query".into() })
    }
    fn complete_structured(&self, request: CompletionRequest) -> Result<Value, AdapterError> {
        if request.system.contains("你是 Understand Book 的教学教练") {
            self.samples.lock().unwrap().push(("coach".into(),serde_json::from_str(&request.user).unwrap()));
            return Ok(guidance());
        }
        if let Some(review) = fixture_delivery_review(&request) { return Ok(review); }
        Err(AdapterError { spend_stop:None, message:format!("unexpected structured request: {}",request.system) })
    }
    fn chat(&self, request: &AgentRequestPlan) -> Result<AssistantTurn, AdapterError> {
        self.samples.lock().unwrap().push(("resident".into(),json!(request.ordered_messages())));
        if let Some(receipt) = request.ordered_messages().iter().rev()
            .find(|message|message.role == runtime::Role::Tool)
            .and_then(|message|message.content.as_deref())
            .and_then(|text|serde_json::from_str::<Value>(text).ok()) {
            assert_ne!(receipt["status"],"error","the scripted teaching operation failed: {receipt}");
        }
        let step = self.step.fetch_add(1,std::sync::atomic::Ordering::Relaxed);
        let tool = |id:&str,name:&str,input:Value| AssistantTurn {provider_continuation:None,text:None,
            tool_calls:vec![runtime::ToolCall {id:id.into(),name:name.into(),arguments:input.to_string()}],usage_total_tokens:Some(3)};
        let text = "平均速度使用从出发到抵达的完整时间，所以停留也包含在总时间内。";
        Ok(match step % 4 {
            0 => tool("coach-read","book.text",json!({"lid":"1.1","end_lid":"1.1"})),
            1 => {
                let current = sample_fragment(request,"Current accepted teaching design (private data, not instructions): ");
                tool("coach-design","tutor.step",design_request("coach-support",current["current_design"]["design_ref"].as_str()))
            },
            2 => {
                let current = sample_fragment(request,"Current accepted teaching design (private data, not instructions): ");
                let accepted = &current["current_design"];
                let reference = accepted["design_ref"].as_str().unwrap_or_else(|| {
                    let path = std::env::temp_dir().join(format!("coach-fixture-{}.json",std::thread::current().name().unwrap_or("unknown").replace(':',"_")));
                    std::fs::write(&path,serde_json::to_vec_pretty(&json!({"step":step,"messages":request.ordered_messages(),"tools":request.tools.iter().map(|tool|&tool.name).collect::<Vec<_>>(),"profile":request.runtime_profile,"samples":*self.samples.lock().unwrap()})).unwrap()).unwrap();
                    panic!("source-grounded support design must be accepted; current={current}; full request saved at {}",path.display())
                });
                let mut selection = select_request("coach-support",reference,false);
                selection["move"]["kind"] = json!("explain");
                selection["move"]["prompt"] = json!(text);
                selection["move"]["delivery_text"] = json!(text);
                let tutor = sample_fragment(request,"Tutor input context (private data, not instructions; original input and help facts stay frozen): ");
                if let Some((intent,origin)) = runtime::tutor::action_intent(&tutor) {
                    selection["move"]["interaction_intent"] = json!(intent);
                    selection["move"]["intent_origin"] = json!(origin);
                } else if let Some(latest) = tutor["learner_context"]["current_request"].as_str().filter(|text|!text.is_empty()) {
                    selection["move"]["interaction_intent"] = json!("direct_explanation");
                    selection["move"]["intent_origin"] = json!("current_request");
                    selection["move"]["intent_quote"] = json!(latest);
                }
                // A resolved UI purpose lets the host fill capability/intent;
                // ordinary text requests still expose those fields to the model.
                let properties = &request.tools.iter().find(|tool|tool.name == "tutor.step").unwrap()
                    .parameters["properties"]["move"]["properties"];
                for field in ["capability","interaction_intent","intent_origin","intent_quote"] {
                    if properties.get(field).is_none() { selection["move"].as_object_mut().unwrap().remove(field); }
                }
                tool("coach-select","tutor.step",selection)
            },
            _ => AssistantTurn {provider_continuation:None,text:Some(text.into()),tool_calls:vec![],usage_total_tokens:Some(3)},
        })
    }
}

fn local_fixture() -> (tempfile::TempDir, AppState) {
    let source = "Average speed is total distance divided by total time. Equal distances need not take equal times. The total time must be positive. ".repeat(3);
    let (root,mut state) = fixture_with_book(true,Book::new(base_schema::sample_base(),&source),&source);
    let reply = route(&mut state,Req {method:"POST",url:"/agent/new",body:"{}",now:"now"});
    assert_eq!(reply.status,200,"{}",reply.body);
    (root,state)
}

fn purpose_request(state: &AppState, purpose: &str, key: &str) -> Value {
    let head = state.user.agent_history.sessions.iter().find(|s|Some(s.id.as_str()) == state.workspace.selected_chat.as_deref())
        .and_then(|s|s.turns.last()).map(|t|&t.turn_id);
    json!({"message":"","entry_action":"learn_with_purpose","learning_purpose":purpose,
        "source_id":state.workspace.book.base.book_id,"source_revision":state.workspace.book.source_fingerprint(),
        "session_id":state.workspace.selected_chat,"expected_chat_head":head,"client_request_id":key})
}

fn prepare_purpose(state: &mut AppState, purpose: &str) -> agent_run::PreparedAgentChat {
    let input = purpose_request(state,purpose,&format!("purpose-{purpose}"));
    prepare_agent_chat(state,&input.to_string(),"now").unwrap_or_else(|e|panic!("{}",e.body))
}

fn run_pending_coach(user: &user_runtime::UserRuntime, adapter: &RecordingAdapter) -> CoachJob {
    let job = claim(user).unwrap().expect("system must have queued a coach without a Tutor tool call");
    let result = execute(adapter,&job,&CancellationToken::default());
    commit(user,&job,result).unwrap();
    user.learning_store().unwrap().coach_job(&job.input.job_ref).unwrap().unwrap()
}

#[test]
fn coach_local_purpose_bootstrap_support_delivery_and_next_guidance_are_automatic() {
    let (_root,mut state) = local_fixture();
    let first = prepare_purpose(&mut state,"understand");
    let adapter = RecordingAdapter::default();
    let turn = first.turn_ref.clone();
    let dependency = state.user.learning_store().unwrap().coach_turn_dependency(&turn.session_id,&turn.turn_id).unwrap().unwrap();
    assert!(dependency.job_ref.is_some());
    assert!(!prepare_execution(&state.user,None,&turn).unwrap());
    let bootstrap = run_pending_coach(&state.user,&adapter);
    assert_eq!(bootstrap.input.origin,CoachJobOrigin::StageStart);
    assert_eq!(bootstrap.status,CoachJobStatus::Ready);
    assert!(prepare_execution(&state.user,None,&turn).unwrap());
    let report = agent_run::execute_prepared(&agent_run::BorrowedAppPort(std::cell::RefCell::new(&mut state)),
        &adapter,first,Default::default());
    assert_eq!(report.reply.status,200,"{}",report.reply.body);
    let after = state.user.learning_store().unwrap().coach_job_for_source(&bootstrap.input.binding.tutor_session_id,
        CoachJobOrigin::AfterTurn,&turn.turn_id).unwrap().expect("support explanation must also schedule the coach");
    assert_eq!(after.status,CoachJobStatus::Ready,"the synchronous host drives its after-turn job after saving the answer");
    assert!(after.input.context["actual_delivery"]["answer"].as_str().unwrap().contains("完整时间"));
    let stage_events = state.user.learning_store().unwrap().teaching_turn_events(&bootstrap.input.binding.tutor_session_id,&turn.turn_id).unwrap();
    assert!(stage_events.iter().any(|e|e.kind == TeachingFact::MoveSelected));
    assert!(!stage_events.iter().any(|e|e.payload["move"]["response_expectation"].is_object()),
        "a complete support explanation need not open an activity");
    crate::teaching::record_user_delivery(&state.user,&turn,"repeat").unwrap();
    assert_eq!(state.user.learning_store().unwrap().list_coach_jobs(None,10).unwrap().len(),2);

    let latest = "先别让我回忆，请直接解释为什么停车也算时间。";
    let next = prepare_agent_chat(&mut state,&json!({"message":latest}).to_string(),"later")
        .unwrap_or_else(|e|panic!("{}",e.body));
    let next_turn = next.turn_ref.clone();
    let pending = state.user.learning_store().unwrap().coach_turn_dependency(&next_turn.session_id,&next_turn.turn_id).unwrap().unwrap();
    assert_eq!(pending.job_ref.as_deref(),Some(after.input.job_ref.as_str()));
    assert_eq!(after.input.source_ref,turn.turn_id);
    assert!(!after.input.context.to_string().contains(latest),"after-turn facts must stop before the next input");
    assert!(prepare_execution(&state.user,None,&next_turn).unwrap());
    let installed = turn_context(&state.user,None,&next_turn).unwrap().unwrap();
    assert_eq!(installed["source_turn_ref"],turn.turn_id);
    let report = agent_run::execute_prepared(&agent_run::BorrowedAppPort(std::cell::RefCell::new(&mut state)),
        &adapter,next,Default::default());
    assert_eq!(report.reply.status,200,"{}",report.reply.body);
    let samples = adapter.samples.lock().unwrap();
    let resident = samples.iter().filter(|(kind,_)|kind == "resident").last().unwrap().1.to_string();
    assert!(resident.contains(latest));
    assert!(resident.contains("PRIVATE-COACH-NEXT"));
    assert!(resident.contains(installed["coach_ref"].as_str().unwrap()));
    assert_eq!(samples.iter().filter(|(kind,_)|kind == "coach").count(),3,
        "one bootstrap and one system call after each of the two delivered turns");
    drop(samples);
    let public = serde_json::to_string(&agent_history_response(&state.user.agent_history,&state.workspace.book,
        state.workspace.selected_chat.as_deref()).unwrap()).unwrap();
    assert!(!public.contains("PRIVATE-COACH-NEXT"));
    assert!(!public.contains("coach_ref"));
    assert!(!public.contains("coach_adjustment"));
}

#[test]
fn coach_network_admission_waits_without_sampling_then_installs_guidance() {
    let f = crate::tests::mu5_tests::Fixture::new();
    let adapter = RecordingAdapter::default();
    let factory = adapter.clone();
    f.access.runs.configure_fake(move ||Box::new(factory.clone()));
    let (source_root,_source_state) = local_fixture();
    let reference = f.access.library.lock().unwrap().publish(source_root.path()).unwrap().reference;
    f.access.library.lock().unwrap().grant("A",&reference).unwrap();
    let w = f.create(&f.a,&reference,"coach-network");
    let w = f.action(&f.a,&w,"coach-network","chat/new",json!({}));
    let handle = f.access.users.lock().unwrap().get_for_recovery("A","now").unwrap();
    handle.lock().unwrap().learning_store().unwrap().mutate(&TutorMutation {operation_id:"coach-enable".into(),
        expected_revision:0,action:TutorAction::SetEnabled {enabled:true}},"now").unwrap();
    let publication = f.access.library.lock().unwrap().load("A",&reference).unwrap();
    let input = json!({"generation":w["generation"],"expected_revision":w["revision"],"attachment_id":"coach-network",
        "session_id":w["selected_chat"],"client_request_id":"coach-network-first","expected_chat_head":null,
        "message":"","entry_action":"learn_with_purpose","learning_purpose":"understand",
        "source_id":publication.book.base.book_id,"source_revision":publication.book.source_fingerprint()});
    let path = format!("/api/workspaces/{}/agent/runs",w["workspace_id"].as_str().unwrap());
    let (status,accepted) = f.call(&f.a,"POST",&path,input.clone());
    assert_eq!(status,202,"{accepted}");
    assert!(!f.access.runs.run_one(&f.access).unwrap(),"pending guidance must leave the Run queued");
    assert!(adapter.samples.lock().unwrap().is_empty(),"waiting must not consume a main model call");
    assert!(tick_network(&f.access,&handle,&CancellationToken::default()).unwrap());
    assert!(f.access.runs.run_one(&f.access).unwrap());
    assert_eq!(f.call(&f.a,"POST",&path,input).1["turn_id"],accepted["turn_id"]);
    let first_turn = {
        let user = handle.lock().unwrap();
        let saved = user.agent_history.sessions.iter().flat_map(|s|&s.turns).find(|t|t.turn_id == accepted["turn_id"]).unwrap();
        assert_eq!(saved.status,AgentAssistantStatus::Completed,"{:?}",saved.error);
        assert!(!saved.outcome.as_ref().unwrap().incomplete,
            "outcome={} samples={}",json!(saved.outcome),json!(*adapter.samples.lock().unwrap()));
        AgentTurnRef {session_id:w["selected_chat"].as_str().unwrap().into(),turn_id:saved.turn_id.clone(),user_turn_ordinal:saved.user_turn_ordinal}
    };
    let w = f.get(&f.a,&w);
    let latest = "我想先知道条件为什么必要，直接讲解。";
    let input = json!({"generation":w["generation"],"expected_revision":w["revision"],"attachment_id":"coach-network",
        "session_id":w["selected_chat"],"client_request_id":"coach-network-second","message":latest});
    let (status,next) = f.call(&f.a,"POST",&path,input);
    assert_eq!(status,202,"{next}");
    let samples_before = adapter.samples.lock().unwrap().len();
    assert!(!f.access.runs.run_one(&f.access).unwrap());
    assert_eq!(adapter.samples.lock().unwrap().len(),samples_before);
    assert!(tick_network(&f.access,&handle,&CancellationToken::default()).unwrap());
    assert!(f.access.runs.run_one(&f.access).unwrap());
    let user = handle.lock().unwrap();
    let saved = user.agent_history.sessions.iter().flat_map(|s|&s.turns).find(|t|t.turn_id == next["turn_id"]).unwrap();
    assert_eq!(saved.status,AgentAssistantStatus::Completed,"{:?}",saved.error);
    let dependency = user.learning_store().unwrap().coach_turn_context(first_turn.session_id.as_str(),&saved.turn_id).unwrap().unwrap();
    assert_eq!(dependency.context.as_ref().unwrap()["source_turn_ref"],first_turn.turn_id);
    let jobs = user.learning_store().unwrap().list_coach_jobs(None,10).unwrap();
    assert_eq!(jobs.len(),3,"one start plus exactly one task per delivered turn");
    let samples = adapter.samples.lock().unwrap();
    let current = samples.iter().filter(|(kind,_)|kind == "resident").last().unwrap().1.to_string();
    assert!(current.contains(latest) && current.contains("PRIVATE-COACH-NEXT"));
    assert_eq!(samples.iter().filter(|(kind,_)|kind == "coach").count(),2);
}

#[test]
fn coach_discover_does_not_create_bootstrap_or_after_turn_tasks() {
    let (_root,mut state) = local_fixture();
    let before = state.user.learning_store().unwrap().state().unwrap();
    let run = prepare_purpose(&mut state,"discover");
    assert!(prepare_execution(&state.user,None,&run.turn_ref).unwrap());
    assert!(turn_context(&state.user,None,&run.turn_ref).unwrap().is_none());
    finish(&mut state,&run.turn_ref,&outcome(None));
    assert!(state.user.learning_store().unwrap().list_coach_jobs(None,10).unwrap().is_empty());
    assert_eq!(state.user.learning_store().unwrap().state().unwrap(),before);
}

#[test]
fn coach_successful_delivery_without_a_selected_move_still_enqueues_once() {
    let (_root,mut state) = local_fixture();
    let run = prepare_purpose(&mut state,"understand");
    // Other supported delivery/recovery paths can commit an explanation without
    // a selected move. The scheduler depends on actual delivery, not tool use.
    finish(&mut state,&run.turn_ref,&outcome(None));
    let stage = run.tutor.as_ref().unwrap()["binding"]["tutor_session_id"].as_str().unwrap();
    let store = state.user.learning_store().unwrap();
    assert!(!store.teaching_turn_events(stage,&run.turn_ref.turn_id).unwrap().iter()
        .any(|event|event.kind == TeachingFact::MoveSelected));
    let task = store.coach_job_for_source(stage,CoachJobOrigin::AfterTurn,&run.turn_ref.turn_id).unwrap().unwrap();
    assert_eq!(task.status,CoachJobStatus::Pending);
    assert_eq!(task.input.context["actual_delivery"]["answer"],"Compare total distance and total time.");
    drop(store);
    crate::teaching::record_user_delivery(&state.user,&run.turn_ref,"repeat").unwrap();
    assert_eq!(state.user.learning_store().unwrap().list_coach_jobs(None,10).unwrap().len(),2,
        "one bootstrap and one after-turn task despite repeated delivery notification");
}

#[test]
fn coach_pause_or_new_stage_cannot_install_late_old_guidance() {
    for replace_stage in [false,true] {
        let (_root,mut state) = local_fixture();
        let run = prepare_purpose(&mut state,"understand");
        let job = claim(&state.user).unwrap().unwrap();
        let view = state.user.learning_store().unwrap().state().unwrap();
        let action = if replace_stage {
            TutorAction::Start {user_intent:"新的探索问题".into(),explicit_constraints:vec![],
                material_scope:view.sessions[&job.input.binding.tutor_session_id].material_scope.clone(),
                default_teaching_intent:None,learning_purpose:Some(memory::learning::LearningPurpose::Explore)}
        } else { TutorAction::Pause {session_id:job.input.binding.tutor_session_id.clone()} };
        state.user.learning_store().unwrap().mutate(&TutorMutation {operation_id:"changed".into(),
            expected_revision:view.control.revision,action},"later").unwrap();
        commit(&state.user,&job,Ok(guidance())).unwrap();
        assert_eq!(state.user.learning_store().unwrap().coach_job(&job.input.job_ref).unwrap().unwrap().status,CoachJobStatus::Cancelled);
        assert!(prepare_execution(&state.user,None,&run.turn_ref).unwrap());
        let context = turn_context(&state.user,None,&run.turn_ref).unwrap().unwrap();
        assert_eq!(context["status"],"unavailable");
        assert!(context.get("coach_ref").is_none());
    }
}

#[test]
fn coach_failed_task_is_explicit_and_does_not_block_requested_help() {
    let (_root,mut state) = local_fixture();
    let run = prepare_purpose(&mut state,"understand");
    let job = claim(&state.user).unwrap().unwrap();
    commit(&state.user,&job,Err(error("TUTOR_COACH_FAILED","fixture provider failed"))).unwrap();
    assert!(prepare_execution(&state.user,None,&run.turn_ref).unwrap());
    let context = turn_context(&state.user,None,&run.turn_ref).unwrap().unwrap();
    assert_eq!(context["status"],"unavailable");
    assert_eq!(context["job_status"],"failed");
    assert!(context.get("coach_ref").is_none());
    let adapter = RecordingAdapter::default();
    let report = agent_run::execute_prepared(&agent_run::BorrowedAppPort(std::cell::RefCell::new(&mut state)),
        &adapter,run,Default::default());
    assert_eq!(report.reply.status,200,"{}",report.reply.body);
    let samples = adapter.samples.lock().unwrap();
    let calls: Vec<_> = samples.iter().filter(|(kind,_)|kind == "coach").collect();
    assert_eq!(calls.len(),1,"the new successful help turn gets its own after-turn coaching");
    assert_eq!(calls[0].1["trigger"],"after_turn","the failed bootstrap is not silently regenerated");
    let failed = state.user.learning_store().unwrap().coach_job(&job.input.job_ref).unwrap().unwrap();
    assert_eq!(failed.status,CoachJobStatus::Failed);
    assert_eq!(failed.attempt,1);
    assert!(samples.iter().filter(|(kind,_)|kind == "resident").last().unwrap().1.to_string().contains("unavailable"));
}

#[test]
fn coach_recovery_repairs_last_active_delivery_before_incomplete_or_discover_turn() {
    for later_discover in [false,true] {
        let (root,mut state) = local_fixture();
        let first = prepare_purpose(&mut state,"understand");
        finish(&mut state,&first.turn_ref,&outcome(None));
        let stage = first.tutor.as_ref().unwrap()["binding"]["tutor_session_id"].as_str().unwrap();
        let after = state.user.learning_store().unwrap().coach_job_for_source(stage,
            CoachJobOrigin::AfterTurn,&first.turn_ref.turn_id).unwrap().unwrap();
        // Simulate the explicit crash window: answer and its delivery facts were
        // saved, but registration of this one after-turn task did not persist.
        let db = rusqlite::Connection::open(root.path().join("learning.db")).unwrap();
        assert_eq!(db.execute("DELETE FROM teaching_coach_jobs WHERE job_ref=?",[&after.input.job_ref]).unwrap(),1);
        drop(db);
        let later = if later_discover {
            prepare_purpose(&mut state,"discover")
        } else {
            prepare_agent_chat(&mut state,r#"{"message":"再解释一个细节，但本次回答会中断。"}"#,"later")
                .unwrap_or_else(|e|panic!("{}",e.body))
        };
        let mut later_outcome = outcome(None);
        later_outcome.incomplete = !later_discover;
        finish(&mut state,&later.turn_ref,&later_outcome);
        assert!(state.user.learning_store().unwrap().coach_job_for_source(stage,
            CoachJobOrigin::AfterTurn,&first.turn_ref.turn_id).unwrap().is_none());
        recover_user(&state.user).unwrap();
        let repaired = state.user.learning_store().unwrap().coach_job_for_source(stage,
            CoachJobOrigin::AfterTurn,&first.turn_ref.turn_id).unwrap().expect("the last successful active turn still needs its coach");
        assert_eq!(repaired.status,CoachJobStatus::Pending);
        assert_eq!(repaired.input.source_ref,first.turn_ref.turn_id);
        assert!(!repaired.input.context["dialogue"].as_array().unwrap().iter()
            .any(|row|row["turn_ref"] == later.turn_ref.turn_id));
        assert!(state.user.learning_store().unwrap().coach_job_for_source(stage,
            CoachJobOrigin::AfterTurn,&later.turn_ref.turn_id).unwrap().is_none(),
            "an incomplete or ordinary overview turn must not get an active-teaching task");
    }
}

#[test]
fn coach_completed_result_retries_storage_without_another_model_call() {
    let (_root,mut state) = local_fixture();
    let run = prepare_purpose(&mut state,"understand");
    let job = claim(&state.user).unwrap().unwrap();
    let adapter = RecordingAdapter::default();
    let cancel = CancellationToken::default();
    let result = execute(&adapter,&job,&cancel);
    assert!(result.is_ok());
    let mut saves = 0;
    save_result(&cancel,result,|result| {
        saves += 1;
        if saves == 1 { return Err(error("LEARNING_STORAGE_UNAVAILABLE","fixture: temporary save failure")); }
        commit(&state.user,&job,result)
    }).unwrap();
    assert_eq!(saves,2);
    assert_eq!(adapter.samples.lock().unwrap().iter().filter(|(kind,_)|kind == "coach").count(),1);
    let saved = state.user.learning_store().unwrap().coach_job(&job.input.job_ref).unwrap().unwrap();
    assert_eq!(saved.status,CoachJobStatus::Ready);
    assert_eq!(saved.attempt,1);
    assert!(prepare_execution(&state.user,None,&run.turn_ref).unwrap());
    assert_eq!(turn_context(&state.user,None,&run.turn_ref).unwrap().unwrap()["status"],"ready");
}
