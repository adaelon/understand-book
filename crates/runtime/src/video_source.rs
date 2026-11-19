//! Durable media citations are admitted only from this turn's successful observations.
use base_schema::{EvidenceTarget, MediaObservation, PublishedBookRef, TranscriptSegment, TranscriptTranslation};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaSource {
    pub target: EvidenceTarget,
    pub publication: PublishedBookRef,
    pub duration_ms: f64,
    pub transcript: Vec<TranscriptSegment>,
    pub translations: Vec<TranscriptTranslation>,
    #[serde(default)]
    pub observations: Vec<MediaObservation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub audio_reviews: Vec<crate::video_audio::AudioReview>,
}

pub fn publication(target: &EvidenceTarget) -> Option<&PublishedBookRef> {
    match target {
        EvidenceTarget::Text { publication, .. } | EvidenceTarget::Visual { publication, .. }
        | EvidenceTarget::Audio { publication, .. } => Some(publication),
        EvidenceTarget::Joint { members } => members.first().and_then(|m| publication(&m.target)),
    }
}

pub fn members(target: &EvidenceTarget) -> Vec<&EvidenceTarget> {
    match target { EvidenceTarget::Joint { members } => members.iter().map(|m| &m.target).collect(), _ => vec![target] }
}

pub fn start_ms(source: &MediaSource) -> f64 {
    members(&source.target).iter().filter_map(|t| match t {
        EvidenceTarget::Visual { frames, regions, .. } => frames.iter().map(|f| f.actual_pts_ms)
            .chain(regions.iter().map(|r| r.frame.actual_pts_ms)).min_by(f64::total_cmp),
        EvidenceTarget::Audio { location, .. } => Some(location.start_ms),
        EvidenceTarget::Text { text_range, .. } => source.transcript.iter().find(|s| s.text_range.lid == text_range.lid).map(|s| s.location.start_ms),
        _ => None,
    }).min_by(f64::total_cmp).unwrap_or(0.0)
}

pub(crate) fn observed(target: &EvidenceTarget, ledger: &[MediaObservation]) -> bool {
    match target {
        EvidenceTarget::Visual { publication, frames, regions } => {
            (!frames.is_empty() || !regions.is_empty()) && frames.iter().all(|frame| ledger.iter().any(|o| matches!(o,
                MediaObservation::Visual { publication: p, frames: fs, .. } if p == publication && fs.iter().any(|f| &f.frame == frame))))
            && regions.iter().all(|region| ledger.iter().any(|o| matches!(o,
                MediaObservation::Visual { publication: p, regions: rs, frames: fs, .. }
                if p == publication && (rs.contains(region) || fs.iter().any(|f| f.frame == region.frame)))))
        }
        EvidenceTarget::Audio { publication, location, review_result_ref } => ledger.iter().any(|o| matches!(o,
            MediaObservation::Audio { publication: p, location: l, review_result_ref: r, .. }
            if p == publication && l == location && r == review_result_ref)),
        _ => false,
    }
}

/// Resolve a frame locator to the crops actually seen when no full frame was read.
/// Never expand a crop's coverage or carry it across publications or timestamps.
pub(crate) fn resolve_observed_crops(mut target: EvidenceTarget, ledger: &[MediaObservation]) -> EvidenceTarget {
    match &mut target {
        EvidenceTarget::Visual { publication,frames,regions } => {
            frames.retain(|frame| {
                if ledger.iter().any(|o| matches!(o,MediaObservation::Visual { publication:p,frames:fs,.. }
                    if p == publication && fs.iter().any(|f| &f.frame == frame))) { return true; }
                let crops = ledger.iter().filter_map(|o| match o {
                    MediaObservation::Visual { publication:p,regions:rs,.. } if p == publication => Some(rs), _ => None,
                }).flatten().filter(|r| &r.frame == frame).collect::<Vec<_>>();
                if crops.is_empty() { return true; }
                for crop in crops { if !regions.contains(crop) { regions.push(crop.clone()); } }
                false
            });
        },
        EvidenceTarget::Joint { members } => for member in members {
            member.target = resolve_observed_crops(member.target.clone(),ledger);
        },
        _ => {},
    }
    target
}

pub fn source_schema() -> serde_json::Value {
    use serde_json::json;
    let p = crate::library::reference_schema();
    let video = json!({"type":"object","properties":{"media_id":{"type":"string"},"media_revision":{"type":"string"}},"required":["media_id","media_revision"],"additionalProperties":false});
    let frame = json!({"type":"object","properties":{"video":video,"actual_pts_ms":{"type":"number"},"width":{"type":"integer"},"height":{"type":"integer"}},"required":["video","actual_pts_ms","width","height"],"additionalProperties":false});
    let region = json!({"type":"object","properties":{"frame":frame,"rect_normalized":{"type":"array","items":{"type":"number"},"minItems":4,"maxItems":4}},"required":["frame","rect_normalized"],"additionalProperties":false});
    let transcript = json!({"type":"object","properties":{"transcript_id":{"type":"string"},"transcript_revision":{"type":"string"},"video":video},"required":["transcript_id","transcript_revision","video"],"additionalProperties":false});
    let text_range = json!({"type":"object","properties":{"lid":{"type":"string"},"range":{"type":"object","properties":{"start":{"type":"integer","minimum":0},"end":{"type":"integer","minimum":0}},"required":["start","end"],"additionalProperties":false}},"required":["lid","range"],"additionalProperties":false});
    let text = json!({"type":"object","properties":{"kind":{"enum":["text"]},"publication":p,"transcript":transcript,"text_range":text_range},"required":["kind","publication","transcript","text_range"],"additionalProperties":false});
    let visual = json!({"type":"object","description":"Cite frames or crops successfully observed this turn. Prefer copying the returned source_target. A frame locator with only crop observations resolves to those actual regions; the result returns the effective target. frames and regions must not both be empty. At most 8 frame/region entries in total across this source, including joint members.","properties":{"kind":{"enum":["visual"]},"publication":p,"frames":{"type":"array","description":"Exact read.frame locators retaining actual_pts_ms and original dimensions. If only crops were observed at a locator, the citation uses those crops rather than claiming the whole frame was seen.","maxItems":8,"items":frame},"regions":{"type":"array","description":"Copy returned region objects for observed crops. Coordinates are normalized [x,y,width,height]; the nested frame retains original dimensions and exact actual_pts_ms.","maxItems":8,"items":region}},"required":["kind","publication","frames","regions"],"additionalProperties":false});
    let audio = json!({"type":"object","description":"Copy the complete target returned by video.review_audio in this turn; retain the exact publication, location and review_result_ref.","properties":{"kind":{"enum":["audio"]},"publication":p,"location":crate::video::location_schema(),"review_result_ref":{"type":"string","minLength":1}},"required":["kind","publication","location","review_result_ref"],"additionalProperties":false});
    let leaf = json!({"oneOf":[text,visual,audio]});
    let joint = json!({"type":"object","properties":{"kind":{"enum":["joint"]},"members":{"type":"array","minItems":2,"items":{"type":"object","properties":{"target":leaf,"support_role":{"type":"string","minLength":1}},"required":["target","support_role"],"additionalProperties":false}}},"required":["kind","members"],"additionalProperties":false});
    json!({"type":"object", "properties": {
        "published_book_ref":crate::library::reference_schema(),
        "quote":{"type":"string","minLength":1,"description":"Continuous verbatim text already observed this turn; include enough context to support the claim and uniquely identify the passage. The tool computes LIDs and exact offsets."},
        "target":{"oneOf":[text,visual,audio,joint]},
        "music_target":crate::music::source_target_schema()
    },"oneOf":[{"required":["quote"]},{"required":["target"]},{"required":["music_target"]}],"examples":[{"quote":"The alpha threshold activates only when the value is greater than 10."}],"additionalProperties":false})
}

// serde's internally tagged buffering and arbitrary_precision floats must pass through
// JSON Value before typed media decoding (history contains fractional original PTS).
pub fn deserialize_snapshot<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<MediaSource>, D::Error> {
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    value.map(serde_json::from_value).transpose().map_err(serde::de::Error::custom)
}
