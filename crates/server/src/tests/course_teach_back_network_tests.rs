use super::*;

fn deliver(f: &Fixture, accepted: &Value, text: &str) -> Value {
    let book = f.access.library.lock().unwrap().load("A", &f.x).unwrap();
    let quote = book.book.text("1.1", None).unwrap();
    let learner = json!({"kind":"explanation","student_graph":{"nodes":[{"id":"s1","label":"比较用时","quotes":[text]}],"edges":[]}});
    let comparison = json!({
        "source":{"source_id":f.x.book_id,"source_revision":book.book.source_fingerprint()},
        "book_graph":{"nodes":[{"id":"p","label":"观测结果"},{"id":"q","label":"当前判断"}],
            "edges":[{"id":"b1","from":"p","to":"q","relation_type":"supports","evidence":[{"lid":"1.1","quote":quote}]}]},
        "rows":[{"row_id":"row","book_edge_ids":["b1"],"book_node_ids":[],"student_edge_ids":[],"student_node_ids":["s1"],
            "status":"unverified_against_passage","summary":"本段还不能核对这个判断。"}],"side_notes":[]
    });
    f.access.runs.configure_fake(move || {
        Box::new(ChatStubAdapter::scripted(vec![
            reader_service::tests::tutor_loop_tests::answer(&learner.to_string()),
            reader_service::tests::tutor_loop_tests::answer(&comparison.to_string()),
        ]))
    });
    assert!(f.access.runs.run_one(&f.access).unwrap());
    let path = format!("/api/agent/runs/{}", accepted["turn_id"].as_str().unwrap());
    let result = f.ok(&f.a, "GET", &path, Value::Null);
    assert_eq!(result["turn"]["status"], "completed", "{result}");
    let card = &result["turn"]["outcome"]["answer_view"]["parts"][0]["comparison"];
    assert_eq!(
        card["student_graph"]["nodes"][0]["quotes"][0]["turn_id"],
        accepted["turn_id"]
    );
    card.clone()
}

#[test]
fn cloud_course_teach_back_survives_restart_in_its_private_lesson_and_chat() {
    let mut f = Fixture::new();
    let probe = Probe::default();
    configure(&f, &probe);
    f.control.set_reader_admin("B", true).unwrap();
    let mut publication = super::super::inquiry_course_tests::publication_input(&f, 0);
    let lesson = &mut publication["manifest"]["inquiry_course"]["lessons"][0];
    lesson["reflection_prompt"] = json!("怎样解释刚才的观察？");
    lesson["assessment"]["source_lids"] = json!(["1.1"]);
    f.ok(&f.b, "POST", "/api/admin/workspace-releases", publication);
    let used = f.ok(
        &f.a,
        "POST",
        "/api/workspace-releases/exam-course/versions/1/use",
        json!({"operation_id":"use","goal":{"text":"解释观察","constraints":[]}}),
    );
    let scope = used["workspace"]["learning_workspace_id"].as_str().unwrap();
    let course_path = format!("/api/learning-workspaces/{scope}/shared-release/course");
    let mut view = f.ok(&f.a, "POST", &course_path, json!({"operation_id":"start","run_id":null,
        "expected_revision":0,"release_version":1,"action":{"kind":"start","lesson_id":"compute-or-bandwidth"}}));
    let run = view["progress"]["run_id"].as_str().unwrap().to_owned();
    for (index, action) in [
        json!({"kind":"advance"}),
        json!({"kind":"predict","response":{"choice":"compute","reason":""}}),
        json!({"kind":"observe","preset":"baseline"}),
        json!({"kind":"observe","preset":"double_compute"}),
        json!({"kind":"observe","preset":"double_bandwidth"}),
        json!({"kind":"advance"}),
    ]
    .into_iter()
    .enumerate()
    {
        view = f.ok(
            &f.a,
            "POST",
            &course_path,
            json!({"operation_id":format!("activity-{index}"),"run_id":run,
            "expected_revision":view["progress"]["revision"],"release_version":1,"action":action}),
        );
    }
    assert_eq!(view["progress"]["stage"], "explanation");
    let scene = f.ok(
        &f.a,
        "POST",
        "/api/workspaces",
        json!({"published_book_ref":f.x,
        "attachment_id":"lesson","learning_workspace_id":scope}),
    );
    let scene = f.action(&f.a, &scene, "lesson", "chat/new", json!({}));
    let chat = scene["selected_chat"].clone();
    let mut first = Fixture::stamp(&scene, "lesson");
    let words = "我认为比较时间就能判断瓶颈。";
    for (key, value) in [
        ("entry_action", json!("explain_back")),
        ("message", json!(words)),
        ("session_id", chat.clone()),
        ("client_request_id", json!("explanation")),
        ("expression_id", json!("explanation")),
        ("operation_id", json!("first")),
        ("expected_chat_head", Value::Null),
        (
            "course_context",
            json!({"run_id":run,"revision":view["progress"]["revision"],"surface":"course"}),
        ),
    ] {
        first[key] = value;
    }
    let (status, accepted) = admit(&f, &scene, &first);
    assert_eq!(status, 202, "{accepted}");
    let mut changed = first.clone();
    changed["course_context"]["run_id"] = json!("other-lesson");
    assert_eq!(
        admit(&f, &scene, &changed).1["error_code"],
        "REQUEST_KEY_CONFLICT"
    );
    // Dispatch the queued request from the frozen course and workspace on disk.
    let f = reopen(f, &probe);
    let card = deliver(&f, &accepted, words);
    let course = format!("{course_path}?run_id={run}");
    let first_view = f.ok(&f.a, "GET", &course, Value::Null);
    let expression = first_view["expressions"]
        .as_array()
        .unwrap()
        .last()
        .unwrap();
    assert_eq!(expression["teach_back"]["session_id"], chat);
    assert_eq!(expression["teach_back"]["turn_id"], accepted["turn_id"]);
    assert_eq!(expression["teach_back"]["round_id"], card["round_id"]);
    assert_eq!(expression["response"], words);
    assert_eq!(expression["round_index"], 0);
    assert_eq!(first_view["reviews"][0]["comparison"], card);
    assert_eq!(first_view["reviews"][0]["displayed"], false);
    assert_eq!(f.call(&f.b, "GET", &course, Value::Null).0, 404);
    let turn_path = format!("/api/agent/runs/{}", accepted["turn_id"].as_str().unwrap());
    assert_eq!(f.call(&f.b, "GET", &turn_path, Value::Null).0, 404);
    let restored = f.ok(&f.a, "GET", &turn_path, Value::Null);
    assert_eq!(restored["turn"]["domain"]["course_retry_context"], first["course_context"]);
    let scene = f.action(&f.a, &f.get(&f.a, &scene), "reopen", "attach", json!({}));
    let shown = f.action(
        &f.a,
        &scene,
        "reopen",
        "tutor/display",
        json!({"chat_session_id":chat,
        "turn_id":accepted["turn_id"],"round_id":card["round_id"]}),
    );
    let shown_view = f.ok(&f.a, "GET", &course, Value::Null);
    assert_eq!(shown_view["reviews"][0]["displayed"], true);
    assert_eq!(shown_view["progress"]["stage"], "explanation");
    let revised = "我现在会分别计算两项用时，再比较哪一项更长。";
    let mut reply = first;
    for (key, value) in Fixture::stamp(&shown, "reopen").as_object().unwrap() {
        reply[key] = value.clone();
    }
    reply["message"] = json!(revised);
    reply["client_request_id"] = json!("revision");
    reply["expression_id"] = json!("revision");
    reply["operation_id"] = json!("revise");
    reply["expected_chat_head"] = accepted["turn_id"].clone();
    reply["course_context"]["revision"] = shown_view["progress"]["revision"].clone();
    reply["teach_back_reply"] =
        json!({"kind":"revise_understanding","round_id":card["round_id"],"row_id":"row"});
    let (status, accepted_reply) = admit(&f, &shown, &reply);
    assert_eq!(status, 202, "{accepted_reply}");
    let second_card = deliver(&f, &accepted_reply, revised);
    assert_eq!(
        second_card["continuation"]["parent_round_id"],
        card["round_id"]
    );
    assert!(second_card["support_delivery_refs"]
        .as_array()
        .unwrap()
        .contains(&card["round_id"]));
    let final_view = f.ok(&f.a, "GET", &course, Value::Null);
    assert_eq!(final_view["expressions"].as_array().unwrap().len(), 2);
    assert_eq!(final_view["expressions"][0], *expression);
    assert_eq!(
        final_view["expressions"][1]["teach_back"]["round_id"],
        second_card["round_id"]
    );
    assert_eq!(final_view["progress"]["stage"], "explanation");
}
