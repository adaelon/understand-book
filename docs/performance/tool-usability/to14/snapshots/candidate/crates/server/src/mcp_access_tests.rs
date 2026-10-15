use super::*;
use crate::{
    auth::{self, AuthService},
    authorization::Authorization,
    control_store::ServiceWriter,
    multi_user_host::{self, Headers, Site},
    user_registry::UserRegistry,
};
use serde_json::{json, Value};
use std::sync::Arc;

const PASSWORD: &str = "fixture-only-password";
const NOW: i64 = 1_790_000_000;

struct Fixture {
    mcp: McpAccess,
    auth: AuthService,
    control: ControlStore,
    root: tempfile::TempDir,
}
impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let writer = ServiceWriter::acquire(root.path()).unwrap();
        let mut control = ControlStore::open(writer.clone()).unwrap();
        for user in ["alice", "bob"] {
            control.provision_password(user, PASSWORD, true).unwrap();
        }
        Self {
            mcp: McpAccess::new(ControlStore::open(writer.clone()).unwrap()),
            auth: AuthService::new(ControlStore::open(writer).unwrap()),
            control,
            root,
        }
    }
    fn login(&self, user: &str) -> (String, Principal) {
        let token = self.auth.login(user, PASSWORD, None, NOW).unwrap();
        let principal = self.auth.authenticate(&token, NOW).unwrap();
        (token, principal)
    }
}
fn bearer(token: &str) -> String {
    format!("Bearer {token}")
}
fn assert_denied(mcp: &McpAccess, token: &str) {
    assert_eq!(
        mcp.authenticate(Some(&bearer(token)))
            .unwrap_err()
            .error_code,
        "MCP_AUTH_REQUIRED"
    );
}

#[test]
fn mcp_access_owner_isolation_one_time_secret_and_reopen() {
    let f = Fixture::new();
    let (_, alice) = f.login("alice");
    let (_, bob) = f.login("bob");
    let a = f.mcp.create(&alice, "  我的 Agent  ", NOW).unwrap();
    let b = f.mcp.create(&bob, "Bob Agent", NOW).unwrap();
    assert_ne!(a.token, b.token);
    let identity = f.mcp.authenticate(Some(&bearer(&a.token))).unwrap();
    assert_eq!(identity.user_id(), "alice");
    assert_eq!(identity.credential_id(), a.credential.credential_id);
    let list = f.mcp.list(&alice, NOW).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].label, "我的 Agent");
    assert!(list[0].active);
    let projection = serde_json::to_value(&list).unwrap();
    assert_eq!(projection[0].as_object().unwrap().len(), 5);
    assert!(!projection.to_string().contains(&a.token));
    let saved: Vec<u8> = f
        .control
        .connection
        .query_row(
            "SELECT token_digest FROM mcp_credentials WHERE credential_id=?",
            [&a.credential.credential_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(saved, digest(&a.token));
    assert_ne!(saved, a.token.as_bytes());
    assert_eq!(
        f.mcp
            .revoke(&bob, &a.credential.credential_id, NOW + 1)
            .unwrap_err()
            .error_code,
        "OBJECT_NOT_FOUND"
    );
    assert_eq!(
        f.mcp
            .revoke(&bob, "missing", NOW + 1)
            .unwrap_err()
            .error_code,
        "OBJECT_NOT_FOUND"
    );
    f.mcp
        .revoke(&alice, &a.credential.credential_id, NOW + 2)
        .unwrap();
    f.mcp
        .revoke(&alice, &a.credential.credential_id, NOW + 3)
        .unwrap();
    assert_denied(&f.mcp, &a.token);
    let row = f.mcp.list(&alice, NOW + 3).unwrap().remove(0);
    assert!(!row.active);
    assert_eq!(row.revoked_at, Some(NOW + 2));
    // Web logout and session expiry are independent of the remote credential.
    f.auth.logout(&bob).unwrap();
    assert_eq!(
        f.mcp
            .authenticate(Some(&bearer(&b.token)))
            .unwrap()
            .user_id(),
        "bob"
    );
    let Fixture {
        mcp,
        auth,
        control,
        root,
    } = f;
    drop((mcp, auth, control));
    let reopened =
        McpAccess::new(ControlStore::open(ServiceWriter::acquire(root.path()).unwrap()).unwrap());
    assert_denied(&reopened, &a.token);
    assert_eq!(
        reopened
            .authenticate(Some(&bearer(&b.token)))
            .unwrap()
            .user_id(),
        "bob"
    );
}

#[test]
fn mcp_access_account_revocations_never_revive_old_credentials() {
    for action in ["disable", "password", "revoke_all"] {
        let mut f = Fixture::new();
        let (_, alice) = f.login("alice");
        let (_, bob) = f.login("bob");
        let a = f.mcp.create(&alice, "Alice", NOW).unwrap();
        let b = f.mcp.create(&bob, "Bob", NOW).unwrap();
        match action {
            "disable" => {
                f.control.set_user_disabled("alice", true).unwrap();
                assert_denied(&f.mcp, &a.token);
                f.control.set_user_disabled("alice", false).unwrap();
            }
            "password" => f
                .control
                .provision_password("alice", PASSWORD, false)
                .unwrap(),
            _ => f.control.revoke_user_sessions("alice").unwrap(),
        }
        assert_denied(&f.mcp, &a.token);
        assert_eq!(
            f.mcp.create(&alice, "stale", NOW).err().unwrap().error_code,
            "AUTH_REQUIRED"
        );
        assert_eq!(
            f.mcp.list(&alice, NOW).unwrap_err().error_code,
            "AUTH_REQUIRED"
        );
        assert_eq!(
            f.mcp
                .revoke(&alice, &a.credential.credential_id, NOW)
                .unwrap_err()
                .error_code,
            "AUTH_REQUIRED"
        );
        assert_eq!(
            f.mcp
                .authenticate(Some(&bearer(&b.token)))
                .unwrap()
                .user_id(),
            "bob"
        );
        let (_, renewed) = f.login("alice");
        assert!(!f.mcp.list(&renewed, NOW).unwrap()[0].active);
        let fresh = f.mcp.create(&renewed, "replacement", NOW + 1).unwrap();
        let Fixture {
            mcp,
            auth,
            control,
            root,
        } = f;
        drop((mcp, auth, control));
        let reopened = McpAccess::new(
            ControlStore::open(ServiceWriter::acquire(root.path()).unwrap()).unwrap(),
        );
        assert_denied(&reopened, &a.token);
        assert_eq!(
            reopened
                .authenticate(Some(&bearer(&fresh.token)))
                .unwrap()
                .user_id(),
            "alice"
        );
    }
}

#[test]
fn mcp_access_rejects_missing_wrong_credentials_and_stale_web_sessions() {
    let f = Fixture::new();
    let (cookie, alice) = f.login("alice");
    let issued = f.mcp.create(&alice, "Agent", NOW).unwrap();
    for header in [
        None,
        Some(""),
        Some("Bearer"),
        Some("Basic value"),
        Some("Bearer ub_mcp_short"),
        Some(cookie.as_str()),
    ] {
        assert_eq!(
            f.mcp.authenticate(header).unwrap_err().error_code,
            "MCP_AUTH_REQUIRED"
        );
    }
    assert_denied(&f.mcp, &cookie);
    assert_denied(&f.mcp, &format!("{TOKEN_PREFIX}{}", "a".repeat(43)));
    assert_denied(&f.mcp, &format!("{} ", issued.token));
    assert_eq!(
        f.mcp
            .authenticate(Some(&format!("bearer {}", issued.token)))
            .unwrap()
            .user_id(),
        "alice"
    );
    assert!(f.auth.authenticate(&issued.token, NOW).is_err());
    for label in [" ".to_string(), "字".repeat(81)] {
        assert_eq!(
            f.mcp.create(&alice, &label, NOW).err().unwrap().error_code,
            "MCP_LABEL_INVALID"
        );
    }
    assert_eq!(f.mcp.list(&alice, NOW).unwrap().len(), 1);
    let expired = NOW + auth::SESSION_SECONDS;
    assert_eq!(
        f.mcp
            .create(&alice, "expired", expired)
            .err()
            .unwrap()
            .error_code,
        "AUTH_REQUIRED"
    );
    assert_eq!(
        f.mcp.list(&alice, expired).unwrap_err().error_code,
        "AUTH_REQUIRED"
    );
    assert!(f.mcp.authenticate(Some(&bearer(&issued.token))).is_ok());
    f.auth.logout(&alice).unwrap();
    assert_eq!(
        f.mcp
            .revoke(&alice, &issued.credential.credential_id, NOW)
            .unwrap_err()
            .error_code,
        "AUTH_REQUIRED"
    );
}

fn headers(token: Option<&str>) -> Headers {
    let mut values = vec![
        ("Host".into(), "reader.example".into()),
        ("Origin".into(), "https://reader.example".into()),
    ];
    if let Some(token) = token {
        values.push(("Cookie".into(), format!("{}={token}", auth::COOKIE)));
        values.push(("X-CSRF-Token".into(), auth::csrf(token)));
    }
    Headers(values)
}

#[test]
fn mcp_access_account_routes_keep_cookie_csrf_and_private_state_boundaries() {
    let root = tempfile::tempdir().unwrap();
    let users = UserRegistry::open(root.path()).unwrap();
    let mut control = ControlStore::open(users.writer()).unwrap();
    for user in ["alice", "bob"] {
        control.provision_password(user, PASSWORD, true).unwrap();
    }
    let access = Arc::new(Authorization::new(users).unwrap());
    let site = Site::new("https://reader.example").unwrap();
    let alice = access.auth.login("alice", PASSWORD, None, NOW).unwrap();
    let bob = access.auth.login("bob", PASSWORD, None, NOW).unwrap();
    let call = |method: &str, path: &str, headers: &Headers, body: &str| {
        multi_user_host::dispatch(&access, &site, method, path, headers, body, NOW)
    };
    let create_path = "/api/account/mcp/credentials";
    let body = r#"{"label":"Agent"}"#;
    assert_eq!(call("POST", create_path, &headers(None), body).status, 401);
    let mut no_csrf = headers(Some(&alice));
    no_csrf.0.retain(|(key, _)| key != "X-CSRF-Token");
    assert_eq!(call("POST", create_path, &no_csrf, body).status, 403);
    let mut wrong_site = headers(Some(&alice));
    wrong_site.0[1].1 = "https://elsewhere.example".into();
    assert_eq!(call("POST", create_path, &wrong_site, body).status, 403);
    wrong_site = headers(Some(&alice));
    wrong_site.0[0].1 = "elsewhere.example".into();
    assert_eq!(call("GET", "/api/account/mcp", &wrong_site, "").status, 403);
    for body in [
        r#"{"label":"Agent","user_id":"bob"}"#,
        r#"{"label":"Agent","extra":true}"#,
        r#"{"label":" "}"#,
    ] {
        assert_eq!(
            call("POST", create_path, &headers(Some(&alice)), body).status,
            400
        );
    }
    assert_eq!(
        call(
            "POST",
            "/api/account/mcp/credentials?user_id=bob",
            &headers(Some(&alice)),
            body
        )
        .status,
        400
    );
    assert_eq!(
        call("GET", create_path, &headers(Some(&alice)), "").status,
        405
    );
    let created = call("POST", create_path, &headers(Some(&alice)), body);
    assert_eq!(created.status, 200);
    assert!(created.cookie.is_none());
    let created: Value = serde_json::from_slice(&created.body).unwrap();
    let token = created["token"].as_str().unwrap();
    let revoke_path = format!(
        "{create_path}/{}/revoke",
        created["credential"]["credential_id"].as_str().unwrap()
    );
    assert_eq!(
        access
            .mcp_access
            .authenticate(Some(&bearer(token)))
            .unwrap()
            .user_id(),
        "alice"
    );
    let listed = call("GET", "/api/account/mcp", &headers(Some(&alice)), "");
    assert_eq!(listed.status, 200);
    let listed: Value = serde_json::from_slice(&listed.body).unwrap();
    assert_eq!(listed["credentials"], json!([created["credential"]]));
    let others = call("GET", "/api/account/mcp", &headers(Some(&bob)), "");
    assert_eq!(
        serde_json::from_slice::<Value>(&others.body).unwrap()["credentials"],
        json!([])
    );
    assert_eq!(
        call("POST", &revoke_path, &headers(Some(&bob)), "{}").status,
        404
    );
    assert_eq!(call("POST", &revoke_path, &no_csrf, "{}").status, 403);
    assert_eq!(
        call("GET", &revoke_path, &headers(Some(&alice)), "").status,
        405
    );
    assert_eq!(
        call("POST", &revoke_path, &headers(Some(&alice)), "{}").status,
        200
    );
    assert_eq!(
        call("POST", &revoke_path, &headers(Some(&alice)), "{}").status,
        200
    );
    assert_denied(&access.mcp_access, token);
    // A fresh MCP token is neither a browser cookie nor a browser Bearer fallback.
    let created = call("POST", create_path, &headers(Some(&alice)), body);
    let created: Value = serde_json::from_slice(&created.body).unwrap();
    let token = created["token"].as_str().unwrap();
    let mut remote = headers(None);
    remote.0.push(("Authorization".into(), bearer(token)));
    for path in [
        "/api/account/mcp",
        "/api/auth/me",
        "/api/library",
        "/api/workspaces",
        "/api/me/chats",
    ] {
        assert_eq!(call("GET", path, &remote, "").status, 401, "{path}");
        assert_eq!(
            call("GET", path, &headers(Some(token)), "").status,
            401,
            "{path}"
        );
    }
    let changed = call(
        "POST",
        "/api/account/password",
        &headers(Some(&alice)),
        &json!({"current_password":PASSWORD,"new_password":"replacement-password"}).to_string(),
    );
    assert_eq!(changed.status, 200);
    assert_denied(&access.mcp_access, token);
    assert_eq!(
        control
            .connection
            .query_row("SELECT count(*) FROM reader_workspaces", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert!(
        !root.path().join("users").exists(),
        "account/MCP calls must not load private state"
    );
}

#[test]
fn mcp_access_real_http_create_list_and_revoke_without_reader_workspace() {
    let root = tempfile::tempdir().unwrap();
    let users = UserRegistry::open(root.path()).unwrap();
    let mut control = ControlStore::open(users.writer()).unwrap();
    control.provision_password("alice", PASSWORD, true).unwrap();
    let access = Arc::new(Authorization::new(users).unwrap());
    let server = multi_user_host::start_with_access(
        "127.0.0.1:0".parse().unwrap(),
        Arc::new(Site::new("https://reader.example").unwrap()),
        access.clone(),
    )
    .unwrap();
    let request = |method: &str, path: &str| {
        ureq::request(method, &format!("{}{path}", server.url))
            .timeout(std::time::Duration::from_secs(10))
            .set("Host", "reader.example")
            .set("Origin", "https://reader.example")
    };
    let login = request("POST", "/api/auth/login")
        .send_json(json!({"username":"alice","password":PASSWORD}))
        .unwrap();
    let cookie = login
        .header("Set-Cookie")
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    let login: Value = login.into_json().unwrap();
    let account = |method: &str, path: &str| {
        request(method, path)
            .set("Cookie", &cookie)
            .set("X-CSRF-Token", login["csrf_token"].as_str().unwrap())
    };
    let created = account("POST", "/api/account/mcp/credentials")
        .send_json(json!({"label":"HTTP Agent"}))
        .unwrap();
    assert_eq!(created.header("Cache-Control"), Some("no-store"));
    assert!(created.header("Set-Cookie").is_none());
    let created: Value = created.into_json().unwrap();
    let token = created["token"].as_str().unwrap();
    assert_eq!(
        access
            .mcp_access
            .authenticate(Some(&bearer(token)))
            .unwrap()
            .user_id(),
        "alice"
    );
    let listed: Value = account("GET", "/api/account/mcp")
        .call()
        .unwrap()
        .into_json()
        .unwrap();
    assert_eq!(listed["credentials"], json!([created["credential"]]));
    assert_eq!(listed["usage"]["free_remaining"], 1000);
    assert_eq!(listed["usage"]["paid_remaining"], 0);
    assert_eq!(listed["usage"]["charged_units"], 0);
    assert!(listed["mcp_url"].as_str().unwrap().ends_with("/mcp"));
    assert!(listed["usage"]["top_up_url"].as_str().unwrap().ends_with("/?account=mcp"));
    let path = format!(
        "/api/account/mcp/credentials/{}/revoke",
        created["credential"]["credential_id"].as_str().unwrap()
    );
    let revoked: Value = account("POST", &path)
        .send_json(json!({}))
        .unwrap()
        .into_json()
        .unwrap();
    assert_eq!(revoked, json!({"revoked":true}));
    assert_denied(&access.mcp_access, token);
    let listed: Value = account("GET", "/api/account/mcp")
        .call()
        .unwrap()
        .into_json()
        .unwrap();
    assert_eq!(listed["credentials"][0]["active"], false);
    assert!(!root.path().join("users").exists());
    server.shutdown();
}

#[test]
fn mcp_access_schema_v9_upgrade_preserves_accounts_and_rolls_back_on_failure() {
    for fail in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let writer = ServiceWriter::acquire(root.path()).unwrap();
        let db = rusqlite::Connection::open(root.path().join("control.sqlite")).unwrap();
        for sql in [
            include_str!("control_schema.sql"),
            include_str!("admin_schema.sql"),
            include_str!("receipt_corrections.sql"),
            include_str!("charge_settlement.sql"),
            include_str!("usage_schema.sql"),
            include_str!("account_registration.sql"),
        ] {
            db.execute_batch(sql).unwrap();
        }
        db.execute_batch("INSERT INTO users(user_id,auth_epoch,password_hash,email,email_verified_at) VALUES('alice',7,'original-hash','alice@example.com',100);
            INSERT INTO auth_sessions VALUES(X'1234','alice',7,200);
            INSERT INTO reader_workspaces(owner_user_id,workspace_id,checkpoint) VALUES('alice','scene','saved');").unwrap();
        db.pragma_update(None, "application_id", 0x55424d55)
            .unwrap();
        db.pragma_update(None, "user_version", 9).unwrap();
        if fail {
            db.execute_batch("CREATE INDEX mcp_credentials_owner ON users(user_id);")
                .unwrap();
        }
        drop(db);
        let opened = ControlStore::open(writer.clone());
        if fail {
            assert_eq!(
                opened.err().unwrap().error_code,
                "CONTROL_STORAGE_UNAVAILABLE"
            );
            let db = rusqlite::Connection::open(root.path().join("control.sqlite")).unwrap();
            assert_eq!(
                db.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
                    .unwrap(),
                9
            );
            assert_eq!(db.query_row("SELECT count(*) FROM sqlite_schema WHERE type='table' AND name='mcp_credentials'",[],|r|r.get::<_,i64>(0)).unwrap(),0);
        } else {
            drop(opened.unwrap());
            let reopened = ControlStore::open(writer).unwrap();
            assert_eq!(
                reopened
                    .connection
                    .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
                    .unwrap(),
                crate::control_store::CONTROL_SCHEMA_VERSION
            );
            let account: (i64,String,String,i64) = reopened.connection.query_row("SELECT auth_epoch,password_hash,email,email_verified_at FROM users WHERE user_id='alice'",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).unwrap();
            assert_eq!(
                account,
                (7, "original-hash".into(), "alice@example.com".into(), 100)
            );
            assert_eq!(
                reopened
                    .connection
                    .query_row("SELECT expires_at FROM auth_sessions", [], |r| r
                        .get::<_, i64>(0))
                    .unwrap(),
                200
            );
            assert_eq!(
                reopened
                    .connection
                    .query_row("SELECT checkpoint FROM reader_workspaces", [], |r| r
                        .get::<_, String>(0))
                    .unwrap(),
                "saved"
            );
            assert_eq!(
                reopened
                    .connection
                    .query_row("SELECT count(*) FROM mcp_credentials", [], |r| r
                        .get::<_, i64>(0))
                    .unwrap(),
                0
            );
        }
    }
}
