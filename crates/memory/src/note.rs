//! Structured reading notes. `Record.content` is the user's text only when this
//! envelope is present; legacy and Agent-created bodies retain unknown authorship.
use crate::{NoteBodyPlacement, SaveInput};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct NoteData {
    pub material: NoteMaterial,
    pub association: NoteAssociation,
    pub retained_excerpt: Option<NoteExcerpt>,
    pub source_bindings: Vec<NoteSourceBinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct NoteMaterial {
    pub book_id: String,
    pub publication_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NoteAssociation {
    Selection,
    BodyPlacement {
        placement: NoteBodyPlacement,
    },
    Answer {
        session_id: String,
        turn_id: String,
    },
    Presentation {
        receipt: NotePresentationReceipt,
        title: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct NotePresentationReceipt {
    pub session_id: String,
    pub turn_id: String,
    pub reference: NotePresentationRef,
    pub state_revision: u32,
    pub saved_state_ref: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct NotePresentationRef {
    pub presentation_id: String,
    pub revision: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NoteExcerpt {
    Original { text: String },
    Assistant { text: String },
}
impl NoteExcerpt {
    pub fn text(&self) -> &str {
        match self {
            Self::Original { text } | Self::Assistant { text } => text,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct NoteSourceBinding {
    pub source_ref_id: String,
    pub book_id: String,
    #[ts(
        type = "{ start_lid: string; end_lid: string; ranges?: Array<{ lid: string; range: { start: number; end: number } }> }"
    )]
    pub evidence_range: read_tools::EvidenceRange,
    pub evidence_text_digest: String,
    pub label_snapshot: String,
    pub preview_snapshot: String,
}

pub(crate) fn validate(input: &SaveInput) -> Result<(), read_tools::ToolError> {
    let Some(note) = &input.note else {
        return Ok(());
    };
    let invalid =
        || crate::invalid_note_placement("Invalid structured note content or association".into());
    if input.mem_type != "note"
        || note.material.book_id != input.book_id
        || (input.content.trim().is_empty()
            && note
                .retained_excerpt
                .as_ref()
                .is_none_or(|e| e.text().trim().is_empty()))
        || note
            .retained_excerpt
            .as_ref()
            .is_some_and(|e| e.text().trim().is_empty())
    {
        return Err(invalid());
    }
    match &note.association {
        NoteAssociation::Selection => {
            let selection = input.selection_context.as_ref().ok_or_else(invalid)?;
            if note.retained_excerpt.as_ref().is_some_and(
                |e| !matches!(e, NoteExcerpt::Original { text } if text == &selection.raw_quote),
            ) {
                return Err(invalid());
            }
        }
        NoteAssociation::BodyPlacement { placement } => {
            crate::validate_note_placement(placement)?;
            if input.selection_context.is_some() || note.retained_excerpt.is_some() {
                return Err(invalid());
            }
        }
        NoteAssociation::Answer {
            session_id,
            turn_id,
        } => {
            if session_id.trim().is_empty()
                || turn_id.trim().is_empty()
                || input.selection_context.is_some()
                || matches!(note.retained_excerpt, Some(NoteExcerpt::Original { .. }))
            {
                return Err(invalid());
            }
        }
        NoteAssociation::Presentation { receipt, .. } => {
            if receipt.session_id.trim().is_empty()
                || receipt.turn_id.trim().is_empty()
                || receipt.reference.presentation_id.trim().is_empty()
                || receipt.reference.revision == 0
                || receipt.state_revision == 0
                || receipt.saved_state_ref.trim().is_empty()
                || input.selection_context.is_some()
                || note.retained_excerpt.is_some()
            {
                return Err(invalid());
            }
        }
    }
    if input.selection_context.is_none()
        && input.note_placement.is_none()
        && input.anchor != crate::Anchor::default()
    {
        return Err(invalid());
    }
    Ok(())
}

#[derive(Deserialize, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct NoteCreateRequest {
    pub association: NoteAssociationInput,
    #[serde(default)]
    #[ts(optional)]
    pub retained_excerpt: Option<String>,
}

#[derive(Deserialize, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NoteAssociationInput {
    Selection,
    BodyPlacement,
    Answer { session_id: String, turn_id: String },
    Presentation { receipt: NotePresentationReceipt },
}
