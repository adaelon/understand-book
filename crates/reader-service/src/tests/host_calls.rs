//! Test-only access to local host operations used by cloud integration fixtures.
use super::*;
use crate::session_event::EffectDisposition;
use crate::user_runtime::UserRuntime;
use base_schema::{MusicQuestionContext, PublishedBookRef};
use memory::workspace_notes::WorkspaceNoteStore;
pub fn precommit_agent_turn(
    state: &mut AppState,
    book_id: &str,
    user: String,
    question_anchor_lid: Option<String>,
    question_quote: Option<AskQuote>,
    presentation_follow_up: Option<runtime::presentation::PresentationFollowUp>,
    now: &str,
) -> Result<AgentTurnRef, ToolError> {
    crate::precommit_agent_turn(
        state,
        book_id,
        user,
        question_anchor_lid,
        question_quote,
        presentation_follow_up,
        now,
    )
}

pub fn prepare_agent_chat(
    state: &mut AppState,
    body: &str,
    now: &str,
) -> Result<crate::agent_run::PreparedAgentChat, Reply> {
    crate::prepare_agent_chat(state, body, now)
}

pub fn agent_source_binding(
    state: &AppState,
    request: &AgentSourceRequest,
) -> Result<SourceBinding, ToolError> {
    crate::agent_source_binding(state, request)
}

pub fn run_precommitted_agent_chat(
    port: &impl crate::agent_run::AppStatePort,
    adapter: &dyn ModelAdapter,
    prepared: &crate::agent_run::PreparedAgentChat,
    context: &mut runtime::run_context::RunContext,
) -> Result<OuterOutcome, ToolError> {
    crate::run_precommitted_agent_chat(port, adapter, prepared, context)
}

pub fn source_manifest_value(book_dir: &Path) -> Result<serde_json::Value, ToolError> {
    crate::source_manifest_value(book_dir)
}

pub fn list_mixed_book_library(root: &Path) -> Vec<Value> {
    crate::list_mixed_book_library(root)
        .into_iter()
        .map(|entry| serde_json::to_value(entry).unwrap())
        .collect()
}

pub fn current_note_source_fingerprint(book_dir: &Path) -> Result<String, ToolError> {
    crate::current_note_source_fingerprint(book_dir)
}

pub fn localize_paper_minimap(
    base: PaperMinimapBase,
    cache_path: Option<PathBuf>,
    adapter: &dyn ModelAdapter,
) -> Reply {
    crate::localize_paper_minimap(base, cache_path, adapter)
}

pub fn finalize_agent_turn(
    state: &mut AppState,
    turn_ref: &AgentTurnRef,
    status: AgentAssistantStatus,
    outcome: Option<OuterOutcome>,
    error: Option<AgentTurnError>,
    run_summary: Option<crate::agent_run::AgentRunSummary>,
    messages: &[Message],
    now: &str,
) -> Result<(), ToolError> {
    crate::finalize_agent_turn(
        state,
        turn_ref,
        status,
        outcome,
        error,
        run_summary,
        messages,
        now,
    )
}

pub fn route_agent_new(state: &mut AppState, now: &str) -> Reply {
    crate::route_agent_new(state, now)
}

pub fn route_agent_history_select(state: &mut AppState, body: &str) -> Reply {
    crate::route_agent_history_select(state, body)
}

pub fn workbench_input_manifest_path(book_dir: &Path) -> PathBuf {
    crate::workbench_input_manifest_path(book_dir)
}

pub mod presentation_sandbox {
    use super::*;
    use crate::presentation_sandbox::*;
    pub fn output_file(dir: &Path, name: &str, limit: u64) -> Result<Vec<u8>, ToolError> {
        crate::presentation_sandbox::output_file(dir, name, limit)
    }

    pub fn png(bytes: &[u8], width: u32, height: u32) -> Result<(), ToolError> {
        crate::presentation_sandbox::png(bytes, width, height)
    }
}
pub mod teaching {
    use super::*;
    use crate::teaching::*;
    pub fn understanding(
        state: &PrivateBookContext<'_>,
        source: &str,
        object: Option<&str>,
        after: u64,
    ) -> Result<Value, ToolError> {
        crate::teaching::understanding(state, source, object, after)
    }

    pub fn prepare(
        state: &PrivateBookContext<'_>,
        turn: &AgentTurnRef,
        request: &Value,
        now: &str,
    ) -> Result<Option<Value>, ToolError> {
        crate::teaching::prepare(state, turn, request, now)
    }

    pub fn start_request(state: &PrivateBookContext<'_>, now: &str) -> Result<Value, ToolError> {
        crate::teaching::start_request(state, now)
    }
}
pub mod music_interact {
    use super::*;
    use crate::music_interact::*;
    pub fn question(
        workspace: &crate::reader_workspace::ReaderWorkspace,
        input: &Value,
    ) -> Result<Option<MusicQuestionContext>, ToolError> {
        crate::music_interact::question(workspace, input)
    }
}
pub mod video_publication {
    use super::*;
    use crate::video_publication::*;
    pub fn persist_local_binding(
        w: &crate::reader_workspace::ReaderWorkspace,
    ) -> Result<(), ToolError> {
        crate::video_publication::persist_local_binding(w)
    }

    pub fn local_resume_directory(
        latest: &Path,
        session: &Option<std::path::PathBuf>,
    ) -> std::path::PathBuf {
        crate::video_publication::local_resume_directory(latest, session)
    }

    pub fn adopt_local(state: &mut crate::AppState) -> Result<serde_json::Value, ToolError> {
        crate::video_publication::adopt_local(state)
    }
}
pub mod video_source {
    use super::*;
    use crate::video_source::*;
    pub fn local_directory(
        workspace: &reader_workspace::ReaderWorkspace,
        reference: &PublishedBookRef,
    ) -> Result<PathBuf, ToolError> {
        crate::video_source::local_directory(workspace, reference)
    }
}
pub mod video_playback {
    use super::*;
    use crate::video_playback::*;
    pub fn ready(directory: &Path) -> Option<PlaybackInfo> {
        crate::video_playback::ready(directory)
    }
}
pub mod book_cover {
    use super::*;
    use crate::book_cover::*;
    pub fn route_local(state: &crate::AppState, url: &str) -> Option<crate::BinaryReply> {
        crate::book_cover::route_local(state, url)
    }
}
pub mod session_runtime {
    use super::*;
    use crate::session_runtime::*;
    pub fn finish(
        user: &mut user_runtime::UserRuntime,
        reference: &AgentTurnRef,
        session: &AgentChatSession,
        at: &str,
    ) -> Result<(), ToolError> {
        crate::session_runtime::finish(user, reference, session, at)
    }

    pub fn checkpoint(
        user: &mut user_runtime::UserRuntime,
        reference: &AgentTurnRef,
        checkpoint: &CompactionCheckpoint,
        messages: &[Message],
    ) -> Result<(), ToolError> {
        crate::session_runtime::checkpoint(user, reference, checkpoint, messages)
    }
}
pub mod workspace_record_context {
    use super::*;
    use crate::workspace_record_context::*;
    pub fn permitted_turn(
        user: &UserRuntime,
        workspace: Option<&str>,
        book_id: &str,
        chat_id: &str,
        turn_id: &str,
    ) -> Result<crate::AgentChatTurn, ToolError> {
        crate::workspace_record_context::permitted_turn(user, workspace, book_id, chat_id, turn_id)
    }

    pub fn read(user: &UserRuntime, workspace: &str, input: Value) -> Result<Value, ToolError> {
        crate::workspace_record_context::read(user, workspace, input)
    }
}
pub mod effect_disposition {
    use super::*;
    use crate::effect_disposition::*;
    pub fn route(
        state: &mut AppState,
        input: &Value,
        at: &str,
    ) -> Result<EffectDisposition, ToolError> {
        crate::effect_disposition::route(state, input, at)
    }
}
pub mod workspace_notes_api {
    use super::*;
    use crate::workspace_notes_api::*;
    pub fn scoped_store(
        user: &user_runtime::UserRuntime,
        scope: Option<&str>,
    ) -> Result<WorkspaceNoteStore, ToolError> {
        crate::workspace_notes_api::scoped_store(user, scope)
    }
}
pub mod note_api {
    use super::*;
    use crate::note_api::*;
    pub fn presentation(
        user: &user_runtime::UserRuntime,
        workspace: &reader_workspace::ReaderWorkspace,
        input: &Value,
        observe: bool,
    ) -> Result<Value, ToolError> {
        crate::note_api::presentation(user, workspace, input, observe)
    }

    pub fn follow_up(
        user: &user_runtime::UserRuntime,
        workspace: &reader_workspace::ReaderWorkspace,
        mem_id: &str,
    ) -> Result<(runtime::presentation::PresentationFollowUp, String), ToolError> {
        crate::note_api::follow_up(user, workspace, mem_id)
    }
}
pub mod agent_run {
    use super::*;
    use crate::agent_run::*;
    use runtime::run_context::CancellationToken;
    pub fn execute_prepared(
        port: &impl AppStatePort,
        adapter: &dyn ModelAdapter,
        prepared: PreparedAgentChat,
        cancellation: CancellationToken,
    ) -> Reply {
        crate::agent_run::execute_prepared(port, adapter, prepared, cancellation).reply
    }
}
pub mod teaching_coach {
    use super::*;
    use crate::teaching_coach::*;
    use runtime::run_context::CancellationToken;
    pub fn drive_inline(
        port: &impl crate::agent_run::AppStatePort,
        adapter: &dyn ModelAdapter,
        workspace: Option<&str>,
        turn: &AgentTurnRef,
        after: bool,
        cancel: &CancellationToken,
    ) -> Result<(), ToolError> {
        crate::teaching_coach::drive_inline(port, adapter, workspace, turn, after, cancel)
    }
}

pub fn borrowed_app_port(state: &mut AppState) -> impl crate::agent_run::AppStatePort + '_ {
    crate::agent_run::BorrowedAppPort(std::cell::RefCell::new(state))
}

pub fn runtime_state_port<'a, P: crate::agent_run::AppStatePort>(
    port: &'a P,
    turn: &'a AgentTurnRef,
    scope: &'a crate::run_scope::RunScope,
) -> impl runtime::run_context::ResidentStatePort + 'a {
    crate::agent_run::RuntimeStatePort {
        port,
        turn_ref: turn,
        scope,
        previewed: Default::default(),
        animations: Default::default(),
        plots: Default::default(),
    }
}

pub fn read_presentation_version(
    user: &crate::user_runtime::UserRuntime,
    owner: &runtime::presentation::PresentationOwner,
    reference: &runtime::presentation::PresentationRef,
) -> Result<runtime::presentation::AgentPresentation, ToolError> {
    crate::presentation_store::PresentationStore::for_user(user)?.read_version(owner, reference)
}
