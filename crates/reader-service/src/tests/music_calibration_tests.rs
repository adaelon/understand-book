use super::*;

pub fn music_state(name: &str) -> AppState {
    let mut state = state_named(name);
    let mut manifest = super::music_publication_tests::fixture(&state.workspace.book_dir);
    manifest["rights"][0]["distribution"] = json!("local_only");
    manifest["coverage"][0]["status"] = json!("pending");
    std::fs::write(state.workspace.book_dir.join(crate::music_media::MANIFEST), manifest.to_string()).unwrap();
    std::fs::write(state.workspace.book_dir.join("music/curation.json"), json!({"version":"music_curation.v1"}).to_string()).unwrap();
    std::fs::create_dir_all(state.workspace.book_dir.join(".build/music")).unwrap();
    state.workspace.book = Arc::new(Book::load(state.workspace.book_dir.to_str().unwrap()).unwrap());
    state
}

pub fn call(state: &mut AppState, method: &str, action: &str, body: Value) -> Reply {
    route(state, Req { method, url: &format!("/build_workbench/music/{action}"), body: &body.to_string(), now: "2026-10-22T00:00:00Z" })
}

#[test]
fn music_calibration_local_only_context_has_real_assets_without_publication() {
    let mut state = music_state("music-curator-local");
    let reply = call(&mut state, "GET", "context", Value::Null);
    assert_eq!(reply.status, 200);
    let context: Value = serde_json::from_str(&reply.body).unwrap();
    assert!(context.get("publication").is_none());
    let audio_url = context["audio_url"].as_str().unwrap().strip_prefix("/api").unwrap();
    let score_url = context["score_pages"][0]["url"].as_str().unwrap().strip_prefix("/api").unwrap();
    assert!(audio_url.starts_with("/build_workbench/music/audio?recording_id=recording&recording_revision=1&score_id=score&score_revision=1"));
    assert_eq!(context["manifest"]["rights"][0]["distribution"], "local_only");
    let audio = crate::music_calibration::local_asset(&state, audio_url).unwrap();
    assert_eq!(std::fs::read(&audio).unwrap(), b"fixture audio bytes");
    let response = crate::video_media::stream_response(&audio, "GET", Some("bytes=0-3")).unwrap();
    assert!(response.headers().iter().any(|header| header.field.equiv("Cache-Control") && header.value.as_str() == "no-store"));
    assert!(crate::music_calibration::local_asset(&state, score_url).is_ok());
    assert!(crate::music_calibration::local_asset(&state, &score_url.replace("score/page?", "score/missing?")).is_err());
    assert!(crate::music_calibration::local_asset(&state, "/build_workbench/music/audio").is_err());
    // CLI supports relative workspaces; publication file checks need absolute ancestors.
    let cwd = std::env::current_dir().unwrap();
    let relative_fixture = tempfile::tempdir_in(&cwd).unwrap();
    super::music_publication_tests::fixture(relative_fixture.path());
    state.workspace.book_dir = relative_fixture.path().strip_prefix(&cwd).unwrap().to_path_buf();
    assert!(!state.workspace.book_dir.is_absolute());
    assert_eq!(call(&mut state, "GET", "context", Value::Null).status, 200);
    assert!(crate::music_calibration::local_asset(&state, audio_url).unwrap().is_absolute());
    let mut changed: Value = serde_json::from_slice(&std::fs::read(state.workspace.book_dir.join(crate::music_media::MANIFEST)).unwrap()).unwrap();
    changed["recording"]["source"]["revision"] = json!("another-performance");
    changed["events"][0]["evidence"][0]["audio"]["recording"]["revision"] = json!("another-performance");
    std::fs::write(state.workspace.book_dir.join(crate::music_media::MANIFEST), changed.to_string()).unwrap();
    // An old context must not silently retrieve the newly selected recording or score page.
    assert!(crate::music_calibration::local_asset(&state, audio_url).is_err());
    assert!(crate::music_calibration::local_asset(&state, score_url).is_err());
    let next = crate::music_calibration::context(&state).unwrap();
    assert_ne!(next["audio_url"], context["audio_url"]);
    assert_ne!(next["score_pages"][0]["url"], context["score_pages"][0]["url"]);
    state.services.reader_only = true;
    assert_eq!(call(&mut state, "GET", "context", Value::Null).body, "null");
    assert_ne!(call(&mut state, "POST", "calibrate", json!({})).status, 200);
    state.services.reader_only = false;
    std::fs::write(state.workspace.book_dir.join("publication.json"), "{}").unwrap();
    assert_eq!(call(&mut state, "GET", "context", Value::Null).body, "null");
    assert!(crate::music_calibration::local_asset(&state, "/build_workbench/music/audio").is_err());
    assert_ne!(call(&mut state, "POST", "calibrate", json!({})).status, 200);
}

#[test]
fn music_calibration_saves_through_core_and_reports_failure_without_losing_prior_revision() {
    let mut state = music_state("music-curator-save");
    let manifest = crate::music_media::load(&state.workspace.book_dir).unwrap();
    let mut revision = json!({"recording":manifest.recording.source,"score":manifest.score.source,
        "start_ms":0,"end_ms":1000,"reason":"UI transport fixture, not a human listening result",
        "occurrences":[manifest.occurrences[0]],"note_alignments":[manifest.note_alignments[0]],
        "coverage":[{"start_ms":0,"end_ms":1000,"granularity":"note","method":"audio_alignment","status":"pending","provenance_id":"audio","detail":"Retain candidate for review"}],"provenance":[]});
    revision["note_alignments"][0]["start_ms"] = json!(30);
    let reply = call(&mut state, "POST", "calibrate", revision.clone());
    assert_eq!(reply.status, 200, "{}", reply.body);
    let context: Value = serde_json::from_str(&reply.body).unwrap();
    assert_eq!(context["manifest"]["note_alignments"][0]["start_ms"].as_f64(), Some(30.0));
    assert_eq!(context["manifest"]["coverage"][0]["status"], "pending");
    assert_eq!(context["history"].as_array().unwrap().len(), 1);
    assert_eq!(call(&mut state, "GET", "context", Value::Null).body, reply.body);
    let saved = std::fs::read(state.workspace.book_dir.join(crate::music_media::MANIFEST)).unwrap();
    revision["coverage"][0]["status"] = json!("accepted");
    revision["note_alignments"] = json!([]);
    let failed = call(&mut state, "POST", "calibrate", revision.clone());
    assert_ne!(failed.status, 200);
    assert!(failed.body.contains("校准未保存"), "{}", failed.body);
    assert_eq!(std::fs::read(state.workspace.book_dir.join(crate::music_media::MANIFEST)).unwrap(), saved);
    revision["recording"]["revision"] = json!("different");
    let changed_source = call(&mut state, "POST", "calibrate", revision);
    assert_ne!(changed_source.status, 200);
    assert!(changed_source.body.contains("工作区已改变"));
    assert_eq!(crate::music_calibration::context(&state).unwrap()["history"].as_array().unwrap().len(), 1);
}
