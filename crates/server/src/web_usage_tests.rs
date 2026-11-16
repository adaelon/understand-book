use super::*;
use crate::control_store::{ServiceWriter, CONTROL_SCHEMA_VERSION};

const NOW: i64 = 100;
const RUN: &str = "web-run";

struct Fixture {
    store: ModelSpendStore,
    root: tempfile::TempDir,
}

impl Fixture {
    fn new(allowance: i64) -> Self {
        let root = tempfile::tempdir().unwrap();
        let writer = ServiceWriter::acquire(root.path()).unwrap();
        let mut control = ControlStore::open(writer).unwrap();
        control.create_user("alice").unwrap();
        control.connection.execute_batch(
            "INSERT INTO allowance_periods(period_id,user_id,starts_at,expires_at) VALUES('period','alice',0,1000);
             INSERT INTO admin_operations(actor,operation_id,user_id,kind,parameters_json,result_json,created_at)
             VALUES('alice','grant','alice','allowance_adjustment','{}','{}',0);",
        ).unwrap();
        control.connection.execute(
            "INSERT INTO allowance_adjustments(actor,operation_id,user_id,period_id,delta_micro_cny,reason,created_at)
             VALUES('alice','grant','alice','period',?,'fixture allowance',0)",
            [allowance],
        ).unwrap();
        Self {
            store: ModelSpendStore::new(control),
            root,
        }
    }

    fn rates(&self, search: i64, fetch: i64) {
        std::fs::write(self.root.path().join("web-rates.json"), json!({"rates":[
            {"version":format!("search-{search}"),"provider":"deepseek","service_kind":"web_search","starts_at":0,"expires_at":1000,"cost_micro_cny_per_request":search,"source":"fixture contract"},
            {"version":format!("fetch-{fetch}"),"provider":"deepseek","service_kind":"web_fetch","starts_at":0,"expires_at":1000,"cost_micro_cny_per_request":fetch,"source":"fixture contract"}
        ]}).to_string()).unwrap();
    }

    fn admit(
        &self,
        kind: WebServiceKind,
        logical: &str,
        attempt: u32,
    ) -> Result<String, ToolError> {
        admit(
            &mut self.store.control.lock().unwrap(),
            "alice",
            RUN,
            None,
            kind,
            logical,
            attempt,
            NOW,
        )
    }

    fn receipt(&self, id: &str, outcome: WebChargeOutcome) -> Value {
        self.store
            .web_receipt(
                RUN,
                id,
                outcome,
                json!({"source":"fixture","outcome":outcome}),
                NOW + 1,
            )
            .unwrap()
    }

    fn balance(&self) -> crate::account_allowance::AllowanceBalance {
        crate::account_allowance::balance(&self.store.control.lock().unwrap().connection, "period")
            .unwrap()
    }

    fn count(&self, table: &str) -> i64 {
        self.store
            .control
            .lock()
            .unwrap()
            .connection
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
            .unwrap()
    }

    fn seed_existing_costs(&self, model: i64, service: i64) {
        let control = self.store.control.lock().unwrap();
        control.connection.execute(
            "INSERT INTO model_call_charges(call_id,logical_call_id,attempt,user_id,period_id,run_ref,parent_task_ref,purpose,model,rate_snapshot,reservation_estimate,state,reserved_micro_cny,provider_cost_micro_cny,provider_cost_status,account_debit_micro_cny,evidence_kind,created_at,settled_at)
             VALUES('model-existing','model-existing',1,'alice','period','web-run','web-run','chat','fixture-model','{\"provider\":\"ahriapi\"}','{}','settled',?1,?1,'confirmed',?1,'provider_usage',90,91)",
            [model],
        ).unwrap();
        control.connection.execute(
            "INSERT INTO service_usage(usage_id,logical_usage_id,attempt,run_ref,parent_task_ref,cost_owner,user_id,period_id,service_kind,admitted_quantity_ms,quantity_ms,rate_snapshot,admission_json,state,reserved_micro_cny,service_charge_micro_cny,account_debit_micro_cny,provider_cost_micro_cny,provider_cost_status,created_at,settled_at)
             VALUES('service-existing','service-existing',1,'web-run','web-run','account','alice','period','video_transcription',1000,1000,'null','{}','settled',?1,?1,?1,?1,'confirmed',90,91)",
            [service],
        ).unwrap();
        control.connection.execute_batch(
            "INSERT INTO charge_reports VALUES('model-existing','{\"source\":\"fixture\"}','accepted',91);
             INSERT INTO service_usage_receipts VALUES('service-existing','{\"source\":\"fixture\"}',91);",
        ).unwrap();
    }
}

#[test]
fn unlimited_allowance_allows_web_usage_and_preserves_actual_costs() {
    let f = Fixture::new(1);
    f.rates(12_000_000, 3_000_000);
    assert_eq!(f.admit(WebServiceKind::Search, "limited", 1).unwrap_err().error_code, "ALLOWANCE_INSUFFICIENT");
    f.store.control.lock().unwrap().connection.execute("UPDATE allowance_periods SET unlimited=1 WHERE period_id='period'", []).unwrap();
    for (kind, logical) in [(WebServiceKind::Search, "unlimited-search"), (WebServiceKind::Fetch, "unlimited-fetch")] {
        let id = f.admit(kind, logical, 1).unwrap();
        f.receipt(&id, WebChargeOutcome::Billed);
    }
    assert_eq!(f.balance().debited_micro_cny.value(), 15_000_000);
    assert!(f.balance().can_reserve(20_000_000));
}

#[test]
fn unlimited_allowance_schema20_upgrade_preserves_existing_balances() {
    let f = Fixture::new(100);
    f.seed_existing_costs(17, 11);
    let writer = {
        let control = f.store.control.lock().unwrap();
        // The fixture starts on the current schema; these later tables did not
        // exist in schema 20 and must not collide with its forward migrations.
        control.connection.execute_batch("DROP TABLE operational_successes; DROP TABLE operational_outcomes;
            ALTER TABLE allowance_periods DROP COLUMN unlimited; PRAGMA user_version=20;").unwrap();
        control.writer.clone()
    };
    drop(f.store);
    for _ in 0..2 {
        let upgraded = ControlStore::open(writer.clone()).unwrap();
        let balance = crate::account_allowance::balance(&upgraded.connection, "period").unwrap();
        assert!(!balance.unlimited);
        assert_eq!(balance.available_micro_cny.value(), 72);
        assert!(balance.can_reserve(72));
        assert!(!balance.can_reserve(73));
        assert_eq!(upgraded.connection.pragma_query_value(None, "user_version", |r| r.get::<_,i64>(0)).unwrap(), CONTROL_SCHEMA_VERSION);
    }
}

#[test]
fn web_usage_requires_explicit_active_rates_and_settles_the_admitted_rate() {
    let f = Fixture::new(100);
    assert_eq!(
        f.admit(WebServiceKind::Search, "missing", 1)
            .unwrap_err()
            .error_code,
        "WEB_RATE_UNAVAILABLE"
    );
    assert_eq!(f.count("web_usage"), 0);
    assert_eq!(f.balance().available_micro_cny.value(), 100);

    f.rates(5, 7);
    let id = f.admit(WebServiceKind::Search, "priced", 1).unwrap();
    f.rates(50, 70);
    let done = f.receipt(&id, WebChargeOutcome::Billed);
    assert_eq!(done["charge"]["rate_snapshot"]["version"], "search-5");
    assert_eq!(done["charge"]["provider_cost_micro_cny"], 5);
    assert_eq!(done["charge"]["account_debit_micro_cny"], 5);

    let mut control = f.store.control.lock().unwrap();
    let expired = admit(
        &mut control,
        "alice",
        RUN,
        None,
        WebServiceKind::Fetch,
        "expired",
        1,
        1000,
    )
    .unwrap_err();
    assert_eq!(expired.error_code, "WEB_RATE_UNAVAILABLE");
}

#[test]
fn web_usage_separates_search_fetch_and_model_costs_without_double_counting_receipts() {
    let f = Fixture::new(100);
    f.rates(5, 7);
    f.seed_existing_costs(17, 11);
    let search = f.admit(WebServiceKind::Search, "search", 1).unwrap();
    let fetch = f.admit(WebServiceKind::Fetch, "fetch", 1).unwrap();
    let free = f.admit(WebServiceKind::Search, "empty", 1).unwrap();
    let unknown = f.admit(WebServiceKind::Fetch, "lost-response", 1).unwrap();
    let search_receipt = f.receipt(&search, WebChargeOutcome::Billed);
    let fetch_receipt = f.receipt(&fetch, WebChargeOutcome::Billed);
    let free_receipt = f.receipt(&free, WebChargeOutcome::Free);
    let pending = f.receipt(&unknown, WebChargeOutcome::Unknown);
    assert_eq!(search_receipt["charge"]["entry_kind"], "web_search");
    assert_eq!(fetch_receipt["charge"]["entry_kind"], "web_fetch");
    assert!(search_receipt["charge"]["model"].is_null());
    assert_eq!(free_receipt["charge"]["state"], "released");
    assert_eq!(free_receipt["charge"]["provider_cost_micro_cny"], 0);
    assert_eq!(pending["charge"]["state"], "pending");
    assert!(pending["charge"]["provider_cost_micro_cny"].is_null());
    assert!(pending["charge"]["account_debit_micro_cny"].is_null());
    assert_eq!(f.receipt(&search, WebChargeOutcome::Billed), search_receipt);
    assert_eq!(f.count("web_usage_receipts"), 4);
    let conflict = f
        .store
        .web_receipt(
            RUN,
            &search,
            WebChargeOutcome::Free,
            json!({"source":"conflicting receipt"}),
            NOW + 2,
        )
        .unwrap_err();
    assert_eq!(conflict.error_code, "WEB_RECEIPT_CONFLICT");
    assert_eq!(f.count("web_usage_receipts"), 4);

    let summary = f
        .store
        .cost_summary(Some("alice"), None, 20, 0, false, NOW)
        .unwrap();
    for aggregate in [&summary["account"], &summary["tasks"]["items"][0]] {
        assert_eq!(aggregate["request_count"], 6);
        assert_eq!(aggregate["confirmed_cost_micro_cny"], 40);
        assert_eq!(aggregate["account_debit_micro_cny"], 40);
        assert_eq!(aggregate["pending_micro_cny"], 7);
        assert_eq!(aggregate["provider_cost_complete"], false);
        let breakdown = &aggregate["cost_breakdown"];
        assert_eq!(breakdown["model"]["confirmed_cost_micro_cny"], 17);
        assert_eq!(breakdown["service"]["confirmed_cost_micro_cny"], 11);
        assert_eq!(breakdown["web_search"]["confirmed_cost_micro_cny"], 5);
        assert_eq!(breakdown["web_search"]["request_count"], 2);
        assert_eq!(breakdown["web_fetch"]["confirmed_cost_micro_cny"], 7);
        assert_eq!(breakdown["web_fetch"]["pending_micro_cny"], 7);
        assert_eq!(breakdown["web_fetch"]["unknown_cost_requests"], 1);
    }
    assert_eq!(f.balance().available_micro_cny.value(), 53);
}

#[test]
fn web_usage_retries_keep_unknown_attempts_and_share_the_existing_allowance() {
    let f = Fixture::new(20);
    f.rates(5, 7);
    f.seed_existing_costs(5, 3);
    let first = f.admit(WebServiceKind::Search, "retry", 1).unwrap();
    assert_eq!(
        f.admit(WebServiceKind::Search, "retry", 1)
            .unwrap_err()
            .error_code,
        "WEB_ALREADY_ADMITTED"
    );
    assert_eq!(
        f.admit(WebServiceKind::Search, "retry", 2)
            .unwrap_err()
            .error_code,
        "WEB_ALREADY_ADMITTED"
    );
    f.receipt(&first, WebChargeOutcome::Unknown);
    let retry = f.admit(WebServiceKind::Search, "retry", 2).unwrap();
    assert_ne!(first, retry);
    f.receipt(&retry, WebChargeOutcome::Billed);
    assert_eq!(f.count("web_usage"), 2);
    assert_eq!(f.balance().pending_micro_cny.value(), 5);
    assert_eq!(f.balance().debited_micro_cny.value(), 13);
    assert_eq!(f.balance().available_micro_cny.value(), 2);
    assert_eq!(
        f.admit(WebServiceKind::Fetch, "too-expensive", 1)
            .unwrap_err()
            .error_code,
        "ALLOWANCE_INSUFFICIENT"
    );
    assert_eq!(f.count("web_usage"), 2);

    // A later free receipt releases only its own attempt, not the billed retry.
    f.receipt(&first, WebChargeOutcome::Free);
    assert_eq!(f.balance().available_micro_cny.value(), 7);
    let fetch = f.admit(WebServiceKind::Fetch, "now-affordable", 1).unwrap();
    assert_eq!(f.balance().active_reserved_micro_cny.value(), 7);
    f.receipt(&fetch, WebChargeOutcome::Billed);
    assert_eq!(f.balance().available_micro_cny.value(), 0);
    assert_eq!(f.balance().debited_micro_cny.value(), 20);
}

#[test]
fn web_usage_recovery_and_late_receipt_keep_original_period_and_unknown_cost() {
    let f = Fixture::new(100);
    f.rates(5, 7);
    let id = f.admit(WebServiceKind::Fetch, "interrupted", 1).unwrap();
    f.store.recover().unwrap();
    assert_eq!(f.balance().active_reserved_micro_cny.value(), 0);
    assert_eq!(f.balance().pending_micro_cny.value(), 7);
    {
        let control = f.store.control.lock().unwrap();
        let pending = charge(&control.connection, &id).unwrap().unwrap();
        assert_eq!(pending["state"], "pending");
        assert!(pending["provider_cost_micro_cny"].is_null());
        control.connection.execute_batch(
            "UPDATE allowance_periods SET closed_at=101 WHERE period_id='period';
             INSERT INTO allowance_periods(period_id,user_id,starts_at,expires_at) VALUES('next-period','alice',101,2000);",
        ).unwrap();
    }
    let done = f.receipt(&id, WebChargeOutcome::Billed);
    assert_eq!(done["charge"]["period_id"], "period");
    assert_eq!(done["charge"]["account_debit_micro_cny"], 7);
    let control = f.store.control.lock().unwrap();
    assert_eq!(
        crate::account_allowance::balance(&control.connection, "next-period")
            .unwrap()
            .debited_micro_cny
            .value(),
        0
    );
}

#[test]
fn web_usage_manual_reconciliation_is_visible_in_reader_list_and_web_detail() {
    let f = Fixture::new(100);
    f.rates(5, 7);
    let id = f
        .admit(WebServiceKind::Fetch, "recovered-receipt", 1)
        .unwrap();
    let pending = f.receipt(&id, WebChargeOutcome::Unknown);
    let mut input = crate::charge_admin::Reconcile {
        revision: pending["charge"]["revision"].as_i64().unwrap(),
        provider_cost_micro_cny: Some(7),
        waive_account: false,
        reason: " Recovered provider receipt ".into(),
        evidence: " Provider confirms one billed page read ".into(),
    };
    input.normalize().unwrap();
    let result = {
        let mut control = f.store.control.lock().unwrap();
        let tx = control.connection.transaction().unwrap();
        tx.execute(
            "INSERT INTO admin_operations(actor,operation_id,user_id,kind,parameters_json,result_json,created_at)
             VALUES('alice','web-reconcile','alice','charge-reconcile',?,'{}',?)",
            params![json!({"call_id":id,"input":input}).to_string(), NOW + 2],
        ).unwrap();
        let result = input
            .apply(&tx, "alice", "web-reconcile", "alice", &id, NOW + 2)
            .unwrap();
        tx.commit().unwrap();
        result
    };
    assert_eq!(result["charge"]["entry_kind"], "web_fetch");
    assert_eq!(result["charge"]["state"], "settled");
    assert_eq!(result["charge"]["provider_cost_micro_cny"], 7);
    assert_eq!(result["charge"]["account_debit_micro_cny"], 7);
    assert_eq!(result["account_delta_micro_cny"], 7);
    assert!(result["provider_delta_micro_cny"].is_null());
    assert_eq!(f.balance().pending_micro_cny.value(), 0);
    assert_eq!(f.balance().available_micro_cny.value(), 93);

    let list = f
        .store
        .charges(
            Some("alice"),
            false,
            20,
            0,
            true,
            None,
            Some(("run", RUN)),
            None,
        )
        .unwrap();
    assert_eq!(list["total"], 1);
    assert_eq!(list["items"][0]["call_id"], id);
    assert_eq!(list["items"][0]["entry_kind"], "web_fetch");
    assert_eq!(list["items"][0]["service_kind"], "web_fetch");
    assert_eq!(list["items"][0]["account_debit_micro_cny"], 7);
    assert!(list["items"][0]["model"].is_null());
    assert_eq!(list["period_groups"][0]["debited_micro_cny"], 7);
    assert_eq!(list["period_groups"][0]["unknown_provider_cost_count"], 0);
    assert_eq!(
        f.store
            .charges(Some("alice"), true, 20, 0, true, None, None, None)
            .unwrap()["total"],
        0
    );

    let detail = f.store.charge_detail(&id, 20, 0).unwrap();
    assert_eq!(detail["reports"]["total"], 1);
    assert_eq!(
        detail["reports"]["items"][0]["outcome"]["outcome"],
        "unknown"
    );
    assert_eq!(detail["reconciliations"]["total"], 1);
    let correction = &detail["reconciliations"]["items"][0];
    assert_eq!(correction["operation_id"], "web-reconcile");
    assert_eq!(correction["before"]["state"], "pending");
    assert_eq!(correction["after"]["state"], "settled");
    assert_eq!(correction["account_delta_micro_cny"], 7);
    assert_eq!(correction["reason"], "Recovered provider receipt");
    assert_eq!(
        correction["evidence"],
        "Provider confirms one billed page read"
    );
    assert_eq!(f.count("web_usage_reconciliations"), 1);
    assert_eq!(f.count("charge_reconciliations"), 0);
    assert_eq!(f.count("model_call_charges"), 0);
}

#[test]
fn web_usage_schema_19_upgrade_preserves_model_service_and_their_receipts() {
    let f = Fixture::new(100);
    f.seed_existing_costs(17, 11);
    let (writer, before_model, before_service) = {
        let control = f.store.control.lock().unwrap();
        let model =
            crate::model_spend_store::charge(&control.connection, "model-existing").unwrap();
        let service =
            crate::service_usage::charge(&control.connection, "service-existing").unwrap();
        // Recreate precisely the pre-web projection while retaining existing data.
        control
            .connection
            .execute_batch(
                "DROP TABLE operational_successes;
             DROP TABLE operational_outcomes;
             ALTER TABLE allowance_periods DROP COLUMN unlimited;
             DROP VIEW billable_usage;
             DROP TABLE web_usage_reconciliations;
             DROP TABLE web_usage_receipts;
             DROP TABLE web_usage;",
            )
            .unwrap();
        let old_view = include_str!("service_usage.sql")
            .split_once("CREATE VIEW billable_usage AS")
            .unwrap()
            .1;
        control
            .connection
            .execute_batch(&format!("CREATE VIEW billable_usage AS{old_view}"))
            .unwrap();
        control
            .connection
            .pragma_update(None, "user_version", 19)
            .unwrap();
        (control.writer.clone(), model, service)
    };
    drop(f.store);
    let upgraded = ControlStore::open(writer.clone()).unwrap();
    assert_eq!(
        upgraded
            .connection
            .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        CONTROL_SCHEMA_VERSION
    );
    assert_eq!(
        crate::model_spend_store::charge(&upgraded.connection, "model-existing").unwrap(),
        before_model
    );
    assert_eq!(
        crate::service_usage::charge(&upgraded.connection, "service-existing").unwrap(),
        before_service
    );
    for table in ["charge_reports", "service_usage_receipts"] {
        assert_eq!(
            upgraded
                .connection
                .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }
    assert_eq!(
        crate::account_allowance::balance(&upgraded.connection, "period")
            .unwrap()
            .debited_micro_cny
            .value(),
        28
    );
    assert_eq!(
        upgraded
            .connection
            .query_row("SELECT count(*) FROM web_usage", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
    drop(upgraded);
    // Reopening an upgraded database must not rerun the migration or reset costs.
    let reopened = ControlStore::open(writer).unwrap();
    assert_eq!(
        crate::account_allowance::balance(&reopened.connection, "period")
            .unwrap()
            .available_micro_cny
            .value(),
        72
    );
}
