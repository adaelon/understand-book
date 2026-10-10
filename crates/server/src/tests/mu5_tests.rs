use super::*;
use crate::{
    authorization::Authorization,
    control_store::ControlStore,
    multi_user_host::{self, Headers, Site},
    published_library::PublishedBookRef,
    user_registry::UserRegistry,
    workspace_registry::{NetworkRunPort, WORKSPACE_IDLE_TTL},
};
use runtime::run_context::ResidentStatePort;

pub(super) struct Fixture {
    pub(super) root: tempfile::TempDir,
    pub(super) access: Arc<Authorization>,
    pub(super) control: ControlStore,
    pub(super) x: PublishedBookRef,
    pub(super) y: PublishedBookRef,
    pub(super) a: String,
    pub(super) b: String,
}

#[test]
fn tutor_t18_published_readiness_requires_the_exact_reader_grant() {
    let f = Fixture::new();
    // Model an already published book from before tutor_readiness was introduced.
    let directory = f.access.library.lock().unwrap().load("A", &f.x).unwrap().directory().to_path_buf();
    let path = directory.join("publication.json");
    let mut manifest: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    manifest.as_object_mut().unwrap().remove("tutor_readiness");
    let original_permissions = std::fs::metadata(&path).unwrap().permissions();
    let mut writable = original_permissions.clone();
    writable.set_readonly(false);
    std::fs::set_permissions(&path, writable).unwrap();
    std::fs::write(&path, manifest.to_string()).unwrap();
    std::fs::set_permissions(&path, original_permissions).unwrap();
    let path = f.x.url("tutor_readiness");
    let ready = f.ok(&f.a, "GET", &path, json!({}));
    assert_eq!(ready["status"], "ready");
    assert_eq!(ready["teaching_assets"]["teaching_map_revision"], "v1");
    assert!(ready.get("required_stages").is_none());
    f.access.library.lock().unwrap().revoke("A", &f.x).unwrap();
    assert_ne!(f.call(&f.a, "GET", &path, json!({})).0, 200);
}
impl Fixture {
    pub(super) fn new() -> Self {
        Self::configured(None)
    }
    pub(super) fn configured(sandbox_config: Option<&std::path::Path>) -> Self {
        let root = tempfile::tempdir().unwrap();
        if let Some(config) = sandbox_config {
            std::fs::copy(config, root.path().join("presentation-sandbox.json")).unwrap();
        }
        let users = UserRegistry::open(root.path()).unwrap();
        let mut control = ControlStore::open(users.writer()).unwrap();
        for id in ["A", "B"] {
            control
                .provision_password(id, "fixture-only-password", true)
                .unwrap();
        }
        let access = Arc::new(Authorization::new(users).unwrap());
        let publish = |id: &str| {
            let dir = write_multi_leaf_book(&format!("mu5-{id}-{}", uuid::Uuid::now_v7()), id, 30);
            let book = Book::load(dir.to_str().unwrap()).unwrap();
            let source = std::fs::read(dir.join("source.txt")).unwrap();
            super::tutor_tests::t15_foundation(&dir, &book);
            std::fs::write(dir.join("source.txt"), source).unwrap();
            super::tutor_tests::t15_close(&dir, &book, "pass1", &["base.json", "source.txt"]);
            std::fs::create_dir_all(dir.join("teaching/versions/v1")).unwrap();
            std::fs::write(dir.join("teaching/versions/v1/map.json"),json!({"source_id":id,"source_revision":book.source_fingerprint(),"objects":{"active_refs":[{"source_id":id,"object_id":"speed"}],"objects":[{"ref":{"source_id":id,"object_id":"speed"},"meaning":"speed","object_revision":1,"source_bindings":[{"source_id":id,"source_revision":book.source_fingerprint(),"lid":"1.1"}]}]},"cognitive_materials":{"materials":[]}}).to_string()).unwrap();
            std::fs::write(dir.join("teaching_readiness.json"),json!({"version":"teaching_readiness.v1","status":"ready","source_id":id,"source_revision":book.source_fingerprint(),"teaching_map_revision":"v1","map_path":"teaching/versions/v1/map.json","coverage":{"source":"complete","structure":"complete","objects":"complete","cognitive_materials":"complete","source_review":"passed"},"limitations":[]}).to_string()).unwrap();
            let mut library = access.library.lock().unwrap();
            let p = library.publish(&dir).unwrap().reference;
            for owner in ["A", "B"] {
                library.grant(owner, &p).unwrap();
            }
            p
        };
        let x = publish("mu5-x");
        let y = publish("mu5-y");
        let a = access
            .auth
            .login("A", "fixture-only-password", None, multi_user_host::now())
            .unwrap();
        let b = access
            .auth
            .login("B", "fixture-only-password", None, multi_user_host::now())
            .unwrap();
        Self {
            root,
            access,
            control,
            x,
            y,
            a,
            b,
        }
    }
    pub(super) fn call(&self, token: &str, method: &str, path: &str, input: Value) -> (u16, Value) {
        let reply = multi_user_host::dispatch(
            &self.access,
            &Site::new("https://reader.example").unwrap(),
            method,
            path,
            &Headers(vec![
                ("Host".into(), "reader.example".into()),
                ("Origin".into(), "https://reader.example".into()),
                ("Cookie".into(), format!("{}={token}", crate::auth::COOKIE)),
                ("X-CSRF-Token".into(), crate::auth::csrf(token)),
            ]),
            &input.to_string(),
            multi_user_host::now(),
        );
        (reply.status, serde_json::from_slice(&reply.body).unwrap())
    }
    pub(super) fn ok(&self, token: &str, method: &str, path: &str, input: Value) -> Value {
        let (status, value) = self.call(token, method, path, input);
        assert_eq!(status, 200, "{path}: {value}");
        value
    }
    pub(super) fn create(&self, token: &str, reference: &PublishedBookRef, attachment: &str) -> Value {
        self.ok(
            token,
            "POST",
            "/api/workspaces",
            json!({"published_book_ref":reference,"attachment_id":attachment}),
        )
    }
    pub(super) fn action(
        &self,
        token: &str,
        w: &Value,
        attachment: &str,
        action: &str,
        mut input: Value,
    ) -> Value {
        input["generation"] = w["generation"].clone();
        input["expected_revision"] = w["revision"].clone();
        input["attachment_id"] = json!(attachment);
        self.ok(
            token,
            "POST",
            &format!(
                "/api/workspaces/{}/{action}",
                w["workspace_id"].as_str().unwrap()
            ),
            input,
        )
    }
    pub(super) fn get(&self, token: &str, w: &Value) -> Value {
        self.ok(
            token,
            "GET",
            &format!("/api/workspaces/{}", w["workspace_id"].as_str().unwrap()),
            json!({}),
        )
    }
    pub(super) fn stamp(w: &Value, attachment: &str) -> Value {
        json!({"generation":w["generation"],"expected_revision":w["revision"],"attachment_id":attachment})
    }
    pub(super) fn run(&self, w: &Value, attachment: &str) -> NetworkRunPort {
        let principal = self
            .access
            .auth
            .authenticate(&self.a, multi_user_host::now())
            .unwrap();
        let context = self
            .access
            .context(principal, multi_user_host::now())
            .unwrap();
        let turn_id = format!("turn-{}", uuid::Uuid::now_v7());
        let chat = w["selected_chat"].as_str().unwrap();
        let ordinal = {
            let mut user = context.user.lock().unwrap();
            let mut s = user.agent_history.sessions.iter().find(|s| s.id == chat).unwrap().clone();
            let ordinal = s.turns.last().map_or(1, |t| t.user_turn_ordinal + 1);
            s.turns.push(serde_json::from_value(json!({"turn_id":turn_id,"user_turn_ordinal":ordinal,"user":"Explain this","status":"pending_assistant","published_book_ref":w["published_book_ref"],"question_anchor_lid":null,"question_quote":null})).unwrap());
            let user = &mut *user;
            user.session_store.as_mut().unwrap().accept(&mut user.agent_history, s, "now").unwrap();
            ordinal
        };
        let turn = AgentTurnRef {
            session_id: chat.into(),
            turn_id,
            user_turn_ordinal: ordinal,
        };
        NetworkRunPort::capture(
            self.access.clone(),
            context,
            w["workspace_id"].as_str().unwrap(),
            serde_json::from_value(Self::stamp(w, attachment)).unwrap(),
            turn,
            ChatStubAdapter::scripted(vec![]).model_runtime_profile(),
            "Explain this",
        )
        .unwrap()
    }
}

#[test]
fn mu5_real_http_windows_isolate_readers_and_share_user_authority() {
    let f = Fixture::new();
    let server = multi_user_host::start_with_access(
        "127.0.0.1:0".parse().unwrap(),
        Arc::new(Site::new("https://reader.example").unwrap()),
        f.access.clone(),
    )
    .unwrap();
    let http = |token: &str, path: &str, input: Value| {
        ureq::post(&format!("{}{path}", server.url))
            .set("Host", "reader.example")
            .set("Origin", "https://reader.example")
            .set("Cookie", &format!("{}={token}", crate::auth::COOKIE))
            .set("X-CSRF-Token", &crate::auth::csrf(token))
            .send_json(input)
            .unwrap()
            .into_json::<Value>()
            .unwrap()
    };
    let a1 = http(
        &f.a,
        "/api/workspaces",
        json!({"published_book_ref":f.x,"attachment_id":"phone"}),
    );
    let a2 = http(
        &f.a,
        "/api/workspaces",
        json!({"published_book_ref":f.y,"attachment_id":"desktop"}),
    );
    let b1 = http(
        &f.b,
        "/api/workspaces",
        json!({"published_book_ref":f.x,"attachment_id":"B"}),
    );
    let mut move_a = Fixture::stamp(&a1, "phone");
    move_a["lid"] = json!("1.20");
    let moved = http(
        &f.a,
        &format!(
            "/api/workspaces/{}/reader/goto",
            a1["workspace_id"].as_str().unwrap()
        ),
        move_a,
    );
    assert_ne!(
        moved["reader"]["viewport"]["top_lid"],
        a1["reader"]["viewport"]["top_lid"]
    );
    assert_eq!(f.get(&f.a, &a2)["reader"], a2["reader"]);
    assert_eq!(f.get(&f.b, &b1)["reader"], b1["reader"]);
    let changed = f.action(
        &f.b,
        &b1,
        "B",
        "book/open",
        json!({"published_book_ref":f.y}),
    );
    assert_eq!(changed["published_book_ref"], json!(f.y));
    assert_eq!(f.get(&f.a, &moved)["published_book_ref"], json!(f.x));
    for action in ["reader/state", "takeover", "book/open"] {
        assert_eq!(
            f.call(
                &f.b,
                "POST",
                &format!(
                    "/api/workspaces/{}/{action}",
                    a1["workspace_id"].as_str().unwrap()
                ),
                Fixture::stamp(&moved, "phone")
            )
            .0,
            404
        );
    }
    let note = f.action(
        &f.a,
        &moved,
        "phone",
        "reader/note",
        json!({"lid":"1.1","text":"shared private note"}),
    );
    assert!(note["result"]["note_id"].is_string());
    let h1 = f.access.users.lock().unwrap().get("A", "now").unwrap();
    let h2 = f.access.users.lock().unwrap().get("A", "now").unwrap();
    assert!(Arc::ptr_eq(&h1, &h2));
    let text =
        std::fs::read_to_string(f.access.users.lock().unwrap().paths("A").unwrap().memory).unwrap();
    assert!(text.contains("shared private note"));
    assert!(
        !std::fs::read_to_string(f.access.users.lock().unwrap().paths("B").unwrap().memory)
            .unwrap()
            .contains("shared private note")
    );
    server.shutdown();
}

#[test]
fn mu5_attachment_refresh_duplicate_takeover_and_checkpoint_cas() {
    let f = Fixture::new();
    let w = f.create(&f.a, &f.x, "one");
    let w = f.action(&f.a, &w, "one", "chat/new", json!({}));
    let refreshed = f.action(&f.a, &w, "one", "attach", json!({}));
    assert_eq!(refreshed, w);
    let fork = f.action(&f.a, &w, "two", "attach", json!({}));
    assert_ne!(fork["workspace_id"], w["workspace_id"]);
    assert_eq!(fork["selected_chat"], w["selected_chat"]);
    let moved = f.action(&f.a, &fork, "two", "checkpoint", json!({"top_lid":"1.18"}));
    assert_eq!(f.get(&f.a, &w)["reader"], w["reader"]);
    let conflict = f.call(
        &f.a,
        "POST",
        &format!(
            "/api/workspaces/{}/checkpoint",
            fork["workspace_id"].as_str().unwrap()
        ),
        Fixture::stamp(&fork, "two"),
    );
    assert_eq!(conflict.1["error_code"], "WORKSPACE_REVISION_CONFLICT");
    let taken = f.action(&f.a, &w, "three", "takeover", json!({}));
    assert!(taken["generation"].as_u64() > w["generation"].as_u64());
    for action in ["reader/state", "reader/scroll", "checkpoint"] {
        let mut input = Fixture::stamp(&w, "one");
        input["delta"] = json!(1);
        let r = f.call(
            &f.a,
            "POST",
            &format!(
                "/api/workspaces/{}/{action}",
                w["workspace_id"].as_str().unwrap()
            ),
            input,
        );
        assert_eq!(r.1["error_code"], "WORKSPACE_STALE");
    }
    let detached = f.action(&f.a, &moved, "two", "detach", json!({}));
    let reloaded = f.action(&f.a, &detached, "refresh", "attach", json!({}));
    assert_eq!(reloaded["workspace_id"], fork["workspace_id"]);
    assert_eq!(reloaded["selected_chat"], fork["selected_chat"]);
    assert_eq!(reloaded["reader"]["viewport"], moved["reader"]["viewport"]);
    let chats = f.ok(&f.a, "GET", "/api/me/chats", json!({}));
    assert_eq!(chats["sessions"].as_array().unwrap().len(), 1);
}

#[test]
fn mu5_old_run_keeps_original_private_scope_but_reader_stales_and_revocation_stops_access() {
    let f = Fixture::new();
    let w = f.create(&f.a, &f.x, "one");
    let w = f.action(&f.a, &w, "one", "chat/new", json!({}));
    let mut port = f.run(&w, "one");
    assert!(port.read_live_reader(|r| r.revision()).is_ok());
    let w2 = f.action(
        &f.a,
        &w,
        "one",
        "book/open",
        json!({"published_book_ref":f.y}),
    );
    assert_eq!(
        port.read_live_reader(|_| panic!("must not read new Reader"))
            .unwrap_err()
            .error_code,
        "WORKSPACE_STALE"
    );
    assert_eq!(
        port.apply_reader(|_, _| panic!("must not apply to new Reader"))
            .unwrap_err()
            .error_code,
        "WORKSPACE_STALE"
    );
    let original = port.scope.book.clone();
    port.submit_private(|store| {
        Reader::save_note(
            &original,
            store,
            "1.1",
            "original material",
            "long_term",
            "now",
        )
    })
    .unwrap()
    .unwrap();
    assert_eq!(port.scope.publication.as_ref().unwrap().reference, f.x);
    assert_eq!(f.get(&f.a, &w2)["reader"], w2["reader"]);
    let user = f.access.users.lock().unwrap().get("A", "now").unwrap();
    assert!(port.scope.check_user(&user.lock().unwrap()).is_ok());
    f.access.library.lock().unwrap().revoke("A", &f.x).unwrap();
    assert!(port.submit_private(|_| panic!("revoked access")).is_err());
    assert!(port.read_live_reader(|_| ()).is_err());
}

#[test]
fn mu5_tutor_shared_revision_and_old_teaching_control_are_enforced() {
    let f = Fixture::new();
    let w = f.create(&f.a, &f.x, "one");
    let w = f.action(&f.a, &w, "one", "chat/new", json!({}));
    let a2 = f.create(&f.a, &f.y, "two");
    let enabled = f.ok(&f.a,"POST","/api/me/tutor/mutate",json!({"operation_id":"enable","expected_revision":0,"action":{"kind":"set_enabled","enabled":true}}));
    assert_eq!(enabled["control"]["revision"], 1);
    assert_eq!(f.call(&f.a,"POST","/api/tutor/mutate",json!({"operation_id":"loser","expected_revision":0,"action":{"kind":"set_enabled","enabled":false}})).0,409);
    let mut port = f.run(&w, "one");
    {
        let user = f.access.users.lock().unwrap().get("A", "now").unwrap();
        let user = user.lock().unwrap();
        let private = port.scope.private_user(&user);
        teaching::start_request(&private, "now").unwrap();
        teaching::prepare(
            &private,
            &AgentTurnRef {
                session_id: port.scope.chat_session_id.clone(),
                turn_id: port.scope.turn_id.clone(),
                user_turn_ordinal: 1,
            },
            &json!({"message":"Teach this"}),
            "now",
        )
        .unwrap()
        .unwrap();
    }
    assert!(port.tutor_active().unwrap());
    let control = f.ok(&f.a, "GET", "/api/tutor/state", json!({}));
    let disabled = f.ok(&f.a,"POST","/api/tutor/mutate",json!({"operation_id":"disable-from-two","expected_revision":control["control"]["revision"],"action":{"kind":"set_enabled","enabled":false}}));
    assert!(!port.tutor_active().unwrap());
    assert!(port
        .tutor_step(
            json!({"operation":"material","object_id":"speed"}),
            &[],
            &[]
        )
        .is_err());
    f.ok(&f.a,"POST","/api/tutor/mutate",json!({"operation_id":"re-enable","expected_revision":disabled["control"]["revision"],"action":{"kind":"set_enabled","enabled":true}}));
    assert!(
        !port.tutor_active().unwrap(),
        "re-enable cannot revive old control binding"
    );
    assert_eq!(f.get(&f.a, &a2)["published_book_ref"], json!(f.y));
    assert_eq!(
        f.ok(&f.b, "GET", "/api/tutor/state", json!({}))["control"]["revision"],
        0
    );
}

#[test]
fn mu5_capacity_idle_reload_pins_and_continue_reading_use_server_sequence() {
    let f = Fixture::new();
    let first = f.create(&f.a, &f.x, "one");
    let first = f.action(
        &f.a,
        &first,
        "one",
        "checkpoint",
        json!({"top_lid":"1.17","client_time":999999999999i64}),
    );
    let second = f.create(&f.a, &f.x, "two");
    assert_eq!(second["reader"]["viewport"], first["reader"]["viewport"]);
    let second = f.action(
        &f.a,
        &second,
        "two",
        "checkpoint",
        json!({"top_lid":"1.5","client_time":1}),
    );
    let third = f.create(&f.a, &f.x, "three");
    assert_eq!(third["reader"]["viewport"], second["reader"]["viewport"]);
    let fourth = f.create(&f.a, &f.y, "four");
    let rejected = f.call(
        &f.a,
        "POST",
        "/api/workspaces",
        json!({"published_book_ref":f.x,"attachment_id":"five"}),
    );
    assert_eq!(rejected.1["error_code"], "WORKSPACE_CAPACITY");
    let fourth = f.action(&f.a, &fourth, "four", "chat/new", json!({}));
    let pin = f.run(&fourth, "four");
    assert_eq!(
        f.access
            .workspaces
            .lock()
            .unwrap()
            .evict_idle(std::time::Instant::now() + WORKSPACE_IDLE_TTL + Duration::from_secs(1))
            .unwrap(),
        3
    );
    f.access.library.lock().unwrap().evict_cache();
    assert_eq!(f.access.library.lock().unwrap().resident_usage().0, 1);
    let restored = f.action(&f.a, &first, "new-instance", "attach", json!({}));
    assert_eq!(restored["reader"]["viewport"], first["reader"]["viewport"]);
    assert!(restored["generation"].as_u64() > first["generation"].as_u64());
    assert_eq!(restored["published_book_ref"], json!(f.x));
    drop(pin);
}

#[test]
fn mu5_history_order_is_identical_before_and_after_selecting_a_chat() {
    let f = Fixture::new();
    let w = f.create(&f.a, &f.x, "history");
    let first = f.action(&f.a, &w, "history", "chat/new", json!({}));
    let second = f.action(&f.a, &first, "history", "chat/new", json!({}));
    let selected = f.action(&f.a, &second, "history", "chat/history", json!({}));
    let empty = f.create(&f.a, &f.x, "empty");
    assert!(empty["selected_chat"].is_null());
    let unselected = f.action(&f.a, &empty, "empty", "chat/history", json!({}));
    assert_eq!(selected["result"]["sessions"][0]["id"], second["selected_chat"]);
    assert_eq!(unselected["result"]["sessions"], selected["result"]["sessions"]);
}

#[test]
fn mu5_idle_history_requires_reattachment_even_after_refreshing_the_generation() {
    let f = Fixture::new();
    let w = f.create(&f.a, &f.x, "idle-page");
    let w = f.action(&f.a, &w, "idle-page", "chat/new", json!({}));
    f.access.workspaces.lock().unwrap()
        .evict_idle(std::time::Instant::now() + WORKSPACE_IDLE_TTL + Duration::from_secs(1)).unwrap();
    let cold = f.get(&f.a, &w);
    let path = format!("/api/workspaces/{}/chat/history", w["workspace_id"].as_str().unwrap());
    let stale = f.call(&f.a, "POST", &path, Fixture::stamp(&cold, "idle-page"));
    assert_eq!(stale.0, 409);
    assert_eq!(stale.1["error_code"], "WORKSPACE_STALE");
    assert_eq!(stale.1["category"], "conflict");
    let attached = f.action(&f.a, &cold, "idle-page", "attach", json!({}));
    let history = f.action(&f.a, &attached, "idle-page", "chat/history", json!({}));
    assert_eq!(history["result"]["active_session_id"], w["selected_chat"]);
}

#[test]
fn mu5_checkpoint_failure_unknown_version_and_deleted_chat_reconcile() {
    let f = Fixture::new();
    let w = f.create(&f.a, &f.x, "one");
    let w = f.action(&f.a, &w, "one", "chat/new", json!({}));
    f.control.connection.execute_batch("CREATE TRIGGER fail_checkpoint BEFORE UPDATE ON reader_workspaces BEGIN SELECT RAISE(ABORT,'fixture checkpoint failure'); END;").unwrap();
    let mut input = Fixture::stamp(&w, "one");
    input["lid"] = json!("1.22");
    let r = f.call(
        &f.a,
        "POST",
        &format!(
            "/api/workspaces/{}/reader/goto",
            w["workspace_id"].as_str().unwrap()
        ),
        input,
    );
    assert_eq!(r.0, 503);
    assert_eq!(f.get(&f.a, &w), w, "failed commit changed visible Reader");
    f.control
        .connection
        .execute_batch("DROP TRIGGER fail_checkpoint;")
        .unwrap();
    let user = f.access.users.lock().unwrap().get("A", "now").unwrap();
    {
        let mut user = user.lock().unwrap();
        let user = &mut *user;
        let id = w["selected_chat"].as_str().unwrap();
        user.session_store.as_mut().unwrap().delete(&mut user.agent_history, id).unwrap();
    }
    let reconciled = f.get(&f.a, &w);
    assert!(reconciled["selected_chat"].is_null());
    assert!(reconciled["generation"].as_u64() > w["generation"].as_u64());
    f.control.connection.execute("UPDATE reader_workspaces SET checkpoint=json_set(checkpoint,'$.version',99) WHERE workspace_id=?",[w["workspace_id"].as_str().unwrap()]).unwrap();
    assert_eq!(
        f.call(
            &f.a,
            "GET",
            &format!("/api/workspaces/{}", w["workspace_id"].as_str().unwrap()),
            json!({})
        )
        .1["error_code"],
        "WORKSPACE_CHECKPOINT_INCOMPATIBLE"
    );
}

#[test]
fn mu5_live_reader_observes_user_navigation_but_late_run_effect_does_not_override_it() {
    let f = Fixture::new();
    let w = f.create(&f.a, &f.x, "one");
    let w = f.action(&f.a, &w, "one", "chat/new", json!({}));
    let mut port = f.run(&w, "one");
    let book = port.scope.book.clone();
    port.apply_reader(|s, r| r.goto_lid(&book, s, "1.5", "now"))
        .unwrap()
        .unwrap();
    let w = f.get(&f.a, &w);
    let moved = f.action(&f.a, &w, "one", "reader/goto", json!({"lid":"1.20"}));
    assert_eq!(
        port.read_live_reader(|r| json!(r.viewport())).unwrap(),
        moved["reader"]["viewport"]
    );
    let late = port.apply_reader(|s, r| r.goto_lid(&book, s, "1.1", "now"));
    assert!(matches!(late,Err(e) if e.error_code == "READER_USER_ACTION_SUPERSEDED"));
    assert_eq!(f.get(&f.a, &moved)["reader"], moved["reader"]);
}

#[test]
fn rs5_resumption_reads_committed_scene_without_warming_or_writing_it() {
    let f = Fixture::new();
    let w = f.create(&f.a, &f.x, "original");
    let w = f.action(&f.a, &w, "original", "chat/new", json!({}));
    let w = f.action(&f.a, &w, "original", "checkpoint", json!({"top_lid":"1.16"}));
    let port = f.run(&w, "original");
    drop(port);
    let other = f.create(&f.a, &f.y, "other-window");
    let path = format!("/api/workspaces/{}/resumption", w["workspace_id"].as_str().unwrap());
    let saved = || f.control.connection.query_row(
        "SELECT generation,revision,checkpoint,checkpoint_seq FROM reader_workspaces WHERE workspace_id=?",
        [w["workspace_id"].as_str().unwrap()], |r| Ok((r.get::<_, u64>(0)?,r.get::<_, u64>(1)?,r.get::<_, String>(2)?,r.get::<_, u64>(3)?))
    ).unwrap();
    let before = saved();
    f.access.workspaces.lock().unwrap().evict_idle(std::time::Instant::now() + WORKSPACE_IDLE_TTL + Duration::from_secs(1)).unwrap();
    f.access.library.lock().unwrap().evict_cache();
    let summary = f.ok(&f.a, "GET", &path, json!({}));
    assert_eq!(summary["published_book_ref"], w["published_book_ref"]);
    assert_eq!(summary["selected_chat"], w["selected_chat"]);
    assert_eq!(summary["last_question"], "Explain this");
    assert!(summary["position_excerpt"].as_str().is_some_and(|s| !s.is_empty()));
    assert_eq!(saved(), before, "preview must not advance even a cold workspace");
    assert_eq!(f.get(&f.a, &other)["published_book_ref"], json!(f.y));
    assert_ne!(f.call(&f.b, "GET", &path, json!({})).0, 200);
    f.access.library.lock().unwrap().revoke("A", &f.x).unwrap();
    assert_ne!(f.call(&f.a, "GET", &path, json!({})).0, 200);
}

#[test]
fn rs5_resumption_without_questions_and_after_chat_deletion_keeps_position() {
    let f = Fixture::new();
    let w = f.create(&f.a, &f.x, "original");
    let path = format!("/api/workspaces/{}/resumption", w["workspace_id"].as_str().unwrap());
    let empty = f.ok(&f.a, "GET", &path, json!({}));
    assert!(empty["last_question"].is_null());
    assert!(empty["selected_chat"].is_null());
    let w = f.action(&f.a, &w, "original", "chat/new", json!({}));
    let port = f.run(&w, "original");
    {
        let handle = f.access.users.lock().unwrap().get("A", "now").unwrap();
        let mut user = handle.lock().unwrap();
        let mut session = user.agent_history.sessions.iter().find(|s| s.id == port.scope.chat_session_id).unwrap().clone();
        session.turns[0].status = AgentAssistantStatus::Cancelled;
        session.turns[0].error = Some(AgentTurnError { error_code: "CANCELLED".into(), category: "cancelled".into(), message: "Stopped by reader".into() });
        let turn = AgentTurnRef { session_id: session.id.clone(), turn_id: session.turns[0].turn_id.clone(), user_turn_ordinal: 1 };
        crate::session_runtime::finish(&mut user, &turn, &session, "finished").unwrap();
    }
    drop(port);
    f.ok(&f.a, "POST", "/api/agent/history/delete", json!({"session_id":w["selected_chat"]}));
    let deleted = f.ok(&f.a, "GET", &path, json!({}));
    assert!(deleted["last_question"].is_null());
    assert!(deleted["selected_chat"].is_null());
    assert_eq!(deleted["position_excerpt"], empty["position_excerpt"]);
}

#[test]
fn mu5_restart_restores_exact_publication_chat_position_and_layout() {
    let f = Fixture::new();
    let w = f.create(&f.a, &f.x, "old");
    let w = f.action(&f.a, &w, "old", "chat/new", json!({}));
    let w = f.action(&f.a, &w, "old", "checkpoint", json!({"top_lid":"1.16"}));
    let w = f.action(&f.a,&w,"old","reader/layout.apply",json!({"actions":[{"kind":"open_slot","slot_id":"technical.evidence","region":"right"},{"kind":"focus_slot","slot_id":"technical.evidence"}]}));
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
    let users = UserRegistry::open(root.path()).unwrap();
    let control = ControlStore::open(users.writer()).unwrap();
    let f = Fixture {
        root,
        access: Arc::new(Authorization::new(users).unwrap()),
        control,
        x,
        y,
        a,
        b,
    };
    let summary = f.ok(&f.a, "GET", &format!("/api/workspaces/{}/resumption", w["workspace_id"].as_str().unwrap()), json!({}));
    assert_eq!(summary["published_book_ref"], w["published_book_ref"]);
    assert_eq!(summary["selected_chat"], w["selected_chat"]);
    // The original stamp must still work after a read-only preview across service restart.
    let restored = f.action(&f.a, &w, "new-page", "attach", json!({}));
    assert_eq!(restored["workspace_id"], w["workspace_id"]);
    assert_eq!(restored["selected_chat"], w["selected_chat"]);
    assert_eq!(restored["published_book_ref"], w["published_book_ref"]);
    assert_eq!(restored["reader"]["viewport"], w["reader"]["viewport"]);
    assert_eq!(restored["reader"]["layout"], w["reader"]["layout"]);
    assert!(restored["generation"].as_u64() > w["generation"].as_u64());
    assert_eq!(
        f.ok(&f.a, "GET", "/api/me/chats", json!({}))["sessions"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn mu5_presentation_linked_view_keeps_chat_and_late_restore_cannot_replace_user_state() {
    use runtime::presentation::*;
    let f = Fixture::new();
    let w = f.create(&f.a, &f.x, "parent");
    let w = f.action(&f.a, &w, "parent", "chat/new", json!({}));
    let port = f.run(&w, "parent");
    let reference = PresentationRef {
        presentation_id: "mu5-presentation".into(),
        revision: 1,
    };
    {
        let handle = f.access.users.lock().unwrap().get("A", "now").unwrap();
        let mut user = handle.lock().unwrap();
        let mut session = user
            .agent_history
            .sessions
            .iter()
            .find(|s| s.id == port.scope.chat_session_id)
            .unwrap().clone();
        session.turns[0].status = AgentAssistantStatus::Completed;
        session.turns[0].outcome = Some(OuterOutcome {
            answer: Some("A simple scene".into()),
            answer_view: Some(AgentAnswerView {
                parts: vec![AgentAnswerPart::Presentation {
                    presentation_id: reference.presentation_id.clone(),
                    revision: 1,
                }],
                sources: vec![],
            }),
            incomplete: false,
            warning: None,
            turns: 1,
            tokens_spent: 0,
            effects: vec![],
            trace: vec![],
            profile_usage: Default::default(),
            memory_updates: vec![],
            source_bindings: vec![],
            delivery_diagnostics: None,
            request_audit: Default::default(),
        });
        let version = AgentPresentation {
            reference: reference.clone(),
            candidate_id: "candidate".into(),
            owner: PresentationOwner {
                book_id: f.x.book_id.clone(),
                session_id: port.scope.chat_session_id.clone(),
            },
            created_by_turn_id: port.scope.turn_id.clone(),
            based_on: None,
            content: PresentationContent {
                animation_assets: Default::default(),
                title: "A scene".into(),
                content_files: BTreeMap::from([("index.html".into(), "<p>Scene</p>".into())]),
                entrypoint: "index.html".into(),
                readable_content: "A simple scene".into(),
                source_bindings: vec![],
                assumptions: vec![],
                state_contract: json!({}),
                initial_state: json!({}),
            },
        };
        let path = user
            .presentation_root()
            .unwrap()
            .join("versions/mu5-presentation/1.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, serde_json::to_vec(&version).unwrap()).unwrap();
        let turn = AgentTurnRef { session_id: session.id.clone(), turn_id: session.turns[0].turn_id.clone(), user_turn_ordinal: 1 };
        crate::session_runtime::finish(&mut user, &turn, &session, "finished").unwrap();
    }
    let request = json!({"session_id":port.scope.chat_session_id,"turn_id":port.scope.turn_id,"reference":reference});
    let linked = f.action(&f.a, &w, "parent", "linked/attach", request.clone());
    assert_eq!(linked["workspace_id"], w["workspace_id"]);
    assert_eq!(linked["selected_chat"], w["selected_chat"]);
    let mut save = request.clone();
    save["state"] = json!({"values":{"slider":1},"visible_step":null,"observed_result":"One","source_ref_ids":[]});
    let saved = f.action(
        &f.a,
        &w,
        linked["attachment_id"].as_str().unwrap(),
        "presentation/save",
        save.clone(),
    );
    let frozen: PresentationFollowUp =
        serde_json::from_value(saved["presentation"].clone()).unwrap();
    // Start another Run with the exact saved state; its old receipt survives later browser changes.
    drop(port);
    let mut port = f.run(&saved, "parent");
    assert_eq!(port.scope.presentation_follow_up.as_ref(), Some(&frozen));
    port.restore_presentation(frozen.clone()).unwrap();
    let saved = f.get(&f.a, &saved);
    save["state"]["values"]["slider"] = json!(9);
    let current = f.action(&f.a, &saved, "parent", "presentation/save", save);
    assert_eq!(
        port.restore_presentation(frozen.clone())
            .unwrap_err()
            .error_code,
        "READER_USER_ACTION_SUPERSEDED"
    );
    assert_eq!(
        f.get(&f.a, &current)["presentation"],
        current["presentation"]
    );
    let restored = f.action(
        &f.a,
        &current,
        "parent",
        "presentation/restore",
        json!({"saved_state":frozen}),
    );
    assert_eq!(restored["presentation"], json!(frozen));
    let fork = f.action(&f.a, &restored, "other-window", "fork", json!({}));
    let _ = f.action(
        &f.a,
        &fork,
        "other-window",
        "checkpoint",
        json!({"top_lid":"1.10"}),
    );
    let taken = f.action(&f.a, &restored, "new-parent", "takeover", json!({}));
    let stale = f.call(
        &f.a,
        "POST",
        &format!(
            "/api/workspaces/{}/reader/state",
            w["workspace_id"].as_str().unwrap()
        ),
        Fixture::stamp(&restored, linked["attachment_id"].as_str().unwrap()),
    );
    assert_eq!(stale.1["error_code"], "WORKSPACE_STALE");
    assert_eq!(taken["selected_chat"], w["selected_chat"]);
    assert_eq!(
        f.ok(&f.a, "GET", "/api/me/chats", json!({}))["sessions"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let dir = f
        .access
        .library
        .lock()
        .unwrap()
        .load("A", &f.x)
        .unwrap()
        .directory()
        .to_path_buf();
    let next = f
        .access
        .library
        .lock()
        .unwrap()
        .publish(&dir)
        .unwrap()
        .reference;
    f.access.library.lock().unwrap().grant("A", &next).unwrap();
    let next_w = f.action(
        &f.a,
        &taken,
        "new-parent",
        "book/open",
        json!({"published_book_ref":next}),
    );
    let next_w = f.action(
        &f.a,
        &next_w,
        "new-parent",
        "chat/select",
        json!({"session_id":w["selected_chat"]}),
    );
    let mut wrong = Fixture::stamp(&next_w, "new-parent");
    wrong["saved_state"] = json!(frozen);
    let rejected = f.call(
        &f.a,
        "POST",
        &format!(
            "/api/workspaces/{}/presentation/restore",
            w["workspace_id"].as_str().unwrap()
        ),
        wrong,
    );
    assert_eq!(rejected.1["error_code"], "PUBLICATION_BINDING_MISMATCH");
}

#[test]
fn mu5_schema_two_upgrade_keeps_owner_rows_and_workspace_book_budget_is_shared() {
    let root = tempfile::tempdir().unwrap();
    let writer = crate::control_store::ServiceWriter::acquire(root.path()).unwrap();
    let connection = rusqlite::Connection::open(root.path().join("control.sqlite")).unwrap();
    connection
        .execute_batch(include_str!("control_schema_v4.sql"))
        .unwrap();
    connection
        .pragma_update(None, "application_id", 0x55424d55_i64)
        .unwrap();
    let mut control = ControlStore {
        connection,
        writer: writer.clone(),
    };
    control.create_user("prior").unwrap();
    control.connection.execute_batch("INSERT INTO reader_workspaces(owner_user_id,workspace_id) VALUES('prior','prior-scene'); ALTER TABLE reader_workspaces DROP COLUMN checkpoint_seq; ALTER TABLE run_admissions DROP COLUMN cancel_requested; ALTER TABLE run_admissions DROP COLUMN key_closed; ALTER TABLE run_admissions DROP COLUMN unsaved; PRAGMA user_version=2;").unwrap();
    drop(control);
    let control = ControlStore::open(writer).unwrap();
    assert_eq!(
        control
            .connection
            .pragma_query_value(None, "user_version", |r| r.get::<_, u64>(0))
            .unwrap(),
        crate::control_store::CONTROL_SCHEMA_VERSION as u64
    );
    assert_eq!(
        control
            .connection
            .query_row(
                "SELECT count(*) FROM reader_workspaces WHERE owner_user_id='prior'",
                [],
                |r| r.get::<_, u64>(0)
            )
            .unwrap(),
        1
    );
    let f = Fixture::new();
    *f.access.library.lock().unwrap() = crate::published_library::PublishedLibrary::with_budget(
        ControlStore::open(f.access.users.lock().unwrap().writer()).unwrap(),
        1,
        2 * 1024 * 1024 * 1024,
    );
    let w = f.create(&f.a, &f.x, "one");
    f.access.library.lock().unwrap().evict_cache();
    assert_eq!(
        f.call(
            &f.a,
            "POST",
            "/api/workspaces",
            json!({"published_book_ref":f.y,"attachment_id":"two"})
        )
        .1["error_code"],
        "BOOK_CAPACITY"
    );
    let _ = f.action(&f.a, &w, "one", "detach", json!({}));
    let next = f.create(&f.a, &f.y, "two");
    assert_eq!(next["published_book_ref"], json!(f.y));
}

#[test]
fn adm1_admin_identity_does_not_bypass_reader_private_or_material_scope() {
    let mut f = Fixture::new();
    f.control.set_reader_admin("A", true).unwrap();
    let now = multi_user_host::now();
    let admin = f.access.auth.authenticate(&f.a, now).unwrap();
    let reader = f.access.auth.authenticate(&f.b, now).unwrap();
    f.access.auth.require_reader_admin(&admin, now).unwrap();
    assert_eq!(
        f.access.auth.require_reader_admin(&reader, now).unwrap_err().error_code,
        "ADMIN_REQUIRED"
    );
    let w = f.create(&f.b, &f.x, "reader-page");
    let w = f.action(&f.b, &w, "reader-page", "chat/new", json!({}));
    let chat_path = format!("/api/me/chats/{}", w["selected_chat"].as_str().unwrap());
    assert_eq!(f.call(&f.b, "GET", &chat_path, json!({})).0, 200);
    assert_eq!(f.call(&f.a, "GET", &chat_path, json!({})).0, 404);
    let workspace_path = format!("/api/workspaces/{}", w["workspace_id"].as_str().unwrap());
    assert_eq!(f.call(&f.a, "GET", &workspace_path, json!({})).0, 404);
    f.access.library.lock().unwrap().revoke("A", &f.x).unwrap();
    let book_path = format!("/api/books/{}/publications/{}/manifest", f.x.book_id, f.x.publication_id);
    assert_eq!(f.call(&f.b, "GET", &book_path, json!({})).0, 200);
    assert_eq!(f.call(&f.a, "GET", &book_path, json!({})).0, 404);
}

#[test]
fn mu5_global_capacity_and_admission_pin_prevent_idle_eviction() {
    let mut f = Fixture::new();
    let mut tokens = vec![f.a.clone(), f.b.clone()];
    for owner in ["C", "D", "E", "F"] {
        f.control
            .provision_password(owner, "fixture-only-password", true)
            .unwrap();
        f.access.library.lock().unwrap().grant(owner, &f.x).unwrap();
        tokens.push(
            f.access
                .auth
                .login(owner, "fixture-only-password", None, multi_user_host::now())
                .unwrap(),
        );
    }
    let mut first = Value::Null;
    for token in &tokens[..5] {
        for n in 0..4 {
            let w = f.create(token, &f.x, &format!("page-{n}"));
            if first.is_null() {
                first = w;
            }
        }
    }
    let rejected = f.call(
        &tokens[5],
        "POST",
        "/api/workspaces",
        json!({"published_book_ref":f.x,"attachment_id":"page"}),
    );
    assert_eq!(rejected.1["error_code"], "WORKSPACE_CAPACITY");
    f.control.connection.execute("INSERT INTO run_admissions(owner_user_id,client_request_id,turn_id,chat_session_id,workspace_id,workspace_generation,book_id,publication_id,dispatch_state) VALUES('A','request','turn','chat',?,?,?,?,'queued')",rusqlite::params![first["workspace_id"].as_str().unwrap(),first["generation"].as_u64().unwrap(),f.x.book_id,f.x.publication_id]).unwrap();
    assert_eq!(
        f.access
            .workspaces
            .lock()
            .unwrap()
            .evict_idle(std::time::Instant::now() + WORKSPACE_IDLE_TTL + Duration::from_secs(1))
            .unwrap(),
        19
    );
    assert_eq!(f.get(&f.a, &first)["generation"], first["generation"]);
    let fresh = f.create(&tokens[5], &f.x, "page");
    assert!(fresh["workspace_id"].is_string());
}

#[test]
fn rn2_network_notes_keep_private_access_after_chat_deletion_and_restart() {
    use runtime::presentation::*;
    let f = Fixture::new();
    let w = f.create(&f.a, &f.x, "parent");
    let w = f.action(&f.a, &w, "parent", "chat/new", json!({}));
    let port = f.run(&w, "parent");
    let reference = PresentationRef {
        presentation_id: "mu5-presentation".into(),
        revision: 1,
    };
    {
        let handle = f.access.users.lock().unwrap().get("A", "now").unwrap();
        let mut user = handle.lock().unwrap();
        let mut session = user
            .agent_history
            .sessions
            .iter()
            .find(|s| s.id == port.scope.chat_session_id)
            .unwrap().clone();
        session.turns[0].status = AgentAssistantStatus::Completed;
        session.turns[0].outcome = Some(OuterOutcome {
            answer: Some("A simple scene".into()),
            answer_view: Some(AgentAnswerView {
                parts: vec![AgentAnswerPart::Presentation {
                    presentation_id: reference.presentation_id.clone(),
                    revision: 1,
                }],
                sources: vec![],
            }),
            incomplete: false,
            warning: None,
            turns: 1,
            tokens_spent: 0,
            effects: vec![],
            trace: vec![],
            profile_usage: Default::default(),
            memory_updates: vec![],
            source_bindings: vec![],
            delivery_diagnostics: None,
            request_audit: Default::default(),
        });
        let version = AgentPresentation {
            reference: reference.clone(),
            candidate_id: "candidate".into(),
            owner: PresentationOwner {
                book_id: f.x.book_id.clone(),
                session_id: port.scope.chat_session_id.clone(),
            },
            created_by_turn_id: port.scope.turn_id.clone(),
            based_on: None,
            content: PresentationContent {
                animation_assets: Default::default(),
                title: "A scene".into(),
                content_files: BTreeMap::from([("index.html".into(), "<p>Scene</p>".into())]),
                entrypoint: "index.html".into(),
                readable_content: "A simple scene".into(),
                source_bindings: vec![],
                assumptions: vec![],
                state_contract: json!({}),
                initial_state: json!({}),
            },
        };
        let path = user
            .presentation_root()
            .unwrap()
            .join("versions/mu5-presentation/1.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, serde_json::to_vec(&version).unwrap()).unwrap();
        let turn = AgentTurnRef { session_id: session.id.clone(), turn_id: session.turns[0].turn_id.clone(), user_turn_ordinal: 1 };
        crate::session_runtime::finish(&mut user, &turn, &session, "finished").unwrap();
    }
    let request = json!({"session_id":port.scope.chat_session_id,"turn_id":port.scope.turn_id,"reference":reference});

    let mut save = request.clone();
    save["state"] = json!({"values":{"slider":2},"visible_step":"recorded","observed_result":"Two","source_ref_ids":[]});
    let w = f.action(&f.a, &w, "parent", "presentation/save", save);
    let receipt = w["presentation"].clone();
    let w = f.action(&f.a, &w, "parent", "memory/save", json!({"type":"note","content":"My note","note":{"association":{"kind":"presentation","receipt":receipt}}}));
    let id = w["result"]["record"]["mem_id"].as_str().unwrap().to_string();
    drop(port);
    let chat_path = format!("/api/me/chats/{}", w["selected_chat"].as_str().unwrap());
    assert_eq!(f.call(&f.a, "DELETE", &chat_path, json!({})).0, 200);
    let Fixture { root, access, control, x, y, a, b } = f;
    drop(access); drop(control);
    let users = UserRegistry::open(root.path()).unwrap();
    let control = ControlStore::open(users.writer()).unwrap();
    let f = Fixture { root, access: Arc::new(Authorization::new(users).unwrap()), control, x, y, a, b };
    let w = f.get(&f.a, &w);
    let w = f.action(&f.a, &w, "reopened", "attach", json!({}));
    let read = json!({"mem_id":id,"restore":true});
    let restored = f.action(&f.a, &w, "reopened", "memory/presentation.read", read.clone());
    assert_eq!(restored["result"]["restored_state"]["values"]["slider"], 2);
    assert_eq!(restored["result"]["reference"], json!(reference));
    let other = f.create(&f.b, &f.x, "other");
    let mut input = Fixture::stamp(&other, "other"); input["mem_id"] = json!(id);
    assert_ne!(f.call(&f.b, "POST", &format!("/api/workspaces/{}/memory/presentation.read", other["workspace_id"].as_str().unwrap()), input).0, 200);
    let wrong = f.create(&f.a, &f.y, "wrong");
    let mut input = Fixture::stamp(&wrong, "wrong"); input["mem_id"] = json!(id);
    assert_ne!(f.call(&f.a, "POST", &format!("/api/workspaces/{}/memory/presentation.read", wrong["workspace_id"].as_str().unwrap()), input).0, 200);
    f.access.library.lock().unwrap().revoke("A", &f.x).unwrap();
    let mut input = Fixture::stamp(&w, "reopened"); input["mem_id"] = json!(id);
    assert_ne!(f.call(&f.a, "POST", &format!("/api/workspaces/{}/memory/presentation.read", w["workspace_id"].as_str().unwrap()), input).0, 200);
}
