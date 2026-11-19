//! A regeneration keeps the concrete reader changes that made its source stale.
use super::*;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegenerationContext {
    pub source: BoardRegenerationRef,
    original_request: String,
    protected: Vec<ProtectedRecord>,
    deleted: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProtectedRecord {
    collection: String,
    id: String,
    // Only values changed since the pending suggestion's base are retained.
    fields: BTreeMap<String, Value>,
    preserve_child_order: bool,
    preserve_parent: bool,
    current_parent: Option<String>,
}

fn invalid(message: impl Into<String>) -> ToolError {
    runtime::board_author::error("BOARD_REGENERATION_PROTECTED", message)
}

fn parent(board: &BoardDocument, id: &str) -> Option<String> {
    board.items.iter().find_map(|item| match &item.target {
        BoardTarget::Topic { children, .. } if children.iter().any(|child| child == id) => Some(item.item_id.clone()),
        _ => None,
    })
}

fn records(board: &BoardDocument, collection: &str) -> Vec<Value> {
    let value = match collection {
        "items" => json!(board.items), "mind_maps" => json!(board.mind_maps),
        "links" => json!(board.links), "mappings" => json!(board.mappings),
        "routes" => json!(board.routes), _ => unreachable!(),
    };
    value.as_array().unwrap().clone()
}

fn id_field(collection: &str) -> &str {
    match collection { "items" => "item_id", "mind_maps" => "map_id", "links" => "link_id", "mappings" => "mapping_id", "routes" => "route_id", _ => unreachable!() }
}

fn freeze(source: BoardRegenerationRef, original_request: String, base: &BoardDocument, current: &BoardDocument) -> RegenerationContext {
    let mut context = RegenerationContext {source,original_request,protected:vec![],deleted:vec![]};
    for collection in ["items", "mind_maps", "links", "mappings", "routes"] {
        let id_key = id_field(collection);
        let old = records(base,collection);
        let latest = records(current,collection);
        for record in &latest {
            let id = record[id_key].as_str().unwrap();
            let before = old.iter().find(|item|item[id_key]==id);
            let topic = collection=="items" && record["target"]["kind"]=="topic";
            let paths:Vec<String> = if topic {
                ["/title","/source_bindings","/target/kind","/target/style/color","/target/style/bold","/target/based_on"].into_iter().map(str::to_owned).collect()
            } else if collection=="items" {
                ["/title","/readable_content","/explanation_blocks","/source_bindings","/target","/position","/size","/reply_to_item_id","/block_id"].into_iter().map(str::to_owned).collect()
            } else {
                record.as_object().unwrap().keys().filter(|key|key.as_str()!=id_key).map(|key|format!("/{key}")).collect()
            };
            let fields:BTreeMap<_,_> = paths.into_iter().filter_map(|path| {
                let value = record.pointer(&path).cloned().unwrap_or(Value::Null);
                (before.and_then(|item|item.pointer(&path)) != record.pointer(&path)).then_some((path,value))
            }).collect();
            let preserve_child_order = topic && before.map(|item|&item["target"]["children"])!=Some(&record["target"]["children"]);
            let current_parent = topic.then(||parent(current,id)).flatten();
            let preserve_parent = topic && (before.is_none() || parent(base,id)!=current_parent);
            if before.is_none() || !fields.is_empty() || preserve_child_order || preserve_parent {
                context.protected.push(ProtectedRecord {collection:collection.into(),id:id.into(),fields,preserve_child_order,preserve_parent,current_parent});
            }
        }
        for removed in old.iter().filter(|record|!latest.iter().any(|item|item[id_key]==record[id_key])) {
            context.deleted.push((collection.into(),removed[id_key].as_str().unwrap().into()));
        }
    }
    context
}

pub(crate) fn regeneration_context(user: &user_runtime::UserRuntime, board: &BoardDocument, follow_up: &BoardFollowUp) -> Result<Option<Box<RegenerationContext>>,ToolError> {
    let Some(source)=follow_up.regenerate_from.as_ref() else {return Ok(None)};
    if follow_up.board_revision!=board.revision {
        return Err(runtime::board_author::error("BOARD_REVISION_CONFLICT","Reload the current board before regenerating a pending suggestion"));
    }
    let original=user.agent_history.sessions.iter()
        .find(|session|session.id==source.session_id && session.matches_scope(board.learning_workspace_id.as_deref(),&board.book_id))
        .and_then(|session|session.turns.iter().find(|turn|turn.turn_id==source.turn_id))
        .filter(|turn|turn.status!=AgentAssistantStatus::Cancelled)
        .ok_or_else(||invalid("The pending suggestion is unavailable in this board's conversation"))?;
    let candidate=original.domain.board_authoring.as_ref()
        .filter(|candidate|candidate.board_id==board.board_id && candidate.candidate_id==source.candidate_id
            && candidate.candidate_revision==source.candidate_revision && candidate.conflict.is_some())
        .ok_or_else(||invalid("Regeneration requires the exact retained conflicting candidate"))?;
    let original_scope=original.domain.board_context.as_ref().ok_or_else(||invalid("The pending suggestion has no retained board scope"))?;
    if follow_up.item_id!=original_scope.follow_up.item_id {
        return Err(invalid("Select the original suggestion's topic before regenerating it"));
    }
    let base=candidate.edit_base.as_ref().ok_or_else(||invalid("The pending suggestion has no semantic base; start a new explicit request from the current content"))?;
    Ok(Some(Box::new(freeze(source.clone(),original.user.clone(),base,board))))
}

pub(crate) fn regeneration_public_context(context: &RegenerationContext) -> Value {
    json!({"source":context.source,"original_request":context.original_request,
        "preserve_current_fields":context.protected,"preserve_deleted_identities":context.deleted,
        "instruction":"This is an explicit retry of the retained pending suggestion. The original_request identifies the unfinished work; it does not authorize reverting the reader's subsequent changes. The listed current fields and identities take precedence. Keep them, and complete the remaining work within the selected subtree (for example add the missing example). Omit any old rename/style/move that would overwrite a protected field. Do not recreate removed items. Unchanged fields remain editable. The server verifies this before preview and commit."})
}

pub(super) fn require_preserved(context: &RegenerationContext, current: &BoardDocument, proposed: &BoardDocument) -> Result<(),ToolError> {
    for collection in ["items", "mind_maps", "links", "mappings", "routes"] {
        let id_key=id_field(collection);
        let latest=records(current,collection);
        let next=records(proposed,collection);
        for protected in context.protected.iter().filter(|record|record.collection==collection) {
            let before=latest.iter().find(|item|item[id_key]==protected.id);
            let after=next.iter().find(|item|item[id_key]==protected.id);
            let fail=|field:&str|invalid(format!("Regeneration must preserve the reader's current {collection}/{} {field}. Remove the operation that changes this protected field and complete only the remaining work; other branches and new example children are still allowed.",protected.id));
            match (before,after) {
                (Some(before),Some(after)) => {
                    for path in protected.fields.keys() {
                        if before.pointer(path)!=after.pointer(path) {return Err(fail(path))}
                    }
                    if protected.preserve_child_order {
                        let children=before["target"]["children"].as_array().unwrap();
                        let mut proposed_children=after["target"]["children"].as_array().ok_or_else(||fail("children"))?.iter();
                        if !children.iter().all(|child|proposed_children.any(|next|next==child)) {return Err(fail("children/order"))}
                    }
                    if protected.preserve_parent && parent(current,&protected.id)!=parent(proposed,&protected.id) {return Err(fail("parent"))}
                }
                (None,None) => {},
                _ => return Err(fail("identity")),
            }
        }
        for (_,id) in context.deleted.iter().filter(|(kind,_)|kind==collection) {
            // If the reader subsequently restores an identity, retain that
            // restoration too. The retry itself may never resurrect it.
            if latest.iter().find(|item|item[id_key]==*id)!=next.iter().find(|item|item[id_key]==*id) {
                return Err(invalid(format!("Regeneration must preserve the reader's removal/restoration of {collection}/{id}")));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    const NOW:&str="2026-10-28T12:00:00Z";
    fn board() -> BoardDocument {
        let empty:BoardDocument=serde_json::from_value(json!({"board_id":"regeneration","learning_workspace_id":null,"book_id":"rust","publication_id":null,"source_revision":null,
            "title":"切片","revision":1,"created_at":NOW,"updated_at":NOW,"items":[],"deliveries":[],"links":[],"mappings":[],"routes":[]})).unwrap();
        apply_board_edits(&empty,&serde_json::from_value::<Vec<BoardEditOperation>>(json!([
            {"kind":"create_mind_map","map":{"map_id":"map","root_item_id":"root","anchor":{"x":0,"y":0}},"root":{"item_id":"root","title":"切片"}},
            {"kind":"insert_topic","parent_item_id":"root","index":0,"topic":{"item_id":"mutable","title":"旧标题"}},
            {"kind":"insert_topic","parent_item_id":"mutable","index":0,"topic":{"item_id":"detail","title":"可继续改的说明"}},
            {"kind":"insert_topic","parent_item_id":"root","index":1,"topic":{"item_id":"removed","title":"不再需要"}}
        ])).unwrap(),NOW).unwrap()
    }
    fn context(base:&BoardDocument,current:&BoardDocument)->RegenerationContext {
        freeze(BoardRegenerationRef{session_id:"session".into(),turn_id:"turn".into(),candidate_id:"candidate".into(),candidate_revision:1},"把标题改回旧标题并补例子".into(),base,current)
    }
    #[test]
    fn regeneration_preserves_changed_fields_and_deleted_identities_while_allowing_remaining_work() {
        let base=board();
        let current=apply_board_edits(&base,&serde_json::from_value::<Vec<BoardEditOperation>>(json!([
            {"kind":"update_topic","item_id":"mutable","title":"人工保留的新标题","style":{"color":"#15803d","bold":false}},
            {"kind":"remove_subtree","item_id":"removed"},
            {"kind":"insert_topic","parent_item_id":"mutable","index":0,"topic":{"item_id":"human-child","title":"人工新增例子"}}
        ])).unwrap(),NOW).unwrap();
        let protected=context(&base,&current);
        // Reverting to the old base is exactly the replay-04 failure; a
        // write-difference check against that old base would miss it.
        for operation in [
            json!({"kind":"update_topic","item_id":"mutable","title":"旧标题"}),
            json!({"kind":"update_topic","item_id":"mutable","style":{"color":"#2563eb","bold":false}}),
            json!({"kind":"remove_subtree","item_id":"human-child"}),
            json!({"kind":"move_subtree","item_id":"human-child","parent_item_id":"root","index":1}),
            json!({"kind":"insert_topic","parent_item_id":"root","index":1,"topic":{"item_id":"removed","title":"复活旧主题"}}),
        ] {
            let proposed=apply_board_edits(&current,&[serde_json::from_value(operation.clone()).unwrap()],NOW).unwrap();
            assert_eq!(require_preserved(&protected,&current,&proposed).unwrap_err().error_code,"BOARD_REGENERATION_PROTECTED","{operation}");
        }
        let proposed=apply_board_edits(&current,&serde_json::from_value::<Vec<BoardEditOperation>>(json!([
            {"kind":"update_topic","item_id":"detail","title":"改清楚未冲突的说明"},
            {"kind":"update_topic","item_id":"mutable","style":{"color":"#15803d","bold":true}},
            {"kind":"insert_topic","parent_item_id":"mutable","index":1,"topic":{"item_id":"new-example","title":"s[0] = 99"}}
        ])).unwrap(),NOW).unwrap();
        require_preserved(&protected,&current,&proposed).unwrap();
        assert_eq!(proposed.items.iter().find(|item|item.item_id=="mutable").unwrap().title,"人工保留的新标题");
        let public=regeneration_public_context(&protected);
        assert!(public.to_string().contains("人工保留的新标题"));
        assert!(public.to_string().contains("/target/style/color"));
    }
}
