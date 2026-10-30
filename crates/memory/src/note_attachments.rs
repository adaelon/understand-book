//! Display references are independent of a thought's immutable expression and origin.
use crate::{MemoryStore, NotePresentationReceipt, NoteRef, PersonalNoteView};
use read_tools::ToolError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ts_rs::TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub enum NoteAttachmentTarget {
    Message {
        session_id: String,
        turn_id: String,
        role: NoteMessageRole,
        /// Rendered text chosen by the user, with its occurrence in this message.
        quote: Option<String>,
        #[serde(default)]
        occurrence: u32,
    },
    Presentation {
        receipt: NotePresentationReceipt,
        visible_step: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub enum NoteMessageRole {
    User,
    Assistant,
}

impl NoteAttachmentTarget {
    pub fn session_id(&self) -> &str {
        match self {
            Self::Message { session_id, .. } => session_id,
            Self::Presentation { receipt, .. } => &receipt.session_id,
        }
    }
    pub fn same_location(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Presentation {
                    receipt: a,
                    visible_step: sa,
                },
                Self::Presentation {
                    receipt: b,
                    visible_step: sb,
                },
            ) => {
                a.session_id == b.session_id
                    && a.turn_id == b.turn_id
                    && a.reference == b.reference
                    && if sa.is_some() || sb.is_some() {
                        sa == sb
                    } else {
                        a == b
                    }
            }
            _ => self == other,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct NoteAttachment {
    pub attachment_id: String,
    pub note_id: String,
    pub target: NoteAttachmentTarget,
    pub label: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct NoteAttachmentView {
    pub attachment: NoteAttachment,
    pub note: PersonalNoteView,
}

impl MemoryStore {
    pub fn note_attachments(
        &self,
        note_id: Option<&str>,
        session_id: Option<&str>,
    ) -> Vec<NoteAttachmentView> {
        self.document
            .note_attachments
            .iter()
            .filter(|a| {
                note_id.is_none_or(|id| a.note_id == id)
                    && session_id.is_none_or(|id| a.target.session_id() == id)
            })
            .filter_map(|a| {
                self.personal_note(&NoteRef {
                    note_id: a.note_id.clone(),
                    revision: None,
                })
                .ok()
                .map(|note| NoteAttachmentView {
                    attachment: a.clone(),
                    note,
                })
            })
            .collect()
    }

    pub fn attach_personal_note(
        &mut self,
        note_id: &str,
        target: NoteAttachmentTarget,
        label: String,
        now: &str,
    ) -> Result<NoteAttachment, ToolError> {
        self.personal_note(&NoteRef {
            note_id: note_id.into(),
            revision: None,
        })?;
        if let Some(existing) = self
            .document
            .note_attachments
            .iter()
            .find(|a| a.note_id == note_id && a.target.same_location(&target))
        {
            return Ok(existing.clone());
        }
        let attachment = NoteAttachment {
            attachment_id: format!("attachment-{}", uuid::Uuid::now_v7()),
            note_id: note_id.into(),
            target,
            label,
            created_at: now.into(),
        };
        let mut candidate = self.projection_mutation_candidate()?;
        candidate.note_attachments.push(attachment.clone());
        self.commit_document(candidate)?;
        Ok(attachment)
    }

    pub fn remove_note_attachment(&mut self, attachment_id: &str) -> Result<(), ToolError> {
        if !self
            .document
            .note_attachments
            .iter()
            .any(|a| a.attachment_id == attachment_id)
        {
            return Ok(());
        }
        let mut candidate = self.projection_mutation_candidate()?;
        candidate
            .note_attachments
            .retain(|a| a.attachment_id != attachment_id);
        self.commit_document(candidate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NoteContent, MEMORY_SCHEMA_VERSION};
    fn content(text: &str) -> NoteContent {
        serde_json::from_value(serde_json::json!({"content":text,"authorship":"user","context":{"label":"最初的阅读位置","book_id":"original"}})).unwrap()
    }
    fn target() -> NoteAttachmentTarget {
        NoteAttachmentTarget::Message {
            session_id: "session-a".into(),
            turn_id: "turn-a".into(),
            role: NoteMessageRole::Assistant,
            quote: Some("当时的一句话".into()),
            occurrence: 0,
        }
    }
    #[test]
    fn note_attachments_reopen_follow_edits_and_remove_without_changing_origin() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("memory.json");
        let mut store = MemoryStore::open(&path).unwrap();
        let note = store
            .create_personal_note(content("第一种理解"), "create", "1")
            .unwrap();
        let a = store
            .attach_personal_note(&note.note_id, target(), "原句下".into(), "2")
            .unwrap();
        assert_eq!(
            store
                .attach_personal_note(&note.note_id, target(), "重试".into(), "3")
                .unwrap(),
            a
        );
        drop(store);
        let mut store = MemoryStore::open(&path).unwrap();
        assert_eq!(store.note_attachments(None, Some("session-a")).len(), 1);
        assert!(store
            .note_attachments(None, Some("other-session"))
            .is_empty());
        let updated = store
            .revise_personal_note(&note.note_id, 1, content("补上成立的条件"), "edit", "4")
            .unwrap();
        assert_eq!(
            store.note_attachments(Some(&note.note_id), None)[0].note,
            updated
        );
        store.remove_note_attachment(&a.attachment_id).unwrap();
        store.remove_note_attachment(&a.attachment_id).unwrap();
        assert_eq!(
            store
                .personal_note(&NoteRef {
                    note_id: note.note_id,
                    revision: None
                })
                .unwrap(),
            updated
        );
        assert_eq!(
            updated.value.context.label.as_deref(),
            Some("最初的阅读位置")
        );
    }
    #[test]
    fn note_attachments_delete_and_upgrade_v5() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("memory.json");
        let mut store = MemoryStore::open(&path).unwrap();
        let note = store
            .create_personal_note(content("保留下来的想法"), "create", "1")
            .unwrap();
        drop(store);
        let mut old: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        old["schema_version"] = serde_json::json!(5);
        old.as_object_mut().unwrap().remove("note_attachments");
        std::fs::write(&path, serde_json::to_vec(&old).unwrap()).unwrap();
        let mut store = MemoryStore::open(&path).unwrap();
        assert_eq!(store.document.schema_version, MEMORY_SCHEMA_VERSION);
        store
            .attach_personal_note(&note.note_id, target(), "原句下".into(), "2")
            .unwrap();
        store.delete(&note.note_id).unwrap();
        assert!(MemoryStore::open(&path)
            .unwrap()
            .document
            .note_attachments
            .is_empty());
    }
}
