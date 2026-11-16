use super::mu5_tests::Fixture;
use serde_json::{json, Value};

fn fixture() -> (Fixture,String) {
    let mut f = Fixture::new();
    f.control.set_reader_admin("B",true).unwrap();
    let now = crate::multi_user_host::now();
    let p = f.ok(&f.b,"POST","/api/admin/users/A/allowance-periods",json!({"operation_id":"q4-period","starts_at":now-100,"expires_at":now+3600,"basis":{"version":"test-basis","basis_micro_cny":1_000_000}}))["allowance_period"]["period_id"].as_str().unwrap().to_owned();
    (f,p)
}
fn offer(id:&str,enabled:bool) -> Value {
    json!({"operation_id":format!("offer-{id}"),"offer_id":id,"title":"补充50%（验收配置）","amount_fen":123,"delta_milli_percent":50000,"basis_version":"test-basis","instructions":"联系测试运营，确认付款后登记到账","enabled":enabled})
}
fn account(f:&Fixture)->Value { f.ok(&f.a,"GET","/api/account/allowance",Value::Null) }
fn post(f:&Fixture,action:&str,body:Value)->Value { f.ok(&f.b,"POST",&format!("/api/admin/users/A/{action}"),body) }

#[test]
fn budget_admin_addons_preserve_exact_budget_across_period_bases() {
    let (f,p) = fixture();
    let body = json!({"operation_id":"money-offer","offer_id":"budget-addon","title":"加购","amount_fen":500,"budget_micro_cny":3_333_333,"instructions":"联系运营","enabled":true});
    f.ok(&f.b, "POST", "/api/admin/allowance-offers", body.clone());
    f.ok(&f.b, "POST", "/api/admin/allowance-offers", body);
    let offer = account(&f)["replenishment"]["offers"][0].clone();
    assert_eq!(offer["budget_micro_cny"], 3_333_333);
    assert_eq!(account(&f)["current_allowance"]["balance"]["available_micro_cny"], 0);
    post(&f, "receipts", json!({"operation_id":"budget-addon-paid","period_id":p,"revision":0,"amount_fen":500,"delta_micro_cny":offer["budget_micro_cny"],"paid_at":crate::multi_user_host::now(),"channel":"test","note":""}));
    assert_eq!(account(&f)["current_allowance"]["balance"]["available_micro_cny"], 3_333_333);
}

#[test]
fn q4_offers_require_explicit_basis_and_do_not_grant_before_manual_receipt() {
    let (f,p) = fixture();
    let original = account(&f)["current_allowance"].clone();
    assert_eq!(account(&f)["replenishment"]["offers"],json!([]));
    assert_eq!(f.call(&f.a,"POST","/api/admin/allowance-offers",offer("public",true)).0,403);
    let saved = f.ok(&f.b,"POST","/api/admin/allowance-offers",offer("public",true));
    assert_eq!(f.ok(&f.b,"POST","/api/admin/allowance-offers",offer("public",true)),saved);
    f.ok(&f.b,"POST","/api/admin/allowance-offers",offer("hidden",false));
    assert_eq!(account(&f)["replenishment"]["offers"].as_array().unwrap().len(),1);
    assert_eq!(account(&f)["replenishment"]["offers"][0]["amount_fen"],123);
    assert_eq!(account(&f)["current_allowance"],original,"reading a purchase option does not grant allowance");
    let mut invalid = offer("unknown",true); invalid["basis_version"] = json!("not-configured");
    assert_eq!(f.call(&f.b,"POST","/api/admin/allowance-offers",invalid).1["error_code"],"ALLOWANCE_BASIS_UNBOUND");
    let result = post(&f,"percentage-receipts",json!({"operation_id":"q4-paid","period_id":p,"revision":0,"basis_version":"test-basis","delta_milli_percent":50000,"amount_fen":123,"paid_at":crate::multi_user_host::now(),"channel":"fixture","external_ref":"q4-payment","note":"test price"}));
    assert_eq!(result["allowance_period"]["balance"]["percentages"]["available"]["value"],"50");
    let now = crate::multi_user_host::now();
    post(&f,"allowance-validity",json!({"operation_id":"q4-expire","period_id":p,"revision":1,"starts_at":now-100,"expires_at":now-1}));
    assert_eq!(account(&f)["status"],"expired");
    assert_eq!(account(&f)["replenishment"]["offers"],json!([]));
}

#[test]
fn q4_percentage_receipt_corrections_replay_and_preserve_original_cash_and_grant() {
    let (f,p) = fixture();
    let paid = json!({"operation_id":"q4-paid","period_id":p,"revision":0,"basis_version":"test-basis","delta_milli_percent":50000,"amount_fen":123,"paid_at":crate::multi_user_host::now(),"channel":"fixture","note":"test price"});
    let receipt = post(&f,"percentage-receipts",paid.clone());
    let correction = json!({"operation_id":"q4-correct","receipt_id":receipt["receipt_id"],"period_id":p,"revision":1,"basis_version":"test-basis","delta_milli_percent":-10000,"delta_fen":-23,"reason":"actual paid amount corrected"});
    let corrected = post(&f,"percentage-receipt-corrections",correction.clone());
    assert_eq!(post(&f,"percentage-receipt-corrections",correction),corrected);
    assert_eq!(post(&f,"percentage-receipts",paid),receipt);
    assert_eq!(account(&f)["current_allowance"]["balance"]["percentages"]["available"]["value"],"40");
    let receipts = f.ok(&f.b,"GET","/api/admin/users/A/receipts",Value::Null);
    assert_eq!(receipts["items"][0]["amount_fen"],123);
    assert_eq!(receipts["items"][0]["effective_amount_fen"],100);
    assert_eq!(receipts["items"][0]["grant_percentage"]["value"],"50");
    let corrections = f.ok(&f.b,"GET","/api/admin/users/A/receipt-corrections",Value::Null);
    assert_eq!(corrections["total"],1);
    assert_eq!(corrections["items"][0]["delta_fen"],-23);
    assert_eq!(corrections["items"][0]["delta_percentage"]["value"],"-10");
    assert_eq!(corrections["items"][0]["allowance_basis"]["version"],"test-basis");
    let invalid = json!({"operation_id":"q4-over-correct","receipt_id":receipt["receipt_id"],"period_id":p,"revision":2,"basis_version":"test-basis","delta_milli_percent":-10000,"delta_fen":-101,"reason":"invalid cash"});
    assert_eq!(f.call(&f.b,"POST","/api/admin/users/A/percentage-receipt-corrections",invalid).1["error_code"],"RECEIPT_CORRECTION_EXCEEDS_AMOUNT");
    assert_eq!(account(&f)["current_allowance"]["revision"],2);
    assert_eq!(account(&f)["current_allowance"]["balance"]["percentages"]["available"]["value"],"40");
}

#[test]
#[ignore = "real HTTP host for percentage-allowance.spec.ts and Q3 client integration"]
fn q4_browser_host() {
    let (f,p) = fixture();
    post(&f,"allowance-percentage-adjustments",json!({"operation_id":"q4-initial","period_id":p,"revision":0,"basis_version":"test-basis","delta_milli_percent":73000,"reason":"acceptance fixture"}));
    f.control.connection.execute("INSERT INTO model_call_charges(call_id,logical_call_id,attempt,user_id,period_id,run_ref,parent_task_ref,purpose,model,rate_snapshot,reservation_estimate,state,reserved_micro_cny,provider_cost_micro_cny,provider_cost_status,account_debit_micro_cny,evidence_kind,created_at,settled_at) VALUES('q4-answer','q4-answer',1,'A',?,'run','lesson','answer','fixture','{}','{}','settled',0,23000,'confirmed',23000,'fixture',10,11)",[&p]).unwrap();
    // More than a page, with tiny nonzero rows that still sum exactly.
    for index in 0..21 {
        let call = format!("q4-tiny-{index}");
        f.control.connection.execute("INSERT INTO model_call_charges(call_id,logical_call_id,attempt,user_id,period_id,task_ref,purpose,model,rate_snapshot,reservation_estimate,state,reserved_micro_cny,provider_cost_micro_cny,provider_cost_status,account_debit_micro_cny,evidence_kind,created_at,settled_at) VALUES(?1,?1,1,'A',?2,'memory','memory_review','fixture','{}','{}','settled',0,1,'confirmed',1,'fixture',12,13)",rusqlite::params![call,p]).unwrap();
    }
    let workspace = f.ok(&f.a,"POST","/api/learning-workspaces",json!({"goal":{"text":"准备考试资料"},"members":[f.x]}));
    let server = crate::multi_user_host::start_with_access("127.0.0.1:4186".parse().unwrap(),std::sync::Arc::new(crate::multi_user_host::Site::new("https://reader.example").unwrap()),f.access.clone()).unwrap();
    let control = tiny_http::Server::http("127.0.0.1:4187").unwrap();
    println!("Q4_BROWSER_READY");
    let deadline = std::time::Instant::now()+std::time::Duration::from_secs(900);
    while std::time::Instant::now()<deadline {
        let Some(request) = control.recv_timeout(std::time::Duration::from_secs(1)).unwrap() else { continue; };
        if request.url()=="/stop" { request.respond(tiny_http::Response::from_string("stopped")).unwrap(); break; }
        let value = json!({"period_id":p,"reader_cookie":format!("{}={}",crate::auth::COOKIE,f.a),"admin_cookie":format!("{}={}",crate::auth::COOKIE,f.b),"reader_csrf":crate::auth::csrf(&f.a),"admin_csrf":crate::auth::csrf(&f.b),"learning_workspace_id":workspace["learning_workspace_id"]});
        request.respond(tiny_http::Response::from_string(value.to_string()).with_header(tiny_http::Header::from_bytes("Content-Type","application/json").unwrap())).unwrap();
    }
    server.shutdown();
}
