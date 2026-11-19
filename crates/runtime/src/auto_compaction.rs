use crate::compaction::{CompactionCheckpoint, CompactionError};
use crate::{AgentRequestPlan, AssistantTurn, Message, ModelAdapter, Role};
use crate::provider_stream::{ModelDelta, ModelUsage};
use serde_json::Value;

pub const COMPACTION_FAILED: &str = "COMPACTION_FAILED";
pub const ACTIVE_CONTEXT_EXHAUSTED: &str = "ACTIVE_CONTEXT_EXHAUSTED";
pub const TURN_LIMIT_EXCEEDED: &str = "TURN_LIMIT_EXCEEDED";
pub const LEGACY_CONTEXT_BUDGET_EXCEEDED: &str = "CONTEXT_BUDGET_EXCEEDED";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActiveContextBudget {
    pub estimated_input_tokens: u32,
    pub reserved_tokens: u32,
    pub pressure_tokens: u32,
    pub high_watermark_tokens: u32,
    pub target_input_tokens: u32,
    pub over_high_watermark: bool,
    pub fits: bool,
}

impl ActiveContextBudget {
    pub fn from_plan(plan: &AgentRequestPlan) -> Self {
        let profile = &plan.runtime_profile;
        let reserved_tokens = plan.active_context.output_reserve_tokens
            .saturating_add(plan.active_context.safety_margin_tokens);
        let pressure_tokens = plan
            .active_context
            .estimated_input_tokens
            .saturating_add(reserved_tokens);
        let ratio = profile.compaction.high_watermark_ratio.clamp(0.0, 1.0);
        let high_watermark_tokens =
            ((profile.context_window_tokens as f64) * f64::from(ratio)).floor() as u32;
        let target_input_tokens = high_watermark_tokens.saturating_sub(reserved_tokens);
        Self {
            estimated_input_tokens: plan.active_context.estimated_input_tokens,
            reserved_tokens,
            pressure_tokens,
            high_watermark_tokens,
            target_input_tokens,
            over_high_watermark: pressure_tokens >= high_watermark_tokens,
            fits: plan.active_context.fits,
        }
    }
}

/// A successful request measures one active context, not the sum of the turn's
/// billed requests. Retain the actual header and replay so a rewrite cannot
/// carry the old measurement into a different input.
#[derive(Debug, Default)]
pub(crate) struct ContextTokenMeter {
    anchor: Option<ContextTokenAnchor>,
}

#[derive(Debug)]
struct ContextTokenAnchor {
    provider: Option<String>,
    header: Value,
    messages: Vec<Value>,
    estimated_tokens: u32,
    measured_tokens: u32,
}

/// Usage notifications describe logical-call spend. Output retries include
/// discarded attempts, so they cannot establish an active-window measurement.
#[derive(Debug, Default)]
pub(crate) struct ContextUsageCapture {
    usage: Option<ModelUsage>,
    retried_output: bool,
}

impl ContextUsageCapture {
    pub(crate) fn observe(&mut self, delta: &ModelDelta) {
        match delta {
            ModelDelta::Usage(usage) => self.usage = Some(usage.clone()),
            ModelDelta::RetryReset => self.retried_output = true,
            _ => {}
        }
    }

    fn context_tokens(&self) -> Option<(u32, u32)> {
        if self.retried_output { return None; }
        self.usage.as_ref().and_then(|usage| usage.input_tokens.zip(usage.output_tokens))
    }
}

impl ContextTokenMeter {
    pub(crate) fn calibrate_plan(&self, adapter: &dyn ModelAdapter, plan: &mut AgentRequestPlan) {
        let Some(projection) = adapter.context_request_projection(plan) else { return; };
        let estimated = self.measure(adapter.projection_identity(), &projection);
        plan.update_input_estimate(estimated);
    }

    fn measure(&self, provider: Option<String>, projection: &Value) -> u32 {
        let estimated = crate::model_runtime::estimate_provider_request_input_tokens(projection);
        let Some((header, messages)) = split_projection(projection) else { return estimated; };
        let Some(anchor) = self.anchor.as_ref().filter(|anchor| {
            anchor.provider == provider && anchor.header == header
                && messages.starts_with(&anchor.messages)
        }) else { return estimated; };
        // Use the same estimate at both ends. The signed delta avoids adding
        // already-measured assistant output a second time. Cache-read tokens
        // are already included in ModelUsage.input_tokens.
        let delta = i64::from(estimated) - i64::from(anchor.estimated_tokens);
        (i64::from(anchor.measured_tokens.max(anchor.estimated_tokens)) + delta)
            .clamp(0, i64::from(u32::MAX)) as u32
    }

    pub(crate) fn record_response(
        &mut self,
        adapter: &dyn ModelAdapter,
        request: &AgentRequestPlan,
        turn: &AssistantTurn,
        captured: &ContextUsageCapture,
    ) {
        self.anchor = None;
        let Some((input, output)) = captured.context_tokens() else { return; };
        // Hidden reasoning that is not replayed does not occupy the next input.
        // Without a comparable output surface, keep the full current estimate.
        if captured.usage.as_ref().and_then(|usage| usage.reasoning_output_tokens)
            .is_some_and(|tokens| tokens > 0)
            && turn.provider_continuation.as_ref()
                .is_none_or(|state| state.reasoning_content.is_empty())
        { return; }
        let Some(original) = adapter.context_request_projection(request) else { return; };
        let Some((original_header, original_messages)) = split_projection(&original) else { return; };
        let mut completed = request.clone();
        completed.input.push(Message {
            provider_continuation: turn.provider_continuation.clone(),
            role: Role::Assistant,
            content: turn.text.clone(),
            tool_calls: turn.tool_calls.clone(),
            tool_call_id: None,
        });
        let Some(projection) = adapter.context_request_projection(&completed) else { return; };
        let Some((completed_header, completed_messages)) = split_projection(&projection) else { return; };
        if completed_header != original_header || !completed_messages.starts_with(original_messages) {
            return;
        }
        self.record_projection(adapter.projection_identity(), &projection, input, output);
    }

    fn record_projection(&mut self, provider: Option<String>, projection: &Value, input: u32, output: u32) {
        let Some((header, messages)) = split_projection(projection) else { return; };
        self.anchor = Some(ContextTokenAnchor {
            provider,
            header,
            messages: messages.to_vec(),
            estimated_tokens: crate::model_runtime::estimate_provider_request_input_tokens(projection),
            measured_tokens: input.saturating_add(output),
        });
    }
}

fn split_projection(projection: &Value) -> Option<(Value, &[Value])> {
    let messages = projection.get("messages")?.as_array()?;
    let mut header = projection.as_object()?.clone();
    header.remove("messages");
    // These transport switches do not describe model context.
    header.remove("stream");
    header.remove("stream_options");
    Some((Value::Object(header), messages.as_slice()))
}

pub trait CompactionCheckpointSink {
    fn persist_effects(&mut self, _effects: &[crate::orchestrator::AgentEffect]) -> Result<(), read_tools::ToolError> { Ok(()) }
    /// Complete messages and phase facts, never token patches. Failure stops
    /// execution before the next model/tool side effect.
    fn persist_progress(&mut self, _messages: &[Message], _activities: &[crate::run_events::RunActivity]) -> Result<(), read_tools::ToolError> { Ok(()) }
    fn prepare_persisted_messages(&mut self, _messages: &mut [Message]) {}
    fn install(
        &mut self,
        checkpoint: &CompactionCheckpoint,
        raw_messages: &[Message],
    ) -> Result<(), CompactionError>;
}

#[derive(Debug, Default)]
pub struct EphemeralCompactionCheckpointSink {
    pub installed: Option<CompactionCheckpoint>,
}

impl CompactionCheckpointSink for EphemeralCompactionCheckpointSink {
    fn install(
        &mut self,
        checkpoint: &CompactionCheckpoint,
        _raw_messages: &[Message],
    ) -> Result<(), CompactionError> {
        self.installed = Some(checkpoint.clone());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_runtime::{ModelRuntimeProfile, ProviderToolProtocol};

    #[test]
    fn active_context_budget_uses_current_request_and_reserves_not_cumulative_usage() {
        let mut profile =
            ModelRuntimeProfile::fallback("budget-fixture", ProviderToolProtocol::Native);
        profile.context_window_tokens = 1_000;
        profile.output_reserve_tokens = 100;
        profile.safety_margin_tokens = 50;
        profile.compaction.high_watermark_ratio = 0.75;
        let messages = vec![Message::user("x".repeat(2_500))];
        let plan = AgentRequestPlan::for_agent_turn(profile, &messages, &[]);
        let budget = ActiveContextBudget::from_plan(&plan);
        assert_eq!(budget.reserved_tokens, 150);
        assert_eq!(budget.high_watermark_tokens, 750);
        assert_eq!(budget.target_input_tokens, 600);
        assert!(budget.over_high_watermark);
        assert!(budget.fits);
    }

    fn completed_projection() -> Value {
        serde_json::json!({
            "model":"meter-model", "max_tokens":100, "tool_choice":"auto",
            "tools":[{"type":"function","function":{"name":"read","parameters":{"type":"object"}}}],
            "messages":[{"role":"user","content":"read the source"},
                {"role":"assistant","content":"observed", "reasoning_content":"reasoning"}]
        })
    }

    #[test]
    fn measured_context_adds_only_the_new_surface_not_cached_or_output_twice() {
        let previous = completed_projection();
        let mut meter = ContextTokenMeter::default();
        let mut capture = ContextUsageCapture::default();
        capture.observe(&ModelDelta::Usage(ModelUsage {
            billed_input_tokens: None,
            billed_output_tokens: None,
            input_tokens:Some(8_000), output_tokens:Some(200),
            cached_input_tokens:Some(7_900), cache_creation_input_tokens:Some(50),
            reasoning_output_tokens:Some(100), total_tokens:Some(8_200),
        }));
        let (input, output) = capture.context_tokens().unwrap();
        meter.record_projection(Some("provider".into()), &previous, input, output);
        let mut current = previous.clone();
        current["messages"].as_array_mut().unwrap().push(serde_json::json!({"role":"tool","content":"x".repeat(400)}));
        let estimate = crate::model_runtime::estimate_provider_request_input_tokens;
        assert_eq!(meter.measure(Some("provider".into()), &current),
            8_200 + estimate(&current) - estimate(&previous));
    }

    #[test]
    fn smaller_reported_usage_cannot_lower_the_current_full_estimate() {
        let previous = completed_projection();
        let mut meter = ContextTokenMeter::default();
        meter.record_projection(None, &previous, 1, 1);
        assert_eq!(meter.measure(None, &previous),
            crate::model_runtime::estimate_provider_request_input_tokens(&previous));
    }

    #[test]
    fn changed_route_header_or_old_prefix_discards_the_usage_anchor() {
        let previous = completed_projection();
        let mut meter = ContextTokenMeter::default();
        meter.record_projection(Some("provider".into()), &previous, 8_000, 200);
        let mut changed = Vec::new();
        let mut model = previous.clone(); model["model"] = "another-model".into(); changed.push(model);
        let mut tools = previous.clone(); tools["tools"][0]["function"]["name"] = "edit".into(); changed.push(tools);
        let mut settings = previous.clone(); settings["max_tokens"] = 200.into(); changed.push(settings);
        let mut replacement = previous.clone(); replacement["messages"][0]["content"] = "summary".into(); changed.push(replacement);
        let mut pruned = previous.clone(); pruned["messages"].as_array_mut().unwrap().remove(0); changed.push(pruned);
        for current in changed {
            assert_eq!(meter.measure(Some("provider".into()), &current),
                crate::model_runtime::estimate_provider_request_input_tokens(&current));
        }
        assert_eq!(meter.measure(Some("different-provider".into()), &previous),
            crate::model_runtime::estimate_provider_request_input_tokens(&previous));
    }

    #[test]
    fn incomplete_or_cumulative_retry_spend_does_not_measure_context() {
        let mut capture = ContextUsageCapture::default();
        capture.observe(&ModelDelta::Usage(ModelUsage { total_tokens:Some(42), ..Default::default() }));
        assert!(capture.context_tokens().is_none());
        let usage = ModelUsage { input_tokens:Some(1_000), output_tokens:Some(100), ..Default::default() };
        capture.observe(&ModelDelta::Usage(usage.clone()));
        capture.observe(&ModelDelta::Usage(usage));
        assert_eq!(capture.context_tokens(), Some((1_000, 100)), "frames replace rather than add");
        capture.observe(&ModelDelta::RetryReset);
        capture.observe(&ModelDelta::Usage(ModelUsage { input_tokens:Some(2_000), output_tokens:Some(300), ..Default::default() }));
        assert!(capture.context_tokens().is_none(), "discarded output attempts are spend, not retained context");
    }

    #[test]
    fn calibrated_pressure_keeps_the_hard_window_and_explicit_output_reserve() {
        let mut profile = ModelRuntimeProfile::fallback("fixture", ProviderToolProtocol::Native);
        profile.context_window_tokens = 1_000;
        profile.output_reserve_tokens = 300;
        profile.safety_margin_tokens = 50;
        let mut plan = AgentRequestPlan::for_agent_turn(profile, &[Message::user("hello")], &[]);
        plan.output_token_limit = Some(100);
        plan.update_input_estimate(800);
        let budget = ActiveContextBudget::from_plan(&plan);
        assert_eq!(budget.reserved_tokens, 150);
        assert_eq!(plan.active_context.remaining_tokens, 50);
        assert!(budget.fits);
        assert!(budget.over_high_watermark);
        plan.update_input_estimate(851);
        assert!(!ActiveContextBudget::from_plan(&plan).fits);
    }
}
