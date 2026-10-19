use super::{adm5_tests, mu5_tests::Fixture};
use runtime::{
    model_spend::{ChargeAttribution, ChargeScope, SendEvidence, SendIdentity, SendOutcome, SpendStop},
    provider_stream::ModelUsage, CompletionRequest, ProviderConfig, ProviderRegistry,
};
use serde_json::{json, Value};
use std::time::Duration;

fn identity(call: &str, scope: ChargeScope) -> SendIdentity {
    SendIdentity {
        call_id: call.into(), logical_call_id: call.into(), attempt: 1,
        scope: Some(scope), purpose: "prebuild".into(),
        model: "fixture-model".into(), provider: "fixture".into(),
    }
}
fn usage(tokens: u32, succeeded: bool) -> SendOutcome {
    SendOutcome {
        evidence: SendEvidence::Response,
        usage: Some(ModelUsage {
            input_tokens: Some(tokens), cached_input_tokens: Some(0), output_tokens: Some(0),
            total_tokens: Some(tokens), ..Default::default()
        }),
        succeeded,
    }
}
fn request() -> Value {
    json!({"model":"fixture-model","messages":[{"role":"user","content":"fixture"}],"max_tokens":100})
}
fn charge(f: &Fixture, call: &str) -> Value {
    f.ok(&f.b, "GET", &format!("/api/admin/charges/{call}"), json!({}))
}
fn balance(f: &Fixture) -> Value {
    f.ok(&f.a, "GET", "/api/account/allowance", json!({}))["current_allowance"]["balance"].clone()
}
fn workspace(f: &Fixture, owner: &str, id: &str) {
    f.control.connection.execute(
        "INSERT INTO learning_workspaces(owner_user_id,learning_workspace_id,created_at,updated_at) VALUES(?,?,0,0)",
        rusqlite::params![owner, id],
    ).unwrap();
}

#[test]
fn q2_platform_costs_need_no_reader_period_and_survive_failure_late_and_duplicate_receipts() {
    let mut f = Fixture::new();
    adm5_tests::seed(&mut f, "fixture", 1000);
    let before = balance(&f);
    let scope = ChargeScope::OperatorTask { task_ref: "public-build".into() };
    let port = f.access.spend.operator_port("public-build".into(), Default::default());
    let failed = identity("platform-failed", scope.clone());
    port.before_send(&failed, &request()).unwrap();
    port.after_send(&failed, &usage(42, false)).unwrap();
    port.after_send(&failed, &usage(42, false)).unwrap();
    let value = charge(&f, &failed.call_id);
    assert_eq!(value["cost_owner"], "platform");
    assert!(value["user_id"].is_null());
    assert!(value["period_id"].is_null());
    assert_eq!(value["parent_task_ref"], "public-build");
    assert_eq!(value["provider_cost_micro_cny"], 42);
    assert_eq!(value["account_debit_micro_cny"], 0);
    assert_eq!(value["send_outcome"]["succeeded"], false);
    assert_eq!(value["reports"]["total"], 1);
    let (status, error) = f.call(&f.b, "POST", "/api/admin/charges/platform-failed/reconcile", json!({
        "operation_id":"platform-account-waiver","revision":value["revision"],"waive_account":true,"reason":"fixture","evidence":"fixture"
    }));
    assert_eq!(status, 409);
    assert_eq!(error["error_code"], "CHARGE_ACCOUNT_REQUIRED");
    assert_eq!(charge(&f, &failed.call_id), value, "an account-only correction cannot mutate a platform fact");

    let late = identity("platform-late", scope.clone());
    port.before_send(&late, &request()).unwrap();
    port.after_send(&late, &SendOutcome { evidence: SendEvidence::OutcomeUnknown, usage: None, succeeded: false }).unwrap();
    assert!(charge(&f, &late.call_id)["provider_cost_micro_cny"].is_null());
    port.after_send(&late, &usage(27, false)).unwrap();
    assert_eq!(charge(&f, &late.call_id)["provider_cost_micro_cny"], 27);

    let unsent = identity("platform-unsent", scope);
    port.before_send(&unsent, &request()).unwrap();
    port.after_send(&unsent, &SendOutcome { evidence: SendEvidence::NotSent, usage: None, succeeded: false }).unwrap();
    assert_eq!(charge(&f, &unsent.call_id)["provider_cost_micro_cny"], 0);
    assert_eq!(balance(&f), before, "platform costs never reserve or debit reader allowance");
    assert_eq!(f.ok(&f.a, "GET", "/api/account/usage", json!({}))["total"], 0);
    let report = f.ok(&f.b, "GET", "/api/admin/usage", json!({}));
    assert_eq!(report["summary"]["confirmed_cost_micro_cny"], 69);
    assert_eq!(report["summary"]["account_debit_micro_cny"], 0);
    assert_eq!(report["tasks"]["items"][0]["cost_owner"], "platform");
    assert!(report["tasks"]["items"][0]["user_id"].is_null());
    assert_eq!(report["tasks"]["items"][0]["reference_kind"], "operator_task");

    let reader_scope = ChargeScope::ReaderTask { user_id: "A".into(), task_ref: "public-build".into() };
    let reader_port = f.access.spend.port(reader_scope.clone(), f.x.clone(), Default::default());
    let reader_call = identity("same-reference-account", reader_scope);
    reader_port.before_send(&reader_call, &request()).unwrap();
    reader_port.after_send(&reader_call, &usage(3, true)).unwrap();
    let platform_detail = f.ok(&f.b, "GET", "/api/admin/charges?operator_task_ref=public-build", json!({}));
    assert_eq!(platform_detail["total"], 3);
    assert!(platform_detail["items"].as_array().unwrap().iter().all(|row| row["cost_owner"] == "platform"));
    let reader_detail = f.ok(&f.b, "GET", "/api/admin/charges?user_id=A&task_ref=public-build", json!({}));
    assert_eq!(reader_detail["total"], 1);
    assert_eq!(reader_detail["items"][0]["call_id"], "same-reference-account");
}

#[test]
fn q2_transport_costs_share_parent_but_keep_workspace_and_account_wide_ownership() {
    let fake = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let url = format!("http://{}", fake.server_addr().to_ip().unwrap());
    let mut f = Fixture::new();
    adm5_tests::seed(&mut f, &url, 100_000);
    workspace(&f, "A", "learning-A");
    workspace(&f, "A", "learning-B");
    // Another open reading scene must not change the explicitly bound task.
    let _other_scene = f.create(&f.a, &f.y, "last-open-page");
    let worker = std::thread::spawn(move || {
        for _ in 0..5 {
            let request = fake.recv_timeout(Duration::from_secs(10)).unwrap().unwrap();
            request.respond(tiny_http::Response::from_string(json!({
                "choices":[{"message":{"role":"assistant","content":"{\"ok\":true}"},"finish_reason":"stop"}],
                "usage":{"prompt_tokens":10,"prompt_cache_hit_tokens":0,"completion_tokens":1,"total_tokens":11}
            }).to_string()).with_header(tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap())).unwrap();
        }
    });
    let adapter = ProviderRegistry::adapter_from_config(ProviderConfig::from_values("native", "fixture-key", url, "fixture-model").unwrap());
    for purpose in ["answer", "compaction", "repair", "memory_review"] {
        let global = purpose == "memory_review";
        let scope = if global {
            ChargeScope::ReaderTask { user_id: "A".into(), task_ref: "review-child".into() }
        } else {
            ChargeScope::ReaderRun { user_id: "A".into(), run_ref: "answer-child".into() }
        };
        let attribution = ChargeAttribution {
            parent_task_ref: "learning-task".into(),
            learning_workspace_id: if global { None } else { Some("learning-A".into()) },
            related_learning_workspace_ids: if global { vec!["learning-A".into(), "learning-B".into()] } else { vec![] },
        };
        adapter.set_spend_context(scope.clone(), Some(f.access.spend.port_with_attribution(scope, f.x.clone(), attribution, Default::default())));
        adapter.set_model_purpose(purpose);
        assert_eq!(adapter.complete_structured(CompletionRequest {
            system: "fixture".into(), user: "fixture".into(), output_token_limit: Some(100), reasoning_effort: None,
        }).unwrap()["ok"], true);
    }
    adapter.set_spend_context(
        ChargeScope::OperatorTask { task_ref: "transport-public-build".into() },
        Some(f.access.spend.operator_port("transport-public-build".into(), Default::default())),
    );
    adapter.set_model_purpose("prebuild");
    adapter.complete_structured(CompletionRequest {
        system: "fixture".into(), user: "fixture".into(), output_token_limit: Some(100), reasoning_effort: None,
    }).unwrap();
    worker.join().unwrap();
    let charges = f.ok(&f.b, "GET", "/api/admin/charges?user_id=A", json!({}));
    assert_eq!(charges["total"], 4);
    let rows = charges["items"].as_array().unwrap();
    for purpose in ["answer", "compaction", "repair", "memory_review"] {
        let charge = rows.iter().find(|row| row["purpose"] == purpose).unwrap();
        assert_eq!(charge["parent_task_ref"], "learning-task");
        assert_eq!(charge["provider_cost_micro_cny"], 11);
        assert_eq!(charge["account_debit_micro_cny"], 11);
        if purpose == "memory_review" {
            assert!(charge["learning_workspace_id"].is_null());
            assert_eq!(charge["related_learning_workspace_ids"], json!(["learning-A", "learning-B"]));
        } else {
            assert_eq!(charge["learning_workspace_id"], "learning-A");
        }
    }
    assert_eq!(balance(&f)["debited_micro_cny"], 44, "parent and source workspace references are not extra charges");
    let platform = f.ok(&f.b, "GET", "/api/admin/charges?operator_task_ref=transport-public-build", json!({}));
    assert_eq!(platform["total"], 1);
    assert_eq!(platform["items"][0]["provider_cost_micro_cny"], 11);
    assert_eq!(platform["items"][0]["account_debit_micro_cny"], 0);
}

#[test]
fn q2_workspace_attribution_checks_owner_and_waiver_keeps_actual_cost_and_task() {
    let mut f = Fixture::new();
    adm5_tests::seed(&mut f, "fixture", 1000);
    workspace(&f, "A", "owned");
    workspace(&f, "B", "foreign");
    let scope = ChargeScope::ReaderTask { user_id: "A".into(), task_ref: "child".into() };
    for (primary, related) in [(Some("foreign".into()), vec![]), (None, vec!["foreign".into()])] {
        let port = f.access.spend.port_with_attribution(scope.clone(), f.x.clone(), ChargeAttribution {
            parent_task_ref: "parent".into(), learning_workspace_id: primary, related_learning_workspace_ids: related,
        }, Default::default());
        assert_eq!(port.before_send(&identity("rejected", scope.clone()), &request()), Err(SpendStop::PermissionRevoked));
    }
    let cancel = runtime::run_context::CancellationToken::default();
    let port = f.access.spend.port_with_attribution(scope.clone(), f.x.clone(), ChargeAttribution {
        parent_task_ref: "parent".into(), learning_workspace_id: Some("owned".into()), related_learning_workspace_ids: vec![],
    }, cancel.clone());
    let call = identity("cancelled", scope);
    port.before_send(&call, &request()).unwrap();
    cancel.cancel();
    // Cancellation and archiving after admission cannot erase returned usage.
    f.control.connection.execute("UPDATE learning_workspaces SET archived=1 WHERE learning_workspace_id='owned'", []).unwrap();
    port.after_send(&call, &usage(23, false)).unwrap();
    let background_scope = ChargeScope::ReaderTask { user_id:"A".into(), task_ref:"archived-review".into() };
    let background = f.access.spend.port_with_attribution(background_scope.clone(), f.x.clone(), ChargeAttribution {
        parent_task_ref:"original-review-job".into(), learning_workspace_id:None, related_learning_workspace_ids:vec!["owned".into()],
    }, Default::default());
    assert_eq!(background.before_send(&identity("after-archive", background_scope), &request()), Err(SpendStop::PermissionRevoked));
    let before = charge(&f, &call.call_id);
    f.ok(&f.b, "POST", "/api/admin/charges/cancelled/reconcile", json!({
        "operation_id":"q2-waive","revision":before["revision"],"waive_account":true,"reason":"fixture waiver","evidence":"original usage retained"
    }));
    port.after_send(&call, &usage(23, false)).unwrap();
    let after = charge(&f, &call.call_id);
    assert_eq!(after["provider_cost_micro_cny"], 23);
    assert_eq!(after["account_debit_micro_cny"], 0);
    assert_eq!(after["parent_task_ref"], "parent");
    assert_eq!(after["learning_workspace_id"], "owned");
    assert_eq!(after["reports"]["total"], 1);
    assert_eq!(after["reconciliations"]["total"], 1);
    assert_eq!(balance(&f)["debited_micro_cny"], 0);
}
