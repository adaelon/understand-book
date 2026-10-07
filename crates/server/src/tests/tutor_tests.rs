use super::*;

#[test]
fn tutor_readiness_is_separate_from_control_and_rejects_stale_or_missing_publication() {
    let root = tempfile::tempdir().unwrap();
    let mut state = state_named("teaching-ready");
    state.workspace.book_dir = root.path().to_path_buf();
    let get = |state: &mut AppState| {
        let reply = route(state, Req { method: "GET", url: "/tutor/readiness", body: "", now: "now" });
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
