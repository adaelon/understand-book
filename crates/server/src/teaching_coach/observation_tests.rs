use super::*;
use crate::tests::tutor_loop_tests::{fixture_with_book, finish, outcome};

fn fixture(observing: bool) -> (tempfile::TempDir, AppState) {
    fixture_with_base(observing,base_schema::sample_base())
}
fn fixture_with_base(observing: bool,base:base_schema::ReadOnlyBase) -> (tempfile::TempDir, AppState) {
    let source="Average speed uses total elapsed time, including a stop. ".repeat(10);
    let (root,mut state)=fixture_with_book(true,Book::new(base,&source),&source);
    if observing {
        let mut store=state.user.learning_store().unwrap();
        let view=store.state().unwrap();
        store.mutate(&memory::learning::TutorMutation {operation_id:"disable-for-observing".into(),expected_revision:view.control.revision,
            action:memory::learning::TutorAction::SetEnabled {enabled:false}},"before").unwrap();
    }
    let reply=route(&mut state,Req {method:"POST",url:"/agent/new",body:"{}",now:"before"});
    assert_eq!(reply.status,200);
    (root,state)
}
fn prepare(state:&mut AppState,text:&str) -> agent_run::PreparedAgentChat {
    prepare_agent_chat(state,&json!({"message":text}).to_string(),"now").unwrap_or_else(|reply|panic!("{}",reply.body))
}
fn source(state:&AppState) -> SourceBinding {
    let range=EvidenceRange {start_lid:"1.1".into(),end_lid:"1.1".into(),ranges:vec![]};
    let resolved=state.workspace.book.resolve_source(&range,"zh",None).unwrap();
    SourceBinding {source_ref_id:"source-read".into(),book_id:state.workspace.book.base.book_id.clone(),publication_id:None,
        source_revision:Some(state.workspace.book.source_fingerprint().into()),evidence_range:Some(range),
        evidence_text_digest:resolved.evidence_text_digest,media:None,music:None,label_snapshot:resolved.label,preview_snapshot:resolved.preview}
}
fn complete(state:&mut AppState,run:&agent_run::PreparedAgentChat,incomplete:bool)->CoachJob {
    let mut response=outcome(None);response.incomplete=incomplete;response.source_bindings=vec![source(state)];
    finish(state,&run.turn_ref,&response);
    let binding:TeachingBinding=serde_json::from_value(run.tutor.as_ref().unwrap()["binding"].clone()).unwrap();
    state.user.learning_store().unwrap().coach_job_for_source(&binding.tutor_session_id,CoachJobOrigin::AfterTurn,&run.turn_ref.turn_id).unwrap().unwrap()
}
fn candidate(job:&CoachJob)->Value {
    json!({"next_step":{"focus":"总时间的组成","participation":{"choice":"conclude","reason":"当前问题已回答，等待真实后续表达"}},
        "reason":"整理原表达，不要求追加活动","alternatives":[],"needs_read":[],
        "observations":job.input.context["observation_facts"].as_array().unwrap().iter().map(|fact|json!({
            "fact_ref":fact["fact_ref"],"decision":"record","reason":"用户用自己的话确认总时间的范围",
            "hypothesis":{"target":{"learning_focus":"总时间包含停车时间","expected_performance":"说明总时间的组成","capability":"explanation",
                "source_bindings":[{"start_lid":"1.1","end_lid":"1.1"}],"object_refs":[]},
                "interpretation":"用户受助后表达总时间包含停车；尚未验证独立应用。","teaching_implication":"确认范围并保留受助条件。","block_refs":[]}})).collect::<Vec<_>>()})
}
fn claim_exact(state:&AppState,job:&CoachJob)->CoachJob {
    state.user.learning_store().unwrap().claim_coach_job(&job.input.job_ref,"work").unwrap().unwrap()
}
fn commit_candidate(state:&AppState,job:&CoachJob,value:Value) {
    commit_with_material(&state.user,job,Ok(value),Some((&state.workspace.book,&state.workspace.book_dir))).unwrap();
}

#[test]
fn background_observation_records_missing_understanding_in_observing_mode_once() {
    let (_root,mut state)=fixture(true);
    let run=prepare(&mut state,"也就是说停车也算总时间，对吗？");
    assert_eq!(run.tutor.as_ref().unwrap()["status"],"observing");
    let job=complete(&mut state,&run,false);
    let fact=job.input.context["observation_facts"][0]["fact_ref"].as_str().unwrap();
    assert!(!state.user.learning_store().unwrap().fact_has_interpretation(fact).unwrap());
    let claimed=claim_exact(&state,&job);let value=candidate(&job);
    commit_candidate(&state,&claimed,value.clone());commit_candidate(&state,&claimed,value);
    let store=state.user.learning_store().unwrap();
    let saved=store.coach_job(&job.input.job_ref).unwrap().unwrap();
    assert_eq!(saved.status,CoachJobStatus::Ready);
    assert_eq!(saved.guidance.as_ref().unwrap()["observation_receipts"][0]["status"],"recorded");
    assert_eq!(store.teaching_expression_interpretations(fact).unwrap().len(),1);
    assert!(!store.state().unwrap().control.enabled);
}

#[test]
fn background_observation_reuses_front_record_and_leaves_failed_observation_unfinished() {
    let (_root,mut state)=fixture(true);let run=prepare(&mut state,"停车也算总时间吗？");let job=complete(&mut state,&run,false);
    let claimed=claim_exact(&state,&job);let value=candidate(&job);
    let mut other=claimed.clone();other.input.job_ref="earlier-front-observation".into();
    // The shared acceptance path can have completed after the task input froze.
    let mut accepted=value.clone();observations::commit(&state.user,&other,&mut accepted,Some((&state.workspace.book,&state.workspace.book_dir))).unwrap();
    commit_candidate(&state,&claimed,value);
    assert_eq!(state.user.learning_store().unwrap().coach_job(&job.input.job_ref).unwrap().unwrap().guidance.unwrap()["observation_receipts"][0]["status"],"existing");
    let next=prepare(&mut state,"为什么总时间不能只算运动？");let next_job=complete(&mut state,&next,false);let next_claim=claim_exact(&state,&next_job);
    let mut missing=candidate(&next_job);missing["observations"]=json!([]);commit_candidate(&state,&next_claim,missing);
    let failed=state.user.learning_store().unwrap().coach_job(&next_job.input.job_ref).unwrap().unwrap();
    assert_eq!(failed.status,CoachJobStatus::Failed);
    assert!(!state.user.learning_store().unwrap().fact_has_interpretation(next_job.input.context["observation_facts"][0]["fact_ref"].as_str().unwrap()).unwrap());
}

#[test]
fn background_observation_preserves_fallback_help_before_input_not_current_answer() {
    let (_root,mut state)=fixture(true);
    let previous=prepare(&mut state,"先讲讲总时间");let previous_job=complete(&mut state,&previous,true);
    let followup=prepare(&mut state,"也就是说停车也算总时间，对吗？");let job=complete(&mut state,&followup,true);
    let frozen=&job.input.context["observation_facts"][0];
    let prior=frozen["prior_help"].as_array().unwrap();assert_eq!(prior.len(),1);
    assert_eq!(prior[0]["kind"],"usage_observed");assert_eq!(prior[0]["usage"]["kind"],"assistant_delivery");
    assert_eq!(prior[0]["binding"]["turn_id"],previous.turn_ref.turn_id);
    assert_ne!(prior[0]["binding"]["turn_id"],followup.turn_ref.turn_id);
    let claimed=claim_exact(&state,&job);commit_candidate(&state,&claimed,candidate(&job));
    let store=state.user.learning_store().unwrap();let evidence=store.teaching_expression_interpretations(frozen["fact_ref"].as_str().unwrap()).unwrap().remove(0);
    assert_eq!(evidence.support_delivery_refs,vec![prior[0]["event_id"].as_str().unwrap()]);assert!(evidence.assistance_refs.is_empty());
    assert!(evidence.supersedes.is_none());assert!(store.coach_job(&previous_job.input.job_ref).unwrap().is_some());
}

#[test]
fn background_observation_survives_later_pause_and_backfill_has_separate_identity() {
    let (_root,mut state)=fixture(false);let run=prepare(&mut state,"停车也算总时间吗？");let job=complete(&mut state,&run,true);let claimed=claim_exact(&state,&job);
    let mut store=state.user.learning_store().unwrap();let view=store.state().unwrap();
    store.mutate(&memory::learning::TutorMutation {operation_id:"pause-after-input".into(),expected_revision:view.control.revision,
        action:memory::learning::TutorAction::Pause {session_id:job.input.binding.tutor_session_id.clone()}},"later").unwrap();
    assert!(applicable(&state.user,&claimed).unwrap());assert!(!guidance_applicable(&state.user,&claimed).unwrap());
    commit_candidate(&state,&claimed,candidate(&job));assert_eq!(store.coach_job(&job.input.job_ref).unwrap().unwrap().status,CoachJobStatus::Ready);
    enqueue_observation_backfill(&state.user,&run.turn_ref,"repair").unwrap();enqueue_observation_backfill(&state.user,&run.turn_ref,"again").unwrap();
    let backfill=store.coach_job_for_source(&job.input.binding.tutor_session_id,CoachJobOrigin::ObservationBackfill,&run.turn_ref.turn_id).unwrap().unwrap();
    assert_ne!(backfill.input.job_ref,job.input.job_ref);assert_eq!(backfill.attempt,0);
    assert_eq!(store.list_coach_jobs(None,20).unwrap().iter().filter(|job|job.input.origin==CoachJobOrigin::ObservationBackfill).count(),1);
}

#[test]
fn background_observation_explicit_retry_preserves_attempts_and_ready_idempotence() {
    for interrupted in [false,true] {
        let (_root,mut state)=fixture(true);let run=prepare(&mut state,"停车也算总时间吗？");let original=complete(&mut state,&run,true);
        enqueue_observation_backfill(&state.user,&run.turn_ref,"backfill").unwrap();
        let mut store=state.user.learning_store().unwrap();
        let job=store.coach_job_for_source(&original.input.binding.tutor_session_id,CoachJobOrigin::ObservationBackfill,&run.turn_ref.turn_id).unwrap().unwrap();
        let claimed=claim_exact(&state,&job);
        if interrupted {store.recover_interrupted_coach_jobs("restart").unwrap();}
        else {commit_with_material(&state.user,&claimed,Err(error("TUTOR_COACH_FAILED","provider failure")),None).unwrap();}
        let failed=store.coach_job(&job.input.job_ref).unwrap().unwrap();
        assert!(matches!(failed.status,CoachJobStatus::Failed|CoachJobStatus::Interrupted));
        enqueue_observation_backfill(&state.user,&run.turn_ref,"ordinary-repeat").unwrap();
        assert_eq!(store.coach_job(&job.input.job_ref).unwrap().unwrap(),failed,"ordinary repeated enqueue never replays unknown paid work");
        retry_observation_backfill(&state.user,&run.turn_ref,"explicit-retry").unwrap();
        assert_eq!(store.coach_job(&job.input.job_ref).unwrap().unwrap().status,CoachJobStatus::Pending);
        let second=claim_exact(&state,&job);assert_eq!(second.attempt,2);assert_eq!(second.attempts[0],failed.attempts[0]);
        commit_candidate(&state,&second,candidate(&job));
        let ready=store.coach_job(&job.input.job_ref).unwrap().unwrap();assert_eq!(ready.status,CoachJobStatus::Ready);
        retry_observation_backfill(&state.user,&run.turn_ref,"repeat-ready").unwrap();
        assert_eq!(store.coach_job(&job.input.job_ref).unwrap().unwrap(),ready);
    }
}

#[test]
fn background_observation_keeps_explicit_cross_chat_help_but_rejects_new_cross_chat_refs() {
    let (_root,mut state)=fixture(false);let first=prepare(&mut state,"先解释总时间");
    let binding:TeachingBinding=serde_json::from_value(first.tutor.as_ref().unwrap()["binding"].clone()).unwrap();
    let mut store=state.user.learning_store().unwrap();
    for id in ["referenced-prior-help","unreferenced-prior-help"] {
        store.append_teaching(&memory::teaching::TeachingEvent {event_id:id.into(),binding:binding.clone(),
            kind:TeachingFact::MessageDelivered,causal_refs:vec![],payload:json!({"answer":"总时间包括停车时间。"}),occurred_at:"before".into()}).unwrap();
    }
    complete(&mut state,&first,false);
    assert_eq!(route(&mut state,Req {method:"POST",url:"/agent/new",body:"{}",now:"next"}).status,200);
    let second=prepare_agent_chat(&mut state,&json!({"message":"所以停车也算，对吗？","teaching_ref":"referenced-prior-help"}).to_string(),"now")
        .unwrap_or_else(|reply|panic!("{}",reply.body));
    let job=complete(&mut state,&second,false);assert_ne!(job.input.binding.chat_session_id,binding.chat_session_id);
    let value=candidate(&job);let candidate=&value["observations"][0]["hypothesis"];
    let request=json!({"operation_id":"reject-unfrozen-cross-chat","target":candidate["target"],
        "fact_refs":job.input.context["observation_facts"][0]["fact_refs"],"interpretation":candidate["interpretation"],
        "teaching_implication":candidate["teaching_implication"]});
    let bound=source(&state);let range=bound.evidence_range.clone().unwrap();
    assert!(crate::teaching::accept_hypothesis_with_assistance(&state.private_context(),&job.input.binding,&request,
        &[bound],&[range],&["unreferenced-prior-help".into()]).is_err());
    let claimed=claim_exact(&state,&job);commit_candidate(&state,&claimed,value);
    let fact=job.input.context["observation_facts"][0]["fact_ref"].as_str().unwrap();
    let evidence=store.teaching_expression_interpretations(fact).unwrap().remove(0);
    assert!(evidence.support_delivery_refs.contains(&"referenced-prior-help".into()));
}

#[test]
fn background_observation_uses_persisted_review_reads_without_display_references() {
    let (_root,mut state)=fixture(false);let run=prepare(&mut state,"停车也算总时间吗？");
    let binding:TeachingBinding=serde_json::from_value(run.tutor.as_ref().unwrap()["binding"].clone()).unwrap();
    let range=source(&state).evidence_range.unwrap();
    state.user.learning_store().unwrap().append_teaching(&memory::teaching::TeachingEvent {
        event_id:"reviewed-full-read".into(),binding:binding.clone(),kind:TeachingFact::AgentWorkReviewed,
        causal_refs:vec![],payload:json!({"source_evidence":[{"source_id":binding.source_id,
            "source_revision":binding.source_revision,"read_ranges":[range]}]}),occurred_at:"review".into()}).unwrap();
    let mut response=outcome(None);response.incomplete=true;response.source_bindings.clear();finish(&mut state,&run.turn_ref,&response);
    let job=state.user.learning_store().unwrap().coach_job_for_source(&binding.tutor_session_id,CoachJobOrigin::AfterTurn,&run.turn_ref.turn_id).unwrap().unwrap();
    assert_eq!(job.input.context["source_bindings"],json!([]));
    let hydrated=observations::with_material(&job,&state.workspace.book).unwrap();
    assert!(!hydrated.input.context["read_sources"][0]["text"].as_str().unwrap().is_empty());
    let mut wrong=job.clone();wrong.input.context["turn_facts"].as_array_mut().unwrap().iter_mut()
        .filter(|fact|fact["kind"]=="agent_work_reviewed")
        .for_each(|fact|fact["payload"]["source_evidence"][0]["source_revision"]=json!("other-version"));
    assert!(observations::commit(&state.user,&wrong,&mut candidate(&job),Some((&state.workspace.book,&state.workspace.book_dir))).is_err());
    let claimed=claim_exact(&state,&job);commit_candidate(&state,&claimed,candidate(&job));
    assert_eq!(state.user.learning_store().unwrap().coach_job(&job.input.job_ref).unwrap().unwrap().status,CoachJobStatus::Ready);
}

fn append_read_receipts(state:&mut AppState,turn:&AgentTurnRef,lid:&str) {
    let messages:Vec<Message>=serde_json::from_value(json!([
        {"role":"Assistant","content":null,"tool_calls":[{"id":"saved-read","name":"book.text","arguments":json!({"lid":lid}).to_string()}]},
        {"role":"Tool","content":json!({"lid":lid,"text":state.workspace.book.text(lid,Some(lid)).unwrap()}).to_string(),"tool_calls":[],"tool_call_id":"saved-read"}
    ])).unwrap();
    let mut store=state.user.session_store.take().unwrap();
    store.append(&mut state.user.agent_history,&turn.session_id,"read",Some(&turn.turn_id),
        crate::session_event::EventBody::MessageAppended {messages}).unwrap();
    state.user.session_store=Some(store);
}

#[test]
fn background_observation_failed_backfill_recovers_only_original_turn_reads_before_explicit_retry() {
    let mut base=base_schema::sample_base();
    base.lid_nodes[0].span.end=200;base.lid_nodes[0].children.push("1.2".into());
    let mut second=base.lid_nodes[1].clone();second.lid="1.2".into();second.path=vec![1,2];
    second.span=base_schema::Span {start:100,end:200};base.lid_nodes.push(second);
    let (_root,mut state)=fixture_with_base(true,base);
    let paths=crate::session_store::SessionPaths::from_history(state.user.history_path.as_ref().unwrap());
    let (mut sessions,mut history)=crate::session_store::SessionStore::open(paths).unwrap();
    for session in state.user.agent_history.sessions.clone() {sessions.create(&mut history,session).unwrap();}
    for (book,chat) in state.user.agent_history.active_by_book.clone() {sessions.select(&mut history,&book,&chat).unwrap();}
    state.user.agent_history=history;state.user.session_store=Some(sessions);
    let run=prepare(&mut state,"停车也算总时间吗？");
    append_read_receipts(&mut state,&run.turn_ref,"1.1");
    let mut response=outcome(None);response.incomplete=true;response.source_bindings.clear();finish(&mut state,&run.turn_ref,&response);
    enqueue_observation_backfill(&state.user,&run.turn_ref,"repair").unwrap();
    let binding:TeachingBinding=serde_json::from_value(run.tutor.as_ref().unwrap()["binding"].clone()).unwrap();
    let store=state.user.learning_store().unwrap();
    let job=store.coach_job_for_source(&binding.tutor_session_id,CoachJobOrigin::ObservationBackfill,&run.turn_ref.turn_id).unwrap().unwrap();
    let claimed=claim_exact(&state,&job);commit_candidate(&state,&claimed,candidate(&job));
    let failed=store.coach_job(&job.input.job_ref).unwrap().unwrap();assert_eq!(failed.status,CoachJobStatus::Failed);
    assert!(failed.error.as_ref().unwrap()["message"].as_str().unwrap().contains("尚未读取"));
    // The same call ID in a later turn cannot contribute that turn's source.
    let later=prepare(&mut state,"再解释另一段");append_read_receipts(&mut state,&later.turn_ref,"1.2");
    let recovered=freeze_observation_reads(&state.user,&failed,&state.workspace.book,true).unwrap();
    let ranges:Vec<EvidenceRange>=serde_json::from_value(recovered.input.context["verified_read_ranges"].clone()).unwrap();
    assert_eq!(ranges,vec![EvidenceRange {start_lid:"1.1".into(),end_lid:"1.1".into(),ranges:vec![]}]);
    let mut original_context=failed.input.context.clone();original_context["verified_read_ranges"]=json!(ranges);
    assert_eq!(recovered.input.context,original_context);assert_eq!(recovered.attempts,failed.attempts);
    assert_eq!(recovered.status,CoachJobStatus::Failed,"verifying source reads alone never spends or retries");
    let hydrated=observations::with_material(&recovered,&state.workspace.book).unwrap();
    assert_eq!(hydrated.input.context["read_sources"][0]["range"]["start_lid"],"1.1");
    retry_observation_backfill(&state.user,&run.turn_ref,"explicit").unwrap();
    let second=claim_exact(&state,&recovered);assert_eq!(second.attempt,2);
    commit_candidate(&state,&second,candidate(&second));
    let ready=store.coach_job(&job.input.job_ref).unwrap().unwrap();assert_eq!(ready.status,CoachJobStatus::Ready);
    assert_eq!(freeze_observation_reads(&state.user,&ready,&state.workspace.book,true).unwrap(),ready);
}
