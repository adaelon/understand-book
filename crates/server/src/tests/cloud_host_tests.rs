use super::mu5_tests::Fixture;
use crate::{
    control_store::{ControlStore, ServiceWriter},
    multi_user_host::{dispatch, Headers, Site},
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

#[test]
fn cloud_host_desktop_exchange_uses_independent_same_account_session() {
    let f = Fixture::new();
    let now = crate::multi_user_host::now();
    let site = Site::new("https://reader.example").unwrap();
    let verifier = "a".repeat(43);
    let redirect_uri = "http://127.0.0.1:49521/callback";
    let input = json!({
        "redirect_uri":redirect_uri,
        "code_challenge":URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes())),
        "state":"b".repeat(43),
    });
    let unauthenticated = Headers(vec![
        ("Host".into(), "reader.example".into()),
        ("Origin".into(), "https://reader.example".into()),
    ]);
    assert_eq!(dispatch(&f.access, &site, "POST", "/api/auth/desktop/authorize",
        &unauthenticated, &input.to_string(), now).status, 401);
    let no_csrf = Headers(vec![
        ("Host".into(), "reader.example".into()),
        ("Origin".into(), "https://reader.example".into()),
        ("Cookie".into(), format!("{}={}", crate::auth::COOKIE, f.a)),
    ]);
    assert_eq!(dispatch(&f.access, &site, "POST", "/api/auth/desktop/authorize",
        &no_csrf, &input.to_string(), now).status, 403);
    let authorized = f.ok(&f.a, "POST", "/api/auth/desktop/authorize", input);
    let redirect = url::Url::parse(authorized["redirect_url"].as_str().unwrap()).unwrap();
    let code = redirect.query_pairs().find(|(key, _)| key == "code").unwrap().1.into_owned();
    assert_eq!(redirect.query_pairs().find(|(key, _)| key == "state").unwrap().1, "b".repeat(43));
    let exchange = json!({"code":code,"code_verifier":verifier,"redirect_uri":redirect_uri}).to_string();
    let reply = dispatch(&f.access, &site, "POST", "/api/auth/desktop/exchange",
        &unauthenticated, &exchange, now);
    assert_eq!(reply.status, 200, "{}", String::from_utf8_lossy(&reply.body));
    assert!(reply.cookie.is_none(), "the native client owns its independent WebView cookie");
    let session: Value = serde_json::from_slice(&reply.body).unwrap();
    let desktop = session["session_token"].as_str().unwrap();
    assert_ne!(desktop, f.a);
    assert_eq!(f.ok(desktop, "GET", "/api/auth/me", Value::Null)["user_id"], "A");
    let source = format!("{}?lid=1.1", f.x.url("text"));
    assert_eq!(f.ok(desktop, "GET", &source, Value::Null),
        f.ok(&f.a, "GET", &source, Value::Null));
    assert_eq!(dispatch(&f.access, &site, "POST", "/api/auth/desktop/exchange",
        &unauthenticated, &exchange, now).status, 400);
    f.ok(desktop, "POST", "/api/auth/logout", json!({}));
    assert_eq!(f.call(desktop, "GET", "/api/auth/me", Value::Null).0, 401);
    assert_eq!(f.ok(&f.a, "GET", "/api/auth/me", Value::Null)["user_id"], "A");
    let other_origin = Headers(vec![
        ("Host".into(), "reader.example".into()),
        ("Origin".into(), "https://other.example".into()),
    ]);
    assert_eq!(dispatch(&f.access, &site, "POST", "/api/auth/desktop/exchange",
        &other_origin, &exchange, now).status, 403);
}

#[test]
fn cloud_schema_18_upgrade_preserves_account_and_reader_checkpoint() {
    let root = tempfile::tempdir().unwrap();
    let writer = ServiceWriter::acquire(root.path()).unwrap();
    {
        let db = rusqlite::Connection::open(root.path().join("control.sqlite")).unwrap();
        // These are the actual schema-18 creation scripts, before commercial
        // tables were introduced. Reopening exercises the production upgrade.
        for schema in [
            include_str!("../control_schema.sql"),
            include_str!("../admin_schema.sql"),
            include_str!("../receipt_corrections.sql"),
            include_str!("../charge_settlement.sql"),
            include_str!("../usage_schema.sql"),
            include_str!("../account_registration.sql"),
            include_str!("../mcp_access.sql"),
            include_str!("../mcp_usage.sql"),
            include_str!("../mcp_packs.sql"),
            include_str!("../default_book_grants.sql"),
            include_str!("../publication_operations.sql"),
            include_str!("../learning_workspaces.sql"),
            include_str!("../allowance_basis.sql"),
            include_str!("../agent_cost_ownership.sql"),
            include_str!("../external_build_costs.sql"),
            include_str!("../allowance_offers.sql"),
            include_str!("../workspace_publication.sql"),
            include_str!("../workspace_release_instances.sql"),
            include_str!("../operator_builds.sql"),
        ] { db.execute_batch(schema).unwrap(); }
        db.execute_batch("INSERT INTO users(user_id) VALUES('existing-student');
            INSERT INTO reader_workspaces(owner_user_id,workspace_id,selected_chat,revision,checkpoint,checkpoint_seq)
            VALUES('existing-student','reading','saved-chat',7,'{\"selection\":\"1.2\"}',9);").unwrap();
        db.pragma_update(None, "application_id", 0x55424d55).unwrap();
        db.pragma_update(None, "user_version", 18).unwrap();
    }
    let saved = root.path().join("saved-private-record.json");
    std::fs::write(&saved, "{\"answer\":\"原有作答与目标\"}").unwrap();
    for _ in 0..2 {
        let control = ControlStore::open(writer.clone()).unwrap();
        let version: i64 = control.connection.pragma_query_value(None, "user_version", |row| row.get(0)).unwrap();
        assert_eq!(version, crate::control_store::CONTROL_SCHEMA_VERSION);
        let record: (String, i64, String, i64) = control.connection.query_row(
            "SELECT selected_chat,revision,checkpoint,checkpoint_seq FROM reader_workspaces WHERE owner_user_id='existing-student'",
            [], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).unwrap();
        assert_eq!(record, ("saved-chat".into(),7,"{\"selection\":\"1.2\"}".into(),9));
        assert!(crate::cloud_plans::current(&control.connection, "existing-student", 100).unwrap().is_none());
        let charge_count: i64 = control.connection.query_row("SELECT count(*) FROM billable_usage", [], |row| row.get(0)).unwrap();
        assert_eq!(charge_count, 0);
    }
    assert_eq!(std::fs::read_to_string(saved).unwrap(), "{\"answer\":\"原有作答与目标\"}");
}
