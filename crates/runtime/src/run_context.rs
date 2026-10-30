//! Turn-owned data and the operation-scoped authoritative Reader/Memory port.
use crate::{
    orchestrator::{
        AnswerProvenanceLedger, OuterConfig, TurnEvidenceLedger, TurnEvidencePlanLedger,
        TurnLocatorLedger, TurnProgressLedger,
    },
    tool_exposure::ToolExposureState,
    tool_result::ActiveToolResultLedger,
    Message, ModelRuntimeProfile,
};
use memory::MemoryStore;
use reader::Reader;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

#[derive(Clone, Default)]
pub struct CancellationToken {
    requested: Arc<AtomicBool>,
    host_stop: Option<Arc<AtomicBool>>,
}

impl CancellationToken {
    pub fn with_host_stop(host_stop: Arc<AtomicBool>) -> Self {
        Self {
            host_stop: Some(host_stop),
            ..Self::default()
        }
    }
    pub fn cancel(&self) {
        self.requested.store(true, Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.requested.load(Ordering::Acquire)
            || self
                .host_stop
                .as_ref()
                .is_some_and(|stop| stop.load(Ordering::Acquire))
    }
    pub fn check(&self) -> Result<(), read_tools::ToolError> {
        if self.is_cancelled() {
            Err(read_tools::ToolError {
                error_code: "AGENT_RUN_CANCELLED".into(),
                category: "cancelled".into(),
                message: "Agent run was cancelled".into(),
            })
        } else {
            Ok(())
        }
    }
}

/// Every model purpose (including nested query, compaction and repair) uses this adapter.
pub struct CancellableAdapter<'a> {
    pub inner: &'a dyn crate::ModelAdapter,
    pub cancellation: CancellationToken,
}

impl CancellableAdapter<'_> {
    fn check(&self) -> Result<(), crate::AdapterError> {
        self.cancellation
            .check()
            .map_err(|error| crate::AdapterError {
                spend_stop: None,
                message: error.message,
            })
    }
}

impl crate::ModelAdapter for CancellableAdapter<'_> {
    fn tool_update_mode(&self) -> Option<crate::deepseek_messages::ToolUpdateMode> { self.inner.tool_update_mode() }
    fn context_request_projection(&self, request: &crate::AgentRequestPlan) -> Option<serde_json::Value> { self.inner.context_request_projection(request) }
    fn projection_identity(&self) -> Option<String> { self.inner.projection_identity() }

    fn model_provider(&self) -> Option<String> {
        self.inner.model_provider()
    }
    fn set_spend_context(&self, scope: crate::model_spend::ChargeScope, port: Option<std::sync::Arc<dyn crate::model_spend::ModelSpendPort>>) { self.inner.set_spend_context(scope,port); }
    fn set_model_purpose(&self, purpose: &str) { self.inner.set_model_purpose(purpose); }
    fn stream_text_is_structured(&self) -> bool {
        self.inner.stream_text_is_structured()
    }
    fn complete_observed(
        &self,
        request: crate::CompletionRequest,
        observer: &mut dyn crate::provider_stream::ModelObserver,
    ) -> Result<crate::ParsedResponse, crate::AdapterError> {
        self.check()?;
        let result = self.inner.complete_observed(request, observer);
        self.check()?;
        result
    }

    fn complete_structured_observed(
        &self,
        request: crate::CompletionRequest,
        observer: &mut dyn crate::provider_stream::ModelObserver,
    ) -> Result<serde_json::Value, crate::AdapterError> {
        self.check()?;
        let result = self.inner.complete_structured_observed(request, observer);
        self.check()?;
        result
    }

    fn chat_observed(
        &self,
        request: &crate::AgentRequestPlan,
        observer: &mut dyn crate::provider_stream::ModelObserver,
    ) -> Result<crate::AssistantTurn, crate::AdapterError> {
        self.check()?;
        let result = self.inner.chat_observed(request, observer);
        self.check()?;
        result
    }

    fn run_events(&self) -> Option<crate::run_events::RunEvents> {
        self.inner.run_events()
    }
    fn set_run_cancellation(&self, cancellation: CancellationToken) {
        self.inner.set_run_cancellation(cancellation);
    }
    fn model_runtime_profile(&self) -> ModelRuntimeProfile {
        self.inner.model_runtime_profile()
    }
    fn complete(
        &self,
        request: crate::CompletionRequest,
    ) -> Result<crate::ParsedResponse, crate::AdapterError> {
        self.check()?;
        let result = self.inner.complete(request);
        self.check()?;
        result
    }
    fn complete_structured(
        &self,
        request: crate::CompletionRequest,
    ) -> Result<serde_json::Value, crate::AdapterError> {
        self.check()?;
        let result = self.inner.complete_structured(request);
        self.check()?;
        result
    }
    fn chat(
        &self,
        request: &crate::AgentRequestPlan,
    ) -> Result<crate::AssistantTurn, crate::AdapterError> {
        self.check()?;
        let result = self.inner.chat(request);
        self.check()?;
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CompletionRequest, ProviderConfig, ProviderRegistry};
    use std::io::Read;

    #[test]
    fn cancelled_native_and_react_transport_do_not_retry_after_connection_loss() {
        for mode in ["native", "react"] {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let cancellation = CancellationToken::default();
            let stop = cancellation.clone();
            let provider = std::thread::spawn(move || {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                    .unwrap();
                let mut headers = Vec::new();
                let mut byte = [0];
                while !headers.ends_with(b"\r\n\r\n") {
                    socket.read_exact(&mut byte).unwrap();
                    headers.push(byte[0]);
                }
                let length: usize = String::from_utf8_lossy(&headers)
                    .lines()
                    .find_map(|line| {
                        line.to_lowercase()
                            .strip_prefix("content-length:")
                            .map(|value| value.trim().parse().unwrap())
                    })
                    .unwrap();
                socket.read_exact(&mut vec![0; length]).unwrap();
                stop.cancel();
                drop(socket); // normal supported transport retry trigger, after cancellation
                listener
            });
            let config = ProviderConfig::from_values(
                mode,
                "test-key",
                format!("http://{address}"),
                "test-model",
            )
            .unwrap();
            let adapter = ProviderRegistry::adapter_from_config_with_timeout(
                config,
                std::time::Duration::from_secs(2),
            );
            adapter.set_run_cancellation(cancellation);
            let error = adapter
                .complete(CompletionRequest {
                    followup_user: None,
                    images: Vec::new(),
                    output_token_limit: None,
                    reasoning_effort: None,
                    system: "test".into(),
                    user: "test".into(),
                })
                .unwrap_err();
            assert_eq!(error.message, "Agent run was cancelled");
            let listener = provider.join().unwrap();
            listener.set_nonblocking(true).unwrap();
            assert_eq!(
                listener.accept().unwrap_err().kind(),
                std::io::ErrorKind::WouldBlock,
                "{mode} retried after cancellation"
            );
        }
    }
}

/// A deterministic design or selection rejection may identify a host-validated source
/// locator to read next. The locator does not itself count as read evidence.
#[derive(Debug)]
pub struct TeachingSelectionPreflightError {
    pub error: read_tools::ToolError,
    pub missing_source_read: Option<read_tools::EvidenceRange>,
}

impl From<read_tools::ToolError> for TeachingSelectionPreflightError {
    fn from(error: read_tools::ToolError) -> Self {
        Self { error, missing_source_read: None }
    }
}

/// An operation must be deterministic: model calls run after this borrow ends.
pub trait ResidentStatePort {
    fn personal_note_context(&mut self)->Result<serde_json::Value,read_tools::ToolError>{Ok(serde_json::Value::Null)}
    fn personal_note_search(&mut self, request:serde_json::Value)->Result<serde_json::Value,read_tools::ToolError>{
        self.submit_private(|store|store.search_personal_notes(request["query"].as_str().unwrap_or_default(),request["book_id"].as_str(),request["limit"].as_u64().unwrap_or(8).clamp(1,20) as usize)
            .map(|hits|serde_json::json!({"hits":hits,"semantic":{"status":"unavailable","reason":"this host supplies lexical retrieval"}})))?
    }
    fn board_authoring_available(&self) -> bool { false }
    fn author_board(&mut self, _request: crate::board_author::AuthorRequest, _bindings: &[crate::orchestrator::SourceBinding], _messages: &[Message], _cancellation: &CancellationToken) -> Result<crate::board_author::AuthorResult, read_tools::ToolError> {
        Err(crate::board_author::unavailable())
    }
    fn music_source_context(&mut self) -> Result<Option<serde_json::Value>, read_tools::ToolError> { Ok(None) }
    fn read_music(&mut self, _reference: Option<&base_schema::PublishedBookRef>, _request: read_tools::music::MusicRead) -> Result<read_tools::music::MusicReadResult, read_tools::ToolError> { Err(crate::music::unavailable()) }
    fn interact_music(&mut self, _request: crate::music::MusicInteractRequest, _cancellation: &CancellationToken) -> Result<crate::music::MusicPlaybackReceipt, read_tools::ToolError> { Err(crate::music::unavailable()) }
    fn web_available(&self) -> bool { false }
    fn web_call(&mut self, _name: &str, _arguments: serde_json::Value, _cancellation: &CancellationToken) -> Result<serde_json::Value, read_tools::ToolError> {
        Err(crate::web::unavailable())
    }
    fn interact_presentation(&mut self, _request: crate::presentation_interact::InteractRequest, _cancellation: &CancellationToken) -> Result<serde_json::Value, read_tools::ToolError> {
        Err(crate::presentation_interact::unavailable())
    }
    fn workspace_records_available(&self) -> bool { false }
    fn workspace_records(&mut self, _request: serde_json::Value) -> Result<serde_json::Value, read_tools::ToolError> {
        Err(memory::teaching::invalid("Linked workspace records are unavailable"))
    }
    fn library_available(&self) -> bool { false }
    fn library_source(&mut self, _reference: &base_schema::PublishedBookRef) -> Result<crate::library::LibrarySource, read_tools::ToolError> { Err(crate::library::unavailable()) }
    fn library_read(&mut self, _name: &str, _arguments: serde_json::Value) -> Result<crate::library::LibraryReadResult, read_tools::ToolError> {
        Err(crate::library::unavailable())
    }
    fn library_learning(&mut self, _arguments: serde_json::Value, _bindings: &[crate::orchestrator::SourceBinding]) -> Result<serde_json::Value, read_tools::ToolError> {
        Err(crate::library::unavailable())
    }
    fn record_presentation_methods(&mut self, _observations: &crate::presentation::PresentationMethodObservations) -> Result<(), read_tools::ToolError> { Ok(()) }
    fn library_media_source_for_text(&mut self, _reference: &base_schema::PublishedBookRef, _range: &read_tools::EvidenceRange) -> Result<Option<crate::video_source::MediaSource>, read_tools::ToolError> { Ok(None) }
    fn media_source_for_text(&mut self, _range: &read_tools::EvidenceRange) -> Result<Option<crate::video_source::MediaSource>, read_tools::ToolError> { Ok(None) }
    fn media_source_snapshot(&mut self, _target: base_schema::EvidenceTarget) -> Result<crate::video_source::MediaSource, read_tools::ToolError> { Err(crate::video::unavailable()) }
    fn video_source_context(&mut self) -> Result<Option<serde_json::Value>, read_tools::ToolError> { Ok(None) }
    fn review_video_audio(&mut self, _reference: Option<&base_schema::PublishedBookRef>, _request: crate::video_audio::ReviewAudio, _cancellation: &CancellationToken) -> Result<crate::video_audio::AudioReview, read_tools::ToolError> { Err(crate::video::unavailable()) }
    fn search_video(&mut self, _reference: Option<&base_schema::PublishedBookRef>, _request: read_tools::video_search::VideoSearch) -> Result<serde_json::Value, read_tools::ToolError> { Err(crate::video::unavailable()) }
    fn read_video_frames(&mut self, _reference: Option<&base_schema::PublishedBookRef>, _request: crate::video::ReadFrames, _cancellation: &CancellationToken) -> Result<crate::video::FrameBatch, read_tools::ToolError> {
        Err(crate::video::unavailable())
    }
    fn presentation_unavailable_reason(&self) -> Option<&'static str> { None }
    fn tutor_assessment_input(&mut self, _action: &str) -> Result<serde_json::Value, read_tools::ToolError> {
        Err(memory::teaching::invalid("Assessment unavailable"))
    }
    fn tutor_assessment_accept(&mut self, _action: &str, _items: serde_json::Value) -> Result<serde_json::Value, read_tools::ToolError> {
        Err(memory::teaching::invalid("Assessment unavailable"))
    }
    fn tutor_active(&mut self) -> Result<bool, read_tools::ToolError> { Ok(true) }
    /// Host-frozen dependency result for this admitted turn, after any managed
    /// coach wait. This must not read an unrelated latest job or run a model.
    fn tutor_coach_context(&mut self) -> Result<Option<serde_json::Value>, read_tools::ToolError> { Ok(None) }
    fn tutor_working_state(&mut self) -> Result<serde_json::Value, read_tools::ToolError> {
        Ok(serde_json::Value::Null)
    }
    /// Retain actual agent work separately from learner evidence, bound by the host.
    fn tutor_record_review(&mut self, _record: serde_json::Value) -> Result<serde_json::Value, read_tools::ToolError> {
        Ok(serde_json::Value::Null)
    }
    fn tutor_step_preflight(&self, request: &serde_json::Value, _evidence: &[crate::orchestrator::SourceBinding], _ranges: &[read_tools::EvidenceRange]) -> Result<serde_json::Value, TeachingSelectionPreflightError> {
        Ok(request.clone())
    }
    fn tutor_step(&mut self, _request: serde_json::Value, _evidence: &[crate::orchestrator::SourceBinding], _ranges: &[read_tools::EvidenceRange]) -> Result<serde_json::Value, read_tools::ToolError> {
        Err(memory::teaching::invalid("Teaching context unavailable"))
    }
    fn persist_goal(&mut self, _goal: &crate::goal::ResidentGoal) -> Result<(), read_tools::ToolError> {
        Ok(())
    }
    fn update_workspace_goal(&mut self, _update: &crate::workspace_goal::WorkspaceGoalUpdate, _current_user: &str) -> Result<base_schema::WorkspaceGoal, read_tools::ToolError> {
        Err(read_tools::ToolError { error_code:"WORKSPACE_GOAL_UNAVAILABLE".into(), category:"validation".into(), message:"This run has no writable learning workspace".into() })
    }
    fn author_presentation(
        &mut self,
        _request: crate::presentation_author::AuthorRequest,
        _bindings: &[crate::orchestrator::SourceBinding],
        _messages: &[Message],
        _cancellation: &CancellationToken,
    ) -> Result<crate::presentation_author::AuthorResult, read_tools::ToolError> {
        Err(crate::presentation_author::unavailable())
    }

    fn submit_private<R>(&mut self, operation: impl FnOnce(&mut MemoryStore) -> R) -> Result<R, read_tools::ToolError>;
    fn read_live_reader<R>(&mut self, operation: impl FnOnce(&Reader) -> R) -> Result<R, read_tools::ToolError>;
    fn apply_reader<R>(&mut self, operation: impl FnOnce(&mut MemoryStore, &mut Reader) -> R) -> Result<R, read_tools::ToolError>;
    fn reader_input(&mut self, book: &read_tools::Book, question: &str) -> ReaderInputSnapshot;

}

pub struct BorrowedResidentState<'a> {
    pub store: &'a mut MemoryStore,
    pub reader: &'a mut Reader,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReaderInputSnapshot {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video_question: Option<base_schema::VideoQuestionContext>,
    pub state: reader::ReaderState,
    pub minimap_context: Option<crate::context_fragment::ContextFragment>,
}

impl ReaderInputSnapshot {
    pub fn capture(book: &read_tools::Book, reader: &Reader, question: &str) -> Self {
        Self { video_question: None, state: reader.state(), minimap_context: crate::orchestrator::paper_minimap_context_fragment(book, reader, question) }
    }
}

impl ResidentStatePort for BorrowedResidentState<'_> {
    fn submit_private<R>(&mut self, operation: impl FnOnce(&mut MemoryStore) -> R) -> Result<R, read_tools::ToolError> { Ok(operation(self.store)) }
    fn read_live_reader<R>(&mut self, operation: impl FnOnce(&Reader) -> R) -> Result<R, read_tools::ToolError> { Ok(operation(self.reader)) }
    fn apply_reader<R>(&mut self, operation: impl FnOnce(&mut MemoryStore, &mut Reader) -> R) -> Result<R, read_tools::ToolError> { Ok(operation(self.store, self.reader)) }
    fn reader_input(&mut self, book: &read_tools::Book, question: &str) -> ReaderInputSnapshot { ReaderInputSnapshot::capture(book, self.reader, question) }
}

pub struct RunContext {
    pub personal_note_reads: Vec<memory::NoteRef>,
    /// The previous accepted turn's recoverable input. It never grants current
    /// tool activation or evidence authority.
    pub active_model_projection: Option<crate::active_projection::ActiveModelProjection>,
    /// Last successful model sampling in this run; finalized with its accepted
    /// response before the business transcript is redacted.
    pub sampled_model_projection: Option<crate::active_projection::ActiveModelProjection>,
    pub(crate) tool_declaration_history: crate::tool_history::ToolDeclarationHistory,
    pub(crate) model_turns: usize,
    pub(crate) tokens_spent: u32,
    pub(crate) request_audit: crate::agent_request_audit::AgentRequestAudit,
    pub(crate) question_image: Option<base_schema::VideoQuestionImage>,
    pub audio_reviews: Vec<crate::video_audio::AudioReview>,
    pub media_observations: Vec<base_schema::MediaObservation>,
    /// Actual player requests and host receipts, kept apart from source evidence.
    pub(crate) music_interactions: Vec<serde_json::Value>,
    pub(crate) pending_video: Vec<crate::video::FrameBatch>,
    pub tutor: Option<serde_json::Value>,
    pub tutor_resolved: bool,
    /// A design has authority for generation only after it entered a sampling.
    pub(crate) sampled_teaching_design_ref: Option<String>,
    /// Authoritative selections that the successful final answer will deliver.
    pub teaching_delivery_move_refs: Vec<String>,
    /// Accepted teaching text and private allocation of reasoning for this turn.
    pub(crate) teaching_delivery_draft: Option<serde_json::Value>,
    pub(crate) board_delivery_draft: Option<String>,
    pub(crate) board_candidate_id: Option<String>,
    pub(crate) board_review_preview: Option<crate::board_author::ReviewPreview>,
    pub(crate) board_selected_candidate_id: Option<String>,
    /// Successful choices from this design in the current turn, including
    /// choices omitted from the latest draft but explicitly retained later.
    pub(crate) accepted_teaching_moves: std::collections::BTreeMap<String, serde_json::Value>,
    pub goal: Option<crate::goal::ResidentGoal>,
    /// Original direction accepted with the run; it remains the task's provenance.
    pub workspace_goal: Option<crate::workspace_goal::WorkspaceGoalContext>,
    /// A revision explicitly accepted by the host during this run.
    pub accepted_workspace_goal: Option<base_schema::WorkspaceGoal>,
    pub current_user_message: Option<String>,
    /// Frozen, host-accepted board selection. Kept distinct from author proposals.
    pub board_request_context: Option<serde_json::Value>,
    pub response_language: Option<crate::response_language::ResponseLanguage>,
    pub(crate) presentation_authoring: Option<crate::presentation_author::PresentationAuthoringContext>,
    /// Current-request coach handoff, including unavailable attempts. Changing
    /// layout/technical needs during this run must not buy another coach call.
    pub(crate) presentation_coach: Option<serde_json::Value>,
    /// Native-board planning has different expression capabilities from HTML.
    /// Mixed authoring requests reuse one handoff per surface, never across them.
    pub(crate) native_board_coach: Option<serde_json::Value>,
    pub(crate) presentation_authoring_finished: bool,
    pub(crate) presentation_method_reads: std::collections::HashSet<String>,
    pub(crate) presentation_method_observations: crate::presentation::PresentationMethodObservations,
    pub(crate) presentation_candidates: std::collections::BTreeSet<String>,
    pub(crate) presentation_teaching_designs: std::collections::BTreeMap<String, String>,
    pub(crate) presentation_images: Vec<crate::presentation_author::PreviewImage>,
    /// Actual author previews, retained separately from learner observations.
    pub(crate) presentation_preview_observations: std::collections::BTreeMap<String, Vec<serde_json::Value>>,
    pub(crate) presentation_delivery_observations: std::collections::BTreeMap<String, Vec<serde_json::Value>>,
    /// Retained after the author's image queue is consumed, for independent review.
    pub(crate) pending_previews: std::collections::BTreeSet<String>,
    pub(crate) unobserved_preview_environments: std::collections::HashSet<(String, String)>,
    pub(crate) pending_plot_refs: std::collections::HashSet<String>,
    pub(crate) inspected_presentations: std::collections::HashSet<String>,
    pub(crate) delivered_presentations: Vec<crate::presentation::PresentationRef>,
    pub messages: Vec<Message>,
    pub config: OuterConfig,
    pub runtime_profile: ModelRuntimeProfile,
    pub cancellation: CancellationToken,
    pub events: crate::run_events::RunEvents,
    pub effects: Vec<crate::orchestrator::AgentEffect>,
    pub trace: Vec<crate::orchestrator::TraceStep>,
    pub navigation: Option<(String, String)>,
    pub(crate) evidence_ledger: TurnEvidenceLedger,
    pub(crate) evidence_plan_ledger: TurnEvidencePlanLedger,
    pub(crate) locator_ledger: TurnLocatorLedger,
    pub(crate) progress_ledger: TurnProgressLedger,
    pub(crate) answer_provenance: AnswerProvenanceLedger,
    pub(crate) tool_exposure_state: ToolExposureState,
    pub(crate) active_tool_results: ActiveToolResultLedger,
}

impl RunContext {
    pub(crate) fn accepted_board_text(&self) -> Option<&str> {
        self.board_delivery_draft.as_deref().filter(|text|
            crate::board_author::validate_teaching_delivery(self.tutor.as_ref(), self.tutor_resolved,
                self.teaching_delivery_draft.as_ref(), text).is_ok())
    }
    pub fn restore_model_projection(
        &mut self,
        projection: Option<crate::active_projection::ActiveModelProjection>,
        provider_identity: Option<&str>,
    ) {
        self.active_model_projection = projection.filter(|projection|
            projection.history_message_count == self.messages.len()
                && projection.matches_provider(provider_identity, &self.runtime_profile));
        if let Some(projection) = &self.active_model_projection {
            self.tool_declaration_history = projection.tool_history.clone();
            self.tool_declaration_history.resume_input(&projection.input);
        }
    }

    pub fn finished_model_projection(&self) -> Option<crate::active_projection::ActiveModelProjection> {
        self.sampled_model_projection.as_ref()?.finish(&self.messages)
    }

    /// Close the provider protocol for the unexecuted suffix of a cancelled tool batch.
    /// These are cancellation receipts, not executed tools or activity records.
    pub fn close_cancelled_tool_calls(&mut self) {
        self.close_stopped_tool_calls("AGENT_RUN_CANCELLED", "cancelled", "Tool was not executed because the run was cancelled");
    }

    pub fn close_stopped_tool_calls(&mut self, code: &str, category: &str, message: &str) {
        self.presentation_authoring = None;
        self.presentation_authoring_finished = true;
        let start = self
            .messages
            .iter()
            .rposition(|message| message.role == crate::Role::User)
            .unwrap_or(0);
        let mut pending = Vec::new();
        for message in &self.messages[start..] {
            if message.role == crate::Role::Assistant {
                pending.extend(message.tool_calls.iter().map(|call| call.id.clone()));
            } else if message.role == crate::Role::Tool {
                pending.retain(|id| Some(id) != message.tool_call_id.as_ref());
            }
        }
        for id in pending {
            self.messages.push(Message {
                provider_continuation: None,
                role: crate::Role::Tool,
                tool_call_id: Some(id),
                tool_calls: Vec::new(),
                content: Some(
                    serde_json::json!({"error_code":code, "category":category, "message":message})
                    .to_string(),
                ),
            });
        }
    }

    pub fn new(
        messages: Vec<Message>,
        config: OuterConfig,
        runtime_profile: ModelRuntimeProfile,
    ) -> Self {
        Self {
            active_model_projection: None,
            personal_note_reads: Vec::new(),
            sampled_model_projection: None,
            tool_declaration_history: Default::default(),
            model_turns: 0,
            tokens_spent: 0,
            request_audit: Default::default(),
            question_image: None,
            audio_reviews: Vec::new(),
            media_observations: Vec::new(),
            music_interactions: Vec::new(),
            pending_video: Vec::new(),
            tutor: None,
            tutor_resolved: false,
            sampled_teaching_design_ref: None,
            teaching_delivery_move_refs: Vec::new(),
            teaching_delivery_draft: None,
            board_delivery_draft: None,
            board_candidate_id: None,
            board_review_preview: None,
            board_selected_candidate_id: None,
            accepted_teaching_moves: Default::default(),
            goal: None,
            workspace_goal: None,
            accepted_workspace_goal: None,
            current_user_message: None,
            board_request_context: None,
            response_language: None,
            presentation_authoring: None,
            presentation_coach: None,
            native_board_coach: None,
            presentation_authoring_finished: false,
            presentation_method_reads: Default::default(),
            presentation_method_observations: Default::default(),
            presentation_candidates: Default::default(),
            presentation_teaching_designs: Default::default(),
            presentation_images: Vec::new(),
            presentation_preview_observations: Default::default(),
            presentation_delivery_observations: Default::default(),
            pending_previews: Default::default(),
            unobserved_preview_environments: Default::default(),
            pending_plot_refs: Default::default(),
            inspected_presentations: Default::default(),
            delivered_presentations: Vec::new(),
            messages,
            config,
            runtime_profile,
            cancellation: Default::default(),
            events: Default::default(),
            effects: Vec::new(),
            trace: Vec::new(),
            navigation: None,
            evidence_ledger: Default::default(),
            evidence_plan_ledger: Default::default(),
            locator_ledger: Default::default(),
            progress_ledger: Default::default(),
            answer_provenance: Default::default(),
            tool_exposure_state: Default::default(),
            active_tool_results: Default::default(),
        }
    }
}
