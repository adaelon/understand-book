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
fn music_publication_requires_distribution_rights_and_immutable_original_bytes() {
    let f = Fixture::new(); let input = f.root.path().join("music-rights");
    let source = fixture(&input); let mut candidate = source.clone();
    let mut library = f.access.library.lock().unwrap();
    candidate["rights"] = json!([]); save(&input, &candidate);
    assert_eq!(library.publish(&input).unwrap_err().error_code, "MUSIC_DISTRIBUTION_UNAVAILABLE");
    candidate = source.clone(); candidate["rights"][0]["distribution"] = json!("local_only"); save(&input, &candidate);
    assert_eq!(library.publish(&input).unwrap_err().error_code, "MUSIC_DISTRIBUTION_UNAVAILABLE");
    save(&input, &source); let first = library.publish(&input).unwrap(); library.grant("A", &first.reference).unwrap();
    std::fs::write(input.join("music/audio.mp3"), b"changed performance").unwrap();
    assert_eq!(library.publish(&input).unwrap_err().error_code, "BOOK_CONTENT_CHANGED");
    assert_eq!(std::fs::read(library.music_asset("A", &first.reference, "music/audio").unwrap()).unwrap(), b"fixture audio bytes");
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
