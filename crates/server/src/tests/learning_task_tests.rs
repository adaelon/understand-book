use super::*;
use memory::course::{CourseAction, CourseMutation, InquiryCourse};

fn scope_scene(f: &Fixture, attachment: &str) -> (Value, Value) {
    let scope = f.ok(
        &f.a,
        "POST",
        "/api/learning-workspaces",
        json!({
            "goal":{"text":"学习资源下界","constraints":[]}, "members":[f.x]
        }),
    );
    let scene = f.ok(
        &f.a,
        "POST",
        "/api/workspaces",
        json!({
            "published_book_ref":f.x, "attachment_id":attachment,
            "learning_workspace_id":scope["learning_workspace_id"]
        }),
    );
    let scene = f.action(&f.a, &scene, attachment, "chat/new", json!({}));
    (scope, scene)
}

#[test]
fn learning_task_keeps_original_closed_chat_filters_finished_courses_and_isolates_scope() {
    let f = Fixture::new();
    let (scope, scene) = scope_scene(&f, "learning-task");
    let scope_id = scope["learning_workspace_id"].as_str().unwrap();
    let chat = scene["selected_chat"].as_str().unwrap();
    let publication = f.access.library.lock().unwrap().load("A", &f.x).unwrap();
    let users = f.access.users.lock().unwrap().get("A", "now").unwrap();
    let mut store = users
        .lock()
        .unwrap()
        .learning_store_for_source(Some(scope_id), &f.x.book_id)
        .unwrap();
    let mut course: InquiryCourse = serde_json::from_str(include_str!(
        "../../../../docs/examples/inquiry-course/ai-infra-resource-bounds.v1.course.json"
    ))
    .unwrap();
    let lesson = &mut course.lessons[0];
    lesson.summary.source.publication = f.x.clone();
    lesson.summary.source.source_revision = publication.book.source_fingerprint().into();
    let started = store
        .mutate_course(
            &CourseMutation {
                operation_id: "learning-task-start".into(),
                run_id: None,
                expected_revision: 0,
                release_version: 1,
                action: CourseAction::Start {
                    lesson_id: lesson.summary.id.clone(),
                },
            },
            Some(("task-release", 1, lesson)),
            1,
            "1",
            None,
        )
        .unwrap();
    let run = &started.progress.run_id;
    f.action(
        &f.a,
        &scene,
        "learning-task",
        "learning-task",
        json!({"course_run_id":run,"chat_session_id":chat}),
    );
    let mut closed = store.save_closed_book(serde_json::from_value(json!({
        "id":"original-practice","owner_id":run,"course":true,"chat_session_id":chat,
        "source_id":f.x.book_id,"source_revision":publication.book.source_fingerprint(),
        "goal_revision":store.state().unwrap().workspace_goal_revision,
        "revision":0,"status":"paused","target":"解释两项资源下界","criteria":"说明条件",
        "attempt":1,"assisted":false,"needs_retry":false,"visible_turn_ids":[],"draft":"未提交草稿",
        "completion_reason":"","learner_quote":""
    })).unwrap(), "learning-task-practice", "2").unwrap();
    let selected = f.ok(&f.a, "GET", "/api/me/learning-tasks", Value::Null);
    assert_eq!(selected["tasks"][0]["kind"], "closed_book");
    assert_eq!(selected["tasks"][0]["closed_book_id"], "original-practice");
    assert_eq!(selected["tasks"][0]["chat_session_id"], chat);
    assert_eq!(selected["tasks"][0]["published_book_ref"], json!(f.x));
    assert_eq!(selected["tasks"][0]["learning_workspace_id"], scope_id);
    assert!(selected["tasks"][0].get("course_context").is_none());
    assert!(selected.to_string().find("未提交草稿").is_none());
    let path = users
        .lock()
        .unwrap()
        .session_store
        .as_ref()
        .unwrap()
        .paths
        .selection
        .clone();
    let before = std::fs::read(&path).unwrap();
    let other_chat = f.action(&f.a, &scene, "learning-task", "chat/new", json!({}));
    assert_ne!(other_chat["selected_chat"], chat);
    let after_navigation = std::fs::read(&path).unwrap();
    assert_eq!(
        f.ok(&f.a, "GET", "/api/me/learning-tasks", Value::Null),
        selected
    );
    assert_eq!(std::fs::read(&path).unwrap(), after_navigation);
    assert_eq!(before, after_navigation);
    let reader_tasks = f.ok(
        &f.a,
        "GET",
        &format!(
            "/api/workspaces/{}/learning-task",
            scene["workspace_id"].as_str().unwrap()
        ),
        Value::Null,
    );
    assert_eq!(reader_tasks, selected);
    assert_eq!(
        f.ok(&f.b, "GET", "/api/me/learning-tasks", Value::Null)["tasks"],
        json!([])
    );
    let (_, other_scope) = scope_scene(&f, "another-scope");
    let mut input = Fixture::stamp(&other_scope, "another-scope");
    input["course_run_id"] = json!(run);
    assert_ne!(
        f.call(
            &f.a,
            "POST",
            &format!(
                "/api/workspaces/{}/learning-task",
                other_scope["workspace_id"].as_str().unwrap()
            ),
            input
        )
        .0,
        200
    );
    let archive = format!("/api/learning-workspaces/{scope_id}/archive");
    f.ok(&f.a, "POST", &archive, json!({"archived":true}));
    assert_eq!(
        f.ok(&f.a, "GET", "/api/me/learning-tasks", Value::Null)["tasks"][0]["status"],
        "unavailable"
    );
    f.ok(&f.a, "POST", &archive, json!({"archived":false}));
    closed.completion_reason = "已完成当前目标".into();
    closed.status = "ended".into();
    let mut closed = store
        .save_closed_book(closed, "learning-task-complete", "3")
        .unwrap();
    let current = f.ok(&f.a, "GET", "/api/me/learning-tasks", Value::Null);
    assert_eq!(current["tasks"][0]["kind"], "course");
    assert!(current["tasks"][0]["closed_book_id"].is_null());
    closed.id = "next-practice".into();
    closed.completion_reason.clear();
    closed.status = "paused".into();
    store
        .save_closed_book(closed, "learning-task-next-practice", "4")
        .unwrap();
    let original_chat = f.action(
        &f.a,
        &other_chat,
        "learning-task",
        "chat/select",
        json!({"session_id":chat}),
    );
    f.action(
        &f.a,
        &original_chat,
        "learning-task",
        "learning-task",
        json!({"course_run_id":run,"chat_session_id":chat}),
    );
    f.ok(
        &f.a,
        "POST",
        "/api/agent/history/delete",
        json!({"session_id":chat}),
    );
    let deleted = f.ok(&f.a, "GET", "/api/me/learning-tasks", Value::Null);
    assert_eq!(deleted["tasks"][0]["status"], "unavailable");
    assert_eq!(deleted["tasks"][0]["chat_session_id"], chat);
    store
        .mutate_course(
            &CourseMutation {
                operation_id: "learning-task-skip".into(),
                run_id: Some(run.clone()),
                expected_revision: started.progress.revision,
                release_version: 1,
                action: CourseAction::Skip,
            },
            None,
            1,
            "5",
            None,
        )
        .unwrap();
    assert_eq!(
        f.ok(&f.a, "GET", "/api/me/learning-tasks", Value::Null)["tasks"],
        json!([])
    );
}

#[test]
fn learning_task_conversation_requires_a_pending_question_or_explicit_continuation() {
    let f = Fixture::new();
    let (_, scene) = scope_scene(&f, "conversation-task");
    let chat = scene["selected_chat"].as_str().unwrap();
    let handle = f.access.users.lock().unwrap().get("A", "now").unwrap();
    let append = |id: &str, question: Option<&str>| {
        let mut user = handle.lock().unwrap();
        let session = user
            .agent_history
            .sessions
            .iter_mut()
            .find(|s| s.id == chat)
            .unwrap();
        let part = match question {
            Some(text) => json!({"kind":"socratic_question","text":text}),
            None => json!({"kind":"markdown","text":"这是已交付的讲解。"}),
        };
        session.turns.push(serde_json::from_value(json!({
            "turn_id":id,"user_turn_ordinal":session.turns.len()+1,"user":"帮我理解这一段","status":"completed","published_book_ref":f.x,
            "question_anchor_lid":null,"question_quote":null,
            "outcome":{"answer":question.unwrap_or("这是已交付的讲解。"),"answer_view":{"parts":[part],"sources":[]},
                "incomplete":false,"turns":1,"tokens_spent":10,"effects":[],"trace":[]}
        })).unwrap());
    };
    append("original-question", None);
    let path = format!(
        "/api/workspaces/{}/learning-task",
        scene["workspace_id"].as_str().unwrap()
    );
    assert_ne!(
        f.call(
            &f.a,
            "POST",
            &path,
            Fixture::stamp(&scene, "conversation-task")
        )
        .0,
        200
    );
    f.action(
        &f.a,
        &scene,
        "conversation-task",
        "learning-task",
        json!({"continue_conversation":true}),
    );
    assert_eq!(
        f.ok(&f.a, "GET", "/api/me/learning-tasks", Value::Null)["tasks"][0]["chat_session_id"],
        chat
    );
    append("a-later-finished-exchange", None);
    assert_eq!(
        f.ok(&f.a, "GET", "/api/me/learning-tasks", Value::Null)["tasks"],
        json!([])
    );
    append("a-delivered-question", Some("为什么读取时间仍是下界？"));
    f.action(
        &f.a,
        &scene,
        "conversation-task",
        "learning-task",
        json!({}),
    );
    assert_eq!(
        f.ok(&f.a, "GET", "/api/me/learning-tasks", Value::Null)["tasks"][0]["question"],
        "为什么读取时间仍是下界？"
    );
    assert!(
        f.ok(&f.a, "GET", "/api/me/learning-tasks", Value::Null)["tasks"][0]
            .get("course_context")
            .is_none()
    );
}

fn publish_task_collection(f: &Fixture, version: u32, references: &[PublishedBookRef]) {
    let mut course: Value = serde_json::from_str(include_str!(
        "../../../../docs/examples/inquiry-course/ai-infra-resource-bounds.v1.course.json"
    ))
    .unwrap();
    let template = course["lessons"][0].clone();
    let lessons = references.iter().enumerate().map(|(index, reference)| {
        let publication = f.access.library.lock().unwrap().load("A", reference).unwrap();
        let mut lesson = template.clone();
        lesson["summary"]["id"] = json!(format!("lesson-{}", index + 1));
        lesson["summary"]["unit_id"] = json!(if index == 0 { "unit-one" } else { "unit-two" });
        lesson["summary"]["source"] = json!({"publication":reference,
            "source_revision":publication.book.source_fingerprint(), "start_lid":"1.1","end_lid":"1.3",
            "label":"课程归属测试来源","structure_refs":["fixture"]});
        lesson["assessment"]["source_lids"] = json!(["1.2"]);
        lesson
    }).collect::<Vec<_>>();
    course["lessons"] = json!(lessons);
    course["routes"] = json!([]);
    course["units"] = json!([
        {"id":"unit-one","title":if version == 1 { "原单元一" } else { "新单元一" }},
        {"id":"unit-two","title":if version == 1 { "原单元二" } else { "新单元二" }}
    ]);
    let mut input = super::super::s1_workspace_publication_tests::publication(
        f,
        &format!("learning-task-collection-v{version}"),
        version - 1,
    );
    input["release_id"] = json!("learning-task-collection");
    input["manifest"]["name"] = json!(if version == 1 {
        "原发布整课名称"
    } else {
        "新版跨书课程"
    });
    // Extra reference material does not turn a course whose lessons use one book
    // into a cross-book course.
    let mut materials = references.to_vec();
    for reference in [&f.x, &f.y] {
        if !materials.contains(reference) {
            materials.push(reference.clone());
        }
    }
    input["manifest"]["materials"] = json!(materials
        .iter()
        .map(|reference| json!({"published_book_ref":reference,"required_capabilities":["text"]}))
        .collect::<Vec<_>>());
    input["manifest"]["inquiry_course"] = course;
    f.ok(&f.b, "POST", "/api/admin/workspace-releases", input);
}

fn start_collection_task(f: &Fixture, version: u32, attachment: &str) -> (Value, Value, Value) {
    let used = f.ok(&f.a, "POST",
        &format!("/api/workspace-releases/learning-task-collection/versions/{version}/use"),
        json!({"operation_id":format!("use-{attachment}"),"goal":{"text":"理解本课程的资源限制","constraints":[]}}));
    let scope = used["workspace"]["learning_workspace_id"].as_str().unwrap();
    let run = f.ok(
        &f.a,
        "POST",
        &format!("/api/learning-workspaces/{scope}/shared-release/course"),
        json!({"operation_id":format!("start-{attachment}"),"run_id":null,"expected_revision":0,
            "release_version":version,"action":{"kind":"start","lesson_id":"lesson-2"}}),
    );
    let scene = f.ok(
        &f.a,
        "POST",
        "/api/workspaces",
        json!({"published_book_ref":f.x,
        "attachment_id":attachment,"learning_workspace_id":scope}),
    );
    let scene = f.action(&f.a, &scene, attachment, "chat/new", json!({}));
    f.action(
        &f.a,
        &scene,
        attachment,
        "learning-task",
        json!({"course_run_id":run["progress"]["run_id"],
        "chat_session_id":scene["selected_chat"]}),
    );
    (used, scene, run)
}

#[test]
fn learning_task_course_context_keeps_original_collection_and_current_publication_title() {
    let mut f = Fixture::new();
    f.control.set_reader_admin("B", true).unwrap();
    let original = f.access.library.lock().unwrap().load("A", &f.x).unwrap();
    let another_version = f
        .access
        .library
        .lock()
        .unwrap()
        .publish(original.directory())
        .unwrap()
        .reference;
    for user in ["A", "B"] {
        f.access
            .library
            .lock()
            .unwrap()
            .grant(user, &another_version)
            .unwrap();
    }
    let newer = f
        .access
        .library
        .lock()
        .unwrap()
        .load("A", &another_version)
        .unwrap();
    std::fs::write(
        original.directory().with_extension("display.json"),
        r#"{"title":"原版书名"}"#,
    )
    .unwrap();
    std::fs::write(
        newer.directory().with_extension("display.json"),
        r#"{"title":"同书另一版标题"}"#,
    )
    .unwrap();
    assert_ne!(f.x.publication_id, another_version.publication_id);
    publish_task_collection(&f, 1, &[another_version.clone(), f.x.clone()]);
    let (used, scene, run) = start_collection_task(&f, 1, "original-collection");
    let original_context = json!({"release_id":"learning-task-collection","release_version":1,
        "collection_kind":"book","collection_title":"原版书名","unit_title":"原单元二",
        "lesson_number":2,"lesson_count":2});
    let read_tasks = || f.ok(&f.a, "GET", "/api/me/learning-tasks", Value::Null);
    assert_eq!(read_tasks()["tasks"][0]["course_context"], original_context);
    let scope = used["workspace"]["learning_workspace_id"].as_str().unwrap();
    let user = f.access.users.lock().unwrap().get("A", "now").unwrap();
    let mut store = user
        .lock()
        .unwrap()
        .learning_store_for_source(Some(scope), &f.x.book_id)
        .unwrap();
    store.save_closed_book(serde_json::from_value(json!({"id":"collection-practice",
        "owner_id":run["progress"]["run_id"],"course":true,"chat_session_id":scene["selected_chat"],
        "source_id":f.x.book_id,"source_revision":original.book.source_fingerprint(),
        "goal_revision":store.state().unwrap().workspace_goal_revision,"revision":0,"status":"paused",
        "target":"解释当前资源限制","criteria":"说明成立条件","attempt":1,"assisted":false,
        "needs_retry":false,"visible_turn_ids":[],"draft":"","completion_reason":"","learner_quote":""})).unwrap(),
        "collection-practice", "now").unwrap();
    let practice = read_tasks();
    assert_eq!(practice["tasks"][0]["kind"], "closed_book");
    assert_eq!(practice["tasks"][0]["course_context"], original_context);
    publish_task_collection(&f, 2, &[f.y.clone(), f.x.clone(), another_version]);
    assert_eq!(read_tasks()["tasks"][0]["course_context"], original_context);
    let (_, _, second_run) = start_collection_task(&f, 2, "cross-book-collection");
    let tasks = read_tasks();
    let cross_book = tasks["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|task| task["course_run_id"] == second_run["progress"]["run_id"])
        .unwrap();
    assert_eq!(
        cross_book["course_context"],
        json!({"release_id":"learning-task-collection",
        "release_version":2,"collection_kind":"course","collection_title":"新版跨书课程",
        "unit_title":"新单元二","lesson_number":2,"lesson_count":3})
    );
    f.ok(
        &f.a,
        "POST",
        &format!("/api/learning-workspaces/{scope}/archive"),
        json!({"archived":true}),
    );
    let archived_tasks = read_tasks();
    let archived = archived_tasks["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|task| task["course_run_id"] == run["progress"]["run_id"])
        .unwrap();
    assert_eq!(archived["status"], "unavailable");
    assert_eq!(archived["course_context"], original_context);
    assert_eq!(
        f.ok(&f.b, "GET", "/api/me/learning-tasks", Value::Null)["tasks"],
        json!([])
    );
    let selection = user
        .lock()
        .unwrap()
        .session_store
        .as_ref()
        .unwrap()
        .paths
        .selection
        .clone();
    let before = std::fs::read(&selection).unwrap();
    read_tasks();
    assert_eq!(std::fs::read(&selection).unwrap(), before);
}
