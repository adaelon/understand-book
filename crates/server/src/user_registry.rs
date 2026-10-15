//! One loaded private authority per user. Handles pin users across background work/Runs.
use crate::{
    control_store::{ControlStore, ServiceWriter},
    user_runtime::UserRuntime,
};
use read_tools::ToolError;
use std::{
    collections::BTreeMap,
    path::Path,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub const USER_IDLE_TTL: Duration = Duration::from_secs(30 * 60);
pub const MAX_LOADED_USERS: usize = 10;
pub type UserHandle = Arc<Mutex<UserRuntime>>;

pub struct UserRegistry {
    control: ControlStore,
    users: BTreeMap<String, UserHandle>,
    pub(crate) max_loaded: usize,
    pub(crate) presentation_limits: (u64, usize),
}

impl UserRegistry {
    /// One registry is created with the root's exclusive writer, never per request/window.
    pub fn open(root: &Path) -> Result<Self, ToolError> {
        Ok(Self {
            control: ControlStore::open(ServiceWriter::acquire(root)?)?,
            users: BTreeMap::new(),
            max_loaded: MAX_LOADED_USERS,
            presentation_limits: (512 * 1024 * 1024, 4096),
        })
    }
    pub fn create_user(&mut self, owner: &str) -> Result<(), ToolError> {
        self.control.create_user(owner)
    }
    pub fn writer(&self) -> std::sync::Arc<ServiceWriter> {
        self.control.writer.clone()
    }
    pub fn paths(
        &self,
        owner: &str,
    ) -> Result<crate::user_storage_paths::UserStoragePaths, ToolError> {
        self.control.user_paths(owner)
    }
    pub fn get(&mut self, owner: &str, now: &str) -> Result<UserHandle, ToolError> {
        let paths = self.control.user_paths(owner)?;
        self.load(owner, paths, now)
    }
    /// Service recovery may finish accepted work for an account that has since been disabled.
    pub(crate) fn get_for_recovery(
        &mut self,
        owner: &str,
        now: &str,
    ) -> Result<UserHandle, ToolError> {
        let exists: bool = self
            .control
            .connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM users WHERE user_id=?)",
                [owner],
                |r| r.get(0),
            )
            .map_err(|_| {
                crate::user_storage_paths::error(
                    "CONTROL_STORAGE_UNAVAILABLE",
                    "unavailable",
                    "Cannot recover owner",
                )
            })?;
        if !exists {
            return Err(crate::authorization::missing());
        }
        let paths = self.control.writer.paths(owner)?;
        self.load(owner, paths, now)
    }
    fn load(
        &mut self,
        owner: &str,
        paths: crate::user_storage_paths::UserStoragePaths,
        now: &str,
    ) -> Result<UserHandle, ToolError> {
        if !self.users.contains_key(owner) {
            self.evict_idle(Instant::now())?;
            if self.users.len() >= self.max_loaded {
                return Err(crate::user_storage_paths::error(
                    "USER_RUNTIME_CAPACITY",
                    "unavailable",
                    "All resident user slots are occupied",
                ));
            }
            let mut user = UserRuntime::open_service(&paths, self.control.writer.clone(), now)?;
            user.presentation_limits = Some(self.presentation_limits);
            self.users.insert(owner.into(), Arc::new(Mutex::new(user)));
        }
        let handle = self.users.get(owner).unwrap();
        handle.lock().unwrap_or_else(|e| e.into_inner()).last_access = Instant::now();
        Ok(handle.clone())
    }
    pub(crate) fn flush_reads(&self) -> Result<(), ToolError> {
        for user in self.users.values() {
            user.lock().unwrap().store.flush_pending_reads()?;
        }
        Ok(())
    }
    pub(crate) fn loaded_users(&self) -> Vec<UserHandle> {
        self.users.values().cloned().collect()
    }
    /// Startup discovers durable work without loading every account into RAM.
    pub(crate) fn pending_review_owners(
        &self,
    ) -> Result<std::collections::VecDeque<String>, ToolError> {
        let mut query = self
            .control
            .connection
            .prepare("SELECT user_id FROM users WHERE disabled=0 ORDER BY user_id")
            .map_err(|_| {
                crate::user_storage_paths::error(
                    "REVIEW_RECOVERY_FAILED",
                    "unavailable",
                    "Cannot read background owners",
                )
            })?;
        let owners = query
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(|_| {
                crate::user_storage_paths::error(
                    "REVIEW_RECOVERY_FAILED",
                    "unavailable",
                    "Cannot read background owners",
                )
            })?;
        let mut pending = std::collections::VecDeque::new();
        for owner in owners {
            let owner = owner.map_err(|_| {
                crate::user_storage_paths::error(
                    "REVIEW_RECOVERY_FAILED",
                    "unavailable",
                    "Cannot read background owner",
                )
            })?;
            let path = self.control.writer.paths(&owner)?.memory;
            match std::fs::read(&path) {
                Ok(bytes) => {
                    let document: serde_json::Value =
                        serde_json::from_slice(&bytes).map_err(|_| {
                            crate::user_storage_paths::error(
                                "REVIEW_RECOVERY_FAILED",
                                "unavailable",
                                "Cannot read persisted memory",
                            )
                        })?;
                    if document["review_state"]["review_jobs"]
                        .as_array()
                        .is_some_and(|jobs| jobs.iter().any(|j| j["status"] != "completed"))
                    {
                        pending.push_back(owner);
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => {
                    return Err(crate::user_storage_paths::error(
                        "REVIEW_RECOVERY_FAILED",
                        "unavailable",
                        "Cannot read persisted memory",
                    ))
                }
            }
        }
        Ok(pending)
    }
    pub(crate) fn load_for_review(
        &mut self,
        owner: &str,
        now: &str,
    ) -> Result<UserHandle, ToolError> {
        if let Some(handle) = self.users.get(owner) {
            return Ok(handle.clone());
        }
        let handle = self.get(owner, now)?;
        // Background recovery does not count as reader activity; its handle pins I/O.
        handle.lock().unwrap().last_access = Instant::now() - USER_IDLE_TTL;
        Ok(handle)
    }
    /// Call while holding the registry lock, before allocating additional users.
    /// An outstanding handle pins background work, migration, Run and unsaved-result owners.
    pub fn evict_idle(&mut self, now: Instant) -> Result<usize, ToolError> {
        let mut removed = 0;
        let keys: Vec<_> = self.users.keys().cloned().collect();
        for key in keys {
            let handle = &self.users[&key];
            if Arc::strong_count(handle) != 1 {
                continue;
            }
            if handle
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .prepare_eviction(now)?
            {
                self.users.remove(&key);
                removed += 1;
            }
        }
        Ok(removed)
    }
}
