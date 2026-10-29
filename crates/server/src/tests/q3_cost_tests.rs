use super::{adm5_tests, mu5_tests::Fixture};
use crate::{control_store::ControlStore, external_build_costs::Admission};
use serde_json::{json, Value};

fn fixture() -> Fixture {
    let mut f = Fixture::new();
    adm5_tests::seed(&mut f,"fixture",10_000);
    f.control.connection.execute("INSERT INTO learning_workspaces(owner_user_id,learning_workspace_id,created_at,updated_at) VALUES('A','build-space',0,0)",[]).unwrap();
    f
}
fn budget(f: &Fixture, task: &str, max: i64) -> Value {
    f.ok(&f.b,"POST",&format!("/api/admin/build-tasks/{task}/budget"),json!({"cost_owner":"account","user_id":"A","learning_workspace_id":"build-space","max_micro_cny":max,"confirmation":format!("confirm-{task}")}))
}
fn admission(call: &str, execution: &str, reserve: i64) -> Value {
    json!({"execution_id":execution,"call_id":call,"logical_call_id":call,"attempt":1,"granularity":"calls","reserved_micro_cny":reserve,"provider":"fixture","model":"fixture-model","purpose":"external_build","budget_enforced":true,"receipt_capability":"complete"})
}
fn admit(f: &Fixture, task: &str, call: &str, reserve: i64) -> Value {
    f.ok(&f.b,"POST",&format!("/api/admin/build-tasks/{task}/admit"),admission(call,"execution",reserve))
}
fn receipt(call: &str, status: &str, cost: Option<i64>, succeeded: bool) -> Value {
    json!({"call_id":call,"status":status,"provider_cost_micro_cny":cost,"usage":{"tokens":42},"evidence":{"provider_receipt":format!("receipt-{call}-{status}")},"succeeded":succeeded})
}
fn settle(f: &Fixture, task: &str, call: &str, cost: i64, succeeded: bool) -> Value {
    f.ok(&f.b,"POST",&format!("/api/admin/build-tasks/{task}/receipts"),receipt(call,"confirmed",Some(cost),succeeded))
}
fn status(f: &Fixture, task: &str) -> Value {
    f.ok(&f.b,"GET",&format!("/api/admin/build-tasks/{task}"),json!({}))
}

#[test]
fn unlimited_allowance_allows_build_usage_beyond_balance_with_task_budget_intact() {
    let f = fixture();
    f.control.connection.execute("UPDATE allowance_periods SET unlimited=1 WHERE user_id='A'", []).unwrap();
    budget(&f, "unlimited-build", 30_000_000);
    admit(&f, "unlimited-build", "unlimited-build-first", 12_000_000);
    let done = settle(&f, "unlimited-build", "unlimited-build-first", 12_000_000, true);
    assert_eq!(done["charge"]["provider_cost_micro_cny"], 12_000_000);
    admit(&f, "unlimited-build", "unlimited-build-next", 18_000_000);
    assert_eq!(f.call(&f.b, "POST", "/api/admin/build-tasks/unlimited-build/admit", admission("over-task-budget", "execution", 1)).1["error_code"], "BUILD_BUDGET_EXHAUSTED");
}

#[test]
fn q3_parallel_admission_and_late_costs_never_exceed_confirmed_account_ceiling() {
    let f = fixture();
    budget(&f,"parallel",100);
    let handles: Vec<_> = ["first","second"].into_iter().map(|call| {
        let spend = f.access.spend.clone();
        let input: Admission = serde_json::from_value(admission(call,"execution",60)).unwrap();
        std::thread::spawn(move || (call,spend.build_admit("parallel",input,crate::multi_user_host::now())))
    }).collect();
    let results: Vec<_> = handles.into_iter().map(|h|h.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|(_,r)|r.is_ok()).count(),1,"parallel children share one remaining ceiling");
    let winner = results.iter().find(|(_,r)|r.is_ok()).unwrap().0;
    let loser = results.iter().find(|(_,r)|r.is_err()).unwrap().0;
    assert_eq!(results.iter().find(|(_,r)|r.is_err()).unwrap().1.as_ref().unwrap_err().error_code,"BUILD_BUDGET_EXHAUSTED");
    settle(&f,"parallel",winner,40,true);
    admit(&f,"parallel",loser,60);
    let pending = f.ok(&f.b,"POST","/api/admin/build-tasks/parallel/receipts",receipt(loser,"partial",Some(20),false));
    assert_eq!(pending["charge"]["provider_cost_status"],"pending");
    assert!(pending["charge"]["provider_cost_micro_cny"].is_null());
    assert_eq!(pending["charge"]["send_outcome"]["provider_cost_micro_cny"],20,"partial amount remains evidence, never an exact debit");
    assert_eq!(pending["budget"]["remaining_micro_cny"],0);
    let late = settle(&f,"parallel",loser,90,false);
    assert_eq!(late["charge"]["account_debit_micro_cny"],60);
    assert_eq!(late["charge"]["provider_cost_micro_cny"],90);
    assert_eq!(late["charge"]["platform_covered_micro_cny"],30);
    assert_eq!(late["budget"]["debited_micro_cny"],100);
    assert_eq!(late["budget"]["provider_cost_micro_cny"],130);
    assert_eq!(late["budget"]["platform_covered_micro_cny"],30);
    assert_eq!(settle(&f,"parallel",loser,90,false),late,"the identical late receipt is idempotent");
    // An existing supported manual correction also obeys this parent ceiling.
    f.ok(&f.b,"POST",&format!("/api/admin/charges/{loser}/reconcile"),json!({"operation_id":"correct-external-cost","revision":late["charge"]["revision"],"provider_cost_micro_cny":120,"waive_account":false,"reason":"provider correction","evidence":"provider invoice"}));
    let corrected = status(&f,"parallel");
    assert_eq!(corrected["debited_micro_cny"],100);
    assert_eq!(corrected["provider_cost_micro_cny"],160);
    assert_eq!(corrected["platform_covered_micro_cny"],60);
}

#[test]
fn q3_concurrent_completed_transports_keep_full_cost_and_cap_account_sum() {
    let f = fixture();
    budget(&f,"both",100);
    admit(&f,"both","one",50);
    admit(&f,"both","two",50);
    let handles: Vec<_> = [("one",80),("two",90)].into_iter().map(|(call,cost)| {
        let spend = f.access.spend.clone();
        let input = serde_json::from_value(receipt(call,"confirmed",Some(cost),true)).unwrap();
        std::thread::spawn(move ||spend.build_receipt("both",input,crate::multi_user_host::now()).unwrap())
    }).collect();
    for handle in handles { handle.join().unwrap(); }
    let actual = status(&f,"both");
    assert_eq!(actual["debited_micro_cny"],100);
    assert_eq!(actual["provider_cost_micro_cny"],170);
    assert_eq!(actual["platform_covered_micro_cny"],70);
    assert_eq!(actual["reserved_micro_cny"],0);
}

#[test]
fn q3_failure_cancel_restart_retry_and_explicit_budget_increase_keep_prior_facts() {
    let f = fixture();
    let original = budget(&f,"resume",100);
    assert_eq!(budget(&f,"resume",100),original);
    admit(&f,"resume","complete-unit",20);
    settle(&f,"resume","complete-unit",20,true);
    let mut failed = admission("failed-send","execution",20);
    failed["logical_call_id"] = json!("retryable-unit");
    f.ok(&f.b,"POST","/api/admin/build-tasks/resume/admit",failed);
    settle(&f,"resume","failed-send",10,false);
    let mut retry = admission("retry-send","execution",20);
    retry["logical_call_id"] = json!("retryable-unit");
    retry["attempt"] = json!(2);
    f.ok(&f.b,"POST","/api/admin/build-tasks/resume/admit",retry);
    settle(&f,"resume","retry-send",15,true);
    admit(&f,"resume","cancel-before-send",20);
    f.ok(&f.b,"POST","/api/admin/build-tasks/resume/receipts",receipt("cancel-before-send","not_sent",None,false));
    admit(&f,"resume","interrupted",30);
    f.access.spend.recover().unwrap();
    assert_eq!(status(&f,"resume")["pending_micro_cny"],30);
    let (code,error) = f.call(&f.b,"POST","/api/admin/build-tasks/resume/admit",admission("complete-unit","execution",20));
    assert_eq!(code,409);
    assert_eq!(error["error_code"],"BUILD_SEND_ALREADY_ADMITTED","resume cannot pay for an already admitted transport twice");
    settle(&f,"resume","interrupted",25,false);
    assert_eq!(status(&f,"resume")["debited_micro_cny"],70);
    let increase = json!({"cost_owner":"account","user_id":"A","learning_workspace_id":"build-space","max_micro_cny":150,"confirmation":"resume-confirmed-increase","expected_revision":0});
    let changed = f.ok(&f.b,"POST","/api/admin/build-tasks/resume/budget",increase.clone());
    assert_eq!(changed["revision"],1);
    assert_eq!(changed["remaining_micro_cny"],80);
    assert_eq!(f.ok(&f.b,"POST","/api/admin/build-tasks/resume/budget",increase),changed);
    let mut stale = json!({"cost_owner":"account","user_id":"A","learning_workspace_id":"build-space","max_micro_cny":200,"confirmation":"stale","expected_revision":0});
    assert_eq!(f.call(&f.b,"POST","/api/admin/build-tasks/resume/budget",stale.clone()).0,409);
    stale["expected_revision"] = json!(1);
    stale["max_micro_cny"] = json!(50);
    assert_eq!(f.call(&f.b,"POST","/api/admin/build-tasks/resume/budget",stale).0,409);
    assert_eq!(f.control.connection.query_row("SELECT count(*) FROM build_budget_confirmations WHERE task_ref='resume'",[],|r|r.get::<_,i64>(0)).unwrap(),2);
    let (code,_) = f.call(&f.a,"GET","/api/admin/build-tasks/resume",json!({}));
    assert_eq!(code,403,"reader credentials cannot impersonate an executor");
}

#[test]
fn q3_execution_total_and_details_have_one_ledger_level_and_platform_scope() {
    let f = fixture();
    let before = f.ok(&f.a,"GET","/api/account/allowance",json!({}));
    f.ok(&f.b,"POST","/api/admin/build-tasks/public/budget",json!({"cost_owner":"platform","max_micro_cny":100,"confirmation":"operator approved preparation"}));
    let mut total = admission("aggregate","external-run",80);
    total["granularity"] = json!("execution_total");
    total["budget_enforced"] = json!(false);
    total["receipt_capability"] = json!("partial");
    f.ok(&f.b,"POST","/api/admin/build-tasks/public/admit",total);
    let detail = admission("internal-model-send","external-run",10);
    let (code,error) = f.call(&f.b,"POST","/api/admin/build-tasks/public/admit",detail);
    assert_eq!(code,409);
    assert_eq!(error["error_code"],"BUILD_SETTLEMENT_LEVEL_CONFLICT");
    let mut report = receipt("aggregate","confirmed",Some(70),true);
    report["usage"] = json!({"calls":[{"cost_micro_cny":30},{"cost_micro_cny":40}],"billing_basis":"provider execution receipt"});
    let settled = f.ok(&f.b,"POST","/api/admin/build-tasks/public/receipts",report.clone());
    assert_eq!(settled["budget"]["provider_cost_micro_cny"],70);
    assert_eq!(settled["budget"]["platform_covered_micro_cny"],70);
    assert_eq!(settled["budget"]["debited_micro_cny"],0);
    assert_eq!(f.ok(&f.b,"POST","/api/admin/build-tasks/public/receipts",report.clone()),settled);
    report["provider_cost_micro_cny"] = json!(71);
    assert_eq!(f.call(&f.b,"POST","/api/admin/build-tasks/public/receipts",report).0,409);
    assert_eq!(status(&f,"public")["provider_cost_micro_cny"],70);
    let detail = f.ok(&f.b,"GET","/api/admin/charges/aggregate",json!({}));
    assert_eq!(detail["reports"]["total"],2);
    assert_eq!(detail["needs_reconciliation"],true);
    assert_eq!(f.ok(&f.a,"GET","/api/account/allowance",json!({})),before);
    assert_eq!(f.ok(&f.a,"GET","/api/account/usage",json!({}))["total"],0);
}

#[test]
fn q3_uncontrolled_personal_execution_is_rejected_and_late_usage_keeps_original_period() {
    let f = fixture();
    budget(&f,"personal",100);
    for (enforced,capability) in [(false,"complete"),(true,"partial"),(true,"none")] {
        let mut request = admission("never-sent","execution",20);
        request["budget_enforced"] = json!(enforced);
        request["receipt_capability"] = json!(capability);
        let (code,error) = f.call(&f.b,"POST","/api/admin/build-tasks/personal/admit",request);
        assert_eq!(code,409);
        assert_eq!(error["error_code"],"BUILD_EXECUTOR_UNMETERED");
    }
    assert_eq!(status(&f,"personal")["reserved_micro_cny"],0);
    admit(&f,"personal","late-period",20);
    let old = f.ok(&f.b,"GET","/api/admin/build-tasks/personal/charges/late-period",json!({}))["charge"]["period_id"].clone();
    f.control.connection.execute("UPDATE allowance_periods SET closed_at=? WHERE user_id='A'",[crate::multi_user_host::now()]).unwrap();
    f.control.connection.execute("UPDATE learning_workspaces SET archived=1 WHERE learning_workspace_id='build-space'",[]).unwrap();
    let late = settle(&f,"personal","late-period",30,true);
    assert_eq!(late["charge"]["period_id"],old);
    assert_eq!(late["charge"]["account_debit_micro_cny"],30);
    let (code,error) = f.call(&f.b,"POST","/api/admin/build-tasks/personal/admit",admission("after-archive","execution",20));
    assert_eq!(code,409);
    assert_eq!(error["error_code"],"BUILD_WORKSPACE_UNAVAILABLE");
}

#[test]
fn q3_late_provider_receipt_fills_cost_without_undoing_explicit_account_waiver() {
    let f = fixture();
    budget(&f,"waived",100);
    admit(&f,"waived","waived-call",80);
    let pending = f.ok(&f.b,"POST","/api/admin/build-tasks/waived/receipts",receipt("waived-call","unknown",None,false));
    f.ok(&f.b,"POST","/api/admin/charges/waived-call/reconcile",json!({"operation_id":"waive-pending-external","revision":pending["charge"]["revision"],"waive_account":true,"reason":"operator accepts this cost","evidence":"support correction"}));
    let late = settle(&f,"waived","waived-call",120,false);
    assert_eq!(late["charge"]["state"],"released");
    assert_eq!(late["charge"]["account_debit_micro_cny"],0);
    assert_eq!(late["charge"]["provider_cost_micro_cny"],120);
    assert_eq!(late["budget"]["platform_covered_micro_cny"],120);
    assert_eq!(settle(&f,"waived","waived-call",120,false),late);
}

#[test]
fn q3_budget_percentages_use_current_planning_basis_without_repricing_history() {
    let f = fixture();
    let plan = budget(&f,"plan",100);
    assert!(plan["planning_allowance"]["percentages"].is_null(),"an unbound legacy period is not guessed");
    let old = plan["planning_allowance"]["period_id"].as_str().unwrap();
    f.ok(&f.b,"POST","/api/admin/users/A/allowance-basis",json!({"operation_id":"plan-basis","period_id":old,"revision":1,"basis":{"version":"plan-v1","basis_micro_cny":1000}}));
    let plan = status(&f,"plan");
    assert_eq!(plan["planning_allowance"]["percentages"]["maximum"]["value"],"10");
    admit(&f,"plan","plan-first",30);
    settle(&f,"plan","plan-first",30,true);
    assert_eq!(status(&f,"plan")["planning_allowance"]["percentages"]["remaining"]["value"],"7");
    let now = crate::multi_user_host::now();
    f.control.connection.execute("UPDATE allowance_periods SET closed_at=? WHERE period_id=?",rusqlite::params![now,old]).unwrap();
    assert!(status(&f,"plan")["planning_allowance"].is_null());
    let new = f.ok(&f.b,"POST","/api/admin/users/A/allowance-periods",json!({"operation_id":"plan-next-period","starts_at":now,"expires_at":now+1000,"basis":{"version":"plan-v2","basis_micro_cny":2000}}));
    let plan = status(&f,"plan");
    assert_eq!(plan["planning_allowance"]["period_id"],new["allowance_period"]["period_id"]);
    assert_eq!(plan["planning_allowance"]["percentages"]["maximum"]["value"],"5");
    assert_eq!(plan["planning_allowance"]["percentages"]["remaining"]["value"],"3.5");
    let original = f.ok(&f.b,"GET","/api/admin/build-tasks/plan/charges/plan-first",json!({}));
    assert_eq!(original["charge"]["allowance_percentages"]["debited"]["value"],"3");
    assert!(plan["planning_allowance"]["percentages"].get("debited").is_none());
    let operator = f.ok(&f.b,"POST","/api/admin/build-tasks/operator-plan/budget",json!({"cost_owner":"platform","max_micro_cny":100,"confirmation":"operator only"}));
    assert!(operator["planning_allowance"].is_null());
}

#[test]
fn q3_schema15_upgrade_preserves_existing_ledger_and_rolls_back_failed_addition() {
    let f = fixture();
    let before = f.ok(&f.a,"GET","/api/account/allowance",json!({}));
    // No external tasks exist yet. Recreate the supported v15 boundary exactly.
    f.control.connection.execute_batch(include_str!("remove_schema19.sql")).unwrap();
    f.control.connection.execute_batch("DROP TABLE workspace_release_instance_private_materials; DROP TABLE workspace_release_instance_operations; DROP TABLE workspace_release_instance_history; DROP TABLE workspace_release_instances; DROP TABLE workspace_release_archives; DROP TABLE operator_builds; DROP TABLE workspace_releases; DROP TABLE external_build_calls; DROP TABLE external_build_executions; DROP TABLE build_budget_confirmations; DROP TABLE build_task_budgets; DROP TABLE allowance_offers; PRAGMA user_version=15;").unwrap();
    f.control.connection.execute_batch("CREATE TABLE external_build_calls(keep TEXT); INSERT INTO external_build_calls VALUES('original');").unwrap();
    assert!(ControlStore::open(f.control.writer.clone()).is_err());
    assert_eq!(f.control.connection.pragma_query_value(None,"user_version",|r|r.get::<_,i64>(0)).unwrap(),15);
    assert!(f.control.connection.prepare("SELECT * FROM build_task_budgets").is_err());
    assert_eq!(f.control.connection.query_row("SELECT keep FROM external_build_calls",[],|r|r.get::<_,String>(0)).unwrap(),"original");
    f.control.connection.execute_batch("DROP TABLE external_build_calls").unwrap();
    for _ in 0..2 {
        let reopened = ControlStore::open(f.control.writer.clone()).unwrap();
        assert_eq!(reopened.connection.pragma_query_value(None,"user_version",|r|r.get::<_,i64>(0)).unwrap(),crate::control_store::CONTROL_SCHEMA_VERSION);
        assert_eq!(reopened.connection.query_row("SELECT count(*) FROM build_task_budgets",[],|r|r.get::<_,i64>(0)).unwrap(),0);
    }
    assert_eq!(f.ok(&f.a,"GET","/api/account/allowance",json!({})),before);
}
