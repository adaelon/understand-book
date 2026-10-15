use super::*;
use super::mu5_tests::Fixture;

#[test]
fn mu12_reads_accept_advanced_revision_but_require_same_authorized_scene() {
    let f = Fixture::new();
    let w = f.create(&f.a, &f.x, "page");
    let advanced = f.action(&f.a, &w, "page", "reader/goto", json!({"lid":"1.2"}));
    let path = |action: &str| format!("/api/workspaces/{}/{action}", w["workspace_id"].as_str().unwrap());
    let old = Fixture::stamp(&w, "page");
    for action in ["reader/state", "profile/manifest", "memory/recall", "chat/history", "profile/memory", "reader/paper_minimap.state"] {
        let read = f.ok(&f.a, "POST", &path(action), old.clone());
        assert_eq!(read["revision"], advanced["revision"]);
        assert_eq!(f.call(&f.b, "POST", &path(action), old.clone()).0, 404);
        assert_eq!(f.call(&f.a, "POST", &path(action), Fixture::stamp(&advanced, "wrong")).0, 409);
    }
    let mut write = old.clone(); write["lid"] = json!("1.3");
    assert_eq!(f.call(&f.a, "POST", &path("reader/goto"), write).0, 409);
    let changed = f.action(&f.a, &advanced, "page", "chat/new", json!({}));
    assert_ne!(changed["generation"], w["generation"]);
    assert_eq!(f.call(&f.a, "POST", &path("memory/recall"), old).0, 409);
}

#[test]
fn mu8_reader_commands_share_private_authority_and_reject_other_material() {
    let f = Fixture::new();
    let w = f.create(&f.a, &f.x, "page");
    let other = f.create(&f.a, &f.y, "other");
    let b = f.create(&f.b, &f.x, "b");
    let history = f.action(&f.a, &w, "page", "chat/history", json!({}));
    assert_eq!(history["result"]["active_session_id"], "");
    assert_eq!(f.action(&f.a, &w, "page", "profile/manifest", json!({}))["result"]["profile_id"], "technical_learning");
    let note = f.action(&f.a, &w, "page", "memory/save", json!({"type":"note","content":"A private note","selection_context":{"status":"resolved","raw_quote":"text","resolved_quote":"text","ranges":[{"lid":"1.1","range":{"start":0,"end":1}}]}}));
    let id = note["result"]["record"]["mem_id"].clone();
    assert!(id.is_string(), "{note}");
    assert_eq!(f.action(&f.b, &b, "b", "memory/recall", json!({}))["result"], json!([]));
    assert_eq!(f.action(&f.a, &other, "other", "memory/recall", json!({"book_id":f.x.book_id}))["result"], json!([]));
    let mut wrong = Fixture::stamp(&other, "other"); wrong["mem_id"] = id.clone();
    assert_eq!(f.call(&f.a, "POST", &format!("/api/workspaces/{}/memory/delete",other["workspace_id"].as_str().unwrap()),wrong).0,404);
    let changed = f.action(&f.a, &w, "page", "memory/replace", json!({"mem_id":id,"content":"updated"}));
    assert_eq!(changed["result"]["content"], "updated");
    let new_chat = f.action(&f.a, &w, "page", "chat/new", json!({}));
    assert_eq!(f.action(&f.a, &new_chat, "page", "chat/history", json!({}))["result"]["active_session_id"], new_chat["selected_chat"]);
    let mut input = Fixture::stamp(&new_chat, "page");
    input["session_id"] = new_chat["selected_chat"].clone(); input["turn_id"] = json!("missing");
    assert_eq!(f.call(&f.a, "POST", &format!("/api/workspaces/{}/tutor/activities",new_chat["workspace_id"].as_str().unwrap()),input).0,400);
}

#[test]
fn mu8_profile_and_sources_keep_owner_and_publication_bindings() {
    let f = Fixture::new();
    let w = seed_presentation(&f);
    let b = f.create(&f.b, &f.x, "b");
    let other = f.create(&f.a, &f.y, "other");
    let revision = f.action(&f.a, &w, "parent", "profile/memory", json!({}))["result"]["status"]["document_revision"].as_u64().unwrap();
    let input = profile_mutation(revision, json!({"kind":"remember", "operation_id":"mu8-profile", "evidence_text":"A wants detailed explanations",
        "fact":profile_fact_draft("book", "depth", "detailed", "normal")}));
    assert_eq!(f.action(&f.a, &w, "parent", "profile/memory/apply", input)["result"]["outcome"]["kind"], "remembered");
    assert_eq!(f.action(&f.a, &w, "parent", "profile/memory", json!({}))["result"]["facts"].as_array().unwrap().len(), 1);
    assert_eq!(f.action(&f.b, &b, "b", "profile/memory", json!({}))["result"]["facts"], json!([]));
    let turn = f.action(&f.a, &w, "parent", "chat/history", json!({}))["result"]["current"]["turns"][0]["turn_id"].clone();
    let source = json!({"turn_id":turn,"source_ref_id":"mu8-source"});
    assert_eq!(f.action(&f.a, &w, "parent", "agent/source.resolve", source.clone())["result"]["stale"], false);
    for (token, scene, attachment) in [(&f.b, &b, "b"), (&f.a, &other, "other")] {
        let mut request = Fixture::stamp(scene, attachment);
        request.as_object_mut().unwrap().extend(source.as_object().unwrap().clone());
        assert_ne!(f.call(token,"POST",&format!("/api/workspaces/{}/agent/source.resolve", scene["workspace_id"].as_str().unwrap()),request).0, 200);
    }
}

#[test]
fn mu8_saved_run_recovery_uses_the_same_turn_view_as_history() {
    let f = Fixture::new();
    let w = seed_presentation(&f);
    let history = f.action(&f.a, &w, "parent", "chat/history", json!({}));
    let expected = &history["result"]["current"]["turns"][0];
    let recovered = f.ok(&f.a, "GET", &format!("/api/agent/runs/{}", expected["turn_id"].as_str().unwrap()), json!({}));
    for (field, value) in expected.as_object().unwrap() {
        assert_eq!(&recovered["turn"][field], value, "{field}");
    }
}

#[test]
#[ignore = "MU8 browser fixture; bounded lifetime and isolated temporary service root"]
fn mu8_browser_fixture() {
    let f = Fixture::new();
    seed_pdf(&f);
    let seeded = seed_presentation(&f);
    f.action(&f.a, &seeded, "parent", "detach", json!({}));
    f.access.runs.configure_fake(|| Box::new(ChatStubAdapter::scripted(vec![AssistantTurn {
        provider_continuation: None, text: Some("MU8 原问题的回答".into()), tool_calls: vec![], usage_total_tokens: Some(1),
    }])));
    let server = crate::multi_user_host::start_with_access(
        "127.0.0.1:4188".parse().unwrap(), Arc::new(crate::multi_user_host::Site::new("https://localhost:4189").unwrap()), f.access.clone(),
    ).unwrap();
    println!("MU8 browser fixture ready at {}", server.url);
    let stop = std::env::var_os("MU8_STOP_FILE").map(PathBuf::from);
    let until = std::time::Instant::now() + Duration::from_secs(1200);
    while std::time::Instant::now() < until && !stop.as_ref().is_some_and(|p| p.exists()) { std::thread::sleep(Duration::from_millis(200)); }
    server.shutdown();
}

#[test]
fn mu8_presentation_restore_is_per_workspace_not_latest_private_state() {
    let f = Fixture::new();
    let w = seed_presentation(&f);
    let fork = f.action(&f.a, &w, "second", "fork", json!({}));
    let session = f.action(&f.a, &w, "parent", "chat/history", json!({}));
    let request = json!({"session_id":w["selected_chat"], "turn_id":session["result"]["current"]["turns"][0]["turn_id"], "reference":{"presentation_id":"mu5-presentation","revision":1}});
    let mut save = request.clone();
    save["state"] = json!({"values":{"slider":9},"visible_step":null,"observed_result":"Nine","source_ref_ids":[]});
    let saved = f.action(&f.a, &w, "parent", "presentation/save", save);
    assert!(f.action(&f.a, &fork, "second", "presentation/read", request.clone())["result"]["restored_state"].is_null());
    assert_eq!(f.action(&f.a, &saved, "parent", "presentation/read", request)["result"]["restored_state"]["values"]["slider"], 9);
}

fn seed_presentation(f: &Fixture) -> Value {
    use runtime::presentation::*;
    let w = f.create(&f.a, &f.x, "parent");
    let w = f.action(&f.a, &w, "parent", "chat/new", json!({}));
    let port = f.run(&w, "parent");
    let reference = PresentationRef {
        presentation_id: "mu5-presentation".into(),
        revision: 1,
    };
    let book = f.access.library.lock().unwrap().load("A", &f.x).unwrap().book.clone();
    let evidence_range = read_tools::EvidenceRange { start_lid: "1.1".into(), end_lid: "1.1".into(), ranges: vec![] };
    let resolved = book.resolve_source(&evidence_range, "zh-CN", None).unwrap();
    let binding = SourceBinding { source_ref_id: "mu8-source".into(), book_id: f.x.book_id.clone(), evidence_range,
        evidence_text_digest: resolved.evidence_text_digest, label_snapshot: "MU8 原文".into(), preview_snapshot: resolved.preview };
    {
        let handle = f.access.users.lock().unwrap().get("A", "now").unwrap();
        let mut user = handle.lock().unwrap();
        let mut session = user
            .agent_history
            .sessions
            .iter()
            .find(|s| s.id == port.scope.chat_session_id)
            .unwrap().clone();
        session.title = "MU8 演示".into();
        session.turns[0].status = AgentAssistantStatus::Completed;
        session.turns[0].outcome = Some(OuterOutcome {
            answer: Some("MU8 interactive scene".into()),
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
                content_files: BTreeMap::from([("index.html".into(), "<p>MU8 interactive scene</p><input id=slider type=range min=0 max=10 value=2>".into())]),
                entrypoint: "index.html".into(),
                readable_content: "MU8 interactive scene".into(),
                source_bindings: vec![binding],
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
        crate::session_runtime::append(&mut user, &turn, "title", crate::session_event::EventBody::SessionUpdated { title: session.title.clone() }).unwrap();
        crate::session_runtime::finish(&mut user, &turn, &session, "finished").unwrap();
    }
    drop(port);
    w
}

fn seed_pdf(f: &Fixture) {
    let mut state = state_named("mu8-pdf");
    write_current_book_files(&state);
    write_pdf_runtime_artifacts(&mut state);
    let dir = &state.workspace.book_dir;
    let pdf = std::fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packages/core/test/fixtures/hybrid-foundation-goldset/v1/licensed-inline-formula/paper.pdf")).unwrap();
    std::fs::write(dir.join("paper.pdf"), &pdf).unwrap();
    let mut manifest = source_manifest_value(dir).unwrap();
    manifest["canonical_source"]["sha256"] = json!(current_note_source_fingerprint(dir).unwrap());
    manifest["original_pdf"]["sha256"] = json!(sha256_hex(&pdf));
    std::fs::write(dir.join("source_manifest.json"), manifest.to_string()).unwrap();
    std::fs::write(dir.join("alignment_report.json"), r#"{"config_hash":"cfg-a"}"#).unwrap();
    std::fs::create_dir_all(dir.join(".build/source-reconciliation")).unwrap();
    std::fs::write(dir.join(".build/source-reconciliation/report.json"), json!({"book_id":state.workspace.book.base.book_id,"unresolved":[]}).to_string()).unwrap();
    let mut library = f.access.library.lock().unwrap();
    let published = library.publish(dir).unwrap().reference;
    library.grant("A", &published).unwrap();
}
