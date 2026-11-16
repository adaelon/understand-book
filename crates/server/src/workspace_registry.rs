//! Network reading scenes. Lock order: user -> registry -> scene -> library.
//! The registry owns metadata and bounded scenes, never private Store copies.
use crate::{
    authorization::{missing, AuthorizedContext},
    control_store::ControlStore,
    published_library::{PublishedBookRef, PublishedLibrary},
    reader_workspace::ReaderWorkspace,
    user_runtime::UserRuntime,
    user_storage_paths::error,
};
use read_tools::ToolError;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub const WORKSPACE_IDLE_TTL: Duration = Duration::from_secs(15 * 60);
pub const MAX_USER_WORKSPACES: usize = 4;
pub const MAX_WORKSPACES: usize = 20;

pub(crate) fn stale() -> ToolError {
    error(
        "WORKSPACE_STALE",
        "conflict",
        "Reading workspace has changed",
    )
}
fn storage() -> ToolError {
    error(
        "WORKSPACE_STORAGE_FAILED",
        "unavailable",
        "Workspace checkpoint could not be committed",
    )
}
fn invalid() -> ToolError {
    error(
        "WORKSPACE_INPUT_INVALID",
        "validation",
        "Invalid workspace request",
    )
}
fn conflict() -> ToolError {
    error(
        "WORKSPACE_REVISION_CONFLICT",
        "conflict",
        "Refresh the workspace revision",
    )
}

#[derive(Clone, Serialize, Deserialize)]
struct Checkpoint {
    #[serde(default)]
    video_drafts: crate::video_draft::Drafts,
    version: u32,
    #[serde(default)]
    learning_workspace_id: Option<String>,
    #[serde(default)]
    learning_chat_selections: BTreeMap<String, String>,
    #[serde(default)]
    learning_reading_positions: Vec<crate::reader_workspace::LearningReadingPosition>,
    reader: reader::ReaderCheckpoint,
    presentation: Option<runtime::presentation::PresentationFollowUp>,
}
struct Record {
    reference: PublishedBookRef,
    selected_chat: Option<String>,
    generation: u64,
    revision: u64,
    checkpoint: Checkpoint,
}
#[derive(Deserialize)]
pub(crate) struct WorkspaceStamp {
    pub attachment_id: String,
    pub generation: u64,
    pub expected_revision: u64,
}
#[derive(PartialEq, Eq)]
enum OpeningOrigin {
    Created,
    AttachedFrom(String),
}
pub(crate) struct ResidentWorkspace {
    pub workspace: ReaderWorkspace,
    pub(crate) revision: u64,
    attachment: Option<String>,
    opening_origin: Option<OpeningOrigin>,
    linked: BTreeSet<String>,
    presentation: Option<runtime::presentation::PresentationFollowUp>,
    touched: Instant,
}
pub(crate) type WorkspaceHandle = Arc<Mutex<ResidentWorkspace>>;

pub struct WorkspaceRegistry {
    control: ControlStore,
    residents: BTreeMap<(String, String), WorkspaceHandle>,
    limits: Arc<crate::service_limits::ServiceLimits>,
}
impl WorkspaceRegistry {
    pub fn new(control: ControlStore) -> Self {
        Self::with_limits(
            control,
            Arc::new(crate::service_limits::ServiceLimits::default()),
        )
    }
    pub fn with_limits(
        control: ControlStore,
        limits: Arc<crate::service_limits::ServiceLimits>,
    ) -> Self {
        Self {
            limits,
            control,
            residents: BTreeMap::new(),
        }
    }

    fn record(&self, owner: &str, id: &str) -> Result<Record, ToolError> {
        let row: Option<(Option<String>, Option<String>, Option<String>, u64, u64, Option<String>)> = self.control.connection.query_row(
            "SELECT book_id,publication_id,selected_chat,generation,revision,checkpoint FROM reader_workspaces WHERE owner_user_id=? AND workspace_id=?",
            params![owner,id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))
        ).optional().map_err(|_| storage())?;
        let (book, publication, selected_chat, generation, revision, checkpoint) =
            row.ok_or_else(missing)?;
        let checkpoint: Checkpoint = serde_json::from_str(&checkpoint.ok_or_else(|| {
            error(
                "WORKSPACE_CHECKPOINT_INCOMPATIBLE",
                "conflict",
                "Workspace has no supported checkpoint",
            )
        })?)
        .map_err(|_| storage())?;
        if checkpoint.version != 1 {
            return Err(error(
                "WORKSPACE_CHECKPOINT_INCOMPATIBLE",
                "conflict",
                "Workspace checkpoint version is unsupported",
            ));
        }
        Ok(Record {
            reference: PublishedBookRef {
                book_id: book.ok_or_else(missing)?,
                publication_id: publication.ok_or_else(missing)?,
            },
            selected_chat,
            generation,
            revision,
            checkpoint,
        })
    }

    fn checkpoint(scene: &ResidentWorkspace) -> Checkpoint {
        let mut positions = scene.workspace.learning_reading_positions.clone();
        if let (Some(scope), Some(publication)) = (&scene.workspace.learning_workspace_id, &scene.workspace.publication) {
            positions.retain(|p| p.learning_workspace_id != *scope || p.published_book_ref != publication.reference);
            positions.push(crate::reader_workspace::LearningReadingPosition {
                learning_workspace_id: scope.clone(), published_book_ref: publication.reference.clone(),
                reader: scene.workspace.reader.checkpoint(),
            });
        }
        Checkpoint {
            video_drafts: scene.workspace.video_drafts.clone(),
            version: 1,
            learning_workspace_id: scene.workspace.learning_workspace_id.clone(),
            learning_chat_selections: scene.workspace.learning_chat_selections.clone(),
            learning_reading_positions: positions,
            reader: scene.workspace.reader.checkpoint(),
            presentation: scene.presentation.clone(),
        }
    }
    fn persist(
        &mut self,
        owner: &str,
        scene: &mut ResidentWorkspace,
        insert: bool,
    ) -> Result<(), ToolError> {
        if let (Some(scope), Some(chat)) = (&scene.workspace.learning_workspace_id, &scene.workspace.selected_chat) {
            scene.workspace.learning_chat_selections.insert(scope.clone(), chat.clone());
        }
        let reference = &scene.workspace.publication.as_ref().unwrap().reference;
        let checkpoint = Self::checkpoint(scene);
        let saved = serde_json::to_string(&checkpoint).map_err(|_| storage())?;
        #[cfg(test)]
        let _timing = crate::tests::mu10_tests::measure("control_commit");
        let tx = self
            .control
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|_| storage())?;
        let seq: u64 = tx
            .query_row(
                "SELECT COALESCE(MAX(checkpoint_seq),0)+1 FROM reader_workspaces",
                [],
                |r| r.get(0),
            )
            .map_err(|_| storage())?;
        let changed = if insert {
            tx.execute("INSERT INTO reader_workspaces(owner_user_id,workspace_id,book_id,publication_id,selected_chat,generation,revision,checkpoint,checkpoint_seq) VALUES(?,?,?,?,?,?,?,?,?)",
                params![owner,scene.workspace.id,reference.book_id,reference.publication_id,scene.workspace.selected_chat,scene.workspace.generation,scene.revision+1,saved,seq])
        } else {
            tx.execute("UPDATE reader_workspaces SET book_id=?,publication_id=?,selected_chat=?,generation=?,revision=revision+1,checkpoint=?,checkpoint_seq=? WHERE owner_user_id=? AND workspace_id=? AND revision=?",
                params![reference.book_id,reference.publication_id,scene.workspace.selected_chat,scene.workspace.generation,saved,seq,owner,scene.workspace.id,scene.revision])
        }.map_err(|_| storage())?;
        if changed != 1 {
            return Err(conflict());
        }
        tx.commit().map_err(|_| storage())?;
        scene.workspace.learning_reading_positions = checkpoint.learning_reading_positions;
        scene.revision += 1;
        scene.touched = Instant::now();
        Ok(())
    }

    // Acceptance only transfers draft ownership; it is not a new reader action.
    // Preserve the revision captured by the accepted Run for its reader effects.
    pub(crate) fn persist_video_draft_handoff(
        &mut self,
        owner: &str,
        scene: &ResidentWorkspace,
    ) -> Result<(), ToolError> {
        let saved = serde_json::to_string(&Self::checkpoint(scene)).map_err(|_| storage())?;
        let changed = self.control.connection.execute(
            "UPDATE reader_workspaces SET checkpoint=? WHERE owner_user_id=? AND workspace_id=? AND revision=?",
            params![saved, owner, scene.workspace.id, scene.revision],
        ).map_err(|_| storage())?;
        if changed != 1 { return Err(conflict()); }
        Ok(())
    }

    pub(crate) fn evict_idle(&mut self, now: Instant) -> Result<usize, ToolError> {
        let mut removed = 0;
        let keys: Vec<_> = self.residents.keys().cloned().collect();
        for key in keys {
            let handle = &self.residents[&key];
            if Arc::strong_count(handle) != 1 {
                continue;
            }
            let scene = handle.lock().unwrap();
            if now.saturating_duration_since(scene.touched) < WORKSPACE_IDLE_TTL
                || self.busy(&key.0, &key.1)?
            {
                continue;
            }
            // Each successful command already committed a checkpoint; no dirty scene is evicted.
            drop(scene);
            self.residents.remove(&key);
            removed += 1;
        }
        Ok(removed)
    }
    fn busy(&self, owner: &str, id: &str) -> Result<bool, ToolError> {
        self.control.connection.query_row("SELECT EXISTS(SELECT 1 FROM run_admissions WHERE owner_user_id=? AND workspace_id=? AND dispatch_state IN ('preparing','queued','claimed'))", params![owner,id], |r| r.get(0)).map_err(|_| storage())
    }
    fn reserve(&mut self, owner: &str) -> Result<(), ToolError> {
        self.evict_idle(Instant::now())?;
        if self.residents.len() >= self.limits.workspaces
            || self.residents.keys().filter(|(u, _)| u == owner).count()
                >= self.limits.user_workspaces
        {
            return Err(error(
                "WORKSPACE_CAPACITY",
                "unavailable",
                "Resident workspace capacity is occupied",
            ));
        }
        Ok(())
    }
    pub(crate) fn scene(
        &mut self,
        owner: &str,
        id: &str,
        user: &UserRuntime,
        library: &Mutex<PublishedLibrary>,
    ) -> Result<WorkspaceHandle, ToolError> {
        let mut record = self.record(owner, id)?;
        library
            .lock()
            .unwrap()
            .authorize(owner, &record.reference)
            .map_err(|_| missing())?;
        let key = (owner.to_owned(), id.to_owned());
        if let Some(handle) = self.residents.get(&key).cloned() {
            let mut scene = handle.lock().unwrap();
            self.refresh_learning_scope(owner, &mut scene.workspace, false)?;
            self.reconcile_chat(owner, &mut scene, user)?;
            scene.touched = Instant::now();
            drop(scene);
            return Ok(handle);
        }
        self.reserve(owner)?;
        let publication = library.lock().unwrap().load(owner, &record.reference)?;
        if record.selected_chat.as_ref().is_some_and(|id| {
            !user
                .agent_history
                .sessions
                .iter()
                .any(|s| &s.id == id && s.matches_scope(record.checkpoint.learning_workspace_id.as_deref(), &record.reference.book_id))
        }) {
            record.selected_chat = None;
            record.checkpoint.presentation = None;
        }
        let mut workspace = ReaderWorkspace::local(
            publication.directory.clone(),
            publication.book.clone(),
            reader::Reader::from_checkpoint(&publication.book, &record.checkpoint.reader),
            vec![],
            None,
        );
        workspace.publication = Some(publication);
        workspace.learning_workspace_id = record.checkpoint.learning_workspace_id.clone();
        workspace.learning_chat_selections = record.checkpoint.learning_chat_selections.clone();
        workspace.learning_reading_positions = record.checkpoint.learning_reading_positions.clone();
        self.refresh_learning_scope(owner, &mut workspace, false)?;
        workspace.id = id.into();
        workspace.generation = record.generation + 1;
        workspace.video_drafts = record.checkpoint.video_drafts;
        workspace.selected_chat = record.selected_chat;
        workspace.messages = messages(user, workspace.selected_chat.as_deref());
        let mut scene = ResidentWorkspace {
            workspace,
            revision: record.revision,
            attachment: None,
            opening_origin: None,
            linked: BTreeSet::new(),
            presentation: record.checkpoint.presentation,
            touched: Instant::now(),
        };
        self.persist(owner, &mut scene, false)?;
        let handle = Arc::new(Mutex::new(scene));
        self.residents.insert(key, handle.clone());
        Ok(handle)
    }
    fn reconcile_chat(
        &mut self,
        owner: &str,
        scene: &mut ResidentWorkspace,
        user: &UserRuntime,
    ) -> Result<(), ToolError> {
        if scene.workspace.selected_chat.as_ref().is_some_and(|id| {
            !user
                .agent_history
                .sessions
                .iter()
                .any(|s| &s.id == id && s.matches_scope(scene.workspace.learning_workspace_id.as_deref(), &scene.workspace.book.base.book_id))
        }) {
            let old = scene.workspace.selected_chat.take();
            let presentation = scene.presentation.take();
            scene.workspace.invalidate();
            if let Err(e) = self.persist(owner, scene, false) {
                scene.workspace.generation -= 1;
                scene.workspace.selected_chat = old;
                scene.presentation = presentation;
                return Err(e);
            }
            scene.workspace.messages.clear();
            scene.linked.clear();
        }
        Ok(())
    }

    pub(crate) fn clear_chat(&mut self, owner: &str, chat: &str) -> Result<(), ToolError> {
        self.control.connection.execute("UPDATE reader_workspaces SET selected_chat=NULL,generation=generation+1,revision=revision+1,checkpoint=json_set(checkpoint,'$.presentation',NULL) WHERE owner_user_id=? AND selected_chat=?", params![owner,chat]).map_err(|_| storage())?;
        for ((user, _), handle) in &self.residents {
            if user != owner {
                continue;
            }
            let mut scene = handle.lock().unwrap();
            if scene.workspace.selected_chat.as_deref() == Some(chat) {
                scene.workspace.selected_chat = None;
                scene.workspace.invalidate();
                scene.workspace.messages.clear();
                scene.presentation = None;
                scene.linked.clear();
                scene.revision += 1;
            }
        }
        Ok(())
    }

    pub(crate) fn create(
        &mut self,
        context: &AuthorizedContext,
        user: &UserRuntime,
        library: &Mutex<PublishedLibrary>,
        input: &Value,
    ) -> Result<Value, ToolError> {
        let attachment = attachment(input)?;
        let reference = reference(input, library)?;
        let learning_workspace_id = learning_scope(input.get("learning_workspace_id"))?;
        if let Some(result) = self.reuse_opening(
            context.user_id(), user, library, &attachment, &reference,
            learning_workspace_id.as_deref(), &OpeningOrigin::Created,
        )? {
            return Ok(result);
        }
        self.create_bound(
            context.user_id(),
            user,
            library,
            reference,
            attachment,
            None,
            None,
            learning_workspace_id,
            Some(OpeningOrigin::Created),
        )
    }
    // A timed-out response must not consume another resident slot when the same
    // browser retries. This receipt lives only as long as the bounded resident.
    fn reuse_opening(
        &mut self,
        owner: &str,
        user: &UserRuntime,
        library: &Mutex<PublishedLibrary>,
        attachment: &str,
        reference: &PublishedBookRef,
        learning_workspace_id: Option<&str>,
        origin: &OpeningOrigin,
    ) -> Result<Option<Value>, ToolError> {
        let handle = self.residents.iter().find_map(|((resident_owner, _), handle)| {
            if resident_owner != owner { return None; }
            let scene = handle.lock().unwrap();
            (scene.attachment.as_deref() == Some(attachment)
                && scene.opening_origin.as_ref() == Some(origin)
                && scene.workspace.publication.as_ref().map(|p| &p.reference) == Some(reference)
                && scene.workspace.learning_workspace_id.as_deref() == learning_workspace_id)
                .then(|| handle.clone())
        });
        let Some(handle) = handle else { return Ok(None); };
        library.lock().unwrap().authorize(owner, reference)?;
        let mut scene = handle.lock().unwrap();
        self.refresh_learning_scope(owner, &mut scene.workspace, true)?;
        self.reconcile_chat(owner, &mut scene, user)?;
        scene.touched = Instant::now();
        Ok(Some(view(&scene)))
    }
    fn create_bound(
        &mut self,
        owner: &str,
        user: &UserRuntime,
        library: &Mutex<PublishedLibrary>,
        reference: PublishedBookRef,
        attachment: String,
        copy: Option<Checkpoint>,
        chat: Option<String>,
        learning_workspace_id: Option<String>,
        opening_origin: Option<OpeningOrigin>,
    ) -> Result<Value, ToolError> {
        self.reserve(owner)?;
        let publication = library.lock().unwrap().load(owner, &reference)?;
        let checkpoint = if copy.is_some() {
            copy
        } else {
            let saved: Option<String> = self.control.connection.query_row("SELECT checkpoint FROM reader_workspaces WHERE owner_user_id=? AND book_id=? AND publication_id=? AND checkpoint IS NOT NULL AND json_extract(checkpoint,'$.learning_workspace_id') IS ? ORDER BY checkpoint_seq DESC LIMIT 1", params![owner,reference.book_id,reference.publication_id,learning_workspace_id], |r| r.get(0)).optional().map_err(|_| storage())?;
            saved
                .map(|s| serde_json::from_str::<Checkpoint>(&s).map_err(|_| storage()))
                .transpose()?
                .map(|mut c| {
                    c.presentation = None;
                    c
                })
        };
        if checkpoint.as_ref().is_some_and(|c| c.version != 1) {
            return Err(error(
                "WORKSPACE_CHECKPOINT_INCOMPATIBLE",
                "conflict",
                "Workspace checkpoint version is unsupported",
            ));
        }
        let reader = checkpoint
            .as_ref()
            .map(|c| reader::Reader::from_checkpoint(&publication.book, &c.reader))
            .unwrap_or_else(|| reader::Reader::new(&publication.book, reader::DEFAULT_RADIUS));
        let mut workspace = ReaderWorkspace::local(
            publication.directory.clone(),
            publication.book.clone(),
            reader,
            messages(user, chat.as_deref()),
            None,
        );
        workspace.id = format!("ws-{}", uuid::Uuid::now_v7());
        workspace.generation = 1;
        workspace.publication = Some(publication);
        workspace.learning_workspace_id = learning_workspace_id;
        workspace.learning_chat_selections = checkpoint.as_ref().map(|c| c.learning_chat_selections.clone()).unwrap_or_default();
        workspace.learning_reading_positions = checkpoint.as_ref().map(|c| c.learning_reading_positions.clone()).unwrap_or_default();
        workspace.video_drafts = checkpoint.as_ref().map(|c|c.video_drafts.clone()).unwrap_or_default();
        self.refresh_learning_scope(owner, &mut workspace, true)?;
        workspace.selected_chat = chat.or_else(|| workspace.learning_workspace_id.as_ref()
            .and_then(|scope| workspace.learning_chat_selections.get(scope)).filter(|id|user.agent_history.sessions.iter()
                .any(|s|&s.id == *id && s.matches_scope(workspace.learning_workspace_id.as_deref(), &reference.book_id))).cloned())
            .or_else(|| workspace.learning_workspace_id.as_deref().and_then(|scope|
            user.agent_history.sessions.iter().filter(|s| s.matches_scope(Some(scope), &reference.book_id))
                .max_by(|a,b| a.updated_at.cmp(&b.updated_at)).map(|s| s.id.clone())));
        workspace.messages = messages(user, workspace.selected_chat.as_deref());
        let mut scene = ResidentWorkspace {
            workspace,
            revision: 0,
            attachment: Some(attachment),
            opening_origin,
            linked: BTreeSet::new(),
            presentation: checkpoint.and_then(|c| c.presentation),
            touched: Instant::now(),
        };
        self.persist(owner, &mut scene, true)?;
        let result = view(&scene);
        self.residents.insert(
            (owner.into(), scene.workspace.id.clone()),
            Arc::new(Mutex::new(scene)),
        );
        Ok(result)
    }

    pub(crate) fn refresh_learning_scope(&self, owner: &str, workspace: &mut ReaderWorkspace, for_run: bool) -> Result<(), ToolError> {
        let Some(id) = workspace.learning_workspace_id.as_deref() else { return Ok(()); };
        let learning = self.control.learning_workspace(owner, id)?;
        if for_run && learning.archived {
            return Err(error("LEARNING_WORKSPACE_ARCHIVED", "conflict", "Restore the learning workspace before starting new work"));
        }
        let members: Vec<_> = learning.members.into_iter().filter(|m| m.readable).map(|m| m.published_book_ref).collect();
        if for_run && !members.iter().any(|m| Some(m) == workspace.publication.as_ref().map(|p| &p.reference)) {
            return Err(error("WORKSPACE_MATERIAL_OUT_OF_SCOPE", "permission", "The publication is not a readable member of this learning workspace"));
        }
        workspace.learning_goal = Some(learning.goal);
        workspace.learning_members = Some(members);
        Ok(())
    }

    pub(crate) fn check_voice_scene(&mut self, context: &AuthorizedContext, user: &UserRuntime,
        library: &Mutex<PublishedLibrary>, id: &str, input: &Value) -> Result<(), ToolError> {
        let scene = self.scene(context.user_id(),id,user,library)?;
        let scene = scene.lock().unwrap();
        let stamp = serde_json::from_value(input.clone()).map_err(|_|invalid())?;
        scene.check_attachment(&stamp)
    }

    pub(crate) fn prepare_guidance(
        &mut self, context: &AuthorizedContext, user: &UserRuntime,
        library: &Mutex<PublishedLibrary>, id: &str, action: &str, input: &Value,
    ) -> Result<(crate::reading_guidance::GuidanceWork, PublishedBookRef), ToolError> {
        let scene = self.scene(context.user_id(), id, user, library)?;
        let scene = scene.lock().unwrap();
        let stamp = serde_json::from_value(input.clone()).map_err(|_| invalid())?;
        scene.check_attachment(&stamp)?;
        let work = crate::reading_guidance::GuidanceWork::prepare(&private_context(user, &scene.workspace), action, input,
            &crate::multi_user_host::now().to_string())?;
        Ok((work, scene.workspace.publication.as_ref().ok_or_else(missing)?.reference.clone()))
    }

    pub(crate) fn finish_guidance(&mut self, context: &AuthorizedContext, user: &UserRuntime,
        library: &Mutex<PublishedLibrary>, id: &str, publication: &PublishedBookRef,
        work: &crate::reading_guidance::GuidanceWork, result: Value) -> Result<Value, ToolError> {
        let scene = self.scene(context.user_id(), id, user, library)?;
        let scene = scene.lock().unwrap();
        if scene.workspace.publication.as_ref().map(|p| &p.reference) != Some(publication) { return Err(missing()); }
        work.finish(&private_context(user, &scene.workspace), result, &crate::multi_user_host::now().to_string())
    }

    pub(crate) fn prepare_translation(
        &mut self,
        context: &AuthorizedContext,
        user: &UserRuntime,
        library: &Mutex<PublishedLibrary>,
        id: &str,
        input: &Value,
    ) -> Result<(crate::SelectionTranslationWork, PublishedBookRef), ToolError> {
        let scene = self.scene(context.user_id(), id, user, library)?;
        let scene = scene.lock().unwrap();
        let stamp = serde_json::from_value(input.clone()).map_err(|_| invalid())?;
        scene.check_attachment(&stamp)?;
        let mut command = input.clone();
        for key in ["attachment_id", "generation", "expected_revision"] {
            command.as_object_mut().ok_or_else(invalid)?.remove(key);
        }
        let request = serde_json::from_value(command).map_err(|_| invalid())?;
        crate::selection_manifest_value(&scene.workspace.book_dir)?;
        let work = crate::prepare_selection_translation(&scene.workspace.book, request)?;
        Ok((work, scene.workspace.publication.as_ref().ok_or_else(missing)?.reference.clone()))
    }

    pub(crate) fn latest_resumption(
        &self,
        owner: &str,
        user: &UserRuntime,
        library: &Mutex<PublishedLibrary>,
    ) -> Result<Value, ToolError> {
        let course_refs = crate::course_access::material_refs(&self.control.connection,owner,crate::multi_user_host::now())?;
        let id: Option<String> = self.control.connection.query_row(
            "SELECT w.workspace_id FROM reader_workspaces w
             WHERE w.owner_user_id=?1 AND w.checkpoint IS NOT NULL
             AND (EXISTS(SELECT 1 FROM book_grants g WHERE g.owner_user_id=w.owner_user_id AND g.book_id=w.book_id AND g.publication_id=w.publication_id)
                  OR EXISTS(SELECT 1 FROM json_each(?2) m WHERE json_extract(m.value,'$.book_id')=w.book_id AND json_extract(m.value,'$.publication_id')=w.publication_id))
             ORDER BY w.checkpoint_seq DESC LIMIT 1",
            params![owner,course_refs], |row| row.get(0),
        ).optional().map_err(|_| storage())?;
        match id {
            Some(id) => self.resumption(owner, &id, user, library),
            None => Ok(Value::Null),
        }
    }

    fn resumption(
        &self,
        owner: &str,
        id: &str,
        user: &UserRuntime,
        library: &Mutex<PublishedLibrary>,
    ) -> Result<Value, ToolError> {
        // Preview the committed scene without loading/attaching a resident workspace.
        let record = self.record(owner, id)?;
        let publication = library.lock().unwrap().load(owner, &record.reference)?;
        let lid = &record.checkpoint.reader.top_lid;
        let position = publication.book.source_label(&read_tools::EvidenceRange {
            start_lid: lid.clone(), end_lid: lid.clone(), ranges: vec![],
        }, "zh").ok();
        let excerpt = publication.book.text(lid, None).ok()
            .map(|text| text.chars().take(140).collect::<String>());
        let chat = user.agent_history.sessions.iter().find(|session|
            Some(&session.id) == record.selected_chat.as_ref()
                && session.matches_scope(record.checkpoint.learning_workspace_id.as_deref(), &record.reference.book_id));
        let question = chat.and_then(|session| session.turns.iter().rev().find(|turn|
            turn.published_book_ref.as_ref() == Some(&record.reference)
                && !turn.user.trim().is_empty()))
            .map(|turn| turn.user.chars().take(240).collect::<String>());
        Ok(json!({"workspace_id":id, "published_book_ref":record.reference,
            "learning_workspace_id":record.checkpoint.learning_workspace_id,
            "selected_chat":chat.map(|session| &session.id), "position_label":position,
            "position_excerpt":excerpt, "last_question":question}))
    }

    pub(crate) fn request(
        &mut self,
        context: &AuthorizedContext,
        user: &mut UserRuntime,
        library: &Mutex<PublishedLibrary>,
        id: &str,
        action: &str,
        input: &Value,
        now: &str,
    ) -> Result<Value, ToolError> {
        let owner = context.user_id();
        if action == "resumption" {
            return self.resumption(owner, id, user, library);
        }
        // Attach compares the client's last persisted version before a cold reload increments it.
        if matches!(action, "attach" | "takeover" | "fork") {
            let record = self.record(owner, id)?;
            let stamp: WorkspaceStamp =
                serde_json::from_value(input.clone()).map_err(|_| invalid())?;
            if stamp.generation != record.generation {
                return Err(stale());
            }
            if stamp.expected_revision != record.revision {
                return Err(conflict());
            }
        }
        let handle = self.scene(owner, id, user, library)?;
        let mut scene = handle.lock().unwrap();
        if action.is_empty() {
            return Ok(view(&scene));
        }
        if action == "learning-task/state" {
            return crate::learning_task::local_list(user, &scene.workspace);
        }
        if matches!(action, "attach" | "takeover" | "fork") {
            let next = attachment(input)?;
            if action == "fork"
                || (action == "attach" && scene.attachment.as_ref().is_some_and(|a| a != &next))
            {
                let saved = Self::checkpoint(&scene);
                let chat = scene.workspace.selected_chat.clone();
                let reference = scene
                    .workspace
                    .publication
                    .as_ref()
                    .unwrap()
                    .reference
                    .clone();
                let learning_workspace_id = scene.workspace.learning_workspace_id.clone();
                drop(scene);
                drop(handle);
                let opening_origin = (action == "attach")
                    .then(|| OpeningOrigin::AttachedFrom(id.to_owned()));
                if let Some(origin) = opening_origin.as_ref() {
                    if let Some(mut result) = self.reuse_opening(
                        owner, user, library, &next, &reference,
                        learning_workspace_id.as_deref(), origin,
                    )? {
                        result["forked_from"] = json!(id);
                        return Ok(result);
                    }
                }
                let mut result =
                    self.create_bound(owner, user, library, reference, next, Some(saved), chat, learning_workspace_id, opening_origin)?;
                result["forked_from"] = json!(id);
                return Ok(result);
            }
            if scene.attachment.as_ref() != Some(&next) || action == "takeover" {
                scene.workspace.invalidate();
                if let Err(e) = self.persist(owner, &mut scene, false) {
                    scene.workspace.generation -= 1;
                    return Err(e);
                }
                scene.attachment = Some(next);
                scene.opening_origin = None;
                scene.linked.clear();
            }
            return Ok(view(&scene));
        }
        let stamp: WorkspaceStamp = serde_json::from_value(input.clone()).map_err(|_| invalid())?;
        if matches!(action, "reader/state" | "profile/manifest" | "profile/memory" | "memory/recall" | "memory/prepare_note"
            | "chat/history" | "knowledge/tree" | "knowledge/resume" | "reader/paper_minimap.state" | "reader/pdf_selection.resolve"
            | "reader/pdf_ranges.project" | "agent/source.resolve" | "agent/source.read" | "memory/presentation.read" | "memory/presentation.observe"
            | "boards/list" | "boards/read" | "boards/conversation" | "boards/catalog" | "boards/search" | "boards/related" | "boards/source.resolve" | "boards/source.read" | "boards/presentation.read" | "boards/presentation.observe"
            | "notes/list" | "notes/read" | "notes/source.resolve" | "notes/source.read"
            | "presentation/read" | "presentation/observe" | "presentation/feedback/read") {
            scene.check_attachment(&stamp)?;
        } else {
            scene.check(&stamp)?;
        }
        // Transport binding is validated above; shared business parsers see their own payload.
        let mut command = input.clone();
        for key in ["attachment_id", "generation", "expected_revision"] {
            command.as_object_mut().ok_or_else(invalid)?.remove(key);
        }
        let input = &command;
        if action == "learning-task" {
            let result = crate::learning_task::select(user, &scene.workspace, input, now)?;
            let mut value = view(&scene); value["result"] = result; return Ok(value);
        }
        if matches!(action, "notes/source.resolve" | "notes/source.read" | "notes/source.open") {
            let binding = crate::workspace_notes_api::source_binding(user, &scene.workspace, input)?;
            let same_publication = binding.book_id == scene.workspace.book.base.book_id
                && binding.publication_id.as_ref() == scene.workspace.publication.as_ref().map(|p| &p.reference.publication_id);
            if !same_publication {
                let mut value = view(&scene);
                value["result"] = if action == "notes/source.read" {
                    crate::cross_book::source_window(library, owner, &binding, input)?
                } else {
                    crate::cross_book::source_view(library, owner, &binding, action.ends_with(".open"))?
                };
                return Ok(value);
            }
        }
        if action.starts_with("notes/") && action != "notes/source.open" {
            let result = crate::workspace_notes_api::route(user, &scene.workspace, action.trim_start_matches("notes/"), input, now)?;
            let mut value = view(&scene); value["result"] = result; return Ok(value);
        }
        if matches!(action, "boards/source.resolve" | "boards/source.read" | "boards/source.open") {
            let binding = crate::board_api::source_binding(user, &scene.workspace, input)?;
            let same_publication = binding.book_id == scene.workspace.book.base.book_id
                && binding.publication_id.as_ref() == scene.workspace.publication.as_ref().map(|p| &p.reference.publication_id);
            if !same_publication {
                let mut value = view(&scene);
                value["result"] = if action == "boards/source.read" {
                    crate::cross_book::source_window(library, owner, &binding, input)?
                } else {
                    crate::cross_book::source_view_at(library, owner, &binding, action.ends_with(".open"), crate::board_api::optional_source_lid(input, "lid")?)?
                };
                return Ok(value);
            }
        }
        if action.starts_with("boards/") && action != "boards/source.open" {
            let result = crate::board_api::route_mut(user, &scene.workspace, action.trim_start_matches("boards/"), input, now)?;
            let mut value = view(&scene); value["result"] = result; return Ok(value);
        }
        if action == "agent/source.resolve" {
            let request = crate::parse_agent_source_request(&input.to_string()).map_err(|r| crate::workspace_client::reply(r).unwrap_err())?;
            let binding = crate::workspace_source_binding(user, &scene.workspace, &request)?;
            if let Some(music) = &binding.music {
                let reference = &music.target.publication;
                let loaded = library.lock().unwrap().load(owner, reference);
                let mut popup = crate::music_source::popup(&binding,
                    loaded.as_ref().map(|p| (p.book.as_ref(), p.directory())).map_err(|e| crate::user_storage_paths::error(&e.error_code, &e.category, &e.message)),
                    reference.url("music/audio"));
                if scene.workspace.publication.as_ref().map(|p| &p.reference) != Some(reference) { popup["can_open_in_reader"] = json!(false); }
                let mut value = view(&scene); value["result"] = popup; return Ok(value);
            }
            if let Some(media) = &binding.media {
                let loaded = library.lock().unwrap().load(owner, &media.publication);
                let popup = crate::video_source::popup(&binding, loaded.as_ref().map(|p| (p.book.as_ref(), p.directory())).map_err(|e| crate::user_storage_paths::error(&e.error_code, &e.category, &e.message)), media.publication.url("video/media"));
                let mut value = view(&scene); value["result"] = popup; return Ok(value);
            }
        }
        if action == "reader/adopt_same_source_publication" {
            if scene.workspace.learning_workspace_id.is_some() {
                let mut value = view(&scene);
                value["result"] = json!({"status":"selection_required"});
                return Ok(value);
            }
            let in_flight: bool = self.control.connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM run_admissions WHERE owner_user_id=? AND workspace_id=? AND (dispatch_state IN ('preparing','queued','claimed') OR unsaved=1))",
                params![owner,id], |r| r.get(0)).map_err(|_| storage())?;
            let mut value = view(&scene);
            if in_flight || scene.workspace.video_adoption_busy(user) {
                value["result"] = json!({"status":"busy"}); return Ok(value);
            }
            let old = scene.workspace.publication.as_ref().unwrap().clone();
            let music = old.directory().join(crate::music_media::MANIFEST).exists();
            let projection = |publication: &crate::published_library::PublishedBook| {
                if music { crate::music_media::projection(publication.directory()) }
                else { crate::video_publication::projection(&publication.book, publication.directory()) }
            };
            let media_key = if music { "music" } else { "video" };
            let next = library.lock().unwrap().video_successor(owner, &old.reference)?;
            let Some(next) = next else {
                value["result"] = json!({"status":"current"});
                if input["publication_id"].as_str().is_some_and(|id| id != old.reference.publication_id) {
                    value["result"][media_key] = projection(&old)?;
                }
                return Ok(value);
            };
            // Build the projection before committing, so a read failure cannot half-adopt.
            let media = projection(&next)?;
            scene.workspace.adopt_same_source_publication(next)?;
            if let Err(e) = self.persist(owner, &mut scene, false) {
                scene.workspace.adopt_same_source_publication(old)?;
                return Err(e);
            }
            let mut value = view(&scene); value["result"] = json!({"status":"adopted"});
            value["result"][media_key] = media;
            return Ok(value);
        }
        if action == "reader/video_draft" {
            let before = scene.workspace.video_drafts.clone();
            let result = crate::video_draft::command(&mut scene.workspace, user, input)?;
            if input["action"] != "get" {
                if let Err(e) = self.persist(owner, &mut scene, false) {
                    scene.workspace.video_drafts = before;
                    return Err(e);
                }
            }
            let mut value = view(&scene); value["result"] = result; return Ok(value);
        }
        if action == "linked/attach" {
            if scene.attachment.as_deref() != Some(&stamp.attachment_id) {
                return Err(stale());
            }
            if scene.linked.len() >= 4 {
                return Err(error(
                    "WORKSPACE_ATTACHMENT_CAPACITY",
                    "conflict",
                    "Attached view capacity is occupied",
                ));
            }
            let session = input["session_id"].as_str().ok_or_else(invalid)?;
            if scene.workspace.selected_chat.as_deref() != Some(session) {
                return Err(stale());
            }
            let reference =
                serde_json::from_value(input["reference"].clone()).map_err(|_| invalid())?;
            let turn = input["turn_id"].as_str().ok_or_else(invalid)?;
            with_turn_presentation(user, &scene.workspace, library, session, turn, |private| {
                crate::presentation_api::delivered_version(private, session, turn, &reference)
            })?;
            let linked = format!("view-{}", uuid::Uuid::now_v7());
            scene.linked.insert(linked.clone());
            let mut result = view(&scene);
            result["attachment_id"] = json!(linked);
            result["linked"] = json!(true);
            return Ok(result);
        }
        if action == "detach" {
            if scene.linked.remove(&stamp.attachment_id) {
                return Ok(json!({"detached":true}));
            }
            scene.workspace.invalidate();
            if let Err(e) = self.persist(owner, &mut scene, false) {
                scene.workspace.generation -= 1;
                return Err(e);
            }
            scene.attachment = None;
            scene.linked.clear();
            let result = view(&scene);
            drop(scene);
            drop(handle);
            let key = (owner.into(), id.into());
            if Arc::strong_count(&self.residents[&key]) == 1 && !self.busy(owner, id)? {
                self.residents.remove(&key);
            }
            return Ok(result);
        }
        if action == "book/open" {
            if scene.attachment.as_deref() != Some(&stamp.attachment_id) {
                return Err(stale());
            }
            let reference = reference(input, library)?;
            let publication = library.lock().unwrap().load(owner, &reference)?;
            let learning_workspace_id = if input.get("learning_workspace_id").is_some() {
                learning_scope(input.get("learning_workspace_id"))?
            } else { scene.workspace.learning_workspace_id.clone() };
            let reader = learning_workspace_id.as_deref().and_then(|scope| scene.workspace.learning_reading_positions.iter()
                .find(|p| p.learning_workspace_id == scope && p.published_book_ref == reference))
                .map(|p| reader::Reader::from_checkpoint(&publication.book, &p.reader))
                .unwrap_or_else(|| reader::Reader::new(&publication.book, reader::DEFAULT_RADIUS));
            let mut workspace = ReaderWorkspace::local(
                publication.directory.clone(),
                publication.book.clone(),
                reader,
                vec![],
                None,
            );
            workspace.id = id.into();
            workspace.generation = scene.workspace.generation + 1;
            workspace.publication = Some(publication);
            workspace.learning_chat_selections = scene.workspace.learning_chat_selections.clone();
            workspace.learning_reading_positions = scene.workspace.learning_reading_positions.clone();
            workspace.video_drafts = scene.workspace.video_drafts.clone();
            workspace.learning_workspace_id = learning_workspace_id;
            self.refresh_learning_scope(owner, &mut workspace, true)?;
            if workspace.learning_workspace_id.is_some() {
                workspace.selected_chat = if workspace.learning_workspace_id == scene.workspace.learning_workspace_id {
                    scene.workspace.selected_chat.clone()
                } else {
                    workspace.learning_workspace_id.as_ref().and_then(|scope| workspace.learning_chat_selections.get(scope))
                        .filter(|id|user.agent_history.sessions.iter().any(|s| &s.id == *id
                            && s.matches_scope(workspace.learning_workspace_id.as_deref(), &workspace.book.base.book_id))).cloned()
                        .or_else(|| user.agent_history.sessions.iter().filter(|s| s.matches_scope(workspace.learning_workspace_id.as_deref(), &workspace.book.base.book_id))
                            .max_by(|a,b| a.updated_at.cmp(&b.updated_at)).map(|s| s.id.clone()))
                };
                workspace.messages = messages(user, workspace.selected_chat.as_deref());
            }
            let mut next = ResidentWorkspace {
                workspace,
                revision: scene.revision,
                attachment: scene.attachment.clone(),
                opening_origin: None,
                linked: BTreeSet::new(),
                presentation: None,
                touched: Instant::now(),
            };
            self.persist(owner, &mut next, false)?;
            *scene = next;
            return Ok(view(&scene));
        }
        if matches!(action, "chat/select" | "chat/new") {
            if scene.attachment.as_deref() != Some(&stamp.attachment_id) {
                return Err(stale());
            }
            let chat = if action == "chat/new" && user.session_store.is_some() {
                let mut session = crate::new_agent_session(&scene.workspace.book.base.book_id, now, 0);
                session.learning_workspace_id = scene.workspace.learning_workspace_id.clone();
                session.id = format!("chat-{}", uuid::Uuid::now_v7());
                let id = session.id.clone();
                let crate::user_runtime::UserRuntime { session_store, agent_history, .. } = user;
                session_store.as_mut().unwrap().create(agent_history, session)?;
                id
            } else if action == "chat/new" {
                let mut candidate = user.agent_history.clone();
                let mut session = crate::new_agent_session(
                    &scene.workspace.book.base.book_id,
                    now,
                    candidate.sessions.len() + 1,
                );
                session.learning_workspace_id = scene.workspace.learning_workspace_id.clone();
                session.id = format!("chat-{}", uuid::Uuid::now_v7());
                let id = session.id.clone();
                candidate.sessions.push(session);
                crate::save_agent_history_path(&user.history_path, &candidate)?;
                user.agent_history = candidate;
                id
            } else {
                input["session_id"].as_str().ok_or_else(invalid)?.into()
            };
            if !user
                .agent_history
                .sessions
                .iter()
                .any(|s| s.id == chat && s.matches_scope(scene.workspace.learning_workspace_id.as_deref(), &scene.workspace.book.base.book_id))
            {
                return Err(missing());
            }
            if scene.workspace.selected_chat.as_ref() != Some(&chat) {
                let old = scene.workspace.selected_chat.replace(chat);
                let presentation = scene.presentation.take();
                scene.workspace.invalidate();
                if let Err(e) = self.persist(owner, &mut scene, false) {
                    scene.workspace.generation -= 1;
                    scene.workspace.selected_chat = old;
                    scene.presentation = presentation;
                    return Err(e);
                }
                scene.linked.clear();
                scene.workspace.messages = messages(user, scene.workspace.selected_chat.as_deref());
            }
            return Ok(view(&scene));
        }
        if action == "reader/state" {
            return Ok(view(&scene));
        }
        if action == "reader/paper_minimap.state" {
            let mut value = view(&scene);
            value["result"] = json!({"base":scene.workspace.book.paper_minimap(),"state":scene.workspace.reader.paper_minimap_state()});
            return Ok(value);
        }
        if matches!(
            action,
            "reader/pdf_selection.resolve" | "reader/pdf_ranges.project"
        ) {
            let reply = if action.ends_with("resolve") {
                crate::route_pdf_selection_resolve(
                    &scene.workspace.book,
                    &scene.workspace.book_dir,
                    input,
                )
            } else {
                crate::route_pdf_ranges_project(
                    &scene.workspace.book,
                    &scene.workspace.book_dir,
                    input,
                )
            };
            if reply.status != 200 {
                return Err(invalid());
            }
            return serde_json::from_str(&reply.body).map_err(|_| storage());
        }
        if matches!(action, "presentation/feedback/read" | "presentation/feedback/set"
            | "presentation/feedback/clear" | "presentation/feedback/attach_state") {
            let session = input["session_id"].as_str().ok_or_else(invalid)?;
            let turn = input["turn_id"].as_str().ok_or_else(invalid)?;
            if scene.workspace.selected_chat.as_deref() != Some(session) { return Err(stale()); }
            let result = with_turn_presentation(user, &scene.workspace, library, session, turn, |private| {
                crate::workspace_client::reply(crate::presentation_api::feedback_route(
                    private, &input.to_string(), action.rsplit('/').next().unwrap(), now,
                ))
            })?;
            let mut value = view(&scene); value["result"] = result; return Ok(value);
        }
        if action == "presentation/interaction" {
            let result = if crate::presentation_interact::is_board_command(&scene.workspace, input) {
                crate::presentation_interact::respond(&scene.workspace, &private_context(user, &scene.workspace), input)?
            } else if let Some(receipt) = input.get("receipt").filter(|r| !r.is_null()) {
                with_turn_presentation(user, &scene.workspace, library,
                    receipt["session_id"].as_str().ok_or_else(invalid)?,
                    receipt["turn_id"].as_str().ok_or_else(invalid)?,
                    |private| crate::presentation_interact::respond(&scene.workspace, private, input))?
            } else {
                crate::presentation_interact::respond(&scene.workspace, &private_context(user, &scene.workspace), input)?
            };
            let mut value = view(&scene); value["result"] = result; return Ok(value);
        }
        if action == "music/respond" {
            let result = crate::music_interact::respond(&scene.workspace, input)?;
            let mut value = view(&scene); value["result"] = result; return Ok(value);
        }
        if matches!(action, "presentation/read" | "presentation/observe") {
            let session = input["session_id"].as_str().ok_or_else(invalid)?;
            let turn = input["turn_id"].as_str().ok_or_else(invalid)?;
            if scene.workspace.selected_chat.as_deref() != Some(session) { return Err(stale()); }
            let mut request = input.clone();
            // An explicit receipt previews that snapshot without moving the workspace.
            if request["saved_state"].is_null() {
                request["saved_state"] = scene.presentation.as_ref().filter(|saved|
                    saved.session_id == session && saved.turn_id == turn && json!(saved.reference) == input["reference"]
                ).map(|s| json!(s)).unwrap_or(Value::Null);
            }
            let result = with_turn_presentation(user, &scene.workspace, library, session, turn, |private| {
                crate::workspace_client::reply(crate::presentation_api::route_with_restore(
                    private, &request.to_string(), action == "presentation/observe", false
                ))
            })?;
            let mut value = view(&scene); value["result"] = result; return Ok(value);
        }
        if matches!(action, "profile/memory" | "profile/memory/apply" | "agent/goals/cancel") {
            let reply = match action {
                "profile/memory" => crate::user_profile_memory_state(user, &scene.workspace, now),
                "profile/memory/apply" => {
                    if scene.workspace.selected_chat.is_none() { return Err(invalid()); }
                    crate::user_profile_memory_apply(user, &mut scene.workspace, &input.to_string(), now)
                }
                _ => crate::user_goal_cancel(user, &scene.workspace, &input.to_string(), now),
            };
            let mut value = view(&scene);
            value["result"] = crate::workspace_client::reply(reply)?;
            return Ok(value);
        }
        if matches!(action, "agent/source.resolve" | "agent/source.read" | "agent/source.open") {
            let source = if action == "agent/source.read" { &input["source"] } else { input };
            let request = crate::parse_agent_source_request(&source.to_string()).map_err(|r| crate::workspace_client::reply(r).unwrap_err())?;
            let mut binding = crate::workspace_source_binding(user, &scene.workspace, &request)?;
            // Older delivered pages used an implicit current-book source. Resolve it
            // against their saved turn, not whichever material the Reader now displays.
            if binding.publication_id.is_none() && request.note_mem_id.is_none() {
                if let Some(reference) = user.agent_history.sessions.iter()
                    .filter(|s| s.matches_scope(scene.workspace.learning_workspace_id.as_deref(), &scene.workspace.book.base.book_id))
                    .flat_map(|s| &s.turns).find(|t| t.turn_id == request.turn_id)
                    .and_then(|t| t.published_book_ref.as_ref())
                    .filter(|r| r.book_id == binding.book_id && (action == "agent/source.read" || scene.workspace.publication.as_ref().map(|p| &p.reference) != Some(*r))) {
                    binding.publication_id = Some(reference.publication_id.clone());
                }
            }
            if action == "agent/source.read" {
                let mut value = view(&scene);
                let publication = scene.workspace.publication.as_ref().map(|p| &p.reference);
                value["result"] = if crate::board_api::validate_source_publication(&scene.workspace.book, &binding, publication).is_ok() {
                    crate::board_api::workspace_source_window(&scene.workspace, &binding, input)?
                } else {
                    crate::cross_book::source_window(library, owner, &binding, input)?
                };
                return Ok(value);
            }
            if binding.publication_id.is_some() {
                let mut value = view(&scene);
                value["result"] = crate::cross_book::source_view(library, owner, &binding, action.ends_with(".open"))?;
                return Ok(value);
            }
        }
        if action == "chat/history" {
            let mut value = view(&scene);
            value["result"] = crate::workspace_client::history_with_library(user, &scene.workspace, library, owner)?;
            return Ok(value);
        }
        if let Some(result) = crate::workspace_client::read(user, &scene.workspace, action, input, now) {
            let mut value = view(&scene);
            value["result"] = result?;
            return Ok(value);
        }
        if action == "agent/effect/dispose" {
            let disposition = match crate::effect_disposition::start(user, &scene.workspace, input, now)? {
                crate::effect_disposition::Start::Existing(value) => value,
                crate::effect_disposition::Start::Execute(reference, record) => {
                    let previous = scene.workspace.reader.clone();
                    let receipt = crate::effect_disposition::execute(user, &mut scene.workspace, &record, now);
                    if let Err(e) = self.persist(owner, &mut scene, false) {
                        scene.workspace.reader = previous;
                        return Err(e);
                    }
                    crate::effect_disposition::complete(user, &reference, &record, receipt, now)?
                }
            };
            let mut value = view(&scene); value["result"] = json!(disposition);
            return Ok(value);
        }
        if action.starts_with("memory/") {
            let result = crate::workspace_client::memory(user, &scene.workspace, action, input, now)?;
            let mut value = view(&scene); value["result"] = result;
            return Ok(value);
        }
        // Reader is a candidate; a failed checkpoint leaves the visible committed scene intact.
        let mut reader = scene.workspace.reader.clone();
        let mut presentation = scene.presentation.clone();
        let book = &scene.workspace.book;
        let result = match action {
            "notes/source.open" => {
                let binding = crate::workspace_notes_api::source_binding(user, &scene.workspace, input)?;
                let range = binding.evidence_range.as_ref().ok_or_else(invalid)?;
                book.resolve_source(range, "zh-CN", Some(&binding.evidence_text_digest))?;
                reader.goto_lid(book, &mut user.store, &range.start_lid, now)?;
                json!({"source_ref_id":binding.source_ref_id,"opened":true})
            }
            "boards/source.open" => {
                let binding = crate::board_api::source_binding(user, &scene.workspace, input)?;
                let range = binding.evidence_range.as_ref().ok_or_else(invalid)?;
                book.resolve_source(range, "zh-CN", Some(&binding.evidence_text_digest))?;
                let lid = crate::board_api::optional_source_lid(input, "lid")?.unwrap_or(&range.start_lid);
                reader.goto_lid(book, &mut user.store, lid, now)?;
                json!({"source_ref_id":binding.source_ref_id,"opened":true})
            }
            "agent/source.open" => {
                let request = crate::parse_agent_source_request(&input.to_string()).map_err(|r| crate::workspace_client::reply(r).unwrap_err())?;
                let binding = crate::workspace_source_binding(user, &scene.workspace, &request)?;
                let range = binding.evidence_range.as_ref().ok_or_else(invalid)?;
                book.resolve_source(range, "zh-CN", Some(&binding.evidence_text_digest))?;
                reader.goto_lid(book, &mut user.store, &range.start_lid, now)?;
                json!({"source_ref_id":binding.source_ref_id,"opened":true})
            }
            "reader/position" => json!(reader.confirm_visible_position(book, &mut user.store,
                serde_json::from_value(input.clone()).map_err(|_| invalid())?, now)?),
            "reader/goto" => {
                let effect = reader.goto_lid(book, &mut user.store,
                    input["lid"].as_str().ok_or_else(invalid)?, now)?;
                crate::reader_navigation_result(book, &effect, input["include_text"].as_bool() == Some(true))?
            },
            "reader/scroll" => json!(reader.scroll(
                book,
                &mut user.store,
                input["delta"].as_i64().ok_or_else(invalid)?,
                now
            )?),
            "reader/note" => json!(reader.note(
                book,
                &mut user.store,
                input["lid"].as_str().ok_or_else(invalid)?,
                input["text"].as_str().ok_or_else(invalid)?,
                "long_term",
                now
            )?),
            "reader/highlight" => {
                let range = input
                    .get("range")
                    .map(|r| -> Result<_, ToolError> {
                        Ok((
                            u32::try_from(r["start"].as_u64().ok_or_else(invalid)?)
                                .map_err(|_| invalid())?,
                            u32::try_from(r["end"].as_u64().ok_or_else(invalid)?)
                                .map_err(|_| invalid())?,
                        ))
                    })
                    .transpose()?;
                json!(reader.highlight(
                    book,
                    &mut user.store,
                    input["lid"].as_str().ok_or_else(invalid)?,
                    range,
                    input["source_session_id"].as_str()
                        .filter(|id| id.starts_with("highlight-group:"))
                        .map(str::to_owned)
                        .or_else(|| scene.workspace.selected_chat.clone()),
                    "long_term",
                    now
                )?)
            }
            "reader/layout.apply" => {
                if let Some(proposal) = input["proposal_id"].as_str() {
                    json!(reader.apply_layout_proposal(
                        book,
                        proposal,
                        input["base_layout_rev"].as_u64().ok_or_else(invalid)?
                    )?)
                } else {
                    json!(reader.apply_layout_actions(
                        book,
                        serde_json::from_value(input["actions"].clone()).map_err(|_| invalid())?
                    )?)
                }
            }
            "reader/paper_minimap.apply" => {
                let rev = input["base_state_rev"].as_u64().ok_or_else(invalid)?;
                if let Some(id) = input["undo_effect_id"].as_str() {
                    json!(reader.undo_paper_minimap_effect_by_id(id, rev, now)?)
                } else if let Some(id) = input["dismiss_proposal_id"].as_str() {
                    json!(reader.dismiss_paper_minimap_proposal(
                        id,
                        input["base_map_rev"].as_str().ok_or_else(invalid)?,
                        rev
                    )?)
                } else if let Some(id) = input["proposal_id"].as_str() {
                    json!(reader.apply_paper_minimap_proposal(
                        book,
                        id,
                        input["base_map_rev"].as_str().ok_or_else(invalid)?,
                        rev,
                        now
                    )?)
                } else {
                    json!(reader.apply_paper_minimap_commands(
                        book,
                        rev,
                        reader::PaperMinimapActor::User,
                        serde_json::from_value(input["commands"].clone()).map_err(|_| invalid())?,
                        "user minimap action",
                        vec![],
                        None,
                        now
                    )?)
                }
            }
            "checkpoint" => {
                if let Some(lid) = input["top_lid"].as_str() {
                    if !book
                        .base
                        .lid_nodes
                        .iter()
                        .any(|n| n.lid == lid && n.children.is_empty())
                    {
                        return Err(invalid());
                    }
                    reader.goto_lid(book, &mut user.store, lid, now)?;
                }
                json!({"saved":true})
            }
            "presentation/save" => {
                if input["session_id"].as_str() != scene.workspace.selected_chat.as_deref() {
                    return Err(stale());
                }
                let saved = with_turn_presentation(
                    user,
                    &scene.workspace,
                    library,
                    input["session_id"].as_str().ok_or_else(invalid)?,
                    input["turn_id"].as_str().ok_or_else(invalid)?,
                    |private| crate::workspace_client::reply(crate::presentation_api::save_browser_state(
                        &scene.workspace, private, &input.to_string(), now,
                    )),
                )?;
                presentation = Some(serde_json::from_value(saved).map_err(|_| storage())?);
                json!(presentation)
            }
            "presentation/restore" => {
                let saved: runtime::presentation::PresentationFollowUp =
                    serde_json::from_value(input["saved_state"].clone()).map_err(|_| invalid())?;
                with_turn_presentation(
                    user,
                    &scene.workspace,
                    library,
                    &saved.session_id,
                    &saved.turn_id,
                    |private| crate::presentation_api::follow_up_context(private, &saved),
                )?;
                presentation = Some(saved);
                json!(presentation)
            }
            _ => return Err(crate::authorization::deferred()),
        };
        user.store.flush_pending_reads()?;
        let old_reader = std::mem::replace(&mut scene.workspace.reader, reader);
        let old_presentation = std::mem::replace(&mut scene.presentation, presentation);
        if let Err(e) = self.persist(owner, &mut scene, false) {
            scene.workspace.reader = old_reader;
            scene.presentation = old_presentation;
            return Err(e);
        }
        let mut value = view(&scene);
        value["result"] = result;
        Ok(value)
    }
}

impl ResidentWorkspace {
    fn check_attachment(&self, stamp: &WorkspaceStamp) -> Result<(), ToolError> {
        if self.workspace.generation != stamp.generation
            || (self.attachment.as_ref() != Some(&stamp.attachment_id)
                && !self.linked.contains(&stamp.attachment_id))
        {
            return Err(stale());
        }
        Ok(())
    }
    pub(crate) fn check(&self, stamp: &WorkspaceStamp) -> Result<(), ToolError> {
        self.check_attachment(stamp)?;
        if self.revision != stamp.expected_revision {
            return Err(conflict());
        }
        Ok(())
    }
}
fn attachment(input: &Value) -> Result<String, ToolError> {
    input["attachment_id"]
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 128)
        .map(str::to_owned)
        .ok_or_else(invalid)
}
fn reference(
    input: &Value,
    library: &Mutex<PublishedLibrary>,
) -> Result<PublishedBookRef, ToolError> {
    if let Some(value) = input.get("published_book_ref") {
        serde_json::from_value(value.clone()).map_err(|_| invalid())
    } else {
        library
            .lock()
            .unwrap()
            .default_ref(input["book_id"].as_str().ok_or_else(invalid)?)
    }
}
fn messages(user: &UserRuntime, chat: Option<&str>) -> Vec<runtime::Message> {
    user.agent_history
        .sessions
        .iter()
        .find(|s| Some(s.id.as_str()) == chat)
        .map(|s| s.messages.clone())
        .unwrap_or_default()
}
/// Historical pages retain their original source even when a shared chat is opened
/// from another material. The selected chat, its scope and the original grant still apply.
fn with_turn_presentation<R>(
    user: &UserRuntime,
    w: &ReaderWorkspace,
    library: &Mutex<PublishedLibrary>,
    chat: &str,
    turn: &str,
    operation: impl FnOnce(&crate::PrivateBookContext<'_>) -> Result<R, ToolError>,
) -> Result<R, ToolError> {
    if w.selected_chat.as_deref() != Some(chat) { return Err(stale()); }
    let session = user.agent_history.sessions.iter()
        .find(|s| s.id == chat && s.matches_scope(w.learning_workspace_id.as_deref(), &w.book.base.book_id))
        .ok_or_else(missing)?;
    let reference = session.turns.iter().find(|t| t.turn_id == turn)
        .and_then(|t| t.published_book_ref.as_ref()).ok_or_else(missing)?;
    let publication = library.lock().unwrap().load(user.user_id().ok_or_else(missing)?, reference)?;
    operation(&crate::PrivateBookContext {
        user,
        learning_workspace_id: w.learning_workspace_id.as_deref(),
        book: &publication.book,
        book_dir: publication.directory(),
        messages: &session.messages,
        selected_chat: Some(chat),
    })
}

fn require_turn_publication(
    user: &UserRuntime,
    w: &ReaderWorkspace,
    chat: &str,
    turn: &str,
) -> Result<(), ToolError> {
    let saved = user
        .agent_history
        .sessions
        .iter()
        .find(|s| s.id == chat && s.matches_scope(w.learning_workspace_id.as_deref(), &w.book.base.book_id))
        .and_then(|s| s.turns.iter().find(|t| t.turn_id == turn))
        .ok_or_else(missing)?;
    if saved.published_book_ref.as_ref() != w.publication.as_ref().map(|p| &p.reference) {
        return Err(error(
            "PUBLICATION_BINDING_MISMATCH",
            "conflict",
            "Open the original publication for this turn",
        ));
    }
    Ok(())
}
fn private_context<'a>(
    user: &'a UserRuntime,
    w: &'a ReaderWorkspace,
) -> crate::PrivateBookContext<'a> {
    crate::PrivateBookContext {
        learning_workspace_id: w.learning_workspace_id.as_deref(),
        user,
        book: &w.book,
        book_dir: &w.book_dir,
        messages: &w.messages,
        selected_chat: w.selected_chat.as_deref(),
    }
}
fn learning_scope(value: Option<&Value>) -> Result<Option<String>, ToolError> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value.as_str().filter(|id| !id.is_empty()).map(|id| Some(id.to_owned())).ok_or_else(invalid),
    }
}

fn view(scene: &ResidentWorkspace) -> Value {
    let positions: BTreeMap<_, _> = scene.workspace.learning_reading_positions.iter()
        .map(|p| (&p.learning_workspace_id, &p.published_book_ref)).collect();
    json!({"workspace_id":scene.workspace.id,"generation":scene.workspace.generation,"revision":scene.revision,
        "learning_workspace_id":scene.workspace.learning_workspace_id,
        "learning_workspace_goal":scene.workspace.learning_goal,"learning_workspace_positions":positions,
        "published_book_ref":scene.workspace.publication.as_ref().map(|p| &p.reference),"selected_chat":scene.workspace.selected_chat,
        "reader":crate::reader_state_response(&scene.workspace.book,&scene.workspace.reader),"presentation":scene.presentation})
}

/// The MU6 executor receives this port after admission. It cannot borrow local AppState.
pub(crate) struct NetworkRunPort {
    previewed: std::collections::HashMap<String, std::collections::HashSet<String>>,
    animations: std::collections::HashMap<String, crate::presentation_animation::RenderedAnimation>,
    plots: std::collections::HashMap<String, crate::presentation_plot::PlotAsset>,
    access: Arc<crate::authorization::Authorization>,
    user: crate::user_registry::UserHandle,
    scene: Option<WorkspaceHandle>,
    pub scope: crate::run_scope::RunScope,
    turn: crate::AgentTurnRef,
    effect_revision: u64,
}
impl NetworkRunPort {
    fn media_publication(&self, reference: &base_schema::PublishedBookRef) -> Result<Arc<crate::published_library::PublishedBook>, ToolError> {
        self.authorized()?;
        self.scope.check_user(&self.user.lock().unwrap())?;
        crate::cross_book::check_member(self.scope.members.as_deref(), reference)?;
        self.access.library.lock().unwrap().load(&self.scope.user_id, reference)
    }
    pub(crate) fn capture(
        access: Arc<crate::authorization::Authorization>,
        context: AuthorizedContext,
        id: &str,
        stamp: WorkspaceStamp,
        turn: crate::AgentTurnRef,
        profile: runtime::ModelRuntimeProfile,
        message: &str,
    ) -> Result<Self, ToolError> {
        access
            .auth
            .validate(&context.principal, crate::multi_user_host::now())?;
        let user = context.user.lock().unwrap();
        let scene = access.workspaces.lock().unwrap().scene(
            context.user_id(),
            id,
            &user,
            &access.library,
        )?;
        let w = scene.lock().unwrap();
        w.check(&stamp)?;
        require_turn_publication(&user, &w.workspace, &turn.session_id, &turn.turn_id)?;
        let saved_turn = user.agent_history.sessions.iter()
            .find(|session| session.id == turn.session_id)
            .and_then(|session| session.turns.iter().find(|saved| saved.turn_id == turn.turn_id))
            .ok_or_else(missing)?;
        let scope = crate::run_scope::RunScope {
            board_context: saved_turn.domain.board_context.clone(),
            learning_workspace_id: w.workspace.learning_workspace_id.clone(),
            goal: w.workspace.learning_goal.clone(), members: w.workspace.learning_members.clone(),
            publication: w.workspace.publication.clone(),
            user_id: context.user_id().into(),
            workspace_id: id.into(),
            workspace_generation: w.workspace.generation,
            chat_session_id: turn.session_id.clone(),
            turn_id: turn.turn_id.clone(),
            confirmation_id: user
                .agent_history
                .confirmation_id(&turn.session_id)
                .map(str::to_owned),
            book: Some(w.workspace.book.clone()),
            book_dir: w.workspace.book_dir.clone(),
            provider_binding: profile,
            reader_input: Some(runtime::run_context::ReaderInputSnapshot::capture(
                &w.workspace.book,
                &w.workspace.reader,
                message,
            )),
            question_quote: None,
            music_question: None,
            presentation_follow_up: w.presentation.clone(),
        };
        scope.check_user(&user)?;
        scope.check_scene(&w.workspace)?;
        let effect_revision = w.revision;
        drop(w);
        drop(user);
        Ok(Self {
            previewed: Default::default(), animations: Default::default(), plots: Default::default(),
            access,
            user: context.user,
            scene: Some(scene),
            scope,
            turn,
            effect_revision,
        })
    }
    pub(crate) fn resume(
        access: Arc<crate::authorization::Authorization>,
        user: crate::user_registry::UserHandle,
        scope: crate::run_scope::RunScope,
        turn: crate::AgentTurnRef,
        effect_revision: u64,
    ) -> Result<Self, ToolError> {
        if scope.book.is_none() {
            scope.check_user(&user.lock().unwrap())?;
            return Ok(Self {previewed:Default::default(),animations:Default::default(),plots:Default::default(),access,user,scene:None,scope,turn,effect_revision});
        }
        let guard = user.lock().unwrap();
        let mut registry = access.workspaces.lock().unwrap();
        let record = registry.record(&scope.user_id, &scope.workspace_id)?;
        let scene = if record.generation != scope.workspace_generation
            || record.reference != scope.publication.as_ref().unwrap().reference
        {
            registry
                .residents
                .get(&(scope.user_id.clone(), scope.workspace_id.clone()))
                .cloned()
        } else {
            Some(registry.scene(&scope.user_id, &scope.workspace_id, &guard, &access.library)?)
        };
        drop(registry);
        drop(guard);
        Ok(Self {
            previewed: Default::default(), animations: Default::default(), plots: Default::default(),
            access,
            user,
            scene,
            scope,
            turn,
            effect_revision,
        })
    }
    pub(crate) fn observe(&self, stream: &Arc<crate::agent_stream::RunStream>) {
        if let Some(scene) = &self.scene {
            let mut scene = scene.lock().unwrap();
            if self.scope.check_scene(&scene.workspace).is_ok() {
                scene.workspace.active_agent_stream = Some(Arc::downgrade(stream));
            }
        }
    }
    fn authorized(&self) -> Result<(), ToolError> {
        if let Some(publication)=&self.scope.publication {self.access.library.lock().unwrap().authorize(&self.scope.user_id,&publication.reference).map_err(|_|missing())?;}
        Ok(())
    }
    fn private<R>(
        &self,
        operation: impl FnOnce(&crate::PrivateBookContext<'_>) -> Result<R, ToolError>,
    ) -> Result<R, ToolError> {
        self.authorized()?;
        let user = self.user.lock().unwrap();
        self.scope.check_user(&user)?;
        operation(&self.scope.private_user(&user))
    }
    pub(crate) fn restore_presentation(
        &mut self,
        saved: runtime::presentation::PresentationFollowUp,
    ) -> Result<(), ToolError> {
        self.authorized()?;
        let user = self.user.lock().unwrap();
        self.scope.check_user(&user)?;
        let mut registry = self.access.workspaces.lock().unwrap();
        let mut scene = self.scene.as_ref().ok_or_else(stale)?.lock().unwrap();
        self.scope.check_scene(&scene.workspace)?;
        self.check_effect_revision(&scene)?;
        with_turn_presentation(&user, &scene.workspace, &self.access.library,
            &saved.session_id, &saved.turn_id,
            |private| crate::presentation_api::follow_up_context(private, &saved))?;
        let old = scene.presentation.replace(saved);
        if let Err(e) = registry.persist(&self.scope.user_id, &mut scene, false) {
            scene.presentation = old;
            return Err(e);
        }
        self.effect_revision = scene.revision;
        Ok(())
    }
    fn check_effect_revision(&self, scene: &ResidentWorkspace) -> Result<(), ToolError> {
        if scene.revision != self.effect_revision {
            return Err(error(
                "READER_USER_ACTION_SUPERSEDED",
                "conflict",
                "A newer user action takes priority over this Run effect",
            ));
        }
        Ok(())
    }
}
struct NetworkAuthorStorage<'a> {
    access: &'a crate::authorization::Authorization,
    user: &'a crate::user_registry::UserHandle,
    scope: &'a crate::run_scope::RunScope,
}
impl crate::presentation_author::AuthorStorage for NetworkAuthorStorage<'_> {
    fn with_private<R>(&self, operation: impl FnOnce(&crate::PrivateBookContext<'_>) -> Result<R, ToolError>) -> Result<R, ToolError> {
        self.access.library.lock().unwrap().authorize(&self.scope.user_id, &self.scope.publication.as_ref().ok_or_else(missing)?.reference)?;
        let user = self.user.lock().unwrap();
        self.scope.check_user(&user)?;
        operation(&self.scope.private_user(&user))
    }
}
impl crate::board_author::Storage for NetworkAuthorStorage<'_> {
    fn source_workspace(&self, reference: Option<&base_schema::PublishedBookRef>) -> Result<ReaderWorkspace, ToolError> {
        let publication = if let Some(reference) = reference {
            crate::cross_book::check_member(self.scope.members.as_deref(), reference)?;
            Some(self.access.library.lock().unwrap().load(&self.scope.user_id, reference)?)
        } else { self.scope.publication.clone() };
        Ok(crate::board_author::source_workspace(self.scope, publication))
    }
    fn with_user<R>(&self, operation: impl FnOnce(&mut crate::user_runtime::UserRuntime) -> Result<R, ToolError>) -> Result<R, ToolError> {
        self.access.library.lock().unwrap().authorize(&self.scope.user_id, &self.scope.publication.as_ref().ok_or_else(missing)?.reference)?;
        let mut user = self.user.lock().unwrap();
        self.scope.check_user(&user)?;
        operation(&mut user)
    }
}
impl runtime::run_context::ResidentStatePort for NetworkRunPort {
    fn workspace_notes_available(&self) -> bool { self.scope.learning_workspace_id.is_some() }
    fn workspace_notes(&mut self, request: Value) -> Result<Value, ToolError> {
        self.authorized()?;
        let user = self.user.lock().unwrap();
        self.scope.check_user(&user)?;
        crate::workspace_notes_api::agent(&user, &self.scope, request, &crate::multi_user_host::now().to_string())
    }
    fn personal_note_search(&mut self,request:Value)->Result<Value,ToolError>{
        self.authorized()?;
        let prepared={let user=self.user.lock().unwrap();self.scope.check_user(&user)?;crate::personal_note_retrieval::prepare(&user.store,&request)?};
        let vectors=crate::personal_note_retrieval::infer(&prepared);
        self.authorized()?;let user=self.user.lock().unwrap();self.scope.check_user(&user)?;
        crate::personal_note_retrieval::finish(&user.store,prepared,vectors)
    }
    fn personal_note_context(&mut self)->Result<Value,ToolError>{
        self.authorized()?;let mut user=self.user.lock().unwrap();self.scope.check_user(&user)?;
        crate::personal_note_context::for_turn(&mut user,&self.scope.chat_session_id,&self.scope.turn_id)
    }
    fn board_authoring_available(&self) -> bool { self.scope.board_context.is_some() }
    fn author_board(&mut self, request: runtime::board_author::AuthorRequest, bindings: &[runtime::orchestrator::SourceBinding], messages: &[runtime::Message], cancellation: &runtime::run_context::CancellationToken) -> Result<runtime::board_author::AuthorResult, ToolError> {
        self.authorized()?;
        let frozen = self.scope.board_context.as_ref().ok_or_else(runtime::board_author::unavailable)?;
        crate::board_author::execute(&NetworkAuthorStorage { access: &self.access, user: &self.user, scope: &self.scope }, frozen, &self.turn, request, bindings, messages, cancellation)
    }
    fn music_source_context(&mut self) -> Result<Option<Value>, ToolError> {
        self.authorized()?;
        self.scope.check_user(&self.user.lock().unwrap())?;
        crate::music_interact::source_context(&self.scope)
    }
    fn read_music(&mut self, reference: Option<&base_schema::PublishedBookRef>, request: read_tools::music::MusicRead) -> Result<read_tools::music::MusicReadResult, ToolError> {
        self.authorized()?;
        self.scope.check_user(&self.user.lock().unwrap())?;
        if let Some(reference) = reference {
            let publication = self.media_publication(reference)?;
            let manifest = publication.book.load_music_source(publication.directory())?;
            read_tools::music::read(&manifest, reference, &request)
        } else {
            let reference = self.scope.publication.as_ref().ok_or_else(runtime::music::unavailable)?.reference.clone();
            let manifest = self.scope.reading_book().load_music_source(&self.scope.book_dir)?;
            read_tools::music::read(&manifest, &reference, &request)
        }
    }
    fn music_example_source(&mut self, reference: &base_schema::PublishedBookRef) -> Result<base_schema::MusicSourceManifest, ToolError> {
        self.authorized()?;
        self.scope.check_user(&self.user.lock().unwrap())?;
        let publication = self.media_publication(reference)?;
        publication.book.load_music_source(publication.directory())
    }
    fn interact_music(&mut self, request: runtime::music::MusicInteractRequest, cancellation: &runtime::run_context::CancellationToken) -> Result<runtime::music::MusicPlaybackReceipt, ToolError> {
        self.authorized()?;
        let scene = self.scene.as_ref().ok_or_else(stale)?;
        let stream = {
            let scene = scene.lock().unwrap();
            self.scope.check_scene(&scene.workspace)?;
            scene.workspace.active_agent_stream.as_ref().and_then(|s| s.upgrade()).ok_or_else(runtime::music::unavailable)?
        };
        crate::music_interact::execute(&stream, request, cancellation,
            |commands, request| commands.prepare(&self.scope, &self.turn, request),
            || { self.authorized()?; self.scope.check_scene(&scene.lock().unwrap().workspace) })
    }

    fn web_available(&self) -> bool {
        crate::web_search::available()
    }

    fn web_call(&mut self, name: &str, arguments: Value, cancellation: &runtime::run_context::CancellationToken) -> Result<Value, ToolError> {
        self.authorized()?;
        self.scope.check_user(&self.user.lock().unwrap())?;
        // Network I/O never retains Reader locks; charging keeps the original
        // Run's account and attribution even when its visible scene changes.
        let result = crate::web_search::call(&self.scope, &self.access.spend, name, arguments, cancellation)?;
        self.authorized()?;
        self.scope.check_user(&self.user.lock().unwrap())?;
        Ok(result)
    }

    fn interact_presentation(&mut self, request: runtime::presentation_interact::InteractRequest, cancellation: &runtime::run_context::CancellationToken) -> Result<Value, ToolError> {
        self.authorized()?;
        let scene = self.scene.as_ref().ok_or_else(stale)?;
        let stream = {
            let scene = scene.lock().unwrap();
            self.scope.check_scene(&scene.workspace)?;
            scene.workspace.active_agent_stream.as_ref().and_then(|s| s.upgrade()).ok_or_else(runtime::presentation_interact::unavailable)?
        };
        let result = crate::presentation_interact::execute(&stream, request, cancellation,
            |commands, request| self.private(|state| crate::presentation_interact::prepare(commands, state, &self.turn, request)),
            || { self.authorized()?; self.scope.check_scene(&scene.lock().unwrap().workspace) });
        // The saved browser result uses the same workspace action path as human saves.
        // Do not advance effect authority past unrelated user navigation.
        result
    }

    fn workspace_records_available(&self) -> bool { true }
    fn workspace_records(&mut self, request: Value) -> Result<Value, ToolError> {
        self.authorized()?;
        let user = self.user.lock().unwrap();
        self.scope.check_user(&user)?;
        crate::workspace_record_context::read_scoped(&user, self.scope.learning_workspace_id.as_deref(),
            &self.scope.reading_book().base.book_id, request)
    }
    fn library_available(&self) -> bool { true }
    fn library_source(&mut self, reference: &base_schema::PublishedBookRef) -> Result<runtime::library::LibrarySource, ToolError> {
        let publication = self.media_publication(reference)?;
        Ok(runtime::library::LibrarySource { reference: reference.clone(), title: crate::book_title::read(publication.directory(), None).unwrap_or_else(|| "参考材料".into()), book: publication.book.clone() })
    }
    fn library_read(&mut self, name: &str, arguments: Value) -> Result<runtime::library::LibraryReadResult, ToolError> {
        self.authorized()?;
        crate::cross_book::read(&self.access.library, &self.scope.user_id, self.scope.members.as_deref(), name, arguments)
    }
    fn library_learning(&mut self, arguments:Value, bindings:&[runtime::orchestrator::SourceBinding]) -> Result<Value,ToolError> {
        self.authorized()?;
        let reference=serde_json::from_value(arguments["published_book_ref"].clone()).map_err(|e|memory::teaching::invalid(e.to_string()))?;
        crate::cross_book::check_member(self.scope.members.as_deref(), &reference)?;
        let publication=self.access.library.lock().unwrap().load(&self.scope.user_id,&reference)?;
        self.private(|state|crate::cross_book::learning(&crate::PrivateBookContext {user:state.user,book:&publication.book,book_dir:publication.directory(),
            learning_workspace_id:self.scope.learning_workspace_id.as_deref(), messages:state.messages,selected_chat:state.selected_chat},&self.turn,&reference,&arguments,bindings,&crate::multi_user_host::now().to_string()))
    }
    fn record_presentation_methods(&mut self, observations: &runtime::presentation::PresentationMethodObservations) -> Result<(), ToolError> {
        use crate::presentation_author::AuthorStorage;
        self.authorized()?;
        let storage = NetworkAuthorStorage { access: &self.access, user: &self.user, scope: &self.scope };
        storage.with_private(|state| state.record_presentation_methods(&self.turn.session_id, &self.turn.turn_id, observations))
    }
    fn media_source_snapshot(&mut self, target: base_schema::EvidenceTarget) -> Result<runtime::video_source::MediaSource, ToolError> {
        let reference = runtime::video_source::publication(&target).ok_or_else(missing)?;
        let publication = self.media_publication(reference)?;
        crate::video_source::snapshot(&publication.book, publication.directory(), target)
    }
    fn media_source_for_text(&mut self, range: &read_tools::EvidenceRange) -> Result<Option<runtime::video_source::MediaSource>, ToolError> {
        self.authorized()?;
        crate::video_source::text_snapshot(&self.scope.reading_book(), &self.scope.book_dir, range)
    }
    fn library_media_source_for_text(&mut self, reference: &base_schema::PublishedBookRef, range: &read_tools::EvidenceRange) -> Result<Option<runtime::video_source::MediaSource>, ToolError> {
        self.authorized()?;
        crate::cross_book::check_member(self.scope.members.as_deref(), reference)?;
        let publication = self.access.library.lock().unwrap().load(&self.scope.user_id, reference)?;
        crate::video_source::text_snapshot(&publication.book, publication.directory(), range)
    }
    fn video_source_context(&mut self) -> Result<Option<serde_json::Value>, ToolError> {
        self.authorized()?;
        self.scope.check_user(&self.user.lock().unwrap())?;
        crate::video_frames::source_context(&self.scope)
    }
    fn review_video_audio(&mut self, reference: Option<&base_schema::PublishedBookRef>, request: runtime::video_audio::ReviewAudio, cancellation: &runtime::run_context::CancellationToken) -> Result<runtime::video_audio::AudioReview, ToolError> {
        self.authorized()?;
        self.scope.check_user(&self.user.lock().unwrap())?;
        let result = if let Some(reference) = reference {
            let publication = self.media_publication(reference)?;
            let result = crate::video_audio::review_audio_metered(&publication.book, publication.directory(), reference, &self.scope.turn_id, request, cancellation, &crate::video_audio::AudioConfig::from_env()?,Some((&self.access.spend,&self.scope)))?;
            self.media_publication(reference)?;
            result
        } else { crate::video_audio::review_for_run_metered(&self.scope, request, cancellation,Some(&self.access.spend))? };
        self.authorized()?;
        self.scope.check_user(&self.user.lock().unwrap())?;
        Ok(result)
    }
    fn search_video(&mut self, reference: Option<&base_schema::PublishedBookRef>, request: read_tools::video_search::VideoSearch) -> Result<serde_json::Value, ToolError> {
        self.authorized()?;
        self.scope.check_user(&self.user.lock().unwrap())?;
        let result = if let Some(reference) = reference {
            let publication = self.media_publication(reference)?;
            let mut result = publication.book.search_video(publication.directory(), reference, request)?;
            crate::cross_book::scope_video_hints(&mut result, reference);
            result
        } else { crate::video_frames::search_for_run(&self.scope, request)? };
        self.authorized()?;
        Ok(result)
    }
    fn read_video_frames(&mut self, reference: Option<&base_schema::PublishedBookRef>, request: runtime::video::ReadFrames, cancellation: &runtime::run_context::CancellationToken) -> Result<runtime::video::FrameBatch, ToolError> {
        self.authorized()?;
        self.scope.check_user(&self.user.lock().unwrap())?;
        let result = if let Some(reference) = reference {
            let publication = self.media_publication(reference)?;
            let cache = publication.localization_cache.parent().unwrap().join("video-frames");
            let result = crate::video_frames::read_frames(&publication.book, publication.directory(), reference, &self.scope.turn_id, &cache, request, cancellation)?;
            self.media_publication(reference)?;
            result
        } else { crate::video_frames::read_for_run(&self.scope, request, cancellation)? };
        self.authorized()?;
        Ok(result)
    }
    fn presentation_unavailable_reason(&self) -> Option<&'static str> {
        self.access.sandbox.unavailable_reason()
    }

    fn author_presentation(&mut self, request: runtime::presentation_author::AuthorRequest,
        bindings: &[runtime::orchestrator::SourceBinding], messages: &[runtime::Message], cancellation: &runtime::run_context::CancellationToken,
    ) -> Result<runtime::presentation_author::AuthorResult, ToolError> {
        self.authorized()?;
        self.access.sandbox.require()?;
        let storage = NetworkAuthorStorage { access: &self.access, user: &self.user, scope: &self.scope };
        crate::presentation_author::AuthorSession { storage: &storage, turn_ref: &self.turn,
            previewed: &mut self.previewed, animations: &mut self.animations, plots: &mut self.plots,
            sandbox: Some(crate::presentation_sandbox::Execution { sandbox: &self.access.sandbox, resources: &self.access.resources, owner: &self.scope.user_id }),
        }.author(request, bindings, messages, cancellation)
    }

    fn submit_private<R>(
        &mut self,
        operation: impl FnOnce(&mut memory::MemoryStore) -> R,
    ) -> Result<R, ToolError> {
        self.authorized()?;
        let mut user = self.user.lock().unwrap();
        self.scope.check_user(&user)?;
        Ok(operation(&mut user.store))
    }
    fn read_live_reader<R>(
        &mut self,
        operation: impl FnOnce(&reader::Reader) -> R,
    ) -> Result<R, ToolError> {
        self.authorized()?;
        let user = self.user.lock().unwrap();
        self.scope.check_user(&user)?;
        let scene = self.scene.as_ref().ok_or_else(stale)?.lock().unwrap();
        self.scope.check_scene(&scene.workspace)?;
        Ok(operation(&scene.workspace.reader))
    }
    fn apply_reader<R>(
        &mut self,
        operation: impl FnOnce(&mut memory::MemoryStore, &mut reader::Reader) -> R,
    ) -> Result<R, ToolError> {
        self.authorized()?;
        let mut user = self.user.lock().unwrap();
        self.scope.check_user(&user)?;
        let mut registry = self.access.workspaces.lock().unwrap();
        let mut scene = self.scene.as_ref().ok_or_else(stale)?.lock().unwrap();
        self.scope.check_scene(&scene.workspace)?;
        self.check_effect_revision(&scene)?;
        let mut reader = scene.workspace.reader.clone();
        let result = operation(&mut user.store, &mut reader);
        user.store.flush_pending_reads()?;
        let old = std::mem::replace(&mut scene.workspace.reader, reader);
        if let Err(e) = registry.persist(&self.scope.user_id, &mut scene, false) {
            scene.workspace.reader = old;
            return Err(e);
        }
        self.effect_revision = scene.revision;
        if let Some(stream) = scene
            .workspace
            .active_agent_stream
            .as_ref()
            .and_then(|s| s.upgrade())
        {
            stream.reader_changed(crate::reader_state_response(
                &self.scope.reading_book(),
                &scene.workspace.reader,
            ));
        }
        Ok(result)
    }
    fn reader_input(
        &mut self,
        _: &read_tools::Book,
        _: &str,
    ) -> runtime::run_context::ReaderInputSnapshot {
        self.scope.reader_input.clone().expect("reading scope")
    }
    fn tutor_active(&mut self) -> Result<bool, ToolError> {
        if let Some(id) = &self.scope.learning_workspace_id {
            let user = self.user.lock().unwrap();
            self.access.sync_learning_goal(&user, id, &crate::multi_user_host::now().to_string())?;
        }
        self.private(|p| crate::teaching::turn_active(p, &self.turn.turn_id))
    }
    fn tutor_step_preflight(
        &self,
        request: &Value,
        evidence: &[runtime::orchestrator::SourceBinding],
        ranges: &[read_tools::EvidenceRange],
    ) -> Result<Value, runtime::run_context::TeachingSelectionPreflightError> {
        self.authorized()?;
        let user = self.user.lock().unwrap();
        self.scope.check_user(&user)?;
        crate::teaching::preflight_step(&self.scope.private_user(&user), &self.turn, request, evidence, ranges)
    }
    fn tutor_step(
        &mut self,
        request: Value,
        evidence: &[runtime::orchestrator::SourceBinding],
        ranges: &[read_tools::EvidenceRange],
    ) -> Result<Value, ToolError> {
        self.private(|p| crate::teaching::step(p, &self.turn, request, evidence, ranges))
    }
    fn tutor_assessment_input(&mut self, action: &str) -> Result<Value, ToolError> {
        self.private(|p| crate::teaching::assessment_input(p, &self.turn, action))
    }
    fn tutor_record_review(&mut self, record: Value) -> Result<Value, ToolError> {
        self.private(|p| crate::teaching::record_agent_review(p,&self.turn,record))
    }
    fn tutor_working_state(&mut self) -> Result<Value, ToolError> {
        self.private(|p| crate::teaching::agent_working_state(p,&self.turn))
    }
    fn tutor_coach_context(&mut self) -> Result<Option<Value>, ToolError> {
        self.private(|p| crate::teaching_coach::turn_context(p.user,p.learning_workspace_id,&self.turn))
    }
    fn tutor_assessment_accept(&mut self, action: &str, items: Value) -> Result<Value, ToolError> {
        self.private(|p| crate::teaching::assessment_accept(p, &self.turn, action, items))
    }
    fn persist_goal(&mut self, goal: &runtime::goal::ResidentGoal) -> Result<(), ToolError> {
        self.authorized()?;
        let mut user = self.user.lock().unwrap();
        self.scope.check_user(&user)?;
        if user.session_store.is_some() {
            return crate::session_runtime::append(&mut user, &self.turn,
                &crate::multi_user_host::now().to_string(), crate::session_event::EventBody::GoalUpdated { goal: goal.clone() });
        }
        let mut candidate = user.agent_history.clone();
        let stored = candidate
            .sessions
            .iter_mut()
            .find(|s| s.id == self.scope.chat_session_id)
            .and_then(|s| s.goals.iter_mut().find(|g| g.id == goal.id))
            .ok_or_else(missing)?;
        if stored.revision > goal.revision {
            return Err(conflict());
        }
        *stored = goal.clone();
        crate::save_agent_history_path(&user.history_path, &candidate)?;
        user.agent_history = candidate;
        Ok(())
    }
    fn update_workspace_goal(&mut self, update: &runtime::workspace_goal::WorkspaceGoalUpdate, current_user: &str) -> Result<base_schema::WorkspaceGoal, ToolError> {
        self.authorized()?;
        let mut user = self.user.lock().unwrap();
        self.access.revise_goal_from_run(&mut user, &self.scope, &self.turn, update, current_user, crate::multi_user_host::now())
    }
}
