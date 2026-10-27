use super::*;
use std::sync::{Arc, Mutex};

const NOW: &str = "2026-10-26T08:00:00Z";

fn fixture(logged: bool) -> (tempfile::TempDir, AppState, AgentTurnRef, crate::board_author::CommittedPart) {
    let (root, mut state, metadata) = crate::tests::board_browser_host::fixture();
    let sources = state.user.agent_history.sessions.iter().find(|session| session.id == metadata["session_id"])
        .unwrap().turns.iter().find(|turn| turn.turn_id == metadata["turn_id"]).unwrap().source_bindings.clone();
    let created = crate::board_api::route(&state.user,&state.workspace,"create",&json!({"title":"分段板书"}),NOW).unwrap();
    let document = crate::board_api::route(&state.user,&state.workspace,"import",&json!({"board_id":created["board_id"],
        "board_revision":created["revision"],"source":metadata["sources"]["prebuild"]}),NOW).unwrap();
    let follow_up: memory::boards::BoardFollowUp = serde_json::from_value(json!({"board_id":document["board_id"],
        "board_revision":document["revision"],"item_id":document["items"][0]["item_id"]})).unwrap();
    let frozen = crate::board_api::freeze_follow_up(&state.user,&state.workspace,&follow_up).unwrap();
    if logged {
        let (history,storage) = session_store::load_chat_storage(&state.user.history_path).unwrap();
        state.user.agent_history=history; state.user.session_store=storage;
        assert_eq!(route_agent_new(&mut state,NOW).status,200);
    }
    let book_id=state.workspace.book.base.book_id.clone();
    let turn = precommit_agent_turn_with_goal(&mut state,&book_id,"分两段解释平均速度。".into(),None,None,None,None,None,None,None,
        Some(crate::session_event::TurnDomain {board_context:Some(frozen),..Default::default()}),None,NOW).unwrap();
    let part = crate::board_author::CommittedPart {
        delivery: memory::boards::BoardDelivery {commit_id:Some("candidate-local-first".into()),board_id:follow_up.board_id,
            before_revision:follow_up.board_revision,after_revision:follow_up.board_revision+1,item_ids:vec!["explanation-first".into()],
            block_ids:vec!["block-first".into()],origin_session_id:turn.session_id.clone(),origin_turn_id:turn.turn_id.clone(),parent_item_id:follow_up.item_id},
        candidate_revision:1,delivery_text:"第一步，分别计算两段时间。".into(),
        answer_parts:vec![json!({"kind":"markdown","text":"第一步，分别计算两段时间。"})],source_bindings:sources,
        teaching_delivery_move_refs:vec![],committed_at:NOW.into(),delivery_scope:Some(json!({"kind":"board_part"})),
    };
    (root,state,turn,part)
}

#[test]
fn wb4_committed_session_part_is_durable_pending_idempotent_and_survives_cancelled_reopen() {
    let (_root,mut state,reference,part)=fixture(true);
    record_board_commit(&mut state.user,&reference,&part).unwrap();
    let seq=state.user.session_store.as_ref().unwrap().logs[&reference.session_id].projection.through_seq;
    record_board_commit(&mut state.user,&reference,&part).unwrap();
    assert_eq!(state.user.session_store.as_ref().unwrap().logs[&reference.session_id].projection.through_seq,seq);
    assert_eq!(current_turn(&state.user,&reference).unwrap().status,AgentAssistantStatus::PendingAssistant);
    let mut changed=part.clone(); changed.delivery_text="a different delivery".into();
    assert!(record_board_commit(&mut state.user,&reference,&changed).is_err());
    let messages=state.workspace.messages.clone();
    finalize_user_agent_turn(&mut state.user,&reference,AgentAssistantStatus::Cancelled,None,
        Some(AgentTurnError {error_code:"AGENT_RUN_CANCELLED".into(),category:"cancelled".into(),message:"Stopped after the first part".into()}),
        None,&messages,NOW).unwrap();
    let (history,storage)=session_store::load_chat_storage(&state.user.history_path).unwrap();
    state.user.agent_history=history; state.user.session_store=storage;
    let turn=current_turn(&state.user,&reference).unwrap();
    assert_eq!(turn.status,AgentAssistantStatus::Cancelled);
    assert_eq!(json!(turn.domain.board_commits),json!([part]));
    assert_eq!(json!(turn_view(&state.workspace.book,turn))["board_commits"],json!([part.delivery]));
    assert_eq!(crate::authorization::turn_json(turn)["board_commits"],json!([part.delivery]));
    record_board_commit(&mut state.user,&reference,&part).unwrap();
    let state=Arc::new(Mutex::new(state));
    let coordinator=crate::agent_run::RunCoordinator::new(state.clone(),Arc::new(std::sync::atomic::AtomicBool::new(false)),
        crate::observability::ObservabilityRuntime::disabled(None));
    let snapshot=coordinator.stream(&reference.turn_id).unwrap().snapshot();
    assert_eq!(snapshot.execution_state,"cancelled");
    assert_eq!(json!(snapshot.board_commits),json!([part.delivery]));
    let source_id=part.source_bindings[0].source_ref_id.clone();
    let locked=state.lock().unwrap();
    let binding=workspace_source_binding(&locked.user,&locked.workspace,&AgentSourceRequest {
        turn_id:reference.turn_id,source_ref_id:source_id,note_mem_id:None,
    }).unwrap();
    assert_eq!(binding,part.source_bindings[0]);
}

#[test]
fn wb4_committed_session_part_survives_interruption_and_allows_exact_receipt_repair() {
    let (_root,mut state,reference,part)=fixture(true);
    record_board_commit(&mut state.user,&reference,&part).unwrap();
    recover(state.user.session_store.as_mut().unwrap(),&mut state.user.agent_history,&BTreeSet::new()).unwrap();
    assert_eq!(current_turn(&state.user,&reference).unwrap().status,AgentAssistantStatus::Failed);
    let mut second=part.clone(); second.delivery.commit_id=Some("candidate-local-second".into()); second.candidate_revision=2;
    second.delivery.before_revision=part.delivery.after_revision; second.delivery.after_revision+=1;
    second.delivery.item_ids=vec!["explanation-second".into()]; second.delivery.block_ids=vec!["block-second".into()];
    record_board_commit(&mut state.user,&reference,&second).unwrap();
    let (history,_) = session_store::load_chat_storage(&state.user.history_path).unwrap();
    let reopened=history.sessions.iter().find(|session|session.id==reference.session_id).unwrap().turns.iter()
        .find(|turn|turn.turn_id==reference.turn_id).unwrap();
    assert_eq!(json!(reopened.domain.board_commits),json!([part,second]));
    assert_eq!(reopened.status,AgentAssistantStatus::Failed);
}

#[test]
fn wb4_legacy_commit_and_presentation_effects_persist_without_replacing_receipts() {
    let (_root,mut state,reference,part)=fixture(false);
    assert!(state.user.session_store.is_none());
    record_board_commit(&mut state.user,&reference,&part).unwrap();
    record_board_commit(&mut state.user,&reference,&part).unwrap();
    let presentation=runtime::presentation::PresentationRef {presentation_id:"local-part-scene".into(),revision:1};
    deliver_presentations(&mut state.user,&reference,&[presentation.clone()],NOW).unwrap();
    deliver_presentations(&mut state.user,&reference,&[presentation.clone()],NOW).unwrap();
    let history: AgentHistory=serde_json::from_slice(&std::fs::read(state.user.history_path.as_ref().unwrap()).unwrap()).unwrap();
    let turn=history.sessions.iter().find(|session|session.id==reference.session_id).unwrap().turns.iter().find(|turn|turn.turn_id==reference.turn_id).unwrap();
    assert_eq!(json!(turn.domain.board_commits),json!([part]));
    assert_eq!(turn.domain.effects.iter().filter(|effect|matches!(&effect.effect,
        crate::session_event::DeliveredEffect::Presentation {reference} if reference==&presentation)).count(),1);
    let mut wrong=part; wrong.delivery.origin_turn_id="other-turn".into();
    assert!(record_board_commit(&mut state.user,&reference,&wrong).is_err());
}
