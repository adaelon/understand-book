//! Per-reader session files and separately persisted chat selection (ADR-0152).
use crate::session_event::{EventBody, SessionCreated};
use crate::session_log::SessionLog;
use crate::*;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};

#[derive(Debug, Clone)]
pub(crate) struct SessionPaths {
    pub root: PathBuf,
    pub selection: PathBuf,
    pub presentations: PathBuf,
}
impl SessionPaths {
    pub fn from_history(history: &Path) -> Self {
        let parent = history.parent().unwrap_or_else(|| Path::new("."));
        Self {
            root: parent.join("agent-sessions"),
            selection: parent.join("agent-chat-selection.json"),
            presentations: history.with_extension("presentations"),
        }
    }
    pub fn session(&self, id: &str) -> PathBuf {
        self.root
            .join(format!("s_{}.jsonl", URL_SAFE_NO_PAD.encode(id.as_bytes())))
    }
}

pub(crate) struct SessionStore {
    pub paths: SessionPaths,
    pub logs: BTreeMap<String, SessionLog>,
}

#[derive(Serialize, Deserialize, Default)]
struct Selection {
    active_by_book: BTreeMap<String, String>,
}

/// Persistent chats always start from JSONL. Old snapshots remain untouched.
pub(crate) fn load_chat_storage(
    path: &Option<PathBuf>,
) -> Result<(AgentHistory, Option<SessionStore>), ToolError> {
    if let Some(path) = path {
        let paths = SessionPaths::from_history(path);
        let (store, history) = SessionStore::open(paths)?;
        return Ok((history, Some(store)));
    }
    Ok((AgentHistory::default(), None))
}

impl SessionStore {
    pub fn ensure_book(&mut self, history: &mut AgentHistory, book: &str, now: &str) -> Result<(), ToolError> {
        let id = history.active_by_book.get(book).cloned()
            .or_else(|| history.sessions.iter().find(|s| s.book_id == book).map(|s| s.id.clone()));
        let id = match id {
            Some(id) => id,
            None => {
                let session = new_agent_session(book, now, history.sessions.len());
                let id = session.id.clone();
                self.create(history, session)?;
                id
            }
        };
        self.select(history, book, &id)
    }
    pub fn settle(&mut self, history: &mut AgentHistory, id: &str) -> Result<(), ToolError> {
        let log = self.logs.get_mut(id).ok_or_else(|| agent_history_internal("session log missing"))?;
        if let Some(event) = log.pending_event() {
            log.append(event)?;
            let session = history.sessions.iter_mut().find(|s| s.id == id)
                .ok_or_else(|| agent_history_internal("session projection missing"))?;
            *session = log.projection.session.as_ref().unwrap().clone();
        }
        Ok(())
    }
    pub fn append(&mut self, history: &mut AgentHistory, id: &str, at: &str,
        turn: Option<&str>, body: EventBody) -> Result<(), ToolError> {
        self.settle(history, id)?;
        let log = self.logs.get_mut(id).ok_or_else(|| agent_history_internal("session log missing"))?;
        let mut event = log.next_event(at, turn, body);
        log.append(event.clone())?;
        let index = history.sessions.iter().position(|s| s.id == id)
            .ok_or_else(|| agent_history_internal("session projection missing"))?;
        if matches!(event.body, EventBody::TurnPrepared { .. }) {
            history.sessions[index].updated_at = event.at;
        } else {
            // Apply the same sanitized delta without copying the existing transcript.
            crate::session_log::sanitize(&mut event);
            let mut projection = crate::session_event::SessionProjection {
                session: Some(history.sessions.remove(index)), ..Default::default()
            };
            projection.apply(event);
            history.sessions.insert(index, projection.session.unwrap());
        }
        Ok(())
    }

    pub fn accept(&mut self, history: &mut AgentHistory, mut session: AgentChatSession,
        at: &str) -> Result<(), ToolError> {
        self.settle(history, &session.id)?;
        let log = self.logs.get(&session.id).ok_or_else(|| agent_history_internal("session log missing"))?;
        let seq = log.projection.through_seq;
        let mut turn = session.turns.pop().ok_or_else(|| agent_history_internal("pending turn missing"))?;
        let input = turn.admission_input.take().map(|f| f.map_messages(|_| {
            crate::session_event::HistoryPosition::Committed { history_through_seq: seq }
        }));
        let goals = changed_goals(log.projection.session.as_ref().unwrap(), &session.goals);
        self.append(history, &session.id, at, Some(&turn.turn_id.clone()),
            EventBody::TurnAccepted(crate::session_event::TurnAccepted {
                turn, history_through_seq: seq, input, goals, title: session.title,
            }))
    }

    pub fn open(paths: SessionPaths) -> Result<(Self, AgentHistory), ToolError> {
        memory::ReaderPrivateStorageGate::create_dir_all(&paths.root)
            .map_err(|e| agent_history_internal(e.to_string()))?;
        let mut logs = BTreeMap::new();
        for entry in
            std::fs::read_dir(&paths.root).map_err(|e| agent_history_internal(e.to_string()))?
        {
            let path = entry
                .map_err(|e| agent_history_internal(e.to_string()))?
                .path();
            if path.extension().and_then(|s| s.to_str()) != Some("jsonl") {
                continue;
            }
            let log = SessionLog::open(path.clone())?;
            // An interrupted first append has no committed session yet. Its
            // bytes remain available to the original writer/maintenance.
            let Some(session) = &log.projection.session else {
                continue;
            };
            if paths.session(&session.id) != path || logs.contains_key(&session.id) {
                return Err(agent_history_load_error(
                    &path,
                    "session_identity",
                    "filename does not match logical session ID",
                ));
            }
            logs.insert(session.id.clone(), log);
        }
        recover_interrupted_agent_history_commit(&paths.selection)?;
        let selection = match std::fs::read(&paths.selection) {
            Ok(bytes) => serde_json::from_slice::<Selection>(&bytes).map_err(|e| {
                agent_history_load_error(&paths.selection, "selection", e.to_string())
            })?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Selection::default(),
            Err(e) => {
                return Err(agent_history_load_error(
                    &paths.selection,
                    "selection",
                    e.to_string(),
                ))
            }
        };
        let mut history = AgentHistory {
            sessions: logs
                .values()
                .map(|l| l.projection.session.as_ref().unwrap().clone())
                .collect(),
            active_by_book: selection.active_by_book,
            ..Default::default()
        };
        history
            .sessions
            .sort_by(|a, b| a.created_at.cmp(&b.created_at).then(a.id.cmp(&b.id)));
        repair_selection(&mut history);
        Ok((Self { paths, logs }, history))
    }

    pub fn create(
        &mut self,
        history: &mut AgentHistory,
        session: AgentChatSession,
    ) -> Result<(), ToolError> {
        if self.logs.contains_key(&session.id)
            || !session.turns.is_empty()
            || !session.goals.is_empty()
            || session.compaction_checkpoint.is_some()
        {
            return Err(agent_history_internal(
                "create requires a new empty session",
            ));
        }
        let mut log = SessionLog::open(self.paths.session(&session.id))?;
        if log.projection.session.is_some() {
            return Err(agent_history_internal("session already exists"));
        }
        let event = log.next_event(
            &session.created_at,
            None,
            EventBody::SessionCreated(SessionCreated {
                session_id: session.id.clone(),
                book_id: session.book_id.clone(),
                title: session.title.clone(),
                created_at: session.created_at.clone(),
                messages: session.messages.clone(),
            }),
        );
        log.append(event)?;
        history
            .sessions
            .push(log.projection.session.as_ref().unwrap().clone());
        self.logs.insert(session.id, log);
        Ok(())
    }

    pub fn select(
        &self,
        history: &mut AgentHistory,
        book: &str,
        id: &str,
    ) -> Result<(), ToolError> {
        if !history
            .sessions
            .iter()
            .any(|s| s.id == id && s.book_id == book)
        {
            return Err(agent_history_internal(
                "selected session does not belong to this book",
            ));
        }
        let mut selection = history.active_by_book.clone();
        selection.insert(book.into(), id.into());
        self.save_selection(&selection)?;
        history.active_by_book = selection;
        Ok(())
    }

    pub fn delete(&mut self, history: &mut AgentHistory, id: &str) -> Result<(), ToolError> {
        let session = history
            .sessions
            .iter()
            .find(|s| s.id == id)
            .ok_or_else(|| agent_history_internal("session not found"))?;
        if session
            .turns
            .iter()
            .any(|t| t.status == AgentAssistantStatus::PendingAssistant)
        {
            return Err(user_storage_paths::error(
                "CHAT_BUSY",
                "conflict",
                "The chat has a pending run",
            ));
        }
        std::fs::remove_file(self.paths.session(id))
            .map_err(|e| agent_history_internal(e.to_string()))?;
        self.logs.remove(id);
        history.sessions.retain(|s| s.id != id);
        history.pending_confirmations.remove(id);
        history.pending_memory_ops.remove(id);
        history.pending_governance_mutations.remove(id);
        repair_selection(history);
        // Deletion is authoritative in the session directory. A failed metadata
        // save must not resurrect it in memory; restart prunes stale selections.
        self.save_selection(&history.active_by_book)
    }

    fn save_selection(&self, active_by_book: &BTreeMap<String, String>) -> Result<(), ToolError> {
        let path = &self.paths.selection;
        memory::ReaderPrivateStorageGate::enforce(path)?;
        recover_interrupted_agent_history_commit(path)?;
        let bytes = serde_json::to_vec(&Selection {
            active_by_book: active_by_book.clone(),
        })
        .map_err(|e| agent_history_internal(e.to_string()))?;
        let temp = agent_history_temporary_path(path);
        let backup = agent_history_backup_path(path);
        let write = || -> std::io::Result<()> {
            let mut file = std::fs::File::create(&temp)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            if backup.exists() {
                std::fs::remove_file(&backup)?;
            }
            if path.exists() {
                std::fs::rename(path, &backup)?;
            }
            if let Err(e) = std::fs::rename(&temp, path) {
                if backup.exists() {
                    let _ = std::fs::rename(&backup, path);
                }
                return Err(e);
            }
            Ok(())
        };
        write().map_err(|e| agent_history_internal(format!("save chat selection: {e}")))
    }
}

pub(crate) fn changed_goals(before: &AgentChatSession, after: &[runtime::goal::ResidentGoal]) -> Vec<runtime::goal::ResidentGoal> {
    after.iter().filter(|g| !before.goals.iter().any(|old| old == *g)).cloned().collect()
}

fn repair_selection(history: &mut AgentHistory) {
    history.active_by_book.retain(|book, id| {
        history
            .sessions
            .iter()
            .any(|s| &s.id == id && &s.book_id == book)
    });
    let mut ordered = history.sessions.iter().collect::<Vec<_>>();
    ordered.sort_by(|a, b| {
        // Startup chats use "server-start". Real timestamps sort after that
        // label, with offset awareness.
        let parsed = |s: &str| {
            time::OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339).ok()
                .or_else(|| s.parse::<i64>().ok().and_then(|n| time::OffsetDateTime::from_unix_timestamp(n).ok()))
        };
        parsed(&b.updated_at)
            .cmp(&parsed(&a.updated_at))
            .then(b.updated_at.cmp(&a.updated_at))
            .then(b.id.cmp(&a.id))
    });
    for s in ordered {
        history
            .active_by_book
            .entry(s.book_id.clone())
            .or_insert_with(|| s.id.clone());
    }
}

pub(crate) fn route_new(state: &mut AppState, book: &str, now: &str) -> Reply {
    let mut session = new_agent_session(book, now, 0);
    session.id = format!("chat-{}", uuid::Uuid::now_v7());
    let id = session.id.clone();
    let store = state.user.session_store.as_mut().unwrap();
    if let Err(e) = store.create(&mut state.user.agent_history, session) {
        return err_reply(&e);
    }
    let reply = route_select(state, book, &id);
    if reply.status != 200 {
        return reply;
    }
    let history: Value = serde_json::from_str(&reply.body).unwrap();
    ok_json(&json!({"ok": true, "history": history}))
}

pub(crate) fn route_select(state: &mut AppState, book: &str, id: &str) -> Reply {
    let Some(session) = state
        .user
        .agent_history
        .sessions
        .iter()
        .find(|s| s.book_id == book && s.id == id)
    else {
        return validation(
            "INVALID_RANGE",
            "agent history session 不属于当前 book 或不存在",
        );
    };
    let messages = session.messages.clone();
    if let Err(e) =
        state
            .user
            .session_store
            .as_ref()
            .unwrap()
            .select(&mut state.user.agent_history, book, id)
    {
        return err_reply(&e);
    }
    state.workspace.select_chat(id.into(), messages);
    match agent_history_response(&state.user.agent_history, &state.workspace.book, Some(id)) {
        Ok(response) => ok_json(&response),
        Err(e) => err_reply(&e),
    }
}

pub(crate) fn route_delete(state: &mut AppState, book: &str, id: &str, now: &str) -> Reply {
    if !state
        .user
        .agent_history
        .sessions
        .iter()
        .any(|s| s.book_id == book && s.id == id)
    {
        return validation(
            "INVALID_RANGE",
            "agent history session 不属于当前 book 或不存在",
        );
    }
    let result = state
        .user
        .session_store
        .as_mut()
        .unwrap()
        .delete(&mut state.user.agent_history, id);
    // If removal succeeded but selection persistence failed, clear the deleted
    // workspace reference as well. The directory remains authoritative.
    if state.workspace.selected_chat.as_deref() == Some(id)
        && !state.user.agent_history.sessions.iter().any(|s| s.id == id)
    {
        state.workspace.selected_chat = None;
        state.workspace.messages.clear();
        state.workspace.invalidate();
    }
    if let Err(e) = result {
        return err_reply(&e);
    }
    let selected = state
        .workspace
        .selected_chat
        .clone()
        .filter(|id| {
            state
                .user
                .agent_history
                .sessions
                .iter()
                .any(|s| s.id == *id && s.book_id == book)
        })
        .or_else(|| state.user.agent_history.active_by_book.get(book).cloned());
    if let Some(selected) = selected {
        route_select(state, book, &selected)
    } else {
        let created = route_new(state, book, now);
        if created.status != 200 {
            return created;
        }
        let value: Value = serde_json::from_str(&created.body).unwrap();
        ok_json(&value["history"])
    }
}

#[cfg(test)]
mod tests;
