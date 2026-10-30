//! One Resident lifecycle per host. Execution never owns AppState's lock.
use super::*;
use crate::agent_stream::{RunDescriptor, RunSnapshot, RunStream};
use runtime::run_context::{CancellableAdapter, CancellationToken, ResidentStatePort, RunContext};
use runtime::run_events::{RunEventSink, RuntimeEvent};
use std::sync::{atomic::AtomicBool, Arc, Condvar, Mutex};

pub(crate) trait AppStatePort {
    fn with_app<R>(&self, operation: impl FnOnce(&mut AppState) -> R) -> R;
}

pub(crate) struct BorrowedAppPort<'a>(pub std::cell::RefCell<&'a mut AppState>);
impl AppStatePort for BorrowedAppPort<'_> {
    fn with_app<R>(&self, operation: impl FnOnce(&mut AppState) -> R) -> R {
        operation(&mut self.0.borrow_mut())
    }
}
impl AppStatePort for Arc<Mutex<AppState>> {
    fn with_app<R>(&self, operation: impl FnOnce(&mut AppState) -> R) -> R {
        operation(&mut self.lock().unwrap_or_else(|error| error.into_inner()))
    }
}

/// Private authority only; independent of the local or network Reader host.
pub(crate) trait UserStatePort {
    fn check_access(&self) -> Result<(), ToolError> { Ok(()) }
    fn with_user<R>(&self, operation: impl FnOnce(&mut crate::user_runtime::UserRuntime) -> R) -> R;
}
impl<P: AppStatePort> UserStatePort for P {
    fn with_user<R>(&self, operation: impl FnOnce(&mut crate::user_runtime::UserRuntime) -> R) -> R {
        self.with_app(|state| operation(&mut state.user))
    }
}
pub(crate) struct NetworkUserPort {
    pub user: crate::user_registry::UserHandle,
    pub access: Arc<crate::authorization::Authorization>,
    pub owner: String,
    pub publication: Option<crate::published_library::PublishedBookRef>,
}
impl UserStatePort for NetworkUserPort {
    fn check_access(&self) -> Result<(), ToolError> {
        self.publication.as_ref().map(|reference|self.access.library.lock().unwrap().authorize(&self.owner,reference).map_err(|_|crate::authorization::missing())).transpose().map(|_|())
    }
    fn with_user<R>(&self, operation: impl FnOnce(&mut crate::user_runtime::UserRuntime) -> R) -> R {
        operation(&mut self.user.lock().unwrap())
    }
}

pub(crate) struct RuntimeStatePort<'a, P> {
    pub port: &'a P,
    pub turn_ref: &'a AgentTurnRef,
    pub scope: &'a crate::run_scope::RunScope,
    pub previewed: std::collections::HashMap<String, std::collections::HashSet<String>>,
    pub animations: std::collections::HashMap<String, crate::presentation_animation::RenderedAnimation>,
    pub plots: std::collections::HashMap<String, crate::presentation_plot::PlotAsset>,
}
impl<P: AppStatePort> RuntimeStatePort<'_, P> {
    pub(crate) fn with_private<R>(&self, operation: impl FnOnce(&PrivateBookContext<'_>) -> Result<R, ToolError>) -> Result<R, ToolError> {
        self.port.with_app(|state| {
            self.scope.check_owner(state)?;
            operation(&self.scope.private_context(state))
        })
    }
}
impl<P: AppStatePort> ResidentStatePort for RuntimeStatePort<'_, P> {
    fn personal_note_context(&mut self)->Result<Value,ToolError>{
        self.port.with_app(|state|{self.scope.check_owner(state)?;crate::personal_note_context::for_turn(&mut state.user,&self.scope.chat_session_id,&self.scope.turn_id)})
    }
    fn personal_note_search(&mut self,request:Value)->Result<Value,ToolError>{
        let prepared=self.port.with_app(|state|{self.scope.check_owner(state)?;crate::personal_note_retrieval::prepare(&state.user.store,&request)})?;
        let vectors=crate::personal_note_retrieval::infer(&prepared);
        self.port.with_app(|state|{self.scope.check_owner(state)?;crate::personal_note_retrieval::finish(&state.user.store,prepared,vectors)})
    }
    fn workspace_records_available(&self) -> bool { true }
    fn workspace_records(&mut self, request: Value) -> Result<Value, ToolError> {
        self.port.with_app(|state| {
            self.scope.check_owner(state)?;
            crate::workspace_record_context::read_scoped(&state.user, self.scope.learning_workspace_id.as_deref(), &self.scope.reading_book().base.book_id, request)
        })
    }
    fn board_authoring_available(&self) -> bool { self.scope.board_context.is_some() }
    fn author_board(&mut self, request: runtime::board_author::AuthorRequest, bindings: &[SourceBinding], messages: &[Message], cancellation: &CancellationToken) -> Result<runtime::board_author::AuthorResult, ToolError> {
        self.author_board_candidate(request, bindings, messages, cancellation)
    }
    fn music_source_context(&mut self) -> Result<Option<Value>, ToolError> {
        self.port.with_app(|state| self.scope.check_owner(state))?;
        crate::music_interact::source_context(self.scope)
    }
    fn read_music(&mut self, reference: Option<&base_schema::PublishedBookRef>, request: read_tools::music::MusicRead) -> Result<read_tools::music::MusicReadResult, ToolError> {
        self.port.with_app(|state| self.scope.check_owner(state))?;
        let declared = crate::music_interact::publication(&self.scope.book_dir)?;
        if reference.is_some_and(|r| r != &declared) { return Err(runtime::music::unavailable()); }
        let manifest = self.scope.reading_book().load_music_source(&self.scope.book_dir)?;
        read_tools::music::read(&manifest, &declared, &request)
    }
    fn interact_music(&mut self, request: runtime::music::MusicInteractRequest, cancellation: &CancellationToken) -> Result<runtime::music::MusicPlaybackReceipt, ToolError> {
        let stream = self.port.with_app(|state| {
            self.scope.check_workspace(state)?;
            state.workspace.active_agent_stream.as_ref().and_then(|s| s.upgrade()).ok_or_else(runtime::music::unavailable)
        })?;
        crate::music_interact::execute(&stream, request, cancellation,
            |commands, request| commands.prepare(self.scope, self.turn_ref, request),
            || self.port.with_app(|state| self.scope.check_workspace(state)))
    }

    fn interact_presentation(&mut self, request: runtime::presentation_interact::InteractRequest, cancellation: &CancellationToken) -> Result<Value, ToolError> {
        let stream = self.port.with_app(|state| {
            self.scope.check_workspace(state)?;
            state.workspace.active_agent_stream.as_ref().and_then(|s| s.upgrade()).ok_or_else(runtime::presentation_interact::unavailable)
        })?;
        crate::presentation_interact::execute(&stream, request, cancellation,
            |commands, request| self.with_private(|state| crate::presentation_interact::prepare(commands, state, self.turn_ref, request)),
            || self.port.with_app(|state| self.scope.check_workspace(state)))
    }

    fn library_source(&mut self, reference: &base_schema::PublishedBookRef) -> Result<runtime::library::LibrarySource, ToolError> {
        self.port.with_app(|state| self.scope.check_owner(state))?;
        crate::video_frames::require_reference(self.scope, Some(reference))?;
        Ok(runtime::library::LibrarySource { reference: reference.clone(), book: self.scope.reading_book().clone(), title: "当前资料".into() })
    }

    fn record_presentation_methods(&mut self, observations: &runtime::presentation::PresentationMethodObservations) -> Result<(), ToolError> {
        self.with_private(|state| state.record_presentation_methods(&self.turn_ref.session_id, &self.turn_ref.turn_id, observations))
    }
    fn media_source_snapshot(&mut self, target: base_schema::EvidenceTarget) -> Result<runtime::video_source::MediaSource, ToolError> {
        self.port.with_app(|state| self.scope.check_owner(state))?;
        crate::video_source::snapshot(&self.scope.reading_book(), &self.scope.book_dir, target)
    }
    fn media_source_for_text(&mut self, range: &EvidenceRange) -> Result<Option<runtime::video_source::MediaSource>, ToolError> {
        self.port.with_app(|state| self.scope.check_owner(state))?;
        crate::video_source::text_snapshot(&self.scope.reading_book(), &self.scope.book_dir, range)
    }
    fn video_source_context(&mut self) -> Result<Option<Value>, ToolError> {
        self.port.with_app(|state| self.scope.check_owner(state))?;
        crate::video_frames::source_context(self.scope)
    }
    fn review_video_audio(&mut self, reference: Option<&base_schema::PublishedBookRef>, request: runtime::video_audio::ReviewAudio, cancellation: &CancellationToken) -> Result<runtime::video_audio::AudioReview, ToolError> {
        crate::video_frames::require_reference(self.scope, reference)?;
        self.port.with_app(|state| self.scope.check_owner(state))?;
        let result = crate::video_audio::review_for_run(self.scope, request, cancellation)?;
        self.port.with_app(|state| self.scope.check_owner(state))?;
        Ok(result)
    }
    fn search_video(&mut self, reference: Option<&base_schema::PublishedBookRef>, request: read_tools::video_search::VideoSearch) -> Result<Value, ToolError> {
        crate::video_frames::require_reference(self.scope, reference)?;
        self.port.with_app(|state| self.scope.check_owner(state))?;
        crate::video_frames::search_for_run(self.scope, request)
    }
    fn read_video_frames(&mut self, reference: Option<&base_schema::PublishedBookRef>, request: runtime::video::ReadFrames, cancellation: &CancellationToken) -> Result<runtime::video::FrameBatch, ToolError> {
        crate::video_frames::require_reference(self.scope, reference)?;
        self.port.with_app(|state| self.scope.check_owner(state))?;
        crate::video_frames::read_for_run(self.scope, request, cancellation)
    }
    fn tutor_assessment_input(&mut self, action: &str) -> Result<Value, ToolError> {
        self.with_private(|state| teaching::assessment_input(state, self.turn_ref, action))
    }
    fn tutor_assessment_accept(&mut self, action: &str, items: Value) -> Result<Value, ToolError> {
        self.with_private(|state| teaching::assessment_accept(state, self.turn_ref, action, items))
    }
    fn tutor_active(&mut self) -> Result<bool, ToolError> {
        self.with_private(|state| teaching::turn_active(state, &self.turn_ref.turn_id))
    }
    fn tutor_step_preflight(&self, request: &Value, evidence: &[SourceBinding], ranges: &[EvidenceRange]) -> Result<Value, runtime::run_context::TeachingSelectionPreflightError> {
        self.port.with_app(|state| {
            self.scope.check_owner(state)?;
            teaching::preflight_step(&self.scope.private_context(state), self.turn_ref, request, evidence, ranges)
        })
    }
    fn tutor_step(&mut self, request: Value, evidence: &[SourceBinding], ranges: &[EvidenceRange]) -> Result<Value, ToolError> {
        self.with_private(|state| teaching::step(state, self.turn_ref, request, evidence, ranges))
    }
    fn tutor_record_review(&mut self, record: Value) -> Result<Value, ToolError> {
        self.with_private(|state| teaching::record_agent_review(state,self.turn_ref,record))
    }
    fn tutor_working_state(&mut self) -> Result<Value, ToolError> {
        self.with_private(|state| teaching::agent_working_state(state,self.turn_ref))
    }
    fn tutor_coach_context(&mut self) -> Result<Option<Value>, ToolError> {
        self.with_private(|state| crate::teaching_coach::turn_context(state.user, state.learning_workspace_id, self.turn_ref))
    }
    fn persist_goal(&mut self, goal: &runtime::goal::ResidentGoal) -> Result<(), ToolError> {
        self.port.with_app(|state| {
            self.scope.check_owner(state)?;
            if state.user.session_store.is_some() {
                return crate::session_runtime::append(&mut state.user, self.turn_ref,
                    &crate::multi_user_host::now().to_string(), crate::session_event::EventBody::GoalUpdated { goal: goal.clone() });
            }
            let mut candidate = state.user.agent_history.clone();
            let session = candidate.sessions.iter_mut().find(|s| s.id == self.turn_ref.session_id)
                .ok_or_else(|| agent_history_internal("Goal session disappeared"))?;
            let stored = session.goals.iter_mut().find(|g| g.id == goal.id)
                .ok_or_else(|| agent_history_internal("Goal disappeared"))?;
            if stored.revision > goal.revision {
                return Err(agent_history_internal("Goal revision moved ahead of this run"));
            }
            *stored = goal.clone();
            commit_agent_history_candidate(state, candidate)
        })
    }
    fn author_presentation(
        &mut self,
        request: runtime::presentation_author::AuthorRequest,
        bindings: &[SourceBinding],
        messages: &[Message],
        cancellation: &CancellationToken,
    ) -> Result<runtime::presentation_author::AuthorResult, ToolError> {
        self.author(request, bindings, messages, cancellation)
    }
    fn submit_private<R>(&mut self, operation: impl FnOnce(&mut MemoryStore) -> R) -> Result<R, ToolError> {
        self.port.with_app(|state| {
            self.scope.check_owner(state)?;
            Ok(operation(&mut state.user.store))
        })
    }
    fn read_live_reader<R>(&mut self, operation: impl FnOnce(&Reader) -> R) -> Result<R, ToolError> {
        self.port.with_app(|state| {
            self.scope.check_workspace(state)?;
            Ok(operation(&state.workspace.reader))
        })
    }
    fn apply_reader<R>(&mut self, operation: impl FnOnce(&mut MemoryStore, &mut Reader) -> R) -> Result<R, ToolError> {
        self.port.with_app(|state| {
            self.scope.check_workspace(state)?;
            let before = state.workspace.reader.revision();
            let result = operation(&mut state.user.store, &mut state.workspace.reader);
            if state.workspace.reader.revision() != before {
                if let Some(stream) = state.workspace.active_agent_stream.as_ref().and_then(|s| s.upgrade()) {
                    stream.reader_changed(reader_state_response(&self.scope.reading_book(), &state.workspace.reader));
                }
            }
            Ok(result)
        })
    }
    fn reader_input(&mut self, _: &Book, _: &str) -> runtime::run_context::ReaderInputSnapshot {
        self.scope.reader_input.clone().expect("reading scope")
    }

}

pub(crate) struct RunCheckpointSink<'a, P> {
    pub port: &'a P,
    pub turn_ref: &'a AgentTurnRef,
}
impl<P: UserStatePort> CompactionCheckpointSink for RunCheckpointSink<'_, P> {
    fn persist_progress(&mut self, messages: &[Message], activities: &[runtime::run_events::RunActivity]) -> Result<(), ToolError> {
        self.port.with_user(|user| crate::session_runtime::progress(user, self.turn_ref, messages, activities))
    }
    fn persist_effects(&mut self, effects: &[AgentEffect]) -> Result<(), ToolError> {
        self.port.with_user(|user| session_runtime::deliver_effects(user, self.turn_ref, effects))
    }
    fn prepare_persisted_messages(&mut self, messages: &mut [Message]) {
        self.port.with_user(|user| {
            if user.session_store.is_some() { crate::session_runtime::clean(messages); }
        });
    }
    fn install(
        &mut self,
        checkpoint: &CompactionCheckpoint,
        messages: &[Message],
    ) -> Result<(), CompactionError> {
        self.port.with_user(|user| {
            let active = user
                .agent_history
                .sessions
                .iter()
                .find(|session| session.id == self.turn_ref.session_id)
                .and_then(|session| {
                    session
                        .turns
                        .iter()
                        .find(|turn| turn.turn_id == self.turn_ref.turn_id)
                })
                .is_some_and(|turn| turn.status == AgentAssistantStatus::PendingAssistant);
            if !active {
                return Err(CompactionError {
                    error_code: "COMPACTION_FAILED".into(),
                    message: "run is no longer pending".into(),
                });
            }
            if user.session_store.is_some() {
                return crate::session_runtime::checkpoint(user, self.turn_ref, checkpoint, messages)
                    .map_err(|e| CompactionError { error_code: "COMPACTION_FAILED".into(), message: e.message });
            }
            ServerAgentCompactionCheckpointSink {
                history_path: &user.history_path,
                agent_history: &mut user.agent_history,
                session_id: &self.turn_ref.session_id,
            }
            .install(checkpoint, messages)
        })
    }
}

pub(crate) struct PreparedAgentChat {
    pub response_language: Option<runtime::response_language::ResponseLanguage>,
    pub exam_calendar: Option<Value>,
    pub accepted_workspace_goal: Option<base_schema::WorkspaceGoal>,
    pub tutor: Option<Value>,
    pub goal: Option<runtime::goal::ResidentGoal>,
    pub scope: crate::run_scope::RunScope,
    pub turn_ref: AgentTurnRef,
    pub message: String,
    pub agent_message: String,
    pub messages: Vec<Message>,
    pub now: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentRunSummary {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) usage: Option<crate::service_limits::RunUsage>,
    pub effects: Vec<runtime::orchestrator::AgentEffect>,
    pub trace: Vec<runtime::orchestrator::TraceStep>,
    #[serde(default)]
    pub activities: Option<Vec<runtime::run_events::RunActivity>>,
    #[serde(default)]
    pub last_seq: Option<u64>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct UnsavedRun {
    pub book_id: String,
    pub session_id: String,
    pub turn_id: String,
    pub summary: AgentRunSummary,
    pub outcome: Option<OuterOutcome>,
    pub error: AgentTurnError,
}

pub(crate) struct ExecutionReport {
    pub reply: Reply,
    unsaved: Option<UnsavedRun>,
}
impl ExecutionReport {
    fn saved(reply: Reply) -> Self {
        Self {
            reply,
            unsaved: None,
        }
    }
}

pub(crate) fn execute_prepared(
    port: &impl AppStatePort,
    adapter: &dyn ModelAdapter,
    prepared: PreparedAgentChat,
    cancellation: CancellationToken,
) -> ExecutionReport {
    let turn=prepared.turn_ref.clone();
    let workspace=prepared.scope.learning_workspace_id.clone();
    let report=execute_observed(port, adapter, prepared, cancellation.clone(), None, None);
    if let Err(error)=crate::teaching_coach::drive_inline(port,adapter,workspace.as_deref(),&turn,true,&cancellation) {
        eprintln!("Teaching coach persistence: {}",error.error_code);
    }
    report
}

/// Test-only streaming entry point with the same inline coach envelope as execute_prepared.
#[cfg(test)]
pub(crate) fn execute_prepared_with_stream_for_test(
    port: &impl AppStatePort,
    adapter: &dyn ModelAdapter,
    prepared: PreparedAgentChat,
    cancellation: CancellationToken,
    stream: &Arc<RunStream>,
) -> ExecutionReport {
    let turn = prepared.turn_ref.clone();
    let workspace = prepared.scope.learning_workspace_id.clone();
    let report = execute_observed(port, adapter, prepared, cancellation.clone(), Some(stream), None);
    if let Err(error) = crate::teaching_coach::drive_inline(
        port, adapter, workspace.as_deref(), &turn, true, &cancellation,
    ) {
        eprintln!("Teaching coach persistence: {}", error.error_code);
    }
    report
}

/// Shared model/cancellation/history-redaction envelope for local and network Runs.
pub(crate) struct FinishedRun {
    pub result: Result<OuterOutcome, ToolError>,
    pub cancelled: bool,
    pub summary: AgentRunSummary,
    pub messages: Vec<Message>,
    pub active_model_projection: Option<runtime::active_projection::ActiveModelProjection>,
}
pub(crate) fn execute_model(
    adapter: &dyn ModelAdapter,
    prepared: &PreparedAgentChat,
    cancellation: CancellationToken,
    stream: Option<&Arc<RunStream>>,
    event_sink: Option<Arc<dyn RunEventSink>>,
    event_anchor: std::time::Instant,
    run: impl FnOnce(&dyn ModelAdapter, &PreparedAgentChat, &mut RunContext) -> Result<OuterOutcome, ToolError>,
) -> FinishedRun {
    let mut messages = prepared.messages.clone();
    runtime::presentation_author::redact_history(&mut messages);
    runtime::tool_exposure::redact_history(&mut messages);
    let mut context = RunContext::new(
        messages,
        OuterConfig::default(),
        prepared.scope.provider_binding.clone(),
    );
    context.tutor = prepared.tutor.clone();
    context.board_request_context = prepared.scope.board_context.as_ref()
        .map(crate::board_api::FrozenBoardFollowUp::request_context);
    context.workspace_goal = prepared.scope.learning_workspace_id.as_ref()
        .zip(prepared.scope.goal.as_ref())
        .map(|(id, goal)| runtime::workspace_goal::WorkspaceGoalContext {
            learning_workspace_id: id.clone(), goal: goal.clone(),
            exam_calendar: prepared.exam_calendar.clone(),
        });
    context.accepted_workspace_goal = prepared.accepted_workspace_goal.clone();
    context.goal = prepared.goal.clone();
    context.current_user_message = Some(prepared.message.clone());
    context.response_language = prepared.response_language;
    context.cancellation = cancellation.clone();
    context.events = runtime::run_events::RunEvents::with_start(
        event_anchor,
        event_sink,
    );
    adapter.set_run_cancellation(cancellation.clone());
    let observed = runtime::run_events::ObservedAdapter {
        inner: adapter,
        events: context.events.clone(),
        cancellation: cancellation.clone(),
        runtime_profile: context.runtime_profile.clone(),
    };
    let adapter = CancellableAdapter {
        inner: &observed,
        cancellation: cancellation.clone(),
    };
    let result = run(&adapter, prepared, &mut context);
    let spend_stop = result.as_ref().err()
        .filter(|error| error.category == "model_spend")
        .and_then(|error| runtime::model_spend::SpendStop::from_code(&error.error_code));
    let cancelled = cancellation.is_cancelled() && spend_stop.is_none();
    if let Some(reason) = spend_stop {
        // Pre-turn compaction can stop before the accepted question is appended.
        if !context.messages.iter().enumerate().any(|(index, message)|
            index >= prepared.messages.len() && message.role == runtime::Role::User) {
            context.messages.push(Message::user(prepared.agent_message.clone()));
        }
        context.close_stopped_tool_calls(reason.code(), "model_spend", "Tool was not executed because model spending stopped");
    }
    if cancelled {
        context.close_cancelled_tool_calls();
    }
    let active_model_projection = if result.is_ok() || cancelled || spend_stop.is_some() {
        context.finished_model_projection()
    } else { None };
    runtime::presentation_author::redact_history(&mut context.messages);
    runtime::tool_exposure::redact_history(&mut context.messages);
    if let Some(stream) = stream {
        stream.finalizing();
    }
    let last_seq = stream.map(|s| s.snapshot().last_seq + 2);
    let summary = match &result {
        Ok(outcome) => AgentRunSummary {
            usage: None,
            effects: outcome.effects.clone(),
            trace: outcome.trace.clone(),
            activities: Some(context.events.activities()),
            last_seq,
        },
        Err(_) => AgentRunSummary {
            usage: None,
            effects: runtime::orchestrator::run_effects(context.effects, &context.navigation),
            trace: context.trace,
            activities: Some(context.events.activities()),
            last_seq,
        },
    };
    let result = if cancelled {
        Err(cancellation.check().unwrap_err())
    } else {
        result
    };
    // A provider/protocol failure has no committed assistant answer. Its tool receipts carry
    // run-local state (deferred activations, evidence bindings and candidate
    // handles), so feeding that suffix into the next run makes expired state
    // look reusable. Keep the user's question for an ordinary "retry" follow-up,
    // while retaining the detailed failure trace out of band in run_summary. A
    // cancelled or spend-stopped run keeps committed receipts and closes its
    // unexecuted suffix, so explicit continuation sees the effects already made.
    let persisted_messages = if result.is_ok() || cancelled || spend_stop.is_some() {
        context.messages.clone()
    } else {
        let mut messages = context.messages.clone();
        if let Some(current_user) = messages
            .iter()
            .rposition(|message| message.role == runtime::Role::User)
            .filter(|index| *index >= prepared.messages.len())
        {
            messages.truncate(current_user + 1);
        } else {
            // Pre-turn compaction can fail before this question is appended.
            // Keep the last completed answer and retain the accepted question.
            messages = prepared.messages.clone();
            messages.push(Message::user(prepared.agent_message.clone()));
            runtime::presentation_author::redact_history(&mut messages);
            runtime::tool_exposure::redact_history(&mut messages);
        }
        messages
    };
    FinishedRun { result, cancelled, summary, messages: persisted_messages, active_model_projection }
}

fn execute_observed(
    port: &impl AppStatePort,
    adapter: &dyn ModelAdapter,
    prepared: PreparedAgentChat,
    cancellation: CancellationToken,
    stream: Option<&Arc<RunStream>>,
    observability: Option<&Arc<crate::observability::ObservabilityRuntime>>,
) -> ExecutionReport {
    let observation_run = observability.and_then(|runtime| {
        runtime.start_run_with_input(prepared.scope.book_id(), &prepared.turn_ref.session_id,
            || json!({"message":prepared.message,"messages":prepared.messages}))
    });
    let observation_sink = observation_run.as_ref().map(|run| run.sink());
    let event_sink: Option<Arc<dyn RunEventSink>> = match (stream, observation_sink) {
        (Some(stream), Some(observation)) => Some(Arc::new(RunEventFanout {
            stream: stream.clone(),
            observation,
        })),
        (Some(stream), None) => Some(stream.clone()),
        (None, Some(observation)) => Some(observation),
        (None, None) => None,
    };
    let FinishedRun { result, cancelled, summary, messages: persisted_messages, active_model_projection } = execute_model(
        adapter, &prepared, cancellation, stream, event_sink,
        observation_run.as_ref().map(|r| r.event_anchor()).unwrap_or_else(std::time::Instant::now),
        |adapter, prepared, context| {
            port.with_app(|state| crate::teaching_coach::prepare_execution(&state.user,
                prepared.scope.learning_workspace_id.as_deref(), &prepared.turn_ref))?;
            context.cancellation.check()?;
            run_precommitted_agent_chat(port, adapter, prepared, context)
        },
    );
    let observation_outcome = result.as_ref().ok().cloned();
    let observation_error_code = result
        .as_ref()
        .err()
        .map(|error| error.error_code.clone())
        .or_else(|| {
            observation_outcome
                .as_ref()
                .filter(|outcome| crate::observability::lifecycle::delivery_failed(outcome))
                .map(|_| "ANSWER_DELIVERY_FAILED".into())
        });
    let report = port.with_app(|state| {
        if prepared.scope.check_workspace(state).is_ok() && summary
            .effects
            .iter()
            .any(|effect| matches!(effect, runtime::orchestrator::AgentEffect::Goto { .. }))
        {
            save_session(state, None);
        }
        let (status, outcome, error) = match &result {
            Ok(outcome) => (AgentAssistantStatus::Completed, Some(outcome.clone()), None),
            Err(error) => (
                if cancelled {
                    AgentAssistantStatus::Cancelled
                } else {
                    AgentAssistantStatus::Failed
                },
                None,
                Some(AgentTurnError {
                    error_code: error.error_code.clone(),
                    category: error.category.clone(),
                    message: saved_error_message(error),
                }),
            ),
        };
        let saved_projection = session_runtime::model_projection(
            &mut state.user, &prepared.turn_ref, &persisted_messages, active_model_projection.as_ref());
        if let Err(error) = saved_projection.and_then(|()| finalize_agent_turn(
            state,
            &prepared.turn_ref,
            status,
            outcome.clone(),
            error,
            Some(summary.clone()),
            &persisted_messages,
            &prepared.now,
        )) {
            return ExecutionReport {
                reply: err_reply(&error),
                unsaved: Some(UnsavedRun {
                    book_id: prepared.scope.book_id().to_owned(),
                    session_id: prepared.turn_ref.session_id.clone(),
                    turn_id: prepared.turn_ref.turn_id.clone(),
                    summary,
                    outcome,
                    error: AgentTurnError {
                        error_code: error.error_code,
                        category: error.category,
                        message: error.message,
                    },
                }),
            };
        }
        if let Err(error) = board_delivery::ensure_saved(&mut state.user, &prepared.turn_ref, &prepared.now) {
            return ExecutionReport {
                reply: err_reply(&error),
                unsaved: Some(UnsavedRun {
                    book_id: prepared.scope.book_id().to_owned(),
                    session_id: prepared.turn_ref.session_id.clone(),
                    turn_id: prepared.turn_ref.turn_id.clone(),
                    summary,
                    outcome,
                    error: AgentTurnError { error_code: error.error_code, category: error.category, message: error.message },
                }),
            };
        }
        if let Err(error) = if prepared.scope.book.is_some() {teaching::record_delivery(&prepared.scope.private_context(state), &prepared.turn_ref, &prepared.now)} else {Ok(())} {
            return ExecutionReport::saved(err_reply(&error));
        }
        if let Err(error) = session_runtime::link_teaching(&mut state.user, &prepared.turn_ref, &prepared.now) {
            return ExecutionReport::saved(err_reply(&error));
        }
        if prepared.scope.check_workspace(state).is_ok()
        {
            state.workspace.messages = persisted_messages;
        }
        if let Err(error) = reconcile_agent_history_review_jobs(state, &prepared.now) {
            return ExecutionReport::saved(err_reply(&error));
        }
        ExecutionReport::saved(match result {
            Ok(outcome) => {
                let mut value = serde_json::to_value(&outcome).expect("serializable outcome");
                if let Some(turn) = state.user.agent_history.sessions.iter().find(|s| s.id == prepared.turn_ref.session_id)
                    .and_then(|s| s.turns.iter().find(|t| t.turn_id == prepared.turn_ref.turn_id)) {
                    if turn.board_follow_up.is_some() {
                        value["board_follow_up"] = json!(turn.board_follow_up);
                        value["board_delivery"] = json!(turn.board_delivery);
                        value["board_delivery_error"] = json!(turn.board_delivery_error);
                        value["board_commits"] = json!(session_runtime::board_commits(turn));
                    }
                }
                ok_json(&value)
            },
            Err(error) => err_reply(&error),
        })
    });
    if let Some(run) = observation_run {
        run.finish(
            observation_outcome.as_ref(),
            cancelled,
            if report.unsaved.is_some() {
                runtime::observation::PersistenceState::Failed
            } else {
                runtime::observation::PersistenceState::Saved
            },
            observation_error_code.as_deref().or_else(|| report.unsaved.as_ref().map(|run| run.error.error_code.as_str())),
        );
    }
    report
}

pub(crate) struct RunEventFanout {
    pub(crate) stream: Arc<RunStream>,
    pub(crate) observation: Arc<dyn RunEventSink>,
}

impl RunEventSink for RunEventFanout {
    fn captures_content(&self) -> bool { self.observation.captures_content() }
    fn activity_input(&self, step_id: u32, input: Value) {
        self.observation.activity_input(step_id, input);
    }
    fn activity_output(&self, step_id: u32, output: Value) {
        self.observation.activity_output(step_id, output);
    }
    fn emit(&self, event: RuntimeEvent) {
        self.stream.emit(event.clone());
        self.observation.emit(event);
    }

    fn answer_patch(&self, patch: runtime::answer_stream::AnswerPatch) {
        self.stream.answer_patch(patch);
    }

    fn board_committed(&self, delivery: &memory::boards::BoardDelivery) {
        self.stream.board_committed(delivery);
        self.observation.board_committed(delivery);
    }

    fn answer_first_patch(&self, elapsed_ms: f64) {
        self.observation.answer_first_patch(elapsed_ms);
    }


    fn source_bindings(&self, bindings: &[runtime::orchestrator::SourceBinding]) {
        self.stream.source_bindings(bindings);
    }

    fn effect_created(&self, step_id: u32, effect: &runtime::orchestrator::AgentEffect) {
        self.stream.effect_created(step_id, effect);
    }
}

struct ActiveRun {
    stream: Arc<RunStream>,
    turn_ref: AgentTurnRef,
    cancellation: CancellationToken,
}
#[derive(Default)]
struct Slot {
    active: Option<ActiveRun>,
    boundary: bool,
    stopping: bool,
}

pub struct RunCoordinator {
    state: Arc<Mutex<AppState>>,
    slot: Mutex<Slot>,
    exited: Condvar,
    host_stop: Arc<AtomicBool>,
    unsaved: Mutex<Option<UnsavedRun>>,
    unsaved_stream: Mutex<Option<Arc<RunStream>>>,
    unsaved_book: Mutex<Option<Arc<Book>>>,
    observability: Arc<crate::observability::ObservabilityRuntime>,
}

impl RunCoordinator {
    pub fn new(
        state: Arc<Mutex<AppState>>,
        host_stop: Arc<AtomicBool>,
        observability: Arc<crate::observability::ObservabilityRuntime>,
    ) -> Self {
        Self {
            state,
            slot: Mutex::new(Slot::default()),
            exited: Condvar::new(),
            host_stop,
            unsaved: Mutex::new(None),
            unsaved_stream: Mutex::new(None),
            unsaved_book: Mutex::new(None),
            observability,
        }
    }
    fn reserve(
        &self,
        body: &str,
        now: &str,
    ) -> Result<(PreparedAgentChat, CancellationToken, Arc<RunStream>), Reply> {
        let mut slot = self.slot.lock().unwrap_or_else(|error| error.into_inner());
        if let Some(reply) = self.state.with_app(|state| interaction_input::local_repeat(state, body))? {
            return Err(reply);
        }
        if slot.active.is_some() || slot.boundary || slot.stopping {
            return Err(Reply { status: 409, body: json!({ "error_code": "AGENT_RUN_BUSY", "category": "conflict",
                "message": "A Resident run or context transition is active", "turn_id": slot.active.as_ref().map(|r| &r.turn_ref.turn_id) }).to_string() });
        }
        let prepared = self
            .state
            .with_app(|state| prepare_agent_chat(state, body, now))?;
        let cancellation = CancellationToken::with_host_stop(self.host_stop.clone());
        let stream = RunStream::new(RunDescriptor {
            book_id: prepared.scope.book_id().to_owned(),
            session_id: prepared.turn_ref.session_id.clone(),
            turn_id: prepared.turn_ref.turn_id.clone(),
        });
        self.state
            .with_app(|state| state.workspace.active_agent_stream = Some(Arc::downgrade(&stream)));
        slot.active = Some(ActiveRun {
            turn_ref: prepared.turn_ref.clone(),
            cancellation: cancellation.clone(),
            stream: stream.clone(),
        });
        Ok((prepared, cancellation, stream))
    }
    fn execute(
        &self,
        prepared: PreparedAgentChat,
        cancellation: CancellationToken,
        stream: Arc<RunStream>,
        adapter: &dyn ModelAdapter,
        on_exit: impl FnOnce(),
    ) -> Reply {
        let _running = RunGuard(self);
        let descriptor = stream.snapshot().descriptor;
        let original_book = prepared.scope.book.clone();
        let result = execute_observed(
            &self.state,
            adapter,
            prepared,
            cancellation,
            Some(&stream),
            Some(&self.observability),
        );
        self.state.with_app(|state| {
            if let Some(turn) = state.user.agent_history.sessions.iter().find(|session| session.id == descriptor.session_id)
                .and_then(|session| session.turns.iter().find(|turn| turn.turn_id == descriptor.turn_id)) {
                stream.restore_board_commits(&session_runtime::board_commits(turn));
            }
        });
        if let Some(unsaved) = result.unsaved {
            *self.unsaved_book.lock().unwrap() = original_book.clone();
            stream.finish(None, Some(json!(unsaved.error)));
            *self.unsaved.lock().unwrap() = Some(unsaved);
            *self.unsaved_stream.lock().unwrap() = Some(stream);
        } else {
            let view = self.state.with_app(|state| {
                state
                    .user.agent_history
                    .sessions
                    .iter()
                    .find(|s| s.id == descriptor.session_id)
                    .and_then(|s| s.turns.iter().find(|t| t.turn_id == descriptor.turn_id))
                    .map(|turn| json!(scoped_turn_view(original_book.as_deref(), turn)))
            });
            stream.finish(view, None);
        }
        on_exit();
        result.reply
    }
    pub fn run(
        &self,
        body: &str,
        now: &str,
        adapter: &dyn ModelAdapter,
        on_started: impl FnOnce(),
        on_exit: impl FnOnce(),
    ) -> Reply {
        let (prepared, cancellation, stream) = match self.reserve(body, now) {
            Ok(run) => run,
            Err(reply) => return reply,
        };
        on_started();
        self.execute(prepared, cancellation, stream, adapter, on_exit)
    }
    pub fn start(
        self: &Arc<Self>,
        body: &str,
        now: &str,
        adapter: Box<dyn ModelAdapter + Send>,
        on_started: impl FnOnce(),
        on_exit: impl FnOnce() + Send + 'static,
    ) -> Reply {
        let (prepared, cancellation, stream) = match self.reserve(body, now) {
            Ok(run) => run,
            Err(reply) => return reply,
        };
        let descriptor = stream.snapshot().descriptor;
        on_started();
        let coordinator = self.clone();
        std::thread::spawn(move || {
            coordinator.execute(prepared, cancellation, stream, adapter.as_ref(), on_exit);
        });
        Reply {
            status: 202,
            body: json!(descriptor).to_string(),
        }
    }
    pub fn stream(&self, turn_id: &str) -> Option<Arc<RunStream>> {
        let retained = {
            let slot = self.slot.lock().unwrap();
            slot
                .active
                .as_ref()
                .filter(|r| r.turn_ref.turn_id == turn_id)
                .map(|active| active.stream.clone())
        }.or_else(|| self
            .unsaved_stream
            .lock()
            .unwrap()
            .as_ref()
            .filter(|s| s.snapshot().descriptor.turn_id == turn_id)
            .cloned());
        if let Some(stream) = retained {
            let descriptor = stream.snapshot().descriptor;
            self.state.with_app(|state| {
                if let Some(turn) = state.user.agent_history.sessions.iter().find(|session| session.id == descriptor.session_id)
                    .and_then(|session| session.turns.iter().find(|turn| turn.turn_id == turn_id)) {
                    stream.restore_board_commits(&session_runtime::board_commits(turn));
                }
            });
            return Some(stream);
        }
        self.state.with_app(|state| {
            state
                .user.agent_history
                .sessions
                .iter()
                .filter(|s| s.book_id == state.workspace.book.base.book_id)
                .find_map(|session| {
                    session
                        .turns
                        .iter()
                        .find(|t| t.turn_id == turn_id)
                        .map(|turn| {
                            let board_error = board_delivery::pending_error(turn);
                            let execution = match turn.status {
                                _ if board_error.is_some() => "ended",
                                AgentAssistantStatus::Completed => "completed",
                                AgentAssistantStatus::Cancelled => "cancelled",
                                AgentAssistantStatus::Failed => "failed",
                                AgentAssistantStatus::PendingAssistant => "interrupted",
                            };
                            RunStream::from_snapshot(RunSnapshot {
                                board_commits: session_runtime::board_commits(turn),
                                music_command: None,
                                presentation_command: None,
                                descriptor: RunDescriptor {
                                    book_id: session.book_id.clone(),
                                    session_id: session.id.clone(),
                                    turn_id: turn.turn_id.clone(),
                                },
                                last_seq: turn
                                    .run_summary
                                    .as_ref()
                                    .and_then(|s| s.last_seq)
                                    .unwrap_or(0),
                                execution_state: execution.into(),
                                persistence_state: if board_error.is_some() || turn.status
                                    == AgentAssistantStatus::PendingAssistant
                                {
                                    "failed"
                                } else {
                                    "saved"
                                }
                                .into(),
                                activities: turn
                                    .run_summary
                                    .as_ref()
                                    .and_then(|s| s.activities.clone())
                                    .unwrap_or_default(),
                                reader_state: None,
                                effects: Vec::new(),
                                draft: None,
                                final_view: Some(json!(turn_view(&state.workspace.book, turn))),
                                error: board_error.map(|error| json!(error)),
                            })
                        })
                })
        })
    }
    pub fn cancel_run(&self, turn_id: &str) -> Option<RunSnapshot> {
        {
            let slot = self.slot.lock().unwrap();
            if let Some(active) = slot
                .active
                .as_ref()
                .filter(|r| r.turn_ref.turn_id == turn_id)
            {
                active.cancellation.cancel();
                active.stream.cancelling();
            }
        }
        self.stream(turn_id).map(|s| s.snapshot())
    }
    /// Retry only the persisted board delivery, never the model or the accepted question.
    pub(crate) fn retry_save(&self, turn_id: &str, now: &str) -> Result<RunSnapshot, ToolError> {
        if self.slot.lock().unwrap().active.as_ref().is_some_and(|run| run.turn_ref.turn_id == turn_id) {
            return Err(user_storage_paths::error("AGENT_RUN_BUSY", "conflict", "请等待当前回答完成"));
        }
        let snapshot = self.state.with_app(|state| {
            let (reference, book_id) = state.user.agent_history.sessions.iter().find_map(|session|
                session.turns.iter().find(|turn| turn.turn_id == turn_id).map(|turn| (
                    AgentTurnRef { session_id: session.id.clone(), turn_id: turn.turn_id.clone(), user_turn_ordinal: turn.user_turn_ordinal },
                    session.book_id.clone(),
                ))).ok_or_else(|| user_storage_paths::error("AGENT_RUN_NOT_FOUND", "not_found", "Run does not exist"))?;
            if let Some(store) = state.user.session_store.as_mut() {
                store.settle(&mut state.user.agent_history, &reference.session_id)?;
            }
            let turn = state.user.agent_history.sessions.iter().find(|s| s.id == reference.session_id).unwrap()
                .turns.iter().find(|t| t.turn_id == turn_id).unwrap();
            if turn.status == AgentAssistantStatus::PendingAssistant || turn.domain.board_context.is_none() {
                return Err(user_storage_paths::error("TURN_NOT_UNSAVED", "conflict", "此回答没有可补存的画板交付"));
            }
            board_delivery::ensure_saved(&mut state.user, &reference, now)?;
            let turn = state.user.agent_history.sessions.iter().find(|s| s.id == reference.session_id).unwrap()
                .turns.iter().find(|t| t.turn_id == turn_id).unwrap();
            Ok(RunSnapshot {
                board_commits: session_runtime::board_commits(turn),
                music_command: None, presentation_command: None,
                descriptor: RunDescriptor { book_id, session_id: reference.session_id, turn_id: turn_id.into() },
                last_seq: turn.run_summary.as_ref().and_then(|summary| summary.last_seq).unwrap_or(0),
                execution_state: match turn.status {
                    AgentAssistantStatus::Completed => "completed", AgentAssistantStatus::Cancelled => "cancelled",
                    AgentAssistantStatus::Failed => "failed", AgentAssistantStatus::PendingAssistant => unreachable!(),
                }.into(), persistence_state: "saved".into(),
                activities: turn.run_summary.as_ref().and_then(|summary| summary.activities.clone()).unwrap_or_default(),
                reader_state: None, effects: Vec::new(), draft: None,
                final_view: Some(json!(turn_view(&state.workspace.book, turn))), error: None,
            })
        })?;
        let mut unsaved = self.unsaved.lock().unwrap();
        if unsaved.as_ref().is_some_and(|run| run.turn_id == turn_id) {
            *unsaved = None;
            *self.unsaved_book.lock().unwrap() = None;
            if let Some(stream) = self.unsaved_stream.lock().unwrap().take() {
                stream.finish(snapshot.final_view.clone(), None);
            }
        }
        Ok(snapshot)
    }
    pub fn unsaved_run(&self) -> Option<UnsavedRun> {
        self.unsaved
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }
    /// Requests stop; the slot remains owned until execution AND persistence exit.
    pub fn cancel(&self) -> Option<String> {
        let slot = self.slot.lock().unwrap_or_else(|error| error.into_inner());
        slot.active.as_ref().map(|active| {
            active.cancellation.cancel();
            active.stream.cancelling();
            active.turn_ref.turn_id.clone()
        })
    }
    pub fn with_boundary<R>(&self, operation: impl FnOnce() -> R) -> R {
        let mut slot = self.slot.lock().unwrap_or_else(|error| error.into_inner());
        while slot.boundary {
            slot = self
                .exited
                .wait(slot)
                .unwrap_or_else(|error| error.into_inner());
        }
        slot.boundary = true;
        if let Some(active) = &slot.active {
            active.cancellation.cancel();
            active.stream.cancelling();
        }
        self.exited.notify_all();
        while slot.active.is_some() {
            slot = self
                .exited
                .wait(slot)
                .unwrap_or_else(|error| error.into_inner());
        }
        drop(slot);
        let _boundary = BoundaryGuard(self);
        operation()
    }
    #[cfg(test)]
    pub(crate) fn wait_until_cancelling(&self) {
        let slot = self.slot.lock().unwrap();
        let (_slot, result) = self
            .exited
            .wait_timeout_while(slot, std::time::Duration::from_secs(15), |slot| {
                !slot
                    .active
                    .as_ref()
                    .is_some_and(|active| active.cancellation.is_cancelled())
            })
            .unwrap();
        assert!(!result.timed_out(), "boundary never requested cancellation");
    }
    pub fn stop_and_wait(&self) {
        self.slot
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .stopping = true;
        self.with_boundary(|| ());
    }
    pub(crate) fn deletes_active_session(&self, body: &str) -> bool {
        let value: Value = serde_json::from_str(body).unwrap_or(Value::Null);
        let slot = self.slot.lock().unwrap_or_else(|error| error.into_inner());
        slot.active.as_ref().is_some_and(|active| {
            value["session_id"].as_str() == Some(active.turn_ref.session_id.as_str())
        })
    }
}
struct RunGuard<'a>(&'a RunCoordinator);
impl Drop for RunGuard<'_> {
    fn drop(&mut self) {
        self.0
            .slot
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .active = None;
        self.0.exited.notify_all();
    }
}
struct BoundaryGuard<'a>(&'a RunCoordinator);
impl Drop for BoundaryGuard<'_> {
    fn drop(&mut self) {
        self.0
            .slot
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .boundary = false;
        self.0.exited.notify_all();
    }
}

pub(crate) fn recover_pending(
    history: &mut AgentHistory,
    path: &Option<PathBuf>,
) -> Result<(), ToolError> {
    recover_pending_except(history, path, &std::collections::BTreeSet::new())
}

pub(crate) fn recover_pending_except(history: &mut AgentHistory, path: &Option<PathBuf>, admitted: &std::collections::BTreeSet<String>) -> Result<(), ToolError> {
    let mut candidate = history.clone();
    let mut changed = false;
    for turn in candidate
        .sessions
        .iter_mut()
        .flat_map(|session| &mut session.turns)
    {
        if turn.status == AgentAssistantStatus::PendingAssistant && !admitted.contains(&turn.turn_id) {
            turn.status = AgentAssistantStatus::Failed;
            turn.error = Some(AgentTurnError {
                error_code: "INTERRUPTED".into(),
                category: "interrupted".into(),
                message: "The previous Reader process stopped before this run finished".into(),
            });
            changed = true;
        }
    }
    if changed {
        save_agent_history_path(path, &candidate)?;
        *history = candidate;
    }
    Ok(())
}

/// Used only by terminal history candidates; unsaved responses keep the execution reason separately.
pub(crate) fn saved_error_message(error: &ToolError) -> String {
    if error.category == "model_spend" && runtime::model_spend::SpendStop::from_code(&error.error_code).is_some() {
        format!("{} 本次已完成内容已保存。", error.message)
    } else { error.message.clone() }
}
