//! Personal thoughts: one identity, immutable saved expressions, independent of a Book.
use crate::{Anchor, MemoryDocument, MemoryStore, NoteBodyPlacement, NoteData, Record, Usage};
use read_tools::ToolError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct NoteRef {
    pub note_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub revision: Option<u32>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub enum NoteAuthorship {
    User,
    AssistantDraft,
    AdoptedAssistant,
    MaterialExcerpt,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct NoteContext {
    #[serde(default)]
    pub book_id: Option<String>,
    #[serde(default)]
    pub publication_id: Option<String>,
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub lid: Option<String>,
    #[serde(default)]
    pub video_time_ms: Option<u32>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub turn_id: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub origin: Option<crate::NoteAttachmentTarget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub board_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub board_item_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct NoteRevision {
    pub revision: u32,
    pub saved_at: String,
    pub title: Option<String>,
    pub authorship: NoteAuthorship,
    /// Original expression and source receipt. Display locations are projected separately.
    pub record: Record,
    pub context: NoteContext,
    #[serde(default)]
    pub source_refs: Vec<NoteRef>,
    #[serde(default)]
    pub change_note: Option<String>,
    #[serde(default)]
    pub based_on_turn: Option<NoteTurnRef>,
    #[serde(default)]
    pub restored_from: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct NoteTurnRef {
    pub session_id: String,
    pub turn_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PersonalNote {
    pub note_id: String,
    pub created_at: String,
    pub current_revision: u32,
    pub revisions: Vec<NoteRevision>,
    pub location: NoteLocation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NoteLocation {
    pub layer: String,
    pub anchor: Anchor,
    pub placement: Option<NoteBodyPlacement>,
    pub detached: bool,
    pub usage: Usage,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct PersonalNoteView {
    pub note_id: String,
    pub created_at: String,
    pub current_revision: u32,
    pub value: NoteRevision,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct NoteContent {
    pub content: String,
    #[serde(default)]
    pub title: Option<String>,
    pub authorship: NoteAuthorship,
    #[serde(default)]
    pub context: NoteContext,
    #[serde(default)]
    pub source_refs: Vec<NoteRef>,
    #[serde(default)]
    pub change_note: Option<String>,
    #[serde(default)]
    pub based_on_turn: Option<NoteTurnRef>,
    /// Resolved by the material endpoint; personal text needs no material receipt.
    #[serde(default)]
    pub source: Option<NoteData>,
    #[serde(default)]
    pub capture: Option<NoteCapture>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct NoteCapture {
    pub anchor: Anchor,
    pub selection_context: Option<crate::SelectionContext>,
    pub placement: Option<NoteBodyPlacement>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct NoteOperation {
    pub request: Option<serde_json::Value>,
    pub reference: NoteRef,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct NoteLink {
    pub link_id: String,
    pub from: NoteRef,
    pub to: NoteRef,
    pub reason: Option<String>,
    /// Versions read for the current explanation; creation endpoints remain stable.
    #[serde(default)]
    pub reason_refs: Vec<NoteRef>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct NoteDiscussion {
    pub reference: NoteRef,
    pub turn: NoteTurnRef,
    pub created_at: String,
    /// Only a user expression explicitly about this note; recall alone is not expression.
    pub expression: Option<String>,
}

pub(crate) fn note_error(code: &str, category: &str, message: impl Into<String>) -> ToolError {
    ToolError {
        error_code: code.into(),
        category: category.into(),
        message: message.into(),
    }
}

impl PersonalNote {
    pub(crate) fn from_record(record: Record, authorship: NoteAuthorship) -> Self {
        let context = NoteContext {
            book_id: (!record.book_id.is_empty()).then(|| record.book_id.clone()),
            publication_id: record
                .note
                .as_ref()
                .and_then(|n| n.material.publication_id.clone()),
            lid: record.anchor.lid.clone(),
            session_id: record.source_session_id.clone(),
            ..Default::default()
        };
        Self {
            note_id: record.mem_id.clone(),
            created_at: record.generated_at.clone(),
            current_revision: 1,
            location: NoteLocation {
                layer: record.layer.clone(),
                anchor: record.anchor.clone(),
                placement: record.note_placement.clone(),
                detached: false,
                usage: record.usage.clone(),
            },
            revisions: vec![NoteRevision {
                revision: 1,
                saved_at: record.generated_at.clone(),
                title: None,
                authorship,
                record,
                context,
                source_refs: vec![],
                change_note: None,
                based_on_turn: None,
                restored_from: None,
            }],
        }
    }

    pub fn current(&self) -> &NoteRevision {
        &self.revisions[self.current_revision as usize - 1]
    }

    pub fn as_record(&self) -> Record {
        let mut record = self.current().record.clone();
        record.layer = self.location.layer.clone();
        record.usage = self.location.usage.clone();
        record.anchor = self.location.anchor.clone();
        record.note_placement = self.location.placement.clone();
        if self.location.detached {
            record.anchor = Anchor::default();
            record.note_placement = None;
        }
        if let Some(placement) = &record.note_placement {
            record.citations = vec![crate::MemCitation {
                lid: placement.lid().into(),
                book_id: record.book_id.clone(),
                note: None,
            }];
        }
        record
    }

    fn view(&self, revision: Option<u32>) -> Result<PersonalNoteView, ToolError> {
        let revision = revision.unwrap_or(self.current_revision);
        let value = revision
            .checked_sub(1)
            .and_then(|i| self.revisions.get(i as usize))
            .ok_or_else(|| note_error("NOTE_REVISION_NOT_FOUND", "not_found", "此笔记版本不存在"))?
            .clone();
        Ok(PersonalNoteView {
            note_id: self.note_id.clone(),
            created_at: self.created_at.clone(),
            current_revision: self.current_revision,
            value,
        })
    }

    pub(crate) fn append_record(&mut self, record: Record, now: &str) {
        self.location.usage = record.usage.clone();
        if self.current().record.content == record.content
            && self.current().record.note == record.note
            && self.current().record.selection_context == record.selection_context
        {
            return;
        }
        let mut value = self.current().clone();
        value.revision += 1;
        value.saved_at = now.into();
        value.record = record;
        value.restored_from = None;
        value.change_note = None;
        value.based_on_turn = None;
        self.current_revision = value.revision;
        self.revisions.push(value);
    }
}

impl MemoryDocument {
    pub(crate) fn migrate_notes(&mut self) {
        let records = std::mem::take(&mut self.records);
        for record in records {
            if record.mem_type == "note" {
                // Earlier stores did not record who wrote a body. Do not invent authorship.
                self.notes
                    .push(PersonalNote::from_record(record, NoteAuthorship::Unknown));
            } else {
                self.records.push(record);
            }
        }
    }

    pub(crate) fn validate_notes(&self) -> Result<(), String> {
        let mut identities = std::collections::BTreeSet::new();
        for note in &self.notes {
            if !identities.insert(&note.note_id)
                || note.revisions.is_empty()
                || note.current_revision as usize != note.revisions.len()
            {
                return Err("笔记身份或修订序列不完整".into());
            }
            for (index, revision) in note.revisions.iter().enumerate() {
                if revision.revision as usize != index + 1
                    || revision.record.mem_id != note.note_id
                    || revision.record.mem_type != "note"
                {
                    return Err("笔记修订身份不一致".into());
                }
            }
        }
        if self.records.iter().any(|r| r.mem_type == "note") {
            return Err("笔记正文必须存储在 notes 中".into());
        }
        Ok(())
    }
}

impl MemoryStore {
    pub fn read_personal_note_page(
        &self,
        input: &serde_json::Value,
    ) -> Result<serde_json::Value, ToolError> {
        use serde_json::json;
        let reference: NoteRef = serde_json::from_value(input.clone()).map_err(|_| {
            note_error(
                "INVALID_NOTE_REF",
                "validation",
                "需要笔记身份和可选确切版本",
            )
        })?;
        let value = self.personal_note(&reference)?;
        let exact = NoteRef {
            note_id: value.note_id.clone(),
            revision: Some(value.value.revision),
        };
        let section = input["section"].as_str().unwrap_or("body");
        let start = input["start_char"].as_u64().unwrap_or(0) as usize;
        let limit = input["limit_chars"].as_u64().unwrap_or(3000).clamp(1, 4000) as usize;
        let discussions = self.note_discussions(&reference.note_id);
        let note_links = self.note_links(&reference.note_id);
        let source_text = value
            .value
            .record
            .note
            .as_ref()
            .and_then(|s| s.retained_excerpt.as_ref())
            .map(|e| e.text())
            .unwrap_or_default();
        let text = match section {
            "body" => value.value.record.content.as_str(),
            "source" => source_text,
            "expression" => discussions
                .iter()
                .find(|d| Some(d.turn.turn_id.as_str()) == input["turn_id"].as_str())
                .and_then(|d| d.expression.as_deref())
                .ok_or_else(|| {
                    note_error("NOTE_EXPRESSION_NOT_FOUND", "not_found", "相关表述不存在")
                })?,
            "link" => note_links
                .iter()
                .find(|link| Some(link.link_id.as_str()) == input["link_id"].as_str())
                .and_then(|link| link.reason.as_deref())
                .ok_or_else(|| note_error("NOTE_LINK_NOT_FOUND", "not_found", "联系说明不存在"))?,
            "history" => {
                let revisions = self.personal_note_history(
                    &reference.note_id,
                    input["before_revision"]
                        .as_u64()
                        .and_then(|r| r.try_into().ok()),
                    10,
                )?;
                return Ok(
                    json!({"reference":exact,"revisions":revisions.iter().map(|r|json!({"revision":r.revision,"saved_at":r.saved_at,"authorship":r.authorship,"restored_from":r.restored_from})).collect::<Vec<_>>(),"next_before_revision":revisions.last().filter(|r|r.revision>1).map(|r|r.revision)}),
                );
            }
            _ => {
                return Err(note_error(
                    "INVALID_NOTE_SECTION",
                    "validation",
                    "section 必须是 body、source、expression、link 或 history",
                ))
            }
        };
        let length = text.chars().count();
        if start > length {
            return Err(note_error(
                "NOTE_RANGE_INVALID",
                "validation",
                "读取范围超出长度",
            ));
        }
        let end = start.saturating_add(limit).min(length);
        let expressions:Vec<_>=discussions.iter().rev().filter(|d|d.expression.is_some()).skip(input["expressions_after"].as_u64().unwrap_or(0) as usize).take(4).map(|d|json!({"reference":d.reference,"turn":d.turn,"created_at":d.created_at,
            "expression":d.expression.as_ref().unwrap().chars().take(300).collect::<String>(),"truncated":d.expression.as_ref().unwrap().chars().count()>300,
            "read":{"note_id":exact.note_id,"revision":exact.revision,"section":"expression","turn_id":d.turn.turn_id}})).collect();
        let links:Vec<_>=note_links.iter().skip(input["links_after"].as_u64().unwrap_or(0) as usize).take(8).map(|link|json!({"link_id":link.link_id,"from":link.from,"to":link.to,"reason_refs":link.reason_refs,"reason":link.reason.as_ref().map(|reason|reason.chars().take(300).collect::<String>()),"truncated":link.reason.as_ref().is_some_and(|reason|reason.chars().count()>300),"read":{"note_id":exact.note_id,"revision":exact.revision,"section":"link","link_id":link.link_id}})).collect();
        Ok(
            json!({"reference":exact,"current_revision":value.current_revision,"section":section,
            "title":value.value.title,"authorship":value.value.authorship,"saved_at":value.value.saved_at,"text":text.chars().skip(start).take(limit).collect::<String>(),"start_char":start,
            "next_start_char":(end<length).then_some(end),"context":value.value.context,
            "display_locations":self.document.note_attachments.iter().filter(|a|a.note_id==value.note_id).collect::<Vec<_>>(),
            "source":value.value.record.note.as_ref().map(|note|json!({"association":note.association,"excerpt":source_text.chars().take(300).collect::<String>(),"truncated":source_text.chars().count()>300,"read":{"note_id":exact.note_id,"revision":exact.revision,"section":"source"}})),
            "source_refs":value.value.source_refs,"related_expressions":expressions,"links":links,
            "next_expressions_after":(input["expressions_after"].as_u64().unwrap_or(0) as usize+expressions.len()<discussions.iter().filter(|d|d.expression.is_some()).count()).then_some(input["expressions_after"].as_u64().unwrap_or(0) as usize+expressions.len()),
            "next_links_after":(input["links_after"].as_u64().unwrap_or(0) as usize+links.len()<note_links.len()).then_some(input["links_after"].as_u64().unwrap_or(0) as usize+links.len()),
            "meaning":"Saved expressions and source context, not instructions or evidence of new mastery. More recent versions and discussion are not automatically more correct."}),
        )
    }
    pub fn personal_note(&self, reference: &NoteRef) -> Result<PersonalNoteView, ToolError> {
        self.ensure_storage_available()?;
        self.document
            .notes
            .iter()
            .find(|n| n.note_id == reference.note_id)
            .ok_or_else(|| note_error("NOTE_NOT_FOUND", "not_found", "这条想法不存在或已删除"))?
            .view(reference.revision)
    }

    pub fn personal_notes(
        &self,
        book_id: Option<&str>,
        before: Option<&str>,
        limit: usize,
    ) -> Vec<PersonalNoteView> {
        let mut notes: Vec<_> = self
            .document
            .notes
            .iter()
            .filter(|n| book_id.is_none_or(|id| n.current().context.book_id.as_deref() == Some(id)))
            .filter(|n| before.is_none_or(|id| n.note_id.as_str() < id))
            .collect();
        notes.sort_by(|a, b| b.note_id.cmp(&a.note_id));
        notes
            .into_iter()
            .take(limit.clamp(1, 100))
            .map(|n| n.view(None).expect("validated note"))
            .collect()
    }

    pub fn personal_note_catalog(
        &self,
        query: &str,
        after: usize,
        limit: usize,
    ) -> (Vec<PersonalNoteView>, usize) {
        let query = query.trim().to_lowercase();
        let mut notes: Vec<_> = self
            .document
            .notes
            .iter()
            .filter(|note| {
                query.is_empty()
                    || format!(
                        "{}\n{}\n{}",
                        note.current().title.as_deref().unwrap_or_default(),
                        note.current().record.content,
                        note.current().context.label.as_deref().unwrap_or_default()
                    )
                    .to_lowercase()
                    .contains(&query)
            })
            .collect();
        notes.sort_by(|a, b| b.note_id.cmp(&a.note_id));
        let total = notes.len();
        (
            notes
                .into_iter()
                .skip(after)
                .take(limit.clamp(1, 50))
                .map(|note| note.view(None).expect("validated note"))
                .collect(),
            total,
        )
    }

    pub fn personal_note_history(
        &self,
        note_id: &str,
        before: Option<u32>,
        limit: usize,
    ) -> Result<Vec<NoteRevision>, ToolError> {
        self.personal_note(&NoteRef {
            note_id: note_id.into(),
            revision: None,
        })?;
        let note = self
            .document
            .notes
            .iter()
            .find(|n| n.note_id == note_id)
            .expect("read above");
        Ok(note
            .revisions
            .iter()
            .rev()
            .filter(|v| before.is_none_or(|b| v.revision < b))
            .take(limit.clamp(1, 50))
            .cloned()
            .collect())
    }

    fn note_operation_replay(
        &self,
        operation_id: &str,
        request: &serde_json::Value,
    ) -> Result<Option<PersonalNoteView>, ToolError> {
        if operation_id.trim().is_empty() {
            return Err(note_error(
                "NOTE_OPERATION_REQUIRED",
                "validation",
                "缺少保存操作身份",
            ));
        }
        if let Some(operation) = self.document.note_operations.get(operation_id) {
            let Some(saved_request) = &operation.request else {
                return Err(note_error(
                    "NOTE_NOT_FOUND",
                    "not_found",
                    "这次保存的想法已删除",
                ));
            };
            if saved_request != request {
                return Err(note_error(
                    "NOTE_OPERATION_CONFLICT",
                    "conflict",
                    "此操作已用于另一份内容",
                ));
            }
            return self.personal_note(&operation.reference).map(Some);
        }
        Ok(None)
    }

    pub fn create_personal_note(
        &mut self,
        content: NoteContent,
        operation_id: &str,
        now: &str,
    ) -> Result<PersonalNoteView, ToolError> {
        let request = serde_json::json!({"create": content});
        if let Some(view) = self.note_operation_replay(operation_id, &request)? {
            return Ok(view);
        }
        self.validate_note_content(&content, &[])?;
        let note_id = format!("note-{}", uuid::Uuid::now_v7());
        let capture = content.capture.clone().unwrap_or_default();
        let book_id = content.context.book_id.clone().unwrap_or_default();
        let citations = if let Some(selection) = &capture.selection_context {
            crate::selection_citations(selection, &book_id)
        } else {
            capture
                .anchor
                .lid
                .as_ref()
                .map(|lid| {
                    vec![crate::MemCitation {
                        lid: lid.clone(),
                        book_id: book_id.clone(),
                        note: None,
                    }]
                })
                .unwrap_or_default()
        };
        let record = Record {
            mem_id: note_id.clone(),
            mem_type: "note".into(),
            layer: "long_term".into(),
            book_id,
            anchor: capture.anchor,
            content: content.content.clone(),
            range: None,
            selection_context: capture.selection_context,
            note_placement: capture.placement,
            note: content.source.clone(),
            citations,
            usage: Usage::default(),
            generated_at: now.into(),
            source_session_id: content.context.session_id.clone(),
        };
        let mut note = PersonalNote::from_record(record, content.authorship);
        let revision = &mut note.revisions[0];
        revision.title = content.title;
        revision.context = content.context;
        revision.source_refs = content.source_refs;
        revision.change_note = content.change_note;
        revision.based_on_turn = content.based_on_turn;
        let view = note.view(None)?;
        let mut candidate = self.projection_mutation_candidate()?;
        candidate.notes.push(note);
        candidate.note_operations.insert(
            operation_id.into(),
            NoteOperation {
                request: Some(request),
                reference: NoteRef {
                    note_id,
                    revision: Some(1),
                },
            },
        );
        self.commit_document(candidate)?;
        Ok(view)
    }

    fn validate_note_content(
        &self,
        content: &NoteContent,
        retained_refs: &[NoteRef],
    ) -> Result<(), ToolError> {
        if content.content.trim().is_empty()
            && content
                .source
                .as_ref()
                .and_then(|s| s.retained_excerpt.as_ref())
                .is_none()
        {
            return Err(note_error(
                "NOTE_EMPTY",
                "validation",
                "写下想法或保留一段素材后再保存",
            ));
        }
        for reference in &content.source_refs {
            if reference.revision.is_none() {
                return Err(note_error(
                    "NOTE_SOURCE_REVISION_REQUIRED",
                    "validation",
                    "引用需指明保存时的版本",
                ));
            }
            if !retained_refs.contains(reference) {
                self.personal_note(reference)?;
            }
        }
        let capture = content.capture.clone().unwrap_or_default();
        crate::validate_note_source(
            &crate::SaveInput {
                mem_id: None,
                mem_type: "note".into(),
                layer: "long_term".into(),
                book_id: content.context.book_id.clone().unwrap_or_default(),
                anchor: capture.anchor,
                content: content.content.clone(),
                range: None,
                selection_context: capture.selection_context,
                note_placement: capture.placement,
                note: content.source.clone(),
                citations: None,
                source_session_id: content.context.session_id.clone(),
            },
            false,
        )?;
        Ok(())
    }

    pub fn revise_personal_note(
        &mut self,
        note_id: &str,
        base_revision: u32,
        content: NoteContent,
        operation_id: &str,
        now: &str,
    ) -> Result<PersonalNoteView, ToolError> {
        let request =
            serde_json::json!({"revise":note_id,"base_revision":base_revision,"content":content});
        if let Some(view) = self.note_operation_replay(operation_id, &request)? {
            return Ok(view);
        }
        let current = self.personal_note(&NoteRef {
            note_id: note_id.into(),
            revision: None,
        })?;
        self.validate_note_content(&content, &current.value.source_refs)?;
        check_base(&current, base_revision)?;
        let mut value = current.value.clone();
        value.record.content = content.content;
        value.record.note = content.source;
        value.record.book_id = content.context.book_id.clone().unwrap_or_default();
        value.record.source_session_id = content.context.session_id.clone();
        if let Some(capture) = content.capture {
            value.record.anchor = capture.anchor;
            value.record.selection_context = capture.selection_context;
            value.record.note_placement = capture.placement;
            if let Some(selection) = &value.record.selection_context {
                value.record.citations =
                    crate::selection_citations(selection, &value.record.book_id);
            }
        }
        value.title = content.title;
        value.authorship = content.authorship;
        value.context = content.context;
        value.source_refs = content.source_refs;
        value.change_note = content.change_note;
        value.based_on_turn = content.based_on_turn;
        let changed = value.record != current.value.record
            || value.title != current.value.title
            || value.authorship != current.value.authorship
            || value.context != current.value.context
            || value.source_refs != current.value.source_refs
            || value.based_on_turn != current.value.based_on_turn;
        let mut candidate = self.projection_mutation_candidate()?;
        let note = candidate
            .notes
            .iter_mut()
            .find(|n| n.note_id == note_id)
            .expect("read above");
        if changed {
            value.revision += 1;
            value.saved_at = now.into();
            value.restored_from = None;
            note.current_revision = value.revision;
            note.revisions.push(value);
        }
        let view = note.view(None)?;
        candidate.note_operations.insert(
            operation_id.into(),
            NoteOperation {
                request: Some(request),
                reference: NoteRef {
                    note_id: note_id.into(),
                    revision: Some(view.value.revision),
                },
            },
        );
        self.commit_document(candidate)?;
        Ok(view)
    }

    pub fn restore_personal_note(
        &mut self,
        note_id: &str,
        base_revision: u32,
        revision: u32,
        operation_id: &str,
        now: &str,
    ) -> Result<PersonalNoteView, ToolError> {
        let request = serde_json::json!({"restore":note_id,"base_revision":base_revision,"revision":revision});
        if let Some(view) = self.note_operation_replay(operation_id, &request)? {
            return Ok(view);
        }
        let current = self.personal_note(&NoteRef {
            note_id: note_id.into(),
            revision: None,
        })?;
        check_base(&current, base_revision)?;
        let mut value = self
            .personal_note(&NoteRef {
                note_id: note_id.into(),
                revision: Some(revision),
            })?
            .value;
        value.revision = base_revision + 1;
        value.saved_at = now.into();
        value.restored_from = Some(revision);
        value.change_note = None;
        value.based_on_turn = None;
        let mut candidate = self.projection_mutation_candidate()?;
        let note = candidate
            .notes
            .iter_mut()
            .find(|n| n.note_id == note_id)
            .expect("read above");
        note.current_revision = value.revision;
        note.revisions.push(value);
        let view = note.view(None)?;
        candidate.note_operations.insert(
            operation_id.into(),
            NoteOperation {
                request: Some(request),
                reference: NoteRef {
                    note_id: note_id.into(),
                    revision: Some(view.value.revision),
                },
            },
        );
        self.commit_document(candidate)?;
        Ok(view)
    }

    pub fn detach_personal_note(&mut self, note_id: &str) -> Result<(), ToolError> {
        self.personal_note(&NoteRef {
            note_id: note_id.into(),
            revision: None,
        })?;
        let mut candidate = self.projection_mutation_candidate()?;
        candidate
            .notes
            .iter_mut()
            .find(|n| n.note_id == note_id)
            .expect("read above")
            .location
            .detached = true;
        self.commit_document(candidate)
    }

    pub fn note_links(&self, note_id: &str) -> Vec<NoteLink> {
        self.document
            .note_links
            .iter()
            .filter(|l| l.from.note_id == note_id || l.to.note_id == note_id)
            .cloned()
            .collect()
    }

    pub fn link_personal_notes(
        &mut self,
        from: NoteRef,
        to: NoteRef,
        reason: Option<String>,
        now: &str,
    ) -> Result<NoteLink, ToolError> {
        if from.note_id == to.note_id {
            return Err(note_error(
                "NOTE_LINK_SELF",
                "validation",
                "请选择另一条想法",
            ));
        }
        let from_value = self.personal_note(&from)?;
        let to_value = self.personal_note(&to)?;
        let mut candidate = self.projection_mutation_candidate()?;
        if let Some(existing) = candidate.note_links.iter_mut().find(|l| {
            (l.from.note_id == from.note_id && l.to.note_id == to.note_id)
                || (l.to.note_id == from.note_id && l.from.note_id == to.note_id)
        }) {
            existing.reason_refs = vec![
                NoteRef {
                    note_id: from.note_id,
                    revision: Some(from_value.value.revision),
                },
                NoteRef {
                    note_id: to.note_id,
                    revision: Some(to_value.value.revision),
                },
            ];
            existing.reason = reason;
            let result = existing.clone();
            self.commit_document(candidate)?;
            return Ok(result);
        }
        let reason_refs = vec![
            NoteRef {
                note_id: from.note_id.clone(),
                revision: Some(from_value.value.revision),
            },
            NoteRef {
                note_id: to.note_id.clone(),
                revision: Some(to_value.value.revision),
            },
        ];
        let link = NoteLink {
            reason_refs,
            link_id: format!("link-{}", uuid::Uuid::now_v7()),
            from: NoteRef {
                revision: Some(from_value.value.revision),
                ..from
            },
            to: NoteRef {
                revision: Some(to_value.value.revision),
                ..to
            },
            reason,
            created_at: now.into(),
        };
        candidate.note_links.push(link.clone());
        self.commit_document(candidate)?;
        Ok(link)
    }

    pub fn unlink_personal_notes(&mut self, link_id: &str) -> Result<(), ToolError> {
        let mut candidate = self.projection_mutation_candidate()?;
        candidate.note_links.retain(|l| l.link_id != link_id);
        self.commit_document(candidate)
    }

    pub fn note_discussions(&self, note_id: &str) -> Vec<NoteDiscussion> {
        self.document
            .note_discussions
            .iter()
            .filter(|d| d.reference.note_id == note_id)
            .cloned()
            .collect()
    }

    pub fn record_note_discussion(&mut self, discussion: NoteDiscussion) -> Result<(), ToolError> {
        if discussion.reference.revision.is_none() {
            return Err(note_error(
                "NOTE_SOURCE_REVISION_REQUIRED",
                "validation",
                "讨论需要确切版本",
            ));
        }
        self.personal_note(&discussion.reference)?;
        if self
            .document
            .note_discussions
            .iter()
            .any(|d| d.reference == discussion.reference && d.turn == discussion.turn)
        {
            return Ok(());
        }
        let mut candidate = self.document_mutation_candidate()?;
        candidate.note_discussions.push(discussion);
        self.commit_document(candidate)
    }

    pub fn note_index_path(&self) -> std::path::PathBuf {
        self.path.with_file_name("personal-note-index")
    }
}

fn check_base(current: &PersonalNoteView, base: u32) -> Result<(), ToolError> {
    if current.current_revision != base {
        return Err(note_error(
            "NOTE_REVISION_CONFLICT",
            "conflict",
            format!(
                "此想法已保存为第 {} 版。请保留草稿，读取当前版本后合并。",
                current.current_revision
            ),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn content(text: &str) -> NoteContent {
        NoteContent {
            content: text.into(),
            title: None,
            authorship: NoteAuthorship::User,
            context: NoteContext::default(),
            source_refs: vec![],
            change_note: None,
            based_on_turn: None,
            source: None,
            capture: None,
        }
    }

    #[test]
    fn personal_notes_synthesis_remains_editable_after_a_source_is_deleted() {
        let directory = tempfile::tempdir().unwrap();
        let mut store = MemoryStore::open(directory.path().join("memory.json")).unwrap();
        let original = store
            .create_personal_note(content("旧假设"), "origin", "1")
            .unwrap();
        let mut explanation = content("综合解释");
        explanation.source_refs.push(NoteRef {
            note_id: original.note_id.clone(),
            revision: Some(1),
        });
        let derived = store
            .create_personal_note(explanation.clone(), "derived", "2")
            .unwrap();
        store.delete(&original.note_id).unwrap();
        explanation.content = "综合解释与新的条件".into();
        let saved = store
            .revise_personal_note(&derived.note_id, 1, explanation, "revised", "3")
            .unwrap();
        assert_eq!(saved.current_revision, 2);
        assert_eq!(saved.value.source_refs[0].note_id, original.note_id);
    }

    #[test]
    fn personal_notes_pages_keep_exact_text_and_all_catalog_entries() {
        use serde_json::json;
        let directory = tempfile::tempdir().unwrap();
        let mut store = MemoryStore::open(directory.path().join("memory.json")).unwrap();
        let text = "中文😀公式 $x^2$\n".repeat(500);
        let note = store
            .create_personal_note(content(&text), "long", "1")
            .unwrap();
        let other = store
            .create_personal_note(content("条件与反例"), "other", "1")
            .unwrap();
        let reason = "这条联系成立的条件。".repeat(600);
        let link = store
            .link_personal_notes(
                NoteRef {
                    note_id: note.note_id.clone(),
                    revision: Some(1),
                },
                NoteRef {
                    note_id: other.note_id,
                    revision: Some(1),
                },
                Some(reason.clone()),
                "1",
            )
            .unwrap();
        for n in 0..7 {
            store
                .record_note_discussion(NoteDiscussion {
                    reference: NoteRef {
                        note_id: note.note_id.clone(),
                        revision: Some(1),
                    },
                    turn: NoteTurnRef {
                        session_id: "chat".into(),
                        turn_id: format!("t{n}"),
                    },
                    created_at: n.to_string(),
                    expression: Some(text.clone()),
                })
                .unwrap();
        }
        for (section, expected) in [
            ("body", text.as_str()),
            ("expression", text.as_str()),
            ("link", reason.as_str()),
        ] {
            let mut start = 0;
            let mut joined = String::new();
            loop {
                let page=store.read_personal_note_page(&json!({"note_id":note.note_id,"revision":1,"section":section,"turn_id":"t1","link_id":link.link_id,"start_char":start,"limit_chars":700})).unwrap();
                joined.push_str(page["text"].as_str().unwrap());
                if let Some(next) = page["next_start_char"].as_u64() {
                    start = next;
                } else {
                    break;
                }
            }
            assert_eq!(joined, expected);
        }
        let page = store
            .read_personal_note_page(&json!({"note_id":note.note_id}))
            .unwrap();
        assert_eq!(page["related_expressions"].as_array().unwrap().len(), 4);
        assert_eq!(page["next_expressions_after"], 4);
        assert_eq!(page["links"][0]["truncated"], true);
        assert_eq!(
            store
                .read_personal_note_page(&json!({"note_id":note.note_id,"expressions_after":4}))
                .unwrap()["related_expressions"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
        for n in 0..105 {
            store
                .create_personal_note(
                    content(&format!("独立想法 {n}")),
                    &format!("entry-{n}"),
                    "2",
                )
                .unwrap();
        }
        let (last, total) = store.personal_note_catalog("", 100, 20);
        assert_eq!(total, 107);
        assert_eq!(last.len(), 7);
        store.delete(&note.note_id).unwrap();
        assert!(store
            .read_personal_note_page(&json!({"note_id":note.note_id,"revision":1}))
            .is_err());
    }

    #[test]
    #[ignore = "Representative PN save/reopen cost; PN_STORAGE_REPORT selects output"]
    fn personal_notes_storage_cost_at_one_thousand_notes_three_revisions() {
        use std::time::Instant;
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("memory.json");
        let mut store = MemoryStore::open(&path).unwrap();
        let initial = store
            .create_personal_note(content(&"参数固定，输入改变激活。".repeat(30)), "seed", "1")
            .unwrap();
        let mut candidate = store.projection_mutation_candidate().unwrap();
        let template = candidate.notes[0].clone();
        candidate.notes.clear();
        for n in 0..1000 {
            let mut note = template.clone();
            note.note_id = format!("note-{n:04}");
            note.current_revision = 3;
            note.revisions = (1..=3)
                .map(|revision| {
                    let mut v = template.revisions[0].clone();
                    v.revision = revision;
                    v.record.mem_id = note.note_id.clone();
                    v.record.content.push_str(&format!("第{revision}次表述"));
                    v
                })
                .collect();
            candidate.notes.push(note);
        }
        candidate.note_operations.clear();
        store.commit_document(candidate).unwrap();
        let start = Instant::now();
        store
            .revise_personal_note(
                "note-0500",
                3,
                content("我现在区分了参数与激活，还想检查注意力的具体机制。"),
                "edit",
                "2",
            )
            .unwrap();
        let save = start.elapsed().as_secs_f64() * 1000.0;
        drop(store);
        let start = Instant::now();
        let store = MemoryStore::open(&path).unwrap();
        let reopen = start.elapsed().as_secs_f64() * 1000.0;
        assert_eq!(store.document.notes.len(), 1000);
        assert_eq!(
            store
                .document
                .notes
                .iter()
                .map(|n| n.revisions.len())
                .sum::<usize>(),
            3001
        );
        let report = serde_json::json!({"notes":1000,"revisions":3001,"characters_per_revision":initial.value.record.content.chars().count(),"bytes":std::fs::metadata(path).unwrap().len(),"save_ms":save,"reopen_ms":reopen});
        if let Ok(path) = std::env::var("PN_STORAGE_REPORT") {
            std::fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
        }
        eprintln!("{report}");
    }

    #[test]
    fn personal_notes_preserve_evolution_retries_conflicts_and_restore_after_reopen() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("memory.json");
        let mut store = MemoryStore::open(&path).unwrap();
        let first = store
            .create_personal_note(content("参数不变，表示为何变化？"), "save-1", "t1")
            .unwrap();
        assert_eq!(
            store
                .create_personal_note(content("参数不变，表示为何变化？"), "save-1", "retry")
                .unwrap(),
            first
        );
        let second = store
            .revise_personal_note(
                &first.note_id,
                1,
                content("参数固定；激活取决于输入。"),
                "save-2",
                "t2",
            )
            .unwrap();
        assert_eq!(second.current_revision, 2);
        let before = std::fs::read(&path).unwrap();
        assert_eq!(
            store
                .revise_personal_note(
                    &first.note_id,
                    1,
                    content("另一个页面的草稿"),
                    "stale",
                    "t3"
                )
                .unwrap_err()
                .error_code,
            "NOTE_REVISION_CONFLICT"
        );
        assert_eq!(std::fs::read(&path).unwrap(), before);
        let same = store
            .revise_personal_note(
                &first.note_id,
                2,
                content("参数固定；激活取决于输入。"),
                "save-3",
                "t3",
            )
            .unwrap();
        assert_eq!(same.current_revision, 2);
        drop(store);
        let mut store = MemoryStore::open(&path).unwrap();
        let restored = store
            .restore_personal_note(&first.note_id, 2, 1, "restore", "t4")
            .unwrap();
        assert_eq!(restored.value.revision, 3);
        assert_eq!(restored.value.restored_from, Some(1));
        assert_eq!(restored.value.record.content, first.value.record.content);
        assert_eq!(
            store
                .personal_note(&NoteRef {
                    note_id: first.note_id.clone(),
                    revision: Some(2)
                })
                .unwrap()
                .value,
            second.value
        );
        let history = store
            .personal_note_history(&first.note_id, None, 2)
            .unwrap();
        assert_eq!(
            history.iter().map(|r| r.revision).collect::<Vec<_>>(),
            vec![3, 2]
        );
        assert_eq!(
            store
                .personal_note_history(&first.note_id, Some(2), 2)
                .unwrap()
                .len(),
            1
        );
        assert!(store.document.records.is_empty());
        assert_eq!(
            store.current_note_record(&first.note_id).unwrap().content,
            first.value.record.content
        );
        store.delete(&first.note_id).unwrap();
        assert!(!std::fs::read_to_string(&path).unwrap().contains("参数不变"));
        assert!(store
            .create_personal_note(content("参数不变，表示为何变化？"), "save-1", "retry")
            .is_err());
        assert!(MemoryStore::open(&path)
            .unwrap()
            .personal_notes(None, None, 20)
            .is_empty());
    }

    #[test]
    fn personal_notes_failed_commit_does_not_publish_revision_or_operation() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("memory.json");
        let mut store = MemoryStore::open(&path).unwrap();
        let note = store
            .create_personal_note(content("原话"), "create", "t1")
            .unwrap();
        let before = std::fs::read(&path).unwrap();
        std::fs::create_dir(crate::atomic_temporary_path(&path)).unwrap();
        assert!(store
            .revise_personal_note(&note.note_id, 1, content("新话"), "edit", "t2")
            .is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert_eq!(
            store
                .personal_note(&NoteRef {
                    note_id: note.note_id.clone(),
                    revision: None
                })
                .unwrap(),
            note
        );
        std::fs::remove_dir(crate::atomic_temporary_path(&path)).unwrap();
        assert_eq!(
            store
                .revise_personal_note(&note.note_id, 1, content("新话"), "edit", "t2")
                .unwrap()
                .current_revision,
            2
        );
    }

    #[test]
    fn personal_note_links_freeze_basis_and_delete_removes_relationship_not_other_note() {
        let directory = tempfile::tempdir().unwrap();
        let mut store = MemoryStore::open(directory.path().join("memory.json")).unwrap();
        let a = store.create_personal_note(content("A"), "a", "t1").unwrap();
        let b = store.create_personal_note(content("B"), "b", "t1").unwrap();
        let link = store
            .link_personal_notes(
                NoteRef {
                    note_id: a.note_id.clone(),
                    revision: None,
                },
                NoteRef {
                    note_id: b.note_id.clone(),
                    revision: None,
                },
                Some("相同的前提".into()),
                "t1",
            )
            .unwrap();
        store
            .revise_personal_note(&a.note_id, 1, content("A2"), "a2", "t2")
            .unwrap();
        assert_eq!(store.note_links(&b.note_id), vec![link.clone()]);
        assert_eq!(
            store
                .personal_note(&link.from)
                .unwrap()
                .value
                .record
                .content,
            "A"
        );
        let revised = store
            .link_personal_notes(
                NoteRef {
                    note_id: a.note_id.clone(),
                    revision: Some(2),
                },
                link.to.clone(),
                Some("新说明".into()),
                "t2",
            )
            .unwrap();
        assert_eq!(revised.link_id, link.link_id);
        assert_eq!(revised.from.revision, Some(1));
        assert_eq!(revised.reason_refs[0].revision, Some(2));
        store.delete(&a.note_id).unwrap();
        assert!(store.note_links(&b.note_id).is_empty());
        assert_eq!(store.personal_notes(None, None, 20).len(), 1);
    }
}
