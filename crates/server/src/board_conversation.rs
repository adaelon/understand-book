//! A board's discussion is a projection of its original chats, not another history store.
use crate::{user_runtime::UserRuntime, ToolError};
use memory::boards::{BoardDocument, BoardTarget};
use serde_json::{json, Value};
use std::collections::BTreeSet;

fn superseded_candidates(user: &UserRuntime, document: &BoardDocument) -> BTreeSet<(String, String, String, u32)> {
    let mut resolved = BTreeSet::new();
    for session in &user.agent_history.sessions {
        if !session.matches_scope(document.learning_workspace_id.as_deref(), &document.book_id) { continue; }
        for turn in &session.turns {
            if turn.status != crate::AgentAssistantStatus::Completed { continue; }
            let Some(context) = &turn.domain.board_context else { continue; };
            if context.follow_up.board_id != document.board_id
                || context.learning_workspace_id != document.learning_workspace_id
                || context.book_id != document.book_id
                || turn.published_book_ref.as_ref().map(|reference| &reference.publication_id) != document.publication_id.as_ref() { continue; }
            let Some(reference) = &context.follow_up.regenerate_from else { continue; };
            let Some(candidate) = &turn.domain.board_authoring else { continue; };
            let delivered = turn.board_delivery.as_ref().is_some_and(|delivery| delivery.board_id == document.board_id)
                || turn.domain.board_commits.iter().any(|part| part.delivery.board_id == document.board_id);
            // A new conflict replaces the prior suggestion with its own card.
            // Otherwise only an actually delivered edit resolves the old card;
            // a plain reply, failed save or cancelled attempt leaves it pending.
            if candidate.conflict.is_none() && (!delivered || candidate.operations.is_empty()) { continue; }
            resolved.insert((reference.session_id.clone(), reference.turn_id.clone(), reference.candidate_id.clone(), reference.candidate_revision));
        }
    }
    resolved
}

pub(crate) fn read(user: &UserRuntime, document: &BoardDocument) -> Result<Value, ToolError> {
    let superseded = superseded_candidates(user, document);
    let imported: BTreeSet<_> = document
        .items
        .iter()
        .filter_map(|item| match &item.target {
            BoardTarget::Answer {
                session_id,
                turn_id,
                ..
            } => Some((session_id.as_str(), turn_id.as_str())),
            _ => None,
        })
        .collect();
    let mut sessions = Vec::new();
    for session in &user.agent_history.sessions {
        let same_scope =
            session.matches_scope(document.learning_workspace_id.as_deref(), &document.book_id);
        let mut turns = Vec::new();
        for turn in &session.turns {
            let native = same_scope
                && (document.learning_workspace_id.is_some() || turn
                    .published_book_ref
                    .as_ref()
                    .map(|reference| reference.publication_id.as_str())
                    == document.publication_id.as_deref())
                && turn.domain.board_context.as_ref().is_some_and(|context| {
                    context.follow_up.board_id == document.board_id
                        && context.learning_workspace_id == document.learning_workspace_id
                        && context.book_id == document.book_id
                });
            let retained;
            let original = if native {
                turn
            } else if imported.contains(&(session.id.as_str(), turn.turn_id.as_str())) {
                if user
                    .session_store
                    .as_ref()
                    .is_some_and(|store| store.logs.contains_key(&session.id))
                {
                    // An explicitly linked source keeps its accepted prefix, even
                    // if the source chat later completes or updates a response.
                    retained = match crate::workspace_record_context::permitted_turn(
                        user,
                        document.learning_workspace_id.as_deref(),
                        &document.book_id,
                        &session.id,
                        &turn.turn_id,
                    ) {
                        Ok(turn) => turn,
                        Err(error) if error.error_code == "OBJECT_NOT_FOUND" => continue,
                        Err(error) => return Err(error),
                    };
                    &retained
                } else if same_scope {
                    // Existing local snapshot histories predate the session log.
                    turn
                } else {
                    continue;
                }
            } else {
                continue;
            };
            let mut value = json!(original);
            value["board_commits"] = json!(crate::session_runtime::board_commits(original));
            if let Some(candidate) = original.domain.board_authoring.as_ref() {
                let confirmation = candidate.requires_confirmation && !candidate.confirmed && !candidate.dismissed && candidate.commit_selection.is_some();
                let replaced = superseded.contains(&(session.id.clone(), original.turn_id.clone(), candidate.candidate_id.clone(), candidate.candidate_revision));
                if original.status != crate::AgentAssistantStatus::Cancelled && !replaced && (candidate.conflict.is_some() || confirmation) {
                    let conflict = candidate.conflict.clone().unwrap_or(Value::Null);
                    value["pending_board_edit"] = json!({
                        "candidate_id": candidate.candidate_id,
                        "candidate_revision": candidate.candidate_revision,
                        "session_id": session.id,
                        "turn_id": original.turn_id,
                        "requires_confirmation": confirmation,
                        "current_board_revision": conflict["current_board_revision"],
                        "item_ids": conflict["item_ids"],
                        "message": conflict["message"],
                        "summary": candidate.delivery_text,
                    });
                }
            }
            turns.push(value);
        }
        if !turns.is_empty() {
            sessions.push((session, turns));
        }
    }
    // Stored turns already have their original order. Session creation order is
    // stable even when an older discussion was the most recently updated one.
    // Local, cloud and restored histories retain different timestamp formats.
    sessions.sort_by(|(left, _), (right, _)| {
        let left_time = crate::teaching::timeline::time_key(&left.created_at);
        let right_time = crate::teaching::timeline::time_key(&right.created_at);
        left_time
            .cmp(&right_time)
            .then_with(|| {
                if left_time.is_none() {
                    left.created_at.cmp(&right.created_at)
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .then_with(|| left.id.cmp(&right.id))
    });
    Ok(json!({
        "board_id": document.board_id,
        "sessions": sessions.into_iter().map(|(session, turns)| json!({
            "session_id": session.id,
            "title": session.title,
            "turns": turns
        })).collect::<Vec<_>>()
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{board_api, AgentAssistantStatus, AgentTurnError};

    const NOW: &str = "2026-10-26T08:00:00Z";

    #[test]
    fn regeneration_resolves_only_the_exact_pending_card_after_delivery_or_a_new_conflict() {
        let (_root, mut state, _) = crate::tests::board_browser_host::fixture();
        let board = board_api::route(&state.user, &state.workspace, "create", &json!({"title":"重新生成"}), NOW).unwrap();
        let document: BoardDocument = serde_json::from_value(board.clone()).unwrap();
        let follow_up = serde_json::from_value(json!({"board_id":document.board_id,"board_revision":document.revision})).unwrap();
        let frozen = board_api::freeze_follow_up(&state.user, &state.workspace, &follow_up).unwrap();
        let mut session = state.user.agent_history.sessions[0].clone();
        let mut original = session.turns[0].clone();
        original.turn_id = "original-conflict".into();
        original.status = AgentAssistantStatus::Completed;
        original.published_book_ref = None;
        original.board_follow_up = Some(follow_up);
        original.domain.board_context = Some(frozen);
        original.board_delivery = None;
        original.domain.board_commits.clear();
        original.domain.board_authoring = Some(serde_json::from_value(json!({
            "board_id":document.board_id,"base_board_revision":1,"candidate_revision":2,"candidate_id":"pending-exact",
            "items":[],"update_items":[],"operations":[],"links":[],"mappings":[],"routes":[],
            "delivery_text":"待处理的例子","answer_parts":[],"previewed":false,"staged":false,
            "conflict":{"current_board_revision":2,"item_ids":["topic"],"message":"人工修改已保留"}
        })).unwrap());
        let mut next = original.clone();
        next.turn_id = "regenerated".into();
        next.domain.board_context.as_mut().unwrap().follow_up.regenerate_from = Some(memory::boards::BoardRegenerationRef {
            session_id:session.id.clone(),turn_id:original.turn_id.clone(),candidate_id:"pending-exact".into(),candidate_revision:2,
        });
        next.board_follow_up = Some(next.domain.board_context.as_ref().unwrap().follow_up.clone());
        let candidate = next.domain.board_authoring.as_mut().unwrap();
        candidate.candidate_id = "new-candidate".into();
        candidate.conflict = None;
        candidate.operations = vec![memory::boards::BoardEditOperation::UpdateTopic { item_id:"topic".into(), title:Some("例子".into()), style:None }];
        let delivery = serde_json::from_value(json!({"board_id":document.board_id,"before_revision":2,"after_revision":3,
            "item_ids":["topic"],"block_ids":[],"origin_session_id":session.id,"origin_turn_id":next.turn_id})).unwrap();
        next.board_delivery = Some(delivery);
        session.turns = vec![original, next];
        state.user.agent_history.sessions = vec![session];
        for status in [AgentAssistantStatus::Failed, AgentAssistantStatus::Cancelled, AgentAssistantStatus::PendingAssistant] {
            state.user.agent_history.sessions[0].turns[1].status = status;
            assert!(read(&state.user,&document).unwrap()["sessions"][0]["turns"][0]["pending_board_edit"].is_object());
        }
        state.user.agent_history.sessions[0].turns[1].status = AgentAssistantStatus::Completed;
        assert!(read(&state.user,&document).unwrap()["sessions"][0]["turns"][0]["pending_board_edit"].is_null());
        state.user.agent_history.sessions[0].turns[1].domain.board_context.as_mut().unwrap().follow_up.regenerate_from.as_mut().unwrap().candidate_revision = 1;
        assert!(read(&state.user,&document).unwrap()["sessions"][0]["turns"][0]["pending_board_edit"].is_object());
        state.user.agent_history.sessions[0].turns[1].domain.board_context.as_mut().unwrap().follow_up.regenerate_from.as_mut().unwrap().candidate_revision = 2;
        state.user.agent_history.sessions[0].turns[1].board_delivery = None;
        assert!(read(&state.user,&document).unwrap()["sessions"][0]["turns"][0]["pending_board_edit"].is_object());
        state.user.agent_history.sessions[0].turns[1].domain.board_authoring.as_mut().unwrap().conflict = Some(json!({"message":"另一次人工修改"}));
        let repeated = read(&state.user,&document).unwrap();
        assert!(repeated["sessions"][0]["turns"][0]["pending_board_edit"].is_null());
        assert!(repeated["sessions"][0]["turns"][1]["pending_board_edit"].is_object());
    }

    #[test]
    fn discussion_orders_mixed_original_timestamps_before_current_chat() {
        let (_root, mut state, _) = crate::tests::board_browser_host::fixture();
        let board = board_api::route(
            &state.user,
            &state.workspace,
            "create",
            &json!({"title":"理解过程"}),
            NOW,
        )
        .unwrap();
        let document: BoardDocument = serde_json::from_value(board.clone()).unwrap();
        let follow_up = serde_json::from_value(json!({
            "board_id":board["board_id"], "board_revision":board["revision"]
        }))
        .unwrap();
        let frozen =
            board_api::freeze_follow_up(&state.user, &state.workspace, &follow_up).unwrap();
        let template = state.user.agent_history.sessions[0].clone();
        // Current cloud seconds, restored RFC3339 and local milliseconds can
        // coexist after importing a local Reader's unchanged session logs.
        let dates = [
            ("a-current", "1793001600"), // 2026-10-26 08:00 UTC
            ("c-local", "1792828800000"), // 2026-10-24 08:00 UTC
            ("d-original", "2026-10-23T16:00:00+08:00"),
            ("b-restored", "2026-10-25T08:00:00Z"),
        ];
        state.user.agent_history.sessions = dates
            .iter()
            .map(|(id, created_at)| {
                let mut session = template.clone();
                session.id = (*id).into();
                session.created_at = (*created_at).into();
                let mut turn = session.turns[0].clone();
                turn.turn_id = format!("{id}-turn");
                turn.board_follow_up = Some(follow_up.clone());
                turn.domain.board_context = Some(frozen.clone());
                session.turns = vec![turn];
                session
            })
            .collect();

        let result = read(&state.user, &document).unwrap();
        let ids: Vec<_> = result["sessions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|session| session["session_id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, ["d-original", "c-local", "b-restored", "a-current"]);
        for (session, (_, original_time)) in state.user.agent_history.sessions.iter().zip(dates) {
            assert_eq!(session.created_at, original_time);
        }
    }

    #[test]
    fn discussion_keeps_original_turns_statuses_and_imports_in_scope() {
        let (_root, mut state, metadata) = crate::tests::board_browser_host::fixture();
        let board = board_api::route(
            &state.user,
            &state.workspace,
            "create",
            &json!({"title":"速度讨论"}),
            NOW,
        )
        .unwrap();
        let board = board_api::route(
            &state.user,
            &state.workspace,
            "import",
            &json!({
                "board_id":board["board_id"], "board_revision":board["revision"],
                "source":metadata["sources"]["answer"]
            }),
            NOW,
        )
        .unwrap();
        let mut document: BoardDocument = serde_json::from_value(board.clone()).unwrap();
        let follow_up = serde_json::from_value(json!({
            "board_id":board["board_id"], "board_revision":board["revision"],
            "item_id":board["items"][0]["item_id"]
        }))
        .unwrap();
        let frozen =
            board_api::freeze_follow_up(&state.user, &state.workspace, &follow_up).unwrap();
        let original = state
            .user
            .agent_history
            .sessions
            .iter()
            .find(|session| session.id == metadata["session_id"].as_str().unwrap())
            .unwrap()
            .clone();
        let original_turn = original
            .turns
            .iter()
            .find(|turn| turn.turn_id == metadata["turn_id"].as_str().unwrap())
            .unwrap()
            .clone();

        let mut recent = original.clone();
        recent.id = "later-session".into();
        recent.title = "接着比较".into();
        recent.created_at = "2026-10-26T09:00:00Z".into();
        let mut failed = original_turn.clone();
        failed.turn_id = "failed-board-turn".into();
        failed.user = "停车时间也计入吗？".into();
        failed.user_turn_ordinal = 1;
        failed.status = AgentAssistantStatus::Failed;
        failed.outcome = None;
        failed.error = Some(AgentTurnError {
            error_code: "MODEL_FAILED".into(),
            category: "provider".into(),
            message: "稍后重试".into(),
        });
        failed.board_follow_up = Some(follow_up.clone());
        failed.domain.board_context = Some(frozen.clone());
        let mut pending = failed.clone();
        pending.turn_id = "pending-board-turn".into();
        pending.user = "把分母画出来".into();
        pending.user_turn_ordinal = 2;
        pending.status = AgentAssistantStatus::PendingAssistant;
        pending.error = None;
        let mut unrelated = original_turn.clone();
        unrelated.turn_id = "unrelated-turn".into();
        let mut other_board = pending.clone();
        other_board.turn_id = "other-board-turn".into();
        other_board
            .domain
            .board_context
            .as_mut()
            .unwrap()
            .follow_up
            .board_id = "another-board".into();
        let mut other_publication = pending.clone();
        other_publication.turn_id = "other-publication-turn".into();
        other_publication.published_book_ref = Some(
            serde_json::from_value(json!({
                "book_id":document.book_id, "publication_id":"different-publication"
            }))
            .unwrap(),
        );
        recent.turns = vec![failed, unrelated, pending, other_board, other_publication];
        let mut foreign = recent.clone();
        foreign.id = "foreign-session".into();
        foreign.learning_workspace_id = Some("other-learning-workspace".into());
        state.user.agent_history.sessions = vec![recent, foreign, original];

        // Repeated retained references must not duplicate the original turn.
        document.items.push(document.items[0].clone());
        let result = read(&state.user, &document).unwrap();
        assert_eq!(result["board_id"], document.board_id);
        let sessions = result["sessions"].as_array().unwrap();
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0]["session_id"], metadata["session_id"]);
        assert_eq!(sessions[0]["turns"].as_array().unwrap().len(), 1);
        assert_eq!(sessions[0]["turns"][0]["user"], original_turn.user);
        assert_eq!(
            sessions[0]["turns"][0]["outcome"],
            json!(original_turn.outcome)
        );
        assert_eq!(sessions[1]["title"], "接着比较");
        assert_eq!(sessions[1]["turns"].as_array().unwrap().len(), 2);
        assert_eq!(sessions[1]["turns"][0]["turn_id"], "failed-board-turn");
        assert_eq!(sessions[1]["turns"][0]["status"], "failed");
        assert_eq!(sessions[1]["turns"][0]["error"]["message"], "稍后重试");
        assert_eq!(sessions[1]["turns"][1]["turn_id"], "pending-board-turn");
        assert_eq!(sessions[1]["turns"][1]["status"], "pending_assistant");
        assert_eq!(sessions[1]["turns"][1]["board_commits"], json!([]));
    }

    #[test]
    fn imported_discussion_uses_linked_prefix_across_workspace_and_publication() {
        let (_root, mut state, metadata) = crate::tests::board_browser_host::fixture();
        let mut outcome = state
            .user
            .agent_history
            .sessions
            .iter()
            .find(|session| session.id == metadata["session_id"])
            .unwrap()
            .turns
            .iter()
            .find(|turn| turn.turn_id == metadata["turn_id"])
            .unwrap()
            .outcome
            .clone()
            .unwrap();
        outcome.answer = Some("原解释：把两段所用时间相加。".into());
        outcome.answer_view = None;
        outcome.source_bindings.clear();
        let board = board_api::route(
            &state.user,
            &state.workspace,
            "create",
            &json!({"title":"跨资料复习"}),
            NOW,
        )
        .unwrap();
        let (history, storage) =
            crate::session_store::load_chat_storage(&state.user.history_path).unwrap();
        state.user.agent_history = history;
        state.user.session_store = storage;
        assert_eq!(crate::route_agent_new(&mut state, NOW).status, 200);
        let book = state.workspace.book.base.book_id.clone();
        let source = crate::precommit_agent_turn(
            &mut state,
            &book,
            "为什么时间需要相加？".into(),
            None,
            None,
            None,
            NOW,
        )
        .unwrap();
        let messages = state.workspace.messages.clone();
        crate::finalize_agent_turn_completed(&mut state, &source, &outcome, &messages, NOW)
            .unwrap();
        let board = board_api::route(
            &state.user,
            &state.workspace,
            "import",
            &json!({
                "board_id":board["board_id"], "board_revision":board["revision"],
                "source":{"kind":"answer","session_id":source.session_id,"turn_id":source.turn_id}
            }),
            NOW,
        )
        .unwrap();
        let mut document: BoardDocument = serde_json::from_value(board).unwrap();
        document.learning_workspace_id = Some("linked-review-workspace".into());
        document.book_id = "another-material".into();
        document.publication_id = Some("another-publication".into());
        let through_seq = state.user.session_store.as_ref().unwrap().logs[&source.session_id]
            .projection
            .through_seq;
        state
            .user
            .learning_store_for_workspace(document.learning_workspace_id.as_deref())
            .unwrap()
            .link_records(
                &memory::learning_records::LearningRecordSelection {
                    source_learning_workspace_id: None,
                    chat_session_id: source.session_id.clone(),
                    through_seq,
                    turn_ids: vec![source.turn_id.clone()],
                },
                NOW,
            )
            .unwrap();
        let later = crate::precommit_agent_turn(
            &mut state,
            &book,
            "这是之后的新问题".into(),
            None,
            None,
            None,
            NOW,
        )
        .unwrap();
        crate::finalize_agent_turn_completed(&mut state, &later, &outcome, &messages, NOW).unwrap();
        // Current cached content must not replace the explicitly linked prefix.
        let session = state
            .user
            .agent_history
            .sessions
            .iter_mut()
            .find(|session| session.id == source.session_id)
            .unwrap();
        session
            .turns
            .iter_mut()
            .find(|turn| turn.turn_id == source.turn_id)
            .unwrap()
            .outcome
            .as_mut()
            .unwrap()
            .answer = Some("后来才出现的回答版本".into());
        let result = read(&state.user, &document).unwrap();
        assert_eq!(result["sessions"].as_array().unwrap().len(), 1);
        assert_eq!(result["sessions"][0]["session_id"], source.session_id);
        assert_eq!(result["sessions"][0]["turns"].as_array().unwrap().len(), 1);
        assert_eq!(result["sessions"][0]["turns"][0]["turn_id"], source.turn_id);
        assert_eq!(
            result["sessions"][0]["turns"][0]["outcome"]["answer"],
            "原解释：把两段所用时间相加。"
        );
        assert!(!result.to_string().contains("这是之后的新问题"));
        assert!(!result.to_string().contains("后来才出现的回答版本"));
        // Removing the original chat does not fabricate a replacement chat from
        // the board's retained answer snapshot.
        state.user.agent_history.sessions.clear();
        assert_eq!(read(&state.user, &document).unwrap()["sessions"], json!([]));
    }
}
