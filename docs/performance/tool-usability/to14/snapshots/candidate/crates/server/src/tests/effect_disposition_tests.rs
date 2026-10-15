use super::*;
use crate::effect_disposition::{self, Start};
use crate::session_event::*;

pub(super) fn fixture(highlight: bool) -> (tempfile::TempDir, AppState, AgentTurnRef, Value, String) {
    let (dir, mut state) = super::session_runtime_tests::setup();
    let book = state.workspace.book.base.book_id.clone();
    let reference = precommit_agent_turn(
        &mut state,
        &book,
        "Save this".into(),
        None,
        None,
        None,
        "t1",
    )
    .unwrap();
    let record = state
        .user
        .store
        .save(
            SaveInput {
                mem_id: None,
                mem_type: if highlight { "highlight" } else { "note" }.into(),
                layer: "session".into(),
                book_id: book,
                anchor: Anchor {
                    lid: Some("1.1".into()),
                    concept: None,
                },
                content: "original evidence".into(),
                range: highlight.then_some(memory::TextRange { start: 0, end: 1 }),
                selection_context: None,
                note: None,
                note_placement: None,
                citations: None,
                source_session_id: Some(reference.session_id.clone()),
            },
            "t1",
        )
        .unwrap();
    let effect = if highlight {
        AgentEffect::Highlight {
            mem_id: record.mem_id.clone(),
            lid: "1.1".into(),
        }
    } else {
        AgentEffect::Note {
            mem_id: record.mem_id.clone(),
            lid: "1.1".into(),
            text: record.content.clone(),
        }
    };
    crate::session_runtime::deliver_effects(&mut state.user, &reference, &[effect.clone()])
        .unwrap();
    finalize_agent_turn(
        &mut state,
        &reference,
        AgentAssistantStatus::Failed,
        None,
        Some(AgentTurnError {
            error_code: "PROVIDER_ERROR".into(),
            category: "provider".into(),
            message: "stopped after save".into(),
        }),
        Some(agent_run::AgentRunSummary {
            usage: None,
            effects: vec![effect.clone()],
            trace: vec![],
            activities: None,
            last_seq: None,
        }),
        &[],
        "t2",
    )
    .unwrap();
    let input = json!({"session_id":reference.session_id,"turn_id":reference.turn_id,
        "effect_id":runtime::orchestrator::effect_id(&effect),"action":"keep"});
    (dir, state, reference, input, record.mem_id)
}

fn reopen(state: &mut AppState) {
    let (history, store) =
        crate::session_store::load_chat_storage(&state.user.history_path).unwrap();
    state.user.agent_history = history;
    state.user.session_store = store;
}

#[test]
fn jl6_keep_roundtrip_preserves_object_mapping_and_exact_retry() {
    for highlight in [false, true] {
        let (_dir, mut state, reference, input, original) = fixture(highlight);
        let first = effect_disposition::route(&mut state, &input, "t3").unwrap();
        let receipt = first.receipt.as_ref().unwrap();
        assert!(receipt.error.is_none());
        assert_eq!(
            receipt.original_object_id.as_deref(),
            Some(original.as_str())
        );
        assert_eq!(
            receipt.result_object_id.as_deref() == Some(&original),
            !highlight
        );
        let records = state.user.store.recall(&RecallQuery {
            layer: Some("long_term".into()),
            ..Default::default()
        });
        assert_eq!(records.len(), 1);
        if highlight {
            assert_eq!(
                records[0].range,
                Some(memory::TextRange { start: 0, end: 1 })
            );
        }
        let path = state
            .user
            .session_store
            .as_ref()
            .unwrap()
            .paths
            .session(&reference.session_id);
        let bytes = std::fs::read(&path).unwrap();
        reopen(&mut state);
        let repeat = effect_disposition::route(&mut state, &input, "t4").unwrap();
        assert_eq!(json!(first), json!(repeat));
        assert_eq!(std::fs::read(path).unwrap(), bytes);
        let public = get(&mut state, "/agent/history");
        assert!(public.body.contains(&receipt.disposition_id));
        assert_eq!(
            state
                .user
                .store
                .recall(&RecallQuery {
                    layer: Some("long_term".into()),
                    ..Default::default()
                })
                .len(),
            1
        );
    }
}

#[test]
fn jl6_business_failure_and_undo_are_durable_results() {
    for missing in [false, true] {
        let (_dir, mut state, _, mut input, original) = fixture(false);
        input["action"] = json!("undo");
        if missing {
            state.user.store.delete(&original).unwrap();
        }
        let first = effect_disposition::route(&mut state, &input, "t3").unwrap();
        assert_eq!(first.receipt.as_ref().unwrap().error.is_some(), missing);
        reopen(&mut state);
        assert_eq!(
            json!(effect_disposition::route(&mut state, &input, "t4").unwrap()),
            json!(first)
        );
        assert!(state.user.store.recall(&Default::default()).is_empty());
    }
}

#[test]
fn jl6_result_saved_without_receipt_reconciles_or_stays_pending_without_reexecution() {
    for full in [false, true] {
        let (_dir, mut state, reference, input, _) = fixture(true);
        let Start::Execute(turn, record) =
            effect_disposition::start(&mut state.user, &state.workspace, &input, "t3").unwrap()
        else {
            panic!()
        };
        let receipt =
            effect_disposition::execute(&mut state.user, &mut state.workspace, &record, "t3");
        state
            .user
            .session_store
            .as_mut()
            .unwrap()
            .logs
            .get_mut(&reference.session_id)
            .unwrap()
            .fail_write = Some(full);
        assert!(effect_disposition::complete(
            &mut state.user,
            &turn,
            &record,
            receipt.clone(),
            "t3"
        )
        .is_err());
        reopen(&mut state);
        let recovered = effect_disposition::route(&mut state, &input, "t4").unwrap();
        assert_eq!(json!(recovered.receipt), json!(Some(receipt)));
        assert_eq!(
            state
                .user
                .store
                .recall(&RecallQuery {
                    layer: Some("long_term".into()),
                    ..Default::default()
                })
                .len(),
            1
        );
    }
    // Started without a result is not permission to re-execute a create.
    let (_dir, mut state, _, input, _) = fixture(true);
    assert!(matches!(
        effect_disposition::start(&mut state.user, &state.workspace, &input, "t3").unwrap(),
        Start::Execute(..)
    ));
    reopen(&mut state);
    assert!(effect_disposition::route(&mut state, &input, "t4")
        .unwrap()
        .receipt
        .is_none());
    assert!(state
        .user
        .store
        .recall(&RecallQuery {
            layer: Some("long_term".into()),
            ..Default::default()
        })
        .is_empty());
}

#[test]
fn jl6_unapplied_proposal_dismiss_and_stale_scene_do_not_apply_layout() {
    let (_dir, mut state, reference, mut input, _) = fixture(false);
    let effect = AgentEffect::LayoutProposal {
        proposal: read_tools::ReaderLayoutProposal {
            proposal_id: "proposal-original".into(),
            base_layout_rev: state.workspace.reader.layout_state().rev,
            actions: vec![],
            summary: "unapplied proposal".into(),
        },
    };
    crate::session_runtime::deliver_effects(&mut state.user, &reference, &[effect.clone()])
        .unwrap();
    input["effect_id"] = json!(runtime::orchestrator::effect_id(&effect));
    input["action"] = json!("dismiss");
    let before = state.workspace.reader.layout_state();
    state.workspace.generation += 1;
    assert_eq!(
        effect_disposition::route(&mut state, &input, "t3")
            .unwrap_err()
            .error_code,
        "WORKSPACE_STALE"
    );
    state.workspace.generation -= 1;
    let result = effect_disposition::route(&mut state, &input, "t4").unwrap();
    assert_eq!(result.started.action, DispositionAction::Dismiss);
    assert!(result.receipt.as_ref().unwrap().error.is_none());
    assert!(result.receipt.as_ref().unwrap().result_object_id.is_none());
    assert_eq!(state.workspace.reader.layout_state(), before);
    reopen(&mut state);
    assert_eq!(
        json!(effect_disposition::route(&mut state, &input, "t5").unwrap()),
        json!(result)
    );
}

#[test]
fn jl6_layout_and_minimap_use_real_proposals_and_operation_receipts() {
    for (kind, action, route, command) in [
        (
            "Layout",
            "undo",
            "/reader/layout.apply",
            json!({"actions":[{"kind":"open_slot","slot_id":"technical.evidence"}]}),
        ),
        (
            "LayoutProposal",
            "keep",
            "/reader/layout.apply",
            json!({"actions":[{"kind":"close_slot","slot_id":"technical.agent"}]}),
        ),
        (
            "PaperMinimap",
            "undo",
            "/reader/paper_minimap.apply",
            json!({"base_state_rev":0,"commands":[{"scope":"session","action":{"kind":"set_layer_visibility","layer":"arguments","visible":false}}]}),
        ),
        (
            "PaperMinimapProposal",
            "keep",
            "/reader/paper_minimap.apply",
            json!({"base_state_rev":0,"actor":"agent","commands":[{"scope":"session","action":{"kind":"set_mode_lens","mode":"deep"}}]}),
        ),
        (
            "PaperMinimapProposal",
            "dismiss",
            "/reader/paper_minimap.apply",
            json!({"base_state_rev":0,"actor":"agent","commands":[{"scope":"session","action":{"kind":"set_mode_lens","mode":"deep"}}]}),
        ),
    ] {
        let (_dir, mut state, reference, mut input, _) = fixture(false);
        let original_layout = state.workspace.reader.layout_state();
        let original_minimap = state.workspace.reader.paper_minimap_state();
        let response = post(&mut state, route, &command.to_string());
        assert_eq!(response.status, 200, "{}", response.body);
        let mut value: Value = serde_json::from_str(&response.body).unwrap();
        value["kind"] = json!(kind);
        let effect: AgentEffect = serde_json::from_value(value).unwrap();
        crate::session_runtime::deliver_effects(&mut state.user, &reference, &[effect.clone()])
            .unwrap();
        input["effect_id"] = json!(runtime::orchestrator::effect_id(&effect));
        input["action"] = json!(action);
        let result = post(&mut state, "/agent/effect/dispose", &input.to_string());
        assert_eq!(result.status, 200, "{}", result.body);
        let result: Value = serde_json::from_str(&result.body).unwrap();
        assert!(result["receipt"]["error"].is_null(), "{result}");
        match (kind, action) {
            ("Layout", _) => assert_eq!(
                state.workspace.reader.layout_state().open_slots,
                original_layout.open_slots
            ),
            ("LayoutProposal", _) => assert!(!state
                .workspace
                .reader
                .layout_state()
                .open_slots
                .contains(&"technical.agent".into())),
            ("PaperMinimap", _) => assert_eq!(
                state.workspace.reader.paper_minimap_state().session_overlay,
                original_minimap.session_overlay
            ),
            (_, "keep") => assert_eq!(
                state.workspace.reader.paper_minimap_state().mode,
                reader::PaperMinimapMode::Deep
            ),
            _ => assert_eq!(
                state.workspace.reader.paper_minimap_state(),
                original_minimap
            ),
        }
        let before = state.workspace.reader.revision();
        reopen(&mut state);
        assert_eq!(
            serde_json::from_str::<Value>(
                &post(&mut state, "/agent/effect/dispose", &input.to_string()).body
            )
            .unwrap(),
            result
        );
        assert_eq!(state.workspace.reader.revision(), before);
    }
}

#[test]
fn jl6_navigation_undo_is_bound_to_original_scene_and_actual_position() {
    for moved in [false, true] {
        let (_dir, mut state, reference, mut input, _) = fixture(false);
        let book = multi_leaf_base(&state.workspace.book.base.book_id, 3);
        state.workspace.book = Arc::new(Book::new(book, &"X".repeat(1000)));
        state.workspace.reader = Reader::new(&state.workspace.book, 1);
        state
            .workspace
            .reader
            .goto_lid(&state.workspace.book, &mut state.user.store, "1.2", "t2")
            .unwrap();
        let effect = AgentEffect::Goto {
            before_anchor: "1.1".into(),
            after_anchor: "1.2".into(),
        };
        crate::session_runtime::deliver_effects(&mut state.user, &reference, &[effect]).unwrap();
        input["effect_id"] = json!("navigation");
        input["action"] = json!("undo");
        if moved {
            state
                .workspace
                .reader
                .goto_lid(&state.workspace.book, &mut state.user.store, "1.3", "t3")
                .unwrap();
        }
        let result = effect_disposition::route(&mut state, &input, "t4").unwrap();
        assert_eq!(result.receipt.unwrap().error.is_some(), moved);
        assert_eq!(
            state.workspace.reader.state().viewport.anchor_lid,
            if moved { "1.3" } else { "1.1" }
        );
    }
}
