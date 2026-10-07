use super::*;
use crate::{
    auth::{AuthService, COOKIE, SESSION_SECONDS},
    authorization::Authorization,
    control_store::ControlStore,
    multi_user_host::{self, Headers, Site},
    published_library::PublishedBookRef,
    user_registry::UserRegistry,
};

const PASSWORD: &str = "fixture-only-password";
const ORIGIN: &str = "https://reader.example";
struct Fixture {
    _root: tempfile::TempDir,
    access: Arc<Authorization>,
    control: ControlStore,
    reference: PublishedBookRef,
}
impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let registry = UserRegistry::open(root.path()).unwrap();
        let mut control = ControlStore::open(registry.writer()).unwrap();
        for id in ["A", "B"] {
            control.provision_password(id, PASSWORD, true).unwrap();
        }
        let access = Arc::new(Authorization::new(registry).unwrap());
        let input = tempfile::tempdir().unwrap();
        let mut base = sample_base();
        base.book_id = "mu4-material".into();
        std::fs::create_dir_all(input.path().join("assets/images")).unwrap();
        std::fs::write(
            input.path().join("base.json"),
            serde_json::to_vec(&base).unwrap(),
        )
        .unwrap();
        std::fs::write(input.path().join("source.txt"), "X".repeat(100) + "尾巴").unwrap();
        std::fs::write(
            input.path().join("assets/images/same.png"),
            b"material-image",
        )
        .unwrap();
        std::fs::write(input.path().join("asset_manifest.json"), json!({"version":"asset_manifest.v1","book_id":base.book_id,"images":[{"status":"available","stored_path":"assets/images/same.png","url_path":"/book/assets/images/same.png"}]}).to_string()).unwrap();
        let reference = access
            .library
            .lock()
            .unwrap()
            .publish(input.path())
            .unwrap()
            .reference;
        access
            .library
            .lock()
            .unwrap()
            .grant("A", &reference)
            .unwrap();
        let mut fixture = Self {
            _root: root,
            access,
            control,
            reference,
        };
        fixture.seed_private();
        fixture
    }
    fn seed_private(&mut self) {
        for id in ["A", "B"] {
            let handle = self.access.users.lock().unwrap().get(id, "now").unwrap();
            let mut user = handle.lock().unwrap();
            let turn: AgentChatTurn = serde_json::from_value(json!({"turn_id":format!("turn-{id}"),"user_turn_ordinal":1,"user":format!("{id}-private-question"),"status":"failed","error":{"error_code":"FIXTURE","category":"interrupted","message":"fixture"},"published_book_ref":self.reference,"question_anchor_lid":null,"question_quote":null})).unwrap();
            let mut session = new_agent_session(&self.reference.book_id, "now", 1);
            session.id = format!("chat-{id}");
            session.title = format!("{id}-private-title");
            session.turns.push(turn);
            user.agent_history.sessions.push(session);
            save_agent_history_path(&user.history_path, &user.agent_history).unwrap();
            self.control
                .connection
                .execute(
                    "INSERT INTO reader_workspaces(owner_user_id,workspace_id) VALUES(?,?)",
                    rusqlite::params![id, format!("workspace-{id}")],
                )
                .unwrap();
        }
    }
    fn login(&self, id: &str) -> String {
        self.access
            .auth
            .login(id, PASSWORD, None, multi_user_host::now())
            .unwrap()
    }
    fn headers(&self, token: Option<&str>, csrf: bool) -> Headers {
        let mut values = vec![
            ("Host".into(), "reader.example".into()),
            ("Origin".into(), ORIGIN.into()),
        ];
        if let Some(token) = token {
            values.push(("Cookie".into(), format!("{COOKIE}={token}")));
            if csrf {
                values.push(("X-CSRF-Token".into(), crate::auth::csrf(token)));
            }
        }
        Headers(values)
    }
    fn call(
        &self,
        token: Option<&str>,
        method: &str,
        url: &str,
        body: Value,
    ) -> multi_user_host::HttpReply {
        multi_user_host::dispatch(
            &self.access,
            &Site::new(ORIGIN).unwrap(),
            method,
            url,
            &self.headers(token, true),
            &body.to_string(),
            multi_user_host::now(),
        )
    }
    fn start(&self) -> multi_user_host::RunningMultiUserServer {
        multi_user_host::start_with_access(
            "127.0.0.1:0".parse().unwrap(),
            Arc::new(Site::new(ORIGIN).unwrap()),
            self.access.clone(),
        )
        .unwrap()
    }
}
fn body(reply: &multi_user_host::HttpReply) -> String {
    String::from_utf8_lossy(&reply.body).into_owned()
}
fn http(
    base: &str,
    method: &str,
    path: &str,
    cookie: Option<&str>,
    csrf: Option<&str>,
    payload: Option<Value>,
) -> ureq::Response {
    let mut req = ureq::request(method, &format!("{base}{path}"))
        .timeout(Duration::from_secs(10))
        .set("Host", "reader.example")
        .set("Origin", ORIGIN);
    if let Some(token) = cookie {
        req = req.set("Cookie", &format!("{COOKIE}={token}"));
    }
    if let Some(csrf) = csrf {
        req = req.set("X-CSRF-Token", csrf);
    }
    let result = match payload {
        Some(payload) => req.send_json(payload),
        None => req.call(),
    };
    match result {
        Ok(r) | Err(ureq::Error::Status(_, r)) => r,
        Err(e) => panic!("HTTP transport failed: {e}"),
    }
}

#[test]
fn mu4_real_host_login_cookie_rotation_csrf_logout_and_no_static_fallback() {
    let f = Fixture::new();
    let server = f.start();
    assert_eq!(
        http(&server.url, "GET", "/", None, None, None).status(),
        200
    );
    for path in [
        "/api/auth/me",
        "/api/library",
        "/unknown.json",
        "/users/B/memory/memory.json",
    ] {
        assert_eq!(
            http(&server.url, "GET", path, None, None, None).status(),
            401,
            "{path}"
        );
    }
    let login = http(
        &server.url,
        "POST",
        "/api/auth/login",
        None,
        None,
        Some(json!({"username":"A","password":PASSWORD})),
    );
    assert_eq!(login.status(), 200);
    let cookie = login.header("Set-Cookie").unwrap().to_string();
    for part in [
        "Secure",
        "HttpOnly",
        "SameSite=Lax",
        "Path=/",
        "Max-Age=43200",
    ] {
        assert!(cookie.contains(part));
    }
    assert!(!cookie.contains("Domain="));
    let token = crate::auth::token_from_cookie(&cookie).unwrap().to_string();
    let identity: Value = login.into_json().unwrap();
    assert_eq!(identity["user_id"], "A");
    assert_eq!(
        http(
            &server.url,
            "POST",
            "/api/auth/logout",
            Some(&token),
            None,
            Some(json!({}))
        )
        .status(),
        403
    );
    assert_eq!(
        http(&server.url, "GET", "/api/auth/me", Some(&token), None, None).status(),
        200
    );
    let rotated = http(
        &server.url,
        "POST",
        "/api/auth/login",
        Some(&token),
        None,
        Some(json!({"username":"A","password":PASSWORD})),
    );
    let next_cookie = rotated.header("Set-Cookie").unwrap().to_string();
    let next = crate::auth::token_from_cookie(&next_cookie).unwrap();
    assert_ne!(token, next);
    assert_eq!(
        http(&server.url, "GET", "/api/library", Some(&token), None, None).status(),
        401
    );
    let result = http(
        &server.url,
        "POST",
        "/api/auth/logout",
        Some(next),
        Some(&crate::auth::csrf(next)),
        Some(json!({})),
    );
    assert_eq!(result.status(), 200);
    assert!(result.header("Set-Cookie").unwrap().contains("Max-Age=0"));
    assert_eq!(
        http(&server.url, "GET", "/api/auth/me", Some(next), None, None).status(),
        401
    );
    server.shutdown();
}

#[test]
fn mu4_mu0_inventory_has_no_anonymous_or_legacy_bypass() {
    let f = Fixture::new();
    let server = f.start();
    let token = f.login("A");
    for line in
        include_str!("../../../../docs/performance/linux-multi-reader-mu0-20260930/routes.csv")
            .lines()
            .skip(1)
    {
        let mut fields = line.split(',');
        let path = fields.next().unwrap();
        let method = fields.next().unwrap();
        if !matches!(method, "GET" | "POST") || path.starts_with("/*") {
            continue;
        }
        let path = path
            .replace("{turn_id}", "turn-B")
            .replace("{relative_path}", "same.png");
        for url in [path.clone(), format!("/api{path}")] {
            assert_eq!(
                http(
                    &server.url,
                    method,
                    &url,
                    None,
                    None,
                    if method == "POST" {
                        Some(json!({}))
                    } else {
                        None
                    }
                )
                .status(),
                401,
                "{method} {url}"
            );
        }
    }
    for path in [
        "/book/open",
        "/book/create",
        "/book/query",
        "/book/synthesize",
        "/build_intent/usage.event",
        "/build_workbench/job.start",
        "/reader/selection.translate",
        "/reader/paper_minimap.localize",
        "/provider/config",
        "/agent/presentation.author",
    ] {
        for path in [path.to_string(), format!("/api{path}")] {
            assert_eq!(
                http(
                    &server.url,
                    "POST",
                    &path,
                    Some(&token),
                    Some(&crate::auth::csrf(&token)),
                    Some(json!({}))
                )
                .status(),
                403,
                "{path}"
            );
        }
    }
    assert_eq!(
        http(
            &server.url,
            "GET",
            "/observability/status",
            Some(&token),
            None,
            None
        )
        .status(),
        403
    );
    server.shutdown();
}

#[test]
fn mu4_identity_origin_csrf_and_directory_injection_fail_before_side_effects() {
    let f = Fixture::new();
    let token = f.login("A");
    let site = Site::new(ORIGIN).unwrap();
    let mut headers = f.headers(Some(&token), true);
    headers.0.extend([
        ("X-User-Id".into(), "B".into()),
        ("X-Forwarded-User".into(), "B".into()),
        ("X-Forwarded-Proto".into(), "https".into()),
    ]);
    let call = |headers: &Headers, method, url, body: &str| {
        multi_user_host::dispatch(
            &f.access,
            &site,
            method,
            url,
            headers,
            body,
            multi_user_host::now(),
        )
    };
    assert_eq!(
        serde_json::from_slice::<Value>(&call(&headers, "GET", "/api/auth/me", "").body).unwrap()
            ["user_id"],
        "A"
    );
    for payload in [
        json!({"user_id":"B"}),
        json!({"owner_user_id":"B"}),
        json!({"dir":"C:/private"}),
    ] {
        assert_eq!(
            call(&headers, "POST", "/api/workspaces", &payload.to_string()).status,
            400
        );
    }
    assert_eq!(
        call(&headers, "GET", "/api/library?user_id=B", "").status,
        400
    );
    assert_eq!(
        call(&headers, "GET", "/api/library?%64ir=private", "").status,
        400
    );
    for bad_origin in [None, Some("https://evil.example"), Some("null")] {
        let mut h = f.headers(Some(&token), true);
        h.0.retain(|(k, _)| k != "Origin");
        if let Some(value) = bad_origin {
            h.0.push(("Origin".into(), value.into()));
        }
        assert_eq!(call(&h, "POST", "/api/auth/logout", "{}").status, 403);
        assert_eq!(
            call(
                &h,
                "POST",
                "/api/auth/login",
                &json!({"username":"A","password":PASSWORD}).to_string()
            )
            .status,
            403
        );
    }
    let mut forged = f.headers(Some(&token), true);
    forged.0.retain(|(k, _)| k != "Host");
    forged.0.push(("Host".into(), "evil.example".into()));
    assert_eq!(call(&forged, "GET", "/api/library", "").status, 403);
    let mut duplicate = f.headers(Some(&token), true);
    duplicate
        .0
        .push(("Cookie".into(), format!("{COOKIE}={token}")));
    assert_eq!(call(&duplicate, "GET", "/api/library", "").status, 401);
    assert_eq!(
        f.call(Some(&token), "GET", "/api/auth/me", Value::Null)
            .status,
        200
    );
    assert_eq!(
        f.control
            .connection
            .query_row("SELECT count(*) FROM reader_workspaces", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        2
    );
}

#[test]
fn mu4_private_objects_are_owner_scoped_at_real_host() {
    let f = Fixture::new();
    let server = f.start();
    let a = f.login("A");
    let b = f.login("B");
    for token in [&a, &b] {
        let owner = if token == &a { "A" } else { "B" };
        let other = if owner == "A" { "B" } else { "A" };
        let own = http(
            &server.url,
            "GET",
            &format!("/api/me/chats/chat-{owner}"),
            Some(token),
            None,
            None,
        )
        .into_string()
        .unwrap();
        assert!(own.contains(&format!("{owner}-private-question")));
        assert!(!own.contains(&format!("{other}-private")));
        for path in [
            format!("/me/chats/chat-{other}"),
            format!("/workspaces/workspace-{other}"),
            format!("/agent/runs/turn-{other}"),
            format!("/agent/runs/turn-{other}/events"),
            "/agent/runs/missing/events".into(),
        ] {
            let response = http(
                &server.url,
                "GET",
                &format!("/api{path}"),
                Some(token),
                None,
                None,
            );
            assert_eq!(response.status(), 404, "{path}");
            assert!(!response.into_string().unwrap().contains("private"));
        }
        assert_eq!(
            http(
                &server.url,
                "POST",
                &format!("/api/agent/runs/turn-{other}/cancel"),
                Some(token),
                Some(&crate::auth::csrf(token)),
                Some(json!({}))
            )
            .status(),
            404
        );
        assert_eq!(
            http(
                &server.url,
                "POST",
                "/api/agent/history/delete",
                Some(token),
                Some(&crate::auth::csrf(token)),
                Some(json!({"session_id":format!("chat-{other}")}))
            )
            .status(),
            404
        );
        assert_eq!(http(&server.url,"POST","/api/agent/presentation.read",Some(token),Some(&crate::auth::csrf(token)),Some(json!({"session_id":format!("chat-{other}"),"turn_id":format!("turn-{other}"),"reference":{"presentation_id":"same","revision":1}}))).status(),404);
    }
    let events = http(
        &server.url,
        "GET",
        "/api/agent/runs/turn-A/events",
        Some(&a),
        None,
        None,
    );
    assert_eq!(events.status(), 200);
    assert_eq!(events.header("Content-Type"), Some("text/event-stream"));
    assert!(events.into_string().unwrap().contains("run.snapshot"));
    assert_eq!(
        http(
            &server.url,
            "POST",
            "/api/agent/runs/turn-A/cancel",
            Some(&a),
            Some(&crate::auth::csrf(&a)),
            Some(json!({}))
        )
        .status(),
        200
    );
    server.shutdown();
}

#[test]
fn mu4_resources_authorize_before_head_range_conditionals_cache_and_revoke() {
    let f = Fixture::new();
    let a = f.login("A");
    let b = f.login("B");
    let site = Site::new(ORIGIN).unwrap();
    for leaf in [
        "manifest",
        "text?lid=1.1",
        "assets/images/same.png",
        "pdf/original",
    ] {
        for method in ["GET", "HEAD"] {
            let mut h = f.headers(Some(&b), true);
            h.0.extend([
                ("Range".into(), "bytes=0-4".into()),
                ("If-None-Match".into(), "*".into()),
            ]);
            assert_eq!(
                multi_user_host::dispatch(
                    &f.access,
                    &site,
                    method,
                    &f.reference.url(leaf),
                    &h,
                    "",
                    multi_user_host::now()
                )
                .status,
                404
            );
        }
    }
    assert_eq!(
        f.call(
            Some(&a),
            "GET",
            &f.reference.url("text?lid=1.1"),
            Value::Null
        )
        .status,
        200
    );
    let server = f.start();
    let path = f.reference.url("assets/images/same.png");
    let response = ureq::get(&format!("{}{path}", server.url))
        .set("Host", "reader.example")
        .set("Cookie", &format!("{COOKIE}={a}"))
        .set("Range", "bytes=0-4")
        .set("If-None-Match", "*")
        .call()
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.header("Cache-Control"), Some("no-store"));
    assert_eq!(response.into_string().unwrap(), "material-image");
    let head = http(&server.url, "HEAD", &path, Some(&a), None, None);
    assert_eq!(head.status(), 200);
    assert_eq!(head.header("Content-Length"), Some("14"));
    assert_eq!(head.into_string().unwrap(), "");
    for leaf in [
        "assets/../source.txt",
        "assets/%2e%2e/source.txt",
        "assets/%252e%252e/source.txt",
        "publication.json",
        "base.json",
        "assets/private.png",
    ] {
        let response = f.call(Some(&a), "GET", &f.reference.url(leaf), Value::Null);
        assert_ne!(response.status, 200);
        assert!(!body(&response).contains(&f._root.path().to_string_lossy().to_string()));
    }
    f.access
        .library
        .lock()
        .unwrap()
        .revoke("A", &f.reference)
        .unwrap();
    assert_eq!(
        http(&server.url, "GET", &path, Some(&a), None, None).status(),
        404
    );
    assert_eq!(
        http(
            &server.url,
            "GET",
            "/api/agent/runs/turn-A/events",
            Some(&a),
            None,
            None
        )
        .status(),
        404
    );
    server.shutdown();
}

#[test]
fn mu4_sessions_survive_restart_expire_and_revoke_without_plaintext_storage() {
    let mut f = Fixture::new();
    let now = multi_user_host::now();
    let token = f.login("A");
    let stored: (String,Vec<u8>) = f.control.connection.query_row("SELECT u.password_hash,s.token_digest FROM users u JOIN auth_sessions s ON u.user_id=s.owner_user_id WHERE u.user_id='A'",[],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
    assert!(stored.0.starts_with("$argon2id$v=19$m=19456,t=2,p=1$"));
    assert!(!stored.0.contains(PASSWORD));
    assert_eq!(stored.1.len(), 32);
    assert_ne!(stored.1, token.as_bytes());
    let reopened = AuthService::new(ControlStore::open(f.control.writer.clone()).unwrap());
    assert!(reopened.authenticate(&token, now + 1).is_ok());
    assert!(reopened
        .authenticate(&token, now + SESSION_SECONDS)
        .is_err());
    f.control
        .provision_password("A", "replacement-fixture-password", false)
        .unwrap();
    assert!(reopened.authenticate(&token, now + 1).is_err());
    let next = reopened
        .login("A", "replacement-fixture-password", None, now)
        .unwrap();
    f.control.set_user_disabled("A", true).unwrap();
    assert!(reopened.authenticate(&next, now).is_err());
    f.control.set_user_disabled("A", false).unwrap();
    assert!(reopened.authenticate(&next, now).is_err());
    let third = reopened
        .login("A", "replacement-fixture-password", None, now)
        .unwrap();
    f.control.revoke_user_sessions("A").unwrap();
    assert!(reopened.authenticate(&third, now).is_err());
    let b = f.login("B");
    assert!(f.access.auth.authenticate(&b, now).is_ok());
}

#[test]
fn mu4_login_limiter_and_mode_are_bounded_and_explicit() {
    let f = Fixture::new();
    for _ in 0..5 {
        assert_eq!(
            f.call(
                None,
                "POST",
                "/api/auth/login",
                json!({"username":"A","password":"bad"})
            )
            .status,
            401
        );
    }
    assert_eq!(
        f.call(
            None,
            "POST",
            "/api/auth/login",
            json!({"username":"A","password":PASSWORD})
        )
        .status,
        429
    );
    assert_eq!(
        f.call(
            None,
            "POST",
            "/api/auth/login",
            json!({"username":"B","password":PASSWORD})
        )
        .status,
        200
    );
    assert!(multi_user_host::start(multi_user_host::MultiUserConfig {
        root: f._root.path().into(),
        addr: "0.0.0.0:0".parse().unwrap(),
        origin: ORIGIN.into()
    })
    .is_err());
    for origin in [
        "http://reader.example",
        "https://reader.example/path",
        "https://user:password@reader.example",
        "https://reader.example?x=1",
    ] {
        assert!(Site::new(origin).is_err());
    }
    let mut headers = f.headers(None, true);
    headers.0.push(("X-User-Id".into(), "local".into()));
    assert_eq!(
        multi_user_host::dispatch(
            &f.access,
            &Site::new(ORIGIN).unwrap(),
            "GET",
            "/api/auth/me",
            &headers,
            "",
            multi_user_host::now()
        )
        .status,
        401
    );
}

#[test]
fn mu4_same_legacy_ids_and_delivered_presentation_stay_in_user_root() {
    use runtime::presentation::*;
    let f = Fixture::new();
    f.access
        .library
        .lock()
        .unwrap()
        .grant("B", &f.reference)
        .unwrap();
    for id in ["A", "B"] {
        let handle = f.access.users.lock().unwrap().get(id, "now").unwrap();
        let mut user = handle.lock().unwrap();
        let session = &mut user.agent_history.sessions[0];
        session.id = "chat-same".into();
        session.turns[0].turn_id = "turn-same".into();
        session.turns[0].status = AgentAssistantStatus::Completed;
        session.turns[0].error = None;
        session.turns[0].outcome = Some(OuterOutcome {
            answer: Some(format!("{id}-private-answer")),
            answer_view: Some(AgentAnswerView {
                parts: vec![AgentAnswerPart::Presentation {
                    presentation_id: "presentation-same".into(),
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
            reference: PresentationRef {
                presentation_id: "presentation-same".into(),
                revision: 1,
            },
            candidate_id: "candidate-same".into(),
            owner: PresentationOwner {
                book_id: f.reference.book_id.clone(),
                session_id: "chat-same".into(),
            },
            created_by_turn_id: "turn-same".into(),
            based_on: None,
            content: PresentationContent {
                animation_assets: Default::default(),
                title: format!("{id}-private-title"),
                content_files: BTreeMap::from([(
                    "index.html".into(),
                    format!("<p>{id}-private-content</p>"),
                )]),
                entrypoint: "index.html".into(),
                readable_content: format!("{id}-private-content"),
                source_bindings: vec![],
                assumptions: vec![],
                state_contract: json!({}),
                initial_state: json!({}),
            },
        };
        let path = user
            .presentation_root()
            .unwrap()
            .join("versions/presentation-same/1.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, serde_json::to_vec(&version).unwrap()).unwrap();
        save_agent_history_path(&user.history_path, &user.agent_history).unwrap();
    }
    let server = f.start();
    for id in ["A", "B"] {
        let token = f.login(id);
        let request = json!({"session_id":"chat-same","turn_id":"turn-same","reference":{"presentation_id":"presentation-same","revision":1}});
        let response = http(
            &server.url,
            "POST",
            "/api/me/presentation.read",
            Some(&token),
            Some(&crate::auth::csrf(&token)),
            Some(request.clone()),
        );
        assert_eq!(response.status(), 200);
        let body = response.into_string().unwrap();
        assert!(body.contains(&format!("{id}-private-content")));
        assert!(!body.contains(if id == "A" { "B-private" } else { "A-private" }));
        let other_revision = json!({"reference":{"presentation_id":"presentation-same","revision":2},"session_id":"chat-same","turn_id":"turn-same"});
        assert_eq!(
            http(
                &server.url,
                "POST",
                "/api/me/presentation.read",
                Some(&token),
                Some(&crate::auth::csrf(&token)),
                Some(other_revision)
            )
            .status(),
            404
        );
    }
    server.shutdown();
}

#[test]
fn mu4_pdf_real_host_is_bound_and_private_storage_failure_is_sanitized() {
    let f = Fixture::new();
    let source = tempfile::tempdir().unwrap();
    let mut state = state_named("mu4-pdf");
    state.workspace.book_dir = source.path().into();
    write_current_book_files(&state);
    write_note_pdf_route_artifacts(&mut state);
    // Real publications exceed tiny_http's default 32 KiB chunking threshold.
    // Preserve their known length so the deployment proxy can serve PDF.js ranges.
    let pdf_path = source.path().join("paper.pdf");
    let mut pdf_bytes = std::fs::read(&pdf_path).unwrap();
    pdf_bytes.resize(96 * 1024, b' ');
    std::fs::write(&pdf_path, &pdf_bytes).unwrap();
    let manifest_path = source.path().join("source_manifest.json");
    let mut manifest: Value =
        serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();
    manifest["original_pdf"]["sha256"] = json!(sha256_hex(
        &std::fs::read(source.path().join("paper.pdf")).unwrap()
    ));
    std::fs::write(manifest_path, manifest.to_string()).unwrap();
    std::fs::write(
        source.path().join("alignment_report.json"),
        r#"{"config_hash":"cfg-a"}"#,
    )
    .unwrap();
    let reference = f
        .access
        .library
        .lock()
        .unwrap()
        .publish(source.path())
        .unwrap()
        .reference;
    f.access
        .library
        .lock()
        .unwrap()
        .grant("A", &reference)
        .unwrap();
    let a = f.login("A");
    let b = f.login("B");
    let server = f.start();
    let path = reference.url("pdf/original");
    let response = http(&server.url, "GET", &path, Some(&a), None, None);
    assert_eq!(response.status(), 200);
    assert_eq!(response.header("Content-Type"), Some("application/pdf"));
    assert_eq!(response.header("Content-Length"), Some("98304"));
    assert_eq!(response.header("Transfer-Encoding"), None);
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut response.into_reader(), &mut bytes).unwrap();
    assert!(bytes.starts_with(b"%PDF"));
    for method in ["HEAD", "GET"] {
        assert_eq!(
            http(&server.url, method, &path, Some(&b), None, None).status(),
            404
        );
    }
    f.access
        .library
        .lock()
        .unwrap()
        .revoke("A", &reference)
        .unwrap();
    assert_eq!(
        http(&server.url, "HEAD", &path, Some(&a), None, None).status(),
        404
    );
    server.shutdown();
    // A newly provisioned user's inaccessible private root must not expose its OS path.
    let mut control = ControlStore::open(f.control.writer.clone()).unwrap();
    control.provision_password("C", PASSWORD, true).unwrap();
    let paths = f.access.users.lock().unwrap().paths("C").unwrap();
    std::fs::create_dir_all(paths.memory.parent().unwrap().parent().unwrap()).unwrap();
    std::fs::write(paths.memory.parent().unwrap(), "blocked").unwrap();
    let c = f
        .access
        .auth
        .login("C", PASSWORD, None, multi_user_host::now())
        .unwrap();
    let failed = f.call(Some(&c), "GET", "/api/library", Value::Null);
    assert_eq!(failed.status, 403);
    assert!(!body(&failed).contains("users"));
    assert!(!body(&failed).contains(&f._root.path().to_string_lossy().to_string()));
}

#[test]
fn mu4_open_sse_closes_on_logout_disable_and_material_revocation() {
    use crate::agent_stream::{RunDescriptor, RunStream};
    use std::io::{BufRead, BufReader, Write};
    for revoke in ["logout", "disable", "grant"] {
        let mut f = Fixture::new();
        let token = f.login("A");
        let principal = f
            .access
            .auth
            .authenticate(&token, multi_user_host::now())
            .unwrap();
        let context = f
            .access
            .context(principal.clone(), multi_user_host::now())
            .unwrap();
        assert!(f.access.observation(&context, "turn-B").is_err());
        let permit = f.access.observation(&context, "turn-A").unwrap();
        let stream = RunStream::new(RunDescriptor {
            book_id: f.reference.book_id.clone(),
            session_id: "chat-A".into(),
            turn_id: "turn-A".into(),
        });
        let listener = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let address = listener.server_addr().to_ip().unwrap();
        let active = stream.clone();
        let host = std::thread::spawn(move || {
            let request = listener.recv().unwrap();
            crate::agent_stream::serve_authorized(request, active, None, permit);
        });
        let mut socket = std::net::TcpStream::connect(address).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        socket
            .write_all(b"GET /events HTTP/1.1\r\nHost: reader.example\r\nConnection: close\r\n\r\n")
            .unwrap();
        let mut reader = BufReader::new(socket);
        let mut line = String::new();
        loop {
            line.clear();
            assert!(reader.read_line(&mut line).unwrap() > 0);
            if line.starts_with("data:") {
                break;
            }
        }
        match revoke {
            "logout" => f.access.auth.logout(&principal).unwrap(),
            "disable" => f.control.set_user_disabled("A", true).unwrap(),
            _ => f
                .access
                .library
                .lock()
                .unwrap()
                .revoke("A", &f.reference)
                .unwrap(),
        }
        stream.reader_changed(json!({"sentinel":"must-not-be-pushed-after-revocation"}));
        let mut tail = String::new();
        std::io::Read::read_to_string(&mut reader, &mut tail).unwrap();
        assert!(!tail.contains("must-not-be-pushed-after-revocation"));
        host.join().unwrap();
    }
}
