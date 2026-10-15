//! Short source-audio review results. Recognition is evidence from a tool, not a main-model listen.
use base_schema::{EvidenceTarget, MediaLocation, MediaObservation, PublishedBookRef, TranscriptSegment};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewAudio {
    pub location: MediaLocation,
    pub question: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReviewedWord {
    pub text: String,
    pub start_ms: f64,
    pub end_ms: f64,
    pub probability: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioReview {
    pub run_id: String,
    pub review_result_ref: String,
    pub publication: PublishedBookRef,
    pub location: MediaLocation,
    pub question: String,
    pub reviewed_text: String,
    pub reviewed_words: Vec<ReviewedWord>,
    pub uncertainties: Vec<String>,
    pub provider: Value,
    pub transcript: Vec<TranscriptSegment>,
}

impl AudioReview {
    pub fn target(&self) -> EvidenceTarget {
        EvidenceTarget::Audio { publication: self.publication.clone(), location: self.location.clone(), review_result_ref: self.review_result_ref.clone() }
    }

    pub fn observation(&self, run_id: &str, tool_call_id: &str) -> MediaObservation {
        MediaObservation::Audio { observation_id: uuid::Uuid::now_v7().to_string(), run_id: run_id.into(),
            publication: self.publication.clone(), tool_call_id: tool_call_id.into(), location: self.location.clone(), review_result_ref: self.review_result_ref.clone() }
    }
}

pub fn spec() -> crate::ToolSpec {
    crate::ToolSpec {
        name: "video.review_audio".into(),
        description: "Review 0–30 seconds of original audio from this Run's fixed publication with the configured local Whisper tool. Returns fresh machine-recognized words, original-time estimates, uncertainties, saved transcript for comparison and an audio source target. Use for what the speaker said, underflow/overflow or unclear terminology. Preserve uncertainties; recognition probabilities are not proof of correctness. Read related frames separately with video.read_frames when needed, and distinguish visible code from spoken wording. The main model receives the review tool result, not audio input. source.present can cite its exact target. Does not edit the saved transcript.".into(),
        parameters: json!({"type":"object","properties":{
            "location":crate::video::spec().parameters["properties"]["location"],
            "question":{"type":"string","minLength":1,"maxLength":2000}
        },"required":["location","question"],"additionalProperties":false}),
    }
}
