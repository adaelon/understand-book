//! Recoverable model input between pressure boundaries. This is private execution
//! state, separate from the compact business transcript and evidence authority.
use crate::{compaction::CompactionCheckpoint, AgentRequestPlan, Message, ModelRuntimeProfile};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActiveModelProjection {
    pub provider_identity: String,
    pub runtime_profile: ModelRuntimeProfile,
    pub instructions: String,
    pub input: Vec<Message>,
    /// Number of business messages represented by this input. The input also
    /// contains frozen context and sampling decisions absent from that history.
    pub history_message_count: usize,
    pub checkpoint_window_id: Option<String>,
    #[serde(default)]
    pub tool_history: crate::tool_history::ToolDeclarationHistory,
}

impl ActiveModelProjection {
    /// Preserve conversation/tool input while replacing the previous turn's
    /// host working state with this turn's freshly projected state. The caller supplies the ledger's message count,
    /// so repeated question/answer text cannot select the wrong boundary.
    pub fn apply(
        &self,
        plan: &mut AgentRequestPlan,
        raw_messages: &[Message],
        added_context_message_count: usize,
        checkpoint: Option<&CompactionCheckpoint>,
    ) -> bool {
        if self.runtime_profile != plan.runtime_profile
            || self.instructions != plan.instructions
            || self.checkpoint_window_id.as_deref() != checkpoint.map(|value| value.window_id.as_str())
        {
            return false;
        }
        let Some(raw_suffix) = raw_messages.len().checked_sub(self.history_message_count) else {
            return false;
        };
        let Some(suffix_len) = raw_suffix.checked_add(added_context_message_count) else {
            return false;
        };
        let Some(boundary) = plan.input.len().checked_sub(suffix_len) else {
            return false;
        };
        let mut input = Vec::with_capacity(self.input.len() + suffix_len);
        input.extend(self.input.iter().filter(|message| !is_turn_context(message)).cloned());
        input.extend_from_slice(&plan.input[boundary..]);
        plan.input = input;
        true
    }

    /// Capture only a request actually accepted by the model. Image bytes are
    /// explicitly ephemeral in AgentRequestPlan; a text-only snapshot must not
    /// pretend it can reproduce a multimodal request after restart.
    pub fn capture_request(
        plan: &AgentRequestPlan,
        history_message_count: usize,
        provider_identity: Option<&str>,
        checkpoint: Option<&CompactionCheckpoint>,
        tool_history: &crate::tool_history::ToolDeclarationHistory,
    ) -> Option<Self> {
        let provider_identity = provider_identity?;
        if !plan.preview_images.is_empty()
            || !plan.video_images.is_empty()
            || plan.question_image.is_some()
        {
            return None;
        }
        let mut tool_history = tool_history.clone();
        tool_history.resume_input(&[]);
        Some(Self {
            provider_identity: provider_identity.to_owned(),
            runtime_profile: plan.runtime_profile.clone(),
            instructions: plan.instructions.clone(),
            input: plan.input.clone(),
            history_message_count,
            checkpoint_window_id: checkpoint.map(|value| value.window_id.clone()),
            tool_history,
        })
    }

    /// Finish the sampled request with the accepted assistant response and any
    /// committed tool/cancellation receipts that followed it. No evidence,
    /// activation, author handle or run-local permission is restored here.
    pub fn finish(&self, raw_messages: &[Message]) -> Option<Self> {
        let suffix = raw_messages.get(self.history_message_count..)?;
        let mut projection = self.clone();
        projection.input.extend_from_slice(suffix);
        projection.history_message_count = raw_messages.len();
        Some(projection)
    }

    pub fn matches_provider(&self, identity: Option<&str>, profile: &ModelRuntimeProfile) -> bool {
        identity == Some(self.provider_identity.as_str()) && profile == &self.runtime_profile
    }
}

fn is_turn_context(message: &Message) -> bool {
    let Some(text) = message.content.as_deref() else { return false; };
    // These are private projections, absent from the business transcript. All
    // current manifests and frozen state are republished at the next turn.
    (text.starts_with("context_fragment.v1\n") && !text.contains("\nscope=session_stable\n"))
        || ["runtime_state_snapshot.v1\n", "sampling_guidance.v1\n",
            "runtime_decision_messages.v1\n", "presentation_guidance_state.v1\n"]
            .iter().any(|prefix| text.starts_with(prefix))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{model_runtime::ProviderToolProtocol, Role, ToolCall};

    fn profile() -> ModelRuntimeProfile {
        ModelRuntimeProfile::fallback("projection-test", ProviderToolProtocol::Native)
    }

    #[test]
    fn durable_projection_preserves_prefix_when_business_history_is_redacted() {
        let raw = vec![Message::system("system"), Message::user("same question"), Message {
            role: Role::Assistant, content: None, tool_call_id: None,
            provider_continuation: None,
            tool_calls: vec![ToolCall { id: "call-1".into(), name: "presentation.author".into(),
                arguments: "{\"html\":\"original body\"}".into() }],
        }, Message { role: Role::Tool, content: Some("original result".into()),
            tool_call_id: Some("call-1".into()), tool_calls: vec![], provider_continuation: None }];
        let mut request = AgentRequestPlan::for_agent_turn(profile(), &raw, &[]);
        request.input.insert(0, Message::system("first frozen state"));
        let saved = ActiveModelProjection::capture_request(&request, raw.len(), Some("route-a"), None,
            &Default::default()).unwrap();
        let mut finished_raw = raw.clone();
        finished_raw.push(Message { role: Role::Assistant, content: Some("accepted answer".into()),
            tool_calls: vec![], tool_call_id: None, provider_continuation: None });
        let saved = saved.finish(&finished_raw).unwrap();
        let saved: ActiveModelProjection = serde_json::from_slice(&serde_json::to_vec(&saved).unwrap()).unwrap();
        let mut next_raw = finished_raw;
        next_raw[2].tool_calls[0].arguments = "{\"candidate_id\":\"saved\"}".into();
        next_raw[3].content = Some("historical receipt".into());
        next_raw.push(Message::user("same question"));
        let mut next = AgentRequestPlan::for_agent_turn(profile(), &next_raw, &[]);
        next.input.insert(next.input.len() - 1, Message::system("second frozen state"));
        next.input.push(Message::system("current runtime decisions"));
        assert!(saved.apply(&mut next, &next_raw, 2, None));
        assert_eq!(&next.input[..saved.input.len()], saved.input.as_slice());
        assert_eq!(&next.input[saved.input.len()..], &[
            Message::system("second frozen state"), Message::user("same question"),
            Message::system("current runtime decisions"),
        ]);
        assert!(serde_json::to_string(&next.input).unwrap().contains("original body"));
    }

    #[test]
    fn next_turn_replaces_host_state_without_rewriting_conversation_or_sources() {
        let raw = vec![Message::system("fixed"),Message::user("first")];
        let mut first = AgentRequestPlan::for_agent_turn(profile(), &raw, &[]);
        first.input.push(Message::system("runtime_state_snapshot.v1\nold-state"));
        first.input.push(Message::system("sampling_guidance.v1\nold-workflows"));
        first.input.push(Message::user("context_fragment.v1\nkey=reader.source_cache\nscope=turn_frozen\nold-source"));
        let saved = ActiveModelProjection::capture_request(&first, raw.len(), Some("route"), None, &Default::default()).unwrap();
        let mut next_raw = raw.clone(); next_raw.push(Message::user("follow up"));
        let mut next = AgentRequestPlan::for_agent_turn(profile(), &next_raw, &[]);
        next.input.push(Message::system("runtime_state_snapshot.v1\nnew-state"));
        assert!(saved.apply(&mut next,&next_raw,1,None));
        assert_eq!(next.input,vec![Message::user("first"),Message::user("follow up"),Message::system("runtime_state_snapshot.v1\nnew-state")]);
        assert_eq!(saved.input, first.input, "saved transcript/projection is not mutated");
    }

    #[test]
    fn changed_model_or_shortened_history_cannot_restore_an_old_window() {
        let raw = vec![Message::system("system"), Message::user("question")];
        let plan = AgentRequestPlan::for_agent_turn(profile(), &raw, &[]);
        let saved = ActiveModelProjection::capture_request(&plan, raw.len(), Some("route-a"), None,
            &Default::default()).unwrap();
        assert!(!saved.matches_provider(Some("route-b"), &profile()));
        let mut changed = plan.clone();
        changed.runtime_profile.matched_model = "another-model".into();
        assert!(!saved.apply(&mut changed, &raw, 0, None));
        let mut shortened = plan.clone();
        assert!(!saved.apply(&mut shortened, &raw[..1], 0, None));
        let mut changed_instructions = plan;
        changed_instructions.instructions.push_str(" changed policy");
        assert!(!saved.apply(&mut changed_instructions, &raw, 0, None));
    }

    #[test]
    fn pressure_checkpoint_begins_a_new_model_projection() {
        let raw = vec![Message::system("system"), Message::user("question")];
        let plan = AgentRequestPlan::for_agent_turn(profile(), &raw, &[]);
        let saved = ActiveModelProjection::capture_request(&plan, raw.len(), Some("route-a"), None,
            &Default::default()).unwrap();
        let checkpoint = CompactionCheckpoint {
            schema_version: "test".into(), prompt_version: "test".into(), window_id: "next-window".into(),
            source_history_revision: "source".into(), raw_retained_item_ids: vec![],
            semantic: Default::default(), tool_receipts: vec![], pending_effects: vec![],
            context_revisions: Default::default(),
        };
        let mut compacted = AgentRequestPlan::for_agent_turn(profile(), &[
            Message::system("system"), Message::system("compacted checkpoint"), Message::user("question")], &[]);
        let expected = compacted.input.clone();
        assert!(!saved.apply(&mut compacted, &raw, 0, Some(&checkpoint)));
        assert_eq!(compacted.input, expected);
        let next = ActiveModelProjection::capture_request(&compacted, raw.len(), Some("route-a"),
            Some(&checkpoint), &Default::default()).unwrap();
        assert!(next.apply(&mut compacted, &raw, 0, Some(&checkpoint)));
        assert_eq!(compacted.input, expected);
    }
}
