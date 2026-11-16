use super::mu5_tests::Fixture;
use serde_json::json;

#[test]
fn operations_health_uses_billing_endpoint_identity_not_adapter_mode() {
    let mut f=Fixture::new();
    super::adm5_tests::seed(&mut f,"https://provider.example/v1",1000);
    for mode in ["native","react"] {
        f.access.runs.configure(runtime::ProviderConfig::from_values(mode,"fixture-key","https://provider.example/v1/","fixture-model").unwrap()).unwrap();
        let report=f.ok(&f.b,"GET","/api/admin/operations",json!({}));
        assert_eq!(report["model"]["can_answer"],true,"{mode}: {report}");
        assert_eq!(report["model"]["code"],"MODEL_READY");
    }
}

#[test]
fn operations_api_is_admin_only_and_records_rejected_requests() {
    let mut f=Fixture::new(); f.control.set_reader_admin("B",true).unwrap();
    assert_eq!(f.call("","GET","/api/admin/operations",json!({})).0,401);
    assert_eq!(f.call(&f.a,"GET","/api/admin/operations",json!({})).0,403);
    let w=f.create(&f.a,&f.x,"ops");
    let path=format!("/api/workspaces/{}/agent/runs",w["workspace_id"].as_str().unwrap());
    assert_ne!(f.call(&f.a,"POST",&path,json!({"client_request_id":"ops-invalid-request"})).0,202);
    let report=f.ok(&f.b,"GET","/api/admin/operations",json!({}));
    assert!(!report["model"]["can_answer"].as_bool().unwrap());
    let rows=report["recent_outcomes"].as_array().unwrap();
    assert_eq!(rows.len(),1); assert_eq!(rows[0]["request_id"],"ops-invalid-request");
    assert_eq!(rows[0]["is_probe"],false);
    assert!(rows[0].get("message").is_none());
    assert!(rows[0].get("answer").is_none());
}

#[test]
fn operations_migration_from_22_retains_accounts_and_reopens() {
    let f=Fixture::new();
    f.control.connection.execute_batch("DROP TABLE operational_successes; DROP TABLE operational_outcomes; PRAGMA user_version=22;").unwrap();
    let store=crate::control_store::ControlStore::open(f.control.writer.clone()).unwrap();
    assert_eq!(store.connection.pragma_query_value(None,"user_version",|r|r.get::<_,i64>(0)).unwrap(),24);
    assert_eq!(store.connection.query_row("SELECT count(*) FROM users WHERE user_id IN ('A','B')",[],|r|r.get::<_,i64>(0)).unwrap(),2);
    drop(store);
    crate::control_store::ControlStore::open(f.control.writer.clone()).unwrap();
}

#[test]
fn operations_records_cancelled_and_provider_failure_turns() {
    let mut f=Fixture::new(); f.control.set_reader_admin("B",true).unwrap();
    f.access.runs.configure_fake(|| Box::new(crate::UnconfiguredAdapter));
    for cancel in [true,false] {
        let w=f.create(&f.a,&f.x,if cancel {"cancel"} else {"failure"});
        let attachment=if cancel {"cancel"} else {"failure"};
        let w=f.action(&f.a,&w,attachment,"chat/new",json!({}));
        let mut input=Fixture::stamp(&w,attachment);
        input["session_id"]=w["selected_chat"].clone();
        input["client_request_id"]=json!(attachment);
        input["message"]=json!("hello");
        let path=format!("/api/workspaces/{}/agent/runs",w["workspace_id"].as_str().unwrap());
        let (status,admitted)=f.call(&f.a,"POST",&path,input); assert_eq!(status,202,"{admitted}");
        if cancel {
            f.ok(&f.a,"POST",&format!("/api/agent/runs/{}/cancel",admitted["turn_id"].as_str().unwrap()),json!({}));
        } else {
            assert!(f.access.runs.run_one(&f.access).unwrap());
        }
        let report=f.ok(&f.b,"GET","/api/admin/operations",json!({}));
        assert!(report["recent_outcomes"].as_array().unwrap().iter().any(|r|r["request_id"]==attachment && r["outcome"]==if cancel {"cancelled"} else {"partial"}),"{report}");
        if !cancel { assert_eq!(report["signals"]["failures"][0]["error_code"],"PROVIDER_ERROR"); }
    }
}
