use super::*;
use base_schema::PublishedBookRef;

fn shared_fixture() -> Fixture {
    let mut f = Fixture::new();
    f.control.set_reader_admin("B", true).unwrap();
    f
}
fn publish(f: &Fixture, operation: &str, previous: u32, materials: &[PublishedBookRef]) -> Value {
    let mut input =
        super::super::s1_workspace_publication_tests::publication(f, operation, previous);
    input["manifest"]["materials"] = json!(materials
        .iter()
        .map(|r| json!({"published_book_ref":r,"required_capabilities":["text"]}))
        .collect::<Vec<_>>());
    input["manifest"]["topic_links"][0]["materials"] = json!(materials);
    input["manifest"]["suggested_route"][0]["materials"] = json!(materials);
    input["manifest"]["public_goal"] = json!(format!("公共目标版本 {}", previous + 1));
    f.ok(&f.b, "POST", "/api/admin/workspace-releases", input)
}
fn use_release(f: &Fixture, token: &str, operation: &str, goal: &str) -> Value {
    f.ok(
        token,
        "POST",
        "/api/workspace-releases/exam-course/versions/1/use",
        json!({"operation_id":operation,"goal":{"text":goal,"constraints":["保留个人范围"]}}),
    )
}
fn instance_url(result: &Value) -> String {
    format!(
        "/api/learning-workspaces/{}/shared-release",
        result["workspace"]["learning_workspace_id"]
            .as_str()
            .unwrap()
    )
}
fn scene(f: &Fixture, token: &str, workspace: &Value, attachment: &str) -> Value {
    let scene = f.ok(token,"POST","/api/workspaces",json!({"published_book_ref":f.x,"attachment_id":attachment,"learning_workspace_id":workspace["workspace"]["learning_workspace_id"]}));
    f.action(token, &scene, attachment, "chat/new", json!({}))
}

#[test]
fn s2_two_users_confirm_private_goals_reuse_public_results_and_keep_chat_memory_separate() {
    let f = shared_fixture();
    let probe = Probe::default();
    configure(&f, &probe);
    publish(&f, "s2-public", 0, &[f.x.clone(), f.y.clone()]);
    let before: i64 = f
        .control
        .connection
        .query_row("SELECT count(*) FROM model_call_charges", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        f.call(
            &f.a,
            "POST",
            "/api/workspace-releases/exam-course/versions/1/use",
            json!({"operation_id":"blank","goal":{"text":"  "}})
        )
        .1["error_code"],
        "WORKSPACE_GOAL_REQUIRED"
    );
    let a = use_release(&f, &f.a, "s2-use", "我只复习速度和变化");
    let b = use_release(&f, &f.b, "s2-use", "我准备另一场考试");
    assert_ne!(
        a["workspace"]["learning_workspace_id"],
        b["workspace"]["learning_workspace_id"]
    );
    assert_eq!(a["workspace"]["goal"]["text"], "我只复习速度和变化");
    assert_eq!(
        a["instance"]["release"]["manifest"],
        b["instance"]["release"]["manifest"]
    );
    assert_eq!(a, use_release(&f, &f.a, "s2-use", "我只复习速度和变化"));
    assert_eq!(
        f.call(
            &f.a,
            "POST",
            "/api/workspace-releases/exam-course/versions/1/use",
            json!({"operation_id":"s2-use","goal":{"text":"different"}})
        )
        .1["error_code"],
        "WORKSPACE_INSTANCE_OPERATION_CONFLICT"
    );
    assert_eq!(f.call(&f.b, "GET", &instance_url(&a), Value::Null).0, 404);
    assert_eq!(
        f.control
            .connection
            .query_row("SELECT count(*) FROM model_call_charges", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        before
    );
    assert_eq!(probe.calls.load(Ordering::SeqCst), 0);

    let sa = scene(&f, &f.a, &a, "page");
    let sb = scene(&f, &f.b, &b, "other");
    let mut input = Fixture::stamp(&sa, "page");
    input["session_id"] = sa["selected_chat"].clone();
    input["client_request_id"] = json!("s2-learning");
    input["message"] = json!("我不明白这里的速度");
    assert_eq!(admit(&f, &sa, &input).0, 202);
    f.access.runs.run_one(&f.access).unwrap();
    let history = f.action(&f.b, &sb, "other", "chat/history", json!({}));
    assert!(history["result"]["current"]["turns"]
        .as_array()
        .unwrap()
        .is_empty());
    let view = f.action(&f.a, &f.get(&f.a, &sa), "page", "profile/memory", json!({}));
    let revision = view["result"]["status"]["document_revision"]
        .as_u64()
        .unwrap();
    f.action(&f.a,&f.get(&f.a,&sa),"page","profile/memory/apply",profile_mutation(revision,json!({"kind":"remember","operation_id":"private-learning","evidence_text":"我需要逐步解释速度","fact":profile_fact_draft("book","depth","仅在我的工作区逐步解释速度","normal")})));
    let a_view = f.action(&f.a, &f.get(&f.a, &sa), "page", "profile/memory", json!({}));
    let b_view = f.action(
        &f.b,
        &f.get(&f.b, &sb),
        "other",
        "profile/memory",
        json!({}),
    );
    assert!(a_view["result"]
        .to_string()
        .contains("仅在我的工作区逐步解释速度"));
    assert!(!b_view["result"]
        .to_string()
        .contains("仅在我的工作区逐步解释速度"));
    let f = reopen(f, &probe);
    assert_eq!(
        f.ok(&f.a, "GET", &instance_url(&a), Value::Null)["instance"]["version"],
        1
    );
    assert_eq!(
        f.ok(&f.b, "GET", &instance_url(&b), Value::Null)["instance"]["version"],
        1
    );
}

#[test]
fn s2_explicit_update_preserves_private_goal_and_material_identity_across_overlapping_versions() {
    let f = shared_fixture();
    publish(&f, "s2-v1", 0, std::slice::from_ref(&f.x));
    let a = use_release(&f, &f.a, "s2-a", "我的个人目标");
    let b = use_release(&f, &f.b, "s2-b", "另一人的目标");
    let workspace = a["workspace"]["learning_workspace_id"].as_str().unwrap();
    let path = instance_url(&a);
    f.ok(
        &f.a,
        "POST",
        &format!("/api/learning-workspaces/{workspace}/members"),
        json!({"published_book_ref":f.y}),
    );
    f.ok(
        &f.a,
        "POST",
        &format!("/api/learning-workspaces/{workspace}/goal"),
        json!({"goal":{"text":"后来修改的个人目标","constraints":["只剩三天"]}}),
    );
    publish(&f, "s2-v2", 1, &[f.x.clone(), f.y.clone()]);
    let preview = f.ok(
        &f.a,
        "GET",
        &format!("{path}/preview?version=2"),
        Value::Null,
    );
    assert_eq!(preview["changes"]["added_materials"], json!([f.y]));
    assert_eq!(preview["preserved_private_materials"], json!([f.y]));
    assert_eq!(preview["changes"]["public_goal_changed"], true);
    assert_eq!(
        f.ok(&f.a, "GET", &path, Value::Null)["instance"]["version"],
        1,
        "preview cannot apply a version"
    );
    let input = json!({"operation_id":"select-v2","expected_version":1,"version":2});
    let updated = f.ok(&f.a, "POST", &format!("{path}/update"), input.clone());
    assert_eq!(updated["workspace"]["goal"]["text"], "后来修改的个人目标");
    assert_eq!(
        updated["workspace"]["goal"]["constraints"],
        json!(["只剩三天"])
    );
    assert_eq!(
        updated["workspace"]["goal_history"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        updated,
        f.ok(&f.a, "POST", &format!("{path}/update"), input)
    );
    assert_eq!(
        f.ok(&f.b, "GET", &instance_url(&b), Value::Null)["instance"]["version"],
        1
    );
    publish(&f, "s2-v3", 2, std::slice::from_ref(&f.x));
    let stale = f.call(
        &f.a,
        "POST",
        &format!("{path}/update"),
        json!({"operation_id":"stale","expected_version":1,"version":3}),
    );
    assert_eq!(stale.1["error_code"], "WORKSPACE_INSTANCE_VERSION_CONFLICT");
    let third = f.ok(
        &f.a,
        "POST",
        &format!("{path}/update"),
        json!({"operation_id":"select-v3","expected_version":2,"version":3}),
    );
    assert!(
        third["workspace"]["members"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["published_book_ref"] == json!(f.y)),
        "explicitly added material survives after a later public version removes it"
    );
    assert_eq!(third["instance"]["history"].as_array().unwrap().len(), 3);
    // An explicit personal removal also removes the private retention intent.
    f.ok(
        &f.a,
        "DELETE",
        &format!("/api/learning-workspaces/{workspace}/members"),
        json!({"published_book_ref":f.y}),
    );
    publish(&f, "s2-v4", 3, &[f.x.clone(), f.y.clone()]);
    f.ok(
        &f.a,
        "POST",
        &format!("{path}/update"),
        json!({"operation_id":"select-v4","expected_version":3,"version":4}),
    );
    publish(&f, "s2-v5", 4, std::slice::from_ref(&f.x));
    let fifth = f.ok(
        &f.a,
        "POST",
        &format!("{path}/update"),
        json!({"operation_id":"select-v5","expected_version":4,"version":5}),
    );
    assert_eq!(fifth["workspace"]["members"].as_array().unwrap().len(), 1);
}

#[test]
fn s2_source_bindings_and_chat_journal_keep_the_original_material_after_selected_update() {
    let f = shared_fixture();
    publish(&f, "s2-source-v1", 0, &[f.x.clone(), f.y.clone()]);
    let a = use_release(&f, &f.a, "s2-source-use", "保留学习来源");
    let b = use_release(&f, &f.b, "s2-other-source-use", "另一人的理解");
    let scene = scene(&f, &f.a, &a, "page");
    let quote = f
        .access
        .library
        .lock()
        .unwrap()
        .load("A", &f.x)
        .unwrap()
        .book
        .text("1.1", None)
        .unwrap();
    let probe = Probe {
        primary_quote: Some(quote),
        ..Default::default()
    };
    configure(&f, &probe);
    let mut input = Fixture::stamp(&scene, "page");
    input["session_id"] = scene["selected_chat"].clone();
    input["client_request_id"] = json!("s2-source-turn");
    input["message"] = json!("说明这段原文");
    assert_eq!(admit(&f, &scene, &input).0, 202);
    f.access.runs.run_one(&f.access).unwrap();
    let (path, bytes, source, turn) = {
        let handle = context(&f, &f.a).user;
        let user = handle.lock().unwrap();
        let session = user
            .agent_history
            .sessions
            .iter()
            .find(|s| Some(s.id.as_str()) == scene["selected_chat"].as_str())
            .unwrap();
        let turn = session.turns.last().unwrap();
        let source = turn.source_bindings[0].clone();
        let path = user
            .session_store
            .as_ref()
            .unwrap()
            .paths
            .session(&session.id);
        (
            path.clone(),
            std::fs::read(path).unwrap(),
            source,
            turn.turn_id.clone(),
        )
    };
    let (evidence_ref, evidence) = {
        let handle = context(&f, &f.a).user;
        let user = handle.lock().unwrap();
        let publication = f.access.library.lock().unwrap().load("A", &f.x).unwrap();
        let session = user
            .agent_history
            .sessions
            .iter()
            .find(|s| Some(s.id.as_str()) == scene["selected_chat"].as_str())
            .unwrap();
        let turn = session.turns.last().unwrap();
        let turn_ref = AgentTurnRef {
            session_id: session.id.clone(),
            turn_id: turn.turn_id.clone(),
            user_turn_ordinal: turn.user_turn_ordinal,
        };
        crate::cross_book::record_delivery(
            &user,
            session,
            turn,
            turn.outcome.as_ref().unwrap(),
            "s2-delivery",
        )
        .unwrap();
        let private = PrivateBookContext {
            user: &user,
            learning_workspace_id: session.learning_workspace_id.as_deref(),
            book: &publication.book,
            book_dir: publication.directory(),
            messages: &session.messages,
            selected_chat: Some(&session.id),
        };
        let observation = crate::cross_book::learning(&private,&turn_ref,&f.x,&json!({"operation":"observe","operation_id":"s2-understanding","learner_quote":turn.user_authored_text(),"learning_focus":"理解速度","expected_performance":"解释原文关系","capability":"explanation","source_ref_ids":[source.source_ref_id],"interpretation":"用户请求解释速度的原文","teaching_implication":"先解释原表达"}),std::slice::from_ref(&source),"observed").unwrap();
        let id = observation["evidence_ref"].as_str().unwrap().to_owned();
        let evidence = json!(user
            .learning_store_for_workspace(session.learning_workspace_id.as_deref())
            .unwrap()
            .evidence(&id)
            .unwrap());
        (id, evidence)
    };
    {
        let handle = context(&f, &f.b).user;
        let user = handle.lock().unwrap();
        assert!(user
            .learning_store_for_workspace(b["workspace"]["learning_workspace_id"].as_str())
            .unwrap()
            .evidence(&evidence_ref)
            .is_err());
    }
    publish(&f, "s2-source-v2", 1, std::slice::from_ref(&f.y));
    let updated = f.ok(
        &f.a,
        "POST",
        &format!("{}/update", instance_url(&a)),
        json!({"operation_id":"s2-source-update","expected_version":1,"version":2}),
    );
    assert_eq!(updated["workspace"]["members"].as_array().unwrap().len(), 1);
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert_eq!(
        source.publication_id.as_deref(),
        Some(f.x.publication_id.as_str())
    );
    {
        let handle = context(&f, &f.a).user;
        let user = handle.lock().unwrap();
        assert_eq!(
            json!(user
                .learning_store_for_workspace(a["workspace"]["learning_workspace_id"].as_str())
                .unwrap()
                .evidence(&evidence_ref)
                .unwrap()),
            evidence
        );
    }
    let moved = f.action(
        &f.a,
        &f.get(&f.a, &scene),
        "page",
        "book/open",
        json!({"published_book_ref":f.y}),
    );
    let resolved = f.action(
        &f.a,
        &moved,
        "page",
        "agent/source.resolve",
        json!({"turn_id":turn,"source_ref_id":source.source_ref_id}),
    );
    assert!(resolved["result"].to_string().contains(&f.x.publication_id));
    let f = reopen(f, &probe);
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert_eq!(
        f.ok(&f.a, "GET", &instance_url(&a), Value::Null)["instance"]["history"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn s2_use_and_update_require_real_access_and_only_offer_visible_versions() {
    let f = shared_fixture();
    publish(&f, "s2-access-v1", 0, &[f.x.clone(), f.y.clone()]);
    f.access.library.lock().unwrap().revoke("A", &f.y).unwrap();
    assert_eq!(
        f.call(
            &f.a,
            "POST",
            "/api/workspace-releases/exam-course/versions/1/use",
            json!({"operation_id":"s2-rejected","goal":{"text":"不能借入口获取授权"}})
        )
        .1["error_code"],
        "WORKSPACE_RELEASE_NOT_READY"
    );
    assert_eq!(
        f.ok(&f.a, "GET", "/api/learning-workspaces", Value::Null)["workspaces"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    f.access.library.lock().unwrap().grant("A", &f.y).unwrap();
    let a = use_release(&f, &f.a, "s2-access-use", "保留授权范围");
    let mut v2 = super::super::s1_workspace_publication_tests::publication(&f, "s2-hidden-v2", 1);
    v2["visible_to_users"] = json!(["B"]);
    f.ok(&f.b, "POST", "/api/admin/workspace-releases", v2);
    assert_eq!(
        f.ok(&f.a, "GET", &instance_url(&a), Value::Null)["instance"]["latest_version"],
        1
    );
    assert_eq!(
        f.call(
            &f.a,
            "GET",
            &format!("{}/preview?version=2", instance_url(&a)),
            Value::Null
        )
        .0,
        404
    );
    publish(&f, "s2-access-v3", 2, &[f.x.clone(), f.y.clone()]);
    f.access.library.lock().unwrap().revoke("A", &f.y).unwrap();
    let failed = f.call(
        &f.a,
        "POST",
        &format!("{}/update", instance_url(&a)),
        json!({"operation_id":"s2-rejected-update","expected_version":1,"version":3}),
    );
    assert_eq!(failed.1["error_code"], "WORKSPACE_RELEASE_NOT_READY");
    assert_eq!(
        f.ok(&f.a, "GET", &instance_url(&a), Value::Null)["instance"]["version"],
        1
    );
    assert_ne!(
        f.call(
            &f.a,
            "GET",
            &format!("{}?lid=1.1", f.y.url("text")),
            Value::Null
        )
        .0,
        200
    );
    let ordinary = f.ok(
        &f.a,
        "POST",
        "/api/learning-workspaces",
        json!({"goal":{"text":"普通工作区"}}),
    );
    assert!(f.ok(
        &f.a,
        "GET",
        &format!(
            "/api/learning-workspaces/{}/shared-release",
            ordinary["learning_workspace_id"].as_str().unwrap()
        ),
        Value::Null
    )["instance"]
        .is_null());
}

#[test]
fn s2_shared_archive_stops_new_use_and_keeps_existing_instances_and_receipts() {
    let f = shared_fixture();
    publish(&f, "s2-archive-v1", 0, &[f.x.clone(), f.y.clone()]);
    let a = use_release(&f, &f.a, "s2-existing", "现有学习继续");
    let input = json!({"operation_id":"archive-shared","archived":true});
    assert_eq!(
        f.call(
            &f.a,
            "POST",
            "/api/admin/workspace-releases/exam-course/archive",
            input.clone()
        )
        .0,
        403
    );
    let receipt = f.ok(
        &f.b,
        "POST",
        "/api/admin/workspace-releases/exam-course/archive",
        input.clone(),
    );
    assert_eq!(
        receipt,
        f.ok(
            &f.b,
            "POST",
            "/api/admin/workspace-releases/exam-course/archive",
            input
        )
    );
    assert_eq!(
        f.ok(&f.a, "GET", &instance_url(&a), Value::Null)["instance"]["release"]["archived"],
        true
    );
    assert_eq!(
        f.call(
            &f.a,
            "POST",
            "/api/workspace-releases/exam-course/versions/1/use",
            json!({"operation_id":"new-use","goal":{"text":"新的学习"}})
        )
        .1["error_code"],
        "WORKSPACE_RELEASE_ARCHIVED"
    );
    assert_eq!(
        a["workspace"]["learning_workspace_id"],
        use_release(&f, &f.a, "s2-existing", "现有学习继续")["workspace"]["learning_workspace_id"],
        "retry refers to existing use and does not create a new instance"
    );
    assert_eq!(
        f.call(
            &f.a,
            "GET",
            &format!("{}?lid=1.1", f.x.url("text")),
            Value::Null
        )
        .0,
        200
    );
    let export = f.ok(
        &f.a,
        "GET",
        "/api/workspace-releases/exam-course/versions/1/export",
        Value::Null,
    );
    assert_eq!(export["version"], 1);
    f.ok(
        &f.b,
        "POST",
        "/api/admin/workspace-releases/exam-course/archive",
        json!({"operation_id":"restore-shared","archived":false}),
    );
    assert_eq!(
        f.ok(&f.a, "GET", &instance_url(&a), Value::Null)["instance"]["release"]["archived"],
        false
    );
    let new_use = use_release(&f, &f.a, "new-use", "新的学习");
    assert_ne!(
        a["workspace"]["learning_workspace_id"],
        new_use["workspace"]["learning_workspace_id"]
    );
}

#[test]
fn s2_schema17_upgrade_rolls_back_all_new_tables_and_preserves_old_facts_and_reopened_instances() {
    let f = shared_fixture();
    let release = publish(&f, "s2-before-upgrade", 0, &[f.x.clone(), f.y.clone()]);
    let existing = f.ok(
        &f.a,
        "POST",
        "/api/learning-workspaces",
        json!({"goal":{"text":"升级前个人目标"},"members":[f.x]}),
    );
    f.ok(&f.b,"POST","/api/admin/build-tasks/s2-old-cost/budget",json!({"cost_owner":"platform","max_micro_cny":100,"confirmation":"existing platform task"}));
    f.ok(&f.b,"POST","/api/admin/build-tasks/s2-old-cost/admit",json!({"execution_id":"s2-old-execution","call_id":"s2-old-call","logical_call_id":"s2-old-logical","attempt":1,"granularity":"execution_total","reserved_micro_cny":20,"provider":"fixture","model":"fixture","purpose":"prebuild","budget_enforced":true,"receipt_capability":"complete"}));
    f.ok(&f.b,"POST","/api/admin/build-tasks/s2-old-cost/receipts",json!({"call_id":"s2-old-call","status":"confirmed","provider_cost_micro_cny":12,"evidence":{"fixture":"confirmed"},"succeeded":true}));
    let original_manifest:String = f.control.connection.query_row("SELECT manifest_json FROM workspace_releases WHERE release_id='exam-course' AND version=1",[],|r|r.get(0)).unwrap();
    f.control.connection.execute_batch(include_str!("remove_schema19.sql")).unwrap();
    f.control.connection.execute_batch("DROP TABLE workspace_release_instance_operations; DROP TABLE workspace_release_instance_history; DROP TABLE workspace_release_instance_private_materials; DROP TABLE workspace_release_instances; DROP TABLE workspace_release_archives; DROP TABLE operator_builds; PRAGMA user_version=17; CREATE TABLE operator_builds(original TEXT); INSERT INTO operator_builds VALUES('keep-before-upgrade');").unwrap();
    assert!(ControlStore::open(f.control.writer.clone()).is_err());
    assert_eq!(
        f.control
            .connection
            .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        17
    );
    assert_eq!(f.control.connection.query_row("SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN ('workspace_release_instances','workspace_release_instance_history','workspace_release_instance_operations','workspace_release_instance_private_materials','workspace_release_archives')",[],|r|r.get::<_,i64>(0)).unwrap(),0,"failed B1 table creation rolls back every preceding S2 table");
    assert_eq!(
        f.control
            .connection
            .query_row("SELECT original FROM operator_builds", [], |r| r
                .get::<_, String>(0))
            .unwrap(),
        "keep-before-upgrade"
    );
    f.control
        .connection
        .execute_batch("DROP TABLE operator_builds")
        .unwrap();
    let mut upgraded = ControlStore::open(f.control.writer.clone()).unwrap();
    assert_eq!(
        upgraded
            .connection
            .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        crate::control_store::CONTROL_SCHEMA_VERSION
    );
    assert_eq!(upgraded.connection.query_row("SELECT manifest_json FROM workspace_releases WHERE release_id='exam-course' AND version=1",[],|r|r.get::<_,String>(0)).unwrap(),original_manifest);
    assert_eq!(upgraded.connection.query_row("SELECT provider_cost_micro_cny FROM model_call_charges WHERE call_id='s2-old-call'",[],|r|r.get::<_,i64>(0)).unwrap(),12);
    assert_eq!(
        json!(upgraded
            .learning_workspace("A", existing["learning_workspace_id"].as_str().unwrap())
            .unwrap()),
        existing
    );
    let a = upgraded
        .use_workspace_release(
            "A",
            "exam-course",
            1,
            crate::workspace_release_instances::UseRelease {
                exam_sprint: None,
                operation_id: "s2-after-upgrade".into(),
                goal: base_schema::WorkspaceGoalInput {
                    text: "升级后开始学习".into(),
                    constraints: vec![],
                },
            },
            multi_user_host::now(),
        )
        .unwrap();
    drop(upgraded);
    let mut reopened = ControlStore::open(f.control.writer.clone()).unwrap();
    assert_eq!(
        reopened
            .workspace_release_instance(
                "A",
                a["workspace"]["learning_workspace_id"].as_str().unwrap()
            )
            .unwrap()["instance"],
        a["instance"]
    );
    assert_eq!(
        reopened
            .use_workspace_release(
                "A",
                "exam-course",
                1,
                crate::workspace_release_instances::UseRelease {
                exam_sprint: None,
                    operation_id: "s2-after-upgrade".into(),
                    goal: base_schema::WorkspaceGoalInput {
                        text: "升级后开始学习".into(),
                        constraints: vec![]
                    }
                },
                multi_user_host::now()
            )
            .unwrap()["workspace"]["learning_workspace_id"],
        a["workspace"]["learning_workspace_id"]
    );
    let admin = crate::admin_store::AdminStore::new(reopened);
    assert_eq!(admin.operation("B", "s2-before-upgrade").unwrap(), release);
}
