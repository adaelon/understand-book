use super::mu5_tests::Fixture;
use serde_json::{json, Value};

fn fixture() -> Fixture {
    let mut f = Fixture::new();
    f.control.set_reader_admin("B", true).unwrap();
    f
}
fn post(f: &Fixture, action: &str, body: Value) -> Value {
    f.ok(&f.b, "POST", &format!("/api/admin/users/A/{action}"), body)
}
fn period(f: &Fixture, operation: &str, start: i64, end: i64, basis: Option<(&str,i64)>) -> String {
    let mut body = json!({"operation_id":operation,"starts_at":start,"expires_at":end});
    if let Some((version,amount)) = basis { body["basis"] = json!({"version":version,"basis_micro_cny":amount}); }
    post(f,"allowance-periods",body)["allowance_period"]["period_id"].as_str().unwrap().to_owned()
}
fn adjustment(f: &Fixture, p: &str, op: &str, revision: i64, amount: i64) {
    post(f,"allowance-adjustments",json!({"operation_id":op,"period_id":p,"revision":revision,"delta_micro_cny":amount,"reason":"fixture allowance"}));
}
fn settled(f: &Fixture, p: &str, call: &str, cost: i64) {
    f.control.connection.execute("INSERT INTO model_call_charges(call_id,logical_call_id,attempt,user_id,period_id,run_ref,parent_task_ref,purpose,model,rate_snapshot,reservation_estimate,state,reserved_micro_cny,provider_cost_micro_cny,provider_cost_status,account_debit_micro_cny,evidence_kind,created_at,settled_at) VALUES(?1,?1,1,'A',?2,'run','lesson','answer','fixture','{}','{}','settled',0,?3,'confirmed',?3,'fixture',10,11)",rusqlite::params![call,p,cost]).unwrap();
}
fn account(f: &Fixture) -> Value { f.ok(&f.a,"GET","/api/account/allowance",json!({})) }
fn history(f: &Fixture) -> Value { f.ok(&f.a,"GET","/api/account/usage",json!({})) }

#[test]
fn q1_fixed_basis_topup_does_not_change_historical_percentages() {
    let f = fixture();
    let now = crate::multi_user_host::now();
    let p = period(&f,"period",now-10,now+1000,Some(("fixture-v1",1_000_000)));
    post(&f,"allowance-percentage-adjustments",json!({"operation_id":"gift","period_id":p,"revision":0,"basis_version":"fixture-v1","delta_milli_percent":73000,"reason":"fixture 73%"}));
    settled(&f,&p,"answer",23000);
    let before = history(&f)["items"][0]["allowance_percentages"].clone();
    assert_eq!(before["debited"]["value"],"2.3");
    assert_eq!(account(&f)["current_allowance"]["balance"]["percentages"]["available"]["value"],"70.7");
    let receipt = json!({"operation_id":"topup","period_id":p,"revision":1,"basis_version":"fixture-v1","delta_milli_percent":50000,"amount_fen":123,"paid_at":now,"channel":"fixture","note":"not commercial pricing"});
    let result = post(&f,"percentage-receipts",receipt.clone());
    assert_eq!(post(&f,"percentage-receipts",receipt),result);
    let balance = account(&f)["current_allowance"]["balance"].clone();
    assert_eq!(balance["available_micro_cny"],1_207_000);
    assert_eq!(balance["percentages"]["available"]["value"],"120.7");
    assert_eq!(history(&f)["items"][0]["allowance_percentages"],before);
    let receipts = f.ok(&f.b,"GET","/api/admin/users/A/receipts",json!({}));
    assert_eq!(receipts["items"][0]["amount_fen"],123);
    assert_eq!(receipts["total"],1);
    let adjustments = f.ok(&f.b,"GET","/api/admin/users/A/allowance-adjustments",json!({}));
    assert!(adjustments["items"].as_array().unwrap().iter().any(|v|v["delta_percentage"]["value"]=="50"));
    let reopen = crate::model_spend_store::ModelSpendStore::new(crate::control_store::ControlStore::open(f.control.writer.clone()).unwrap());
    assert_eq!(reopen.allowance("A",now).unwrap()["current_allowance"]["balance"],balance);
}

#[test]
fn q1_cross_period_task_history_groups_original_bases_before_pagination() {
    let f = fixture();
    let now = crate::multi_user_host::now();
    let old = period(&f,"old",now-100,now-1,Some(("fixture-v1",1_000_000)));
    let new = period(&f,"new",now,now+1000,Some(("fixture-v2",2_000_000)));
    adjustment(&f,&old,"old-grant",0,100000);
    adjustment(&f,&new,"new-grant",0,100000);
    settled(&f,&old,"old-call",23000);
    settled(&f,&new,"new-call",23000);
    let result = f.ok(&f.b,"GET","/api/admin/charges?user_id=A&parent_task_ref=lesson&limit=1",json!({}));
    assert_eq!(result["items"].as_array().unwrap().len(),1);
    assert_eq!(result["total"],2);
    let groups = result["period_groups"].as_array().unwrap();
    assert_eq!(groups.len(),2);
    assert_eq!(groups.iter().find(|g|g["period_id"]==old).unwrap()["percentages"]["debited"]["value"],"2.3");
    assert_eq!(groups.iter().find(|g|g["period_id"]==new).unwrap()["percentages"]["debited"]["value"],"1.15");
    assert!(result.get("total_percentage").is_none());
    let reader = history(&f);
    assert_eq!(reader["period_groups"],result["period_groups"]);
    assert_eq!(reader["items"].as_array().unwrap().iter().find(|c|c["period_id"]==old).unwrap()["allowance_basis"]["version"],"fixture-v1");
}

#[test]
fn q1_small_costs_aggregate_exactly_before_percent_display() {
    let f = fixture();
    let now = crate::multi_user_host::now();
    let p = period(&f,"period",now-1,now+1000,Some(("fixture-large",10_000_000)));
    adjustment(&f,&p,"grant",0,100000);
    for call in ["tiny-a","tiny-b","tiny-c"] { settled(&f,&p,call,4999); }
    let rows = history(&f);
    for item in rows["items"].as_array().unwrap() {
        assert_eq!(item["allowance_percentages"]["debited"]["below_tenth"],true);
        assert_eq!(item["allowance_percentages"]["debited"]["nonzero"],true);
    }
    assert_eq!(rows["period_groups"][0]["debited_micro_cny"],14997);
    assert_eq!(rows["period_groups"][0]["percentages"]["debited"]["value"],"0.14997");
    assert_eq!(account(&f)["current_allowance"]["balance"]["percentages"]["debited"]["value"],"0.14997");
}

#[test]
fn q1_zero_deficit_pending_and_expired_have_distinct_projections() {
    let f = fixture();
    let now = crate::multi_user_host::now();
    assert_eq!(account(&f)["status"],"no_period");
    let p = period(&f,"period",now-10,now+1000,Some(("fixture-v1",1_000_000)));
    assert_eq!(account(&f)["current_allowance"]["balance"]["percentages"]["available"]["value"],"0");
    adjustment(&f,&p,"grant",0,1000);
    settled(&f,&p,"overrun",1200);
    let balance = account(&f)["current_allowance"]["balance"].clone();
    assert_eq!(balance["available_micro_cny"],-200);
    assert_eq!(balance["percentages"]["available"]["value"],"0");
    assert_eq!(balance["percentages"]["deficit"]["value"],"0.02");
    f.control.connection.execute("INSERT INTO model_call_charges(call_id,logical_call_id,attempt,user_id,period_id,task_ref,parent_task_ref,purpose,model,rate_snapshot,reservation_estimate,state,reserved_micro_cny,provider_cost_status,evidence_kind,created_at) VALUES('pending','pending',1,'A',?,'memory','lesson','memory_review','fixture','{}','{}','pending',500,'pending','unknown',12)",[&p]).unwrap();
    let pending = history(&f);
    let row = pending["items"].as_array().unwrap().iter().find(|v|v["call_id"]=="pending").unwrap();
    assert!(row["allowance_percentages"]["debited"].is_null());
    assert_eq!(pending["period_groups"][0]["unknown_provider_cost_count"],1);
    assert_eq!(pending["period_groups"][0]["percentages"]["pending"]["value"],"0.05");
    post(&f,"allowance-validity",json!({"operation_id":"expire","period_id":p,"revision":1,"starts_at":now-10,"expires_at":now-1}));
    let expired = account(&f);
    assert_eq!(expired["status"],"expired");
    assert!(expired["current_allowance"].is_null());
    assert_eq!(expired["latest_allowance"]["validity_state"],"expired");
    assert_eq!(expired["latest_allowance"]["balance"]["available_micro_cny"],-700);
    let earlier = crate::allowance_admin::current(&f.control.connection,"A",now-5).unwrap();
    assert_eq!(earlier["validity_state"],"active");
}

#[test]
fn q1_existing_period_binding_preserves_money_and_rejects_repricing() {
    let f = fixture();
    let now = crate::multi_user_host::now();
    let p = period(&f,"legacy",now-10,now+1000,None);
    adjustment(&f,&p,"grant",0,730000);
    settled(&f,&p,"legacy-call",23000);
    let before = account(&f)["current_allowance"].clone();
    assert_eq!(before["basis_status"],"unbound");
    assert!(before["balance"]["percentages"].is_null());
    assert!(history(&f)["items"][0]["allowance_basis"].is_null());
    let bind = json!({"operation_id":"bind","period_id":p,"revision":1,"basis":{"version":"fixture-v1","basis_micro_cny":1_000_000}});
    let result = post(&f,"allowance-basis",bind.clone());
    assert_eq!(post(&f,"allowance-basis",bind),result);
    let after = result["allowance_period"].clone();
    for key in ["granted_micro_cny","debited_micro_cny","active_reserved_micro_cny","pending_micro_cny","available_micro_cny"] { assert_eq!(after["balance"][key],before["balance"][key]); }
    for key in ["starts_at","expires_at","closed_at"] { assert_eq!(after[key],before[key]); }
    assert_eq!(history(&f)["items"][0]["allowance_percentages"]["debited"]["value"],"2.3");
    let denied = f.call(&f.b,"POST","/api/admin/users/A/allowance-basis",json!({"operation_id":"reprice","period_id":p,"revision":2,"basis":{"version":"fixture-v2","basis_micro_cny":2_000_000}}));
    assert_eq!(denied.0,409);
    assert_eq!(denied.1["error_code"],"ALLOWANCE_BASIS_ALREADY_BOUND");
    let mismatch = f.call(&f.b,"POST","/api/admin/users/A/allowance-percentage-adjustments",json!({"operation_id":"wrong-version","period_id":p,"revision":2,"basis_version":"fixture-v2","delta_milli_percent":50000,"reason":"fixture"}));
    assert_eq!(mismatch.0,409);
    assert_eq!(account(&f)["current_allowance"],after);
    assert_eq!(f.call(&f.a,"POST","/api/admin/users/A/allowance-basis",json!({"operation_id":"not-admin","period_id":p,"revision":2,"basis":{"version":"x","basis_micro_cny":100}})).0,403);
}

#[test]
fn q1_percent_sales_reject_invalid_basis_and_fractional_internal_grants() {
    let f = fixture();
    let now = crate::multi_user_host::now();
    for (op,basis) in [("zero",0),("negative",-1)] {
        assert_eq!(f.call(&f.b,"POST","/api/admin/users/A/allowance-periods",json!({"operation_id":op,"starts_at":now-1,"expires_at":now+1000,"basis":{"version":op,"basis_micro_cny":basis}})).0,400);
    }
    let p = period(&f,"small",now-1,now+1000,Some(("fixture-small",1)));
    let failed = f.call(&f.b,"POST","/api/admin/users/A/percentage-receipts",json!({"operation_id":"inexact","period_id":p,"revision":0,"basis_version":"fixture-small","delta_milli_percent":50000,"amount_fen":100,"paid_at":now,"channel":"fixture","note":"fixture"}));
    assert_eq!(failed.0,400);
    assert_eq!(failed.1["error_code"],"ALLOWANCE_PERCENT_NOT_REPRESENTABLE");
    assert_eq!(account(&f)["current_allowance"]["revision"],0);
    assert_eq!(f.ok(&f.b,"GET","/api/admin/users/A/receipts",json!({}))["total"],0);
    let conflict = f.call(&f.b,"POST","/api/admin/users/A/allowance-periods",json!({"operation_id":"same-version-new-price","starts_at":now+1000,"expires_at":now+2000,"basis":{"version":"fixture-small","basis_micro_cny":2}}));
    assert_eq!(conflict.0,409);
    assert_eq!(conflict.1["error_code"],"ALLOWANCE_BASIS_VERSION_CONFLICT");
    assert_eq!(f.ok(&f.b,"GET","/api/admin/users/A/allowance-periods",json!({}))["total"],1);
}
