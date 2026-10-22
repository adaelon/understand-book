use super::mu5_tests::Fixture;
use serde_json::{json, Value};

fn create(f: &Fixture, id: &str) -> Value {
    json!({"operation_id":format!("create-{id}"),"task_id":id,"name":"视频公共准备",
        "max_micro_cny":200,"reserved_per_execution_micro_cny":100,"visible_to_users":["A","B"],"materials":[
        {"material_id":"video","label":"视频","target_input":f.root.path().join("course.MP4"),
            "root_dir":f.root.path().join("builds"),"pass2":"disabled","max_parallel":1}]})
}
fn get(f: &Fixture, id: &str) -> Value {
    f.ok(&f.b,"GET",&format!("/api/admin/operator-builds/{id}"),Value::Null)
}
fn claim(f: &Fixture) -> Value {
    f.ok(&f.b,"POST","/api/admin/operator-builds/next",json!({"host_session_id":"video-host"}))["task"].clone()
}
fn transition(f: &Fixture, id: &str, command: &str) -> Value {
    super::b1_operator_build_tests::transition(f,id,command)
}
fn admission(index: u32) -> Value {
    json!({"execution_id":format!("video-execution-{index}"),"call_id":format!("video-call-{index}"),
        "logical_call_id":format!("video:translation:{index}"),"attempt":1,"granularity":"execution_total",
        "reserved_micro_cny":100,"provider":"openai","model":"codex-video","purpose":"external_build",
        "budget_enforced":false,"receipt_capability":"partial"})
}

#[test]
fn video_operator_one_click_confirms_budget_and_can_pause_before_plan_without_losing_identity() {
    let f=super::b1_operator_build_tests::fixture(); let id="video-one-click";
    let input=create(&f,id);
    let created=f.ok(&f.b,"POST","/api/admin/operator-builds",input.clone());
    assert_eq!(f.ok(&f.b,"POST","/api/admin/operator-builds",input),created);
    let material=&created["operator_build"]["materials"][0];
    assert_eq!(material["material_kind"],"video"); assert_eq!(material["build_tutor"],false);
    assert_eq!(material["required_capabilities"],json!(["text","video","translation","visual_index","graph","discourse"]));
    assert!(created["operator_build"]["budget"].is_null());
    let started=transition(&f,id,"start");
    assert_eq!(started["state"],"queued"); assert_eq!(started["budget"]["cost_owner"],"platform");
    assert_eq!(started["budget"]["remaining_micro_cny"],200);
    assert_eq!(transition(&f,id,"pause")["state"],"interrupted");
    let resumed=transition(&f,id,"continue");
    assert_eq!(resumed["confirmation"],started["confirmation"]);
    assert_eq!(resumed["confirmed_at"],started["confirmed_at"]);
    let active=claim(&f); assert_eq!(active["action"],"run");
    let workspace=f.root.path().join("builds/video-video");
    let plan=json!({"version":"operator_video_plan.v1","target_input":active["materials"][0]["target_input"],
        "workspace_dir":workspace,"review_markdown":"字幕、画面、知识关系与原讲解结构","build_tutor":false});
    let progress=json!({"host_session_id":"video-host","state":"running","plan":plan,
        "workspace_dir":workspace,"invocation_ref":"video:video-one-click:video",
        "observation":{"progress":{"stage":"transcription","completed":1,"total":3,"unit":"chunks"}}});
    let saved=f.ok(&f.b,"POST",&format!("/api/admin/operator-builds/{id}/materials/video/progress"),progress.clone());
    assert_eq!(saved["materials"][0]["observation"]["progress"]["completed"],1);
    // A host reconnect reuses the persisted scope and real progress, not a fresh execution.
    assert_eq!(claim(&f)["materials"],saved["materials"]);
    let mut changed=progress; changed["plan"]["workspace_dir"]=json!(f.root.path().join("another"));
    assert_eq!(f.call(&f.b,"POST",&format!("/api/admin/operator-builds/{id}/materials/video/progress"),changed).0,409);
    assert_eq!(f.control.connection.query_row("SELECT count(*) FROM model_call_charges",[],|r|r.get::<_,i64>(0)).unwrap(),0);
}

#[test]
fn video_operator_receipts_hold_shared_budget_and_resume_keeps_platform_ownership() {
    let f=super::b1_operator_build_tests::fixture(); let id="video-budget";
    f.ok(&f.b,"POST","/api/admin/operator-builds",create(&f,id));
    let original=transition(&f,id,"start"); claim(&f);
    let endpoint=format!("/api/admin/build-tasks/{id}");
    for index in 1..=2 {
        f.ok(&f.b,"POST",&format!("{endpoint}/admit"),admission(index));
        let receipt=json!({"call_id":format!("video-call-{index}"),"status":if index==1 {"partial"} else {"unknown"},
            "usage":if index==1 {json!({"input_tokens":20,"output_tokens":5})} else {Value::Null},
            "evidence":{"source":"codex_video_execution","stage":"translation"},"succeeded":index==1});
        let settled=f.ok(&f.b,"POST",&format!("{endpoint}/receipts"),receipt.clone());
        assert_eq!(f.ok(&f.b,"POST",&format!("{endpoint}/receipts"),receipt),settled);
    }
    assert_eq!(f.call(&f.b,"POST",&format!("{endpoint}/admit"),admission(3)).1["error_code"],"BUILD_BUDGET_EXHAUSTED");
    let budget=&get(&f,id)["budget"];
    assert_eq!(budget["pending_micro_cny"],200); assert_eq!(budget["remaining_micro_cny"],0);
    assert_eq!(budget["unknown_count"],2); assert_eq!(budget["debited_micro_cny"],0);
    f.ok(&f.b,"POST",&format!("/api/admin/operator-builds/{id}/finish"),json!({"host_session_id":"video-host","state":"interrupted","error":"BUILD_BUDGET_EXHAUSTED"}));
    let raised=f.ok(&f.b,"POST",&format!("/api/admin/operator-builds/{id}/continue"),json!({"operation_id":"video-raise-budget","expected_revision":get(&f,id)["revision"],"max_micro_cny":300}))["operator_build"].clone();
    assert_eq!(raised["confirmation"],original["confirmation"]);
    assert_eq!(raised["budget"]["pending_micro_cny"],200);
    assert_eq!(raised["budget"]["remaining_micro_cny"],100);
    f.ok(&f.b,"POST",&format!("{endpoint}/admit"),admission(3));
    f.ok(&f.b,"POST",&format!("{endpoint}/receipts"),json!({"call_id":"video-call-3","status":"not_sent","evidence":{"source":"spawn_failed_before_send"},"succeeded":false}));
    assert_eq!(get(&f,id)["budget"]["remaining_micro_cny"],100);
    f.ok(&f.b,"POST",&format!("{endpoint}/receipts"),json!({"call_id":"video-call-1","status":"confirmed","provider_cost_micro_cny":70,"evidence":{"source":"actual_provider_receipt"},"succeeded":true}));
    assert_eq!(get(&f,id)["budget"]["provider_cost_micro_cny"],70);
    let costs=f.ok(&f.b,"GET","/api/admin/cost-summary",Value::Null);
    assert_eq!(costs["platform"]["confirmed_cost_micro_cny"],70);
    assert_eq!(costs["account"]["request_count"],0);
}

#[test]
fn video_operator_rejects_false_tutor_capability_and_keeps_book_plan_review() {
    let f=super::b1_operator_build_tests::fixture();
    let mut input=create(&f,"video-no-teaching"); input["materials"][0]["build_tutor"]=json!(true);
    assert_eq!(f.call(&f.b,"POST","/api/admin/operator-builds",input).1["error_code"],"OPERATOR_VIDEO_TUTOR_UNSUPPORTED");
    let mut wrong_kind=create(&f,"video-as-book");wrong_kind["materials"][0]["material_kind"]=json!("book");
    assert_eq!(f.call(&f.b,"POST","/api/admin/operator-builds",wrong_kind).0,400);
    let id="mixed-book-video"; let mut mixed=create(&f,id);
    let book=super::b1_operator_build_tests::create(&f,"ignored")["materials"][1].clone();
    mixed["materials"].as_array_mut().unwrap().push(book);
    let created=f.ok(&f.b,"POST","/api/admin/operator-builds",mixed);
    assert_eq!(created["operator_build"]["materials"][1]["build_tutor"],false);
    assert_eq!(f.call(&f.b,"POST",&format!("/api/admin/operator-builds/{id}/start"),json!({"operation_id":"mixed-early","expected_revision":0})).0,409);
}

#[test]
fn music_operator_uses_shared_queue_budget_and_stable_curation_plan_without_transcript() {
    let f=super::b1_operator_build_tests::fixture(); let id="music-curated";
    let mut input=create(&f,id);
    input["materials"][0]["material_kind"]=json!("music");
    input["materials"][0]["target_input"]=json!(f.root.path().join("schubert.curation.json"));
    let created=f.ok(&f.b,"POST","/api/admin/operator-builds",input);
    assert_eq!(created["operator_build"]["materials"][0]["required_capabilities"],json!(["music","music_score","music_events"]));
    let started=transition(&f,id,"start");
    assert_eq!(started["state"],"queued");
    assert_eq!(started["budget"]["cost_owner"],"platform");
    assert_eq!(transition(&f,id,"pause")["state"],"interrupted");
    let resumed=transition(&f,id,"continue");
    assert_eq!(resumed["confirmation"],started["confirmation"]);
    let active=claim(&f);
    let workspace=f.root.path().join("builds/music-schubert");
    let plan=json!({"version":"operator_music_plan.v1","target_input":active["materials"][0]["target_input"],
        "workspace_dir":workspace,"review_markdown":"录音、谱本、演奏出现与有来源的音乐事件","build_tutor":false});
    let progress=json!({"host_session_id":"video-host","state":"running","plan":plan,"workspace_dir":workspace,
        "invocation_ref":"music:music-curated:video","observation":{"progress":{"stage":"music_score","completed":2,"total":3,"unit":"tasks"}}});
    let saved=f.ok(&f.b,"POST",&format!("/api/admin/operator-builds/{id}/materials/video/progress"),progress.clone());
    assert_eq!(claim(&f)["materials"],saved["materials"]);
    let mut changed=progress;changed["plan"]["workspace_dir"]=json!(f.root.path().join("other"));
    assert_eq!(f.call(&f.b,"POST",&format!("/api/admin/operator-builds/{id}/materials/video/progress"),changed).0,409);
    assert_eq!(get(&f,id)["budget"]["provider_cost_micro_cny"],0);
    assert_eq!(f.control.connection.query_row("SELECT count(*) FROM model_call_charges",[],|r|r.get::<_,i64>(0)).unwrap(),0);
}

#[test]
fn music_operator_rejects_unimplemented_tutor_prebuild_and_book_plan_substitution() {
    let f=super::b1_operator_build_tests::fixture();
    let mut input=create(&f,"music-no-tutor");
    input["materials"][0]["material_kind"]=json!("music");
    input["materials"][0]["target_input"]=json!(f.root.path().join("curation.json"));
    input["materials"][0]["build_tutor"]=json!(true);
    assert_eq!(f.call(&f.b,"POST","/api/admin/operator-builds",input.clone()).1["error_code"],"OPERATOR_MUSIC_TUTOR_UNSUPPORTED");
    input["materials"][0]["build_tutor"]=json!(false);
    f.ok(&f.b,"POST","/api/admin/operator-builds",input);
    transition(&f,"music-no-tutor","prepare");claim(&f);
    let workspace=f.root.path().join("builds/music");
    let wrong_plan=json!({"host_session_id":"video-host","state":"planned","workspace_dir":workspace,
        "plan":{"build_plan_path":"book-plan.json","workspace_dir":workspace,"plan":{"private_artifacts":[]}}});
    assert_eq!(f.call(&f.b,"POST","/api/admin/operator-builds/music-no-tutor/materials/video/progress",wrong_plan).0,400);
}
