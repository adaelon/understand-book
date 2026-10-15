use super::*;

#[test]
fn video_revision_failure_history_notes_and_explicit_switch_keep_original_sources() {
    use super::mu5_tests::Fixture;
    let f = Fixture::new();
    let input = f.root.path().join("old-video");
    super::video_draft_tests::fixture(&input);
    let original = f.access.library.lock().unwrap().publish(&input).unwrap().reference;
    f.access.library.lock().unwrap().grant("A", &original).unwrap();
    let mut w = f.create(&f.a, &original, "old-window");
    w = f.action(&f.a, &w, "old-window", "chat/new", json!({}));
    w = f.action(&f.a, &w, "old-window", "reader/note", json!({"lid":"1","text":"VI17 original note"}));
    let port = f.run(&w, "old-window");
    let range = EvidenceRange {start_lid:"1".into(),end_lid:"1".into(),ranges:vec![read_tools::SourceSelectedRange {
        lid:"1".into(),range:read_tools::SourceTextRange {start:0,end:8}}]};
    let media = crate::video_source::text_snapshot(&port.scope.book, &port.scope.book_dir, &range).unwrap().unwrap();
    let handle = f.access.users.lock().unwrap().get("A", "now").unwrap();
    {
        let mut user = handle.lock().unwrap();
        let session = user.agent_history.sessions.iter().find(|s|s.id==port.scope.chat_session_id).unwrap();
        let turn = &session.turns[0];
        let reference = AgentTurnRef {session_id:session.id.clone(),turn_id:turn.turn_id.clone(),user_turn_ordinal:turn.user_turn_ordinal};
        crate::session_runtime::append(&mut user,&reference,"source",crate::session_event::EventBody::SourcesBound {bindings:vec![SourceBinding {
            source_ref_id:"vi17-old".into(),book_id:original.book_id.clone(),evidence_range:None,evidence_text_digest:String::new(),
            media:Some(media),label_snapshot:"原正文".into(),preview_snapshot:"A frame.".into()}]}).unwrap();
    }
    let revised = f.root.path().join("revision");
    super::video_draft_tests::fixture(&revised);
    for file in ["base.json","video_source_manifest.json","transcript_map.json"] {
        let p=revised.join(file);let mut value:Value=serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
        match file {
            "base.json"=>value["book_id"]=json!("vi17-revised"),
            "video_source_manifest.json"=>{value["material"]["book_id"]=json!("vi17-revised");value["previous_book_id"]=json!("vi6");
                value["material"]["transcript"]["transcript_revision"]=json!("revised");},
            _=>{value["transcript"]["transcript_revision"]=json!("revised");value["segments"][0]["text"]=json!("A scene.");},
        }
        std::fs::write(p,value.to_string()).unwrap();
    }
    // Map/text disagreement prevents publication and cannot change the old entry.
    assert!(f.access.library.lock().unwrap().publish(&revised).is_err());
    assert_eq!(f.access.library.lock().unwrap().load("A",&original).unwrap().book.text("1",None).unwrap(),"A frame.");
    std::fs::write(revised.join("source.txt"),"A scene.").unwrap();
    let revision=f.access.library.lock().unwrap().publish(&revised).unwrap();
    assert!(revision.previous_publication.is_none());
    f.access.library.lock().unwrap().grant("A",&revision.reference).unwrap();
    assert!(f.access.library.lock().unwrap().video_successor("A",&original).unwrap().is_none());
    let quiet=f.create(&f.a,&original,"quiet");
    let unchanged=f.action(&f.a,&quiet,"quiet","reader/adopt_same_source_publication",json!({}));
    assert_eq!(unchanged["published_book_ref"],json!(original));
    let opened=f.action(&f.a,&unchanged,"quiet","book/open",json!({"published_book_ref":revision.reference}));
    assert_eq!(opened["published_book_ref"],json!(revision.reference));
    let resolved=f.action(&f.a,&opened,"quiet","agent/source.resolve",json!({"turn_id":port.scope.turn_id,"source_ref_id":"vi17-old"}));
    assert_eq!(resolved["result"]["media"]["publication"],json!(original));
    assert_eq!(resolved["result"]["media_url"],original.url("video/media"));
    assert_eq!(resolved["result"]["stale"],false);
    let loaded=f.access.library.lock().unwrap().load("A",&revision.reference).unwrap();
    let local=reader_workspace::ReaderWorkspace::local(loaded.directory().into(),loaded.book.clone(),Reader::new(&loaded.book,DEFAULT_RADIUS),vec![],None);
    assert_eq!(crate::video_source::local_directory(&local,&original).unwrap(),port.scope.book_dir);
    let user=handle.lock().unwrap();
    let notes=user.store.recall(&memory::RecallQuery {text:Some("VI17 original note".into()),..Default::default()});
    assert_eq!(notes.len(),1);
    let encoded=serde_json::to_string(&notes).unwrap();
    assert!(encoded.contains(&original.book_id));assert!(!encoded.contains("vi17-revised"));
    let session=user.agent_history.sessions.iter().find(|s|s.id==port.scope.chat_session_id).unwrap();
    assert_eq!(session.turns[0].published_book_ref.as_ref(),Some(&original));
    drop(user);
    f.access.library.lock().unwrap().revoke("A",&original).unwrap();
    let denied=f.action(&f.a,&opened,"quiet","agent/source.resolve",json!({"turn_id":port.scope.turn_id,"source_ref_id":"vi17-old"}));
    assert_eq!(denied["result"]["stale"],true);assert!(denied["result"]["media_url"].is_null());
}

#[test]
fn video_source_history_uses_original_network_publication_and_authorization() {
    use super::mu5_tests::Fixture;
    let f = Fixture::new();
    let input = f.root.path().join("video-input");
    super::video_draft_tests::fixture(&input);
    let original = f.access.library.lock().unwrap().publish(&input).unwrap().reference;
    f.access.library.lock().unwrap().grant("A", &original).unwrap();
    let newer = f.access.library.lock().unwrap().publish(&input).unwrap().reference;
    f.access.library.lock().unwrap().grant("A", &newer).unwrap();
    assert_ne!(original, newer);
    let w = f.create(&f.a, &original, "page");
    let w = f.action(&f.a, &w, "page", "chat/new", json!({}));
    let port = f.run(&w, "page");
    let range = EvidenceRange { start_lid: "1".into(), end_lid: "1".into(), ranges: vec![read_tools::SourceSelectedRange { lid: "1".into(), range: read_tools::SourceTextRange { start: 0, end: 8 } }] };
    let media = crate::video_source::text_snapshot(&port.scope.book, &port.scope.book_dir, &range).unwrap().unwrap();
    let handle = f.access.users.lock().unwrap().get("A", "now").unwrap();
    {
        let mut user = handle.lock().unwrap();
        let session = user.agent_history.sessions.iter().find(|s| s.id == port.scope.chat_session_id).unwrap();
        let turn = &session.turns[0];
        let reference = AgentTurnRef { session_id: session.id.clone(), turn_id: turn.turn_id.clone(), user_turn_ordinal: turn.user_turn_ordinal };
        crate::session_runtime::append(&mut user, &reference, "source", crate::session_event::EventBody::SourcesBound { bindings: vec![SourceBinding {
            source_ref_id: "saved-media".into(), book_id: original.book_id.clone(), evidence_range: None, evidence_text_digest: String::new(),
            media: Some(media), label_snapshot: "旧英文来源".into(), preview_snapshot: "A frame.".into(),
        }] }).unwrap();
    }
    let current = f.create(&f.a, &newer, "new-page");
    let resolve = || f.action(&f.a, &current, "new-page", "agent/source.resolve", json!({"turn_id":port.scope.turn_id,"source_ref_id":"saved-media"}));
    let response = resolve();
    assert_eq!(response["result"]["media"]["publication"], json!(original));
    assert_eq!(response["result"]["media_url"], original.url("video/media"));
    assert_eq!(response["result"]["stale"], false);
    f.access.library.lock().unwrap().revoke("A", &original).unwrap();
    let absent = resolve();
    assert_eq!(absent["result"]["stale"], true);
    assert_eq!(absent["result"]["label"], "旧英文来源");
    assert_eq!(absent["result"]["media"]["publication"], json!(original));
    assert!(absent["result"]["media_url"].is_null());
}

#[test]
fn video_source_original_frames_translations_joint_snapshot_and_unavailable() {
    let root = tempfile::tempdir().unwrap();
    let (book, publication, request) = crate::video_frames::tests::fixture(root.path());
    let batch = crate::video_frames::read_frames(&book, root.path(), &publication, "run", &root.path().join("cache"), request, &runtime::run_context::CancellationToken::default()).unwrap();
    let frame = batch.images[0].read.frame.clone();
    let (_, map) = book.load_video_source(root.path()).unwrap();
    let translation = base_schema::TranscriptTranslation { transcript: map.transcript.clone(), segment_id: "s".into(), zh_text: "一帧。".into() };
    std::fs::write(root.path().join("transcript_translations.json"), json!([translation]).to_string()).unwrap();
    let visual = base_schema::EvidenceTarget::Visual { publication: publication.clone(), frames: vec![frame.clone()], regions: vec![] };
    let audio = base_schema::EvidenceTarget::Audio { publication: publication.clone(), location: batch.location.clone(), review_result_ref: "fixture-audio-tool-result".into() };
    let text = base_schema::EvidenceTarget::Text { publication: publication.clone(), transcript: Some(map.transcript), text_range: map.segments[0].text_range.clone() };
    for target in [visual.clone(), audio.clone(), text.clone(), base_schema::EvidenceTarget::Joint { members: vec![
        base_schema::JointEvidenceMember { target: visual, support_role: "画面".into() },
        base_schema::JointEvidenceMember { target: text, support_role: "英文".into() },
        base_schema::JointEvidenceMember { target: audio, support_role: "原话".into() },
    ] }] {
        let media = crate::video_source::snapshot(&book, root.path(), target).unwrap();
        assert_eq!(media.translations[0].zh_text, "一帧。");
        let binding = SourceBinding { source_ref_id: "source-original".into(), book_id: publication.book_id.clone(), evidence_range: None, evidence_text_digest: String::new(), label_snapshot: "原片来源".into(), preview_snapshot: "A frame.".into(), media: Some(media) };
        let saved: SourceBinding = serde_json::from_str(&serde_json::to_string(&binding).unwrap()).unwrap();
        // A revisable VU artifact cannot redirect this saved reference.
        std::fs::write(root.path().join("visual_units.json"), r#"{"units":[{"id":"replacement","start_ms":1500}]}"#).unwrap();
        let popup = crate::video_source::popup(&saved, Ok((&book, root.path())), publication.url("video/media"));
        assert_eq!(popup["stale"], false); assert_eq!(popup["media"]["publication"], json!(publication));
        if popup["frames"].as_array().unwrap().len() > 0 {
            assert_eq!(popup["frames"][0]["frame"], json!(frame));
            assert_eq!(popup["frames"][0]["url"], format!("data:image/png;base64,{}", batch.images[0].png_base64));
        }
        let absent = crate::video_source::popup(&saved, Err(authorization::missing()), "unavailable".into());
        assert_eq!(absent["stale"], true); assert_eq!(absent["label"], "原片来源");
        assert_eq!(absent["media"], json!(binding.media.unwrap())); assert!(absent["media_url"].is_null());
    }
}
