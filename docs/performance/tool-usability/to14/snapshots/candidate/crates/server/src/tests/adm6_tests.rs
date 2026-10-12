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
        respond(fake.recv_timeout(Duration::from_secs(15)).unwrap().unwrap(),json!({"role":"assistant","content":"We can start with your question."}),10);
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
    respond(fake.recv_timeout(Duration::from_secs(15)).unwrap().unwrap(),json!({"role":"assistant","content":"{\"candidate_facts\":\"invalid\"}"}),10);
    respond(fake.recv_timeout(Duration::from_secs(15)).unwrap().unwrap(),json!({"role":"assistant","content":json!({"candidate_facts":[],"intent_observations":[]}).to_string()}),10);
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
