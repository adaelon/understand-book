//! TB5: captured TB4 understanding through the existing UI routes and storage.
use super::*;
use memory::learning_evidence::LearningEvidence;
use memory::teaching::TeachingEvent;

fn directory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/performance/tutor-building-blocks-tb5")
}

fn captured_fixture() -> (tempfile::TempDir, AppState, Value) {
    let (root, state) = super::tutor_building_blocks_tests::fixture();
    let tb4 = directory().parent().unwrap().join("tutor-building-blocks-tb4/browser");
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
    // Restore the captured historical row as stored. New writes deliberately
    // reject its mixed learner-expression and assistant-delivery fact_refs.
    let db=rusqlite::Connection::open(root.path().join("learning.db")).unwrap();
    db.execute("INSERT INTO learning_evidence(learning_workspace_id,evidence_id,source_id,evidence) VALUES ('',?,?,?)",
        rusqlite::params![evidence.evidence_id,evidence.source_id,serde_json::to_string(&evidence).unwrap()]).unwrap();
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
    for field in ["prompt", "learner_quote", "assistance_count"] {
        assert_eq!(corrected[field], detail[field], "{field}");
    }
    assert_eq!(corrected["fact_refs"], json!([original["action_ref"]]), "new corrections keep original words as their basis, with delivered help separate");
    let historic=response(&mut state, "/tutor/evidence", json!({"evidence_ref":original["evidence_id"]}));
    assert_eq!(historic["superseded"],true);
    let mut expected=detail.clone();expected["superseded"]=json!(true);
    assert_eq!(historic,expected,"the original record stays unchanged; its read-time correction status updates");
    let view = response(&mut state, "/tutor/understanding", json!({}));
    assert_eq!(view["interpretations"].as_array().unwrap().len(), 1);
    assert_eq!(view["interpretations"][0]["evidence_id"], saved["evidence_ref"]);
    assert!(view["rows"].as_array().unwrap().is_empty());
    let timeline=response(&mut state,"/tutor/timeline",json!({"evidence_ref":saved["evidence_ref"]}));
    assert_eq!(timeline["order"],"oldest_first");
    assert_eq!(timeline["entries"][0]["evidence_id"],original["evidence_id"]);
    assert_eq!(timeline["entries"][0]["superseded"],true);
    assert_eq!(timeline["entries"][1]["supersedes"],original["evidence_id"]);
    assert!(timeline["entries"][0]["occurred_at"].is_null(),"the fixture lacks an original timestamp; do not invent one");
}

#[test]
fn tutor_understanding_timeline_exposes_original_times_and_typed_input_time_support() {
    let (_root,mut state,original)=captured_fixture();
    let mut store=state.user.learning_store().unwrap();
    let template=store.teaching_event(original["action_ref"].as_str().unwrap()).unwrap();
    let mut before=template.clone();before.event_id="timeline:first".into();before.binding.turn_id="first-turn".into();
    before.occurred_at="2026-10-24T03:00:00Z".into();before.payload=json!({"usage":{"kind":"user_message"},"response":"我不明白 Copy 和共享引用的关系"});
    let mut support=template.clone();support.event_id="timeline:delivered".into();support.kind=memory::teaching::TeachingFact::UsageObserved;
    support.occurred_at="2026-10-24T03:01:00Z".into();support.payload=json!({"usage":{"kind":"assistant_delivery"},"answer":"Copy 描述值的复制，共享引用决定访问权限。"});
    let mut after=template.clone();after.event_id="timeline:second".into();after.binding.turn_id="second-turn".into();
    after.occurred_at="2026-10-24T03:02:00Z".into();after.payload=json!({"usage":{"kind":"user_message"},"response":"Copy 本身不让共享引用可写，对吗？"});
    for fact in [&before,&support,&after] { store.append_teaching(fact).unwrap(); }
    let mut first:LearningEvidence=serde_json::from_value(original.clone()).unwrap();
    first.evidence_id="timeline:interpretation:first".into();first.action_ref=before.event_id.clone();first.fact_refs=vec![before.event_id.clone()];first.learner_quote=before.learner_expression().unwrap().into();
    first.target.as_mut().unwrap().object_refs=vec![memory::teaching::TeachingObjectRevision {source_id:first.source_id.clone(),object_id:"copy-access".into(),object_revision:1}];
    let mut second=first.clone();second.evidence_id="timeline:interpretation:second".into();second.action_ref=after.event_id.clone();second.fact_refs=vec![after.event_id.clone()];second.learner_quote=after.learner_expression().unwrap().into();
    second.support_delivery_refs=vec![support.event_id.clone()];
    // Delayed extraction of the first expression must not reorder the learner's history.
    store.append_evidence(&second).unwrap();store.append_evidence(&first).unwrap();
    drop(store);
    let view=response(&mut state,"/tutor/understanding",json!({}));
    assert_eq!(view["interpretations"][0]["evidence_id"],second.evidence_id);
    assert_eq!(view["interpretations"][0]["occurred_at"],after.occurred_at);
    assert_eq!(view["interpretations"][0]["source_session_id"],after.binding.chat_session_id);
    assert_eq!(view["interpretations"][0]["source_turn_id"],"second-turn");
    let timeline=response(&mut state,"/tutor/timeline",json!({"evidence_ref":second.evidence_id}));
    assert_eq!(timeline["entries"].as_array().unwrap().len(),2);
    assert_eq!(timeline["entries"][0]["learner_quote"],before.learner_expression().unwrap());
    let latest=&timeline["entries"][1];
    assert_eq!(latest["assistance_count"],1);
    assert_eq!(latest["assistance"][0]["kind"],"usage_observed");
    assert_eq!(latest["assistance"][0]["text"],support.payload["answer"]);
    assert_eq!(latest["assistance_refs"],json!([]));
    assert_eq!(latest["support_delivery_refs"],json!([support.event_id]));
    assert_eq!(latest["expressions"][0]["text"],after.learner_expression().unwrap());
    assert_eq!(latest["superseded"],false,"later observations are not corrections of accurate past observations");
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
