use crate::{MemoryStore, NoteAuthorship, NoteContext, NoteRef, PersonalNote};
use read_tools::{LexicalTextIndex, ToolError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct NoteSearchHit {
    pub reference: NoteRef,
    pub title: Option<String>,
    pub excerpt: String,
    pub authorship: NoteAuthorship,
    pub context: NoteContext,
    pub lexical_rank: Option<usize>,
    pub semantic_rank: Option<usize>,
    /// Learning-map recall (ADR-0178 M2); present only when a caller asked for it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub structural_rank: Option<usize>,
    pub score: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteIndexDocument {
    pub note_id: String,
    pub revision: u32,
    pub expression_count: usize,
    pub text: String,
}
struct Chunk {
    note_id: String,
    revision: u32,
    book_id: Option<String>,
    text: String,
}
pub(crate) struct NoteLexicalIndex {
    stamp: Vec<(String, u32, usize)>,
    chunks: Vec<Chunk>,
    index: LexicalTextIndex,
}

fn text(note: &PersonalNote) -> String {
    let value = note.current();
    let mut text = value
        .title
        .as_deref()
        .filter(|t| !t.trim().is_empty())
        .map(|t| format!("标题：{t}\n"))
        .unwrap_or_default();
    text.push_str(&value.record.content);
    if let Some(excerpt) = value
        .record
        .note
        .as_ref()
        .and_then(|n| n.retained_excerpt.as_ref())
    {
        text.push_str("\n保留素材：");
        text.push_str(excerpt.text());
    }
    if let Some(label) = &value.context.label {
        text.push_str("\n记录情境：");
        text.push_str(label);
    }
    text
}

impl MemoryStore {
    pub fn note_index_documents(&self) -> Vec<NoteIndexDocument> {
        self.document
            .notes
            .iter()
            .map(|n| {
                let expressions = self.note_discussions(&n.note_id);
                let mut text = text(n);
                for expression in &expressions {
                    if let Some(words) = &expression.expression {
                        text.push_str(&format!(
                            "\n相关讨论中的原话（{}，回合 {}）：{}",
                            expression.created_at, expression.turn.turn_id, words
                        ));
                    }
                }
                NoteIndexDocument {
                    note_id: n.note_id.clone(),
                    revision: n.current_revision,
                    expression_count: expressions.len(),
                    text,
                }
            })
            .collect()
    }
    pub fn search_personal_notes(
        &self,
        query: &str,
        book_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<NoteSearchHit>, ToolError> {
        self.search_personal_notes_matching(query, book_id, None, limit)
    }

    /// Restrict candidates before ranking and limiting, for views such as boards.
    pub fn search_personal_notes_in(
        &self,
        query: &str,
        note_ids: &BTreeSet<String>,
        limit: usize,
    ) -> Result<Vec<NoteSearchHit>, ToolError> {
        self.search_personal_notes_matching(query, None, Some(note_ids), limit)
    }

    fn search_personal_notes_matching(
        &self,
        query: &str,
        book_id: Option<&str>,
        note_ids: Option<&BTreeSet<String>>,
        limit: usize,
    ) -> Result<Vec<NoteSearchHit>, ToolError> {
        self.ensure_storage_available()?;
        if query.trim().is_empty() || query.chars().count() > 4096 || !(1..=100).contains(&limit) {
            return Err(crate::personal_notes::note_error(
                "INVALID_NOTE_QUERY",
                "validation",
                "请输入 1–4096 字查询，并选择 1–100 个候选",
            ));
        }
        let stamp: Vec<_> = self
            .document
            .notes
            .iter()
            .map(|n| {
                (
                    n.note_id.clone(),
                    n.current_revision,
                    self.document
                        .note_discussions
                        .iter()
                        .filter(|d| d.reference.note_id == n.note_id)
                        .count(),
                )
            })
            .collect();
        let mut cached = self.note_lexical_index.lock().unwrap();
        if cached.as_ref().is_none_or(|index| index.stamp != stamp) {
            let mut chunks = Vec::new();
            for indexed in self.note_index_documents() {
                let note = self
                    .document
                    .notes
                    .iter()
                    .find(|n| n.note_id == indexed.note_id)
                    .unwrap();
                let characters: Vec<_> = indexed.text.chars().collect();
                let mut start = 0;
                while start < characters.len() {
                    let end = (start + 600).min(characters.len());
                    chunks.push(Chunk {
                        note_id: note.note_id.clone(),
                        revision: note.current_revision,
                        book_id: note.current().context.book_id.clone(),
                        text: characters[start..end].iter().collect(),
                    });
                    if end == characters.len() {
                        break;
                    }
                    start = end - 80;
                }
            }
            let index = LexicalTextIndex::new(chunks.iter().map(|c| c.text.as_str()));
            *cached = Some(NoteLexicalIndex {
                stamp,
                chunks,
                index,
            });
        }
        let cached = cached.as_ref().unwrap();
        let mut seen = BTreeSet::new();
        let ranked = cached.index.search(query, cached.chunks.len(), |index| {
            book_id.is_none_or(|id| cached.chunks[index].book_id.as_deref() == Some(id))
                && note_ids.is_none_or(|ids| ids.contains(&cached.chunks[index].note_id))
        });
        let mut hits = Vec::new();
        for (index, score) in ranked {
            let chunk = &cached.chunks[index];
            if !seen.insert(&chunk.note_id) {
                continue;
            }
            let note = self
                .document
                .notes
                .iter()
                .find(|n| n.note_id == chunk.note_id)
                .expect("indexed current note");
            hits.push(NoteSearchHit {
                reference: NoteRef {
                    note_id: chunk.note_id.clone(),
                    revision: Some(chunk.revision),
                },
                title: note.current().title.clone(),
                excerpt: chunk.text.clone(),
                authorship: note.current().authorship,
                context: note.current().context.clone(),
                lexical_rank: Some(hits.len() + 1),
                semantic_rank: None,
                structural_rank: None,
                score,
            });
            if hits.len() == limit {
                break;
            }
        }
        Ok(hits)
    }
}
