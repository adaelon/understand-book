use super::*;
use memory::boards::{BoardDelivery, BoardDocument, BoardFollowUp, BoardTarget};

const NOW: &str = "2026-10-25T10:00:00Z";
const LATER: &str = "2026-10-25T10:01:00Z";

#[test]
fn wb4_final_and_interrupted_runs_keep_parts_without_copying_the_whole_answer() {
    for status in [AgentAssistantStatus::Completed, AgentAssistantStatus::Cancelled, AgentAssistantStatus::Failed] {
        let mut fixture = fixture(true);
        let original = read_board(&fixture);
        let parts = fixture.outcome.answer_view.as_ref().unwrap().parts.clone();
        let part_text = parts.iter().filter_map(|part| match part {
            AgentAnswerPart::Markdown { text } => Some(text.as_str()),
            AgentAnswerPart::MusicExample { example } => Some(example.label.as_str()), _ => None,
        }).collect::<Vec<_>>().join("\n\n");
        let mut item = original.items[0].clone();
        item.item_id = "wb4-formal-part-answer".into();
        item.block_id = Some("wb4-formal-part-block".into());
        item.reply_to_item_id = fixture.selected.item_id.clone();
        item.position.x = 600.0;
        item.title = "已经交付的局部解释".into();
        item.readable_content = part_text.clone();
        item.target = BoardTarget::Answer {
            session_id: fixture.reference.session_id.clone(), turn_id: fixture.reference.turn_id.clone(),
            question: turn(&fixture).user.clone(), answer_parts: parts.iter().map(|part| json!(part)).collect(),
        };
        let boards = board_api::scoped_store(&fixture.state.user, original.learning_workspace_id.as_deref()).unwrap();
        let receipt = boards.append_authored_part(
            &fixture.selected, original.revision, &fixture.reference.session_id, &fixture.reference.turn_id,
            "board-local-unit-1", vec![item], vec![], vec![], vec![], vec![], NOW,
        ).unwrap();
        let part = board_author::CommittedPart {
            delivery: receipt.clone(), candidate_revision: 1,
            delivery_text: part_text,
            answer_parts: parts.iter().map(|part| json!(part)).collect(),
            source_bindings: fixture.outcome.source_bindings.clone(),
            teaching_delivery_move_refs: vec![], committed_at: NOW.into(), delivery_scope: None,
        };
        session_runtime::record_board_commit(&mut fixture.state.user, &fixture.reference, &part).unwrap();
        let committed = read_board(&fixture);

        if status == AgentAssistantStatus::Completed {
            complete(&mut fixture);
        } else {
            let messages = fixture.state.workspace.messages.clone();
            finalize_agent_turn(&mut fixture.state, &fixture.reference, status, None,
                Some(AgentTurnError { error_code: "WB4_STOPPED".into(), category: "runtime".into(), message: "Later work stopped".into() }),
                None, &messages, LATER).unwrap();
        }
        ensure_saved(&mut fixture.state.user, &fixture.reference, LATER).unwrap();
        repair(&mut fixture.state.user, LATER).unwrap();
        assert_eq!(read_board(&fixture), committed);
        assert!(turn(&fixture).board_delivery.is_none());
        assert!(turn(&fixture).board_delivery_error.is_none());
        assert_eq!(session_runtime::board_commits(turn(&fixture)), vec![receipt.clone()]);

        let frozen = turn(&fixture).domain.board_context.clone().unwrap();
        assert_eq!(board_api::deliver_follow_up(&fixture.state.user, &frozen,
            &fixture.reference.session_id, &fixture.reference.turn_id, LATER).unwrap_err().error_code,
            "BOARD_REQUEST_INVALID");
        let (history, storage) = session_store::load_chat_storage(&fixture.state.user.history_path).unwrap();
        fixture.state.user.agent_history = history;
        fixture.state.user.session_store = storage;
        assert_eq!(session_runtime::board_commits(turn(&fixture)), vec![receipt]);
        assert_eq!(read_board(&fixture), committed);
    }
}

struct FollowUpFixture {
    _root: tempfile::TempDir,
    state: AppState,
    selected: BoardFollowUp,
    reference: AgentTurnRef,
    outcome: OuterOutcome,
}

fn call(state: &AppState, operation: &str, input: Value) -> Value {
    board_api::route(&state.user, &state.workspace, operation, &input, NOW).unwrap()
}

fn read_board(fixture: &FollowUpFixture) -> BoardDocument {
    serde_json::from_value(
        call(
            &fixture.state,
            "read",
            json!({"board_id":fixture.selected.board_id}),
        )["document"]
            .clone(),
    )
    .unwrap()
}

fn turn(fixture: &FollowUpFixture) -> &AgentChatTurn {
    fixture
        .state
        .user
        .agent_history
        .sessions
        .iter()
        .find(|session| session.id == fixture.reference.session_id)
        .unwrap()
        .turns
        .iter()
        .find(|turn| turn.turn_id == fixture.reference.turn_id)
        .unwrap()
}

/// Real retained source and compiled answer; the new turn is admitted and
/// finalized through the same persistence boundaries as a Resident run.
fn fixture(logged: bool) -> FollowUpFixture {
    let (root, mut state, metadata) = crate::tests::board_browser_host::fixture();
    let original = state
        .user
        .agent_history
        .sessions
        .iter()
        .find(|session| session.id == metadata["session_id"])
        .unwrap()
        .turns
        .iter()
        .find(|turn| turn.turn_id == metadata["turn_id"])
        .unwrap();
    let mut outcome = original.outcome.clone().unwrap();
    outcome.source_bindings = original.source_bindings.clone();

    let board = call(&state, "create", json!({"title":"平均速度的持续讲解"}));
    let board = call(
        &state,
        "import",
        json!({
            "board_id":board["board_id"], "board_revision":board["revision"],
            "source":metadata["sources"]["prebuild"],
        }),
    );
    let selected: BoardFollowUp = serde_json::from_value(json!({
        "board_id":board["board_id"], "board_revision":board["revision"],
        "item_id":board["items"][0]["item_id"],
    }))
    .unwrap();
    let frozen = board_api::freeze_follow_up(&state.user, &state.workspace, &selected).unwrap();

    if logged {
        // Start a genuinely new log-backed chat. The original fixture chat is
        // intentionally absent; its prebuilt card still supplies the context.
        let (history, storage) =
            session_store::load_chat_storage(&state.user.history_path).unwrap();
        state.user.agent_history = history;
        state.user.session_store = storage;
        assert_eq!(route_agent_new(&mut state, NOW).status, 200);
        outcome
            .answer_view
            .as_mut()
            .unwrap()
            .parts
            .retain(|part| !matches!(part, AgentAnswerPart::Presentation { .. }));
    }
    let book_id = state.workspace.book.base.book_id.clone();
    let reference = precommit_agent_turn_with_goal(
        &mut state,
        &book_id,
        "请沿着这张卡片解释完整的例子。".into(),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(session_event::TurnDomain {
            board_context: Some(frozen),
            ..Default::default()
        }),
        None,
        false,
        NOW,
    )
    .unwrap();
    FollowUpFixture {
        _root: root,
        state,
        selected,
        reference,
        outcome,
    }
}

fn complete(fixture: &mut FollowUpFixture) {
    let messages = fixture.state.workspace.messages.clone();
    finalize_agent_turn_completed(
        &mut fixture.state,
        &fixture.reference,
        &fixture.outcome,
        &messages,
        NOW,
    )
    .unwrap();
}

#[test]
fn persisted_incomplete_board_delivery_reopens_without_rewriting_history() {
    let mut fixture = fixture(true);
    complete(&mut fixture);
    ensure(&mut fixture.state.user, &fixture.reference, LATER).unwrap();
    let board = read_board(&fixture);
    let receipt = turn(&fixture).board_delivery.clone().unwrap();
    let path = fixture.state.user.session_store.as_ref().unwrap()
        .logs[&fixture.reference.session_id].path.clone();

    // Earlier releases saved formatted fallback answers even when their
    // request was incomplete. Reproduce that existing on-disk event sequence.
    let mut events: Vec<Value> = std::fs::read_to_string(&path).unwrap().lines()
        .map(|line| serde_json::from_str(line).unwrap()).collect();
    let finished = events.iter_mut().find(|event| event["kind"] == "turn.finished"
        && event["turn_id"] == fixture.reference.turn_id).unwrap();
    finished["payload"]["outcome"]["incomplete"] = json!(true);
    let bytes = events.iter().map(|event| serde_json::to_string(event).unwrap() + "\n")
        .collect::<String>();
    std::fs::write(&path, &bytes).unwrap();

    let (history, storage) = session_store::load_chat_storage(&fixture.state.user.history_path)
        .expect("an existing incomplete receipt must not prevent loading all user sessions");
    fixture.state.user.agent_history = history;
    fixture.state.user.session_store = storage;
    assert!(!deliverable(turn(&fixture)), "new incomplete answers remain in discussion");
    assert!(pending_error(turn(&fixture)).is_none(), "old receipts remain saved even when their answer was incomplete");
    assert_eq!(turn(&fixture).board_delivery.as_ref(), Some(&receipt));
    repair(&mut fixture.state.user, LATER).unwrap();
    assert_eq!(read_board(&fixture), board);
    assert_eq!(std::fs::read_to_string(path).unwrap(), bytes);
}

fn diagnostics(fallback_issues: Option<bool>) -> AnswerDeliveryDiagnostics {
    let failed = json!({"issues":[{
        "error_code":"ANSWER_DELIVERY_INVALID", "start":null, "end":null,
        "trigger_value":null, "match_form":"invalid_source", "source_channels":[],
    }]});
    serde_json::from_value(json!({
        "initial":failed, "repair":failed,
        "fallback":fallback_issues.map(|issues| if issues { failed.clone() } else { json!({"issues":[]}) }),
    })).unwrap()
}

#[test]
fn wb2_delivery_retains_complete_answer_sources_and_scene_then_reuses_same_receipt() {
    let mut fixture = fixture(false);
    let expected_parts = json!(fixture.outcome.answer_view.as_ref().unwrap().parts);
    let expected_sources = json!(note_api::bindings(&fixture.outcome.source_bindings));
    let expected_text = fixture
        .outcome
        .answer_view
        .as_ref()
        .unwrap()
        .parts
        .iter()
        .filter_map(|part| match part {
            AgentAnswerPart::Markdown { text } => Some(text.as_str()),
            AgentAnswerPart::MusicExample { example } => Some(example.label.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    let moved = call(
        &fixture.state,
        "update",
        json!({
            "board_id":fixture.selected.board_id, "board_revision":fixture.selected.board_revision,
            "positions":[{"item_id":fixture.selected.item_id,"position":{"x":920,"y":430}}],
        }),
    );
    complete(&mut fixture);

    // The board append can succeed before the chat receipt is acknowledged.
    let frozen = turn(&fixture).domain.board_context.clone().unwrap();
    let receipt = board_api::deliver_follow_up(
        &fixture.state.user,
        &frozen,
        &fixture.reference.session_id,
        &fixture.reference.turn_id,
        NOW,
    )
    .unwrap();
    assert!(turn(&fixture).board_delivery.is_none());
    let delivered = read_board(&fixture);
    assert_eq!(delivered.deliveries, vec![receipt.clone()]);
    assert_eq!(
        receipt.before_revision,
        moved["revision"].as_u64().unwrap() as u32
    );
    assert_eq!(receipt.after_revision, delivered.revision);
    assert_eq!(receipt.origin_session_id, fixture.reference.session_id);
    assert_eq!(receipt.origin_turn_id, fixture.reference.turn_id);
    assert_eq!(receipt.parent_item_id, fixture.selected.item_id);
    assert_eq!(receipt.item_ids.len(), 2);
    assert_eq!(receipt.block_ids.len(), 1);
    let answer = delivered
        .items
        .iter()
        .find(|item| matches!(item.target, BoardTarget::Answer { .. }))
        .unwrap();
    assert_eq!(answer.readable_content, expected_text);
    assert_eq!(json!(answer.source_bindings), expected_sources);
    assert_eq!(
        answer.reply_to_item_id.as_ref(),
        fixture.selected.item_id.as_ref()
    );
    assert_eq!(answer.block_id.as_ref(), receipt.block_ids.first());
    assert!(answer.position.x > 920.0);
    assert_eq!(answer.position.y, 430.0);
    let BoardTarget::Answer {
        answer_parts,
        session_id,
        turn_id,
        ..
    } = &answer.target
    else {
        unreachable!()
    };
    assert_eq!(json!(answer_parts), expected_parts);
    assert_eq!(session_id, &fixture.reference.session_id);
    assert_eq!(turn_id, &fixture.reference.turn_id);
    let scene = delivered
        .items
        .iter()
        .find(|item| matches!(item.target, BoardTarget::Presentation { .. }))
        .unwrap();
    let BoardTarget::Presentation {
        receipt: scene_receipt,
        ..
    } = &scene.target
    else {
        unreachable!()
    };
    assert_eq!(scene_receipt.session_id, fixture.reference.session_id);
    let scene_view = call(
        &fixture.state,
        "presentation.read",
        json!({
            "board_id":fixture.selected.board_id,"item_id":scene.item_id,
        }),
    );
    assert!(scene_view.to_string().contains("return_speed"));

    ensure(&mut fixture.state.user, &fixture.reference, LATER).unwrap();
    ensure(&mut fixture.state.user, &fixture.reference, LATER).unwrap();
    repair(&mut fixture.state.user, LATER).unwrap();
    assert_eq!(turn(&fixture).board_delivery.as_ref(), Some(&receipt));
    assert_eq!(read_board(&fixture), delivered);
    fixture.state.user.agent_history =
        load_agent_history(&fixture.state.user.history_path).unwrap();
    assert_eq!(turn(&fixture).board_delivery.as_ref(), Some(&receipt));
    assert_eq!(
        turn(&fixture).board_follow_up.as_ref(),
        Some(&fixture.selected)
    );
}

#[test]
fn wb2_delivery_ignores_pending_cancelled_failed_and_final_invalid_answers() {
    for case in [
        "pending",
        "cancelled",
        "failed",
        "repair_failed",
        "fallback_failed",
    ] {
        let mut fixture = fixture(false);
        let initial = read_board(&fixture);
        match case {
            "pending" => {}
            "cancelled" | "failed" => {
                let messages = fixture.state.workspace.messages.clone();
                finalize_agent_turn(
                    &mut fixture.state,
                    &fixture.reference,
                    if case == "cancelled" {
                        AgentAssistantStatus::Cancelled
                    } else {
                        AgentAssistantStatus::Failed
                    },
                    None,
                    Some(AgentTurnError {
                        error_code: case.to_uppercase(),
                        category: "runtime".into(),
                        message: case.into(),
                    }),
                    None,
                    &messages,
                    NOW,
                )
                .unwrap();
            }
            _ => {
                fixture.outcome.delivery_diagnostics =
                    Some(diagnostics((case == "fallback_failed").then_some(true)));
                complete(&mut fixture);
                assert!(turn(&fixture)
                    .outcome
                    .as_ref()
                    .unwrap()
                    .delivery_diagnostics
                    .is_none());
                assert!(turn(&fixture).delivery_diagnostics.is_some());
            }
        }
        ensure(&mut fixture.state.user, &fixture.reference, LATER).unwrap();
        repair(&mut fixture.state.user, LATER).unwrap();
        assert_eq!(read_board(&fixture), initial, "{case}");
        assert!(turn(&fixture).board_delivery.is_none(), "{case}");
        assert!(turn(&fixture).board_delivery_error.is_none(), "{case}");
    }
}

#[test]
fn wb2_delivery_accepts_successful_final_fallback_despite_prior_diagnostics() {
    let mut fixture = fixture(false);
    fixture.outcome.delivery_diagnostics = Some(diagnostics(Some(false)));
    complete(&mut fixture);
    ensure(&mut fixture.state.user, &fixture.reference, LATER).unwrap();
    assert!(turn(&fixture).board_delivery.is_some());
    assert_eq!(read_board(&fixture).deliveries.len(), 1);
}

#[test]
fn incomplete_fallback_stays_in_discussion_without_becoming_a_board_answer() {
    for logged in [false, true] {
        let mut fixture = fixture(logged);
        let original = read_board(&fixture);
        fixture.outcome.incomplete = true;
        fixture.outcome.warning = Some("PROVIDER_ERROR".into());
        fixture.outcome.delivery_diagnostics = Some(diagnostics(Some(false)));
        fixture.outcome.answer = Some("The request could not be completed.".into());
        fixture.outcome.answer_view = Some(AgentAnswerView {
            parts: vec![AgentAnswerPart::Markdown { text: fixture.outcome.answer.clone().unwrap() }],
            sources: vec![],
        });
        fixture.outcome.source_bindings.clear();
        complete(&mut fixture);
        let frozen = turn(&fixture).domain.board_context.clone().unwrap();
        assert_eq!(board_api::deliver_follow_up(&fixture.state.user, &frozen,
            &fixture.reference.session_id, &fixture.reference.turn_id, LATER).unwrap_err().error_code,
            "BOARD_REQUEST_INVALID");
        ensure(&mut fixture.state.user, &fixture.reference, LATER).unwrap();
        repair(&mut fixture.state.user, LATER).unwrap();
        assert_eq!(read_board(&fixture), original);
        assert!(turn(&fixture).board_delivery.is_none());
        assert!(turn(&fixture).outcome.as_ref().unwrap().incomplete);
        assert_eq!(turn(&fixture).outcome.as_ref().unwrap().answer.as_deref(), Some("The request could not be completed."));
    }
}

#[test]
fn wb2_delivery_keeps_answer_and_records_removed_target_without_recreating_it() {
    for remove_board in [false, true] {
        let mut fixture = fixture(false);
        let original = read_board(&fixture);
        if remove_board {
            call(
                &fixture.state,
                "delete",
                json!({
                    "board_id":original.board_id,"board_revision":original.revision,
                }),
            );
        } else {
            call(
                &fixture.state,
                "update",
                json!({
                    "board_id":original.board_id,"board_revision":original.revision,
                    "remove_item_ids":[fixture.selected.item_id],
                }),
            );
        }
        complete(&mut fixture);
        let answer = json!(turn(&fixture).outcome);
        ensure(&mut fixture.state.user, &fixture.reference, LATER).unwrap();
        ensure(&mut fixture.state.user, &fixture.reference, LATER).unwrap();
        fixture.state.user.agent_history =
            load_agent_history(&fixture.state.user.history_path).unwrap();
        assert_eq!(turn(&fixture).status, AgentAssistantStatus::Completed);
        assert_eq!(json!(turn(&fixture).outcome), answer);
        assert!(turn(&fixture).board_delivery.is_none());
        assert_eq!(
            turn(&fixture)
                .board_delivery_error
                .as_ref()
                .unwrap()
                .error_code,
            "BOARD_NOT_FOUND"
        );
        if remove_board {
            assert_eq!(
                board_api::route(
                    &fixture.state.user,
                    &fixture.state.workspace,
                    "read",
                    &json!({"board_id":original.board_id}),
                    NOW,
                )
                .unwrap_err()
                .error_code,
                "BOARD_NOT_FOUND"
            );
        } else {
            let board = read_board(&fixture);
            assert!(board.items.is_empty());
            assert!(board.deliveries.is_empty());
            assert_eq!(board.revision, original.revision + 1);
        }
    }
}

fn link_event(
    user: &user_runtime::UserRuntime,
    reference: &AgentTurnRef,
    receipt: BoardDelivery,
) -> session_event::SessionEvent {
    user.session_store.as_ref().unwrap().logs[&reference.session_id].next_event(
        LATER,
        Some(&reference.turn_id),
        session_event::EventBody::BoardDelivered(session_event::BoardDelivered {
            delivery: Some(receipt),
            error: None,
        }),
    )
}

#[test]
fn wb2_delivery_repairs_lost_log_ack_after_reopen_without_duplicate_cards() {
    for full_write in [false, true] {
        let mut fixture = fixture(true);
        let pending_receipt = BoardDelivery {
            commit_id: None,
            board_id: fixture.selected.board_id.clone(),
            before_revision: fixture.selected.board_revision,
            after_revision: fixture.selected.board_revision + 1,
            item_ids: vec!["future-answer".into()],
            block_ids: vec!["future-block".into()],
            origin_session_id: fixture.reference.session_id.clone(),
            origin_turn_id: fixture.reference.turn_id.clone(),
            parent_item_id: fixture.selected.item_id.clone(),
        };
        let event = link_event(&fixture.state.user, &fixture.reference, pending_receipt);
        assert!(
            fixture.state.user.session_store.as_ref().unwrap().logs[&fixture.reference.session_id]
                .projection
                .validate(&event)
                .is_err(),
            "a pending turn cannot link a board delivery"
        );
        complete(&mut fixture);
        fixture
            .state
            .user
            .session_store
            .as_mut()
            .unwrap()
            .logs
            .get_mut(&fixture.reference.session_id)
            .unwrap()
            .fail_write = Some(full_write);
        assert!(ensure(&mut fixture.state.user, &fixture.reference, LATER).is_err());
        let delivered = read_board(&fixture);
        assert_eq!(delivered.deliveries.len(), 1);
        assert_eq!(delivered.items.len(), 2);
        assert!(turn(&fixture).board_delivery.is_none());
        let receipt = delivered.deliveries[0].clone();
        let mut wrong = receipt.clone();
        wrong.board_id = "different-board".into();
        let event = link_event(&fixture.state.user, &fixture.reference, wrong);
        assert!(
            fixture.state.user.session_store.as_ref().unwrap().logs[&fixture.reference.session_id]
                .projection
                .validate(&event)
                .is_err(),
            "a receipt cannot retarget the accepted turn"
        );

        // Reopen drops in-memory pending writes. A complete line is recovered;
        // a torn line is repaired from the board's durable original receipt.
        let (history, storage) =
            session_store::load_chat_storage(&fixture.state.user.history_path).unwrap();
        fixture.state.user.agent_history = history;
        fixture.state.user.session_store = storage;
        repair(&mut fixture.state.user, LATER).unwrap();
        ensure(&mut fixture.state.user, &fixture.reference, LATER).unwrap();
        assert_eq!(turn(&fixture).board_delivery.as_ref(), Some(&receipt));
        assert_eq!(
            turn(&fixture).board_follow_up.as_ref(),
            Some(&fixture.selected)
        );
        assert_eq!(read_board(&fixture), delivered);
        let event = link_event(&fixture.state.user, &fixture.reference, receipt.clone());
        let log =
            &fixture.state.user.session_store.as_ref().unwrap().logs[&fixture.reference.session_id];
        assert!(
            log.projection.validate(&event).is_err(),
            "an already linked turn rejects another delivery event"
        );
        let persisted_links = std::fs::read_to_string(&log.path)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .filter(|event| event["kind"] == "board.delivered")
            .count();
        assert_eq!(persisted_links, 1);
        let (history, storage) =
            session_store::load_chat_storage(&fixture.state.user.history_path).unwrap();
        fixture.state.user.agent_history = history;
        fixture.state.user.session_store = storage;
        assert_eq!(turn(&fixture).board_delivery.as_ref(), Some(&receipt));
    }
}

#[test]
fn wb2_delivery_persists_temporary_board_failure_then_repairs_without_rerunning_answer() {
    for logged in [false, true] {
        let mut fixture = fixture(logged);
        complete(&mut fixture);
        let original_board = read_board(&fixture);
        let original_answer = json!(turn(&fixture).outcome);
        let original_messages = fixture
            .state
            .user
            .agent_history
            .sessions
            .iter()
            .find(|session| session.id == fixture.reference.session_id)
            .unwrap()
            .messages
            .clone();
        let original_turn_count = fixture
            .state
            .user
            .agent_history
            .sessions
            .iter()
            .find(|session| session.id == fixture.reference.session_id)
            .unwrap()
            .turns
            .len();

        // Use the same real head-commit failure as BoardStore's storage tests.
        // Only this fixture's empty blocker directory is created and removed;
        // the committed head and every retained revision remain untouched.
        let blocker = fixture
            ._root
            .path()
            .join("boards/unscoped")
            .join(&fixture.selected.board_id)
            .join("head.replace.tmp");
        assert!(blocker.starts_with(fixture._root.path()));
        std::fs::create_dir(&blocker).unwrap();
        let result = ensure_saved(&mut fixture.state.user, &fixture.reference, LATER);
        std::fs::remove_dir(&blocker).unwrap();
        assert_eq!(result.unwrap_err().error_code, "BOARD_DELIVERY_UNSAVED");
        assert_eq!(read_board(&fixture), original_board);
        assert!(turn(&fixture).board_delivery.is_none());
        let failure = turn(&fixture).board_delivery_error.clone().unwrap();
        assert_eq!(failure.error_code, "INTERNAL_ERROR");
        assert_eq!(turn(&fixture).status, AgentAssistantStatus::Completed);
        assert_eq!(json!(turn(&fixture).outcome), original_answer);

        if logged {
            let (history, storage) =
                session_store::load_chat_storage(&fixture.state.user.history_path).unwrap();
            fixture.state.user.agent_history = history;
            fixture.state.user.session_store = storage;
        } else {
            fixture.state.user.agent_history =
                load_agent_history(&fixture.state.user.history_path).unwrap();
        }
        assert_eq!(turn(&fixture).board_delivery_error.as_ref(), Some(&failure));
        if logged {
            repair(&mut fixture.state.user, LATER).unwrap();
        } else {
            ensure(&mut fixture.state.user, &fixture.reference, LATER).unwrap();
        }
        let receipt = turn(&fixture).board_delivery.clone().unwrap();
        assert!(turn(&fixture).board_delivery_error.is_none());
        assert!(pending_error(turn(&fixture)).is_none());
        let repaired_board = read_board(&fixture);
        assert_eq!(repaired_board.deliveries, vec![receipt.clone()]);
        assert_eq!(repaired_board.revision, original_board.revision + 1);
        assert_eq!(repaired_board.items.len(), if logged { 2 } else { 3 });
        repair(&mut fixture.state.user, LATER).unwrap();
        ensure(&mut fixture.state.user, &fixture.reference, LATER).unwrap();
        assert_eq!(read_board(&fixture), repaired_board);
        assert_eq!(json!(turn(&fixture).outcome), original_answer);
        let session = fixture
            .state
            .user
            .agent_history
            .sessions
            .iter()
            .find(|session| session.id == fixture.reference.session_id)
            .unwrap();
        assert_eq!(session.turns.len(), original_turn_count);
        assert_eq!(session.messages, original_messages);

        if logged {
            let (history, storage) =
                session_store::load_chat_storage(&fixture.state.user.history_path).unwrap();
            fixture.state.user.agent_history = history;
            fixture.state.user.session_store = storage;
        } else {
            fixture.state.user.agent_history =
                load_agent_history(&fixture.state.user.history_path).unwrap();
        }
        assert_eq!(turn(&fixture).board_delivery.as_ref(), Some(&receipt));
        assert!(turn(&fixture).board_delivery_error.is_none());
    }
}
