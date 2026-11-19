//! Bounded model reads of original workspace conversations and selected history.
use crate::{service_error, learning_record_api, session_recap, user_runtime::UserRuntime};
use memory::learning_records::{LearningRecordSelection, LinkedLearningRecords};
use read_tools::ToolError;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    operation: String,
    chat_session_id: Option<String>,
    query: Option<String>,
    turn_id: Option<String>,
    record_id: Option<String>,
    through_seq: Option<u64>,
    #[serde(default)]
    after: usize,
    limit: Option<usize>,
}

pub(crate) fn read(user: &UserRuntime, workspace: &str, input: Value) -> Result<Value, ToolError> {
    read_scoped(user, Some(workspace), "", input)
}

pub fn read_scoped(
    user: &UserRuntime,
    workspace: Option<&str>,
    book_id: &str,
    input: Value,
) -> Result<Value, ToolError> {
    let invalid = || memory::teaching::invalid("请选择本工作区的原记录、检索文本及有效分页范围");
    let request: Request = serde_json::from_value(input.clone()).map_err(|_| invalid())?;
    let limit = request.limit.unwrap_or(8);
    let query = request.query.as_deref().map(str::trim);
    if !(1..=20).contains(&limit)
        || query.is_some_and(|q| q.is_empty() || q.chars().count() > 512)
        || request.through_seq == Some(0)
        || (request.through_seq.is_some() && request.chat_session_id.is_none())
        || (request.operation == "search" && query.is_none())
    {
        return Err(invalid());
    }
    let mut selection = Value::Null;
    let mut items = match request.operation.as_str() {
        "list" => summaries(user, workspace, book_id)?,
        "read" => {
            let id = request.chat_session_id.as_deref().ok_or_else(invalid)?;
            let result = records_at(user, workspace, book_id, id, request.through_seq)?;
            selection = result.0;
            result.1
        }
        "search" => {
            let chats = if let Some(id) = &request.chat_session_id {
                vec![id.clone()]
            } else {
                summaries(user, workspace, book_id)?.into_iter()
                    .filter(|row| row["available"] != false)
                    .filter_map(|row| row["chat_session_id"].as_str().map(str::to_owned))
                    .collect()
            };
            let mut found = Vec::new();
            for chat in chats {
                let (_, records) = records_at(user, workspace, book_id, &chat, request.through_seq)?;
                found.extend(records.into_iter().filter(|row| matches_query(row, query.unwrap())));
            }
            deduplicate(found)
        }
        _ => return Err(invalid()),
    };
    if request.operation != "search" {
        if let Some(query) = query {
            items.retain(|row| matches_query(row, query));
        }
    }
    if let Some(turn) = &request.turn_id {
        items.retain(|row| row["turn_id"].as_str() == Some(turn));
    }
    if let Some(record) = &request.record_id {
        items.retain(|row| row["record_id"].as_str() == Some(record));
    }
    let total = items.len();
    let rows: Vec<_> = items.into_iter().skip(request.after).take(limit).collect();
    let end = request.after.saturating_add(rows.len());
    let next = (end < total).then_some(end);
    let next_request = next.map(|after| {
        let mut next = input;
        next["after"] = json!(after);
        if request.operation == "read" {
            next["through_seq"] = selection["through_seq"].clone();
        }
        next
    });
    Ok(json!({"learning_workspace_id":workspace,"book_id":workspace.is_none().then_some(book_id),
        "selection":selection,"records":rows,"total":total,"next":next,"next_request":next_request,
        "query":query,"search_scope":"all_permitted_records_before_pagination",
        "historical":true,"current_mastery":false}))
}

pub(crate) fn summaries(user: &UserRuntime, workspace: Option<&str>, book_id: &str) -> Result<Vec<Value>, ToolError> {
    let mut rows = Vec::new();
    let mut seen = BTreeSet::new();
    for session in user.agent_history.sessions.iter()
        .filter(|session| session.matches_scope(workspace, book_id) && !session.turns.is_empty())
    {
        let through_seq = user.session_store.as_ref().and_then(|store| store.logs.get(&session.id))
            .map(|log| log.projection.through_seq);
        if !seen.insert(session.id.clone()) { continue; }
        let references = learning_record_api::publications(session);
        rows.push(json!({"chat_session_id":session.id,"session_id":session.id,"title":session.title,
            "book_id":session.book_id,"source_learning_workspace_id":session.learning_workspace_id,
            "published_book_ref":references.first(),"published_book_refs":references,
            "created_at":session.created_at,"updated_at":session.updated_at,"turn_count":session.turns.len(),
            "through_seq":through_seq,"origin":"native","available":true,
            "selection":{"source_learning_workspace_id":session.learning_workspace_id,
                "chat_session_id":session.id,"through_seq":through_seq},
            "read_request":{"operation":"read","chat_session_id":session.id,"through_seq":through_seq}}));
    }
    if let Some(workspace) = workspace {
        for mut row in learning_record_api::linked_summaries(user, workspace)? {
            let id = row["selection"]["chat_session_id"].as_str().unwrap().to_owned();
            if !seen.insert(id.clone()) { continue; }
            row["chat_session_id"] = json!(id);
            row["session_id"] = json!(id);
            row["source_learning_workspace_id"] = row["selection"]["source_learning_workspace_id"].clone();
            row["origin"] = json!("linked");
            row["read_request"] = json!({"operation":"read","chat_session_id":id,"through_seq":row["through_seq"]});
            rows.push(row);
        }
    }
    rows.sort_by(|a,b| b["updated_at"].as_str().cmp(&a["updated_at"].as_str())
        .then_with(|| a["chat_session_id"].as_str().cmp(&b["chat_session_id"].as_str())));
    Ok(rows)
}

/// Resolves only this scope's own conversations or its explicitly linked prefix.
/// Board collection uses the same records as model search, never a wider cache.
pub(crate) fn records_for_chat(
    user: &UserRuntime,
    workspace: Option<&str>,
    book_id: &str,
    id: &str,
) -> Result<(Value, Vec<Value>), ToolError> {
    records_at(user, workspace, book_id, id, None)
}

/// Retains the typed original turn after the same selection check used by search.
pub(crate) fn permitted_turn(
    user: &UserRuntime,
    workspace: Option<&str>,
    book_id: &str,
    chat_id: &str,
    turn_id: &str,
) -> Result<crate::AgentChatTurn, ToolError> {
    if let Some(session) = user.agent_history.sessions.iter()
        .find(|session| session.id == chat_id && session.matches_scope(workspace, book_id))
    {
        return session.turns.iter().find(|turn| turn.turn_id == turn_id)
            .cloned().ok_or_else(service_error::missing);
    }
    let (selection, rows) = records_for_chat(user, workspace, book_id, chat_id)?;
    if !rows.iter().any(|row|row["kind"] == "chat_turn" && row["turn_id"].as_str() == Some(turn_id)) {
        return Err(service_error::missing());
    }
    let through = selection["through_seq"].as_u64().ok_or_else(service_error::missing)?;
    let log = user.session_store.as_ref().and_then(|store|store.logs.get(chat_id)).ok_or_else(service_error::missing)?;
    let saved;
    let projection = if through == log.projection.through_seq { &log.projection } else {
        saved = session_recap::snapshot(user, chat_id, Some(through))?;
        &saved
    };
    projection.session.as_ref().and_then(|session|session.turns.iter().find(|turn|turn.turn_id == turn_id))
        .cloned().ok_or_else(service_error::missing)
}

fn records_at(user: &UserRuntime, workspace: Option<&str>, book_id: &str, id: &str,
    through: Option<u64>) -> Result<(Value, Vec<Value>), ToolError>
{
    if let Some(session) = user.agent_history.sessions.iter()
        .find(|session| session.id == id && session.matches_scope(workspace, book_id))
    {
        let Some(log) = user.session_store.as_ref().and_then(|store|store.logs.get(id)) else {
            if through.is_some() {
                return Err(memory::teaching::invalid("原会话只保存了历史快照，没有可读取的事件序号"));
            }
            let turns = session.turns.iter().map(|turn|turn.turn_id.clone()).collect::<Vec<_>>();
            let bundle = user.learning_store_for_workspace(workspace)?.chat_record_bundle(id, &turns)?;
            return Ok(project_session_records(session, None, bundle, "native"));
        };
        let saved;
        let projection = if through.is_some_and(|seq| seq != log.projection.through_seq) {
            saved = session_recap::snapshot(user, id, through)?;
            &saved
        } else { &log.projection };
        let session = projection.session.as_ref().ok_or_else(service_error::missing)?;
        let turns = session.turns.iter().map(|turn|turn.turn_id.clone()).collect::<Vec<_>>();
        let mut bundle = user.learning_store_for_workspace(workspace)?.chat_record_bundle(id, &turns)?;
        bundle.selection.through_seq = projection.through_seq;
        Ok(project_records(projection, bundle, "native"))
    } else {
        let workspace = workspace.ok_or_else(service_error::missing)?;
        let result = selected_records(user, workspace, id)?;
        if through.is_some_and(|seq| result.0["through_seq"].as_u64() != Some(seq)) {
            return Err(memory::teaching::invalid("请读取已选入的历史范围，不能扩展原聊天前缀"));
        }
        Ok(result)
    }
}

/// Shared by teaching continuity and the paged reader. The saved prefix is
/// authoritative even when the source conversation has since grown.
pub(crate) fn selected_records(user: &UserRuntime, workspace: &str, id: &str) -> Result<(Value, Vec<Value>), ToolError> {
    let store = user.learning_store_for_workspace(Some(workspace))?;
    let selected = store.linked_record_selections()?.into_iter()
        .find(|linked| linked.selection.chat_session_id == id).ok_or_else(service_error::missing)?;
    let history = session_recap::snapshot(user, id, Some(selected.selection.through_seq))?;
    let session = history.session.as_ref().ok_or_else(service_error::missing)?;
    if session.learning_workspace_id != selected.selection.source_learning_workspace_id {
        return Err(service_error::missing());
    }
    Ok(project_records(&history, store.linked_records(&selected.selection)?, "linked"))
}

fn project_records(history: &crate::session_event::SessionProjection, bundle: LinkedLearningRecords,
    origin: &str) -> (Value, Vec<Value>)
{
    project_session_records(history.session.as_ref().unwrap(), Some(history), bundle, origin)
}

fn project_session_records(session: &crate::AgentChatSession,
    history: Option<&crate::session_event::SessionProjection>, bundle: LinkedLearningRecords,
    origin: &str) -> (Value, Vec<Value>)
{
    let selection = &bundle.selection;
    // Snapshot-only histories have no event position or per-turn event times.
    let through = history.map(|_| selection.through_seq);
    let selected_turns: BTreeSet<_> = selection.turn_ids.iter().collect();
    let mut items = Vec::new();
    for turn in session.turns.iter().filter(|turn|selected_turns.contains(&turn.turn_id)) {
        let original = history.map(|history|learning_record_api::historical_turn(turn, history))
            .unwrap_or_else(||json!({"question_quote":turn.question_quote,
                "workspace_goal":turn.domain.scene.as_ref().and_then(|scene|scene.workspace_goal.as_ref()),
                "accepted_workspace_goal":turn.domain.workspace_goal_update}));
        let mut row = json!({"kind":"chat_turn","record_id":format!("chat_turn:{}",turn.turn_id),
            "turn_id":turn.turn_id,"chat_session_id":session.id,"published_book_ref":turn.published_book_ref,
            "user":turn.user_authored_text(),"user_display":turn.user,"answer":turn.outcome.as_ref().map(|outcome|&outcome.answer),
            "answer_view":turn.outcome.as_ref().and_then(|outcome|outcome.answer_view.as_ref()),
            "question_quote":original["question_quote"],"music_question":turn.music_question,"video_question":turn.video_question,
            "workspace_goal":original["workspace_goal"],"accepted_workspace_goal":original["accepted_workspace_goal"],
            "sources":turn.source_bindings,"status":turn.status,"occurred_at":original["occurred_at"],
            "updated_at":original["updated_at"],"accepted_event_seq":original["accepted_event_seq"],"event_seq":original["event_seq"]});
        add_origin(&mut row, selection, through, origin);
        items.push(row);
    }
    let facts: BTreeMap<_,_> = bundle.facts.iter().map(|fact|(fact.event_id.as_str(),fact)).collect();
    for fact in &bundle.facts {
        let mut row = json!({"kind":"fact","record_id":format!("fact:{}",fact.event_id),
            "chat_session_id":fact.binding.chat_session_id,"turn_id":fact.binding.turn_id,
            "occurred_at":fact.occurred_at,"published_book_ref":fact.payload["published_book_ref"],"record":fact});
        add_origin(&mut row, selection, through, origin);
        items.push(row);
    }
    for evidence in &bundle.evidence {
        let fact = facts.get(evidence.action_ref.as_str());
        let mut row = json!({"kind":"interpretation","record_id":format!("interpretation:{}",evidence.evidence_id),
            "chat_session_id":fact.map(|fact|&fact.binding.chat_session_id),"turn_id":fact.map(|fact|&fact.binding.turn_id),
            "published_book_ref":fact.map(|fact|&fact.payload["published_book_ref"]),"record":evidence});
        add_origin(&mut row, selection, through, origin);
        items.push(row);
    }
    for assessment in &bundle.assessments {
        let fact = facts.get(assessment.action_ref.as_str());
        let mut row = json!({"kind":"assessment","record_id":format!("assessment:{}",assessment.action_ref),
            "chat_session_id":fact.map(|fact|&fact.binding.chat_session_id),"turn_id":fact.map(|fact|&fact.binding.turn_id),
            "published_book_ref":fact.map(|fact|&fact.payload["published_book_ref"]),"record":assessment});
        add_origin(&mut row, selection, through, origin);
        items.push(row);
    }
    let mut selected = json!(selection);
    selected["through_seq"] = json!(through);
    (selected, deduplicate(items))
}

fn add_origin(row: &mut Value, selection: &LearningRecordSelection, through: Option<u64>, origin: &str) {
    row["origin"] = json!(origin);
    row["source_learning_workspace_id"] = json!(selection.source_learning_workspace_id);
    row["through_seq"] = json!(through);
    row["selection_chat_session_id"] = json!(selection.chat_session_id);
    row["historical"] = json!(true);
    row["current_mastery"] = json!(false);
    row["read_request"] = json!({"operation":"read","chat_session_id":selection.chat_session_id,
        "through_seq":through,"record_id":row["record_id"]});
}

fn deduplicate(rows: Vec<Value>) -> Vec<Value> {
    let mut seen = BTreeSet::new();
    rows.into_iter().filter(|row|seen.insert((row["source_learning_workspace_id"].as_str().map(str::to_owned),
        row["record_id"].as_str().unwrap_or_default().to_owned()))).collect()
}

fn matches_query(row: &Value, query: &str) -> bool {
    fn contains(value: &Value, query: &str) -> bool {
        match value {
            Value::String(text) => text.to_lowercase().contains(query),
            Value::Array(values) => values.iter().any(|value|contains(value, query)),
            Value::Object(values) => values.values().any(|value|contains(value, query)),
            _ => false,
        }
    }
    let query = query.to_lowercase();
    ["user","answer","answer_view","question_quote","sources","workspace_goal","accepted_workspace_goal","record","title"]
        .iter().any(|field|contains(&row[*field], &query))
}
