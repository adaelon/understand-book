use super::mu5_tests::Fixture;
use super::*;
use runtime::run_context::ResidentStatePort;

#[test]
fn video_workspace_cross_frames_keep_publication_and_frozen_membership() {
    let f=super::b1_operator_build_tests::fixture();
    let dir=f.root.path().join("cross-video");
    let(book,_,request)=crate::video_frames::tests::fixture(&dir);
    std::fs::write(dir.join("base.json"),serde_json::to_vec(&book.base).unwrap()).unwrap();
    std::fs::write(dir.join("source.txt"),"A frame.").unwrap();
    let reference=f.access.library.lock().unwrap().publish_online(&crate::online_publication::PublicationRequest {operation_id:"cross-frames".into(),input_directory:dir,grant_users:vec!["A".into()]}).unwrap().reference;
    let space=f.ok(&f.a,"POST","/api/learning-workspaces",json!({"goal":{"text":"结合书籍和真实视频画面学习"},"members":[f.x,reference]}));
    let scene=f.ok(&f.a,"POST","/api/workspaces",json!({"published_book_ref":f.x,"attachment_id":"cross","learning_workspace_id":space["learning_workspace_id"]}));
    let scene=f.action(&f.a,&scene,"cross","chat/new",json!({}));let mut run=f.run(&scene,"cross");
    let list=run.library_read("library.list",json!({})).unwrap();
    assert!(list.body["items"].as_array().unwrap().iter().any(|v|v["video"]["published_book_ref"]==json!(reference)));
    let original_book=run.scope.publication.as_ref().unwrap().reference.clone();
    let frame=run.read_video_frames(Some(&reference),request.clone(),&Default::default()).unwrap();
    assert_eq!(frame.publication,reference);assert_eq!(frame.images.len(),2);assert_eq!(frame.transcript[0].text,"A frame.");
    assert_eq!(run.scope.publication.as_ref().unwrap().reference,original_book);
    let mut hints=json!({"text_reads":[{"segment_id":"s","text_range":{"lid":"1","range":{"start":0,"end":8}},"location":request.location}],"read_hints":[request]});
    crate::cross_book::scope_video_hints(&mut hints,&reference);
    assert_eq!(hints["read_hints"][0]["published_book_ref"],json!(reference));
    assert_eq!(hints["text_reads"][0]["tool"],"library.read");
    assert!(run.library_read("library.read",hints["text_reads"][0]["arguments"].clone()).is_ok());
    let target=serde_json::from_value(frame.projection()["frames"][0]["source_target"].clone()).unwrap();
    let snapshot=run.media_source_snapshot(target).unwrap();assert_eq!(snapshot.publication,reference);
    assert_eq!(run.read_video_frames(Some(&f.y),request.clone(),&Default::default()).unwrap_err().error_code,"WORKSPACE_MATERIAL_UNAVAILABLE");
    let query=serde_json::from_value(json!({"query":"diagram"})).unwrap();assert!(run.search_video(Some(&reference),query).is_ok());
    f.access.library.lock().unwrap().revoke("A",&reference).unwrap();
    assert!(run.read_video_frames(Some(&reference),request.clone(),&Default::default()).is_err());
    assert!(run.review_video_audio(Some(&reference),serde_json::from_value(json!({"location":request.location,"question":"Which word was spoken?"})).unwrap(),&Default::default()).is_err());
}

#[test]
#[ignore = "uses an actually prepared short video and installed local Whisper"]
fn video_workspace_cross_original_audio_real_whisper() {
    let f=super::b1_operator_build_tests::fixture();
    let directory=std::path::PathBuf::from(std::env::var("B2_VIDEO_WORKSPACE").expect("prepared video directory"));
    let before=std::fs::read(directory.join("source.txt")).unwrap();
    let reference=f.access.library.lock().unwrap().publish_online(&crate::online_publication::PublicationRequest {operation_id:"cross-real-audio".into(),input_directory:directory.clone(),grant_users:vec!["A".into()]}).unwrap().reference;
    let source=f.access.library.lock().unwrap().load("A",&reference).unwrap();let(manifest,_)=source.book.load_video_source(source.directory()).unwrap();
    let space=f.ok(&f.a,"POST","/api/learning-workspaces",json!({"goal":{"text":"对照书籍与原视频解释张量"},"members":[f.x,reference]}));
    let scene=f.ok(&f.a,"POST","/api/workspaces",json!({"published_book_ref":f.x,"attachment_id":"cross-real","learning_workspace_id":space["learning_workspace_id"]}));
    let scene=f.action(&f.a,&scene,"cross-real","chat/new",json!({}));let mut run=f.run(&scene,"cross-real");
    assert!(run.library_read("library.read",json!({"published_book_ref":f.x,"operation":"text","lid":"1.1"})).is_ok());
    let location=json!({"video":manifest.source.video,"start_ms":0,"end_ms":manifest.source.duration_ms.min(30000.0)});
    let frames=run.read_video_frames(Some(&reference),serde_json::from_value(json!({"location":location,"sampling":[1000]})).unwrap(),&Default::default()).unwrap();
    let review=run.review_video_audio(Some(&reference),serde_json::from_value(json!({"location":location,"question":"What does the speaker say about a tensor?"})).unwrap(),&Default::default()).unwrap();
    assert!(!review.reviewed_text.is_empty());assert!(!review.reviewed_words.is_empty());assert_eq!(review.publication,reference);
    assert!(review.reviewed_words.iter().all(|w| w.start_ms>=0.0&&w.end_ms<=manifest.source.duration_ms+1.0));
    assert_eq!(run.scope.publication.as_ref().unwrap().reference,f.x);assert_eq!(std::fs::read(directory.join("source.txt")).unwrap(),before);
    let snapshot=run.media_source_snapshot(review.target()).unwrap();assert_eq!(snapshot.publication,reference);
    let report=json!({"cross_material":true,"current_book":f.x,"reference_video":reference,"frames":frames.projection(),"review":review,"saved_transcript_unchanged":true,"model_calls":0});
    std::fs::create_dir_all("target/b2-v1-20261018").unwrap();std::fs::write("target/b2-v1-20261018/real-cross-audio.json",serde_json::to_vec_pretty(&report).unwrap()).unwrap();
}

fn release(f: &Fixture, video: &base_schema::PublishedBookRef, version: u32) -> Value {
    let mut input = super::s1_workspace_publication_tests::publication(f, &format!("mixed-{version}"), version);
    input["release_id"] = json!("mixed-course");
    input["manifest"]["name"] = json!("书籍与视频共同学习");
    input["manifest"]["materials"] = json!([
        {"published_book_ref":f.x,"required_capabilities":["text","tutor"]},
        {"published_book_ref":video,"required_capabilities":["text","video"]}
    ]);
    input["manifest"]["topic_links"] = json!([]);
    input["manifest"]["suggested_route"] = json!([]);
    input
}

fn instance(f: &Fixture, operation: &str) -> Value {
    f.ok(&f.a,"POST","/api/workspace-releases/mixed-course/versions/1/use",
        json!({"operation_id":operation,"goal":{"text":"结合书籍与视频理解同一概念"}}))
}

#[test]
fn video_workspace_mixed_release_scope_drafts_selected_update_and_original_sources() {
    let f = super::b1_operator_build_tests::fixture();
    let dir=f.root.path().join("video"); super::video_draft_tests::fixture(&dir);
    let original=f.access.library.lock().unwrap().publish_online(&crate::online_publication::PublicationRequest {
        operation_id:"video-first".into(),input_directory:dir.clone(),grant_users:vec!["A".into(),"B".into()]
    }).unwrap().reference;
    let published=f.ok(&f.b,"POST","/api/admin/workspace-releases",release(&f,&original,0));
    assert_eq!(published["workspace_release"]["availability"]["status"],"ready");
    let caps=&published["workspace_release"]["availability"]["materials"][1]["available_capabilities"];
    assert!(caps.as_array().unwrap().contains(&json!("video")));
    assert!(!caps.as_array().unwrap().contains(&json!("tutor")));
    assert!(f.ok(&f.b,"GET","/api/admin/books",Value::Null)["books"].as_array().unwrap().iter().any(|m|m["published_book_ref"]==json!(original)&&m["material_kind"]=="video"));
    let first=instance(&f,"mixed-personal"); let second=instance(&f,"mixed-other-personal");
    let scope=first["workspace"]["learning_workspace_id"].clone();
    let make_scene=|attachment:&str| f.ok(&f.a,"POST","/api/workspaces",json!({"published_book_ref":f.x,"attachment_id":attachment,"learning_workspace_id":scope}));
    let scene=make_scene("book"); let scene=f.action(&f.a,&scene,"book","chat/new",json!({}));
    let mut run=f.run(&scene,"book");
    let list=run.library_read("library.list",json!({})).unwrap();
    assert!(list.body.to_string().contains(&original.publication_id));
    assert!(run.library_read("library.read",json!({"published_book_ref":f.y,"operation":"text","lid":"1.1"})).is_err());
    let range=EvidenceRange {start_lid:"1".into(),end_lid:"1".into(),ranges:vec![read_tools::SourceSelectedRange {lid:"1".into(),range:read_tools::SourceTextRange {start:0,end:8}}]};
    let media=run.library_media_source_for_text(&original,&range).unwrap().unwrap();
    assert_eq!(media.publication,original);
    assert!(run.library_media_source_for_text(&f.y,&range).is_err());
    f.access.library.lock().unwrap().revoke("A",&original).unwrap();
    assert!(run.library_media_source_for_text(&original,&range).is_err());
    f.access.library.lock().unwrap().grant("A",&original).unwrap();
    let handle=f.access.users.lock().unwrap().get("A","now").unwrap();
    let turn=AgentTurnRef {session_id:run.scope.chat_session_id.clone(),turn_id:run.scope.turn_id.clone(),user_turn_ordinal:1};
    let revision=f.access.library.lock().unwrap().load("A",&original).unwrap().book.source_fingerprint().to_owned();
    let binding=SourceBinding { music: None,publication_id:Some(original.publication_id.clone()),source_revision:Some(revision),
        source_ref_id:"mixed-video-source".into(),book_id:original.book_id.clone(),evidence_range:None,evidence_text_digest:String::new(),
        media:Some(media),label_snapshot:"原视频英文".into(),preview_snapshot:"A frame.".into()};
    crate::session_runtime::append(&mut handle.lock().unwrap(),&turn,"video-source",crate::session_event::EventBody::SourcesBound {bindings:vec![binding.clone()]}).unwrap();
    let observation=run.library_learning(json!({"published_book_ref":original,"operation":"observe","operation_id":"video-understanding",
        "source_ref_ids":["mixed-video-source"],"learner_quote":"Explain this","learning_focus":"视频英文含义","expected_performance":"解释这一句","capability":"explanation",
        "interpretation":"用户请求解释视频原句","teaching_implication":"联系书中的概念解释原句"}),std::slice::from_ref(&binding)).unwrap();
    let evidence_id=observation["evidence_ref"].as_str().unwrap();
    let user=handle.lock().unwrap();
    assert!(user.learning_store_for_workspace(scope.as_str()).unwrap().evidence(evidence_id).is_ok());
    assert!(user.learning_store_for_workspace(second["workspace"]["learning_workspace_id"].as_str()).unwrap().evidence(evidence_id).is_err());
    drop(user);
    let scene=f.action(&f.a,&scene,"book","book/open",json!({"published_book_ref":original}));
    let material=f.ok(&f.a,"GET",&original.url("video_transcript"),Value::Null);
    assert_eq!(material["material"]["book_id"],original.book_id);
    let draft=json!({"message":"解释视频中的这一句","context":{"draft_id":"scoped-video-draft","publication":original,
        "material":material["material"],"frame":{"video":material["material"]["video"],"actual_pts_ms":1000.0,"width":1920,"height":1080},"region":null,"range":null}});
    let scene=f.action(&f.a,&scene,"book","reader/video_draft",json!({"window_id":"same-window","action":"save","draft":draft}));
    let other=f.action(&f.a,&scene,"book","book/open",json!({"published_book_ref":original,"learning_workspace_id":second["workspace"]["learning_workspace_id"]}));
    assert!(f.action(&f.a,&other,"book","reader/video_draft",json!({"window_id":"same-window","action":"get"}))["result"]["draft"].is_null());
    let wrong_source=Fixture::stamp(&other,"book");
    let mut wrong_source=wrong_source;wrong_source["turn_id"]=json!(turn.turn_id);wrong_source["source_ref_id"]=json!("mixed-video-source");
    assert_ne!(f.call(&f.a,"POST",&format!("/api/workspaces/{}/agent/source.resolve",other["workspace_id"].as_str().unwrap()),wrong_source).0,200);
    let scene=f.action(&f.a,&other,"book","book/open",json!({"published_book_ref":original,"learning_workspace_id":scope}));
    assert_eq!(f.action(&f.a,&scene,"book","reader/video_draft",json!({"window_id":"same-window","action":"get"}))["result"]["draft"],draft);
    super::video_publication_tests::batch(&dir,"zh","translation");
    let newer=f.access.library.lock().unwrap().publish_online(&crate::online_publication::PublicationRequest {
        operation_id:"video-next".into(),input_directory:dir,grant_users:vec!["A".into(),"B".into()]
    }).unwrap().reference;
    let guarded=f.action(&f.a,&scene,"book","reader/adopt_same_source_publication",json!({}));
    assert_eq!(guarded["result"]["status"],"selection_required");
    assert_eq!(guarded["published_book_ref"],json!(original));
    f.ok(&f.b,"POST","/api/admin/workspace-releases",release(&f,&newer,1));
    let instance_path=format!("/api/learning-workspaces/{}/shared-release",scope.as_str().unwrap());
    let preview=f.ok(&f.a,"GET",&format!("{instance_path}/preview?version=2"),Value::Null);
    assert!(preview["changes"]["added_materials"].as_array().unwrap().contains(&json!(newer)));
    assert_eq!(f.ok(&f.a,"GET",&instance_path,Value::Null)["instance"]["version"],1);
    f.ok(&f.a,"POST",&format!("{instance_path}/update"),json!({"operation_id":"video-select-v2","expected_version":1,"version":2}));
    let updated=f.action(&f.a,&scene,"book","book/open",json!({"published_book_ref":newer}));
    let source=f.action(&f.a,&updated,"book","agent/source.resolve",json!({"turn_id":turn.turn_id,"source_ref_id":"mixed-video-source"}));
    assert_eq!(source["result"]["media"]["publication"],json!(original));
    assert_eq!(source["result"]["stale"],false);
    assert_eq!(handle.lock().unwrap().agent_history.sessions.iter().find(|s|s.id==turn.session_id).unwrap().turns[0].source_bindings[0],binding);
    assert_eq!(f.ok(&f.a,"GET",&format!("/api/learning-workspaces/{}/shared-release",second["workspace"]["learning_workspace_id"].as_str().unwrap()),Value::Null)["instance"]["version"],1);
}

#[test]
fn video_workspace_operator_reuses_prebuilt_video_and_publishes_without_new_calls() {
    let f=super::b1_operator_build_tests::fixture();
    let dir=f.root.path().join("video");super::video_draft_tests::fixture(&dir);
    let video=f.access.library.lock().unwrap().publish(&dir).unwrap().reference;
    for (index,target) in [dir.join("media/original.mp4"),dir.clone()].into_iter().enumerate() {
        let mut input=super::b1_operator_build_tests::create(&f,&format!("video-input-{index}"));
        input["materials"][1]["target_input"]=json!(target);
        let created=f.ok(&f.b,"POST","/api/admin/operator-builds",input);
        assert_eq!(created["operator_build"]["materials"][1]["material_kind"],"video");
        assert_eq!(created["operator_build"]["materials"][1]["build_tutor"],false);
    }
    let task="mixed-operator";
    f.ok(&f.b,"POST","/api/admin/operator-builds",json!({"operation_id":"create-mixed-operator","task_id":task,"name":"书和视频公共准备",
        "max_micro_cny":1000,"reserved_per_execution_micro_cny":100,"visible_to_users":["A","B"],"materials":[
            {"material_id":"book","label":"书籍","published_book_ref":f.x,"required_capabilities":["text","tutor"]},
            {"material_id":"video","label":"视频","published_book_ref":video,"required_capabilities":["text","video"]}]}));
    assert_eq!(super::b1_operator_build_tests::transition(&f,task,"prepare")["state"],"awaiting_confirmation");
    assert_eq!(super::b1_operator_build_tests::transition(&f,task,"start")["state"],"ready");
    let mut composition=release(&f,&video,0);composition.as_object_mut().unwrap().remove("operation_id");
    let result=f.ok(&f.b,"POST",&format!("/api/admin/operator-builds/{task}/publish"),json!({"operation_id":"publish-mixed-operator","release":composition}));
    assert_eq!(result["workspace_release"]["availability"]["status"],"ready");
    assert_eq!(instance(&f,"mixed-operator-use")["workspace"]["members"].as_array().unwrap().len(),2);
    assert_eq!(f.control.connection.query_row("SELECT count(*) FROM model_call_charges",[],|r|r.get::<_,i64>(0)).unwrap(),0);
}

#[test]
fn video_workspace_tutor_readiness_uses_current_native_products_after_old_publication() {
    let f = super::b1_operator_build_tests::fixture();
    let directory = f.root.path().join("video-tutor");
    super::tutor_tests::video_tutor_foundation(&directory);
    let publication = f.access.library.lock().unwrap().publish(&directory).unwrap();
    f.access.library.lock().unwrap().grant("A", &publication.reference).unwrap();
    let loaded = f.access.library.lock().unwrap().load("A", &publication.reference).unwrap();
    let material = crate::workspace_publication::Material {published_book_ref:publication.reference.clone(), required_capabilities:vec!["video".into(), "tutor".into()]};
    let read = || crate::workspace_publication::material_readiness(&f.control.connection, &material).unwrap();
    let path = loaded.directory().join("publication.json");
    let mut saved: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    saved["tutor_readiness"]["status"] = json!("preparing");
    saved["tutor_readiness"]["reason"] = json!("old book-only admission");
    let permissions = std::fs::metadata(&path).unwrap().permissions();
    let mut writable = permissions.clone(); writable.set_readonly(false);
    std::fs::set_permissions(&path, writable).unwrap();
    std::fs::write(&path, saved.to_string()).unwrap();
    std::fs::set_permissions(&path, permissions).unwrap();
    f.control.connection.execute("UPDATE book_publications SET manifest=? WHERE book_id=? AND publication_id=?",
        rusqlite::params![saved.to_string(),publication.reference.book_id,publication.reference.publication_id]).unwrap();
    assert_eq!(read()["missing_capabilities"], json!([]));
    assert_eq!(crate::tutor_api::readiness_view(&loaded.book, loaded.directory())["status"], "ready");
    let structure = loaded.directory().join("video-enrichments/structure/discourse.json");
    std::fs::rename(&structure, structure.with_extension("held")).unwrap();
    assert_eq!(read()["missing_capabilities"], json!(["tutor"]));
}

#[test]
fn video_workspace_readiness_requires_original_media_and_accepted_dependencies() {
    let f=super::b1_operator_build_tests::fixture();
    let dir=f.root.path().join("video");super::video_draft_tests::fixture(&dir);
    super::video_publication_tests::batch(&dir,"zh","translation");
    let publication=f.access.library.lock().unwrap().publish(&dir).unwrap();
    f.access.library.lock().unwrap().grant("A",&publication.reference).unwrap();
    let material=crate::workspace_publication::Material {published_book_ref:publication.reference.clone(),required_capabilities:vec!["text".into(),"video".into(),"translation".into()]};
    let read=||crate::workspace_publication::material_readiness(&f.control.connection,&material).unwrap();
    assert_eq!(read()["missing_capabilities"],json!([]));
    let path=f.access.library.lock().unwrap().load("A",&publication.reference).unwrap().directory().to_path_buf();
    std::fs::rename(path.join("video-enrichments/zh/transcript_translations.json"),path.join("translation-held.json")).unwrap();
    assert_eq!(read()["missing_capabilities"],json!(["translation"]));
    std::fs::rename(path.join("translation-held.json"),path.join("video-enrichments/zh/transcript_translations.json")).unwrap();
    std::fs::rename(path.join("media/original.mp4"),path.join("media-held.mp4")).unwrap();
    let missing=read();assert!(missing["available_capabilities"].as_array().unwrap().contains(&json!("text")));
    assert!(missing["missing_capabilities"].as_array().unwrap().contains(&json!("video")));
}

#[test]
#[ignore = "actual mixed book/video shared workspace browser service"]
fn video_workspace_browser_host() {
    let f=super::b1_operator_build_tests::fixture();let dir=f.root.path().join("video");
    let (book,_,_)=crate::video_frames::tests::fixture_with_duration(&dir,10);
    std::fs::write(dir.join("base.json"),json!(book.base).to_string()).unwrap();
    std::fs::write(dir.join("source.txt"),"A frame.").unwrap();
    let video=f.access.library.lock().unwrap().publish_online(&crate::online_publication::PublicationRequest {
        operation_id:"browser-video1".into(),input_directory:dir.clone(),grant_users:vec!["A".into(),"B".into()]
    }).unwrap().reference;
    let initial=release(&f,&video,0);f.ok(&f.b,"POST","/api/admin/workspace-releases",initial);
    let video2=f.access.library.lock().unwrap().publish_online(&crate::online_publication::PublicationRequest {
        operation_id:"browser-video2".into(),input_directory:dir,grant_users:vec!["A".into(),"B".into()]
    }).unwrap().reference;
    let origin="https://127.0.0.1:4190";
    let server=crate::multi_user_host::start_with_access("127.0.0.1:4190".parse().unwrap(),Arc::new(crate::multi_user_host::Site::new(origin).unwrap()),f.access.clone()).unwrap();
    let control=tiny_http::Server::http("127.0.0.1:4191").unwrap();
    println!("VIDEO_WORKSPACE_BROWSER_READY");
    let until=std::time::Instant::now()+std::time::Duration::from_secs(2400);
    while std::time::Instant::now()<until {
        let Some(request)=control.recv_timeout(std::time::Duration::from_secs(1)).unwrap() else {continue};
        if request.url()=="/stop" {request.respond(tiny_http::Response::from_string("stopped")).unwrap();break;}
        let value=match request.url() {
            "/fixture"=>json!({"book":f.x,"first":f.x,"second":f.y,"video":video,"video1":video,"video2":video2,"release_id":"mixed-course","root":f.root.path(),"origin":origin,"reader_user_id":"A","admin_user_id":"B","password":"fixture-only-password","reader_cookie":format!("{}={}",crate::auth::COOKIE,f.a),"admin_cookie":format!("{}={}",crate::auth::COOKIE,f.b),"admin_csrf":crate::auth::csrf(&f.b)}),
            "/publish-next"=>f.ok(&f.b,"POST","/api/admin/workspace-releases",release(&f,&video2,1)),
            _=>Value::Null,
        };
        request.respond(tiny_http::Response::from_string(value.to_string()).with_header(tiny_http::Header::from_bytes("Content-Type","application/json").unwrap())).unwrap();
    }
    server.shutdown();
}

#[test]
#[ignore = "measures catalog admission against an existing complete CS336 publication"]
fn video_workspace_catalog_existing_course_benchmark() {
    let directory=std::path::PathBuf::from(std::env::var("VIDEO_CATALOG_PUBLICATION").expect("existing publication directory"));
    let manifest:crate::published_library::PublicationManifest=serde_json::from_slice(&std::fs::read(directory.join("publication.json")).unwrap()).unwrap();
    let db=rusqlite::Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE book_publications(book_id TEXT,publication_id TEXT,manifest TEXT,directory TEXT)").unwrap();
    db.execute("INSERT INTO book_publications VALUES(?,?,?,?)",rusqlite::params![manifest.reference.book_id,manifest.reference.publication_id,serde_json::to_string(&manifest).unwrap(),directory.to_str().unwrap()]).unwrap();
    let material=crate::workspace_publication::Material {published_book_ref:manifest.reference,
        required_capabilities:["text","video","transcript","translation","visual_index","graph","discourse","tutor"].into_iter().map(str::to_owned).collect()};
    let mut samples=Vec::new();
    for _ in 0..3 {
        let start=std::time::Instant::now();
        let readiness=crate::workspace_publication::material_readiness(&db,&material).unwrap();
        let elapsed_ms=start.elapsed().as_secs_f64()*1000.0;
        assert_eq!(readiness["publication_available"],true);
        assert_eq!(readiness["missing_capabilities"],json!([]));
        samples.push(json!({"elapsed_ms":elapsed_ms,"readiness":readiness}));
    }
    let report=json!({"samples":samples,"model_calls":0});
    if let Ok(path)=std::env::var("VIDEO_CATALOG_REPORT") {std::fs::write(path,serde_json::to_vec_pretty(&report).unwrap()).unwrap();}
    println!("{report}");
}
