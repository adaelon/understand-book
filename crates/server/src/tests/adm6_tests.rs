use super::mu5_tests::Fixture;
use runtime::ProviderConfig;
use serde_json::{json, Value};
use std::time::Duration;

fn admit(f: &Fixture, w: &Value, input: &Value) -> Value {
    let (status, value) = f.call(
        &f.a,
        "POST",
        &format!(
            "/api/workspaces/{}/agent/runs",
            w["workspace_id"].as_str().unwrap()
        ),
        input.clone(),
    );
    assert_eq!(status, 202, "{value}");
    value
}
fn status(f: &Fixture, turn: &str) -> Value {
    f.ok(&f.a, "GET", &format!("/api/agent/runs/{turn}"), json!({}))
}
fn respond(request: tiny_http::Request, message: Value, tokens: u32) {
    request.respond(tiny_http::Response::from_string(json!({
        "choices":[{"message":message,"finish_reason":"stop"}],
        "usage":{"prompt_tokens":tokens,"prompt_cache_hit_tokens":0,"completion_tokens":1,"total_tokens":tokens+1}
    }).to_string()).with_header(tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap())).unwrap();
}

// This fixture builds eight real turns before testing the review worker. Drain
// context maintenance and observing-only coaching without consuming its script.
fn next_review_fixture_request(fake: &tiny_http::Server) -> tiny_http::Request {
    loop {
        let mut request = fake.recv_timeout(Duration::from_secs(15)).unwrap().unwrap();
        let mut body = String::new();
        request.as_reader().read_to_string(&mut body).unwrap();
        let body: Value = serde_json::from_str(&body).unwrap();
        let messages = body["messages"].as_array().unwrap();
        let system = messages.first().and_then(|message| message["content"].as_str()).unwrap_or("");
        let tail = messages.last().filter(|message| message["role"] == "user")
            .and_then(|message| message["content"].as_str()).unwrap_or("");
        let output = if system.starts_with(runtime::compaction::COMPACTION_GENERATION_PROMPT)
            || tail.starts_with(runtime::compaction::COMPACTION_GENERATION_PROMPT) {
            // Both the old standalone request and the appended source map carry
            // eligible_items as one JSON line; repair diagnostics are separate.
            let input = tail.lines().filter_map(|line| serde_json::from_str::<Value>(line).ok())
                .find(|value| value["eligible_items"].is_array()).expect("compaction source map");
            let ids = input["eligible_items"].as_array().unwrap().iter()
                .map(|item| item["source_item_id"].as_str().unwrap().to_owned()).collect::<Vec<_>>();
            assert!(!ids.is_empty());
            let item_id = "review.goal".to_owned();
            Some(serde_json::to_value(runtime::CompactionDraft {
                active_goal: vec![runtime::SourcedCheckpointItem {
                    item_id: item_id.clone(),
                    text: "Earlier turns requested brief source explanations and received an invitation to ask a question.".into(),
                    source_item_ids: ids.clone(), evidence_refs: vec![],
                }],
                progress: vec![], decisions: vec![], user_constraints: vec![], open_obligations: vec![],
                unresolved_ambiguities: vec![], critical_facts: vec![], critical_examples: vec![], next_steps: vec![],
                source_coverage: ids.into_iter().map(|source_item_id| runtime::SourceCoverage {
                    source_item_id, disposition: runtime::SourceDisposition::Compacted,
                    target_item_ids: vec![item_id.clone()], reason: None,
                }).collect(),
            }).unwrap())
        } else if system.starts_with("你是 Understand Book 的教学教练。") {
            let input: Value = serde_json::from_str(tail).unwrap();
            assert_eq!(input["trigger"], "after_turn");
            assert_eq!(input["teaching_context"]["status"], "observing");
            let observations: Vec<_> = input["observation_facts"].as_array().unwrap().iter()
                .filter(|fact| fact["already_interpreted"] != true)
                .map(|fact| json!({"fact_ref":fact["fact_ref"], "decision":"no_new_information",
                    "reason":"This review transport fixture does not assess learner understanding."})).collect();
            Some(json!({"next_step":{"focus":"Keep the accepted Resident task",
                "participation":{"choice":"conclude", "reason":"Observation only."}},
                "observations":observations, "reason":"The delivered response is already recorded."}))
        } else { None };
        if let Some(output) = output {
            respond(request, json!({"role":"assistant", "content":output.to_string()}), 10);
        } else {
            return request;
        }
    }
}

#[test]
fn multi_user_review_real_transport_is_billed_and_durable() {
    let fake = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let url = format!("http://{}",fake.server_addr().to_ip().unwrap());
    let mut f = Fixture::new();
    super::adm5_tests::seed(&mut f,&url,10_000_000);
    f.access.runs.configure(ProviderConfig::from_values("native","fixture-key",url.clone(),"fixture-model").unwrap()).unwrap();
    let w = f.create(&f.a,&f.x,"page");
    let mut w = f.action(&f.a,&w,"page","chat/new",json!({}));
    // Eight unreviewed turns make the interrupted job eligible immediately.
    for ordinal in 0..8 {
        let mut input = Fixture::stamp(&w,"page");
        input["client_request_id"] = json!(format!("review-transport-{ordinal}"));
        input["session_id"] = w["selected_chat"].clone();
        input["message"] = json!("Explain the source briefly.");
        admit(&f,&w,&input);
        let access = f.access.clone();
        let worker = std::thread::spawn(move || access.runs.run_one(&access));
        respond(next_review_fixture_request(&fake),json!({"role":"assistant","content":"We can start with your question."}),10);
        worker.join().unwrap().unwrap();
        w = f.get(&f.a,&w);
    }
    assert_eq!(f.access.users.lock().unwrap().pending_review_owners().unwrap(),std::collections::VecDeque::from(["A".to_string()]));
    let mut backlog = std::collections::VecDeque::new();
    crate::multi_user_review::remember_pending(&f.access, &mut backlog);
    f.access.users.lock().unwrap().evict_idle(std::time::Instant::now()+crate::user_registry::USER_IDLE_TTL+Duration::from_secs(1)).unwrap();
    assert!(f.access.users.lock().unwrap().loaded_users().is_empty());
    // Persist an interrupted attempt, then release the entire old service.
    {
        let owner = backlog.pop_front().unwrap();
        let handle = f.access.users.lock().unwrap().load_for_review(&owner, "1000").unwrap();
        let mut user = handle.lock().unwrap();
        let job = user.store.review_state().review_jobs[0].job_id.clone();
        user.store.claim_review_job(&job, "1000").unwrap();
    }
    let paths = f.access.users.lock().unwrap().paths("A").unwrap();
    let Fixture { root, access, control, .. } = f;
    drop(access);
    drop(control);
    let access = std::sync::Arc::new(crate::authorization::Authorization::new(
        crate::user_registry::UserRegistry::open(root.path()).unwrap(),
    ).unwrap());
    assert!(access.users.lock().unwrap().loaded_users().is_empty(), "restart must not depend on a login");
    access.runs.configure(ProviderConfig::from_values("native", "fixture-key", url, "fixture-model").unwrap()).unwrap();
    let control = crate::control_store::ControlStore::open(access.users.lock().unwrap().writer()).unwrap();
    let server = crate::multi_user_host::start_with_access(
        "127.0.0.1:0".parse().unwrap(),
        std::sync::Arc::new(crate::multi_user_host::Site::new("https://reader.example").unwrap()),
        access.clone(),
    ).unwrap();
    // Invalid extractor output is charged, retained for retry, then recovered.
    respond(next_review_fixture_request(&fake),json!({"role":"assistant","content":"{\"candidate_facts\":\"invalid\"}"}),10);
    respond(next_review_fixture_request(&fake),json!({"role":"assistant","content":json!({"candidate_facts":[],"intent_observations":[]}).to_string()}),10);
    // Wait for commit before shutdown, which intentionally cancels in-flight I/O.
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        if access.users.lock().unwrap().loaded_users().iter().any(|user| {
            user.lock().unwrap().store.review_state().review_jobs.iter()
                .any(|job| job.status == memory::ReviewJobStatus::Completed)
        }) { break; }
        std::thread::sleep(Duration::from_millis(20));
    }
    server.shutdown();
    let charges: i64 = control.connection.query_row("SELECT count(*) FROM model_call_charges WHERE user_id='A' AND purpose='memory_review' AND task_ref IS NOT NULL AND state='settled'",[],|r|r.get(0)).unwrap();
    assert_eq!(charges,2);
    let (parents, scopes): (i64, i64) = control.connection.query_row(
        "SELECT count(DISTINCT parent_task_ref),count(learning_workspace_id) FROM model_call_charges WHERE user_id='A' AND purpose='memory_review'",
        [], |r| Ok((r.get(0)?, r.get(1)?)),
    ).unwrap();
    assert_eq!(parents, 1, "review retries remain under the original durable job");
    assert_eq!(scopes, 0, "account-wide profile extraction does not inherit an open reading scene");
    assert!(access.users.lock().unwrap().pending_review_owners().unwrap().is_empty());
    let persisted: Value = serde_json::from_slice(&std::fs::read(paths.memory).unwrap()).unwrap();
    assert_eq!(persisted["review_state"]["review_jobs"][0]["status"],"completed");
    assert_eq!(persisted["review_state"]["review_jobs"][0]["attempts"],3);
}

#[test]
fn adm6_real_run_preserves_note_goal_and_explicit_continue_uses_new_call() {
    for fail_save in [false, true] {
        let fake = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let url = format!("http://{}", fake.server_addr().to_ip().unwrap());
        let mut f = Fixture::new();
        let period = super::adm5_tests::seed(&mut f, &url, 1_000_000);
        f.access
            .runs
            .configure(
                ProviderConfig::from_values("native", "fixture-key", url, "fixture-model").unwrap(),
            )
            .unwrap();
        let w = f.create(&f.a, &f.x, "page");
        let w = f.action(&f.a, &w, "page", "chat/new", json!({}));
        let mut input = Fixture::stamp(&w, "page");
        input["session_id"] = w["selected_chat"].clone();
        input["client_request_id"] = json!("adm6-original");
        input["message"] = json!("请在 1.1 保存笔记，再解释这段内容");
        let accepted = admit(&f, &w, &input);
        let turn = accepted["turn_id"].as_str().unwrap();
        let access = f.access.clone();
        let worker = std::thread::spawn(move || access.runs.run_one(&access));
        let request = fake.recv_timeout(Duration::from_secs(10)).unwrap().unwrap();
        if fail_save {
            let handle = f.access.users.lock().unwrap().get("A", "now").unwrap();
            let mut user = handle.lock().unwrap();
            let log = user
                .session_store
                .as_mut()
                .unwrap()
                .logs
                .get_mut(input["session_id"].as_str().unwrap())
                .unwrap();
            log.fail_write = Some(false);
            log.fail_terminal_only = true;
        }
        respond(
            request,
            json!({"role":"assistant","content":null,"tool_calls":[{
                "id":"adm6-note","type":"function","function":{"name":"reader.note","arguments":json!({"lid":"1.1","text":"adm6 durable result"}).to_string()}
            }]}),
            1_000_001,
        );
        worker.join().unwrap().unwrap();
        assert!(
            fake.try_recv().unwrap().is_none(),
            "stopped before second HTTP"
        );
        let stopped = status(&f, turn);
        if fail_save {
            assert_eq!(
                stopped["snapshot"]["persistence_state"], "failed",
                "{stopped}"
            );
            let error = &stopped["snapshot"]["error"];
            assert_eq!(error["error_code"], "TURN_UNSAVED");
            assert_eq!(
                error["execution_error"]["error_code"],
                "ALLOWANCE_INSUFFICIENT"
            );
            assert!(!error.to_string().contains("已保存"));
            f.ok(
                &f.a,
                "POST",
                &format!("/api/agent/runs/{turn}/retry-save"),
                json!({}),
            );
            assert!(
                fake.try_recv().unwrap().is_none(),
                "retry-save is not a model retry"
            );
        }
        let stopped = status(&f, turn);
        assert_eq!(stopped["turn"]["status"], "failed", "{stopped}");
        assert_eq!(
            stopped["turn"]["error"]["error_code"],
            "ALLOWANCE_INSUFFICIENT"
        );
        assert!(stopped["turn"]["error"]["message"]
            .as_str()
            .unwrap()
            .contains("已保存"));
        let handle = f.access.users.lock().unwrap().get("A", "now").unwrap();
        let (goal, note_id) = {
            let user = handle.lock().unwrap();
            let session = user
                .agent_history
                .sessions
                .iter()
                .find(|s| Some(s.id.as_str()) == input["session_id"].as_str())
                .unwrap();
            assert_eq!(session.goals[0].status, runtime::goal::GoalStatus::Open);
            assert_eq!(
                session.goals[0].last_stop_reason.as_deref(),
                Some("ALLOWANCE_INSUFFICIENT")
            );
            assert!(session
                .messages
                .iter()
                .any(|m| m.tool_call_id.as_deref() == Some("adm6-note")));
            let notes = user.store.recall(&memory::RecallQuery {
                text: Some("adm6 durable result".into()),
                ..Default::default()
            });
            assert_eq!(notes.len(), 1);
            (
                session.goals[0].id.clone(),
                serde_json::to_value(&notes[0]).unwrap(),
            )
        };
        // Old accepted request is a lookup, never another execution.
        assert_eq!(admit(&f, &w, &input)["turn_id"], turn);
        assert!(!f.access.runs.run_one(&f.access).unwrap());
        // Ordinary reading and history remain available at negative allowance.
        let w = f.get(&f.a, &w);
        assert_eq!(w["selected_chat"], input["session_id"]);
        f.ok(&f.a, "GET", "/api/account/usage", json!({}));
        f.ok(&f.b, "POST", "/api/admin/users/A/allowance-adjustments", json!({
            "operation_id":"adm6-refill","period_id":period,"revision":1,"delta_micro_cny":2_000_000,"reason":"continue fixture"
        }));
        assert!(
            !f.access.runs.run_one(&f.access).unwrap(),
            "refill does not resume automatically"
        );
        let mut next = Fixture::stamp(&w, "page");
        next["session_id"] = input["session_id"].clone();
        next["client_request_id"] = json!("adm6-continue");
        next["message"] = json!("继续");
        next["goal_id"] = json!(goal);
        let continued = admit(&f, &w, &next);
        assert_ne!(continued["turn_id"], turn);
        let access = f.access.clone();
        let worker = std::thread::spawn(move || access.runs.run_one(&access));
        let mut request = fake.recv_timeout(Duration::from_secs(10)).unwrap().unwrap();
        let mut body = String::new();
        request.as_reader().read_to_string(&mut body).unwrap();
        assert!(
            body.contains("adm6-note"),
            "new run receives the committed tool receipt"
        );
        respond(
            request,
            json!({"role":"assistant","content":"笔记已保留，继续解释当前内容。"}),
            20,
        );
        worker.join().unwrap().unwrap();
        assert_eq!(
            status(&f, continued["turn_id"].as_str().unwrap())["turn"]["status"],
            "completed"
        );
        let user = handle.lock().unwrap();
        let notes = user.store.recall(&memory::RecallQuery {
            text: Some("adm6 durable result".into()),
            ..Default::default()
        });
        assert_eq!(notes.len(), 1);
        assert_eq!(serde_json::to_value(&notes[0]).unwrap(), note_id);
        drop(user);
        let usage = f.ok(&f.a, "GET", "/api/account/usage", json!({}));
        let calls = usage["items"].as_array().unwrap();
        assert_eq!(calls.len(), 2);
        assert_ne!(calls[0]["call_id"], calls[1]["call_id"]);
        assert_ne!(calls[0]["run_ref"], calls[1]["run_ref"]);
        assert!(fake.try_recv().unwrap().is_none());
    }
}

#[test]
fn adm6_real_admission_denials_are_saved_without_http() {
    for reason in ["insufficient", "expired", "rate", "storage"] {
        let fake = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let url = format!("http://{}", fake.server_addr().to_ip().unwrap());
        let mut f = Fixture::new();
        let period = super::adm5_tests::seed(
            &mut f,
            &url,
            if reason == "insufficient" {
                1
            } else {
                1_000_000
            },
        );
        let expected = match reason {
            "expired" => {
                f.control
                    .connection
                    .execute(
                        "UPDATE allowance_periods SET expires_at=? WHERE period_id=?",
                        rusqlite::params![crate::multi_user_host::now() - 1, period],
                    )
                    .unwrap();
                "ALLOWANCE_EXPIRED"
            }
            "rate" => {
                std::fs::write(f.root.path().join("model-rates.json"), "{}").unwrap();
                std::sync::Arc::get_mut(&mut f.access).unwrap().spend =
                    std::sync::Arc::new(crate::model_spend_store::ModelSpendStore::new(
                        crate::control_store::ControlStore::open(f.control.writer.clone()).unwrap(),
                    ));
                "MODEL_RATE_UNAVAILABLE"
            }
            "storage" => {
                f.control.connection.execute_batch("CREATE TRIGGER adm6_fail BEFORE INSERT ON model_call_charges BEGIN SELECT RAISE(ABORT,'fixture'); END;").unwrap();
                "MODEL_SPEND_STORAGE_UNAVAILABLE"
            }
            _ => "ALLOWANCE_INSUFFICIENT",
        };
        f.access
            .runs
            .configure(
                ProviderConfig::from_values("native", "fixture-key", url, "fixture-model").unwrap(),
            )
            .unwrap();
        let w = f.create(&f.a, &f.x, "page");
        let w = f.action(&f.a, &w, "page", "chat/new", json!({}));
        let mut input = Fixture::stamp(&w, "page");
        input["session_id"] = w["selected_chat"].clone();
        input["client_request_id"] = json!("adm6-denied");
        input["message"] = json!("你好");
        let accepted = admit(&f, &w, &input);
        f.access.runs.run_one(&f.access).unwrap();
        let result = status(&f, accepted["turn_id"].as_str().unwrap());
        assert_eq!(result["turn"]["error"]["error_code"], expected, "{result}");
        assert!(fake.try_recv().unwrap().is_none());
        assert!(!f.get(&f.a, &w).is_null());
    }
}


#[test]
fn w3_background_review_real_transport_keeps_origin_after_switch_and_archive() {
    let fake = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let url = format!("http://{}", fake.server_addr().to_ip().unwrap());
    let mut f = Fixture::new();
    super::adm5_tests::seed(&mut f, &url, 10_000_000);
    f.access.runs.configure(ProviderConfig::from_values("native", "fixture-key", url, "fixture-model").unwrap()).unwrap();
    let a = f.ok(&f.a, "POST", "/api/learning-workspaces", json!({"goal":{"text":"Learn source A"},"members":[f.x]}));
    let b = f.ok(&f.a, "POST", "/api/learning-workspaces", json!({"goal":{"text":"Study for exam B"},"members":[f.x]}));
    let a_id = a["learning_workspace_id"].as_str().unwrap();
    let scene = f.ok(&f.a, "POST", "/api/workspaces", json!({"published_book_ref":f.x,"attachment_id":"original","learning_workspace_id":a_id}));
    let scene = f.action(&f.a, &scene, "original", "chat/new", json!({}));
    let mut input = Fixture::stamp(&scene, "original");
    input["client_request_id"] = json!("w3-review-origin"); input["session_id"] = scene["selected_chat"].clone();
    input["message"] = json!("I prefer worked examples.");
    admit(&f, &scene, &input);
    let access = f.access.clone();
    let run = std::thread::spawn(move || access.runs.run_one(&access));
    respond(fake.recv_timeout(Duration::from_secs(15)).unwrap().unwrap(), json!({"role":"assistant","content":"We will use worked examples."}), 10);
    run.join().unwrap().unwrap();
    let (front_scope, parent, run_ref): (String, String, String) = f.control.connection.query_row(
        "SELECT learning_workspace_id,parent_task_ref,run_ref FROM model_call_charges WHERE run_ref IS NOT NULL AND user_id='A'",
        [], |row|Ok((row.get(0)?,row.get(1)?,row.get(2)?))).unwrap();
    assert_eq!(front_scope, a_id);
    assert_eq!(parent, run_ref);
    let now_ms = (crate::multi_user_host::now() as u64) * 1000 + 61_000;
    let cancel = runtime::run_context::CancellationToken::default();
    f.ok(&f.a, "POST", &format!("/api/learning-workspaces/{a_id}/archive"), json!({"archived":true}));
    assert_eq!(crate::multi_user_review::tick(&f.access, &cancel, now_ms), 0);
    assert!(fake.recv_timeout(Duration::from_millis(20)).unwrap().is_none(), "archived work must not dispatch a paid background send");
    let handle = f.access.users.lock().unwrap().get("A", "now").unwrap();
    assert_eq!(handle.lock().unwrap().store.review_state().review_jobs[0].attempts, 0);
    f.ok(&f.a, "POST", &format!("/api/learning-workspaces/{a_id}/archive"), json!({"archived":false}));
    let access = f.access.clone();
    let review = std::thread::spawn(move || crate::multi_user_review::tick(&access, &Default::default(), now_ms));
    let request = fake.recv_timeout(Duration::from_secs(15)).unwrap().unwrap();
    let other = f.ok(&f.a, "POST", "/api/workspaces", json!({"published_book_ref":f.x,"attachment_id":"other","learning_workspace_id":b["learning_workspace_id"]}));
    f.action(&f.a, &other, "other", "chat/new", json!({}));
    let turn_id = input["session_id"].as_str().and_then(|session_id| {
        let user = handle.lock().unwrap();
        user.agent_history.sessions.iter().find(|s|s.id==session_id).and_then(|s|s.turns.last().map(|t|t.turn_id.clone()))
    }).unwrap();
    respond(request, json!({"role":"assistant","content":json!({"candidate_facts":[{"source":"user_stated","scope":"book",
        "payload":{"kind":"explanation_preference","key":"examples","value":"worked examples"},
        "evidence":[{"turn_id":turn_id,"user_quote":"I prefer worked examples."}]}],"intent_observations":[]}).to_string()}), 10);
    assert_eq!(review.join().unwrap(), 1);
    assert_eq!(crate::multi_user_review::tick(&f.access, &cancel, now_ms + 1000), 0);
    let (count, primary, related, debit): (i64, Option<String>, String, i64) = f.control.connection.query_row(
        "SELECT count(*),learning_workspace_id,related_learning_workspace_ids,sum(account_debit_micro_cny) FROM model_call_charges WHERE purpose='memory_review' AND user_id='A'",
        [], |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).unwrap();
    assert_eq!(count, 1); assert!(primary.is_none()); assert_eq!(serde_json::from_str::<Value>(&related).unwrap(), json!([a_id])); assert_eq!(debit, 11);
    let user = handle.lock().unwrap();
    let job = &user.store.review_state().review_jobs[0];
    assert_eq!(job.learning_workspace_id.as_deref(), Some(a_id)); assert_eq!(job.goal_revisions, [1]);
    assert_eq!(user.store.profile_facts().len(), 1);
    assert!(matches!(&user.store.profile_facts()[0].scope, memory::ProfileScope::Workspace { learning_workspace_id, .. } if learning_workspace_id==a_id));
}
