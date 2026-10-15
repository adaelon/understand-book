//! Private Tutor control and explicit session lifecycle. Events own history;
//! the current projection can be rebuilt without reading any chat messages.
use read_tools::ToolError;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};
use ts_rs::TS;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct TutorControl {
    pub enabled: bool,
    pub revision: u32,
    pub current_tutor_session_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
#[serde(rename_all = "snake_case")]
pub enum TutorSessionStatus {
    Active,
    Paused,
    Ended,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
#[serde(rename_all = "snake_case")]
pub enum TutorSessionMode {
    DirectExplanation,
    GuidedInquiry,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
#[serde(rename_all = "snake_case")]
pub enum TutorMaterialRole {
    Primary,
    Supporting,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct TutorMaterial {
    pub source_id: String,
    pub scope_refs: Vec<String>,
    pub role: TutorMaterialRole,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct TutorFocus {
    pub interpretation: String,
    pub target_object_refs: Vec<String>,
    pub capability_targets: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct TutorSession {
    pub id: String,
    pub revision: u32,
    pub status: TutorSessionStatus,
    pub user_intent: String,
    pub explicit_constraints: Vec<String>,
    pub current_focus: TutorFocus,
    pub material_scope: Vec<TutorMaterial>,
    pub default_teaching_intent: Option<TutorSessionMode>,
    pub path_instance_ref: Option<String>,
    pub progress_ref: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct TutorState {
    pub control: TutorControl,
    pub sessions: BTreeMap<String, TutorSession>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TutorAction {
    SetEnabled {
        enabled: bool,
    },
    Start {
        user_intent: String,
        explicit_constraints: Vec<String>,
        material_scope: Vec<TutorMaterial>,
        default_teaching_intent: Option<TutorSessionMode>,
    },
    Pause {
        session_id: String,
    },
    Resume {
        session_id: String,
    },
    End {
        session_id: String,
    },
    SetMode {
        session_id: String,
        mode: Option<TutorSessionMode>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct TutorMutation {
    pub operation_id: String,
    pub expected_revision: u32,
    pub action: TutorAction,
}

fn failure(code: &str, category: &str, message: impl ToString) -> ToolError {
    ToolError {
        error_code: code.into(),
        category: category.into(),
        message: message.to_string(),
    }
}
pub(crate) fn storage(error: impl ToString) -> ToolError {
    failure("LEARNING_STORAGE_UNAVAILABLE", "unavailable", error)
}
fn invalid(message: &str) -> ToolError {
    failure("TUTOR_ACTION_INVALID", "validation", message)
}

impl TutorState {
    fn pause_current(&mut self) {
        if let Some(session) = self
            .control
            .current_tutor_session_id
            .as_ref()
            .and_then(|id| self.sessions.get_mut(id))
        {
            if session.status == TutorSessionStatus::Active {
                session.status = TutorSessionStatus::Paused;
                session.revision += 1;
            }
        }
    }

    fn apply(&mut self, request: &TutorMutation) -> Result<(), ToolError> {
        if request.operation_id.trim().is_empty() {
            return Err(invalid("缺少操作身份。"));
        }
        if request.expected_revision != self.control.revision {
            return Err(failure(
                "TUTOR_REVISION_CONFLICT",
                "conflict",
                "教学状态已变化，请刷新后重试。",
            ));
        }
        match &request.action {
            TutorAction::SetEnabled { enabled } => {
                // Enabling records intent only. Readiness and scope decide whether a paused
                // session can run; those gates are integrated by T5/T7, not bypassed here.
                if !enabled {
                    self.pause_current();
                }
                self.control.enabled = *enabled;
            }
            TutorAction::Start {
                user_intent,
                explicit_constraints,
                material_scope,
                default_teaching_intent,
            } => {
                if user_intent.trim().is_empty() {
                    return Err(invalid("请填写本次学习意图。"));
                }
                if material_scope
                    .iter()
                    .any(|material| material.source_id.trim().is_empty())
                {
                    return Err(invalid("参考材料缺少来源身份。"));
                }
                self.pause_current();
                let id = format!("tutor-{}", request.operation_id);
                self.sessions.insert(
                    id.clone(),
                    TutorSession {
                        id: id.clone(),
                        revision: 1,
                        status: if self.control.enabled {
                            TutorSessionStatus::Active
                        } else {
                            TutorSessionStatus::Paused
                        },
                        user_intent: user_intent.trim().into(),
                        explicit_constraints: explicit_constraints.clone(),
                        current_focus: TutorFocus::default(),
                        material_scope: material_scope.clone(),
                        default_teaching_intent: default_teaching_intent.clone(),
                        path_instance_ref: None,
                        progress_ref: None,
                    },
                );
                self.control.current_tutor_session_id = Some(id);
            }
            TutorAction::Resume { session_id } => {
                if !self.control.enabled {
                    return Err(invalid("请先开启 Tutor。"));
                }
                if !self.sessions.contains_key(session_id) {
                    return Err(invalid("教学会话不存在。"));
                }
                self.pause_current();
                let session = self.sessions.get_mut(session_id).unwrap();
                session.status = TutorSessionStatus::Active;
                session.revision += 1;
                self.control.current_tutor_session_id = Some(session_id.clone());
            }
            TutorAction::Pause { session_id } | TutorAction::End { session_id } => {
                let session = self
                    .sessions
                    .get_mut(session_id)
                    .ok_or_else(|| invalid("教学会话不存在。"))?;
                if matches!(request.action, TutorAction::Pause { .. })
                    && session.status == TutorSessionStatus::Ended
                {
                    return Err(invalid("已结束的会话需要显式恢复。"));
                }
                session.status = if matches!(request.action, TutorAction::End { .. }) {
                    TutorSessionStatus::Ended
                } else {
                    TutorSessionStatus::Paused
                };
                session.revision += 1;
            }
            TutorAction::SetMode { session_id, mode } => {
                let session = self
                    .sessions
                    .get_mut(session_id)
                    .ok_or_else(|| invalid("教学会话不存在。"))?;
                session.default_teaching_intent = mode.clone();
                session.revision += 1;
            }
        }
        self.control.revision += 1;
        Ok(())
    }
}

pub struct LearningStore {
    pub(crate) connection: Connection,
    pub(crate) writer_lease: Option<std::sync::Arc<dyn Send + Sync>>,
}

impl LearningStore {
    pub fn open(path: &Path, private: bool) -> Result<Self, ToolError> {
        // MemoryStore already applied the reader-private directory gate at startup.
        if let Some(parent) = path.parent() {
            crate::ReaderPrivateStorageGate::create_dir_all(parent).map_err(storage)?;
        }
        let mut connection = Connection::open(path).map_err(storage)?;
        let version: i64 = connection.pragma_query_value(None, "user_version", |r| r.get(0)).map_err(storage)?;
        let application: i64 = connection.pragma_query_value(None, "application_id", |r| r.get(0)).map_err(storage)?;
        let unknown_tables: i64 = connection.query_row("SELECT count(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' AND name NOT IN ('tutor_events','tutor_projection','teaching_trace','teaching_assessments','learning_evidence','learner_projection')", [], |r| r.get(0)).map_err(storage)?;
        if !(0..=1).contains(&version) || unknown_tables != 0
            || (version == 0 && application != 0) || (version == 1 && application != 0x55424c4e) {
            return Err(failure("LEARNING_SCHEMA_INCOMPATIBLE", "conflict", "Learning schema requires a compatible binary"));
        }
        connection.busy_timeout(std::time::Duration::from_secs(5)).map_err(storage)?;
        connection.pragma_update(None, "foreign_keys", "ON").map_err(storage)?;
        connection.pragma_update(None, "synchronous", "FULL").map_err(storage)?;
        if private {
            crate::ReaderPrivateStorageGate::secure_file(path)?;
        }
        if version == 0 {
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(storage)?;
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS tutor_events (
                revision INTEGER PRIMARY KEY, operation_id TEXT NOT NULL UNIQUE,
                request TEXT NOT NULL, occurred_at TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS tutor_projection (id INTEGER PRIMARY KEY CHECK(id=1), state TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS teaching_trace (seq INTEGER PRIMARY KEY AUTOINCREMENT,
                event_id TEXT NOT NULL UNIQUE, session_id TEXT NOT NULL, event TEXT NOT NULL);
             CREATE INDEX IF NOT EXISTS teaching_trace_session ON teaching_trace(session_id, seq);
             CREATE TABLE IF NOT EXISTS teaching_assessments (action_ref TEXT PRIMARY KEY, assessment TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS learning_evidence (seq INTEGER PRIMARY KEY AUTOINCREMENT, evidence_id TEXT UNIQUE NOT NULL, source_id TEXT NOT NULL, evidence TEXT NOT NULL);
             CREATE INDEX IF NOT EXISTS learning_evidence_source ON learning_evidence(source_id,seq);
             CREATE TABLE IF NOT EXISTS learner_projection (source_id TEXT PRIMARY KEY, projection TEXT NOT NULL);"
        ).map_err(storage)?;
        tx.pragma_update(None, "user_version", 1).map_err(storage)?;
        tx.pragma_update(None, "application_id", 0x55424c4e).map_err(storage)?;
        tx.commit().map_err(storage)?;
        }
        Ok(Self { connection, writer_lease: None })
    }

    pub fn state(&self) -> Result<TutorState, ToolError> {
        let json: Option<String> = self
            .connection
            .query_row("SELECT state FROM tutor_projection WHERE id=1", [], |row| {
                row.get(0)
            })
            .optional()
            .map_err(storage)?;
        match json {
            Some(json) => serde_json::from_str(&json).map_err(storage),
            None => self.replay(None),
        }
    }

    fn replay(&self, until: Option<u32>) -> Result<TutorState, ToolError> {
        let mut statement = self
            .connection
            .prepare("SELECT request FROM tutor_events WHERE revision <= ? ORDER BY revision")
            .map_err(storage)?;
        let rows = statement
            .query_map([until.unwrap_or(u32::MAX)], |row| row.get::<_, String>(0))
            .map_err(storage)?;
        let mut state = TutorState::default();
        for row in rows {
            state.apply(
                &serde_json::from_str::<TutorMutation>(&row.map_err(storage)?).map_err(storage)?,
            )?;
        }
        Ok(state)
    }

    pub fn mutate(&mut self, request: &TutorMutation, now: &str) -> Result<TutorState, ToolError> {
        // A write transaction covers both revision selection and the event/projection commit.
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let previous: Option<(u32, String)> = transaction
            .query_row(
                "SELECT revision, request FROM tutor_events WHERE operation_id=?",
                [&request.operation_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(storage)?;
        if let Some((revision, json)) = previous {
            if serde_json::from_str::<TutorMutation>(&json).map_err(storage)? != *request {
                return Err(invalid("同一操作身份不能用于不同请求。"));
            }
            drop(transaction);
            return self.replay(Some(revision));
        }
        let json: Option<String> = transaction
            .query_row("SELECT state FROM tutor_projection WHERE id=1", [], |row| {
                row.get(0)
            })
            .optional()
            .map_err(storage)?;
        let mut state: TutorState = if let Some(json) = json {
            serde_json::from_str(&json).map_err(storage)?
        } else {
            let mut statement = transaction
                .prepare("SELECT request FROM tutor_events ORDER BY revision")
                .map_err(storage)?;
            let rows = statement
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(storage)?;
            let mut state = TutorState::default();
            for row in rows {
                state.apply(
                    &serde_json::from_str::<TutorMutation>(&row.map_err(storage)?)
                        .map_err(storage)?,
                )?;
            }
            state
        };
        state.apply(request)?;
        transaction
            .execute(
                "INSERT INTO tutor_events VALUES (?, ?, ?, ?)",
                params![
                    state.control.revision,
                    request.operation_id,
                    serde_json::to_string(request).map_err(storage)?,
                    now
                ],
            )
            .map_err(storage)?;
        transaction.execute("INSERT INTO tutor_projection VALUES (1, ?) ON CONFLICT(id) DO UPDATE SET state=excluded.state", [serde_json::to_string(&state).map_err(storage)?]).map_err(storage)?;
        transaction.commit().map_err(storage)?;
        Ok(state)
    }

    pub fn rebuild(&mut self) -> Result<TutorState, ToolError> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        transaction
            .execute("DELETE FROM tutor_projection", [])
            .map_err(storage)?;
        transaction.commit().map_err(storage)?;
        // Missing projection is read directly from events; the next mutation materializes it.
        self.replay(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn change(store: &mut LearningStore, id: &str, action: TutorAction) -> TutorState {
        let revision = store.state().unwrap().control.revision;
        store
            .mutate(
                &TutorMutation {
                    operation_id: id.into(),
                    expected_revision: revision,
                    action,
                },
                "now",
            )
            .unwrap()
    }
    fn start(intent: &str) -> TutorAction {
        TutorAction::Start {
            user_intent: intent.into(),
            explicit_constraints: vec!["先理解直觉".into()],
            material_scope: vec![TutorMaterial {
                source_id: "book-a".into(),
                scope_refs: vec!["chapter-1".into()],
                role: TutorMaterialRole::Primary,
            }],
            default_teaching_intent: Some(TutorSessionMode::GuidedInquiry),
        }
    }
    #[test]
    fn lifecycle_survives_restart_and_replays_one_current_session() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("learning.db");
        let mut store = LearningStore::open(&path, false).unwrap();
        assert_eq!(store.state().unwrap(), TutorState::default());
        change(&mut store, "on", TutorAction::SetEnabled { enabled: true });
        let first = change(&mut store, "first", start("理解注意力"));
        assert_eq!(
            first.sessions["tutor-first"].current_focus,
            TutorFocus::default()
        );
        let second = change(&mut store, "second", start("推导梯度"));
        assert_eq!(
            second.sessions["tutor-first"].status,
            TutorSessionStatus::Paused
        );
        assert_eq!(
            second.control.current_tutor_session_id.as_deref(),
            Some("tutor-second")
        );
        let resumed = change(
            &mut store,
            "switch",
            TutorAction::Resume {
                session_id: "tutor-first".into(),
            },
        );
        assert_eq!(
            resumed
                .sessions
                .values()
                .filter(|s| s.status == TutorSessionStatus::Active)
                .count(),
            1
        );
        let disabled = change(
            &mut store,
            "off",
            TutorAction::SetEnabled { enabled: false },
        );
        assert!(disabled
            .sessions
            .values()
            .all(|s| s.status != TutorSessionStatus::Active));
        drop(store);
        let mut reopened = LearningStore::open(&path, false).unwrap();
        assert_eq!(reopened.state().unwrap(), disabled);
        assert_eq!(reopened.rebuild().unwrap(), disabled);
        change(
            &mut reopened,
            "on-again",
            TutorAction::SetEnabled { enabled: true },
        );
        change(
            &mut reopened,
            "continue",
            TutorAction::Resume {
                session_id: "tutor-first".into(),
            },
        );
        let paused = change(
            &mut reopened,
            "pause",
            TutorAction::Pause {
                session_id: "tutor-first".into(),
            },
        );
        assert!(paused.control.enabled);
        let ended = change(
            &mut reopened,
            "end",
            TutorAction::End {
                session_id: "tutor-first".into(),
            },
        );
        assert!(ended.control.enabled);
        assert_eq!(
            ended.sessions["tutor-first"].status,
            TutorSessionStatus::Ended
        );
        let restored = change(
            &mut reopened,
            "restore-history",
            TutorAction::Resume {
                session_id: "tutor-first".into(),
            },
        );
        assert_eq!(
            restored.sessions["tutor-first"].status,
            TutorSessionStatus::Active
        );
        assert_eq!(restored.sessions["tutor-first"].user_intent, "理解注意力");
        assert_eq!(reopened.rebuild().unwrap(), restored);
    }

    #[test]
    fn retry_returns_original_result_and_stale_or_invalid_actions_do_not_write() {
        let directory = tempfile::tempdir().unwrap();
        let mut store = LearningStore::open(&directory.path().join("learning.db"), false).unwrap();
        let request = TutorMutation {
            operation_id: "enable".into(),
            expected_revision: 0,
            action: TutorAction::SetEnabled { enabled: true },
        };
        let original = store.mutate(&request, "now").unwrap();
        let next = change(&mut store, "start", start("学习"));
        assert_eq!(store.mutate(&request, "retry").unwrap(), original);
        assert_eq!(store.state().unwrap(), next);
        let mut stale = request.clone();
        stale.operation_id = "stale".into();
        assert_eq!(
            store.mutate(&stale, "now").unwrap_err().error_code,
            "TUTOR_REVISION_CONFLICT"
        );
        let mut invalid_request = request.clone();
        invalid_request.action = TutorAction::SetEnabled { enabled: false };
        assert_eq!(
            store
                .mutate(&invalid_request, "now")
                .unwrap_err()
                .error_code,
            "TUTOR_ACTION_INVALID"
        );
        assert_eq!(store.state().unwrap(), next);
    }

    #[test]
    fn mu2_learning_versions_preserve_legacy_events_and_reject_foreign_databases() {
        let directory = tempfile::tempdir().unwrap(); let path = directory.path().join("learning.db");
        let mut store = LearningStore::open(&path, false).unwrap();
        let expected = change(&mut store, "original", TutorAction::SetEnabled { enabled: true });
        store.connection.pragma_update(None, "user_version", 0).unwrap();
        store.connection.pragma_update(None, "application_id", 0).unwrap(); drop(store);
        let store = LearningStore::open(&path, false).unwrap();
        assert_eq!(store.state().unwrap(), expected);
        assert_eq!(store.replay(None).unwrap(), expected);
        assert_eq!(store.connection.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0)).unwrap(), 1);
        assert_eq!(store.connection.pragma_query_value(None, "journal_mode", |r| r.get::<_, String>(0)).unwrap(), "delete");
        store.connection.pragma_update(None, "application_id", 12345).unwrap(); drop(store);
        let before = std::fs::read(&path).unwrap();
        assert!(matches!(LearningStore::open(&path, false), Err(e) if e.error_code == "LEARNING_SCHEMA_INCOMPATIBLE"));
        assert_eq!(std::fs::read(path).unwrap(), before);
    }

    #[test]
    fn failed_sqlite_write_rolls_back_event_and_projection() {
        let directory = tempfile::tempdir().unwrap();
        let mut store = LearningStore::open(&directory.path().join("learning.db"), false).unwrap();
        let before = change(&mut store, "on", TutorAction::SetEnabled { enabled: true });
        store.connection.execute_batch("CREATE TRIGGER fail_projection BEFORE UPDATE ON tutor_projection BEGIN SELECT RAISE(ABORT, 'disk write failure'); END;").unwrap();
        let request = TutorMutation {
            operation_id: "off".into(),
            expected_revision: 1,
            action: TutorAction::SetEnabled { enabled: false },
        };
        assert_eq!(
            store.mutate(&request, "now").unwrap_err().error_code,
            "LEARNING_STORAGE_UNAVAILABLE"
        );
        assert_eq!(store.state().unwrap(), before);
        assert_eq!(store.replay(None).unwrap(), before);
        store
            .connection
            .execute_batch("DROP TRIGGER fail_projection")
            .unwrap();
        assert!(!store.mutate(&request, "retry").unwrap().control.enabled);
    }

    #[test]
    fn mode_is_separate_from_enable_and_empty_intent_is_rejected() {
        let directory = tempfile::tempdir().unwrap();
        let mut store = LearningStore::open(&directory.path().join("learning.db"), false).unwrap();
        let paused = change(&mut store, "first", start("学习"));
        assert_eq!(
            paused.sessions["tutor-first"].status,
            TutorSessionStatus::Paused
        );
        let changed = change(
            &mut store,
            "mode",
            TutorAction::SetMode {
                session_id: "tutor-first".into(),
                mode: Some(TutorSessionMode::DirectExplanation),
            },
        );
        assert!(!changed.control.enabled);
        let request = TutorMutation {
            operation_id: "empty".into(),
            expected_revision: changed.control.revision,
            action: start(" "),
        };
        assert!(store.mutate(&request, "now").is_err());
        assert_eq!(store.replay(None).unwrap(), changed);
    }
}
