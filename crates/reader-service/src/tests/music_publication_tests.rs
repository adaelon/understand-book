use super::*;

pub fn fixture(directory: &Path) -> Value {
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

pub fn save(directory: &Path, value: &Value) {
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
