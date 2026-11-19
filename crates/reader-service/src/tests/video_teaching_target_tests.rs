use super::*;
use crate::tests::tutor_loop_tests::prepare_activity_chat as prepare_agent_chat;

pub fn binding() -> TeachingBinding {
    TeachingBinding { tutor_session_id:"tutor".into(), session_revision:1, control_revision:1,
        source_id:"lecture".into(), source_revision:"book-source-revision".into(), map_revision:None,
        chat_session_id:"chat".into(), turn_id:"turn".into() }
}

pub fn video_targets(book: &str, review: &str) -> Vec<base_schema::EvidenceTarget> {
    let publication = json!({"book_id":book,"publication_id":"original"});
    let video = json!({"media_id":"lecture-video","media_revision":"original-video"});
    let text = json!({"kind":"text","publication":publication,"transcript":{
        "transcript_id":"transcript","transcript_revision":"1","video":video},
        "text_range":{"lid":"1.1","range":{"start":0,"end":30}}});
    let visual = json!({"kind":"visual","publication":publication,
        "frames":[{"video":video,"actual_pts_ms":1200,"width":1920,"height":1080}],"regions":[]});
    let audio = json!({"kind":"audio","publication":publication,
        "location":{"video":video,"start_ms":1000,"end_ms":2000},"review_result_ref":review});
    let joint = json!({"kind":"joint","members":[{"target":visual,"support_role":"shown relation"},
        {"target":audio,"support_role":"spoken explanation"}]});
    vec![text, visual, audio, joint].into_iter().map(|target| serde_json::from_value(target).unwrap()).collect()
}

pub fn source(binding: &TeachingBinding, target: base_schema::EvidenceTarget) -> SourceBinding {
    let publication = runtime::video_source::publication(&target).unwrap().clone();
    SourceBinding { source_ref_id:"read-video".into(), book_id:binding.source_id.clone(),
        publication_id:Some(publication.publication_id.clone()), source_revision:Some(binding.source_revision.clone()),
        evidence_range:None, evidence_text_digest:String::new(), label_snapshot:"Lecture excerpt".into(),
        preview_snapshot:"The full journey includes moving and stopped time.".into(), music:None,
        media:Some(runtime::video_source::MediaSource {target, publication, duration_ms:3000.0,
            transcript:vec![],translations:vec![],observations:vec![],audio_reviews:vec![]}) }
}

pub fn request() -> Value {
    json!({"learning_focus":"whole journey","expected_performance":"Explain which intervals belong in total time",
        "capability":"explanation","source_bindings":[],"source_ref_ids":["read-video"]})
}

#[test]
fn video_teaching_target_binds_each_observed_modality_and_rechecks_normalized_review() {
    let binding = binding();
    for target in video_targets(&binding.source_id,"original-review") {
        let source = source(&binding,target.clone());
        assert!(bind_target(request(),&binding,&[]).is_err(),"unread media cannot become teaching evidence");
        let bound = bind_target(request(),&binding,&[source.clone()]).unwrap();
        assert!(bound.source_bindings.is_empty(),"media must not become invented text LIDs");
        assert!(bound.music_bindings.is_none());
        assert_eq!(bound.media_bindings.as_ref().unwrap()[0].target,target);
        assert_eq!(bind_target(json!(bound),&binding,&[source.clone()]).unwrap(),bound,
            "reviewed normalized media target must commit unchanged");
        assert!(bind_target(json!(bound),&binding,&[]).is_err(),"a saved target is not a new turn's observation");
        let mut stale = source.clone();
        stale.source_revision=Some("other-source-version".into());
        assert!(bind_target(request(),&binding,&[stale]).is_err());
        let mut forged = json!(bound);
        forged["media_bindings"][0]["target"] = json!(video_targets(&binding.source_id,"unread-review")[2]);
        assert!(bind_target(forged,&binding,&[source]).is_err(),"an unobserved audio receipt cannot replace the reviewed source");
    }
}

#[test]
fn video_teaching_target_audio_continuation_uses_the_new_observation_without_changing_scope() {
    let binding = binding();
    let original = source(&binding,video_targets(&binding.source_id,"first-review")[3].clone());
    let mut target = bind_target(request(),&binding,&[original]).unwrap();
    assert!(refresh_inherited_media(&mut target,&binding,&[]).is_err());
    let reread = source(&binding,video_targets(&binding.source_id,"second-review")[3].clone());
    refresh_inherited_media(&mut target,&binding,&[reread.clone()]).unwrap();
    assert_eq!(target.media_bindings.as_ref().unwrap()[0].target,reread.media.as_ref().unwrap().target);
    assert_eq!(bind_target(json!(target),&binding,&[reread.clone()]).unwrap(),target);
    let mut changed = reread;
    if let base_schema::EvidenceTarget::Joint { members } = &mut changed.media.as_mut().unwrap().target {
        if let base_schema::EvidenceTarget::Audio { location,.. } = &mut members[1].target { location.start_ms=500.0; }
    }
    assert!(refresh_inherited_media(&mut target,&binding,&[changed]).is_err(),"the next read must not silently change the original clip");
}

#[test]
fn video_teaching_target_design_delivery_and_next_turn_preserve_media_sources() {
    let text = "Average speed is total distance divided by all elapsed time, including stops. ".repeat(4);
    let (_root,mut state) = crate::tests::tutor_loop_tests::fixture_with_book(false,
        Book::new(base_schema::sample_base(),&text),&text);
    let first = prepare_agent_chat(&mut state,r#"{"message":"Explain the whole journey shown in this lecture."}"#,"first")
        .unwrap_or_else(|error| panic!("{}",error.body));
    let binding = bound(&state.private_context(),&first.turn_ref.turn_id).unwrap().unwrap().binding;
    let read = source(&binding,video_targets(&binding.source_id,"first-review")[3].clone());
    let mut design = crate::tests::teaching_design_tests::design_request("video-design",None);
    design["design"]["target"]=request();
    design["design"]["blocks"][0]["source_bindings"]=json!([]);
    let checked = preflight_step(&state.private_context(),&first.turn_ref,&design,&[read.clone()],&[]).unwrap();
    let accepted = step(&state.private_context(),&first.turn_ref,checked,&[read.clone()],&[]).unwrap();
    let mut selection = crate::tests::teaching_design_tests::select_request("video-explain",accepted["design_ref"].as_str().unwrap(),false);
    selection["move"]["target"]=request();
    selection["move"]["continuation"]=json!({"recommended_option_id":"compare","options":[{
        "option_id":"compare","label":"Compare the stopped and moving intervals","purpose":"Connect both intervals to the same journey","move_kind":"compare"}]});
    let checked = preflight_step(&state.private_context(),&first.turn_ref,&selection,&[read.clone()],&[]).unwrap();
    step(&state.private_context(),&first.turn_ref,checked,&[read.clone()],&[]).unwrap();
    let mut output=crate::tests::tutor_loop_tests::outcome(None);
    output.source_bindings=vec![read];
    crate::tests::tutor_loop_tests::finish(&mut state,&first.turn_ref,&output);
    let delivered=state.user.learning_store().unwrap().teaching_deliveries(&first.turn_ref.session_id,&first.turn_ref.turn_id).unwrap().remove(0);
    assert!(delivered.payload["move"]["target"]["media_bindings"].is_array());
    assert_eq!(continuation_view(&state.private_context(),&delivered).unwrap().unwrap().availability,
        runtime::tutor_continuation::ContinuationAvailability::Eligible);

    let second = prepare_agent_chat(&mut state,r#"{"message":"Compare these intervals."}"#,"second")
        .unwrap_or_else(|error| panic!("{}",error.body));
    assert!(second.tutor.as_ref().unwrap()["learner_context"]["current_target"]["media_bindings"].is_array());
    let mut followup=selection;
    followup["move"]["move_id"]=json!("video-compare");
    followup["move"]["target"]=Value::Null;
    followup["move"]["continuation"]=Value::Null;
    followup["move"]["actions"]=json!(["submit"]);
    followup["move"]["assessment"]=json!({"rule":{"kind":"choice_set","choices":["all","moving"],"correct":["all"]},
        "source_lids":[],"source_ref_ids":["read-video"],"source_sufficient":true,"feedback":"after_submit"});
    followup["move"]["participation"]=json!({"choice":"invite","reason":"Check the current relationship after the explanation",
        "learner_inference":"Choose which intervals belong to the whole journey","support":"The lecture excerpt provides the two intervals"});
    assert!(preflight_step(&state.private_context(),&second.turn_ref,&followup,&[],&[]).is_err(),
        "stored media targets must be reread before a new delivery");
    let reread=source(&binding,video_targets(&binding.source_id,"second-review")[3].clone());
    let checked=preflight_step(&state.private_context(),&second.turn_ref,&followup,&[reread.clone()],&[]).unwrap();
    assert_eq!(checked["move"]["target"]["media_bindings"][0]["target"]["members"][1]["target"]["review_result_ref"],"second-review");
    step(&state.private_context(),&second.turn_ref,checked,&[reread.clone()],&[]).unwrap();
    let mut output=crate::tests::tutor_loop_tests::outcome(None);
    output.source_bindings=vec![reread];
    crate::tests::tutor_loop_tests::finish(&mut state,&second.turn_ref,&output);
    let delivered=state.user.learning_store().unwrap().teaching_deliveries(&second.turn_ref.session_id,&second.turn_ref.turn_id).unwrap().remove(0);
    let interaction:runtime::tutor_continuation::ResolvedInteraction=serde_json::from_value(json!({
        "action":{"origin":"typed"},"user_supplement":"Explain this same comparison", "accepted_request":"Explain this same comparison",
        "answer_ref":null,"answer_text":null,"source_excerpt":null,"source_bindings":[],
        "target":delivered.payload["move"]["target"],"expected_chat_head":null,"presentation_follow_up":null})).unwrap();
    assert_eq!(pending_activity_help(&state.private_context(),&interaction).unwrap(),vec![delivered.event_id.clone()],
        "help for the same media target must preserve its pending activity");
    let display=json!({"delivery_ref":delivered.event_id}).to_string();
    let displayed=route(&state.private_context(),Req {method:"POST",url:"/tutor/display",body:&display,now:"display"});
    assert_eq!(displayed.status,200,"{}",displayed.body);
    let result=behavior(&state.private_context(),BehaviorRequest { operation_id:"video-answer".into(),delivery_ref:delivered.event_id,
        action:"submit".into(),response:Some("all".into()),reasoning:None,response_origin:None,scene:None },"answer").unwrap();
    assert_eq!(result["assessment"],"correct","a video-only target must assess an actual submission without invented text LIDs");
}

#[test]
fn video_teaching_target_overlap_respects_original_publication_and_observed_regions() {
    let targets=video_targets("lecture","review");
    assert!(media_targets_overlap(&targets[2],&video_targets("lecture","new-review")[2]));
    assert!(media_targets_overlap(&targets[3],&targets[1]));
    assert!(!media_targets_overlap(&targets[1],&video_targets("other-lecture","review")[1]));
    let mut earlier=targets[2].clone();
    if let base_schema::EvidenceTarget::Audio { location,.. }=&mut earlier { location.start_ms=0.0;location.end_ms=1000.0; }
    assert!(!media_targets_overlap(&targets[2],&earlier),"adjacent clips do not overlap");
}
