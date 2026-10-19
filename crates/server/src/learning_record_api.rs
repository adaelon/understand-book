//! Explicit links to original chats and learning facts; reads never adopt a chat.
use crate::{authorization::{self, Authorization, AuthorizedContext}, session_recap, user_storage_paths::error};
use memory::learning_records::LearningRecordSelection;
use read_tools::ToolError;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashMap};

fn invalid() -> ToolError { error("INVALID_LEARNING_RECORD_REQUEST", "validation", "请选择原学习记录及已确认的工作区目标") }

fn publications(session: &crate::AgentChatSession) -> Vec<base_schema::PublishedBookRef> {
    session.turns.iter().flat_map(|turn| turn.published_book_ref.clone().into_iter()
        .chain(turn.source_bindings.iter().filter_map(|binding| session_recap::source_publication(turn, binding))))
        .collect::<BTreeSet<_>>().into_iter().collect()
}

pub(crate) fn historical_turn(turn: &crate::AgentChatTurn, projection: &crate::session_event::SessionProjection) -> Value {
    let mut value = authorization::turn_json(turn);
    value["question_quote"] = json!(turn.question_quote);
    value["workspace_goal"] = json!(projection.inputs.get(&turn.turn_id).and_then(|input|input.learning_workspace_goal.as_ref())
        .or_else(||turn.domain.scene.as_ref().and_then(|scene|scene.workspace_goal.as_ref())));
    value["accepted_workspace_goal"] = json!(turn.domain.workspace_goal_update);
    value
}

pub(crate) fn read(access: &Authorization, context: &AuthorizedContext, id: Option<&str>,
    query: &HashMap<String, String>, now: i64) -> Result<Value, ToolError> {
    if query.keys().any(|key| key != "through_seq") || (id.is_none() && !query.is_empty()) { return Err(invalid()); }
    let user = context.user.lock().unwrap();
    user.check_owner(context.user_id())?;
    let Some(id) = id else {
        let mut records = Vec::new();
        for session in user.agent_history.sessions.iter().filter(|session| !session.turns.is_empty()) {
            let Some(log) = user.session_store.as_ref().and_then(|store| store.logs.get(&session.id)) else { continue; };
            let source_goal = session.learning_workspace_id.as_deref()
                .map(|scope| access.learning_workspace_metadata(context.user_id(), scope).map(|workspace| workspace.goal.text)).transpose()?;
            let refs = publications(session);
            records.push(json!({"session_id":session.id,"title":session.title,"book_id":session.book_id,
                "source_learning_workspace_id":session.learning_workspace_id,"learning_workspace_id":session.learning_workspace_id,
                "source_goal":source_goal,"published_book_ref":refs.first(),"published_book_refs":refs,
                "updated_at":session.updated_at,"turn_count":session.turns.len(),"through_seq":log.projection.through_seq}));
        }
        records.sort_by(|a,b| b["updated_at"].as_str().cmp(&a["updated_at"].as_str()));
        return Ok(json!({"records":records}));
    };
    let through = query.get("through_seq").map(|value| value.parse::<u64>().ok().filter(|n| *n > 0).ok_or_else(invalid)).transpose()?;
    let projection = session_recap::snapshot(&user, id, through)?;
    let session = projection.session.as_ref().ok_or_else(authorization::missing)?;
    let mut recap = session_recap::project(&projection, &now.to_string());
    {
        let mut library = access.library.lock().unwrap();
        let mut cache = std::collections::BTreeMap::new();
        session_recap::resolve(&mut recap, &projection, &user, |_, reference| {
            let reference = reference?;
            cache.entry(reference.clone()).or_insert_with(|| library.load(context.user_id(), reference).ok()
                .map(|publication| (publication.book.clone(), publication.directory.clone()))).clone()
        });
    }
    let mut sources = Vec::new();
    for turn in &session.turns {
        for binding in &turn.source_bindings {
            let reference = session_recap::source_publication(turn, binding);
            let mut original = binding.clone();
            original.publication_id = reference.as_ref().map(|reference| reference.publication_id.clone());
            let resolved = reference.as_ref().and_then(|_| crate::cross_book::source_view(&access.library, context.user_id(), &original, false).ok());
            sources.push(json!({"turn_id":turn.turn_id,"source_ref_id":binding.source_ref_id,"label":binding.label_snapshot,
                "quote":binding.preview_snapshot,"published_book_ref":reference,"view":resolved,
                "unavailable_reason":resolved.is_none().then_some(if reference.is_none() { "原记录没有发布版本，保留当时来源片段" } else { "原发布或原文当前不可用" })}));
        }
    }
    let turns = session.turns.iter().map(|turn| turn.turn_id.clone()).collect::<Vec<_>>();
    let mut learning = user.learning_store_for_workspace(session.learning_workspace_id.as_deref())?.chat_record_bundle(id, &turns)?;
    learning.selection.through_seq = projection.through_seq;
    Ok(json!({"session_id":session.id,"title":session.title,"book_id":session.book_id,
        "learning_workspace_id":session.learning_workspace_id,"through_seq":projection.through_seq,
        "turns":session.turns.iter().map(|turn|historical_turn(turn,&projection)).collect::<Vec<_>>(),
        "recap":recap,"sources":sources,"learning":learning}))
}

pub(crate) fn linked_request(access: &Authorization, context: &AuthorizedContext, method: &str,
    workspace_id: &str, input: Value, now: i64) -> Result<Value, ToolError> {
    let workspace = access.learning_workspace_metadata(context.user_id(), workspace_id)?;
    let user = context.user.lock().unwrap();
    user.check_owner(context.user_id())?;
    let mut target = user.learning_store_for_workspace(Some(workspace_id))?;
    match method {
        "GET" => {
            Ok(json!({"records":linked_summaries(&user, workspace_id)?}))
        }
        "POST" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Link { session_id: String, through_seq: u64, goal_revision: u32 }
            let request: Link = serde_json::from_value(input).map_err(|_| invalid())?;
            if workspace.archived { return Err(error("LEARNING_WORKSPACE_ARCHIVED", "conflict", "请先恢复工作区，再带入已有记录")); }
            if workspace.goal.revision != request.goal_revision {
                return Err(error("WORKSPACE_GOAL_REVISION_CONFLICT", "conflict", "工作区目标已更新，请查看新目标后再带入记录"));
            }
            let projection = session_recap::snapshot(&user, &request.session_id, Some(request.through_seq))?;
            let session = projection.session.as_ref().ok_or_else(authorization::missing)?;
            if session.turns.is_empty() || session.learning_workspace_id.as_deref() == Some(workspace_id) { return Err(invalid()); }
            let selection = LearningRecordSelection { source_learning_workspace_id:session.learning_workspace_id.clone(),
                chat_session_id:session.id.clone(),turn_ids:session.turns.iter().map(|turn|turn.turn_id.clone()).collect(),
                through_seq:projection.through_seq };
            Ok(json!(target.link_records(&selection, &now.to_string())?))
        }
        _ => Err(error("METHOD_NOT_ALLOWED", "method", "Use GET or POST for learning records")),
    }
}

pub(crate) fn linked_summaries(user: &crate::user_runtime::UserRuntime, workspace_id: &str) -> Result<Vec<Value>, ToolError> {
    Ok(user.learning_store_for_workspace(Some(workspace_id))?.linked_record_selections()?.into_iter().map(|linked| {
        let original = user.agent_history.sessions.iter().find(|session| session.id == linked.selection.chat_session_id
            && session.learning_workspace_id == linked.selection.source_learning_workspace_id);
        json!({"selection":linked.selection,"linked_at":linked.linked_at,"through_seq":linked.selection.through_seq,
            "title":original.map(|session|session.title.as_str()),"book_id":original.map(|session|session.book_id.as_str()),"available":original.is_some()})
    }).collect())
}
