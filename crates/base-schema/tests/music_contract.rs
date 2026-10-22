use base_schema::*;

fn fixture() -> MusicSourceManifest {
    serde_json::from_str(include_str!("fixtures/music.json")).unwrap()
}

#[test]
fn music_repeated_score_objects_have_independent_occurrences_and_overlapping_chord_notes() {
    let m = fixture();
    m.validate().unwrap();
    assert_eq!(m.note_alignments.len(), 4);
    assert_eq!(m.capabilities(), ["music", "music_score", "music_events", "music_note_alignment"]);
    let json = serde_json::to_string(&m).unwrap();
    assert_eq!(serde_json::from_str::<MusicSourceManifest>(&json).unwrap(), m);
}

#[test]
fn music_missing_note_is_not_note_coverage_and_cannot_be_hidden_from_occurrence() {
    let mut m = fixture();
    m.note_alignments.pop();
    assert!(m.validate().unwrap_err().contains("note coverage gap"));
    m.coverage[0].granularity = MusicGranularity::Measure;
    m.validate().unwrap();
    assert!(!m.capabilities().iter().any(|c| c == "music_note_alignment"));
    m.occurrences[1].score_object_ids.pop();
    assert!(m.validate().unwrap_err().contains("occurrence"));
}

#[test]
fn music_bad_references_and_repeated_tie_attacks_are_rejected() {
    let mut m = fixture();
    m.note_alignments[0].occurrence_id = "missing".into();
    assert!(m.validate().unwrap_err().contains("note alignment"));
    let mut m = fixture();
    m.score_objects[0].tie = MusicTie::Stop;
    assert!(m.validate().unwrap_err().contains("note alignment"));
    m.note_alignments[0].attack = false;
    m.note_alignments[2].attack = false;
    m.validate().unwrap();
    m.events[0].relations[0].target_event_id = "missing".into();
    assert!(m.validate().unwrap_err().contains("event relation"));
}

#[test]
fn music_synthetic_timing_cannot_claim_performance_coverage() {
    let mut m = fixture();
    m.provenance[1].kind = MusicProvenanceKind::ScoreSynthesis;
    m.coverage[0].method = MusicAlignmentMethod::ScoreSynthesis;
    assert!(m.validate().is_err());
    m.recording.kind = MusicRecordingKind::ScoreSynthesis;
    m.validate().unwrap();
    m.coverage.push(m.coverage[0].clone());
    assert!(m.validate().unwrap_err().contains("coverage overlap"));
}

#[test]
fn music_successor_preserves_sources_while_layout_and_alignment_can_change() {
    let old = fixture();
    let mut next = old.clone();
    next.pages[0].width = 800.0;
    next.score_objects[0].svg_element_id = "new-render-id".into();
    next.score.musicxml_path = "repacked/score.musicxml".into();
    next.recording.media_path = "repacked/original.flac".into();
    next.note_alignments[2].start_ms = 2100.0;
    next.events[1].explanation = "Updated teaching annotation.".into();
    next.validate_successor(&old).unwrap();
    next.score_objects[0].midi_pitch = Some(61);
    assert!(next.validate_successor(&old).unwrap_err().contains("immutable score location"));
    next = old.clone();
    next.recording.source.revision = "2".into();
    next.events[1].evidence[0].audio.as_mut().unwrap().recording = next.recording.source.clone();
    assert!(next.validate_successor(&old).unwrap_err().contains("immutable material source"));
}

#[test]
fn music_question_is_pinned_to_original_publication_and_recording_range() {
    let m = fixture();
    let publication = PublishedBookRef { book_id: m.book_id.clone(), publication_id: "release-1".into() };
    let q = MusicQuestionContext { draft_id: "question-1".into(), publication: publication.clone(),
        audio: MusicAudioLocation { recording: m.recording.source.clone(), start_ms: 2100.0, end_ms: 2600.0 },
        score: Some(MusicScoreLocation { score: m.score.source.clone(), source_ids: vec!["score/note/c".into()] }),
        occurrence_id: Some("repeat".into()), focused_part_ids: vec!["piano".into()], focused_voices: vec![] };
    q.validate(&m, &publication).unwrap();
    let mut newer = publication.clone(); newer.publication_id = "release-2".into();
    assert!(q.validate(&m, &newer).unwrap_err().contains("question publication"));
    let mut wrong_occurrence = q; wrong_occurrence.occurrence_id = Some("first".into());
    assert!(wrong_occurrence.validate(&m, &publication).unwrap_err().contains("question focus"));
}

#[test]
fn music_single_instrument_roles_can_select_distinct_real_voices() {
    let mut m = fixture();
    assert_eq!(m.parts.len(), 1);
    assert_eq!(m.parts[0].voices, ["1", "2"]);
    assert!(m.events[0].roles[0].voice_ids.is_empty());
    let legacy = serde_json::to_value(&m).unwrap();
    assert!(legacy["events"][0]["roles"][0].get("voice_ids").is_none());
    m.events[0].roles = vec![
        MusicEventRole { part_id: "piano".into(), role: "melody".into(), voice_ids: vec!["1".into()], theme_id: None },
        MusicEventRole { part_id: "piano".into(), role: "accompaniment".into(), voice_ids: vec!["2".into()], theme_id: None },
    ];
    m.validate().unwrap();
    m.events[0].roles[1].voice_ids = vec!["3".into()];
    assert!(m.validate().unwrap_err().contains("event role"));
    m.events[0].roles[1].voice_ids = vec!["2".into(), "2".into()];
    assert!(m.validate().unwrap_err().contains("event role"));
    m.events[0].roles[1].voice_ids.clear();
    m.validate().unwrap();
}

#[test]
fn music_question_voice_focus_is_optional_and_bound_to_its_real_part() {
    let m = fixture();
    let publication = PublishedBookRef { book_id: m.book_id.clone(), publication_id: "release-1".into() };
    let legacy = serde_json::json!({
        "draft_id": "question-1", "publication": publication,
        "audio": { "recording": m.recording.source, "start_ms": 2100, "end_ms": 2600 },
        "score": null, "occurrence_id": "repeat", "focused_part_ids": []
    });
    let mut q: MusicQuestionContext = serde_json::from_value(legacy).unwrap();
    q.validate(&m, &publication).unwrap();
    assert!(q.focused_voices.is_empty());
    assert!(serde_json::to_value(&q).unwrap().get("focused_voices").is_none());
    q.focused_voices = vec![MusicVoiceFocus { part_id: "piano".into(), voice: "1".into() }];
    q.validate(&m, &publication).unwrap();
    q.focused_voices[0].voice = "3".into();
    assert!(q.validate(&m, &publication).unwrap_err().contains("voice focus"));
    q.focused_voices[0] = MusicVoiceFocus { part_id: "flute".into(), voice: "1".into() };
    assert!(q.validate(&m, &publication).unwrap_err().contains("voice focus"));
    q.focused_voices = vec![MusicVoiceFocus { part_id: "piano".into(), voice: "2".into() }; 2];
    assert!(q.validate(&m, &publication).unwrap_err().contains("question voice focus"));
}

#[test]
fn music_event_playback_occurrences_are_optional_and_keep_audio_evidence_separate() {
    let mut m = fixture();
    assert!(m.events[1].occurrence_ids.is_empty());
    assert!(serde_json::to_value(&m).unwrap()["events"][1].get("occurrence_ids").is_none());
    let evidence = m.events[1].evidence[0].audio.clone();
    m.events[1].occurrence_ids = vec!["repeat".into()];
    m.validate().unwrap();
    assert_eq!(m.events[1].evidence[0].audio, evidence);
    m.events[1].occurrence_ids = vec!["missing".into()];
    assert!(m.validate().unwrap_err().contains("event occurrence"));
    m.events[1].occurrence_ids = vec!["repeat".into(), "repeat".into()];
    assert!(m.validate().unwrap_err().contains("event occurrence"));
}

#[test]
fn music_theme_identity_can_follow_roles_across_events_and_absence_stays_compatible() {
    let mut m = fixture();
    assert!(m.events[0].roles[0].theme_id.is_none());
    assert!(serde_json::to_value(&m).unwrap()["events"][0]["roles"][0].get("theme_id").is_none());
    m.events[0].roles[0].theme_id = Some("main-theme".into());
    m.events[1].roles = vec![MusicEventRole {
        part_id: "piano".into(), role: "returning theme".into(), voice_ids: vec!["2".into()], theme_id: Some("main-theme".into()),
    }];
    m.validate().unwrap();
    let mut encoded = serde_json::to_value(&m).unwrap();
    encoded["events"][0]["roles"][0]["theme_id"] = serde_json::Value::Null;
    let restored: MusicSourceManifest = serde_json::from_value(encoded).unwrap();
    restored.validate().unwrap();
    assert!(restored.events[0].roles[0].theme_id.is_none());
    m.events[1].roles[0].theme_id = Some(" ".into());
    assert!(m.validate().unwrap_err().contains("event role"));
}
