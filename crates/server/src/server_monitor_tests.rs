use super::*;
use crate::{auth::{self, AuthService}, control_store::{ControlStore, ServiceWriter}};

#[test]
fn monitor_memory_uses_available_and_disk_preserves_reserved_space() {
    let memory = parse_memory("MemTotal: 8000 kB\nMemFree: 100 kB\nMemAvailable: 3000 kB\n").unwrap();
    assert_eq!(memory.used_bytes, 5000 * 1024);
    assert_eq!(memory.available_bytes, 3000 * 1024);
    assert_eq!(memory.used_percent, 62.5);
    assert!(parse_memory("MemTotal: 8000 kB\nMemFree: 100 kB\n").is_none());
    let disk = Capacity::new(1000, 200, 150).unwrap();
    assert_eq!(disk.used_bytes, 800);
    assert_eq!(disk.available_bytes, 150);
}

#[test]
fn monitor_network_counts_default_interfaces_once_and_uses_elapsed_time() {
    let interfaces = default_interfaces(
        "Iface Destination Gateway Flags RefCnt Use Metric Mask\neth0 00000000 0100000A 0003 0 0 100 00000000\neth0 00000000 0100000A 0003 0 0 200 00000000\nveth0 0000000A 00000000 0001 0 0 0 00FFFFFF\n",
        "00000000000000000000000000000000 00 00000000000000000000000000000000 00 fe800000000000000000000000000001 00000400 00000000 00000000 00000003 eth0\n",
    );
    assert_eq!(interfaces, BTreeSet::from(["eth0".to_owned()]));
    let before = parse_network("lo: 999 0 0 0 0 0 0 0 999\nveth0: 777 0 0 0 0 0 0 0 777\neth0: 1000 0 0 0 0 0 0 0 2000", &interfaces).unwrap();
    let after = BTreeMap::from([("eth0".into(), (5000, 4000))]);
    let instant = Instant::now();
    let previous = (instant, before);
    let sample = network_sample(Some(&previous), &after, instant + Duration::from_secs(8));
    assert_eq!(sample.receive_bytes_per_second, Some(500.0));
    assert_eq!(sample.transmit_bytes_per_second, Some(250.0));
    assert_eq!(sample.interfaces, ["eth0"]);
    assert!(network_sample(None, &after, instant).receive_bytes_per_second.is_none());
    assert!(network_sample(Some(&previous), &after, instant + Duration::from_secs(30)).receive_bytes_per_second.is_none());
    let reset = BTreeMap::from([("eth0".into(), (5, 4))]);
    assert!(network_sample(Some(&previous), &reset, instant + INTERVAL).receive_bytes_per_second.is_none());
    let replaced = BTreeMap::from([("eth1".into(), (5000, 4000))]);
    assert!(network_sample(Some(&previous), &replaced, instant + INTERVAL).receive_bytes_per_second.is_none());
    assert!(parse_network("eth1: 1 0 0 0 0 0 0 0 2", &interfaces).is_none());
}

#[test]
fn monitor_presence_deduplicates_sessions_and_expires_or_revokes_them() {
    let root = tempfile::tempdir().unwrap();
    let writer = ServiceWriter::acquire(root.path()).unwrap();
    let control = ControlStore::open(writer.clone()).unwrap();
    control.connection.execute_batch("INSERT INTO users(user_id) VALUES('A'),('B');").unwrap();
    let auth = AuthService::new(ControlStore::open(writer).unwrap());
    let now = crate::multi_user_host::now();
    let token = |owner| auth::write_session(&control.connection, owner, 0, None, now).unwrap();
    let a1 = auth.authenticate(&token("A"), now).unwrap();
    let a2 = auth.authenticate(&token("A"), now).unwrap();
    let b = auth.authenticate(&token("B"), now).unwrap();
    let monitor = ServerMonitor::new(root.path());
    let instant = Instant::now();
    for principal in [&a1, &a1, &a2, &b] { monitor.heartbeat_at(principal, instant); }
    assert_eq!(monitor.online_users(&control.connection, now, instant).unwrap(), 2);
    auth.logout(&a1).unwrap();
    assert_eq!(monitor.online_users(&control.connection, now, instant).unwrap(), 2);
    auth.logout(&a2).unwrap();
    assert_eq!(monitor.online_users(&control.connection, now, instant).unwrap(), 1);
    control.connection.execute("UPDATE users SET disabled=1 WHERE user_id='B'", []).unwrap();
    assert_eq!(monitor.online_users(&control.connection, now, instant).unwrap(), 0);
    control.connection.execute("UPDATE users SET disabled=0 WHERE user_id='B'", []).unwrap();
    assert_eq!(monitor.online_users(&control.connection, now + 90, instant + ONLINE_WINDOW).unwrap(), 0);
    assert!(monitor.presence.lock().unwrap().is_empty());
}

#[test]
fn monitor_shared_cache_bounds_history_and_uses_hong_kong_midnight() {
    let root = tempfile::tempdir().unwrap();
    let writer = ServiceWriter::acquire(root.path()).unwrap();
    let control = ControlStore::open(writer).unwrap();
    control.connection.execute("INSERT INTO users(user_id) VALUES('A')", []).unwrap();
    let now: i64 = 1_792_886_400; // Calculate a deterministic Hong Kong day.
    let today = (now + 28800).div_euclid(86400) * 86400 - 28800;
    for (kind, reference, at) in [("question", "yesterday", today-1), ("question", "today", today), ("completed", "old-run", today), ("question", "tomorrow", today+86400)] {
        crate::admin_usage::record(&control.connection, "A", kind, reference, at).unwrap();
    }
    let admin = AdminStore::new(control);
    let monitor = ServerMonitor::new(root.path());
    let instant = Instant::now();
    let first = monitor.snapshot_at(&admin, 12, now, instant).unwrap();
    assert_eq!(first["current"]["activity"]["today_turns"], 1);
    assert_eq!(first["current"]["activity"]["today_completed_turns"], 1);
    assert_eq!(first, monitor.snapshot_at(&admin, 12, now+1, instant+Duration::from_secs(1)).unwrap());
    for i in 1..130 {
        monitor.snapshot_at(&admin, 12, now+i*5, instant+Duration::from_secs(i as u64*5)).unwrap();
    }
    let samples = monitor.samples.lock().unwrap();
    assert_eq!(samples.history.len(), 120);
    assert_eq!(samples.history.front().unwrap().sampled_at, now+50);
    drop(samples);
    let resumed = monitor.snapshot_at(&admin, 12, now+1800, instant+Duration::from_secs(1800)).unwrap();
    assert_eq!(resumed["history"].as_array().unwrap().len(), 1);
}

#[cfg(target_os = "linux")]
#[test]
fn monitor_linux_reads_real_host_resources() {
    let root = tempfile::tempdir().unwrap();
    let (memory, disk, network, issues) = host_sample(root.path());
    assert!(issues.is_empty(), "{issues:?}");
    assert!(memory.unwrap().total_bytes > 0);
    assert!(disk.unwrap().total_bytes > 0);
    assert!(!network.unwrap().is_empty());
}
