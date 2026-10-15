use super::*;
use crate::{
    authorization::Authorization,
    multi_user_host::{self, Headers, Site},
    user_registry::UserRegistry,
};
use std::{
    sync::{
        atomic::{AtomicBool, AtomicU16, Ordering},
        mpsc, Arc,
    },
    thread,
};
use tiny_http::{Response, Server};

const ORIGIN: &str = "https://reader.example:8443";
const PASSWORD: &str = "fixture-only-password";
pub(crate) struct Mailbox {
    pub(crate) endpoint: String,
    pub(crate) received: mpsc::Receiver<Value>,
    pub(crate) status: Arc<AtomicU16>,
    pub(crate) delay: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}
impl Mailbox {
    pub(crate) fn new() -> Self {
        let server = Server::http("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}/emails", server.server_addr());
        let (tx, received) = mpsc::channel();
        let status = Arc::new(AtomicU16::new(200));
        let delay = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let (s, d, end) = (status.clone(), delay.clone(), stop.clone());
        let worker = thread::spawn(move || {
            while !end.load(Ordering::Relaxed) {
                let Some(mut request) = server.recv_timeout(Duration::from_millis(50)).unwrap()
                else {
                    continue;
                };
                let body: Value = serde_json::from_reader(request.as_reader()).unwrap();
                tx.send(body).unwrap();
                if d.load(Ordering::Relaxed) {
                    thread::sleep(Duration::from_millis(1400));
                }
                let _ = request.respond(
                    Response::from_string("{}").with_status_code(s.load(Ordering::Relaxed)),
                );
            }
        });
        Self {
            endpoint,
            received,
            status,
            delay,
            stop,
            worker: Some(worker),
        }
    }
    pub(crate) fn code(&self, email: &str) -> String {
        let body = self.received.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(body["to"], json!([email]));
        body["text"]
            .as_str()
            .unwrap()
            .split("验证码：")
            .nth(1)
            .unwrap()
            .lines()
            .next()
            .unwrap()
            .into()
    }
}
impl Drop for Mailbox {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.worker.take().unwrap().join().unwrap();
    }
}
struct Fixture {
    access: Arc<Authorization>,
    site: Arc<Site>,
    control: ControlStore,
    mailbox: Mailbox,
    server: Option<multi_user_host::RunningMultiUserServer>,
    _root: tempfile::TempDir,
}
impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let users = UserRegistry::open(root.path()).unwrap();
        let mut control = ControlStore::open(users.writer()).unwrap();
        control.create_user("admin").unwrap();
        let access = Arc::new(Authorization::new(users).unwrap());
        let mailbox = Mailbox::new();
        let mut site = Site::new(ORIGIN).unwrap();
        site.mail = account_mail::tests::settings(&mailbox.endpoint, "1");
        let site = Arc::new(site);
        let server = multi_user_host::start_with_access(
            "127.0.0.1:0".parse().unwrap(),
            site.clone(),
            access.clone(),
        )
        .unwrap();
        Self {
            access,
            site,
            control,
            mailbox,
            server: Some(server),
            _root: root,
        }
    }
    fn invite(&self) -> Value {
        self.access
            .admin
            .create_invites("admin", &uuid::Uuid::now_v7().to_string(), 1, 100)
            .unwrap()["invites"][0]
            .clone()
    }
    fn call(&self, action: &str, body: Value) -> (u16, Value) {
        http(
            &self.server.as_ref().unwrap().url,
            "POST",
            &format!("/api/auth/register/{action}"),
            "reader.example:8443",
            ORIGIN,
            body,
        )
    }
    fn at(&self, action: &str, body: Value, now: i64) -> (u16, Value) {
        let reply = multi_user_host::dispatch(
            &self.access,
            &self.site,
            "POST",
            &format!("/api/auth/register/{action}"),
            &Headers(vec![
                ("Host".into(), "reader.example:8443".into()),
                ("Origin".into(), ORIGIN.into()),
            ]),
            &body.to_string(),
            now,
        );
        assert!(reply.cookie.is_none());
        (reply.status, serde_json::from_slice(&reply.body).unwrap())
    }
    fn start(&self, email: &str, invite: &Value) -> (Value, String) {
        let (status, body) = self.call(
            "start",
            json!({"email":email,"password":PASSWORD,"invite_code":invite["code"]}),
        );
        assert_eq!(status, 200, "{body}");
        let code = self
            .mailbox
            .code(&account_mail::normalize_email(email).unwrap());
        (body, code)
    }
    fn finish(&self, request: &Value, code: &str) -> (u16, Value) {
        self.call(
            "complete",
            json!({"request_id":request["request_id"],"verification_code":code}),
        )
    }
    fn count(&self, table: &str) -> i64 {
        self.control
            .connection
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
            .unwrap()
    }
    fn state(&self, invite: &Value) -> String {
        self.control
            .connection
            .query_row(
                "SELECT state FROM beta_invites WHERE invite_id=?",
                [invite["invite_id"].as_str().unwrap()],
                |r| r.get(0),
            )
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.take().unwrap().shutdown();
    }
}
fn http(
    base: &str,
    method: &str,
    path: &str,
    host: &str,
    origin: &str,
    body: Value,
) -> (u16, Value) {
    let result = ureq::request(method, &format!("{base}{path}"))
        .timeout(Duration::from_secs(10))
        .set("Host", host)
        .set("Origin", origin)
        .send_json(body);
    let response = match result {
        Ok(r) | Err(ureq::Error::Status(_, r)) => r,
        Err(e) => panic!("{e}"),
    };
    assert!(response.header("Set-Cookie").is_none());
    (response.status(), response.into_json().unwrap())
}

#[test]
fn inv4_real_host_registration_receipt_and_account_defaults() {
    let f = Fixture::new();
    let mut invite = f.invite();
    let raw = invite["code"].as_str().unwrap().to_lowercase();
    invite["code"] = json!(format!(
        " {}-{}-{}-{} ",
        &raw[..5],
        &raw[5..10],
        &raw[10..15],
        &raw[15..]
    ));
    let (request, code) = f.start(" New+Tag@Example.com ", &invite);
    assert_eq!(request["resend_after"], 60);
    assert_eq!(f.count("users"), 1);
    assert_eq!(f.state(&invite), "unused");
    let (status, receipt) = f.finish(&request, &code);
    assert_eq!(status, 200, "{receipt}");
    assert_eq!(
        receipt,
        json!({"request_id":request["request_id"],"completed":true})
    );
    assert_eq!(f.finish(&request, "cleared-code").1, receipt);
    assert_eq!(
        f.call("resend", json!({"request_id":request["request_id"]}))
            .1,
        receipt
    );
    assert_eq!(f.count("users"), 2);
    let (owner, hash, disabled, admin, verified): (String,String,bool,bool,i64) = f.control.connection.query_row("SELECT user_id,password_hash,disabled,is_admin,email_verified_at FROM users WHERE email='new+tag@example.com'", [], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).unwrap();
    crate::user_storage_paths::validate_user_id(&owner).unwrap();
    assert!(hash.starts_with("$argon2id$"));
    assert!(!hash.contains(PASSWORD));
    assert!(!disabled && !admin && verified > 0);
    assert_eq!(f.state(&invite), "used");
    for table in [
        "auth_sessions",
        "book_grants",
        "allowance_periods",
        "allowance_adjustments",
    ] {
        assert_eq!(f.count(table), 0);
    }
    let saved = super::request(
        &f.control.connection,
        request["request_id"].as_str().unwrap(),
    )
    .unwrap();
    assert!(saved.hash.is_none() && saved.code.is_none());
    assert_eq!(saved.completed.as_deref(), Some(owner.as_str()));
    let reopened = Registration::new(ControlStore::open(f.control.writer.clone()).unwrap());
    assert_eq!(
        reopened
            .complete(
                Complete {
                    request_id: request["request_id"].as_str().unwrap().into(),
                    verification_code: String::new()
                },
                multi_user_host::now()
            )
            .unwrap(),
        receipt
    );
    // The generated stable identity remains usable alongside INV5 email login.
    assert!(f
        .access
        .auth
        .login(&owner, PASSWORD, None, multi_user_host::now())
        .is_ok());
}

#[test]
fn inv4_failed_attempts_resend_replaces_code_and_expiration() {
    let f = Fixture::new();
    let invite = f.invite();
    let (pending, old) = f.start("tries@example.com", &invite);
    for _ in 0..5 {
        assert_eq!(
            f.finish(&pending, "wrong").1["error_code"],
            "REGISTRATION_CODE_INVALID"
        );
    }
    assert_eq!(
        f.finish(&pending, &old).1["error_code"],
        "REGISTRATION_ATTEMPTS_EXCEEDED"
    );
    let saved = request(
        &f.control.connection,
        pending["request_id"].as_str().unwrap(),
    )
    .unwrap();
    assert_eq!(saved.failures, 5);
    let reopened = Registration::new(ControlStore::open(f.control.writer.clone()).unwrap());
    assert_eq!(
        reopened
            .complete(
                Complete {
                    request_id: pending["request_id"].as_str().unwrap().into(),
                    verification_code: old.clone()
                },
                multi_user_host::now()
            )
            .unwrap_err()
            .error_code,
        "REGISTRATION_ATTEMPTS_EXCEEDED"
    );
    assert_eq!(
        f.call("resend", json!({"request_id":pending["request_id"]}))
            .0,
        429
    );
    account_mail::tests::expire(&f.site.mail);
    let now = pending["expires_at"].as_i64().unwrap() - 60;
    let (status, resent) = f.at("resend", json!({"request_id":pending["request_id"]}), now);
    assert_eq!(status, 200);
    assert_eq!(resent["expires_at"], now + VERIFICATION_SECONDS);
    let new = f.mailbox.code("tries@example.com");
    assert_ne!(new, old);
    assert_eq!(
        request(
            &f.control.connection,
            pending["request_id"].as_str().unwrap()
        )
        .unwrap()
        .failures,
        0
    );
    assert_eq!(f.finish(&pending, &old).0, 400);
    assert_eq!(f.finish(&pending, &new).0, 200);
    assert_eq!(
        f.at(
            "complete",
            json!({"request_id":pending["request_id"],"verification_code":new}),
            now + VERIFICATION_SECONDS
        )
        .1["error_code"],
        "REGISTRATION_EXPIRED"
    );
    let invite2 = f.invite();
    let (pending2, code2) = f.start("expired@example.com", &invite2);
    let deadline = pending2["expires_at"].as_i64().unwrap();
    assert_eq!(
        f.at(
            "complete",
            json!({"request_id":pending2["request_id"],"verification_code":code2}),
            deadline
        )
        .1["error_code"],
        "REGISTRATION_EXPIRED"
    );
    assert_eq!(
        f.at(
            "resend",
            json!({"request_id":pending2["request_id"]}),
            deadline
        )
        .1["error_code"],
        "REGISTRATION_EXPIRED"
    );
    assert_eq!(f.state(&invite2), "unused");
    let cleanup_at = resent["expires_at"].as_i64().unwrap().max(deadline) + 1;
    let (status, fresh) = f.at(
        "start",
        json!({"email":"fresh@example.com","password":PASSWORD,"invite_code":invite2["code"]}),
        cleanup_at,
    );
    assert_eq!(status, 200, "{fresh}");
    f.mailbox.code("fresh@example.com");
    assert_eq!(f.count("registration_requests"), 1);
    assert_eq!(f.state(&invite), "used");
    assert_eq!(f.state(&invite2), "unused");
}

#[test]
fn inv4_request_write_failure_does_not_send_or_replace_saved_credentials() {
    let f = Fixture::new();
    let invite = f.invite();
    f.control.connection.execute_batch("CREATE TRIGGER fail_start BEFORE INSERT ON registration_requests BEGIN SELECT RAISE(ABORT,'fixture'); END;").unwrap();
    let (status, error) = f.call(
        "start",
        json!({"email":"save@example.com","password":PASSWORD,"invite_code":invite["code"]}),
    );
    assert_eq!(status, 503);
    assert!(error.get("request_id").is_none());
    assert_eq!(f.count("registration_requests"), 0);
    assert_eq!(f.state(&invite), "unused");
    assert!(f.mailbox.received.try_recv().is_err());
    f.control
        .connection
        .execute_batch("DROP TRIGGER fail_start")
        .unwrap();
    account_mail::tests::expire(&f.site.mail);
    let (pending, code) = f.start("save@example.com", &invite);
    assert_eq!(f.finish(&pending, "wrong").0, 400);
    let before = request(
        &f.control.connection,
        pending["request_id"].as_str().unwrap(),
    )
    .unwrap();
    f.control.connection.execute_batch("CREATE TRIGGER fail_resend BEFORE UPDATE OF verification_code ON registration_requests BEGIN SELECT RAISE(ABORT,'fixture'); END;").unwrap();
    account_mail::tests::expire(&f.site.mail);
    assert_eq!(
        f.call("resend", json!({"request_id":pending["request_id"]}))
            .0,
        503
    );
    let after = request(
        &f.control.connection,
        pending["request_id"].as_str().unwrap(),
    )
    .unwrap();
    assert_eq!(
        (before.code, before.expires_at, before.failures),
        (after.code, after.expires_at, after.failures)
    );
    assert!(f.mailbox.received.try_recv().is_err());
    f.control
        .connection
        .execute_batch("DROP TRIGGER fail_resend")
        .unwrap();
    assert_eq!(f.finish(&pending, &code).0, 200);
}

#[test]
fn inv4_competing_requests_consume_one_invite_or_one_email() {
    for same_email in [false, true] {
        let f = Fixture::new();
        let first = f.invite();
        let second = if same_email {
            f.invite()
        } else {
            first.clone()
        };
        let (a, ac) = f.start("a@example.com", &first);
        account_mail::tests::expire(&f.site.mail);
        let (b, bc) = f.start(
            if same_email {
                " A@Example.com "
            } else {
                "b@example.com"
            },
            &second,
        );
        let base = &f.server.as_ref().unwrap().url;
        let barrier = std::sync::Barrier::new(2);
        let send = |req: &Value, code: &str| {
            barrier.wait();
            http(
                base,
                "POST",
                "/api/auth/register/complete",
                "reader.example:8443",
                ORIGIN,
                json!({"request_id":req["request_id"],"verification_code":code}),
            )
        };
        let results = thread::scope(|s| {
            let x = s.spawn(|| send(&a, &ac));
            let y = s.spawn(|| send(&b, &bc));
            [x.join().unwrap(), y.join().unwrap()]
        });
        assert_eq!(results.iter().filter(|r| r.0 == 200).count(), 1);
        let failure = results.iter().find(|r| r.0 != 200).unwrap();
        assert_eq!(failure.0, 409);
        assert_eq!(
            failure.1["error_code"],
            if same_email {
                "ACCOUNT_EMAIL_IN_USE"
            } else {
                "INVITE_UNAVAILABLE"
            }
        );
        assert_eq!(f.count("users"), 2);
        if same_email {
            assert_ne!(f.state(&first), f.state(&second));
        }
    }
}

#[test]
fn inv4_disable_and_email_changes_are_rechecked_before_resend_and_complete() {
    let f = Fixture::new();
    for bind_email in [false, true] {
        let invite = f.invite();
        let email = if bind_email {
            "taken@example.com"
        } else {
            "disabled@example.com"
        };
        let (pending, code) = f.start(email, &invite);
        if bind_email {
            f.control
                .connection
                .execute(
                    "UPDATE users SET email=?,email_verified_at=1 WHERE user_id='admin'",
                    [email],
                )
                .unwrap();
        } else {
            f.access
                .admin
                .disable_invite(invite["invite_id"].as_str().unwrap(), 200)
                .unwrap();
        }
        let expected = if bind_email {
            "ACCOUNT_EMAIL_IN_USE"
        } else {
            "INVITE_UNAVAILABLE"
        };
        assert_eq!(f.finish(&pending, &code).1["error_code"], expected);
        assert_eq!(
            f.call("resend", json!({"request_id":pending["request_id"]}))
                .1["error_code"],
            expected
        );
        assert_eq!(
            f.call(
                "start",
                json!({"email":email,"password":PASSWORD,"invite_code":invite["code"]})
            )
            .1["error_code"],
            expected
        );
        assert_eq!(f.count("users"), 1);
        assert_eq!(
            f.state(&invite),
            if bind_email { "unused" } else { "disabled" }
        );
    }
}

#[test]
fn inv4_write_failures_roll_back_every_completion_stage() {
    let f = Fixture::new();
    let invite = f.invite();
    let (pending, code) = f.start("rollback@example.com", &invite);
    for event in [
        "BEFORE INSERT ON users",
        "BEFORE UPDATE OF email ON users",
        "BEFORE UPDATE ON beta_invites",
        "BEFORE UPDATE OF completed_user_id ON registration_requests",
    ] {
        f.control.connection.execute_batch(&format!("CREATE TRIGGER fail_registration {event} BEGIN SELECT RAISE(ABORT,'fixture'); END;")).unwrap();
        assert_eq!(f.finish(&pending, &code).0, 503);
        assert_eq!(f.count("users"), 1);
        assert_eq!(f.state(&invite), "unused");
        let saved = request(
            &f.control.connection,
            pending["request_id"].as_str().unwrap(),
        )
        .unwrap();
        assert!(
            saved.hash.is_some()
                && saved.code.as_deref() == Some(code.as_str())
                && saved.completed.is_none()
        );
        f.control
            .connection
            .execute_batch("DROP TRIGGER fail_registration")
            .unwrap();
    }
    assert_eq!(f.finish(&pending, &code).0, 200);
}

#[test]
fn inv4_mail_rejection_timeout_and_recovery_leave_database_unlocked() {
    let f = Fixture::new();
    let invite = f.invite();
    f.mailbox.status.store(422, Ordering::Relaxed);
    let (status, pending) = f.call(
        "start",
        json!({"email":"retry@example.com","password":PASSWORD,"invite_code":invite["code"]}),
    );
    assert_eq!(status, 503);
    assert_eq!(pending["error_code"], "ACCOUNT_MAIL_REJECTED");
    assert!(pending["request_id"].is_string());
    let old = f.mailbox.code("retry@example.com");
    assert_eq!(f.state(&invite), "unused");
    assert_eq!(f.count("users"), 1);
    assert_eq!(
        f.call("resend", json!({"request_id":pending["request_id"]}))
            .0,
        429
    );
    account_mail::tests::expire(&f.site.mail);
    f.mailbox.status.store(200, Ordering::Relaxed);
    f.mailbox.delay.store(true, Ordering::Relaxed);
    let (reply, new) = thread::scope(|s| {
        let base = &f.server.as_ref().unwrap().url;
        let id = &pending["request_id"];
        let worker = s.spawn(move || {
            http(
                base,
                "POST",
                "/api/auth/register/resend",
                "reader.example:8443",
                ORIGIN,
                json!({"request_id":id}),
            )
        });
        let code = f.mailbox.code("retry@example.com");
        let store = f
            .access
            .registration
            .control
            .try_lock()
            .expect("mail must not hold registration database lock");
        f.control
            .connection
            .execute(
                "UPDATE users SET auth_epoch=auth_epoch+1 WHERE user_id='admin'",
                [],
            )
            .unwrap();
        assert_eq!(
            request(&store.connection, id.as_str().unwrap())
                .unwrap()
                .code
                .as_deref(),
            Some(code.as_str())
        );
        (worker.join().unwrap(), code)
    });
    assert_eq!(reply.0, 503);
    assert_eq!(reply.1["error_code"], "ACCOUNT_MAIL_TIMEOUT");
    assert_eq!(reply.1["request_id"], pending["request_id"]);
    assert_eq!(f.finish(&pending, &old).0, 400);
    assert_eq!(f.finish(&pending, &new).0, 200); // Timeout may still deliver the current code.
    assert_eq!(f.state(&invite), "used");
}

#[test]
fn inv4_public_route_input_origin_and_unconfigured_mail() {
    let f = Fixture::new();
    let invite = f.invite();
    let base = &f.server.as_ref().unwrap().url;
    let valid =
        json!({"email":"input@example.com","password":PASSWORD,"invite_code":invite["code"]});
    for (host, origin) in [
        ("bad.example", ORIGIN),
        ("reader.example:8443", "https://bad.example"),
        ("reader.example:8443", ""),
    ] {
        for action in ["start", "resend", "complete"] {
            assert_eq!(
                http(
                    base,
                    "POST",
                    &format!("/api/auth/register/{action}"),
                    host,
                    origin,
                    valid.clone()
                )
                .0,
                403
            );
        }
    }
    for (field, value, expected) in [
        ("email", json!("not-email"), "ACCOUNT_EMAIL_INVALID"),
        ("password", json!("short"), "PASSWORD_INVALID"),
        ("invite_code", json!("bad"), "INVITE_INVALID"),
        ("user_id", json!("chosen"), "INVALID_REQUEST"),
    ] {
        let mut body = valid.clone();
        body[field] = value;
        assert_eq!(f.call("start", body).1["error_code"], expected);
    }
    for action in ["resend", "complete"] {
        assert_eq!(
            f.call(
                action,
                json!({"request_id":"missing","owner_user_id":"chosen"})
            )
            .0,
            400
        );
    }
    assert_eq!(
        f.call(
            "complete",
            json!({"request_id":"missing","verification_code":"123456"})
        )
        .1["error_code"],
        "REGISTRATION_NOT_FOUND"
    );
    for path in [
        "/auth/register/start",
        "/api/auth/register/other",
        "/api/auth/register/start?user_id=chosen",
    ] {
        assert_eq!(
            http(
                base,
                "POST",
                path,
                "reader.example:8443",
                ORIGIN,
                valid.clone()
            )
            .0,
            401
        );
    }
    assert_ne!(
        http(
            base,
            "GET",
            "/api/auth/register/start",
            "reader.example:8443",
            ORIGIN,
            valid.clone()
        )
        .0,
        200
    );
    let site = Site::new(ORIGIN).unwrap();
    let reply = multi_user_host::dispatch(
        &f.access,
        &site,
        "POST",
        "/api/auth/register/start",
        &Headers(vec![
            ("Host".into(), "reader.example:8443".into()),
            ("Origin".into(), ORIGIN.into()),
        ]),
        &valid.to_string(),
        multi_user_host::now(),
    );
    assert_eq!(reply.status, 503);
    assert_eq!(
        serde_json::from_slice::<Value>(&reply.body).unwrap()["error_code"],
        "ACCOUNT_MAIL_UNAVAILABLE"
    );
    assert_eq!(f.count("registration_requests"), 0);
    assert_eq!(f.count("users"), 1);
    assert_eq!(f.state(&invite), "unused");
}

#[test]
fn inv4_public_attempt_budgets_reset_and_hash_capacity_is_bounded() {
    let f = Fixture::new();
    let invite = f.invite();
    let slot = f.access.registration.password_slot.lock().unwrap();
    assert_eq!(
        f.call(
            "start",
            json!({"email":"busy@example.com","password":PASSWORD,"invite_code":invite["code"]})
        )
        .1["error_code"],
        "REGISTRATION_RATE_LIMITED"
    );
    drop(slot);
    for _ in 1..60 {
        assert_eq!(
            f.call(
                "start",
                json!({"email":"bad","password":PASSWORD,"invite_code":invite["code"]})
            )
            .0,
            400
        );
    }
    assert_eq!(
        f.call(
            "start",
            json!({"email":"bad","password":PASSWORD,"invite_code":invite["code"]})
        )
        .0,
        429
    );
    for _ in 0..60 {
        assert_eq!(
            f.call(
                "complete",
                json!({"request_id":"missing","verification_code":"bad"})
            )
            .0,
            404
        );
    }
    assert_eq!(
        f.call(
            "complete",
            json!({"request_id":"missing","verification_code":"bad"})
        )
        .0,
        429
    );
    for attempts in [
        &f.access.registration.starts,
        &f.access.registration.completions,
    ] {
        for at in attempts.lock().unwrap().iter_mut() {
            *at = Instant::now() - Duration::from_secs(60);
        }
    }
    let (pending, code) = f.start("after@example.com", &invite);
    assert_eq!(f.finish(&pending, &code).0, 200);
}
