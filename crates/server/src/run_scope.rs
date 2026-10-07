//! Fixed Run ownership, including the original immutable publication when present.
use crate::*;
use runtime::run_context::ReaderInputSnapshot;

#[derive(Clone)]
pub(crate) struct RunScope {
    pub publication: Option<std::sync::Arc<crate::published_library::PublishedBook>>,
    pub user_id: String,
    pub workspace_id: String,
    pub workspace_generation: u64,
    pub chat_session_id: String,
    pub turn_id: String,
    pub confirmation_id: Option<String>,
    pub book: std::sync::Arc<Book>,
    pub book_dir: PathBuf,
    pub provider_binding: runtime::ModelRuntimeProfile,
    pub reader_input: ReaderInputSnapshot,
    pub question_quote: Option<AskQuote>,
    pub presentation_follow_up: Option<runtime::presentation::PresentationFollowUp>,
}

impl RunScope {
    pub fn capture(
        state: &AppState,
        turn: &AgentTurnRef,
        message: &str,
        question_quote: Option<AskQuote>,
        presentation_follow_up: Option<runtime::presentation::PresentationFollowUp>,
    ) -> Self {
        Self {
            publication: state.workspace.publication.clone(),
            user_id: state
                .user
                .user_id()
                .expect("Resident local identity")
                .into(),
            workspace_id: state.workspace.id.clone(),
            workspace_generation: state.workspace.generation,
            chat_session_id: turn.session_id.clone(),
            turn_id: turn.turn_id.clone(),
            confirmation_id: state.user.agent_history.confirmation_id(&turn.session_id).map(str::to_string),
            book: state.workspace.book.clone(),
            book_dir: state.workspace.book_dir.clone(),
            provider_binding: state.services.adapter.model_runtime_profile(),
            reader_input: ReaderInputSnapshot::capture(
                &state.workspace.book,
                &state.workspace.reader,
                message,
            ),
            question_quote,
            presentation_follow_up,
        }
    }

    pub fn check_owner(&self, state: &AppState) -> Result<(), ToolError> {
        self.check_user(&state.user)
    }

    pub fn check_user(&self, user: &crate::user_runtime::UserRuntime) -> Result<(), ToolError> {
        if user.user_id() != Some(self.user_id.as_str()) {
            return Err(ToolError {
                error_code: "RUN_OWNER_MISMATCH".into(),
                category: "permission".into(),
                message: "Run owner is unavailable".into(),
            });
        }
        if !user.agent_history.sessions.iter().any(|s| {
            s.id == self.chat_session_id
                && s.book_id == self.book.base.book_id
                && s.turns.iter().any(|t| t.turn_id == self.turn_id)
        }) {
            return Err(agent_history_internal("Run chat or turn is unavailable"));
        }
        Ok(())
    }

    pub fn check_workspace(&self, state: &AppState) -> Result<(), ToolError> {
        self.check_owner(state)?;
        self.check_scene(&state.workspace)
    }

    pub fn check_scene(&self, w: &crate::reader_workspace::ReaderWorkspace) -> Result<(), ToolError> {
        if w.id != self.workspace_id
            || w.generation != self.workspace_generation
            || w.publication.as_ref().map(|p| &p.reference) != self.publication.as_ref().map(|p| &p.reference)
            || w.selected_chat.as_deref() != Some(self.chat_session_id.as_str())
        {
            return Err(ToolError {
                error_code: "WORKSPACE_STALE".into(),
                category: "conflict".into(),
                message: "The Run's reading workspace has changed".into(),
            });
        }
        Ok(())
    }

    pub fn private_context<'a>(&'a self, state: &'a AppState) -> PrivateBookContext<'a> {
        self.private_user(&state.user)
    }

    pub fn private_user<'a>(&'a self, user: &'a crate::user_runtime::UserRuntime) -> PrivateBookContext<'a> {
        let messages = user
            .agent_history
            .sessions
            .iter()
            .find(|s| s.id == self.chat_session_id && s.book_id == self.book.base.book_id)
            .map(|s| s.messages.as_slice())
            .unwrap_or(&[]);
        PrivateBookContext {
            user,
            book: &self.book,
            book_dir: &self.book_dir,
            messages,
            selected_chat: Some(&self.chat_session_id),
        }
    }
}
