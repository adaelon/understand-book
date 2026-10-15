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

pub fn source_schema() -> serde_json::Value {
    use book_tool_contracts::schema_value;
    use serde_json::json;
    let p = schema_value::<PublishedBookRef>();
    let text = json!({"type":"object","properties":{"kind":{"enum":["text"]},"publication":p,"transcript":schema_value::<base_schema::TranscriptRef>(),"text_range":schema_value::<base_schema::TranscriptTextRange>()},"required":["kind","publication","transcript","text_range"],"additionalProperties":false});
    let visual = json!({"type":"object","properties":{"kind":{"enum":["visual"]},"publication":p,"frames":{"type":"array","maxItems":8,"items":schema_value::<base_schema::FrameLocation>()},"regions":{"type":"array","maxItems":8,"items":schema_value::<base_schema::FrameRegion>()}},"required":["kind","publication","frames","regions"],"additionalProperties":false});
    let audio = json!({"type":"object","properties":{"kind":{"enum":["audio"]},"publication":p,"location":schema_value::<base_schema::MediaLocation>(),"review_result_ref":{"type":"string","minLength":1}},"required":["kind","publication","location","review_result_ref"],"additionalProperties":false});
    let leaf = json!({"oneOf":[text,visual,audio]});
    let joint = json!({"type":"object","properties":{"kind":{"enum":["joint"]},"members":{"type":"array","minItems":2,"items":{"type":"object","properties":{"target":leaf,"support_role":{"type":"string","minLength":1}},"required":["target","support_role"],"additionalProperties":false}}},"required":["kind","members"],"additionalProperties":false});
    json!({"type":"object", "properties": {
        "quote":{"type":"string","minLength":1,"description":"Continuous verbatim text observed this turn."},
        "target":{"oneOf":[text,visual,audio,joint]}
    },"oneOf":[{"required":["quote"]},{"required":["target"]}],"additionalProperties":false})
}

// serde's internally tagged buffering and arbitrary_precision floats must pass through
// JSON Value before typed media decoding (history contains fractional original PTS).
pub fn deserialize_snapshot<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<MediaSource>, D::Error> {
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    value.map(serde_json::from_value).transpose().map_err(serde::de::Error::custom)
}
