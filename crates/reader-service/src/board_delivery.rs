//! Finish a board follow-up against its accepted target, independently of the UI.
use crate::*;

/// Current policy for adding a new answer to a board. Persisted receipts are
/// historical facts and use has_completed_answer when their log is replayed.
pub(crate) fn deliverable(turn: &AgentChatTurn) -> bool {
    turn.outcome.as_ref().is_some_and(|outcome| !outcome.incomplete)
        && has_completed_answer(turn)
}

/// Delivery diagnostics move out of OuterOutcome when the terminal turn commits.
/// Earlier releases also recorded receipts for formatted incomplete answers;
/// those records must remain readable when the policy for new answers changes.
pub(crate) fn has_completed_answer(turn: &AgentChatTurn) -> bool {
    if turn.status != AgentAssistantStatus::Completed
        || turn.delivery_diagnostics.as_ref()
            .and_then(|diagnostics| diagnostics.fallback.as_ref().or(diagnostics.repair.as_ref()))
            .is_some_and(|attempt| !attempt.issues.is_empty())
    {
        return false;
    }
    let Some(outcome) = turn.outcome.as_ref() else { return false; };
    if observability::lifecycle::delivery_failed(outcome) { return false; }
    outcome.answer_view.as_ref().map_or_else(
        || outcome.answer.as_ref().is_some_and(|answer| !answer.trim().is_empty()),
        |view| view.parts.iter().any(|part| match part {
            AgentAnswerPart::Markdown { text } | AgentAnswerPart::SocraticQuestion { text, .. } => !text.trim().is_empty(),
            AgentAnswerPart::MusicExample { example } => !example.label.is_empty(),
            AgentAnswerPart::Presentation { presentation_id, .. } => !presentation_id.is_empty(),
            AgentAnswerPart::Sources { source_ref_ids } => !source_ref_ids.is_empty(),
        }),
    )
}

fn unavailable(error_code: &str) -> bool {
    matches!(error_code,
        "BOARD_NOT_FOUND" | "OBJECT_NOT_FOUND" | "BOARD_REVISION_CONFLICT"
        | "PRESENTATION_NOT_FOUND" | "INVALID_BOARD" | "BOARD_REQUEST_INVALID")
}

/// The answer may be durable before its board receipt is. Keep the model's
/// terminal result for save retry, but do not publish that as a saved run yet.
pub(crate) fn pending_error(turn: &AgentChatTurn) -> Option<ToolError> {
    if turn.domain.board_context.is_none()
        || turn.domain.board_authoring.as_ref().is_some_and(|candidate| candidate.requires_confirmation || candidate.conflict.is_some())
        || !deliverable(turn)
        || turn.board_delivery.is_some()
        || !turn.domain.board_commits.is_empty()
    {
        return None;
    }
    let message = turn.board_delivery_error.as_ref().map_or_else(
        || "画板交付尚未保存，请重试保存。".to_owned(),
        |error| format!("画板交付尚未保存，请重试保存。{}: {}", error.error_code, error.message),
    );
    Some(user_storage_paths::error("BOARD_DELIVERY_UNSAVED", "storage", &message))
}

pub fn ensure_saved(
    user: &mut user_runtime::UserRuntime,
    reference: &AgentTurnRef,
    now: &str,
) -> Result<(), ToolError> {
    ensure(user, reference, now)?;
    let turn = user.agent_history.sessions.iter()
        .find(|session| session.id == reference.session_id)
        .and_then(|session| session.turns.iter().find(|turn| turn.turn_id == reference.turn_id))
        .ok_or_else(|| agent_history_internal("original board follow-up turn missing"))?;
    pending_error(turn).map_or(Ok(()), Err)
}

pub(crate) fn ensure(
    user: &mut user_runtime::UserRuntime,
    reference: &AgentTurnRef,
    now: &str,
) -> Result<(), ToolError> {
    // A prior append may already be durable while its acknowledgment was lost.
    // Settle it before deciding whether another board or session write is needed.
    if let Some(store) = user.session_store.as_mut() {
        store.settle(&mut user.agent_history, &reference.session_id)?;
    }
    // A published local part is independent of the final answer's fate. Recover
    // its confirmation and teaching support even when this run was cancelled.
    // The final chat response points to these parts; it is not appended again.
    if !board_author::repair_commits(user, reference, now)?.is_empty() {
        return Ok(());
    }
    let turn = user.agent_history.sessions.iter()
        .find(|session| session.id == reference.session_id)
        .and_then(|session| session.turns.iter().find(|turn| turn.turn_id == reference.turn_id))
        .ok_or_else(|| agent_history_internal("original board follow-up turn missing"))?;
    if turn.board_delivery.is_some()
        || turn.domain.board_authoring.as_ref().is_some_and(|candidate| candidate.requires_confirmation || candidate.conflict.is_some())
        || turn.board_delivery_error.as_ref().is_some_and(|error| unavailable(&error.error_code))
        || !deliverable(turn)
    {
        return Ok(());
    }
    let Some(frozen) = turn.domain.board_context.clone() else { return Ok(()); };
    let link = match board_api::deliver_follow_up(
        user, &frozen, &reference.session_id, &reference.turn_id, now,
    ) {
        Ok(delivery) => session_event::BoardDelivered { delivery: Some(delivery), error: None },
        Err(error) => session_event::BoardDelivered {
            delivery: None,
            error: Some(AgentTurnError {
                error_code: error.error_code, category: error.category, message: error.message,
            }),
        },
    };
    // BoardStore returns the original receipt for this session and turn on retry.
    // A failed link can consequently be repaired without duplicating any cards.
    session_runtime::link_board_delivery(user, reference, now, link)
}

pub(crate) fn repair(user: &mut user_runtime::UserRuntime, now: &str) -> Result<(), ToolError> {
    let pending: Vec<_> = user.agent_history.sessions.iter().flat_map(|session| {
        session.turns.iter().filter(|turn| turn.domain.board_context.is_some()
            && (!turn.domain.board_commits.is_empty()
                || (turn.board_delivery.is_none()
                    && turn.board_delivery_error.as_ref().is_none_or(|error| !unavailable(&error.error_code)))))
            .map(|turn| AgentTurnRef {
                session_id: session.id.clone(), turn_id: turn.turn_id.clone(),
                user_turn_ordinal: turn.user_turn_ordinal,
            })
    }).collect();
    for reference in pending { ensure(user, &reference, now)?; }
    Ok(())
}

#[cfg(test)]
mod tests;
