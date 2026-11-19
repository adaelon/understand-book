use super::*;
use crate::{AdapterError, CompletionRequest, ParsedResponse};
use base_schema::{EvidenceTarget, FrameLocation, FrameRead, FrameRegion, MediaLocation, MediaObservation, PublishedBookRef, VideoQuestionContext};
use serde_json::{json, Value};

fn fixture(id: &str, time: f64, crop: bool) -> (SourceBinding, MediaObservation, crate::video::FrameBatch, VideoQuestionContext) {
    let publication = PublishedBookRef {book_id:"review-video".into(), publication_id:"original".into()};
    let frame: FrameLocation = serde_json::from_value(json!({"video":{"media_id":"video","media_revision":"1"},
        "actual_pts_ms":time,"width":640,"height":480})).unwrap();
    let region = crop.then(|| FrameRegion {frame:frame.clone(),rect_normalized:[0.1,0.2,0.4,0.5]});
    let read = FrameRead {requested_ms:time,frame:frame.clone()};
    let target = EvidenceTarget::Visual {publication:publication.clone(),
        frames:if crop {vec![]} else {vec![frame.clone()]},regions:region.iter().cloned().collect()};
    let observation = MediaObservation::Visual {observation_id:format!("seen-{id}"),run_id:"run".into(),
        publication:publication.clone(),model_request_id:"teacher-request".into(),
        frames:if crop {vec![]} else {vec![read.clone()]},regions:region.iter().cloned().collect()};
    let binding = SourceBinding {source_ref_id:id.into(),book_id:publication.book_id.clone(),
        publication_id:Some(publication.publication_id.clone()),source_revision:Some("source-revision".into()),
        evidence_range:None,evidence_text_digest:String::new(),music:None,label_snapshot:id.into(),preview_snapshot:"source locator".into(),
        media:Some(crate::video_source::MediaSource {target,publication:publication.clone(),duration_ms:10_000.0,
            transcript:vec![],translations:vec![],observations:vec![observation.clone()],audio_reviews:vec![]})};
    let batch = crate::video::FrameBatch {run_id:"review-read".into(),publication:publication.clone(),
        location:MediaLocation {video:frame.video.clone(),start_ms:time,end_ms:time+1000.0},
        images:vec![crate::video::VideoImage {read,region:region.clone(),png_base64:format!("pixels-{id}"),
            image_width:if crop {256} else {640},image_height:if crop {240} else {480}}],transcript:vec![]};
    let question: VideoQuestionContext = serde_json::from_value(json!({"draft_id":"frozen-draft","publication":publication,
        "material":{"book_id":"review-video","video":frame.video,"transcript":{"transcript_id":"transcript","transcript_revision":"1","video":frame.video}},
        "frame":frame,"region":region,"range":null,
        "image":{"png_base64":"attached-pixels","width":256,"height":240,"origin":"selection"}})).unwrap();
    (binding,observation,batch,question)
}

#[test]
fn teaching_video_review_reopens_only_cited_or_targeted_observed_pixels() {
    let (crop,crop_observation,crop_batch,_) = fixture("selected-crop",1000.5,true);
    let (frame,frame_observation,frame_batch,_) = fixture("cited-frame",2000.0,false);
    let (unrelated,unrelated_observation,_,_) = fixture("other-frame",8000.0,false);
    let design = json!({"design":{"target":{"media_bindings":[{"target":crop.media.as_ref().unwrap().target}]}}});
    let draft = json!({"text":"Look at this frame. [[source:cited-frame]]","moves":[]});
    let selected = teaching_review_source_bindings(vec![crop,frame,unrelated],&design,&draft,&[]);
    assert_eq!(selected.iter().map(|source| source.source_ref_id.as_str()).collect::<Vec<_>>(),vec!["selected-crop","cited-frame"]);
    let observations = vec![crop_observation,frame_observation,unrelated_observation];
    let mut calls = Vec::new();
    let images = teaching_review_video_images(&selected,&observations,None,|publication,request| {
        calls.push((publication.clone(),request.clone()));
        Ok(if request.sampling[0] == 1000.5 {crop_batch.clone()} else {frame_batch.clone()})
    }).unwrap();
    assert_eq!(calls.len(),2);
    assert_eq!(calls[0].1.sampling,vec![1000.5]);
    assert_eq!(calls[0].1.region,Some([0.1,0.2,0.4,0.5]));
    assert_eq!(calls[1].1.sampling,vec![2000.0]);
    assert_eq!(calls[1].1.region,None);
    assert_eq!(images[0].png_base64,"pixels-selected-crop");
    let caption:Value = serde_json::from_str(&images[0].caption).unwrap();
    assert_eq!(caption["publication"]["publication_id"],"original");
    assert_eq!(caption["region"]["rect_normalized"],json!([0.1,0.2,0.4,0.5]));
}

#[test]
fn teaching_video_review_rejects_unobserved_or_unrecoverable_frame_without_locator_fallback() {
    let (binding,observation,batch,_) = fixture("selected-crop",1000.5,true);
    let error = teaching_review_video_images(std::slice::from_ref(&binding),&[],None,|_,_| {
        panic!("unobserved targets must not be read for review")
    }).unwrap_err();
    assert_eq!(error.error_code,"SOURCE_NOT_OBSERVED");
    let error = teaching_review_video_images(&[binding],&[observation],None,|_,_| {
        let mut changed = batch.clone();changed.images[0].region=None;Ok(changed)
    }).unwrap_err();
    assert_eq!(error.error_code,"TEACHING_REVIEW_SOURCE_UNAVAILABLE");
}

#[test]
fn teaching_video_review_structured_request_receives_pixels_and_frozen_question_metadata() {
    struct Reviewer;
    impl ModelAdapter for Reviewer {
        fn complete(&self,_:CompletionRequest)->Result<ParsedResponse,AdapterError> {unreachable!()}
        fn chat(&self,_:&AgentRequestPlan)->Result<AssistantTurn,AdapterError> {unreachable!()}
        fn complete_structured(&self,request:CompletionRequest)->Result<Value,AdapterError> {
            let packet:Value = serde_json::from_str(&request.user).unwrap();
            assert_eq!(packet["frozen_input"]["video_question"]["frame"]["actual_pts_ms"],1000.5);
            assert_eq!(packet["frozen_input"]["video_question"]["image"]["origin"],"selection");
            assert!(!request.user.contains("attached-pixels"));
            assert!(!request.user.contains("pixels-selected-crop"));
            assert_eq!(request.images.len(),2);
            assert_eq!(request.images[0].png_base64,"pixels-selected-crop");
            assert_eq!(request.images[1].png_base64,"attached-pixels");
            assert!(request.images[1].caption.contains("\"source_observation\":false"));
            assert!(request.system.contains("teaching_review_video_source"));
            assert_eq!(request.output_token_limit,Some(8_000));
            Ok(json!({"decision":"accept"}))
        }
    }
    let (binding,observation,batch,question) = fixture("selected-crop",1000.5,true);
    let images = teaching_review_video_images(&[binding],&[observation],Some(&question),|_,_|Ok(batch.clone())).unwrap();
    let packet = json!({"frozen_input":{"video_question":teaching_review_video_question(Some(&question))},
        "proposed_presentations":[]});
    let adapter = Reviewer;
    let (review,_) = crate::tutor::review_delivery(&adapter,adapter.model_runtime_profile(),packet,images).unwrap();
    assert_eq!(review["decision"],"accept");
}
