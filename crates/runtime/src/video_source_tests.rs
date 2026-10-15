mod video_sources {
    use super::*;
    use base_schema::{EvidenceTarget, MediaObservation};
    #[test]
    fn video_source_text_visual_audio_joint_and_observation_gate() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!("../../base-schema/fixtures/video-contract.json")).unwrap();
        let f = &fixture["cases"][0];
        let book = Book::new(serde_json::from_value(f["base"].clone()).unwrap(), f["source_text"].as_str().unwrap());
        let mut targets: Vec<EvidenceTarget> = serde_json::from_value(f["targets"].clone()).unwrap();
        let observations: Vec<MediaObservation> = serde_json::from_value(f["observations"].clone()).unwrap();
        let mut ledger = TurnEvidenceLedger::default();
        assert!(ledger.check_media_target(&book, &targets[1], &observations).is_err());
        if let (EvidenceTarget::Visual { frames, .. }, MediaObservation::Visual { frames: seen, .. }) = (&mut targets[1], &observations[0]) {
            *frames = seen.iter().map(|r| r.frame.clone()).collect();
        }
        for target in &targets[..4] { assert!(ledger.check_media_target(&book, target, &[]).is_err()); }
        ledger.observe(EvidenceRange { start_lid: "1.1".into(), end_lid: "1.1".into(), ranges: vec![] });
        for target in &targets[..4] { ledger.check_media_target(&book, target, &observations).unwrap(); }
        let mut crop_only = observations.clone();
        if let MediaObservation::Visual { frames, .. } = &mut crop_only[0] { frames.clear(); }
        assert!(ledger.check_media_target(&book, &targets[1], &crop_only).is_err());
        ledger.check_media_target(&book, &targets[4], &crop_only).unwrap();
        let mut audio = targets[2].clone();
        if let EvidenceTarget::Audio { review_result_ref, .. } = &mut audio { *review_result_ref = "unseen-result".into(); }
        assert!(ledger.check_media_target(&book, &audio, &observations).is_err());
        let mut visual = targets[1].clone();
        if let EvidenceTarget::Visual { publication, .. } = &mut visual { publication.publication_id = "new-publication".into(); }
        assert!(ledger.check_media_target(&book, &visual, &observations).is_err());
        for target in &targets[..4] {
            let media = crate::video_source::MediaSource { publication: crate::video_source::publication(target).unwrap().clone(), target: target.clone(), duration_ms: 4641376.145,
                transcript: serde_json::from_value(f["map"]["segments"].clone()).unwrap(), translations: vec![], observations: observations.clone(), audio_reviews: vec![] };
            let first = ledger.present_media(media.clone()).unwrap();
            assert_eq!(ledger.present_media(media).unwrap().source_ref_id, first.source_ref_id);
            let compiled = compile_agent_answer(&format!("材料说明。[[source:{}]]", first.source_ref_id), &ledger.bindings(), &AnswerProvenanceLedger::default()).unwrap();
            assert_eq!(compiled.bindings.len(), 1);
            let saved = serde_json::to_string(&compiled.bindings).unwrap();
            let restored: Vec<SourceBinding> = serde_json::from_str(&saved).unwrap();
            assert!(restored[0].evidence_range.is_none());
            assert_eq!(&restored[0].media.as_ref().unwrap().target, target);
        }
    }
}
