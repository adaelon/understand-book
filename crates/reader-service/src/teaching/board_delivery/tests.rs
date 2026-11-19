use super::*;
use memory::boards::{BoardItem,BoardPosition,BoardSize,BoardTarget};

const NOW:&str="2026-10-25T20:00:00Z";

fn reopen_fixture(state:&mut AppState) {
    // board_browser_host::fixture writes history.json snapshots. Its old chat
    // and presentation ownership must be reopened with that storage format;
    // load_chat_storage intentionally starts a separate JSONL history.
    assert!(state.user.session_store.is_none());
    let history_path=state.user.history_path.as_ref().unwrap();
    assert!(history_path.is_file());
    assert!(!crate::session_store::SessionPaths::from_history(history_path).root.exists());
    let memory_path=history_path.parent().unwrap().join("memory.json");
    state.user.agent_history=load_agent_history(&state.user.history_path).unwrap();
    state.user.store=MemoryStore::open(memory_path).unwrap();
    let selected=state.workspace.selected_chat.as_ref().unwrap();
    let session=state.user.agent_history.sessions.iter().find(|session|&session.id==selected).unwrap();
    state.workspace.messages=session.messages.clone();
}

fn setup()->(tempfile::TempDir,AppState,crate::agent_run::PreparedAgentChat) {
    let (root,mut state,metadata)=crate::tests::board_browser_host::fixture();
    reopen_fixture(&mut state);
    let board=crate::board_api::route(&state.user,&state.workspace,"create",&json!({"title":"局部讲解"}),NOW).unwrap();
    let board=crate::board_api::route(&state.user,&state.workspace,"import",&json!({"board_id":board["board_id"],
        "board_revision":board["revision"],"source":metadata["sources"]["prebuild"]}),NOW).unwrap();
    let run=prepare_agent_chat(&mut state,&json!({"message":"先说明总时间，再用不同速度比较全程。","client_request_id":"wb4-local-teaching",
        "session_id":metadata["session_id"],"expected_chat_head":metadata["turn_id"],
        "board_follow_up":{"board_id":board["board_id"],"board_revision":board["revision"],"item_id":board["items"][0]["item_id"]}}).to_string(),NOW)
        .unwrap_or_else(|reply|panic!("{}",reply.body));
    (root,state,run)
}

fn support() -> Value {
    json!({"delivery_draft":{"text":"The whole journey includes moving time and stopped time. Add the intervals before comparing its average speed."},"delivery_move_refs":[]})
}

fn commit(state:&mut AppState,run:&crate::agent_run::PreparedAgentChat,selected:&Value,id:&str,
    extra:Vec<AgentAnswerPart>)->crate::board_author::CommittedPart {
    let frozen=run.scope.board_context.as_ref().unwrap();
    let store=crate::board_api::scoped_store(&state.user,frozen.learning_workspace_id.as_deref()).unwrap();
    let current=store.read(&frozen.follow_up.board_id).unwrap();
    let text=selected["delivery_draft"]["text"].as_str().unwrap_or("已经正式交付的局部演示。").to_owned();
    let mut parts=vec![AgentAnswerPart::Markdown {text:text.clone()}];parts.extend(extra);
    let answer_parts:Vec<Value>=parts.iter().map(|part|json!(part)).collect();
    let item=BoardItem {item_id:format!("item-{id}"),title:"局部解释".into(),readable_content:text.clone(),
        position:BoardPosition {x:800.0,y:400.0},size:BoardSize {width:300.0,height:200.0},created_at:NOW.into(),
        reply_to_item_id:None,block_id:None,explanation_blocks:vec![],source_bindings:vec![],
        target:BoardTarget::Answer {session_id:run.turn_ref.session_id.clone(),turn_id:run.turn_ref.turn_id.clone(),
            question:"比较速度".into(),answer_parts:answer_parts.clone()}};
    let delivery=store.append_authored_part(&frozen.follow_up,current.revision,&run.turn_ref.session_id,&run.turn_ref.turn_id,
        id,vec![item],vec![],vec![],vec![],vec![],NOW).unwrap();
    let part=crate::board_author::CommittedPart {delivery,candidate_revision:1,delivery_text:text,answer_parts,
        source_bindings:vec![],teaching_delivery_move_refs:serde_json::from_value(selected["delivery_move_refs"].clone()).unwrap_or_default(),
        committed_at:NOW.into(),delivery_scope:selected.get("delivery_scope").cloned()};
    crate::session_runtime::record_board_commit(&mut state.user,&run.turn_ref,&part).unwrap();
    part
}

fn cancel_and_reopen(state:&mut AppState,turn:&AgentTurnRef) {
    let cancellation=runtime::run_context::CancellationToken::default();
    cancellation.cancel();
    let error=cancellation.check().unwrap_err();
    let messages=state.workspace.messages.clone();
    finalize_agent_turn(state,turn,AgentAssistantStatus::Cancelled,None,
        Some(AgentTurnError {error_code:error.error_code,category:error.category,message:error.message}),
        None,&messages,NOW).unwrap();
    reopen_fixture(state);
    let saved=state.user.agent_history.sessions.iter().find(|session|session.id==turn.session_id).unwrap()
        .turns.iter().find(|saved|saved.turn_id==turn.turn_id).unwrap();
    assert_eq!(saved.status,AgentAssistantStatus::Cancelled);
    assert!(saved.outcome.is_none());
    assert_eq!(saved.error.as_ref().unwrap().error_code,"AGENT_RUN_CANCELLED");
}

#[test]
fn wb4_ordinary_board_support_survives_cancel_and_freezes_help_for_the_next_expression() {
    let (_root,mut state,run)=setup();
    let selected=support();
    assert!(record_board_local_delivery(&state.user,&run.turn_ref,"part-one",NOW).is_err(),"an uncommitted draft is not a delivery");
    let part=commit(&mut state,&run,&selected,"part-one",vec![]);
    // Exercise repair after cancellation: the board content is already durable.
    cancel_and_reopen(&mut state,&run.turn_ref);
    record_board_local_delivery(&state.user,&run.turn_ref,"part-one","later-repair").unwrap();
    record_board_local_delivery(&state.user,&run.turn_ref,"part-one","retry").unwrap();
    let deliveries=state.user.learning_store().unwrap().teaching_deliveries(&run.turn_ref.session_id,&run.turn_ref.turn_id).unwrap();
    assert_eq!(deliveries.len(),1);
    let delivery=&deliveries[0];
    assert_eq!(delivery.payload["answer"],part.delivery_text);
    assert_eq!(delivery.occurred_at,NOW,"recovery retains actual commit time");
    assert_eq!(delivery.payload["actor"],"teacher");assert!(delivery.learner_expression().is_none());
    let events=state.user.learning_store().unwrap().teaching_turn_events(&delivery.binding.tutor_session_id,&run.turn_ref.turn_id).unwrap();
    assert!(!events.iter().any(|event|event.kind==TeachingFact::HelpDisplayed));
    let next=prepare_agent_chat(&mut state,&json!({"message":"我知道应该把时间先加起来了。","client_request_id":"wb4-after-local-help",
        "session_id":run.turn_ref.session_id,"expected_chat_head":run.turn_ref.turn_id}).to_string(),"2026-10-25T20:01:00Z")
        .unwrap_or_else(|reply|panic!("{}",reply.body));
    let grounding=&next.tutor.as_ref().unwrap()["learner_context"]["learner_grounding"];
    assert!(grounding["expressions"].as_array().unwrap().iter().any(|expression|
        expression["text"]=="我知道应该把时间先加起来了。" && expression["support_delivery_refs"].as_array().unwrap().contains(&json!(delivery.event_id))));
    assert!(grounding["delivered_support"].as_array().unwrap().iter().any(|support|support["delivery_ref"]==delivery.event_id));
}

#[test]
fn wb4_final_delivery_does_not_replace_a_committed_local_support() {
    let (_root,mut state,run)=setup();
    let selected=support();
    let part=commit(&mut state,&run,&selected,"part-one",vec![]);
    record_board_local_delivery(&state.user,&run.turn_ref,"part-one",NOW).unwrap();
    let mut outcome=crate::tests::tutor_loop_tests::outcome(None);
    outcome.answer=Some("两个局部已分别说明总时间与平均速度，可以按画板上的顺序回顾。".into());
    outcome.answer_view=Some(AgentAnswerView {parts:vec![AgentAnswerPart::Markdown {text:outcome.answer.clone().unwrap()}],sources:vec![]});
    outcome.teaching_delivery_move_refs=serde_json::from_value(selected["delivery_move_refs"].clone()).unwrap();
    crate::tests::tutor_loop_tests::finish(&mut state,&run.turn_ref,&outcome);
    record_user_delivery(&state.user,&run.turn_ref,"finish-retry").unwrap();
    let deliveries=state.user.learning_store().unwrap().teaching_deliveries(&run.turn_ref.session_id,&run.turn_ref.turn_id).unwrap();
    assert_eq!(deliveries.len(),1);assert_eq!(deliveries[0].payload["answer"],part.delivery_text);
    assert_eq!(review_context(&state.user,&run.turn_ref)[0]["text"],part.delivery_text);
}

#[test]
fn wb4_committed_presentation_versions_are_readable_while_pending_and_after_cancel() {
    let (_root,mut state,run)=setup();
    let old_session=state.user.agent_history.sessions.iter().find(|session|session.id==run.turn_ref.session_id).unwrap();
    let old_ref=old_session.turns.iter().find_map(|turn|turn.outcome.as_ref().and_then(|outcome|outcome.answer_view.as_ref())
        .and_then(|view|view.parts.iter().find_map(|part|if let AgentAnswerPart::Presentation {presentation_id,revision}=part {
            Some(runtime::presentation::PresentationRef {presentation_id:presentation_id.clone(),revision:*revision})
        } else {None}))).unwrap();
    let mut content=state.private_context().read_presentation(&run.turn_ref.session_id,&old_ref).unwrap().content;
    let first=state.private_context().create_presentation_candidate(&run.turn_ref.session_id,&run.turn_ref.turn_id,None,content.clone()).unwrap();
    let v1=state.private_context().persist_presentation_candidate(&run.turn_ref.session_id,&run.turn_ref.turn_id,&first.candidate_id).unwrap();
    assert!(crate::presentation_api::delivered_version(&state.private_context(),&run.turn_ref.session_id,&run.turn_ref.turn_id,&v1).is_err());
    commit(&mut state,&run,&Value::Null,"page-one",vec![AgentAnswerPart::Presentation {presentation_id:v1.presentation_id.clone(),revision:v1.revision}]);
    crate::session_runtime::deliver_presentations(&mut state.user,&run.turn_ref,std::slice::from_ref(&v1),NOW).unwrap();
    record_board_local_delivery(&state.user,&run.turn_ref,"page-one",NOW).unwrap();
    let original=crate::presentation_api::delivered_version(&state.private_context(),&run.turn_ref.session_id,&run.turn_ref.turn_id,&v1).unwrap();
    let receipt=crate::presentation_store::PresentationStore::for_user(&state.user).unwrap().save_retained_state(&original,&run.turn_ref.turn_id,
        runtime::presentation::PresentationState {values:json!({"page":{"return_speed":1.5},"controls":[],"parameter_edits":{}}),
            visible_step:None,observed_result:"已保存的旧现场".into(),source_ref_ids:vec![],observation:None}).unwrap();
    content.title="第二版演示".into();
    let second=state.private_context().create_presentation_candidate(&run.turn_ref.session_id,&run.turn_ref.turn_id,Some(v1.clone()),content).unwrap();
    let v2=state.private_context().persist_presentation_candidate(&run.turn_ref.session_id,&run.turn_ref.turn_id,&second.candidate_id).unwrap();
    commit(&mut state,&run,&Value::Null,"page-two",vec![AgentAnswerPart::Presentation {presentation_id:v2.presentation_id.clone(),revision:v2.revision}]);
    crate::session_runtime::deliver_presentations(&mut state.user,&run.turn_ref,std::slice::from_ref(&v2),NOW).unwrap();
    cancel_and_reopen(&mut state,&run.turn_ref);
    record_board_local_delivery(&state.user,&run.turn_ref,"page-two","repair").unwrap();
    assert_eq!(crate::presentation_api::delivered_version(&state.private_context(),&run.turn_ref.session_id,&run.turn_ref.turn_id,&v1).unwrap().reference,v1);
    assert_eq!(crate::presentation_api::delivered_version(&state.private_context(),&run.turn_ref.session_id,&run.turn_ref.turn_id,&v2).unwrap().reference,v2);
    let saved=crate::presentation_store::PresentationStore::for_user(&state.user).unwrap().read_retained_state(&original.owner,&receipt).unwrap();
    assert_eq!(saved.receipt.reference,v1);assert_eq!(saved.state.values["page"]["return_speed"],1.5);
}
