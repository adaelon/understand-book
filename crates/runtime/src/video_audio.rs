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
        description: "Review a 1–30000 ms clip of original audio from the selected workspace publication (published_book_ref), or this Run's publication when omitted with the configured local Whisper tool. Use a half-open [start_ms,end_ms) range on the original timeline, within video duration. Returns fresh machine-recognized words, original-time estimates, uncertainties, saved transcript for comparison and an audio source target. Use for what the speaker said or unclear terminology. Preserve uncertainties; recognition probabilities are not proof of correctness. Read related frames separately with video.read_frames when needed. The main model receives the review tool result, not audio input. Copy the returned target into source.present to cite it. Does not edit the saved transcript.".into(),
        parameters: json!({"type":"object","properties":{
            "published_book_ref":crate::library::reference_schema(),
            "location":crate::video::location_schema(),
            "question":{"type":"string","minLength":1,"maxLength":2000,"description":"Nonblank listening question of at most 2000 characters. Describe the uncertain wording to compare; this text is recorded as review context and is not supplied to Whisper as a recognition prompt."}
        },"required":["location","question"],"additionalProperties":false,
        "examples":[{"location":{"video":{"media_id":"test-video","media_revision":"1"},"start_ms":0,"end_ms":1000},"question":"Did the speaker say underflow or overflow?"}]}),
    }
}
