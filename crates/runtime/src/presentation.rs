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

/// Teaching semantics projected from the page's current calculation state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct PresentationObservation {
    pub summary: String,
    #[ts(type = "Record<string, unknown>")]
    pub state: BTreeMap<String, Value>,
    /// Only for pages whose displayed result can lag behind the current inputs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "Record<string, unknown>")]
    pub rendered: Option<BTreeMap<String, Value>>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub observation: Option<PresentationObservation>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub enum PresentationFeedbackChoice {
    Like,
    Dislike,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub enum PresentationFeedbackReason {
    EasierToUnderstand,
    AptExample,
    UsefulInteraction,
    Unclear,
    IncorrectContent,
    HardToRead,
    HardToOperate,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(deny_unknown_fields)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct PresentationFeedbackSelection {
    pub choice: PresentationFeedbackChoice,
    #[serde(default)]
    pub reasons: Vec<PresentationFeedbackReason>,
    #[serde(default)]
    pub comment: String,
}

/// Current private preference for one content version, not a learning judgment.
/// A cleared record retains its revision so outstanding snapshots stay invalid.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct PresentationFeedback {
    pub session_id: String,
    pub turn_id: String,
    pub reference: PresentationRef,
    pub feedback_revision: u32,
    pub selection: Option<PresentationFeedbackSelection>,
    pub saved_state: Option<PresentationFollowUp>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

/// Reader projection deliberately omits internal evidence locations and ownership data.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct PresentationView {
    #[serde(default)]
    pub action_contract: Vec<crate::presentation_interact::PresentationActionContract>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub teaching_binding: Option<PresentationTeachingBinding>,
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

/// The concrete relationship made observable by this exact page version.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema, TS)]
#[serde(deny_unknown_fields)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct PresentationTeachingBinding {
    pub design_ref: String,
    pub block_refs: Vec<String>,
    #[serde(default)]
    pub regions: Vec<PresentationTeachingRegion>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema, TS)]
#[serde(deny_unknown_fields)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct PresentationTeachingRegion {
    pub region_id: String,
    pub block_refs: Vec<String>,
    #[serde(default)]
    pub state_fields: Vec<String>,
    pub observation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PresentationOwner {
    pub book_id: String,
    pub session_id: String,
}

/// Author-declared use, separate from catalog exposure and source observation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PresentationMethodRef {
    pub method_id: String,
    pub revision: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PresentationMethodBasis {
    Read { turn_id: String, tool_call_ids: Vec<String> },
    Candidate { candidate_id: String },
    Version { reference: PresentationRef },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PresentationMethodUse {
    #[serde(flatten)]
    pub method: PresentationMethodRef,
    pub basis: PresentationMethodBasis,
}

/// Exact source range included in a successful Provider sampling, after projection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PresentationMethodRead {
    pub tool_call_id: String,
    pub method_id: String,
    pub revision: u64,
    pub file: String,
    pub offset: usize,
    pub characters: usize,
    pub end_offset: usize,
    pub total_characters: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct PresentationMethodObservations {
    /// The compiled catalog exposed in this run, including an empty catalog.
    pub catalog: Option<Value>,
    pub reads: Vec<PresentationMethodRead>,
}

/// Logical files, not filesystem paths. HTML may include CSS/JS and data assets.
/// Source bindings come from the existing source compiler, never from page assertions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PresentationContent {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub action_contract: Vec<crate::presentation_interact::PresentationActionContract>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub teaching_binding: Option<PresentationTeachingBinding>,
    /// None means historical use was not recorded; Some([]) declares no methods.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method_refs: Option<Vec<PresentationMethodUse>>,
    #[serde(default)]
    pub method_observations: PresentationMethodObservations,
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
