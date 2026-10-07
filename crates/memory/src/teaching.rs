//! Append-only teaching facts. Payloads are exact deliveries/observations, never mastery.
use crate::learning::{storage, LearningStore};
use read_tools::ToolError;
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TeachingBinding {
    pub tutor_session_id: String,
    pub session_revision: u32,
    pub control_revision: u32,
    pub source_id: String,
    pub source_revision: String,
    pub map_revision: String,
    pub chat_session_id: String,
    pub turn_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TeachingFact {
    TurnBound,
    MoveSelected,
    MessageDelivered,
    Displayed,
    LearnerAction,
    HelpDelivered,
    HelpDisplayed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TeachingEvent {
    pub event_id: String,
    pub binding: TeachingBinding,
    pub kind: TeachingFact,
    pub causal_refs: Vec<String>,
    pub payload: Value,
    pub occurred_at: String,
}

pub fn invalid(message: impl Into<String>) -> ToolError {
    ToolError {
        error_code: "TEACHING_INVALID".into(),
        category: "validation".into(),
        message: message.into(),
    }
}

impl LearningStore {
    pub fn teaching_activity_events(
        &self,
        delivery: &str,
    ) -> Result<Vec<TeachingEvent>, ToolError> {
        let mut query = self.connection.prepare("SELECT event FROM teaching_trace WHERE json_extract(event,'$.payload.delivery_ref')=? ORDER BY seq").map_err(storage)?;
        let rows = query
            .query_map([delivery], |r| r.get::<_, String>(0))
            .map_err(storage)?;
        rows.map(|row| serde_json::from_str(&row.map_err(storage)?).map_err(storage))
            .collect()
    }
    pub fn teaching_recent(
        &self,
        session: &str,
        limit: u32,
    ) -> Result<Vec<TeachingEvent>, ToolError> {
        let mut query = self.connection.prepare("SELECT event FROM teaching_trace WHERE session_id=? AND json_extract(event,'$.kind') IN ('message_delivered','learner_action','help_displayed') ORDER BY seq DESC LIMIT ?").map_err(storage)?;
        let rows = query
            .query_map(params![session, limit.min(20)], |r| r.get::<_, String>(0))
            .map_err(storage)?;
        rows.map(|row| serde_json::from_str(&row.map_err(storage)?).map_err(storage))
            .collect()
    }
    pub fn teaching_deliveries(
        &self,
        chat: &str,
        turn: &str,
    ) -> Result<Vec<TeachingEvent>, ToolError> {
        let mut query = self.connection.prepare("SELECT event FROM teaching_trace WHERE json_extract(event,'$.binding.chat_session_id')=? AND json_extract(event,'$.binding.turn_id')=? AND json_extract(event,'$.kind')='message_delivered' ORDER BY seq").map_err(storage)?;
        let rows = query
            .query_map(params![chat, turn], |r| r.get::<_, String>(0))
            .map_err(storage)?;
        rows.map(|row| serde_json::from_str(&row.map_err(storage)?).map_err(storage))
            .collect()
    }
    pub fn teaching_event(&self, id: &str) -> Result<TeachingEvent, ToolError> {
        let json: Option<String> = self
            .connection
            .query_row(
                "SELECT event FROM teaching_trace WHERE event_id=?",
                [id],
                |r| r.get(0),
            )
            .optional()
            .map_err(storage)?;
        serde_json::from_str(&json.ok_or_else(|| invalid("教学引用不存在"))?).map_err(storage)
    }

    /// Oldest-first page with a stable cursor; callers never need to load all private history.
    pub fn teaching_events(
        &self,
        session: &str,
        after: u64,
        limit: u32,
    ) -> Result<Vec<(u64, TeachingEvent)>, ToolError> {
        let mut query = self.connection.prepare(
            "SELECT seq,event FROM teaching_trace WHERE session_id=? AND seq>? ORDER BY seq LIMIT ?"
        ).map_err(storage)?;
        let rows = query
            .query_map(params![session, after, limit.min(100)], |r| {
                Ok((r.get::<_, u64>(0)?, r.get::<_, String>(1)?))
            })
            .map_err(storage)?;
        rows.map(|row| {
            let (seq, json) = row.map_err(storage)?;
            Ok((seq, serde_json::from_str(&json).map_err(storage)?))
        })
        .collect()
    }

    pub fn teaching_turn_events(
        &self,
        session: &str,
        turn: &str,
    ) -> Result<Vec<TeachingEvent>, ToolError> {
        let mut query = self.connection.prepare("SELECT event FROM teaching_trace WHERE session_id=? AND json_extract(event,'$.binding.turn_id')=? ORDER BY seq").map_err(storage)?;
        let rows = query
            .query_map(params![session, turn], |r| r.get::<_, String>(0))
            .map_err(storage)?;
        rows.map(|row| serde_json::from_str(&row.map_err(storage)?).map_err(storage))
            .collect()
    }

    pub fn append_teaching(&mut self, event: &TeachingEvent) -> Result<TeachingEvent, ToolError> {
        if event.event_id.trim().is_empty() {
            return Err(invalid("缺少教学事件身份"));
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let previous: Option<String> = transaction
            .query_row(
                "SELECT event FROM teaching_trace WHERE event_id=?",
                [&event.event_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(storage)?;
        if let Some(json) = previous {
            let old: TeachingEvent = serde_json::from_str(&json).map_err(storage)?;
            let mut retry = event.clone();
            retry.occurred_at = old.occurred_at.clone();
            if old != retry {
                return Err(invalid("同一教学操作不能改写已保存事实"));
            }
            return Ok(old);
        }
        // Causal edges always stay inside the same private teaching session.
        for id in &event.causal_refs {
            let owner: Option<String> = transaction
                .query_row(
                    "SELECT session_id FROM teaching_trace WHERE event_id=?",
                    [id],
                    |r| r.get(0),
                )
                .optional()
                .map_err(storage)?;
            if owner.as_deref() != Some(&event.binding.tutor_session_id) {
                return Err(invalid("教学因果引用不属于此会话"));
            }
        }
        transaction
            .execute(
                "INSERT INTO teaching_trace(event_id,session_id,event) VALUES (?,?,?)",
                params![
                    event.event_id,
                    event.binding.tutor_session_id,
                    serde_json::to_string(event).map_err(storage)?
                ],
            )
            .map_err(storage)?;
        transaction.commit().map_err(storage)?;
        Ok(event.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn teaching_facts_are_immutable_retryable_and_survive_projection_rebuild() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("learning.db");
        let mut store = LearningStore::open(&path, false).unwrap();
        let mut event = TeachingEvent {
            event_id: "delivery".into(),
            binding: TeachingBinding {
                tutor_session_id: "tutor".into(),
                session_revision: 1,
                control_revision: 1,
                source_id: "book".into(),
                source_revision: "old".into(),
                map_revision: "v1".into(),
                chat_session_id: "chat1".into(),
                turn_id: "turn1".into(),
            },
            kind: TeachingFact::MessageDelivered,
            causal_refs: vec![],
            payload: serde_json::json!({"text":"original"}),
            occurred_at: "now".into(),
        };
        let first = store.append_teaching(&event).unwrap();
        event.occurred_at = "later".into();
        assert_eq!(store.append_teaching(&event).unwrap(), first);
        event.payload = serde_json::json!({"text":"changed"});
        assert!(store.append_teaching(&event).is_err());
        event.event_id = "next".into();
        event.causal_refs = vec!["delivery".into()];
        store.connection.execute_batch("CREATE TRIGGER teaching_fail BEFORE INSERT ON teaching_trace BEGIN SELECT RAISE(ABORT,'write failed'); END;").unwrap();
        assert!(store.append_teaching(&event).is_err());
        assert!(store.teaching_event("next").is_err());
        store
            .connection
            .execute_batch("DROP TRIGGER teaching_fail")
            .unwrap();
        store.rebuild().unwrap();
        drop(store);
        let store = LearningStore::open(&path, false).unwrap();
        assert_eq!(store.teaching_event("delivery").unwrap(), first);
        assert_eq!(store.teaching_events("tutor", 0, 100).unwrap().len(), 1);
    }
}
