use super::*;
use crate::control_store::{ControlStore, ServiceWriter};
use serde_json::Value;
use std::{
    sync::{mpsc, Arc},
    thread,
};
use tiny_http::{Response, Server};

const KEY: &str = "fixture-mail-api-secret";
const CODE: &str = "038529";
const TOKEN: &str = "fixture-reset-token-secret";
const ORIGIN: &str = "https://reader.example:8443";

pub(crate) fn settings(endpoint: &str, timeout: &str) -> AccountMail {
    AccountMail::from_settings(ORIGIN, |key| match key {
        "UNDERSTAND_BOOK_MAIL_FROM" => Some("noreply@example.com".into()),
        "UNDERSTAND_BOOK_MAIL_API_KEY" => Some(KEY.into()),
        "UNDERSTAND_BOOK_MAIL_ENDPOINT" => Some(endpoint.into()),
        "UNDERSTAND_BOOK_MAIL_TIMEOUT_SECONDS" => Some(timeout.into()),
        _ => None,
    })
    .unwrap()
}

struct Captured {
    path: String,
    authorization: String,
    body: Value,
}
fn receiver(
    responses: Vec<(u16, Duration)>,
) -> (String, mpsc::Receiver<Captured>, thread::JoinHandle<()>) {
    let server = Server::http("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}/emails", server.server_addr());
    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        for (status, delay) in responses {
            let mut request = server
                .recv_timeout(Duration::from_secs(10))
                .unwrap()
                .expect("expected mail request");
            assert_eq!(request.method().as_str(), "POST");
            let authorization = request
                .headers()
                .iter()
                .find(|h| h.field.equiv("Authorization"))
                .unwrap()
                .value
                .to_string();
            let path = request.url().to_owned();
            let body: Value = serde_json::from_reader(request.as_reader()).unwrap();
            tx.send(Captured {
                path,
                authorization,
                body,
            })
            .unwrap();
            thread::sleep(delay);
            // Echo credentials in failures: application diagnostics must discard them.
            let body = if status == 200 {
                r#"{"id":"fixture-accepted"}"#.into()
            } else {
                format!("{KEY} {CODE} {TOKEN}")
            };
            let _ = request.respond(
                Response::from_string(body)
                    .with_status_code(status)
                    .with_header(
                        tiny_http::Header::from_bytes("Location", "/unexpected-redirect").unwrap(),
                    ),
            );
        }
        assert!(
            server
                .recv_timeout(Duration::from_millis(150))
                .unwrap()
                .is_none(),
            "unexpected resend or redirect"
        );
    });
    (endpoint, rx, handle)
}
fn receive(rx: &mpsc::Receiver<Captured>) -> Captured {
    rx.recv_timeout(Duration::from_secs(5)).unwrap()
}
pub(crate) fn expire(mail: &AccountMail) {
    for (at, _) in mail.attempts.lock().unwrap().iter_mut() {
        *at = Instant::now() - Duration::from_secs(60);
    }
}

#[test]
fn inv3_actual_http_mail_content_and_trusted_links() {
    let (endpoint, rx, server) = receiver(vec![(200, Duration::ZERO); 3]);
    let site = crate::multi_user_host::Site::new("https://READER.example:8443/").unwrap();
    let mut mail = settings(&endpoint, "2");
    // Use precisely the origin normalized by Site, as production startup does.
    mail.origin = site.mail.origin;
    mail.reserve(" Reader+Tag@Example.com ")
        .unwrap()
        .verification(VerificationPurpose::Registration, CODE)
        .unwrap();
    mail.reserve("bind@example.com")
        .unwrap()
        .verification(VerificationPurpose::EmailBinding, "000001")
        .unwrap();
    mail.reserve("reset@example.com")
        .unwrap()
        .password_reset(TOKEN)
        .unwrap();
    let messages: Vec<_> = (0..3).map(|_| receive(&rx)).collect();
    for message in &messages {
        assert_eq!(message.path, "/emails");
        assert_eq!(message.authorization, format!("Bearer {KEY}"));
        assert_eq!(message.body["from"], "noreply@example.com");
        assert!(message.body["text"].as_str().unwrap().contains(ORIGIN));
    }
    assert_eq!(messages[0].body["to"], json!(["reader+tag@example.com"]));
    assert!(messages[0].body["subject"]
        .as_str()
        .unwrap()
        .contains("注册"));
    assert!(messages[0].body["text"].as_str().unwrap().contains(CODE));
    assert!(messages[0].body["text"]
        .as_str()
        .unwrap()
        .contains("15 分钟"));
    assert!(messages[1].body["subject"]
        .as_str()
        .unwrap()
        .contains("绑定"));
    assert!(messages[1].body["text"]
        .as_str()
        .unwrap()
        .contains("000001"));
    let text = messages[2].body["text"].as_str().unwrap();
    assert!(text.contains("30 分钟"));
    assert!(text.contains("打开链接不会修改密码"));
    assert!(text.contains(&format!("{ORIGIN}/?account=reset-password#token={TOKEN}")));
    server.join().unwrap();
}

#[test]
fn inv3_rejection_unknown_and_no_automatic_retry() {
    let cases = [
        (422, "ACCOUNT_MAIL_REJECTED"),
        (401, "ACCOUNT_MAIL_REJECTED"),
        (429, "ACCOUNT_MAIL_PROVIDER_RATE_LIMITED"),
        (503, "ACCOUNT_MAIL_DELIVERY_UNKNOWN"),
        (302, "ACCOUNT_MAIL_REJECTED"),
    ];
    let (endpoint, rx, server) = receiver(
        cases
            .iter()
            .map(|(status, _)| (*status, Duration::ZERO))
            .collect(),
    );
    let mail = settings(&endpoint, "2");
    for (i, (_, expected)) in cases.iter().enumerate() {
        let err = mail
            .reserve(&format!("reader{i}@example.com"))
            .unwrap()
            .password_reset(TOKEN)
            .unwrap_err();
        assert_eq!(&err.error_code, expected);
        let diagnostic = format!("{err:?}");
        for secret in [KEY, CODE, TOKEN] {
            assert!(!diagnostic.contains(secret));
        }
        receive(&rx);
    }
    server.join().unwrap();
}

#[test]
fn inv3_timeout_preserves_request_and_leaves_database_and_mail_locks_free() {
    let (endpoint, rx, server) = receiver(vec![
        (200, Duration::from_millis(1400)),
        (200, Duration::ZERO),
    ]);
    let mail = settings(&endpoint, "1");
    let root = tempfile::tempdir().unwrap();
    let control = Arc::new(Mutex::new(
        ControlStore::open(ServiceWriter::acquire(root.path()).unwrap()).unwrap(),
    ));
    // Follow the future account handler contract: reserve, commit, unlock, send.
    let send = mail.reserve("reader@example.com").unwrap();
    {
        let mut store = control.lock().unwrap();
        let tx = store.connection.transaction().unwrap();
        tx.execute_batch(
            "INSERT INTO users(user_id) VALUES('reader');
            INSERT INTO password_reset_requests VALUES('saved-token','reader',0,9999,NULL);",
        )
        .unwrap();
        tx.commit().unwrap();
    }
    thread::scope(|scope| {
        let started = Instant::now();
        let worker = scope.spawn(move || send.password_reset("saved-token"));
        receive(&rx); // HTTP received, provider has not responded yet.
        let store = control
            .try_lock()
            .expect("database remains available during mail I/O");
        store
            .connection
            .execute("UPDATE users SET auth_epoch=1 WHERE user_id='reader'", [])
            .unwrap();
        let mut second = ControlStore::open(store.writer.clone()).unwrap();
        let tx = second.connection.transaction().unwrap();
        tx.execute("INSERT INTO users(user_id) VALUES('other')", [])
            .unwrap();
        tx.commit().unwrap();
        assert!(
            mail.attempts.try_lock().is_ok(),
            "mail I/O must not hold the limiter"
        );
        let err = worker.join().unwrap().unwrap_err();
        assert_eq!(err.error_code, "ACCOUNT_MAIL_TIMEOUT");
        assert!(started.elapsed() < Duration::from_secs(3));
        let used: Option<i64> = store
            .connection
            .query_row("SELECT used_at FROM password_reset_requests", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(used, None);
    });
    assert_eq!(
        mail.reserve("reader@example.com").err().unwrap().error_code,
        "ACCOUNT_MAIL_RATE_LIMITED"
    );
    expire(&mail);
    mail.reserve("reader@example.com")
        .unwrap()
        .password_reset("saved-token")
        .unwrap();
    receive(&rx);
    server.join().unwrap();
}

#[test]
fn inv3_shared_rolling_limits_and_concurrent_same_mailbox() {
    let (endpoint, rx, server) = receiver(vec![(200, Duration::ZERO); 61]);
    let mail = settings(&endpoint, "2");
    // Parallel reservations for differently cased versions of one mailbox.
    thread::scope(|s| {
        let a = s.spawn(|| mail.reserve("Same@Example.com"));
        let b = s.spawn(|| mail.reserve(" same@example.com "));
        let results = [a.join().unwrap(), b.join().unwrap()];
        assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
        for result in results {
            match result {
                Ok(send) => send
                    .verification(VerificationPurpose::Registration, CODE)
                    .unwrap(),
                Err(e) => assert_eq!(e.error_code, "ACCOUNT_MAIL_RATE_LIMITED"),
            }
        }
    });
    receive(&rx);
    assert!(mail.reserve("same@example.com").is_err());
    for i in 1..60 {
        mail.reserve(&format!("reader{i}@example.com"))
            .unwrap()
            .password_reset(TOKEN)
            .unwrap();
        receive(&rx);
    }
    assert_eq!(
        mail.reserve("next@example.com").err().unwrap().error_code,
        "ACCOUNT_MAIL_RATE_LIMITED"
    );
    // Expire only the oldest attempt, not the later attempts in the window.
    mail.attempts.lock().unwrap().front_mut().unwrap().0 = Instant::now() - Duration::from_secs(60);
    mail.reserve("same@example.com")
        .unwrap()
        .verification(VerificationPurpose::EmailBinding, CODE)
        .unwrap();
    receive(&rx);
    assert_eq!(
        mail.reserve("reader1@example.com")
            .err()
            .unwrap()
            .error_code,
        "ACCOUNT_MAIL_RATE_LIMITED"
    );
    assert_eq!(mail.attempts.lock().unwrap().len(), 60);
    server.join().unwrap();
}

#[test]
fn inv3_configuration_and_credential_formats() {
    let absent = AccountMail::from_settings(ORIGIN, |_| None).unwrap();
    assert_eq!(
        absent
            .reserve("reader@example.com")
            .err()
            .unwrap()
            .error_code,
        "ACCOUNT_MAIL_UNAVAILABLE"
    );
    for (key, value) in [
        ("UNDERSTAND_BOOK_MAIL_FROM", "noreply@example.com"),
        ("UNDERSTAND_BOOK_MAIL_API_KEY", KEY),
        ("UNDERSTAND_BOOK_MAIL_ENDPOINT", DEFAULT_ENDPOINT),
    ] {
        assert!(AccountMail::from_settings(ORIGIN, |k| (k == key).then(|| value.into())).is_err());
    }
    for (key, value) in [
        (
            "UNDERSTAND_BOOK_MAIL_ENDPOINT",
            "http://mail.example/emails",
        ),
        (
            "UNDERSTAND_BOOK_MAIL_ENDPOINT",
            "https://secret@example.com/emails",
        ),
        ("UNDERSTAND_BOOK_MAIL_TIMEOUT_SECONDS", "0"),
        ("UNDERSTAND_BOOK_MAIL_TIMEOUT_SECONDS", "31"),
        ("UNDERSTAND_BOOK_MAIL_FROM", "Name <noreply@example.com>"),
        ("UNDERSTAND_BOOK_MAIL_API_KEY", "bad\nkey"),
    ] {
        let err = AccountMail::from_settings(ORIGIN, |k| {
            if k == key {
                Some(value.into())
            } else {
                match k {
                    "UNDERSTAND_BOOK_MAIL_FROM" => Some("noreply@example.com".into()),
                    "UNDERSTAND_BOOK_MAIL_API_KEY" => Some(KEY.into()),
                    _ => None,
                }
            }
        })
        .err()
        .unwrap();
        assert!(!err.contains(KEY));
    }
    let defaults = AccountMail::from_settings(ORIGIN, |k| match k {
        "UNDERSTAND_BOOK_MAIL_FROM" => Some("Noreply@Example.com".into()),
        "UNDERSTAND_BOOK_MAIL_API_KEY" => Some(KEY.into()),
        _ => None,
    })
    .unwrap();
    assert_eq!(defaults.transport.unwrap().endpoint, DEFAULT_ENDPOINT);
    assert_eq!(
        normalize_email(" A.B+Tag@Example.com ").unwrap(),
        "a.b+tag@example.com"
    );
    for input in [
        "a@@example.com",
        "a@example.com,b@example.com",
        "a\nb@example.com",
        "@example.com",
        "a@",
        "a..b@example.com",
    ] {
        assert!(normalize_email(input).is_err());
    }
    assert!(crate::multi_user_host::Site::new("https://reader.example/path").is_err());
    for _ in 0..20 {
        let code = verification_code().unwrap();
        assert_eq!(code.len(), 6);
        assert!(code.bytes().all(|b| b.is_ascii_digit()));
        let token = reset_token().unwrap();
        assert_eq!(token.len(), 43);
        assert_eq!(URL_SAFE_NO_PAD.decode(token).unwrap().len(), 32);
    }
    assert_eq!(MAX_VERIFICATION_ATTEMPTS, 5);
}

#[test]
fn inv3_host_startup_configuration_preserves_authentication() {
    const MODE: &str = "UB_INV3_TEST_HOST_MODE";
    if let Ok(mode) = std::env::var(MODE) {
        let root = tempfile::tempdir().unwrap();
        {
            let mut store =
                ControlStore::open(ServiceWriter::acquire(root.path()).unwrap()).unwrap();
            store
                .provision_password("reader", "fixture-only-password", true)
                .unwrap();
        }
        let result = crate::multi_user_host::start(crate::multi_user_host::MultiUserConfig {
            root: root.path().to_owned(),
            addr: "127.0.0.1:0".parse().unwrap(),
            origin: ORIGIN.into(),
        });
        if mode == "invalid" {
            assert!(result
                .err()
                .unwrap()
                .contains("Invalid account mail configuration"));
            return;
        }
        let server = result.unwrap();
        let call = |method: &str,
                    path: &str,
                    origin: &str,
                    cookie: Option<&str>,
                    csrf: Option<&str>,
                    body: Option<Value>| {
            let mut request = ureq::request(method, &format!("{}{path}", server.url))
                .timeout(Duration::from_secs(5))
                .set("Host", "reader.example:8443")
                .set("Origin", origin);
            if let Some(cookie) = cookie {
                request = request.set("Cookie", cookie);
            }
            if let Some(csrf) = csrf {
                request = request.set("X-CSRF-Token", csrf);
            }
            let result = match body {
                Some(body) => request.send_json(body),
                None => request.call(),
            };
            match result {
                Ok(response) | Err(ureq::Error::Status(_, response)) => response,
                Err(e) => panic!("host fixture failed: {e}"),
            }
        };
        assert_eq!(
            call("GET", "/api/auth/me", ORIGIN, None, None, None).status(),
            401
        );
        let body = json!({"username":"reader","password":"fixture-only-password"});
        assert_eq!(
            call(
                "POST",
                "/api/auth/login",
                "https://other.example",
                None,
                None,
                Some(body.clone())
            )
            .status(),
            403
        );
        let response = call("POST", "/api/auth/login", ORIGIN, None, None, Some(body));
        assert_eq!(response.status(), 200);
        let cookie = response
            .header("Set-Cookie")
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_owned();
        let body: Value = response.into_json().unwrap();
        let csrf = body["csrf_token"].as_str().unwrap();
        assert_eq!(body["user_id"], "reader");
        assert_eq!(
            call("GET", "/api/auth/me", ORIGIN, Some(&cookie), None, None).status(),
            200
        );
        assert_eq!(
            call(
                "POST",
                "/api/auth/logout",
                ORIGIN,
                Some(&cookie),
                None,
                Some(json!({}))
            )
            .status(),
            403
        );
        assert_eq!(
            call(
                "POST",
                "/api/auth/logout",
                ORIGIN,
                Some(&cookie),
                Some(csrf),
                Some(json!({}))
            )
            .status(),
            200
        );
        assert_eq!(
            call("GET", "/api/auth/me", ORIGIN, Some(&cookie), None, None).status(),
            401
        );
        server.shutdown();
        return;
    }
    for mode in ["off", "on", "invalid"] {
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "account_mail::tests::inv3_host_startup_configuration_preserves_authentication",
                "--nocapture",
            ])
            .env(MODE, mode)
            .env("UB_OBSERVABILITY_MODE", "off");
        for key in [
            "UNDERSTAND_BOOK_MAIL_FROM",
            "UNDERSTAND_BOOK_MAIL_API_KEY",
            "UNDERSTAND_BOOK_MAIL_ENDPOINT",
            "UNDERSTAND_BOOK_MAIL_TIMEOUT_SECONDS",
        ] {
            command.env_remove(key);
        }
        if mode != "off" {
            command.env("UNDERSTAND_BOOK_MAIL_FROM", "noreply@example.com");
        }
        if mode == "on" {
            command
                .env("UNDERSTAND_BOOK_MAIL_API_KEY", KEY)
                .env("UNDERSTAND_BOOK_MAIL_ENDPOINT", "http://127.0.0.1:9/emails");
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{mode}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn inv3_dns_wait_is_bounded_and_resolution_errors_are_preserved() {
    let (release, stalled) = mpsc::channel();
    let result = resolve_bounded(
        move || {
            let _ = stalled.recv();
            Ok(vec![])
        },
        Duration::from_millis(50),
    );
    assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::TimedOut);
    release.send(()).unwrap();
    let result = resolve_bounded(
        || Err(std::io::Error::new(std::io::ErrorKind::NotFound, "fixture")),
        Duration::from_secs(1),
    );
    assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::NotFound);
}

#[test]
fn inv3_application_logs_do_not_contain_credentials() {
    const CHILD_ENDPOINT: &str = "UB_INV3_TEST_RECEIVER";
    if let Ok(endpoint) = std::env::var(CHILD_ENDPOINT) {
        let mail = settings(&endpoint, "2");
        let err = mail
            .reserve("reader@example.com")
            .unwrap()
            .verification(VerificationPurpose::Registration, CODE)
            .unwrap_err();
        // This is the application's permitted logging surface.
        eprintln!("{err:?}");
        return;
    }
    let (endpoint, rx, server) = receiver(vec![(422, Duration::ZERO)]);
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "account_mail::tests::inv3_application_logs_do_not_contain_credentials",
            "--nocapture",
        ])
        .env(CHILD_ENDPOINT, endpoint)
        .output()
        .unwrap();
    assert!(output.status.success());
    receive(&rx);
    let logs = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(logs.contains("ACCOUNT_MAIL_REJECTED"));
    for secret in [KEY, CODE, TOKEN] {
        assert!(!logs.contains(secret));
    }
    server.join().unwrap();
}
