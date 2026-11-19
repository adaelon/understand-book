use super::mu5_tests::Fixture;
use serde_json::{json, Value};

fn publish(f: &Fixture, version: u32) -> Value {
    let mut course: Value = serde_json::from_str(include_str!(
        "../../../../docs/examples/inquiry-course/ai-infra-resource-bounds.v1.course.json"
    ))
    .unwrap();
    let book = f.access.library.lock().unwrap().load("A", &f.x).unwrap();
    // Synthetic fixture proves transport/storage. Source-content acceptance uses
    // the real AI Infra publication separately, not these fixture paragraphs.
    course["lessons"][0]["summary"]["source"] = json!({"publication":f.x,"source_revision":book.book.source_fingerprint(),"start_lid":"1.1","end_lid":"1.3","label":"课程流程测试来源","structure_refs":["fixture"]});
    course["lessons"][0]["assessment"]["source_lids"] = json!(["1.2"]);
    if version > 0 {
        course["lessons"][0]["solution"] = json!("新版解答，不可替换已开始的题目");
    }
    let mut input = super::s1_workspace_publication_tests::publication(
        f,
        &format!("inquiry-v{}", version + 1),
        version,
    );
    input["manifest"]["inquiry_course"] = course;
    f.ok(&f.b, "POST", "/api/admin/workspace-releases", input)
}

#[test]
fn inquiry_course_chat_freezes_visible_work_and_preserves_course_help() {
    let mut f = Fixture::new();
    f.control.set_reader_admin("B", true).unwrap();
    publish(&f, 0);
    let used=f.ok(&f.a,"POST","/api/workspace-releases/exam-course/versions/1/use",json!({"operation_id":"course-chat-use","goal":{"text":"学习资源下界","constraints":[]}}));
    let scope=used["workspace"]["learning_workspace_id"].as_str().unwrap();
    let path=format!("/api/learning-workspaces/{scope}/shared-release/course");
    let start=f.ok(&f.a,"POST",&path,json!({"operation_id":"start-chat","run_id":null,"expected_revision":0,"release_version":1,"action":{"kind":"start","lesson_id":"compute-or-bandwidth"}}));
    let run=start["progress"]["run_id"].as_str().unwrap();
    let current=f.ok(&f.a,"POST",&path,json!({"operation_id":"source-chat","run_id":run,"expected_revision":start["progress"]["revision"],"release_version":1,"action":{"kind":"source"}}));
    let publication=f.access.library.lock().unwrap().load("A",&f.x).unwrap();
    let mut scene=crate::reader_workspace::ReaderWorkspace::local(publication.directory().to_path_buf(),publication.book.clone(),reader::Reader::new(&publication.book,reader::DEFAULT_RADIUS),vec![],None);
    scene.publication=Some(publication.clone());
    scene.learning_workspace_id=Some(scope.into());
    let handle=f.access.users.lock().unwrap().get("A","now").unwrap();
    let mut user=handle.lock().unwrap();
    let request=json!({"message":"原文说的下界是什么意思？","question_anchor_lid":"1.1","course_context":{"run_id":run,"revision":current["progress"]["revision"],"surface":"source"}});
    let input=crate::validate_agent_input(&mut user,&scene,&request.to_string()).unwrap_or_else(|reply|panic!("{:?}",reply.body));
    let frozen=&input.request["course_context"];
    assert_eq!(frozen["lesson"]["progress"]["stage"],"introduction");
    assert!(frozen["lesson"]["solution"].is_null() && frozen["lesson"]["task"].is_null());
    assert!(frozen["prior_help"].as_array().unwrap().len() >= 2);
    assert!(input.agent_message.contains("Prepared course at submission"));
    assert_eq!(input.display_user,"原文说的下界是什么意思？");
    let context=crate::PrivateBookContext { learning_workspace_id:Some(scope),user:&user,book:&publication.book,book_dir:publication.directory(),messages:&[],selected_chat:None };
    let turn=crate::AgentTurnRef { session_id:"course-chat".into(),turn_id:"course-chat-turn".into(),user_turn_ordinal:1 };
    let prepared=crate::teaching::freeze_prepare(&context,&turn,&input.request,None,"now").unwrap();
    assert_eq!(prepared.context.as_ref().unwrap()["inquiry_course"],*frozen);
    assert_eq!(prepared.context.as_ref().unwrap()["status"],"course");
    assert_eq!(prepared.context.as_ref().unwrap()["user_intent"],start["summary"]["objective"]);
    let fact=prepared.events.iter().find(|e|e.payload["usage"]["kind"]=="user_message").unwrap();
    assert_eq!(fact.binding.tutor_session_id,run);
    assert!(!fact.payload["assistance_refs"].as_array().unwrap().is_empty());
    assert!(fact.payload["support_delivery_refs"].as_array().unwrap().len() >= 2);
    prepared.resume(&user).unwrap();
    assert!(crate::teaching::turn_active(&context,&turn.turn_id).unwrap());
    assert!(crate::teaching::step(&context,&turn,json!({"operation":"design","operation_id":"competing-plan"}),&[],&[]).is_err());
    let state=user.learning_store_for_workspace(Some(scope)).unwrap().state().unwrap();
    assert!(!state.control.enabled && state.sessions.is_empty(),"course entry must not change the global switch or create another session");
    // The next same-course reply restores purpose from the delivered history
    // and the actual binding; normalized admission fields omit course context.
    let question="只减少较快的一项，下界会变化吗？";
    let purpose=json!({"focus":"资源下界","reason":"接着已有比较改变一项","learner_inference":"较慢项仍限制下界","evidence_to_notice":"保留原比较并说明未变的约束","basis_refs":[fact.event_id]});
    let mut delivered=super::tutor_loop_tests::outcome(None);
    delivered.answer=Some(format!("先比较两项必要时间。\n\n{question}"));
    delivered.answer_view=Some(serde_json::from_value(json!({"parts":[{"kind":"markdown","text":"先比较两项必要时间。\n\n"},{"kind":"socratic_question","text":question,"purpose":purpose}],"sources":[]})).unwrap());
    user.agent_history.sessions.push(serde_json::from_value(json!({"id":turn.session_id,"learning_workspace_id":scope,"book_id":scene.book.base.book_id,"title":"课程接续","created_at":"now","updated_at":"now","messages":[],
        "turns":[{"turn_id":turn.turn_id,"user_turn_ordinal":1,"user":"原文说的下界是什么意思？","status":"completed","published_book_ref":scene.publication.as_ref().unwrap().reference,"outcome":delivered}]})).unwrap());
    scene.selected_chat=Some(turn.session_id.clone());
    let follow=crate::validate_agent_input(&mut user,&scene,&request.to_string()).unwrap_or_else(|reply|panic!("{}",reply.body));
    assert_eq!(follow.request["socratic_dialogue"]["question"],question);
    assert_eq!(follow.request["socratic_dialogue"]["purpose"],purpose);
    assert_eq!(follow.request["socratic_dialogue"]["course_run_id"],run);
    let context=crate::PrivateBookContext { learning_workspace_id:Some(scope),user:&user,book:&scene.book,book_dir:&scene.book_dir,messages:&[],selected_chat:scene.selected_chat.as_deref() };
    let follow_turn=crate::AgentTurnRef{session_id:turn.session_id.clone(),turn_id:"course-purpose-reply".into(),user_turn_ordinal:2};
    let prepared_follow=crate::teaching::freeze_prepare(&context,&follow_turn,&follow.request,None,"now").unwrap();
    assert_eq!(prepared_follow.context.as_ref().unwrap()["socratic_dialogue"]["purpose"],purpose);
    assert_eq!(prepared_follow.context.as_ref().unwrap()["status"],"course");
    let outside=crate::validate_agent_input(&mut user,&scene,&json!({"message":"离开课程，聊另一个问题"}).to_string()).unwrap_or_else(|reply|panic!("{}",reply.body));
    assert!(outside.request["socratic_dialogue"].is_null());
    // The browser cannot attach a future experiment or another book's course.
    let mut bad=request["course_context"].clone();
    bad["exploration"]=json!({"work_gflops":600,"read_gb":16,"compute_tflops":120,"bandwidth_tb_s":2,"target_ms":9});
    assert!(crate::inquiry_course::freeze_chat(&user,&scene,&bad).is_err());
    scene.publication=Some(f.access.library.lock().unwrap().load("A",&f.y).unwrap());
    assert!(crate::inquiry_course::freeze_chat(&user,&scene,&request["course_context"]).is_err());
    scene.publication=Some(publication);
    scene.learning_workspace_id=None;
    assert!(crate::inquiry_course::freeze_chat(&user,&scene,&request["course_context"]).is_err());
    scene.learning_workspace_id=Some(scope.into());
    drop(user);
    let mut revision=current["progress"]["revision"].clone();
    for (action,status,active) in [("pause","observing",false),("resume","course",true),("skip","observing",false)] {
        let next=f.ok(&f.a,"POST",&path,json!({"operation_id":format!("{action}-chat"),"run_id":run,"expected_revision":revision,"release_version":1,"action":{"kind":action}}));
        revision=next["progress"]["revision"].clone();
        assert_eq!(next["progress"]["stage"],"introduction");
        let user=handle.lock().unwrap();
        let frozen_course=crate::inquiry_course::freeze_chat(&user,&scene,&request["course_context"]).unwrap().unwrap();
        let context=crate::PrivateBookContext { learning_workspace_id:Some(scope),user:&user,book:&scene.book,book_dir:&scene.book_dir,messages:&[],selected_chat:None };
        assert_eq!(crate::teaching::turn_active(&context,&turn.turn_id).unwrap(),active);
        let next_turn=crate::AgentTurnRef { session_id:turn.session_id.clone(),turn_id:format!("{action}-turn"),user_turn_ordinal:2 };
        let input=json!({"message":"我还能问刚才那句话吗？","course_context":frozen_course});
        let prepared=crate::teaching::freeze_prepare(&context,&next_turn,&input,None,"now").unwrap();
        assert_eq!(prepared.context.as_ref().unwrap()["status"],status);
        assert_eq!(prepared.context.as_ref().unwrap()["binding"]["tutor_session_id"],run);
        assert!(!prepared.context.as_ref().unwrap()["inquiry_course"]["prior_help"].as_array().unwrap().is_empty());
    }
    assert_eq!(frozen["lesson"]["progress"]["stage"],"introduction");
}

#[test]
fn inquiry_course_api_keeps_private_answers_frozen_runs_and_owner_scope() {
    let mut f = Fixture::new();
    f.control.set_reader_admin("B", true).unwrap();
    publish(&f, 0);
    let public = f.ok(
        &f.a,
        "GET",
        "/api/workspace-releases/exam-course/versions/1",
        Value::Null,
    );
    assert!(public["manifest"]["inquiry_course"]["lessons"][0]
        .get("question")
        .is_some());
    for path in [
        "/api/workspace-releases",
        "/api/workspace-releases/exam-course/versions/1/export",
    ] {
        let value = f.ok(&f.a, "GET", path, Value::Null).to_string();
        assert!(!value.contains("read-bound") && !value.contains("correct\""));
    }
    let used = f.ok(
        &f.a,
        "POST",
        "/api/workspace-releases/exam-course/versions/1/use",
        json!({"operation_id":"use-inquiry","goal":{"text":"学习资源下界","constraints":[]}}),
    );
    let workspace = used["workspace"]["learning_workspace_id"].as_str().unwrap();
    let path = format!("/api/learning-workspaces/{workspace}/shared-release/course");
    let catalog = f.ok(&f.a, "GET", &path, Value::Null);
    assert!(catalog["progress"].as_array().unwrap().is_empty());
    assert_eq!(f.call(&f.b, "GET", &path, Value::Null).0, 404);
    let mut v=f.ok(&f.a,"POST",&path,json!({"operation_id":"start","run_id":null,"expected_revision":0,"release_version":1,"action":{"kind":"start","lesson_id":"compute-or-bandwidth"}}));
    assert!(v["experiment"].is_null() && v["task"].is_null() && v["solution"].is_null());
    let initial = v.clone();
    let run = v["progress"]["run_id"].as_str().unwrap().to_owned();
    let source_path = format!("{path}?run_id={run}&source=1&include_outline=false");
    assert_eq!(f.call(&f.a, "GET", &source_path, Value::Null).0, 404);
    v = f.ok(&f.a, "POST", &path, json!({
        "operation_id": "read-source", "run_id": run, "expected_revision": v["progress"]["revision"],
        "release_version": 1, "action": {"kind": "source"}
    }));
    let window = f.ok(&f.a, "GET", &source_path, Value::Null);
    assert_eq!(window["published_book_ref"], json!(f.x));
    assert_eq!(window["focus_lid"], "1.1");
    assert_eq!(f.call(&f.b, "GET", &source_path, Value::Null).0, 404);
    assert_eq!(f.ok(&f.a, "GET", &format!("{path}?run_id={run}"), Value::Null)["progress"], v["progress"]);
    publish(&f, 1);
    let preview=f.ok(&f.a,"GET",&format!("/api/learning-workspaces/{workspace}/shared-release/preview?version=2"),Value::Null);
    assert_eq!(preview["changes"]["composition_changed"],true);
    f.ok(
        &f.a,
        "POST",
        &format!("/api/learning-workspaces/{workspace}/shared-release/update"),
        json!({"operation_id":"update","expected_version":1,"version":2}),
    );
    assert_eq!(f.ok(&f.a, "GET", &path, Value::Null)["release_version"], 2);
    let start = json!({"operation_id":"start","run_id":null,"expected_revision":0,"release_version":1,"action":{"kind":"start","lesson_id":"compute-or-bandwidth"}});
    assert_eq!(initial, f.ok(&f.a, "POST", &path, start.clone()));
    let mut stale = start;
    stale["operation_id"] = json!("stale-start");
    assert_eq!(
        f.call(&f.a, "POST", &path, stale).1["error_code"],
        "COURSE_REVISION_CONFLICT"
    );
    for (id, action) in [
        ("next", json!({"kind":"advance"})),
        (
            "predict",
            json!({"kind":"predict","response":{"choice":"compute","reason":"先猜算力"}}),
        ),
        ("base", json!({"kind":"observe","preset":"baseline"})),
        (
            "compute",
            json!({"kind":"observe","preset":"double_compute"}),
        ),
        (
            "bandwidth",
            json!({"kind":"observe","preset":"double_bandwidth"}),
        ),
        ("explain", json!({"kind":"advance"})),
        ("task", json!({"kind":"advance"})),
        ("hint", json!({"kind":"hint"})),
        (
            "answer",
            json!({"kind":"answer","response":{"choice":"sum","reason":"误以为顺序执行"}}),
        ),
        ("complete", json!({"kind":"advance"})),
    ] {
        let input = json!({"operation_id":id,"run_id":run,"expected_revision":v["progress"]["revision"],"release_version":1,"action":action});
        v = f.ok(&f.a, "POST", &path, input.clone());
        assert_eq!(v, f.ok(&f.a, "POST", &path, input));
    }
    assert_eq!(v["progress"]["status"], "completed");
    assert_eq!(v["assessment_status"], "incorrect");
    assert_eq!(v["progress"]["release_version"], 1);
    assert!(!v["solution"].as_str().unwrap().contains("新版"));
    let mut read = f.ok(&f.a, "GET", &format!("{path}?run_id={run}"), Value::Null);
    assert_eq!(read["reviews"].as_array().unwrap().len(), 2);
    read["reviews"] = json!([]); // Live background state is not part of an operation receipt.
    assert_eq!(v, read);
    f.access.library.lock().unwrap().revoke("A", &f.x).unwrap();
    assert_ne!(
        f.call(&f.a, "GET", &format!("{path}?run_id={run}"), Value::Null)
            .0,
        200
    );
}

#[test]
fn inquiry_course_publish_rejects_broken_source_and_answer_contract() {
    let mut f = Fixture::new();
    f.control.set_reader_admin("B", true).unwrap();
    let published = publish(&f, 0);
    for (id, change) in [("source", 0), ("criteria", 1)] {
        let mut input = super::s1_workspace_publication_tests::publication(&f, id, 1);
        let mut course = published["workspace_release"]["manifest"]["inquiry_course"].clone();
        if change == 0 {
            course["lessons"][0]["summary"]["source"]["start_lid"] = json!("missing");
        } else {
            course["lessons"][0]["assessment"]["rule"]["correct"] = json!(["missing"]);
        }
        input["manifest"]["inquiry_course"] = course;
        assert_ne!(
            f.call(&f.b, "POST", "/api/admin/workspace-releases", input)
                .0,
            200
        );
    }
}

#[test]
#[ignore = "actual local service for inquiry course browser acceptance; COURSE_BOOK_DIR selects the public source"]
fn inquiry_course_browser_host() {
    use std::{
        sync::Arc,
        time::{Duration, Instant},
    };
    let mut f = Fixture::new();
    f.control.set_reader_admin("B", true).unwrap();
    if let Ok(rates) = std::env::var("COURSE_MODEL_RATES") {
        let config = runtime::ProviderConfig::from_env().unwrap();
        std::fs::copy(rates, f.root.path().join("model-rates.json")).unwrap();
        Arc::get_mut(&mut f.access).unwrap().spend = Arc::new(crate::model_spend_store::ModelSpendStore::new(
            crate::control_store::ControlStore::open(f.control.writer.clone()).unwrap()));
        f.access.spend.validate_provider(&config.base_url, &config.model).unwrap();
        Arc::get_mut(&mut f.access).unwrap().runs.observability = crate::observability::ObservabilityRuntime::from_env();
        f.access.runs.configure(config).unwrap();
        let now = crate::multi_user_host::now();
        let period = f.ok(&f.b,"POST","/api/admin/users/A/allowance-periods",json!({"operation_id":"course-preview-period","starts_at":now-100,"expires_at":now+43200}))["allowance_period"]["period_id"].as_str().unwrap().to_owned();
        f.ok(&f.b,"POST","/api/admin/users/A/allowance-adjustments",json!({"operation_id":"course-preview-grant","period_id":period,"revision":0,"delta_micro_cny":5_000_000,"reason":"隔离样课试用额度；无真实付款"}));
    }
    let source = std::env::var("COURSE_BOOK_DIR")
        .expect("COURSE_BOOK_DIR must identify the actual public book directory");
    let publication = f
        .access
        .library
        .lock()
        .unwrap()
        .publish_online(&crate::online_publication::PublicationRequest {
            operation_id: "inquiry-source".into(),
            input_directory: std::path::PathBuf::from(source),
            grant_users: vec!["A".into(), "B".into()],
        })
        .unwrap();
    let course_source = std::env::var("COURSE_FILE")
        .map(|path| std::fs::read_to_string(path).expect("COURSE_FILE must identify a course fixture"))
        .unwrap_or_else(|_| include_str!("../../../../docs/examples/inquiry-course/ai-infra-resource-bounds.course.json").to_owned());
    let mut course: Value = serde_json::from_str(&course_source).unwrap();
    for lesson in course["lessons"].as_array_mut().unwrap() {
        lesson["summary"]["source"]["publication"] = json!(publication.reference);
        lesson["summary"]["source"]["source_revision"] = json!(publication.source_fingerprint);
    }
    let mut input = super::s1_workspace_publication_tests::publication(&f, "inquiry-browser", 0);
    input["manifest"]["name"] = json!("AI Infra · 资源升级决策课");
    input["manifest"]["public_goal"] = json!("用资源模型判断计算与读取的时间下界");
    input["manifest"]["materials"] =
        json!([{"published_book_ref":publication.reference,"required_capabilities":["text"]}]);
    input["manifest"]["suggested_route"] = json!([]);
    input["manifest"]["topic_links"] = json!([]);
    input["manifest"]["inquiry_course"] = course;
    f.ok(&f.b, "POST", "/api/admin/workspace-releases", input);
    let used=f.ok(&f.a,"POST","/api/workspace-releases/exam-course/versions/1/use",json!({"operation_id":"inquiry-browser-use","goal":{"text":"用资源模型判断计算与读取的时间下界","constraints":[]}}));
    let origin = "https://127.0.0.1:4260";
    let server = crate::multi_user_host::start_with_access(
        "127.0.0.1:4260".parse().unwrap(),
        Arc::new(crate::multi_user_host::Site::new(origin).unwrap()),
        f.access.clone(),
    )
    .unwrap();
    let control = tiny_http::Server::http("127.0.0.1:4261").unwrap();
    println!("INQUIRY_COURSE_BROWSER_READY");
    let until = Instant::now() + Duration::from_secs(43200);
    while Instant::now() < until {
        let Some(request) = control.recv_timeout(Duration::from_secs(1)).unwrap() else {
            continue;
        };
        if request.url() == "/stop" {
            request
                .respond(tiny_http::Response::from_string("stopped"))
                .unwrap();
            break;
        }
        let fixture = json!({"origin":origin,"reader_cookie":format!("{}={}",crate::auth::COOKIE,f.a),"reader_csrf":crate::auth::csrf(&f.a),"new_reader_cookie":format!("{}={}",crate::auth::COOKIE,f.b),"new_reader_csrf":crate::auth::csrf(&f.b),"workspace":used["workspace"],"reference":publication.reference,"storage_path":f.root.path()});
        request
            .respond(
                tiny_http::Response::from_string(fixture.to_string()).with_header(
                    tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap(),
                ),
            )
            .unwrap();
    }
    server.shutdown();
    // This is a human tryout: keep its real private learning records after shutdown.
    let _ = f.root.keep();
}
