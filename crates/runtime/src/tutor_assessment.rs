//! Reacquire the exact frozen video pixels for the isolated response evaluator.
use crate::video::{FrameBatch, ReadFrames};
use base_schema::PublishedBookRef;
use memory::{assessment::AssessmentSource, teaching::invalid};
use read_tools::ToolError;
use std::collections::BTreeMap;

pub(crate) fn read_visual_sources(
    packet: &serde_json::Value,
    read: impl FnMut(&PublishedBookRef, ReadFrames) -> Result<FrameBatch, ToolError>,
) -> Result<Vec<FrameBatch>, ToolError> {
    let sources: BTreeMap<String, AssessmentSource> = serde_json::from_value(
        packet.get("source_references").cloned().unwrap_or_else(|| serde_json::json!({})))
        .map_err(|e| invalid(format!("Frozen assessment sources are unreadable: {e}")))?;
    let targets: Vec<_> = sources.values().filter_map(|source| source.visual.as_ref())
        .map(|visual| (&visual.target, visual.duration_ms)).collect();
    crate::video::read_exact_visual_targets(&targets, read)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ModelAdapter;
    use base_schema::MediaLocation;
    use serde_json::json;

    fn fixture() -> (serde_json::Value, FrameBatch) {
        let publication=PublishedBookRef {book_id:"video".into(),publication_id:"original".into()};
        let frame:base_schema::FrameLocation=serde_json::from_value(json!({"video":{"media_id":"video","media_revision":"v1"},"actual_pts_ms":1000,"width":640,"height":480})).unwrap();
        let region=base_schema::FrameRegion {frame:frame.clone(),rect_normalized:[0.1,0.2,0.4,0.5]};
        let packet=json!({"source_references":{"read-frame":{"source_id":"video","source_revision":"book-revision","text":"",
            "visual":{"target":{"kind":"visual","publication":publication,"frames":[],"regions":[region]},"duration_ms":2000}}}});
        let batch=FrameBatch {run_id:"assessment".into(),publication,
            location:MediaLocation {video:frame.video.clone(),start_ms:1000.0,end_ms:2000.0},
            images:vec![crate::video::VideoImage {read:base_schema::FrameRead {requested_ms:1000.0,frame},region:Some(region),
                png_base64:"actual-frozen-pixels".into(),image_width:256,image_height:240}],transcript:vec![]};
        (packet,batch)
    }

    #[test]
    fn media_assessment_reopens_original_frame_and_crop_and_rejects_different_pixels() {
        let (packet,batch)=fixture();
        let images=read_visual_sources(&packet,|reference,request| {
            assert_eq!(*reference,batch.publication);
            assert_eq!(request.sampling,vec![1000.0]);
            assert_eq!(request.region,Some([0.1,0.2,0.4,0.5]));
            Ok(batch.clone())
        }).unwrap();
        assert_eq!(images[0].images[0].png_base64,"actual-frozen-pixels");
        assert!(read_visual_sources(&packet,|_,_| {
            let mut other=batch.clone();other.images[0].region=None;Ok(other)
        }).is_err(),"a full frame cannot replace the originally read crop");
        assert!(read_visual_sources(&packet,|_,_| {
            let mut other=batch.clone();other.images[0].read.frame.actual_pts_ms=1200.0;Ok(other)
        }).is_err(),"a nearby frame cannot stand in for the frozen original");
        assert!(read_visual_sources(&packet,|_,_| {
            let mut other=batch.clone();other.publication.publication_id="another-publication".into();Ok(other)
        }).is_err(),"another publication cannot stand in for the frozen original");
    }

    #[test]
    fn media_assessment_reuses_the_same_exact_frame_for_multiple_source_references() {
        let (mut packet,batch)=fixture();
        packet["source_references"]["second-reference"]=packet["source_references"]["read-frame"].clone();
        let mut reads=0;
        let images=read_visual_sources(&packet,|_,_| {reads+=1;Ok(batch.clone())}).unwrap();
        assert_eq!(reads,1,"references to the same exact publication/frame/crop share one read");
        assert_eq!(images.len(),1);
    }

    #[test]
    fn media_assessment_isolated_evaluator_receives_pixels_and_frozen_reference_identity() {
        struct Evaluator;
        impl ModelAdapter for Evaluator {
            fn complete(&self,_:crate::CompletionRequest)->Result<crate::ParsedResponse,crate::AdapterError> {unreachable!()}
            fn chat(&self,request:&crate::AgentRequestPlan)->Result<crate::AssistantTurn,crate::AdapterError> {
                assert_eq!(request.video_images.len(),1);
                assert_eq!(request.video_images[0].image.png_base64,"actual-frozen-pixels");
                assert!(request.input.iter().any(|m| m.content.as_deref().is_some_and(|s|s.contains("read-frame"))));
                assert!(request.video_images[0].image.region.is_some());
                Ok(crate::AssistantTurn {provider_continuation:None,text:Some("{\"items\":[]}".into()),tool_calls:vec![],usage_total_tokens:Some(12)})
            }
        }
        let (packet,batch)=fixture();let adapter=Evaluator;
        let (output,usage)=crate::tutor::evaluate(&adapter,adapter.model_runtime_profile(),packet,vec![batch]).unwrap();
        assert_eq!(output["tool_calls"],false);assert_eq!(usage,12);
    }
}
