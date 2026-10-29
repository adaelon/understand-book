use super::mu5_tests::Fixture;
use crate::{auth, multi_user_host::{self, Headers, HttpReply, Site}};
use rusqlite::params;
use serde_json::json;

fn setup() -> (Fixture, String) {
    let mut f = Fixture::new();
    f.control.set_reader_admin("B", true).unwrap();
    let batch = f.ok(&f.b, "POST", "/api/admin/invite-batches", json!({"operation_id":"guest-fixture","count":1}));
    (f, batch["invites"][0]["code"].as_str().unwrap().to_owned())
}
fn redeem(f: &Fixture, code: &str, previous: Option<&str>) -> HttpReply {
    let mut headers = vec![("Host".into(), "reader.example".into()), ("Origin".into(), "https://reader.example".into())];
    if let Some(token) = previous {
        headers.push(("Cookie".into(), format!("{}={token}", auth::COOKIE)));
        headers.push(("X-CSRF-Token".into(), auth::csrf(token)));
    }
    multi_user_host::dispatch(&f.access, &Site::new("https://reader.example").unwrap(), "POST", "/api/auth/guest", &Headers(headers),
        &json!({"invite_code":code}).to_string(), multi_user_host::now())
}

#[test]
fn guest_trial_entry_is_removed_without_consuming_invites_or_creating_accounts() {
    let (f, code) = setup();
    for (token, expected) in [(None, 401), (Some(f.a.as_str()), 403)] {
        let reply = redeem(&f, &code, token);
        assert_eq!(reply.status, expected);
        assert!(reply.cookie.is_none());
    }
    assert_eq!(f.control.connection.query_row("SELECT state FROM beta_invites WHERE code=?", [&code], |r| r.get::<_, String>(0)).unwrap(), "unused");
    assert_eq!(f.control.connection.query_row("SELECT count(*) FROM users", [], |r| r.get::<_, i64>(0)).unwrap(), 2);
    assert_eq!(f.control.connection.query_row("SELECT count(*) FROM auth_sessions", [], |r| r.get::<_, i64>(0)).unwrap(), 2);
    assert_eq!(f.control.connection.query_row("SELECT count(*) FROM allowance_periods", [], |r| r.get::<_, i64>(0)).unwrap(), 0);
    assert_eq!(f.ok(&f.a, "GET", "/api/auth/me", json!({}))["is_guest"], false);
}

#[test]
fn guest_trial_existing_session_and_reader_remain_available_after_entry_removal() {
    let (f, code) = setup();
    let now = multi_user_host::now();
    let owner = "guest_existing";
    // Persist an identity and session issued before the guest entry was removed.
    f.control.connection.execute("INSERT INTO users(user_id) VALUES(?)", [owner]).unwrap();
    f.control.connection.execute("UPDATE beta_invites SET state='used',used_by=?,used_at=? WHERE code=?", params![owner, now, code]).unwrap();
    f.access.library.lock().unwrap().grant(owner, &f.x).unwrap();
    let token = auth::write_session(&f.control.connection, owner, 0, None, now).unwrap();
    let workspace = f.create(&token, &f.x, "existing-guest");
    let workspace = f.action(&token, &workspace, "existing-guest", "reader/note", json!({"lid":"1.1","text":"existing guest note"}));

    let denied = redeem(&f, &code, Some(&token));
    assert_eq!(denied.status, 403);
    assert_eq!(serde_json::from_slice::<serde_json::Value>(&denied.body).unwrap()["error_code"], "CAPABILITY_FORBIDDEN");
    let me = f.ok(&token, "GET", "/api/auth/me", json!({}));
    assert_eq!(me["user_id"], owner);
    assert_eq!(me["is_guest"], true);
    assert_eq!(f.ok(&token, "GET", "/api/library", json!({}))["books"].as_array().unwrap().len(), 1);
    assert_eq!(f.get(&token, &workspace)["revision"], workspace["revision"]);
    assert_eq!(f.access.auth.authenticate(&token, now + 24 * 60 * 60).unwrap().user_id(), owner);
}
