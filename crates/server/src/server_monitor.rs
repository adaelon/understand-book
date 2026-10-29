//! Small, shared on-demand samples for the admin dashboard. No private Reader state is loaded.
use crate::{admin_store::AdminStore, auth::Principal, model_spend_store::api_storage};
use read_tools::ToolError;
use rusqlite::Connection;
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, Instant},
};

const INTERVAL: Duration = Duration::from_secs(5);
const ONLINE_WINDOW: Duration = Duration::from_secs(90);
const HISTORY_SECONDS: i64 = 600;

pub(crate) struct ServerMonitor {
    root: PathBuf,
    started: Instant,
    presence: Mutex<BTreeMap<Vec<u8>, Instant>>,
    samples: Mutex<Samples>,
}

#[derive(Default)]
struct Samples {
    last_sample: Option<Instant>,
    network: Option<(Instant, Counters)>,
    history: VecDeque<Sample>,
}

#[derive(Clone, Debug, Serialize)]
struct Capacity {
    total_bytes: u64,
    used_bytes: u64,
    available_bytes: u64,
    used_percent: f64,
}

impl Capacity {
    fn new(total: u64, free: u64, available: u64) -> Option<Self> {
        if total == 0 || free > total || available > free { return None; }
        Some(Self {
            total_bytes: total, used_bytes: total - free, available_bytes: available,
            used_percent: (total - free) as f64 / total as f64 * 100.0,
        })
    }
}

type Counters = BTreeMap<String, (u64, u64)>;

#[derive(Clone, Serialize)]
struct Network {
    interfaces: Vec<String>,
    receive_bytes_per_second: Option<f64>,
    transmit_bytes_per_second: Option<f64>,
}

#[derive(Clone, Serialize)]
struct Activity {
    online_users: usize,
    active_runs: u64,
    queued_runs: u64,
    active_run_limit: usize,
    today_turns: u64,
    today_completed_turns: u64,
}

#[derive(Clone, Serialize)]
struct Sample {
    sampled_at: i64,
    service_uptime_seconds: u64,
    memory: Option<Capacity>,
    disk: Option<Capacity>,
    network: Option<Network>,
    activity: Activity,
    issues: Vec<String>,
}

impl ServerMonitor {
    pub(crate) fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(), started: Instant::now(),
            presence: Mutex::new(BTreeMap::new()), samples: Mutex::new(Samples::default()),
        }
    }

    pub(crate) fn heartbeat(&self, principal: &Principal) {
        self.heartbeat_at(principal, Instant::now());
    }

    fn heartbeat_at(&self, principal: &Principal, instant: Instant) {
        let mut presence = self.presence.lock().unwrap();
        presence.retain(|_, seen| instant.duration_since(*seen) < ONLINE_WINDOW);
        // Reuse the authenticated session identity; do not retain a credential or add another digest.
        presence.insert(principal.session_id().to_vec(), instant);
    }

    fn online_users(&self, db: &Connection, now: i64, instant: Instant) -> Result<usize, ToolError> {
        let mut presence = self.presence.lock().unwrap();
        presence.retain(|_, seen| instant.duration_since(*seen) < ONLINE_WINDOW);
        if presence.is_empty() { return Ok(0); }
        // Current session/account state excludes logout, revocation and disabled accounts.
        let mut query = db.prepare("SELECT s.token_digest,s.owner_user_id FROM auth_sessions s JOIN users u ON u.user_id=s.owner_user_id WHERE s.expires_at>? AND u.disabled=0 AND u.auth_epoch=s.auth_epoch").map_err(api_storage)?;
        let sessions = query.query_map([now], |r| Ok((r.get::<_, Vec<u8>>(0)?, r.get::<_, String>(1)?))).map_err(api_storage)?;
        let mut users = BTreeSet::new();
        for session in sessions {
            let (session, owner) = session.map_err(api_storage)?;
            if presence.contains_key(&session) { users.insert(owner); }
        }
        Ok(users.len())
    }

    pub(crate) fn snapshot(&self, admin: &AdminStore, active_limit: usize, now: i64) -> Result<Value, ToolError> {
        self.snapshot_at(admin, active_limit, now, Instant::now())
    }

    fn snapshot_at(&self, admin: &AdminStore, active_limit: usize, now: i64, instant: Instant) -> Result<Value, ToolError> {
        let mut state = self.samples.lock().unwrap();
        if state.last_sample.is_none_or(|last| instant.duration_since(last) >= INTERVAL) {
            // Read the OS outside the shared control database lock.
            let (memory, disk, counters, issues) = host_sample(&self.root);
            let network = counters.as_ref().map(|current| network_sample(state.network.as_ref(), current, instant));
            let activity = {
                let mut control = admin.control.lock().unwrap();
                let tx = control.connection.transaction().map_err(api_storage)?;
                let (active_runs, queued_runs) = tx.query_row(
                    "SELECT count(CASE WHEN dispatch_state='claimed' THEN 1 END),count(CASE WHEN dispatch_state IN ('preparing','queued') THEN 1 END) FROM run_admissions WHERE dispatch_state IN ('preparing','queued','claimed') AND unsaved=0",
                    [], |r| Ok((r.get(0)?, r.get(1)?)),
                ).map_err(api_storage)?;
                let today_start = (now + 28800).div_euclid(86400) * 86400 - 28800;
                let (today_turns, today_completed_turns) = tx.query_row(
                    "SELECT count(CASE WHEN kind='question' THEN 1 END),count(CASE WHEN kind='completed' THEN 1 END) FROM admin_usage_events WHERE occurred_at>=? AND occurred_at<?",
                    [today_start, today_start + 86400], |r| Ok((r.get(0)?, r.get(1)?)),
                ).map_err(api_storage)?;
                Activity { online_users: self.online_users(&tx, now, instant)?, active_runs, queued_runs,
                    active_run_limit: active_limit, today_turns, today_completed_turns }
            };
            state.network = counters.map(|counters| (instant, counters));
            state.history.retain(|sample| sample.sampled_at > now - HISTORY_SECONDS);
            state.history.push_back(Sample {
                sampled_at: now, service_uptime_seconds: instant.duration_since(self.started).as_secs(),
                memory, disk, network, activity, issues,
            });
            while state.history.len() > 120 { state.history.pop_front(); }
            state.last_sample = Some(instant);
        }
        Ok(json!({
            "current": state.history.back(), "history": state.history,
            "interval_seconds": INTERVAL.as_secs(), "history_seconds": HISTORY_SECONDS,
            "online_window_seconds": ONLINE_WINDOW.as_secs(), "timezone": "Asia/Hong_Kong",
            "platform": std::env::consts::OS,
        }))
    }
}

fn network_sample(previous: Option<&(Instant, Counters)>, current: &Counters, instant: Instant) -> Network {
    let rates = previous.and_then(|(last, old)| {
        let elapsed = instant.duration_since(*last).as_secs_f64();
        // A new NIC, reset counter or a closed dashboard begins a fresh interval.
        if elapsed <= 0.0 || elapsed > 15.0 || !old.keys().eq(current.keys()) { return None; }
        let mut received = 0u64;
        let mut sent = 0u64;
        for (name, (rx, tx)) in current {
            let (old_rx, old_tx) = old.get(name)?;
            received = received.checked_add(rx.checked_sub(*old_rx)?)?;
            sent = sent.checked_add(tx.checked_sub(*old_tx)?)?;
        }
        Some((received as f64 / elapsed, sent as f64 / elapsed))
    });
    Network {
        interfaces: current.keys().cloned().collect(),
        receive_bytes_per_second: rates.map(|value| value.0),
        transmit_bytes_per_second: rates.map(|value| value.1),
    }
}

#[cfg(any(target_os = "linux", test))]
fn parse_memory(text: &str) -> Option<Capacity> {
    let field = |name| text.lines().find_map(|line| {
        let (key, rest) = line.split_once(':')?;
        (key == name).then(|| rest.split_whitespace().next()?.parse::<u64>().ok()?.checked_mul(1024)).flatten()
    });
    // Available includes reclaimable cache; MemFree alone overstates pressure.
    let available = field("MemAvailable")?;
    Capacity::new(field("MemTotal")?, available, available)
}

#[cfg(any(target_os = "linux", test))]
fn default_interfaces(ipv4: &str, ipv6: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for line in ipv4.lines().skip(1) {
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.len() >= 8 && fields[1] == "00000000" && fields[7] == "00000000"
            && u32::from_str_radix(fields[3], 16).is_ok_and(|flags| flags & 1 != 0) && fields[0] != "lo" {
            names.insert(fields[0].to_owned());
        }
    }
    for line in ipv6.lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.len() == 10 && fields[0] == "00000000000000000000000000000000" && fields[1] == "00"
            && u32::from_str_radix(fields[8], 16).is_ok_and(|flags| flags & 1 != 0 && flags & 0x200 == 0) && fields[9] != "lo" {
            names.insert(fields[9].to_owned());
        }
    }
    names
}

#[cfg(any(target_os = "linux", test))]
fn parse_network(text: &str, interfaces: &BTreeSet<String>) -> Option<Counters> {
    if interfaces.is_empty() { return None; }
    let mut counters = Counters::new();
    for line in text.lines() {
        let Some((name, rest)) = line.split_once(':') else { continue; };
        let name = name.trim();
        if !interfaces.contains(name) { continue; }
        let fields: Vec<_> = rest.split_whitespace().collect();
        counters.insert(name.to_owned(), (fields.first()?.parse().ok()?, fields.get(8)?.parse().ok()?));
    }
    (counters.len() == interfaces.len()).then_some(counters)
}

#[cfg(target_os = "linux")]
fn disk_capacity(root: &Path) -> Option<Capacity> {
    use std::{ffi::CString, mem::MaybeUninit, os::unix::ffi::OsStrExt};
    let path = CString::new(root.as_os_str().as_bytes()).ok()?;
    let mut value = MaybeUninit::<libc::statvfs>::uninit();
    // SAFETY: path is NUL terminated; statvfs initializes value on success.
    let value = unsafe {
        if libc::statvfs(path.as_ptr(), value.as_mut_ptr()) != 0 { return None; }
        value.assume_init()
    };
    Capacity::new(
        value.f_blocks.checked_mul(value.f_frsize)?,
        value.f_bfree.checked_mul(value.f_frsize)?,
        value.f_bavail.checked_mul(value.f_frsize)?,
    )
}

type HostSample = (Option<Capacity>, Option<Capacity>, Option<Counters>, Vec<String>);

#[cfg(target_os = "linux")]
fn host_sample(root: &Path) -> HostSample {
    let memory = std::fs::read_to_string("/proc/meminfo").ok().and_then(|text| parse_memory(&text));
    let disk = disk_capacity(root);
    let interfaces = default_interfaces(
        &std::fs::read_to_string("/proc/net/route").unwrap_or_default(),
        &std::fs::read_to_string("/proc/net/ipv6_route").unwrap_or_default(),
    );
    let network = std::fs::read_to_string("/proc/net/dev").ok().and_then(|text| parse_network(&text, &interfaces));
    let mut issues = vec![];
    if memory.is_none() { issues.push("暂时无法读取服务器内存。".into()); }
    if disk.is_none() { issues.push("暂时无法读取服务数据所在磁盘。".into()); }
    if network.is_none() { issues.push("暂时无法读取默认路由网卡流量。".into()); }
    (memory, disk, network, issues)
}

#[cfg(not(target_os = "linux"))]
fn host_sample(_root: &Path) -> HostSample {
    (None, None, None, vec!["系统资源采集在 Linux 服务上可用；当前环境仅显示在线与对话统计。".into()])
}

#[cfg(test)]
#[path = "server_monitor_tests.rs"]
mod tests;
