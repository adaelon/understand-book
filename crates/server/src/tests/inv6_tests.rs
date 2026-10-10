use super::mu5_tests::Fixture;
use crate::{
    account_mail,
    account_registration::tests::Mailbox,
    auth,
    control_store::ControlStore,
    multi_user_host::{self, Headers, Site},
};
use serde_json::{json, Value};
use std::{
    sync::{atomic::Ordering, Arc},
    time::Duration,
};

const ORIGIN: &str = "https://reader.example";
const PASSWORD: &str = "fixture-only-password";
struct Binding {
    f: Fixture,
    site: Site,
    mail: Mailbox,
}
impl Binding {
    fn new() -> Self {
        let mail = Mailbox::new();
        let mut site = Site::new(ORIGIN).unwrap();
        site.mail = account_mail::tests::settings(&mail.endpoint, "1");
        Self {
            f: Fixture::new(),
            site,
            mail,
        }
    }
    fn at(&self, token: &str, action: &str, body: Value, now: i64) -> (u16, Value) {
        let reply = multi_user_host::dispatch(
            &self.f.access,
            &self.site,
            "POST",
            &format!("/api/account/email/{action}"),
            &headers(token),
            &body.to_string(),
            now,
        );
        assert!(reply.cookie.is_none());
        (reply.status, serde_json::from_slice(&reply.body).unwrap())
    }
    fn call(&self, token: &str, action: &str, body: Value) -> (u16, Value) {
        self.at(token, action, body, multi_user_host::now())
    }
    fn start(&self, token: &str, email: &str) -> (Value, String) {
        let (status, reply) = self.call(
            token,
            "start",
            json!({"email":email,"current_password":PASSWORD}),
        );
        assert_eq!(status, 200, "{reply}");
        let code = self
            .mail
            .code(&account_mail::normalize_email(email).unwrap());
        (reply, code)
    }
    fn finish(&self, token: &str, pending: &Value, code: &str) -> (u16, Value) {
        self.call(
            token,
            "complete",
            json!({"request_id":pending["request_id"],"verification_code":code}),
        )
    }
    fn email(&self, owner: &str) -> Option<String> {
        self.f
            .control
            .connection
            .query_row("SELECT email FROM users WHERE user_id=?", [owner], |r| {
                r.get(0)
            })
            .unwrap()
    }
}
fn headers(token: &str) -> Headers {
    Headers(vec![
        ("Host".into(), "reader.example".into()),
        ("Origin".into(), ORIGIN.into()),
        ("Cookie".into(), format!("{}={token}", auth::COOKIE)),
        ("X-CSRF-Token".into(), auth::csrf(token)),
    ])
}

#[test]
fn inv6_binding_preserves_account_workspace_notes_chats_role_and_allowance() {
    let mut b = Binding::new();
    let f = &mut b.f;
    f.control.set_reader_admin("A", true).unwrap();
    let w = f.create(&f.a, &f.x, "reader");
    let w = f.action(&f.a, &w, "reader", "chat/new", json!({}));
    f.action(
        &f.a,
        &w,
        "reader",
        "reader/note",
        json!({"lid":"1.1","text":"existing private note"}),
    );
    let w = f.get(&f.a, &w);
    let at = multi_user_host::now();
    let period = f.ok(
        &f.a,
        "POST",
        "/api/admin/users/A/allowance-periods",
        json!({"operation_id":"period","starts_at":at-1,"expires_at":at+1000}),
    );
    f.ok(&f.a,"POST","/api/admin/users/A/allowance-adjustments",json!({"operation_id":"gift","period_id":period["allowance_period"]["period_id"],"revision":0,"delta_micro_cny":12345,"reason":"fixture"}));
    let allowance = f.ok(&f.a, "GET", "/api/account/allowance", json!({}));
    let paths = f.access.users.lock().unwrap().paths("A").unwrap();
    let memory = std::fs::read(&paths.memory).unwrap();
    let history = f.ok(&f.a, "GET", "/api/me/chats", json!({}));
    f.ok(&f.a,"POST","/api/me/tutor/mutate",json!({"operation_id":"learning-before-binding","expected_revision":0,"action":{"kind":"set_enabled","enabled":true}}));
    let learning = f.ok(&f.a, "GET", "/api/me/tutor/state", json!({}));
    let account: (String, i64, bool) = f
        .control
        .connection
        .query_row(
            "SELECT password_hash,auth_epoch,is_admin FROM users WHERE user_id='A'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    let (pending, code) = b.start(&b.f.a, " Original+Tag@Example.COM ");
    assert!(b.email("A").is_none());
    let (status, done) = b.finish(&b.f.a, &pending, &code);
    assert_eq!(status, 200, "{done}");
    assert_eq!(
        done,
        json!({"user_id":"A","email":"original+tag@example.com","completed":true})
    );
    assert_eq!(b.finish(&b.f.a, &pending, "cleared").1, done);
    let token =
        b.f.access
            .auth
            .login(" ORIGINAL+TAG@example.com ", PASSWORD, None, at)
            .unwrap();
    assert_eq!(b.f.get(&token, &w), w);
    assert_eq!(b.f.ok(&token, "GET", "/api/me/chats", json!({})), history);
    assert_eq!(
        b.f.ok(&token, "GET", "/api/me/tutor/state", json!({})),
        learning
    );
    assert_eq!(
        b.f.ok(&token, "GET", "/api/account/allowance", json!({})),
        allowance
    );
    assert_eq!(std::fs::read(&paths.memory).unwrap(), memory);
    let after: (String, i64, bool) =
        b.f.control
            .connection
            .query_row(
                "SELECT password_hash,auth_epoch,is_admin FROM users WHERE user_id='A'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
    assert_eq!(account, after);
    assert_eq!(
        b.f.ok(&token, "GET", "/api/auth/me", json!({}))["capabilities"]["admin"],
        true
    );
    assert_eq!(
        b.f.ok(&token, "GET", "/api/library", json!({}))["books"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let db = ControlStore::open(b.f.control.writer.clone()).unwrap();
    assert_eq!(
        db.connection
            .query_row("SELECT count(*) FROM beta_invites", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        db.connection
            .query_row("SELECT email FROM users WHERE user_id='A'", [], |r| r
                .get::<_, String>(0))
            .unwrap(),
        "original+tag@example.com"
    );
    assert_eq!(
        b.call(
            &token,
            "start",
            json!({"email":"change@example.com","current_password":PASSWORD})
        )
        .1["error_code"],
        "ACCOUNT_EMAIL_ALREADY_BOUND"
    );
}

#[test]
fn inv6_wrong_password_conflict_and_other_owner_never_change_account() {
    let b = Binding::new();
    assert_eq!(
        b.call(
            &b.f.a,
            "start",
            json!({"email":"a@example.com","current_password":"wrong"})
        )
        .1["error_code"],
        "CURRENT_PASSWORD_INVALID"
    );
    assert!(b.f.ok(&b.f.a, "GET", "/api/auth/me", json!({}))["email"].is_null());
    assert!(b.mail.received.try_recv().is_err());
    let (pending, code) = b.start(&b.f.a, "a@example.com");
    assert_eq!(b.finish(&b.f.b, &pending, &code).0, 404);
    assert_eq!(
        b.call(
            &b.f.b,
            "resend",
            json!({"request_id":pending["request_id"]})
        )
        .0,
        404
    );
    b.f.control
        .connection
        .execute(
            "UPDATE users SET email='a@example.com',email_verified_at=1 WHERE user_id='B'",
            [],
        )
        .unwrap();
    assert_eq!(
        b.finish(&b.f.a, &pending, &code).1["error_code"],
        "ACCOUNT_EMAIL_IN_USE"
    );
    assert_eq!(
        b.call(
            &b.f.a,
            "resend",
            json!({"request_id":pending["request_id"]})
        )
        .1["error_code"],
        "ACCOUNT_EMAIL_IN_USE"
    );
    assert!(b.email("A").is_none());
}

#[test]
fn inv6_revocation_password_change_disable_and_logout_invalidate_binding_authority() {
    for action in ["revoke", "password", "disable", "logout"] {
        let mut b = Binding::new();
        let (pending, code) = b.start(&b.f.a, "epoch@example.com");
        match action {
            "revoke" => b.f.control.revoke_user_sessions("A").unwrap(),
            "password" => {
                b.f.control
                    .provision_password("A", "replacement-password", false)
                    .unwrap()
            }
            "disable" => {
                b.f.control.set_user_disabled("A", true).unwrap();
                b.f.control.set_user_disabled("A", false).unwrap();
            }
            _ => {
                b.f.ok(&b.f.a, "POST", "/api/auth/logout", json!({}));
            }
        }
        assert_eq!(b.finish(&b.f.a, &pending, &code).0, 401);
        if action != "logout" {
            let token =
                b.f.access
                    .auth
                    .login(
                        "A",
                        if action == "password" {
                            "replacement-password"
                        } else {
                            PASSWORD
                        },
                        None,
                        multi_user_host::now(),
                    )
                    .unwrap();
            assert_eq!(
                b.finish(&token, &pending, &code).1["error_code"],
                "EMAIL_BINDING_EXPIRED"
            );
            assert_eq!(
                b.call(
                    &token,
                    "resend",
                    json!({"request_id":pending["request_id"]})
                )
                .1["error_code"],
                "EMAIL_BINDING_EXPIRED"
            );
        }
        assert!(b.email("A").is_none());
    }
}

#[test]
fn inv6_attempts_persist_resend_replaces_code_expiry_and_new_request_supersedes() {
    let b = Binding::new();
    let (pending, old) = b.start(&b.f.a, "resend@example.com");
    for _ in 0..5 {
        assert_eq!(
            b.finish(&b.f.a, &pending, "wrong").1["error_code"],
            "EMAIL_BINDING_CODE_INVALID"
        );
    }
    let reopened = crate::account_email::EmailBinding::new(
        ControlStore::open(b.f.control.writer.clone()).unwrap(),
    );
    let principal =
        b.f.access
            .auth
            .authenticate(&b.f.a, multi_user_host::now())
            .unwrap();
    assert_eq!(
        reopened
            .complete(
                &principal,
                serde_json::from_value(
                    json!({"request_id":pending["request_id"],"verification_code":old})
                )
                .unwrap(),
                multi_user_host::now()
            )
            .unwrap_err()
            .error_code,
        "EMAIL_BINDING_ATTEMPTS_EXCEEDED"
    );
    assert_eq!(
        b.call(
            &b.f.a,
            "resend",
            json!({"request_id":pending["request_id"]})
        )
        .0,
        429
    );
    account_mail::tests::expire(&b.site.mail);
    let (status, resent) = b.call(
        &b.f.a,
        "resend",
        json!({"request_id":pending["request_id"]}),
    );
    assert_eq!(status, 200);
    let code = b.mail.code("resend@example.com");
    assert_ne!(code, old);
    assert_eq!(b.finish(&b.f.a, &pending, &old).0, 400);
    assert_eq!(
        b.at(
            &b.f.a,
            "complete",
            json!({"request_id":pending["request_id"],"verification_code":code}),
            resent["expires_at"].as_i64().unwrap()
        )
        .1["error_code"],
        "EMAIL_BINDING_EXPIRED"
    );
    assert_eq!(
        b.at(
            &b.f.a,
            "resend",
            json!({"request_id":pending["request_id"]}),
            resent["expires_at"].as_i64().unwrap()
        )
        .1["error_code"],
        "EMAIL_BINDING_EXPIRED"
    );
    let (new, new_code) = b.start(&b.f.a, "corrected@example.com");
    assert_eq!(b.finish(&b.f.a, &pending, &code).0, 404);
    assert_eq!(b.finish(&b.f.a, &new, &new_code).0, 200);
}

#[test]
fn inv6_write_failures_roll_back_email_receipt_and_pending_credentials() {
    let b = Binding::new();
    b.f.control.connection.execute_batch("CREATE TRIGGER fail_start BEFORE INSERT ON email_binding_requests BEGIN SELECT RAISE(ABORT,'fixture'); END").unwrap();
    assert_eq!(
        b.call(
            &b.f.a,
            "start",
            json!({"email":"rollback@example.com","current_password":PASSWORD})
        )
        .0,
        503
    );
    assert!(b.mail.received.try_recv().is_err());
    b.f.control
        .connection
        .execute_batch("DROP TRIGGER fail_start")
        .unwrap();
    account_mail::tests::expire(&b.site.mail);
    let (pending, code) = b.start(&b.f.a, "rollback@example.com");
    account_mail::tests::expire(&b.site.mail);
    b.f.control.connection.execute_batch("CREATE TRIGGER fail_resend BEFORE UPDATE OF verification_code ON email_binding_requests BEGIN SELECT RAISE(ABORT,'fixture'); END").unwrap();
    assert_eq!(
        b.call(
            &b.f.a,
            "resend",
            json!({"request_id":pending["request_id"]})
        )
        .0,
        503
    );
    assert!(b.mail.received.try_recv().is_err());
    b.f.control
        .connection
        .execute_batch("DROP TRIGGER fail_resend")
        .unwrap();
    for event in [
        "BEFORE UPDATE OF email ON users",
        "BEFORE UPDATE OF used_at ON email_binding_requests",
    ] {
        b.f.control
            .connection
            .execute_batch(&format!(
                "CREATE TRIGGER fail_finish {event} BEGIN SELECT RAISE(ABORT,'fixture'); END"
            ))
            .unwrap();
        assert_eq!(b.finish(&b.f.a, &pending, &code).0, 503);
        assert!(b.email("A").is_none());
        assert!(b
            .f
            .control
            .connection
            .query_row(
                "SELECT used_at FROM email_binding_requests WHERE request_id=?",
                [pending["request_id"].as_str().unwrap()],
                |r| r.get::<_, Option<i64>>(0)
            )
            .unwrap()
            .is_none());
        b.f.control
            .connection
            .execute_batch("DROP TRIGGER fail_finish")
            .unwrap();
    }
    assert_eq!(b.finish(&b.f.a, &pending, &code).0, 200);
}

#[test]
fn inv6_competing_accounts_bind_one_email() {
    let b = Binding::new();
    let (a, ac) = b.start(&b.f.a, "shared@example.com");
    account_mail::tests::expire(&b.site.mail);
    let (c, cc) = b.start(&b.f.b, "shared@example.com");
    let gate = std::sync::Barrier::new(2);
    let finish = |token: &str, pending: &Value, code: &str| {
        let service = crate::account_email::EmailBinding::new(
            ControlStore::open(b.f.control.writer.clone()).unwrap(),
        );
        let principal =
            b.f.access
                .auth
                .authenticate(token, multi_user_host::now())
                .unwrap();
        gate.wait();
        service.complete(
            &principal,
            serde_json::from_value(
                json!({"request_id":pending["request_id"],"verification_code":code}),
            )
            .unwrap(),
            multi_user_host::now(),
        )
    };
    let results = std::thread::scope(|s| {
        let x = s.spawn(|| finish(&b.f.a, &a, &ac));
        let y = s.spawn(|| finish(&b.f.b, &c, &cc));
        [x.join().unwrap(), y.join().unwrap()]
    });
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results
            .into_iter()
            .find_map(Result::err)
            .unwrap()
            .error_code,
        "ACCOUNT_EMAIL_IN_USE"
    );
}

#[test]
fn inv6_host_requires_cookie_csrf_origin_and_strict_inputs() {
    let b = Binding::new();
    for action in ["start", "resend", "complete"] {
        for key in ["Cookie", "X-CSRF-Token", "Origin"] {
            let mut h = headers(&b.f.a);
            h.0.retain(|(k, _)| k != key);
            let r = multi_user_host::dispatch(
                &b.f.access,
                &b.site,
                "POST",
                &format!("/api/account/email/{action}"),
                &h,
                "{}",
                multi_user_host::now(),
            );
            assert_eq!(r.status, if key == "Cookie" { 401 } else { 403 });
        }
        assert_eq!(b.call(&b.f.a, action, json!({"owner_user_id":"B"})).0, 400);
        let r = multi_user_host::dispatch(
            &b.f.access,
            &b.site,
            "GET",
            &format!("/api/account/email/{action}"),
            &headers(&b.f.a),
            "{}",
            multi_user_host::now(),
        );
        assert_ne!(r.status, 200);
    }
    assert!(b.email("A").is_none());
    assert!(b.mail.received.try_recv().is_err());
    let r = multi_user_host::dispatch(
        &b.f.access,
        &Site::new(ORIGIN).unwrap(),
        "POST",
        "/api/account/email/start",
        &headers(&b.f.a),
        &json!({"email":"unavailable@example.com","current_password":PASSWORD}).to_string(),
        multi_user_host::now(),
    );
    assert_eq!(r.status, 503);
    assert_eq!(
        b.f.control
            .connection
            .query_row("SELECT count(*) FROM email_binding_requests", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[test]
fn inv6_real_host_mail_failure_and_timeout_keep_request_and_release_database() {
    let b = Binding::new();
    b.mail.status.store(422, Ordering::Relaxed);
    let (status, pending) = b.call(
        &b.f.a,
        "start",
        json!({"email":"delivery@example.com","current_password":PASSWORD}),
    );
    assert_eq!(status, 503);
    assert!(pending["request_id"].is_string());
    let old = b.mail.code("delivery@example.com");
    assert!(b.email("A").is_none());
    account_mail::tests::expire(&b.site.mail);
    b.mail.status.store(200, Ordering::Relaxed);
    b.mail.delay.store(true, Ordering::Relaxed);
    let server = multi_user_host::start_with_access(
        "127.0.0.1:0".parse().unwrap(),
        Arc::new(b.site),
        b.f.access.clone(),
    )
    .unwrap();
    let (reply, code) = std::thread::scope(|s| {
        let worker = s.spawn(|| {
            let result = ureq::post(&format!("{}/api/account/email/resend", server.url))
                .timeout(Duration::from_secs(5))
                .set("Host", "reader.example")
                .set("Origin", ORIGIN)
                .set("Cookie", &format!("{}={}", auth::COOKIE, b.f.a))
                .set("X-CSRF-Token", &auth::csrf(&b.f.a))
                .send_json(json!({"request_id":pending["request_id"]}));
            let response = match result {
                Ok(r) | Err(ureq::Error::Status(_, r)) => r,
                Err(e) => panic!("{e}"),
            };
            (response.status(), response.into_json::<Value>().unwrap())
        });
        let code = b.mail.code("delivery@example.com");
        let _lock =
            b.f.access
                .email_binding
                .control
                .try_lock()
                .expect("mail outside database mutex");
        b.f.control
            .connection
            .execute("UPDATE users SET is_admin=1 WHERE user_id='B'", [])
            .unwrap();
        (worker.join().unwrap(), code)
    });
    assert_eq!(reply.0, 503);
    assert_eq!(reply.1["error_code"], "ACCOUNT_MAIL_TIMEOUT");
    assert_eq!(reply.1["request_id"], pending["request_id"]);
    assert_ne!(code, old);
    let principal =
        b.f.access
            .auth
            .authenticate(&b.f.a, multi_user_host::now())
            .unwrap();
    assert!(b
        .f
        .access
        .email_binding
        .complete(
            &principal,
            serde_json::from_value(
                json!({"request_id":pending["request_id"],"verification_code":code})
            )
            .unwrap(),
            multi_user_host::now()
        )
        .is_ok());
    server.shutdown();
}
