//! Host fixture access; these helpers are compiled only with test support.
use super::*;
use crate::user_runtime::UserRuntime;
use runtime::presentation::{
    PresentationFollowUp, PresentationRef, PresentationState, SavedPresentationState,
};

pub trait UserRuntimeTestExt {
    fn test_presentation_root(&self) -> Result<PathBuf, ToolError>;
    fn test_intent_store(
        &self,
    ) -> Result<crate::intent_build_store::IntentArtifactStore, ToolError>;
}
impl UserRuntimeTestExt for UserRuntime {
    fn test_presentation_root(&self) -> Result<PathBuf, ToolError> {
        self.presentation_root()
    }
    fn test_intent_store(
        &self,
    ) -> Result<crate::intent_build_store::IntentArtifactStore, ToolError> {
        self.intent_store()
    }
}

pub trait PrivateBookTestExt {
    fn test_save_presentation_state(
        &self,
        session: &str,
        turn: &str,
        reference: &PresentationRef,
        state: PresentationState,
    ) -> Result<PresentationFollowUp, ToolError>;
    fn test_read_presentation_state(
        &self,
        receipt: &PresentationFollowUp,
    ) -> Result<SavedPresentationState, ToolError>;
}
impl PrivateBookTestExt for PrivateBookContext<'_> {
    fn test_save_presentation_state(
        &self,
        session: &str,
        turn: &str,
        reference: &PresentationRef,
        state: PresentationState,
    ) -> Result<PresentationFollowUp, ToolError> {
        self.save_presentation_state(session, turn, reference, state)
    }
    fn test_read_presentation_state(
        &self,
        receipt: &PresentationFollowUp,
    ) -> Result<SavedPresentationState, ToolError> {
        self.read_presentation_state(receipt)
    }
}

pub trait AgentHistoryTestExt {
    fn test_arm_confirmation(&mut self, session: &str);
}
impl AgentHistoryTestExt for AgentHistory {
    fn test_arm_confirmation(&mut self, session: &str) {
        self.arm_confirmation(session);
    }
}

pub trait RunStreamTestExt {
    fn test_event_frame(&self, event: &crate::agent_stream::RunEvent) -> String;
}
impl RunStreamTestExt for crate::agent_stream::RunStream {
    fn test_event_frame(&self, event: &crate::agent_stream::RunEvent) -> String {
        self.event_frame(event)
    }
}

pub fn book_entry_cover(entry: &Value) -> Value {
    entry["cover"].clone()
}

pub fn observability_config(
    get: impl FnMut(&str) -> Option<String>,
    allow_http: bool,
) -> Result<
    Option<crate::observability::config::ObservabilityConfig>,
    crate::observability::config::ConfigError,
> {
    crate::observability::config::ObservabilityConfig::from_getter(get, allow_http)
}

pub fn observability_with_transport(
    config: crate::observability::config::ObservabilityConfig,
    transport: Box<dyn crate::observability::tests::LangSmithTransport>,
) -> Arc<crate::observability::ObservabilityRuntime> {
    crate::observability::ObservabilityRuntime::with_transport(config, transport)
}
