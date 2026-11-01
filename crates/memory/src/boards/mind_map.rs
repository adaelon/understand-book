//! Semantic topic editing shared by the manual editor and authored candidates.
//! Geometry is derived by the Reader's layout module; it is not tree structure.
use super::*;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct BoardTopicStyle {
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub bold: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub enum BoardMindMapLayout {
    #[default]
    Balanced,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub enum BoardTopicSide {
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct BoardMindMap {
    pub map_id: String,
    pub root_item_id: String,
    #[serde(default)]
    pub layout: BoardMindMapLayout,
    /// Top-left of the root topic, in the existing board's world coordinates.
    pub anchor: BoardPosition,
    #[serde(default)]
    pub branch_sides: BTreeMap<String, BoardTopicSide>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct BoardTopicInput {
    pub item_id: String,
    pub title: String,
    #[serde(default)]
    pub style: BoardTopicStyle,
    #[serde(default)]
    pub based_on: Vec<BoardContentRef>,
    #[serde(default)]
    pub source_bindings: Vec<NoteSourceBinding>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub enum BoardEditOperation {
    CreateMindMap {
        map: BoardMindMap,
        root: BoardTopicInput,
    },
    InsertTopic {
        parent_item_id: String,
        index: usize,
        topic: BoardTopicInput,
    },
    UpdateTopic {
        item_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        title: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        style: Option<BoardTopicStyle>,
    },
    MoveSubtree {
        item_id: String,
        parent_item_id: String,
        index: usize,
    },
    CopySubtree {
        item_id: String,
        parent_item_id: String,
        index: usize,
        id_prefix: String,
    },
    RemoveSubtree {
        item_id: String,
    },
    SetMindMap {
        map_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        layout: Option<BoardMindMapLayout>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        anchor: Option<BoardPosition>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        branch_sides: Option<BTreeMap<String, BoardTopicSide>>,
    },
    PutLink {
        link: BoardLink,
    },
    RemoveLink {
        link_id: String,
    },
    PutMapping {
        mapping: BoardExplanationMapping,
    },
    RemoveMapping {
        mapping_id: String,
    },
    PutRoute {
        route: BoardRoute,
    },
    RemoveRoute {
        route_id: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub enum BoardEditActor {
    Human,
    Ai,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct BoardEditTransaction {
    pub revision: u32,
    pub before_revision: u32,
    pub actor: BoardEditActor,
    pub action: String,
    pub undo_of: Option<u32>,
    pub redo_of: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct BoardEditHistory {
    pub transactions: Vec<BoardEditTransaction>,
    pub undo_stack: Vec<u32>,
    pub redo_stack: Vec<u32>,
}

/// Project one atomic batch without persistence or alteration of its base.
/// References to new explanations are checked by `BoardStore::validate_candidate`
/// after the caller has also appended its authored explanation blocks.
pub fn apply_board_edits(
    base: &BoardDocument,
    operations: &[BoardEditOperation],
    now: &str,
) -> Result<BoardDocument, ToolError> {
    let mut document = base.clone();
    apply_operations(&mut document, operations, now)?;
    document.revision = base
        .revision
        .checked_add(1)
        .ok_or_else(|| invalid("Board revision limit reached"))?;
    document.updated_at = now.into();
    Ok(document)
}

fn topic_item(input: &BoardTopicInput, now: &str) -> BoardItem {
    BoardItem {
        item_id: input.item_id.clone(),
        title: input.title.clone(),
        reply_to_item_id: None,
        block_id: None,
        explanation_blocks: vec![],
        readable_content: String::new(),
        position: BoardPosition { x: 0.0, y: 0.0 },
        size: BoardSize {
            width: 160.0,
            height: 48.0,
        },
        created_at: now.into(),
        source_bindings: input.source_bindings.clone(),
        target: BoardTarget::Topic {
            children: vec![],
            style: input.style.clone(),
            based_on: input.based_on.clone(),
        },
    }
}

fn children(document: &BoardDocument, item_id: &str) -> Result<Vec<String>, ToolError> {
    document
        .items
        .iter()
        .find_map(|item| {
            if item.item_id != item_id {
                return None;
            }
            match &item.target {
                BoardTarget::Topic { children, .. } => Some(children.clone()),
                _ => None,
            }
        })
        .ok_or_else(|| not_found("The selected topic is unavailable"))
}

fn children_mut<'a>(
    document: &'a mut BoardDocument,
    item_id: &str,
) -> Result<&'a mut Vec<String>, ToolError> {
    document
        .items
        .iter_mut()
        .find_map(|item| {
            if item.item_id != item_id {
                return None;
            }
            match &mut item.target {
                BoardTarget::Topic { children, .. } => Some(children),
                _ => None,
            }
        })
        .ok_or_else(|| not_found("The selected parent topic is unavailable"))
}

pub fn board_topic_parent<'a>(document: &'a BoardDocument, child_item_id: &str) -> Option<&'a str> {
    document.items.iter().find_map(|item| match &item.target {
        BoardTarget::Topic { children, .. } if children.iter().any(|id| id == child_item_id) => {
            Some(item.item_id.as_str())
        }
        _ => None,
    })
}

pub fn board_topic_subtree(
    document: &BoardDocument,
    item_id: &str,
) -> Result<Vec<String>, ToolError> {
    let topics: BTreeMap<_, _> = document
        .items
        .iter()
        .filter_map(|item| match &item.target {
            BoardTarget::Topic { children, .. } => Some((item.item_id.as_str(), children)),
            _ => None,
        })
        .collect();
    let mut pending = vec![item_id.to_string()];
    let mut result = Vec::new();
    let mut seen = BTreeSet::new();
    while let Some(id) = pending.pop() {
        if !seen.insert(id.clone()) {
            return Err(invalid(
                "A topic tree cannot contain a cycle or repeated child",
            ));
        }
        let descendants = topics
            .get(id.as_str())
            .ok_or_else(|| not_found("The selected topic is unavailable"))?;
        pending.extend(descendants.iter().rev().cloned());
        result.push(id);
    }
    Ok(result)
}

fn insert_child(
    document: &mut BoardDocument,
    parent: &str,
    index: usize,
    id: String,
) -> Result<(), ToolError> {
    let children = children_mut(document, parent)?;
    if index > children.len() {
        return Err(invalid(
            "The topic insertion position is outside its parent",
        ));
    }
    children.insert(index, id);
    Ok(())
}

fn fresh_topic(document: &BoardDocument, input: &BoardTopicInput) -> Result<(), ToolError> {
    validate_id(&input.item_id)?;
    if document
        .items
        .iter()
        .any(|item| item.item_id == input.item_id)
    {
        return Err(conflict("This topic identity already exists"));
    }
    if input.title.trim().is_empty() {
        return Err(invalid("A topic needs a title"));
    }
    Ok(())
}

fn replace_or_push<T>(items: &mut Vec<T>, item: T, same: impl Fn(&T, &T) -> bool) {
    if let Some(index) = items.iter().position(|current| same(current, &item)) {
        items[index] = item;
    } else {
        items.push(item);
    }
}

fn remove_by_id<T>(items: &mut Vec<T>, matches: impl Fn(&T) -> bool) -> Result<(), ToolError> {
    let index = items
        .iter()
        .position(matches)
        .ok_or_else(|| not_found("The relationship to remove is unavailable"))?;
    items.remove(index);
    Ok(())
}

fn remap_visual(
    target: &BoardVisualTarget,
    ids: &BTreeMap<String, String>,
    links: &BTreeMap<String, String>,
) -> Option<BoardVisualTarget> {
    match target {
        BoardVisualTarget::Item { item_id } => ids.get(item_id).map(|id| BoardVisualTarget::Item {
            item_id: id.clone(),
        }),
        BoardVisualTarget::TopicEdge { child_item_id } => {
            ids.get(child_item_id)
                .map(|id| BoardVisualTarget::TopicEdge {
                    child_item_id: id.clone(),
                })
        }
        BoardVisualTarget::Link { link_id } => {
            links.get(link_id).map(|id| BoardVisualTarget::Link {
                link_id: id.clone(),
            })
        }
        BoardVisualTarget::PresentationAnchor { .. } => None,
    }
}

fn copy_subtree(
    document: &mut BoardDocument,
    item_id: &str,
    parent_item_id: &str,
    index: usize,
    prefix: &str,
    now: &str,
) -> Result<(), ToolError> {
    validate_id(prefix)?;
    let subtree = board_topic_subtree(document, item_id)?;
    let ids: BTreeMap<_, _> = subtree
        .iter()
        .map(|id| (id.clone(), format!("{prefix}-{id}")))
        .collect();
    let originals: BTreeMap<_, _> = document
        .items
        .iter()
        .map(|item| (item.item_id.as_str(), item))
        .collect();
    let mut copies = Vec::new();
    for id in &subtree {
        let original = originals[id.as_str()];
        let BoardTarget::Topic {
            children,
            style,
            based_on,
        } = &original.target
        else {
            unreachable!()
        };
        let input = BoardTopicInput {
            item_id: ids[id].clone(),
            title: original.title.clone(),
            style: style.clone(),
            based_on: based_on.clone(),
            source_bindings: original.source_bindings.clone(),
        };
        fresh_topic(document, &input)?;
        let mut copied = topic_item(&input, now);
        if let BoardTarget::Topic {
            children: target_children,
            ..
        } = &mut copied.target
        {
            *target_children = children.iter().map(|child| ids[child].clone()).collect();
        }
        copies.push(copied);
    }
    // Internal visual relationships are copied; provenance still addresses the
    // original retained content. Deliveries and learning facts are not copied.
    let mut link_ids = BTreeMap::new();
    let mut copied_links = Vec::new();
    loop {
        let before = link_ids.len();
        for link in &document.links {
            if link.note_relation_id.is_some() || link_ids.contains_key(&link.link_id) {
                continue;
            }
            if let (Some(from), Some(to)) = (
                remap_visual(&link.from, &ids, &link_ids),
                remap_visual(&link.to, &ids, &link_ids),
            ) {
                let mut copied = link.clone();
                copied.link_id = format!("{prefix}-{}", link.link_id);
                validate_id(&copied.link_id)?;
                if document
                    .links
                    .iter()
                    .any(|old| old.link_id == copied.link_id)
                {
                    return Err(conflict("A copied relationship identity already exists"));
                }
                copied.from = from;
                copied.to = to;
                link_ids.insert(link.link_id.clone(), copied.link_id.clone());
                copied_links.push(copied);
            }
        }
        if link_ids.len() == before {
            break;
        }
    }
    let mut copied_mappings = Vec::new();
    for mapping in &document.mappings {
        if let Some(visual) = remap_visual(&mapping.visual, &ids, &link_ids) {
            let mut copied = mapping.clone();
            copied.mapping_id = format!("{prefix}-{}", mapping.mapping_id);
            validate_id(&copied.mapping_id)?;
            if document
                .mappings
                .iter()
                .any(|old| old.mapping_id == copied.mapping_id)
            {
                return Err(conflict("A copied mapping identity already exists"));
            }
            copied.visual = visual;
            copied_mappings.push(copied);
        }
    }
    document.items.extend(copies);
    insert_child(document, parent_item_id, index, ids[item_id].clone())?;
    document.links.extend(copied_links);
    document.mappings.extend(copied_mappings);
    Ok(())
}

pub(super) fn apply_operations(
    document: &mut BoardDocument,
    operations: &[BoardEditOperation],
    now: &str,
) -> Result<(), ToolError> {
    for operation in operations {
        match operation {
            BoardEditOperation::CreateMindMap { map, root } => {
                validate_id(&map.map_id)?;
                if map.root_item_id != root.item_id {
                    return Err(invalid("A mind map root must use its root topic identity"));
                }
                if document
                    .mind_maps
                    .iter()
                    .any(|old| old.map_id == map.map_id)
                {
                    return Err(conflict("This mind map identity already exists"));
                }
                fresh_topic(document, root)?;
                document.items.push(topic_item(root, now));
                document.mind_maps.push(map.clone());
            }
            BoardEditOperation::InsertTopic {
                parent_item_id,
                index,
                topic,
            } => {
                fresh_topic(document, topic)?;
                insert_child(document, parent_item_id, *index, topic.item_id.clone())?;
                document.items.push(topic_item(topic, now));
            }
            BoardEditOperation::UpdateTopic {
                item_id,
                title,
                style,
            } => {
                let item = document
                    .items
                    .iter_mut()
                    .find(|item| &item.item_id == item_id)
                    .ok_or_else(|| not_found("The topic to edit is unavailable"))?;
                let BoardTarget::Topic {
                    style: current_style,
                    ..
                } = &mut item.target
                else {
                    return Err(invalid("Only topics can be edited as topics"));
                };
                if let Some(title) = title {
                    if title.trim().is_empty() {
                        return Err(invalid("A topic needs a title"));
                    }
                    item.title = title.clone();
                }
                if let Some(style) = style {
                    *current_style = style.clone();
                }
            }
            BoardEditOperation::MoveSubtree {
                item_id,
                parent_item_id,
                index,
            } => {
                let descendants = board_topic_subtree(document, item_id)?;
                if descendants.contains(parent_item_id) {
                    return Err(invalid("A topic cannot be moved into its own branch"));
                }
                let old_parent = board_topic_parent(document, item_id)
                    .ok_or_else(|| invalid("Move the root by changing its mind map anchor"))?
                    .to_string();
                children_mut(document, &old_parent)?.retain(|id| id != item_id);
                // The index is in the destination children AFTER removing the
                // moved node, making reorder semantics consistent for all callers.
                insert_child(document, parent_item_id, *index, item_id.clone())?;
            }
            BoardEditOperation::CopySubtree {
                item_id,
                parent_item_id,
                index,
                id_prefix,
            } => copy_subtree(document, item_id, parent_item_id, *index, id_prefix, now)?,
            BoardEditOperation::RemoveSubtree { item_id } => {
                let removed: BTreeSet<_> = board_topic_subtree(document, item_id)?
                    .into_iter()
                    .collect();
                if let Some(parent) = board_topic_parent(document, item_id).map(str::to_string) {
                    children_mut(document, &parent)?.retain(|id| id != item_id);
                }
                document
                    .items
                    .retain(|item| !removed.contains(&item.item_id));
                document
                    .mind_maps
                    .retain(|map| !removed.contains(&map.root_item_id));
                prune_removed_targets(document);
            }
            BoardEditOperation::SetMindMap {
                map_id,
                layout,
                anchor,
                branch_sides,
            } => {
                let map = document
                    .mind_maps
                    .iter_mut()
                    .find(|map| &map.map_id == map_id)
                    .ok_or_else(|| not_found("The mind map is unavailable"))?;
                if let Some(layout) = layout {
                    map.layout = *layout;
                }
                if let Some(anchor) = anchor {
                    map.anchor = anchor.clone();
                }
                if let Some(sides) = branch_sides {
                    map.branch_sides = sides.clone();
                }
            }
            BoardEditOperation::PutLink { link } => {
                if link.note_relation_id.is_some()
                    || document.links.iter().any(|current| {
                        current.link_id == link.link_id && current.note_relation_id.is_some()
                    })
                {
                    return Err(invalid(
                        "Shared note relationships are edited through the notebook",
                    ));
                }
                replace_or_push(&mut document.links, link.clone(), |a, b| {
                    a.link_id == b.link_id
                });
            }
            BoardEditOperation::RemoveLink { link_id } => {
                if document
                    .links
                    .iter()
                    .any(|link| &link.link_id == link_id && link.note_relation_id.is_some())
                {
                    return Err(invalid(
                        "Shared note relationships are edited through the notebook",
                    ));
                }
                remove_by_id(&mut document.links, |link| &link.link_id == link_id)?;
                prune_removed_targets(document);
            }
            BoardEditOperation::PutMapping { mapping } => {
                replace_or_push(&mut document.mappings, mapping.clone(), |a, b| {
                    a.mapping_id == b.mapping_id
                })
            }
            BoardEditOperation::RemoveMapping { mapping_id } => {
                remove_by_id(&mut document.mappings, |mapping| {
                    &mapping.mapping_id == mapping_id
                })?
            }
            BoardEditOperation::PutRoute { route } => {
                replace_or_push(&mut document.routes, route.clone(), |a, b| {
                    a.route_id == b.route_id
                })
            }
            BoardEditOperation::RemoveRoute { route_id } => {
                remove_by_id(&mut document.routes, |route| &route.route_id == route_id)?
            }
        }
    }
    normalize_branch_sides(document)?;
    validate_mind_maps(document)
}

fn normalize_branch_sides(document: &mut BoardDocument) -> Result<(), ToolError> {
    for index in 0..document.mind_maps.len() {
        let ids = children(document, &document.mind_maps[index].root_item_id)?;
        let sides = &mut document.mind_maps[index].branch_sides;
        sides.retain(|id, _| ids.contains(id));
        for id in ids {
            if sides.contains_key(&id) {
                continue;
            }
            let left = sides
                .values()
                .filter(|side| **side == BoardTopicSide::Left)
                .count();
            let right = sides.len() - left;
            sides.insert(
                id,
                if right <= left {
                    BoardTopicSide::Right
                } else {
                    BoardTopicSide::Left
                },
            );
        }
    }
    Ok(())
}

pub fn validate_mind_maps(document: &BoardDocument) -> Result<(), ToolError> {
    let topics: BTreeMap<_, _> = document
        .items
        .iter()
        .filter_map(|item| match &item.target {
            BoardTarget::Topic { children, .. } => Some((item.item_id.as_str(), (item, children))),
            _ => None,
        })
        .collect();
    let mut parents = BTreeSet::new();
    for (item, children) in topics.values() {
        if item.title.trim().is_empty()
            || !item.readable_content.is_empty()
            || item.block_id.is_some()
            || !item.explanation_blocks.is_empty()
        {
            return Err(invalid(
                "A topic stores its short title separately from retained explanations",
            ));
        }
        for child in *children {
            if !topics.contains_key(child.as_str()) {
                return Err(invalid("Every topic child must be an existing topic"));
            }
            if !parents.insert(child.as_str()) {
                return Err(invalid("A topic can have only one parent"));
            }
        }
    }
    let mut maps = BTreeSet::new();
    let mut visited = BTreeSet::new();
    for map in &document.mind_maps {
        validate_id(&map.map_id)?;
        if !maps.insert(&map.map_id) || parents.contains(map.root_item_id.as_str()) {
            return Err(invalid(
                "Mind maps need unique identities and independent root topics",
            ));
        }
        if !map.anchor.x.is_finite() || !map.anchor.y.is_finite() {
            return Err(invalid("A mind map anchor must be finite"));
        }
        let (_, root_children) = topics
            .get(map.root_item_id.as_str())
            .ok_or_else(|| invalid("The mind map root topic is unavailable"))?;
        if map
            .branch_sides
            .keys()
            .any(|id| !root_children.contains(id))
        {
            return Err(invalid(
                "Branch sides can only name direct children of the root",
            ));
        }
        let mut pending = vec![map.root_item_id.as_str()];
        while let Some(id) = pending.pop() {
            if !visited.insert(id) {
                return Err(invalid(
                    "Topics must form acyclic trees and belong to one mind map",
                ));
            }
            let (_, children) = topics[id];
            pending.extend(children.iter().map(String::as_str));
        }
    }
    if visited.len() != topics.len() {
        return Err(invalid(
            "Every topic must be reachable from exactly one mind map root",
        ));
    }
    Ok(())
}

pub(super) fn record_edit(
    document: &mut BoardDocument,
    previous: &BoardDocument,
    actor: BoardEditActor,
    action: &str,
) {
    document.edit_history = previous.edit_history.clone();
    document
        .edit_history
        .transactions
        .push(BoardEditTransaction {
            revision: document.revision,
            before_revision: previous.revision,
            actor,
            action: action.into(),
            undo_of: None,
            redo_of: None,
        });
    document.edit_history.undo_stack.push(document.revision);
    document.edit_history.redo_stack.clear();
}

fn restore_changed<T: Clone + PartialEq>(
    current: &mut Vec<T>,
    from: &[T],
    to: &[T],
    id: impl Fn(&T) -> &str,
) -> Result<(), ToolError> {
    let ids: BTreeSet<_> = from
        .iter()
        .chain(to)
        .map(|entry| id(entry).to_string())
        .collect();
    for key in ids {
        let previous = from.iter().find(|entry| id(entry) == key);
        let target = to.iter().find(|entry| id(entry) == key);
        if previous == target {
            continue;
        }
        let actual = current.iter().position(|entry| id(entry) == key);
        if actual.map(|index| &current[index]) != previous {
            return Err(conflict(
                "This edit can no longer be undone because its content changed",
            ));
        }
        match (actual, target) {
            (Some(index), Some(target)) => current[index] = target.clone(),
            (Some(index), None) => {
                current.remove(index);
            }
            (None, Some(target)) => {
                let index = to
                    .iter()
                    .position(|entry| id(entry) == key)
                    .unwrap_or(current.len())
                    .min(current.len());
                current.insert(index, target.clone());
            }
            (None, None) => {}
        }
    }
    Ok(())
}

fn restore_visible_changes(
    current: &mut BoardDocument,
    from: &BoardDocument,
    to: &BoardDocument,
) -> Result<(), ToolError> {
    if from.title != to.title {
        if current.title != from.title {
            return Err(conflict("The board title changed since this edit"));
        }
        current.title = to.title.clone();
    }
    restore_changed(&mut current.items, &from.items, &to.items, |item| {
        &item.item_id
    })?;
    restore_changed(&mut current.links, &from.links, &to.links, |link| {
        &link.link_id
    })?;
    restore_changed(
        &mut current.mappings,
        &from.mappings,
        &to.mappings,
        |mapping| &mapping.mapping_id,
    )?;
    restore_changed(&mut current.routes, &from.routes, &to.routes, |route| {
        &route.route_id
    })?;
    restore_changed(
        &mut current.mind_maps,
        &from.mind_maps,
        &to.mind_maps,
        |map| &map.map_id,
    )?;
    // The current delivery receipts deliberately survive. Restoring an earlier
    // whole document would erase committed turn identities and recreate content
    // on retries, and would roll current workspace note text backwards.
    Ok(())
}

impl BoardStore {
    /// Notes keep their own revision clock. Candidates must compare this map
    /// even when the board revision did not change during generation.
    pub fn note_revisions(
        &self,
        document: &BoardDocument,
        read_item_ids: &[String],
    ) -> Result<BTreeMap<String, u32>, ToolError> {
        let note_ids: BTreeSet<_> = document
            .items
            .iter()
            .filter(|item| read_item_ids.contains(&item.item_id))
            .filter_map(|item| match &item.target {
                BoardTarget::Note { note_id, .. } => Some(note_id.as_str()),
                _ => None,
            })
            .collect();
        if note_ids.is_empty() {
            return Ok(BTreeMap::new());
        }
        let notes = crate::workspace_notes::WorkspaceNoteStore::open(
            &self.private_root,
            self.learning_workspace_id.as_deref(),
            self.private,
        )?
        .list()?;
        Ok(notes
            .notes
            .iter()
            .filter(|note| note_ids.contains(note.note_id.as_str()))
            .map(|note| (note.note_id.clone(), note.revision))
            .collect())
    }

    pub fn edit(
        &self,
        board_id: &str,
        expected_revision: u32,
        operations: Vec<BoardEditOperation>,
        actor: BoardEditActor,
        action: &str,
        now: &str,
    ) -> Result<BoardDocument, ToolError> {
        let _access = access()?;
        let head = self.read_head(board_id)?;
        if head.revision != expected_revision {
            return Err(conflict(
                "This board changed in another window; reopen it before saving",
            ));
        }
        if operations.is_empty() {
            return Err(invalid("A board edit needs at least one operation"));
        }
        let previous = self.read_snapshot(&head, head.revision)?;
        let mut document = apply_board_edits(&previous, &operations, now)?;
        record_edit(&mut document, &previous, actor, action);
        self.validate_document(&document)?;
        self.commit(&document)?;
        self.hydrate_notes(document)
    }

    pub fn undo(
        &self,
        board_id: &str,
        expected_revision: u32,
        now: &str,
    ) -> Result<BoardDocument, ToolError> {
        self.restore_edit(board_id, expected_revision, false, now)
    }

    pub fn redo(
        &self,
        board_id: &str,
        expected_revision: u32,
        now: &str,
    ) -> Result<BoardDocument, ToolError> {
        self.restore_edit(board_id, expected_revision, true, now)
    }

    fn restore_edit(
        &self,
        board_id: &str,
        expected_revision: u32,
        redo: bool,
        now: &str,
    ) -> Result<BoardDocument, ToolError> {
        let _access = access()?;
        let head = self.read_head(board_id)?;
        if head.revision != expected_revision {
            return Err(conflict(
                "This board changed in another window; reopen it before saving",
            ));
        }
        let mut document = self.read_snapshot(&head, head.revision)?;
        let stack = if redo {
            &document.edit_history.redo_stack
        } else {
            &document.edit_history.undo_stack
        };
        let revision = *stack.last().ok_or_else(|| {
            invalid(if redo {
                "There is no edit to redo"
            } else {
                "There is no edit to undo"
            })
        })?;
        let transaction = document
            .edit_history
            .transactions
            .iter()
            .find(|transaction| transaction.revision == revision)
            .ok_or_else(|| storage("The board edit history is missing its transaction"))?
            .clone();
        let before = self.read_snapshot(&head, transaction.before_revision)?;
        let after = self.read_snapshot(&head, transaction.revision)?;
        if redo {
            restore_visible_changes(&mut document, &before, &after)?;
        } else {
            restore_visible_changes(&mut document, &after, &before)?;
        }
        document.revision = head
            .revision
            .checked_add(1)
            .ok_or_else(|| invalid("Board revision limit reached"))?;
        document.updated_at = now.into();
        if redo {
            document.edit_history.redo_stack.pop();
            document.edit_history.undo_stack.push(revision);
        } else {
            document.edit_history.undo_stack.pop();
            document.edit_history.redo_stack.push(revision);
        }
        document
            .edit_history
            .transactions
            .push(BoardEditTransaction {
                revision: document.revision,
                before_revision: head.revision,
                actor: BoardEditActor::Human,
                action: transaction.action,
                undo_of: (!redo).then_some(revision),
                redo_of: redo.then_some(revision),
            });
        normalize_note_references(&mut document);
        self.validate_document(&document)?;
        self.commit(&document)?;
        self.hydrate_notes(document)
    }
}

fn item_semantic_eq(left: &BoardItem, right: &BoardItem) -> bool {
    left.item_id == right.item_id
        && left.title == right.title
        && left.target == right.target
        && left.readable_content == right.readable_content
        && left.source_bindings == right.source_bindings
        && left.reply_to_item_id == right.reply_to_item_id
        && left.block_id == right.block_id
        && left.explanation_blocks == right.explanation_blocks
        && left.created_at == right.created_at
}

fn item_write_conflict(before: &BoardItem, after: &BoardItem, current: &BoardItem) -> bool {
    if before.title != after.title && before.title != current.title {
        return true;
    }
    match (&before.target, &after.target, &current.target) {
        (
            BoardTarget::Topic {
                children: a,
                style: sa,
                based_on: ba,
            },
            BoardTarget::Topic {
                children: b,
                style: sb,
                based_on: bb,
            },
            BoardTarget::Topic {
                children: c,
                style: sc,
                based_on: bc,
            },
        ) => (a != b && a != c) || (sa != sb && sa != sc) || (ba != bb && ba != bc),
        _ => !item_semantic_eq(before, current),
    }
}

fn collection_conflict<T: PartialEq>(
    base: &[T],
    projected: &[T],
    latest: &[T],
    id: impl Fn(&T) -> &str,
) -> bool {
    let ids: BTreeSet<_> = base.iter().chain(projected).map(&id).collect();
    ids.into_iter().any(|key| {
        let before = base.iter().find(|entry| id(entry) == key);
        let after = projected.iter().find(|entry| id(entry) == key);
        before != after && latest.iter().find(|entry| id(entry) == key) != before
    })
}

/// A bounded rebase checks actual written semantic fields plus explicit read
/// dependencies. It never treats new layout coordinates as changed content.
/// `base` and `latest` may be hydrated, so read dependencies include the current
/// text/provenance of a workspace note used to construct the candidate.
pub fn board_edits_conflict(
    base: &BoardDocument,
    latest: &BoardDocument,
    operations: &[BoardEditOperation],
    read_item_ids: &[String],
) -> Result<bool, ToolError> {
    let projected = apply_board_edits(base, operations, &base.updated_at)?;
    for id in read_item_ids {
        let before = base.items.iter().find(|item| &item.item_id == id);
        let current = latest.items.iter().find(|item| &item.item_id == id);
        if !matches!((before, current), (Some(a), Some(b)) if item_semantic_eq(a, b)) {
            return Ok(true);
        }
    }
    let ids: BTreeSet<_> = base
        .items
        .iter()
        .chain(&projected.items)
        .map(|item| &item.item_id)
        .collect();
    for id in ids {
        let before = base.items.iter().find(|item| &item.item_id == id);
        let after = projected.items.iter().find(|item| &item.item_id == id);
        let current = latest.items.iter().find(|item| &item.item_id == id);
        match (before, after, current) {
            (Some(a), Some(b), Some(c)) if !item_semantic_eq(a, b) => {
                if item_write_conflict(a, b, c) {
                    return Ok(true);
                }
            }
            (Some(a), None, Some(c)) if item_semantic_eq(a, c) => {}
            (None, Some(_), None) => {}
            (Some(a), Some(b), _) if item_semantic_eq(a, b) => {}
            (None, None, _) => {}
            _ => return Ok(true),
        }
    }
    if collection_conflict(&base.links, &projected.links, &latest.links, |link| {
        &link.link_id
    }) || collection_conflict(
        &base.mappings,
        &projected.mappings,
        &latest.mappings,
        |mapping| &mapping.mapping_id,
    ) || collection_conflict(&base.routes, &projected.routes, &latest.routes, |route| {
        &route.route_id
    }) {
        return Ok(true);
    }
    let maps: BTreeSet<_> = base
        .mind_maps
        .iter()
        .chain(&projected.mind_maps)
        .map(|map| &map.map_id)
        .collect();
    for id in maps {
        let before = base.mind_maps.iter().find(|map| &map.map_id == id);
        let after = projected.mind_maps.iter().find(|map| &map.map_id == id);
        let current = latest.mind_maps.iter().find(|map| &map.map_id == id);
        if before == after {
            continue;
        }
        match (before, after, current) {
            (Some(a), Some(b), Some(c)) => {
                if (a.layout != b.layout && a.layout != c.layout)
                    || (a.anchor != b.anchor && a.anchor != c.anchor)
                    || (a.root_item_id != b.root_item_id && a.root_item_id != c.root_item_id)
                    || (a.branch_sides != b.branch_sides && a.branch_sides != c.branch_sides)
                {
                    return Ok(true);
                }
            }
            _ if current != before => return Ok(true),
            _ => {}
        }
    }
    // Replaying also reads a copied branch and its internal relationships. A
    // changed source must not silently produce a different copied suggestion.
    // Compare the newly created semantic objects, using the same creation time.
    let Ok(replayed) = apply_board_edits(latest, operations, &base.updated_at) else {
        return Ok(true);
    };
    let created_items =
        |before: &BoardDocument, after: &BoardDocument| -> BTreeMap<String, BoardItem> {
            after
                .items
                .iter()
                .filter(|item| !before.items.iter().any(|old| old.item_id == item.item_id))
                .map(|item| (item.item_id.clone(), item.clone()))
                .collect()
        };
    if created_items(base, &projected) != created_items(latest, &replayed) {
        return Ok(true);
    }
    macro_rules! created_entities {
        ($field:ident, $id:ident) => {{
            let expected: BTreeMap<_, _> = projected
                .$field
                .iter()
                .filter(|entry| !base.$field.iter().any(|old| old.$id == entry.$id))
                .map(|entry| (&entry.$id, entry))
                .collect();
            let actual: BTreeMap<_, _> = replayed
                .$field
                .iter()
                .filter(|entry| !latest.$field.iter().any(|old| old.$id == entry.$id))
                .map(|entry| (&entry.$id, entry))
                .collect();
            if expected != actual {
                return Ok(true);
            }
        }};
    }
    created_entities!(links, link_id);
    created_entities!(mappings, mapping_id);
    created_entities!(routes, route_id);
    Ok(false)
}

#[cfg(test)]
mod tests;
