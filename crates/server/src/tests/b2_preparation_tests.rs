use super::{mu5_tests::Fixture, adm5_tests};
use serde_json::{json,Value};
use base64::Engine;

fn fixture() -> (Fixture,String,String) {
    let mut f=Fixture::new(); let period=adm5_tests::seed(&mut f,"fixture",100_000);
    f.control.connection.execute("INSERT INTO allowance_bases VALUES('b2-basis',100000)",[]).unwrap();
    f.control.connection.execute("INSERT INTO allowance_period_bases VALUES(?,'b2-basis')",[&period]).unwrap();
    crate::cloud_storage::tests::seed(&f.control.connection,"A",&period,1024*1024,1024*1024);
    let workspace=f.ok(&f.a,"POST","/api/learning-workspaces",json!({"goal":{"text":"学习并解释深度学习"},"members":[f.x]}))["learning_workspace_id"].as_str().unwrap().to_owned();
    (f,workspace,period)
}
fn path(workspace:&str)->String{format!("/api/learning-workspaces/{workspace}/preparations")}
fn get(f:&Fixture,workspace:&str,id:&str)->Value{f.ok(&f.a,"GET",&path(workspace),Value::Null)["items"].as_array().unwrap().iter().find(|v|v["task_id"]==id).unwrap().clone()}
fn action(f:&Fixture,workspace:&str,id:&str,action:&str,period:&str,limit:i64)->Value{
    f.ok(&f.a,"POST",&path(workspace),json!({"action":action,"operation_id":uuid::Uuid::now_v7().to_string(),"task_id":id,"expected_revision":get(f,workspace,id)["revision"],"period_id":period,"max_milli_percent":limit}))
}
fn uploaded(f:&Fixture,workspace:&str,id:&str)->Value{
    let input=json!({"action":"upload","upload_id":id,"filename":"lesson.md","offset":0,"total":20,"chunk_base64":base64::engine::general_purpose::STANDARD.encode(b"# Learning\n\nA lesson")});
    let a=f.ok(&f.a,"POST",&path(workspace),input.clone()); assert_eq!(a,f.ok(&f.a,"POST",&path(workspace),input));
    f.ok(&f.a,"POST",&path(workspace),json!({"action":"create","operation_id":format!("create-{id}"),"task_id":id,"upload_id":id}))
}
fn planned(f:&Fixture,w:&str,id:&str){
    action(f,w,id,"prepare","",0);
    f.ok(&f.b,"POST","/api/admin/operator-builds/next",json!({"host_session_id":"b2-host"}));
    let directory=f.access.library.lock().unwrap().load("A",&f.y).unwrap().directory().to_path_buf();
    f.ok(&f.b,"POST",&format!("/api/admin/operator-builds/{id}/materials/source/progress"),json!({"host_session_id":"b2-host","state":"planned","workspace_dir":directory,"plan":{"workspace_dir":directory,"build_plan_path":directory.join("plan.json"),"plan":{"private_artifacts":[],"estimate":{"input_tokens":{"lower":10,"upper":100},"output_tokens":{"lower":10,"upper":100},"unknown_stages":["structure"]}},"model_runtime":{"provider":"fixture","model":"fixture-model","max_output_tokens":100,"context_window_tokens":1000},"review_markdown":"复用已完成来源"}}));
    f.ok(&f.b,"POST",&format!("/api/admin/operator-builds/{id}/finish"),json!({"host_session_id":"b2-host","state":"awaiting_confirmation"}));
}
fn admission(call:&str)->Value{json!({"execution_id":"b2-execution","call_id":call,"logical_call_id":call,"attempt":1,"granularity":"calls","reserved_micro_cny":1,"provider":"fixture","model":"fixture-model","purpose":"personal_prepare","budget_enforced":true,"receipt_capability":"complete","request":{"model":"fixture-model","input_tokens_estimate":25,"max_tokens":25}})}
fn receipt(call:&str,tokens:u32)->Value{json!({"call_id":call,"status":"partial","usage":{"input_tokens":tokens,"output_tokens":0,"cached_input_tokens":0,"cache_creation_input_tokens":null,"total_tokens":tokens},"evidence":{"source":"controlled-provider"},"succeeded":true})}

#[test]
fn b2_personal_upload_budget_cap_resume_import_and_reuse_stay_in_original_workspace(){
    let(f,w,p)=fixture(); uploaded(&f,&w,"b2-task"); planned(&f,&w,"b2-task");
    let planned=get(&f,&w,"b2-task");assert_eq!(planned["estimate"]["status"],"estimated");assert_eq!(planned["estimate"]["partial"],true);
    action(&f,&w,"b2-task","start",&p,1000); // 1%, 1000 internal units.
    let adm="/api/admin/build-tasks/b2-task";
    f.ok(&f.b,"POST",&format!("{adm}/metered-admit"),admission("b2-paid"));
    let settled=f.ok(&f.b,"POST",&format!("{adm}/metered-receipts"),receipt("b2-paid",1200));
    assert_eq!(settled["charge"]["account_debit_micro_cny"],1000);assert_eq!(settled["budget"]["platform_covered_micro_cny"],200);
    assert_eq!(f.ok(&f.b,"POST",&format!("{adm}/metered-receipts"),receipt("b2-paid",1200)),settled);
    assert_eq!(f.call(&f.b,"POST",&format!("{adm}/metered-admit"),admission("b2-blocked")).1["error_code"],"BUILD_BUDGET_EXHAUSTED");
    action(&f,&w,"b2-task","pause",&p,0);
    action(&f,&w,"b2-task","continue",&p,3000);
    f.ok(&f.b,"POST",&format!("{adm}/metered-admit"),admission("b2-after-topup"));
    f.ok(&f.b,"POST",&format!("{adm}/metered-receipts"),receipt("b2-after-topup",25));
    let other=f.ok(&f.a,"POST","/api/learning-workspaces",json!({"goal":{"text":"考试冲刺"}}));
    f.ok(&f.b,"POST","/api/admin/operator-builds/next",json!({"host_session_id":"b2-host"}));
    let directory=f.access.library.lock().unwrap().load("A",&f.y).unwrap().directory().to_path_buf();
    f.ok(&f.b,"POST","/api/admin/operator-builds/b2-task/materials/source/progress",json!({"host_session_id":"b2-host","state":"complete","workspace_dir":directory,"invocation_ref":"b2-invocation"}));
    f.ok(&f.b,"POST","/api/admin/operator-builds/b2-task/materials/source/import",json!({"host_session_id":"b2-host","workspace_dir":directory,"invocation_ref":"b2-invocation"}));
    f.ok(&f.b,"POST","/api/admin/operator-builds/b2-task/finish",json!({"host_session_id":"b2-host","state":"ready"}));
    let original=f.ok(&f.a,"GET",&format!("/api/learning-workspaces/{w}"),Value::Null);assert_eq!(original["members"].as_array().unwrap().len(),2);
    assert!(f.ok(&f.a,"GET",&format!("/api/learning-workspaces/{}",other["learning_workspace_id"].as_str().unwrap()),Value::Null)["members"].as_array().unwrap().is_empty());
    let before=f.ok(&f.b,"GET",adm,Value::Null);
    f.ok(&f.a,"POST",&format!("/api/learning-workspaces/{w}/goal"),json!({"goal":{"text":"改为复习同一本书"}}));
    let repeated=f.ok(&f.a,"POST",&path(&w),json!({"action":"create","operation_id":"again","task_id":"another-task","upload_id":"b2-task"}));assert_eq!(repeated["task_id"],"b2-task");
    let reuse=f.ok(&f.a,"POST",&path(&w),json!({"action":"create","operation_id":"reuse","task_id":"reuse-task","name":"已有资料","published_book_ref":f.x}));assert_eq!(reuse["state"],"ready");
    assert_eq!(f.ok(&f.b,"GET",adm,Value::Null),before);
    assert_ne!(f.call(&f.b,"GET",&path(&w),Value::Null).0,200);
}

#[test]
fn b2_partial_usage_and_changed_allowance_period_require_explicit_continuation(){
    let(f,w,p)=fixture();uploaded(&f,&w,"partial");planned(&f,&w,"partial");action(&f,&w,"partial","start",&p,5000);
    let adm="/api/admin/build-tasks/partial";
    f.ok(&f.b,"POST",&format!("{adm}/metered-admit"),admission("missing-cache"));
    let mut partial=receipt("missing-cache",25);partial["usage"]["cached_input_tokens"]=Value::Null;
    let value=f.ok(&f.b,"POST",&format!("{adm}/metered-receipts"),partial);assert_eq!(value["charge"]["state"],"pending");assert!(value["charge"]["account_debit_micro_cny"].is_null());
    let now=crate::multi_user_host::now();
    f.control.connection.execute("UPDATE allowance_periods SET closed_at=? WHERE period_id=?",rusqlite::params![now-1,p]).unwrap();
    let next=f.ok(&f.b,"POST","/api/admin/users/A/allowance-periods",json!({"operation_id":"next-period","starts_at":now,"expires_at":now+3600,"basis":{"version":"b2-next-basis","basis_micro_cny":200000}}))["allowance_period"]["period_id"].as_str().unwrap().to_owned();
    f.ok(&f.b,"POST","/api/admin/users/A/allowance-adjustments",json!({"operation_id":"next-grant","period_id":next,"revision":0,"delta_micro_cny":10000,"reason":"top up"}));
    assert_eq!(f.call(&f.b,"POST",&format!("{adm}/metered-admit"),admission("changed-period")).1["error_code"],"BUILD_PERIOD_CONFIRMATION_REQUIRED");
    action(&f,&w,"partial","pause",&next,0);action(&f,&w,"partial","continue",&next,5000);
    f.ok(&f.b,"POST",&format!("{adm}/metered-admit"),admission("confirmed-period"));
    let late=f.ok(&f.b,"POST",&format!("{adm}/metered-receipts"),receipt("missing-cache",25));assert_eq!(late["charge"]["period_id"],p);assert_eq!(late["charge"]["state"],"settled");
    f.ok(&f.b,"POST",&format!("{adm}/metered-receipts"),receipt("confirmed-period",25));
    let task=get(&f,&w,"partial");let groups=task["usage_periods"].as_array().unwrap();
    assert_eq!(groups.iter().find(|v|v["period_id"]==p).unwrap()["used"]["value"],"0.025");
    assert_eq!(groups.iter().find(|v|v["period_id"]==next).unwrap()["used"]["value"],"0.0125");
}

#[test]
#[ignore = "actual HTTP host for B2 preparation and browser acceptance"]
fn b2_browser_host(){
    let mut f=Fixture::new();let period=adm5_tests::seed_model(&mut f,"probe","scripted",1_000_000);
    f.control.connection.execute("INSERT INTO allowance_bases VALUES('b2-http-basis',1000000)",[]).unwrap();
    f.control.connection.execute("INSERT INTO allowance_period_bases VALUES(?,'b2-http-basis')",[&period]).unwrap();
    crate::cloud_storage::tests::seed(&f.control.connection,"A",&period,64*1024*1024,64*1024*1024);
    let origin="https://127.0.0.1:4190";
    let server=crate::multi_user_host::start_with_access("127.0.0.1:4190".parse().unwrap(),std::sync::Arc::new(crate::multi_user_host::Site::new(origin).unwrap()),f.access.clone()).unwrap();
    let control=tiny_http::Server::http("127.0.0.1:4191").unwrap();
    println!("B2_BROWSER_READY");
    let until=std::time::Instant::now()+std::time::Duration::from_secs(2400);
    while std::time::Instant::now()<until {
        let Some(request)=control.recv_timeout(std::time::Duration::from_secs(1)).unwrap() else{continue};
        if request.url()=="/stop" {request.respond(tiny_http::Response::from_string("stopped")).unwrap();break;}
        let result=json!({"first":f.x,"second":f.y,"root":f.root.path(),"period_id":period,"origin":origin,
            "reader_cookie":format!("{}={}",crate::auth::COOKIE,f.a),"reader_csrf":crate::auth::csrf(&f.a),"admin_cookie":format!("{}={}",crate::auth::COOKIE,f.b),"admin_csrf":crate::auth::csrf(&f.b)});
        request.respond(tiny_http::Response::from_string(result.to_string()).with_header(tiny_http::Header::from_bytes("Content-Type","application/json").unwrap())).unwrap();
    }
    server.shutdown();
}
