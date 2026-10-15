//! One authoritative private state per reader (ADR-0147, MU1a/MU2).
//! The host borrows this object for commands; it is never cloned for a Run.
use crate::{intent_build_store::IntentArtifactStore, AgentHistory};
use memory::{learning::LearningStore, MemoryStore};
use read_tools::ToolError;
use runtime::profile_context::ProfileContextCache;
use std::path::PathBuf;

pub const LOCAL_USER_ID: &str = "local";

pub struct UserRuntime {
    pub(crate) presentation_limits: Option<(u64, usize)>,
    // None is the visitor's unavailable Resident state, not an anonymous reader.
    user_id: Option<String>,
    pub(crate) last_access: std::time::Instant,
    pub store: MemoryStore,
    pub history_path: Option<PathBuf>,
    pub agent_history: AgentHistory,
    pub(crate) session_store: Option<crate::session_store::SessionStore>,
    pub profile_context_cache: ProfileContextCache,
    pub intent_store_root: Option<PathBuf>,
}

impl UserRuntime {
    /// Explicit local-host adapter. Existing paths and loaded objects move intact.
    pub fn local(
        store: MemoryStore,
        history_path: Option<PathBuf>,
        agent_history: AgentHistory,
        intent_store_root: Option<PathBuf>,
    ) -> Self {
        Self {
            presentation_limits: None,
            user_id: Some(LOCAL_USER_ID.into()),
            last_access: std::time::Instant::now(),
            store,
            history_path,
            agent_history,
            session_store: None,
            profile_context_cache: ProfileContextCache::default(),
            intent_store_root,
        }
    }

    /// MCP has no Resident identity or private history/artifact write root.
    /// The caller supplies its existing unavailable MemoryStore.
    pub fn visitor(store: MemoryStore) -> Self {
        Self {
            presentation_limits: None,
            user_id: None,
            last_access: std::time::Instant::now(),
            store,
            history_path: None,
            agent_history: AgentHistory::default(),
            session_store: None,
            profile_context_cache: ProfileContextCache::default(),
            intent_store_root: None,
        }
    }

    pub fn user_id(&self) -> Option<&str> {
        self.user_id.as_deref()
    }

    pub(crate) fn open_service(
        paths: &crate::user_storage_paths::UserStoragePaths,
        writer: std::sync::Arc<crate::control_store::ServiceWriter>,
        now: &str,
    ) -> Result<Self, ToolError> {
        paths.prepare()?;
        let mut store =
            MemoryStore::open_private_with_learning(paths.memory.clone(), paths.learning.clone())?;
        store.retain_writer_lease(writer.clone());
        // Validate the private database before exposing a partially loaded user.
        let _learning = store.learning_store()?;
        let history_path = Some(paths.history.clone());
        let (mut agent_history, mut session_store) = crate::session_store::load_chat_storage(&history_path)?;
        let control = crate::control_store::ControlStore::open(writer)?;
        let mut query = control.connection.prepare("SELECT turn_id FROM run_admissions WHERE owner_user_id=?").map_err(|_| crate::agent_history_internal("Cannot read admission recovery index"))?;
        let admitted = query.query_map([&paths.owner], |r| r.get::<_,String>(0)).map_err(|_| crate::agent_history_internal("Cannot read admission recovery index"))?.collect::<Result<std::collections::BTreeSet<_>,_>>().map_err(|_| crate::agent_history_internal("Cannot read admission recovery index"))?;
        if session_store.is_none() {
            crate::agent_run::recover_pending_except(&mut agent_history, &history_path, &admitted)?;
        } else {
            crate::session_runtime::recover(session_store.as_mut().unwrap(), &mut agent_history, &admitted)?;
        }
        let mut user = Self::local(
            store,
            history_path,
            agent_history,
            Some(paths.private.clone()),
        );
        user.user_id = Some(paths.owner.clone());
        user.session_store = session_store;
        user.presentation_limits = Some((512 * 1024 * 1024, 4096));
        let cursors = crate::agent_history_review_cursors(&user.agent_history);
        user.store
            .initialize_review_watermark_baseline(&cursors, now)?;
        user.store.resume_review_jobs(&cursors, now)?;
        user.store
            .resume_interrupted_historical_backfill_jobs(now)?;
        Ok(user)
    }

    pub fn check_owner(&self, owner: &str) -> Result<(), ToolError> {
        if self.user_id() != Some(owner) {
            return Err(crate::user_storage_paths::error(
                "USER_OWNER_MISMATCH",
                "permission",
                "User runtime owner changed",
            ));
        }
        Ok(())
    }

    pub(crate) fn prepare_eviction(&mut self, now: std::time::Instant) -> Result<bool, ToolError> {
        self.agent_history.expire_confirmations(now);
        if now.saturating_duration_since(self.last_access) < crate::user_registry::USER_IDLE_TTL
            || !self.agent_history.pending_memory_ops.is_empty()
            || !self.agent_history.pending_governance_mutations.is_empty()
            || self.agent_history.sessions.iter().any(|s| {
                s.turns
                    .iter()
                    .any(|t| t.status == crate::AgentAssistantStatus::PendingAssistant)
            })
            || self
                .store
                .review_state()
                .review_jobs
                .iter()
                .any(|j| j.status == memory::ReviewJobStatus::Running)
            || self
                .store
                .historical_backfill_jobs()
                .iter()
                .any(|j| j.status == memory::HistoricalBackfillJobStatus::Running)
        {
            return Ok(false);
        }
        self.store.flush_pending_reads()?;
        if self.session_store.is_none() {
            crate::save_agent_history_path(&self.history_path, &self.agent_history)?;
        }
        Ok(true)
    }

    pub fn learning_store(&self) -> Result<LearningStore, ToolError> {
        self.store.learning_store()
    }

    pub(crate) fn presentation_root(&self) -> Result<PathBuf, ToolError> {
        self.history_path
            .as_ref()
            .map(|path| crate::session_store::SessionPaths::from_history(path).presentations)
            .ok_or_else(|| ToolError {
                error_code: "PRESENTATION_STORAGE_UNAVAILABLE".into(),
                category: "unavailable".into(),
                message: "Presentation delivery requires private persistent history".into(),
            })
    }

    pub(crate) fn intent_store(&self) -> Result<IntentArtifactStore, ToolError> {
        let root = self.intent_store_root.as_ref().ok_or_else(|| ToolError {
            error_code: "READER_PRIVATE_STORAGE_UNAVAILABLE".into(),
            category: "permission".into(),
            message: "this host cannot access reader-private build intents".into(),
        })?;
        IntentArtifactStore::open(root)
    }
}
