use super::*;
use crate::tests::tutor_loop_tests::{fixture_with_book, fixture_delivery_review, finish, outcome};
use memory::learning::{TutorAction, TutorMutation};

type Samples = Arc<Mutex<Vec<(String, Value)>>>;

#[derive(Default)]
struct RecordingAdapter { samples: Samples, step:std::sync::atomic::AtomicUsize }

impl Clone for RecordingAdapter {
    fn clone(&self) -> Self { Self {samples:self.samples.clone(),step:std::sync::atomic::AtomicUsize::new(0)} }
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

fn fixture_guidance(input: &Value) -> Value {
    let mut output = guidance();
    output["observations"] = Value::Array(input["observation_facts"].as_array().into_iter().flatten()
        .filter(|fact| fact["already_interpreted"] != true).map(|fact| json!({
            "fact_ref":fact["fact_ref"], "decision":"no_new_information",
            "reason":"此脚本仅验证普通调用机制，没有新增理解判断；理解提取由独立观察用例覆盖。"
        })).collect());
    output.as_object_mut().unwrap().retain(|field,_| matches!(field.as_str(),"observations"|"reason"));
    output
}

impl ModelAdapter for RecordingAdapter {
    fn complete(&self, _: CompletionRequest) -> Result<ParsedResponse, AdapterError> {
        Err(AdapterError { spend_stop:None, message:"unexpected inner query".into() })
    }
    fn complete_structured(&self, request: CompletionRequest) -> Result<Value, AdapterError> {
        if request.system.contains("你整理 Understand Book 普通对话中已经发生的理解变化") {
            let input: Value = serde_json::from_str(&request.user).unwrap();
            self.samples.lock().unwrap().push(("coach".into(),input.clone()));
            return Ok(fixture_guidance(&input));
        }
        if let Some(review) = fixture_delivery_review(&request) { return Ok(review); }
        Err(AdapterError { spend_stop:None, message:format!("unexpected structured request: {}",request.system) })
    }
    fn chat(&self, request: &AgentRequestPlan) -> Result<AssistantTurn, AdapterError> {
        self.samples.lock().unwrap().push(("resident".into(),json!(request.ordered_messages())));
        let step = self.step.fetch_add(1,std::sync::atomic::Ordering::Relaxed);
        Ok(if step % 2 == 0 {
            AssistantTurn {provider_continuation:None,text:None,tool_calls:vec![runtime::ToolCall {
                id:"read".into(),name:"book.text".into(),arguments:json!({"lid":"1.1","end_lid":"1.1"}).to_string()}],usage_total_tokens:Some(3)}
        } else {
            AssistantTurn {provider_continuation:None,text:Some("Elapsed time includes the stop: the clock keeps running.".into()),tool_calls:vec![],usage_total_tokens:Some(3)}
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
    json!({"message":if purpose == "discover" {""} else {"I do not understand why the stopped time counts."},"entry_action":"learn_with_purpose","learning_purpose":purpose,
        "source_id":state.workspace.book.base.book_id,"source_revision":state.workspace.book.source_fingerprint(),
        "session_id":state.workspace.selected_chat,"expected_chat_head":head,"client_request_id":key})
}

fn prepare_purpose(state: &mut AppState, purpose: &str) -> agent_run::PreparedAgentChat {
    let input = purpose_request(state,purpose,&format!("purpose-{purpose}"));
    prepare_agent_chat(state,&input.to_string(),"now").unwrap_or_else(|e|panic!("{}",e.body))
}

#[test]
fn coach_local_claim_skips_other_books_and_revisions_before_the_page_limit() {
    let (_root, state) = local_fixture();
    let book = state.workspace.book.clone();
    let input = CoachJobInput {
        job_ref: "current-source".into(), origin: CoachJobOrigin::AfterTurn,
        source_ref: "current-turn".into(), workspace_goal_revision: None,
        context: json!({"observation_facts":[]}),
        binding: TeachingBinding {
            tutor_session_id: "stage".into(), session_revision: 0, control_revision: 0,
            source_id: book.base.book_id.clone(), source_revision: book.source_fingerprint().into(),
            map_revision: None, chat_session_id: "chat".into(), turn_id: "current-turn".into(),
        },
    };
    let mut store = state.user.learning_store().unwrap();
    for index in 0..33 {
        let mut other = input.clone();
        other.job_ref = format!("other-source-{index}");
        other.source_ref = format!("other-turn-{index}");
        other.binding.turn_id = other.source_ref.clone();
        if index % 2 == 0 {
            other.binding.source_id = "another-book".into();
        } else {
            other.binding.source_revision = "previous-material-revision".into();
        }
        store.ensure_coach_job(&other, "now").unwrap();
    }
    let state = Arc::new(Mutex::new(state));
    assert!(!tick_local(&state, None, &CancellationToken::default()).unwrap(),
        "a local tick must leave jobs for unloaded material pending");
    assert!(store.list_coach_jobs(None, 40).unwrap().iter().all(|job|
        job.status == CoachJobStatus::Pending && job.attempt == 0));
    store.ensure_coach_job(&input, "later").unwrap();
    let state = state.lock().unwrap();
    let claimed = claim_for_source(&state.user, &book).unwrap().unwrap();
    assert_eq!(claimed.input.job_ref, input.job_ref);
    assert_eq!(claimed.attempt, 1);
    assert!(store.list_coach_jobs(Some(CoachJobStatus::Pending), 40).unwrap().iter().all(|job|job.attempt == 0));
    assert_eq!(claim(&state.user).unwrap().unwrap().input.job_ref, "other-source-0",
        "the network worker can still discover all source books");
}


#[test]
fn coach_ordinary_turns_record_observations_without_bootstrap_or_next_turn_planning() {
    let (_root,mut state) = local_fixture();
    let adapter = RecordingAdapter::default();
    let first = prepare_purpose(&mut state,"understand");
    let turn = first.turn_ref.clone();
    let stage = first.tutor.as_ref().unwrap()["binding"]["tutor_session_id"].as_str().unwrap().to_owned();
    assert!(state.user.learning_store().unwrap().list_coach_jobs(None,10).unwrap().is_empty());
    assert!(turn_context(&state.user,None,&turn).unwrap().is_none());
    let report = agent_run::execute_prepared(&agent_run::BorrowedAppPort(std::cell::RefCell::new(&mut state)),
        &adapter,first,Default::default());
    assert_eq!(report.reply.status,200,"{}",report.reply.body);
    let after = state.user.learning_store().unwrap().coach_job_for_source(&stage,CoachJobOrigin::AfterTurn,&turn.turn_id).unwrap().unwrap();
    assert_eq!(after.status,CoachJobStatus::Ready);
    assert!(after.input.context["actual_delivery"]["answer"].as_str().unwrap().contains("clock keeps running"));
    let events=state.user.learning_store().unwrap().teaching_turn_events(&stage,&turn.turn_id).unwrap();
    assert!(!events.iter().any(|e|matches!(e.kind,TeachingFact::MoveSelected|TeachingFact::DesignRevised)));
    crate::teaching::record_user_delivery(&state.user,&turn,"repeat").unwrap();
    assert_eq!(state.user.learning_store().unwrap().list_coach_jobs(None,10).unwrap().len(),1);

    let latest="Please explain why the stopped interval still counts.";
    let next=prepare_agent_chat(&mut state,&json!({"message":latest}).to_string(),"later").unwrap_or_else(|e|panic!("{}",e.body));
    assert!(turn_context(&state.user,None,&next.turn_ref).unwrap().is_none());
    assert!(!after.input.context.to_string().contains(latest),"historical observations stop before the next input");
    let report=agent_run::execute_prepared(&agent_run::BorrowedAppPort(std::cell::RefCell::new(&mut state)),
        &adapter,next,Default::default());
    assert_eq!(report.reply.status,200,"{}",report.reply.body);
    let samples=adapter.samples.lock().unwrap();
    assert_eq!(samples.iter().filter(|(kind,_)|kind=="coach").count(),2,"one observation per delivered turn, no bootstrap");
    let resident=samples.iter().filter(|(kind,_)|kind=="resident").last().unwrap().1.to_string();
    assert!(resident.contains(latest));
    assert!(!resident.contains("PRIVATE-COACH-NEXT"));
    let public=serde_json::to_string(&agent_history_response(&state.user.agent_history,&state.workspace.book,state.workspace.selected_chat.as_deref()).unwrap()).unwrap();
    assert!(!public.contains("coach_adjustment") && !public.contains("coach_ref"));
}

#[test]
fn coach_network_pending_and_running_observations_do_not_block_turns() {
    let f=crate::tests::mu5_tests::Fixture::new();
    let adapter=RecordingAdapter::default();let factory=adapter.clone();
    f.access.runs.configure_fake(move ||Box::new(factory.clone()));
    let (source_root,_source_state)=local_fixture();
    let reference=f.access.library.lock().unwrap().publish(source_root.path()).unwrap().reference;
    f.access.library.lock().unwrap().grant("A",&reference).unwrap();
    let w=f.create(&f.a,&reference,"coach-network");
    let mut w=f.action(&f.a,&w,"coach-network","chat/new",json!({}));
    let handle=f.access.users.lock().unwrap().get_for_recovery("A","now").unwrap();
    let publication=f.access.library.lock().unwrap().load("A",&reference).unwrap();
    let path=format!("/api/workspaces/{}/agent/runs",w["workspace_id"].as_str().unwrap());
    let mut pending=None;
    for index in 0..2 {
        let input=json!({"generation":w["generation"],"expected_revision":w["revision"],"attachment_id":"coach-network",
            "session_id":w["selected_chat"],"client_request_id":format!("ordinary-{index}"),"message":"Explain the stopped interval."});
        let (status,accepted)=f.call(&f.a,"POST",&path,input.clone());assert_eq!(status,202,"{accepted}");
        assert!(f.access.runs.run_one(&f.access).unwrap());
        assert_eq!(f.call(&f.a,"POST",&path,input).1["turn_id"],accepted["turn_id"]);
        {
            let user=handle.lock().unwrap();
            let saved=user.agent_history.sessions.iter().flat_map(|s|&s.turns).find(|t|t.turn_id==accepted["turn_id"]).unwrap();
            assert_eq!(saved.status,AgentAssistantStatus::Completed,"{:?}",saved.error);
            assert!(!saved.outcome.as_ref().unwrap().incomplete);
            let dependency=user.learning_store().unwrap().coach_turn_context(w["selected_chat"].as_str().unwrap(),&saved.turn_id).unwrap().unwrap();
            assert!(dependency.job_ref.is_none() && dependency.context.is_none());
            if index==0 {pending=claim(&user).unwrap();}
        }
        w=f.get(&f.a,&w);
    }
    assert!(!adapter.samples.lock().unwrap().iter().any(|(kind,_)|kind=="coach"),"foreground does not wait for observations");
    let job=pending.unwrap();assert_eq!(job.input.origin,CoachJobOrigin::AfterTurn);
    let result=execute(&adapter,&job,&publication.book,&CancellationToken::default());
    commit(&handle.lock().unwrap(),&job,result).unwrap();
    let user=handle.lock().unwrap();let jobs=user.learning_store().unwrap().list_coach_jobs(None,10).unwrap();
    assert_eq!(jobs.len(),2);assert!(jobs.iter().all(|job|job.input.origin==CoachJobOrigin::AfterTurn));
    assert_eq!(jobs.iter().filter(|job|job.status==CoachJobStatus::Ready).count(),1);
    assert_eq!(jobs.iter().filter(|job|job.status==CoachJobStatus::Pending).count(),1);
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
    assert_eq!(state.user.learning_store().unwrap().list_coach_jobs(None,10).unwrap().len(),1,
        "one observation despite repeated delivery notification");
}

#[test]
fn coach_observations_survive_goal_changes_without_becoming_next_turn_plans() {
    for replace_goal in [false,true] {
        let (_root,mut state)=local_fixture();let run=prepare_purpose(&mut state,"understand");
        finish(&mut state,&run.turn_ref,&outcome(None));
        let job=claim(&state.user).unwrap().unwrap();
        let view=state.user.learning_store().unwrap().state().unwrap();
        let goal=view.control.current_tutor_session_id.as_ref().unwrap();
        let action=if replace_goal {TutorAction::Start {user_intent:"A new learning goal".into(),explicit_constraints:vec![],
            material_scope:view.sessions[goal].material_scope.clone(),default_teaching_intent:None,learning_purpose:None}}
            else {TutorAction::Pause {session_id:goal.clone()}};
        state.user.learning_store().unwrap().mutate(&TutorMutation {operation_id:"changed".into(),expected_revision:view.control.revision,action},"later").unwrap();
        let result=execute(&RecordingAdapter::default(),&job,&state.workspace.book,&CancellationToken::default());
        commit(&state.user,&job,result).unwrap();
        assert_eq!(state.user.learning_store().unwrap().coach_job(&job.input.job_ref).unwrap().unwrap().status,CoachJobStatus::Ready);
        let next=prepare_agent_chat(&mut state,r#"{"message":"Explain the clock."}"#,"later").unwrap_or_else(|e|panic!("{}",e.body));
        assert!(turn_context(&state.user,None,&next.turn_ref).unwrap().is_none());
    }
}

#[test]
fn coach_failed_observation_is_explicit_and_does_not_block_requested_help() {
    let (_root,mut state)=local_fixture();let first=prepare_purpose(&mut state,"understand");
    finish(&mut state,&first.turn_ref,&outcome(None));
    let job=claim(&state.user).unwrap().unwrap();
    commit(&state.user,&job,Err(error("TUTOR_COACH_FAILED","fixture provider failed"))).unwrap();
    let next=prepare_agent_chat(&mut state,r#"{"message":"Please explain the clock."}"#,"later").unwrap_or_else(|e|panic!("{}",e.body));
    assert!(turn_context(&state.user,None,&next.turn_ref).unwrap().is_none());
    let adapter=RecordingAdapter::default();
    let report=agent_run::execute_prepared(&agent_run::BorrowedAppPort(std::cell::RefCell::new(&mut state)),&adapter,next,Default::default());
    assert_eq!(report.reply.status,200,"{}",report.reply.body);
    assert_eq!(adapter.samples.lock().unwrap().iter().filter(|(kind,_)|kind=="coach").count(),1);
    let failed=state.user.learning_store().unwrap().coach_job(&job.input.job_ref).unwrap().unwrap();
    assert_eq!(failed.status,CoachJobStatus::Failed);assert_eq!(failed.attempt,1,"no silent replay of the failed observation");
}

#[test]
fn coach_background_spend_stop_does_not_reject_next_foreground_context() {
    let (_root,mut state)=local_fixture();let run=prepare_purpose(&mut state,"understand");
    finish(&mut state,&run.turn_ref,&outcome(None));
    let job=claim(&state.user).unwrap().unwrap();let stop=runtime::model_spend::SpendStop::InsufficientAllowance;
    commit(&state.user,&job,Err(stop.tool_error())).unwrap();
    let next=prepare_agent_chat(&mut state,r#"{"message":"Explain elapsed time."}"#,"later").unwrap_or_else(|e|panic!("{}",e.body));
    assert!(prepare_execution(&state.user,None,&next.turn_ref).unwrap());
    assert!(turn_context(&state.user,None,&next.turn_ref).unwrap().is_none());
    let failed=state.user.learning_store().unwrap().coach_job(&job.input.job_ref).unwrap().unwrap();
    assert_eq!(failed.status,CoachJobStatus::Failed);
    assert_eq!(failed.error.unwrap()["error_code"],stop.tool_error().error_code);
}

#[test]
fn coach_invalid_correction_persists_failed_job_without_replaying_worker_attempt() {
    struct InvalidCoach(std::sync::atomic::AtomicUsize);
    impl ModelAdapter for InvalidCoach {
        fn complete(&self, _:CompletionRequest) -> Result<ParsedResponse,AdapterError> { unreachable!() }
        fn chat(&self, _:&AgentRequestPlan) -> Result<AssistantTurn,AdapterError> { unreachable!() }
        fn complete_structured(&self, _:CompletionRequest) -> Result<Value,AdapterError> {
            self.0.fetch_add(1,std::sync::atomic::Ordering::Relaxed);
            let mut output = guidance();
            output["next_step"]["reason"] = json!("misplaced output field");
            Ok(output)
        }
    }
    let (_root,mut state) = local_fixture();
    let run = prepare_purpose(&mut state,"understand");
    finish(&mut state,&run.turn_ref,&outcome(None));
    let job = claim(&state.user).unwrap().unwrap();
    let adapter = InvalidCoach(std::sync::atomic::AtomicUsize::new(0));
    let result = execute(&adapter,&job,&state.workspace.book,&CancellationToken::default());
    assert!(result.is_err());
    commit(&state.user,&job,result).unwrap();
    assert_eq!(adapter.0.load(std::sync::atomic::Ordering::Relaxed),2);
    let failed = state.user.learning_store().unwrap().coach_job(&job.input.job_ref).unwrap().unwrap();
    assert_eq!(failed.status,CoachJobStatus::Failed);
    assert_eq!(failed.attempt,1,"a correction is a second observed model call within the original claimed worker attempt");
    assert_eq!(failed.attempts.len(),1);
    assert_eq!(failed.attempts[0].status,CoachJobStatus::Failed);
    assert_eq!(failed.error,failed.attempts[0].error);
    assert!(failed.error.as_ref().unwrap()["message"].as_str().unwrap().contains("after one correction"));
    assert!(prepare_execution(&state.user,None,&run.turn_ref).unwrap());
    assert!(turn_context(&state.user,None,&run.turn_ref).unwrap().is_none(), "the observation failure never becomes foreground planning");
}

#[test]
fn coach_recovery_repairs_current_stage_and_keeps_usable_incomplete_observations() {
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
            CoachJobOrigin::AfterTurn,&first.turn_ref.turn_id).unwrap();
        if later_discover {
            let repaired=repaired.expect("a button-only overview does not replace the latest authored exchange");
            assert_eq!(repaired.status,CoachJobStatus::Pending);
            assert_eq!(repaired.input.source_ref,first.turn_ref.turn_id);
            assert!(!repaired.input.context["dialogue"].as_array().unwrap().iter()
                .any(|row|row["turn_ref"] == later.turn_ref.turn_id));
        } else {
            assert!(repaired.is_none(), "startup repairs the latest authored exchange without replaying older observation work");
        }
        let later_job = state.user.learning_store().unwrap().coach_job_for_source(stage,
            CoachJobOrigin::AfterTurn,&later.turn_ref.turn_id).unwrap();
        if later_discover {
            assert!(later_job.is_none(),"an overview action with no authored expression has no observation to schedule");
        } else {
            let later_job = later_job.expect("a usable incomplete delivery still needs its learner expression recorded");
            assert_eq!(later_job.status,CoachJobStatus::Pending);
            assert_eq!(later_job.input.source_ref,later.turn_ref.turn_id);
            assert!(!later_job.input.context["observation_facts"].as_array().unwrap().is_empty());
        }
    }
}

#[test]
fn coach_recovery_keeps_the_saved_observation_when_the_old_goal_is_paused() {
    let (_root,mut state)=local_fixture();let run=prepare_purpose(&mut state,"understand");
    let stage=run.tutor.as_ref().unwrap()["binding"]["tutor_session_id"].as_str().unwrap().to_owned();
    finalize_agent_turn_completed(&mut state,&run.turn_ref,&outcome(None),&[],"saved").unwrap();
    let view=state.user.learning_store().unwrap().state().unwrap();let goal=view.control.current_tutor_session_id.clone().unwrap();
    state.user.learning_store().unwrap().mutate(&TutorMutation {operation_id:"recovery-pause".into(),expected_revision:view.control.revision,
        action:TutorAction::Pause {session_id:goal.clone()}},"paused").unwrap();
    recover_user(&state.user).unwrap();recover_user(&state.user).unwrap();
    let store=state.user.learning_store().unwrap();
    assert_eq!(store.state().unwrap().sessions[&goal].status,memory::learning::TutorSessionStatus::Paused);
    let jobs=store.list_coach_jobs(None,20).unwrap();assert_eq!(jobs.len(),1);assert_eq!(jobs[0].input.origin,CoachJobOrigin::AfterTurn);
    assert!(store.teaching_turn_events(&stage,&run.turn_ref.turn_id).unwrap().iter().any(|event|event.payload["usage"]["kind"]=="assistant_delivery"));
}

#[test]
fn coach_recovery_repairs_saved_answer_delivery_and_registration_once() {
    for incomplete in [false,true] {
        let (_root,mut state)=local_fixture();
        let run=prepare_purpose(&mut state,"understand");
        let stage=run.tutor.as_ref().unwrap()["binding"]["tutor_session_id"].as_str().unwrap().to_owned();
        let mut saved=outcome(None);
        saved.incomplete=incomplete;
        finalize_agent_turn_completed(&mut state,&run.turn_ref,&saved,&[],"saved").unwrap();
        let before=state.user.learning_store().unwrap().list_coach_jobs(None,20).unwrap().len();
        assert!(!state.user.learning_store().unwrap().teaching_turn_events(&stage,&run.turn_ref.turn_id).unwrap()
            .iter().any(|event|event.payload["usage"]["kind"]=="assistant_delivery"));
        recover_user(&state.user).unwrap();
        let store=state.user.learning_store().unwrap();
        let job=store.coach_job_for_source(&stage,CoachJobOrigin::AfterTurn,&run.turn_ref.turn_id).unwrap().unwrap();
        assert_eq!(job.status,CoachJobStatus::Pending);
        assert_eq!(job.input.context["actual_delivery"]["answer"],json!(saved.answer));
        let events=store.teaching_turn_events(&stage,&run.turn_ref.turn_id).unwrap();
        assert_eq!(events.iter().filter(|event|event.payload["usage"]["kind"]=="assistant_delivery").count(),1);
        assert_eq!(store.list_coach_jobs(None,20).unwrap().len(),before+1);
        drop(store);
        recover_user(&state.user).unwrap();
        let store=state.user.learning_store().unwrap();
        assert_eq!(store.list_coach_jobs(None,20).unwrap().len(),before+1);
        assert_eq!(store.teaching_turn_events(&stage,&run.turn_ref.turn_id).unwrap(),events,
            "repeated startup must preserve the original delivery facts");
        assert_eq!(store.coach_job_for_source(&stage,CoachJobOrigin::AfterTurn,&run.turn_ref.turn_id).unwrap().unwrap(),job,
            "repeated startup must reuse the original queued job");
    }
}

#[test]
fn coach_completed_result_retries_storage_without_another_model_call() {
    let (_root,mut state) = local_fixture();
    let run = prepare_purpose(&mut state,"understand");
    finish(&mut state,&run.turn_ref,&outcome(None));
    let job = claim(&state.user).unwrap().unwrap();
    let adapter = RecordingAdapter::default();
    let cancel = CancellationToken::default();
    let result = execute(&adapter,&job,&state.workspace.book,&cancel);
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
    assert!(turn_context(&state.user,None,&run.turn_ref).unwrap().is_none());
    let next = AgentTurnRef { session_id:run.turn_ref.session_id.clone(),turn_id:"after-persisted-coach".into(),
        user_turn_ordinal:run.turn_ref.user_turn_ordinal+1 };
    ensure_turn(&state.user,None,&next,run.tutor.as_ref(),"later").unwrap();
    assert!(turn_context(&state.user,None,&next).unwrap().is_none(), "saved observations do not become next-turn plans");
}
