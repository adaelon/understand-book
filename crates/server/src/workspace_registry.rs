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
    version: u32,
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
pub(crate) struct ResidentWorkspace {
    pub workspace: ReaderWorkspace,
    pub(crate) revision: u64,
    attachment: Option<String>,
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
        Checkpoint {
            version: 1,
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
        let reference = &scene.workspace.publication.as_ref().unwrap().reference;
        let saved = serde_json::to_string(&Self::checkpoint(scene)).map_err(|_| storage())?;
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
        scene.revision += 1;
        scene.touched = Instant::now();
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
                .any(|s| &s.id == id && s.book_id == record.reference.book_id)
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
        workspace.id = id.into();
        workspace.generation = record.generation + 1;
        workspace.selected_chat = record.selected_chat;
        workspace.messages = messages(user, workspace.selected_chat.as_deref());
        let mut scene = ResidentWorkspace {
            workspace,
            revision: record.revision,
            attachment: None,
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
                .any(|s| &s.id == id && s.book_id == scene.workspace.book.base.book_id)
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
        self.create_bound(
            context.user_id(),
            user,
            library,
            reference,
            attachment,
            None,
            None,
        )
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
    ) -> Result<Value, ToolError> {
        self.reserve(owner)?;
        let publication = library.lock().unwrap().load(owner, &reference)?;
        let checkpoint = if copy.is_some() {
            copy
        } else {
            let saved: Option<String> = self.control.connection.query_row("SELECT checkpoint FROM reader_workspaces WHERE owner_user_id=? AND book_id=? AND publication_id=? AND checkpoint IS NOT NULL ORDER BY checkpoint_seq DESC LIMIT 1", params![owner,reference.book_id,reference.publication_id], |r| r.get(0)).optional().map_err(|_| storage())?;
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
        workspace.selected_chat = chat;
        let mut scene = ResidentWorkspace {
            workspace,
            revision: 0,
            attachment: Some(attachment),
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
                drop(scene);
                drop(handle);
                let mut result =
                    self.create_bound(owner, user, library, reference, next, Some(saved), chat)?;
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
                scene.linked.clear();
            }
            return Ok(view(&scene));
        }
        let stamp: WorkspaceStamp = serde_json::from_value(input.clone()).map_err(|_| invalid())?;
        if matches!(action, "reader/state" | "profile/manifest" | "profile/memory" | "memory/recall"
            | "chat/history" | "reader/paper_minimap.state" | "reader/pdf_selection.resolve"
            | "reader/pdf_ranges.project" | "agent/source.resolve") {
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
            require_turn_publication(
                user,
                &scene.workspace,
                session,
                input["turn_id"].as_str().ok_or_else(invalid)?,
            )?;
            let private = private_context(user, &scene.workspace);
            crate::presentation_api::delivered_version(
                &private,
                session,
                input["turn_id"].as_str().ok_or_else(invalid)?,
                &reference,
            )?;
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
            let mut workspace = ReaderWorkspace::local(
                publication.directory.clone(),
                publication.book.clone(),
                reader::Reader::new(&publication.book, reader::DEFAULT_RADIUS),
                vec![],
                None,
            );
            workspace.id = id.into();
            workspace.generation = scene.workspace.generation + 1;
            workspace.publication = Some(publication);
            let mut next = ResidentWorkspace {
                workspace,
                revision: scene.revision,
                attachment: scene.attachment.clone(),
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
                .any(|s| s.id == chat && s.book_id == scene.workspace.book.base.book_id)
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
        if matches!(action, "presentation/read" | "presentation/observe") {
            let session = input["session_id"].as_str().ok_or_else(invalid)?;
            let turn = input["turn_id"].as_str().ok_or_else(invalid)?;
            if scene.workspace.selected_chat.as_deref() != Some(session) { return Err(stale()); }
            require_turn_publication(user, &scene.workspace, session, turn)?;
            let mut request = input.clone();
            request["saved_state"] = scene.presentation.as_ref().filter(|saved|
                saved.session_id == session && saved.turn_id == turn && json!(saved.reference) == input["reference"]
            ).map(|s| json!(s)).unwrap_or(Value::Null);
            let result = crate::workspace_client::reply(crate::presentation_api::route_with_restore(
                &private_context(user, &scene.workspace), &request.to_string(), action == "presentation/observe", false
            ))?;
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
            "agent/source.open" => {
                let request = crate::parse_agent_source_request(&input.to_string()).map_err(|r| crate::workspace_client::reply(r).unwrap_err())?;
                let binding = crate::workspace_source_binding(user, &scene.workspace, &request)?;
                book.resolve_source(&binding.evidence_range, "zh-CN", Some(&binding.evidence_text_digest))?;
                reader.goto_lid(book, &mut user.store, &binding.evidence_range.start_lid, now)?;
                json!({"source_ref_id":binding.source_ref_id,"opened":true})
            }
            "reader/goto" => json!(reader.goto_lid(
                book,
                &mut user.store,
                input["lid"].as_str().ok_or_else(invalid)?,
                now
            )?),
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
                    scene.workspace.selected_chat.clone(),
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
                require_turn_publication(
                    user,
                    &scene.workspace,
                    input["session_id"].as_str().ok_or_else(invalid)?,
                    input["turn_id"].as_str().ok_or_else(invalid)?,
                )?;
                let reply = crate::presentation_api::save_state(
                    &private_context(user, &scene.workspace),
                    &input.to_string(),
                );
                if reply.status != 200 {
                    return Err(invalid());
                }
                presentation = Some(serde_json::from_str(&reply.body).map_err(|_| storage())?);
                json!(presentation)
            }
            "presentation/restore" => {
                let saved: runtime::presentation::PresentationFollowUp =
                    serde_json::from_value(input["saved_state"].clone()).map_err(|_| invalid())?;
                require_turn_publication(
                    user,
                    &scene.workspace,
                    &saved.session_id,
                    &saved.turn_id,
                )?;
                crate::presentation_api::follow_up_context(
                    &private_context(user, &scene.workspace),
                    &saved,
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
        .find(|s| s.id == chat && s.book_id == w.book.base.book_id)
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
        user,
        book: &w.book,
        book_dir: &w.book_dir,
        messages: &w.messages,
        selected_chat: w.selected_chat.as_deref(),
    }
}
fn view(scene: &ResidentWorkspace) -> Value {
    json!({"workspace_id":scene.workspace.id,"generation":scene.workspace.generation,"revision":scene.revision,
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
        let scope = crate::run_scope::RunScope {
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
            book: w.workspace.book.clone(),
            book_dir: w.workspace.book_dir.clone(),
            provider_binding: profile,
            reader_input: runtime::run_context::ReaderInputSnapshot::capture(
                &w.workspace.book,
                &w.workspace.reader,
                message,
            ),
            question_quote: None,
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
        self.access
            .library
            .lock()
            .unwrap()
            .authorize(
                &self.scope.user_id,
                &self.scope.publication.as_ref().unwrap().reference,
            )
            .map_err(|_| missing())
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
        crate::presentation_api::follow_up_context(&self.scope.private_user(&user), &saved)?;
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
impl runtime::run_context::ResidentStatePort for NetworkRunPort {
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
                &self.scope.book,
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
        self.scope.reader_input.clone()
    }
    fn tutor_active(&mut self) -> Result<bool, ToolError> {
        self.private(|p| crate::teaching::turn_active(p, &self.turn.turn_id))
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
}
