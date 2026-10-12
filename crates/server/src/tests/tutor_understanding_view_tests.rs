//! TB5: captured TB4 understanding through the existing UI routes and storage.
use super::*;
use memory::learning_evidence::LearningEvidence;
use memory::teaching::TeachingEvent;

fn directory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/performance/tutor-building-blocks-tb5")
}

fn captured_fixture() -> (tempfile::TempDir, AppState, Value) {
    let (root, state) = super::tutor_building_blocks_tests::fixture();
    let tb4 = directory().join("../tutor-building-blocks-tb4/browser");
    let read = |file: &str| -> Value {
        serde_json::from_str(&std::fs::read_to_string(tb4.join(file)).unwrap()).unwrap()
    };
    let result = read("followup-result.json");
    let context = read("followup-context.json");
    let evidence: LearningEvidence = serde_json::from_value(result["evidence"].clone()).unwrap();
    assert_eq!(evidence.source_revision, state.workspace.book.source_fingerprint());
    let mut store = state.user.learning_store().unwrap();
    for id in &evidence.fact_refs {
        let fact = context["read_facts"].as_array().unwrap().iter().find(|fact| fact["event_id"] == *id).unwrap();
        // Trace reads contain the original binding and payload, but not the
        // event timestamp/causal envelope. This is an isolated view fixture.
        let event: TeachingEvent = serde_json::from_value(json!({
            "event_id":fact["event_id"],"binding":fact["binding"],"kind":fact["kind"],
            "payload":fact["content"],"causal_refs":[],"occurred_at":"tb5-captured-fixture"
        })).unwrap();
        store.append_teaching(&event).unwrap();
    }
    store.append_evidence(&evidence).unwrap();
    store.rebuild_learner_projection(&evidence.source_id).unwrap();
    (root, state, result["evidence"].clone())
}

fn response(state: &mut AppState, url: &str, body: Value) -> Value {
    let reply = post(state, url, &body.to_string());
    assert_eq!(reply.status, 200, "{}", reply.body);
    serde_json::from_str(&reply.body).unwrap()
}

#[test]
fn tutor_tb5_captured_hypothesis_keeps_conditions_and_correction_without_performance() {
    let (_root, mut state, original) = captured_fixture();
    let detail = response(&mut state, "/tutor/evidence", json!({"evidence_ref":original["evidence_id"]}));
    assert_eq!(detail["prompt"], original["target"]["expected_performance"]);
    assert_eq!(detail["interpretation"], original["interpretation"]);
    assert_eq!(detail["learner_quote"], original["learner_quote"]);
    assert_eq!(detail["assistance_count"], 0);
    assert!(detail["prompt"].as_str().unwrap().contains("此前讲解和教具已显示结果"));
    let saved = response(&mut state, "/tutor/correct", json!({
        "evidence_ref":original["evidence_id"],"operation_id":"tb5-correction",
        "text":"我只能说明这两组的结果，还需要帮助把理由接起来。"
    }));
    let corrected = response(&mut state, "/tutor/evidence", saved.clone());
    assert_eq!(corrected["nature"], "hypothesis");
    assert_eq!(corrected["status"], "unassessed");
    for field in ["prompt", "learner_quote", "fact_refs", "assistance_count"] {
        assert_eq!(corrected[field], detail[field], "{field}");
    }
    let view = response(&mut state, "/tutor/understanding", json!({}));
    assert_eq!(view["interpretations"].as_array().unwrap().len(), 1);
    assert_eq!(view["interpretations"][0]["evidence_id"], saved["evidence_ref"]);
    assert!(view["rows"].as_array().unwrap().is_empty());
}

#[test]
#[ignore = "TB5 local browser fixture on 4187; no model calls"]
fn tutor_tb5_understanding_browser_host() {
    let (root, mut state, original) = captured_fixture();
    let initial = response(&mut state, "/tutor/evidence", json!({"evidence_ref":original["evidence_id"]}));
    let save = |name: &str, value: &Value| {
        std::fs::create_dir_all(directory().join("browser")).unwrap();
        std::fs::write(directory().join("browser").join(name), serde_json::to_vec_pretty(value).unwrap()).unwrap();
    };
    save("initial-evidence.json", &initial);
    let server = tiny_http::Server::http("127.0.0.1:4187").unwrap();
    println!("TB5_BROWSER_READY");
    let deadline = std::time::Instant::now() + Duration::from_secs(900);
    let mut stopped = false;
    while std::time::Instant::now() < deadline {
        let Some(mut request) = server.recv_timeout(Duration::from_secs(1)).unwrap() else { continue; };
        let path = request.url().to_string();
        let mut body = String::new();
        request.as_reader().read_to_string(&mut body).unwrap();
        let reply = match path.as_str() {
            "/fixture" => ok_json(&initial),
            "/reopen" => {
                state.user.store = MemoryStore::open(root.path().join("memory.json")).unwrap();
                ok_json(&json!({"ok":true}))
            },
            "/stop" => {
                request.respond(tiny_http::Response::from_string("stopped")).unwrap();
                stopped = true;
                break;
            },
            _ => route(&mut state, Req { method:request.method().as_str(),url:&path,body:&body,now:"tb5-browser" }),
        };
        request.respond(tiny_http::Response::from_string(reply.body).with_status_code(reply.status)
            .with_header(tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap())).unwrap();
    }
    assert!(stopped, "browser scenario did not finish");
    let view = response(&mut state, "/tutor/understanding", json!({}));
    assert!(view["rows"].as_array().unwrap().is_empty());
    let corrected = response(&mut state, "/tutor/evidence", json!({"evidence_ref":view["interpretations"][0]["evidence_id"]}));
    assert!(corrected["correction"].is_string());
    assert_eq!(corrected["status"], "unassessed");
    assert_eq!(corrected["prompt"], initial["prompt"]);
    assert_eq!(corrected["learner_quote"], initial["learner_quote"]);
    save("corrected-evidence.json", &corrected);
    save("understanding.json", &view);
}
