//! MU10 acceptance uses real HTTP, private files, SQLite and killed processes.
use super::mu5_tests::Fixture;
use super::*;
use crate::{
    authorization::Authorization,
    multi_user_host::{self, Site},
    user_registry::UserRegistry,
};
use std::{
    io::{Read, Write},
    path::Path,
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
    time::Instant,
};

static TIMING_ACTIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static TIMINGS: std::sync::LazyLock<Mutex<std::collections::BTreeMap<&'static str, Vec<f64>>>> =
    std::sync::LazyLock::new(Mutex::default);
pub(crate) struct Timing(&'static str, Option<Instant>);
pub(crate) fn measure(kind: &'static str) -> Timing {
    Timing(
        kind,
        TIMING_ACTIVE.load(Ordering::Relaxed).then(Instant::now),
    )
}
impl Drop for Timing {
    fn drop(&mut self) {
        if let Some(start) = self.1 {
            TIMINGS
                .lock()
                .unwrap()
                .entry(self.0)
                .or_default()
                .push(start.elapsed().as_secs_f64() * 1000.0);
        }
    }
}

#[derive(Clone, Default)]
struct Probe {
    calls: Arc<AtomicUsize>,
    delay: Duration,
    note_then_crash: bool,
    resolve_tutor: bool,
    gate: Option<Arc<(Mutex<bool>, std::sync::Condvar)>>,
}
impl ModelAdapter for Probe {
    fn complete(&self, _: CompletionRequest) -> Result<ParsedResponse, AdapterError> {
        Err(AdapterError {
            message: "MU10 fixture".into(),
        })
    }
    fn chat(&self, _: &AgentRequestPlan) -> Result<AssistantTurn, AdapterError> {
        let n = self.calls.fetch_add(1, Ordering::SeqCst);
        if self.resolve_tutor && n == 0 {
            return Ok(AssistantTurn {
                provider_continuation: None,
                text: None,
                tool_calls: vec![runtime::ToolCall {
                    id: "mu10-outside".into(),
                    name: "tutor.step".into(),
                    arguments: json!({"operation":"outside"}).to_string(),
                }],
                usage_total_tokens: Some(1),
            });
        }
        if let Some(gate) = &self.gate {
            let (_guard, timeout) = gate
                .1
                .wait_timeout_while(gate.0.lock().unwrap(), Duration::from_secs(30), |ready| {
                    !*ready
                })
                .unwrap();
            assert!(!timeout.timed_out(), "MU10 model gate not released");
        }
        if self.note_then_crash {
            if n > 0 {
                kill_barrier();
            }
            return Ok(AssistantTurn {
                provider_continuation: None,
                text: None,
                tool_calls: vec![runtime::ToolCall {
                    id: "mu10-note".into(),
                    name: "reader.note".into(),
                    arguments: json!({"lid":"1.1","text":"MU10 durable note"}).to_string(),
                }],
                usage_total_tokens: Some(1),
            });
        }
        std::thread::sleep(self.delay);
        Ok(AssistantTurn {
            provider_continuation: None,
            text: Some("你好！".into()),
            tool_calls: vec![],
            usage_total_tokens: None,
        })
    }
}
fn configure(access: &Arc<Authorization>, probe: &Probe) {
    let probe = probe.clone();
    access.runs.configure_fake(move || Box::new(probe.clone()));
}
fn limits(f: Fixture, limits: Value) -> Fixture {
    let Fixture {
        root,
        access,
        control,
        x,
        y,
        a,
        b,
    } = f;
    drop(access);
    drop(control);
    std::fs::write(root.path().join("service-limits.json"), limits.to_string()).unwrap();
    let users = UserRegistry::open(root.path()).unwrap();
    let control = crate::control_store::ControlStore::open(users.writer()).unwrap();
    Fixture {
        root,
        access: Arc::new(Authorization::new(users).unwrap()),
        control,
        x,
        y,
        a,
        b,
    }
}
fn start(access: &Arc<Authorization>) -> multi_user_host::RunningMultiUserServer {
    multi_user_host::start_with_access(
        "127.0.0.1:0".parse().unwrap(),
        Arc::new(Site::new("https://reader.example").unwrap()),
        access.clone(),
    )
    .unwrap()
}
fn http(url: &str, token: &str, method: &str, path: &str, body: Value) -> (u16, Value) {
    let request = ureq::request(method, &format!("{url}{path}"))
        .timeout(Duration::from_secs(20))
        .set("Host", "reader.example")
        .set("Origin", "https://reader.example")
        .set("Cookie", &format!("{}={token}", crate::auth::COOKIE))
        .set("X-CSRF-Token", &crate::auth::csrf(token));
    let response = match if method == "GET" {
        request.call()
    } else {
        request.send_json(body)
    } {
        Ok(r) | Err(ureq::Error::Status(_, r)) => r,
        Err(e) => panic!("MU10 HTTP transport: {e}"),
    };
    (response.status(), response.into_json().unwrap())
}
fn ok(url: &str, token: &str, method: &str, path: &str, body: Value) -> Value {
    let (code, value) = http(url, token, method, path, body);
    assert_eq!(code, 200, "{path}: {value}");
    value
}
fn scene(f: &Fixture) -> (Value, Value, String) {
    let w = f.create(&f.a, &f.x, "page");
    let w = f.action(&f.a, &w, "page", "chat/new", json!({}));
    let mut input = Fixture::stamp(&w, "page");
    input["session_id"] = w["selected_chat"].clone();
    input["client_request_id"] = json!("mu10-key");
    input["message"] = json!("你好");
    let path = format!(
        "/api/workspaces/{}/agent/runs",
        w["workspace_id"].as_str().unwrap()
    );
    (w, input, path)
}
fn until(mut predicate: impl FnMut() -> bool, seconds: u64) {
    let end = Instant::now() + Duration::from_secs(seconds);
    while !predicate() {
        assert!(Instant::now() < end, "MU10 deadline exceeded");
        std::thread::sleep(Duration::from_millis(20));
    }
}
pub(crate) fn kill_barrier() -> ! {
    std::fs::write(std::env::var_os("MU10_READY").unwrap(), b"committed").unwrap();
    loop {
        std::thread::sleep(Duration::from_secs(1));
    }
}
struct KilledChild(Child);
impl Drop for KilledChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
#[ignore = "MU10 subprocess helper; launched only by mu10_real_host_kill_commit_windows"]
fn mu10_kill_host_child() {
    let manifest = std::env::var_os("MU10_MANIFEST").expect("parent supplies manifest");
    let f = Fixture::new();
    let probe = Probe {
        note_then_crash: std::env::var("MU10_KILL_POINT").as_deref() == Ok("note"),
        ..Default::default()
    };
    configure(&f.access, &probe);
    let (w, mut input, path) = scene(&f);
    if std::env::var("MU10_KILL_POINT").as_deref() == Ok("teaching_receipt") {
        use memory::teaching::{TeachingBinding, TeachingEvent, TeachingFact};
        f.ok(&f.a, "POST", "/api/tutor/mutate", json!({"operation_id":"on","expected_revision":0,"action":{"kind":"set_enabled","enabled":true}}));
        let handle = f.access.users.lock().unwrap().get("A", "now").unwrap();
        let user = handle.lock().unwrap();
        let publication = f.access.library.lock().unwrap().load("A", &f.x).unwrap();
        teaching::start_request(
            &PrivateBookContext {
                user: &user,
                book: &publication.book,
                book_dir: publication.directory(),
                messages: &[],
                selected_chat: w["selected_chat"].as_str(),
            },
            "original",
        )
        .unwrap();
        let mut learning = user.learning_store().unwrap();
        let state = learning.state().unwrap();
        let session = &state.sessions[state.control.current_tutor_session_id.as_ref().unwrap()];
        let binding = TeachingBinding {
            tutor_session_id: session.id.clone(),
            session_revision: session.revision,
            control_revision: state.control.revision,
            source_id: f.x.book_id.clone(),
            source_revision: publication.book.source_fingerprint().into(),
            map_revision: "v1".into(),
            chat_session_id: "original-chat".into(),
            turn_id: "original-turn".into(),
        };
        learning.append_teaching(&TeachingEvent {
            event_id: "delivery".into(), binding, kind: TeachingFact::MessageDelivered,
            causal_refs: vec![], occurred_at: "original".into(),
            payload: json!({"move":{"move_id":"question","prompt":"Explain speed","object_ids":["speed"]}}),
        }).unwrap();
        input["teaching_ref"] = json!("delivery");
        input["message"] = json!("I think speed is distance over time");
    }
    if probe.note_then_crash {
        input["message"] = json!("请在 1.1 保存笔记");
    }
    let server = start(&f.access);
    std::fs::write(
        manifest,
        json!({"root":f.root.path(),"url":server.url,"token":f.a,"input":input,"path":path})
            .to_string(),
    )
    .unwrap();
    loop {
        std::thread::sleep(Duration::from_secs(1));
    }
}

#[test]
fn mu10_real_host_kill_commit_windows() {
    for point in [
        "preparing",
        "pending",
        "teaching_receipt",
        "prepared",
        "queued",
        "claimed",
        "note",
        "terminal",
    ] {
        let root = tempfile::tempdir().unwrap();
        let ready = root.path().join("ready");
        let manifest = root.path().join("manifest.json");
        let log = std::fs::File::create(root.path().join("child.log")).unwrap();
        let mut child = KilledChild(
            Command::new(std::env::current_exe().unwrap())
                .args(["mu10_kill_host_child", "--ignored", "--nocapture"])
                .env("MU10_KILL_POINT", point)
                .env("MU10_READY", &ready)
                .env("MU10_MANIFEST", &manifest)
                .env("TMPDIR", root.path())
                .env("TEMP", root.path())
                .env("TMP", root.path())
                .stdout(Stdio::from(log.try_clone().unwrap()))
                .stderr(Stdio::from(log))
                .spawn()
                .unwrap(),
        );
        until(
            || {
                assert!(
                    child.0.try_wait().unwrap().is_none(),
                    "child failed: {}",
                    std::fs::read_to_string(root.path().join("child.log")).unwrap()
                );
                manifest.exists()
            },
            30,
        );
        let meta: Value = serde_json::from_slice(&std::fs::read(&manifest).unwrap()).unwrap();
        let request = meta.clone();
        // A crash may lose the HTTP response; the persistent request key remains authoritative.
        let sender = std::thread::spawn(move || {
            let _ = ureq::post(&format!(
                "{}{}",
                request["url"].as_str().unwrap(),
                request["path"].as_str().unwrap()
            ))
            .timeout(Duration::from_secs(20))
            .set("Host", "reader.example")
            .set("Origin", "https://reader.example")
            .set(
                "Cookie",
                &format!(
                    "{}={}",
                    crate::auth::COOKIE,
                    request["token"].as_str().unwrap()
                ),
            )
            .set(
                "X-CSRF-Token",
                &crate::auth::csrf(request["token"].as_str().unwrap()),
            )
            .send_json(request["input"].clone());
        });
        until(|| ready.exists(), 30);
        child.0.kill().unwrap();
        child.0.wait().unwrap();
        sender.join().unwrap();
        let access = Arc::new(
            Authorization::new(
                UserRegistry::open(Path::new(meta["root"].as_str().unwrap())).unwrap(),
            )
            .unwrap(),
        );
        let probe = Probe {
            resolve_tutor: point == "teaching_receipt",
            ..Default::default()
        };
        configure(&access, &probe);
        let original_teaching = if point == "teaching_receipt" {
            let handle = access.users.lock().unwrap().get("A", "now").unwrap();
            let user = handle.lock().unwrap();
            let turn = &user.agent_history.sessions[0].turns[0];
            let frozen = serde_json::to_value(turn.admission_input.as_ref().unwrap()).unwrap();
            assert_eq!(frozen["preparation_complete"], false);
            let learning = user.learning_store().unwrap();
            let original = learning
                .teaching_event(
                    frozen["teaching"]["events"][0]["event_id"]
                        .as_str()
                        .unwrap(),
                )
                .unwrap();
            assert_eq!(original.payload["attempt"], 1);
            assert!(learning
                .teaching_event(
                    frozen["teaching"]["events"][1]["event_id"]
                        .as_str()
                        .unwrap()
                )
                .is_err());
            Some(original)
        } else {
            None
        };
        let before = Instant::now();
        let server = start(&access);
        let recovery = before.elapsed();
        let token = meta["token"].as_str().unwrap();
        let key = "/api/agent/runs?client_request_id=mu10-key";
        if point != "preparing" {
            until(
                || {
                    let state = ok(&server.url, token, "GET", key, json!({}));
                    state["dispatch_state"] != "queued" && state["dispatch_state"] != "claimed"
                },
                15,
            );
        }
        let (code, result) = http(&server.url, token, "GET", key, json!({}));
        if point == "preparing" {
            assert_eq!(code, 409);
            assert_eq!(result["error_code"], "REQUEST_KEY_CLOSED");
            let db = rusqlite::Connection::open(
                Path::new(meta["root"].as_str().unwrap()).join("control.sqlite"),
            )
            .unwrap();
            assert_eq!(
                db.query_row("SELECT dispatch_state FROM run_admissions", [], |r| r
                    .get::<_, String>(0))
                    .unwrap(),
                "admission_failed"
            );
            assert_eq!(
                http(
                    &server.url,
                    token,
                    "POST",
                    meta["path"].as_str().unwrap(),
                    meta["input"].clone()
                )
                .1["error_code"],
                "REQUEST_KEY_CLOSED"
            );
        } else {
            let handle = access.users.lock().unwrap().get("A", "now").unwrap();
            let user = handle.lock().unwrap();
            assert_eq!(user.agent_history.sessions[0].turns.len(), 1, "{point}");
            let turn = &user.agent_history.sessions[0].turns[0];
            if let Some(original) = &original_teaching {
                let learning = user.learning_store().unwrap();
                assert_eq!(
                    &learning.teaching_event(&original.event_id).unwrap(),
                    original
                );
                let frozen = serde_json::to_value(turn.admission_input.as_ref().unwrap()).unwrap();
                assert_eq!(frozen["preparation_complete"], true);
                for event in frozen["teaching"]["events"].as_array().unwrap() {
                    assert_eq!(
                        &serde_json::to_value(
                            learning
                                .teaching_event(event["event_id"].as_str().unwrap())
                                .unwrap()
                        )
                        .unwrap(),
                        event
                    );
                }
            }
            if ["claimed", "note"].contains(&point) {
                assert_eq!(turn.error.as_ref().unwrap().error_code, "INTERRUPTED");
            } else {
                assert_eq!(
                    turn.status,
                    AgentAssistantStatus::Completed,
                    "{point}: {:?}",
                    turn.error
                );
            }
            if point == "note" {
                assert!(std::fs::read_to_string(
                    access.users.lock().unwrap().paths("A").unwrap().memory
                )
                .unwrap()
                .contains("MU10 durable note"));
            }
        }
        assert_eq!(
            probe.calls.load(Ordering::SeqCst),
            if point == "teaching_receipt" {
                2
            } else {
                usize::from(["pending", "prepared", "queued"].contains(&point))
            },
            "{point}"
        );
        assert!(recovery < Duration::from_secs(10));
        println!(
            "MU10 kill point={point} recovery_ms={} state={} model_calls={}",
            recovery.as_millis(),
            result["dispatch_state"],
            probe.calls.load(Ordering::SeqCst)
        );
        server.shutdown();
    }
}

#[test]
fn mu10_host_sqlite_busy_is_bounded_and_does_not_accept() {
    let f = Fixture::new();
    let probe = Probe::default();
    configure(&f.access, &probe);
    let (_, input, path) = scene(&f);
    let server = start(&f.access);
    let lock = rusqlite::Connection::open(f.root.path().join("control.sqlite")).unwrap();
    lock.execute_batch("BEGIN IMMEDIATE").unwrap();
    let before = Instant::now();
    let (code, result) = http(&server.url, &f.a, "POST", &path, input.clone());
    let elapsed = before.elapsed();
    lock.execute_batch("ROLLBACK").unwrap();
    assert_eq!(code, 503, "{result}");
    assert!(
        elapsed < Duration::from_millis(5500),
        "busy exceeded SQLite 5s bound plus HTTP overhead: {elapsed:?}"
    );
    assert_eq!(probe.calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        f.control
            .connection
            .query_row("SELECT count(*) FROM run_admissions", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(http(&server.url, &f.a, "POST", &path, input).0, 202);
    until(|| probe.calls.load(Ordering::SeqCst) == 1, 10);
    println!("MU10 SQLite busy elapsed_ms={}", elapsed.as_millis());
    server.shutdown();
}

#[test]
fn mu10_slow_sse_does_not_block_commit_and_reconnect_resets_overflow() {
    let f = Fixture::new();
    let probe = Probe {
        delay: Duration::from_secs(3),
        ..Default::default()
    };
    configure(&f.access, &probe);
    let (_, input, path) = scene(&f);
    let server = start(&f.access);
    let (code, accepted) = http(&server.url, &f.a, "POST", &path, input);
    assert_eq!(code, 202);
    let turn = accepted["turn_id"].as_str().unwrap();
    let principal = f
        .access
        .auth
        .authenticate(&f.a, multi_user_host::now())
        .unwrap();
    let ctx = f.access.context(principal, multi_user_host::now()).unwrap();
    let stream = f.access.runs.stream(&ctx, turn).unwrap();
    let mut socket =
        std::net::TcpStream::connect(server.url.trim_start_matches("http://")).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    write!(socket, "GET /api/agent/runs/{turn}/events HTTP/1.1\r\nHost: reader.example\r\nCookie: {}={}\r\n\r\n", crate::auth::COOKIE, f.a).unwrap();
    let mut headers = [0; 256];
    assert!(socket.read(&mut headers).unwrap() > 0);
    let before = Instant::now();
    // Exceed both socket send capacity and the event buffer while this client stops reading.
    for revision in 0..4000 {
        stream.reader_changed(json!({"revision":revision,"payload":"x".repeat(8192)}));
    }
    assert!(
        before.elapsed() < Duration::from_secs(5),
        "event publishing waited on slow socket"
    );
    until(
        || {
            ok(
                &server.url,
                &f.a,
                "GET",
                &format!("/api/agent/runs/{turn}"),
                json!({}),
            )["dispatch_state"]
                == "settled"
        },
        15,
    );
    let frames = stream.read_after(Some(0), Duration::ZERO).0;
    assert_eq!(frames[0].event_type, "run.snapshot");
    assert_eq!(probe.calls.load(Ordering::SeqCst), 1);
    drop(socket);
    let response = ureq::get(&format!("{}/api/agent/runs/{turn}/events", server.url))
        .timeout(Duration::from_secs(5))
        .set("Host", "reader.example")
        .set("Cookie", &format!("{}={}", crate::auth::COOKIE, f.a))
        .set("Last-Event-ID", "old-boot:1")
        .call()
        .unwrap()
        .into_string()
        .unwrap();
    assert!(
        response.contains("event: run.snapshot")
            && response.contains("\"persistence_state\":\"saved\"")
    );
    server.shutdown();
}

#[cfg(target_os = "linux")]
struct Mount(std::path::PathBuf);
#[cfg(target_os = "linux")]
impl Drop for Mount {
    fn drop(&mut self) {
        let _ = Command::new("umount").arg(&self.0).status();
    }
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "MU10 Linux root, isolated 4 MiB tmpfs; never fills the host disk"]
fn mu10_linux_disk_full_keeps_unsaved_then_retries_without_model() {
    let f = Fixture::new();
    let volume = tempfile::tempdir().unwrap();
    assert!(Command::new("mount")
        .args(["-t", "tmpfs", "-o", "size=4m,nosuid,nodev", "tmpfs"])
        .arg(volume.path())
        .status()
        .unwrap()
        .success());
    let _mount = Mount(volume.path().to_owned());
    persistence_fault(&f, volume.path(), true);
}

#[cfg(unix)]
#[test]
#[ignore = "MU10 Linux non-root process; chmod must enforce the real permission failure"]
fn mu10_linux_permission_failure_keeps_unsaved_then_retries_without_model() {
    assert_ne!(
        Command::new("id").arg("-u").output().unwrap().stdout,
        b"0\n",
        "run this test as an unprivileged user"
    );
    let f = Fixture::new();
    let volume = tempfile::tempdir().unwrap();
    persistence_fault(&f, volume.path(), false);
}

#[cfg(unix)]
fn persistence_fault(f: &Fixture, volume: &Path, full: bool) {
    use std::os::unix::fs::PermissionsExt;
    let gate = Arc::new((Mutex::new(false), std::sync::Condvar::new()));
    let probe = Probe {
        gate: Some(gate.clone()),
        ..Default::default()
    };
    configure(&f.access, &probe);
    let (w, input, path) = scene(f);
    let handle = f.access.users.lock().unwrap().get("A", "now").unwrap();
    let history = volume.join("agent-history.json");
    {
        let mut user = handle.lock().unwrap();
        std::fs::copy(user.history_path.as_ref().unwrap(), &history).unwrap();
        user.history_path = Some(history.clone());
    }
    let server = start(&f.access);
    let (code, accepted) = http(&server.url, &f.a, "POST", &path, input);
    assert_eq!(code, 202);
    let turn = accepted["turn_id"].as_str().unwrap();
    until(|| probe.calls.load(Ordering::SeqCst) == 1, 10);
    let committed = std::fs::read(&history).unwrap();
    if full {
        let mut filler = std::fs::File::create(volume.join("filler")).unwrap();
        let error = loop {
            if let Err(e) = filler.write_all(&[0; 65536]) {
                break e;
            }
        };
        assert_eq!(error.raw_os_error(), Some(28), "must inject actual ENOSPC");
    } else {
        std::fs::set_permissions(volume, std::fs::Permissions::from_mode(0o500)).unwrap();
        assert_eq!(
            std::fs::File::create(volume.join("permission-probe"))
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::PermissionDenied
        );
    }
    *gate.0.lock().unwrap() = true;
    gate.1.notify_all();
    let run_path = format!("/api/agent/runs/{turn}");
    until(
        || ok(&server.url, &f.a, "GET", &run_path, json!({}))["persistence_state"] == "failed",
        10,
    );
    assert_eq!(
        std::fs::read(&history).unwrap(),
        committed,
        "failed replace damaged the accepted pending turn"
    );
    assert_eq!(
        http(
            &server.url,
            &f.a,
            "DELETE",
            &format!("/api/me/chats/{}", w["selected_chat"].as_str().unwrap()),
            json!({})
        )
        .1["error_code"],
        "CHAT_BUSY"
    );
    if full {
        std::fs::remove_file(volume.join("filler")).unwrap();
    } else {
        std::fs::set_permissions(volume, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let saved = ok(
        &server.url,
        &f.a,
        "POST",
        &format!("{run_path}/retry-save"),
        json!({}),
    );
    assert_eq!(saved["dispatch_state"], "settled");
    assert_eq!(saved["turn"]["status"], "completed");
    assert_eq!(probe.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        serde_json::from_slice::<Value>(&std::fs::read(history).unwrap()).unwrap()["sessions"][0]
            ["turns"][0]["status"],
        "completed"
    );
    server.shutdown();
}

fn p95(values: &mut [f64]) -> f64 {
    values.sort_by(f64::total_cmp);
    values[((values.len() * 95).div_ceil(100)).saturating_sub(1)]
}

#[test]
#[ignore = "MU10 five-minute 10-user/20-workspace frozen load, isolated service and fake Provider"]
fn mu10_capacity_five_minutes() {
    let f = limits(Fixture::new(), json!({"user_queued_runs":2}));
    let probe = Probe {
        delay: Duration::from_secs(10),
        ..Default::default()
    };
    configure(&f.access, &probe);
    let mut users = vec![("A".to_owned(), f.a.clone()), ("B".to_owned(), f.b.clone())];
    let mut control =
        crate::control_store::ControlStore::open(f.access.users.lock().unwrap().writer()).unwrap();
    for i in 2..10 {
        let owner = format!("load-{i}");
        control
            .provision_password(&owner, "fixture-only-password", true)
            .unwrap();
        for reference in [&f.x, &f.y] {
            f.access
                .library
                .lock()
                .unwrap()
                .grant(&owner, reference)
                .unwrap();
        }
        let token = f
            .access
            .auth
            .login(
                &owner,
                "fixture-only-password",
                None,
                multi_user_host::now(),
            )
            .unwrap();
        users.push((owner, token));
    }
    // Limits are frozen at MU0; configure before opening the registry in production.
    assert_eq!(f.access.resources.limits.model_slots, 2);
    let server = start(&f.access);
    let mut clients = vec![];
    for (owner, token) in users {
        let mut scenes = vec![];
        for reference in [&f.x, &f.y] {
            let w = ok(
                &server.url,
                &token,
                "POST",
                "/api/workspaces",
                json!({"published_book_ref":reference,"attachment_id":"load"}),
            );
            scenes.push(ok(
                &server.url,
                &token,
                "POST",
                &format!(
                    "/api/workspaces/{}/chat/new",
                    w["workspace_id"].as_str().unwrap()
                ),
                Fixture::stamp(&w, "load"),
            ));
        }
        clients.push((owner, token, scenes));
    }
    let end = Instant::now() + Duration::from_secs(300);
    TIMINGS.lock().unwrap().clear();
    TIMING_ACTIVE.store(true, Ordering::Relaxed);
    let resources = f.access.resources.clone();
    let sampling = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let sampling_worker = sampling.clone();
    let monitor = std::thread::spawn(move || {
        let mut peak = 0;
        while sampling_worker.load(Ordering::Relaxed) {
            peak = peak.max(resources.measured_usage().0);
            std::thread::sleep(Duration::from_millis(25));
        }
        peak
    });
    let tasks: Vec<_> = clients.into_iter().map(|(owner, token, scenes)| {
        let url = server.url.clone();
        std::thread::spawn(move || {
            let mut reads = vec![]; let mut writes = vec![]; let mut admits = vec![]; let mut completions = 0; let mut turn: Option<String> = None;
            let mut iteration = 0;
            while Instant::now() < end {
                let before = Instant::now();
                let w = ok(&url, &token, "GET", &format!("/api/workspaces/{}", scenes[0]["workspace_id"].as_str().unwrap()), json!({}));
                reads.push(before.elapsed().as_secs_f64()*1000.0);
                if iteration % 10 == 0 {
                    let other = ok(&url, &token, "GET", &format!("/api/workspaces/{}", scenes[1]["workspace_id"].as_str().unwrap()), json!({}));
                    let mut input = Fixture::stamp(&other,"load"); input["type"] = json!("note"); input["content"] = json!(format!("{owner} sample {iteration}"));
                    input["selection_context"] = json!({"status":"resolved","raw_quote":"t","resolved_quote":"t","ranges":[{"lid":"1.1","range":{"start":0,"end":1}}]});
                    let before = Instant::now();
                    ok(&url, &token, "POST", &format!("/api/workspaces/{}/memory/save", other["workspace_id"].as_str().unwrap()), input);
                    writes.push(before.elapsed().as_secs_f64()*1000.0);
                }
                if let Some(id) = &turn {
                    if ok(&url,&token,"GET",&format!("/api/agent/runs/{id}"),json!({}))["dispatch_state"] == "settled" { turn = None; completions += 1; }
                } else {
                    let mut input = Fixture::stamp(&w,"load"); input["session_id"] = w["selected_chat"].clone(); input["client_request_id"] = json!(format!("load-{iteration}")); input["message"] = json!("你好");
                    let before = Instant::now(); let (code, accepted) = http(&url,&token,"POST",&format!("/api/workspaces/{}/agent/runs", w["workspace_id"].as_str().unwrap()),input);
                    assert_eq!(code,202,"{accepted}"); admits.push(before.elapsed().as_secs_f64()*1000.0); turn = Some(accepted["turn_id"].as_str().unwrap().into());
                }
                iteration += 1; std::thread::sleep(Duration::from_millis(100));
            }
            (owner, reads, writes, admits, completions)
        })
    }).collect();
    let mut read = vec![];
    let mut write = vec![];
    let mut admit = vec![];
    let mut counts = serde_json::Map::new();
    for task in tasks {
        let (owner, r, w, a, n) = task.join().unwrap();
        read.extend(r);
        write.extend(w);
        admit.extend(a);
        counts.insert(owner, json!(n));
    }
    until(
        || {
            f.control.connection.query_row("SELECT count(*) FROM run_admissions WHERE dispatch_state NOT IN ('settled','admission_failed')", [], |r| r.get::<_,i64>(0)).unwrap() == 0
        },
        90,
    );
    let usage = f.access.library.lock().unwrap().resident_usage();
    TIMING_ACTIVE.store(false, Ordering::Relaxed);
    sampling.store(false, Ordering::Relaxed);
    let peak_models = monitor.join().unwrap();
    let idle_resources = f.access.resources.measured_usage();
    let mut timing = TIMINGS.lock().unwrap();
    let user_lock = timing.get_mut("user_lock").unwrap();
    let lock_samples = user_lock.len();
    let lock_p95 = p95(user_lock);
    let commit = timing.get_mut("control_commit").unwrap();
    let commit_samples = commit.len();
    let commit_p95 = p95(commit);
    let report = json!({"seconds":300,"users":10,"workspaces":20,"read_samples":read.len(),"read_p95_ms":p95(&mut read),"private_p95_ms":p95(&mut write),"admit_p95_ms":p95(&mut admit),"user_lock_samples":lock_samples,"user_lock_p95_ms":lock_p95,"control_commit_samples":commit_samples,"control_commit_p95_ms":commit_p95,"peak_models":peak_models,"idle_resources":idle_resources,"completions":counts,"model_calls":probe.calls.load(Ordering::SeqCst),"live_books":usage.0,"book_bytes":usage.1});
    println!("MU10_CAPACITY {report}");
    if let Some(path) = std::env::var_os("MU10_CAPACITY_REPORT") {
        std::fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    }
    server.shutdown();
    assert!(
        report["read_p95_ms"].as_f64().unwrap() <= 250.0
            && report["private_p95_ms"].as_f64().unwrap() <= 500.0
            && report["admit_p95_ms"].as_f64().unwrap() <= 500.0,
        "frozen latency gate: {report}"
    );
    assert!(
        counts.values().all(|n| n.as_u64().unwrap() >= 1),
        "every user must make progress"
    );
    assert!(usage.1 <= 2 * 1024 * 1024 * 1024);
    assert!(
        lock_p95 <= 50.0 && commit_p95 <= 100.0,
        "frozen lock/commit gate: {report}"
    );
    assert_eq!(peak_models, 2);
    assert_eq!(idle_resources, (0, 0, 0));
}

#[test]
fn mu10_recovery_100_unclaimed_turns_keeps_original_inputs() {
    let f = limits(
        Fixture::new(),
        json!({"queued_runs":100,"user_queued_runs":100,"workspaces":100,"user_workspaces":100}),
    );
    let probe = Probe::default();
    configure(&f.access, &probe);
    for i in 0..100 {
        let page = format!("recovery-{i}");
        let w = f.create(&f.a, &f.x, &page);
        let w = f.action(&f.a, &w, &page, "chat/new", json!({}));
        let mut input = Fixture::stamp(&w, &page);
        input["session_id"] = w["selected_chat"].clone();
        input["client_request_id"] = json!(page);
        input["message"] = json!("你好");
        assert_eq!(
            f.call(
                &f.a,
                "POST",
                &format!(
                    "/api/workspaces/{}/agent/runs",
                    w["workspace_id"].as_str().unwrap()
                ),
                input
            )
            .0,
            202
        );
    }
    let root = f.root.path().to_owned();
    let Fixture {
        root: retained,
        access,
        control,
        ..
    } = f;
    drop(access);
    drop(control);
    let before = Instant::now();
    let access = Arc::new(Authorization::new(UserRegistry::open(&root).unwrap()).unwrap());
    configure(&access, &probe);
    access.runs.recover(&access).unwrap();
    let elapsed = before.elapsed();
    let db = rusqlite::Connection::open(root.join("control.sqlite")).unwrap();
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM run_admissions WHERE dispatch_state='queued' AND attempt=0",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        100
    );
    let user = access.users.lock().unwrap().get("A", "now").unwrap();
    assert_eq!(
        user.lock()
            .unwrap()
            .agent_history
            .sessions
            .iter()
            .flat_map(|s| &s.turns)
            .filter(|t| t.admission_input.is_some()
                && t.status == AgentAssistantStatus::PendingAssistant)
            .count(),
        100
    );
    assert_eq!(probe.calls.load(Ordering::SeqCst), 0);
    println!("MU10 recovery_100_ms={}", elapsed.as_millis());
    assert!(elapsed < Duration::from_secs(10));
    drop(user);
    drop(access);
    drop(db);
    drop(retained);
}
