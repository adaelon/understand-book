use super::*;
use crate::{
    control_store::{ControlStore, ServiceWriter},
    published_library::PublishedLibrary,
};

fn library(root: &Path, count: usize) -> PublishedLibrary {
    let mut control = ControlStore::open(ServiceWriter::acquire(root).unwrap()).unwrap();
    for owner in ["A", "B", "local"] {
        control.create_user(owner).unwrap();
    }
    PublishedLibrary::with_budget(control, count, 2 * 1024 * 1024 * 1024)
}

fn material(path: &Path, id: &str, image: &[u8]) {
    std::fs::create_dir_all(path.join("assets/images")).unwrap();
    let mut base = sample_base();
    base.book_id = id.into();
    std::fs::write(path.join("base.json"), serde_json::to_vec(&base).unwrap()).unwrap();
    std::fs::write(path.join("source.txt"), "X".repeat(100) + "尾巴").unwrap();
    std::fs::write(path.join("assets/images/same.png"), image).unwrap();
    std::fs::write(path.join("asset_manifest.json"), json!({"version":"asset_manifest.v1", "book_id":id,
        "images":[{"lid":"1.1","status":"available","stored_path":"assets/images/same.png","url_path":"/book/assets/images/same.png"}]}).to_string()).unwrap();
    std::fs::write(path.join("secret.txt"), "not a runtime dependency").unwrap();
}

#[test]
fn book_cover_publication_copies_metadata_cover_and_enforces_grants_without_loading_book() {
    let root = tempfile::tempdir().unwrap();
    let input = tempfile::tempdir().unwrap();
    material(input.path(), "covered-book", b"body image");
    std::fs::write(input.path().join("assets/cover.png"), b"cover image").unwrap();
    let mut assets: Value = serde_json::from_slice(&std::fs::read(input.path().join("asset_manifest.json")).unwrap()).unwrap();
    assets["cover"] = json!({"stored_path":"assets/cover.png","mime":"image/png"});
    std::fs::write(input.path().join("asset_manifest.json"), assets.to_string()).unwrap();
    let mut library = library(root.path(), 1);
    let publication = library.publish(input.path()).unwrap();
    library.grant("A", &publication.reference).unwrap();
    let catalog = library.list("A").unwrap();
    assert_eq!(catalog[0]["cover"], json!({"kind":"image","url":publication.reference.url("assets/cover.png")}));
    assert_eq!(library.resident_usage(), (0, 0));
    assert!(library.list("B").unwrap().is_empty());
    assert!(library.asset("B", &publication.reference, "assets/cover.png").is_err());
    assert_eq!(library.asset("A", &publication.reference, "assets/cover.png").unwrap().body, b"cover image");
    library.revoke("A", &publication.reference).unwrap();
    assert!(library.asset("A", &publication.reference, "assets/cover.png").is_err());
}

#[test]
fn book_cover_local_reads_an_unopened_book_and_pdf_without_switching_the_reader() {
    let root = tempfile::tempdir().unwrap();
    let image_book = root.path().join("image book");
    let pdf_book = root.path().join("pdf book");
    material(&image_book, "image-book", b"image");
    material(&pdf_book, "pdf-book", b"figure");
    std::fs::write(image_book.join("asset_manifest.json"), json!({"cover":{"stored_path":"assets/images/same.png"}}).to_string()).unwrap();
    std::fs::write(pdf_book.join("source_manifest.json"), json!({"version":"source_manifest.v2","original_pdf":{"path":"original.pdf"}}).to_string()).unwrap();
    std::fs::write(pdf_book.join("original.pdf"), b"%PDF-first-page").unwrap();
    let mut state = state_named("book-cover-local");
    state.services.library_root = Some(root.path().to_path_buf());
    let before = state.workspace.book_dir.clone();
    let entries = list_mixed_book_library(root.path());
    assert_eq!(entries.len(), 2);
    for (entry, kind, bytes) in [(&entries[0], "image", b"image".as_slice()), (&entries[1], "pdf", b"%PDF-first-page".as_slice())] {
        let cover = entry.cover.as_ref().unwrap();
        assert_eq!(cover.kind, kind);
        let result = crate::book_cover::route_local(&state, cover.url.strip_prefix("/api").unwrap()).unwrap();
        assert_eq!(result.status, 200);
        assert_eq!(result.body, bytes);
    }
    assert_eq!(state.workspace.book_dir, before);
    assert_eq!(crate::book_cover::route_local(&state, "/book/library/cover?dir=unknown").unwrap().status, 404);
}

#[test]
fn mu3_publications_isolate_same_name_resources_and_require_grants_on_cache_hits() {
    let root = tempfile::tempdir().unwrap();
    let input = tempfile::tempdir().unwrap();
    let mut library = library(root.path(), 20);
    material(&input.path().join("x"), "book-x", b"image-X");
    material(&input.path().join("y"), "book-y", b"image-Y");
    let x = library.publish(&input.path().join("x")).unwrap();
    let y = library.publish(&input.path().join("y")).unwrap();
    library.grant("A", &x.reference).unwrap();
    library.grant("B", &y.reference).unwrap();
    let catalog = library.list("A").unwrap();
    assert_eq!(catalog.len(), 1);
    assert_eq!(catalog[0]["published_book_ref"], json!(x.reference));
    assert!(catalog[0].get("directory").is_none());
    assert!(library.load("B", &x.reference).is_err());
    let a = library.load("A", &x.reference).unwrap();
    assert!(!a.directory().join("secret.txt").exists());
    assert!(library.load("B", &x.reference).is_err());
    library.grant("B", &x.reference).unwrap();
    let shared = library.load("B", &x.reference).unwrap();
    assert!(Arc::ptr_eq(&a.book, &shared.book));
    let b = library.load("B", &y.reference).unwrap();
    std::thread::scope(|scope| {
        let left = scope.spawn(|| a.asset("assets/images/same.png").unwrap().body);
        let right = scope.spawn(|| b.asset("assets/images/same.png").unwrap().body);
        assert_eq!(left.join().unwrap(), b"image-X");
        assert_eq!(right.join().unwrap(), b"image-Y");
    });
    for path in [
        "../source.txt",
        "assets/../base.json",
        "assets/%2e%2e/secret.txt",
        "assets\\images\\same.png",
        "secret.txt",
        "publication.json",
        "assets/images/absent.png",
    ] {
        assert!(library.asset("A", &x.reference, path).is_err(), "{path}");
    }
    let mut state = state_named("mu3-resources");
    state.workspace.bind_publication(a);
    let reply =
        route_workspace_asset_file(&state.workspace, &x.reference.url("assets/images/same.png"))
            .unwrap();
    assert_eq!(reply.body, b"image-X");
    assert_ne!(
        route_workspace_asset_file(&state.workspace, "/book/assets/images/same.png")
            .unwrap()
            .status,
        200
    );
    assert_ne!(
        route_workspace_asset_file(&state.workspace, &y.reference.url("assets/images/same.png"))
            .unwrap()
            .status,
        200
    );
}

#[test]
fn mu3_default_change_keeps_old_run_history_and_capacity_pins() {
    let root = tempfile::tempdir().unwrap();
    let input = tempfile::tempdir().unwrap();
    material(input.path(), "book-x", b"same");
    let mut library = library(root.path(), 1);
    let first = library.publish(input.path()).unwrap();
    library.grant("local", &first.reference).unwrap();
    let p = library.load("local", &first.reference).unwrap();
    let charge = p.manifest.resident_bytes;
    let mut state = state_named("mu3-old-run");
    let private = tempfile::tempdir().unwrap();
    state.user.history_path = Some(private.path().join("history.json"));
    state.workspace.bind_publication(p);
    let prepared = prepare_agent_chat(&mut state, r#"{"message":"你好"}"#, "now")
        .unwrap_or_else(|r| panic!("{}", r.body));
    let original = prepared
        .scope
        .publication
        .as_ref()
        .unwrap()
        .reference
        .clone();
    let second = library.publish(input.path()).unwrap();
    library.grant("local", &second.reference).unwrap();
    assert_eq!(library.default_ref("book-x").unwrap(), second.reference);
    assert_eq!(original, first.reference);
    library.evict_cache();
    assert_eq!(library.resident_usage(), (1, charge));
    assert!(
        matches!(library.load("local", &second.reference), Err(e) if e.error_code == "BOOK_CAPACITY")
    );
    // An unregistered replacement scene drops its publication, while the old Run still pins it.
    state.workspace = state_named("mu3-new-scene").workspace;
    assert_eq!(library.resident_usage(), (1, charge));
    assert!(prepared.scope.check_workspace(&state).is_err());
    assert!(prepared.scope.check_owner(&state).is_ok());
    assert_eq!(
        prepared.scope.book.text("1.1", None).unwrap(),
        "X".repeat(100)
    );
    let history = load_agent_history(&state.user.history_path).unwrap();
    assert_eq!(
        history.sessions[0].turns[0].published_book_ref.as_ref(),
        Some(&first.reference)
    );
    let view = turn_view(&prepared.scope.book, &history.sessions[0].turns[0]);
    assert_eq!(view.published_book_ref, Some(first.reference.clone()));
    let adapter = ChatStubAdapter::scripted(vec![AssistantTurn {
        provider_continuation: None,
        text: Some("你好。".into()),
        tool_calls: vec![],
        usage_total_tokens: Some(3),
    }]);
    let result = agent_run::execute_prepared(
        &agent_run::BorrowedAppPort(std::cell::RefCell::new(&mut state)),
        &adapter,
        prepared,
        Default::default(),
    );
    assert_eq!(result.reply.status, 200, "{}", result.reply.body);
    let saved = load_agent_history(&state.user.history_path).unwrap();
    assert_eq!(
        saved.sessions[0].turns[0].status,
        AgentAssistantStatus::Completed
    );
    assert_eq!(
        saved.sessions[0].turns[0].published_book_ref.as_ref(),
        Some(&first.reference)
    );
    assert_eq!(library.resident_usage(), (0, 0));
    let newest = library.load("local", &second.reference).unwrap();
    assert_eq!(newest.reference, second.reference);
    state.workspace.bind_publication(newest);
    let source = agent_source_binding(
        &state,
        &AgentSourceRequest {
            turn_id: saved.sessions[0].turns[0].turn_id.clone(),
            source_ref_id: "s1".into(),
        },
    );
    assert!(matches!(source, Err(e) if e.error_code=="PUBLICATION_BINDING_REQUIRED"));
    library.set_default(&first.reference).unwrap();
    assert_eq!(library.default_ref("book-x").unwrap(), first.reference);
}

#[test]
fn mu3_published_localization_is_shared_single_flight_and_urls_are_bound() {
    let root = tempfile::tempdir().unwrap();
    let input = tempfile::tempdir().unwrap();
    let mut state = state_named("mu3-shared-cache");
    state.workspace.book_dir = input.path().into();
    attach_paper_profile(&mut state);
    write_pdf_runtime_artifacts(&mut state);
    let mut manifest = source_manifest_value(input.path()).unwrap();
    manifest["canonical_source"]["sha256"] =
        json!(current_note_source_fingerprint(input.path()).unwrap());
    manifest["original_pdf"]["sha256"] = json!(sha256_hex(
        &std::fs::read(input.path().join("paper.pdf")).unwrap()
    ));
    std::fs::write(
        input.path().join("source_manifest.json"),
        manifest.to_string(),
    )
    .unwrap();
    std::fs::write(
        input.path().join("alignment_report.json"),
        r#"{"config_hash":"cfg-a"}"#,
    )
    .unwrap();
    std::fs::create_dir_all(input.path().join(".build/source-reconciliation")).unwrap();
    std::fs::write(
        input
            .path()
            .join(".build/source-reconciliation/report.json"),
        json!({"book_id":state.workspace.book.base.book_id,"unresolved":[]}).to_string(),
    )
    .unwrap();
    let mut library = library(root.path(), 20);
    let p = library.publish(input.path()).unwrap();
    for owner in ["A", "B", "local"] {
        library.grant(owner, &p.reference).unwrap();
    }
    let a = library.load("A", &p.reference).unwrap();
    let b = library.load("B", &p.reference).unwrap();
    let base = a.book.paper_minimap();
    assert!(!base.regions.is_empty());
    let answer = json!({"regions":base.regions.iter().map(|r| json!({"id":r.region_id,"zh":"研究方法"})).collect::<Vec<_>>(),
        "landmarks":base.landmarks.iter().map(|l| json!({"id":l.landmark_id,"zh":format!("中文：{}",l.label)})).collect::<Vec<_>>()}).to_string();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let adapter = StructuredRecordingAdapter {
        users: calls.clone(),
        answer,
    };
    std::thread::scope(|scope| {
        let first = scope.spawn(|| {
            localize_paper_minimap(base.clone(), Some(a.localization_cache.clone()), &adapter)
        });
        let second = scope.spawn(|| {
            localize_paper_minimap(base.clone(), Some(b.localization_cache.clone()), &adapter)
        });
        assert_eq!(first.join().unwrap().status, 200);
        assert_eq!(second.join().unwrap().status, 200);
    });
    assert_eq!(calls.lock().unwrap().len(), 1);
    assert!(a
        .localization_cache
        .starts_with(root.path().canonicalize().unwrap().join("shared-cache")));
    assert!(a.localization_cache.is_file());
    assert!(!a
        .directory()
        .join("paper-minimap-localizations.json")
        .exists());
    library
        .open_workspace(&mut state, &p.reference, "now")
        .unwrap();
    assert!(
        route_workspace_asset_file(&state.workspace, &p.reference.url("source_manifest")).is_none()
    );
    let reply = get(&mut state, &p.reference.url("source_manifest"));
    assert_eq!(reply.status, 200, "{}", reply.body);
    let body: Value = serde_json::from_str(&reply.body).unwrap();
    assert_eq!(body["published_book_ref"], json!(p.reference));
    assert_eq!(body["resource_base"], p.reference.url(""));
    let typed = library
        .read(
            "local",
            &p.reference,
            "text",
            &HashMap::from([("lid".into(), "1.1".into())]),
            &state.user,
        )
        .unwrap();
    assert_eq!(typed.status, 200);
    assert!(library
        .read("B", &p.reference, "text", &HashMap::new(), &state.user)
        .is_err());
}

#[test]
fn mu3_rejects_revised_content_missing_dependencies_and_preserves_teaching_readiness() {
    let root = tempfile::tempdir().unwrap();
    let input = tempfile::tempdir().unwrap();
    material(input.path(), "book-x", b"same");
    let mut library = library(root.path(), 20);
    let first = library.publish(input.path()).unwrap();
    assert_eq!(first.teaching_readiness["status"], "preparing");
    std::fs::write(input.path().join("source.txt"), "Y".repeat(100)).unwrap();
    assert!(
        matches!(library.publish(input.path()), Err(e) if e.error_code=="BOOK_CONTENT_CHANGED")
    );
    material(input.path(), "book-x-v2", b"same");
    let second = library.publish(input.path()).unwrap();
    assert_ne!(first.reference.book_id, second.reference.book_id);
    std::fs::write(input.path().join("assets/images/same.png"), b"changed").unwrap();
    assert!(
        matches!(library.publish(input.path()), Err(e) if e.error_code=="BOOK_CONTENT_CHANGED")
    );
    std::fs::remove_file(input.path().join("assets/images/same.png")).unwrap();
    assert!(library.publish(input.path()).is_err());
    assert_eq!(library.default_ref("book-x-v2").unwrap(), second.reference);
    material(input.path(), "book-x", b"same");
    let book = Book::load(input.path().to_str().unwrap()).unwrap();
    std::fs::create_dir_all(input.path().join("teaching/versions/rev1")).unwrap();
    std::fs::write(
        input.path().join("teaching/versions/rev1/map.json"),
        json!({"source_id":"book-x","source_revision":book.source_fingerprint()}).to_string(),
    )
    .unwrap();
    let mut receipt = json!({"version":"teaching_readiness.v1","source_id":"book-x","source_revision":book.source_fingerprint(),"status":"ready","teaching_map_revision":"rev1","map_path":"teaching/versions/rev1/map.json", "coverage":{"source":"complete","structure":"complete","objects":"complete","cognitive_materials":"complete","source_review":"failed"}});
    std::fs::write(
        input.path().join("teaching_readiness.json"),
        receipt.to_string(),
    )
    .unwrap();
    let stale = library.publish(input.path()).unwrap();
    assert_eq!(stale.teaching_readiness["status"], "stale");
    receipt["coverage"]["source_review"] = json!("passed");
    std::fs::write(
        input.path().join("teaching_readiness.json"),
        receipt.to_string(),
    )
    .unwrap();
    let ready = library.publish(input.path()).unwrap();
    assert_eq!(ready.teaching_readiness["status"], "ready");
    library.grant("A", &ready.reference).unwrap();
    let loaded = library.load("A", &ready.reference).unwrap();
    assert!(loaded
        .directory()
        .join("teaching/versions/rev1/map.json")
        .is_file());
}

#[test]
fn mu3_readonly_pdf_package_relocates_attachments_and_routes_without_writes() {
    let root = tempfile::tempdir().unwrap();
    let input = tempfile::tempdir().unwrap();
    let mut state = state_named("mu3-pdf");
    state.workspace.book_dir = input.path().to_path_buf();
    write_current_book_files(&state);
    write_note_pdf_route_artifacts(&mut state);
    // Real supported local manifest may point outside the workspace.
    let external = tempfile::tempdir().unwrap();
    let external_pdf = external.path().join("input.pdf");
    std::fs::copy(input.path().join("paper.pdf"), &external_pdf).unwrap();
    let mut source: Value =
        serde_json::from_slice(&std::fs::read(input.path().join("source_manifest.json")).unwrap())
            .unwrap();
    source["original_pdf"]["path"] = json!(external_pdf);
    source["original_pdf"]["sha256"] = json!(sha256_hex(&std::fs::read(&external_pdf).unwrap()));
    source["capabilities"]["view_pdf"]["artifact_path"] = json!(external_pdf);
    std::fs::write(
        input.path().join("source_manifest.json"),
        source.to_string(),
    )
    .unwrap();
    std::fs::write(
        input.path().join("alignment_report.json"),
        r#"{"config_hash":"cfg-a"}"#,
    )
    .unwrap();
    let mut library = library(root.path(), 20);
    let publication = library.publish(input.path()).unwrap();
    library.grant("local", &publication.reference).unwrap();
    let p = library.load("local", &publication.reference).unwrap();
    let published_dir = p.directory().to_path_buf();
    assert!(std::fs::metadata(published_dir.join("base.json"))
        .unwrap()
        .permissions()
        .readonly());
    drop(external);
    assert!(library
        .asset("local", &publication.reference, "original.pdf")
        .unwrap()
        .body
        .starts_with(b"%PDF"));
    assert_eq!(route_pdf_source_map(&published_dir).status, 200);
    let private = tempfile::tempdir().unwrap();
    state.workspace.session_path = Some(private.path().join("session.json"));
    state.workspace.bind_publication(p);
    state.services.reader_only = true;
    let before: BTreeMap<_, _> = publication
        .files
        .keys()
        .map(|path| {
            (
                path.clone(),
                std::fs::read(published_dir.join(path)).unwrap(),
            )
        })
        .collect();
    assert_eq!(get(&mut state, "/book/text?lid=1.1").status, 200);
    let query = post(
        &mut state,
        "/book/query",
        r#"{"query":"什么是命令模式","intent":"definition","targets":["命令模式"],"obligations":[{"requirement":"给出定义"}],"anchor_lid":"1.1"}"#,
    );
    assert_eq!(query.status, 200, "{}", query.body);
    assert!(query.body.contains("\"status\":\"complete\""));
    assert_eq!(
        post(&mut state, "/reader/goto", r#"{"lid":"1.1"}"#).status,
        200
    );
    save_session(&state, None);
    assert!(private.path().join("session.json").is_file());
    assert_ne!(
        post(
            &mut state,
            "/book/open",
            &json!({"dir": input.path()}).to_string()
        )
        .status,
        200
    );
    assert_eq!(get(&mut state, "/book/build_workbench").status, 200);
    for (path, bytes) in before {
        assert_eq!(std::fs::read(published_dir.join(path)).unwrap(), bytes);
    }
}

#[test]
fn mu3_service_metadata_reopens_defaults_and_schema_one_upgrades_without_losing_users() {
    let root = tempfile::tempdir().unwrap();
    let input = tempfile::tempdir().unwrap();
    let writer = ServiceWriter::acquire(root.path()).unwrap();
    let mut control = ControlStore::open(writer.clone()).unwrap();
    control.create_user("A").unwrap();
    control
        .connection
        .execute_batch("DROP TABLE book_defaults; ALTER TABLE reader_workspaces DROP COLUMN checkpoint_seq; ALTER TABLE run_admissions DROP COLUMN cancel_requested; ALTER TABLE run_admissions DROP COLUMN key_closed; ALTER TABLE run_admissions DROP COLUMN unsaved; PRAGMA user_version=1;")
        .unwrap();
    drop(control);
    let mut library = PublishedLibrary::new(ControlStore::open(writer.clone()).unwrap());
    material(input.path(), "book-x", b"image");
    let p = library.publish(input.path()).unwrap();
    library.grant("A", &p.reference).unwrap();
    drop(library);
    let mut reopened = PublishedLibrary::new(ControlStore::open(writer).unwrap());
    assert_eq!(reopened.default_ref("book-x").unwrap(), p.reference);
    assert!(reopened.load("A", &p.reference).is_ok());
}

#[test]
fn mu3_failed_registration_is_invisible_and_byte_budget_prevents_loading() {
    let root = tempfile::tempdir().unwrap();
    let input = tempfile::tempdir().unwrap();
    material(input.path(), "book-x", b"image");
    let writer = ServiceWriter::acquire(root.path()).unwrap();
    let mut control = ControlStore::open(writer.clone()).unwrap();
    control.create_user("A").unwrap();
    let connection = rusqlite::Connection::open(writer.root().join("control.sqlite")).unwrap();
    connection.execute_batch("CREATE TRIGGER reject_publication BEFORE INSERT ON book_publications BEGIN SELECT RAISE(ABORT, 'injected registration failure'); END;").unwrap();
    let mut library = PublishedLibrary::with_budget(control, 20, 0);
    assert!(library.publish(input.path()).is_err());
    assert!(library.default_ref("book-x").is_err());
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM book_publications", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    connection
        .execute_batch("DROP TRIGGER reject_publication;")
        .unwrap();
    let p = library.publish(input.path()).unwrap();
    library.grant("A", &p.reference).unwrap();
    assert!(matches!(library.load("A",&p.reference),Err(e) if e.error_code=="BOOK_CAPACITY"));
    assert_eq!(library.resident_usage(), (0, 0));
    // A missing publication never resolves to the latest publication instead.
    let mut missing = p.reference.clone();
    missing.publication_id = "missing-publication".into();
    assert!(library
        .asset("A", &missing, "assets/images/same.png")
        .is_err());
}

#[test]
fn mu3_url_binding_preserves_user_and_source_plaintext() {
    let reference = crate::published_library::PublishedBookRef {
        book_id: "book-x".into(),
        publication_id: "publication-1".into(),
    };
    let input = json!({"user":"/book/text is the command I typed", "text":"/book/assets/example.png", "nested":{"answer":"/api/book/foo"}});
    let output = crate::published_library::bind_reply(ok_json(&input), &reference);
    let value: Value = serde_json::from_str(&output.body).unwrap();
    for key in ["user", "text", "nested"] {
        assert_eq!(value[key], input[key]);
    }
    let assets = json!({"version":"asset_manifest.v1","images":[{"url_path":"/book/assets/images/x.png","alt":"/book/text"}]});
    let output = crate::published_library::bind_reply(ok_json(&assets), &reference);
    let value: Value = serde_json::from_str(&output.body).unwrap();
    assert_eq!(
        value["images"][0]["url_path"],
        reference.url("assets/images/x.png")
    );
    assert_eq!(value["images"][0]["alt"], "/book/text");
}

#[test]
fn mu3_writer_reopen_preserves_sealed_package_permissions() {
    let root = tempfile::tempdir().unwrap();
    let input = tempfile::tempdir().unwrap();
    material(input.path(), "book-x", b"image");
    let mut library = library(root.path(), 20);
    let publication = library.publish(input.path()).unwrap();
    library.grant("A", &publication.reference).unwrap();
    let p = library.load("A", &publication.reference).unwrap();
    let directory = p.directory().to_path_buf();
    drop(p);
    drop(library);
    let writer = ServiceWriter::acquire(root.path()).unwrap();
    let mut reopened = PublishedLibrary::new(ControlStore::open(writer).unwrap());
    assert!(std::fs::metadata(directory.join("base.json"))
        .unwrap()
        .permissions()
        .readonly());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&directory).unwrap().permissions().mode() & 0o222,
            0
        );
    }
    assert!(reopened.load("A", &publication.reference).is_ok());
}

#[test]
fn mu3_unready_build_and_unsafe_declared_dependency_never_become_defaults() {
    let root = tempfile::tempdir().unwrap();
    let input = tempfile::tempdir().unwrap();
    let mut state = state_named("mu3-unready");
    state.workspace.book_dir = input.path().into();
    attach_paper_profile(&mut state);
    let mut library = library(root.path(), 20);
    assert!(
        matches!(library.publish(input.path()), Err(e) if e.error_code=="PUBLICATION_NOT_READY")
    );
    material(input.path(), "safe-book", b"image");
    std::fs::remove_file(input.path().join("book_structure.json")).unwrap();
    let path = input.path().join("asset_manifest.json");
    let mut assets: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assets["images"][0]["stored_path"] = json!("assets/../../secret.txt");
    std::fs::write(path, assets.to_string()).unwrap();
    assert!(library.publish(input.path()).is_err());
    assert!(library.default_ref("safe-book").is_err());
}

#[cfg(unix)]
#[test]
fn mu3_symlinks_are_rejected_at_import_and_resource_read() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let input = tempfile::tempdir().unwrap();
    material(input.path(), "book-x", b"image");
    let mut library = library(root.path(), 20);
    std::fs::remove_file(input.path().join("assets/images/same.png")).unwrap();
    symlink(
        input.path().join("secret.txt"),
        input.path().join("assets/images/same.png"),
    )
    .unwrap();
    assert!(library.publish(input.path()).is_err());
}
