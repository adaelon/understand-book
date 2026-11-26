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
    json!({"reason":"整理原表达，不要求追加活动",
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

struct MaterialRepairAdapter {
    outputs: std::cell::RefCell<std::collections::VecDeque<Value>>,
    calls: std::cell::RefCell<Vec<runtime::CompletionRequest>>,
}
impl ModelAdapter for MaterialRepairAdapter {
    fn complete(&self, _:runtime::CompletionRequest)->Result<runtime::ParsedResponse,runtime::AdapterError> { unreachable!() }
    fn chat(&self, _: &runtime::AgentRequestPlan)->Result<runtime::AssistantTurn,runtime::AdapterError> { unreachable!() }
    fn complete_structured(&self, request:runtime::CompletionRequest)->Result<Value,runtime::AdapterError> {
        self.calls.borrow_mut().push(request);
        Ok(self.outputs.borrow_mut().pop_front().expect("one correction only"))
    }
}

#[test]
fn coach_material_correction_rejects_unread_parent_before_persistence() {
    let mut base=base_schema::sample_base();
    base.lid_nodes[0].span.end=200;
    let (_root,mut state)=fixture_with_base(true,base);
    let run=prepare(&mut state,"停车也算总时间");
    let job=complete(&mut state,&run,false);
    let claimed=claim_exact(&state,&job);
    let frozen=observations::freeze_original_reads(&state.user,&claimed,&state.workspace.book,false).unwrap();
    let hydrated=observations::with_material(&frozen,&state.workspace.book,&state.workspace.book_dir).unwrap();
    let valid=candidate(&job);let mut invalid=valid.clone();
    invalid["observations"][0]["hypothesis"]["target"]["source_bindings"]=json!([{"start_lid":"1","end_lid":"1"}]);
    let adapter=MaterialRepairAdapter {outputs:std::cell::RefCell::new([invalid,valid].into()),
        calls:Default::default()};
    let result=execute(&adapter,&hydrated,&state.workspace.book,&CancellationToken::default()).unwrap();
    let calls=adapter.calls.borrow();
    assert_eq!(calls.len(),2);
    assert_eq!(calls[0].user,calls[1].user,"correction must keep the same source and learner evidence");
    assert!(calls[1].followup_user.as_ref().unwrap().contains("超出 read_sources"));
    commit_candidate(&state,&frozen,result);
    assert_eq!(state.user.learning_store().unwrap().coach_job(&job.input.job_ref).unwrap().unwrap().status,CoachJobStatus::Ready);
}

#[test]
fn coach_material_correction_checks_exact_knowledge_candidate_and_learner_quote() {
    for invalid_quote in [false,true] {
        let (_root,mut state)=fixture(true);
        let words="停车也算总时间";
        let run=prepare(&mut state,words);let job=complete(&mut state,&run,false);
        let claimed=claim_exact(&state,&job);
        let frozen=observations::freeze_original_reads(&state.user,&claimed,&state.workspace.book,false).unwrap();
        let hydrated=observations::with_material(&frozen,&state.workspace.book,&state.workspace.book_dir).unwrap();
        let reference=hydrated.input.context["knowledge_candidates"][0]["reference"].clone();
        assert!(reference.is_object());
        let mut valid=candidate(&job);
        valid["observations"][0]["hypothesis"]["understanding"]=json!({"knowledge_links":[{"reference":reference,"quote":words}]});
        let mut invalid=valid.clone();
        let link=&mut invalid["observations"][0]["hypothesis"]["understanding"]["knowledge_links"][0];
        if invalid_quote {link["quote"]=json!("老师的结论");}
        else {link["reference"]["entry_id"]=json!("not-a-supplied-candidate");}
        let adapter=MaterialRepairAdapter {outputs:std::cell::RefCell::new([invalid,valid].into()),
            calls:Default::default()};
        let result=execute(&adapter,&hydrated,&state.workspace.book,&CancellationToken::default()).unwrap();
        let calls=adapter.calls.borrow();
        assert_eq!(calls.len(),2);
        assert!(calls[1].followup_user.as_ref().unwrap().contains(if invalid_quote {"这次用户表达中的原话"} else {"本轮已提供的来源条目"}));
        commit_candidate(&state,&frozen,result);
        assert_eq!(state.user.learning_store().unwrap().coach_job(&job.input.job_ref).unwrap().unwrap().status,CoachJobStatus::Ready);
    }
}

#[test]
fn knowledge_selected_focus_survives_the_bounded_candidate_page() {
    let mut base=base_schema::sample_base();
    base.graph_nodes=serde_json::from_value(json!((0..40).map(|i|json!({"id":format!("claim:{i:02}"),"name":format!("同段论断 {i}"),"type":"claim","source_lid":"1.1","occurrences":[]})).collect::<Vec<_>>())).unwrap();base.graph_edges.clear();
    let (_root,mut state)=fixture_with_base(true,base);
    let reference=json!({"source_id":state.workspace.book.base.book_id,"source_revision":state.workspace.book.source_fingerprint(),"publication_id":null,"artifact":"pass1","entry_id":"claim:39"});
    let index=state.workspace.book.knowledge_index(&state.workspace.book_dir,None).unwrap();
    let node_id=index.nodes.values().find(|n|n.reference.entry_id=="claim:39").unwrap().id.clone();
    let view=crate::knowledge_tree::view(&state.user,&state.workspace,&json!({"current_lid":"1.1"})).unwrap();
    assert_eq!(view["node"]["id"], index.scope_for_lid("1.1").unwrap());
    assert_eq!(view["current_scope"], view["node"]["id"]);
    assert_eq!(view["children"].as_array().unwrap().len(),1);
    assert_eq!(view["children"][0]["child_count"],40);
    assert_eq!(view["entry_point"]["reference"]["entry_id"],"claim:00");
    let claims=crate::knowledge_tree::view(&state.user,&state.workspace,&json!({"node_id":view["children"][0]["id"],"current_lid":"1.1"})).unwrap();
    assert_eq!(claims["total"],40);
    assert_eq!(claims["children"].as_array().unwrap().len(),12);
    assert_eq!(claims["node"]["id"],view["children"][0]["id"]);
    let run=prepare_agent_chat(&mut state,&json!({"message":"停车也算总时间","knowledge_focus":{"reference":reference,"node_id":node_id}}).to_string(),"now").unwrap_or_else(|reply|panic!("{}",reply.body));
    let job=complete(&mut state,&run,false);
    let hydrated=observations::with_material(&job,&state.workspace.book,&state.workspace.book_dir).unwrap();
    assert_eq!(hydrated.input.context["knowledge_candidates"].as_array().unwrap().len(),32);
    assert_eq!(hydrated.input.context["knowledge_candidates"][0]["reference"],reference);
    let mut value=candidate(&job);value["observations"][0]["hypothesis"]["understanding"]=json!({"knowledge_links":[{"reference":reference,"quote":"停车也算总时间"}]});
    let claimed=claim_exact(&state,&job);commit_candidate(&state,&claimed,value);
    let saved=state.user.learning_store().unwrap().coach_job(&job.input.job_ref).unwrap().unwrap();
    assert_eq!(saved.status,CoachJobStatus::Ready,"{:?}",saved.error);
}

#[test]
fn knowledge_observation_attaches_only_the_expressed_claim_and_resumes_without_a_note() {
    let mut base=base_schema::sample_base();
    base.graph_nodes=serde_json::from_value(json!([
        {"id":"claim:time","name":"总时间包含停车","type":"claim","source_lid":"1.1","occurrences":[]},
        {"id":"claim:distance","name":"总路程包含往返","type":"claim","source_lid":"1.1","occurrences":[]}
    ])).unwrap();base.graph_edges=serde_json::from_value(json!([{"source":"claim:time","target":"claim:distance","type":"contrasts_with","direction":"directed","scope":"local","weight":0.8}])).unwrap();
    let (_root,mut state)=fixture_with_base(true,base);
    let words="停车也算在总时间里";
    let run=prepare(&mut state,words);let job=complete(&mut state,&run,false);
    let hydrated=observations::with_material(&job,&state.workspace.book,&state.workspace.book_dir).unwrap();
    let candidates=hydrated.input.context["knowledge_candidates"].as_array().unwrap();
    assert_eq!(candidates.len(),2);
    let selected=candidates.iter().find(|c|c["reference"]["entry_id"]=="claim:time").unwrap();
    assert!(selected["relations"].as_array().unwrap().iter().any(|r|r["reference"]["entry_id"]=="claim:distance" && r["kind"]=="contrasts_with"));
    let mut course_job=hydrated.clone();
    course_job.input.context["course_response"]=json!({"activity":"reflection"});
    course_job.input.context["teaching_context"]["learner_context"]=json!({"current_understanding":[
        {"evidence_ref":"related","target":{"focus_ref":"old-focus"},"knowledge_links":[{"reference":selected["relations"][0]["reference"]}],"demonstrated":["已表达的关系"]},
        {"evidence_ref":"source-overlap","target":{"source_bindings":[{"start_lid":"1.1","end_lid":"1.1"}]}},
        {"evidence_ref":"unrelated","target":{"source_bindings":[{"start_lid":"missing","end_lid":"missing"}]}}
    ]});
    let scoped=observations::with_material(&course_job,&state.workspace.book,&state.workspace.book_dir).unwrap();
    let prior=scoped.input.context["teaching_context"]["learner_context"]["current_understanding"].as_array().unwrap();
    assert_eq!(prior.len(),2);assert_eq!(prior[0]["evidence_ref"],"related");assert_eq!(prior[1]["evidence_ref"],"source-overlap");
    assert!(scoped.input.context["known_focuses"].as_array().unwrap().iter().any(|t|t["focus_ref"]=="old-focus"));
    let mut value=candidate(&job);
    value["observations"][0]["hypothesis"]["understanding"]=json!({"demonstrated":["解释停车计入总时间"],
        "knowledge_links":[{"reference":selected["reference"],"quote":words}]});
    let claimed=claim_exact(&state,&job);commit_candidate(&state,&claimed,value);
    let saved=state.user.learning_store().unwrap().coach_job(&job.input.job_ref).unwrap().unwrap();
    assert_eq!(saved.status,CoachJobStatus::Ready,"{:?}",saved.error);
    let id=saved.guidance.as_ref().unwrap()["observation_receipts"][0]["evidence_ref"].as_str().unwrap();
    let evidence=state.user.learning_store().unwrap().evidence(id).unwrap();
    assert!(evidence.note_refs.is_empty());
    let index=state.workspace.book.knowledge_index(&state.workspace.book_dir,None).unwrap();
    for node in index.nodes.values().filter(|n|n.reference.artifact=="pass1") {
        let view=crate::knowledge_tree::view(&state.user,&state.workspace,&json!({"node_id":node.id})).unwrap();
        assert_eq!(view["fruits"].as_array().unwrap().len(),if node.reference.entry_id=="claim:time"{1}else{0});
    }
    let overview=crate::knowledge_tree::view(&state.user,&state.workspace,&json!({"node_id":"root","overview":true})).unwrap();
    let nodes:Vec<_>=overview["map_groups"].as_array().unwrap().iter().flat_map(|g|g["nodes"].as_array().unwrap()).collect();
    let time=nodes.iter().find(|n|n["reference"]["entry_id"]=="claim:time").unwrap();
    let distance=nodes.iter().find(|n|n["reference"]["entry_id"]=="claim:distance").unwrap();
    assert_eq!(time["understanding"][0]["evidence_ref"],id);assert!(distance["understanding"].as_array().unwrap().is_empty());
    let resumed=crate::knowledge_tree::resume(&state.user,&json!({"workspace_id":null,"evidence_ref":id})).unwrap();
    assert_eq!(resumed["understanding_ref"]["evidence_ref"],id);
    assert_eq!(resumed["context"]["turn_id"],run.turn_ref.turn_id);
    let frozen=crate::personal_note_context::freeze(&state.user,&json!({"understanding_ref":resumed["understanding_ref"]}),Some(&run.turn_ref.session_id)).unwrap().unwrap();
    assert_eq!(frozen["understanding"]["learner_quote"],words);
    for (purpose, label, message) in [
        (Some("practice"), "换个例子练习", ""),
        (None, "接着这一点", "停车很久时，应该怎样算总时间？"),
    ] {
        let request = json!({"entry_action":"continue_answer","learning_purpose":purpose,
            "message":message,"source_id":state.workspace.book.base.book_id,"source_revision":resumed["source_revision"],
            "session_id":run.turn_ref.session_id,"client_request_id":label,"expected_chat_head":run.turn_ref.turn_id,
            "answer_ref":{"turn_id":run.turn_ref.turn_id},"understanding_ref":resumed["understanding_ref"]});
        let parsed = crate::interaction_input::InteractionInput::parse(&request).unwrap();
        let Some(runtime::tutor_continuation::InteractionResolution::RunInput {interaction}) =
            parsed.resolve(&state.user,&state.workspace,&request,None).unwrap() else { panic!("expected an understanding continuation") };
        assert_eq!(interaction.accepted_request,label);
        assert_eq!(interaction.user_supplement,message);
        assert_eq!(serde_json::to_value(interaction.learning_purpose).unwrap(),json!(purpose));
        assert_eq!(serde_json::to_value(interaction.target).unwrap(),resumed["target"]);
    }
    let note = state.user.store.create_personal_note(
        serde_json::from_value(json!({"content":words,"authorship":"user"})).unwrap(),"time-note","1").unwrap();
    let note_ref = json!({"note_id":note.note_id,"revision":1});
    let request = json!({"entry_action":"revisit_note","message":"","note_ref":note_ref,"note_refs":[note_ref],
        "source_id":state.workspace.book.base.book_id,"source_revision":resumed["source_revision"],
        "session_id":run.turn_ref.session_id,"client_request_id":"understanding-note","expected_chat_head":run.turn_ref.turn_id,
        "understanding_ref":resumed["understanding_ref"]});
    let parsed = crate::interaction_input::InteractionInput::parse(&request).unwrap();
    let Some(runtime::tutor_continuation::InteractionResolution::RunInput {interaction}) =
        parsed.resolve(&state.user,&state.workspace,&request,None).unwrap() else {panic!("expected note practice")};
    assert_eq!(interaction.accepted_request,"换个例子练习");
    assert_eq!(serde_json::to_value(interaction.note_ref).unwrap(),note_ref);
    assert_eq!(serde_json::to_value(interaction.target).unwrap(),resumed["target"]);
    let reference:read_tools::knowledge::KnowledgeReference=serde_json::from_value(selected["reference"].clone()).unwrap();
    let fabricated=read_tools::knowledge::KnowledgeLink{reference:reference.clone(),quote:"这句话用户没有说过".into()};
    assert!(crate::knowledge_tree::validate_links(&[fabricated],candidates,words).is_err());
    let mut stale=reference;stale.source_revision="other".into();
    assert!(crate::knowledge_tree::validate_links(&[read_tools::knowledge::KnowledgeLink{reference:stale,quote:words.into()}],candidates,words).is_err());
    let corrected=state.user.learning_store().unwrap().correct_evidence(id,"tree-correction","刚才是参考讲解后才说出来的").unwrap();
    assert_eq!(corrected.understanding.knowledge_links,evidence.understanding.knowledge_links);
    let node=index.nodes.values().find(|n|n.reference.entry_id=="claim:time").unwrap();
    let view=crate::knowledge_tree::view(&state.user,&state.workspace,&json!({"node_id":node.id})).unwrap();
    assert_eq!(view["fruits"][0]["evidence_ref"],corrected.evidence_id);
    assert!(view["fruits"][0]["demonstrated"].as_array().unwrap().is_empty());
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
        action:memory::learning::TutorAction::Pause {session_id:view.control.current_tutor_session_id.unwrap()}},"later").unwrap();
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
    let hydrated=observations::with_material(&job,&state.workspace.book,&state.workspace.book_dir).unwrap();
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
fn background_observation_recovers_current_resident_citation_without_another_read() {
    let (_root,mut state)=fixture(true);
    let run=prepare(&mut state,"停车也算总时间");
    let mut response=outcome(None);
    let mut citation=source(&state);citation.source_revision=None;
    response.source_bindings=vec![citation];finish(&mut state,&run.turn_ref,&response);
    let stage=run.tutor.as_ref().unwrap()["binding"]["tutor_session_id"].as_str().unwrap();
    let job=state.user.learning_store().unwrap().coach_job_for_source(stage,CoachJobOrigin::AfterTurn,&run.turn_ref.turn_id).unwrap().unwrap();
    let frozen=observations::freeze_original_reads(&state.user,&claim_exact(&state,&job),&state.workspace.book,false).unwrap();
    assert_eq!(frozen.input.context["source_bindings"][0]["source_revision"],state.workspace.book.source_fingerprint());
    let hydrated=observations::with_material(&frozen,&state.workspace.book,&state.workspace.book_dir).unwrap();
    assert_eq!(hydrated.input.context["read_sources"].as_array().unwrap().len(),1,"citation and receipt must not duplicate source text");
    commit_candidate(&state,&frozen,candidate(&hydrated));
    assert_eq!(state.user.learning_store().unwrap().coach_job(&job.input.job_ref).unwrap().unwrap().status,CoachJobStatus::Ready);
}

#[test]
fn knowledge_dialogue_observation_reuses_prior_help_without_a_new_read() {
    let mut base=base_schema::sample_base();
    base.graph_nodes=serde_json::from_value(json!([
        {"id":"claim:time","name":"总时间包含停车","type":"claim","source_lid":"1.1","occurrences":[]}
    ])).unwrap();base.graph_edges.clear();
    let (_root,mut state)=fixture_with_base(true,base);
    let prior=prepare(&mut state,"请解释总时间，问我一个问题");
    let mut delivery=outcome(None);let mut citation=source(&state);
    // Local-reader citations have no publication revision; the original
    // teaching binding and exact source excerpt still identify the material.
    citation.source_revision=None;delivery.source_bindings=vec![citation];
    finish(&mut state,&prior.turn_ref,&delivery);
    let run=prepare(&mut state,"停车也算总时间");
    let mut response=outcome(None);response.source_bindings.clear();finish(&mut state,&run.turn_ref,&response);
    let binding:TeachingBinding=serde_json::from_value(run.tutor.as_ref().unwrap()["binding"].clone()).unwrap();
    let job=state.user.learning_store().unwrap().coach_job_for_source(&binding.tutor_session_id,CoachJobOrigin::AfterTurn,&run.turn_ref.turn_id).unwrap().unwrap();
    assert_eq!(job.input.context["source_bindings"],json!([]));
    assert_eq!(job.input.context["observation_facts"][0]["prior_help"][0]["binding"]["turn_id"],prior.turn_ref.turn_id);
    let claimed=claim_exact(&state,&job);
    let frozen=observations::freeze_original_reads(&state.user,&claimed,&state.workspace.book,false).unwrap();
    let hydrated=observations::with_material(&frozen,&state.workspace.book,&state.workspace.book_dir).unwrap();
    assert!(!hydrated.input.context["read_sources"].as_array().unwrap().is_empty());
    assert_eq!(hydrated.input.context["knowledge_candidates"][0]["reference"]["entry_id"],"claim:time");
    let mut value=candidate(&hydrated);value["observations"][0]["hypothesis"]["understanding"]=json!({"knowledge_links":[{"reference":hydrated.input.context["knowledge_candidates"][0]["reference"],"quote":"停车也算总时间"}]});
    commit_candidate(&state,&frozen,value);
    let saved=state.user.learning_store().unwrap().coach_job(&job.input.job_ref).unwrap().unwrap();
    assert_eq!(saved.status,CoachJobStatus::Ready,"{:?}",saved.error);
    let evidence=state.user.learning_store().unwrap().evidence(saved.guidance.as_ref().unwrap()["observation_receipts"][0]["evidence_ref"].as_str().unwrap()).unwrap();
    assert_eq!(evidence.support_delivery_refs,vec![job.input.context["observation_facts"][0]["prior_help"][0]["event_id"].as_str().unwrap()]);
    assert_eq!(evidence.understanding.knowledge_links[0].quote,"停车也算总时间");
    let again=prepare(&mut state,"因为整趟行程还没有结束");
    let mut reply=outcome(None);reply.source_bindings.clear();finish(&mut state,&again.turn_ref,&reply);
    let again_job=state.user.learning_store().unwrap().coach_job_for_source(&binding.tutor_session_id,CoachJobOrigin::AfterTurn,&again.turn_ref.turn_id).unwrap().unwrap();
    let again_frozen=observations::freeze_original_reads(&state.user,&claim_exact(&state,&again_job),&state.workspace.book,false).unwrap();
    assert_eq!(again_frozen.input.context["verified_read_ranges"],frozen.input.context["verified_read_ranges"]);
}

#[test]
fn foreground_understanding_keeps_help_delivered_before_the_expression() {
    let (_root,mut state)=fixture(true);
    let prior=prepare(&mut state,"请解释总时间");
    complete(&mut state,&prior,false);
    let run=prepare(&mut state,"请记录：停车也算总时间");
    let job=complete(&mut state,&run,false);
    let fact=&job.input.context["observation_facts"][0];
    let hypothesis=candidate(&job)["observations"][0]["hypothesis"].clone();
    let request=json!({"operation_id":"explicit-understanding","target":hypothesis["target"],
        "fact_refs":fact["fact_refs"],"interpretation":hypothesis["interpretation"],
        "teaching_implication":hypothesis["teaching_implication"]});
    let context=PrivateBookContext {user:&state.user,learning_workspace_id:None,
        book:&state.workspace.book,book_dir:&state.workspace.book_dir,messages:&[],
        selected_chat:Some(&run.turn_ref.session_id)};
    let accepted=crate::teaching::accept_hypothesis(&context,&job.input.binding,&request,
        &[source(&state)],&[EvidenceRange{start_lid:"1.1".into(),end_lid:"1.1".into(),ranges:vec![]}]).unwrap();
    let evidence=state.user.learning_store().unwrap().evidence(accepted["evidence_ref"].as_str().unwrap()).unwrap();
    assert_eq!(evidence.support_delivery_refs,vec![fact["prior_help"][0]["event_id"].as_str().unwrap()]);
    assert_eq!(evidence.learner_quote,"请记录：停车也算总时间");
}

#[test]
fn ma2_unreviewed_delivery_restores_full_original_reads_and_input_time_help() {
    let (_root,mut state)=fixture(false);
    let paths=crate::session_store::SessionPaths::from_history(state.user.history_path.as_ref().unwrap());
    let (mut sessions,mut history)=crate::session_store::SessionStore::open(paths).unwrap();
    for session in state.user.agent_history.sessions.clone() { sessions.create(&mut history,session).unwrap(); }
    for (book,chat) in state.user.agent_history.active_by_book.clone() { sessions.select(&mut history,&book,&chat).unwrap(); }
    state.user.agent_history=history;state.user.session_store=Some(sessions);
    let prior=prepare(&mut state,"请先解释总时间");
    complete(&mut state,&prior,false);
    let run=prepare(&mut state,"也就是说停车也算总时间，对吗？");
    append_read_receipts(&mut state,&run.turn_ref,"1.1");
    let mut response=outcome(None);response.source_bindings.clear();
    finish(&mut state,&run.turn_ref,&response);
    let binding:TeachingBinding=serde_json::from_value(run.tutor.as_ref().unwrap()["binding"].clone()).unwrap();
    let store=state.user.learning_store().unwrap();
    let job=store.coach_job_for_source(&binding.tutor_session_id,CoachJobOrigin::AfterTurn,&run.turn_ref.turn_id).unwrap().unwrap();
    assert!(!job.input.context["turn_facts"].as_array().unwrap().iter().any(|fact|fact["kind"]=="agent_work_reviewed"));
    assert!(store.agent_working_state(&binding).unwrap()["current_work"].is_null());
    let claimed=claim_exact(&state,&job);
    let frozen=observations::freeze_original_reads(&state.user,&claimed,&state.workspace.book,false).unwrap();
    assert_eq!(frozen.input.context["verified_read_ranges"],json!([{"start_lid":"1.1","end_lid":"1.1"}]));
    let hydrated=observations::with_material(&frozen,&state.workspace.book,&state.workspace.book_dir).unwrap();
    assert_eq!(hydrated.input.context["read_sources"][0]["text"],state.workspace.book.text("1.1",Some("1.1")).unwrap());
    let fact=&hydrated.input.context["observation_facts"][0];
    assert_eq!(fact["user_expression"],"也就是说停车也算总时间，对吗？");
    let help=fact["prior_help"].as_array().unwrap();
    assert!(!help.is_empty());
    assert!(help.iter().all(|item|item["binding"]["turn_id"]==prior.turn_ref.turn_id));
    commit_candidate(&state,&frozen,candidate(&hydrated));
    let saved=store.coach_job(&job.input.job_ref).unwrap().unwrap();
    assert_eq!(saved.status,CoachJobStatus::Ready);
    assert_eq!(saved.guidance.unwrap()["observation_receipts"][0]["status"],"recorded");
    assert!(store.agent_working_state(&binding).unwrap()["current_work"].is_null());
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
    let hydrated=observations::with_material(&recovered,&state.workspace.book,&state.workspace.book_dir).unwrap();
    assert_eq!(hydrated.input.context["read_sources"][0]["range"]["start_lid"],"1.1");
    retry_observation_backfill(&state.user,&run.turn_ref,"explicit").unwrap();
    let second=claim_exact(&state,&recovered);assert_eq!(second.attempt,2);
    commit_candidate(&state,&second,candidate(&second));
    let ready=store.coach_job(&job.input.job_ref).unwrap().unwrap();assert_eq!(ready.status,CoachJobStatus::Ready);
    assert_eq!(freeze_observation_reads(&state.user,&ready,&state.workspace.book,true).unwrap(),ready);
}

#[test]
fn inquiry_course_expression_uses_existing_understanding_without_a_chat_session() {
    course_expression_understanding(false);
}

#[test]
fn inquiry_course_expression_keeps_prior_agent_explanation_from_the_same_course() {
    course_expression_understanding(true);
}

fn course_expression_understanding(with_chat_help: bool) {
    use memory::course::*;
    let (_root, state) = fixture(true);
    let book = &state.workspace.book;
    let mut course:InquiryCourse = serde_json::from_str(include_str!("../../../../docs/examples/inquiry-course/ai-infra-resource-bounds.v1.course.json")).unwrap();
    let lesson = &mut course.lessons[0];
    lesson.summary.source.publication.book_id = book.base.book_id.clone();
    lesson.summary.source.source_revision = book.source_fingerprint().into();
    lesson.summary.source.start_lid = "1.1".into(); lesson.summary.source.end_lid = "1.1".into();
    let mut store = state.user.learning_store().unwrap();
    let start = CourseMutation {operation_id:"course-start".into(),run_id:None,expected_revision:0,release_version:1,
        action:CourseAction::Start {lesson_id:lesson.summary.id.clone()}};
    let mut v=store.mutate_course(&start,Some(("course-release",1,lesson)),1,"1",None).unwrap();
    let next=CourseMutation {operation_id:"course-next".into(),run_id:Some(v.progress.run_id.clone()),expected_revision:v.progress.revision,release_version:1,action:CourseAction::Advance};
    v=store.mutate_course(&next,None,1,"2",None).unwrap();
    let chat_help = format!("usage:observation:{}:assistant:course-chat-help",book.base.book_id);
    if with_chat_help {
        for (turn,run) in [("course-chat-help",v.progress.run_id.as_str()),("other-course-help","another-course")] {
            let binding=memory::teaching::TeachingBinding { tutor_session_id:format!("observation:{}",book.base.book_id),
                session_revision:0,control_revision:0,source_id:book.base.book_id.clone(),source_revision:book.source_fingerprint().into(),
                map_revision:None,chat_session_id:"book-chat".into(),turn_id:turn.into() };
            store.append_teaching(&memory::teaching::TeachingEvent { event_id:format!("binding:{turn}"),binding:binding.clone(),
                kind:memory::teaching::TeachingFact::TurnBound,causal_refs:vec![],occurred_at:"2".into(),
                payload:json!({"inquiry_course":{"lesson":{"progress":{"run_id":run}}}}) }).unwrap();
            store.append_teaching(&memory::teaching::TeachingEvent { event_id:format!("usage:{}:assistant:{turn}",binding.tutor_session_id),binding,
                kind:memory::teaching::TeachingFact::UsageObserved,causal_refs:vec![],occurred_at:"2".into(),
                payload:json!({"usage":{"kind":"assistant_delivery"},"answer":"原 Agent 已解释：总时间应该包含停车等待。"}) }).unwrap();
        }
    }
    let response=CourseMutation {operation_id:"course-expression".into(),run_id:Some(v.progress.run_id.clone()),expected_revision:v.progress.revision,release_version:1,
        action:CourseAction::Predict {response:CourseResponse {choice:"compute".into(),reason:"总时间应该包含停车等待".into()}}};
    v=store.mutate_course(&response,None,1,"3",None).unwrap();
    assert_eq!(v,store.mutate_course(&response,None,1,"4",None).unwrap());
    let review=&v.expressions[0];
    let job=store.claim_coach_job(&review.job_ref,"5").unwrap().unwrap();
    assert_eq!(job.input.context["course_response"]["selected_option"]["text"],lesson.prediction.options.iter().find(|o|o.id=="compute").unwrap().text);
    if with_chat_help {
        assert!(job.input.context["observation_facts"][0]["prior_help"].as_array().unwrap().iter().any(|h|h["text"]=="原 Agent 已解释：总时间应该包含停车等待。"));
    }
    assert!(applicable(&state.user,&job).unwrap());
    let job=observations::freeze_original_reads(&state.user,&job,book,false).unwrap();
    let hydrated=observations::with_material(&job,book,&state.workspace.book_dir).unwrap();
    assert!(!hydrated.input.context["read_sources"].as_array().unwrap().is_empty());
    let mut value=candidate(&job);
    value["course_feedback"]=json!({"response_quote":"停车等待","acknowledgement":"你保留了等待时间。","guidance":"比较两种时间组成。","follow_up":null});
    commit_candidate(&state,&job,value);
    let saved=store.coach_job(&review.job_ref).unwrap().unwrap();
    assert_eq!(saved.status,CoachJobStatus::Ready,"{:?}",saved.error);
    let evidence=store.teaching_expression_interpretations(&review.fact_ref).unwrap();
    assert_eq!(evidence.len(),1);
    assert_eq!(evidence[0].learner_quote,"总时间应该包含停车等待");
    assert_eq!(evidence[0].support_delivery_refs.len(),if with_chat_help {3} else {2},"later or other-course help must not become prior help");
    if with_chat_help { assert!(evidence[0].support_delivery_refs.contains(&chat_help)); }
    assert!(store.course_view(&v.progress.run_id).unwrap().reviews[0].feedback.is_none());
    let show=CourseMutation {operation_id:"show-feedback".into(),run_id:Some(v.progress.run_id.clone()),expected_revision:v.progress.revision,release_version:1,
        action:CourseAction::ReadFeedback {job_ref:review.job_ref.clone()}};
    store.mutate_course(&show,None,1,"6",None).unwrap();
    let visible=store.course_view(&v.progress.run_id).unwrap();
    assert!(visible.reviews[0].feedback.is_some());
    assert_eq!(visible.reviews[0].understanding[0].evidence_ref,evidence[0].evidence_id);
}
