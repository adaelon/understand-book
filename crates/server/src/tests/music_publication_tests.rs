use super::*;
use super::mu5_tests::Fixture;

pub(super) fn fixture(directory: &Path) -> Value {
    std::fs::create_dir_all(directory.join("music")).unwrap();
    let book_id = format!("music-{}", uuid::Uuid::now_v7());
    let source = "# Music fixture\n\nCurator guide; this is not a transcript.";
    std::fs::write(directory.join("source.txt"), source).unwrap();
    std::fs::write(directory.join("base.json"), json!({"book_id":book_id,
        "lid_nodes":[{"lid":"1","path":[1],"kind":"paragraph","span":{"start":0,"end":source.len()},"children":[]}],
        "graph_nodes":[],"graph_edges":[]}).to_string()).unwrap();
    std::fs::write(directory.join("music/audio.mp3"), b"fixture audio bytes").unwrap();
    std::fs::write(directory.join("music/source.musicxml"), b"<score-partwise/>").unwrap();
    std::fs::write(directory.join("music/page.svg"), b"<svg xmlns=\"http://www.w3.org/2000/svg\"><g id=\"n1\"/></svg>").unwrap();
    let manifest = json!({"version":"music_source_manifest.v1","book_id":book_id,"title":"Music fixture",
        "work":{"work_id":"work","title":"Work","composer":"Composer","movement":null},
        "score":{"source":{"score_id":"score","revision":"1"},"title":"Score","musicxml_path":"music/source.musicxml"},
        "recording":{"source":{"recording_id":"recording","revision":"1"},"title":"Recording","performers":"Fixture",
            "media_path":"music/audio.mp3","content_type":"audio/mpeg","duration_ms":2000,"kind":"performance"},
        "rights":[{"asset_kind":"recording","title":"Audio","url":"https://example.test/audio","license":"CC0","attribution":"Fixture","distribution":"allowed"},
            {"asset_kind":"score","title":"Score","url":"https://example.test/score","license":"CC0","attribution":"Fixture","distribution":"allowed"}],
        "parts":[{"part_id":"p","name":"Piano","instrument":"Piano","group":"keyboard","voices":["1"]}],
        "pages":[{"page_id":"page","path":"music/page.svg","width":100,"height":100}],
        "score_objects":[{"source_id":"s1","part_id":"p","measure":"1","beat":0,"duration_beats":1,"voice":"1","midi_pitch":60,
            "rest":false,"tie":"none","page_id":"page","svg_element_id":"n1"}],
        "occurrences":[{"occurrence_id":"o1","order":0,"measure":"1","start_ms":0,"end_ms":1000,"score_object_ids":["s1"]},
            {"occurrence_id":"o2","order":1,"measure":"1","start_ms":1000,"end_ms":2000,"score_object_ids":["s1"]}],
        "note_alignments":[{"occurrence_id":"o1","score_object_id":"s1","start_ms":10,"end_ms":900,"attack":true,"provenance_id":"audio"},
            {"occurrence_id":"o2","score_object_id":"s1","start_ms":1010,"end_ms":1900,"attack":true,"provenance_id":"audio"}],
        "coverage":[{"start_ms":0,"end_ms":2000,"granularity":"note","method":"audio_alignment","status":"accepted","provenance_id":"audio","detail":"Synthetic contract fixture"}],
        "provenance":[{"provenance_id":"audio","kind":"audio_analysis","description":"Fixture processing","source_url":null,"processor":"fixture"}],
        "events":[{"event_id":"event","title":"Repeat","explanation":"The source returns","listen_for":"Compare both occurrences",
            "start_ms":1000,"end_ms":2000,"roles":[{"part_id":"p","role":"theme"}],
            "evidence":[{"provenance_id":"audio","score_object_ids":["s1"],"audio":{"recording":{"recording_id":"recording","revision":"1"},"start_ms":1000,"end_ms":2000}}],
            "relations":[],"concept_tags":["repetition"]}],
        "structure":[{"section_id":"section","parent_id":null,"title":"Whole","description":"Two occurrences","start_ms":0,"end_ms":2000,"event_ids":["event"],"coverage":"detailed"}],
        "demonstrations":[]});
    save(directory, &manifest);
    manifest
}

fn save(directory: &Path, value: &Value) {
    std::fs::write(directory.join(crate::music_media::MANIFEST), value.to_string()).unwrap();
}

#[test]
fn music_read_tools_load_decimal_score_tempo_from_published_bytes() {
    let directory = tempfile::tempdir().unwrap();
    let mut source = fixture(directory.path());
    source["provenance"].as_array_mut().unwrap().push(json!({"provenance_id":"notation", "kind":"score",
        "description":"Original MusicXML tempo", "source_url":null, "processor":null}));
    source["score_marks"] = json!([{"mark_id":"tempo", "part_id":"p", "measure":"1", "measure_index":1,
        "beat":0, "staff":null, "score_object_ids":["s1"], "provenance_id":"notation",
        "value":{"kind":"direction", "words":["Allegro ma non troppo"],
            "metronome":{"beat_unit":"quarter", "dots":0, "per_minute":"88"}, "sound_tempo_quarter_bpm":88.0002}}]);
    save(directory.path(), &source);
    let book = Book::load(directory.path().to_str().unwrap()).unwrap();
    let loaded = book.load_music_source(directory.path()).unwrap();
    assert_eq!(loaded, crate::music_media::load(directory.path()).unwrap());
    assert!(matches!(loaded.score_marks[0].value, base_schema::MusicScoreMarkValue::Direction {
        sound_tempo_quarter_bpm:Some(bpm), ..
    } if bpm == 88.0002));
}

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
    let request = AgentSourceRequest { note_mem_id:None, turn_id:run.turn_ref.turn_id, source_ref_id:"guide".into() };
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
fn music_readiness_explains_missing_curated_events_without_hiding_asset_failures() {
    let root = tempfile::tempdir().unwrap();
    let mut source = fixture(root.path());
    source["events"] = json!([]);
    source["structure"][0]["event_ids"] = json!([]);
    source["structure"][0]["coverage"] = json!("navigation_only");
    save(root.path(), &source);
    let book = Book::load(root.path().to_str().unwrap()).unwrap();
    let readiness = crate::tutor_api::readiness_view(&book, root.path());
    assert_eq!(readiness["status"], "preparing");
    assert_eq!(readiness["reason"], "本章可听原声、看总谱、按段落跳转，尚无精选导听片段");
    std::fs::remove_file(root.path().join("music/audio.mp3")).unwrap();
    let missing = crate::tutor_api::readiness_view(&book, root.path());
    assert_eq!(missing["status"], "preparing");
    assert_ne!(missing["reason"], readiness["reason"]);
}

#[test]
fn music_learning_purpose_admission_freezes_scope_and_continues_original_material() {
    use super::video_draft_tests::{media_learning_state, media_purpose_input, media_continue_input};
    for selected in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let manifest = fixture(root.path());
        let reference = base_schema::PublishedBookRef {book_id:manifest["book_id"].as_str().unwrap().into(),publication_id:"original".into()};
        std::fs::write(root.path().join("publication.json"), json!({"version":"publication.v1","reference":reference,
            "source_fingerprint":"fixture","files":{},"capabilities":{},"artifact_versions":{},"teaching_readiness":{},"build_readiness":{},"resident_bytes":0}).to_string()).unwrap();
        let book = Book::load(root.path().to_str().unwrap()).unwrap();
        let mut state = media_learning_state(root.path(), book);
        let mut input = media_purpose_input(&state, "music-purpose");
        if selected {
            input["music_question"] = json!({"draft_id":"purpose-music-question","publication":reference,
                "audio":{"recording":{"recording_id":"recording","revision":"1"},"start_ms":1100,"end_ms":1900},
                "score":{"score":{"score_id":"score","revision":"1"},"source_ids":["s1"]},
                "occurrence_id":"o2","focused_part_ids":["p"]});
        }
        let first = prepare_agent_chat(&mut state, &input.to_string(), "first").unwrap_or_else(|r| panic!("{}",r.body));
        assert_eq!(first.tutor.as_ref().unwrap()["status"], "observing");
        let stage_id = first.tutor.as_ref().unwrap()["binding"]["tutor_session_id"].as_str().unwrap().to_owned();
        assert_eq!(first.tutor.as_ref().unwrap()["learning_purpose"],"understand");
        assert_eq!(first.tutor.as_ref().unwrap()["binding"]["source_id"],state.workspace.book.base.book_id);
        assert_eq!(first.scope.music_question.is_some(), selected);
        let saved = state.user.agent_history.sessions.iter().flat_map(|s| &s.turns).find(|t| t.turn_id == first.turn_ref.turn_id).unwrap();
        assert!(saved.domain.interaction.as_ref().unwrap().source_bindings.is_empty(), "music scope is not the curator guide's text range");
        assert!(saved.domain.interaction.as_ref().unwrap().accepted_request.contains(if selected {"音乐片段"} else {"当前音乐"}));
        super::tutor_loop_tests::finish(&mut state, &first.turn_ref, &super::tutor_loop_tests::outcome(None));
        assert!(state.user.learning_store().unwrap().coach_job_for_source(&stage_id,memory::tutor_coach::CoachJobOrigin::AfterTurn,&first.turn_ref.turn_id).unwrap().is_none(),"a purpose button alone supplies no authored learner expression");
        let input = media_continue_input(&state, &first.turn_ref);
        let next = prepare_agent_chat(&mut state, &input.to_string(), "next").unwrap_or_else(|r| panic!("{}",r.body));
        assert_eq!(next.tutor.as_ref().unwrap()["binding"]["tutor_session_id"], stage_id);
        assert_eq!(json!(next.scope.music_question), json!(first.scope.music_question));
        assert!(state.user.learning_store().unwrap().coach_turn_dependency(&next.turn_ref.session_id,&next.turn_ref.turn_id).unwrap().is_none_or(|dependency|dependency.job_ref.is_none()));
        super::tutor_loop_tests::finish(&mut state, &next.turn_ref, &super::tutor_loop_tests::outcome(None));
        let again_input = media_continue_input(&state, &next.turn_ref);
        let again = prepare_agent_chat(&mut state, &again_input.to_string(), "again").unwrap_or_else(|r| panic!("{}",r.body));
        assert_eq!(again.tutor.as_ref().unwrap()["binding"]["tutor_session_id"], stage_id);
        assert_eq!(json!(again.scope.music_question), json!(first.scope.music_question));
        if selected {
            super::tutor_loop_tests::finish(&mut state, &again.turn_ref, &super::tutor_loop_tests::outcome(None));
            let whole_input = media_purpose_input(&state, "music-whole-after-clip");
            let whole = prepare_agent_chat(&mut state, &whole_input.to_string(), "whole").unwrap_or_else(|r| panic!("{}",r.body));
            assert_eq!(whole.tutor.as_ref().unwrap()["binding"]["tutor_session_id"], stage_id,
                "the observation owner stays on this source; the explicit goal keeps its requested scope");
        }
    }
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

#[test]
fn music_media_stream_has_audio_type_and_range_semantics() {
    let root = tempfile::tempdir().unwrap(); let path = root.path().join("recording.mp3");
    std::fs::write(&path, b"0123456789").unwrap();
    let response = crate::video_media::stream_response(&path, "GET", Some("bytes=3-6")).unwrap();
    assert_eq!(response.status_code().0, 206);
    assert_eq!(response.data_length(), Some(4));
    assert!(response.headers().iter().any(|h| h.field.equiv("Content-Type") && h.value.as_str() == "audio/mpeg"));
    assert_eq!(crate::video_media::content_type(Path::new("score.svg")), "image/svg+xml");
    assert_eq!(crate::video_media::stream_response(&path, "GET", Some("bytes=12-")).unwrap().status_code().0, 416);
    assert_eq!(crate::video_media::stream_response(&path, "HEAD", Some("bytes=3-6")).unwrap().data_length(), Some(10));
}

