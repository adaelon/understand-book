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

/// The reader's purpose for this stage, independent of its teaching method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
#[serde(rename_all = "snake_case")]
pub enum LearningPurpose {
    Discover,
    Understand,
    Exam,
    Explore,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub learning_purpose: Option<LearningPurpose>,
    pub path_instance_ref: Option<String>,
    pub progress_ref: Option<String>,
    #[serde(default)]
    pub workspace_goal_revision: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct TutorState {
    pub control: TutorControl,
    pub sessions: BTreeMap<String, TutorSession>,
    #[serde(default)]
    pub workspace_goal_revision: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TutorAction {
    /// Server-owned synchronization with the accepted workspace goal.
    SyncWorkspaceGoal { revision: u32 },
    SetEnabled {
        enabled: bool,
    },
    Start {
        user_intent: String,
        explicit_constraints: Vec<String>,
        material_scope: Vec<TutorMaterial>,
        default_teaching_intent: Option<TutorSessionMode>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        learning_purpose: Option<LearningPurpose>,
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
            TutorAction::SyncWorkspaceGoal { revision } => {
                if *revision == 0 || self.workspace_goal_revision.is_some_and(|old| old >= *revision) {
                    return Err(invalid("工作区目标版本必须向前推进。"));
                }
                self.pause_current();
                self.control.current_tutor_session_id = None;
                self.workspace_goal_revision = Some(*revision);
                // Workspace goal revisions are independent of the user's Tutor
                // control revision. Clearing the current session invalidates
                // previously bound continuation while preserving control CAS.
                return Ok(());
            }
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
                learning_purpose,
            } => {
                if user_intent.trim().is_empty() {
                    return Err(invalid("请填写本次学习意图。"));
                }
                if *learning_purpose == Some(LearningPurpose::Discover) {
                    return Err(invalid("快速概览使用普通问答，不建立教学阶段。"));
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
                        learning_purpose: *learning_purpose,
                        path_instance_ref: None,
                        progress_ref: None,
                        workspace_goal_revision: self.workspace_goal_revision,
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
                if self.sessions[session_id].workspace_goal_revision != self.workspace_goal_revision {
                    return Err(failure("TUTOR_WORKSPACE_GOAL_CHANGED", "conflict", "学习目标已修改，请按新目标开始阶段。"));
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

const LEGACY_LEARNING_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS tutor_events (
                revision INTEGER PRIMARY KEY, operation_id TEXT NOT NULL UNIQUE,
                request TEXT NOT NULL, occurred_at TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS tutor_projection (id INTEGER PRIMARY KEY CHECK(id=1), state TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS teaching_trace (seq INTEGER PRIMARY KEY AUTOINCREMENT,
                event_id TEXT NOT NULL UNIQUE, session_id TEXT NOT NULL, event TEXT NOT NULL);
             CREATE INDEX IF NOT EXISTS teaching_trace_session ON teaching_trace(session_id, seq);
             CREATE TABLE IF NOT EXISTS teaching_assessments (action_ref TEXT PRIMARY KEY, assessment TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS learning_evidence (seq INTEGER PRIMARY KEY AUTOINCREMENT, evidence_id TEXT UNIQUE NOT NULL, source_id TEXT NOT NULL, evidence TEXT NOT NULL);
             CREATE INDEX IF NOT EXISTS learning_evidence_source ON learning_evidence(source_id,seq);
             CREATE TABLE IF NOT EXISTS learner_projection (source_id TEXT PRIMARY KEY, projection TEXT NOT NULL);";

/// The public state keeps all historical stages. The private projection also
/// remembers which stage the reader last selected for each material. Replaying
/// Start/Resume gives existing histories the same restoration as new ones.
#[derive(Default, Serialize, Deserialize)]
struct TutorProjection {
    #[serde(flatten)]
    state: TutorState,
    source_sessions: BTreeMap<String, String>,
}

impl TutorProjection {
    fn apply(&mut self, request: &TutorMutation) -> Result<(), ToolError> {
        self.state.apply(request)?;
        match request.action {
            TutorAction::Start { .. } | TutorAction::Resume { .. } => {
                if let Some(session) = self.state.control.current_tutor_session_id.as_ref()
                    .and_then(|id| self.state.sessions.get(id)) {
                    for material in &session.material_scope {
                        self.source_sessions.insert(material.source_id.clone(), session.id.clone());
                    }
                }
            }
            TutorAction::SyncWorkspaceGoal { .. } => self.source_sessions.clear(),
            _ => {}
        }
        Ok(())
    }

    fn view(mut self, source: Option<&str>) -> TutorState {
        if let Some(source) = source {
            self.state.control.current_tutor_session_id = self.source_sessions.get(source).cloned();
        }
        self.state
    }
}

pub struct LearningStore {
    pub(crate) connection: Connection,
    pub(crate) writer_lease: Option<std::sync::Arc<dyn Send + Sync>>,
    pub(crate) workspace_scope: String,
    source_scope: Option<String>,
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
        let unknown_tables: i64 = connection.query_row("SELECT count(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' AND name NOT IN ('tutor_events','tutor_projection','teaching_trace','teaching_assessments','learning_evidence','learner_projection','linked_learning_records','teaching_coach_jobs','teaching_coach_turns')", [], |r| r.get(0)).map_err(storage)?;
        if !(0..=4).contains(&version) || unknown_tables != 0
            || (version == 0 && application != 0) || (version > 0 && application != 0x55424c4e) {
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
        tx.execute_batch(LEGACY_LEARNING_SCHEMA).map_err(storage)?;
        tx.pragma_update(None, "user_version", 1).map_err(storage)?;
        tx.pragma_update(None, "application_id", 0x55424c4e).map_err(storage)?;
        tx.commit().map_err(storage)?;
        }
        if version < 2 {
            let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(storage)?;
            tx.execute_batch(include_str!("learning_workspace_scope.sql")).map_err(storage)?;
            tx.pragma_update(None, "user_version", 2).map_err(storage)?;
            tx.commit().map_err(storage)?;
        }
        if version < 3 {
            let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(storage)?;
            tx.execute_batch(include_str!("learning_records.sql")).map_err(storage)?;
            tx.pragma_update(None, "user_version", 3).map_err(storage)?;
            tx.commit().map_err(storage)?;
        }
        if version < 4 {
            let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(storage)?;
            tx.execute_batch(crate::tutor_coach::SCHEMA).map_err(storage)?;
            tx.pragma_update(None, "user_version", 4).map_err(storage)?;
            tx.commit().map_err(storage)?;
        }
        Ok(Self { connection, writer_lease: None, workspace_scope: String::new(), source_scope: None })
    }

    /// Select an immutable learning boundary for this handle. Legacy records stay
    /// in the absent-workspace scope until the user explicitly imports them.
    pub fn with_workspace(mut self, learning_workspace_id: Option<&str>) -> Self {
        self.workspace_scope = learning_workspace_id.unwrap_or_default().to_owned();
        self
    }

    /// Select this material's saved stage without changing its lifecycle or
    /// creating a teaching action merely because the reader navigated.
    pub fn with_source(mut self, source_id: &str) -> Self {
        self.source_scope = Some(source_id.to_owned());
        self
    }

    pub fn learning_workspace_id(&self) -> Option<&str> {
        (!self.workspace_scope.is_empty()).then_some(self.workspace_scope.as_str())
    }

    /// The authoritative goal is kept by the workspace store; the learning log
    /// only records its revision to bind stages and stop stale continuation.
    pub fn sync_workspace_goal(&mut self, revision: u32, now: &str) -> Result<TutorState, ToolError> {
        if self.workspace_scope.is_empty() { return Err(invalid("目标关联需要学习工作区。")); }
        let state = self.state()?;
        if state.workspace_goal_revision.is_some_and(|current| current >= revision) { return Ok(state); }
        self.mutate(&TutorMutation {
            operation_id: format!("workspace-goal:{revision}"),
            expected_revision: state.control.revision,
            action: TutorAction::SyncWorkspaceGoal { revision },
        }, now)
    }

    pub fn state(&self) -> Result<TutorState, ToolError> {
        let saved: Option<(String,u32)> = self.connection.query_row(
            "SELECT state,event_watermark FROM tutor_projection WHERE id=1 AND learning_workspace_id=?",
            [&self.workspace_scope], |row| Ok((row.get(0)?,row.get(1)?))).optional().map_err(storage)?;
        let revision: u32 = self.connection.query_row(
            "SELECT coalesce(max(revision),0) FROM tutor_events", [], |row|row.get(0)).map_err(storage)?;
        if let Some((json,watermark)) = saved {
            if watermark == revision {
                // Old cached projections have no source index; the authoritative
                // event history reconstructs it without rewriting any records.
                if let Ok(projection) = serde_json::from_str::<TutorProjection>(&json) {
                    return Ok(projection.view(self.source_scope.as_deref()));
                }
            }
        }
        self.replay(None)
    }

    fn replay(&self, until: Option<u32>) -> Result<TutorState, ToolError> {
        Ok(replay_workspace(&self.connection, &self.workspace_scope, until)?.view(self.source_scope.as_deref()))
    }

    pub fn mutate(&mut self, request: &TutorMutation, now: &str) -> Result<TutorState, ToolError> {
        let transaction = self.connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(storage)?;
        let previous: Option<(u32, String)> = transaction.query_row(
            "SELECT revision, request FROM tutor_events WHERE learning_workspace_id=? AND operation_id=?",
            params![self.workspace_scope,request.operation_id], |row|Ok((row.get(0)?,row.get(1)?)))
            .optional().map_err(storage)?;
        if let Some((revision, json)) = previous {
            if serde_json::from_str::<TutorMutation>(&json).map_err(storage)? != *request {
                return Err(invalid("同一操作身份不能用于不同请求。"));
            }
            return Ok(replay_workspace(&transaction, &self.workspace_scope, Some(revision))?.view(self.source_scope.as_deref()));
        }
        let mut state = replay_workspace(&transaction, &self.workspace_scope, None)?;
        state.apply(request)?;
        let event_revision: u32 = transaction.query_row(
            "SELECT coalesce(max(revision),0)+1 FROM tutor_events", [], |row|row.get(0)).map_err(storage)?;
        transaction.execute(
            "INSERT INTO tutor_events(revision,operation_id,request,occurred_at,learning_workspace_id) VALUES (?,?,?,?,?)",
            params![event_revision,request.operation_id,serde_json::to_string(request).map_err(storage)?,now,self.workspace_scope]).map_err(storage)?;
        transaction.execute(
            "INSERT INTO tutor_projection(id,state,event_watermark,learning_workspace_id) VALUES (1,?,?,?) ON CONFLICT(learning_workspace_id,id) DO UPDATE SET state=excluded.state,event_watermark=excluded.event_watermark",
            params![serde_json::to_string(&state).map_err(storage)?,event_revision,self.workspace_scope]).map_err(storage)?;
        transaction.commit().map_err(storage)?;
        Ok(state.view(self.source_scope.as_deref()))
    }

    pub fn rebuild(&mut self) -> Result<TutorState, ToolError> {
        self.connection.execute("DELETE FROM tutor_projection WHERE learning_workspace_id=?",[&self.workspace_scope]).map_err(storage)?;
        self.replay(None)
    }
}

/// The teaching switch is account-wide. A workspace's control revision changes
/// for its own actions and global switch changes, never for another workspace's
/// stage changes. The event log revision separately preserves total replay order.
fn replay_workspace(connection: &Connection, scope: &str, until: Option<u32>) -> Result<TutorProjection, ToolError> {
    let mut statement = connection.prepare(
        "SELECT learning_workspace_id,request FROM tutor_events WHERE revision<=? ORDER BY revision").map_err(storage)?;
    let rows=statement.query_map([until.unwrap_or(u32::MAX)],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?))).map_err(storage)?;
    let mut state=TutorProjection::default();
    for row in rows {
        let (event_scope,json)=row.map_err(storage)?;
        let mut request: TutorMutation=serde_json::from_str(&json).map_err(storage)?;
        if event_scope == scope {
            state.apply(&request)?;
        } else if matches!(request.action,TutorAction::SetEnabled{..}) {
            request.expected_revision=state.state.control.revision;
            state.apply(&request)?;
        }
    }
    Ok(state)

}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_goals_restore_old_history_without_borrowing_another_books_goal() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("learning.db");
        let mut store = LearningStore::open(&path, false).unwrap();
        change(&mut store, "on", TutorAction::SetEnabled { enabled:true });
        change(&mut store, "a", start("理解计算图"));
        let mut book_b = start("理解资源核算");
        if let TutorAction::Start { material_scope, .. } = &mut book_b {
            material_scope[0].source_id = "book-b".into();
        }
        let legacy = change(&mut store, "b", book_b);
        // Existing installations have a cached TutorState with no source index.
        store.connection.execute("UPDATE tutor_projection SET state=?", [serde_json::to_string(&legacy).unwrap()]).unwrap();
        let a = LearningStore::open(&path, false).unwrap().with_source("book-a").state().unwrap();
        assert_eq!(a.control.current_tutor_session_id.as_deref(), Some("tutor-a"));
        assert_eq!(a.sessions["tutor-a"].user_intent, "理解计算图");
        assert_eq!(a.sessions["tutor-a"].status, TutorSessionStatus::Paused);
        let empty = LearningStore::open(&path, false).unwrap().with_source("unread-book").state().unwrap();
        assert!(empty.control.current_tutor_session_id.is_none());
        assert_eq!(empty.sessions.len(), 2, "old stages remain in history");
        let mut a = LearningStore::open(&path, false).unwrap().with_source("book-a");
        let resumed = change(&mut a, "resume-a", TutorAction::Resume { session_id:"tutor-a".into() });
        assert_eq!(resumed.control.current_tutor_session_id.as_deref(), Some("tutor-a"));
        let b = LearningStore::open(&path, false).unwrap().with_source("book-b").state().unwrap();
        assert_eq!(b.control.current_tutor_session_id.as_deref(), Some("tutor-b"));
        assert_eq!(a.rebuild().unwrap(), resumed);
    }

    #[test]
    fn source_goals_keep_workspace_and_joint_material_boundaries() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("learning.db");
        let mut workspace = LearningStore::open(&path, false).unwrap().with_workspace(Some("study"));
        workspace.sync_workspace_goal(1, "now").unwrap();
        change(&mut workspace, "on", TutorAction::SetEnabled { enabled:true });
        let mut joint = start("比较两份资料");
        if let TutorAction::Start { material_scope, .. } = &mut joint {
            material_scope.push(TutorMaterial { source_id:"book-b".into(), scope_refs:vec![], role:TutorMaterialRole::Supporting });
        }
        change(&mut workspace, "joint", joint);
        let source_state = |scope, source| LearningStore::open(&path, false).unwrap().with_workspace(scope).with_source(source).state().unwrap();
        assert_eq!(source_state(Some("study"), "book-a").control.current_tutor_session_id.as_deref(), Some("tutor-joint"));
        assert_eq!(source_state(Some("study"), "book-b").control.current_tutor_session_id.as_deref(), Some("tutor-joint"));
        change(&mut workspace, "a", start("只学当前书"));
        assert_eq!(source_state(Some("study"), "book-a").control.current_tutor_session_id.as_deref(), Some("tutor-a"));
        assert_eq!(source_state(Some("study"), "book-b").control.current_tutor_session_id.as_deref(), Some("tutor-joint"));
        assert!(source_state(None, "book-a").control.current_tutor_session_id.is_none());
        assert!(source_state(Some("other"), "book-a").control.current_tutor_session_id.is_none());
        workspace.sync_workspace_goal(2, "later").unwrap();
        assert!(source_state(Some("study"), "book-a").control.current_tutor_session_id.is_none());
        assert!(source_state(Some("study"), "book-b").control.current_tutor_session_id.is_none());
        assert_eq!(source_state(Some("study"), "book-a").sessions.len(), 2);
    }
    #[test]
    fn w4_goal_revision_stops_old_stage_and_replays_without_changing_teaching_switch() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("learning.db");
        let mut store = LearningStore::open(&path, true).unwrap().with_workspace(Some("study"));
        store.sync_workspace_goal(1, "100").unwrap();
        change(&mut store, "on", TutorAction::SetEnabled { enabled: true });
        let original = change(&mut store, "original", start("学习深度学习"));
        let original_id = original.control.current_tutor_session_id.clone().unwrap();
        assert_eq!(original.sessions[&original_id].workspace_goal_revision, Some(1));
        let changed = store.sync_workspace_goal(2, "101").unwrap();
        assert!(changed.control.enabled);
        assert_eq!(changed.control.revision, original.control.revision);
        assert!(changed.control.current_tutor_session_id.is_none());
        assert_eq!(changed.sessions[&original_id].status, TutorSessionStatus::Paused);
        assert_eq!(changed.sessions[&original_id].workspace_goal_revision, Some(1));
        let rejected = store.mutate(&TutorMutation { operation_id:"old-continue".into(),
            expected_revision:changed.control.revision, action:TutorAction::Resume {session_id:original_id.clone()} }, "102").unwrap_err();
        assert_eq!(rejected.error_code, "TUTOR_WORKSPACE_GOAL_CHANGED");
        let next = change(&mut store, "exam", start("考前只剩三天，先复习薄弱点"));
        assert_eq!(next.sessions[next.control.current_tutor_session_id.as_ref().unwrap()].workspace_goal_revision, Some(2));
        assert_eq!(store.sync_workspace_goal(2, "103").unwrap(), next);
        assert_eq!(store.sync_workspace_goal(1, "103").unwrap(), next, "an old admitted turn cannot restore the old goal");
        assert_eq!(store.rebuild().unwrap(), next);
        drop(store);
        assert_eq!(LearningStore::open(&path, true).unwrap().with_workspace(Some("study")).state().unwrap(), next);
        assert!(LearningStore::open(&path, true).unwrap().with_workspace(Some("other")).state().unwrap().workspace_goal_revision.is_none());
    }
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
            learning_purpose: None,
        }
    }
    #[test]
    fn purpose_is_restored_with_its_stage_and_discover_does_not_start_one() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("learning.db");
        let mut store = LearningStore::open(&path, false).unwrap();
        change(&mut store, "on", TutorAction::SetEnabled { enabled:true });
        let mut action = start("复习这一章");
        if let TutorAction::Start { learning_purpose, .. } = &mut action { *learning_purpose = Some(LearningPurpose::Exam); }
        let before = change(&mut store, "exam", action.clone());
        assert_eq!(before.sessions["tutor-exam"].learning_purpose, Some(LearningPurpose::Exam));
        assert_eq!(store.rebuild().unwrap(), before);
        if let TutorAction::Start { learning_purpose, .. } = &mut action { *learning_purpose = Some(LearningPurpose::Discover); }
        assert_eq!(store.mutate(&TutorMutation { operation_id:"overview".into(), expected_revision:before.control.revision, action }, "now")
            .unwrap_err().error_code, "TUTOR_ACTION_INVALID");
        assert_eq!(store.state().unwrap(), before);
        drop(store);
        assert_eq!(LearningStore::open(&path, false).unwrap().state().unwrap(), before);
        let legacy: TutorAction = serde_json::from_value(serde_json::json!({"kind":"start", "user_intent":"legacy",
            "explicit_constraints":[], "material_scope":[], "default_teaching_intent":null})).unwrap();
        assert!(matches!(legacy, TutorAction::Start { learning_purpose:None, .. }));
    }
    #[test]
    fn w3_workspace_stages_restore_independently_with_one_global_teaching_switch() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("learning.db");
        let mut a = LearningStore::open(&path, false).unwrap().with_workspace(Some("A"));
        let mut b = LearningStore::open(&path, false).unwrap().with_workspace(Some("B"));
        let legacy = LearningStore::open(&path, false).unwrap();
        change(&mut a, "on", TutorAction::SetEnabled { enabled: true });
        let original_a = change(&mut a, "stage", start("推导梯度"));
        let original_b = change(&mut b, "stage", start("考试练习"));
        assert_eq!(a.state().unwrap().control.revision, original_a.control.revision);
        assert_eq!(original_a.sessions["tutor-stage"].user_intent, "推导梯度");
        assert_eq!(original_b.sessions["tutor-stage"].user_intent, "考试练习");
        assert_eq!(a.state().unwrap().sessions["tutor-stage"].user_intent, "推导梯度");
        assert!(legacy.state().unwrap().sessions.is_empty());
        change(&mut b, "off", TutorAction::SetEnabled { enabled: false });
        for store in [&a, &b, &legacy] {
            let state = store.state().unwrap();
            assert!(!state.control.enabled);
            assert!(state.sessions.values().all(|session| session.status != TutorSessionStatus::Active));
        }
        change(&mut a, "on-again", TutorAction::SetEnabled { enabled: true });
        let resumed = change(&mut a, "resume", TutorAction::Resume { session_id: "tutor-stage".into() });
        assert_eq!(resumed.sessions["tutor-stage"].status, TutorSessionStatus::Active);
        assert_eq!(b.state().unwrap().sessions["tutor-stage"].status, TutorSessionStatus::Paused);
        assert_eq!(a.rebuild().unwrap(), resumed);
        drop(a);
        let reopened = LearningStore::open(&path, false).unwrap().with_workspace(Some("A"));
        assert_eq!(reopened.state().unwrap(), resumed);
        assert_eq!(b.rebuild().unwrap().sessions["tutor-stage"].user_intent, "考试练习");
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
        for version in [0, 1] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("learning.db");
            let connection = Connection::open(&path).unwrap();
            connection.execute_batch(LEGACY_LEARNING_SCHEMA).unwrap();
            let request=TutorMutation { operation_id:"original".into(), expected_revision:0,
                action:TutorAction::SetEnabled{enabled:true} };
            connection.execute("INSERT INTO tutor_events VALUES(1,'original',?,'now')",
                [serde_json::to_string(&request).unwrap()]).unwrap();
            connection.pragma_update(None,"user_version",version).unwrap();
            if version == 1 { connection.pragma_update(None,"application_id",0x55424c4e).unwrap(); }
            drop(connection);
            let store=LearningStore::open(&path,false).unwrap();
            assert!(store.state().unwrap().control.enabled);
            assert_eq!(store.state().unwrap(),store.replay(None).unwrap());
            assert_eq!(store.connection.pragma_query_value(None,"user_version",|r|r.get::<_,i64>(0)).unwrap(),4);
            assert_eq!(store.connection.pragma_query_value(None,"journal_mode",|r|r.get::<_,String>(0)).unwrap(),"delete");
            store.connection.pragma_update(None,"application_id",12345).unwrap();
            drop(store);
            let before=std::fs::read(&path).unwrap();
            assert!(matches!(LearningStore::open(&path,false),Err(e) if e.error_code == "LEARNING_SCHEMA_INCOMPATIBLE"));
            assert_eq!(std::fs::read(path).unwrap(),before);
        }
    }

    #[test]
    fn w3_schema_upgrade_preserves_legacy_fact_bytes_and_sequence_without_adopting_them() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("learning.db");
        let connection = Connection::open(&path).unwrap();
        connection.execute_batch(LEGACY_LEARNING_SCHEMA).unwrap();
        connection.pragma_update(None, "user_version", 1).unwrap();
        connection.pragma_update(None, "application_id", 0x55424c4e).unwrap();
        let event = serde_json::json!({"event_id":"old","binding":{"tutor_session_id":"old-session","session_revision":1,
            "control_revision":0,"source_id":"book","source_revision":"v1","map_revision":null,"chat_session_id":"chat","turn_id":"turn"},
            "kind":"usage_observed","causal_refs":[],"payload":{"response":"原始表达"},"occurred_at":"then"}).to_string();
        connection.execute("INSERT INTO teaching_trace(seq,event_id,session_id,event) VALUES(41,'old','old-session',?)", [&event]).unwrap();
        connection.execute("INSERT INTO teaching_assessments VALUES('old','{\"status\":\"uncertain\"}')", []).unwrap();
        connection.execute("INSERT INTO learning_evidence(seq,evidence_id,source_id,evidence) VALUES(73,'old-evidence','book','{\"old\":true}')", []).unwrap();
        connection.execute("INSERT INTO learner_projection VALUES('book','{\"old_projection\":true}')", []).unwrap();
        drop(connection);
        for _ in 0..2 {
            let store = LearningStore::open(&path, false).unwrap();
            assert_eq!(store.teaching_event("old").unwrap().occurred_at, "then");
            let trace: (u64, String, String) = store.connection.query_row("SELECT seq,event,learning_workspace_id FROM teaching_trace", [], |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
            assert_eq!(trace, (41, event.clone(), String::new()));
            let evidence: (u64, String, String) = store.connection.query_row("SELECT seq,evidence,learning_workspace_id FROM learning_evidence", [], |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
            assert_eq!(evidence, (73, "{\"old\":true}".into(), String::new()));
            let assessment: String = store.connection.query_row("SELECT assessment FROM teaching_assessments WHERE learning_workspace_id=''", [], |r|r.get(0)).unwrap();
            assert_eq!(assessment, "{\"status\":\"uncertain\"}");
            let projection: String = store.connection.query_row("SELECT projection FROM learner_projection WHERE learning_workspace_id=''", [], |r|r.get(0)).unwrap();
            assert_eq!(projection, "{\"old_projection\":true}");
            let scoped = store.with_workspace(Some("new"));
            assert!(scoped.teaching_event("old").is_err());
            assert!(scoped.evidence_page("book",0,100).unwrap().is_empty());
            assert!(scoped.assessment("old").unwrap().is_none());
            assert!(scoped.learner_projection("book").unwrap().0.is_none());
        }
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
