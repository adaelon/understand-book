use super::mu5_tests::Fixture;
use crate::{
    admin_usage,
    control_store::{ControlStore, ServiceWriter},
};
use serde_json::{json, Value};

fn report(f: &Fixture, dates: &str) -> Value {
    f.ok(&f.b, "GET", &format!("/api/admin/usage?{dates}"), json!({}))
}

#[test]
fn adm9_schema7_upgrade_preserves_data_and_rolls_back_failed_install() {
    let root = tempfile::tempdir().unwrap();
    let db = rusqlite::Connection::open(root.path().join("control.sqlite")).unwrap();
    db.execute_batch(include_str!("control_schema_v4.sql"))
        .unwrap();
    db.execute_batch("ALTER TABLE users ADD COLUMN is_admin INTEGER NOT NULL DEFAULT 0;")
        .unwrap();
    db.execute_batch(include_str!("../admin_schema.sql"))
        .unwrap();
    db.execute_batch(include_str!("../receipt_corrections.sql"))
        .unwrap();
    db.execute_batch(include_str!("../charge_settlement.sql"))
        .unwrap();
    db.execute_batch("INSERT INTO users(user_id) VALUES('old'); PRAGMA application_id=1430408533; PRAGMA user_version=7;").unwrap();
    // Existing index of the same name forces an error after CREATE TABLE.
    db.execute_batch("CREATE INDEX admin_usage_events_time ON users(disabled)")
        .unwrap();
    let writer = ServiceWriter::acquire(root.path()).unwrap();
    assert!(ControlStore::open(writer.clone()).is_err());
    assert_eq!(
        db.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        7
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM sqlite_master WHERE name='admin_usage_events'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    db.execute_batch("DROP INDEX admin_usage_events_time")
        .unwrap();
    let store = ControlStore::open(writer.clone()).unwrap();
    assert_eq!(
        store
            .connection
            .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        crate::control_store::CONTROL_SCHEMA_VERSION
    );
    assert_eq!(
        store
            .connection
            .query_row("SELECT user_id FROM users", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "old"
    );
    assert_eq!(
        admin_usage::account_activity(&store.connection, "old").unwrap()["first_used_at"],
        Value::Null
    );
    admin_usage::record(&store.connection, "old", "read", "r", 1_790_438_399).unwrap();
    drop(store);
    assert_eq!(
        admin_usage::account_activity(&ControlStore::open(writer).unwrap().connection, "old")
            .unwrap()["active_days"],
        1
    );
}

#[test]
fn adm9_activity_hong_kong_days_and_successful_open_only() {
    let mut f = Fixture::new();
    f.control.set_reader_admin("B", true).unwrap();
    let mut q = std::collections::HashMap::from([
        ("from".into(), "2026-10-08".into()),
        ("to".into(), "2026-10-08".into()),
    ]);
    let day = admin_usage::range(&mut q, 0).unwrap().start;
    for (owner, kind, id, at) in [
        ("A", "read", "before", day - 1),
        ("A", "read", "first", day),
        ("A", "read", "again", day + 30),
        ("A", "question", "q", day + 60),
        ("B", "read", "b", day + 60),
    ] {
        admin_usage::record(&f.control.connection, owner, kind, id, at).unwrap();
    }
    admin_usage::record(&f.control.connection, "A", "question", "q", day + 86400).unwrap(); // replay cannot create a visit
    let selected = report(&f, "from=2026-10-08&to=2026-10-08");
    assert_eq!(selected["summary"]["active_users"], 2);
    assert_eq!(selected["summary"]["returning_users"], 1);
    assert_eq!(
        report(&f, "from=2026-10-09&to=2026-10-09")["summary"]["active_users"],
        0
    );
    let user = f.ok(&f.b, "GET", "/api/admin/users/A", json!({}));
    assert_eq!(user["activity"]["active_days"], 2);
    assert_eq!(user["activity"]["return_days"], 1);
    assert_eq!(user["activity"]["last_question_at"], day + 60);
    assert_eq!(
        f.ok(&f.b, "GET", "/api/admin/users?search=A", json!({}))["users"][0]["activity"],
        user["activity"]
    );
    let count = || {
        f.control
            .connection
            .query_row("SELECT count(*) FROM admin_usage_events", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap()
    };
    let before = count();
    f.ok(&f.a, "GET", "/api/auth/me", json!({}));
    f.ok(&f.a, "GET", "/api/library", json!({}));
    f.call(&f.a,"POST","/api/workspaces",json!({"published_book_ref":{"book_id":"missing","publication_id":"missing"},"attachment_id":"page"}));
    assert_eq!(count(), before);
    let w = f.create(&f.a, &f.x, "page");
    assert_eq!(count(), before + 1);
    f.get(&f.a, &w);
    assert_eq!(count(), before + 1);
    f.action(&f.a, &w, "page", "attach", json!({}));
    assert_eq!(count(), before + 2);
    assert_eq!(f.call(&f.a, "GET", "/api/admin/usage", json!({})).0, 403);
    for query in [
        "from=bad",
        "from=2026-10-09&to=2026-10-08",
        "limit=101",
        "unknown=true",
        "user_id=missing",
    ] {
        assert!(
            f.call(&f.b, "GET", &format!("/api/admin/usage?{query}"), json!({}))
                .0
                >= 400
        );
    }
}

#[test]
fn adm9_costs_receipts_unknowns_task_pages_and_detail_agree() {
    use runtime::{model_spend::*, provider_stream::ModelUsage};
    let mut f = Fixture::new();
    let period = super::adm5_tests::seed(&mut f, "fixture", 10_000_000);
    let request = json!({"model":"fixture-model","messages":[],"max_tokens":10});
    for (call, run, tokens) in [
        ("paid", "r1", Some(51)),
        ("pending", "r1", None),
        ("waived", "r2", None),
        ("flight", "r2", None),
    ] {
        let scope = ChargeScope::ReaderRun {
            user_id: "A".into(),
            run_ref: run.into(),
        };
        let port = f
            .access
            .spend
            .port(scope.clone(), f.x.clone(), Default::default());
        let id = SendIdentity {
            call_id: call.into(),
            logical_call_id: call.into(),
            attempt: 1,
            scope: Some(scope),
            purpose: "outer".into(),
            model: "fixture-model".into(),
            provider: "fixture".into(),
        };
        port.before_send(&id, &request).unwrap();
        if call != "flight" {
            port.after_send(
                &id,
                &SendOutcome {
                    evidence: SendEvidence::Response,
                    usage: tokens.map(|v| ModelUsage {
                        input_tokens: Some(v),
                        cached_input_tokens: Some(0),
                        output_tokens: Some(0),
                        ..Default::default()
                    }),
                    succeeded: tokens.is_some(),
                },
            )
            .unwrap();
        }
    }
    let c = f.ok(&f.b, "GET", "/api/admin/charges/waived", json!({}));
    f.ok(&f.b,"POST","/api/admin/charges/waived/reconcile",json!({"operation_id":"waive","revision":c["revision"],"waive_account":true,"reason":"fixture","evidence":"fixture"}));
    let day = admin_usage::range(
        &mut std::collections::HashMap::from([
            ("from".into(), "2026-10-08".into()),
            ("to".into(), "2026-10-08".into()),
        ]),
        0,
    )
    .unwrap()
    .start;
    f.control
        .connection
        .execute("UPDATE model_call_charges SET created_at=?", [day])
        .unwrap();
    let receipt=f.ok(&f.b,"POST","/api/admin/users/A/receipts",json!({"operation_id":"pay","period_id":period,"revision":1,"amount_fen":1234,"delta_micro_cny":1,"paid_at":day,"channel":"wechat","note":"fixture"}));
    let receipt_id = receipt["receipt_id"].as_str().unwrap();
    // A later correction belongs to the original receipt's reporting day.
    f.ok(&f.b,"POST","/api/admin/users/A/receipt-corrections",json!({"operation_id":"correct","receipt_id":receipt_id,"period_id":period,"revision":2,"delta_micro_cny":0,"delta_fen":-34,"reason":"fixture"}));
    let dates = "from=2026-10-08&to=2026-10-08";
    let report = report(&f, &format!("{dates}&limit=1"));
    let s = &report["summary"];
    assert_eq!(s["request_count"], 4);
    assert_eq!(s["confirmed_cost_micro_cny"], 51);
    assert_eq!(s["account_debit_micro_cny"], 51);
    assert_eq!(s["unknown_cost_requests"], 3);
    assert_eq!(s["pending_requests"], 2); // unknown waived cost still visible
    assert_eq!(s["active_requests"], 1);
    assert_eq!(s["receipt_original_fen"], 1234);
    assert_eq!(s["receipt_correction_fen"], -34);
    assert_eq!(s["receipt_net_fen"], 1200);
    assert_eq!(report["tasks"]["total"], 2);
    assert_eq!(report["tasks"]["items"][0]["reference"], "r1");
    let next = super::adm9_tests::report(&f, &format!("{dates}&limit=1&offset=1"));
    assert_eq!(next["tasks"]["items"][0]["reference"], "r2");
    let detail = f.ok(
        &f.b,
        "GET",
        &format!("/api/admin/charges?user_id=A&run_ref=r1&{dates}&limit=1"),
        json!({}),
    );
    assert_eq!(detail["total"], 2);
    let all = f.ok(
        &f.b,
        "GET",
        &format!("/api/admin/charges?{dates}"),
        json!({}),
    );
    let rows = all["items"].as_array().unwrap();
    for field in ["provider_cost_micro_cny", "account_debit_micro_cny"] {
        let sum: i64 = rows.iter().filter_map(|r| r[field].as_i64()).sum();
        assert_eq!(
            sum,
            s[if field == "provider_cost_micro_cny" {
                "confirmed_cost_micro_cny"
            } else {
                field
            }]
        );
    }
    assert_eq!(
        f.ok(
            &f.b,
            "GET",
            &format!("/api/admin/users/A/usage?{dates}"),
            json!({})
        )["summary"],
        *s
    );
    assert_eq!(
        f.ok(
            &f.b,
            "GET",
            &format!("/api/admin/users/B/usage?{dates}"),
            json!({})
        )["summary"]["request_count"],
        0
    );
    assert_eq!(
        super::adm9_tests::report(&f, "from=2026-10-09&to=2026-10-09")["summary"]["request_count"],
        0
    );
    let pending = f.ok(&f.b, "GET", "/api/admin/charges/pending", json!({}));
    f.ok(&f.b,"POST","/api/admin/charges/pending/reconcile",json!({"operation_id":"late-cost","revision":pending["revision"],"provider_cost_micro_cny":23,"waive_account":false,"reason":"later evidence","evidence":"fixture"}));
    let revised = super::adm9_tests::report(&f, dates);
    assert_eq!(revised["summary"]["confirmed_cost_micro_cny"], 74);
    assert_eq!(revised["summary"]["account_debit_micro_cny"], 74);
    assert_eq!(revised["summary"]["pending_micro_cny"], 0);
    assert_eq!(revised["summary"]["pending_requests"], 1); // waived unknown survives
    let scope = ChargeScope::ReaderTask {
        user_id: "A".into(),
        task_ref: "r1".into(),
    };
    let port = f
        .access
        .spend
        .port(scope.clone(), f.x.clone(), Default::default());
    let id = SendIdentity {
        call_id: "background".into(),
        logical_call_id: "background".into(),
        attempt: 1,
        scope: Some(scope),
        purpose: "tutor".into(),
        model: "fixture-model".into(),
        provider: "fixture".into(),
    };
    port.before_send(&id, &request).unwrap();
    port.after_send(
        &id,
        &SendOutcome {
            evidence: SendEvidence::NotSent,
            usage: None,
            succeeded: false,
        },
    )
    .unwrap();
    f.control
        .connection
        .execute(
            "UPDATE model_call_charges SET created_at=? WHERE call_id='background'",
            [day],
        )
        .unwrap();
    assert_eq!(super::adm9_tests::report(&f, dates)["tasks"]["total"], 3);
    assert_eq!(
        f.ok(
            &f.b,
            "GET",
            &format!("/api/admin/charges?task_ref=r1&{dates}"),
            json!({})
        )["items"][0]["call_id"],
        "background"
    );
    assert_eq!(
        f.ok(
            &f.b,
            "GET",
            &format!("/api/admin/charges?run_ref=r1&{dates}"),
            json!({})
        )["total"],
        2
    );
}

#[test]
fn adm9_saved_delivery_only_and_retry_does_not_duplicate_activity() {
    use runtime::ProviderConfig;
    for fail_save in [false, true] {
        let fake = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let url = format!("http://{}", fake.server_addr().to_ip().unwrap());
        let mut f = Fixture::new();
        super::adm5_tests::seed(&mut f, &url, 1_000_000);
        f.access
            .runs
            .configure(
                ProviderConfig::from_values("native", "fixture", url, "fixture-model").unwrap(),
            )
            .unwrap();
        let w = f.create(&f.a, &f.x, "page");
        let w = f.action(&f.a, &w, "page", "chat/new", json!({}));
        let mut input = Fixture::stamp(&w, "page");
        input["session_id"] = w["selected_chat"].clone();
        input["message"] = json!("你好");
        input["client_request_id"] = json!("adm9-question");
        let path = format!(
            "/api/workspaces/{}/agent/runs",
            w["workspace_id"].as_str().unwrap()
        );
        let (status, accepted) = f.call(&f.a, "POST", &path, input.clone());
        assert_eq!(status, 202, "{accepted}");
        f.call(&f.a, "POST", &path, input.clone());
        let count = |kind: &str| {
            f.control
                .connection
                .query_row(
                    "SELECT count(*) FROM admin_usage_events WHERE kind=?",
                    [kind],
                    |r| r.get::<_, i64>(0),
                )
                .unwrap()
        };
        assert_eq!(count("question"), 1);
        let access = f.access.clone();
        let worker = std::thread::spawn(move || access.runs.run_one(&access));
        let request = fake
            .recv_timeout(std::time::Duration::from_secs(10))
            .unwrap()
            .unwrap();
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
        request.respond(tiny_http::Response::from_string(json!({"choices":[{"message":{"role":"assistant","content":"你好，可以开始阅读。"},"finish_reason":"stop"}],"usage":{"prompt_tokens":1,"prompt_cache_hit_tokens":0,"completion_tokens":1}}).to_string())).unwrap();
        worker.join().unwrap().unwrap();
        assert_eq!(count("completed"), if fail_save { 0 } else { 1 });
        let retry = format!(
            "/api/agent/runs/{}/retry-save",
            accepted["turn_id"].as_str().unwrap()
        );
        f.ok(&f.a, "POST", &retry, json!({}));
        f.ok(&f.a, "POST", &retry, json!({}));
        assert_eq!(count("completed"), 1);
        assert_eq!(count("question"), 1);
        assert!(fake.try_recv().unwrap().is_none());
        let handle = f.access.users.lock().unwrap().get("A", "now").unwrap();
        let mut session = handle
            .lock()
            .unwrap()
            .agent_history
            .sessions
            .iter()
            .find(|s| Some(s.id.as_str()) == input["session_id"].as_str())
            .unwrap()
            .clone();
        for field in ["incomplete", "warning", "failed", "cancelled", "delivery"] {
            let mut turn = session.turns[0].clone();
            match field {
                "incomplete"=>turn.outcome.as_mut().unwrap().incomplete=true,
                "warning"=>turn.outcome.as_mut().unwrap().warning=Some("stopped".into()),
                "failed"=>turn.status=crate::AgentAssistantStatus::Failed,
                "cancelled"=>turn.status=crate::AgentAssistantStatus::Cancelled,
                _=>turn.delivery_diagnostics=Some(serde_json::from_value(json!({"initial":{"issues":[]},"repair":{"issues":[{"error_code":"fixture","start":null,"end":null,"trigger_value":null,"match_form":"fixture","source_channels":[]}]}})).unwrap()),
            }
            assert!(!admin_usage::delivered(&session, &turn), "{field}");
        }
        session.turns.clear();
        f.ok(
            &f.a,
            "DELETE",
            &format!("/api/me/chats/{}", input["session_id"].as_str().unwrap()),
            json!({}),
        );
        assert_eq!(count("completed"), 1);
    }
}

#[test]
#[ignore = "isolated HTTPS candidate; set ADM9_CANDIDATE_INFO and run explicitly"]
fn adm9_candidate_host() {
    use runtime::{model_spend::*, provider_stream::ModelUsage};
    let info = std::path::PathBuf::from(std::env::var("ADM9_CANDIDATE_INFO").unwrap());
    let mut f = Fixture::new();
    let period = super::adm5_tests::seed(&mut f, "fixture", 10_000_000);
    let now = crate::multi_user_host::now();
    admin_usage::record(&f.control.connection, "A", "read", "yesterday", now - 86400).unwrap();
    f.create(&f.a, &f.x, "candidate");
    admin_usage::record(&f.control.connection, "A", "question", "question", now).unwrap();
    for index in 0..43 {
        let run = if index <= 22 {
            "r00".to_owned()
        } else {
            format!("r{:02}", index - 22)
        };
        let scope = ChargeScope::ReaderRun {
            user_id: "A".into(),
            run_ref: run,
        };
        let port = f
            .access
            .spend
            .port(scope.clone(), f.x.clone(), Default::default());
        let id = SendIdentity {
            call_id: format!("candidate-{index:02}"),
            logical_call_id: format!("candidate-{index:02}"),
            attempt: 1,
            scope: Some(scope),
            purpose: "outer".into(),
            model: "fixture-model".into(),
            provider: "fixture".into(),
        };
        port.before_send(
            &id,
            &json!({"model":"fixture-model","messages":[],"max_tokens":100}),
        )
        .unwrap();
        if index != 22 {
            port.after_send(
                &id,
                &SendOutcome {
                    evidence: SendEvidence::Response,
                    usage: if index >= 20 && index <= 21 {
                        None
                    } else {
                        Some(ModelUsage {
                            input_tokens: Some(100),
                            cached_input_tokens: Some(0),
                            output_tokens: Some(0),
                            ..Default::default()
                        })
                    },
                    succeeded: index < 20 || index > 22,
                },
            )
            .unwrap();
        }
    }
    let charge = f.ok(&f.b, "GET", "/api/admin/charges/candidate-21", json!({}));
    f.ok(&f.b,"POST","/api/admin/charges/candidate-21/reconcile",json!({"operation_id":"waive","revision":charge["revision"],"waive_account":true,"reason":"candidate","evidence":"fixture"}));
    f.ok(&f.b,"POST","/api/admin/users/A/receipts",json!({"operation_id":"receipt","period_id":period,"revision":1,"amount_fen":1234,"delta_micro_cny":1,"paid_at":now,"channel":"wechat","note":"candidate"}));
    let server = crate::multi_user_host::start_with_access(
        "127.0.0.1:18788".parse().unwrap(),
        std::sync::Arc::new(crate::multi_user_host::Site::new("https://localhost:18443").unwrap()),
        f.access.clone(),
    )
    .unwrap();
    std::fs::write(
        &info,
        json!({"root":f.root.path(),"url":server.url,"book":f.x}).to_string(),
    )
    .unwrap();
    while !info.with_extension("stop").exists() {
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    server.shutdown();
}
