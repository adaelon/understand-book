use super::mu5_tests::Fixture;
use crate::{
    auth,
    control_store::ControlStore,
    multi_user_host::{self, Headers, Site},
};
use serde_json::{json, Value};
use std::sync::Arc;

fn fixture() -> Fixture {
    let mut f = Fixture::new();
    f.control.set_reader_admin("B", true).unwrap();
    f
}
fn post(f: &Fixture, path: &str, body: Value) -> Value {
    f.ok(&f.b, "POST", path, body)
}
fn count(f: &Fixture, table: &str) -> i64 {
    f.control
        .connection
        .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
        .unwrap()
}
fn login(f: &Fixture, owner: &str, password: &str) -> String {
    f.access
        .auth
        .login(owner, password, None, multi_user_host::now())
        .unwrap()
}

#[test]
fn adm2_online_lifecycle_receipts_and_other_reader_workspace() {
    let f = fixture();
    let w = f.create(&f.a, &f.x, "reader-page");
    let before = f.get(&f.a, &w);
    let me = f.ok(&f.b, "GET", "/api/auth/me", json!({}));
    assert_eq!(me["capabilities"]["admin"], true);
    assert!(me["capabilities"]["presentation"].is_object());
    assert_eq!(me["csrf_token"], auth::csrf(&f.b));
    assert_eq!(
        f.ok(&f.a, "GET", "/api/auth/me", json!({}))["capabilities"]["admin"],
        false
    );

    let create =
        json!({"operation_id":"create-C","user_id":"C","password":"initial-test-password"});
    let receipt = post(&f, "/api/admin/users", create.clone());
    assert_eq!(receipt["actor"], "B");
    assert_eq!(receipt["user_id"], "C");
    assert_eq!(post(&f, "/api/admin/users", create), receipt);
    assert!(!f.root.path().join("users/C").exists());
    assert_eq!(count(&f, "reader_workspaces"), 1);
    let detail = f.ok(&f.b, "GET", "/api/admin/users/C", json!({}));
    assert_eq!(detail["is_admin"], false);
    assert_eq!(detail["book_grants"]["total"], 0);
    let token = login(&f, "C", "initial-test-password");
    assert!(f.ok(&token, "GET", "/api/library", json!({}))["books"]
        .as_array()
        .unwrap()
        .is_empty());
    post(
        &f,
        "/api/admin/users/C/book-grants",
        json!({"operation_id":"grant-C","published_book_ref":f.x}),
    );
    assert_eq!(
        f.ok(&token, "GET", "/api/library", json!({}))["books"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        f.ok(&f.b, "GET", "/api/admin/users/C", json!({}))["book_grants"]["items"][0],
        json!(f.x)
    );
    post(
        &f,
        "/api/admin/users/C/book-revocations",
        json!({"operation_id":"revoke-book-C","published_book_ref":f.x}),
    );
    assert_eq!(
        f.call(&token, "GET", &f.x.url("manifest"), json!({})).0,
        404
    );

    let reset = json!({"operation_id":"password-C","password":"replacement-test-password"});
    let password_receipt = post(&f, "/api/admin/users/C/password", reset.clone());
    assert_eq!(f.call(&token, "GET", "/api/auth/me", json!({})).0, 401);
    let token = login(&f, "C", "replacement-test-password");
    assert_eq!(
        post(&f, "/api/admin/users/C/password", reset),
        password_receipt
    );
    assert_eq!(f.call(&token, "GET", "/api/auth/me", json!({})).0, 200);
    post(
        &f,
        "/api/admin/users/C/revoke-sessions",
        json!({"operation_id":"sessions-C"}),
    );
    assert_eq!(f.call(&token, "GET", "/api/auth/me", json!({})).0, 401);
    let token = login(&f, "C", "replacement-test-password");
    let disable = json!({"operation_id":"disable-C","disabled":true});
    let disabled = post(&f, "/api/admin/users/C/status", disable.clone());
    assert_eq!(f.call(&token, "GET", "/api/auth/me", json!({})).0, 401);
    assert!(f
        .access
        .auth
        .login(
            "C",
            "replacement-test-password",
            None,
            multi_user_host::now()
        )
        .is_err());
    post(
        &f,
        "/api/admin/users/C/status",
        json!({"operation_id":"enable-C","disabled":false}),
    );
    assert_eq!(post(&f, "/api/admin/users/C/status", disable), disabled);
    assert_eq!(
        f.ok(&f.b, "GET", "/api/admin/users/C", json!({}))["disabled"],
        false
    );
    assert_eq!(f.call(&token, "GET", "/api/auth/me", json!({})).0, 401);
    let _fresh = login(&f, "C", "replacement-test-password");
    assert_eq!(f.get(&f.a, &w), before);
    assert_eq!(f.call(&f.a, "GET", &f.x.url("manifest"), json!({})).0, 200);
    assert_eq!(
        f.ok(&f.b, "GET", "/api/admin/operations/create-C", json!({})),
        receipt
    );
    let reopened =
        crate::admin_store::AdminStore::new(ControlStore::open(f.control.writer.clone()).unwrap());
    assert_eq!(reopened.operation("B", "create-C").unwrap(), receipt);
    let mut rows = f
        .control
        .connection
        .prepare("SELECT parameters_json,result_json FROM admin_operations")
        .unwrap();
    for row in rows
        .query_map([], |r| {
            Ok(format!(
                "{}{}",
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?
            ))
        })
        .unwrap()
    {
        let saved = row.unwrap();
        for secret in [
            "initial-test-password",
            "replacement-test-password",
            "$argon2",
            "password_hash",
            "csrf_token",
        ] {
            assert!(!saved.contains(secret));
        }
    }
}

#[test]
fn adm2_permission_csrf_validation_pagination_and_actor_scoped_replay() {
    let mut f = fixture();
    for (method, path, body) in [
        ("GET", "/api/admin/users", json!({})),
        ("GET", "/api/admin/users/B", json!({})),
        ("GET", "/api/admin/books", json!({})),
        ("GET", "/api/admin/operations/key", json!({})),
        (
            "POST",
            "/api/admin/users",
            json!({"operation_id":"key","user_id":"C","password":"fixture-password"}),
        ),
        (
            "POST",
            "/api/admin/users/B/password",
            json!({"operation_id":"key","password":"fixture-password"}),
        ),
        (
            "POST",
            "/api/admin/users/B/status",
            json!({"operation_id":"key","disabled":true}),
        ),
        (
            "POST",
            "/api/admin/users/B/revoke-sessions",
            json!({"operation_id":"key"}),
        ),
        (
            "POST",
            "/api/admin/users/B/book-grants",
            json!({"operation_id":"key","published_book_ref":f.x}),
        ),
        (
            "POST",
            "/api/admin/users/B/book-revocations",
            json!({"operation_id":"key","published_book_ref":f.x}),
        ),
    ] {
        assert_eq!(f.call(&f.a, method, path, body).0, 403, "{path}");
    }
    let body = json!({"operation_id":"key","user_id":"C","password":"fixture-password"});
    for (origin, csrf, token, expected) in [
        ("https://reader.example", None, Some(f.b.as_str()), 403),
        (
            "https://reader.example",
            Some("wrong"),
            Some(f.b.as_str()),
            403,
        ),
        (
            "https://other.example",
            Some(auth::csrf(&f.b)).as_deref(),
            Some(f.b.as_str()),
            403,
        ),
        ("https://reader.example", None, None, 401),
    ] {
        let mut h = vec![
            ("Host".into(), "reader.example".into()),
            ("Origin".into(), origin.into()),
        ];
        if let Some(csrf) = csrf {
            h.push(("X-CSRF-Token".into(), csrf.into()));
        }
        if let Some(token) = token {
            h.push(("Cookie".into(), format!("{}={token}", auth::COOKIE)));
        }
        let reply = multi_user_host::dispatch(
            &f.access,
            &Site::new("https://reader.example").unwrap(),
            "POST",
            "/api/admin/users",
            &Headers(h),
            &body.to_string(),
            multi_user_host::now(),
        );
        assert_eq!(reply.status, expected);
    }
    assert_eq!(count(&f, "users"), 2);
    assert_eq!(count(&f, "admin_operations"), 0);
    for bad in [
        json!({"operation_id":"bad","user_id":"C","password":"short"}),
        json!({"operation_id":"bad","user_id":"C","password":"fixture-password","is_admin":true}),
    ] {
        assert_eq!(f.call(&f.b, "POST", "/api/admin/users", bad).0, 400);
    }
    assert_eq!(
        f.call(
            &f.b,
            "POST",
            "/api/admin/users/missing/status",
            json!({"operation_id":"bad","disabled":true})
        )
        .0,
        404
    );
    assert_eq!(f.call(&f.b,"POST","/api/admin/users/A/book-grants",json!({"operation_id":"bad","published_book_ref":{"book_id":"unknown","publication_id":"missing"}})).0,404);
    assert_eq!(
        f.call(&f.b, "GET", "/api/library?user_id=A", json!({})).0,
        400
    );
    for query in ["limit=0", "limit=101", "offset=-1", "owner_user_id=A"] {
        assert_eq!(
            f.call(&f.b, "GET", &format!("/api/admin/users?{query}"), json!({}))
                .0,
            400
        );
    }
    let page = f.ok(&f.b, "GET", "/api/admin/users?limit=1&offset=1", json!({}));
    assert_eq!(page["total"], 2);
    assert_eq!(page["users"].as_array().unwrap().len(), 1);
    assert_eq!(page["users"][0]["user_id"], "B");
    assert_eq!(
        f.ok(&f.b, "GET", "/api/admin/books?limit=1", json!({}))["books"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let receipt = post(
        &f,
        "/api/admin/users/A/book-grants",
        json!({"operation_id":"key","published_book_ref":f.x}),
    );
    assert_eq!(
        f.call(
            &f.b,
            "POST",
            "/api/admin/users/A/book-grants",
            json!({"operation_id":"key","published_book_ref":f.y})
        )
        .0,
        409
    );
    assert_eq!(
        f.call(
            &f.b,
            "POST",
            "/api/admin/users/B/book-grants",
            json!({"operation_id":"key","published_book_ref":f.x})
        )
        .0,
        409
    );
    f.control.set_reader_admin("A", true).unwrap();
    assert_eq!(
        f.call(&f.a, "GET", "/api/admin/operations/key", json!({}))
            .0,
        404
    );
    let other = f.ok(
        &f.a,
        "POST",
        "/api/admin/users/A/book-grants",
        json!({"operation_id":"key","published_book_ref":f.x}),
    );
    assert_ne!(receipt["actor"], other["actor"]);
    f.control.set_reader_admin("B", false).unwrap();
    assert_eq!(f.call(&f.b, "GET", "/api/admin/users", json!({})).0, 403);
    assert_eq!(
        f.ok(&f.b, "GET", "/api/auth/me", json!({}))["capabilities"]["admin"],
        false
    );
}

#[test]
fn adm2_receipt_failure_rolls_back_account_password_sessions_and_material() {
    let f = fixture();
    f.access.library.lock().unwrap().revoke("A", &f.y).unwrap();
    f.control.connection.execute_batch("CREATE TRIGGER fail_receipt BEFORE INSERT ON admin_operations BEGIN SELECT RAISE(ABORT,'fixture receipt failure'); END;").unwrap();
    let old_hash: String = f
        .control
        .connection
        .query_row(
            "SELECT password_hash FROM users WHERE user_id='A'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let before_sessions = count(&f, "auth_sessions");
    for (path, body) in [
        (
            "/api/admin/users",
            json!({"operation_id":"fail","user_id":"C","password":"fixture-password"}),
        ),
        (
            "/api/admin/users/A/password",
            json!({"operation_id":"fail","password":"replacement-password"}),
        ),
        (
            "/api/admin/users/A/status",
            json!({"operation_id":"fail","disabled":true}),
        ),
        (
            "/api/admin/users/A/revoke-sessions",
            json!({"operation_id":"fail"}),
        ),
        (
            "/api/admin/users/A/book-grants",
            json!({"operation_id":"fail","published_book_ref":f.y}),
        ),
        (
            "/api/admin/users/A/book-revocations",
            json!({"operation_id":"fail","published_book_ref":f.x}),
        ),
    ] {
        assert_eq!(f.call(&f.b, "POST", path, body).0, 503);
    }
    assert_eq!(count(&f, "users"), 2);
    assert_eq!(count(&f, "auth_sessions"), before_sessions);
    assert_eq!(count(&f, "admin_operations"), 0);
    assert_eq!(
        f.control
            .connection
            .query_row(
                "SELECT password_hash FROM users WHERE user_id='A'",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
        old_hash
    );
    assert_eq!(f.call(&f.a, "GET", "/api/auth/me", json!({})).0, 200);
    assert!(f
        .access
        .library
        .lock()
        .unwrap()
        .authorize("A", &f.x)
        .is_ok());
    assert!(f
        .access
        .library
        .lock()
        .unwrap()
        .authorize("A", &f.y)
        .is_err());
}

#[test]
fn adm2_real_http_host_concurrent_create_and_current_role() {
    let mut f = fixture();
    let server = multi_user_host::start_with_access(
        "127.0.0.1:0".parse().unwrap(),
        Arc::new(Site::new("https://reader.example").unwrap()),
        f.access.clone(),
    )
    .unwrap();
    let call = |method: &str, path: &str, body: Value| {
        let response = ureq::request(method, &format!("{}{path}", server.url))
            .set("Host", "reader.example")
            .set("Origin", "https://reader.example")
            .set("Cookie", &format!("{}={}", auth::COOKIE, f.b))
            .set("X-CSRF-Token", &auth::csrf(&f.b))
            .send_json(body);
        let response = match response {
            Ok(r) => r,
            Err(ureq::Error::Status(_, r)) => r,
            Err(e) => panic!("{e}"),
        };
        (response.status(), response.into_json::<Value>().unwrap())
    };
    let body = json!({"operation_id":"http-create","user_id":"C","password":"fixture-password"});
    let replies = std::thread::scope(|s| {
        let a = s.spawn(|| call("POST", "/api/admin/users", body.clone()));
        let b = s.spawn(|| call("POST", "/api/admin/users", body.clone()));
        (a.join().unwrap(), b.join().unwrap())
    });
    assert_eq!(replies.0 .0, 200);
    assert_eq!(replies.0, replies.1);
    assert_eq!(count(&f, "admin_operations"), 1);
    assert_eq!(
        call("GET", "/api/admin/operations/http-create", json!({})).1,
        replies.0 .1
    );
    f.control.set_reader_admin("B", false).unwrap();
    assert_eq!(call("GET", "/api/admin/users", json!({})).0, 403);
    server.shutdown();
}
