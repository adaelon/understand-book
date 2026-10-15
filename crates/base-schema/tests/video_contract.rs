#[path = "support/video_fixture.rs"]
mod video_fixture;
use base_schema::*;
use video_fixture::{Case, Fixture};

fn fixture() -> Fixture {
    serde_json::from_str(include_str!("../fixtures/video-contract.json")).unwrap()
}
fn lecture() -> Case {
    fixture().cases.remove(0)
}

#[test]
fn video_shared_fixture_roundtrips_and_keeps_real_pts_and_independent_repeats() {
    let f = fixture();
    for c in &f.cases {
        c.validate().unwrap();
    }
    let back: Fixture = serde_json::from_str(&serde_json::to_string(&f).unwrap()).unwrap();
    assert_eq!(f, back);
    let MediaObservation::Visual {
        frames, regions, ..
    } = &f.cases[0].observations[0]
    else {
        panic!()
    };
    assert_eq!(frames[0].requested_ms, 555123.0);
    assert_eq!(frames[0].frame.actual_pts_ms, 555133.3125);
    assert_eq!(regions[0].frame.width, 1920);
    assert_eq!(regions[0].frame.height, 1080);
    let EvidenceTarget::Visual { frames, .. } = &f.cases[1].targets[0] else {
        panic!()
    };
    assert_eq!(
        frames.iter().map(|f| f.actual_pts_ms).collect::<Vec<_>>(),
        vec![1000.0, 5000.0]
    );
    assert!(f.cases[1].base.lid_nodes.is_empty());
}

#[test]
fn video_missing_identity_is_not_a_source() {
    let mut value = serde_json::to_value(lecture()).unwrap();
    value["manifest"]["source"]["video"]
        .as_object_mut()
        .unwrap()
        .remove("media_id");
    assert!(serde_json::from_value::<Case>(value).is_err());
    let mut c = lecture();
    c.manifest.source.video.media_revision.clear();
    assert!(c.validate().is_err());
}

#[test]
fn video_wrong_bindings_mappings_and_bounds_are_rejected() {
    let mutations: Vec<Box<dyn Fn(&mut Case)>> = vec![
        Box::new(|c| c.map.transcript.transcript_revision = "other".into()),
        Box::new(|c| c.map.segments[0].text_range.lid = "missing".into()),
        Box::new(|c| c.map.segments[0].text_range.range.end += 1),
        Box::new(|c| c.map.segments[0].text = "invented transcript".into()),
        Box::new(|c| c.map.segments[0].location.end_ms = c.manifest.source.duration_ms + 1.0),
        Box::new(|c| c.map.segments[0].location.video.media_revision = "edited-film".into()),
        Box::new(|c| c.map.segments[1].segment_id = c.map.segments[0].segment_id.clone()),
        Box::new(|c| c.map.segments.swap(0, 1)),
        Box::new(|c| c.map.segments[0].words[0].range.end = 1000),
        Box::new(|c| c.map.segments[0].words[0].end_ms = 590000.0),
        Box::new(|c| c.questions[0].frame.actual_pts_ms = c.manifest.source.duration_ms),
        Box::new(|c| c.questions[0].frame.actual_pts_ms = f64::NAN),
        Box::new(|c| {
            c.questions[0].region.as_mut().unwrap().rect_normalized = [0.9, 0.0, 0.2, 1.0]
        }),
        Box::new(|c| c.questions[0].region.as_mut().unwrap().frame.width = 1120),
        Box::new(|c| c.questions[0].publication.book_id = "new-book".into()),
        Box::new(|c| c.questions[0].material.transcript.transcript_revision = "new-text".into()),
        Box::new(|c| c.translations[0].transcript.transcript_revision = "old".into()),
        Box::new(|c| c.translations[0].segment_id = "missing".into()),
        Box::new(|c| c.manifest.source.imported_path = "C:/external.mp4".into()),
        Box::new(|c| c.manifest.previous_book_id = Some(c.manifest.material.book_id.clone())),
        Box::new(|c| c.manifest.coverage.push(c.manifest.coverage[0].clone())),
    ];
    for (i, mutate) in mutations.iter().enumerate() {
        let mut c = lecture();
        mutate(&mut c);
        assert!(c.validate().is_err(), "mutation {i} accepted");
    }
}

#[test]
fn video_segment_only_timing_and_silence_do_not_claim_word_alignment() {
    let mut c = lecture();
    c.map.segments[1].words.clear();
    c.validate().unwrap();
    let f = fixture();
    let silent = &f.cases[1];
    assert!(silent.map.segments.is_empty());
    assert!(silent.manifest.coverage[0]
        .gaps(&silent.manifest.source)
        .unwrap()
        .is_empty());
    let source = &silent.manifest.source;
    let p = |start, end| MediaLocation {
        video: source.video.clone(),
        start_ms: start,
        end_ms: end,
    };
    let coverage = VideoCoverage {
        capability: VideoCapability::Transcript,
        accepted: vec![p(4000.0, 6000.0), p(0.0, 2000.0)],
        pending: vec![],
        failed: vec![p(2000.0, 4000.0)],
    };
    assert_eq!(coverage.gaps(source).unwrap(), vec![p(2000.0, 4000.0)]);
    let mut overlap = coverage;
    overlap.accepted.push(p(2500.0, 3000.0));
    assert!(overlap.gaps(source).is_err());
}

#[test]
fn video_real_zero_duration_word_estimate_is_not_a_playable_interval() {
    let f = fixture();
    let c = &f.cases[2];
    c.validate().unwrap();
    let word = &c.map.segments[0].words[0];
    assert_eq!(word.start_ms, 1180000.0);
    assert_eq!(word.end_ms, word.start_ms);
    let empty = MediaLocation {
        video: c.manifest.source.video.clone(),
        start_ms: word.start_ms,
        end_ms: word.end_ms,
    };
    assert!(c.manifest.source.validate_location(&empty).is_err());
}

#[test]
fn video_receipts_require_processing_identity_and_joint_requires_support_per_member() {
    let mut c = lecture();
    if let MediaObservation::Visual {
        model_request_id, ..
    } = &mut c.observations[0]
    {
        model_request_id.clear();
    }
    assert!(c.validate().is_err());
    let mut c = lecture();
    if let MediaObservation::Audio { tool_call_id, .. } = &mut c.observations[1] {
        tool_call_id.clear();
    }
    assert!(c.validate().is_err());
    let mut c = lecture();
    if let EvidenceTarget::Joint { members } = &mut c.targets[3] {
        members[0].support_role.clear();
    }
    assert!(c.validate().is_err());
    let mut c = lecture();
    if let EvidenceTarget::Joint { members } = &mut c.targets[3] {
        if let EvidenceTarget::Audio { publication, .. } = &mut members[1].target {
            publication.publication_id = "later".into();
        }
    }
    assert!(c.validate().is_err());
}

#[test]
fn video_moving_workspace_or_repartitioning_vu_does_not_rewrite_locations() {
    let mut c = lecture();
    let old = c.targets.clone();
    c.manifest.source.original_path = "D:/moved/original.mp4".into();
    c.manifest.source.imported_path = "materials/original.mp4".into();
    c.validate().unwrap();
    assert_eq!(old, c.targets);
    // VU IDs and boundaries are deliberately absent from saved evidence targets.
    assert!(!serde_json::to_string(&old).unwrap().contains("visual_unit"));
    let mut next = c.clone();
    next.manifest.material.book_id = "revised".into();
    next.manifest.previous_book_id = Some(c.manifest.material.book_id.clone());
    next.manifest.material.transcript.transcript_revision = "corrected".into();
    next.manifest.validate().unwrap();
    next.manifest.validate_successor(&c.manifest).unwrap();
    assert!(old[0]
        .validate(&next.manifest, &c.base, &c.source_text)
        .is_err());
    old[0]
        .validate(&c.manifest, &c.base, &c.source_text)
        .unwrap();
}

#[test]
fn video_source_revision_and_enrichment_have_distinct_book_identity_rules() {
    let c = lecture();
    let mut next = c.manifest.clone();
    next.coverage.clear();
    next.validate_successor(&c.manifest).unwrap();
    next.material.transcript.transcript_revision = "corrected".into();
    assert!(next.validate_successor(&c.manifest).is_err());
    next.material.book_id = "new-book".into();
    assert!(next.validate_successor(&c.manifest).is_err());
    next.previous_book_id = Some(c.manifest.material.book_id.clone());
    next.validate_successor(&c.manifest).unwrap();
    next.material.transcript = c.manifest.material.transcript.clone();
    assert!(next.validate_successor(&c.manifest).is_err());
}
