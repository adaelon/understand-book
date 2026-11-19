//! Service-wide capacity and per-request permits. No business state lives here.
use runtime::provider_stream::ModelUsage;
use runtime::run_context::CancellationToken;
use runtime::AdapterError;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};

#[derive(Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ServiceLimits {
    pub http_workers: usize,
    pub active_runs: usize,
    pub user_active_runs: usize,
    pub queued_runs: usize,
    pub user_queued_runs: usize,
    pub model_slots: usize,
    pub user_model_slots: usize,
    pub model_request_timeout_seconds: u64,
    pub background_model_request_timeout_seconds: u64,
    pub model_connect_timeout_seconds: u64,
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
            http_workers: 4,
            active_runs: 8,
            user_active_runs: 1,
            queued_runs: 20,
            user_queued_runs: 4,
            model_slots: 8,
            user_model_slots: 1,
            model_request_timeout_seconds: 600,
            background_model_request_timeout_seconds: 180,
            model_connect_timeout_seconds: 15,
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
            limits.http_workers,
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
            || limits.model_request_timeout_seconds == 0
            || limits.background_model_request_timeout_seconds == 0
            || limits.model_connect_timeout_seconds == 0
        {
            return Err(invalid());
        }
        Ok(Arc::new(limits))
    }
}
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Resource {
    Model,
    BackgroundModel,
    Preview,
    Plot,
    Animation,
    Observation,
    SyncWait,
}
#[derive(Default)]
struct Slots {
    active: BTreeMap<(Resource, String), usize>,
    waiting: std::collections::VecDeque<(u64, Resource, String)>,
    next: u64,
    last_owner: BTreeMap<Resource, String>,
}
pub struct Resources {
    pub limits: Arc<ServiceLimits>,
    slots: Mutex<Slots>,
    changed: Condvar,
}
pub struct Permit {
    resources: Arc<Resources>,
    resource: Resource,
    owner: String,
}
impl Resources {
    pub fn measured_usage(&self) -> (usize, usize, usize) {
        let slots = self.slots.lock().unwrap();
        (
            slots
                .active
                .iter()
                .filter(|((r, _), _)| matches!(r, Resource::Model | Resource::BackgroundModel))
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
            // One separate background call can finish without occupying the
            // foreground allowance. Never fan background work out unboundedly.
            Resource::BackgroundModel => (1, 1),
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
            || (resource == Resource::BackgroundModel
                && slots.waiting.iter().any(|(_, r, _)| *r == Resource::Model))
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
    pub fn acquire_model(
        self: &Arc<Self>,
        owner: &str,
        cancellation: &CancellationToken,
        background: bool,
    ) -> Result<Permit, AdapterError> {
        let resource = if background { Resource::BackgroundModel } else { Resource::Model };
        let (global_limit, user_limit) = self.bounds(resource);
        let mut slots = self.slots.lock().unwrap();
        let ticket = slots.next;
        slots.next += 1;
        slots.waiting.push_back((ticket, resource, owner.into()));
        loop {
            if let Err(e) = cancellation.check() {
                slots.waiting.retain(|(id, _, _)| *id != ticket);
                self.changed.notify_all();
                return Err(AdapterError {
                    spend_stop: None,
                    message: e.error_code,
                });
            }
            let mut users = BTreeMap::new();
            for (id, waiting_resource, user) in &slots.waiting {
                if *waiting_resource == resource && slots
                    .active
                    .get(&(resource, user.clone()))
                    .copied()
                    .unwrap_or(0)
                    < user_limit
                {
                    users.entry(user.clone()).or_insert(*id);
                }
            }
            let next = users
                .iter()
                .find(|(u, _)| slots.last_owner.get(&resource).is_none_or(|last| *u > last))
                .or_else(|| users.iter().next())
                .map(|(_, id)| *id);
            let total: usize = slots
                .active
                .iter()
                .filter(|((r, _), _)| *r == resource)
                .map(|(_, n)| *n)
                .sum();
            // A paid background call may finish while a foreground call starts,
            // but new background sends yield until foreground waiters are served.
            let foreground_waiting = background
                && slots.waiting.iter().any(|(_, r, _)| *r == Resource::Model);
            if next == Some(ticket) && total < global_limit && !foreground_waiting {
                slots.waiting.retain(|(id, _, _)| *id != ticket);
                slots.last_owner.insert(resource, owner.into());
                *slots
                    .active
                    .entry((resource, owner.into()))
                    .or_default() += 1;
                self.changed.notify_all();
                return Ok(Permit {
                    resources: self.clone(),
                    resource,
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
pub struct RunUsage {
    pub calls: Vec<Option<ModelUsage>>,
    pub known_total_tokens: u64,
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capacity_200_candidate_overrides_default_capacity() {
        let root = tempfile::tempdir().unwrap();
        let defaults = ServiceLimits::load(root.path()).unwrap();
        assert_eq!(defaults.http_workers, 4);
        assert_eq!(defaults.active_runs, 8);
        assert_eq!(defaults.model_slots, 8);
        assert_eq!(defaults.resident_users, 10);
        assert_eq!(defaults.model_request_timeout_seconds, 600);
        std::fs::write(root.path().join("service-limits.json"), include_str!(
            "../../../scripts/linux/service-limits.200-users.json.example"
        )).unwrap();
        let candidate = ServiceLimits::load(root.path()).unwrap();
        assert_eq!(candidate.http_workers, 8);
        assert_eq!(candidate.active_runs, 64);
        assert_eq!(candidate.model_slots, 8);
        assert_eq!(candidate.resident_users, 300);
        assert_eq!(candidate.sse_connections, 256);
        assert_eq!(candidate.background_model_request_timeout_seconds, 180);
        assert_eq!(candidate.model_connect_timeout_seconds, 15);
    }

    #[test]
    fn capacity_limits_reject_zero_workers_timeouts_and_unknown_keys() {
        let root = tempfile::tempdir().unwrap();
        for key in ["http_workers", "model_request_timeout_seconds",
            "background_model_request_timeout_seconds", "model_connect_timeout_seconds",
            "unknown_capacity_option"] {
            std::fs::write(root.path().join("service-limits.json"),
                serde_json::json!({key: 0}).to_string()).unwrap();
            assert_eq!(ServiceLimits::load(root.path()).err().unwrap().error_code,
                "SERVICE_LIMITS_INVALID", "{key}");
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
        assert!(resources.acquire_model("A", &cancellation, false).is_err());
    }

    fn wait_for_model_waiters(resources: &Resources, expected: usize) {
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let mut slots = resources.slots.lock().unwrap();
        while slots.waiting.len() != expected && std::time::Instant::now() < deadline {
            slots = resources.changed.wait_timeout(slots, Duration::from_millis(10)).unwrap().0;
        }
        assert_eq!(slots.waiting.len(), expected);
    }

    #[test]
    fn foreground_model_does_not_wait_for_same_users_background_call() {
        let resources = Resources::new(Arc::new(ServiceLimits { model_slots: 1, ..Default::default() }));
        std::thread::scope(|scope| {
            let background = resources.acquire_model("A", &CancellationToken::default(), true).unwrap();
            let (tx, rx) = std::sync::mpsc::channel();
            let resources = &resources;
            scope.spawn(move || tx.send(resources.acquire_model("A", &CancellationToken::default(), false)).unwrap());
            let foreground = rx.recv_timeout(Duration::from_secs(5)).unwrap().unwrap();
            assert_eq!(resources.measured_usage(), (2, 2, 0), "both lanes count as model work");
            assert!(resources.try_acquire(Resource::Model, "B").is_none(), "foreground remains bounded");
            assert!(resources.try_acquire(Resource::BackgroundModel, "B").is_none(), "background remains bounded");
            drop((foreground, background));
        });
        assert_eq!(resources.measured_usage(), (0, 0, 0));
    }

    #[test]
    fn background_model_lane_is_bounded_and_waiting_cancellation_releases_ticket() {
        let resources = Resources::new(Arc::new(ServiceLimits::default()));
        let cancellation = CancellationToken::default();
        std::thread::scope(|scope| {
            let background = resources.acquire_model("A", &CancellationToken::default(), true).unwrap();
            let (tx, rx) = std::sync::mpsc::channel();
            let resources = &resources;
            let cancellation = &cancellation;
            scope.spawn(move || tx.send(resources.acquire_model("B", cancellation, true)).unwrap());
            wait_for_model_waiters(resources, 1);
            assert_eq!(resources.measured_usage(), (1, 1, 1));
            cancellation.cancel();
            assert!(rx.recv_timeout(Duration::from_secs(5)).unwrap().is_err());
            assert_eq!(resources.measured_usage(), (1, 1, 0));
            drop(background);
        });
        assert!(resources.acquire_model("B", &CancellationToken::default(), true).is_ok());
    }

    #[test]
    fn background_model_send_yields_to_foreground_waiter() {
        let resources = Resources::new(Arc::new(ServiceLimits { model_slots: 1, ..Default::default() }));
        std::thread::scope(|scope| {
            let foreground = resources.acquire_model("A", &CancellationToken::default(), false).unwrap();
            let background = resources.acquire_model("A", &CancellationToken::default(), true).unwrap();
            let (background_tx, background_rx) = std::sync::mpsc::channel();
            let (foreground_tx, foreground_rx) = std::sync::mpsc::channel();
            let resources = &resources;
            scope.spawn(move || background_tx.send(resources.acquire_model("B", &CancellationToken::default(), true)).unwrap());
            wait_for_model_waiters(resources, 1);
            scope.spawn(move || foreground_tx.send(resources.acquire_model("B", &CancellationToken::default(), false)).unwrap());
            wait_for_model_waiters(resources, 2);
            drop(background);
            assert!(resources.try_acquire(Resource::BackgroundModel, "C").is_none());
            assert!(background_rx.recv_timeout(Duration::from_millis(100)).is_err(), "background cannot start while foreground is waiting");
            drop(foreground);
            let foreground = foreground_rx.recv_timeout(Duration::from_secs(5)).unwrap().unwrap();
            let background = background_rx.recv_timeout(Duration::from_secs(5)).unwrap().unwrap();
            assert_eq!(resources.measured_usage(), (2, 2, 0));
            drop((foreground, background));
        });
    }
}
