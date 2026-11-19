//! Reader-owned conversations opened from a delivered answer (ADR-0188).
use crate::{
    course_context::covered_follow_up_chats, new_agent_session, user_runtime::UserRuntime,
    user_storage_paths::error, AgentAssistantStatus, AgentChatSession, AgentChatTurn,
};
use base_schema::PublishedBookRef;
use read_tools::ToolError;
use runtime::orchestrator::AgentAnswerPart;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AnswerAnchor {
    pub start_part: usize,
    pub start_offset: usize,
    pub end_part: usize,
    pub end_offset: usize,
    pub excerpt: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CreateFollowUp {
    pub parent_session_id: String,
    pub parent_turn_id: String,
    pub anchor: AnswerAnchor,
    pub question: String,
    pub client_request_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct FollowUpOrigin {
    pub parent_session_id: String,
    pub parent_turn_id: String,
    pub anchor: AnswerAnchor,
    pub question: String,
    pub client_request_id: String,
    pub parent_question: String,
    pub parent_explanation: String,
    pub publication: Option<PublishedBookRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_revision: Option<String>,
}

fn missing() -> ToolError {
    error("FOLLOW_UP_UNAVAILABLE", "not_found", "原对话当前不可用")
}

fn covered() -> ToolError {
    error(
        "FOLLOW_UP_CLOSED_BOOK",
        "conflict",
        "请先回到阅读，再打开追问路径",
    )
}

pub(crate) fn byte_at_utf16(text: &str, offset: usize) -> Option<usize> {
    let mut units = 0;
    for (byte, ch) in text.char_indices() {
        if units == offset {
            return Some(byte);
        }
        units += ch.len_utf16();
        if units > offset {
            return None;
        }
    }
    (units == offset).then_some(text.len())
}

/// Validate against the exact persisted Markdown parts, including repeated words.
pub fn verified_excerpt(turn: &AgentChatTurn, anchor: &AnswerAnchor) -> bool {
    if turn.status != AgentAssistantStatus::Completed || anchor.excerpt.trim().is_empty() {
        return false;
    }
    let Some(outcome) = turn.outcome.as_ref().filter(|outcome| !outcome.incomplete) else {
        return false;
    };
    let parts = outcome
        .answer_view
        .as_ref()
        .map(|v| v.parts.as_slice())
        .unwrap_or_default();
    let mut rest = anchor.excerpt.as_str();
    let mut consume = |text: &str, index: usize| -> Option<()> {
        let start = if index == anchor.start_part {
            byte_at_utf16(text, anchor.start_offset)?
        } else {
            0
        };
        let end = if index == anchor.end_part {
            byte_at_utf16(text, anchor.end_offset)?
        } else {
            text.len()
        };
        rest = rest.strip_prefix(text.get(start..end)?)?;
        Some(())
    };
    if parts.is_empty() {
        if anchor.start_part != 0 || anchor.end_part != 0 {
            return false;
        }
        let Some(text) = outcome.answer.as_deref() else {
            return false;
        };
        return consume(text, 0).is_some() && rest.is_empty();
    }
    if anchor.start_part > anchor.end_part || anchor.end_part >= parts.len() {
        return false;
    }
    for index in anchor.start_part..=anchor.end_part {
        let text = match &parts[index] {
            AgentAnswerPart::Markdown { text } => text,
            AgentAnswerPart::SocraticQuestion { text, .. }
                if anchor.start_part == anchor.end_part =>
            {
                text
            }
            AgentAnswerPart::Sources { .. }
                if index > anchor.start_part && index < anchor.end_part =>
            {
                continue
            }
            _ => return false,
        };
        if consume(text, index).is_none() {
            return false;
        }
    }
    rest.is_empty()
}

struct ScopeTree<'a> {
    sessions: HashMap<&'a str, &'a AgentChatSession>,
    children: HashMap<&'a str, Vec<&'a AgentChatSession>>,
    masked: HashSet<String>,
}

impl<'a> ScopeTree<'a> {
    fn load(user: &'a UserRuntime, scope: Option<&str>, book: &str) -> Result<Self, ToolError> {
        let mut tree = Self {
            sessions: HashMap::new(),
            children: HashMap::new(),
            masked: covered_follow_up_chats(user, scope, book)?,
        };
        for session in user
            .agent_history
            .sessions
            .iter()
            .filter(|s| s.matches_scope(scope, book))
        {
            tree.sessions.insert(&session.id, session);
            if let Some(origin) = &session.follow_up_origin {
                tree.children
                    .entry(&origin.parent_session_id)
                    .or_default()
                    .push(session);
            }
        }
        Ok(tree)
    }

    fn root(&self, id: &str) -> Result<&'a AgentChatSession, ToolError> {
        let mut current = *self.sessions.get(id).ok_or_else(missing)?;
        loop {
            if self.masked.contains(&current.id) {
                return Err(covered());
            }
            let parent = current
                .follow_up_origin
                .as_ref()
                .and_then(|origin| self.sessions.get(origin.parent_session_id.as_str()));
            match parent {
                Some(parent) => current = parent,
                None => return Ok(current),
            }
        }
    }
}

fn preview(session: &AgentChatSession) -> Value {
    let last = session.turns.last();
    let origin = session.follow_up_origin.as_ref();
    let question = last
        .map(|turn| turn.user.as_str())
        .or_else(|| origin.map(|origin| origin.question.as_str()))
        .unwrap_or_default();
    let outcome = last
        .filter(|t| t.status == AgentAssistantStatus::Completed)
        .and_then(|t| t.outcome.as_ref())
        .filter(|o| !o.incomplete);
    let answer = outcome.map(|o| {
        if let Some(view) = o.answer_view.as_ref().filter(|v| !v.parts.is_empty()) {
            view.parts
                .iter()
                .filter_map(AgentAnswerPart::text)
                .flat_map(str::chars)
                .take(240)
                .collect::<String>()
        } else {
            o.answer
                .as_deref()
                .unwrap_or_default()
                .chars()
                .take(240)
                .collect()
        }
    });
    let status = match last.map(|t| t.status) {
        Some(AgentAssistantStatus::Completed) if outcome.is_some() => "ready",
        Some(AgentAssistantStatus::PendingAssistant) => "pending",
        Some(_) => "failed",
        None => "unsent",
    };
    json!({ "session_id":session.id, "title":session.title, "origin":origin,
        "question":question, "answer":answer, "status":status })
}

pub(crate) fn check_start(
    user: &UserRuntime,
    scope: Option<&str>,
    book: &str,
    session_id: &str,
) -> Result<(), ToolError> {
    ScopeTree::load(user, scope, book)?
        .root(session_id)
        .map(|_| ())
}

pub fn path(
    user: &UserRuntime,
    scope: Option<&str>,
    book: &str,
    session_id: &str,
) -> Result<Value, ToolError> {
    let tree = ScopeTree::load(user, scope, book)?;
    let mut pending = vec![tree.root(session_id)?];
    let mut nodes = Vec::new();
    while let Some(session) = pending.pop() {
        nodes.push(preview(session));
        if let Some(children) = tree.children.get(session.id.as_str()) {
            pending.extend(
                children
                    .iter()
                    .rev()
                    .copied()
                    .filter(|s| !tree.masked.contains(&s.id)),
            );
        }
    }
    Ok(json!({"nodes":nodes}))
}

/// Create only the child. Admission and explicit selection keep their original contracts.
pub fn create(
    user: &mut UserRuntime,
    scope: Option<&str>,
    book: &read_tools::Book,
    input: &CreateFollowUp,
    now: &str,
) -> Result<Value, ToolError> {
    if input.question.trim().is_empty()
        || input.client_request_id.trim().is_empty()
        || input.client_request_id.len() > 128
    {
        return Err(error(
            "FOLLOW_UP_INVALID",
            "validation",
            "请填写追问内容并使用有效的请求标识",
        ));
    }
    let tree = ScopeTree::load(user, scope, &book.base.book_id)?;
    if let Some((session, origin)) = user.agent_history.sessions.iter().find_map(|s| {
        s.follow_up_origin
            .as_ref()
            .filter(|o| o.client_request_id == input.client_request_id)
            .map(|o| (s, o))
    }) {
        tree.root(&session.id)?;
        if origin.parent_session_id != input.parent_session_id
            || origin.parent_turn_id != input.parent_turn_id
            || origin.question != input.question
            || origin.anchor != input.anchor
        {
            return Err(error(
                "FOLLOW_UP_REQUEST_CONFLICT",
                "conflict",
                "这次提交已经用于另一条追问",
            ));
        }
        return Ok(json!({"session_id":session.id}));
    }
    tree.root(&input.parent_session_id)?;
    let parent = tree.sessions[input.parent_session_id.as_str()];
    let turn = parent
        .turns
        .iter()
        .find(|t| t.turn_id == input.parent_turn_id)
        .ok_or_else(missing)?;
    // Old logs can contain inline LID markers that the chat view projects as source chips.
    let source_id = turn
        .published_book_ref
        .as_ref()
        .map(|p| p.book_id.as_str())
        .unwrap_or(&parent.book_id);
    let legacy = turn
        .outcome
        .as_ref()
        .filter(|_| source_id == book.base.book_id)
        .and_then(|outcome| crate::legacy_answer_projection(&turn.turn_id, outcome, book));
    let projected = legacy.map(|legacy| {
        let mut projected = turn.clone();
        let outcome = projected.outcome.as_mut().unwrap();
        outcome.answer = Some(legacy.answer);
        outcome.answer_view = Some(legacy.view);
        projected
    });
    let turn = projected.as_ref().unwrap_or(turn);
    if !verified_excerpt(turn, &input.anchor) {
        return Err(error(
            "FOLLOW_UP_QUOTE_CHANGED",
            "conflict",
            "原回答中的这段文字已变化，请重新选取",
        ));
    }
    let source_revision = crate::teaching::bound_user(user, scope, &turn.turn_id)?
        .filter(|event| {
            event.binding.chat_session_id == parent.id && event.binding.source_id == source_id
        })
        .map(|event| event.binding.source_revision)
        .or_else(|| {
            turn.source_bindings
                .iter()
                .filter(|binding| {
                    binding.book_id == source_id
                        && binding.valid_evidence()
                        && binding.publication_id.as_deref()
                            == turn
                                .published_book_ref
                                .as_ref()
                                .map(|p| p.publication_id.as_str())
                })
                .find_map(|binding| binding.source_revision.clone())
        });
    let origin = FollowUpOrigin {
        parent_session_id: parent.id.clone(),
        parent_turn_id: turn.turn_id.clone(),
        parent_question: turn.user.clone(),
        publication: turn.published_book_ref.clone(),
        parent_explanation: crate::follow_up_input::explanation(turn, &input.anchor),
        source_revision,
        anchor: input.anchor.clone(),
        question: input.question.clone(),
        client_request_id: input.client_request_id.clone(),
    };
    let source = origin
        .publication
        .as_ref()
        .map(|r| r.book_id.as_str())
        .unwrap_or(&parent.book_id);
    let mut child = new_agent_session(source, now, 0);
    // The child JSONL and run admission can be acknowledged separately.
    child.id = format!("follow-up_{}", input.client_request_id);
    child.learning_workspace_id = parent.learning_workspace_id.clone();
    child.title = input.question.trim().chars().take(60).collect();
    child.follow_up_origin = Some(origin);
    let id = child.id.clone();
    if let Some(store) = &mut user.session_store {
        store.create(&mut user.agent_history, child)?;
    } else {
        user.agent_history.sessions.push(child);
        if let Err(err) = crate::save_agent_history_path(&user.history_path, &user.agent_history) {
            user.agent_history.sessions.pop();
            return Err(err);
        }
    }
    Ok(json!({"session_id":id}))
}

pub(crate) fn route(
    state: &mut crate::AppState,
    method: &str,
    body: &str,
    requested: Option<&str>,
    now: &str,
) -> crate::Reply {
    if !matches!(method, "GET" | "POST") {
        return crate::agent_history_method_not_allowed();
    }
    let scope = state.workspace.learning_workspace_id.as_deref();
    let book = &state.workspace.book.base.book_id;
    let result = (|| {
        let create_input = if method == "POST" {
            Some(
                serde_json::from_str::<CreateFollowUp>(body)
                    .map_err(|_| error("FOLLOW_UP_INVALID", "validation", "追问来源不完整"))?,
            )
        } else {
            None
        };
        let request_id = create_input
            .as_ref()
            .map(|request| request.parent_session_id.as_str())
            .or(requested)
            .ok_or_else(missing)?;
        if let Some(selected) = state
            .workspace
            .selected_chat
            .as_deref()
            .filter(|s| *s != request_id)
        {
            check_start(&state.user, scope, book, selected)?;
        }
        match create_input.as_ref() {
            Some(input) => create(&mut state.user, scope, &state.workspace.book, input, now),
            None => path(&state.user, scope, book, request_id),
        }
    })();
    match result {
        Ok(value) => crate::ok_json(&value),
        Err(err) => crate::err_reply(&err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session_event::tests::{legacy_session, sample_events};
    use crate::session_log::SessionLog;
    use crate::session_store::{SessionPaths, SessionStore};
    use runtime::orchestrator::AgentAnswerView;

    fn two_mentions() -> AgentChatSession {
        let mut session = legacy_session();
        let outcome = session.turns[0].outcome.as_mut().unwrap();
        outcome.answer_view = Some(AgentAnswerView {
            parts: vec![
                AgentAnswerPart::Markdown {
                    text: "🔎对照组。".into(),
                },
                AgentAnswerPart::Sources {
                    source_ref_ids: vec![],
                },
                AgentAnswerPart::Markdown {
                    text: "同一个对照组照常学习。".into(),
                },
            ],
            sources: vec![],
        });
        session
    }

    fn request(parent: &AgentChatSession) -> CreateFollowUp {
        CreateFollowUp {
            parent_session_id: parent.id.clone(),
            parent_turn_id: parent.turns[0].turn_id.clone(),
            question: "这个组也上课吗？".into(),
            client_request_id: "one-follow-up".into(),
            anchor: AnswerAnchor {
                start_part: 2,
                start_offset: 3,
                end_part: 2,
                end_offset: 6,
                excerpt: "对照组".into(),
            },
        }
    }

    #[test]
    fn validates_repeated_and_multiblock_utf16_quotes() {
        let session = two_mentions();
        let turn = &session.turns[0];
        let mut a = request(&session).anchor;
        assert!(verified_excerpt(turn, &a));
        a.start_offset = 2;
        assert!(!verified_excerpt(turn, &a));
        a = AnswerAnchor {
            start_part: 0,
            start_offset: 2,
            end_part: 2,
            end_offset: 3,
            excerpt: "对照组。同一个".into(),
        };
        assert!(verified_excerpt(turn, &a));
        a.start_offset = 1;
        assert!(!verified_excerpt(turn, &a));
        let mut changed = turn.clone();
        changed
            .outcome
            .as_mut()
            .unwrap()
            .answer_view
            .as_mut()
            .unwrap()
            .parts[1] = AgentAnswerPart::Presentation {
            presentation_id: "different-surface".into(),
            revision: 1,
        };
        a.start_offset = 2;
        assert!(!verified_excerpt(&changed, &a));
    }

    #[test]
    fn path_from_grandchild_keeps_siblings_and_legacy_rendered_quotes() {
        let mut state = crate::tests::state_named("follow-up-family");
        let book = state.workspace.book.clone();
        let mut parent = two_mentions();
        parent.book_id = book.base.book_id.clone();
        let reply = parent.turns[0].outcome.as_mut().unwrap();
        reply.answer = Some("先看材料 [LID: 1.1] 再看对照组。".into());
        reply.answer_view = None;
        let mut req = request(&parent);
        req.anchor = AnswerAnchor {
            start_part: 2,
            start_offset: 3,
            end_part: 2,
            end_offset: 6,
            excerpt: "对照组".into(),
        };
        state.user.agent_history.sessions.push(parent.clone());
        let first = create(&mut state.user, None, &book, &req, "10").unwrap()["session_id"]
            .as_str()
            .unwrap()
            .to_owned();
        let mut child_reply = two_mentions().turns.remove(0);
        child_reply.turn_id = "second-discussion".into();
        state
            .user
            .agent_history
            .sessions
            .iter_mut()
            .find(|s| s.id == first)
            .unwrap()
            .turns
            .push(child_reply);
        req.client_request_id = "sibling".into();
        req.question = "这两组在哪上课？".into();
        let sibling = create(&mut state.user, None, &book, &req, "11").unwrap();
        req.parent_session_id = first.clone();
        req.parent_turn_id = "second-discussion".into();
        req.client_request_id = "grandchild".into();
        let grandchild = create(&mut state.user, None, &book, &req, "12").unwrap();
        let unrelated = new_agent_session(&book.base.book_id, "13", 0);
        state.user.agent_history.sessions.push(unrelated);
        let tree = path(
            &state.user,
            None,
            &book.base.book_id,
            grandchild["session_id"].as_str().unwrap(),
        )
        .unwrap();
        let ids: Vec<_> = tree["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n["session_id"].as_str().unwrap())
            .collect();
        assert_eq!(
            ids,
            [
                parent.id,
                first,
                grandchild["session_id"].as_str().unwrap().into(),
                sibling["session_id"].as_str().unwrap().into()
            ]
        );
    }

    #[test]
    fn session_store_follow_ups_preserve_origin_recover_retry_and_orphan() {
        let dir = tempfile::tempdir().unwrap();
        let paths = SessionPaths::from_history(&dir.path().join("agent-history.json"));
        std::fs::create_dir_all(&paths.root).unwrap();
        let state = crate::tests::state_named("follow-up-persist");
        let material = state.workspace.book.clone();
        let mut parent = two_mentions();
        parent.book_id = material.base.book_id.clone();
        let mut log = SessionLog::open(paths.session(&parent.id)).unwrap();
        for event in sample_events(&parent) {
            log.append(event).unwrap();
        }
        drop(log);
        let (store, history) = SessionStore::open(paths).unwrap();
        let mut user = state.user;
        user.session_store = Some(store);
        user.agent_history = history;
        let mut req = request(&parent);
        let book = &parent.book_id;
        let selected = user.agent_history.active_by_book.clone();
        let first = create(&mut user, None, &material, &req, "10").unwrap();
        let child_id = first["session_id"].as_str().unwrap();
        assert_eq!(user.agent_history.active_by_book, selected);
        let branch = path(&user, None, book, child_id).unwrap();
        assert_eq!(branch["nodes"].as_array().unwrap().len(), 2);
        assert_eq!(branch["nodes"][1]["origin"]["anchor"]["start_part"], 2);
        assert_eq!(
            branch["nodes"][1]["origin"]["parent_question"],
            parent.turns[0].user
        );
        assert_eq!(branch["nodes"][1]["status"], "unsent");
        // The initial append reached disk but the creating caller did not receive
        // confirmation. Reconcile the same file before admitting the first turn.
        user.session_store.as_mut().unwrap().logs.remove(child_id);
        user.agent_history.sessions.retain(|s| s.id != child_id);
        assert_eq!(
            create(&mut user, None, &material, &req, "later").unwrap(),
            first
        );
        assert!(path(&user, Some("another-workspace"), book, child_id).is_err());
        let (store, history) =
            SessionStore::open(user.session_store.take().unwrap().paths).unwrap();
        user.session_store = Some(store);
        user.agent_history = history;
        assert_eq!(
            create(&mut user, None, &material, &req, "11").unwrap(),
            first
        );
        assert_eq!(user.agent_history.sessions.len(), 2);
        assert_eq!(path(&user, None, book, child_id).unwrap(), branch);
        req.question.push('？');
        assert_eq!(
            create(&mut user, None, &material, &req, "12")
                .unwrap_err()
                .error_code,
            "FOLLOW_UP_REQUEST_CONFLICT"
        );
        req.client_request_id = "bad-offset".into();
        req.anchor.end_offset -= 1;
        assert_eq!(
            create(&mut user, None, &material, &req, "13")
                .unwrap_err()
                .error_code,
            "FOLLOW_UP_QUOTE_CHANGED"
        );
        assert_eq!(user.agent_history.sessions.len(), 2);
        user.session_store
            .as_mut()
            .unwrap()
            .delete(&mut user.agent_history, &parent.id)
            .unwrap();
        let survivor = path(&user, None, book, child_id).unwrap();
        assert_eq!(survivor["nodes"].as_array().unwrap().len(), 1);
        assert_eq!(
            survivor["nodes"][0]["origin"]["parent_session_id"],
            parent.id
        );
    }
}
