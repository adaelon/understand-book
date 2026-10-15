use super::mu5_tests::Fixture;
use crate::{control_store::ControlStore, model_spend_store::ModelSpendStore};
use runtime::{
    model_spend::*, provider_stream::ModelUsage, CompletionRequest, ProviderConfig,
    ProviderRegistry,
};
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};

fn rates(url: &str) -> Value {
    json!({"rates":[{"version":"fixture","provider":url,"model":"fixture-model","starts_at":0,"expires_at":i64::MAX,
        "input_micro_cny_per_million":1000000,"cached_input_micro_cny_per_million":100000,"output_micro_cny_per_million":1000000,
        "usage_contract":"deep_seek","image_meter":"deep_seek1024","source":{"currency":"CNY","original_prices":"fixture only","cny_conversion":"1","checked_on":"2026-10-08","official_url":"https://api-docs.deepseek.com/quick_start/pricing/"}}]})
}
pub(super) fn seed(f: &mut Fixture, url: &str, amount: i64) -> String {
    seed_model(f, url, "fixture-model", amount)
}
pub(super) fn seed_model(f: &mut Fixture, url: &str, model: &str, amount: i64) -> String {
    f.control.set_reader_admin("B", true).unwrap();
    let now = crate::multi_user_host::now();
    let period = f.ok(
        &f.b,
        "POST",
        "/api/admin/users/A/allowance-periods",
        json!({"operation_id":"period","starts_at":now-100,"expires_at":now+3600}),
    )["allowance_period"]["period_id"]
        .as_str()
        .unwrap()
        .to_owned();
    f.ok(&f.b,"POST","/api/admin/users/A/allowance-adjustments",json!({"operation_id":"grant","period_id":period,"revision":0,"delta_micro_cny":amount,"reason":"fixture"}));
    let mut rates = rates(url);
    rates["rates"][0]["model"] = json!(model);
    std::fs::write(
        f.root.path().join("model-rates.json"),
        rates.to_string(),
    )
    .unwrap();
    Arc::get_mut(&mut f.access).unwrap().spend = Arc::new(ModelSpendStore::new(
        ControlStore::open(f.control.writer.clone()).unwrap(),
    ));
    period
}
fn fixture() -> (Fixture, String) {
    let mut f = Fixture::new();
    let p = seed(&mut f, "fixture", 1000);
    (f, p)
}
fn scope() -> ChargeScope {
    ChargeScope::ReaderRun {
        user_id: "A".into(),
        run_ref: "test-run".into(),
    }
}
fn id(call: &str) -> SendIdentity {
    SendIdentity {
        call_id: call.into(),
        logical_call_id: call.into(),
        attempt: 1,
        scope: Some(scope()),
        purpose: "outer".into(),
        model: "fixture-model".into(),
        provider: "fixture".into(),
    }
}
fn port(f: &Fixture) -> Arc<dyn ModelSpendPort> {
    f.access
        .spend
        .port(scope(), f.x.clone(), Default::default())
}
fn request(output: u32) -> Value {
    json!({"model":"fixture-model","messages":[{"role":"user","content":"private request marker"}],"max_tokens":output})
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
fn charge(f: &Fixture, call: &str) -> Value {
    f.ok(
        &f.b,
        "GET",
        &format!("/api/admin/charges/{call}"),
        json!({}),
    )
}
fn balance(f: &Fixture) -> Value {
    f.ok(&f.a, "GET", "/api/account/allowance", json!({}))["current_allowance"]["balance"].clone()
}
fn reconcile(f: &Fixture, call: &str, op: &str, cost: Option<i64>, waive: bool) -> Value {
    let revision = charge(f, call)["revision"].clone();
    json!({"operation_id":op,"revision":revision,"provider_cost_micro_cny":cost,"waive_account":waive,"reason":" correction ","evidence":" invoice line 1 "})
}

#[test]
fn adm5_two_connections_cannot_reserve_the_same_balance_and_overrun_is_not_capped() {
    let (f, _) = fixture();
    let store = Arc::new(ModelSpendStore::new(
        ControlStore::open(f.control.writer.clone()).unwrap(),
    ));
    let ports = [
        port(&f),
        store.port(scope(), f.x.clone(), Default::default()),
    ];
    let barrier = std::sync::Barrier::new(2);
    let results = std::thread::scope(|s| {
        let handles: Vec<_> = ports
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let b = &barrier;
                s.spawn(move || {
                    b.wait();
                    p.before_send(&id(&format!("call-{i}")), &request(800))
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert!(results.contains(&Err(SpendStop::InsufficientAllowance)));
    let winner = results.iter().position(|r| r.is_ok()).unwrap();
    let call = id(&format!("call-{winner}"));
    assert_eq!(balance(&f)["debited_micro_cny"], 0);
    assert!(balance(&f)["active_reserved_micro_cny"].as_i64().unwrap() > 800);
    ports[winner].after_send(&call, &outcome(1200)).unwrap();
    ports[winner].after_send(&call, &outcome(1200)).unwrap();
    assert_eq!(balance(&f)["available_micro_cny"], -200);
    assert_eq!(balance(&f)["debited_micro_cny"], 1200);
    assert_eq!(
        port(&f).before_send(&id("next"), &request(1)),
        Err(SpendStop::InsufficientAllowance)
    );
    assert_eq!(charge(&f, &call.call_id)["reports"]["total"], 1);
}

#[test]
fn adm5_pending_late_usage_waiver_conflicts_and_append_only_manual_corrections() {
    let (f, p) = fixture();
    let port = port(&f);
    let call = id("pending");
    port.before_send(&call, &request(200)).unwrap();
    port.after_send(&call, &unknown()).unwrap();
    let pending = charge(&f, "pending");
    assert_eq!(pending["state"], "pending");
    assert_eq!(
        balance(&f)["pending_micro_cny"],
        pending["reserved_micro_cny"]
    );
    assert!(pending["provider_cost_micro_cny"].is_null());
    let waive = reconcile(&f, "pending", "waive", None, true);
    let url = "/api/admin/charges/pending/reconcile";
    let first = f.ok(&f.b, "POST", url, waive.clone());
    assert_eq!(first["charge"]["provider_cost_status"], "pending");
    assert_eq!(balance(&f)["available_micro_cny"], 1000);
    assert_eq!(
        f.ok(&f.b, "GET", "/api/admin/charges?pending=true", json!({}))["total"],
        1
    );
    port.after_send(&call, &unknown()).unwrap(); // original receipt remains idempotent after manual change
    assert_eq!(
        port.after_send(&call, &outcome(100)),
        Err(SpendStop::ReconciliationRequired)
    );
    assert_eq!(
        port.after_send(&call, &outcome(100)),
        Err(SpendStop::ReconciliationRequired)
    );
    assert_eq!(charge(&f, "pending")["reports"]["total"], 2);
    assert_eq!(charge(&f, "pending")["needs_reconciliation"], true);
    assert_eq!(balance(&f)["debited_micro_cny"], 0);
    assert_eq!(f.ok(&f.b, "POST", url, waive.clone()), first);
    let correction = reconcile(&f, "pending", "bill", Some(100), true);
    f.ok(&f.b, "POST", url, correction);
    assert_eq!(charge(&f, "pending")["provider_cost_micro_cny"], 100);
    assert_eq!(charge(&f, "pending")["account_debit_micro_cny"], 0);
    assert_eq!(
        f.ok(&f.b, "GET", "/api/admin/charges?pending=true", json!({}))["total"],
        0
    );
    let correction = reconcile(&f, "pending", "debit", Some(130), false);
    let receipt = f.ok(&f.b, "POST", url, correction.clone());
    assert_eq!(receipt["account_delta_micro_cny"], 130);
    assert_eq!(receipt["provider_delta_micro_cny"], 30);
    assert_eq!(receipt["charge"]["period_id"], p);
    assert_eq!(f.ok(&f.b, "POST", url, correction.clone()), receipt);
    assert_eq!(
        f.ok(&f.b, "GET", "/api/admin/operations/debit", json!({})),
        receipt
    );
    let mut different = correction;
    different["provider_cost_micro_cny"] = json!(140);
    assert_eq!(
        f.call(&f.b, "POST", url, different).1["error_code"],
        "ADMIN_OPERATION_CONFLICT"
    );
    let detail = charge(&f, "pending");
    assert_eq!(detail["reconciliations"]["total"], 3);
    assert_eq!(detail["provider_cost_micro_cny"], 130);
    let rows = detail["reconciliations"]["items"].as_array().unwrap();
    let original = rows.iter().find(|r| r["operation_id"] == "waive").unwrap();
    assert_eq!(original["before"]["state"], "pending");
    assert!(original["after"]["provider_cost_micro_cny"].is_null());
    assert_eq!(original["evidence"], "invoice line 1");
    assert_eq!(balance(&f)["available_micro_cny"], 870);
}

#[test]
fn adm5_late_receipt_keeps_original_period_and_not_sent_releases() {
    let (f, p) = fixture();
    let port = port(&f);
    let call = id("old");
    port.before_send(&call, &request(200)).unwrap();
    port.after_send(&call, &unknown()).unwrap();
    let now = crate::multi_user_host::now();
    f.ok(&f.b,"POST","/api/admin/users/A/allowance-validity",json!({"operation_id":"expire","period_id":p,"revision":1,"starts_at":now-100,"expires_at":now}));
    assert!(f.ok(&f.a, "GET", "/api/account/allowance", json!({}))["current_allowance"].is_null());
    assert_eq!(
        port.before_send(&id("expired"), &request(1)),
        Err(SpendStop::AllowanceExpired)
    );
    let new = f.ok(
        &f.b,
        "POST",
        "/api/admin/users/A/allowance-periods",
        json!({"operation_id":"next","starts_at":now,"expires_at":now+100}),
    )["allowance_period"]["period_id"]
        .clone();
    f.ok(&f.b,"POST","/api/admin/users/A/allowance-adjustments",json!({"operation_id":"gift","period_id":new,"revision":0,"delta_micro_cny":1000,"reason":"new period"}));
    port.after_send(&call, &outcome(80)).unwrap();
    assert_eq!(charge(&f, "old")["period_id"], p);
    assert_eq!(balance(&f)["available_micro_cny"], 1000);
    let call = id("not-sent");
    port.before_send(&call, &request(200)).unwrap();
    port.after_send(
        &call,
        &SendOutcome {
            evidence: SendEvidence::NotSent,
            usage: None,
            succeeded: false,
        },
    )
    .unwrap();
    assert_eq!(charge(&f, "not-sent")["provider_cost_micro_cny"], 0);
    assert_eq!(balance(&f)["available_micro_cny"], 1000);
    let call = id("error-with-usage");
    port.before_send(&call, &request(200)).unwrap();
    let mut result = outcome(40);
    result.succeeded = false;
    port.after_send(&call, &result).unwrap();
    assert_eq!(balance(&f)["available_micro_cny"], 960);
}

#[test]
fn adm5_failed_reservation_sent_marker_and_settlement_recover_without_replay() {
    for stage in ["insert", "sent", "settled"] {
        let (f, _) = fixture();
        let port = port(&f);
        let call = id(stage);
        if stage == "insert" {
            f.control.connection.execute_batch("CREATE TRIGGER failure BEFORE INSERT ON model_call_charges BEGIN SELECT RAISE(ABORT,'fixture'); END;").unwrap();
        } else {
            f.control.connection.execute_batch(&format!("CREATE TRIGGER failure BEFORE UPDATE ON model_call_charges WHEN NEW.state='{stage}' BEGIN SELECT RAISE(ABORT,'fixture'); END;")).unwrap();
        }
        if stage == "settled" {
            port.before_send(&call, &request(200)).unwrap();
            assert_eq!(
                port.after_send(&call, &outcome(50)),
                Err(SpendStop::StorageUnavailable)
            );
            assert_eq!(charge(&f, stage)["state"], "sent");
        } else {
            assert_eq!(
                port.before_send(&call, &request(200)),
                Err(SpendStop::StorageUnavailable)
            );
        }
        assert_eq!(
            port.before_send(&id("blocked"), &request(1)),
            Err(SpendStop::StorageUnavailable)
        );
        f.control
            .connection
            .execute_batch("DROP TRIGGER failure")
            .unwrap();
        let reopened = Arc::new(ModelSpendStore::new(
            ControlStore::open(f.control.writer.clone()).unwrap(),
        ));
        reopened.recover().unwrap(); // repeated startup reconciliation changes nothing further
        if stage == "insert" {
            assert_eq!(
                f.ok(&f.b, "GET", "/api/admin/charges", json!({}))["total"],
                0
            );
        } else if stage == "sent" {
            assert_eq!(charge(&f, stage)["state"], "released");
            assert_eq!(balance(&f)["available_micro_cny"], 1000);
        } else {
            assert_eq!(charge(&f, stage)["state"], "pending");
            assert!(balance(&f)["pending_micro_cny"].as_i64().unwrap() > 200);
            reopened
                .port(scope(), f.x.clone(), Default::default())
                .after_send(&call, &outcome(50))
                .unwrap();
            assert_eq!(balance(&f)["available_micro_cny"], 950);
        }
    }
}

#[test]
fn adm5_reconciliation_is_atomic_and_revision_conflicts_do_not_overwrite() {
    let (f, _) = fixture();
    let p = port(&f);
    p.before_send(&id("c"), &request(200)).unwrap();
    let early = reconcile(&f, "c", "early", Some(1), false);
    assert_eq!(
        f.call(&f.b, "POST", "/api/admin/charges/c/reconcile", early)
            .1["error_code"],
        "CHARGE_IN_FLIGHT"
    );
    p.after_send(&id("c"), &unknown()).unwrap();
    let body = reconcile(&f, "c", "fix", Some(20), false);
    f.control.connection.execute_batch("CREATE TRIGGER failure BEFORE UPDATE ON admin_operations BEGIN SELECT RAISE(ABORT,'fixture'); END;").unwrap();
    assert_eq!(
        f.call(&f.b, "POST", "/api/admin/charges/c/reconcile", body.clone())
            .0,
        503
    );
    assert_eq!(charge(&f, "c")["state"], "pending");
    assert_eq!(charge(&f, "c")["reconciliations"]["total"], 0);
    assert_eq!(
        f.call(&f.b, "GET", "/api/admin/operations/fix", json!({}))
            .0,
        404
    );
    f.control
        .connection
        .execute_batch("DROP TRIGGER failure")
        .unwrap();
    f.ok(&f.b, "POST", "/api/admin/charges/c/reconcile", body.clone());
    let mut stale = body;
    stale["operation_id"] = json!("other");
    assert_eq!(
        f.call(&f.b, "POST", "/api/admin/charges/c/reconcile", stale)
            .1["error_code"],
        "CHARGE_REVISION_CONFLICT"
    );
    assert_eq!(balance(&f)["debited_micro_cny"], 20);
}

#[test]
fn adm5_reader_queries_are_private_and_need_no_workspace() {
    let (f, _) = fixture();
    let p = port(&f);
    p.before_send(&id("c"), &request(200)).unwrap();
    p.after_send(&id("c"), &outcome(50)).unwrap();
    assert!(!f.root.path().join("users/A").exists());
    let mine = f.ok(&f.a, "GET", "/api/account/usage?limit=1", json!({}));
    assert_eq!(mine["total"], 1);
    assert!(mine["items"][0].get("rate_snapshot").is_none());
    assert!(mine["items"][0].get("send_identity").is_none());
    assert_eq!(
        f.ok(&f.b, "GET", "/api/account/usage", json!({}))["total"],
        0
    );
    assert_eq!(
        f.call(&f.a, "GET", "/api/account/allowance?user_id=B", json!({}))
            .0,
        400
    );
    assert_eq!(
        f.call(&f.a, "GET", "/api/account/usage?user_id=B", json!({}))
            .0,
        400
    );
    assert_eq!(f.call(&f.a, "GET", "/api/admin/charges", json!({})).0, 403);
    assert_eq!(
        f.call(
            &f.a,
            "POST",
            "/api/admin/charges/c/reconcile",
            reconcile(&f, "c", "bad", Some(0), true)
        )
        .0,
        403
    );
    assert_eq!(
        f.call(&f.b, "GET", "/api/admin/charges?limit=101", json!({}))
            .0,
        400
    );
    assert_eq!(
        f.ok(&f.b, "GET", "/api/admin/charges?user_id=B", json!({}))["total"],
        0
    );
    assert_eq!(
        f.ok(
            &f.b,
            "GET",
            "/api/admin/charges?limit=1&offset=1",
            json!({})
        )["items"],
        json!([])
    );
    assert!(!f.root.path().join("users/A").exists());
    assert!(!charge(&f, "c")
        .to_string()
        .contains("private request marker"));
}

#[test]
fn adm5_permissions_configuration_and_recovery_failure_do_not_block_reading() {
    let (mut f, _) = fixture();
    let p = port(&f);
    f.access.library.lock().unwrap().revoke("A", &f.x).unwrap();
    assert_eq!(
        p.before_send(&id("revoked"), &request(1)),
        Err(SpendStop::PermissionRevoked)
    );
    f.access.library.lock().unwrap().grant("A", &f.x).unwrap();
    f.control.set_user_disabled("A", true).unwrap();
    assert_eq!(
        p.before_send(&id("disabled"), &request(1)),
        Err(SpendStop::PermissionRevoked)
    );
    f.control.set_user_disabled("A", false).unwrap();
    f.a = f
        .access
        .auth
        .login(
            "A",
            "fixture-only-password",
            None,
            crate::multi_user_host::now(),
        )
        .unwrap();
    let cancel = runtime::run_context::CancellationToken::default();
    cancel.cancel();
    assert_eq!(
        f.access
            .spend
            .port(scope(), f.x.clone(), cancel)
            .before_send(&id("cancelled"), &request(1)),
        Err(SpendStop::PermissionRevoked)
    );
    for contents in ["{}", "{malformed"] {
        std::fs::write(f.root.path().join("model-rates.json"), contents).unwrap();
        let store = Arc::new(ModelSpendStore::new(
            ControlStore::open(f.control.writer.clone()).unwrap(),
        ));
        assert_eq!(
            store.validate_provider("fixture", "fixture-model"),
            Err(SpendStop::RateUnavailable)
        );
        assert_eq!(
            store
                .port(scope(), f.x.clone(), Default::default())
                .before_send(&id("bad-rate"), &request(1)),
            Err(SpendStop::RateUnavailable)
        );
        assert_eq!(
            f.ok(&f.a, "GET", "/api/library", json!({}))["books"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
    }
    p.before_send(&id("sent"), &request(100)).unwrap();
    f.control.connection.execute_batch("CREATE TRIGGER recovery_failure BEFORE UPDATE ON model_call_charges BEGIN SELECT RAISE(ABORT,'fixture'); END;").unwrap();
    let store = Arc::new(ModelSpendStore::new(
        ControlStore::open(f.control.writer.clone()).unwrap(),
    ));
    assert_eq!(
        store
            .port(scope(), f.x.clone(), Default::default())
            .before_send(&id("blocked"), &request(1)),
        Err(SpendStop::StorageUnavailable)
    );
    assert!(store.allowance("A", crate::multi_user_host::now()).is_ok());
}

#[test]
fn adm5_schema6_upgrade_is_atomic_and_keeps_charges() {
    use crate::control_store::ServiceWriter;
    let root = tempfile::tempdir().unwrap();
    let db = rusqlite::Connection::open(root.path().join("control.sqlite")).unwrap();
    db.execute_batch(include_str!("control_schema_v4.sql"))
        .unwrap();
    db.execute_batch("ALTER TABLE users ADD COLUMN is_admin INTEGER NOT NULL DEFAULT 0 CHECK(is_admin IN (0,1));")
        .unwrap();
    db.execute_batch(include_str!("../admin_schema.sql"))
        .unwrap();
    db.execute_batch(include_str!("../receipt_corrections.sql"))
        .unwrap();
    db.execute_batch("PRAGMA application_id=1430408533; PRAGMA user_version=6; INSERT INTO users(user_id) VALUES('A'); INSERT INTO allowance_periods(period_id,user_id,starts_at,expires_at) VALUES('p','A',0,100); INSERT INTO model_call_charges(call_id,logical_call_id,attempt,user_id,period_id,run_ref,purpose,model,rate_snapshot,reservation_estimate,state,reserved_micro_cny,provider_cost_status,evidence_kind,created_at) VALUES('old','old',1,'A','p','turn','outer','model','{}','{}','pending',42,'pending','missing',1); CREATE TABLE charge_reports(keep TEXT);").unwrap();
    let writer = ServiceWriter::acquire(root.path()).unwrap();
    assert!(ControlStore::open(writer.clone()).is_err());
    assert_eq!(
        db.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        6
    );
    assert!(db
        .prepare("SELECT revision FROM model_call_charges")
        .is_err());
    db.execute_batch("DROP TABLE charge_reports").unwrap();
    for _ in 0..2 {
        let store = ControlStore::open(writer.clone()).unwrap();
        assert_eq!(
            store
                .connection
                .query_row(
                    "SELECT reserved_micro_cny,revision,state FROM model_call_charges",
                    [],
                    |r| Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, i64>(1)?,
                        r.get::<_, String>(2)?
                    ))
                )
                .unwrap(),
            (42, 0, "pending".into())
        );
    }
}

fn completion() -> CompletionRequest {
    CompletionRequest {
        system: "fixture".into(),
        user: "fixture".into(),
        output_token_limit: Some(100),
        reasoning_effort: None,
    }
}
fn adapter(f: &Fixture, url: &str) -> Box<dyn runtime::ModelAdapter + Send> {
    let a = ProviderRegistry::adapter_from_config_with_timeout(
        ProviderConfig::from_values("native", "fixture-key", url, "fixture-model").unwrap(),
        Duration::from_secs(3),
    );
    a.set_spend_context(scope(), Some(port(f)));
    a
}
fn wire() -> String {
    let body=json!({"choices":[{"message":{"content":"{\"ok\":true}"},"finish_reason":"stop"}],"usage":{"prompt_tokens":20,"prompt_cache_hit_tokens":0,"completion_tokens":10,"total_tokens":30}}).to_string();
    format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len())
}
fn accept(listener: &std::net::TcpListener) -> std::net::TcpStream {
    use std::io::{BufRead, Read};
    listener.set_nonblocking(true).unwrap();
    let until = std::time::Instant::now() + Duration::from_secs(10);
    let mut socket = loop {
        match listener.accept() {
            Ok((s, _)) => break s,
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    && std::time::Instant::now() < until =>
            {
                std::thread::sleep(Duration::from_millis(5))
            }
            Err(e) => panic!("accept: {e}"),
        }
    };
    socket
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut reader = std::io::BufReader::new(&mut socket);
    let mut length = 0;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        if line == "\r\n" {
            break;
        }
        if let Some(v) = line.to_lowercase().strip_prefix("content-length:") {
            length = v.trim().parse().unwrap();
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).unwrap();
    socket
}

#[test]
fn adm5_real_transport_retry_has_separate_pending_and_settled_charges() {
    use std::io::Write;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let mut f = Fixture::new();
    seed(&mut f, &url, 1000);
    let worker = std::thread::spawn(move || {
        drop(accept(&listener));
        let mut socket = accept(&listener);
        socket.write_all(wire().as_bytes()).unwrap();
    });
    assert_eq!(
        adapter(&f, &url).complete_structured(completion()).unwrap()["ok"],
        true
    );
    worker.join().unwrap();
    let list = f.ok(&f.b, "GET", "/api/admin/charges", json!({}));
    let rows = list["items"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["logical_call_id"], rows[1]["logical_call_id"]);
    assert_ne!(rows[0]["call_id"], rows[1]["call_id"]);
    let pending = rows.iter().find(|r| r["state"] == "pending").unwrap();
    let settled = rows.iter().find(|r| r["state"] == "settled").unwrap();
    assert_eq!(pending["attempt"], 1);
    assert!(pending["provider_usage"].is_null());
    assert_eq!(settled["attempt"], 2);
    assert_eq!(settled["account_debit_micro_cny"], 30);
    assert_eq!(
        balance(&f)["available_micro_cny"].as_i64().unwrap(),
        1000 - 30 - pending["reserved_micro_cny"].as_i64().unwrap()
    );
}

#[test]
fn adm5_retry_rechecks_allowance_and_material_permission_before_http() {
    for revoked in [false, true] {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let mut f = Fixture::new();
        seed(&mut f, &url, if revoked { 1000 } else { 200 });
        let access = f.access.clone();
        let publication = f.x.clone();
        let worker = std::thread::spawn(move || {
            let socket = accept(&listener);
            if revoked {
                access
                    .library
                    .lock()
                    .unwrap()
                    .revoke("A", &publication)
                    .unwrap();
            }
            drop(socket);
            listener
        });
        let err = adapter(&f, &url)
            .complete_structured(completion())
            .unwrap_err();
        assert_eq!(
            err.message,
            if revoked {
                SpendStop::PermissionRevoked
            } else {
                SpendStop::InsufficientAllowance
            }
            .code()
        );
        let listener = worker.join().unwrap();
        assert!(listener.accept().is_err());
        assert_eq!(
            f.ok(&f.b, "GET", "/api/admin/charges", json!({}))["total"],
            1
        );
    }
}

#[test]
fn adm5_real_run_settles_before_failed_history_save_and_restart_never_resends() {
    let fake = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let url = format!("http://{}", fake.server_addr().to_ip().unwrap());
    let mut f = Fixture::new();
    seed(&mut f, &url, 1000000);
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
    input["client_request_id"] = json!("key");
    input["message"] = json!("你好");
    let accepted = f.call(
        &f.a,
        "POST",
        &format!(
            "/api/workspaces/{}/agent/runs",
            w["workspace_id"].as_str().unwrap()
        ),
        input.clone(),
    );
    assert_eq!(accepted.0, 202);
    let access = f.access.clone();
    let worker = std::thread::spawn(move || access.runs.run_one(&access));
    let request = fake.recv_timeout(Duration::from_secs(10)).unwrap().unwrap();
    {
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
    request.respond(tiny_http::Response::from_string(json!({"choices":[{"message":{"role":"assistant","content":"你好！"},"finish_reason":"stop"}],"usage":{"prompt_tokens":20,"prompt_cache_hit_tokens":0,"completion_tokens":10,"total_tokens":30}}).to_string()).with_header(tiny_http::Header::from_bytes("Content-Type","application/json").unwrap())).unwrap();
    worker.join().unwrap().unwrap();
    assert_eq!(balance(&f)["debited_micro_cny"], 30);
    assert!(f
        .control
        .connection
        .query_row(
            "SELECT unsaved FROM run_admissions WHERE client_request_id='key'",
            [],
            |r| r.get::<_, bool>(0)
        )
        .unwrap());
    let Fixture {
        root,
        access,
        control,
        x,
        y,
        a,
        b,
    } = f;
    drop(access);
    drop(control);
    let users = crate::user_registry::UserRegistry::open(root.path()).unwrap();
    let control = ControlStore::open(users.writer()).unwrap();
    let access = Arc::new(crate::authorization::Authorization::new(users).unwrap());
    access.runs.recover(&access).unwrap();
    assert!(!access.runs.run_one(&access).unwrap());
    let f = Fixture {
        root,
        access,
        control,
        x,
        y,
        a,
        b,
    };
    assert_eq!(balance(&f)["debited_micro_cny"], 30);
    assert!(fake.try_recv().unwrap().is_none());
}
