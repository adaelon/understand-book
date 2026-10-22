use super::mu5_tests::Fixture;
use super::*;
use runtime::run_context::ResidentStatePort;

const LESSON: &str = include_str!("../../../../docs/examples/cloud-course/lesson.txt");
const TRANSCRIPT: &str =
    include_str!("../../../../docs/examples/cloud-course/video-transcript.txt");

fn write_fixture_structure(directory: &Path, book_id: &str, title: &str, summary: &str) {
    std::fs::write(directory.join("book_structure.json"), json!({
        "header":{"book_id":book_id,"book_version":"v1","profile_id":"technical_learning",
            "profile_version":"technical_learning_v0","core_schema_version":"core_v0","generated_at":"2026-10-20T00:00:00Z"},
        "spine":[{"lid":"1","role":"foundation","summary":{"text":summary,"evidence_lids":["1"]},
            "key_stop_ids":["source-window"],"depends_on":[]}],
        "throughlines":[{"id":"evidence-source","name":title,
            "summary":{"text":summary,"evidence_lids":["1"]},"lids":["1"],"key_stop_ids":["source-window"]}],
        "key_stops":[{"id":"source-window","lid":"1","type":"example","title":title,
            "reason":{"text":summary,"evidence_lids":["1"]}}]
    }).to_string()).unwrap();
}

pub(super) fn prepare_text(f: &Fixture) -> base_schema::PublishedBookRef {
    let dir = f.root.path().join("cloud-course-original-text");
    std::fs::create_dir_all(&dir).unwrap();
    let source = LESSON.trim();
    let base = json!({
        "book_id":"cloud-course-original-text",
        "lid_nodes":[{"lid":"1","path":[1],"kind":"paragraph","span":{"start":0,"end":source.encode_utf16().count()},"children":[]}],
        "graph_nodes":[
            {"id":"concept:source-window","type":"concept","name":"来源窗口与证据范围","occurrences":["1"],"source_lid":null},
            {"id":"concept:video-interval","type":"concept","name":"示范视频与时间片段","occurrences":["1"],"source_lid":null}
        ],
        "graph_edges":[{"source":"concept:source-window","target":"concept:video-interval","type":"applies_to","direction":"directed","scope":"local","weight":1.0}]
    });
    let book = Book::new(serde_json::from_value(base).unwrap(), source);
    super::tutor_tests::t15_foundation(&dir, &book);
    std::fs::write(dir.join("source.txt"), source).unwrap();
    super::tutor_tests::t15_close(&dir, &book, "pass1", &["base.json", "source.txt"]);
    write_fixture_structure(&dir, &book.base.book_id, "来源窗口：文字与示范视频",
        "解释资料版本、原文位置和视频时间范围；对照工作区内的两秒动态图案视频及人工字幕，练习来源定位。");
    super::tutor_tests::t15_close(&dir, &book, "book_structure", &["book_structure.json"]);
    f.access
        .library
        .lock()
        .unwrap()
        .publish(&dir)
        .unwrap()
        .reference
}

pub(super) fn prepare_video(
    f: &Fixture,
) -> (
    PathBuf,
    base_schema::PublishedBookRef,
    runtime::video::ReadFrames,
) {
    let dir = f.root.path().join("cloud-course-original-video");
    let (book, _, request) = crate::video_frames::tests::fixture(&dir);
    let manifest_file = dir.join("video_source_manifest.json");
    let mut manifest: Value = serde_json::from_slice(&std::fs::read(&manifest_file).unwrap()).unwrap();
    manifest["source"]["original_path"] = json!("两秒动态图案示范视频.mp4");
    std::fs::write(manifest_file, manifest.to_string()).unwrap();
    let transcript = TRANSCRIPT.trim();
    let heading = "# 两秒动态图案示范视频\n\n";
    let source = format!("{heading}{transcript}");
    let mut base = serde_json::to_value(&book.base).unwrap();
    base["lid_nodes"][0]["span"]["end"] = json!(source.encode_utf16().count());
    base["graph_nodes"] = json!([{"id":"concept:changing-test-pattern","type":"concept",
        "name":"两秒动态图案示范视频","occurrences":["1"],"source_lid":null}]);
    std::fs::write(dir.join("base.json"), base.to_string()).unwrap();
    std::fs::write(dir.join("source.txt"), &source).unwrap();
    let map_file = dir.join("transcript_map.json");
    let mut map: Value = serde_json::from_slice(&std::fs::read(&map_file).unwrap()).unwrap();
    map["machine_transcription"] =
        json!({"engine":"manual-fixture","model":"original-text","language":"zh"});
    map["segments"][0]["text"] = json!(transcript);
    map["segments"][0]["text_range"]["range"]["start"] = json!(heading.encode_utf16().count());
    map["segments"][0]["text_range"]["range"]["end"] = json!(source.encode_utf16().count());
    std::fs::write(map_file, map.to_string()).unwrap();
    write_fixture_structure(
        &dir,
        &book.base.book_id,
        "两秒动态图案视频",
        "原创无声示范视频的人工字幕；动态图案在零至两秒内持续变化，可按时间范围查看原始画面。",
    );
    let video = f
        .access
        .library
        .lock()
        .unwrap()
        .publish(&dir)
        .unwrap()
        .reference;
    // The documented original fixture is silent; it must still be playable.
    let playback = crate::video_playback::cache_directory(f.root.path(), &video.book_id).unwrap();
    assert!(crate::video_playback::ready(&playback).is_some());
    (dir, video, request)
}

pub(super) fn course(
    f: &Fixture,
    text: &base_schema::PublishedBookRef,
    video: &base_schema::PublishedBookRef,
    expected: u32,
) -> Value {
    let input = json!({
        "operation_id":format!("cloud-course-release-{expected}"),
        "release_id":"cloud-original-fixture",
        "expected_version":expected,
        "visible_to_users":[],
        "allowed_tiers":["basic","plus","pro"],
        "manifest":{
            "name":"资料引用与时间片段：原创验收样例",
            "publisher":"Understand Book 测试材料",
            "public_goal":"找到支持陈述的原文与确切视频片段",
            "applicability":"通用预置课程流程验收，不代表已选定首发学科",
            "prerequisites":"无需先修知识",
            "materials":[
                {"published_book_ref":text,"required_capabilities":["text"]},
                {"published_book_ref":video,"required_capabilities":["text","video","transcript"]}
            ],
            "topic_links":[{
                "title":"原文陈述与视频片段",
                "description":"阅读来源窗口的说明，再定位两秒视频里的动态图案。",
                "materials":[text,video]
            }],
            "suggested_route":[{
                "title":"先读说明，再查看原视频",
                "description":"说清材料版本和时间范围；观看记录不作为掌握证据。",
                "materials":[text,video]
            }],
            "external_resources":[{
                "title":"外部导览示例",
                "url":format!("https://example.invalid/course/v{}",expected+1),
                "description":"仅测试外部网页入口；未导入为教材、字幕或视频"
            }]
        }
    });
    f.ok(&f.b, "POST", "/api/admin/workspace-releases", input)
}

pub(super) fn configure_product(f: &Fixture) {
    // A saleable test product exists only in this temporary database. Production
    // configuration and real payment amounts remain absent from the repository.
    f.ok(&f.b, "POST", "/api/admin/cloud-plans", json!({
        "operation_id":"cloud-course-test-product","product_id":"cloud-course-day3",
        "title":"三天验收套餐（隔离数据库）","tier":"basic","duration":"day3",
        "revision":0,"enabled":true,"test_only":false,
        "amount_fen":123,"basis":{"version":"cloud-course-test-basis","basis_micro_cny":1000000},
        "grant_milli_percent":100000,"storage_bytes":10000000,"max_file_bytes":1000000,
        "retention_seconds":86400,"instructions":"受控测试核款，无真实收款"
    }));
}

pub(super) fn purchase(f: &Fixture, owner: &str) {
    f.ok(
        &f.b,
        "POST",
        &format!("/api/admin/users/{owner}/cloud-plan-purchases"),
        json!({
            "operation_id":format!("cloud-course-paid-{owner}"),"product_id":"cloud-course-day3",
            "product_revision":1,"paid_at":multi_user_host::now(),"channel":"isolated-test",
            "external_ref":format!("cloud-course-test-{owner}"),"note":"无真实付款"
        }),
    );
}

fn use_course(f: &Fixture, token: &str, goal: &str, mode: &str) -> Value {
    let date = (time::OffsetDateTime::now_utc() + time::Duration::days(3))
        .date()
        .to_string();
    f.ok(
        token,
        "POST",
        "/api/workspace-releases/cloud-original-fixture/versions/1/use",
        json!({
            "operation_id":"cloud-course-use","goal":{"text":goal},
            "exam_sprint":{"exam_date":date,"syllabus":"原文、资料版本和时间片段",
                "daily_minutes":30,"utc_offset_minutes":480,"entry_mode":mode}
        }),
    )
}

fn instance_path(instance: &Value) -> String {
    format!(
        "/api/learning-workspaces/{}/shared-release",
        instance["workspace"]["learning_workspace_id"]
            .as_str()
            .unwrap()
    )
}

#[test]
fn cloud_course_purchase_shared_video_private_goals_and_old_sources_survive_update() {
    let mut f = Fixture::new();
    f.control.set_reader_admin("B", true).unwrap();
    let text = prepare_text(&f);
    let (video_dir, video, request) = prepare_video(&f);
    let published = course(&f, &text, &video, 0);
    assert_eq!(
        published["workspace_release"]["availability"]["status"],
        "ready"
    );
    let detail_path = "/api/workspace-releases/cloud-original-fixture/versions/1";
    assert_eq!(f.call(&f.a, "GET", detail_path, Value::Null).0, 404);
    configure_product(&f);
    let new_account = f.ok(&f.a, "GET", "/api/account/plans", Value::Null);
    assert!(new_account["current"].is_null());
    assert_eq!(new_account["catalog"]["items"].as_array().unwrap().len(), 1);
    purchase(&f, "A");
    assert_eq!(
        f.call(&f.b, "GET", detail_path, Value::Null).0,
        404,
        "admin identity does not grant course reader access"
    );
    purchase(&f, "B");
    let detail = f.ok(&f.a, "GET", detail_path, Value::Null);
    assert_eq!(detail["availability"]["status"], "ready");
    assert_eq!(
        detail["manifest"]["external_resources"][0]["url"],
        "https://example.invalid/course/v1"
    );
    assert_eq!(
        detail["availability"]["materials"]
            .as_array()
            .unwrap()
            .len(),
        2,
        "external links do not become searchable publications"
    );
    let capabilities = detail["availability"]["materials"][1]["available_capabilities"]
        .as_array()
        .unwrap();
    assert!(capabilities.contains(&json!("video")));
    assert!(capabilities.contains(&json!("transcript")));
    assert!(!capabilities.contains(&json!("tutor")));

    let before_calls: i64 = f
        .control
        .connection
        .query_row("SELECT count(*) FROM billable_usage", [], |r| r.get(0))
        .unwrap();
    let before_publications: i64 = f
        .control
        .connection
        .query_row("SELECT count(*) FROM book_publications", [], |r| r.get(0))
        .unwrap();
    let a = use_course(&f, &f.a, "我直接从一道不会的题开始", "question");
    let b = use_course(&f, &f.b, "我先做诊断再复习", "diagnostic");
    let a_scope = a["workspace"]["learning_workspace_id"].as_str().unwrap();
    let b_scope = b["workspace"]["learning_workspace_id"].as_str().unwrap();
    assert_ne!(a_scope, b_scope);
    assert_ne!(a["workspace"]["goal"], b["workspace"]["goal"]);
    assert_eq!(
        a["instance"]["release"]["manifest"],
        b["instance"]["release"]["manifest"]
    );
    assert_eq!(f.call(&f.b, "GET", &instance_path(&a), Value::Null).0, 404);
    assert_eq!(
        f.control
            .connection
            .query_row("SELECT count(*) FROM billable_usage", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        before_calls
    );
    assert_eq!(
        f.control
            .connection
            .query_row("SELECT count(*) FROM book_publications", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        before_publications
    );
    assert_eq!(
        f.control
            .connection
            .query_row(
                "SELECT count(*) FROM book_grants WHERE book_id IN (?,?)",
                rusqlite::params![text.book_id, video.book_id],
                |r| r.get::<_, i64>(0),
            )
            .unwrap(),
        0,
        "course use must not create permanent book grants"
    );

    let scene = f.ok(&f.a, "POST", "/api/workspaces", json!({
        "published_book_ref":text,"attachment_id":"cloud-course-page","learning_workspace_id":a_scope
    }));
    let scene = f.action(&f.a, &scene, "cloud-course-page", "chat/new", json!({}));
    let mut run = f.run(&scene, "cloud-course-page");
    let structure = run
        .library_read(
            "library.read",
            json!({
                "published_book_ref":text,"operation":"structure","at":"1"
            }),
        )
        .unwrap();
    assert_eq!(structure.body["data"]["available"], true);
    assert!(
        structure.body["data"]["spine_unit"].is_object(),
        "the original material must have a usable structure entry"
    );
    let directory = run.library_read("library.list", json!({})).unwrap();
    assert!(directory.body.to_string().contains("来源窗口与证据范围"));
    assert!(directory.body.to_string().contains("两秒动态图案示范视频"), "{}", directory.body);
    let original_text = run
        .library_read(
            "library.read",
            json!({"published_book_ref":text,"operation":"text","lid":"1"}),
        )
        .unwrap();
    assert!(original_text
        .body
        .to_string()
        .contains("Source windows are part of the evidence."));
    let frames = run
        .read_video_frames(Some(&video), request, &Default::default())
        .unwrap();
    assert_eq!(frames.images.len(), 2);
    assert_eq!(frames.transcript[0].text, TRANSCRIPT.trim());
    let media = run
        .media_source_snapshot(
            serde_json::from_value(frames.projection()["frames"][0]["source_target"].clone())
                .unwrap(),
        )
        .unwrap();
    let source_revision = f
        .access
        .library
        .lock()
        .unwrap()
        .load("A", &video)
        .unwrap()
        .book
        .source_fingerprint()
        .to_owned();
    let binding = SourceBinding { music: None,
        source_ref_id: "cloud-original-video-source".into(),
        book_id: video.book_id.clone(),
        publication_id: Some(video.publication_id.clone()),
        source_revision: Some(source_revision),
        evidence_range: None,
        evidence_text_digest: String::new(),
        label_snapshot: "原创验收片段".into(),
        preview_snapshot: TRANSCRIPT.trim().into(),
        media: Some(media),
    };
    let turn = AgentTurnRef {
        session_id: run.scope.chat_session_id.clone(),
        turn_id: run.scope.turn_id.clone(),
        user_turn_ordinal: 1,
    };
    let user_handle = f.access.users.lock().unwrap().get("A", "now").unwrap();
    crate::session_runtime::append(
        &mut user_handle.lock().unwrap(),
        &turn,
        "cloud-course-source",
        crate::session_event::EventBody::SourcesBound {
            bindings: vec![binding.clone()],
        },
    )
    .unwrap();
    let learned = run.library_learning(json!({
        "published_book_ref":video,"operation":"observe","operation_id":"cloud-course-observation",
        "source_ref_ids":[binding.source_ref_id],"learner_quote":"Explain this",
        "learning_focus":"确切来源定位","expected_performance":"说清资料版本与视频范围",
        "capability":"explanation","interpretation":"用户需要解释这一片段","teaching_implication":"结合讲义说明来源窗口"
    }), std::slice::from_ref(&binding)).unwrap();
    let evidence_id = learned["evidence_ref"].as_str().unwrap();
    let other = f.access.users.lock().unwrap().get("B", "now").unwrap();
    assert!(other
        .lock()
        .unwrap()
        .learning_store_for_workspace(Some(b_scope))
        .unwrap()
        .evidence(evidence_id)
        .is_err());
    drop(run);

    f.ok(
        &f.a,
        "POST",
        &format!("/api/learning-workspaces/{a_scope}/members"),
        json!({"published_book_ref":f.y}),
    );
    let newer = f
        .access
        .library
        .lock()
        .unwrap()
        .publish(&video_dir)
        .unwrap()
        .reference;
    course(&f, &text, &newer, 1);
    let endpoint = instance_path(&a);
    let preview = f.ok(
        &f.a,
        "GET",
        &format!("{endpoint}/preview?version=2"),
        Value::Null,
    );
    assert!(preview["changes"]["added_materials"]
        .as_array()
        .unwrap()
        .contains(&json!(newer)));
    assert_eq!(
        f.ok(&f.a, "GET", &endpoint, Value::Null)["instance"]["version"],
        1
    );
    let updated = f.ok(
        &f.a,
        "POST",
        &format!("{endpoint}/update"),
        json!({
            "operation_id":"cloud-course-adopt","expected_version":1,"version":2
        }),
    );
    assert_eq!(updated["workspace"]["goal"], a["workspace"]["goal"]);
    assert_eq!(
        updated["instance"]["release"]["manifest"]["external_resources"][0]["url"],
        "https://example.invalid/course/v2"
    );
    assert_eq!(
        f.ok(&f.b, "GET", &instance_path(&b), Value::Null)["instance"]["release"]["manifest"]
            ["external_resources"][0]["url"],
        "https://example.invalid/course/v1"
    );
    assert!(updated["workspace"]["members"]
        .as_array()
        .unwrap()
        .iter()
        .any(|m| m["published_book_ref"] == json!(f.y)));
    assert_eq!(
        f.ok(&f.b, "GET", &instance_path(&b), Value::Null)["instance"]["version"],
        1
    );
    let resolved = f.action(
        &f.a,
        &f.get(&f.a, &scene),
        "cloud-course-page",
        "agent/source.resolve",
        json!({"turn_id":turn.turn_id,"source_ref_id":binding.source_ref_id}),
    );
    assert_eq!(resolved["result"]["media"]["publication"], json!(video));
    assert_eq!(resolved["result"]["stale"], false);
    assert!(user_handle
        .lock()
        .unwrap()
        .learning_store_for_workspace(Some(a_scope))
        .unwrap()
        .evidence(evidence_id)
        .is_ok());
    assert_eq!(
        f.control
            .connection
            .query_row("SELECT count(*) FROM billable_usage", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        before_calls
    );
}

#[test]
fn cloud_course_declared_video_must_have_prepared_video_and_timeline() {
    let mut f = Fixture::new();
    f.control.set_reader_admin("B", true).unwrap();
    let text = prepare_text(&f);
    configure_product(&f);
    purchase(&f, "A");
    let mut publication: Value = serde_json::from_str(include_str!(
        "../../../../docs/examples/cloud-course/release.template.json"
    ))
    .unwrap();
    publication["operation_id"] = json!("cloud-missing-video");
    publication["release_id"] = json!("cloud-missing-video");
    publication["manifest"]["materials"] = json!([
        {"published_book_ref":text,"required_capabilities":["text","video","transcript"]}
    ]);
    publication["manifest"]["topic_links"] = json!([]);
    let result = f.ok(&f.b, "POST", "/api/admin/workspace-releases", publication);
    assert_eq!(
        result["workspace_release"]["availability"]["status"],
        "incomplete"
    );
    let missing = result["workspace_release"]["availability"]["materials"][0]
        ["missing_capabilities"]
        .as_array()
        .unwrap();
    assert!(missing.contains(&json!("video")));
    assert!(missing.contains(&json!("transcript")));
    let (status, rejected) = f.call(
        &f.a,
        "POST",
        "/api/workspace-releases/cloud-missing-video/versions/1/use",
        json!({"operation_id":"cloud-reject-incomplete","goal":{"text":"实际视频还未准备"}}),
    );
    assert_eq!(status, 409);
    assert_eq!(rejected["error_code"], "WORKSPACE_RELEASE_NOT_READY");
    assert!(
        f.ok(&f.a, "GET", "/api/learning-workspaces", Value::Null)["workspaces"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn cloud_course_expired_plan_keeps_private_instance_but_ends_material_license() {
    let mut f = Fixture::new();
    f.control.set_reader_admin("B", true).unwrap();
    let text = prepare_text(&f);
    configure_product(&f);
    purchase(&f, "A");
    let mut publication: Value = serde_json::from_str(include_str!(
        "../../../../docs/examples/cloud-course/release.template.json"
    ))
    .unwrap();
    publication["operation_id"] = json!("cloud-expiry-course");
    publication["release_id"] = json!("cloud-expiry-course");
    publication["manifest"]["materials"] =
        json!([{"published_book_ref":text,"required_capabilities":["text"]}]);
    publication["manifest"]["topic_links"] = json!([]);
    f.ok(
        &f.b,
        "POST",
        "/api/admin/workspace-releases",
        publication.clone(),
    );
    let path = "/api/workspace-releases/cloud-expiry-course/versions/1/use";
    let used = f.ok(
        &f.a,
        "POST",
        path,
        json!({"operation_id":"cloud-expiry-use","goal":{"text":"保留我的个人复习目标"}}),
    );
    let scene = f.ok(&f.a, "POST", "/api/workspaces", json!({
        "published_book_ref":text,"attachment_id":"cloud-expiry","learning_workspace_id":used["workspace"]["learning_workspace_id"]
    }));
    let scene = f.action(
        &f.a,
        &scene,
        "cloud-expiry",
        "checkpoint",
        json!({"top_lid":"1"}),
    );
    let resumed = f.ok(&f.a, "GET", "/api/me/reading-resumption", Value::Null);
    assert_eq!(
        resumed["workspace_id"], scene["workspace_id"],
        "plan-only materials participate in account reading resumption"
    );
    assert_eq!(
        f.ok(
            &f.a,
            "GET",
            &format!("{}?lid=1", text.url("text")),
            Value::Null
        )["text"],
        LESSON.trim()
    );
    f.control
        .connection
        .execute(
            "UPDATE allowance_periods SET closed_at=? WHERE user_id='A' AND closed_at IS NULL",
            [multi_user_host::now()],
        )
        .unwrap();
    let retained = f.ok(&f.a, "GET", &instance_path(&used), Value::Null);
    assert_eq!(retained["instance"]["release_id"], used["instance"]["release_id"]);
    assert_eq!(retained["instance"]["version"], used["instance"]["version"]);
    assert_eq!(
        retained["instance"]["release"]["manifest"],
        used["instance"]["release"]["manifest"]
    );
    let workspace_path = format!(
        "/api/learning-workspaces/{}",
        used["workspace"]["learning_workspace_id"].as_str().unwrap()
    );
    let retained_workspace = f.ok(&f.a, "GET", &workspace_path, Value::Null);
    assert_eq!(retained_workspace["goal"], used["workspace"]["goal"]);
    assert_eq!(retained_workspace["members"][0]["readable"], false);
    assert_eq!(
        f.call(
            &f.a,
            "GET",
            &format!("{}?lid=1", text.url("text")),
            Value::Null
        )
        .0,
        404
    );
    assert!(f
        .ok(&f.a, "GET", "/api/me/reading-resumption", Value::Null)
        .is_null());
    f.access.library.lock().unwrap().grant("A", &text).unwrap();
    assert_eq!(
        f.call(
            &f.a,
            "GET",
            &format!("{}?lid=1", text.url("text")),
            Value::Null
        )
        .0,
        200,
        "a separate explicit grant keeps its original meaning"
    );
    assert_eq!(f.call(&f.a, "POST", path, json!({"operation_id":"cloud-expiry-new-use","goal":{"text":"不能把旧实例元数据当成新的课程授权"}})).0, 404);
    assert_eq!(
        f.ok(&f.a, "GET", &workspace_path, Value::Null)["members"][0]["readable"],
        true
    );

    // The published links are external navigation only; executable/local schemes
    // cannot become browser actions through the ordinary operator input.
    publication["operation_id"] = json!("cloud-external-invalid");
    publication["release_id"] = json!("cloud-external-invalid");
    publication["manifest"]["external_resources"][0]["url"] = json!("javascript:alert(1)");
    assert_eq!(
        f.call(&f.b, "POST", "/api/admin/workspace-releases", publication)
            .0,
        400
    );
}
