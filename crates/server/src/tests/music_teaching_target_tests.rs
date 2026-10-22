use super::*;

#[test]
fn music_teaching_target_binds_only_current_read_sources_and_rechecks_normalized_design() {
    let binding=TeachingBinding {tutor_session_id:"tutor".into(),session_revision:1,control_revision:1,
        source_id:"music".into(),source_revision:"book-source-revision".into(),map_revision:None,chat_session_id:"chat".into(),turn_id:"turn".into()};
    let source:SourceBinding=serde_json::from_value(json!({"source_ref_id":"read-music","book_id":"music","publication_id":"original","source_revision":"book-source-revision",
        "evidence_range":null,"evidence_text_digest":"","label_snapshot":"Repeat","preview_snapshot":"Compare the theme","media":null,
        "music":{"target":{"publication":{"book_id":"music","publication_id":"original"},
            "audio":{"recording":{"recording_id":"recording","revision":"recording-revision"},"start_ms":1000,"end_ms":2000},
            "score":{"score":{"score_id":"score","revision":"score-revision"},"source_ids":["note1"]},
            "occurrence_ids":["repeat"],"event_ids":["return"],"provenance_ids":["analysis"]},
            "title":"Work","label":"Repeat","preview":"Compare the theme","material":{"kind":"prepared_analysis"}}})).unwrap();
    let request=json!({"learning_focus":"theme return","expected_performance":"Compare the two appearances","capability":"explanation","source_bindings":[],"source_ref_ids":["read-music"]});
    assert!(bind_target(request.clone(),&binding,&[]).is_err());
    let target=bind_target(request,&binding,&[source.clone()]).unwrap();
    assert!(target.source_bindings.is_empty());
    assert!(target.media_bindings.is_none(),"prepared music analysis is not an observed video/audio target");
    assert_eq!(target.music_bindings.as_ref().unwrap()[0].target.audio.recording.revision,"recording-revision");
    assert_eq!(target.music_bindings.as_ref().unwrap()[0].source_revision,"book-source-revision");
    assert_eq!(bind_target(json!(target),&binding,&[source.clone()]).unwrap(),target,"reviewed normalized target must commit unchanged");
    let mut changed=json!(target);changed["music_bindings"][0]["target"]["audio"]["start_ms"]=json!(500);
    assert!(bind_target(changed,&binding,&[source.clone()]).is_err(),"an unread earlier recording range cannot be attached to the review");
    assert!(bind_target(json!(target),&binding,&[]).is_err(),"a previous turn target requires the actual source to be read in this turn");
}
