use super::mu5_tests::Fixture;
use serde_json::{json, Value};
pub(super) fn fixture() -> Fixture {
    let mut f = Fixture::new();
    f.control.set_reader_admin("B", true).unwrap();
    f
}
pub(super) fn create(f: &Fixture, id: &str) -> Value {
    let directory = f
        .access
        .library
        .lock()
        .unwrap()
        .load("A", &f.y)
        .unwrap()
        .directory()
        .to_path_buf();
    json!({"operation_id":format!("create-{id}"),"task_id":id,"name":"两书公共准备","max_micro_cny":1000,"reserved_per_execution_micro_cny":100,"visible_to_users":["A","B"],"materials":[
        {"material_id":"existing","label":"已有成果","published_book_ref":f.x},
        {"material_id":"new","label":"需要准备","target_input":directory,"root_dir":f.root.path().join("builds"),"pass2":"enabled","max_parallel":1}
    ]})
}
fn get(f: &Fixture, id: &str) -> Value {
    f.ok(
        &f.b,
        "GET",
        &format!("/api/admin/operator-builds/{id}"),
        Value::Null,
    )
}
pub(super) fn transition(f: &Fixture, id: &str, action: &str) -> Value {
    f.ok(&f.b,"POST",&format!("/api/admin/operator-builds/{id}/{action}"),json!({"operation_id":format!("{action}-{id}-{}",uuid::Uuid::now_v7()),"expected_revision":get(f,id)["revision"]}))["operator_build"].clone()
}
pub(super) fn prepare(f: &Fixture, id: &str) -> Value {
    transition(f, id, "prepare");
    let task = f.ok(
        &f.b,
        "POST",
        "/api/admin/operator-builds/next",
        json!({"host_session_id":"operator-host"}),
    )["task"]
        .clone();
    let directory = task["materials"][1]["target_input"].clone();
    f.ok(&f.b,"POST",&format!("/api/admin/operator-builds/{id}/materials/new/progress"),json!({"host_session_id":"operator-host","state":"planned","plan":{"build_plan_path":f.root.path().join("plan.json"),"workspace_dir":directory,"plan":{"private_artifacts":[],"plan_id":"fixture-plan","revision":1},"review_markdown":"补齐公共结构，复用已完成单元","model_runtime":{"provider":"fixture","model":"fixture"}},"workspace_dir":directory}));
    f.ok(
        &f.b,
        "POST",
        &format!("/api/admin/operator-builds/{id}/finish"),
        json!({"host_session_id":"operator-host","state":"awaiting_confirmation"}),
    )
}
fn admission() -> Value {
    json!({"execution_id":"b1-execution","call_id":"b1-call","logical_call_id":"b1-logical","attempt":1,"granularity":"execution_total","reserved_micro_cny":100,"provider":"fixture","model":"fixture","purpose":"prebuild","budget_enforced":false,"receipt_capability":"partial"})
}

#[test]
fn automatic_course_import_binds_publication_and_publishes_playable_inquiry() {
    let f=fixture();let id="built-inquiry";
    let mut input=create(&f,id);input["materials"][1]["build_course"]=json!(true);
    input["materials"][1]["course_goal"]=json!("通过阅读和推理理解条件");
    f.ok(&f.b,"POST","/api/admin/operator-builds",input);
    prepare(&f,id);transition(&f,id,"start");
    let task=f.ok(&f.b,"POST","/api/admin/operator-builds/next",json!({"host_session_id":"operator-host"}))["task"].clone();
    let directory=task["materials"][1]["plan"]["workspace_dir"].as_str().unwrap();
    let root=std::path::Path::new(directory);
    let book=read_tools::Book::load(root.to_str().unwrap()).unwrap();
    let mut course:Value=serde_json::from_str(include_str!("../../../../docs/examples/inquiry-course/ai-infra-resource-bounds.v1.course.json")).unwrap();
    let lesson=&mut course["lessons"][0];
    lesson["summary"]["source"]=json!({"publication":{"book_id":book.base.book_id,"publication_id":"build"},"source_revision":book.source_fingerprint(),"start_lid":"1.1","end_lid":"1.3","label":"真实版本绑定测试","structure_refs":["1"]});
    lesson["experiment"]=json!({"kind":"inquiry","activity":"reasoning","instruction":"核对条件","material":"对照材料中的定义和结论。","response_prompt":"写出所依据的条件。"});
    lesson["prediction"]["options"]=json!([]);lesson["task"]["options"]=json!([]);
    lesson["assessment"]=json!({"rule":{"kind":"open","items":[{"id":"condition","criterion":"解释条件与结论的关系","source_lids":["1.2"]}]},"source_lids":["1.2"],"source_sufficient":true,"feedback":"after_submit"});
    let lesson_id=lesson["summary"]["id"].as_str().unwrap().to_owned();
    f.ok(&f.b,"POST",&format!("/api/admin/operator-builds/{id}/materials/new/progress"),json!({"host_session_id":"operator-host","state":"complete","invocation_ref":"course-invocation","workspace_dir":directory}));
    let import=json!({"host_session_id":"operator-host","invocation_ref":"course-invocation","workspace_dir":directory});
    assert_ne!(f.call(&f.b,"POST",&format!("/api/admin/operator-builds/{id}/materials/new/import"),import.clone()).0,200);
    std::fs::write(root.join("inquiry_course.json"),json!({"version":"course_artifact.v1","source_id":book.base.book_id,"source_revision":book.source_fingerprint(),"course":course}).to_string()).unwrap();
    std::fs::write(root.join("course_build_report.json"),json!({"source_id":book.base.book_id,"source_revision":book.source_fingerprint(),"lessons":[{"id":lesson_id,"review":{"accepted":true,"issues":[]}}]}).to_string()).unwrap();
    let imported=f.ok(&f.b,"POST",&format!("/api/admin/operator-builds/{id}/materials/new/import"),import);
    let reference=imported["materials"][1]["published_book_ref"].clone();
    assert_eq!(imported["materials"][1]["course"]["lessons"][0]["source"]["publication"],reference);
    assert!(imported["materials"][1].get("prepared_course").is_none());
    f.ok(&f.b,"POST",&format!("/api/admin/operator-builds/{id}/finish"),json!({"host_session_id":"operator-host","state":"ready"}));
    let mut release=super::s1_workspace_publication_tests::publication(&f,"course-release",0);
    release.as_object_mut().unwrap().remove("operation_id");release["manifest"]["materials"][1]["published_book_ref"]=reference;
    release["manifest"]["topic_links"]=json!([]);release["manifest"]["suggested_route"]=json!([]);
    let published=f.ok(&f.b,"POST",&format!("/api/admin/operator-builds/{id}/publish"),json!({"operation_id":"publish-course","release":release}));
    assert_eq!(published["workspace_release"]["version"],1);
    let public=f.ok(&f.a,"GET","/api/workspace-releases/exam-course/versions/1",Value::Null);
    assert!(public["manifest"]["inquiry_course"]["lessons"][0].get("solution").is_none());
    let used=f.ok(&f.a,"POST","/api/workspace-releases/exam-course/versions/1/use",json!({"operation_id":"use-built-course","goal":{"text":"理解条件"}}));
    let workspace=used["workspace"]["learning_workspace_id"].as_str().unwrap();
    let api=format!("/api/learning-workspaces/{workspace}/shared-release/course");
    let opened=f.ok(&f.a,"POST",&api,json!({"operation_id":"start-course","run_id":null,"expected_revision":0,"release_version":1,"action":{"kind":"start","lesson_id":format!("new-{lesson_id}")}}));
    assert_eq!(opened["inquiry_kind"],"reasoning");assert!(opened["experiment"].is_null());
}

#[test]
fn b1_two_materials_reuse_resume_import_publish_and_personal_use_follow_one_platform_budget() {
    let f = fixture();
    let id = "b1-course";
    let mut input = create(&f, id);
    input["materials"][1].as_object_mut().unwrap().remove("pass2");
    let first = f.ok(&f.b, "POST", "/api/admin/operator-builds", input.clone());
    assert_eq!(first["operator_build"]["materials"][1]["pass2"], "disabled");
    assert_eq!(
        first,
        f.ok(&f.b, "POST", "/api/admin/operator-builds", input)
    );
    assert_eq!(prepare(&f, id)["state"], "awaiting_confirmation");
    assert_eq!(
        f.control
            .connection
            .query_row("SELECT count(*) FROM model_call_charges", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    transition(&f, id, "start");
    let claimed = f.ok(
        &f.b,
        "POST",
        "/api/admin/operator-builds/next",
        json!({"host_session_id":"operator-host"}),
    )["task"]
        .clone();
    assert_eq!(claimed["materials"][0]["state"], "reused");
    assert_eq!(
        f.ok(
            &f.b,
            "POST",
            "/api/admin/operator-builds/next",
            json!({"host_session_id":"other-host"})
        )["task"],
        Value::Null
    );
    assert_eq!(
        f.ok(
            &f.b,
            "POST",
            "/api/admin/operator-builds/next",
            json!({"host_session_id":"operator-host"})
        )["task"]["revision"],
        claimed["revision"]
    );
    let directory = claimed["materials"][1]["plan"]["workspace_dir"].clone();
    let progress = json!({"host_session_id":"operator-host","state":"running","invocation_ref":"stable-invocation","workspace_dir":directory});
    f.ok(
        &f.b,
        "POST",
        &format!("/api/admin/operator-builds/{id}/materials/new/progress"),
        progress.clone(),
    );
    f.ok(
        &f.b,
        "POST",
        &format!("/api/admin/build-tasks/{id}/admit"),
        admission(),
    );
    f.ok(&f.b,"POST",&format!("/api/admin/build-tasks/{id}/receipts"),json!({"call_id":"b1-call","status":"partial","evidence":{"source":"stream"},"succeeded":false}));
    transition(&f, id, "pause");
    assert_eq!(get(&f, id)["pause_requested"], true);
    f.ok(&f.b,"POST",&format!("/api/admin/operator-builds/{id}/finish"),json!({"host_session_id":"operator-host","state":"interrupted","error":"test interruption"}));
    transition(&f, id, "continue");
    let resumed = f.ok(
        &f.b,
        "POST",
        "/api/admin/operator-builds/next",
        json!({"host_session_id":"operator-host"}),
    )["task"]
        .clone();
    assert_eq!(
        resumed["materials"][1]["invocation_ref"],
        "stable-invocation"
    );
    assert_eq!(resumed["budget"]["unknown_count"], 1);
    assert_eq!(resumed["budget"]["pending_micro_cny"], 100);
    let mut completed = progress;
    completed["state"] = json!("complete");
    f.ok(
        &f.b,
        "POST",
        &format!("/api/admin/operator-builds/{id}/materials/new/progress"),
        completed,
    );
    let import = json!({"host_session_id":"operator-host","invocation_ref":"stable-invocation","workspace_dir":directory});
    let imported = f.ok(
        &f.b,
        "POST",
        &format!("/api/admin/operator-builds/{id}/materials/new/import"),
        import.clone(),
    );
    assert_eq!(
        imported,
        f.ok(
            &f.b,
            "POST",
            &format!("/api/admin/operator-builds/{id}/materials/new/import"),
            import
        )
    );
    let reference = imported["materials"][1]["published_book_ref"].clone();
    assert_ne!(reference, json!(f.y));
    f.ok(
        &f.b,
        "POST",
        &format!("/api/admin/operator-builds/{id}/finish"),
        json!({"host_session_id":"operator-host","state":"ready"}),
    );
    let mut release = super::s1_workspace_publication_tests::publication(&f, "unused", 0);
    release.as_object_mut().unwrap().remove("operation_id");
    release["manifest"]["materials"][1]["published_book_ref"] = reference.clone();
    release["manifest"]["topic_links"] = json!([]);
    release["manifest"]["suggested_route"] = json!([]);
    let publish = json!({"operation_id":"b1-publish","release":release});
    let published = f.ok(
        &f.b,
        "POST",
        &format!("/api/admin/operator-builds/{id}/publish"),
        publish.clone(),
    );
    assert_eq!(
        published["workspace_release"]["availability"]["status"],
        "ready"
    );
    assert_eq!(
        published,
        f.ok(
            &f.b,
            "POST",
            &format!("/api/admin/operator-builds/{id}/publish"),
            publish
        )
    );
    for (token, operation) in [(&f.a, "A-use"), (&f.b, "B-use")] {
        let used = f.ok(
            token,
            "POST",
            "/api/workspace-releases/exam-course/versions/1/use",
            json!({"operation_id":operation,"goal":{"text":"我的目标"}}),
        );
        assert_eq!(used["workspace"]["members"].as_array().unwrap().len(), 2);
    }
    let receipt = json!({"call_id":"b1-call","status":"confirmed","provider_cost_micro_cny":75,"evidence":{"source":"invoice"},"succeeded":true});
    f.ok(
        &f.b,
        "POST",
        &format!("/api/admin/build-tasks/{id}/receipts"),
        receipt.clone(),
    );
    f.ok(
        &f.b,
        "POST",
        &format!("/api/admin/build-tasks/{id}/receipts"),
        receipt,
    );
    let costs = f.ok(&f.b, "GET", "/api/admin/cost-summary", Value::Null);
    assert_eq!(costs["platform"]["request_count"], 1);
    assert_eq!(costs["platform"]["confirmed_cost_micro_cny"], 75);
    assert_eq!(costs["account"]["request_count"], 0);
}

#[test]
fn b1_requires_admin_exact_plan_state_revision_and_actual_task_outputs() {
    let f = fixture();
    let id = "b1-guards";
    let input = create(&f, id);
    assert_eq!(
        f.call(&f.a, "POST", "/api/admin/operator-builds", input.clone())
            .0,
        403
    );
    f.ok(&f.b, "POST", "/api/admin/operator-builds", input);
    assert_eq!(
        f.call(
            &f.b,
            "POST",
            &format!("/api/admin/operator-builds/{id}/start"),
            json!({"operation_id":"early","expected_revision":0})
        )
        .0,
        409
    );
    prepare(&f, id);
    assert_eq!(
        f.call(
            &f.b,
            "POST",
            &format!("/api/admin/operator-builds/{id}/start"),
            json!({"operation_id":"stale","expected_revision":0})
        )
        .0,
        409
    );
    transition(&f, id, "start");
    f.ok(
        &f.b,
        "POST",
        "/api/admin/operator-builds/next",
        json!({"host_session_id":"operator-host"}),
    );
    assert_eq!(
        f.call(
            &f.b,
            "POST",
            &format!("/api/admin/operator-builds/{id}/finish"),
            json!({"host_session_id":"operator-host","state":"ready"})
        )
        .0,
        409
    );
    assert_eq!(
        f.call(
            &f.b,
            "POST",
            &format!("/api/admin/operator-builds/{id}/materials/new/progress"),
            json!({"host_session_id":"other-host","state":"running"})
        )
        .0,
        409
    );
    let another = crate::control_store::ControlStore::open(f.control.writer.clone()).unwrap();
    assert_eq!(
        another
            .connection
            .query_row("SELECT count(*) FROM operator_builds", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
}
