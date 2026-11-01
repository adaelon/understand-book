//! A learning workspace's reusable notes and their explained relationships.
//! Identity survives edits; reading memories and retained board snapshots remain
//! separate records. The authenticated host selects the private root and scope.
use crate::{NoteSourceBinding, ReaderPrivateStorageGate};
use read_tools::ToolError;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use ts_rs::TS;

// The host owns the private-root writer lease. Reloading inside this lock keeps
// read/check/commit atomic for requests from multiple windows of that host.
static NOTE_ACCESS: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub enum WorkspaceNoteAuthor {
    User,
    Assistant,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub enum WorkspaceNoteOriginKind {
    ReadingNote,
    Answer,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct WorkspaceNoteOrigin {
    pub kind: WorkspaceNoteOriginKind,
    pub book_id: String,
    pub publication_id: Option<String>,
    pub session_id: Option<String>,
    pub turn_id: Option<String>,
    pub mem_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct WorkspaceNote {
    pub note_id: String,
    pub learning_workspace_id: Option<String>,
    pub title: String,
    pub content: String,
    pub revision: u32,
    pub created_at: String,
    pub updated_at: String,
    pub author: WorkspaceNoteAuthor,
    pub updated_by: WorkspaceNoteAuthor,
    pub origin: Option<WorkspaceNoteOrigin>,
    pub source_bindings: Vec<NoteSourceBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct WorkspaceNoteRelation {
    pub relation_id: String,
    pub from_note_id: String,
    pub to_note_id: String,
    pub label: String,
    pub reason: String,
    pub revision: u32,
    pub created_at: String,
    pub updated_at: String,
    pub author: WorkspaceNoteAuthor,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../packages/web/src/generated/")]
pub struct WorkspaceNotebook {
    pub notes: Vec<WorkspaceNote>,
    pub relations: Vec<WorkspaceNoteRelation>,
}

#[derive(Debug, Serialize, Deserialize)]
struct NotebookDocument {
    schema_version: u32,
    learning_workspace_id: Option<String>,
    notebook: WorkspaceNotebook,
}

pub struct WorkspaceNoteStore {
    path: PathBuf,
    learning_workspace_id: Option<String>,
    private: bool,
}

impl WorkspaceNoteStore {
    /// `private` enables the existing reader-private storage permission gate;
    /// the authenticated server enforces write access before calling mutations.
    pub fn open(
        private_root: &Path,
        learning_workspace_id: Option<&str>,
        private: bool,
    ) -> Result<Self, ToolError> {
        let directory = match learning_workspace_id {
            Some(id) => {
                validate_id(id)?;
                private_root.join("notes").join("workspaces").join(id)
            }
            None => private_root.join("notes").join("unscoped"),
        };
        Ok(Self {
            path: directory.join("notebook.json"),
            learning_workspace_id: learning_workspace_id.map(str::to_owned),
            private,
        })
    }

    pub fn list(&self) -> Result<WorkspaceNotebook, ToolError> {
        let _access = access()?;
        let mut notebook = self.load()?;
        notebook.notes.sort_by(|a, b| {
            b.updated_at
                .cmp(&a.updated_at)
                .then(a.note_id.cmp(&b.note_id))
        });
        notebook.relations.sort_by(|a, b| {
            b.updated_at
                .cmp(&a.updated_at)
                .then(a.relation_id.cmp(&b.relation_id))
        });
        Ok(notebook)
    }

    pub fn read(&self, note_id: &str) -> Result<WorkspaceNote, ToolError> {
        let _access = access()?;
        self.load()?
            .notes
            .into_iter()
            .find(|note| note.note_id == note_id)
            .ok_or_else(|| not_found("This note is unavailable in the current workspace"))
    }

    /// IDs, timestamps, provenance and author are supplied by the host, using
    /// its normal ID generator and resolved sources rather than request claims.
    pub fn create(&self, mut note: WorkspaceNote) -> Result<WorkspaceNote, ToolError> {
        let _access = access()?;
        self.validate_note(&note)?;
        let mut notebook = self.load()?;
        if notebook
            .notes
            .iter()
            .any(|existing| existing.note_id == note.note_id)
        {
            return Err(conflict("This note already exists"));
        }
        note.revision = 1;
        notebook.notes.push(note.clone());
        self.commit(notebook)?;
        Ok(note)
    }

    /// Ordinary editing changes only the reader's title and text. Source
    /// receipts, original authorship, identity and creation time stay intact.
    pub fn update(
        &self,
        note_id: &str,
        expected_revision: u32,
        title: &str,
        content: &str,
        editor: WorkspaceNoteAuthor,
        now: &str,
    ) -> Result<WorkspaceNote, ToolError> {
        let _access = access()?;
        let mut notebook = self.load()?;
        let note = notebook
            .notes
            .iter_mut()
            .find(|note| note.note_id == note_id)
            .ok_or_else(|| not_found("This note is unavailable in the current workspace"))?;
        require_revision(note.revision, expected_revision)?;
        note.title = title.to_owned();
        if note.content != content {
            note.updated_by = editor;
        }
        note.content = content.to_owned();
        note.updated_at = now.to_owned();
        note.revision += 1;
        self.validate_note(note)?;
        let updated = note.clone();
        self.commit(notebook)?;
        Ok(updated)
    }

    /// Removing a note and all its incident relations is one atomic commit.
    pub fn remove(&self, note_id: &str, expected_revision: u32) -> Result<(), ToolError> {
        let _access = access()?;
        let mut notebook = self.load()?;
        let note = notebook
            .notes
            .iter()
            .find(|note| note.note_id == note_id)
            .ok_or_else(|| not_found("This note is unavailable in the current workspace"))?;
        require_revision(note.revision, expected_revision)?;
        notebook.notes.retain(|note| note.note_id != note_id);
        notebook
            .relations
            .retain(|relation| relation.from_note_id != note_id && relation.to_note_id != note_id);
        self.commit(notebook)
    }

    /// `None` creates a new relationship. `Some(revision)` edits its explanation;
    /// changing endpoints requires explicitly removing and creating a relation.
    pub fn upsert_relation(
        &self,
        mut relation: WorkspaceNoteRelation,
        expected_revision: Option<u32>,
        now: &str,
    ) -> Result<WorkspaceNoteRelation, ToolError> {
        let _access = access()?;
        let mut notebook = self.load()?;
        validate_relation(&relation, &notebook)?;
        match notebook
            .relations
            .iter_mut()
            .find(|item| item.relation_id == relation.relation_id)
        {
            Some(existing) => {
                require_revision(existing.revision, expected_revision.unwrap_or(0))?;
                if relation.from_note_id != existing.from_note_id
                    || relation.to_note_id != existing.to_note_id
                    || relation.author != existing.author
                    || relation.created_at != existing.created_at
                {
                    return Err(invalid(
                        "A relation's endpoints and original authorship cannot be rewritten",
                    ));
                }
                relation.revision = existing.revision + 1;
                relation.updated_at = now.to_owned();
                *existing = relation.clone();
            }
            None => {
                if expected_revision.is_some() {
                    return Err(not_found("This note relation is unavailable"));
                }
                relation.revision = 1;
                relation.updated_at = now.to_owned();
                notebook.relations.push(relation.clone());
            }
        }
        if now.trim().is_empty() {
            return Err(invalid("A note relation needs an update time"));
        }
        self.commit(notebook)?;
        Ok(relation)
    }

    pub fn remove_relation(
        &self,
        relation_id: &str,
        expected_revision: u32,
    ) -> Result<(), ToolError> {
        let _access = access()?;
        let mut notebook = self.load()?;
        let relation = notebook
            .relations
            .iter()
            .find(|relation| relation.relation_id == relation_id)
            .ok_or_else(|| not_found("This note relation is unavailable"))?;
        require_revision(relation.revision, expected_revision)?;
        notebook
            .relations
            .retain(|relation| relation.relation_id != relation_id);
        self.commit(notebook)
    }

    fn load(&self) -> Result<WorkspaceNotebook, ToolError> {
        crate::recover_interrupted_commit(&self.path)?;
        if !self.path.exists() {
            return Ok(WorkspaceNotebook::default());
        }
        let bytes = std::fs::read(&self.path).map_err(storage)?;
        let document: NotebookDocument = serde_json::from_slice(&bytes).map_err(storage)?;
        if document.schema_version != 1
            || document.learning_workspace_id != self.learning_workspace_id
            || document
                .notebook
                .notes
                .iter()
                .any(|note| note.learning_workspace_id != self.learning_workspace_id)
        {
            return Err(storage(
                "The notebook schema or workspace does not match this store",
            ));
        }
        Ok(document.notebook)
    }

    fn commit(&self, notebook: WorkspaceNotebook) -> Result<(), ToolError> {
        if self.private {
            let directory = self.path.parent().expect("notebook file has a parent");
            ReaderPrivateStorageGate::create_dir_all(directory).map_err(storage)?;
            ReaderPrivateStorageGate::secure_directory(directory)?;
        }
        crate::persist_document_atomically(
            &self.path,
            &NotebookDocument {
                schema_version: 1,
                learning_workspace_id: self.learning_workspace_id.clone(),
                notebook,
            },
            self.private,
        )
    }

    fn validate_note(&self, note: &WorkspaceNote) -> Result<(), ToolError> {
        validate_id(&note.note_id)?;
        if note.learning_workspace_id != self.learning_workspace_id {
            return Err(invalid(
                "The note must belong to the current learning workspace",
            ));
        }
        if note.title.trim().is_empty() || note.content.trim().is_empty() {
            return Err(invalid("A note needs both a title and content"));
        }
        if note.created_at.trim().is_empty() || note.updated_at.trim().is_empty() {
            return Err(invalid("A note needs its creation and update times"));
        }
        if let Some(origin) = &note.origin {
            let supplied = |value: &Option<String>| {
                value
                    .as_deref()
                    .is_some_and(|value| !value.trim().is_empty())
            };
            let valid = !origin.book_id.trim().is_empty()
                && match origin.kind {
                    WorkspaceNoteOriginKind::ReadingNote => supplied(&origin.mem_id),
                    WorkspaceNoteOriginKind::Answer => {
                        supplied(&origin.session_id) && supplied(&origin.turn_id)
                    }
                };
            if !valid {
                return Err(invalid(
                    "A note origin needs its resolved material and source identity",
                ));
            }
        }
        Ok(())
    }
}

fn validate_relation(
    relation: &WorkspaceNoteRelation,
    notebook: &WorkspaceNotebook,
) -> Result<(), ToolError> {
    validate_id(&relation.relation_id)?;
    if relation.from_note_id == relation.to_note_id {
        return Err(invalid("A note relation must connect two different notes"));
    }
    if relation.label.trim().is_empty() || relation.reason.trim().is_empty() {
        return Err(invalid("A note relation needs a label and an explanation"));
    }
    if relation.created_at.trim().is_empty() {
        return Err(invalid("A note relation needs its creation time"));
    }
    if !notebook
        .notes
        .iter()
        .any(|note| note.note_id == relation.from_note_id)
        || !notebook
            .notes
            .iter()
            .any(|note| note.note_id == relation.to_note_id)
    {
        return Err(not_found(
            "Both related notes must exist in the current workspace",
        ));
    }
    Ok(())
}

fn require_revision(actual: u32, expected: u32) -> Result<(), ToolError> {
    if actual != expected {
        return Err(conflict(
            "This note or relation changed in another window; reload before editing",
        ));
    }
    Ok(())
}

fn validate_id(id: &str) -> Result<(), ToolError> {
    if id.is_empty()
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Err(invalid(
            "Notebook identifiers must use letters, numbers, dashes or underscores",
        ));
    }
    Ok(())
}

fn access() -> Result<MutexGuard<'static, ()>, ToolError> {
    NOTE_ACCESS
        .lock()
        .map_err(|_| storage("Note storage lock is unavailable"))
}

fn failure(code: &str, category: &str, message: impl ToString) -> ToolError {
    ToolError {
        error_code: code.into(),
        category: category.into(),
        message: message.to_string(),
    }
}
fn storage(message: impl ToString) -> ToolError {
    failure("NOTE_STORAGE_ERROR", "internal", message)
}
fn invalid(message: &str) -> ToolError {
    failure("INVALID_WORKSPACE_NOTE", "validation", message)
}
fn conflict(message: &str) -> ToolError {
    failure("NOTE_REVISION_CONFLICT", "conflict", message)
}
fn not_found(message: &str) -> ToolError {
    failure("NOTE_NOT_FOUND", "not_found", message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(id: &str, workspace: Option<&str>) -> WorkspaceNote {
        WorkspaceNote {
            note_id: id.into(),
            learning_workspace_id: workspace.map(str::to_owned),
            title: "反馈需要足够具体".into(),
            content: "具体的观察有助于选择下一次练习。".into(),
            revision: 0,
            created_at: "2026-10-27T08:00:00Z".into(),
            updated_at: "2026-10-27T08:00:00Z".into(),
            author: WorkspaceNoteAuthor::User,
            updated_by: WorkspaceNoteAuthor::User,
            origin: None,
            source_bindings: vec![],
        }
    }

    fn relation(id: &str, from: &str, to: &str) -> WorkspaceNoteRelation {
        WorkspaceNoteRelation {
            relation_id: id.into(),
            from_note_id: from.into(),
            to_note_id: to.into(),
            label: "举例说明".into(),
            reason: "这次练琴记录说明了反馈原则的应用。".into(),
            revision: 0,
            created_at: "2026-10-27T08:00:00Z".into(),
            updated_at: "2026-10-27T08:00:00Z".into(),
            author: WorkspaceNoteAuthor::Assistant,
        }
    }

    fn edit(
        store: &WorkspaceNoteStore,
        id: &str,
        revision: u32,
        content: &str,
    ) -> Result<WorkspaceNote, ToolError> {
        store.update(
            id,
            revision,
            "编辑后的想法",
            content,
            WorkspaceNoteAuthor::User,
            "2026-10-27T09:00:00Z",
        )
    }

    #[test]
    fn note_identity_provenance_and_authorship_survive_edits_and_reopen() {
        let temp = tempfile::tempdir().unwrap();
        let store = WorkspaceNoteStore::open(temp.path(), Some("cello"), false).unwrap();
        let mut imported = note("note-1", Some("cello"));
        imported.author = WorkspaceNoteAuthor::Unknown;
        imported.updated_by = WorkspaceNoteAuthor::Unknown;
        imported.origin = Some(WorkspaceNoteOrigin {
            kind: WorkspaceNoteOriginKind::ReadingNote,
            book_id: "cello-book".into(),
            publication_id: Some("edition-1".into()),
            session_id: None,
            turn_id: None,
            mem_id: Some("mem-1".into()),
        });
        imported.source_bindings.push(NoteSourceBinding {
            source_ref_id: "source-1".into(),
            book_id: "cello-book".into(),
            publication_id: Some("edition-1".into()),
            source_revision: None,
            evidence_range: None,
            media: None,
            music: None,
            evidence_text_digest: "existing-source-receipt".into(),
            label_snapshot: "练习目标".into(),
            preview_snapshot: "一次关注一个明确目标".into(),
        });
        let original = store.create(imported).unwrap();
        assert_eq!(original.revision, 1);
        assert_eq!(original.updated_by, WorkspaceNoteAuthor::Unknown);
        let changed = store
            .update(
                "note-1",
                1,
                "我的练习反馈",
                "先记录具体变化，再决定练什么。",
                WorkspaceNoteAuthor::Assistant,
                "2026-10-27T09:00:00Z",
            )
            .unwrap();
        assert_eq!(changed.note_id, original.note_id);
        assert_eq!(changed.revision, 2);
        assert_eq!(changed.created_at, original.created_at);
        assert_eq!(changed.origin, original.origin);
        assert_eq!(changed.source_bindings, original.source_bindings);
        assert_eq!(changed.author, WorkspaceNoteAuthor::Unknown);
        assert_eq!(changed.updated_by, WorkspaceNoteAuthor::Assistant);
        let renamed = store
            .update(
                "note-1",
                2,
                "反馈原则",
                &changed.content,
                WorkspaceNoteAuthor::User,
                "2026-10-27T10:00:00Z",
            )
            .unwrap();
        assert_eq!(renamed.updated_by, WorkspaceNoteAuthor::Assistant);
        let reopened = WorkspaceNoteStore::open(temp.path(), Some("cello"), false).unwrap();
        assert_eq!(reopened.read("note-1").unwrap(), renamed);
        assert_eq!(reopened.list().unwrap().notes, vec![renamed]);

        let mut edited_import = note("answer-edited-on-import", Some("cello"));
        edited_import.author = WorkspaceNoteAuthor::Assistant;
        edited_import.updated_by = WorkspaceNoteAuthor::User;
        let edited_import = store.create(edited_import).unwrap();
        assert_eq!(edited_import.author, WorkspaceNoteAuthor::Assistant);
        assert_eq!(edited_import.updated_by, WorkspaceNoteAuthor::User);
    }

    #[test]
    fn account_and_workspace_scope_isolate_notes_and_relations() {
        let accounts = tempfile::tempdir().unwrap();
        let first =
            WorkspaceNoteStore::open(&accounts.path().join("reader-1"), Some("cello"), false)
                .unwrap();
        let second =
            WorkspaceNoteStore::open(&accounts.path().join("reader-2"), Some("cello"), false)
                .unwrap();
        let another =
            WorkspaceNoteStore::open(&accounts.path().join("reader-1"), Some("physics"), false)
                .unwrap();
        let unscoped =
            WorkspaceNoteStore::open(&accounts.path().join("reader-1"), None, false).unwrap();
        first.create(note("note-1", Some("cello"))).unwrap();
        first.create(note("note-2", Some("cello"))).unwrap();
        first
            .upsert_relation(relation("relation-1", "note-1", "note-2"), None, "now")
            .unwrap();
        for other in [&second, &another, &unscoped] {
            assert_eq!(other.list().unwrap(), WorkspaceNotebook::default());
            assert_eq!(
                other.read("note-1").unwrap_err().error_code,
                "NOTE_NOT_FOUND"
            );
        }
        assert_eq!(
            first
                .create(note("wrong-scope", Some("physics")))
                .unwrap_err()
                .error_code,
            "INVALID_WORKSPACE_NOTE"
        );
        assert!(WorkspaceNoteStore::open(accounts.path(), Some("../escape"), false).is_err());
    }

    #[test]
    fn multiple_windows_preserve_other_notes_and_reject_stale_changes() {
        let temp = tempfile::tempdir().unwrap();
        let first = WorkspaceNoteStore::open(temp.path(), None, false).unwrap();
        let second = WorkspaceNoteStore::open(temp.path(), None, false).unwrap();
        first.create(note("note-1", None)).unwrap();
        second.create(note("note-2", None)).unwrap();
        let changed = edit(&first, "note-1", 1, "修改后的内容").unwrap();
        edit(&second, "note-2", 1, "另一个窗口的内容").unwrap();
        assert_eq!(first.read("note-1").unwrap(), changed);
        assert_eq!(second.list().unwrap().notes.len(), 2);
        assert_eq!(
            edit(&second, "note-1", 1, "过期编辑")
                .unwrap_err()
                .error_code,
            "NOTE_REVISION_CONFLICT"
        );
        assert_eq!(
            second.remove("note-1", 1).unwrap_err().error_code,
            "NOTE_REVISION_CONFLICT"
        );
        assert_eq!(second.read("note-1").unwrap(), changed);
        assert_eq!(
            second.create(note("note-1", None)).unwrap_err().error_code,
            "NOTE_REVISION_CONFLICT"
        );
    }

    #[test]
    fn concurrent_editors_cannot_both_commit_the_same_revision() {
        let temp = tempfile::tempdir().unwrap();
        let first = WorkspaceNoteStore::open(temp.path(), None, false).unwrap();
        first.create(note("note-1", None)).unwrap();
        let second = WorkspaceNoteStore::open(temp.path(), None, false).unwrap();
        let (one, two) = std::thread::scope(|scope| {
            let one = scope.spawn(|| edit(&first, "note-1", 1, "第一个窗口的内容"));
            let two = scope.spawn(|| edit(&second, "note-1", 1, "第二个窗口的内容"));
            (one.join().unwrap(), two.join().unwrap())
        });
        assert_eq!(usize::from(one.is_ok()) + usize::from(two.is_ok()), 1);
        let error = one.as_ref().err().or(two.as_ref().err()).unwrap();
        assert_eq!(error.error_code, "NOTE_REVISION_CONFLICT");
        assert_eq!(first.read("note-1").unwrap().revision, 2);
    }

    #[test]
    fn relations_require_meaning_and_existing_targets_and_keep_revision_control() {
        let temp = tempfile::tempdir().unwrap();
        let store = WorkspaceNoteStore::open(temp.path(), None, false).unwrap();
        store.create(note("principle", None)).unwrap();
        store.create(note("example", None)).unwrap();
        for mut invalid_relation in [
            relation("same", "principle", "principle"),
            relation("empty-label", "principle", "example"),
            relation("empty-reason", "principle", "example"),
        ] {
            if invalid_relation.relation_id == "empty-label" {
                invalid_relation.label = "  ".into();
            }
            if invalid_relation.relation_id == "empty-reason" {
                invalid_relation.reason = "\n".into();
            }
            assert_eq!(
                store
                    .upsert_relation(invalid_relation, None, "now")
                    .unwrap_err()
                    .error_code,
                "INVALID_WORKSPACE_NOTE"
            );
        }
        assert_eq!(
            store
                .upsert_relation(
                    relation("missing", "principle", "other-workspace-note"),
                    None,
                    "now"
                )
                .unwrap_err()
                .error_code,
            "NOTE_NOT_FOUND"
        );
        assert!(store.list().unwrap().relations.is_empty());
        let original = store
            .upsert_relation(
                relation("relation-1", "principle", "example"),
                None,
                "created",
            )
            .unwrap();
        let mut changed = original.clone();
        changed.label = "限制条件".into();
        changed.reason = "这个例子说明了原则在哪种情况下不适用。".into();
        let saved = store.upsert_relation(changed, Some(1), "updated").unwrap();
        assert_eq!(saved.revision, 2);
        assert_eq!(saved.created_at, original.created_at);
        assert_eq!(saved.author, original.author);
        assert_eq!(
            store
                .upsert_relation(original, Some(1), "stale")
                .unwrap_err()
                .error_code,
            "NOTE_REVISION_CONFLICT"
        );
        let mut rewritten = saved.clone();
        rewritten.author = WorkspaceNoteAuthor::User;
        assert_eq!(
            store
                .upsert_relation(rewritten, Some(2), "rewrite")
                .unwrap_err()
                .error_code,
            "INVALID_WORKSPACE_NOTE"
        );
        assert_eq!(
            store
                .remove_relation("relation-1", 1)
                .unwrap_err()
                .error_code,
            "NOTE_REVISION_CONFLICT"
        );
        store.remove_relation("relation-1", 2).unwrap();
        assert!(store.list().unwrap().relations.is_empty());
        assert_eq!(
            store
                .upsert_relation(saved, Some(2), "resurrect")
                .unwrap_err()
                .error_code,
            "NOTE_NOT_FOUND"
        );
    }

    #[test]
    fn deleting_a_note_removes_only_its_incident_relations() {
        let temp = tempfile::tempdir().unwrap();
        let store = WorkspaceNoteStore::open(temp.path(), None, false).unwrap();
        for id in ["a", "b", "c"] {
            store.create(note(id, None)).unwrap();
        }
        for (id, from, to) in [("ab", "a", "b"), ("ca", "c", "a"), ("bc", "b", "c")] {
            store
                .upsert_relation(relation(id, from, to), None, "now")
                .unwrap();
        }
        store.remove("a", 1).unwrap();
        let reopened = WorkspaceNoteStore::open(temp.path(), None, false).unwrap();
        let notebook = reopened.list().unwrap();
        assert_eq!(notebook.notes.len(), 2);
        assert_eq!(notebook.relations.len(), 1);
        assert_eq!(notebook.relations[0].relation_id, "bc");
        assert_eq!(reopened.read("a").unwrap_err().error_code, "NOTE_NOT_FOUND");
    }

    #[test]
    fn invalid_or_failed_writes_preserve_the_previous_document_and_recover_backup() {
        let temp = tempfile::tempdir().unwrap();
        let store = WorkspaceNoteStore::open(temp.path(), None, false).unwrap();
        store.create(note("a", None)).unwrap();
        store.create(note("b", None)).unwrap();
        store
            .upsert_relation(relation("ab", "a", "b"), None, "now")
            .unwrap();
        let before = store.list().unwrap();
        assert_eq!(
            edit(&store, "a", 1, " ").unwrap_err().error_code,
            "INVALID_WORKSPACE_NOTE"
        );
        let blocked = crate::atomic_temporary_path(&store.path);
        std::fs::create_dir(&blocked).unwrap();
        assert!(edit(&store, "a", 1, "不能提交的内容").is_err());
        assert!(store.remove("a", 1).is_err());
        assert_eq!(store.list().unwrap(), before);
        std::fs::remove_dir(blocked).unwrap();
        std::fs::rename(&store.path, crate::atomic_backup_path(&store.path)).unwrap();
        let reopened = WorkspaceNoteStore::open(temp.path(), None, false).unwrap();
        assert_eq!(reopened.list().unwrap(), before);
        assert!(store.path.exists());
        let changed = edit(&reopened, "a", 1, "恢复后可以继续编辑").unwrap();
        assert_eq!(changed.revision, 2);
    }
}
