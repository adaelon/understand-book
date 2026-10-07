//! Service-wide capacity and per-request permits. No business state lives here.
use runtime::provider_stream::{ModelDelta, ModelObserver, ModelUsage};
use runtime::run_context::CancellationToken;
use runtime::{
    AdapterError, AgentRequestPlan, AssistantTurn, CompletionRequest, ModelAdapter, ParsedResponse,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};

#[derive(Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ServiceLimits {
    pub active_runs: usize,
    pub user_active_runs: usize,
    pub queued_runs: usize,
    pub user_queued_runs: usize,
    pub model_slots: usize,
    pub user_model_slots: usize,
    pub preview_slots: usize,
    pub plot_slots: usize,
    pub animation_slots: usize,
    pub sse_connections: usize,
    pub user_sse_connections: usize,
    pub sync_waiters: usize,
    pub user_sync_waiters: usize,
    pub request_bytes: usize,
    pub event_bytes: usize,
    pub resident_users: usize,
    pub workspaces: usize,
    pub user_workspaces: usize,
    pub books: usize,
    pub book_bytes: u64,
    pub presentation_bytes: u64,
    pub presentation_files: usize,
}
impl Default for ServiceLimits {
    fn default() -> Self {
        Self {
            active_runs: 4,
            user_active_runs: 1,
            queued_runs: 20,
            user_queued_runs: 4,
            model_slots: 2,
            user_model_slots: 1,
            preview_slots: 1,
            plot_slots: 1,
            animation_slots: 1,
            sse_connections: 40,
            user_sse_connections: 4,
            sync_waiters: 20,
            user_sync_waiters: 4,
            request_bytes: 65536,
            event_bytes: 256 * 1024,
            resident_users: 10,
            workspaces: 20,
            user_workspaces: 4,
            books: 20,
            book_bytes: 2 * 1024 * 1024 * 1024,
            presentation_bytes: 512 * 1024 * 1024,
            presentation_files: 4096,
        }
    }
}
impl ServiceLimits {
    pub fn load(root: &std::path::Path) -> Result<Arc<Self>, read_tools::ToolError> {
        let invalid = || {
            crate::user_storage_paths::error(
                "SERVICE_LIMITS_INVALID",
                "validation",
                "Invalid service-limits.json",
            )
        };
        let limits: Self = match std::fs::read(root.join("service-limits.json")) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|_| invalid())?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(_) => return Err(invalid()),
        };
        if [
            limits.active_runs,
            limits.user_active_runs,
            limits.queued_runs,
            limits.user_queued_runs,
            limits.model_slots,
            limits.user_model_slots,
            limits.preview_slots,
            limits.plot_slots,
            limits.animation_slots,
            limits.sse_connections,
            limits.user_sse_connections,
            limits.sync_waiters,
            limits.user_sync_waiters,
            limits.request_bytes,
            limits.event_bytes,
            limits.resident_users,
            limits.workspaces,
            limits.user_workspaces,
            limits.books,
            limits.presentation_files,
        ]
        .contains(&0)
            || limits.book_bytes == 0
            || limits.presentation_bytes == 0
        {
            return Err(invalid());
        }
        Ok(Arc::new(limits))
    }
}
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Resource {
    Model,
    Preview,
    Plot,
    Animation,
    Observation,
    SyncWait,
}
#[derive(Default)]
struct Slots {
    active: BTreeMap<(Resource, String), usize>,
    waiting: std::collections::VecDeque<(u64, String)>,
    next: u64,
    last_owner: Option<String>,
}
pub(crate) struct Resources {
    pub limits: Arc<ServiceLimits>,
    slots: Mutex<Slots>,
    changed: Condvar,
}
pub(crate) struct Permit {
    resources: Arc<Resources>,
    resource: Resource,
    owner: String,
}
impl Resources {
    #[cfg(test)]
    pub(crate) fn measured_usage(&self) -> (usize, usize, usize) {
        let slots = self.slots.lock().unwrap();
        (
            slots
                .active
                .iter()
                .filter(|((r, _), _)| *r == Resource::Model)
                .map(|(_, n)| *n)
                .sum(),
            slots.active.values().sum(),
            slots.waiting.len(),
        )
    }
    pub fn new(limits: Arc<ServiceLimits>) -> Arc<Self> {
        Arc::new(Self {
            limits,
            slots: Mutex::new(Slots::default()),
            changed: Condvar::new(),
        })
    }
    fn bounds(&self, resource: Resource) -> (usize, usize) {
        let l = &self.limits;
        match resource {
            Resource::Model => (l.model_slots, l.user_model_slots),
            Resource::Preview => (l.preview_slots, 1),
            Resource::Plot => (l.plot_slots, 1),
            Resource::Animation => (l.animation_slots, 1),
            Resource::Observation => (l.sse_connections, l.user_sse_connections),
            Resource::SyncWait => (l.sync_waiters, l.user_sync_waiters),
        }
    }
    pub fn try_acquire(self: &Arc<Self>, resource: Resource, owner: &str) -> Option<Permit> {
        let mut slots = self.slots.lock().unwrap();
        let (global, user) = self.bounds(resource);
        let key = (resource, owner.to_owned());
        if slots
            .active
            .iter()
            .filter(|((r, _), _)| *r == resource)
            .map(|(_, v)| *v)
            .sum::<usize>()
            >= global
            || slots.active.get(&key).copied().unwrap_or(0) >= user
        {
            return None;
        }
        *slots.active.entry(key).or_default() += 1;
        Some(Permit {
            resources: self.clone(),
            resource,
            owner: owner.into(),
        })
    }
    fn acquire_model(
        self: &Arc<Self>,
        owner: &str,
        cancellation: &CancellationToken,
    ) -> Result<Permit, AdapterError> {
        let mut slots = self.slots.lock().unwrap();
        let ticket = slots.next;
        slots.next += 1;
        slots.waiting.push_back((ticket, owner.into()));
        loop {
            if let Err(e) = cancellation.check() {
                slots.waiting.retain(|(id, _)| *id != ticket);
                self.changed.notify_all();
                return Err(AdapterError {
                    message: e.error_code,
                });
            }
            let mut users = BTreeMap::new();
            for (id, user) in &slots.waiting {
                if slots
                    .active
                    .get(&(Resource::Model, user.clone()))
                    .copied()
                    .unwrap_or(0)
                    < self.limits.user_model_slots
                {
                    users.entry(user.clone()).or_insert(*id);
                }
            }
            let next = users
                .iter()
                .find(|(u, _)| slots.last_owner.as_ref().is_none_or(|last| *u > last))
                .or_else(|| users.iter().next())
                .map(|(_, id)| *id);
            let total: usize = slots
                .active
                .iter()
                .filter(|((r, _), _)| *r == Resource::Model)
                .map(|(_, n)| *n)
                .sum();
            if next == Some(ticket) && total < self.limits.model_slots {
                slots.waiting.retain(|(id, _)| *id != ticket);
                slots.last_owner = Some(owner.into());
                *slots
                    .active
                    .entry((Resource::Model, owner.into()))
                    .or_default() += 1;
                self.changed.notify_all();
                return Ok(Permit {
                    resources: self.clone(),
                    resource: Resource::Model,
                    owner: owner.into(),
                });
            }
            slots = self
                .changed
                .wait_timeout(slots, Duration::from_millis(50))
                .unwrap()
                .0;
        }
    }
}
impl Drop for Permit {
    fn drop(&mut self) {
        let mut slots = self.resources.slots.lock().unwrap();
        let key = (self.resource, self.owner.clone());
        let count = slots.active.get_mut(&key).unwrap();
        *count -= 1;
        if *count == 0 {
            slots.active.remove(&key);
        }
        self.resources.changed.notify_all();
    }
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct RunUsage {
    pub calls: Vec<Option<ModelUsage>>,
    pub known_total_tokens: u64,
}
/// All purposes share model capacity and usage tracking; permits end before tool dispatch.
pub(crate) struct LimitedAdapter<'a> {
    pub inner: &'a dyn ModelAdapter,
    pub resources: Arc<Resources>,
    pub owner: String,
    pub cancellation: CancellationToken,
    pub usage: Mutex<RunUsage>,
    pub stream: Option<Arc<crate::agent_stream::RunStream>>,
    pub authorization: Option<(
        &'a crate::authorization::Authorization,
        &'a crate::published_library::PublishedBookRef,
    )>,
}
impl LimitedAdapter<'_> {
    fn output_limit(&self, requested: Option<u32>) -> Option<u32> {
        Some(
            requested
                .unwrap_or(self.inner.model_runtime_profile().output_reserve_tokens)
                .max(1),
        )
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
            .acquire_model(&self.owner, &self.cancellation);
        if let Some(stream) = &self.stream {
            stream.resource_wait(false);
        }
        let _permit = permit?;
        self.cancellation.check().map_err(|e| AdapterError {
            message: e.error_code,
        })?;
        if let Some((access, publication)) = self.authorization {
            access
                .library
                .lock()
                .unwrap()
                .authorize(&self.owner, publication)
                .map_err(|_| AdapterError {
                    message: "RUN_PERMISSION_REVOKED".into(),
                })?;
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
        r.output_token_limit = self.output_limit(r.output_token_limit);
        self.call(o, |o| self.inner.complete_observed(r, o), |_| None)
    }
    fn complete_structured_observed(
        &self,
        mut r: CompletionRequest,
        o: &mut dyn ModelObserver,
    ) -> Result<serde_json::Value, AdapterError> {
        r.output_token_limit = self.output_limit(r.output_token_limit);
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
        r.output_token_limit = self.output_limit(r.output_token_limit);
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
            system: "fixture".into(),
            user: "fixture".into(),
            output_token_limit: None,
            reasoning_effort: None,
        }
    }
    #[test]
    fn mu6c_saved_usage_with_old_budget_flag_remains_readable() {
        let usage: RunUsage = serde_json::from_value(serde_json::json!({
            "calls": [{"total_tokens": 122686}],
            "known_total_tokens": 122686,
            "budget_exhausted": true
        }))
        .unwrap();
        assert_eq!(usage.known_total_tokens, 122_686);
        assert_eq!(usage.calls[0].as_ref().unwrap().total_tokens, Some(122_686));
        let saved = serde_json::to_value(usage).unwrap();
        assert!(saved.get("budget_exhausted").is_none());
    }
    #[test]
    fn mu6c_resources_are_independent_and_user_permits_release_on_exit() {
        let resources = Resources::new(Arc::new(ServiceLimits::default()));
        let a = resources.try_acquire(Resource::Model, "A").unwrap();
        assert!(resources.try_acquire(Resource::Model, "A").is_none());
        assert!(resources.try_acquire(Resource::Model, "B").is_some());
        let preview = resources.try_acquire(Resource::Preview, "A").unwrap();
        let plot = resources.try_acquire(Resource::Plot, "A").unwrap();
        let animation = resources.try_acquire(Resource::Animation, "A").unwrap();
        assert!(resources.try_acquire(Resource::Preview, "B").is_none());
        drop((a, preview, plot, animation));
        assert!(resources.try_acquire(Resource::Model, "A").is_some());
        let cancellation = CancellationToken::default();
        cancellation.cancel();
        assert!(resources.acquire_model("A", &cancellation).is_err());
    }
}
