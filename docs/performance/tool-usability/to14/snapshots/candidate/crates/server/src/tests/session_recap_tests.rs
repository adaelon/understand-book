use super::*;
use crate::session_event::EventBody;

#[test]
fn jl8_recap_keep_then_undo_uses_retained_object_and_each_cutoff() {
    for highlight in [false, true] {
        let (_dir, mut state, turn, mut input, original) =
            super::effect_disposition_tests::fixture(highlight);
        let kept = crate::effect_disposition::route(&mut state, &input, "kept").unwrap();
        let retained = kept
            .receipt
            .as_ref()
            .unwrap()
            .result_object_id
            .clone()
            .unwrap();
        let kept_recap = recap(&mut state, &turn.session_id, None);
        assert_eq!(kept_recap["effects"][0]["status"], "kept");
        input["action"] = json!("undo");
        let crate::effect_disposition::Start::Execute(reference, record) =
            crate::effect_disposition::start(
                &mut state.user,
                &state.workspace,
                &input,
                "undo-started",
            )
            .unwrap()
        else {
            panic!()
        };
        let pending = recap(&mut state, &turn.session_id, None);
        assert_eq!(pending["effects"][0]["status"], "unconfirmed");
        assert_eq!(pending["effects"][0]["object_id"], retained);
        let receipt = crate::effect_disposition::execute(
            &mut state.user,
            &mut state.workspace,
            &record,
            "undone",
        );
        let undone = crate::effect_disposition::complete(
            &mut state.user,
            &reference,
            &record,
            receipt,
            "undone",
        )
        .unwrap();
        assert!(undone.receipt.as_ref().unwrap().error.is_none());
        assert_eq!(
            undone
                .receipt
                .as_ref()
                .unwrap()
                .original_object_id
                .as_deref(),
            Some(retained.as_str())
        );
        assert!(!state
            .user
            .store
            .recall(&Default::default())
            .iter()
            .any(|r| r.mem_id == retained));
        if highlight {
            assert!(state
                .user
                .store
                .recall(&Default::default())
                .iter()
                .any(|r| r.mem_id == original));
        }
        let (history, store) =
            crate::session_store::load_chat_storage(&state.user.history_path).unwrap();
        state.user.agent_history = history;
        state.user.session_store = store;
        let final_recap = recap(&mut state, &turn.session_id, None);
        assert_eq!(final_recap["effects"][0]["status"], "undone");
        assert_eq!(
            final_recap["effects"][0]["evidence"]
                .as_array()
                .unwrap()
                .len(),
            5
        );
        assert_eq!(
            recap(
                &mut state,
                &turn.session_id,
                kept_recap["through_seq"].as_u64()
            )["effects"][0]["status"],
            "kept"
        );
        assert_eq!(
            json!(crate::effect_disposition::route(&mut state, &input, "retry").unwrap()),
            json!(undone)
        );
        input["action"] = json!("keep");
        assert!(crate::effect_disposition::route(&mut state, &input, "keep-again").is_err());
    }
}

fn recap(state: &mut AppState, session: &str, through: Option<u64>) -> Value {
    let url = format!(
        "/agent/history/recap?session_id={session}{}",
        through
            .map(|n| format!("&through_seq={n}"))
            .unwrap_or_default()
    );
    let reply = get(state, &url);
    assert_eq!(reply.status, 200, "{}", reply.body);
    serde_json::from_str(&reply.body).unwrap()
}

#[test]
fn jl8_recap_disposition_cutoffs_and_current_availability_are_separate() {
    for action in ["keep", "undo"] {
        let (_dir, mut state, turn, mut input, _) = super::effect_disposition_tests::fixture(true);
        input["action"] = json!(action);
        let before = recap(&mut state, &turn.session_id, None);
        let cutoff = before["through_seq"].as_u64().unwrap();
        assert_eq!(before["effects"][0]["status"], "pending");
        assert_eq!(before["questions"][0]["status"], "interrupted");
        let crate::effect_disposition::Start::Execute(reference, record) =
            crate::effect_disposition::start(&mut state.user, &state.workspace, &input, "started")
                .unwrap()
        else {
            panic!()
        };
        let started = recap(&mut state, &turn.session_id, None);
        assert_eq!(started["effects"][0]["status"], "unconfirmed");
        let receipt = crate::effect_disposition::execute(
            &mut state.user,
            &mut state.workspace,
            &record,
            "executed",
        );
        crate::effect_disposition::complete(
            &mut state.user,
            &reference,
            &record,
            receipt.clone(),
            "done",
        )
        .unwrap();
        let done = recap(&mut state, &turn.session_id, None);
        assert_eq!(
            done["effects"][0]["status"],
            if action == "keep" { "kept" } else { "undone" }
        );
        assert_eq!(
            recap(&mut state, &turn.session_id, Some(cutoff))["effects"][0]["status"],
            "pending"
        );
        assert_eq!(
            recap(
                &mut state,
                &turn.session_id,
                started["through_seq"].as_u64()
            )["effects"][0]["status"],
            "unconfirmed"
        );
        for e in done["effects"][0]["evidence"].as_array().unwrap() {
            assert_eq!(e["turn_id"], turn.turn_id);
            assert!(e["event_seq"].as_u64().unwrap() <= done["through_seq"].as_u64().unwrap());
        }
        if action == "keep" {
            assert_eq!(
                done["effects"][0]["object_id"],
                receipt.result_object_id.as_ref().unwrap().as_str()
            );
            state
                .user
                .store
                .delete(receipt.result_object_id.as_ref().unwrap())
                .unwrap();
            let deleted = recap(&mut state, &turn.session_id, done["through_seq"].as_u64());
            assert_eq!(deleted["effects"][0]["status"], "kept");
            assert!(deleted["effects"][0]["unavailable_reason"].is_string());
        }
    }
}

#[test]
fn jl8_recap_is_read_only_deterministic_and_rejects_uncommitted_ranges() {
    let (_dir, mut state) = super::session_runtime_tests::setup();
    let path = state.user.history_path.clone().unwrap();
    let (turn, source) = install_source_bound_turn(&mut state, path);
    let session = state.user.agent_history.sessions[0].id.clone();
    let first = recap(&mut state, &session, None);
    assert_eq!(first["sources"].as_array().unwrap().len(), 1);
    assert_eq!(first["sources"][0]["source_ref_id"], source);
    assert_eq!(first["questions"][0]["status"], "answered");
    let accepted = first["questions"][0]["evidence"][0]["event_seq"]
        .as_u64()
        .unwrap();
    let early = recap(&mut state, &session, Some(accepted));
    assert_eq!(early["questions"][0]["status"], "running");
    assert!(early["sources"].as_array().unwrap().is_empty());
    assert!(!early["continuations"].as_array().unwrap().is_empty());
    let log_path = state
        .user
        .session_store
        .as_ref()
        .unwrap()
        .paths
        .session(&session);
    let log_bytes = std::fs::read(&log_path).unwrap();
    let memory = json!(state.user.store.recall(&Default::default()));
    let learning = json!(state.user.learning_store().unwrap().state().unwrap());
    let reader = json!(state.workspace.reader.state());
    let (history, store) =
        crate::session_store::load_chat_storage(&state.user.history_path).unwrap();
    state.user.agent_history = history;
    state.user.session_store = store;
    let mut again = recap(&mut state, &session, first["through_seq"].as_u64());
    again["generated_at"] = first["generated_at"].clone();
    assert_eq!(first, again);
    assert_eq!(std::fs::read(&log_path).unwrap(), log_bytes);
    assert_eq!(json!(state.user.store.recall(&Default::default())), memory);
    assert_eq!(
        json!(state.user.learning_store().unwrap().state().unwrap()),
        learning
    );
    assert_eq!(json!(state.workspace.reader.state()), reader);
    assert!(!again.to_string().contains("provider_continuation"));
    for suffix in ["&through_seq=0", "&through_seq=999999", "&through_seq=bad"] {
        assert_eq!(
            get(
                &mut state,
                &format!("/agent/history/recap?session_id={session}{suffix}")
            )
            .status,
            400
        );
    }
    assert_ne!(
        get(&mut state, "/agent/history/recap?session_id=missing").status,
        200
    );
    assert_ne!(
        post(
            &mut state,
            &format!("/agent/history/recap?session_id={session}"),
            "{}"
        )
        .status,
        200
    );
    // A complete but unacknowledged append must not be adopted by a read.
    let reference = AgentTurnRef {
        session_id: session.clone(),
        turn_id: turn,
        user_turn_ordinal: 1,
    };
    state
        .user
        .session_store
        .as_mut()
        .unwrap()
        .logs
        .get_mut(&session)
        .unwrap()
        .fail_write = Some(true);
    assert!(crate::session_runtime::append(
        &mut state.user,
        &reference,
        "uncertain",
        EventBody::TeachingLinked(crate::session_event::TeachingLink {
            session_id: "teaching".into(),
            revision: 1,
            receipt_ids: vec!["receipt".into()],
        })
    )
    .is_err());
    assert_eq!(
        recap(&mut state, &session, None)["through_seq"],
        first["through_seq"]
    );
}

#[test]
fn jl8_recap_goals_and_failed_or_dismissed_effects_use_explicit_facts() {
    let (_dir, mut state, turn, input, original) = super::effect_disposition_tests::fixture(false);
    state.user.store.delete(&original).unwrap();
    crate::effect_disposition::route(&mut state, &input, "failed").unwrap();
    let mut goal = runtime::goal::ResidentGoal::new(
        "goal".into(),
        turn.turn_id.clone(),
        "Continue the comparison".into(),
    );
    crate::session_runtime::append(
        &mut state.user,
        &turn,
        "goal",
        EventBody::GoalUpdated { goal: goal.clone() },
    )
    .unwrap();
    let before = recap(&mut state, &turn.session_id, None);
    assert_eq!(before["effects"][0]["status"], "failed");
    assert!(before["continuations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["goal_id"] == "goal"));
    goal.status = runtime::goal::GoalStatus::Completed;
    goal.revision += 1;
    crate::session_runtime::append(
        &mut state.user,
        &turn,
        "complete",
        EventBody::GoalUpdated { goal },
    )
    .unwrap();
    assert!(!recap(&mut state, &turn.session_id, None)["continuations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["goal_id"] == "goal"));
    assert!(
        recap(&mut state, &turn.session_id, before["through_seq"].as_u64())["continuations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["goal_id"] == "goal")
    );
    let effect = AgentEffect::LayoutProposal {
        proposal: read_tools::ReaderLayoutProposal {
            proposal_id: "unapplied".into(),
            base_layout_rev: state.workspace.reader.layout_state().rev,
            actions: vec![],
            summary: "proposal".into(),
        },
    };
    crate::session_runtime::deliver_effects(&mut state.user, &turn, &[effect.clone()]).unwrap();
    let input = json!({"session_id":turn.session_id,"turn_id":turn.turn_id,"effect_id":runtime::orchestrator::effect_id(&effect),"action":"dismiss"});
    crate::effect_disposition::route(&mut state, &input, "dismiss").unwrap();
    assert_eq!(
        recap(&mut state, &turn.session_id, None)["effects"][1]["status"],
        "dismissed"
    );
}


#[test]
fn rn4_recap_follows_edited_note_and_reports_deletion() {
    let (_dir, mut state, turn, input, _) = super::effect_disposition_tests::fixture(false);
    crate::effect_disposition::route(&mut state, &input, "kept").unwrap();
    let before = recap(&mut state, &turn.session_id, None);
    let original = before["effects"][0]["object_id"].as_str().unwrap();
    let saved = state.user.store.replace(memory::ReplaceInput {
        mem_id: original.into(), content: "edited in RN4".into(), selection_context: None,
    }, "edited").unwrap();
    assert_ne!(saved.mem_id, original);
    let cutoff = before["through_seq"].as_u64();
    let after = recap(&mut state, &turn.session_id, cutoff);
    assert_eq!(after["effects"][0]["object_id"], saved.mem_id);
    assert_eq!(after["effects"][0]["unavailable_reason"], Value::Null);
    state.user.store.delete(&saved.mem_id).unwrap();
    let deleted = recap(&mut state, &turn.session_id, cutoff);
    assert_eq!(deleted["effects"][0]["unavailable_reason"], "原成果当前不可用");
}
