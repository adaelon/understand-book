use super::mu5_tests::Fixture;
use crate::{
    admin_store::AdminStore,
    auth,
    control_store::ControlStore,
    multi_user_host::{self, Headers, Site},
};
use serde_json::{json, Value};

fn fixture() -> Fixture {
    let mut f = Fixture::new();
    f.control.set_reader_admin("B", true).unwrap();
    f
}
fn generate(f: &Fixture, id: &str, count: u32) -> Value {
    f.ok(
        &f.b,
        "POST",
        "/api/admin/invite-batches",
        json!({"operation_id":id,"count":count}),
    )
}
fn count(f: &Fixture, table: &str) -> i64 {
    f.control
        .connection
        .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
        .unwrap()
}

#[test]
fn inv2_batches_replay_scope_reopen_and_atomic_failure() {
    let mut f = fixture();
    let batch = generate(&f, "first", 100);
    assert_eq!(batch, generate(&f, "first", 100));
    assert_eq!(
        batch,
        f.ok(&f.b, "GET", "/api/admin/invite-batches/first", json!({}))
    );
    assert_eq!(count(&f, "invite_batches"), 1);
    assert_eq!(count(&f, "beta_invites"), 100);
    assert_eq!(count(&f, "admin_operations"), 0);
    let codes: std::collections::HashSet<_> = batch["invites"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| {
            let code = i["code"].as_str().unwrap();
            assert_eq!(code.len(), 20);
            assert!(code
                .bytes()
                .all(|b| b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789".contains(&b)));
            assert_eq!(i["state"], "unused");
            code
        })
        .collect();
    assert_eq!(codes.len(), 100);
    assert_eq!(
        f.call(
            &f.b,
            "POST",
            "/api/admin/invite-batches",
            json!({"operation_id":"first","count":1})
        )
        .0,
        409
    );
    f.control.set_reader_admin("A", true).unwrap();
    assert_eq!(
        f.call(&f.a, "GET", "/api/admin/invite-batches/first", json!({}))
            .0,
        404
    );
    let other = f.ok(
        &f.a,
        "POST",
        "/api/admin/invite-batches",
        json!({"operation_id":"first","count":1}),
    );
    assert_eq!(other["actor"], "A");
    let reopened = AdminStore::new(ControlStore::open(f.control.writer.clone()).unwrap());
    assert_eq!(reopened.invite_batch("B", "first").unwrap(), batch);
    // Fail after one invite has already been written: neither the batch nor its prefix survives.
    f.control.connection.execute_batch("CREATE TRIGGER fail_invite BEFORE INSERT ON beta_invites WHEN NEW.operation_id='failure' AND EXISTS(SELECT 1 FROM beta_invites WHERE operation_id='failure') BEGIN SELECT RAISE(ABORT,'fixture'); END;").unwrap();
    assert_eq!(
        f.call(
            &f.b,
            "POST",
            "/api/admin/invite-batches",
            json!({"operation_id":"failure","count":2})
        )
        .0,
        503
    );
    assert_eq!(count(&f, "invite_batches"), 2);
    assert_eq!(count(&f, "beta_invites"), 101);
    assert_eq!(
        f.call(&f.b, "GET", "/api/admin/invite-batches/failure", json!({}))
            .0,
        404
    );
    f.control
        .connection
        .execute_batch("DROP TRIGGER fail_invite")
        .unwrap();
    assert_eq!(generate(&f, "failure", 2)["count"], 2);
}

#[test]
fn inv2_states_pagination_and_disable_retry() {
    let f = fixture();
    let batch = generate(&f, "states", 3);
    let first = batch["invites"][0]["invite_id"].as_str().unwrap();
    let used = batch["invites"][1]["invite_id"].as_str().unwrap();
    f.control
        .connection
        .execute_batch(
            "UPDATE users SET email='a@example.com',email_verified_at=10 WHERE user_id='A'",
        )
        .unwrap();
    f.control
        .connection
        .execute(
            "UPDATE beta_invites SET state='used',used_by='A',used_at=20 WHERE invite_id=?",
            [used],
        )
        .unwrap();
    let path = format!("/api/admin/invites/{first}/disable");
    let disabled = f.ok(&f.b, "POST", &path, json!({}));
    assert_eq!(disabled["state"], "disabled");
    assert!(disabled["disabled_at"].is_i64());
    assert_eq!(
        f.access
            .admin
            .disable_invite(first, multi_user_host::now() + 100)
            .unwrap(),
        disabled
    );
    assert_eq!(f.ok(&f.b, "POST", &path, json!({})), disabled);
    let failed = f.call(
        &f.b,
        "POST",
        &format!("/api/admin/invites/{used}/disable"),
        json!({}),
    );
    assert_eq!(failed.0, 409);
    assert_eq!(failed.1["error_code"], "INVITE_ALREADY_USED");
    for state in ["unused", "used", "disabled"] {
        let page = f.ok(
            &f.b,
            "GET",
            &format!("/api/admin/invites?state={state}&limit=1"),
            json!({}),
        );
        assert_eq!(page["total"], 1);
        assert_eq!(page["invites"][0]["state"], state);
        if state == "used" {
            assert_eq!(page["invites"][0]["used_by"], "A");
            assert_eq!(page["invites"][0]["used_email"], "a@example.com");
        }
    }
    let pages: Vec<_> = (0..3)
        .map(|offset| {
            f.ok(
                &f.b,
                "GET",
                &format!("/api/admin/invites?limit=1&offset={offset}"),
                json!({}),
            )
        })
        .collect();
    assert_eq!(pages[0]["total"], 3);
    assert_ne!(pages[0]["invites"], pages[1]["invites"]);
    assert_ne!(pages[1]["invites"], pages[2]["invites"]);
    assert_eq!(
        f.call(
            &f.b,
            "POST",
            "/api/admin/invites/missing/disable",
            json!({})
        )
        .0,
        404
    );
    let recovered = f.ok(&f.b, "GET", "/api/admin/invite-batches/states", json!({}));
    assert_eq!(recovered["invites"][0], disabled);
    assert_eq!(recovered["invites"][1]["state"], "used");
}

#[test]
fn inv2_host_authorization_csrf_and_input_contract() {
    let mut f = fixture();
    for (method, path, body) in [
        (
            "POST",
            "/api/admin/invite-batches",
            json!({"operation_id":"denied","count":1}),
        ),
        ("GET", "/api/admin/invite-batches/denied", json!({})),
        ("GET", "/api/admin/invites", json!({})),
        ("POST", "/api/admin/invites/missing/disable", json!({})),
    ] {
        assert_eq!(f.call(&f.a, method, path, body.clone()).0, 403);
        assert_eq!(f.call("missing", method, path, body).0, 401);
    }
    for (origin, csrf) in [
        ("https://other.example", Some(auth::csrf(&f.b))),
        ("https://reader.example", None),
    ] {
        let mut headers = vec![
            ("Host".into(), "reader.example".into()),
            ("Origin".into(), origin.into()),
            ("Cookie".into(), format!("{}={}", auth::COOKIE, f.b)),
        ];
        if let Some(csrf) = csrf {
            headers.push(("X-CSRF-Token".into(), csrf));
        }
        assert_eq!(
            multi_user_host::dispatch(
                &f.access,
                &Site::new("https://reader.example").unwrap(),
                "POST",
                "/api/admin/invite-batches",
                &Headers(headers),
                r#"{"operation_id":"denied","count":1}"#,
                multi_user_host::now()
            )
            .status,
            403
        );
    }
    for body in [
        json!({"operation_id":"bad","count":0}),
        json!({"operation_id":"bad","count":101}),
        json!({"operation_id":"bad","count":1.5}),
        json!({"operation_id":"","count":1}),
        json!({"operation_id":"bad","count":1,"actor":"A"}),
    ] {
        assert_eq!(
            f.call(&f.b, "POST", "/api/admin/invite-batches", body).0,
            400
        );
    }
    for query in [
        "limit=0",
        "limit=101",
        "offset=-1",
        "state=unknown",
        "actor=A",
    ] {
        assert_eq!(
            f.call(
                &f.b,
                "GET",
                &format!("/api/admin/invites?{query}"),
                json!({})
            )
            .0,
            400
        );
    }
    assert_eq!(
        f.call(
            &f.b,
            "POST",
            "/api/admin/invites/missing/disable",
            json!({"user_id":"A"})
        )
        .0,
        400
    );
    assert_eq!(count(&f, "invite_batches"), 0);
    f.control.set_reader_admin("B", false).unwrap();
    assert_eq!(
        f.call(
            &f.b,
            "POST",
            "/api/admin/invite-batches",
            json!({"operation_id":"denied","count":1})
        )
        .0,
        403
    );
}

#[test]
fn inv2_concurrent_batch_requests_commit_once() {
    let f = fixture();
    let access = f.access.clone();
    let token = f.b.clone();
    let call = || {
        let reply = multi_user_host::dispatch(
            &access,
            &Site::new("https://reader.example").unwrap(),
            "POST",
            "/api/admin/invite-batches",
            &Headers(vec![
                ("Host".into(), "reader.example".into()),
                ("Origin".into(), "https://reader.example".into()),
                ("Cookie".into(), format!("{}={token}", auth::COOKIE)),
                ("X-CSRF-Token".into(), auth::csrf(&token)),
            ]),
            r#"{"operation_id":"parallel","count":10}"#,
            multi_user_host::now(),
        );
        assert_eq!(reply.status, 200);
        serde_json::from_slice::<Value>(&reply.body).unwrap()
    };
    let replies = std::thread::scope(|s| {
        let a = s.spawn(call);
        let b = s.spawn(call);
        (a.join().unwrap(), b.join().unwrap())
    });
    assert_eq!(replies.0, replies.1);
    assert_eq!(count(&f, "invite_batches"), 1);
    assert_eq!(count(&f, "beta_invites"), 10);
}
