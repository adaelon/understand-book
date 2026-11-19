//! Cloud model dispatch retains authorization immediately before each provider call.
use runtime::provider_stream::{ModelDelta, ModelObserver, ModelUsage};
use runtime::run_context::CancellationToken;
use runtime::{AdapterError, AgentRequestPlan, AssistantTurn, CompletionRequest, ModelAdapter, ParsedResponse};
use std::sync::{Arc, Mutex};
pub use reader_service::service_limits::*;

/// Foreground and background have separate bounded capacity and identical usage
/// tracking; permits end before tool dispatch.
pub(crate) struct LimitedAdapter<'a> {
    pub inner: &'a dyn ModelAdapter,
    pub resources: Arc<Resources>,
    pub owner: String,
    pub background: bool,
    pub cancellation: CancellationToken,
    pub usage: Mutex<RunUsage>,
    pub stream: Option<Arc<crate::agent_stream::RunStream>>,
    pub authorization: Option<(
        &'a crate::authorization::Authorization,
        &'a crate::published_library::PublishedBookRef,
    )>,
}
impl LimitedAdapter<'_> {
    fn output_limit(&self, requested: Option<u32>) -> Result<Option<u32>, AdapterError> {
        // An agent plan can carry a more specific profile than this wrapper's
        // adapter. Preserve its limit; the provider finalizes it against the
        // complete request and the actual model window before reserving spend.
        match requested {
            Some(limit) => Ok(Some(limit)),
            None => self.inner.model_runtime_profile().output_token_limit(None, 0).map(Some),
        }
    }
    fn call<T>(
        &self,
        observer: &mut dyn ModelObserver,
        invoke: impl FnOnce(&mut dyn ModelObserver) -> Result<T, AdapterError>,
        fallback: impl FnOnce(&T) -> Option<u32>,
    ) -> Result<T, AdapterError> {
        if let Some(stream) = &self.stream {
            stream.resource_wait(true);
        }
        let permit = self
            .resources
            .acquire_model(&self.owner, &self.cancellation, self.background);
        if let Some(stream) = &self.stream {
            stream.resource_wait(false);
        }
        let _permit = permit?;
        self.cancellation.check().map_err(|e| AdapterError {
            spend_stop: None,
            message: e.error_code,
        })?;
        if let Some((access, publication)) = self.authorization {
            access
                .library
                .lock()
                .unwrap()
                .authorize(&self.owner, publication)
                .map_err(|_| AdapterError::from(runtime::model_spend::SpendStop::PermissionRevoked))?;
        }
        let mut reported = None;
        let result = invoke(&mut |delta: ModelDelta| {
            if let ModelDelta::Usage(ref usage) = delta {
                reported = Some(usage.clone());
            }
            observer.observe(delta);
        });
        if reported.is_none() {
            reported = result
                .as_ref()
                .ok()
                .and_then(fallback)
                .map(|total_tokens| ModelUsage {
                    total_tokens: Some(total_tokens),
                    ..Default::default()
                });
        }
        let mut usage = self.usage.lock().unwrap();
        usage.known_total_tokens +=
            reported.as_ref().and_then(|u| u.total_tokens).unwrap_or(0) as u64;
        usage.calls.push(reported);
        result
    }
}
impl ModelAdapter for LimitedAdapter<'_> {
    fn tool_update_mode(&self) -> Option<runtime::deepseek_messages::ToolUpdateMode> { self.inner.tool_update_mode() }
    fn context_request_projection(&self, request: &runtime::AgentRequestPlan) -> Option<serde_json::Value> { self.inner.context_request_projection(request) }
    fn projection_identity(&self) -> Option<String> { self.inner.projection_identity() }

    fn model_provider(&self) -> Option<String> {
        self.inner.model_provider()
    }
    fn set_spend_context(&self, scope: runtime::model_spend::ChargeScope, port: Option<Arc<dyn runtime::model_spend::ModelSpendPort>>) { self.inner.set_spend_context(scope,port); }
    fn set_model_purpose(&self, purpose: &str) { self.inner.set_model_purpose(purpose); }
    fn stream_text_is_structured(&self) -> bool {
        self.inner.stream_text_is_structured()
    }
    fn model_runtime_profile(&self) -> runtime::ModelRuntimeProfile {
        self.inner.model_runtime_profile()
    }
    fn set_run_cancellation(&self, c: CancellationToken) {
        self.inner.set_run_cancellation(c);
    }
    fn complete(&self, r: CompletionRequest) -> Result<ParsedResponse, AdapterError> {
        self.complete_observed(r, &mut runtime::provider_stream::ignore)
    }
    fn complete_structured(&self, r: CompletionRequest) -> Result<serde_json::Value, AdapterError> {
        self.complete_structured_observed(r, &mut runtime::provider_stream::ignore)
    }
    fn chat(&self, r: &AgentRequestPlan) -> Result<AssistantTurn, AdapterError> {
        self.chat_observed(r, &mut runtime::provider_stream::ignore)
    }
    fn complete_observed(
        &self,
        mut r: CompletionRequest,
        o: &mut dyn ModelObserver,
    ) -> Result<ParsedResponse, AdapterError> {
        r.output_token_limit = self.output_limit(r.output_token_limit)?;
        self.call(o, |o| self.inner.complete_observed(r, o), |_| None)
    }
    fn complete_structured_observed(
        &self,
        mut r: CompletionRequest,
        o: &mut dyn ModelObserver,
    ) -> Result<serde_json::Value, AdapterError> {
        r.output_token_limit = self.output_limit(r.output_token_limit)?;
        self.call(
            o,
            |o| self.inner.complete_structured_observed(r, o),
            |_| None,
        )
    }
    fn chat_observed(
        &self,
        r: &AgentRequestPlan,
        o: &mut dyn ModelObserver,
    ) -> Result<AssistantTurn, AdapterError> {
        let mut r = r.clone();
        r.output_token_limit = self.output_limit(r.output_token_limit)?;
        self.call(
            o,
            |o| self.inner.chat_observed(&r, o),
            |r| r.usage_total_tokens,
        )
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    struct RecordingProvider {
        total_tokens: Option<u32>,
        output_limits: Mutex<Vec<Option<u32>>>,
    }
    impl RecordingProvider {
        fn reply<T>(
            &self,
            limit: Option<u32>,
            observer: &mut dyn ModelObserver,
        ) -> Result<T, AdapterError> {
            self.output_limits.lock().unwrap().push(limit);
            if let Some(total_tokens) = self.total_tokens {
                for total in [3, total_tokens] {
                    observer.observe(ModelDelta::Usage(ModelUsage {
                        total_tokens: Some(total),
                        ..Default::default()
                    }));
                }
            }
            Err(AdapterError {
                spend_stop: None,
                message: "timeout".into(),
            })
        }
    }
    impl ModelAdapter for RecordingProvider {
        fn complete(&self, r: CompletionRequest) -> Result<ParsedResponse, AdapterError> {
            self.complete_observed(r, &mut runtime::provider_stream::ignore)
        }
        fn complete_observed(
            &self,
            r: CompletionRequest,
            o: &mut dyn ModelObserver,
        ) -> Result<ParsedResponse, AdapterError> {
            self.reply(r.output_token_limit, o)
        }
        fn complete_structured_observed(
            &self,
            r: CompletionRequest,
            o: &mut dyn ModelObserver,
        ) -> Result<serde_json::Value, AdapterError> {
            self.reply(r.output_token_limit, o)
        }
        fn chat(&self, r: &AgentRequestPlan) -> Result<AssistantTurn, AdapterError> {
            self.chat_observed(r, &mut runtime::provider_stream::ignore)
        }
        fn chat_observed(
            &self,
            r: &AgentRequestPlan,
            o: &mut dyn ModelObserver,
        ) -> Result<AssistantTurn, AdapterError> {
            self.reply(r.output_token_limit, o)
        }
    }
    fn recording_adapter(inner: &RecordingProvider) -> LimitedAdapter<'_> {
        LimitedAdapter {
            inner,
            resources: Resources::new(Arc::new(ServiceLimits::default())),
            owner: "A".into(),
            background: false,
            cancellation: CancellationToken::default(),
            usage: Mutex::new(Default::default()),
            authorization: None,
            stream: None,
        }
    }
    fn sample(adapter: &LimitedAdapter<'_>, index: usize, limit: Option<u32>) -> AdapterError {
        let mut r = request();
        r.output_token_limit = limit;
        match index % 3 {
            0 => adapter.complete(r).unwrap_err(),
            1 => adapter.complete_structured(r).unwrap_err(),
            _ => {
                let mut plan =
                    AgentRequestPlan::for_agent_turn(adapter.model_runtime_profile(), &[], &[]);
                plan.output_token_limit = limit;
                adapter.chat(&plan).unwrap_err()
            }
        }
    }
    #[test]
    fn mu6c_usage_does_not_stop_after_64_calls() {
        let inner = RecordingProvider {
            total_tokens: None,
            output_limits: Mutex::new(Vec::new()),
        };
        let adapter = recording_adapter(&inner);
        for i in 0..65 {
            assert_eq!(
                sample(&adapter, i, None).message,
                "timeout",
                "call {} must reach the provider",
                i + 1
            );
        }
        let usage = adapter.usage.lock().unwrap();
        assert_eq!(usage.calls.len(), 65);
        assert!(usage.calls.iter().all(Option::is_none));
        assert_eq!(usage.known_total_tokens, 0);
        drop(usage);
        assert!(adapter
            .resources
            .try_acquire(Resource::Model, "A")
            .is_some());
        adapter.cancellation.cancel();
        assert_ne!(sample(&adapter, 65, None).message, "timeout");
        assert_eq!(inner.output_limits.lock().unwrap().len(), 65);
    }
    #[test]
    fn mu6c_usage_does_not_stop_after_120000_tokens() {
        let inner = RecordingProvider {
            total_tokens: Some(122_686),
            output_limits: Mutex::new(Vec::new()),
        };
        let adapter = recording_adapter(&inner);
        for i in 0..3 {
            assert_eq!(
                sample(&adapter, i, Some(8192)).message,
                "timeout",
                "all model purposes must reach the provider"
            );
        }
        let usage = adapter.usage.lock().unwrap();
        assert_eq!(usage.calls.len(), 3);
        assert_eq!(
            usage.known_total_tokens,
            3 * 122_686,
            "count only the last cumulative usage frame per request"
        );
    }
    #[test]
    fn mu6c_usage_does_not_shrink_request_output_limits() {
        let inner = RecordingProvider {
            total_tokens: None,
            output_limits: Mutex::new(Vec::new()),
        };
        let adapter = recording_adapter(&inner);
        adapter.usage.lock().unwrap().known_total_tokens = 119_999;
        for i in 0..3 {
            assert_eq!(sample(&adapter, i, Some(8192)).message, "timeout");
            assert_eq!(sample(&adapter, i, None).message, "timeout");
        }
        let default_limit = inner.model_runtime_profile().output_reserve_tokens;
        assert_eq!(
            *inner.output_limits.lock().unwrap(),
            vec![
                Some(8192),
                Some(default_limit),
                Some(8192),
                Some(default_limit),
                Some(8192),
                Some(default_limit)
            ]
        );
    }
    fn request() -> CompletionRequest {
        CompletionRequest {
            followup_user: None,
            images: Vec::new(),
            system: "fixture".into(),
            user: "fixture".into(),
            output_token_limit: None,
            reasoning_effort: None,
        }
    }
    #[test]
    fn flash_agent_limit_survives_website_adapter() {
        let inner = RecordingProvider { total_tokens: None, output_limits: Mutex::new(Vec::new()) };
        let adapter = recording_adapter(&inner);
        let profile = runtime::ModelRuntimeCatalog::default().resolve(
            "deepseek-flash", runtime::ProviderToolProtocol::Native, None);
        let plan = AgentRequestPlan::for_agent_turn(profile, &[runtime::Message::user("继续学习")], &[]);
        assert_eq!(plan.active_context.output_reserve_tokens, 131_072);
        assert_eq!(adapter.chat(&plan).unwrap_err().message, "timeout");
        assert_eq!(*inner.output_limits.lock().unwrap(), vec![Some(131_072)]);
    }
}

#[cfg(test)]
#[path = "provider_capacity_tests.rs"]
mod provider_capacity_tests;
