pub use reader_service::tests::music_publication_tests::*;
use super::*;

use super::mu5_tests::Fixture;

#[test]
fn music_local_reader_opens_guide_citations_only_for_the_original_publication() {
    let f = Fixture::new();
    let input = f.root.path().join("local-guide-citation");
    fixture(&input);
    let (reference, directory) = {
        let mut library = f.access.library.lock().unwrap();
        let reference = library.publish(&input).unwrap().reference;
        library.grant("A", &reference).unwrap();
        let directory = library.load("A", &reference).unwrap().directory().to_path_buf();
        (reference, directory)
    };
    // Use the same unbound local workspace as the standalone music Reader.
    let mut state = super::video_draft_tests::media_learning_state(&directory, Book::load(directory.to_str().unwrap()).unwrap());
    let run = prepare_agent_chat(&mut state, r#"{"message":"Explain the curator guide."}"#, "now")
        .unwrap_or_else(|r| panic!("{}", r.body));
    let turn = state.user.agent_history.sessions.iter_mut().flat_map(|s| &mut s.turns)
        .find(|turn| turn.turn_id == run.turn_ref.turn_id).unwrap();
    turn.published_book_ref = Some(reference.clone());
    turn.source_bindings.push(SourceBinding { music:None, media:None, source_ref_id:"guide".into(),
        book_id:reference.book_id.clone(), publication_id:Some(reference.publication_id.clone()),
        source_revision:None, evidence_range:None, evidence_text_digest:String::new(), label_snapshot:"Guide".into(), preview_snapshot:"Guide".into() });
    let request: AgentSourceRequest = serde_json::from_value(json!({"note_mem_id":null,"turn_id":run.turn_ref.turn_id,"source_ref_id":"guide"})).unwrap();
    assert!(agent_source_binding(&state, &request).is_ok());
    // Opening another local snapshot must not silently rebind the old card.
    state.workspace.book_dir = input;
    assert_eq!(agent_source_binding(&state, &request).unwrap_err().error_code, "PUBLICATION_BINDING_REQUIRED");
}

#[test]
fn music_pdf_source_publishes_page_evidence_and_preserves_original_bytes() {
    let f = Fixture::new(); let input = f.root.path().join("music-pdf");
    let mut source = fixture(&input);
    source["score"].as_object_mut().unwrap().remove("musicxml_path");
    source["score"]["pdf_path"] = json!("music/source.pdf");
    std::fs::write(input.join("music/source.pdf"), b"%PDF-1.4 original score").unwrap();
    for field in ["score_objects", "occurrences", "note_alignments", "coverage"] { source[field] = json!([]); }
    source["events"][0]["evidence"][0]["score_object_ids"] = json!([]);
    source["events"][0]["evidence"][0]["score_page_ids"] = json!(["page"]);
    source["structure"][0]["coverage"] = json!("navigation_only");
    save(&input, &source);
    let mut library = f.access.library.lock().unwrap();
    let first = library.publish(&input).unwrap(); library.grant("A", &first.reference).unwrap();
    let published = library.load("A", &first.reference).unwrap();
    let view = crate::music_media::projection(published.directory()).unwrap();
    assert_eq!(view["manifest"]["score"]["pdf_path"], "music/source.pdf");
    assert_eq!(crate::music_media::capabilities(&crate::music_media::load(published.directory()).unwrap())["music_score"], true);
    let manifest = crate::music_media::load(published.directory()).unwrap();
    for view in ["score", "events"] {
        let read = read_tools::music::read(&manifest, &first.reference, &serde_json::from_value(json!({"view":view})).unwrap()).unwrap();
        assert_eq!(read.sources.len(), 1);
        assert!(!read.heard_audio);
        let saved = read.sources[0].clone();
        let binding = SourceBinding { music:Some(saved.clone()),media:None,source_ref_id:"pdf-source".into(),book_id:first.reference.book_id.clone(),
            publication_id:Some(first.reference.publication_id.clone()),source_revision:None,evidence_range:None,evidence_text_digest:String::new(),
            label_snapshot:saved.label.clone(),preview_snapshot:saved.preview.clone() };
        let popup = crate::music_source::popup(&binding, Ok((&published.book, published.directory())), first.reference.url("music/audio"));
        assert_eq!(popup["music_score_pages"].as_array().unwrap().len(), 1);
        assert_eq!(popup["music_score_pages"][0]["page_id"], "page");
    }
    let original = library.music_asset("A", &first.reference, "music/source").unwrap();
    assert_eq!(std::fs::read(&original).unwrap(), b"%PDF-1.4 original score");
    std::fs::write(input.join("music/source.pdf"), b"%PDF-1.4 different edition").unwrap();
    assert_eq!(library.publish(&input).unwrap_err().error_code, "BOOK_CONTENT_CHANGED");
    assert_eq!(std::fs::read(original).unwrap(), b"%PDF-1.4 original score");
}

#[test]
fn music_publication_score_facts_reopen_with_original_provenance_and_pages() {
    let f = Fixture::new(); let input = f.root.path().join("score-facts");
    let mut manifest = fixture(&input);
    manifest["provenance"].as_array_mut().unwrap().extend([
        json!({"provenance_id":"notation","kind":"score","description":"Original MusicXML","source_url":null,"processor":null}),
        json!({"provenance_id":"edition","kind":"reference","description":"Publisher identifies F minor","source_url":"https://example.test/edition","processor":null}),
    ]);
    manifest["work"]["tonality"] = json!({"tonic":"F","mode":"minor","explanation":"Reviewed against the original edition","provenance_ids":["edition"]});
    manifest["score_marks"] = json!([{"mark_id":"tempo","part_id":"p","measure":"1","measure_index":1,"beat":0,"staff":null,
        "value":{"kind":"direction","words":["Allegro moderato"],"metronome":null,"sound_tempo_quarter_bpm":80},
        "score_object_ids":["s1"],"provenance_id":"notation"}]);
    save(&input,&manifest);
    let mut library = f.access.library.lock().unwrap();
    let publication = library.publish(&input).unwrap().reference;
    library.grant("A",&publication).unwrap();
    let loaded = library.load("A",&publication).unwrap();
    let source = loaded.book.load_music_source(loaded.directory()).unwrap();
    let read = read_tools::music::read(&source,&publication,&serde_json::from_value(json!({"view":"score","limit":1})).unwrap()).unwrap();
    let saved = read.sources[0].clone();
    let binding = SourceBinding { music:Some(saved.clone()),media:None,source_ref_id:"notation-source".into(),book_id:publication.book_id.clone(),
        publication_id:Some(publication.publication_id.clone()),source_revision:None,evidence_range:None,evidence_text_digest:String::new(),
        label_snapshot:saved.label.clone(),preview_snapshot:saved.preview.clone() };
    manifest["work"]["tonality"]["explanation"] = json!("Revised editorial explanation"); save(&input,&manifest);
    let newer = library.publish(&input).unwrap().reference;
    assert_ne!(newer,publication);
    let popup = crate::music_source::popup(&binding,Ok((&loaded.book,loaded.directory())),publication.url("music/audio"));
    assert_eq!(popup["stale"],false);
    assert_eq!(popup["can_open_in_reader"],true);
    assert_eq!(popup["music"]["target"]["publication"],json!(publication));
    assert_eq!(popup["music"]["material"]["read"]["work_tonality"]["explanation"],"Reviewed against the original edition");
    assert!(popup["highlighted_quote"].as_str().unwrap().contains("Allegro moderato"));
    assert_eq!(popup["music_score_pages"].as_array().unwrap().len(),1);
    assert!(popup["music"]["material"]["provenance"].as_array().unwrap().iter().any(|p| p["provenance_id"]=="edition"));
}

#[test]
fn music_publication_shares_material_preserves_versions_and_authenticates_all_assets() {
    let f = Fixture::new();
    let input = f.root.path().join("music-source");
    let mut source = fixture(&input);
    // Closing a local calibration opens the curator guide. Its unpublished
    // music material has no Reader publication projection yet.
    assert_eq!(crate::music_media::projection(&input).unwrap(), Value::Null);
    let mut library = f.access.library.lock().unwrap();
    let first = library.publish(&input).unwrap();
    library.grant("A", &first.reference).unwrap();
    assert_eq!(library.list("A").unwrap().iter().find(|v| v["published_book_ref"] == json!(first.reference)).unwrap()["media_kind"], "music");
    let a = library.load("A", &first.reference).unwrap();
    assert!(library.music_asset("B", &first.reference, "music/audio").is_err());
    library.grant("B", &first.reference).unwrap();
    let b = library.load("B", &first.reference).unwrap();
    assert!(Arc::ptr_eq(&a.book, &b.book));
    let projection = crate::music_media::projection(a.directory()).unwrap();
    assert_eq!(projection["audio_url"], first.reference.url("music/audio"));
    assert_eq!(projection["manifest"]["occurrences"].as_array().unwrap().len(), 2);
    assert_eq!(first.tutor_readiness["status"], "ready");
    assert!(library.music_asset("A", &first.reference, "music/score/../../source.txt").is_err());
    source["events"][0]["explanation"] = json!("Revised listening guide");
    source["note_alignments"][1]["start_ms"] = json!(1020);
    save(&input, &source);
    let second = library.publish(&input).unwrap();
    assert_eq!(second.previous_publication, Some(first.reference.clone()));
    library.grant("A", &second.reference).unwrap();
    let revised = library.load("A", &second.reference).unwrap();
    assert_eq!(crate::music_media::projection(a.directory()).unwrap()["manifest"]["note_alignments"][1]["start_ms"].as_f64(), Some(1010.0));
    assert_eq!(crate::music_media::projection(revised.directory()).unwrap()["manifest"]["note_alignments"][1]["start_ms"].as_f64(), Some(1020.0));
    // Same-source publication updates inherit existing material grants; loaded
    // workspaces and prior citations still retain their explicit older version.
    assert!(library.load("B", &second.reference).is_ok());
    assert_eq!(crate::music_media::projection(b.directory()).unwrap()["publication"], json!(first.reference));
    library.revoke("B", &second.reference).unwrap();
    assert!(library.load("B", &second.reference).is_err());
    assert_eq!(std::fs::read(library.music_asset("B", &first.reference, "music/audio").unwrap()).unwrap(), b"fixture audio bytes");
    drop(library);
    let context = f.ok(&f.a, "GET", &first.reference.url("music_context"), json!({}));
    assert_eq!(context["publication"], json!(first.reference));
    let site = crate::multi_user_host::Site::new("https://reader.example").unwrap();
    let response = crate::multi_user_host::dispatch(&f.access, &site, "GET", &first.reference.url("music/score/page"),
        &crate::multi_user_host::Headers(vec![("Host".into(), "reader.example".into()),("Cookie".into(),format!("{}={}",crate::auth::COOKIE,f.a))]), "", crate::multi_user_host::now());
    assert_eq!(response.status, 200);
    f.access.library.lock().unwrap().revoke("A", &first.reference).unwrap();
    assert_ne!(f.call(&f.a, "GET", &first.reference.url("music/audio"), json!({})).0, 200);
}

#[test]
fn music_reader_adopts_calibration_without_replacing_chat_or_old_publication() {
    let f = Fixture::new(); let input = f.root.path().join("music-update");
    let mut source = fixture(&input);
    source["coverage"][0]["status"] = json!("pending"); save(&input, &source);
    let original = f.access.library.lock().unwrap().publish(&input).unwrap().reference;
    f.access.library.lock().unwrap().grant("A", &original).unwrap();
    let scene = f.create(&f.a, &original, "music-window");
    let scene = f.action(&f.a, &scene, "music-window", "chat/new", json!({}));
    source["coverage"][0]["status"] = json!("accepted");
    source["note_alignments"][1]["start_ms"] = json!(1030); save(&input, &source);
    let revised = f.access.library.lock().unwrap().publish(&input).unwrap().reference;
    let updated = f.action(&f.a, &scene, "music-window", "reader/adopt_same_source_publication", json!({"publication_id":original.publication_id}));
    assert_eq!(updated["result"]["status"], "adopted");
    assert_eq!(updated["published_book_ref"], json!(revised));
    assert_eq!(updated["result"]["music"]["publication"], json!(revised));
    assert_eq!(updated["result"]["music"]["manifest"]["coverage"][0]["status"], "accepted");
    for key in ["reader", "selected_chat", "generation"] { assert_eq!(updated[key], scene[key], "preserve {key}"); }
    assert_eq!(f.get(&f.a, &updated)["published_book_ref"], json!(revised));
    let late = f.action(&f.a, &updated, "music-window", "reader/adopt_same_source_publication", json!({"publication_id":original.publication_id}));
    assert_eq!(late["result"]["status"], "current");
    assert_eq!(late["result"]["music"]["publication"], json!(revised));
    let old = f.ok(&f.a, "GET", &original.url("music_context"), json!({}));
    assert_eq!(old["manifest"]["coverage"][0]["status"], "pending");
    assert_eq!(old["manifest"]["note_alignments"][1]["start_ms"].as_f64(), Some(1010.0));
}

#[test]
fn music_publication_preserves_rights_records_and_immutable_original_bytes() {
    let f = Fixture::new(); let input = f.root.path().join("music-rights");
    let source = fixture(&input); let mut candidate = source.clone();
    let mut library = f.access.library.lock().unwrap();
    candidate["rights"] = json!([]); save(&input, &candidate);
    assert_eq!(library.publish(&input).unwrap_err().error_code, "MUSIC_RIGHTS_RECORD_MISSING");
    candidate = source;
    for right in candidate["rights"].as_array_mut().unwrap() { right["distribution"] = json!("local_only"); }
    save(&input, &candidate);
    let first = library.publish(&input).unwrap(); library.grant("A", &first.reference).unwrap();
    std::fs::write(input.join("music/audio.mp3"), b"changed performance").unwrap();
    assert_eq!(library.publish(&input).unwrap_err().error_code, "BOOK_CONTENT_CHANGED");
    assert_eq!(std::fs::read(library.music_asset("A", &first.reference, "music/audio").unwrap()).unwrap(), b"fixture audio bytes");
    drop(library);
    let context = f.ok(&f.a, "GET", &first.reference.url("music_context"), json!({}));
    assert_eq!(context["manifest"]["rights"], candidate["rights"]);
}