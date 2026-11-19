//! Semantic edits retain their generated content while the reader edits other branches.
use super::*;
use std::collections::BTreeSet;

pub(super) fn public_candidate(candidate: &Candidate) -> Value {
    let mut value = json!(candidate);
    value.as_object_mut().expect("candidate object").remove("edit_base");
    value
}

fn content_dependencies(value: &Value, ids: &mut BTreeSet<String>) {
    match value {
        Value::Object(object) => {
            if object.contains_key("content_revision") {
                if let Some(id) = object.get("item_id").and_then(Value::as_str) {
                    ids.insert(id.to_owned());
                }
            }
            for value in object.values() { content_dependencies(value, ids); }
        }
        Value::Array(values) => for value in values { content_dependencies(value, ids); },
        _ => {}
    }
}

pub(super) fn dependencies(board: &BoardDocument, candidate: &Candidate) -> Vec<String> {
    let mut ids = candidate.read_item_ids.iter().cloned().collect::<BTreeSet<_>>();
    ids.extend(candidate.update_items.iter().map(|item| item.item_id.clone()));
    content_dependencies(&json!({"items":candidate.items,"operations":candidate.operations,
        "links":candidate.links,"mappings":candidate.mappings,"routes":candidate.routes}), &mut ids);
    ids.retain(|id| board.items.iter().any(|item| &item.item_id == id));
    ids.into_iter().collect()
}

/// Operation IDs imply that the author used topic text, hierarchy and provenance.
/// Style is independent unless explicitly read or actually written by an operation.
fn implicit_context_conflict(base: &BoardDocument, latest: &BoardDocument, candidate: &Candidate) -> bool {
    let ids = candidate.operations.iter().flat_map(|operation| {
        let operation = json!(operation);
        ["item_id", "parent_item_id"].into_iter().filter_map(|field|operation[field].as_str().map(str::to_owned)).collect::<Vec<_>>()
    }).collect::<BTreeSet<_>>();
    ids.into_iter().any(|id| {
        let Some(before) = base.items.iter().find(|item|item.item_id == id) else { return false; };
        let Some(after) = latest.items.iter().find(|item|item.item_id == id) else { return true; };
        match (&before.target, &after.target) {
            (BoardTarget::Topic {children:before_children,based_on:before_sources,..}, BoardTarget::Topic {children:after_children,based_on:after_sources,..}) =>
                before.title != after.title || before_children != after_children || before_sources != after_sources || before.source_bindings != after.source_bindings,
            _ => true,
        }
    })
}

fn subtree(board: &BoardDocument, root: &str) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    let mut pending = vec![root.to_owned()];
    while let Some(id) = pending.pop() {
        if !ids.insert(id.clone()) { continue; }
        if let Some(BoardItem { target: BoardTarget::Topic { children, .. }, .. }) =
            board.items.iter().find(|item| item.item_id == id) {
            pending.extend(children.clone());
        }
    }
    ids
}

/// Selection is an edit boundary, not an invitation to rewrite the entire map.
pub(super) fn require_scope(board: &BoardDocument, projected: &BoardDocument,
    frozen: &board_api::FrozenBoardFollowUp, candidate: &Candidate) -> Result<(), ToolError> {
    if candidate.requires_confirmation && (!candidate.operations.iter().any(|operation|matches!(operation,BoardEditOperation::CreateMindMap { .. }))
        || board.items.iter().any(|old| projected.items.iter().find(|item|item.item_id==old.item_id)!=Some(old))
        || board.links.iter().any(|old| !projected.links.contains(old))
        || board.mappings.iter().any(|old| !projected.mappings.contains(old))
        || board.routes.iter().any(|old| !projected.routes.contains(old))) {
        return Err(runtime::board_author::error("BOARD_CONVERSION_PRESERVE_ORIGINAL", "A conversion creates a new editable mind map while preserving all original items, relationships, explanations and source identities."));
    }
    let Some(selected) = frozen.selected_item.as_ref().filter(|item| matches!(item.target, BoardTarget::Topic { .. })) else { return Ok(()); };
    let mut allowed = subtree(board, &selected.item_id);
    allowed.extend(projected.items.iter().filter(|item| !board.items.iter().any(|old| old.item_id == item.item_id))
        .map(|item| item.item_id.clone()));
    let within = |id: &str| allowed.contains(id);
    let fail = || runtime::board_author::error("BOARD_EDIT_OUTSIDE_SELECTION", "Only the selected topic and its descendants may change. Select the whole map to reorganize other branches.");
    let target_within = |target: &BoardVisualTarget| match target {
        BoardVisualTarget::Item { item_id } | BoardVisualTarget::PresentationAnchor { item_id, .. } => within(item_id),
        BoardVisualTarget::TopicEdge { child_item_id } => within(child_item_id),
        BoardVisualTarget::Link { link_id } => projected.links.iter().find(|link| &link.link_id == link_id)
            .is_some_and(|link| match (&link.from, &link.to) {
                (BoardVisualTarget::Item { item_id: left }, BoardVisualTarget::Item { item_id: right }) => within(left) && within(right),
                _ => false,
            }),
    };
    let route_within = |route: &BoardRoute| route.steps.iter().all(|step|
        step.focus_targets.iter().chain(&step.reveal_targets).all(&target_within));
    for operation in &candidate.operations {
        let valid = match operation {
            BoardEditOperation::CreateMindMap { .. } => false,
            BoardEditOperation::InsertTopic { parent_item_id, .. } => within(parent_item_id),
            BoardEditOperation::UpdateTopic { item_id, .. } | BoardEditOperation::RemoveSubtree { item_id } => within(item_id),
            BoardEditOperation::MoveSubtree { item_id, parent_item_id, .. }
            | BoardEditOperation::CopySubtree { item_id, parent_item_id, .. } => within(item_id) && within(parent_item_id),
            BoardEditOperation::SetMindMap { map_id, .. } => board.mind_maps.iter().find(|map| &map.map_id == map_id).is_some_and(|map| within(&map.root_item_id)),
            BoardEditOperation::PutLink { link } => target_within(&link.from) && target_within(&link.to),
            BoardEditOperation::RemoveLink { link_id } => board.links.iter().find(|link| &link.link_id == link_id).is_some_and(|link| target_within(&link.from) && target_within(&link.to)),
            BoardEditOperation::PutMapping { mapping } => target_within(&mapping.visual),
            BoardEditOperation::RemoveMapping { mapping_id } => board.mappings.iter().find(|mapping| &mapping.mapping_id == mapping_id).is_some_and(|mapping| target_within(&mapping.visual)),
            BoardEditOperation::PutRoute { route } => route_within(route),
            BoardEditOperation::RemoveRoute { route_id } => board.routes.iter().find(|route| &route.route_id == route_id).is_some_and(route_within),
        };
        if !valid { return Err(fail()); }
    }
    if candidate.update_items.iter().any(|item| !within(&item.item_id)) { return Err(fail()); }
    if candidate.links.iter().any(|link| !target_within(&link.from) || !target_within(&link.to))
        || candidate.mappings.iter().any(|mapping| !target_within(&mapping.visual))
        || candidate.routes.iter().any(|route| !route_within(route)) { return Err(fail()); }
    Ok(())
}

fn clear_observation(candidate: &mut Candidate) {
    candidate.previewed = false;
    candidate.staged = false;
    candidate.commit_selection = None;
    candidate.commit_requested_at = None;
}

fn remap_local_revisions(value: &mut Value, ids: &BTreeSet<String>, before: u32, after: u32) {
    match value {
        Value::Object(object) => {
            if object.get("item_id").and_then(Value::as_str).is_some_and(|id| ids.contains(id))
                && object.get("content_revision").and_then(Value::as_u64) == Some(u64::from(before)) {
                object.insert("content_revision".into(), json!(after));
            }
            for value in object.values_mut() { remap_local_revisions(value, ids, before, after); }
        }
        Value::Array(values) => for value in values { remap_local_revisions(value, ids, before, after); },
        _ => {}
    }
}

/// Called before read/preview only. Saving never rebases an already observed candidate.
pub(super) fn refresh(user: &mut user_runtime::UserRuntime, frozen: &board_api::FrozenBoardFollowUp,
    reference: &AgentTurnRef, store: &BoardStore, board: &BoardDocument, now: &str) -> Result<bool, ToolError> {
    let active = turn(user, reference)?;
    let Some(mut candidate) = active.domain.board_authoring.clone().filter(|candidate|
        !candidate.operations.is_empty()
        && !active.domain.board_commits.iter().any(|part| part.candidate_revision == candidate.candidate_revision)) else { return Ok(false); };
    let Some(base) = candidate.edit_base.as_ref() else { return Ok(false); };
    // Match the save order: explanatory items/relationships exist before the
    // operation batch. In particular pruning cannot discard a new mapping only
    // because its new answer has not been projected yet.
    let mut comparison_base = base.clone();
    let mut comparison_latest = board.clone();
    for document in [&mut comparison_base, &mut comparison_latest] {
        document.items.extend(candidate.items.clone());
        document.links.extend(candidate.links.clone());
        document.mappings.extend(candidate.mappings.clone());
        document.routes.extend(candidate.routes.clone());
    }
    let identity_appeared = candidate.items.iter().any(|new|board.items.iter().any(|item|item.item_id==new.item_id))
        || candidate.links.iter().any(|new|board.links.iter().any(|link|link.link_id==new.link_id))
        || candidate.mappings.iter().any(|new|board.mappings.iter().any(|mapping|mapping.mapping_id==new.mapping_id))
        || candidate.routes.iter().any(|new|board.routes.iter().any(|route|route.route_id==new.route_id));
    let conflict = candidate.note_revisions != store.note_revisions(board, &candidate.read_item_ids)?
        || identity_appeared
        || implicit_context_conflict(base, board, &candidate)
        || board_edits_conflict(&comparison_base, &comparison_latest, &candidate.operations, &candidate.read_item_ids)?
        || candidate.update_items.iter().any(|update| base.items.iter().find(|item| item.item_id == update.item_id)
            != board.items.iter().find(|item| item.item_id == update.item_id));
    if conflict {
        let mut touched = candidate.read_item_ids.iter().cloned().collect::<BTreeSet<_>>();
        for operation in &candidate.operations {
            let operation = json!(operation);
            for field in ["item_id", "parent_item_id"] {
                if let Some(id) = operation[field].as_str() { touched.insert(id.into()); }
            }
        }
        let pending = json!({"current_board_revision":board.revision,"item_ids":touched,
            "message":"你正在修改的主题或这次改图依赖的内容已改变。已保留你的内容和 AI 建议，请按当前内容重新生成。"});
        if candidate.conflict.as_ref() == Some(&pending) { return Ok(false); }
        clear_observation(&mut candidate);
        candidate.conflict = Some(pending);
        session_runtime::update_board_authoring(user, reference, now, candidate)?;
        return Ok(true);
    }
    if candidate.base_board_revision == board.revision { return Ok(false); }
    let old_projected = project(base, &candidate)?;
    let created_ids = old_projected.items.iter().filter(|item| base.items.iter().find(|old| old.item_id == item.item_id) != Some(*item))
        .map(|item| item.item_id.clone()).collect::<BTreeSet<_>>();
    let before = candidate.base_board_revision + 1;
    let mut content = json!({"items":candidate.items,"update_items":candidate.update_items,"operations":candidate.operations,
        "links":candidate.links,"mappings":candidate.mappings,"routes":candidate.routes});
    remap_local_revisions(&mut content, &created_ids, before, board.revision + 1);
    candidate.items = serde_json::from_value(content["items"].take()).map_err(|e| error(e.to_string()))?;
    candidate.update_items = serde_json::from_value(content["update_items"].take()).map_err(|e| error(e.to_string()))?;
    candidate.operations = serde_json::from_value(content["operations"].take()).map_err(|e| error(e.to_string()))?;
    candidate.links = serde_json::from_value(content["links"].take()).map_err(|e| error(e.to_string()))?;
    candidate.mappings = serde_json::from_value(content["mappings"].take()).map_err(|e| error(e.to_string()))?;
    candidate.routes = serde_json::from_value(content["routes"].take()).map_err(|e| error(e.to_string()))?;
    candidate.base_board_revision = board.revision;
    candidate.candidate_revision += 1;
    candidate.candidate_id = format!("board:{}:{}", reference.turn_id, candidate.candidate_revision);
    candidate.edit_base = Some(board.clone());
    candidate.conflict = None;
    clear_observation(&mut candidate);
    let projected = project(board, &candidate)?;
    require_scope(board, &projected, frozen, &candidate)?;
    store.validate_candidate(&projected)?;
    require_declared_anchors(user, &projected)?;
    session_runtime::update_board_authoring(user, reference, now, candidate)?;
    Ok(true)
}

pub(super) fn preview_ids(board: &BoardDocument, candidate: &Candidate) -> Vec<String> {
    let base = candidate.edit_base.as_ref();
    let mut ids = board.items.iter().filter(|item| candidate.items.iter().chain(&candidate.update_items).any(|changed| changed.item_id == item.item_id)
        || (!candidate.operations.is_empty() && base.is_some_and(|base| base.items.iter().find(|old| old.item_id == item.item_id) != Some(*item))))
        .map(|item| item.item_id.clone()).collect::<BTreeSet<_>>();
    for operation in &candidate.operations {
        match operation {
            BoardEditOperation::MoveSubtree { item_id, .. } | BoardEditOperation::UpdateTopic { item_id, .. } => ids.extend(subtree(board,item_id)),
            BoardEditOperation::SetMindMap { map_id, .. } => {
                if let Some(map) = board.mind_maps.iter().find(|map| &map.map_id == map_id) { ids.extend(subtree(board,&map.root_item_id)); }
            }
            _ => {}
        }
    }
    ids.into_iter().filter(|id|board.items.iter().any(|item| &item.item_id==id)).collect()
}
