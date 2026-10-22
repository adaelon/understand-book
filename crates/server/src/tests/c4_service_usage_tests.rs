use super::{adm5_tests, mu5_tests::Fixture};
use serde_json::{json, Value};

fn fixture() -> (Fixture,String) {
    let mut f=Fixture::new();
    let period=adm5_tests::seed(&mut f,"fixture",1_000);
    f.control.connection.execute("INSERT INTO learning_workspaces(owner_user_id,learning_workspace_id,created_at,updated_at) VALUES('A','service-space',0,0)",[]).unwrap();
    f.ok(&f.b,"POST","/api/admin/build-tasks/service-task/budget",json!({"cost_owner":"account","user_id":"A","learning_workspace_id":"service-space","max_micro_cny":100,"confirmation":"service-confirmed"}));
    (f,period)
}
fn rates(f: &Fixture, price: i64) {
    std::fs::write(f.root.path().join("service-rates.json"),json!({"rates":[{
        "version":format!("test-{price}"),"service_kind":"video_transcription","starts_at":0,"expires_at":i64::MAX,
        "debit_micro_cny_per_second":price,"estimated_cost_micro_cny_per_second":2
    }]}).to_string()).unwrap();
}
fn admission(id: &str, milliseconds: i64) -> Value {
    json!({"usage_id":id,"logical_usage_id":id,"attempt":1,"service_kind":"video_transcription","quantity_ms":milliseconds})
}
fn receipt(id: &str, status: &str, quantity: Option<i64>) -> Value {
    json!({"usage_id":id,"status":status,"quantity_ms":quantity,"evidence":{"source":"test-whisper","chunk":"chunk-001"}})
}
fn admit(f: &Fixture, id: &str, ms: i64) -> Value {
    f.ok(&f.b,"POST","/api/admin/build-tasks/service-task/service-admit",admission(id,ms))
}
fn settle(f: &Fixture, id: &str, ms: i64) -> Value {
    f.ok(&f.b,"POST","/api/admin/build-tasks/service-task/service-receipts",receipt(id,"completed",Some(ms)))
}

#[test]
fn c4_service_missing_rate_stops_admission_and_shared_parent_caps_models_and_processing() {
    let (f,_) = fixture();
    assert_eq!(f.call(&f.b,"POST","/api/admin/build-tasks/service-task/service-admit",admission("svc-one",6000)).1["error_code"],"SERVICE_RATE_UNAVAILABLE");
    rates(&f,10);
    let first=admit(&f,"svc-one",6000);
    assert_eq!(first["charge"]["reserved_micro_cny"],60);
    assert_eq!(first["budget"]["remaining_micro_cny"],40);
    assert_eq!(admit(&f,"svc-one",6000)["charge"],first["charge"]);
    let model=json!({"execution_id":"combined","call_id":"model-one","logical_call_id":"model-one","attempt":1,"granularity":"calls","reserved_micro_cny":41,"provider":"fixture","model":"fixture-model","purpose":"external_build","budget_enforced":true,"receipt_capability":"complete"});
    assert_eq!(f.call(&f.b,"POST","/api/admin/build-tasks/service-task/admit",model.clone()).1["error_code"],"BUILD_BUDGET_EXHAUSTED");
    let mut model=model;model["reserved_micro_cny"]=json!(40);
    f.ok(&f.b,"POST","/api/admin/build-tasks/service-task/admit",model);
    assert_eq!(f.ok(&f.a,"GET","/api/account/allowance",json!({}))["current_allowance"]["balance"]["active_reserved_micro_cny"],100);
    rates(&f,99); // Existing admission keeps its original rate.
    let done=settle(&f,"svc-one",6000);
    assert_eq!(done["charge"]["service_charge_micro_cny"],60);
    assert_eq!(done["charge"]["account_debit_micro_cny"],60);
    assert_eq!(done["charge"]["provider_cost_micro_cny"],12);
    assert_eq!(done["charge"]["provider_cost_status"],"estimated");
    assert_eq!(settle(&f,"svc-one",6000),done);
    let model_done=f.ok(&f.b,"POST","/api/admin/build-tasks/service-task/receipts",json!({"call_id":"model-one","status":"confirmed","provider_cost_micro_cny":80,"usage":{},"evidence":{"invoice":"model"},"succeeded":true}));
    assert_eq!(model_done["charge"]["account_debit_micro_cny"],40);
    assert_eq!(model_done["budget"]["debited_micro_cny"],100);
    assert_eq!(model_done["budget"]["platform_covered_micro_cny"],40);
    let charges=f.ok(&f.a,"GET","/api/account/usage",json!({}));
    assert_eq!(charges["total"],2);
    assert!(charges["items"].as_array().unwrap().iter().any(|v|v["entry_kind"]=="service" && v["quantity_ms"]==6000 && v["model"].is_null()));
    let summary=f.ok(&f.b,"GET","/api/admin/cost-summary",json!({}));
    assert_eq!(summary["account"]["account_debit_micro_cny"],100);
    assert_eq!(summary["account"]["confirmed_cost_micro_cny"],80);
    assert_eq!(summary["account"]["estimated_cost_micro_cny"],12);
    assert_eq!(summary["account"]["provider_cost_complete"],false);
}

#[test]
fn c4_service_unknown_preserves_reservation_and_late_receipt_original_period() {
    let (f,period)=fixture(); rates(&f,10);
    admit(&f,"svc-late",3000);
    let pending=f.ok(&f.b,"POST","/api/admin/build-tasks/service-task/service-receipts",receipt("svc-late","unknown",None));
    assert_eq!(pending["charge"]["state"],"pending");
    assert!(pending["charge"]["account_debit_micro_cny"].is_null());
    assert_eq!(pending["budget"]["pending_micro_cny"],30);
    let mut replacement=admission("svc-repeat",3000); replacement["logical_usage_id"]=json!("svc-late"); replacement["attempt"]=json!(2);
    assert_eq!(f.call(&f.b,"POST","/api/admin/build-tasks/service-task/service-admit",replacement).1["error_code"],"SERVICE_ALREADY_ADMITTED");
    f.control.connection.execute("UPDATE allowance_periods SET closed_at=? WHERE period_id=?",rusqlite::params![crate::multi_user_host::now(),period]).unwrap();
    let next=f.ok(&f.b,"POST","/api/admin/users/A/allowance-periods",json!({"operation_id":"service-next","starts_at":crate::multi_user_host::now()-1,"expires_at":crate::multi_user_host::now()+1000}))["allowance_period"]["period_id"].as_str().unwrap().to_owned();
    let done=settle(&f,"svc-late",3000);
    assert_eq!(done["charge"]["period_id"],period);
    assert_eq!(done["charge"]["account_debit_micro_cny"],30);
    assert_eq!(crate::account_allowance::balance(&f.control.connection,&next).unwrap().debited_micro_cny.value(),0);
}

#[test]
fn c4_service_platform_work_keeps_usage_and_unknown_cost_without_student_debit() {
    let (f,_) = fixture();
    let before=f.ok(&f.a,"GET","/api/account/allowance",json!({}));
    f.ok(&f.b,"POST","/api/admin/build-tasks/public-service/budget",json!({"cost_owner":"platform","max_micro_cny":100,"confirmation":"public"}));
    f.ok(&f.b,"POST","/api/admin/build-tasks/public-service/service-admit",admission("svc-public",2500));
    let done=f.ok(&f.b,"POST","/api/admin/build-tasks/public-service/service-receipts",receipt("svc-public","completed",Some(2500)));
    assert_eq!(done["charge"]["quantity_ms"],2500);
    assert_eq!(done["charge"]["account_debit_micro_cny"],0);
    assert!(done["charge"]["provider_cost_micro_cny"].is_null());
    assert_eq!(done["charge"]["provider_cost_status"],"pending");
    assert_eq!(done["budget"]["unknown_count"],1);
    assert_eq!(f.ok(&f.a,"GET","/api/account/allowance",json!({})),before);
}

#[test]
fn c4_service_account_limit_includes_processing_from_another_parent() {
    let (f,period)=fixture(); rates(&f,10);
    f.control.connection.execute("UPDATE build_task_budgets SET max_micro_cny=1000 WHERE task_ref='service-task'",[]).unwrap();
    admit(&f,"svc-all",100000);
    assert_eq!(crate::account_allowance::balance(&f.control.connection,&period).unwrap().available_micro_cny.value(),0);
    f.ok(&f.b,"POST","/api/admin/build-tasks/another-parent/budget",json!({"cost_owner":"account","user_id":"A","learning_workspace_id":"service-space","max_micro_cny":100,"confirmation":"second"}));
    let model=json!({"execution_id":"another-execution","call_id":"new-model","logical_call_id":"new-model","attempt":1,"granularity":"calls","reserved_micro_cny":1,"provider":"fixture","model":"fixture-model","purpose":"external_build","budget_enforced":true,"receipt_capability":"complete"});
    assert_eq!(f.call(&f.b,"POST","/api/admin/build-tasks/another-parent/admit",model).1["error_code"],"MODEL_ALLOWANCE_INSUFFICIENT");
    f.access.spend.recover().unwrap();
    assert_eq!(crate::account_allowance::balance(&f.control.connection,&period).unwrap().pending_micro_cny.value(),1000);
}

#[test]
fn c4_service_audio_review_uses_original_run_scope_without_build_budget() {
    let (f,period)=fixture(); rates(&f,10);
    let scene=f.create(&f.a,&f.x,"audio-meter");
    let scene=f.action(&f.a,&scene,"audio-meter","chat/new",json!({}));
    let run=f.run(&scene,"audio-meter");
    let now=crate::multi_user_host::now();
    let id=f.access.spend.service_admit_run(&run.scope,2500,now).unwrap();
    let charge=crate::service_usage::charge(&f.control.connection,&id).unwrap().unwrap();
    assert_eq!(charge["run_ref"],run.scope.turn_id);
    assert_eq!(charge["parent_task_ref"],run.scope.turn_id);
    assert_eq!(charge["period_id"],period);
    assert!(charge["task_ref"].is_null());
    assert_eq!(charge["purpose"],"video_audio_review");
    assert_eq!(charge["reserved_micro_cny"],25);
    assert!(!f.control.connection.query_row("SELECT EXISTS(SELECT 1 FROM build_task_budgets WHERE task_ref=?)",[&run.scope.turn_id],|r|r.get::<_,bool>(0)).unwrap());
    let done=f.access.spend.service_receipt(&run.scope.turn_id,serde_json::from_value(receipt(&id,"completed",Some(2500))).unwrap(),now).unwrap();
    assert_eq!(done["charge"]["account_debit_micro_cny"],25);
    assert!(done["budget"].is_null());
}

#[test]
fn c4_service_manual_usage_evidence_is_idempotent_and_does_not_invent_supplier_cost() {
    let (f,_) = fixture(); rates(&f,10);
    admit(&f,"svc-review",2000);
    let pending=f.ok(&f.b,"POST","/api/admin/build-tasks/service-task/service-receipts",receipt("svc-review","unknown",None));
    let body=json!({"operation_id":"review-service-once","revision":pending["charge"]["revision"],"status":"completed","quantity_ms":1800,"reason":"Recovered processing log","evidence":"Whisper decoded duration 1.8 seconds"});
    let done=f.ok(&f.b,"POST","/api/admin/service-usage/svc-review/reconcile",body.clone());
    assert_eq!(done["charge"]["account_debit_micro_cny"],18);
    assert_eq!(done["charge"]["provider_cost_status"],"estimated");
    assert_eq!(f.ok(&f.b,"POST","/api/admin/service-usage/svc-review/reconcile",body.clone()),done);
    assert_eq!(f.ok(&f.b,"GET","/api/admin/operations/review-service-once",json!({})),done);
    let mut changed=body;changed["quantity_ms"]=json!(1900);
    assert_eq!(f.call(&f.b,"POST","/api/admin/service-usage/svc-review/reconcile",changed).0,409);
    assert_eq!(f.call(&f.a,"POST","/api/admin/service-usage/svc-review/reconcile",json!({})).0,403);
}

#[test]
fn c4_service_real_pause_continue_keeps_unknown_and_authorizes_only_a_new_confirmation() {
    let (f,_) = fixture(); rates(&f,10);
    let video=f.root.path().join("pause-video.mp4");std::fs::write(&video,b"fixture").unwrap();
    f.ok(&f.b,"POST","/api/admin/operator-builds",json!({"operation_id":"create-pause-video","task_id":"pause-video","name":"Video pause","max_micro_cny":100,"reserved_per_execution_micro_cny":10,"visible_to_users":["A"],"materials":[{"material_id":"video","label":"Video","material_kind":"video","build_tutor":false,"target_input":video,"root_dir":f.root.path().join("video-builds"),"max_parallel":1,"required_capabilities":["text","video"]}]}));
    super::b1_operator_build_tests::transition(&f,"pause-video","start");
    let first=f.ok(&f.b,"POST","/api/admin/operator-builds/next",json!({"host_session_id":"video-host"}))["task"].clone();
    let mut request=admission("pause-first",2000);request["processing_confirmation"]=first["processing_confirmation"].clone();
    f.ok(&f.b,"POST","/api/admin/build-tasks/pause-video/service-admit",request.clone());
    f.ok(&f.b,"POST","/api/admin/build-tasks/pause-video/service-receipts",receipt("pause-first","unknown",None));
    request["usage_id"]=json!("pause-next");request["attempt"]=json!(2);
    assert_eq!(f.call(&f.b,"POST","/api/admin/build-tasks/pause-video/service-admit",request.clone()).1["error_code"],"SERVICE_ALREADY_ADMITTED");
    super::b1_operator_build_tests::transition(&f,"pause-video","pause");
    f.ok(&f.b,"POST","/api/admin/operator-builds/pause-video/finish",json!({"host_session_id":"video-host","state":"interrupted"}));
    super::b1_operator_build_tests::transition(&f,"pause-video","continue");
    let continued=f.ok(&f.b,"POST","/api/admin/operator-builds/next",json!({"host_session_id":"video-host"}))["task"].clone();
    assert_ne!(continued["processing_confirmation"],first["processing_confirmation"]);
    request["processing_confirmation"]=continued["processing_confirmation"].clone();
    let admitted=f.ok(&f.b,"POST","/api/admin/build-tasks/pause-video/service-admit",request);
    assert_eq!(admitted["budget"]["pending_micro_cny"],20);
    assert_eq!(admitted["budget"]["reserved_micro_cny"],20);
    let done=f.ok(&f.b,"POST","/api/admin/build-tasks/pause-video/service-receipts",receipt("pause-next","completed",Some(2000)));
    assert_eq!(done["budget"]["pending_micro_cny"],20);
    assert_eq!(done["charge"]["state"],"settled");
}
