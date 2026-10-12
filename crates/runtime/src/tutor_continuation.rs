//! Contracts shared by Tutor selection, admission and public continuation views.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use ts_rs::TS;

/// Unicode scalar counts; also used by the model-visible schema.
pub const CONTINUATION_TEXT_MAX: usize = 240;
pub const CONTINUATION_ID_MAX: usize = 64;
pub const CONTINUATION_OPTIONS_MAX: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub enum ContinuationMoveKind { Explain, Compare, Observe }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct ContinuationOption {
    pub option_id: String,
    pub label: String,
    pub purpose: String,
    pub move_kind: ContinuationMoveKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct TeachingContinuation {
    pub recommended_option_id: String,
    pub options: Vec<ContinuationOption>,
}

impl TeachingContinuation {
    pub fn validate(&self) -> Result<(), String> {
        let bounded = |s: &str, max| !s.trim().is_empty() && s.chars().count() <= max;
        if self.options.is_empty() || self.options.len() > CONTINUATION_OPTIONS_MAX
            || !bounded(&self.recommended_option_id, CONTINUATION_ID_MAX)
            || !self.options.iter().any(|o| o.option_id == self.recommended_option_id)
        { return Err("建议需要一至两项，且推荐项必须属于集合".into()); }
        let mut ids = std::collections::HashSet::new();
        for option in &self.options {
            if !bounded(&option.option_id, CONTINUATION_ID_MAX) || !ids.insert(&option.option_id)
                || !bounded(&option.label, CONTINUATION_TEXT_MAX)
                || !bounded(&option.purpose, CONTINUATION_TEXT_MAX)
            { return Err("建议标识必须唯一，文案与用途须非空且不超过长度上限".into()); }
        }
        Ok(())
    }

    pub fn schema() -> Value {
        let text = json!({"type":"string","minLength":1,"maxLength":CONTINUATION_TEXT_MAX});
        let mut label = text.clone();
        label["description"] = json!("A concrete request the reader can choose, phrased as the reader's action. Offer a new comparison, changed condition or observation within the parent target, not an explanation or worked example already in move.prompt.");
        let mut purpose = text.clone();
        purpose["description"] = json!("State the specific NEW insight beyond this turn's completed explanation. If this benefit is already delivered by move.prompt, remove the option. Repeating the same calculation with different numbers is not a new insight.");
        let id = json!({"type":"string","minLength":1,"maxLength":CONTINUATION_ID_MAX});
        json!({"type":"object","additionalProperties":false,
            "description":"Optional future help AFTER this answer. Draft the full current explanation first; then prefer one genuinely distinct next step. Omit continuation entirely when no new in-scope insight remains. An alternative is useful only when it offers a different learning choice.",
            "properties":{"recommended_option_id":id,"options":{"type":"array","minItems":1,
                "maxItems":CONTINUATION_OPTIONS_MAX,"items":{"type":"object","additionalProperties":false,
                    "properties":{"option_id":id,"label":label,"purpose":purpose,
                        "move_kind":{"type":"string","enum":serde_json::to_value([
                            ContinuationMoveKind::Explain,ContinuationMoveKind::Compare,ContinuationMoveKind::Observe]).unwrap()}},
                    "required":["option_id","label","purpose","move_kind"]}}},
            "required":["recommended_option_id","options"]})
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub enum EntryAction { ExplainSelection, ClarifyAnswer, ContinueAnswer }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct AnswerRef {
    pub turn_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub excerpt: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct TeachingChoice { pub delivery_ref: String, pub option_id: String }

/// Provenance is intrinsic to the action, rather than inferred from display text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "origin", rename_all = "snake_case", deny_unknown_fields)]
#[ts(tag = "origin", rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub enum InteractionAction {
    Typed,
    UiEntry { entry_action: EntryAction },
    UiChoice { teaching_choice: TeachingChoice, move_kind: ContinuationMoveKind },
}

/// Server-owned snapshot. Never deserialize this from client action metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct ResolvedInteraction {
    pub action: InteractionAction,
    pub user_supplement: String,
    pub accepted_request: String,
    pub answer_ref: Option<AnswerRef>,
    /// Public delivered answer only; excerpts remain assistant-authored content.
    pub answer_text: Option<String>,
    pub source_excerpt: Option<String>,
    pub source_bindings: Vec<memory::teaching::TeachingSourceBinding>,
    pub target: Option<memory::teaching::TeachingTarget>,
    /// Choice uses the offering head; answer actions use the head at click time.
    pub expected_chat_head: Option<String>,
    pub presentation_follow_up: Option<crate::presentation::PresentationFollowUp>,
}

impl ResolvedInteraction {
    pub fn validates_current_action(&self, intent: &str, kind: &str) -> bool {
        match self.action {
            InteractionAction::Typed => false,
            InteractionAction::UiEntry { entry_action: EntryAction::ExplainSelection | EntryAction::ClarifyAnswer } =>
                intent == "direct_explanation" && kind == "explain",
            InteractionAction::UiEntry { entry_action: EntryAction::ContinueAnswer } => intent == "neutral",
            InteractionAction::UiChoice { move_kind, .. } => match move_kind {
                ContinuationMoveKind::Explain => intent == "direct_explanation" && kind == "explain",
                ContinuationMoveKind::Compare => intent == "neutral" && kind == "compare",
                ContinuationMoveKind::Observe => intent == "neutral" && kind == "observe",
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[ts(tag = "kind", rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub enum InteractionResolution {
    RunInput { interaction: ResolvedInteraction },
    ActivityHelp { delivery_refs: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
#[ts(tag = "status", rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub enum ContinuationAvailability {
    Offered,
    Eligible,
    Consumed { accepted_turn_id: String },
    Stale,
    Unavailable,
}

/// Public allowlist: no private assessment, hidden help, tools or authority fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct TutorContinuationView {
    pub delivery_ref: String,
    pub origin_turn_id: String,
    pub target_summary: String,
    pub continuation: TeachingContinuation,
    pub expected_chat_head: String,
    pub availability: ContinuationAvailability,
}
