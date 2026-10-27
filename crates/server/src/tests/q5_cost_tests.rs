use super::{adm5_tests, mu5_tests::Fixture};
use runtime::{
    model_spend::{
        ChargeAttribution, ChargeScope, ModelSpendPort, SendEvidence, SendIdentity, SendOutcome,
    },
    provider_stream::ModelUsage,
    CompletionRequest, ProviderConfig, ProviderRegistry,
};
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};

fn workspace(f: &Fixture, id: &str) {
    f.control.connection.execute("INSERT INTO learning_workspaces(owner_user_id,learning_workspace_id,created_at,updated_at) VALUES('A',?,0,0)", [id]).unwrap();
    f.control.connection.execute("INSERT INTO learning_workspace_goals(owner_user_id,learning_workspace_id,revision,goal_text,constraints_json,created_at) VALUES('A',?,1,?,'{}',0)", rusqlite::params![id,format!("学习 {id}")]).unwrap();
}
fn bind(f: &Fixture, period: &str, version: &str, amount: i64) {
    f.ok(&f.b,"POST","/api/admin/users/A/allowance-basis",json!({"operation_id":format!("bind-{version}"),"period_id":period,"revision":1,"basis":{"version":version,"basis_micro_cny":amount}}));
}
fn report(f: &Fixture, reader: bool) -> Value {
    f.ok(
        if reader { &f.a } else { &f.b },
        "GET",
        if reader {
            "/api/account/cost-summary"
        } else {
            "/api/admin/cost-summary"
        },
        json!({}),
    )
}
fn task<'a>(report: &'a Value, parent: &str, cost_owner: &str) -> &'a Value {
    report["tasks"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["parent_task_ref"] == parent && r["cost_owner"] == cost_owner)
        .unwrap()
}
fn request() -> Value {
    json!({"model":"fixture-model","messages":[{"role":"user","content":"fixture"}],"max_tokens":100})
}
fn outcome(tokens: u32) -> SendOutcome {
    SendOutcome {
        evidence: SendEvidence::Response,
        usage: Some(ModelUsage {
            input_tokens: Some(tokens),
            cached_input_tokens: Some(0),
            output_tokens: Some(0),
            total_tokens: Some(tokens),
            ..Default::default()
        }),
        succeeded: true,
    }
}
fn unknown() -> SendOutcome {
    SendOutcome {
        evidence: SendEvidence::OutcomeUnknown,
        usage: None,
        succeeded: false,
    }
}
fn admitted(
    f: &Fixture,
    call: &str,
    parent: &str,
    workspace: Option<&str>,
    related: Vec<String>,
) -> (Arc<dyn ModelSpendPort>, SendIdentity) {
    let scope = ChargeScope::ReaderTask {
        user_id: "A".into(),
        task_ref: format!("child-{call}"),
    };
    let port = f.access.spend.port_with_attribution(
        scope.clone(),
        f.x.clone(),
        ChargeAttribution {
            parent_task_ref: parent.into(),
            learning_workspace_id: workspace.map(str::to_owned),
            related_learning_workspace_ids: related,
        },
        Default::default(),
    );
    let id = SendIdentity {
        call_id: call.into(),
        logical_call_id: call.into(),
        attempt: 1,
        scope: Some(scope),
        purpose: "memory_review".into(),
        model: "fixture-model".into(),
        provider: "fixture".into(),
    };
    port.before_send(&id, &request()).unwrap();
    (port, id)
}

// Detect an actual charge being counted again through related workspaces or a
// public reference, or a child escaping its parent. Fix only the report grouping.
#[test]
fn q5_actual_transport_reconciles_parent_workspace_account_and_platform_once() {
    let fake = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let url = format!("http://{}", fake.server_addr().to_ip().unwrap());
    let mut f = Fixture::new();
    let period = adm5_tests::seed(&mut f, &url, 100_000);
    bind(&f, &period, "q5-basis", 1000);
    workspace(&f, "learning-A");
    workspace(&f, "learning-B");
    let worker = std::thread::spawn(move || {
        for _ in 0..5 {
            let request = fake.recv_timeout(Duration::from_secs(10)).unwrap().unwrap();
            request.respond(tiny_http::Response::from_string(json!({
                "choices":[{"message":{"role":"assistant","content":"{\"ok\":true}"},"finish_reason":"stop"}],
                "usage":{"prompt_tokens":10,"prompt_cache_hit_tokens":0,"completion_tokens":1,"total_tokens":11}
            }).to_string()).with_header(tiny_http::Header::from_bytes("Content-Type","application/json").unwrap())).unwrap();
        }
    });
    let adapter = ProviderRegistry::adapter_from_config(
        ProviderConfig::from_values("native", "fixture-key", url, "fixture-model").unwrap(),
    );
    for purpose in ["answer", "repair", "private_understanding", "memory_review"] {
        let account_level = purpose == "memory_review";
        let scope = if purpose == "answer" {
            ChargeScope::ReaderRun {
                user_id: "A".into(),
                run_ref: "foreground-child".into(),
            }
        } else {
            ChargeScope::ReaderTask {
                user_id: "A".into(),
                task_ref: format!("child-{purpose}"),
            }
        };
        adapter.set_spend_context(
            scope.clone(),
            Some(
                f.access.spend.port_with_attribution(
                    scope,
                    f.x.clone(),
                    ChargeAttribution {
                        parent_task_ref: if account_level {
                            "memory-review:durable-job"
                        } else {
                            "original-turn"
                        }
                        .into(),
                        learning_workspace_id: if account_level {
                            None
                        } else {
                            Some("learning-A".into())
                        },
                        related_learning_workspace_ids: if account_level {
                            vec!["learning-A".into(), "learning-B".into()]
                        } else {
                            vec![]
                        },
                    },
                    Default::default(),
                ),
            ),
        );
        adapter.set_model_purpose(purpose);
        assert_eq!(
            adapter
                .complete_structured(CompletionRequest {
                    followup_user: None,
                    images: Vec::new(),
                    system: "fixture".into(),
                    user: "fixture".into(),
                    output_token_limit: Some(100),
                    reasoning_effort: None
                })
                .unwrap()["ok"],
            true
        );
    }
    adapter.set_spend_context(
        ChargeScope::OperatorTask {
            task_ref: "original-turn".into(),
        },
        Some(
            f.access
                .spend
                .operator_port("original-turn".into(), Default::default()),
        ),
    );
    adapter.set_model_purpose("prebuild");
    adapter
        .complete_structured(CompletionRequest {
            followup_user: None,
            images: Vec::new(),
            system: "fixture".into(),
            user: "fixture".into(),
            output_token_limit: Some(100),
            reasoning_effort: None,
        })
        .unwrap();
    worker.join().unwrap();
    let reader = report(&f, true);
    assert_eq!(reader["account"]["account_debit_micro_cny"], 44);
    assert_eq!(
        reader["account"]["period_groups"][0]["percentages"]["debited"]["value"],
        "4.4"
    );
    assert_eq!(reader["workspaces"].as_array().unwrap().len(), 1);
    assert_eq!(reader["workspaces"][0]["account_debit_micro_cny"], 33);
    assert_eq!(reader["workspaces"][0]["workspace_goal"], "学习 learning-A");
    assert_eq!(reader["account_level"][0]["account_debit_micro_cny"], 11);
    assert_eq!(
        task(&reader, "original-turn", "account")["request_count"],
        3
    );
    assert_eq!(
        task(&reader, "memory-review:durable-job", "account")["related_learning_workspace_ids"],
        json!(["learning-A", "learning-B"])
    );
    assert!(reader["platform"].is_null());
    assert_eq!(
        f.ok(
            &f.a,
            "GET",
            "/api/account/usage?parent_task_ref=original-turn",
            json!({})
        )["total"],
        3
    );
    assert_eq!(
        f.ok(
            &f.a,
            "GET",
            "/api/account/usage?learning_workspace_id=learning-A",
            json!({})
        )["total"],
        3
    );
    assert_eq!(
        f.ok(
            &f.a,
            "GET",
            "/api/account/usage?learning_workspace_id=learning-B",
            json!({})
        )["total"],
        0
    );
    assert_eq!(
        f.ok(
            &f.a,
            "GET",
            "/api/account/usage?account_level=true",
            json!({})
        )["total"],
        1
    );
    assert_eq!(
        f.ok(
            &f.b,
            "GET",
            "/api/admin/charges?parent_task_ref=original-turn&cost_owner=platform",
            json!({})
        )["total"],
        1
    );
    let admin = report(&f, false);
    assert_eq!(admin["account"], reader["account"]);
    assert_eq!(admin["platform"]["provider_cost_micro_cny"], 11);
    assert_eq!(admin["platform"]["account_debit_micro_cny"], 0);
    assert_eq!(admin["platform"]["platform_covered_micro_cny"], 11);
    assert!(admin["platform"]["period_groups"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(
        f.ok(&f.b, "GET", "/api/account/cost-summary", json!({}))["account"]["request_count"],
        0
    );
    assert_eq!(
        f.call(&f.a, "GET", "/api/admin/cost-summary", json!({})).0,
        403
    );
    for _ in 0..2 {
        let again = report(&f, false);
        for field in [
            "account",
            "platform",
            "accounts",
            "workspaces",
            "account_level",
            "tasks",
        ] {
            assert_eq!(
                again[field], admin[field],
                "reading shared references or reports cannot append a cost"
            );
        }
    }
    assert_eq!(
        f.ok(&f.b, "GET", "/api/admin/usage", json!({}))["summary"]["confirmed_cost_micro_cny"],
        55,
        "existing operational usage remains compatible"
    );
}

// A late receipt must update its original task/period, never the current basis;
// page size cannot change totals. If it fails, fix the frozen period projection.
#[test]
fn q5_late_settlement_preserves_original_period_and_task_across_pagination() {
    let mut f = Fixture::new();
    let old = adm5_tests::seed(&mut f, "fixture", 100_000);
    bind(&f, &old, "q5-old", 1000);
    workspace(&f, "learning-A");
    let (port, first) = admitted(
        &f,
        "old-confirmed",
        "continued-task",
        Some("learning-A"),
        vec![],
    );
    port.after_send(&first, &outcome(23)).unwrap();
    let (late_port, late) = admitted(
        &f,
        "old-pending",
        "continued-task",
        Some("learning-A"),
        vec![],
    );
    late_port.after_send(&late, &unknown()).unwrap();
    let before = report(&f, true);
    let pending = task(&before, "continued-task", "account");
    assert_eq!(pending["confirmed_cost_micro_cny"], 23);
    assert_eq!(pending["unknown_cost_requests"], 1);
    assert!(pending["provider_cost_micro_cny"].is_null());
    assert_eq!(pending["provider_cost_complete"], false);
    assert!(pending["pending_micro_cny"].as_i64().unwrap() > 0);
    let now = crate::multi_user_host::now();
    f.ok(&f.b,"POST","/api/admin/users/A/allowance-validity",json!({"operation_id":"expire-old","period_id":old,"revision":2,"starts_at":now-100,"expires_at":now-1}));
    let new=f.ok(&f.b,"POST","/api/admin/users/A/allowance-periods",json!({"operation_id":"new-period","starts_at":now,"expires_at":now+3600,"basis":{"version":"q5-new","basis_micro_cny":2000}}))["allowance_period"]["period_id"].as_str().unwrap().to_owned();
    f.ok(&f.b,"POST","/api/admin/users/A/allowance-adjustments",json!({"operation_id":"new-grant","period_id":new,"revision":0,"delta_micro_cny":100_000,"reason":"fixture"}));
    let (new_port, new_call) = admitted(
        &f,
        "new-confirmed",
        "continued-task",
        Some("learning-A"),
        vec![],
    );
    new_port.after_send(&new_call, &outcome(23)).unwrap();
    f.control
        .connection
        .execute(
            "UPDATE learning_workspaces SET archived=1 WHERE learning_workspace_id='learning-A'",
            [],
        )
        .unwrap();
    late_port.after_send(&late, &outcome(17)).unwrap();
    late_port.after_send(&late, &outcome(17)).unwrap();
    let after = f.ok(&f.a, "GET", "/api/account/cost-summary?limit=1", json!({}));
    let task = task(&after, "continued-task", "account");
    assert_eq!(task["request_count"], 3);
    assert_eq!(task["provider_cost_micro_cny"], 63);
    assert_eq!(task["unknown_cost_requests"], 0);
    assert_eq!(task["pending_micro_cny"], 0);
    let groups = task["period_groups"].as_array().unwrap();
    assert_eq!(groups.len(), 2);
    assert_eq!(
        groups.iter().find(|v| v["period_id"] == old).unwrap()["percentages"]["debited"]["value"],
        "4"
    );
    assert_eq!(
        groups.iter().find(|v| v["period_id"] == old).unwrap()["expires_at"],
        now - 1
    );
    assert_eq!(
        groups.iter().find(|v| v["period_id"] == new).unwrap()["starts_at"],
        now
    );
    assert_eq!(
        groups.iter().find(|v| v["period_id"] == new).unwrap()["percentages"]["debited"]["value"],
        "1.15"
    );
    assert!(task.get("total_percentage").is_none());
    assert!(task.get("percentages").is_none());
    assert_eq!(after["workspaces"][0]["archived"], true);
    let empty_page = f.ok(
        &f.a,
        "GET",
        "/api/account/cost-summary?limit=1&offset=1",
        json!({}),
    );
    assert_eq!(empty_page["account"], after["account"]);
    assert!(empty_page["tasks"]["items"].as_array().unwrap().is_empty());
    let detail = f.ok(
        &f.a,
        "GET",
        "/api/account/usage?parent_task_ref=continued-task&limit=1",
        json!({}),
    );
    assert_eq!(detail["total"], 3);
    assert_eq!(detail["period_groups"].as_array().unwrap().len(), 2);
}

// Waiving a reader does not establish supplier cost. A capped build must show
// its real supplier cost and platform difference without debiting another user.
#[test]
fn q5_waived_unknown_and_capped_external_cost_remain_distinct() {
    let mut f = Fixture::new();
    let period = adm5_tests::seed(&mut f, "fixture", 100_000);
    bind(&f, &period, "q5-basis", 1000);
    workspace(&f, "learning-A");
    let (port, id) = admitted(
        &f,
        "waived-unknown",
        "profile-job",
        None,
        vec!["learning-A".into()],
    );
    port.after_send(&id, &unknown()).unwrap();
    let revision =
        f.ok(&f.b, "GET", "/api/admin/charges/waived-unknown", json!({}))["revision"].clone();
    f.ok(&f.b,"POST","/api/admin/charges/waived-unknown/reconcile",json!({"operation_id":"waive","revision":revision,"waive_account":true,"reason":"fixture","evidence":"unknown supplier cost"}));
    let waived = report(&f, true);
    assert_eq!(waived["account_level"][0]["account_debit_micro_cny"], 0);
    assert_eq!(waived["account_level"][0]["unknown_cost_requests"], 1);
    assert_eq!(waived["account_level"][0]["pending_micro_cny"], 0);
    assert!(waived["account_level"][0]["provider_cost_micro_cny"].is_null());
    f.ok(&f.b,"POST","/api/admin/build-tasks/limited-build/budget",json!({"cost_owner":"account","user_id":"A","learning_workspace_id":"learning-A","max_micro_cny":40,"confirmation":"fixture cap"}));
    f.ok(&f.b,"POST","/api/admin/build-tasks/limited-build/admit",json!({"execution_id":"build-execution","call_id":"build-call","logical_call_id":"build-call","attempt":1,"granularity":"execution_total","reserved_micro_cny":40,"provider":"fixture","model":"fixture-model","purpose":"prebuild","budget_enforced":true,"receipt_capability":"complete"}));
    let receipt = json!({"call_id":"build-call","status":"confirmed","provider_cost_micro_cny":55,"evidence":{"invoice":"fixture"},"succeeded":true});
    f.ok(
        &f.b,
        "POST",
        "/api/admin/build-tasks/limited-build/receipts",
        receipt.clone(),
    );
    f.ok(
        &f.b,
        "POST",
        "/api/admin/build-tasks/limited-build/receipts",
        receipt,
    );
    let admin = report(&f, false);
    let build = task(&admin, "limited-build", "account");
    assert_eq!(build["provider_cost_micro_cny"], 55);
    assert_eq!(build["account_debit_micro_cny"], 40);
    assert_eq!(build["platform_covered_micro_cny"], 15);
    assert_eq!(build["request_count"], 1);
    assert_eq!(admin["account"]["confirmed_cost_micro_cny"], 55);
    assert_eq!(admin["account"]["unknown_cost_requests"], 1);
    assert!(admin["account"]["provider_cost_micro_cny"].is_null());
    assert_eq!(
        admin["platform"]["request_count"], 0,
        "account build overhead is a difference, not a duplicate platform call"
    );
}
