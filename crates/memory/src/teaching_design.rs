//! Private, revisable teaching design. A design is neither delivery nor learner evidence.
use crate::teaching::{invalid, TeachingBinding, TeachingSourceBinding, TeachingTarget};
use read_tools::ToolError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema, ts_rs::TS)]
#[serde(deny_unknown_fields)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct TeachingBlock {
    pub block_id: String,
    pub relation: String,
    pub purpose: String,
    pub source_bindings: Vec<TeachingSourceBinding>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema, ts_rs::TS)]
#[serde(deny_unknown_fields)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct TeachingReturnTo {
    #[serde(default)]
    pub block_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invitation_ref: Option<String>,
    /// Teaching event_id carrying the saved page observation, such as a user
    /// usage fact with presentation_follow_up or a displayed/action fact with
    /// scene. Read that fact with tutor.step trace to return to its exact scene.
    /// This is not the receipt's saved_state_ref or a presentation_id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scene_ref: Option<String>,
}

/// The representation chosen for the current connection, before authoring it.
/// Page regions and controls remain in the existing presentation binding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema, ts_rs::TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[ts(tag = "kind", rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub enum TeachingRepresentation {
    Text { reason: String },
    Presentation { reason: String, observation: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema, ts_rs::TS)]
#[serde(deny_unknown_fields)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct TeachingDesign {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub note_refs: Vec<crate::NoteRef>,
    pub target: TeachingTarget,
    pub organizing_model: String,
    pub blocks: Vec<TeachingBlock>,
    #[serde(default)]
    pub connections: Vec<String>,
    pub focus_block_refs: Vec<String>,
    /// Historical designs can omit this; new accepted designs must choose it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub representation: Option<TeachingRepresentation>,
    #[serde(default)]
    pub basis_refs: Vec<String>,
    #[serde(default)]
    pub open_question_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub return_to: Option<TeachingReturnTo>,
    pub revision_reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema, ts_rs::TS)]
#[serde(deny_unknown_fields)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct ResponseExpectation {
    pub public_prompt: String,
    #[serde(default)]
    pub block_refs: Vec<String>,
    /// Begin or continue a Socratic dialogue with this single, plain-text question.
    /// Choose it after supplying the requested support and when the reader can offer a view.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    #[ts(as = "Option<bool>", optional)]
    pub socratic: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub enum InvitationDisposition { Continue, Addressed, Deferred }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema, ts_rs::TS)]
#[serde(deny_unknown_fields)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct TeachingResponseLink {
    #[serde(default)]
    pub fact_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invitation_ref: Option<String>,
    pub disposition: InvitationDisposition,
}

impl TeachingDesign {
    pub fn validate(&self, binding: &TeachingBinding) -> Result<(), ToolError> {
        self.target.validate(binding)?;
        if self.organizing_model.trim().is_empty() || self.revision_reason.trim().is_empty()
            || self.blocks.is_empty() || self.focus_block_refs.is_empty() {
            return Err(invalid("教学设计需要组织关系、关系积木、当前焦点和修订依据"));
        }
        let mut ids = BTreeSet::new();
        for block in &self.blocks {
            if block.block_id.trim().is_empty() || !ids.insert(block.block_id.as_str())
                || block.relation.trim().is_empty() || block.purpose.trim().is_empty() {
                return Err(invalid("关系积木需要唯一身份、关系和用途"));
            }
            let mut target = self.target.clone();
            target.source_bindings = block.source_bindings.clone();
            target.validate(binding)?;
        }
        self.validate_blocks(&self.focus_block_refs)?;
        match &self.representation {
            Some(TeachingRepresentation::Text { reason }) if !reason.trim().is_empty() => {},
            Some(TeachingRepresentation::Presentation { reason, observation })
                if !reason.trim().is_empty() && !observation.trim().is_empty() => {},
            _ => return Err(invalid("教学设计需要选择当前表示方式：文字说明为何足够；演示说明为何需要、要让哪个关系可观察")),
        }
        if let Some(return_to) = &self.return_to { self.validate_blocks(&return_to.block_refs)?; }
        Ok(())
    }

    pub fn validate_blocks(&self, refs: &[String]) -> Result<(), ToolError> {
        if refs.iter().any(|id| !self.blocks.iter().any(|block| &block.block_id == id)) {
            return Err(invalid("关系引用不属于这版教学设计"));
        }
        Ok(())
    }

    /// The current focus is the working scope; the other blocks remain available
    /// for an explicit design revision, not implicit expansion of this move.
    pub fn validate_focus(&self, refs: &[String]) -> Result<(), ToolError> {
        self.validate_blocks(refs)?;
        if refs.iter().any(|id| !self.focus_block_refs.contains(id)) {
            return Err(invalid("本轮关系超出当前教学焦点；需要扩大或改变范围时先修订设计"));
        }
        Ok(())
    }
}
