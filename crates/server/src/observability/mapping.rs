use super::lifecycle::RunIdentity;
use super::policy::{safe_error_code, safe_purpose, safe_tool_name};
use super::queue::{ExportItem, ExportOperation};
use runtime::observation::{
    ExecutionState, ObservationCompleteness, ObservationEnvelope, ObservationKind,
    ObservationMetadata, ObservationSurface, TimingSource, TokenUsage, OBSERVATION_SCHEMA_VERSION,
};
use runtime::run_events::{ActivityStatus, RuntimeEvent};
use serde_json::{json, Map, Value};
use std::collections::HashMap;

pub struct ActivityMapper {
    ids: HashMap<u32, String>,
    revisions: HashMap<u32, u32>,
    max_spans: usize,
}

impl ActivityMapper {
    pub fn new(max_spans: usize) -> Self {
        Self {
            ids: HashMap::new(),
            revisions: HashMap::new(),
            max_spans,
        }
    }

    pub fn map(
        &mut self,
        identity: &RunIdentity,
        event: RuntimeEvent,
    ) -> Option<(ObservationEnvelope, ExportOperation)> {
        let is_new = !self.ids.contains_key(&event.activity.step_id);
        if is_new && self.ids.len() >= self.max_spans {
            return None;
        }
        let span_id = self
            .ids
            .entry(event.activity.step_id)
            .or_insert_with(|| uuid::Uuid::now_v7().to_string())
            .clone();
        let parent_span_id = event
            .activity
            .parent_step_id
            .and_then(|step| self.ids.get(&step).cloned())
            .or_else(|| Some(identity.root_span_id.clone()));
        let revision = self.revisions.entry(event.activity.step_id).or_insert(0);
        *revision += 1;
        let operation = if is_new {
            ExportOperation::Create
        } else {
            ExportOperation::Update
        };
        let kind = match event.activity.kind.as_str() {
            "model" => ObservationKind::Model,
            "tool" => ObservationKind::Tool,
            _ => ObservationKind::Activity,
        };
        let execution_state = match event.activity.status {
            ActivityStatus::Running => ExecutionState::Running,
            ActivityStatus::Succeeded | ActivityStatus::NoResult | ActivityStatus::Rejected => {
                ExecutionState::Completed
            }
            ActivityStatus::Failed => ExecutionState::Failed,
            ActivityStatus::Cancelled => ExecutionState::Cancelled,
        };
        let usage = event
            .activity
            .usage
            .clone()
            .map(|usage| {
                TokenUsage::provider_reported(
                    usage,
                    matches!(event.activity.status, ActivityStatus::Succeeded),
                )
            })
            .unwrap_or_else(TokenUsage::unknown);
        let registered_tool_name =
            (kind == ObservationKind::Tool).then(|| safe_tool_name(&event.activity.name));
        let purpose = (kind == ObservationKind::Model).then(|| safe_purpose(&event.activity.name));
        Some((
            ObservationEnvelope {
                schema_version: OBSERVATION_SCHEMA_VERSION.into(),
                local_run_ref: identity.local_run_ref.clone(),
                span_id,
                parent_span_id,
                revision: *revision,
                surface: ObservationSurface::Resident,
                kind,
                observed_at: Some(identity.observed_at_elapsed(event.elapsed_ms)),
                elapsed_ms: Some(event.elapsed_ms),
                duration_ms: event.activity.duration_ms,
                timing_source: TimingSource::RuntimeAnchor,
                execution_state: Some(execution_state),
                delivery_state: None,
                persistence_state: None,
                usage,
                completeness: ObservationCompleteness::default(),
                metadata: ObservationMetadata {
                    thread_id: Some(identity.thread_id.clone()),
                    book_ref: Some(identity.book_ref.clone()),
                    registered_tool_name,
                    purpose,
                    error_code: safe_error_code(event.activity.error_code.as_deref()),
                    result_count: event.activity.result_count,
                    model_first_text_ms: event.activity.model_first_text_ms,
                    model_first_text_at: event.activity.model_first_text_ms.and_then(|first_ms| {
                        event
                            .activity
                            .started_ms
                            .map(|started_ms| identity.observed_at_elapsed(started_ms + first_ms))
                    }),
                    model_name: event.activity.model_name,
                    model_name_source: event.activity.model_name_source,
                    model_provider: event.activity.model_provider,
                    accepted_evidence_count: event.activity.accepted_evidence_count,
                    evidence_refs: event.activity.evidence_refs,
                    executed: event.activity.started_ms.is_some(),
                    request_diagnostics: event.activity.request_diagnostics,
                    ..Default::default()
                },
            },
            operation,
        ))
    }
}

pub fn export_item(
    observation: ObservationEnvelope,
    operation: ExportOperation,
    project: &str,
) -> ExportItem {
    let root_run_id = observation.local_run_ref.clone();
    let run_id = observation.span_id.clone();
    let parent_run_id = observation.parent_span_id.clone();
    let revision = observation.revision;
    let run_type = match observation.kind {
        ObservationKind::Model => "llm",
        ObservationKind::Tool => "tool",
        ObservationKind::Run | ObservationKind::Activity => "chain",
    };
    let name = observation
        .metadata
        .registered_tool_name
        .clone()
        .or_else(|| observation.metadata.purpose.clone())
        .unwrap_or_else(|| match observation.kind {
            ObservationKind::Run => "resident_run".into(),
            ObservationKind::Activity => "resident_activity".into(),
            ObservationKind::Model => "resident_model".into(),
            ObservationKind::Tool => "unknown_tool".into(),
        });
    let metadata = serde_json::to_value(&observation).unwrap_or(Value::Null);
    let token_event = observation
        .metadata
        .model_first_text_at
        .as_ref()
        .map(|time| json!({"name": "new_token", "time": time}));
    let mut payload = match operation {
        ExportOperation::Create => {
            let mut value = json!({
                "id": run_id,
                "parent_run_id": parent_run_id,
                "name": name,
                "run_type": run_type,
                "start_time": observation.observed_at.clone(),
                "session_name": project,
                "inputs": {},
                "extra": {
                    "metadata": {
                        "thread_id": observation.metadata.thread_id.clone(),
                        "ls_model_name": observation.metadata.model_name.clone(),
                        "ub_observation": metadata
                    }
                }
            });
            if observation.execution_state != Some(ExecutionState::Running) {
                value["end_time"] = observation
                    .observed_at
                    .clone()
                    .map(Value::String)
                    .unwrap_or(Value::Null);
                value["outputs"] = json!({});
                if let Some(code) = &observation.metadata.error_code {
                    value["error"] = Value::String(code.clone());
                }
                if let Some(event) = token_event.clone() {
                    value["events"] = json!([event]);
                }
            }
            value
        }
        ExportOperation::Update => {
            let mut value = Map::new();
            value.insert(
                "end_time".into(),
                observation
                    .observed_at
                    .clone()
                    .map(Value::String)
                    .unwrap_or(Value::Null),
            );
            value.insert("outputs".into(), json!({}));
            if let Some(code) = &observation.metadata.error_code {
                value.insert("error".into(), Value::String(code.clone()));
            }
            if let Some(event) = token_event {
                value.insert("events".into(), json!([event]));
            }
            value.insert(
                "extra".into(),
                json!({
                    "metadata": {
                        "thread_id": observation.metadata.thread_id.clone(),
                        "ls_model_name": observation.metadata.model_name.clone(),
                        "ub_observation": metadata
                    }
                }),
            );
            Value::Object(value)
        }
    };
    // The Runs REST API extracts token counters from extra.metadata.usage_metadata.
    // A top-level usage_metadata field is silently ignored by LangSmith.
    if let Some(usage) = usage_metadata(&observation.usage) {
        payload["extra"]["metadata"]["usage_metadata"] = usage;
    }
    // LangSmith matches prices using both provider and model. Keep the actual
    // configured model (including provider aliases) instead of guessing a tariff.
    if let Some(provider) = &observation.metadata.model_provider {
        payload["extra"]["metadata"]["ls_provider"] = json!(provider);
    }
    ExportItem::new(
        root_run_id,
        run_id,
        parent_run_id,
        revision,
        operation,
        payload,
    )
}

fn usage_metadata(usage: &TokenUsage) -> Option<Value> {
    if usage.source != runtime::observation::UsageSource::ProviderReported {
        return None;
    }
    let mut value = Map::new();
    if let Some(tokens) = usage.input_tokens {
        value.insert("input_tokens".into(), json!(tokens));
    }
    if let Some(tokens) = usage.output_tokens {
        value.insert("output_tokens".into(), json!(tokens));
    }
    if let Some(tokens) = usage.reasoning_output_tokens {
        value.insert("output_token_details".into(), json!({"reasoning": tokens}));
    }
    if let Some(tokens) = usage.total_tokens {
        value.insert("total_tokens".into(), json!(tokens));
    }
    let mut input_details = Map::new();
    if let Some(tokens) = usage.cached_input_tokens {
        input_details.insert("cache_read".into(), json!(tokens));
    }
    if let Some(tokens) = usage.cache_creation_input_tokens {
        input_details.insert("cache_creation".into(), json!(tokens));
    }
    if !input_details.is_empty() {
        value.insert("input_token_details".into(), Value::Object(input_details));
    }
    (!value.is_empty()).then_some(Value::Object(value))
}

#[cfg(test)]
mod tests {
    #[test]
    fn deepseek_reasoning_usage_is_exported_as_an_output_subset() {
        let parsed = runtime::provider_stream::model_usage(&serde_json::json!({"usage": {
            "prompt_tokens":100, "prompt_cache_hit_tokens":80, "completion_tokens":30,
            "completion_tokens_details":{"reasoning_tokens":13}, "total_tokens":130
        }})).unwrap();
        let usage = runtime::observation::TokenUsage::provider_reported(parsed, true);
        let wire = super::usage_metadata(&usage).unwrap();
        assert_eq!(wire["output_tokens"], 30);
        assert_eq!(wire["total_tokens"], 130);
        assert_eq!(wire["output_token_details"]["reasoning"], 13);
        assert_eq!(wire["input_token_details"]["cache_read"], 80);
    }

    use super::*;
    use crate::observability::lifecycle::root_started;
    use runtime::run_events::{RunActivity, RuntimeEvent};

    #[test]
    fn basic_runs_api_payload_omits_trace_id_without_dotted_order() {
        let identity = RunIdentity::new("thread".into(), "book".into());
        let observation = root_started(&identity);
        let create = export_item(observation.clone(), ExportOperation::Create, "dev");
        let update = export_item(observation, ExportOperation::Update, "dev");

        assert!(create.payload.get("trace_id").is_none());
        assert!(create.payload.get("dotted_order").is_none());
        assert!(update.payload.get("trace_id").is_none());
        assert!(update.payload.get("dotted_order").is_none());
    }

    #[test]
    fn runs_api_usage_is_in_metadata_for_create_and_update() {
        let identity = RunIdentity::new("thread".into(), "book".into());
        let mut observation = root_started(&identity);
        observation.kind = ObservationKind::Model;
        observation.execution_state = Some(ExecutionState::Completed);
        observation.usage = TokenUsage::provider_reported(runtime::provider_stream::ModelUsage {
            input_tokens: Some(1000), output_tokens: Some(20), cached_input_tokens: Some(800),
            total_tokens: Some(1020), ..Default::default()
        }, true);
        for operation in [ExportOperation::Create, ExportOperation::Update] {
            let item = export_item(observation.clone(), operation, "dev");
            assert!(item.payload.get("usage_metadata").is_none());
            let usage = &item.payload["extra"]["metadata"]["usage_metadata"];
            assert_eq!(usage["input_tokens"], 1000);
            assert_eq!(usage["input_token_details"]["cache_read"], 800);
            assert_eq!(item.payload["outputs"], json!({}));
        }
    }

    #[test]
    fn provider_and_teaching_purpose_survive_runtime_wrappers_and_export() {
        use crate::host_lifecycle::ServiceAdapter;
        use crate::service_limits::{LimitedAdapter, Resources, ServiceLimits};
        use runtime::run_context::CancellableAdapter;
        use runtime::run_events::{ObservedAdapter, RunEventSink, RunEvents};
        use runtime::{CompletionRequest, ModelAdapter, ProviderConfig, ProviderMode};
        use std::sync::{atomic::AtomicBool, mpsc, Arc, Mutex};

        struct Collector(mpsc::Sender<RuntimeEvent>);
        impl RunEventSink for Collector {
            fn emit(&self, event: RuntimeEvent) {
                self.0.send(event).unwrap();
            }
        }

        for mode in [ProviderMode::Native, ProviderMode::ReAct] {
            for (endpoint, provider) in [
                ("https://api.deepseek.com", Some("deepseek")),
                ("https://api.deepseek.com/v1", Some("deepseek")),
                ("https://other-provider.example/v1", None),
            ] {
                // The real wrappers and adapters provide identity, while the
                // stopped service prevents this regression from sending HTTP.
                let service = ServiceAdapter::from_config(
                    ProviderConfig {
                        agent_reasoning_effort: Default::default(),
                        mode,
                        api_key: "CANARY_PRIVATE_KEY".into(),
                        base_url: endpoint.into(),
                        model: "deepseek-v4-flash".into(),
                    },
                    Arc::new(AtomicBool::new(true)),
                );
                let limited = LimitedAdapter {
                    inner: service.as_ref(),
                    resources: Resources::new(Arc::new(ServiceLimits::default())),
                    owner: "test-owner".into(),
                    background: false,
                    cancellation: Default::default(),
                    usage: Mutex::new(Default::default()),
                    stream: None,
                    authorization: None,
                };
                let (send, receive) = mpsc::channel();
                let events = RunEvents::new(Some(Arc::new(Collector(send))));
                let observed = ObservedAdapter {
                    inner: &limited,
                    events: events.clone(),
                    cancellation: Default::default(),
                    runtime_profile: limited.model_runtime_profile(),
                };
                let cancellable = CancellableAdapter {
                    inner: &observed,
                    cancellation: Default::default(),
                };
                assert_eq!(cancellable.model_provider().as_deref(), provider);
                let _scope = events.scope(None, "tutor_delivery_review");
                assert!(cancellable.complete(CompletionRequest {
                    followup_user: None,
                    system: "CANARY_PRIVATE_SYSTEM".into(),
                    user: "CANARY_PRIVATE_USER".into(),
                    images: Vec::new(),
                    output_token_limit: Some(100),
                    reasoning_effort: None,
                }).is_err());

                let identity = RunIdentity::new("thread".into(), "book".into());
                let mut mapper = ActivityMapper::new(4);
                let events: Vec<_> = receive.try_iter().collect();
                assert_eq!(events.len(), 2);
                for (index, event) in events.into_iter().enumerate() {
                    let (observation, operation) = mapper.map(&identity, event).unwrap();
                    assert_eq!(operation, if index == 0 { ExportOperation::Create } else { ExportOperation::Update });
                    let item = export_item(observation, operation, "dev");
                    let metadata = &item.payload["extra"]["metadata"];
                    assert_eq!(metadata["ls_provider"].as_str(), provider);
                    assert_eq!(metadata["ls_model_name"], "deepseek-v4-flash");
                    assert_eq!(metadata["ub_observation"]["metadata"]["purpose"], "tutor_delivery_review");
                    if provider.is_none() {
                        assert!(metadata.get("ls_provider").is_none());
                    }
                    assert!(!item.payload.to_string().contains("CANARY_PRIVATE"));
                    assert!(!item.payload.to_string().contains(endpoint));
                }
            }
        }
    }

    #[test]
    fn mapping_drops_content_canaries_and_preserves_rejection_semantics() {
        let identity = RunIdentity::new("thread".into(), "book".into());
        let mut mapper = ActivityMapper::new(10);
        let (observation, operation) = mapper
            .map(
                &identity,
                RuntimeEvent {
                    elapsed_ms: 4.0,
                    activity: RunActivity {
                        step_id: 1,
                        parent_step_id: None,
                        kind: "tool".into(),
                        name: "CANARY_PRIVATE_ARGUMENT".into(),
                        label: "CANARY_PRIVATE_LABEL".into(),
                        status: ActivityStatus::Rejected,
                        started_ms: None,
                        duration_ms: None,
                        result_count: None,
                        error_code: Some("provider CANARY_PRIVATE_BODY".into()),
                        usage_total_tokens: None,
                        usage: None,
                        model_first_text_ms: None,
                        request_diagnostics: None,
                        model_name: None,
                        model_name_source: None,
                        model_provider: None,
                        accepted_evidence_count: None,
                        evidence_refs: Vec::new(),
                    },
                },
            )
            .unwrap();
        assert_eq!(operation, ExportOperation::Create);
        assert_eq!(
            observation.metadata.registered_tool_name.as_deref(),
            Some("unknown_tool")
        );
        assert!(!observation.metadata.executed);
        let exported = export_item(observation, operation, "dev");
        assert!(exported.payload["end_time"].is_string());
        assert_eq!(exported.payload["extra"]["metadata"]["thread_id"], "thread");
        let payload = serde_json::to_string(&exported.payload).unwrap();
        assert!(!payload.contains("CANARY_PRIVATE"));
    }

    #[test]
    fn equal_step_numbers_in_different_runs_never_share_span_identity() {
        let mut first_mapper = ActivityMapper::new(10);
        let mut second_mapper = ActivityMapper::new(10);
        let first_identity = RunIdentity::new("thread-a".into(), "book".into());
        let second_identity = RunIdentity::new("thread-b".into(), "book".into());
        let event = || RuntimeEvent {
            elapsed_ms: 1.0,
            activity: RunActivity {
                step_id: 1,
                parent_step_id: None,
                kind: "model".into(),
                name: "outer".into(),
                label: "answer".into(),
                status: ActivityStatus::Running,
                started_ms: Some(1.0),
                duration_ms: None,
                result_count: None,
                error_code: None,
                usage_total_tokens: None,
                usage: None,
                model_first_text_ms: None,
                request_diagnostics: None,
                model_name: Some("configured-model".into()),
                model_name_source: Some("configured".into()),
                model_provider: None,
                accepted_evidence_count: None,
                evidence_refs: Vec::new(),
            },
        };
        let first = first_mapper.map(&first_identity, event()).unwrap().0;
        let second = second_mapper.map(&second_identity, event()).unwrap().0;
        assert_ne!(first.local_run_ref, second.local_run_ref);
        assert_ne!(first.span_id, second.span_id);
        assert_ne!(first.metadata.thread_id, second.metadata.thread_id);
    }

    #[test]
    fn first_tool_terminal_contains_accepted_evidence_without_content() {
        let identity = RunIdentity::new("thread".into(), "book".into());
        let mut mapper = ActivityMapper::new(10);
        let mut activity = RunActivity {
            step_id: 1,
            parent_step_id: None,
            kind: "tool".into(),
            name: "book.text".into(),
            label: "CANARY_PRIVATE_PREVIEW".into(),
            status: ActivityStatus::Running,
            started_ms: Some(1.0),
            duration_ms: None,
            result_count: None,
            error_code: None,
            usage_total_tokens: None,
            usage: None,
            model_first_text_ms: None,
            request_diagnostics: None,
            model_name: None,
            model_name_source: None,
            model_provider: None,
            accepted_evidence_count: None,
            evidence_refs: Vec::new(),
        };
        let (started, operation) = mapper
            .map(
                &identity,
                RuntimeEvent {
                    elapsed_ms: 1.0,
                    activity: activity.clone(),
                },
            )
            .unwrap();
        assert_eq!(operation, ExportOperation::Create);
        assert!(export_item(started.clone(), operation, "dev").payload.get("end_time").is_none());
        activity.accept_evidence(&[read_tools::EvidenceRange {
            start_lid: "1.1".into(), end_lid: "1.1".into(), ranges: Vec::new(),
        }]);
        activity.status = ActivityStatus::Succeeded;
        activity.duration_ms = Some(2.0);
        activity.result_count = Some(3);
        let (finished, operation) = mapper
            .map(
                &identity,
                RuntimeEvent {
                    elapsed_ms: 3.0,
                    activity,
                },
            )
            .unwrap();
        assert_eq!(operation, ExportOperation::Update);
        assert_eq!(finished.span_id, started.span_id);
        assert_eq!(finished.revision, started.revision + 1);
        assert_eq!(finished.metadata.result_count, Some(3));
        assert_eq!(finished.metadata.accepted_evidence_count, Some(1));
        assert_eq!(finished.metadata.evidence_refs, vec!["1.1"]);
        let item = export_item(finished, operation, "dev");
        assert!(item.payload["end_time"].is_string());
        assert!(!serde_json::to_string(&item.payload)
            .unwrap()
            .contains("CANARY_PRIVATE"));
    }
}
