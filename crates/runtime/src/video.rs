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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<[f64; 4]>,
}

impl ReadFrames {
    /// Validate the whole request before a per-model image budget splits its samples.
    pub fn validate_sampling(&self) -> Result<(), read_tools::ToolError> {
        let invalid = |message: String| read_tools::ToolError { error_code:"VIDEO_READ_FAILED".into(),category:"invalid_input".into(),message };
        let duration_ms = self.location.end_ms - self.location.start_ms;
        if duration_ms > 60_000.0 {
            return Err(invalid(format!("location spans {duration_ms} ms; video.read_frames accepts at most 60000 ms per call. Split the range into shorter reads.")));
        }
        if !(1..=8).contains(&self.sampling.len()) {
            return Err(invalid(format!("sampling has {} entries; supply 1–8 sample times per call.",self.sampling.len())));
        }
        for (index,time) in self.sampling.iter().enumerate() {
            if !time.is_finite() {
                return Err(invalid(format!("sampling[{index}]={time} must be a finite original-timeline time in milliseconds.")));
            }
            if *time < self.location.start_ms || *time >= self.location.end_ms {
                return Err(invalid(format!("sampling[{index}]={time} ms is outside [start_ms={}, end_ms={}) ms. end_ms is exclusive: require start_ms <= sampling[i] < end_ms. Remove this sample or choose a time inside the range.",self.location.start_ms,self.location.end_ms)));
            }
            if index > 0 && self.sampling[index-1] >= *time {
                return Err(invalid(format!("sampling[{index}]={time} ms must be greater than sampling[{}]={} ms. Sort sample times in strictly increasing order and remove duplicates.",index-1,self.sampling[index-1])));
            }
        }
        Ok(())
    }
}

/// A ready-to-call read for an already validated input-time frame and its source duration.
pub fn frozen_frame_read_hint(question: &base_schema::VideoQuestionContext, duration_ms: f64) -> ReadFrames {
    let time = question.frame.actual_pts_ms;
    ReadFrames {
        location: MediaLocation { video: question.frame.video.clone(), start_ms: time, end_ms: (time + 1000.0).min(duration_ms) },
        sampling: vec![time],
        region: question.region.as_ref().map(|r| r.rect_normalized),
    }
}

/// The question's pixels stay available in every request, including finalization.
pub fn attach_question_image(plan: &mut crate::AgentRequestPlan, image: Option<&base_schema::VideoQuestionImage>) {
    let Some(image) = image else { return; };
    plan.question_image = Some(image.clone());
    plan.active_context.estimated_input_tokens += 4352;
    plan.active_context.remaining_tokens -= 4352;
    plan.active_context.fits &= plan.active_context.remaining_tokens >= 0;
}

pub(crate) fn location_schema() -> Value {
    json!({"type":"object","description":"Range on the original video timeline in milliseconds: [start_ms, end_ms). Copy video identity from the current video context or a returned location. Require 0 <= start_ms < end_ms <= video duration_ms.","properties":{
        "video":{"type":"object","description":"Exact media_id and media_revision from the current video context or tool result; example IDs are placeholders.","properties":{"media_id":{"type":"string"},"media_revision":{"type":"string"}},"required":["media_id","media_revision"],"additionalProperties":false},
        "start_ms":{"type":"number","minimum":0,"description":"Inclusive start on the original timeline, in milliseconds (10000 means 10 seconds). Must be less than end_ms."},
        "end_ms":{"type":"number","minimum":0,"description":"Exclusive end in milliseconds; this time itself cannot be a sample. Must be greater than start_ms and no greater than video duration_ms."}
    },"required":["video","start_ms","end_ms"],"additionalProperties":false})
}

pub fn spec() -> crate::ToolSpec {
    crate::ToolSpec {
        name: "video.read_frames".into(),
        description: "Read real video frames and overlapping saved English transcript from the selected workspace publication (published_book_ref), or this Run's publication when omitted. User-attached images can be inspected directly without this tool. For the original frozen frame, copy frame_read_hint as the complete arguments. video.search is only needed to locate unknown positions. Supply 1–8 strictly increasing original-millisecond sample times with start_ms <= t < end_ms in a range of at most 60000 ms. Copy exact actual_pts_ms without rounding. Up to 8 frames across calls accompany each model request: status=partial returns available frames plus complete continuation arguments; status=deferred means earlier successful reads filled the batch. Inspect supplied images before continuing. Neither status is a decode failure. Decoding may return a later frame; use returned read.frame for identity. Optional region crops every sample. Each frame includes source_target ready for source.present, preserving its crop. Images become observed after a successful main-model request; samples do not prove continuous coverage. Paths and descriptions are not visual evidence.".into(),
        parameters: json!({"type":"object","properties":{
            "published_book_ref":crate::library::reference_schema(),
            "location":location_schema(),
            "sampling":{"type":"array","description":"1–8 distinct times in strictly increasing order, in original milliseconds. Every t must satisfy location.start_ms <= t < location.end_ms. Do not include end_ms. For [10000,20000), [10000,15433.3125,19000] is valid; 20000 is not.","items":{"type":"number","minimum":0},"minItems":1,"maxItems":8},
            "region":{"type":"array","description":"Optional [x,y,width,height] against the full original frame, with top-left origin; x,y >= 0, width,height > 0, x + width <= 1 and y + height <= 1. Applies to every sample. Omit for full frames. Returned frame dimensions remain the original dimensions; image_width/image_height describe the crop.","items":{"type":"number","minimum":0,"maximum":1},"minItems":4,"maxItems":4}
        },"required":["location","sampling"],"additionalProperties":false,
        "examples":[
            {"location":{"video":{"media_id":"test-video","media_revision":"1"},"start_ms":0,"end_ms":1000},"sampling":[200]},
            {"location":{"video":{"media_id":"test-video","media_revision":"1"},"start_ms":200,"end_ms":1000},"sampling":[200,900],"region":[0.1,0.2,0.5,0.4]}
        ]}),
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
    let mut location = location_schema();
    location["description"] = json!("Optional original-video range [start_ms,end_ms) in milliseconds; start is inclusive, end exclusive, within video duration. Omit to search the full video; the 60000 ms frame-read limit does not apply to search.");
    crate::ToolSpec {
        name: "video.search".into(),
        description: "Locate visual candidates by visible code/text, Chinese descriptions or changes, or an exact vu_id in the selected workspace publication (published_book_ref), or this Run's publication when omitted. Use concise concepts/identifiers from the question; reformulate descriptive queries when needed. Returns independent occurrences, analysis IDs, coverage gaps and video.read_frames hints. The discourse section also recalls teaching roles and follows explanations, examples or limitations to supporting material; inspect via for the traversed relation and read text_reads with book.text or read_hints with video.read_frames. Visual and discourse candidates have separate counts and pagination using the same limit/offset. Cards are derived pointers, never evidence; read actual frames before citing. Default chooses the newest accepted analysis at each position; analysis_id selects a published analysis explicitly. No match does not establish absence.".into(),
        parameters: json!({"type":"object","properties":{
            "published_book_ref":crate::library::reference_schema(),
            "query":{"type":"string","minLength":1,"maxLength":512,"description":"Nonblank query of at most 512 characters: visible identifiers/code, a short Chinese or English description, a teaching role, or an exact returned vu_id. Matching is lexical; no matches do not prove absence."},"within":location,
            "analysis_id":{"type":"string","description":"Optional exact visual analysis ID returned in analysis_ids or a candidate. Omit to use the newest accepted analysis at each position; do not invent an ID."},
            "limit":{"type":"integer","minimum":1,"maximum":20,"default":8,"description":"Maximum candidates per result section (visual and discourse), 1–20; default 8."},
            "offset":{"type":"integer","minimum":0,"default":0,"description":"Zero-based offset applied independently to visual and discourse results. Each section has its own next_offset; follow the section you need with query/within/analysis_id unchanged."}
        },"required":["query"],"additionalProperties":false,
        "examples":[
            {"query":"float16"},
            {"query":"float16","within":{"video":{"media_id":"test-video","media_revision":"1"},"start_ms":0,"end_ms":2000},"limit":4,"offset":0}
        ]}),
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
                "source_target":{"kind":"visual","publication":self.publication,
                    "frames":if i.region.is_none() { vec![i.read.frame.clone()] } else { vec![] },
                    "regions":i.region.iter().collect::<Vec<_>>()},
                "image_width":i.image_width,"image_height":i.image_height})).collect::<Vec<_>>(),
            "transcript":self.transcript})
    }
}

/// Reopen only the original observed frames/crops for teaching review or assessment.
/// Callers retain their own admission rules; this read must preserve source identity.
pub(crate) fn read_exact_visual_targets(
    targets: &[(&base_schema::EvidenceTarget, f64)],
    mut read: impl FnMut(&PublishedBookRef, ReadFrames) -> Result<FrameBatch, read_tools::ToolError>,
) -> Result<Vec<FrameBatch>, read_tools::ToolError> {
    let mut batches: Vec<FrameBatch> = Vec::new();
    for (target, duration_ms) in targets {
        for member in crate::video_source::members(target) {
            let base_schema::EvidenceTarget::Visual { publication, frames, regions } = member else { continue; };
            for (frame, region) in frames.iter().map(|f| (f, None))
                .chain(regions.iter().map(|r| (&r.frame, Some(r)))) {
                if batches.iter().any(|batch| batch.publication == *publication
                    && batch.images[0].read.frame == *frame && batch.images[0].region.as_ref() == region) {
                    continue;
                }
                let request = ReadFrames {
                    location: MediaLocation { video: frame.video.clone(), start_ms: frame.actual_pts_ms,
                        end_ms: (frame.actual_pts_ms + 1000.0).min(*duration_ms) },
                    sampling: vec![frame.actual_pts_ms], region: region.map(|r| r.rect_normalized),
                };
                let batch = read(publication, request)?;
                if batch.publication != *publication || batch.images.len() != 1
                    || batch.images[0].read.frame != *frame || batch.images[0].region.as_ref() != region {
                    return Err(memory::teaching::invalid("Frozen video frame or crop could not be reacquired; original source evidence is required"));
                }
                batches.push(batch);
            }
        }
    }
    Ok(batches)
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
