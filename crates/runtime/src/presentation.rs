//! Content versions are private Reader data. A saved reference is not an answer receipt.
use crate::orchestrator::SourceBinding;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct AnimationCue {
    pub id: String,
    pub label: String,
    #[serde(deserialize_with = "deserialize_cue_seconds")]
    pub at_seconds: f64,
}

fn deserialize_cue_seconds<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
    // AuthorRequest is internally tagged. With arbitrary_precision its buffered
    // JSON number must be decoded through Value before conversion to f64.
    Value::deserialize(deserializer)?.as_f64()
        .ok_or_else(|| serde::de::Error::custom("animation cue at_seconds must be a number"))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct AnimationAsset {
    pub video_base64: String,
    pub poster_png_base64: String,
    pub width: u32,
    pub height: u32,
    pub duration_seconds: f64,
    pub fps: f64,
    pub cues: Vec<AnimationCue>,
}

/// One browser snapshot. Results are page observations, not verified learning evidence.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct PresentationState {
    #[ts(type = "unknown")]
    pub values: Value,
    pub visible_step: Option<String>,
    pub observed_result: String,
    pub source_ref_ids: Vec<String>,
}

/// Receipt returned only after an immutable snapshot has reached private storage.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct PresentationFollowUp {
    pub session_id: String,
    pub turn_id: String,
    pub reference: PresentationRef,
    pub state_revision: u32,
    pub saved_state_ref: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedPresentationState {
    pub receipt: PresentationFollowUp,
    pub owner: PresentationOwner,
    pub state: PresentationState,
}

/// Reader projection deliberately omits internal evidence locations and ownership data.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct PresentationView {
    #[serde(default)]
    pub animation_assets: BTreeMap<String, AnimationAsset>,
    pub restored_state: Option<PresentationState>,
    pub restored_state_revision: Option<u32>,
    pub reference: PresentationRef,
    pub title: String,
    pub content_files: BTreeMap<String, String>,
    pub entrypoint: String,
    pub readable_view: crate::orchestrator::AgentAnswerView,
    pub sources: Vec<crate::orchestrator::AgentAnswerSource>,
    pub assumptions: Vec<String>,
    #[ts(type = "unknown")]
    pub initial_state: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct PresentationRef {
    pub presentation_id: String,
    pub revision: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PresentationOwner {
    pub book_id: String,
    pub session_id: String,
}

/// Logical files, not filesystem paths. HTML may include CSS/JS and data assets.
/// Source bindings come from the existing source compiler, never from page assertions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PresentationContent {
    #[serde(default)]
    pub animation_assets: BTreeMap<String, AnimationAsset>,
    pub title: String,
    pub content_files: BTreeMap<String, String>,
    pub entrypoint: String,
    pub readable_content: String,
    pub source_bindings: Vec<SourceBinding>,
    pub assumptions: Vec<String>,
    pub state_contract: Value,
    pub initial_state: Value,
}

/// Each edit creates a separate candidate; the old candidate and base stay unchanged.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PresentationCandidate {
    pub candidate_id: String,
    pub presentation_id: String,
    pub owner: PresentationOwner,
    pub created_by_turn_id: String,
    pub based_on: Option<PresentationRef>,
    pub content: PresentationContent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentPresentation {
    pub reference: PresentationRef,
    pub candidate_id: String,
    pub owner: PresentationOwner,
    pub created_by_turn_id: String,
    pub based_on: Option<PresentationRef>,
    pub content: PresentationContent,
}
