use super::mu5_tests::Fixture;
use crate::{
    account_mail,
    account_registration::tests::Mailbox,
    auth,
    multi_user_host::{self, Headers, Site},
};
use serde_json::{json, Value};
use std::{
    sync::{atomic::Ordering, Arc},
    time::Duration,
};

const OLD: &str = "fixture-only-password";
const NEW: &str = "replacement-password";
struct Password {
    f: Fixture,
    site: Site,
    mail: Mailbox,
}
impl Password {
    fn new() -> Self {
        let f = Fixture::new();
        f.control
            .connection
            .execute(
                "UPDATE users SET email='a@example.com',email_verified_at=1 WHERE user_id='A'",
                [],
            )
            .unwrap();
        let mail = Mailbox::new();
        let mut site = Site::new("https://reader.example").unwrap();
        site.mail = account_mail::tests::settings(&mail.endpoint, "1");
        Self { f, site, mail }
    }
    fn headers(&self) -> Headers {
        Headers(vec![
            ("Host".into(), "reader.example".into()),
            ("Origin".into(), "https://reader.example".into()),
            ("Cookie".into(), format!("{}={}", auth::COOKIE, self.f.a)),
            ("X-CSRF-Token".into(), auth::csrf(&self.f.a)),
        ])
    }
    fn call(&self, method: &str, path: &str, body: Value, now: i64) -> multi_user_host::HttpReply {
        multi_user_host::dispatch(
            &self.f.access,
            &self.site,
            method,
            path,
            &self.headers(),
            &body.to_string(),
            now,
        )
    }
    fn post(&self, path: &str, body: Value) -> multi_user_host::HttpReply {
        self.call("POST", path, body, multi_user_host::now())
    }
    fn forgot(&self) -> String {
        let r = self.post(
            "/api/auth/password/forgot",
            json!({"email":" A@Example.COM "}),
        );
        assert_eq!(r.status, 200, "{}", String::from_utf8_lossy(&r.body));
        self.token()
    }
    fn token(&self) -> String {
        let mail = self
            .mail
            .received
            .recv_timeout(Duration::from_secs(5))
            .unwrap();
        assert_eq!(mail["to"], json!(["a@example.com"]));
        mail["text"]
            .as_str()
            .unwrap()
            .split("#token=")
            .nth(1)
            .unwrap()
            .lines()
            .next()
            .unwrap()
            .to_owned()
    }
    fn reset(&self, token: &str) -> multi_user_host::HttpReply {
        self.post(
            "/api/auth/password/reset",
            json!({"token":token,"new_password":NEW}),
        )
    }
    fn credentials(&self) -> (String, i64) {
        self.f
            .control
            .connection
            .query_row(
                "SELECT password_hash,auth_epoch FROM users WHERE user_id='A'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap()
    }
}
#[test]
fn inv7_change_revokes_all_sessions_and_preserves_workspace_role_material_and_allowance() {
    let mut p = Password::new();
    p.f.control.set_reader_admin("A", true).unwrap();
    let w = p.f.create(&p.f.a, &p.f.x, "reader");
    let allowance = p.f.ok(&p.f.a, "GET", "/api/account/allowance", json!({}));
    let books = p.f.ok(&p.f.a, "GET", "/api/library", json!({}));
    let now = multi_user_host::now();
    let second =
        p.f.access
            .auth
            .login("a@example.com", OLD, None, now)
            .unwrap();
    let token = p.forgot();
    assert_eq!(
        p.post(
            "/api/account/password",
            json!({"current_password":"wrong","new_password":NEW})
        )
        .status,
        400
    );
    let r = p.post(
        "/api/account/password",
        json!({"current_password":OLD,"new_password":NEW}),
    );
    assert_eq!(r.status, 200);
    assert!(r.cookie.unwrap().contains("Max-Age=0"));
    for old in [&p.f.a, &second] {
        assert!(p.f.access.auth.authenticate(old, now).is_err());
    }
    assert_eq!(p.reset(&token).status, 409);
    // Fixture login + second login + wrong/current checks consume four of five attempts.
    assert!(p.f.access.auth.login("A", OLD, None, now).is_err());
    let fresh = crate::auth::AuthService::new(
        crate::control_store::ControlStore::open(p.f.control.writer.clone()).unwrap(),
    );
    let cookie = fresh.login("a@example.com", NEW, None, now).unwrap();
    assert_eq!(p.f.get(&cookie, &w), w);
    assert_eq!(
        p.f.ok(&cookie, "GET", "/api/account/allowance", json!({})),
        allowance
    );
    assert_eq!(p.f.ok(&cookie, "GET", "/api/library", json!({})), books);
    assert!(fresh
        .is_reader_admin(&fresh.authenticate(&cookie, now).unwrap(), now)
        .unwrap());
}
#[test]
fn inv7_reset_is_one_use_persistent_and_get_does_not_change_credentials() {
    let p = Password::new();
    let token = p.forgot();
    let before = p.credentials();
    let w = p.f.create(&p.f.a, &p.f.x, "reader");
    let allowance = p.f.ok(&p.f.a, "GET", "/api/account/allowance", json!({}));
    for url in [
        "/api/auth/password/reset",
        "/api/auth/password/reset?token=ignored",
    ] {
        assert_ne!(
            p.call(
                "GET",
                url,
                json!({"token":token,"new_password":NEW}),
                multi_user_host::now()
            )
            .status,
            200
        );
    }
    assert_eq!(before, p.credentials());
    let reopened = crate::account_password::AccountPassword::new(
        crate::control_store::ControlStore::open(p.f.control.writer.clone()).unwrap(),
    );
    reopened
        .reset(
            serde_json::from_value(json!({"token":token,"new_password":NEW})).unwrap(),
            multi_user_host::now(),
        )
        .unwrap();
    assert_eq!(p.reset(&token).status, 409);
    assert!(p
        .f
        .access
        .auth
        .authenticate(&p.f.a, multi_user_host::now())
        .is_err());
    assert!(p
        .f
        .access
        .auth
        .login("A", OLD, None, multi_user_host::now())
        .is_err());
    let fresh =
        p.f.access
            .auth
            .login("a@example.com", NEW, None, multi_user_host::now())
            .unwrap();
    assert_eq!(p.f.get(&fresh, &w), w);
    assert_eq!(
        p.f.ok(&fresh, "GET", "/api/account/allowance", json!({})),
        allowance
    );
}

#[test]
fn inv7_reset_competing_connections_commit_once_and_forgot_storage_failure_keeps_old_link() {
    let p = Password::new();
    let token = p.forgot();
    p.f.control.connection.execute_batch("CREATE TRIGGER fail BEFORE INSERT ON password_reset_requests BEGIN SELECT RAISE(ABORT,'fixture'); END").unwrap();
    account_mail::tests::expire(&p.site.mail);
    assert_eq!(
        p.post(
            "/api/auth/password/forgot",
            json!({"email":"a@example.com"})
        )
        .status,
        503
    );
    assert!(p.mail.received.try_recv().is_err());
    p.f.control
        .connection
        .execute_batch("DROP TRIGGER fail")
        .unwrap();
    let a = crate::account_password::AccountPassword::new(
        crate::control_store::ControlStore::open(p.f.control.writer.clone()).unwrap(),
    );
    let b = crate::account_password::AccountPassword::new(
        crate::control_store::ControlStore::open(p.f.control.writer.clone()).unwrap(),
    );
    let gate = std::sync::Barrier::new(2);
    let run = |service: &crate::account_password::AccountPassword| {
        gate.wait();
        service.reset(
            serde_json::from_value(json!({"token":token,"new_password":NEW})).unwrap(),
            multi_user_host::now(),
        )
    };
    let results = std::thread::scope(|s| {
        let first = s.spawn(|| run(&a));
        let second = s.spawn(|| run(&b));
        [first.join().unwrap(), second.join().unwrap()]
    });
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results
            .into_iter()
            .find_map(Result::err)
            .unwrap()
            .error_code,
        "PASSWORD_RESET_INVALID"
    );
}
#[test]
fn inv7_expired_revoked_disabled_and_superseded_tokens_are_rejected() {
    for action in ["expire", "revoke", "disable", "admin-password", "supersede"] {
        let mut p = Password::new();
        let old = p.forgot();
        match action {
            "expire" => {
                p.f.control
                    .connection
                    .execute("UPDATE password_reset_requests SET expires_at=1", [])
                    .unwrap();
            }
            "revoke" => p.f.control.revoke_user_sessions("A").unwrap(),
            "disable" => {
                p.f.control.set_user_disabled("A", true).unwrap();
                p.f.control.set_user_disabled("A", false).unwrap();
            }
            "admin-password" => {
                p.f.control
                    .provision_password("A", "admin-replacement", false)
                    .unwrap()
            }
            _ => {
                account_mail::tests::expire(&p.site.mail);
                let new = p.forgot();
                assert_ne!(old, new);
            }
        }
        let before = p.credentials();
        assert_eq!(p.reset(&old).status, 409);
        assert_eq!(before, p.credentials());
    }
}
#[test]
fn inv7_transaction_failures_restore_password_token_and_sessions() {
    for event in [
        "BEFORE UPDATE OF password_hash ON users",
        "BEFORE DELETE ON auth_sessions",
        "BEFORE UPDATE OF used_at ON password_reset_requests",
    ] {
        let p = Password::new();
        let token = p.forgot();
        let before = p.credentials();
        p.f.control
            .connection
            .execute_batch(&format!(
                "CREATE TRIGGER fail {event} BEGIN SELECT RAISE(ABORT,'fixture'); END"
            ))
            .unwrap();
        assert_eq!(p.reset(&token).status, 503);
        assert_eq!(before, p.credentials());
        assert!(p
            .f
            .access
            .auth
            .authenticate(&p.f.a, multi_user_host::now())
            .is_ok());
        let used: Option<i64> =
            p.f.control
                .connection
                .query_row("SELECT used_at FROM password_reset_requests", [], |r| {
                    r.get(0)
                })
                .unwrap();
        assert_eq!(used, None);
        if !event.contains("used_at") {
            assert_eq!(
                p.post(
                    "/api/account/password",
                    json!({"current_password":OLD,"new_password":NEW})
                )
                .status,
                503
            );
            assert_eq!(before, p.credentials());
        }
        p.f.control
            .connection
            .execute_batch("DROP TRIGGER fail")
            .unwrap();
        let r = p.reset(&token);
        assert_eq!(r.status, 200);
        assert!(r.cookie.unwrap().contains("Max-Age=0"));
    }
}
#[test]
fn inv7_forgot_uniform_response_and_precise_host_contract() {
    let mut p = Password::new();
    let known = p.post(
        "/api/auth/password/forgot",
        json!({"email":"a@example.com"}),
    );
    p.token();
    let absent = p.post(
        "/api/auth/password/forgot",
        json!({"email":"missing@example.com"}),
    );
    assert_eq!((known.status, &known.body), (absent.status, &absent.body));
    p.f.control.set_user_disabled("A", true).unwrap();
    account_mail::tests::expire(&p.site.mail);
    assert_eq!(
        p.post(
            "/api/auth/password/forgot",
            json!({"email":"a@example.com"})
        )
        .body,
        known.body
    );
    assert!(p.mail.received.try_recv().is_err());
    p.f.control.set_user_disabled("A", false).unwrap();
    for path in ["/api/auth/password/forgot", "/api/auth/password/reset"] {
        for key in ["Host", "Origin"] {
            let mut h = p.headers();
            h.0.retain(|(k, _)| k != key);
            assert_eq!(
                multi_user_host::dispatch(
                    &p.f.access,
                    &p.site,
                    "POST",
                    path,
                    &h,
                    "{}",
                    multi_user_host::now()
                )
                .status,
                403
            );
        }
        assert_eq!(p.post(path, json!({"owner_user_id":"B"})).status, 400);
        let h = Headers(vec![
            ("Host".into(), "reader.example".into()),
            ("Origin".into(), "https://reader.example".into()),
        ]);
        assert_eq!(
            multi_user_host::dispatch(
                &p.f.access,
                &p.site,
                "POST",
                &format!("{path}/extra"),
                &h,
                "{}",
                multi_user_host::now()
            )
            .status,
            401
        );
    }
    let p = Password::new();
    for key in ["Cookie", "Origin", "X-CSRF-Token"] {
        let mut h = p.headers();
        h.0.retain(|(k, _)| k != key);
        assert_eq!(
            multi_user_host::dispatch(
                &p.f.access,
                &p.site,
                "POST",
                "/api/account/password",
                &h,
                "{}",
                multi_user_host::now()
            )
            .status,
            if key == "Cookie" { 401 } else { 403 }
        );
    }
    let disabled = Site::new("https://reader.example").unwrap();
    for email in ["a@example.com", "missing@example.com"] {
        assert_eq!(
            multi_user_host::dispatch(
                &p.f.access,
                &disabled,
                "POST",
                "/api/auth/password/forgot",
                &p.headers(),
                &json!({"email":email}).to_string(),
                multi_user_host::now()
            )
            .status,
            503
        );
    }
    assert_eq!(
        p.f.control
            .connection
            .query_row("SELECT count(*) FROM password_reset_requests", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
}
#[test]
fn inv7_mail_failure_timeout_keeps_token_and_sends_outside_transaction() {
    let p = Password::new();
    p.mail.status.store(422, Ordering::Relaxed);
    assert_eq!(
        p.post(
            "/api/auth/password/forgot",
            json!({"email":"a@example.com"})
        )
        .status,
        503
    );
    let old = p.token();
    account_mail::tests::expire(&p.site.mail);
    p.mail.status.store(200, Ordering::Relaxed);
    p.mail.delay.store(true, Ordering::Relaxed);
    let server = multi_user_host::start_with_access(
        "127.0.0.1:0".parse().unwrap(),
        Arc::new(p.site),
        p.f.access.clone(),
    )
    .unwrap();
    let token = std::thread::scope(|s| {
        let worker = s.spawn(|| {
            let r = ureq::post(&format!("{}/api/auth/password/forgot", server.url))
                .set("Host", "reader.example")
                .set("Origin", "https://reader.example")
                .send_json(json!({"email":"a@example.com"}));
            match r {
                Err(ureq::Error::Status(503, r)) => assert_eq!(
                    r.into_json::<Value>().unwrap()["error_code"],
                    "ACCOUNT_MAIL_TIMEOUT"
                ),
                _ => panic!("expected timeout"),
            }
        });
        let mail = p
            .mail
            .received
            .recv_timeout(Duration::from_secs(5))
            .unwrap();
        let token = mail["text"]
            .as_str()
            .unwrap()
            .split("#token=")
            .nth(1)
            .unwrap()
            .lines()
            .next()
            .unwrap()
            .to_owned();
        let _lock =
            p.f.access
                .password
                .control
                .try_lock()
                .expect("mail outside database lock");
        p.f.control
            .connection
            .execute("UPDATE users SET is_admin=1 WHERE user_id='B'", [])
            .unwrap();
        worker.join().unwrap();
        token
    });
    assert_ne!(old, token);
    p.f.access
        .password
        .reset(
            serde_json::from_value(json!({"token":token,"new_password":NEW})).unwrap(),
            multi_user_host::now(),
        )
        .unwrap();
    server.shutdown();
}
