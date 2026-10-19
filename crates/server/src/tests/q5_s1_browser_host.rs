use super::{adm5_tests, mu5_tests::Fixture};
use runtime::{model_spend::{ChargeAttribution, ChargeScope, ModelSpendPort, SendEvidence, SendIdentity, SendOutcome}, provider_stream::ModelUsage};
use serde_json::{json, Value};
use std::{sync::Arc, time::{Duration,Instant}};

fn outcome(tokens: u32) -> SendOutcome {
    SendOutcome { evidence:SendEvidence::Response, usage:Some(ModelUsage {
        input_tokens:Some(tokens),cached_input_tokens:Some(0),output_tokens:Some(0),total_tokens:Some(tokens),..Default::default()
    }), succeeded:true }
}
fn call(port:&Arc<dyn ModelSpendPort>, scope:ChargeScope, id:&str, purpose:&str, tokens:Option<u32>) -> SendIdentity {
    let identity = SendIdentity {call_id:id.into(),logical_call_id:id.into(),attempt:1,scope:Some(scope),purpose:purpose.into(),model:"fixture-model".into(),provider:"fixture".into()};
    port.before_send(&identity,&json!({"model":"fixture-model","messages":[{"role":"user","content":"fixture"}],"max_tokens":10000})).unwrap();
    let result = tokens.map(outcome).unwrap_or(SendOutcome{evidence:SendEvidence::OutcomeUnknown,usage:None,succeeded:false});
    port.after_send(&identity,&result).unwrap();
    identity
}

/// Real Reader/admin HTTP, immutable books and charge lifecycle. No external payment or model call.
#[test]
#[ignore = "real HTTP fixture for q5-s1-cost-publication.spec.ts"]
fn q5_s1_browser_host() {
    let mut f = Fixture::new();
    let first_period = adm5_tests::seed(&mut f,"fixture",1_000_000);
    f.ok(&f.b,"POST","/api/admin/users/A/allowance-basis",json!({"operation_id":"q5-bind-first","period_id":first_period,"revision":1,"basis":{"version":"q5-first-basis","basis_micro_cny":1_000_000}}));
    let workspace = f.ok(&f.a,"POST","/api/learning-workspaces",json!({"goal":{"text":"深度学习期末复习"},"members":[f.x,f.y]}));
    let workspace2 = f.ok(&f.a,"POST","/api/learning-workspaces",json!({"goal":{"text":"概率课程复习"},"members":[f.y]}));
    let first_space = workspace["learning_workspace_id"].as_str().unwrap().to_owned();
    let second_space = workspace2["learning_workspace_id"].as_str().unwrap().to_owned();
    let scope = ChargeScope::ReaderRun{user_id:"A".into(),run_ref:"lesson-run-old".into()};
    let port = f.access.spend.port_with_attribution(scope.clone(),f.x.clone(),ChargeAttribution{
        parent_task_ref:"lesson-parent".into(),learning_workspace_id:Some(first_space.clone()),related_learning_workspace_ids:vec![]
    },Default::default());
    call(&port,scope.clone(),"q5-answer-old","outer",Some(23000));
    call(&port,scope.clone(),"q5-compression-old","compaction",Some(4000));
    let late = call(&port,scope,"q5-late-repair","repair",None);
    let now = crate::multi_user_host::now();
    f.control.connection.execute("UPDATE allowance_periods SET closed_at=? WHERE period_id=?",rusqlite::params![now,first_period]).unwrap();
    let next_period = f.ok(&f.b,"POST","/api/admin/users/A/allowance-periods",json!({"operation_id":"q5-next-period","starts_at":now,"expires_at":now+3600,"basis":{"version":"q5-next-basis","basis_micro_cny":2_000_000}}))["allowance_period"]["period_id"].as_str().unwrap().to_owned();
    f.ok(&f.b,"POST","/api/admin/users/A/allowance-percentage-adjustments",json!({"operation_id":"q5-next-grant","period_id":next_period,"revision":0,"basis_version":"q5-next-basis","delta_milli_percent":100000,"reason":"fixture"}));
    let next_scope = ChargeScope::ReaderTask{user_id:"A".into(),task_ref:"lesson-task-new".into()};
    let next_port = f.access.spend.port_with_attribution(next_scope.clone(),f.x.clone(),ChargeAttribution{parent_task_ref:"lesson-parent".into(),learning_workspace_id:Some(first_space.clone()),related_learning_workspace_ids:vec![]},Default::default());
    call(&next_port,next_scope,"q5-answer-new","outer",Some(46000));
    let memory_scope = ChargeScope::ReaderTask{user_id:"A".into(),task_ref:"memory-review-attempt".into()};
    let memory_port = f.access.spend.port_with_attribution(memory_scope.clone(),f.x.clone(),ChargeAttribution{parent_task_ref:"memory-review-job".into(),learning_workspace_id:None,related_learning_workspace_ids:vec![first_space.clone(),second_space.clone()]},Default::default());
    call(&memory_port,memory_scope,"q5-account-memory","memory_review",Some(10000));
    let other_scope = ChargeScope::ReaderRun{user_id:"A".into(),run_ref:"probability-run".into()};
    let other_port = f.access.spend.port_with_attribution(other_scope.clone(),f.y.clone(),ChargeAttribution{parent_task_ref:"probability-parent".into(),learning_workspace_id:Some(second_space.clone()),related_learning_workspace_ids:vec![]},Default::default());
    call(&other_port,other_scope,"q5-probability","outer",Some(2000));
    let platform_port = f.access.spend.operator_port("public-preparation".into(),Default::default());
    call(&platform_port,ChargeScope::OperatorTask{task_ref:"public-preparation".into()},"q5-public-build","prebuild",Some(90000));
    call(&platform_port,ChargeScope::OperatorTask{task_ref:"public-preparation".into()},"q5-public-pending","prebuild",None);
    let server = crate::multi_user_host::start_with_access("127.0.0.1:4188".parse().unwrap(),Arc::new(crate::multi_user_host::Site::new("https://reader.example").unwrap()),f.access.clone()).unwrap();
    let control = tiny_http::Server::http("127.0.0.1:4189").unwrap();
    println!("Q5_S1_BROWSER_READY");
    let deadline = Instant::now()+Duration::from_secs(1200);
    while Instant::now()<deadline {
        let Some(request)=control.recv_timeout(Duration::from_secs(1)).unwrap() else {continue;};
        let value = match request.url() {
            "/stop" => {request.respond(tiny_http::Response::from_string("stopped")).unwrap();break;},
            "/settle-late" => {port.after_send(&late,&outcome(7000)).unwrap();json!({"settled":true})},
            "/revoke-reader-material" => {f.access.library.lock().unwrap().revoke("A",&f.y).unwrap();json!({"revoked":true})},
            "/grant-reader-material" => {f.access.library.lock().unwrap().grant("A",&f.y).unwrap();json!({"granted":true})},
            "/fixture" => json!({"first":f.x,"second":f.y,"first_period":first_period,"next_period":next_period,"workspace":first_space,"workspace2":second_space,
                "reader_cookie":format!("{}={}",crate::auth::COOKIE,f.a),"admin_cookie":format!("{}={}",crate::auth::COOKIE,f.b)}),
            _ => Value::Null,
        };
        request.respond(tiny_http::Response::from_string(value.to_string()).with_header(tiny_http::Header::from_bytes("Content-Type","application/json").unwrap())).unwrap();
    }
    server.shutdown();
}
