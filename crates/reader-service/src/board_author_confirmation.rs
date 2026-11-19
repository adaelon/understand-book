//! Explicit old-diagram conversion is saved only after the reader accepts its preview.
use super::*;

pub(crate) fn confirmation_update(old: Option<&Candidate>, next: &Candidate) -> bool {
    let Some(old) = old.filter(|old| old.requires_confirmation && !old.confirmed && !old.dismissed) else { return false; };
    if next.confirmed == next.dismissed { return false; }
    let mut allowed = old.clone();
    allowed.confirmed = next.confirmed;
    allowed.dismissed = next.dismissed;
    json!(allowed) == json!(next)
}

fn target(user: &user_runtime::UserRuntime, workspace: &reader_workspace::ReaderWorkspace, input: &Value)
    -> Result<(AgentTurnRef, board_api::FrozenBoardFollowUp, Candidate, BoardStore, BoardDocument), ToolError> {
    let required = |key: &str| input[key].as_str().filter(|value|!value.is_empty())
        .ok_or_else(||error(format!("A conversion requires {key}")));
    let session_id = required("session_id")?;
    let turn_id = required("turn_id")?;
    let saved = crate::workspace_record_context::permitted_turn(user, workspace.learning_workspace_id.as_deref(),
        &workspace.book.base.book_id, session_id, turn_id)?;
    let frozen = saved.domain.board_context.clone().ok_or_else(runtime::board_author::unavailable)?;
    if frozen.learning_workspace_id != workspace.learning_workspace_id || frozen.book_id != workspace.book.base.book_id
        || frozen.publication.as_ref() != workspace.publication.as_ref().map(|publication| &publication.reference)
        || frozen.follow_up.board_id != required("board_id")? { return Err(service_error::missing()); }
    let candidate = saved.domain.board_authoring.filter(|candidate|candidate.requires_confirmation
        && candidate.candidate_id == input["candidate_id"].as_str().unwrap_or_default()
        && Some(u64::from(candidate.candidate_revision)) == input["candidate_revision"].as_u64())
        .ok_or_else(||runtime::board_author::error("BOARD_CANDIDATE_CHANGED", "The conversion preview was replaced. Open its current preview before accepting."))?;
    let reference = AgentTurnRef {session_id:session_id.into(),turn_id:turn_id.into(),user_turn_ordinal:saved.user_turn_ordinal};
    let store = board_api::scoped_store(user,frozen.learning_workspace_id.as_deref())?;
    let board = store.read(&frozen.follow_up.board_id)?;
    if board.book_id != frozen.book_id || board.learning_workspace_id != frozen.learning_workspace_id
        || board.publication_id.as_ref() != frozen.publication.as_ref().map(|publication|&publication.publication_id) {
        return Err(service_error::missing());
    }
    if saved.status == AgentAssistantStatus::Cancelled
        && !board.deliveries.iter().any(|delivery|delivery.commit_id.as_deref()==Some(candidate.candidate_id.as_str())) {
        return Err(runtime::board_author::error("BOARD_CANDIDATE_CANCELLED", "This uncommitted conversion was cancelled. Start a new conversion from the current board."));
    }
    Ok((reference, frozen, candidate, store, board))
}

pub(crate) fn candidate_preview(user: &user_runtime::UserRuntime, workspace: &reader_workspace::ReaderWorkspace, input: &Value) -> Result<Value, ToolError> {
    let (_, frozen, candidate, store, board) = target(user, workspace, input)?;
    if candidate.dismissed { return Err(error("This conversion preview was dismissed")); }
    if !candidate.previewed || candidate.commit_selection.is_none() {
        return Err(runtime::board_author::error("BOARD_PREVIEW_REQUIRED", "The conversion is still being prepared. Wait for its completed preview and teaching review."));
    }
    let projected = validate_current(&store, &board, &candidate)?;
    edits::require_scope(&board, &projected, &frozen, &candidate)?;
    Ok(json!({"candidate_id":candidate.candidate_id,"candidate_revision":candidate.candidate_revision,
        "base_board_revision":candidate.base_board_revision,"document":projected,
        "candidate_item_ids":edits::preview_ids(&projected,&candidate),"explanation":candidate.delivery_text}))
}

pub(crate) fn candidate_confirmation(user: &mut user_runtime::UserRuntime, workspace: &reader_workspace::ReaderWorkspace,
    input: &Value, accept: bool, now: &str) -> Result<Value, ToolError> {
    let (reference, frozen, mut candidate, store, board) = target(user, workspace, input)?;
    if let Some(delivery) = board.deliveries.iter().find(|delivery|delivery.commit_id.as_deref()==Some(candidate.candidate_id.as_str())) {
        let delivery = delivery.clone();
        repair_commits(user, &reference, now)?;
        return Ok(json!({"status":"board_part_committed","delivery":delivery,"document":store.read(&board.board_id)?,"already_committed":true}));
    }
    if candidate.dismissed { return Ok(json!({"status":"candidate_dismissed","candidate_id":candidate.candidate_id})); }
    if !accept {
        if candidate.confirmed { return Err(error("This conversion was already accepted; undo its saved board transaction instead")); }
        candidate.dismissed = true;
        session_runtime::update_board_authoring(user, &reference, now, candidate.clone())?;
        return Ok(json!({"status":"candidate_dismissed","candidate_id":candidate.candidate_id}));
    }
    if !candidate.previewed || candidate.commit_selection.is_none() {
        return Err(runtime::board_author::error("BOARD_PREVIEW_REQUIRED", "Accept only the completed conversion preview"));
    }
    let projected = validate_current(&store, &board, &candidate)?;
    edits::require_scope(&board, &projected, &frozen, &candidate)?;
    if !candidate.confirmed {
        candidate.confirmed = true;
        session_runtime::update_board_authoring(user, &reference, now, candidate.clone())?;
    }
    let delivery = store.append_authored_part_with_edits(&frozen.follow_up, candidate.base_board_revision,
        &reference.session_id, &reference.turn_id, &candidate.candidate_id, candidate.items.clone(), candidate.update_items.clone(),
        candidate.links.clone(), candidate.mappings.clone(), candidate.routes.clone(), candidate.operations.clone(), now)?;
    repair_commits(user, &reference, now)?;
    Ok(json!({"status":"board_part_committed","delivery":delivery,"document":store.read(&board.board_id)?,"already_committed":false}))
}
