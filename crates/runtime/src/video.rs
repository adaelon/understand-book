//! Real video input is separate from presentation previews and text evidence.
use base_schema::{
    FrameRead, FrameRegion, MediaLocation, MediaObservation, PublishedBookRef, TranscriptSegment,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReadFrames {
    pub location: MediaLocation,
    /// Explicit ascending sample times on the original timeline, at most eight.
    pub sampling: Vec<f64>,
    pub region: Option<[f64; 4]>,
}

pub fn spec() -> crate::ToolSpec {
    crate::ToolSpec {
        name: "video.read_frames".into(),
        description: "Read real ordered video frames and matching English transcript from this Run's fixed publication. At most 8 ascending sample times within a range of at most 60 seconds (milliseconds). Optional region [x,y,width,height] is normalized to original pixels; use crops for small code/formulas. Returned samples do not prove continuous coverage. Images become observed only after a successful main-model request; paths or descriptions are not visual evidence.".into(),
        parameters: json!({"type":"object","properties":{
            "location":{"type":"object","properties":{
                "video":{"type":"object","properties":{"media_id":{"type":"string"},"media_revision":{"type":"string"}},"required":["media_id","media_revision"],"additionalProperties":false},
                "start_ms":{"type":"number","minimum":0},"end_ms":{"type":"number","minimum":0}
            },"required":["video","start_ms","end_ms"],"additionalProperties":false},
            "sampling":{"type":"array","items":{"type":"number","minimum":0},"minItems":1,"maxItems":8},
            "region":{"type":"array","items":{"type":"number","minimum":0,"maximum":1},"minItems":4,"maxItems":4}
        },"required":["location","sampling"],"additionalProperties":false}),
    }
}

pub fn unavailable() -> read_tools::ToolError {
    read_tools::ToolError {
        error_code: "VIDEO_UNAVAILABLE".into(),
        category: "unavailable".into(),
        message: "This Run has no published video source".into(),
    }
}

pub fn search_spec() -> crate::ToolSpec {
    let mut location = spec().parameters["properties"]["location"].clone();
    location["description"] = json!("Optional original-video range in milliseconds; omitted means the full video.");
    crate::ToolSpec {
        name: "video.search".into(),
        description: "Locate visual candidates by visible code/text, Chinese descriptions or changes, or an exact vu_id in this Run's fixed publication. Use concise concepts/identifiers from the question; reformulate descriptive queries when needed. Returns independent occurrences, analysis IDs, coverage gaps and video.read_frames hints. The discourse section also recalls teaching roles and follows explanations, examples or limitations to supporting material; inspect via for the traversed relation and read text_reads with book.text or read_hints with video.read_frames. Visual and discourse candidates have separate counts and pagination using the same limit/offset. Cards are derived pointers, never evidence; read actual frames before citing. Default chooses the newest accepted analysis at each position; analysis_id selects a published analysis explicitly. No match does not establish absence.".into(),
        parameters: json!({"type":"object","properties":{
            "query":{"type":"string","minLength":1,"maxLength":512},"within":location,
            "analysis_id":{"type":"string"},"limit":{"type":"integer","minimum":1,"maximum":20},
            "offset":{"type":"integer","minimum":0}
        },"required":["query"],"additionalProperties":false}),
    }
}

#[derive(Debug, Clone)]
pub struct VideoImage {
    pub read: FrameRead,
    pub region: Option<FrameRegion>,
    pub png_base64: String,
    pub image_width: u32,
    pub image_height: u32,
}

#[derive(Debug, Clone)]
pub struct FrameBatch {
    pub run_id: String,
    pub publication: PublishedBookRef,
    pub location: MediaLocation,
    pub images: Vec<VideoImage>,
    pub transcript: Vec<TranscriptSegment>,
}
impl FrameBatch {
    pub fn projection(&self) -> Value {
        json!({"publication":self.publication,"requested_range":self.location,
            "observation_status":"pending_model_request","continuous_coverage":false,
            "frames":self.images.iter().map(|i| json!({"read":i.read,"region":i.region,
                "image_width":i.image_width,"image_height":i.image_height})).collect::<Vec<_>>(),
            "transcript":self.transcript})
    }
}

#[derive(Debug, Clone)]
pub struct ModelVideoImage {
    pub run_id: String,
    pub publication: PublishedBookRef,
    pub model_request_id: String,
    pub image: VideoImage,
}
impl ModelVideoImage {
    pub fn caption(&self) -> String {
        json!({"purpose":"video_source","model_request_id":self.model_request_id,
            "publication":self.publication,"read":self.image.read,"region":self.image.region,
            "continuous_coverage":false})
        .to_string()
    }
}

/// Pending pixels live for one sampling only, never in stored conversation history.
pub fn attach(plan: &mut crate::AgentRequestPlan, batches: Vec<FrameBatch>) {
    let mut images: Vec<_> = batches
        .into_iter()
        .flat_map(|b| {
            b.images.into_iter().map(move |image| ModelVideoImage {
                run_id: b.run_id.clone(),
                publication: b.publication.clone(),
                model_request_id: String::new(),
                image,
            })
        })
        .collect();
    if images.is_empty() {
        return;
    }
    // Include metadata/captions in addition to the existing 4096-token image allowance.
    let request_id = uuid::Uuid::now_v7().to_string();
    for i in &mut images {
        i.model_request_id = request_id.clone();
    }
    let cost = |i: &ModelVideoImage| 4096 + i.caption().len() as i64 + 128;
    let mut remaining = plan.active_context.remaining_tokens - 256;
    let mut kept = Vec::new();
    let mut omitted = 0;
    for image in images.drain(..) {
        let tokens = cost(&image);
        if tokens <= remaining {
            remaining -= tokens;
            kept.push(image);
        } else {
            omitted += 1;
        }
    }
    let tokens: i64 = kept.iter().map(cost).sum::<i64>() + if omitted > 0 { 256 } else { 0 };
    if omitted > 0 {
        plan.input.push(crate::Message::user(format!("Host video input notice: {omitted} pending video images omitted by the active context budget; these frames remain unobserved. Read a smaller batch or crop if needed.")));
    }
    plan.active_context.estimated_input_tokens += tokens as u32;
    plan.active_context.remaining_tokens -= tokens;
    plan.active_context.fits &= plan.active_context.remaining_tokens >= 0;
    plan.video_images = kept;
}

/// Called only after the main adapter succeeds and cancellation has been checked.
pub fn accept(plan: &crate::AgentRequestPlan, ledger: &mut Vec<MediaObservation>) {
    for input in &plan.video_images {
        ledger.push(MediaObservation::Visual {
            observation_id: uuid::Uuid::now_v7().to_string(),
            run_id: input.run_id.clone(),
            publication: input.publication.clone(),
            model_request_id: input.model_request_id.clone(),
            // A crop observes only its region, never the unprovided full frame.
            frames: if input.image.region.is_none() {
                vec![input.image.read.clone()]
            } else {
                vec![]
            },
            regions: input.image.region.clone().into_iter().collect(),
        });
    }
}

pub(crate) fn accept_observed(
    plan: &crate::AgentRequestPlan,
    ledger: &mut Vec<MediaObservation>,
    events: &crate::run_events::RunEvents,
) {
    let start = ledger.len();
    accept(plan, ledger);
    if ledger.len() == start {
        return;
    }
    let observations = &ledger[start..];
    let mut activity = events.begin("evidence", "video.observe", "观察视频画面", true);
    activity.accepted_evidence_count = Some(observations.len() as u32);
    activity.evidence_refs = observations
        .iter()
        .filter_map(|o| match o {
            MediaObservation::Visual { observation_id, .. } => Some(observation_id.clone()),
            _ => None,
        })
        .collect();
    events.output(activity.step_id, || json!({"observations":observations}));
    events.finish(
        activity,
        crate::run_events::ActivityStatus::Succeeded,
        None,
        None,
        Some(observations.len() as u32),
    );
}
