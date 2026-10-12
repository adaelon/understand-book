use super::mu5_tests::Fixture;
use serde_json::json;

#[test]
fn adm7_user_filter_pagination_balance_and_receipt_period_are_server_facts() {
    let mut f = Fixture::new();
    let period = super::adm5_tests::seed(&mut f, "fixture", 1_000_001);
    let result = f.ok(&f.b, "GET", "/api/admin/users?search=a&disabled=false&limit=1&offset=0", json!({}));
    assert_eq!(result["total"], 1);
    assert_eq!(result["users"][0]["user_id"], "A");
    assert_eq!(result["users"][0]["current_allowance"]["balance"]["available_micro_cny"], 1_000_001);
    assert_eq!(f.ok(&f.b,"GET","/api/admin/users?search=a&limit=1&offset=1",json!({}))["users"],json!([]));
    assert_eq!(f.ok(&f.b,"GET","/api/admin/users?search=%25",json!({}))["total"],0); // literal substring, not SQL wildcard
    assert_eq!(f.call(&f.a,"GET","/api/admin/users?search=A",json!({})).0,403);
    assert_eq!(f.call(&f.b,"GET","/api/admin/users?disabled=invalid",json!({})).0,400);
    f.ok(&f.b,"POST","/api/admin/users/A/receipts",json!({"operation_id":"adm7-receipt","period_id":period,"revision":1,"amount_fen":123,"delta_micro_cny":400000,"paid_at":100,"channel":"wechat","note":"fixture"}));
    assert_eq!(f.ok(&f.b,"GET","/api/admin/users/A/receipts",json!({}))["items"][0]["period_id"],period);
    f.ok(&f.b,"POST","/api/admin/users/A/status",json!({"operation_id":"adm7-disable","disabled":true}));
    assert_eq!(f.ok(&f.b,"GET","/api/admin/users?disabled=true",json!({}))["users"][0]["user_id"],"A");
    assert_eq!(f.ok(&f.b,"GET","/api/admin/users?disabled=false",json!({}))["total"],1);
    let operations = f.ok(&f.b,"GET","/api/admin/users/A/operations?limit=1&offset=1",json!({}));
    assert_eq!(operations["total"],4);
    assert_eq!(operations["items"].as_array().unwrap().len(),1);
    assert!(operations["items"][0].get("parameters").is_none());
    assert_eq!(f.call(&f.b,"GET","/api/admin/users/missing/operations",json!({})).0,404);
}

/// An opt-in real Rust/SQLite candidate for the HTTPS browser acceptance script.
#[test]
#[ignore = "interactive candidate host; set ADM7_CANDIDATE_INFO and run explicitly"]
fn adm7_candidate_host() {
    use runtime::model_spend::*;
    let info = std::path::PathBuf::from(std::env::var("ADM7_CANDIDATE_INFO").expect("candidate info path"));
    let mut f = Fixture::new();
    super::adm5_tests::seed(&mut f,"fixture",10_000_000);
    let scope = ChargeScope::ReaderRun { user_id:"A".into(),run_ref:"adm7-fixture".into() };
    let port = f.access.spend.port(scope.clone(),f.x.clone(),Default::default());
    let id = SendIdentity { call_id:"adm7-pending".into(),logical_call_id:"adm7-pending".into(),attempt:1,scope:Some(scope),purpose:"outer".into(),model:"fixture-model".into(),provider:"fixture".into() };
    port.before_send(&id,&json!({"model":"fixture-model","messages":[{"role":"user","content":"fixture"}],"max_tokens":100})).unwrap();
    port.after_send(&id,&SendOutcome { evidence:SendEvidence::OutcomeUnknown,usage:None,succeeded:false }).unwrap();
    let server = crate::multi_user_host::start_with_access("127.0.0.1:18788".parse().unwrap(),std::sync::Arc::new(crate::multi_user_host::Site::new("https://localhost:18443").unwrap()),f.access.clone()).unwrap();
    std::fs::write(&info,json!({"root":f.root.path(),"url":server.url,"book":f.x}).to_string()).unwrap();
    let stop = info.with_extension("stop");
    while !stop.exists() { std::thread::sleep(std::time::Duration::from_millis(200)); }
    server.shutdown();
}

/// ADM8 browser fixture: real admission/SQLite and a local HTTP Provider only.
#[test]
#[ignore = "interactive candidate host; set ADM8_CANDIDATE_INFO and run explicitly"]
fn adm8_candidate_host() {
    use runtime::{model_spend::*, ProviderConfig};
    use std::sync::{atomic::{AtomicBool, AtomicUsize, Ordering}, Arc};
    let info = std::path::PathBuf::from(std::env::var("ADM8_CANDIDATE_INFO").expect("candidate info path"));
    let provider = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let url = format!("http://{}", provider.server_addr().to_ip().unwrap());
    let mut f = Fixture::new();
    let period = super::adm5_tests::seed(&mut f, &url, 10_000_000);
    f.access.runs.configure(ProviderConfig::from_values("native", "fixture-key", url.clone(), "fixture-model").unwrap()).unwrap();
    let scope = ChargeScope::ReaderRun { user_id: "A".into(), run_ref: "adm8-fixture".into() };
    let port = f.access.spend.port(scope.clone(), f.x.clone(), Default::default());
    for index in 0..21 {
        let id = SendIdentity { call_id: format!("adm8-pending-{index:02}"), logical_call_id: format!("adm8-pending-{index:02}"), attempt: 1, scope: Some(scope.clone()), purpose: "outer".into(), model: "fixture-model".into(), provider: url.clone() };
        port.before_send(&id, &json!({"model":"fixture-model","messages":[{"role":"user","content":"fixture"}],"max_tokens":100})).unwrap();
        port.after_send(&id, &SendOutcome { evidence: SendEvidence::OutcomeUnknown, usage: None, succeeded: false }).unwrap();
    }
    let available = f.ok(&f.a, "GET", "/api/account/allowance", json!({}))["current_allowance"]["balance"]["available_micro_cny"].as_i64().unwrap();
    f.ok(&f.b, "POST", "/api/admin/users/A/allowance-adjustments", json!({"operation_id":"adm8-zero", "period_id":period,"revision":1,"delta_micro_cny":-available,"reason":"zero allowance browser fixture"}));
    let done = Arc::new(AtomicBool::new(false));
    let requests = Arc::new(AtomicUsize::new(0));
    let provider_done = done.clone();
    let provider_requests = requests.clone();
    let evidence = info.with_extension("provider.json");
    std::fs::write(&evidence, "{\"requests\":0}").unwrap();
    let worker = std::thread::spawn(move || {
        while !provider_done.load(Ordering::SeqCst) {
            if let Some(request) = provider.recv_timeout(std::time::Duration::from_millis(200)).unwrap() {
                let count = provider_requests.fetch_add(1, Ordering::SeqCst);
                let message = if count == 0 { json!({"role":"assistant","content":null,"tool_calls":[{"id":"adm8-note","type":"function","function":{"name":"reader.note","arguments":json!({"lid":"1.1","text":"adm8 durable result"}).to_string()}}]}) }
                    else { json!({"role":"assistant","content":"已保留笔记，可以继续阅读当前材料。"}) };
                request.respond(tiny_http::Response::from_string(json!({"choices":[{"message":message,"finish_reason":"stop"}],"usage":{"prompt_tokens":20,"prompt_cache_hit_tokens":0,"completion_tokens":1,"total_tokens":21}}).to_string()).with_header(tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap())).unwrap();
                std::fs::write(&evidence, json!({"requests":count+1}).to_string()).unwrap();
            }
        }
    });
    let server = crate::multi_user_host::start_with_access("127.0.0.1:18788".parse().unwrap(), Arc::new(crate::multi_user_host::Site::new("https://localhost:18443").unwrap()), f.access.clone()).unwrap();
    std::fs::write(&info, json!({"root":f.root.path(),"url":server.url,"book":f.x,"period":period}).to_string()).unwrap();
    while !info.with_extension("stop").exists() { std::thread::sleep(std::time::Duration::from_millis(200)); }
    server.shutdown();
    done.store(true, Ordering::SeqCst);
    worker.join().unwrap();
}
