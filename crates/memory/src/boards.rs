//! Private, workspace-scoped knowledge boards. Retained content is independent
//! of chat history; layout and camera state never rewrite its source snapshots.
use crate::{
    NotePresentationReceipt, NotePresentationRef, NoteSourceBinding, ReaderPrivateStorageGate,
};
use read_tools::ToolError;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use ts_rs::TS;

mod mind_map;
pub use mind_map::*;

// A host already owns the private-root writer lease. This short lock also makes
// read/check/commit atomic between requests from multiple windows in that host.
static BOARD_ACCESS: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct BoardDocument {
    pub board_id: String,
    pub learning_workspace_id: Option<String>,
    pub book_id: String,
    pub publication_id: Option<String>,
    pub source_revision: Option<String>,
    pub title: String,
    pub revision: u32,
    pub created_at: String,
    pub updated_at: String,
    pub items: Vec<BoardItem>,
    /// Delivery receipts survive removal of their visible items, so replaying a
    /// completed turn cannot recreate something the reader already removed.
    #[serde(default)]
    pub deliveries: Vec<BoardDelivery>,
    #[serde(default)]
    pub links: Vec<BoardLink>,
    #[serde(default)]
    pub mappings: Vec<BoardExplanationMapping>,
    #[serde(default)]
    pub routes: Vec<BoardRoute>,
    #[serde(default)]
    #[ts(optional, as = "Option<_>")]
    pub mind_maps: Vec<BoardMindMap>,
    #[serde(default)]
    #[ts(optional, as = "Option<_>")]
    pub edit_history: BoardEditHistory,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct BoardSummary {
    pub board_id: String,
    pub learning_workspace_id: Option<String>,
    pub book_id: String,
    pub publication_id: Option<String>,
    pub source_revision: Option<String>,
    pub title: String,
    pub revision: u32,
    pub created_at: String,
    pub updated_at: String,
    pub item_count: u32,
}

impl From<&BoardDocument> for BoardSummary {
    fn from(document: &BoardDocument) -> Self {
        Self {
            board_id: document.board_id.clone(),
            learning_workspace_id: document.learning_workspace_id.clone(),
            book_id: document.book_id.clone(),
            publication_id: document.publication_id.clone(),
            source_revision: document.source_revision.clone(),
            title: document.title.clone(),
            revision: document.revision,
            created_at: document.created_at.clone(),
            updated_at: document.updated_at.clone(),
            item_count: document.items.len() as u32,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct BoardItem {
    pub item_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub reply_to_item_id: Option<String>,
    /// WB2 retains one complete compiled answer as one stable explanation block.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub block_id: Option<String>,
    #[serde(default)]
    pub explanation_blocks: Vec<BoardExplanationBlock>,
    pub title: String,
    /// Complete retained text, not the clipped preview shown on the board.
    pub readable_content: String,
    pub position: BoardPosition,
    pub size: BoardSize,
    pub created_at: String,
    pub source_bindings: Vec<NoteSourceBinding>,
    pub target: BoardTarget,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct BoardPosition {
    #[serde(deserialize_with = "deserialize_board_number")]
    pub x: f64,
    #[serde(deserialize_with = "deserialize_board_number")]
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct BoardSize {
    #[serde(deserialize_with = "deserialize_board_number")]
    pub width: f64,
    #[serde(deserialize_with = "deserialize_board_number")]
    pub height: f64,
}

fn validate_item_update(original: &BoardItem, item: &BoardItem) -> Result<(), ToolError> {
    let topic_revision = matches!((&original.target, &item.target),
        (BoardTarget::Topic { based_on: original_sources, .. }, BoardTarget::Topic { based_on, .. })
            if based_on == original_sources && item.readable_content.is_empty());
    let note_revision=matches!((&original.target,&item.target),(BoardTarget::PersonalNote{reference:old},BoardTarget::PersonalNote{reference:new}) if old.note_id==new.note_id && new.revision.is_some());
    let drawing_revision = matches!((&original.target, &item.target),
        (BoardTarget::Drawing { session_id: old_session, turn_id: old_turn, .. },
         BoardTarget::Drawing { session_id, turn_id, .. }) if old_session == session_id && old_turn == turn_id);
    if ((!drawing_revision && !topic_revision && !note_revision) && (item.target != original.target || item.readable_content != original.readable_content))
        || (note_revision && !item.readable_content.is_empty())
        || item.source_bindings != original.source_bindings
        || item.created_at != original.created_at
        || item.reply_to_item_id != original.reply_to_item_id
        || item.block_id != original.block_id
        || item.explanation_blocks != original.explanation_blocks
    {
        return Err(invalid("Retained board content and original authorship cannot be rewritten"));
    }
    Ok(())
}

fn follow_up_item<'a>(document: &'a BoardDocument, follow_up: &BoardFollowUp) -> Result<Option<&'a BoardItem>, ToolError> {
    let Some(id) = &follow_up.item_id else {
        if follow_up.block_id.is_some() || follow_up.presentation.is_some() {
            return Err(invalid("An explanation block or presentation needs its selected board item"));
        }
        return Ok(None);
    };
    let selected = document.items.iter().find(|item|&item.item_id == id)
        .ok_or_else(||not_found("The selected board item is unavailable"))?;
    if follow_up.block_id.as_ref().is_some_and(|id|resolve_explanation_block(selected,id).is_none()) {
        return Err(not_found("The selected explanation block is unavailable"));
    }
    Ok(Some(selected))
}

fn deserialize_board_number<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
    // AuthorRequest and Drawing/Graphic are tagged enums. serde_json's
    // arbitrary_precision decimal survives their Content buffer as a number
    // map, which Value decodes before conversion to the board's f64 geometry.
    serde_json::Value::deserialize(deserializer)?.as_f64()
        .ok_or_else(|| serde::de::Error::custom("board geometry must be a number"))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct BoardExplanationBlock {
    pub block_id: String,
    pub markdown: String,
    #[ts(type = "Array<unknown>")]
    pub answer_parts: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct BoardContentRef {
    pub item_id: String,
    pub block_id: Option<String>,
    pub content_revision: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct BoardPoint {
    #[serde(deserialize_with = "deserialize_board_number")]
    pub x: f64,
    #[serde(deserialize_with = "deserialize_board_number")]
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub enum BoardShape {
    Rectangle,
    Ellipse,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub enum BoardGraphic {
    Text {
        text: String,
    },
    Path {
        points: Vec<BoardPoint>,
        color: String,
        #[serde(deserialize_with = "deserialize_board_number")]
        width: f64,
    },
    Shape {
        shape: BoardShape,
        text: String,
        color: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub enum BoardVisualTarget {
    Item {
        item_id: String,
    },
    Link {
        link_id: String,
    },
    TopicEdge {
        child_item_id: String,
    },
    PresentationAnchor {
        item_id: String,
        reference: NotePresentationRef,
        anchor_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct BoardLink {
    pub link_id: String,
    pub from: BoardVisualTarget,
    pub to: BoardVisualTarget,
    pub label: String,
    #[serde(default)]
    pub based_on: Vec<BoardContentRef>,
    /// Workspace notebook relations are projected at read time, never retained
    /// as a second editable relationship in a board revision.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub note_relation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct BoardExplanationMapping {
    pub mapping_id: String,
    pub explanation: BoardContentRef,
    pub visual: BoardVisualTarget,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub enum BoardTransition {
    Instant,
    Draw,
    RevealText,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct BoardRouteStep {
    pub step_id: String,
    pub explanation: Option<BoardContentRef>,
    pub reveal_targets: Vec<BoardVisualTarget>,
    pub focus_targets: Vec<BoardVisualTarget>,
    pub transition: BoardTransition,
    pub duration_ms: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct BoardRoute {
    pub route_id: String,
    pub title: String,
    pub steps: Vec<BoardRouteStep>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct BoardPlaybackState {
    pub route_id: String,
    pub step_id: Option<String>,
    pub following: bool,
    pub show_all: bool,
}

/// Each variant records who produced the content and the exact original object.
/// These are server-resolved snapshots, never client-authored replacement facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub enum BoardArtifactKind {
    Pass1,
    Discourse,
    BookStructure,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub enum BoardTarget {
    Topic {
        #[serde(default)]
        children: Vec<String>,
        #[serde(default)]
        style: BoardTopicStyle,
        #[serde(default)]
        based_on: Vec<BoardContentRef>,
    },
    /// The notebook owns the title, body and provenance; the board owns layout.
    Note {
        note_id: String,
        #[serde(default)]
        missing: bool,
    },
    /// Body belongs only to the user's note store. This ref is the saved board snapshot's version.
    PersonalNote { reference: crate::NoteRef },
    Artifact {
        source_id: String,
        source_revision: String,
        artifact: BoardArtifactKind,
        entry_id: String,
        #[ts(type = "unknown")]
        snapshot: serde_json::Value,
    },
    Drawing {
        graphic: BoardGraphic,
        session_id: String,
        turn_id: String,
        based_on: Vec<BoardContentRef>,
    },
    Prebuild {
        source_id: String,
        object_id: String,
        object_revision: u32,
        map_revision: String,
        #[ts(type = "unknown")]
        object_snapshot: serde_json::Value,
    },
    Answer {
        session_id: String,
        turn_id: String,
        question: String,
        #[ts(type = "Array<unknown>")]
        answer_parts: Vec<serde_json::Value>,
    },
    Presentation {
        receipt: NotePresentationReceipt,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        source_book_id: Option<String>,
    },
    Understanding {
        evidence_id: String,
        source_id: String,
        source_revision: String,
        object_id: Option<String>,
        object_revision: Option<u32>,
        learner_quote: String,
        interpretation: String,
        #[ts(type = "unknown")]
        evidence_snapshot: serde_json::Value,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct BoardFollowUp {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub note_refs: Option<Vec<crate::NoteRef>>,
    pub board_id: String,
    pub board_revision: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub item_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub block_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub presentation: Option<NotePresentationReceipt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub intent: Option<BoardFollowUpIntent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub regenerate_from: Option<BoardRegenerationRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct BoardRegenerationRef {
    pub session_id: String,
    pub turn_id: String,
    pub candidate_id: String,
    pub candidate_revision: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub enum BoardFollowUpIntent {
    ConvertToMindMap,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct BoardDelivery {
    pub board_id: String,
    /// A formally committed authored part. Absent for the legacy whole-turn
    /// delivery; retries keep the server's persisted candidate identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub commit_id: Option<String>,
    pub before_revision: u32,
    pub after_revision: u32,
    pub item_ids: Vec<String>,
    pub block_ids: Vec<String>,
    pub origin_session_id: String,
    pub origin_turn_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub parent_item_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub enum BoardLookupTarget {
    Note {
        note_id: String,
    },
    PersonalNote { note_id: String },
    Artifact {
        source_id: String,
        source_revision: String,
        artifact: BoardArtifactKind,
        entry_id: String,
    },
    Prebuild {
        source_id: String,
        object_id: String,
        object_revision: u32,
    },
    Understanding {
        evidence_id: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub enum BoardSearchKind {
    PersonalNote,
    Board,
    Note,
    Prebuild,
    Answer,
    Presentation,
    Understanding,
    Drawing,
    Topic,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct BoardSearchHit {
    pub board_id: String,
    pub board_revision: u32,
    pub board_title: String,
    pub item_id: Option<String>,
    pub block_id: Option<String>,
    pub kind: BoardSearchKind,
    pub title: String,
    pub snippet: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct BoardView {
    pub x: f64,
    pub y: f64,
    pub zoom: f64,
    pub selected_item_id: Option<String>,
    #[serde(default)]
    pub selected_block_id: Option<String>,
    #[serde(default)]
    pub reading_item_id: Option<String>,
    #[serde(default)]
    pub playback: Option<BoardPlaybackState>,
    #[serde(default)]
    pub presentation_states: BTreeMap<String, NotePresentationReceipt>,
    #[serde(default)]
    #[ts(optional, as = "Option<_>")]
    pub folded_topic_ids: Vec<String>,
    #[serde(default)]
    #[ts(optional, as = "Option<_>")]
    pub selected_item_ids: Vec<String>,
    #[serde(default)]
    #[ts(optional, as = "Option<_>")]
    pub mind_map_outline: bool,
}

impl Default for BoardView {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            zoom: 1.0,
            selected_item_id: None,
            selected_block_id: None,
            reading_item_id: None,
            playback: None,
            presentation_states: BTreeMap::new(),
            folded_topic_ids: vec![],
            selected_item_ids: vec![],
            mind_map_outline: false,
        }
    }
}

pub struct BoardStore {
    root: PathBuf,
    private_root: PathBuf,
    learning_workspace_id: Option<String>,
    private: bool,
}

impl BoardStore {
    /// `private_root` is chosen by the authenticated server, never by a request.
    pub fn open(
        private_root: &Path,
        learning_workspace_id: Option<&str>,
        private: bool,
    ) -> Result<Self, ToolError> {
        let root = match learning_workspace_id {
            Some(id) => {
                validate_id(id)?;
                private_root.join("boards").join("workspaces").join(id)
            }
            None => private_root.join("boards").join("unscoped"),
        };
        Ok(Self {
            root,
            private_root: private_root.to_owned(),
            learning_workspace_id: learning_workspace_id.map(str::to_owned),
            private,
        })
    }

    pub fn create(&self, mut document: BoardDocument) -> Result<BoardDocument, ToolError> {
        let _access = access()?;
        document.revision = 1;
        normalize_note_references(&mut document);
        assign_answer_blocks(&mut document);
        self.validate_document(&document)?;
        if self.read_head_optional(&document.board_id)?.is_some() {
            return Err(conflict("This board already exists"));
        }
        self.commit(&document)?;
        self.hydrate_notes(document)
    }

    pub fn read(&self, board_id: &str) -> Result<BoardDocument, ToolError> {
        let _access = access()?;
        let head = self.read_head(board_id)?;
        self.hydrate_notes(self.read_snapshot(&head, head.revision)?)
    }

    pub fn read_revision(&self, board_id: &str, revision: u32) -> Result<BoardDocument, ToolError> {
        let _access = access()?;
        let head = self.read_head(board_id)?;
        if revision == 0 || revision > head.revision {
            return Err(not_found("This board revision is unavailable"));
        }
        self.hydrate_notes(self.read_snapshot(&head, revision)?)
    }

    /// The board's durable receipts are authoritative even if a run stopped
    /// before acknowledging a commit. Document order is commit order.
    pub fn deliveries_for_turn(
        &self,
        board_id: &str,
        session_id: &str,
        turn_id: &str,
    ) -> Result<Vec<BoardDelivery>, ToolError> {
        let _access = access()?;
        let head = self.read_head(board_id)?;
        let document = self.read_snapshot(&head, head.revision)?;
        Ok(document
            .deliveries
            .into_iter()
            .filter(|receipt| {
                receipt.origin_session_id == session_id && receipt.origin_turn_id == turn_id
            })
            .collect())
    }

    pub fn list(&self) -> Result<Vec<BoardSummary>, ToolError> {
        let _access = access()?;
        self.list_unlocked()
    }

    fn list_unlocked(&self) -> Result<Vec<BoardSummary>, ToolError> {
        if !self.root.exists() {
            return Ok(Vec::new());
        }
        let mut boards = Vec::new();
        for entry in std::fs::read_dir(&self.root).map_err(storage)? {
            let entry = entry.map_err(storage)?;
            if !entry.file_type().map_err(storage)?.is_dir() {
                continue;
            }
            let id = entry.file_name().to_string_lossy().into_owned();
            if let Some(head) = self.read_head_optional(&id)? {
                boards.push(head);
            }
        }
        boards.sort_by(|a, b| {
            b.updated_at
                .cmp(&a.updated_at)
                .then(a.board_id.cmp(&b.board_id))
        });
        Ok(boards)
    }

    /// New items arrive already resolved by the server. Existing items may move,
    /// resize or be renamed/removed. Native drawings may be revised; retained
    /// source content and original authorship remain fixed.
    pub fn update(
        &self,
        mut document: BoardDocument,
        expected_revision: u32,
        now: &str,
    ) -> Result<BoardDocument, ToolError> {
        let _access = access()?;
        let head = self.read_head(&document.board_id)?;
        let previous = self.read_snapshot(&head, head.revision)?;
        normalize_note_references(&mut document);
        assign_answer_blocks(&mut document);
        if head.revision != expected_revision {
            self.replay_layout_over_deliveries(&mut document, expected_revision, &head, &previous)?;
        }
        if document.book_id != previous.book_id
            || document.publication_id != previous.publication_id
            || document.source_revision != previous.source_revision
            || document.created_at != previous.created_at
            || document.deliveries != previous.deliveries
            || document.links != previous.links
            || document.mappings != previous.mappings
            || document.routes != previous.routes
            || document.mind_maps != previous.mind_maps
        {
            return Err(invalid(
                "A board's original material and creation time cannot change",
            ));
        }
        let previous_items: BTreeMap<_, _> = previous
            .items
            .iter()
            .map(|item| (&item.item_id, item))
            .collect();
        for item in &document.items {
            if let Some(original) = previous_items.get(&item.item_id) {
                validate_item_update(original, item)?;
            }
        }
        document.revision = head
            .revision
            .checked_add(1)
            .ok_or_else(|| invalid("Board revision limit reached"))?;
        document.updated_at = now.to_owned();
        if previous.items.iter().any(|original| {
            !document
                .items
                .iter()
                .any(|item| item.item_id == original.item_id)
        }) {
            prune_removed_targets(&mut document);
        }
        mind_map::record_edit(&mut document, &previous, BoardEditActor::Human, "Edit board");
        self.validate_document(&document)?;
        self.commit(&document)?;
        self.hydrate_notes(document)
    }

    /// Place one canonical thought and its optional connector as one board revision.
    /// A replay returns the existing card; it never makes a second copy of the text.
    pub fn place_personal_note(&self, board_id: &str, reference: crate::NoteRef, related_item: Option<&str>, now: &str) -> Result<BoardDocument, ToolError> {
        let _access = access()?;
        let head = self.read_head(board_id)?;
        let mut document = self.read_snapshot(&head, head.revision)?;
        let target = related_item.map(|id| document.items.iter().find(|i|i.item_id==id).cloned()
            .ok_or_else(||not_found("关联的画布元素已移除，请重新选择位置"))).transpose()?;
        let existing = document.items.iter().find(|i|matches!(&i.target,BoardTarget::PersonalNote {reference:r} if r.note_id==reference.note_id)).map(|i|i.item_id.clone());
        let mut changed = false;
        let item_id = if let Some(id) = existing { id } else {
            changed = true;
            let id = format!("item-{}",uuid::Uuid::now_v7());
            document.items.push(BoardItem {
                item_id:id.clone(),reply_to_item_id:None,block_id:None,explanation_blocks:vec![],title:"我的想法".into(),readable_content:String::new(),
                position:target.as_ref().map(|i|BoardPosition{x:i.position.x+i.size.width+48.0,y:i.position.y})
                    .unwrap_or(BoardPosition{x:(document.items.len()%3) as f64*360.0,y:(document.items.len()/3) as f64*260.0}),
                size:BoardSize{width:320.0,height:220.0},created_at:now.into(),source_bindings:vec![],target:BoardTarget::PersonalNote{reference},
            });
            id
        };
        if let Some(target) = target {
            if target.item_id==item_id { return Err(invalid("不能把想法关联到它自己")); }
            let from=BoardVisualTarget::Item{item_id}; let to=BoardVisualTarget::Item{item_id:target.item_id};
            if !document.links.iter().any(|l|l.from==from&&l.to==to) {
                document.links.push(BoardLink{link_id:format!("note-link-{}",uuid::Uuid::now_v7()),from,to,label:"我的想法".into(),based_on:vec![],note_relation_id:None,reason:None}); changed=true;
            }
        }
        if changed {
            document.revision=head.revision.checked_add(1).ok_or_else(||invalid("Board revision limit reached"))?;
            document.updated_at=now.into();self.validate_document(&document)?;self.commit(&document)?;
        }
        Ok(document)
    }

    /// A completed turn appends to the latest layout. The target was frozen at
    /// admission; moving the camera or the card during generation cannot retarget it.
    pub fn append_delivery(
        &self,
        follow_up: &BoardFollowUp,
        session_id: &str,
        turn_id: &str,
        mut items: Vec<BoardItem>,
        now: &str,
    ) -> Result<BoardDelivery, ToolError> {
        let _access = access()?;
        let head = self.read_head(&follow_up.board_id)?;
        let mut document = self.read_snapshot(&head, head.revision)?;
        if let Some(delivery) = delivery_for_identity(&document, session_id, turn_id, None)? {
            return Ok(delivery);
        }
        let previous = document.clone();
        if follow_up.board_revision == 0 || follow_up.board_revision > head.revision {
            return Err(invalid("The selected board revision is unavailable"));
        }
        let frozen = self.read_snapshot(&head, follow_up.board_revision)?;
        follow_up_item(&frozen, follow_up)?;
        let parent = follow_up_item(&document, follow_up)?.cloned();
        if session_id.is_empty()
            || turn_id.is_empty()
            || !matches!(items.first().map(|item| &item.target),
                Some(BoardTarget::Answer { session_id: origin_session, turn_id: origin_turn, .. })
                    if origin_session == session_id && origin_turn == turn_id)
        {
            return Err(invalid(
                "A board delivery must start with its completed turn's full answer",
            ));
        }
        let mut delivery = BoardDelivery {
            board_id: follow_up.board_id.clone(),
            commit_id: None,
            before_revision: head.revision,
            after_revision: head
                .revision
                .checked_add(1)
                .ok_or_else(|| invalid("Board revision limit reached"))?,
            item_ids: Vec::new(),
            block_ids: Vec::new(),
            origin_session_id: session_id.into(),
            origin_turn_id: turn_id.into(),
            parent_item_id: follow_up.item_id.clone(),
        };
        for item in &mut items {
            item.reply_to_item_id = parent.as_ref().map(|parent|parent.item_id.clone());
            assign_answer_block(item);
            validate_geometry(item)?;
            item.position = parent.as_ref().map(|parent|BoardPosition {
                x: parent.position.x + parent.size.width + 72.0, y: parent.position.y,
            }).unwrap_or_else(|| BoardPosition { x:64.0,
                y:document.items.iter().map(|item|item.position.y + item.size.height + 72.0)
                    .max_by(f64::total_cmp).unwrap_or(64.0) });
            // Find the first free vertical gap beside the selected item. Existing
            // cards, including cards moved while the answer ran, stay untouched.
            loop {
                let bottom = document
                    .items
                    .iter()
                    .filter(|other| rectangles_overlap(item, other))
                    .map(|other| other.position.y + other.size.height + 32.0)
                    .max_by(f64::total_cmp);
                let Some(bottom) = bottom else {
                    break;
                };
                item.position.y = bottom;
            }
            delivery.item_ids.push(item.item_id.clone());
            delivery.block_ids.extend(item.block_id.clone());
            delivery.block_ids.extend(
                item.explanation_blocks
                    .iter()
                    .map(|block| block.block_id.clone()),
            );
            document.items.push(item.clone());
        }
        document.revision = delivery.after_revision;
        document.updated_at = now.into();
        document.deliveries.push(delivery.clone());
        normalize_note_references(&mut document);
        mind_map::record_edit(&mut document, &previous, BoardEditActor::Ai, "Add explanation");
        self.validate_document(&document)?;
        self.commit(&document)?;
        Ok(delivery)
    }

    /// Commits the exact authored projection that was previewed. Existing user
    /// layout is never recomputed, and a changed base requires a new preview.
    pub fn append_authored_delivery(
        &self,
        follow_up: &BoardFollowUp,
        expected_revision: u32,
        session_id: &str,
        turn_id: &str,
        items: Vec<BoardItem>,
        update_items: Vec<BoardItem>,
        links: Vec<BoardLink>,
        mappings: Vec<BoardExplanationMapping>,
        routes: Vec<BoardRoute>,
        now: &str,
    ) -> Result<BoardDelivery, ToolError> {
        self.append_authored(
            follow_up,
            expected_revision,
            session_id,
            turn_id,
            None,
            items,
            update_items,
            links,
            mappings,
            routes,
            vec![],
            now,
        )
    }

    pub fn append_authored_delivery_with_edits(
        &self, follow_up: &BoardFollowUp, expected_revision: u32,
        session_id: &str, turn_id: &str, items: Vec<BoardItem>, update_items: Vec<BoardItem>,
        links: Vec<BoardLink>, mappings: Vec<BoardExplanationMapping>, routes: Vec<BoardRoute>,
        operations: Vec<BoardEditOperation>, now: &str,
    ) -> Result<BoardDelivery, ToolError> {
        self.append_authored(follow_up, expected_revision, session_id, turn_id, None,
            items, update_items, links, mappings, routes, operations, now)
    }

    /// Commits one complete, observed part while its turn may continue. A retry
    /// uses the same candidate identity and returns its original receipt before
    /// checking the current layout or whether its visible content was removed.
    pub fn append_authored_part(
        &self,
        follow_up: &BoardFollowUp,
        expected_revision: u32,
        session_id: &str,
        turn_id: &str,
        commit_id: &str,
        items: Vec<BoardItem>,
        update_items: Vec<BoardItem>,
        links: Vec<BoardLink>,
        mappings: Vec<BoardExplanationMapping>,
        routes: Vec<BoardRoute>,
        now: &str,
    ) -> Result<BoardDelivery, ToolError> {
        if commit_id.trim().is_empty() {
            return Err(invalid(
                "A board part must retain its candidate commit identity",
            ));
        }
        self.append_authored(
            follow_up,
            expected_revision,
            session_id,
            turn_id,
            Some(commit_id),
            items,
            update_items,
            links,
            mappings,
            routes,
            vec![],
            now,
        )
    }

    pub fn append_authored_part_with_edits(
        &self, follow_up: &BoardFollowUp, expected_revision: u32,
        session_id: &str, turn_id: &str, commit_id: &str,
        items: Vec<BoardItem>, update_items: Vec<BoardItem>, links: Vec<BoardLink>,
        mappings: Vec<BoardExplanationMapping>, routes: Vec<BoardRoute>,
        operations: Vec<BoardEditOperation>, now: &str,
    ) -> Result<BoardDelivery, ToolError> {
        if commit_id.trim().is_empty() { return Err(invalid("A board part must retain its candidate commit identity")); }
        self.append_authored(follow_up, expected_revision, session_id, turn_id, Some(commit_id),
            items, update_items, links, mappings, routes, operations, now)
    }

    fn append_authored(
        &self,
        follow_up: &BoardFollowUp,
        expected_revision: u32,
        session_id: &str,
        turn_id: &str,
        commit_id: Option<&str>,
        mut items: Vec<BoardItem>,
        update_items: Vec<BoardItem>,
        links: Vec<BoardLink>,
        mappings: Vec<BoardExplanationMapping>,
        routes: Vec<BoardRoute>,
        operations: Vec<BoardEditOperation>,
        now: &str,
    ) -> Result<BoardDelivery, ToolError> {
        let _access = access()?;
        let head = self.read_head(&follow_up.board_id)?;
        let mut document = self.read_snapshot(&head, head.revision)?;
        if let Some(receipt) = delivery_for_identity(&document, session_id, turn_id, commit_id)? {
            return Ok(receipt);
        }
        let previous = document.clone();
        if head.revision != expected_revision {
            return Err(conflict(
                "The board changed after this authored content was previewed",
            ));
        }
        if follow_up.board_revision == 0 || follow_up.board_revision > head.revision {
            return Err(invalid("The selected board revision is unavailable"));
        }
        let frozen = self.read_snapshot(&head, follow_up.board_revision)?;
        follow_up_item(&frozen, follow_up)?;
        follow_up_item(&document, follow_up)?;
        if session_id.is_empty()
            || turn_id.is_empty()
            || !matches!(items.first().map(|item| &item.target), Some(BoardTarget::Answer {
                session_id: origin_session, turn_id: origin_turn, .. })
                if origin_session == session_id && origin_turn == turn_id)
        {
            return Err(invalid(
                "A board delivery must start with its turn's complete authored explanation",
            ));
        }
        let after_revision = head
            .revision
            .checked_add(1)
            .ok_or_else(|| invalid("Board revision limit reached"))?;
        let mut receipt = BoardDelivery {
            board_id: document.board_id.clone(),
            commit_id: commit_id.map(str::to_owned),
            before_revision: head.revision,
            after_revision,
            item_ids: Vec::new(),
            block_ids: Vec::new(),
            origin_session_id: session_id.into(),
            origin_turn_id: turn_id.into(),
            parent_item_id: follow_up.item_id.clone(),
        };
        let mut updated = BTreeSet::new();
        for mut item in update_items {
            normalize_note_item(&mut item);
            if !updated.insert(item.item_id.clone()) {
                return Err(invalid("An authored update must name each existing item once"));
            }
            let original = document.items.iter_mut().find(|old|old.item_id == item.item_id)
                .ok_or_else(||not_found("The board item to update is unavailable"))?;
            validate_item_update(original, &item)?;
            if !matches!(original.target, BoardTarget::Drawing { .. }) && item.title != original.title {
                return Err(invalid("An authored layout update cannot rename retained source content"));
            }
            *original = item;
        }
        for item in &mut items {
            item.reply_to_item_id = follow_up.item_id.clone();
            assign_answer_block(item);
            receipt.item_ids.push(item.item_id.clone());
            receipt.block_ids.extend(item.block_id.clone());
            receipt.block_ids.extend(
                item.explanation_blocks
                    .iter()
                    .map(|block| block.block_id.clone()),
            );
        }
        receipt.item_ids.extend(updated);
        document.items.extend(items);
        document.links.extend(links);
        document.mappings.extend(mappings);
        document.routes.extend(routes);
        mind_map::apply_operations(&mut document, &operations, now)?;
        for item in &document.items {
            let original = previous.items.iter().find(|old| old.item_id == item.item_id);
            if original != Some(item) && matches!(item.target, BoardTarget::Topic { .. })
                && !receipt.item_ids.contains(&item.item_id) {
                receipt.item_ids.push(item.item_id.clone());
            }
        }
        document.revision = after_revision;
        document.updated_at = now.into();
        document.deliveries.push(receipt.clone());
        normalize_note_references(&mut document);
        mind_map::record_edit(&mut document, &previous, BoardEditActor::Ai, "AI board edit");
        self.validate_document(&document)?;
        self.commit(&document)?;
        Ok(receipt)
    }

    /// Validates a complete candidate, including references to prior committed
    /// content. The caller supplies its predicted next content revision.
    pub fn validate_candidate(&self, document: &BoardDocument) -> Result<(), ToolError> {
        let _access = access()?;
        let mut candidate = document.clone();
        normalize_note_references(&mut candidate);
        assign_answer_blocks(&mut candidate);
        self.validate_document(&candidate)
    }

    /// Only an append-only delivery may be crossed by an in-flight layout save.
    /// Any concurrent edit to existing cards or the board title remains a conflict.
    fn replay_layout_over_deliveries(
        &self,
        candidate: &mut BoardDocument,
        expected_revision: u32,
        head: &BoardSummary,
        latest: &BoardDocument,
    ) -> Result<(), ToolError> {
        let changed = || conflict("This board changed in another window; reopen it before saving");
        if expected_revision == 0 || expected_revision >= head.revision {
            return Err(changed());
        }
        let base = self.read_snapshot(head, expected_revision)?;
        if latest.title != base.title
            || candidate.deliveries != base.deliveries
            || !latest.deliveries.starts_with(&base.deliveries)
            || latest.links != base.links
            || candidate.links != base.links
            || latest.mappings != base.mappings
            || candidate.mappings != base.mappings
            || latest.routes != base.routes
            || candidate.routes != base.routes
            || latest.mind_maps != base.mind_maps
            || candidate.mind_maps != base.mind_maps
        {
            return Err(changed());
        }
        let appended = &latest.deliveries[base.deliveries.len()..];
        if appended.len() != (head.revision - expected_revision) as usize
            || appended.iter().enumerate().any(|(index, delivery)| {
                delivery.before_revision != expected_revision + index as u32
                    || delivery.after_revision != expected_revision + index as u32 + 1
            })
        {
            return Err(changed());
        }
        let base_ids: BTreeSet<_> = base
            .items
            .iter()
            .map(|item| item.item_id.as_str())
            .collect();
        let delivery_ids: BTreeSet<_> = appended
            .iter()
            .flat_map(|delivery| delivery.item_ids.iter().map(String::as_str))
            .collect();
        if base.items.iter().any(|item| {
            latest
                .items
                .iter()
                .find(|current| current.item_id == item.item_id)
                != Some(item)
        }) || latest.items.iter().any(|item| {
            !base_ids.contains(item.item_id.as_str())
                && !delivery_ids.contains(item.item_id.as_str())
        }) || candidate
            .items
            .iter()
            .any(|item| !base_ids.contains(item.item_id.as_str()))
        {
            return Err(changed());
        }
        candidate.items.extend(
            latest
                .items
                .iter()
                .filter(|item| !base_ids.contains(item.item_id.as_str()))
                .cloned(),
        );
        candidate.deliveries = latest.deliveries.clone();
        Ok(())
    }

    pub fn search(
        &self,
        book_id: &str,
        publication_id: Option<&str>,
        query: &str,
        limit: usize,
    ) -> Result<Vec<BoardSearchHit>, ToolError> {
        let _access = access()?;
        let query = query.trim().to_lowercase();
        let limit = limit.min(100);
        if query.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }
        let mut hits = Vec::new();
        for head in self.list_unlocked()?.into_iter().filter(|head| {
            self.learning_workspace_id.is_some() || (head.book_id == book_id && head.publication_id.as_deref() == publication_id)
        }) {
            let document = self.hydrate_notes(self.read_snapshot(&head, head.revision)?)?;
            if document.title.to_lowercase().contains(&query) {
                hits.push(search_hit(
                    &document,
                    None,
                    snippet(&document.title, &query),
                ));
            }
            for item in &document.items {
                let matching_blocks: Vec<_> = item
                    .explanation_blocks
                    .iter()
                    .filter(|block| block.markdown.to_lowercase().contains(&query))
                    .collect();
                if !matching_blocks.is_empty() {
                    for block in matching_blocks {
                        let mut hit =
                            search_hit(&document, Some(item), snippet(&block.markdown, &query));
                        hit.block_id = Some(block.block_id.clone());
                        hits.push(hit);
                        if hits.len() >= limit {
                            break;
                        }
                    }
                    if hits.len() >= limit {
                        break;
                    }
                    continue;
                }
                let matched = searchable_fields(item)
                    .into_iter()
                    .find(|(_, text)| text.to_lowercase().contains(&query));
                if let Some((label, text)) = matched {
                    hits.push(search_hit(
                        &document,
                        Some(item),
                        format!("{label}{}", snippet(text, &query)),
                    ));
                }
                if hits.len() >= limit {
                    break;
                }
            }
            if hits.len() >= limit {
                break;
            }
        }
        hits.truncate(limit);
        Ok(hits)
    }

    pub fn find_related(
        &self,
        book_id: &str,
        publication_id: Option<&str>,
        target: &BoardLookupTarget,
    ) -> Result<Vec<BoardSearchHit>, ToolError> {
        let _access = access()?;
        let mut hits = Vec::new();
        for head in self.list_unlocked()?.into_iter().filter(|head| {
            self.learning_workspace_id.is_some() || (head.book_id == book_id && head.publication_id.as_deref() == publication_id)
        }) {
            let document = self.hydrate_notes(self.read_snapshot(&head, head.revision)?)?;
            for item in &document.items {
                let matches = match (target, &item.target) {
                    (BoardLookupTarget::Note { note_id }, BoardTarget::Note { note_id: id, missing: false }) => note_id == id,
                    (BoardLookupTarget::PersonalNote{note_id},BoardTarget::PersonalNote{reference})=>note_id==&reference.note_id,
                    (BoardLookupTarget::Artifact { source_id, source_revision, artifact, entry_id },
                     BoardTarget::Artifact { source_id: source, source_revision: revision, artifact: kind, entry_id: id, .. }) =>
                        source_id == source && source_revision == revision && artifact == kind && entry_id == id,
                    (
                        BoardLookupTarget::Prebuild {
                            source_id,
                            object_id,
                            object_revision,
                        },
                        BoardTarget::Prebuild {
                            source_id: source,
                            object_id: object,
                            object_revision: revision,
                            ..
                        },
                    ) => source_id == source && object_id == object && object_revision == revision,
                    (
                        BoardLookupTarget::Prebuild {
                            source_id,
                            object_id,
                            object_revision,
                        },
                        BoardTarget::Understanding {
                            source_id: source,
                            object_id: Some(object),
                            object_revision: Some(revision),
                            ..
                        },
                    ) => source_id == source && object_id == object && object_revision == revision,
                    (
                        BoardLookupTarget::Understanding { evidence_id },
                        BoardTarget::Understanding {
                            evidence_id: id, ..
                        },
                    ) => evidence_id == id,
                    _ => false,
                };
                if matches {
                    hits.push(search_hit(
                        &document,
                        Some(item),
                        snippet(&item.readable_content, ""),
                    ));
                }
            }
        }
        Ok(hits)
    }

    pub fn remove(&self, board_id: &str, expected_revision: u32) -> Result<(), ToolError> {
        let _access = access()?;
        let head = self.read_head(board_id)?;
        if head.revision != expected_revision {
            return Err(conflict(
                "This board changed in another window; reopen it before removing",
            ));
        }
        // board_path accepts one validated identifier within this store's scope.
        let path = self.board_path(board_id)?;
        std::fs::remove_dir_all(&path).map_err(storage)?;
        ReaderPrivateStorageGate::sync_parent(&path).map_err(storage)
    }

    pub fn view(&self, board_id: &str) -> Result<BoardView, ToolError> {
        let _access = access()?;
        let head = self.read_head(board_id)?;
        let document = self.read_snapshot(&head, head.revision)?;
        let mut view = self.read_view(board_id)?;
        normalize_view(&document, &mut view);
        Ok(view)
    }

    /// Camera-only saves preserve the last retained scene for every presentation.
    /// The server resolves and validates any newly supplied presentation receipt.
    pub fn save_view(&self, board_id: &str, view: &BoardView) -> Result<BoardView, ToolError> {
        let _access = access()?;
        let head = self.read_head(board_id)?;
        let document = self.read_snapshot(&head, head.revision)?;
        if !view.x.is_finite() || !view.y.is_finite() || !view.zoom.is_finite() || view.zoom <= 0.0
        {
            return Err(invalid(
                "The board camera needs finite coordinates and a positive zoom",
            ));
        }
        let mut next = view.clone();
        let mut receipts = self.read_view(board_id)?.presentation_states;
        receipts.extend(next.presentation_states);
        receipts.retain(|id, _| {
            document.items.iter().any(|item| {
                item.item_id == *id && matches!(item.target, BoardTarget::Presentation { .. })
            })
        });
        next.presentation_states = receipts;
        normalize_view(&document, &mut next);
        self.write_json(&self.board_path(board_id)?.join("view.json"), &next)?;
        Ok(next)
    }

    fn board_path(&self, board_id: &str) -> Result<PathBuf, ToolError> {
        validate_id(board_id)?;
        Ok(self.root.join(board_id))
    }

    fn read_head_optional(&self, board_id: &str) -> Result<Option<BoardSummary>, ToolError> {
        let path = self.board_path(board_id)?.join("head.json");
        crate::recover_interrupted_commit(&path)?;
        if !path.exists() {
            return Ok(None);
        }
        let head: BoardSummary = read_json(&path)?;
        if head.board_id != board_id
            || head.learning_workspace_id != self.learning_workspace_id
            || head.revision == 0
        {
            return Err(storage(
                "Board head identity does not match its storage scope",
            ));
        }
        Ok(Some(head))
    }

    fn read_head(&self, board_id: &str) -> Result<BoardSummary, ToolError> {
        self.read_head_optional(board_id)?
            .ok_or_else(|| not_found("This board is unavailable"))
    }

    fn read_snapshot(
        &self,
        head: &BoardSummary,
        revision: u32,
    ) -> Result<BoardDocument, ToolError> {
        let path = self
            .board_path(&head.board_id)?
            .join("revisions")
            .join(format!("{revision}.json"));
        let mut document: BoardDocument = read_json(&path)?;
        assign_answer_blocks(&mut document);
        if document.board_id != head.board_id
            || document.revision != revision
            || document.learning_workspace_id != self.learning_workspace_id
        {
            return Err(storage(
                "Board snapshot identity does not match the committed revision",
            ));
        }
        if revision == head.revision && BoardSummary::from(&document) != *head {
            return Err(storage("Board head does not match its committed content"));
        }
        Ok(document)
    }

    fn read_view(&self, board_id: &str) -> Result<BoardView, ToolError> {
        let path = self.board_path(board_id)?.join("view.json");
        crate::recover_interrupted_commit(&path)?;
        if path.exists() {
            read_json(&path)
        } else {
            Ok(BoardView::default())
        }
    }

    fn validate_document(&self, document: &BoardDocument) -> Result<(), ToolError> {
        validate_id(&document.board_id)?;
        validate_mind_maps(document)?;
        if document.learning_workspace_id != self.learning_workspace_id
            || document.book_id.trim().is_empty()
            || document.title.trim().is_empty()
        {
            return Err(invalid(
                "A board needs a title and the current material/workspace scope",
            ));
        }
        let mut ids = BTreeSet::new();
        let mut blocks = BTreeSet::new();
        for item in &document.items {
            validate_id(&item.item_id)?;
            if !ids.insert(&item.item_id) {
                return Err(invalid("Board item identifiers must be unique"));
            }
            if let Some(block_id) = &item.block_id {
                validate_id(block_id)?;
                if !blocks.insert(block_id) {
                    return Err(invalid("Explanation block identifiers must be unique"));
                }
            }
            for block in &item.explanation_blocks {
                validate_id(&block.block_id)?;
                if !blocks.insert(&block.block_id) {
                    return Err(invalid("Explanation block identifiers must be unique"));
                }
            }
            validate_geometry(item)?;
            if let BoardTarget::Note { note_id, .. } = &item.target {
                validate_id(note_id)?;
            }
            if let BoardTarget::Topic { based_on, .. } = &item.target {
                for reference in based_on { self.validate_content_ref(document, reference)?; }
            }
            if let BoardTarget::Drawing {
                graphic,
                session_id,
                turn_id,
                based_on,
            } = &item.target
            {
                if session_id.is_empty() || turn_id.is_empty() {
                    return Err(invalid("A drawing needs its authoring turn identity"));
                }
                if let BoardGraphic::Path { points, width, .. } = graphic {
                    if points.len() < 2
                        || !width.is_finite()
                        || *width <= 0.0
                        || points
                            .iter()
                            .any(|point| !point.x.is_finite() || !point.y.is_finite())
                    {
                        return Err(invalid(
                            "A drawing path needs finite points and a positive width",
                        ));
                    }
                }
                for reference in based_on {
                    self.validate_content_ref(document, reference)?;
                }
            }
        }
        let mut link_ids = BTreeSet::new();
        for link in &document.links {
            validate_id(&link.link_id)?;
            if !link_ids.insert(&link.link_id) {
                return Err(invalid("Board link identifiers must be unique"));
            }
            validate_visual_target(document, &link.from)?;
            validate_visual_target(document, &link.to)?;
            if let (BoardVisualTarget::Item { item_id: from }, BoardVisualTarget::Item { item_id: to }) = (&link.from, &link.to) {
                let is_note = |id: &String| document.items.iter().any(|item| &item.item_id == id && matches!(item.target, BoardTarget::Note { .. }));
                if is_note(from) && is_note(to) {
                    return Err(invalid("Connect workspace notes with notebook relations so every board shares the same label and reason"));
                }
            }
            for reference in &link.based_on {
                self.validate_content_ref(document, reference)?;
            }
            validate_link_path(document, &link.link_id, &mut BTreeSet::new())?;
        }
        let mut mapping_ids = BTreeSet::new();
        for mapping in &document.mappings {
            validate_id(&mapping.mapping_id)?;
            if !mapping_ids.insert(&mapping.mapping_id) {
                return Err(invalid("Board mapping identifiers must be unique"));
            }
            self.validate_content_ref(document, &mapping.explanation)?;
            if !content_is_present(document, &mapping.explanation) {
                return Err(invalid("Mapped explanation is no longer on this board"));
            }
            validate_visual_target(document, &mapping.visual)?;
        }
        let mut route_ids = BTreeSet::new();
        for route in &document.routes {
            validate_id(&route.route_id)?;
            if !route_ids.insert(&route.route_id) {
                return Err(invalid("Board route identifiers must be unique"));
            }
            let mut step_ids = BTreeSet::new();
            for step in &route.steps {
                validate_id(&step.step_id)?;
                if !step_ids.insert(&step.step_id) {
                    return Err(invalid("Board route step identifiers must be unique"));
                }
                if let Some(reference) = &step.explanation {
                    self.validate_content_ref(document, reference)?;
                    if !content_is_present(document, reference) {
                        return Err(invalid("A route explanation is no longer on this board"));
                    }
                }
                for target in step.reveal_targets.iter().chain(&step.focus_targets) {
                    validate_visual_target(document, target)?;
                }
            }
        }
        Ok(())
    }

    fn validate_content_ref(
        &self,
        document: &BoardDocument,
        reference: &BoardContentRef,
    ) -> Result<(), ToolError> {
        let unavailable = |reason: &str| {
            // A new explanation exists only in the proposed revision. Report
            // its exact address when the caller confuses block/item identities
            // or supplies the previous board revision.
            let matches_block = |item: &&BoardItem| reference.block_id.as_deref()
                .is_none_or(|block_id| resolve_explanation_block(item, block_id).is_some());
            let item = if let Some(item) = document.items.iter().find(|item| item.item_id == reference.item_id) {
                matches_block(&item).then_some(item)
            } else if reference.block_id.is_some() {
                let mut matches = document.items.iter().filter(matches_block);
                let first = matches.next();
                if matches.next().is_none() { first } else { None }
            } else {
                None
            };
            let available = item.map(|item| BoardContentRef {
                    item_id: item.item_id.clone(),
                    block_id: reference.block_id.clone(),
                    content_revision: document.revision,
            });
            let repair = available.map_or_else(
                || "Read the board for the exact retained item and block identity.".to_string(),
                |available| format!("Available in the proposed board: {}. Use this exact content reference for that content.", serde_json::json!(available)),
            );
            invalid(&format!("{reason}: {}. {repair}", serde_json::json!(reference)))
        };
        if reference.content_revision == 0 || reference.content_revision > document.revision {
            return Err(unavailable(
                "The referenced board content revision is unavailable",
            ));
        }
        if reference.content_revision == document.revision {
            if !content_is_present(document, reference) {
                return Err(unavailable(
                    "The referenced board item or explanation block is unavailable",
                ));
            }
        } else {
            let path = self
                .board_path(&document.board_id)?
                .join("revisions")
                .join(format!("{}.json", reference.content_revision));
            if !path.is_file() {
                return Err(unavailable(
                    "The referenced board content revision is unavailable",
                ));
            }
            let snapshot =
                self.read_snapshot(&BoardSummary::from(document), reference.content_revision)?;
            if !content_is_present(&snapshot, reference) {
                return Err(unavailable(
                    "The referenced board item or explanation block is unavailable",
                ));
            }
        }
        Ok(())
    }

    fn commit(&self, document: &BoardDocument) -> Result<(), ToolError> {
        let directory = self.board_path(&document.board_id)?;
        // A crash before head replacement leaves only an uncommitted snapshot.
        // It may be replaced on retry; snapshots reachable from head are immutable.
        self.write_json(
            &directory
                .join("revisions")
                .join(format!("{}.json", document.revision)),
            document,
        )?;
        self.write_json(&directory.join("head.json"), &BoardSummary::from(document))
    }

    /// Reads the notebook once for this board. Deleted notes keep their layout
    /// placeholder but never expose an old body from a retained board revision.
    fn hydrate_notes(&self, mut document: BoardDocument) -> Result<BoardDocument, ToolError> {
        if !document.items.iter().any(|item| matches!(item.target, BoardTarget::Note { .. })) {
            return Ok(document);
        }
        let notebook = crate::workspace_notes::WorkspaceNoteStore::open(
            &self.private_root, self.learning_workspace_id.as_deref(), self.private,
        )?.list()?;
        let notes: BTreeMap<_, _> = notebook.notes.iter().map(|note| (note.note_id.as_str(), note)).collect();
        let mut note_items = BTreeMap::<&str, Vec<&str>>::new();
        for item in &mut document.items {
            let BoardTarget::Note { note_id, missing } = &mut item.target else { continue };
            if let Some(note) = notes.get(note_id.as_str()) {
                *missing = false;
                item.title = note.title.clone();
                item.readable_content = note.content.clone();
                item.source_bindings = note.source_bindings.clone();
                note_items.entry(note_id.as_str()).or_default().push(item.item_id.as_str());
            } else {
                *missing = true;
                item.title = "笔记已删除".into();
                item.readable_content.clear();
                item.source_bindings.clear();
            }
        }
        for relation in &notebook.relations {
            let (Some(from), Some(to)) = (note_items.get(relation.from_note_id.as_str()), note_items.get(relation.to_note_id.as_str())) else { continue };
            for from_id in from {
                for to_id in to {
                    document.links.push(BoardLink {
                        link_id: format!("note-relation-{}-{}-{}", relation.relation_id, from_id, to_id),
                        from: BoardVisualTarget::Item { item_id: (*from_id).into() },
                        to: BoardVisualTarget::Item { item_id: (*to_id).into() },
                        label: relation.label.clone(), based_on: vec![],
                        note_relation_id: Some(relation.relation_id.clone()), reason: Some(relation.reason.clone()),
                    });
                }
            }
        }
        Ok(document)
    }

    fn write_json(&self, path: &Path, value: &impl Serialize) -> Result<(), ToolError> {
        if self.private {
            let parent = path.parent().expect("board file has a parent");
            ReaderPrivateStorageGate::create_dir_all(parent).map_err(storage)?;
            ReaderPrivateStorageGate::secure_directory(parent)?;
        }
        crate::persist_document_atomically(path, value, self.private)
    }
}

fn normalize_note_item(item: &mut BoardItem) {
    if let BoardTarget::Note { missing, .. } = &mut item.target {
        *missing = false;
        item.title = "笔记".into();
        item.readable_content.clear();
        item.source_bindings.clear();
    }
}

/// Board revisions persist only note identities. Hydrated text and shared
/// relationship projections must never become stale retained snapshots.
fn normalize_note_references(document: &mut BoardDocument) {
    for item in &mut document.items { normalize_note_item(item); }
    document.links.retain(|link| link.note_relation_id.is_none());
}

/// The original whole-answer address remains valid after fine blocks are added.
pub fn resolve_explanation_block(
    item: &BoardItem,
    block_id: &str,
) -> Option<BoardExplanationBlock> {
    if item.block_id.as_deref() == Some(block_id)
        || (item.block_id.is_none()
            && matches!(item.target, BoardTarget::Answer { .. })
            && block_id == format!("block-{}", item.item_id))
    {
        return Some(BoardExplanationBlock {
            block_id: block_id.into(),
            markdown: item.readable_content.clone(),
            answer_parts: match &item.target {
                BoardTarget::Answer { answer_parts, .. } => answer_parts.clone(),
                _ => vec![serde_json::json!({"kind":"markdown","text":item.readable_content})],
            },
        });
    }
    item.explanation_blocks
        .iter()
        .find(|block| block.block_id == block_id)
        .cloned()
}

fn content_is_present(document: &BoardDocument, reference: &BoardContentRef) -> bool {
    document
        .items
        .iter()
        .find(|item| item.item_id == reference.item_id)
        .is_some_and(|item| {
            reference
                .block_id
                .as_ref()
                .is_none_or(|id| resolve_explanation_block(item, id).is_some())
        })
}

fn validate_visual_target(
    document: &BoardDocument,
    target: &BoardVisualTarget,
) -> Result<(), ToolError> {
    let exists = match target {
        BoardVisualTarget::Item { item_id } => document.items.iter().any(|item| &item.item_id == item_id),
        BoardVisualTarget::Link { link_id } => document.links.iter().any(|link| &link.link_id == link_id),
        BoardVisualTarget::TopicEdge { child_item_id } => board_topic_parent(document, child_item_id).is_some(),
        BoardVisualTarget::PresentationAnchor { item_id, reference, anchor_id } => {
            !anchor_id.trim().is_empty() && document.items.iter().any(|item| &item.item_id == item_id
                && matches!(&item.target, BoardTarget::Presentation { receipt, .. } if &receipt.reference == reference))
        }
    };
    if exists {
        Ok(())
    } else {
        Err(invalid(
            "A board visual target is unavailable or has a different presentation version",
        ))
    }
}

fn validate_link_path(
    document: &BoardDocument,
    link_id: &str,
    visiting: &mut BTreeSet<String>,
) -> Result<(), ToolError> {
    if !visiting.insert(link_id.into()) {
        return Err(invalid(
            "Link endpoints must resolve to board items without a circular link reference",
        ));
    }
    let link = document
        .links
        .iter()
        .find(|link| link.link_id == link_id)
        .ok_or_else(|| invalid("A board link endpoint is unavailable"))?;
    for target in [&link.from, &link.to] {
        if let BoardVisualTarget::Link { link_id } = target {
            validate_link_path(document, link_id, visiting)?;
        }
    }
    visiting.remove(link_id);
    Ok(())
}

fn prune_removed_targets(document: &mut BoardDocument) {
    loop {
        let removed: BTreeSet<_> = document
            .links
            .iter()
            .filter(|link| {
                validate_visual_target(document, &link.from).is_err()
                    || validate_visual_target(document, &link.to).is_err()
            })
            .map(|link| link.link_id.clone())
            .collect();
        if removed.is_empty() {
            break;
        }
        document
            .links
            .retain(|link| !removed.contains(&link.link_id));
    }
    let retained_mappings = document
        .mappings
        .iter()
        .filter(|mapping| {
            content_is_present(document, &mapping.explanation)
                && validate_visual_target(document, &mapping.visual).is_ok()
        })
        .cloned()
        .collect();
    document.mappings = retained_mappings;
    let mut routes = document.routes.clone();
    for route in &mut routes {
        for step in &mut route.steps {
            if step
                .explanation
                .as_ref()
                .is_some_and(|reference| !content_is_present(document, reference))
            {
                step.explanation = None;
            }
            step.reveal_targets
                .retain(|target| validate_visual_target(document, target).is_ok());
            step.focus_targets
                .retain(|target| validate_visual_target(document, target).is_ok());
        }
        route.steps.retain(|step| {
            step.explanation.is_some()
                || !step.reveal_targets.is_empty()
                || !step.focus_targets.is_empty()
        });
    }
    routes.retain(|route| !route.steps.is_empty());
    document.routes = routes;
}

fn normalize_view(document: &BoardDocument, view: &mut BoardView) {
    view.folded_topic_ids.retain(|id| document.items.iter().any(|item| &item.item_id == id && matches!(item.target, BoardTarget::Topic { .. })));
    view.selected_item_ids.retain(|id| document.items.iter().any(|item| &item.item_id == id));
    if view
        .selected_item_id
        .as_ref()
        .is_some_and(|id| !document.items.iter().any(|item| &item.item_id == id))
    {
        view.selected_item_id = None;
    }
    if view
        .reading_item_id
        .as_ref()
        .is_some_and(|id| !document.items.iter().any(|item| &item.item_id == id))
    {
        view.reading_item_id = None;
    }
    if view.selected_block_id.as_ref().is_some_and(|block_id| {
        !document
            .items
            .iter()
            .any(|item| resolve_explanation_block(item, block_id).is_some())
    }) {
        view.selected_block_id = None;
    }
    if let Some(playback) = &mut view.playback {
        if let Some(route) = document
            .routes
            .iter()
            .find(|route| route.route_id == playback.route_id)
        {
            if playback
                .step_id
                .as_ref()
                .is_some_and(|id| !route.steps.iter().any(|step| &step.step_id == id))
            {
                playback.step_id = None;
            }
        } else {
            view.playback = None;
        }
    }
}

fn assign_answer_block(item: &mut BoardItem) {
    if item.block_id.is_none() && matches!(item.target, BoardTarget::Answer { .. }) {
        // Existing WB1 answers also have stable item identities. Deriving their
        // one whole-answer block requires no text rewrite or migration job.
        item.block_id = Some(format!("block-{}", item.item_id));
    }
}

fn assign_answer_blocks(document: &mut BoardDocument) {
    for item in &mut document.items {
        assign_answer_block(item);
    }
}

fn validate_geometry(item: &BoardItem) -> Result<(), ToolError> {
    if !item.position.x.is_finite()
        || !item.position.y.is_finite()
        || !item.size.width.is_finite()
        || !item.size.height.is_finite()
        || item.size.width <= 0.0
        || item.size.height <= 0.0
    {
        return Err(invalid(
            "Board items need finite positions and positive sizes",
        ));
    }
    Ok(())
}

fn rectangles_overlap(left: &BoardItem, right: &BoardItem) -> bool {
    left.position.x < right.position.x + right.size.width + 16.0
        && left.position.x + left.size.width + 16.0 > right.position.x
        && left.position.y < right.position.y + right.size.height + 16.0
        && left.position.y + left.size.height + 16.0 > right.position.y
}

fn searchable_fields(item: &BoardItem) -> Vec<(&'static str, &str)> {
    match &item.target {
        BoardTarget::Understanding {
            learner_quote,
            interpretation,
            ..
        } => vec![
            ("你的原话：", learner_quote),
            ("系统解释：", interpretation),
            ("理解条目：", &item.title),
        ],
        BoardTarget::Answer { question, .. } => vec![
            ("", &item.readable_content),
            ("原问题：", question),
            ("", &item.title),
        ],
        BoardTarget::Drawing {
            graphic: BoardGraphic::Text { text } | BoardGraphic::Shape { text, .. },
            ..
        } => vec![("", text), ("", &item.readable_content), ("", &item.title)],
        _ => vec![("", &item.readable_content), ("", &item.title)],
    }
}

fn search_hit(
    document: &BoardDocument,
    item: Option<&BoardItem>,
    snippet: String,
) -> BoardSearchHit {
    BoardSearchHit {
        board_id: document.board_id.clone(),
        board_revision: document.revision,
        board_title: document.title.clone(),
        item_id: item.map(|item| item.item_id.clone()),
        block_id: item.and_then(|item| item.block_id.clone()),
        kind: item
            .map(|item| match &item.target {
                BoardTarget::Note { .. } => BoardSearchKind::Note,
                BoardTarget::Prebuild { .. } | BoardTarget::Artifact { .. } => BoardSearchKind::Prebuild,
                BoardTarget::Answer { .. } => BoardSearchKind::Answer,
                BoardTarget::Presentation { .. } => BoardSearchKind::Presentation,
                BoardTarget::Understanding { .. } => BoardSearchKind::Understanding,
                BoardTarget::Drawing { .. } => BoardSearchKind::Drawing,
                BoardTarget::Topic { .. } => BoardSearchKind::Topic,
                BoardTarget::PersonalNote { .. } => BoardSearchKind::PersonalNote,
            })
            .unwrap_or(BoardSearchKind::Board),
        title: item
            .map(|item| item.title.as_str())
            .unwrap_or(&document.title)
            .into(),
        snippet,
    }
}

fn snippet(text: &str, query: &str) -> String {
    let folded = text.to_lowercase();
    let matched = folded
        .find(query)
        .map(|index| folded[..index].chars().count())
        .unwrap_or(0);
    let start = matched.saturating_sub(48);
    let excerpt: String = text
        .chars()
        .skip(start)
        .take(160)
        .map(|character| {
            if character == '\n' || character == '\r' {
                ' '
            } else {
                character
            }
        })
        .collect();
    format!(
        "{}{}{}",
        if start > 0 { "…" } else { "" },
        excerpt,
        if text.chars().count() > start + 160 {
            "…"
        } else {
            ""
        }
    )
}

fn delivery_for_identity(
    document: &BoardDocument,
    session_id: &str,
    turn_id: &str,
    commit_id: Option<&str>,
) -> Result<Option<BoardDelivery>, ToolError> {
    let same_turn = |receipt: &&BoardDelivery| {
        receipt.origin_session_id == session_id && receipt.origin_turn_id == turn_id
    };
    if let Some(receipt) = document
        .deliveries
        .iter()
        .filter(same_turn)
        .find(|receipt| receipt.commit_id.as_deref() == commit_id)
    {
        return Ok(Some(receipt.clone()));
    }
    if document
        .deliveries
        .iter()
        .filter(same_turn)
        .any(|receipt| receipt.commit_id.is_some() != commit_id.is_some())
    {
        return Err(invalid(if commit_id.is_none() {
            "This turn already committed board parts; reference those deliveries instead of copying the final answer"
        } else {
            "This turn already has a whole-answer board delivery"
        }));
    }
    Ok(None)
}

fn access() -> Result<MutexGuard<'static, ()>, ToolError> {
    BOARD_ACCESS
        .lock()
        .map_err(|_| storage("Board storage lock is unavailable"))
}

fn validate_id(id: &str) -> Result<(), ToolError> {
    if id.is_empty()
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Err(invalid(
            "Board storage identifiers must use letters, numbers, dashes or underscores",
        ));
    }
    Ok(())
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, ToolError> {
    let bytes = std::fs::read(path).map_err(storage)?;
    serde_json::from_slice(&bytes).map_err(storage)
}

fn failure(code: &str, category: &str, message: impl ToString) -> ToolError {
    ToolError {
        error_code: code.into(),
        category: category.into(),
        message: message.to_string(),
    }
}
fn storage(message: impl ToString) -> ToolError {
    failure("BOARD_STORAGE_ERROR", "internal", message)
}
fn invalid(message: &str) -> ToolError {
    failure("INVALID_BOARD", "validation", message)
}
fn conflict(message: &str) -> ToolError {
    failure("BOARD_REVISION_CONFLICT", "conflict", message)
}
fn not_found(message: &str) -> ToolError {
    failure("BOARD_NOT_FOUND", "not_found", message)
}

#[cfg(test)]
#[path = "boards/wb3_tests.rs"]
mod wb3_tests;

#[cfg(test)]
#[path = "boards/wb4_tests.rs"]
mod wb4_tests;

#[cfg(test)]
#[path = "boards/workspace_note_tests.rs"]
mod workspace_note_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn board_link_defaults_omitted_provenance_without_dropping_explicit_references() {
        let mut value = json!({
            "link_id": "changes-original", "label": "修改原数组",
            "from": {"kind": "item", "item_id": "mutable-slice"},
            "to": {"kind": "item", "item_id": "array-cell"}
        });
        let link: BoardLink = serde_json::from_value(value.clone()).unwrap();
        assert!(link.based_on.is_empty());
        value["based_on"] = json!([{"item_id": "answer-1", "block_id": "map-mut", "content_revision": 2}]);
        let link: BoardLink = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(&link).unwrap()["based_on"], value["based_on"]);
    }

    pub(super) fn answer() -> BoardItem {
        BoardItem {
            item_id: "answer-1".into(), title: "Why total time?".into(),
            reply_to_item_id: None, block_id: None,
            explanation_blocks: vec![],
            readable_content: "The complete explanation, including stopped time.\n\nA second paragraph retained in full.".into(),
            position: BoardPosition { x: 40.0, y: 60.0 }, size: BoardSize { width: 360.0, height: 280.0 },
            created_at: "2026-10-25T10:00:00Z".into(),
            source_bindings: vec![NoteSourceBinding {
                source_ref_id: "source-1".into(), book_id: "physics".into(),
                publication_id: Some("release-1".into()), source_revision: Some("source-v1".into()),
                evidence_range: Some(read_tools::EvidenceRange { start_lid: "1.1".into(), end_lid: "1.2".into(), ranges: vec![] }),
                media: None, music: None, evidence_text_digest: "existing-digest".into(),
                label_snapshot: "Average speed".into(), preview_snapshot: "All elapsed time counts.".into(),
            }],
            target: BoardTarget::Answer { session_id: "chat-1".into(), turn_id: "turn-1".into(),
                question: "Does stopped time count?".into(), answer_parts: vec![json!({"kind":"markdown","text":"full text"})] },
        }
    }

    pub(super) fn document(workspace: Option<&str>) -> BoardDocument {
        BoardDocument {
            board_id: "board-1".into(),
            learning_workspace_id: workspace.map(str::to_owned),
            book_id: "physics".into(),
            publication_id: Some("release-1".into()),
            source_revision: Some("source-v1".into()),
            title: "Average speed".into(),
            revision: 0,
            created_at: "2026-10-25T10:00:00Z".into(),
            updated_at: "2026-10-25T10:00:00Z".into(),
            items: vec![answer()],
            deliveries: vec![],
            links: vec![],
            mappings: vec![],
            routes: vec![],
            mind_maps: vec![],
            edit_history: BoardEditHistory::default(),
        }
    }

    fn receipt(state: u32) -> NotePresentationReceipt {
        NotePresentationReceipt {
            session_id: "chat-1".into(),
            turn_id: "turn-1".into(),
            reference: crate::NotePresentationRef {
                presentation_id: "scene-1".into(),
                revision: 1,
            },
            state_revision: state,
            saved_state_ref: format!("saved-{state}"),
        }
    }

    #[test]
    fn reopen_keeps_full_content_identity_sources_and_historical_revisions() {
        let temp = tempfile::tempdir().unwrap();
        let store = BoardStore::open(temp.path(), Some("workspace-1"), false).unwrap();
        let mut initial = document(Some("workspace-1"));
        let mut prebuild = answer();
        prebuild.item_id = "prebuild-1".into();
        prebuild.target = BoardTarget::Prebuild {
            source_id: "physics".into(),
            object_id: "average-speed".into(),
            object_revision: 3,
            map_revision: "map-v2".into(),
            object_snapshot: json!({"kind":"concept","label":"Average speed"}),
        };
        let mut presentation = answer();
        presentation.item_id = "presentation-1".into();
        presentation.target = BoardTarget::Presentation {
            source_book_id: None,
            receipt: receipt(1),
        };
        let mut understanding = answer();
        understanding.item_id = "understanding-1".into();
        understanding.target = BoardTarget::Understanding {
            evidence_id: "evidence-1".into(),
            source_id: "physics".into(),
            source_revision: "source-v1".into(),
            object_id: Some("average-speed".into()),
            object_revision: Some(3),
            learner_quote: "I should include the stop.".into(),
            interpretation: "The learner accounts for total time in this example.".into(),
            evidence_snapshot: json!({"evidence_id":"evidence-1","nature":"performance"}),
        };
        initial
            .items
            .extend([prebuild, presentation, understanding]);
        let first = store.create(initial).unwrap();
        let mut next = first.clone();
        next.items[0].position.x = 420.0;
        next.title = "Speed and stopped time".into();
        let second = store.update(next, 1, "2026-10-25T11:00:00Z").unwrap();
        drop(store);
        let reopened = BoardStore::open(temp.path(), Some("workspace-1"), false).unwrap();
        assert_eq!(reopened.read("board-1").unwrap(), second);
        assert_eq!(reopened.read_revision("board-1", 1).unwrap(), first);
        assert_eq!(reopened.list().unwrap(), vec![BoardSummary::from(&second)]);
        assert_eq!(second.items[0].item_id, first.items[0].item_id);
        assert_eq!(
            second.items[0].readable_content,
            first.items[0].readable_content
        );
    }

    #[test]
    fn account_and_learning_workspace_scopes_are_independent() {
        let temp = tempfile::tempdir().unwrap();
        let owner = temp.path().join("owner");
        let other = temp.path().join("other");
        let store = BoardStore::open(&owner, Some("workspace-1"), false).unwrap();
        store.create(document(Some("workspace-1"))).unwrap();
        for (root, workspace) in [
            (&owner, Some("workspace-2")),
            (&owner, None),
            (&other, Some("workspace-1")),
        ] {
            let scoped = BoardStore::open(root, workspace, false).unwrap();
            assert!(scoped.list().unwrap().is_empty());
            assert_eq!(
                scoped.read("board-1").unwrap_err().error_code,
                "BOARD_NOT_FOUND"
            );
        }
        assert_eq!(
            store.create(document(None)).unwrap_err().error_code,
            "INVALID_BOARD"
        );
    }

    #[test]
    fn stale_update_and_removal_do_not_overwrite_newer_content() {
        let temp = tempfile::tempdir().unwrap();
        let first_window = BoardStore::open(temp.path(), None, false).unwrap();
        let second_window = BoardStore::open(temp.path(), None, false).unwrap();
        let original = first_window.create(document(None)).unwrap();
        let mut changed = original.clone();
        changed.title = "New title".into();
        let saved = first_window.update(changed, 1, "later").unwrap();
        assert_eq!(
            second_window
                .update(original, 1, "later-again")
                .unwrap_err()
                .error_code,
            "BOARD_REVISION_CONFLICT"
        );
        assert_eq!(
            second_window.remove("board-1", 1).unwrap_err().error_code,
            "BOARD_REVISION_CONFLICT"
        );
        assert_eq!(second_window.read("board-1").unwrap(), saved);
        second_window.remove("board-1", 2).unwrap();
        assert!(first_window.list().unwrap().is_empty());
        assert_eq!(
            first_window.read("board-1").unwrap_err().error_code,
            "BOARD_NOT_FOUND"
        );
    }

    #[test]
    fn layout_edit_cannot_replace_retained_answer_or_sources() {
        let temp = tempfile::tempdir().unwrap();
        let store = BoardStore::open(temp.path(), None, false).unwrap();
        let original = store.create(document(None)).unwrap();
        let mut rewritten = original.clone();
        rewritten.items[0].readable_content = "Invented replacement".into();
        assert_eq!(
            store.update(rewritten, 1, "later").unwrap_err().error_code,
            "INVALID_BOARD"
        );
        assert_eq!(store.read("board-1").unwrap(), original);
    }

    #[test]
    fn camera_and_scene_reopen_without_content_revision_or_receipt_loss() {
        let temp = tempfile::tempdir().unwrap();
        let store = BoardStore::open(temp.path(), None, false).unwrap();
        let mut initial = document(None);
        let mut scene = answer();
        scene.item_id = "presentation-1".into();
        scene.target = BoardTarget::Presentation {
            source_book_id: None,
            receipt: receipt(1),
        };
        initial.items.push(scene);
        store.create(initial).unwrap();
        let mut view = BoardView {
            x: -200.0,
            y: 35.0,
            zoom: 0.75,
            selected_item_id: Some("presentation-1".into()),
            ..Default::default()
        };
        view.presentation_states
            .insert("presentation-1".into(), receipt(2));
        store.save_view("board-1", &view).unwrap();
        view.x = -240.0;
        view.presentation_states.clear();
        store.save_view("board-1", &view).unwrap();
        let reopened = BoardStore::open(temp.path(), None, false).unwrap();
        assert_eq!(reopened.read("board-1").unwrap().revision, 1);
        let saved_view = reopened.view("board-1").unwrap();
        assert_eq!(saved_view.x, -240.0);
        assert_eq!(
            saved_view.presentation_states.get("presentation-1"),
            Some(&receipt(2))
        );
    }

    #[test]
    fn malformed_head_is_reported_instead_of_looking_like_an_empty_library() {
        let temp = tempfile::tempdir().unwrap();
        let store = BoardStore::open(temp.path(), None, false).unwrap();
        store.create(document(None)).unwrap();
        std::fs::write(
            store.board_path("board-1").unwrap().join("head.json"),
            "{broken",
        )
        .unwrap();
        assert_eq!(store.list().unwrap_err().error_code, "BOARD_STORAGE_ERROR");
        assert_eq!(
            store.read("board-1").unwrap_err().error_code,
            "BOARD_STORAGE_ERROR"
        );
    }

    #[test]
    fn failed_head_commit_keeps_previous_revision_readable_and_retryable() {
        let temp = tempfile::tempdir().unwrap();
        let store = BoardStore::open(temp.path(), None, false).unwrap();
        let original = store.create(document(None)).unwrap();
        let blocker = store
            .board_path("board-1")
            .unwrap()
            .join("head.replace.tmp");
        std::fs::create_dir(&blocker).unwrap();
        let mut candidate = original.clone();
        candidate.title = "Changed".into();
        assert!(store.update(candidate.clone(), 1, "later").is_err());
        assert_eq!(store.read("board-1").unwrap(), original);
        assert_eq!(
            store.read_revision("board-1", 2).unwrap_err().error_code,
            "BOARD_NOT_FOUND"
        );
        std::fs::remove_dir(blocker).unwrap();
        let saved = store.update(candidate, 1, "retry").unwrap();
        assert_eq!(saved.revision, 2);
        assert_eq!(store.read("board-1").unwrap(), saved);
    }

    fn follow_up(document: &BoardDocument) -> BoardFollowUp {
        BoardFollowUp {
            note_refs: None,
            board_id: document.board_id.clone(),
            board_revision: document.revision,
            item_id: Some(document.items[0].item_id.clone()),
            block_id: document.items[0].block_id.clone(),
            presentation: None,
            intent: None,
            regenerate_from: None,
        }
    }

    fn next_answer() -> BoardItem {
        let mut item = answer();
        item.item_id = "answer-2".into();
        item.target = BoardTarget::Answer {
            session_id: "chat-2".into(),
            turn_id: "turn-2".into(),
            question: "Why does this example include the stop?".into(),
            answer_parts: vec![json!({"kind":"markdown","text":item.readable_content})],
        };
        item
    }

    fn root_follow_up(document: &BoardDocument) -> BoardFollowUp {
        BoardFollowUp { note_refs:None, board_id:document.board_id.clone(),board_revision:document.revision,
            item_id:None,block_id:None,presentation:None,intent:None,regenerate_from:None }
    }

    #[test]
    fn board_root_delivery_keeps_no_parent_and_reopens_idempotently() {
        let temp = tempfile::tempdir().unwrap();
        let store = BoardStore::open(temp.path(), None, false).unwrap();
        let mut empty = document(None);
        empty.items.clear();
        let original = store.create(empty).unwrap();
        let root = root_follow_up(&original);
        let receipt = store.append_delivery(&root,"chat-2","turn-2",vec![next_answer()],"finished").unwrap();
        assert_eq!(receipt.parent_item_id,None);
        let saved = store.read("board-1").unwrap();
        assert_eq!(saved.items.len(),1);
        assert_eq!(saved.items[0].reply_to_item_id,None);
        assert_eq!(saved.items[0].readable_content,next_answer().readable_content);
        let reopened = BoardStore::open(temp.path(), None, false).unwrap();
        assert_eq!(reopened.append_delivery(&root,"chat-2","turn-2",vec![],"retry").unwrap(),receipt);
        assert_eq!(reopened.read("board-1").unwrap(),saved);
        let mut invalid_root = root;
        invalid_root.block_id = Some("block-answer-2".into());
        assert!(reopened.append_delivery(&invalid_root,"other-chat","other-turn",vec![next_answer()],"invalid").is_err());
    }

    #[test]
    fn board_authored_root_updates_native_drawing_in_same_revision_and_preserves_old_snapshot() {
        let temp = tempfile::tempdir().unwrap();
        let store = BoardStore::open(temp.path(), None, false).unwrap();
        let mut initial = document(None);
        let mut drawing = answer();
        drawing.item_id = "drawing-1".into();
        drawing.readable_content = "原来的状态图".into();
        drawing.target = BoardTarget::Drawing { session_id:"original-chat".into(),turn_id:"original-turn".into(),
            graphic:BoardGraphic::Text {text:"状态".into()},based_on:vec![] };
        initial.items.push(drawing);
        let original = store.create(initial).unwrap();
        let root = root_follow_up(&original);
        let mut revised = original.items[1].clone();
        revised.title = "上下文与状态".into();
        revised.readable_content = "状态属于上下文的一部分".into();
        revised.position = BoardPosition {x:480.0,y:160.0};
        if let BoardTarget::Drawing { graphic, .. } = &mut revised.target {
            *graphic = BoardGraphic::Shape {shape:BoardShape::Rectangle,text:"状态 ⊂ 上下文".into(),color:"#2563eb".into()};
        }
        let receipt = store.append_authored_part(&root,1,"chat-2","turn-2","root-part",
            vec![next_answer()],vec![revised.clone()],vec![],vec![],vec![],"revised").unwrap();
        assert_eq!(receipt.parent_item_id,None);
        assert_eq!(receipt.item_ids,vec!["answer-2","drawing-1"]);
        let saved = store.read("board-1").unwrap();
        assert_eq!(saved.revision,2);
        assert_eq!(saved.items[1],revised);
        assert_eq!(saved.items[0],original.items[0]);
        assert_eq!(saved.items[2].reply_to_item_id,None);
        assert_eq!(store.read_revision("board-1",1).unwrap(),original);
        assert_eq!(store.append_authored_part(&root,1,"chat-2","turn-2","root-part",
            vec![],vec![],vec![],vec![],vec![],"retry").unwrap(),receipt);
        assert_eq!(store.append_authored_part(&root,1,"chat-2","turn-2","stale-part",
            vec![next_answer()],vec![revised],vec![],vec![],vec![],"stale").unwrap_err().error_code,"BOARD_REVISION_CONFLICT");
        let mut rewrite = original.items[0].clone();
        rewrite.readable_content = "替换历史答案".into();
        assert!(store.append_authored_part(&root,2,"chat-2","turn-2","rewrite",
            vec![next_answer()],vec![rewrite],vec![],vec![],vec![],"invalid").is_err());
        assert_eq!(store.read("board-1").unwrap(),saved);
    }

    #[test]
    fn board_layout_can_edit_native_geometry_but_cannot_change_original_authorship() {
        let temp = tempfile::tempdir().unwrap();
        let store = BoardStore::open(temp.path(), None, false).unwrap();
        let mut initial = document(None);
        initial.items[0].target = BoardTarget::Drawing {session_id:"original-chat".into(),turn_id:"original-turn".into(),
            graphic:BoardGraphic::Text{text:"原文".into()},based_on:vec![]};
        let original = store.create(initial).unwrap();
        let mut edited = original.clone();
        if let BoardTarget::Drawing {graphic,..} = &mut edited.items[0].target {
            *graphic = BoardGraphic::Text {text:"修改后的原生文字".into()};
        }
        edited.items[0].readable_content = "修改后的原生文字".into();
        let saved = store.update(edited,1,"edited").unwrap();
        let mut forged = saved.clone();
        if let BoardTarget::Drawing {session_id,..} = &mut forged.items[0].target {
            *session_id = "replacement-author".into();
        }
        assert!(store.update(forged,2,"forged").is_err());
        assert_eq!(store.read("board-1").unwrap(),saved);
        assert_eq!(store.read_revision("board-1",1).unwrap(),original);
    }

    #[test]
    fn delivery_uses_frozen_target_and_current_layout_and_is_idempotent_after_reopen() {
        let temp = tempfile::tempdir().unwrap();
        let store = BoardStore::open(temp.path(), None, false).unwrap();
        let mut original = store.create(document(None)).unwrap();
        let selected = follow_up(&original);
        original.items[0].position = BoardPosition { x: 760.0, y: 300.0 };
        let moved = store.update(original, 1, "moved").unwrap();
        let receipt = store
            .append_delivery(
                &selected,
                "chat-2",
                "turn-2",
                vec![next_answer()],
                "completed",
            )
            .unwrap();
        let current = store.read("board-1").unwrap();
        assert_eq!(current.items[0], moved.items[0]);
        assert_eq!((receipt.before_revision, receipt.after_revision), (2, 3));
        assert_eq!(receipt.item_ids, vec!["answer-2"]);
        assert_eq!(receipt.block_ids, vec!["block-answer-2"]);
        assert_eq!(
            current.items[1].reply_to_item_id.as_deref(),
            Some("answer-1")
        );
        assert!(current.items[1].position.x > moved.items[0].position.x);
        let reopened = BoardStore::open(temp.path(), None, false).unwrap();
        let mut retry = next_answer();
        retry.item_id = "new-id-on-retry".into();
        assert_eq!(
            reopened
                .append_delivery(&selected, "chat-2", "turn-2", vec![retry], "retry")
                .unwrap(),
            receipt
        );
        assert_eq!(reopened.read("board-1").unwrap(), current);
    }

    #[test]
    fn retry_does_not_recreate_delivery_items_the_reader_removed() {
        let temp = tempfile::tempdir().unwrap();
        let store = BoardStore::open(temp.path(), None, false).unwrap();
        let original = store.create(document(None)).unwrap();
        let selected = follow_up(&original);
        let receipt = store
            .append_delivery(
                &selected,
                "chat-2",
                "turn-2",
                vec![next_answer()],
                "completed",
            )
            .unwrap();
        let mut current = store.read("board-1").unwrap();
        current.items.retain(|item| item.item_id != "answer-2");
        let removed = store.update(current, 2, "removed").unwrap();
        assert_eq!(
            store
                .append_delivery(&selected, "chat-2", "turn-2", vec![next_answer()], "retry")
                .unwrap(),
            receipt
        );
        assert_eq!(store.read("board-1").unwrap(), removed);
        let mut rewritten_receipts = removed.clone();
        rewritten_receipts.deliveries.clear();
        assert_eq!(
            store
                .update(rewritten_receipts, 3, "rewrite")
                .unwrap_err()
                .error_code,
            "INVALID_BOARD"
        );
    }

    #[test]
    fn delivery_cannot_revive_a_deleted_parent_or_board() {
        let temp = tempfile::tempdir().unwrap();
        let store = BoardStore::open(temp.path(), None, false).unwrap();
        let mut original = store.create(document(None)).unwrap();
        let selected = follow_up(&original);
        original.items.clear();
        store.update(original, 1, "removed-parent").unwrap();
        assert_eq!(
            store
                .append_delivery(
                    &selected,
                    "chat-2",
                    "turn-2",
                    vec![next_answer()],
                    "completed"
                )
                .unwrap_err()
                .error_code,
            "BOARD_NOT_FOUND"
        );
        assert!(store.read("board-1").unwrap().items.is_empty());
        store.remove("board-1", 2).unwrap();
        assert_eq!(
            store
                .append_delivery(
                    &selected,
                    "chat-2",
                    "turn-2",
                    vec![next_answer()],
                    "completed"
                )
                .unwrap_err()
                .error_code,
            "BOARD_NOT_FOUND"
        );
        assert!(store.list().unwrap().is_empty());
    }

    #[test]
    fn in_flight_layout_replays_only_across_new_deliveries() {
        let temp = tempfile::tempdir().unwrap();
        let store = BoardStore::open(temp.path(), None, false).unwrap();
        let original = store.create(document(None)).unwrap();
        let selected = follow_up(&original);
        let mut dragging = original.clone();
        dragging.items[0].position.x = 680.0;
        let delivery = store
            .append_delivery(
                &selected,
                "chat-2",
                "turn-2",
                vec![next_answer()],
                "completed",
            )
            .unwrap();
        let delivered = store.read("board-1").unwrap().items[1].clone();
        let saved = store.update(dragging, 1, "drop").unwrap();
        assert_eq!(saved.items[0].position.x, 680.0);
        assert_eq!(saved.items[1], delivered);
        assert_eq!(saved.deliveries, vec![delivery]);
        assert_eq!(saved.revision, 3);
        let mut stale = original;
        stale.items[0].position.y = 500.0;
        assert_eq!(
            store.update(stale, 1, "other-drop").unwrap_err().error_code,
            "BOARD_REVISION_CONFLICT"
        );
        assert_eq!(store.read("board-1").unwrap(), saved);
    }

    #[test]
    fn stale_layout_does_not_cross_manual_import_or_modify_delivery_receipts() {
        let temp = tempfile::tempdir().unwrap();
        let store = BoardStore::open(temp.path(), None, false).unwrap();
        let original = store.create(document(None)).unwrap();
        let mut imported = original.clone();
        imported.items.push(next_answer());
        store.update(imported, 1, "manual-import").unwrap();
        let mut dragging = original;
        dragging.items[0].position.x = 80.0;
        assert_eq!(
            store.update(dragging, 1, "drop").unwrap_err().error_code,
            "BOARD_REVISION_CONFLICT"
        );
    }

    fn understanding_item() -> BoardItem {
        let mut item = answer();
        item.item_id = "understanding-1".into();
        item.title = "我的理解记录".into();
        item.readable_content = "你的原话：我认为停车不计算。系统解释：需要澄清总时间。".into();
        item.target = BoardTarget::Understanding {
            evidence_id: "evidence-1".into(),
            source_id: "physics".into(),
            source_revision: "source-v1".into(),
            object_id: Some("average-speed".into()),
            object_revision: Some(3),
            learner_quote: "我认为停车不计算".into(),
            interpretation: "需要澄清总时间".into(),
            evidence_snapshot: json!({}),
        };
        item
    }

    #[test]
    fn search_reads_full_text_and_keeps_learner_quotes_separate_from_interpretations() {
        let temp = tempfile::tempdir().unwrap();
        let store = BoardStore::open(temp.path(), Some("workspace-1"), false).unwrap();
        let mut initial = document(Some("workspace-1"));
        initial.items[0].readable_content = format!(
            "{}\n完整正文的远处：NeedleAtTheEnd 应当能直接找到。",
            "很长的正文。".repeat(500)
        );
        initial.items.push(understanding_item());
        let saved = store.create(initial).unwrap();
        let hits = store
            .search("physics", Some("release-1"), "needleattheend", 30)
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].block_id.as_deref(), Some("block-answer-1"));
        assert_eq!(hits[0].item_id.as_deref(), Some("answer-1"));
        assert!(hits[0].snippet.contains("NeedleAtTheEnd"));
        assert_eq!(hits[0].board_revision, saved.revision);
        // PN6: a personal workspace board can be reused from another material.
        assert_eq!(store.search("physics",Some("release-2"),"needleattheend",30).unwrap().len(),1);
        assert_eq!(store.search("other-book",Some("release-1"),"needleattheend",30).unwrap().len(),1);
        let other_workspace=BoardStore::open(temp.path(),Some("workspace-2"),false).unwrap();
        assert!(other_workspace.search("physics",Some("release-1"),"needleattheend",30).unwrap().is_empty());
        let quote = store
            .search("physics", Some("release-1"), "停车不计算", 30)
            .unwrap();
        assert!(quote[0].snippet.starts_with("你的原话："));
        let interpretation = store
            .search("physics", Some("release-1"), "澄清总时间", 30)
            .unwrap();
        assert!(interpretation[0].snippet.starts_with("系统解释："));
        let title = store
            .search("physics", Some("release-1"), "Average speed", 30)
            .unwrap();
        assert_eq!(title[0].kind, BoardSearchKind::Board);
        assert_eq!(title[0].item_id, None);
    }

    #[test]
    fn related_lookup_uses_exact_objects_and_evidence_in_the_current_material() {
        let temp = tempfile::tempdir().unwrap();
        let store = BoardStore::open(temp.path(), None, false).unwrap();
        let mut initial = document(None);
        let mut prebuild = answer();
        prebuild.item_id = "prebuild-1".into();
        prebuild.target = BoardTarget::Prebuild {
            source_id: "physics".into(),
            object_id: "average-speed".into(),
            object_revision: 3,
            map_revision: "old-map".into(),
            object_snapshot: json!({}),
        };
        initial.items.extend([prebuild, understanding_item()]);
        store.create(initial).unwrap();
        let exact = BoardLookupTarget::Prebuild {
            source_id: "physics".into(),
            object_id: "average-speed".into(),
            object_revision: 3,
        };
        let hits = store
            .find_related("physics", Some("release-1"), &exact)
            .unwrap();
        assert_eq!(hits.len(), 2);
        let another_revision = BoardLookupTarget::Prebuild {
            source_id: "physics".into(),
            object_id: "average-speed".into(),
            object_revision: 4,
        };
        assert!(store
            .find_related("physics", Some("release-1"), &another_revision)
            .unwrap()
            .is_empty());
        let evidence = BoardLookupTarget::Understanding {
            evidence_id: "evidence-1".into(),
        };
        let hits = store
            .find_related("physics", Some("release-1"), &evidence)
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].item_id.as_deref(), Some("understanding-1"));
        assert!(store
            .find_related("physics", Some("other-release"), &evidence)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn wb1_answer_gets_a_stable_block_without_rewriting_its_saved_snapshot() {
        let temp = tempfile::tempdir().unwrap();
        let store = BoardStore::open(temp.path(), None, false).unwrap();
        let saved = store.create(document(None)).unwrap();
        let path = store
            .board_path("board-1")
            .unwrap()
            .join("revisions/1.json");
        let mut legacy = json!(saved);
        legacy.as_object_mut().unwrap().remove("deliveries");
        legacy["items"][0]
            .as_object_mut()
            .unwrap()
            .remove("block_id");
        std::fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
        assert_eq!(
            store.read("board-1").unwrap().items[0].block_id.as_deref(),
            Some("block-answer-1")
        );
        assert_eq!(
            store.read_revision("board-1", 1).unwrap().items[0]
                .block_id
                .as_deref(),
            Some("block-answer-1")
        );
        assert_eq!(read_json::<serde_json::Value>(&path).unwrap(), legacy);
    }

    #[test]
    fn failed_delivery_commit_exposes_neither_new_items_nor_receipt_and_can_retry() {
        let temp = tempfile::tempdir().unwrap();
        let store = BoardStore::open(temp.path(), None, false).unwrap();
        let original = store.create(document(None)).unwrap();
        let selected = follow_up(&original);
        let blocker = store
            .board_path("board-1")
            .unwrap()
            .join("head.replace.tmp");
        std::fs::create_dir(&blocker).unwrap();
        assert!(store
            .append_delivery(
                &selected,
                "chat-2",
                "turn-2",
                vec![next_answer()],
                "completed"
            )
            .is_err());
        assert_eq!(store.read("board-1").unwrap(), original);
        std::fs::remove_dir(blocker).unwrap();
        let receipt = store
            .append_delivery(&selected, "chat-2", "turn-2", vec![next_answer()], "retry")
            .unwrap();
        let saved = store.read("board-1").unwrap();
        assert_eq!(saved.items.len(), 2);
        assert_eq!(saved.deliveries, vec![receipt]);
    }
}
