use super::*;
use super::mu5_tests::Fixture;

#[test]
fn video_playback_head_and_segments_require_current_publication_access() {
    let f = Fixture::new();
    let input = f.root.path().join("playback-video");
    super::video_draft_tests::fixture(&input);
    let original = std::fs::read(input.join("media/original.mp4")).unwrap();
    let mut library = f.access.library.lock().unwrap();
    let publication = library.publish(&input).unwrap();
    library.grant("A", &publication.reference).unwrap();
    let (_, cache) = library.video_directories("A", &publication.reference).unwrap();
    // The deliberately invalid fixture must not leave a partially prepared cache.
    assert!(!cache.exists());
    assert_eq!(std::fs::read(input.join("media/original.mp4")).unwrap(), original);
    std::fs::create_dir_all(&cache).unwrap();
    std::fs::write(cache.join("info.json"), r#"{"time_base":[1,16000],"timeline":{"time_base":[1,16000],"origin_ms":0,"video_start_ms":0,"audio_start_ms":null},"timeline_aligned":true}"#).unwrap();
    std::fs::write(cache.join("index.m3u8"), "#EXTM3U\n#EXT-X-ENDLIST").unwrap();
    std::fs::write(cache.join("media.mp4"), b"0123456789").unwrap();
    let reused = crate::video_playback::prepare(f.root.path(), &input).unwrap().unwrap();
    assert_eq!(std::fs::canonicalize(reused).unwrap(), cache);
    drop(library);
    let request = |token: &str, leaf: &str| crate::multi_user_host::dispatch(
        &f.access, &crate::multi_user_host::Site::new("https://reader.example").unwrap(), "GET", &publication.reference.url(leaf),
        &crate::multi_user_host::Headers(vec![("Host".into(), "reader.example".into()),
            ("Cookie".into(), format!("{}={token}", crate::auth::COOKIE))]), "", crate::multi_user_host::now());
    let head = request(&f.a, "video_playback");
    assert_eq!(head.status, 200);
    let head: Value = serde_json::from_slice(&head.body).unwrap();
    assert_eq!(head["hls"], true);
    assert_eq!(head["time_base"], json!([1,16000]));
    assert!(head.get("map").is_none());
    for leaf in ["video_playback", "video/playback/index.m3u8", "video/playback/media.mp4", "video/media"] {
        assert_eq!(request(&f.a, leaf).status, 200, "{leaf}");
        assert_eq!(request(&f.b, leaf).status, 404, "{leaf}");
    }
    f.control.connection.execute("DELETE FROM book_grants WHERE owner_user_id='A' AND book_id='vi6'", []).unwrap();
    assert_eq!(request(&f.a, "video/playback/media.mp4").status, 404);
}

#[test]
fn video_catalog_uses_visible_course_name_for_temporary_import_filename() {
    let mut f = Fixture::new();
    f.control.set_reader_admin("B", true).unwrap();
    let input = f.root.path().join("temporary-video-title");
    super::video_draft_tests::fixture(&input);
    let source_path = input.join("video_source_manifest.json");
    let mut source: Value = serde_json::from_slice(&std::fs::read(&source_path).unwrap()).unwrap();
    source["source"]["original_path"] = json!(r"E:\generated\vi2\external.mp4");
    std::fs::write(&source_path, source.to_string()).unwrap();
    let mut library = f.access.library.lock().unwrap();
    let publication = library.publish(&input).unwrap();
    library.grant("A", &publication.reference).unwrap();
    library.grant("B", &publication.reference).unwrap();
    drop(library);
    let mut request = json!({"operation_id":"video-title-private","release_id":"video-course","expected_version":0,"visible_to_users":["B"],"manifest":{
        "name":"CS336 第 2 讲：PyTorch 与资源核算","publisher":"课程运营","public_goal":"理解张量运算","applicability":"课程学习","prerequisites":"Python",
        "materials":[{"published_book_ref":publication.reference,"required_capabilities":["text","video"]}],"topic_links":[],"suggested_route":[]}});
    f.ok(&f.b,"POST","/api/admin/workspace-releases",request.clone());
    let title = |owner: &str| f.access.library.lock().unwrap().list(owner).unwrap().into_iter()
        .find(|book| book["published_book_ref"] == json!(publication.reference)).unwrap()["title"].clone();
    assert_eq!(title("A"), "external");
    assert_eq!(title("B"), "CS336 第 2 讲：PyTorch 与资源核算");
    request["operation_id"] = json!("video-title-public");
    request["expected_version"] = json!(1);
    request["visible_to_users"] = json!(["A","B"]);
    f.ok(&f.b,"POST","/api/admin/workspace-releases",request);
    assert_eq!(title("A"), "CS336 第 2 讲：PyTorch 与资源核算");
}

#[test]
fn video_catalog_displays_original_title_and_published_frame() {
    let f = Fixture::new();
    let input = f.root.path().join("video-library-display");
    super::video_draft_tests::fixture(&input);
    let source_path = input.join("video_source_manifest.json");
    let mut source: Value = serde_json::from_slice(&std::fs::read(&source_path).unwrap()).unwrap();
    source["source"]["original_path"] = json!(r"E:\courses\CS336_Lecture02_PyTorch.mp4");
    std::fs::write(&source_path, source.to_string()).unwrap();
    batch(&input, "visual", "visual_index");
    let mut library = f.access.library.lock().unwrap();
    let publication = library.publish(&input).unwrap();
    library.grant("A", &publication.reference).unwrap();
    let catalog = library.list("A").unwrap();
    let video = catalog.iter().find(|book| book["published_book_ref"] == json!(publication.reference)).unwrap();
    assert_eq!(video["title"], "CS336 Lecture02 PyTorch");
    assert_eq!(video["media_kind"], "video");
    assert_eq!(video["cover"]["kind"], "image");
    assert_eq!(video["cover"]["url"], publication.reference.url("video/cover"));
    let cover_url = video["cover"]["url"].as_str().unwrap().to_owned();
    drop(library);
    let request = |token: &str| crate::multi_user_host::dispatch(
        &f.access, &crate::multi_user_host::Site::new("https://reader.example").unwrap(), "GET", &cover_url,
        &crate::multi_user_host::Headers(vec![("Host".into(), "reader.example".into()),
            ("Cookie".into(), format!("{}={token}", crate::auth::COOKIE))]), "", crate::multi_user_host::now());
    let response = request(&f.a);
    assert_eq!(response.status, 200);
    assert_eq!(response.content_type, "image/png");
    assert_eq!(response.body, std::fs::read(input.join("video-enrichments/visual/frame.png")).unwrap());
    assert_eq!(request(&f.b).status, 404);
}

#[test]
fn video_publication_discourse_requires_artifact_and_preserves_other_contributions() {
    let f=Fixture::new();let input=f.root.path().join("video-discourse");
    super::video_draft_tests::fixture(&input);
    let mut library=f.access.library.lock().unwrap();
    let original=library.publish(&input).unwrap();library.grant("A",&original.reference).unwrap();
    batch(&input,"missing-discourse","discourse");assert!(library.publish(&input).is_err());
    let manifest:Value=serde_json::from_slice(&std::fs::read(input.join("video_source_manifest.json")).unwrap()).unwrap();
    let map:Value=serde_json::from_slice(&std::fs::read(input.join("transcript_map.json")).unwrap()).unwrap();
    let evidence=json!({"support_role":"原句描述","target":{"kind":"text","publication":original.reference,
        "transcript":manifest["material"]["transcript"],"text_range":map["segments"][0]["text_range"]}});
    let contribution=json!({"version":"video_discourse.v1","material":manifest["material"],"input_publication":original.reference,
        "items":[{"id":"discourse:d:1","mode":"descriptive","local_function":"description","local_summary":"frame 描述",
            "vu_ids":[],"evidence":[evidence],"relations":[]}],"visual_units":[],"frames":[]});
    std::fs::create_dir_all(input.join("video-enrichments/d")).unwrap();
    std::fs::write(input.join("video-enrichments/d/discourse.json"),contribution.to_string()).unwrap();
    let catalog=json!({"version":"video_enrichments.v1","material":manifest["material"],"batches":[{
        "batch_id":"d","coverage":[{"capability":"discourse","accepted":[map["segments"][0]["location"]],"pending":[],"failed":[]}],"files":["video-enrichments/d/discourse.json"]}]});
    std::fs::write(input.join("video_enrichments.json"),catalog.to_string()).unwrap();
    let second=library.publish(&input).unwrap();
    let loaded=library.load("A",&second.reference).unwrap();
    let query:read_tools::video_search::VideoSearch=serde_json::from_value(json!({"query":"frame"})).unwrap();
    let result=loaded.book.search_video(loaded.directory(),&second.reference,query).unwrap();
    assert_eq!(result["discourse"]["total_matches"],1);assert_eq!(result["discourse"]["candidates"][0]["text_reads"][0]["segment_id"],"s");
    batch(&input,"zh","translation");let third=library.publish(&input).unwrap();
    assert!(third.files.contains_key("video-enrichments/d/discourse.json"));
    assert!(third.files.contains_key("video-enrichments/zh/transcript_translations.json"));
    assert!(!original.files.contains_key("video-enrichments/d/discourse.json"));
    assert_eq!(library.load("A",&third.reference).unwrap().book.text("1",None).unwrap(),"A frame.");
}

pub(super) fn batch(dir: &Path, id: &str, capability: &str) {
    let manifest: Value = serde_json::from_slice(&std::fs::read(dir.join("video_source_manifest.json")).unwrap()).unwrap();
    let prefix = format!("video-enrichments/{id}");
    std::fs::create_dir_all(dir.join(&prefix)).unwrap();
    let files = if capability == "translation" {
        let file = format!("{prefix}/transcript_translations.json");
        std::fs::write(dir.join(&file), json!([{"transcript":manifest["material"]["transcript"],"segment_id":"s","zh_text":"一帧画面。"}]).to_string()).unwrap();
        vec![file]
    } else {
        let frame = format!("{prefix}/frame.png");
        let index = format!("{prefix}/index.json");
        use base64::Engine;
        std::fs::write(dir.join(&frame), base64::engine::general_purpose::STANDARD.decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aV1cAAAAASUVORK5CYII=").unwrap()).unwrap();
        std::fs::write(dir.join(&index), json!({"frame":frame,"location":{"video":manifest["material"]["video"],"actual_pts_ms":0,"width":1920,"height":1080},"description":"VI9 dependency fixture"}).to_string()).unwrap();
        vec![frame, index]
    };
    std::fs::write(dir.join("video_enrichments.json"), json!({"version":"video_enrichments.v1","material":manifest["material"],"batches":[{
        "batch_id":id,"coverage":[{"capability":capability,"accepted":[{"video":manifest["material"]["video"],"start_ms":0,"end_ms":510000}],"pending":[],"failed":[]}],"files":files
    }]}).to_string()).unwrap();
}

#[test]
fn video_publication_interleaved_batches_missing_dependencies_and_authorized_chain() {
    let f = Fixture::new(); let input = f.root.path().join("video");
    super::video_draft_tests::fixture(&input);
    let mut library = f.access.library.lock().unwrap();
    let p1 = library.publish(&input).unwrap(); library.grant("A", &p1.reference).unwrap();
    batch(&input, "zh", "translation");
    let p2 = library.publish(&input).unwrap();
    batch(&input, "visual", "visual_index"); // Producer based on p1 knows nothing about zh.
    let p3 = library.publish(&input).unwrap();
    assert_eq!(p3.previous_publication, Some(p2.reference.clone()));
    assert!(p3.files.contains_key("video-enrichments/zh/transcript_translations.json"));
    assert!(p3.files.contains_key("video-enrichments/visual/frame.png"));
    let book = library.load("A", &p3.reference).unwrap();
    let view = crate::video_publication::projection(&book.book, book.directory()).unwrap();
    assert_eq!(view["translations"][0]["zh_text"], "一帧画面。");
    assert_eq!(view["coverage"].as_array().unwrap().len(), 3);
    library.set_default(&p1.reference).unwrap();
    assert_eq!(library.video_successor("A", &p1.reference).unwrap().unwrap().reference, p3.reference);
    assert!(library.video_successor("A", &p3.reference).unwrap().is_none());
    assert!(library.load("B", &p3.reference).is_err());
    batch(&input, "missing", "visual_index");
    std::fs::remove_file(input.join("video-enrichments/missing/frame.png")).unwrap();
    assert!(library.publish(&input).is_err());
    assert!(library.video_successor("A", &p3.reference).unwrap().is_none());
    // A late old batch still cannot reduce the accumulated snapshot.
    batch(&input, "zh", "translation");
    let p4 = library.publish(&input).unwrap();
    assert!(p4.files.contains_key("video-enrichments/visual/frame.png"));
    assert_eq!(p4.capabilities["coverage"], p3.capabilities["coverage"]);
    let mut manifest: Value = serde_json::from_slice(&std::fs::read(input.join("video_source_manifest.json")).unwrap()).unwrap();
    manifest["material"]["transcript"]["transcript_revision"] = json!("2");
    std::fs::write(input.join("video_source_manifest.json"), manifest.to_string()).unwrap();
    assert!(library.publish(&input).is_err());
}

#[test]
fn video_publication_translation_progress_failure_and_cumulative_acceptance() {
    let f = Fixture::new(); let input = f.root.path().join("video-progress");
    super::video_draft_tests::fixture(&input);
    let manifest: Value = serde_json::from_slice(&std::fs::read(input.join("video_source_manifest.json")).unwrap()).unwrap();
    let mut progress = json!({"version":"video_translation_progress.v1","material":manifest["material"],
        "tasks":[{"id":"translation-1","segment_ids":["s"],"status":"failed","attempts":1,"failure":"interrupted"}]});
    let file = input.join("video_translation_progress.json");
    std::fs::write(&file, progress.to_string()).unwrap();
    let mut library = f.access.library.lock().unwrap();
    let before = library.publish(&input).unwrap(); library.grant("A", &before.reference).unwrap();
    let book = library.load("A", &before.reference).unwrap();
    let view = crate::video_publication::projection(&book.book, book.directory()).unwrap();
    assert_eq!(view["translation_progress"]["tasks"][0]["status"], "failed");
    assert!(view["translations"].as_array().unwrap().is_empty());
    batch(&input, "zh", "translation");
    let accepted = library.publish(&input).unwrap();
    let book = library.load("A", &accepted.reference).unwrap();
    let view = crate::video_publication::projection(&book.book, book.directory()).unwrap();
    assert_eq!(view["translation_progress"]["tasks"][0]["status"], "accepted");
    assert!(view["translation_progress"]["tasks"][0]["failure"].is_null());
    std::fs::remove_file(&file).unwrap();
    batch(&input, "visual", "visual_index");
    let visual = library.publish(&input).unwrap();
    assert!(visual.files.contains_key("video_translation_progress.json"));
    progress["material"]["transcript"]["transcript_revision"] = json!("other");
    std::fs::write(&file, progress.to_string()).unwrap();
    assert!(library.publish(&input).is_err());
}

#[test]
fn video_publication_graph_support_is_cumulative_and_invalid_sources_block_publication() {
    let f=Fixture::new();let input=f.root.path().join("video-graph");super::video_draft_tests::fixture(&input);
    let mut library=f.access.library.lock().unwrap();
    let first=library.publish(&input).unwrap();library.grant("A",&first.reference).unwrap();
    let manifest:Value=serde_json::from_slice(&std::fs::read(input.join("video_source_manifest.json")).unwrap()).unwrap();
    let prefix="video-enrichments/graph";std::fs::create_dir_all(input.join(prefix)).unwrap();
    let frame=json!({"video":manifest["material"]["video"],"actual_pts_ms":0,"width":1920,"height":1080});
    let support=json!({"support_role":"visible code","target":{"kind":"visual","publication":first.reference,"frames":[frame],"regions":[]}});
    let mut graph=json!({"version":"video_graph.v1","material":manifest["material"],"input_publication":first.reference,
      "nodes":[{"id":"video:graph:x","type":"entity","name":"x","identity":{"kind":"video_object","meaning":"input matrix"},"occurrences":[],"source_lid":null,"evidence":[support]}],"edges":[],"frames":[{"frame":frame,"path":format!("{prefix}/frame.png")}]});
    let mut png=vec![0u8;24];png[..8].copy_from_slice(&[137,80,78,71,13,10,26,10]);png[16..20].copy_from_slice(&1920u32.to_be_bytes());png[20..24].copy_from_slice(&1080u32.to_be_bytes());
    std::fs::write(input.join(format!("{prefix}/frame.png")),png).unwrap();
    std::fs::write(input.join(format!("{prefix}/graph.json")),graph.to_string()).unwrap();
    std::fs::write(input.join("video_enrichments.json"),json!({"version":"video_enrichments.v1","material":manifest["material"],"batches":[{
      "batch_id":"graph","coverage":[{"capability":"graph","accepted":[{"video":manifest["material"]["video"],"start_ms":0,"end_ms":1000}],"pending":[],"failed":[]}],"files":[format!("{prefix}/graph.json"),format!("{prefix}/frame.png")]}]}).to_string()).unwrap();
    let second=library.publish(&input).unwrap();
    let loaded=library.load("A",&second.reference).unwrap();
    let c=loaded.book.concept_candidates("x",None,None).unwrap();
    assert!(c.candidates[0].occurrences.is_empty());assert_eq!(c.candidates[0].video_graph.as_ref().unwrap()["evidence_status"],"candidate_only");
    assert!(library.load("A",&first.reference).unwrap().book.concept_candidates("x",None,None).is_err());
    batch(&input,"zh","translation");let third=library.publish(&input).unwrap();
    assert!(library.load("A",&third.reference).unwrap().book.concept_candidates("x",None,None).is_ok());
    // The catalog checks capability dependencies without loading the whole graph.
    // An unavailable graph frame must not make the original video unreadable.
    let directory=library.load("A",&third.reference).unwrap().directory().to_path_buf();
    let frame_path=directory.join("video-enrichments/graph/frame.png");
    let held=directory.join("graph-frame-held.png");
    std::fs::rename(&frame_path,&held).unwrap();
    let material=crate::workspace_publication::Material {published_book_ref:third.reference.clone(),required_capabilities:vec!["text".into(),"video".into(),"translation".into()]};
    assert_eq!(crate::workspace_publication::material_readiness(&f.control.connection,&material).unwrap()["missing_capabilities"],json!([]));
    std::fs::rename(held,frame_path).unwrap();
    // New contribution declares a frame that was never supplied in its dependency set.
    graph["nodes"][0]["evidence"][0]["target"]["frames"][0]["actual_pts_ms"]=json!(3000);
    graph["nodes"][0]["id"]=json!("video:bad:x");
    std::fs::create_dir_all(input.join("video-enrichments/bad")).unwrap();
    graph["frames"]=json!([]);
    std::fs::write(input.join("video-enrichments/bad/graph.json"),graph.to_string()).unwrap();
    std::fs::write(input.join("video_enrichments.json"),json!({"version":"video_enrichments.v1","material":manifest["material"],"batches":[{
      "batch_id":"bad","coverage":[{"capability":"graph","accepted":[{"video":manifest["material"]["video"],"start_ms":2000,"end_ms":4000}],"pending":[],"failed":[]}],"files":["video-enrichments/bad/graph.json"]}]}).to_string()).unwrap();
    assert!(library.publish(&input).is_err());assert!(library.video_successor("A",&third.reference).unwrap().is_none());
}

#[test]
fn video_publication_two_windows_drafts_queue_reconnect_and_original_history() {
    let f = Fixture::new(); let input = f.root.path().join("video");
    f.access.runs.configure_fake(|| Box::new(ChatStubAdapter::scripted(vec![AssistantTurn {
        provider_continuation: None, text: Some("VI9 fixture answer".into()), tool_calls: vec![], usage_total_tokens: Some(1),
    }])));
    super::video_draft_tests::fixture(&input);
    let original = f.access.library.lock().unwrap().publish(&input).unwrap().reference;
    f.access.library.lock().unwrap().grant("A", &original).unwrap();
    let mut a = f.create(&f.a, &original, "a"); a = f.action(&f.a, &a, "a", "chat/new", json!({}));
    let mut b = f.create(&f.a, &original, "b"); b = f.action(&f.a, &b, "b", "chat/new", json!({}));
    let material: Value = serde_json::from_slice(&std::fs::read(input.join("video_source_manifest.json")).unwrap()).unwrap();
    let frame = json!({"video":material["material"]["video"],"actual_pts_ms":492000,"width":1920,"height":1080});
    let mut draft = json!({"message":"","context":{"draft_id":"range","publication":original,"material":material["material"],"frame":frame,
        "region":null,"range":{"video":material["material"]["video"],"start_ms":0,"end_ms":1000}}});
    a = f.action(&f.a, &a, "a", "reader/video_draft", json!({"window_id":"a","action":"save","draft":draft}));
    batch(&input, "zh", "translation");
    let newer = f.access.library.lock().unwrap().publish(&input).unwrap().reference;
    let blocked = f.action(&f.a, &a, "a", "reader/adopt_same_source_publication", json!({}));
    assert_eq!(blocked["result"]["status"], "busy"); assert_eq!(blocked["published_book_ref"], json!(original));
    let before = b.clone();
    b = f.action(&f.a, &b, "b", "reader/adopt_same_source_publication", json!({}));
    assert_eq!(b["published_book_ref"], json!(newer));
    for key in ["reader", "selected_chat", "generation"] { assert_eq!(b[key], before[key], "preserve {key}"); }
    draft["context"]["range"] = Value::Null; draft["context"]["region"] = json!({"frame":frame,"rect_normalized":[0.1,0.1,0.5,0.5]});
    a = f.action(&f.a, &a, "a", "reader/video_draft", json!({"window_id":"a","action":"save","draft":draft}));
    assert_eq!(f.action(&f.a, &a, "a", "reader/adopt_same_source_publication", json!({}))["result"]["status"], "busy");
    let mut request = Fixture::stamp(&a, "a"); request["session_id"] = a["selected_chat"].clone();
    request["message"] = json!("Explain"); request["client_request_id"] = json!("vi9-question");
    request["video_window_id"] = json!("a"); request["video_draft_id"] = json!("range");
    let (status, accepted) = f.call(&f.a, "POST", &format!("/api/workspaces/{}/agent/runs", a["workspace_id"].as_str().unwrap()), request);
    assert_eq!(status, 202, "{accepted}");
    a = f.get(&f.a, &a);
    assert_eq!(f.action(&f.a, &a, "a", "reader/adopt_same_source_publication", json!({}))["result"]["status"], "busy");
    *f.access.workspaces.lock().unwrap() = crate::workspace_registry::WorkspaceRegistry::new(crate::control_store::ControlStore::open(f.access.users.lock().unwrap().writer()).unwrap());
    a = f.get(&f.a, &a); a = f.action(&f.a, &a, "reconnected", "attach", json!({}));
    assert_eq!(f.action(&f.a, &a, "reconnected", "reader/adopt_same_source_publication", json!({}))["result"]["status"], "busy");
    let cancel = f.call(&f.a, "POST", &format!("/api/agent/runs/{}/cancel", accepted["turn_id"].as_str().unwrap()), json!({}));
    assert!(cancel.0 < 300, "{cancel:?}"); f.access.runs.run_one(&f.access).unwrap();
    a = f.get(&f.a, &a);
    // Persist-failed results remain in-flight for adoption even after execution ends.
    f.control.connection.execute("UPDATE run_admissions SET unsaved=1 WHERE turn_id=?", [accepted["turn_id"].as_str().unwrap()]).unwrap();
    assert_eq!(f.action(&f.a, &a, "reconnected", "reader/adopt_same_source_publication", json!({}))["result"]["status"], "busy");
    f.control.connection.execute("UPDATE run_admissions SET unsaved=0 WHERE turn_id=?", [accepted["turn_id"].as_str().unwrap()]).unwrap();
    a = f.action(&f.a, &a, "reconnected", "reader/adopt_same_source_publication", json!({}));
    assert_eq!(a["published_book_ref"], json!(newer));
    let history = f.action(&f.a, &a, "reconnected", "chat/history", json!({}));
    assert_eq!(history["result"]["current"]["turns"][0]["video_question"]["publication"], json!(original));
    let late = f.action(&f.a, &a, "reconnected", "reader/adopt_same_source_publication", json!({"publication_id":original.publication_id}));
    assert_eq!(late["result"]["status"], "current"); assert_eq!(late["result"]["video"]["publication"], json!(newer));
    // A source revision is a separate book and never an automatic successor.
    let revised = f.root.path().join("revised"); super::video_draft_tests::fixture(&revised);
    for file in ["base.json", "video_source_manifest.json", "transcript_map.json"] {
        let path = revised.join(file); let mut value: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        match file {
            "base.json" => value["book_id"] = json!("vi6-revised"),
            "video_source_manifest.json" => { value["material"]["book_id"] = json!("vi6-revised"); value["previous_book_id"] = json!("vi6"); value["material"]["transcript"]["transcript_revision"] = json!("2"); },
            _ => value["transcript"]["transcript_revision"] = json!("2"),
        }
        std::fs::write(path, value.to_string()).unwrap();
    }
    let revision = f.access.library.lock().unwrap().publish(&revised).unwrap().reference;
    f.access.library.lock().unwrap().grant("A", &revision).unwrap();
    assert!(f.access.library.lock().unwrap().video_successor("A", &newer).unwrap().is_none());
    let changed = f.action(&f.a, &a, "reconnected", "book/open", json!({"published_book_ref":revision}));
    assert!(changed["generation"].as_u64().unwrap() > a["generation"].as_u64().unwrap());
}

#[test]
fn video_publication_local_pins_resume_and_preserves_reader_chat_and_draft() {
    let f = Fixture::new(); let input = f.root.path().join("video"); super::video_draft_tests::fixture(&input);
    let original = f.access.library.lock().unwrap().publish(&input).unwrap().reference;
    f.access.library.lock().unwrap().grant("A", &original).unwrap();
    let old = f.access.library.lock().unwrap().load("A", &original).unwrap();
    let mut state = state_named("vi9-local");
    state.workspace = reader_workspace::ReaderWorkspace::local(old.directory().into(), old.book.clone(), Reader::new(&old.book,DEFAULT_RADIUS),vec![],Some(f.root.path().join("local-session.json")));
    state.workspace.restore_chat(&mut state.user.agent_history,"now");
    let chat = state.workspace.selected_chat.clone(); let generation = state.workspace.generation;
    let reader = serde_json::to_value(state.workspace.reader.checkpoint()).unwrap();
    batch(&input,"zh","translation");
    let newer = f.access.library.lock().unwrap().publish(&input).unwrap().reference;
    let latest = f.access.library.lock().unwrap().load("A", &newer).unwrap();
    reader_service::tests::host_calls::video_publication::persist_local_binding(&state.workspace).unwrap();
    assert_eq!(reader_service::tests::host_calls::video_publication::local_resume_directory(latest.directory(), &state.workspace.session_path), old.directory());
    let result = reader_service::tests::host_calls::video_publication::adopt_local(&mut state).unwrap();
    assert_eq!(result["status"], "adopted");
    assert_eq!(state.workspace.selected_chat,chat); assert_eq!(state.workspace.generation,generation);
    assert_eq!(serde_json::to_value(state.workspace.reader.checkpoint()).unwrap(), reader);
    assert!(state.workspace.publication.is_none(), "local ordinary book switching stays available");
    assert_eq!(reader_service::tests::host_calls::video_publication::local_resume_directory(latest.directory(), &state.workspace.session_path), latest.directory());
    assert_eq!(reader_service::tests::host_calls::video_publication::adopt_local(&mut state).unwrap()["status"], "current");
}

#[test]
#[ignore = "VI9 real-media browser host, driven by evals/video/vi9-browser.mjs"]
fn video_publication_browser_host() {
    let root = PathBuf::from(std::env::var_os("VI9_OUTPUT").expect("VI9_OUTPUT"));
    let input = PathBuf::from(std::env::var_os("VI9_WORKSPACE").expect("VI9_WORKSPACE"));
    let f = Fixture::new();
    let initial = f.access.library.lock().unwrap().publish(&input).unwrap();
    f.access.library.lock().unwrap().grant("A", &initial.reference).unwrap();
    std::fs::write(root.join("initial.json"), serde_json::to_vec_pretty(&initial).unwrap()).unwrap();
    struct HeldAnswer(PathBuf);
    impl ModelAdapter for HeldAnswer {
        fn complete(&self, _: CompletionRequest) -> Result<ParsedResponse, AdapterError> { Err(AdapterError { spend_stop: None, message: "fixture".into() }) }
        fn chat(&self, _: &AgentRequestPlan) -> Result<AssistantTurn, AdapterError> {
            std::fs::write(self.0.join("answer-started"), "started").unwrap();
            let deadline = std::time::Instant::now() + Duration::from_secs(120);
            while !self.0.join("release-answer").exists() && std::time::Instant::now() < deadline { std::thread::sleep(Duration::from_millis(100)); }
            Ok(AssistantTurn { provider_continuation: None, text: Some("VI9 传输验收回答：问题保持原发布。".into()), tool_calls: vec![], usage_total_tokens: Some(1) })
        }
    }
    let model_root = root.clone(); f.access.runs.configure_fake(move || Box::new(HeldAnswer(model_root.clone())));
    let server = crate::multi_user_host::start_with_access("127.0.0.1:8794".parse().unwrap(),
        Arc::new(crate::multi_user_host::Site::new("https://localhost:5176").unwrap()), f.access.clone()).unwrap();
    std::fs::write(root.join("ready"), "ready").unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(900);
    let mut handled = String::new();
    while !root.join("stop").exists() && std::time::Instant::now() < deadline {
        if let Ok(bytes) = std::fs::read(root.join("command.json")) {
            if let Ok(command) = serde_json::from_slice::<Value>(&bytes) {
                let id = command["id"].as_str().unwrap().to_string();
                if handled != id {
                    let path = command["input"].as_str().map(PathBuf::from).unwrap_or_else(|| input.clone());
                    let start = std::time::Instant::now();
                    let result = f.access.library.lock().unwrap().publish(&path);
                    let result = match result { Ok(p) => { f.access.library.lock().unwrap().grant("A", &p.reference).unwrap(); json!({"manifest":p}) }, Err(e) => json!({"error":e.error_code}) };
                    std::fs::write(root.join(format!("result-{id}.json")), json!({"id":id,"elapsed_ms":start.elapsed().as_millis(),"result":result}).to_string()).unwrap();
                    handled = id;
                }
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    server.shutdown();
}
