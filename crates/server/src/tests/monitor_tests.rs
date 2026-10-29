use super::mu5_tests::Fixture;
use crate::multi_user_host::{self, Headers, Site};
use serde_json::json;

#[test]
fn monitor_routes_require_admin_and_authenticated_csrf_heartbeat_without_loading_reader() {
    let mut f = Fixture::new();
    f.control.set_reader_admin("B", true).unwrap();
    assert_eq!(f.call(&f.a, "GET", "/api/admin/monitor", json!({})).0, 403);
    assert_eq!(f.call("", "GET", "/api/admin/monitor", json!({})).0, 401);
    let missing_csrf = multi_user_host::dispatch(&f.access, &Site::new("https://reader.example").unwrap(),
        "POST", "/api/account/presence", &Headers(vec![
            ("Host".into(), "reader.example".into()), ("Origin".into(), "https://reader.example".into()),
            ("Cookie".into(), format!("{}={}", crate::auth::COOKIE, f.a)),
        ]), "{}", multi_user_host::now());
    assert_eq!(missing_csrf.status, 403);
    assert_eq!(f.call(&f.a, "GET", "/api/account/presence", json!({})).0, 405);
    f.ok(&f.a, "POST", "/api/account/presence", json!({}));
    f.ok(&f.a, "POST", "/api/account/presence", json!({}));
    let report = f.ok(&f.b, "GET", "/api/admin/monitor", json!({}));
    assert_eq!(report["current"]["activity"]["online_users"], 1);
    assert_eq!(report["current"]["activity"]["active_runs"], 0);
    assert_eq!(report["current"]["activity"]["queued_runs"], 0);
    assert_eq!(report["interval_seconds"], 5);
    assert_eq!(report["history"].as_array().unwrap().len(), 1);
    assert_eq!(f.control.connection.query_row("SELECT count(*) FROM reader_workspaces", [], |row| row.get::<_, u64>(0)).unwrap(), 0);
    assert_eq!(f.call(&f.b, "GET", "/api/admin/monitor?unknown=1", json!({})).0, 400);
}

#[test]
fn monitor_counts_running_and_waiting_turns_without_counting_unsaved_or_finished_work() {
    let mut f = Fixture::new();
    f.control.set_reader_admin("B", true).unwrap();
    let workspace = f.create(&f.a, &f.x, "monitor-test");
    let id = workspace["workspace_id"].as_str().unwrap();
    for (index, state, unsaved) in [(0,"preparing",0),(1,"queued",0),(2,"claimed",0),(3,"claimed",1),(4,"settled",0),(5,"admission_failed",0)] {
        f.control.connection.execute("INSERT INTO run_admissions(owner_user_id,client_request_id,turn_id,chat_session_id,workspace_id,workspace_generation,book_id,publication_id,dispatch_state,unsaved) VALUES('A',?1,?1,'test-chat',?2,1,?3,?4,?5,?6)",
            rusqlite::params![format!("run-{index}"), id, f.x.book_id, f.x.publication_id, state, unsaved]).unwrap();
    }
    let report = f.ok(&f.b, "GET", "/api/admin/monitor", json!({}));
    assert_eq!(report["current"]["activity"]["active_runs"], 1);
    assert_eq!(report["current"]["activity"]["queued_runs"], 2);
}
