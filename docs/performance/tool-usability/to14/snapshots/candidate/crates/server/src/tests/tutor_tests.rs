use super::*;

#[test]
fn tutor_existing_artifacts_work_without_build_or_publication_receipts() {
    let root = tempfile::tempdir().unwrap();
    let state = state_named("tutor-existing-artifacts");
    let book = &state.workspace.book;
    t15_foundation(root.path(), book);
    std::fs::rename(root.path().join(".build"), root.path().join("unused-build-history")).unwrap();
    let read = || crate::tutor_api::tutor_source_readiness(book, root.path());
    assert_eq!(read()["status"], "ready", "{}", read());
    for saved in [Value::Null, json!({"status":"preparing","reason":"old admission"})] {
        std::fs::write(root.path().join("publication.json"), json!({
            "source_fingerprint":book.source_fingerprint(),"tutor_readiness":saved
        }).to_string()).unwrap();
        assert_eq!(read()["status"], "ready", "{}", read());
    }
    for (file, stage) in [("base.json", "pass1"), ("source.txt", "pass1"),
        ("discourse_index.json", "profile_sidecar"), ("book_structure.json", "book_structure")] {
        let path = root.path().join(file);
        let saved = path.with_extension("saved");
        std::fs::rename(&path, &saved).unwrap();
        assert_eq!(read()["blocked_stage"], stage, "{file}: {}", read());
        std::fs::rename(saved, path).unwrap();
    }
    std::fs::write(root.path().join("discourse_index.json"), "{}").unwrap();
    assert_eq!(read()["blocked_stage"], "profile_sidecar");
    std::fs::write(root.path().join("discourse_index.json"), "{\"items\":[]}").unwrap();
    let mut structure: Value = serde_json::from_slice(&std::fs::read(root.path().join("book_structure.json")).unwrap()).unwrap();
    structure["header"]["book_id"] = json!("another-book");
    std::fs::write(root.path().join("book_structure.json"), structure.to_string()).unwrap();
    assert_eq!(read()["blocked_stage"], "book_structure");
}

#[test]
fn tutor_readiness_is_separate_from_control_and_rejects_stale_or_missing_publication() {
    let root = tempfile::tempdir().unwrap();
    let mut state = state_named("teaching-ready");
    state.workspace.book_dir = root.path().to_path_buf();
    let get = |state: &mut AppState| {
        let reply = crate::tutor_api::source_readiness(&state.workspace.book, &state.workspace.book_dir);
        assert_eq!(reply.status, 200);
        serde_json::from_str::<Value>(&reply.body).unwrap()
    };
    assert_eq!(get(&mut state)["status"], "preparing");
    let map_path = root.path().join("teaching/versions/revision1/map.json");
    std::fs::create_dir_all(map_path.parent().unwrap()).unwrap();
    let mut receipt = json!({ "version": "teaching_readiness.v1", "status": "ready", "source_id": state.workspace.book.base.book_id,
        "source_revision": state.workspace.book.source_fingerprint(), "teaching_map_revision": "revision1", "map_path": "teaching/versions/revision1/map.json",
        "coverage": { "source": "complete", "structure": "complete", "objects": "complete", "cognitive_materials": "complete", "source_review": "passed" },
        "limitations": ["作者省略了连接"] });
    let receipt_path = root.path().join("teaching_readiness.json");
    std::fs::write(&receipt_path, receipt.to_string()).unwrap();
    assert_eq!(get(&mut state)["status"], "stale");
    std::fs::write(&map_path, json!({ "source_id": state.workspace.book.base.book_id, "source_revision": state.workspace.book.source_fingerprint() }).to_string()).unwrap();
    let ready = get(&mut state);
    assert_eq!(ready["status"], "ready");
    assert_eq!(ready["limitations"][0], "作者省略了连接");
    let mut stages = serde_json::Map::new();
    teaching_workbench_stages(&state.workspace.book, &state.workspace.book_dir, &mut stages);
    assert_eq!(stages["teaching_publish"]["status"], "done");
    assert_eq!(stages["formal_objects"]["status"], "missing");
    assert!(BUILD_WORKBENCH_STAGE_IDS.contains(&"cognitive_materials"));
    receipt["source_revision"] = json!("old-source");
    std::fs::write(&receipt_path, receipt.to_string()).unwrap();
    assert_eq!(get(&mut state)["status"], "stale");
    teaching_workbench_stages(&state.workspace.book, &state.workspace.book_dir, &mut stages);
    assert_eq!(stages["teaching_publish"]["status"], "stale");
    receipt["source_revision"] = json!(state.workspace.book.source_fingerprint());
    receipt["coverage"]["source_review"] = json!("failed");
    std::fs::write(&receipt_path, receipt.to_string()).unwrap();
    assert_eq!(get(&mut state)["status"], "stale");
}

#[test]
fn tutor_t18_readiness_endpoint_uses_foundation_and_preserves_enabled_intent() {
    let root = tempfile::tempdir().unwrap();
    let mut state = state_named("t18-readiness");
    state.workspace.book_dir = root.path().into();
    state.user.store = MemoryStore::open(root.path().join("memory.json")).unwrap();
    t15_foundation(root.path(), &state.workspace.book);
    post(&mut state, "/tutor/mutate", r#"{"operation_id":"on","expected_revision":0,"action":{"kind":"set_enabled","enabled":true}}"#);
    for receipt in [None, Some(json!({"status":"failed"}))] {
        if let Some(receipt) = receipt {
            std::fs::write(root.path().join("teaching_readiness.json"), receipt.to_string()).unwrap();
        }
        let reply = route(&mut state, Req { method:"GET", url:"/tutor/readiness", body:"", now:"now" });
        let view: Value = serde_json::from_str(&reply.body).unwrap();
        assert_eq!(view["status"], "ready", "{view}");
        assert_ne!(view["teaching_assets"]["status"], "ready");
        assert!(view["limitations"].is_array());
        assert!(view.get("required_stages").is_none());
        assert!(state.user.learning_store().unwrap().state().unwrap().control.enabled);
    }
    std::fs::remove_file(root.path().join("book_structure.json")).unwrap();
    let reply = route(&mut state, Req { method:"GET", url:"/tutor/readiness", body:"", now:"now" });
    assert_eq!(serde_json::from_str::<Value>(&reply.body).unwrap()["status"], "preparing");
    assert!(!teaching::start_request(&state.private_context(), "now").unwrap()["started"].as_bool().unwrap());
    assert!(state.user.learning_store().unwrap().state().unwrap().control.enabled);
}

#[test]
fn tutor_api_is_global_across_chat_and_reopen_and_reports_storage_failure() {
    let root = tempfile::tempdir().unwrap();
    let memory_path = root.path().join("memory.json");
    let mut state = state_named("tutor-api");
    state.user.store = MemoryStore::open(&memory_path).unwrap();
    let read = route(
        &mut state,
        Req {
            method: "GET",
            url: "/tutor/state",
            body: "",
            now: "now",
        },
    );
    assert_eq!(read.status, 200, "{}", read.body);
    assert_eq!(
        serde_json::from_str::<Value>(&read.body).unwrap()["control"]["enabled"],
        false
    );
    let enabled = post(
        &mut state,
        "/tutor/mutate",
        r#"{"operation_id":"enable","expected_revision":0,"action":{"kind":"set_enabled","enabled":true}}"#,
    );
    assert_eq!(enabled.status, 200, "{}", enabled.body);
    let request = r#"{"operation_id":"start","expected_revision":1,"action":{"kind":"start","user_intent":"理解这个关系","explicit_constraints":[],"material_scope":[],"default_teaching_intent":null}}"#;
    let started = post(&mut state, "/tutor/mutate", request);
    assert_eq!(started.status, 200, "{}", started.body);
    let before = state.user.learning_store().unwrap().state().unwrap();
    post(&mut state, "/agent/new", "{}");
    assert_eq!(
        state.user.learning_store().unwrap().state().unwrap(),
        before
    );
    state.user.store = MemoryStore::open(&memory_path).unwrap();
    assert_eq!(
        state.user.learning_store().unwrap().state().unwrap(),
        before
    );
    let retried = post(&mut state, "/tutor/mutate", request);
    assert_eq!(retried, started);
    let conflict = post(
        &mut state,
        "/tutor/mutate",
        r#"{"operation_id":"stale","expected_revision":0,"action":{"kind":"set_enabled","enabled":false}}"#,
    );
    assert_eq!(conflict.status, 409);
    std::fs::remove_file(root.path().join("learning.db")).unwrap();
    std::fs::create_dir(root.path().join("learning.db")).unwrap();
    let failed = post(
        &mut state,
        "/tutor/mutate",
        r#"{"operation_id":"off","expected_revision":2,"action":{"kind":"set_enabled","enabled":false}}"#,
    );
    assert_eq!(failed.status, 503);
    assert!(failed.body.contains("LEARNING_STORAGE_UNAVAILABLE"));
}

// Produce the same accepted close/publication records as the automatic builder.
pub(super) fn t15_close(root: &Path, book: &Book, stage: &str, files: &[&str]) {
    let artifacts: Vec<_> = files.iter().map(|file| {
        let bytes = std::fs::read(root.join(file)).unwrap();
        json!({"path":file,"size_bytes":bytes.len(),"sha256":sha256_hex(&bytes)})
    }).collect();
    let transaction = sha256_hex(stage.as_bytes());
    let receipt = json!({"version":"automatic_build_publication_receipt.v1","stage":stage,
        "transaction_id":transaction,"status":"committed","artifacts":artifacts});
    let publication = root.join(format!(".build/automatic-build/v2/publication/{stage}/{transaction}"));
    std::fs::create_dir_all(&publication).unwrap();
    std::fs::write(publication.join("receipt.json"), receipt.to_string()).unwrap();
    let close = root.join(format!(".build/automatic-build/v2/close/{stage}/{transaction}"));
    std::fs::create_dir_all(&close).unwrap();
    std::fs::write(close.join("accepted.json"), json!({"version":"automatic_build_stage_close_result.v2",
        "stage":stage,"status":"closed","target":{"book_id":book.base.book_id,"profile_id":"technical_learning","input_fingerprint":book.source_fingerprint()},
        "quality":{"gate_status":"passed"},"publication":{"transaction_id":transaction,"receipt_digest":sha256_hex(receipt.to_string().as_bytes())},
        "postcondition":{"stage_closed":true,"policy_contracts":[{"semantic_contract":{"quality_profile":"full"}}]}}).to_string()).unwrap();
}

pub(super) fn t15_foundation(root: &Path, book: &Book) {
    std::fs::write(root.join("base.json"), serde_json::to_vec(&book.base).unwrap()).unwrap();
    std::fs::write(root.join("source.txt"), "X".repeat(100) + "尾巴").unwrap();
    std::fs::write(root.join("discourse_index.json"), "{\"items\":[]}").unwrap();
    std::fs::write(root.join("formula_semantics.json"), "[]").unwrap();
    std::fs::write(root.join("book_structure.json"), json!({"header":{"book_id":book.base.book_id,
        "book_version":"v1","profile_id":"technical_learning","profile_version":"1","core_schema_version":"core_v0","generated_at":"now"},
        "spine":[],"throughlines":[],"key_stops":[]}).to_string()).unwrap();
    t15_close(root, book, "pass1", &["base.json", "source.txt"]);
    t15_close(root, book, "profile_sidecar", &["discourse_index.json", "formula_semantics.json"]);
    t15_close(root, book, "book_structure", &["book_structure.json"]);
}

#[test]
fn tutor_t15_admission_needs_artifacts_but_no_build_receipts_optional_teaching_or_pass2() {
    let root = tempfile::tempdir().unwrap();
    let state = state_named("t15-admission");
    let book = &state.workspace.book;
    t15_foundation(root.path(), book);
    let ready = crate::tutor_api::tutor_source_readiness(book, root.path());
    assert_eq!(ready["status"], "ready", "{ready}");
    assert_eq!(ready["teaching_assets"]["status"], "preparing");
    assert!(!root.path().join("pass2_audit.json").exists());
    // The original readiness still reports complete teaching assets, independently of admission.
    assert_eq!(serde_json::from_str::<Value>(&crate::tutor_api::source_readiness(book, root.path()).body).unwrap()["status"], "preparing");
    let structure = std::fs::read(root.path().join("book_structure.json")).unwrap();
    std::fs::remove_file(root.path().join("book_structure.json")).unwrap();
    assert_ne!(crate::tutor_api::tutor_source_readiness(book, root.path())["status"], "ready");
    std::fs::write(root.path().join("book_structure.json"), &structure).unwrap();
    std::fs::write(root.path().join("book_structure.json"), "{}").unwrap();
    assert_ne!(crate::tutor_api::tutor_source_readiness(book, root.path())["status"], "ready");
    std::fs::write(root.path().join("book_structure.json"), &structure).unwrap();
    for stage in ["pass1", "profile_sidecar", "book_structure"] {
        let close = root.path().join(format!(".build/automatic-build/v2/close/{stage}"));
        let absent = close.with_extension("saved");
        std::fs::rename(&close, &absent).unwrap();
        assert_eq!(crate::tutor_api::tutor_source_readiness(book, root.path())["status"], "ready", "{stage}");
        std::fs::rename(absent, close).unwrap();
    }
}

#[test]
fn tutor_t15_optional_assets_do_not_change_admission_and_only_accepted_refs_are_offered() {
    let root = tempfile::tempdir().unwrap();
    let state = state_named("t15-assets");
    let book = &state.workspace.book;
    t15_foundation(root.path(), book);
    let read = || crate::tutor_api::tutor_source_readiness(book, root.path());
    let artifact = json!({"version":"formal_objects.v1","source_id":book.base.book_id,"source_revision":book.source_fingerprint(),"revision":2});
    std::fs::write(root.path().join("formal_objects.json"), artifact.to_string()).unwrap();
    assert_eq!(read()["teaching_assets"]["formal_objects"]["status"], "unavailable");
    t15_close(root.path(), book, "formal_objects", &["formal_objects.json"]);
    assert_eq!(read()["teaching_assets"]["formal_objects"]["revision"], 2);
    let map = root.path().join("teaching/versions/v1/map.json");
    std::fs::create_dir_all(map.parent().unwrap()).unwrap();
    std::fs::write(&map, json!({"source_id":book.base.book_id,"source_revision":book.source_fingerprint()}).to_string()).unwrap();
    let mut receipt = json!({"version":"teaching_readiness.v1","status":"ready","source_id":book.base.book_id,
        "source_revision":book.source_fingerprint(),"teaching_map_revision":"v1","map_path":"teaching/versions/v1/map.json",
        "coverage":{"source":"complete","structure":"complete","objects":"complete","cognitive_materials":"complete","source_review":"passed"},"limitations":[]});
    std::fs::write(root.path().join("teaching_readiness.json"), receipt.to_string()).unwrap();
    assert_eq!(read()["teaching_assets"]["teaching_map_revision"], "v1");
    receipt["source_revision"] = json!("old");
    std::fs::write(root.path().join("teaching_readiness.json"), receipt.to_string()).unwrap();
    assert_eq!(read()["status"], "ready");
    assert_eq!(read()["teaching_assets"]["status"], "stale");
    assert!(read()["teaching_assets"]["teaching_map_revision"].is_null());
    std::fs::write(root.path().join("formal_objects.json"), json!({"source_revision":"old"}).to_string()).unwrap();
    assert_eq!(read()["teaching_assets"]["formal_objects"]["status"], "stale");
    assert_eq!(read()["status"], "ready");
}

#[test]
fn tutor_t15_ignores_build_history_but_rejects_wrong_publication_source() {
    let root = tempfile::tempdir().unwrap();
    let state = state_named("t15-acceptance");
    let book = &state.workspace.book;
    t15_foundation(root.path(), book);
    // Broken optional job/Pass2 state is unrelated to the accepted prerequisites.
    std::fs::create_dir_all(root.path().join(".build/jobs")).unwrap();
    std::fs::write(root.path().join(".build/jobs/failed.json"), "invalid unfinished job").unwrap();
    std::fs::write(root.path().join("pass2_audit.json"), "invalid optional audit").unwrap();
    assert_eq!(crate::tutor_api::tutor_source_readiness(book, root.path())["status"], "ready");
    let close = root.path().join(format!(".build/automatic-build/v2/close/book_structure/{}/accepted.json", sha256_hex(b"book_structure")));
    let original = std::fs::read(&close).unwrap();
    let mut value: Value = serde_json::from_slice(&original).unwrap();
    value["quality"]["gate_status"] = json!("failed");
    std::fs::write(&close, value.to_string()).unwrap();
    assert_eq!(crate::tutor_api::tutor_source_readiness(book, root.path())["status"], "ready");
    std::fs::write(&close, original).unwrap();
    std::fs::write(root.path().join("publication.json"), json!({"source_fingerprint":book.source_fingerprint()}).to_string()).unwrap();
    let changed = Book::new(book.base.clone(), &("Y".repeat(100) + "尾巴"));
    assert_ne!(crate::tutor_api::tutor_source_readiness(&changed, root.path())["status"], "ready");
}

#[test]
fn tutor_t15_paper_uses_public_artifacts_without_private_reconciliation() {
    let root = tempfile::tempdir().unwrap();
    let state = state_named("t15-paper");
    let base_book = &state.workspace.book;
    t15_foundation(root.path(), base_book);
    let mut structure: Value = serde_json::from_slice(&std::fs::read(root.path().join("book_structure.json")).unwrap()).unwrap();
    structure["header"]["profile_id"] = json!("paper");
    std::fs::write(root.path().join("book_structure.json"), structure.to_string()).unwrap();
    let book = Book::new(base_book.base.clone(), &("X".repeat(100) + "尾巴"))
        .with_book_structure(Some(serde_json::from_value(structure.clone()).unwrap()));
    assert_eq!(current_content_profile(&book), "paper");
    std::fs::write(root.path().join("profile_metadata.json"), json!({"header":structure["header"]}).to_string()).unwrap();
    let fingerprint = json!({"paper_md_sha256":"md", "paper_pdf_sha256":"pdf", "config_hash":workbench_config_hash()});
    let input = root.path().join(".build/input");
    std::fs::create_dir_all(&input).unwrap();
    std::fs::write(input.join("manifest.json"), json!({"book_id":book.base.book_id,"profile_id":"paper","fingerprint":fingerprint}).to_string()).unwrap();
    let reconciliation = root.path().join(".build/source-reconciliation");
    std::fs::create_dir_all(&reconciliation).unwrap();
    let mut report = json!({"book_id":book.base.book_id,"input_fingerprint":fingerprint,"unresolved":[]});
    std::fs::write(reconciliation.join("report.json"), report.to_string()).unwrap();
    let mut manifest = json!({"version":"source_manifest.v2","book_id":book.base.book_id,
        "canonical_source":{"path":"source.txt","sha256":book.source_fingerprint()},"capabilities":{}});
    std::fs::write(root.path().join("source_manifest.json"), manifest.to_string()).unwrap();
    let target_input = sha256_hex(format!("{{\"paper_md_sha256\":\"md\",\"paper_pdf_sha256\":\"pdf\",\"config_hash\":\"{}\"}}", workbench_config_hash()).as_bytes());
    for (stage, files) in [("pass1", vec!["base.json","profile_metadata.json"]),
        ("profile_sidecar", vec!["discourse_index.json","formula_semantics.json"]), ("book_structure",vec!["book_structure.json"])] {
        t15_close(root.path(), &book, stage, &files);
        let file = root.path().join(format!(".build/automatic-build/v2/close/{stage}/{}/accepted.json",sha256_hex(stage.as_bytes())));
        let mut close: Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
        close["target"]["profile_id"] = json!("paper");
        close["target"]["input_fingerprint"] = json!(target_input);
        std::fs::write(file, close.to_string()).unwrap();
    }
    let read = || crate::tutor_api::tutor_source_readiness(&book, root.path());
    assert_eq!(read()["status"], "ready", "{}", read());
    report["unresolved"] = json!([{"id":"unresolved-source-block"}]);
    std::fs::write(reconciliation.join("report.json"), report.to_string()).unwrap();
    assert_eq!(read()["status"], "ready");
    std::fs::rename(root.path().join(".build"), root.path().join("unused-build-history")).unwrap();
    assert_eq!(read()["status"], "ready");
    report["unresolved"] = json!([]);
    manifest["canonical_source"]["sha256"] = json!("stale");
    std::fs::write(root.path().join("source_manifest.json"), manifest.to_string()).unwrap();
    assert_eq!(read()["blocked_stage"], "hybrid_foundation");
}
