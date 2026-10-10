//! Durable admission metadata; business input and output remain in private History.
use crate::*;
use crate::{
    agent_run::{FinishedRun, NetworkUserPort, PreparedAgentChat},
    authorization::{missing, Authorization, AuthorizedContext},
    control_store::ControlStore,
    published_library::PublishedBookRef,
    user_registry::UserHandle,
    workspace_registry::WorkspaceStamp,
};
use runtime::run_context::CancellationToken;
use rusqlite::{params, OptionalExtension};
use std::sync::{Arc, Mutex};

fn fault(code: &str, category: &str) -> ToolError {
    user_storage_paths::error(code, category, "Run request could not be completed")
}
fn storage() -> ToolError {
    fault("ADMISSION_STORAGE_FAILED", "unavailable")
}
fn closed() -> ToolError {
    fault("REQUEST_KEY_CLOSED", "conflict")
}
fn preparing() -> ToolError {
    fault("ADMISSION_PREPARING", "conflict")
}
fn busy() -> ToolError {
    fault("CHAT_BUSY", "conflict")
}
fn invalid() -> ToolError {
    fault("RUN_INPUT_INVALID", "validation")
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct ProviderBinding {
    mode: String,
    endpoint: String,
    model: String,
    profile: runtime::ModelRuntimeProfile,
}
#[derive(Clone)]
struct RunProvider {
    binding: ProviderBinding,
    make: Arc<dyn Fn() -> Box<dyn ModelAdapter + Send> + Send + Sync>,
}

/// Only the necessary request/scene/provider metadata; credentials and temporary candidates are absent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct FrozenTurn<M = Vec<Message>> {
    version: u32,
    owner: String,
    workspace: String,
    generation: u64,
    effect_revision: u64,
    chat: String,
    turn: String,
    ordinal: u64,
    head: Option<String>,
    publication: PublishedBookRef,
    normalized: Value,
    reader: runtime::run_context::ReaderInputSnapshot,
    confirmation: Option<String>,
    provider: ProviderBinding,
    message: String,
    agent_message: String,
    pub(crate) messages: M,
    quote: Option<AskQuote>,
    presentation: Option<runtime::presentation::PresentationFollowUp>,
    teaching: teaching::FrozenTeachingPreparation,
    goal: Option<runtime::goal::ResidentGoal>,
    now: String,
    preparation_complete: bool,
}
impl<M> FrozenTurn<M> {
    pub(crate) fn map_messages<N>(self, map: impl FnOnce(M) -> N) -> FrozenTurn<N> {
        FrozenTurn {
            version: self.version, owner: self.owner, workspace: self.workspace,
            generation: self.generation, effect_revision: self.effect_revision, chat: self.chat,
            turn: self.turn, ordinal: self.ordinal, head: self.head, publication: self.publication,
            normalized: self.normalized, reader: self.reader, confirmation: self.confirmation,
            provider: self.provider, message: self.message, agent_message: self.agent_message,
            messages: map(self.messages), quote: self.quote, presentation: self.presentation,
            teaching: self.teaching, goal: self.goal, now: self.now,
            preparation_complete: self.preparation_complete,
        }
    }
    pub(crate) fn set_prepared(&mut self, teaching: teaching::FrozenTeachingPreparation) {
        self.teaching = teaching;
        self.preparation_complete = true;
    }
}
impl FrozenTurn {
    fn turn_ref(&self) -> AgentTurnRef {
        AgentTurnRef {
            session_id: self.chat.clone(),
            turn_id: self.turn.clone(),
            user_turn_ordinal: self.ordinal,
        }
    }
    fn prepared(&self, access: &Authorization) -> Result<PreparedAgentChat, ToolError> {
        if self.version != 1 {
            return Err(fault("RUN_INPUT_INCOMPATIBLE", "conflict"));
        }
        let publication = access
            .library
            .lock()
            .unwrap()
            .load(&self.owner, &self.publication)?;
        Ok(PreparedAgentChat {
            scope: run_scope::RunScope {
                user_id: self.owner.clone(),
                workspace_id: self.workspace.clone(),
                workspace_generation: self.generation,
                chat_session_id: self.chat.clone(),
                turn_id: self.turn.clone(),
                confirmation_id: self.confirmation.clone(),
                book: publication.book.clone(),
                book_dir: publication.directory().to_path_buf(),
                publication: Some(publication),
                provider_binding: self.provider.profile.clone(),
                reader_input: self.reader.clone(),
                question_quote: self.quote.clone(),
                presentation_follow_up: self.presentation.clone(),
            },
            tutor: self.teaching.context.clone(),
            goal: self.goal.clone(),
            turn_ref: self.turn_ref(),
            message: self.message.clone(),
            agent_message: self.agent_message.clone(),
            messages: self.messages.clone(),
            now: self.now.clone(),
        })
    }
}

#[derive(Debug, Clone)]
struct Admission {
    owner: String,
    key: String,
    turn: String,
    chat: String,
    workspace: String,
    generation: u64,
    publication: PublishedBookRef,
    state: String,
    cancel: bool,
    closed: bool,
    unsaved: bool,
}
fn read_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Admission> {
    Ok(Admission {
        owner: r.get(0)?,
        key: r.get(1)?,
        turn: r.get(2)?,
        chat: r.get(3)?,
        workspace: r.get(4)?,
        generation: r.get(5)?,
        publication: PublishedBookRef {
            book_id: r.get(6)?,
            publication_id: r.get(7)?,
        },
        state: r.get(8)?,
        cancel: r.get(9)?,
        closed: r.get(10)?,
        unsaved: r.get(11)?,
    })
}
const COLUMNS: &str = "owner_user_id,client_request_id,turn_id,chat_session_id,workspace_id,workspace_generation,book_id,publication_id,dispatch_state,cancel_requested,key_closed,unsaved";
struct Active {
    cancellation: CancellationToken,
    stream: Arc<agent_stream::RunStream>,
}
struct Unsaved {
    user: UserHandle,
    prepared: PreparedAgentChat,
    finished: FinishedRun,
    stream: Arc<agent_stream::RunStream>,
}
struct State {
    control: ControlStore,
    deleting: BTreeSet<(String, String)>,
    finishing: BTreeSet<(String, String)>,
    active: BTreeMap<(String, String), Active>,
    reserved: BTreeSet<(String, String)>,
    streams: BTreeMap<(String, String), Arc<agent_stream::RunStream>>,
    last_owner: Option<String>,
    unsaved: BTreeMap<(String, String), Unsaved>,
    stopping: bool,
}
pub(crate) struct RunAdmissions {
    state: Mutex<State>,
    provider: Mutex<Option<RunProvider>>,
    pub(crate) observability: Arc<crate::observability::ObservabilityRuntime>,
    pub(crate) boot: String,
    pub(crate) limits: Arc<crate::service_limits::ServiceLimits>,
    #[cfg(test)]
    pub(crate) fail_after: Mutex<Option<&'static str>>,
}
impl RunAdmissions {
    pub(crate) fn with_limits(
        control: ControlStore,
        limits: Arc<crate::service_limits::ServiceLimits>,
    ) -> Self {
        Self {
            limits,
            state: Mutex::new(State {
                control,
                deleting: BTreeSet::new(),
                finishing: BTreeSet::new(),
                active: BTreeMap::new(),
                reserved: BTreeSet::new(),
                streams: BTreeMap::new(),
                last_owner: None,
                unsaved: BTreeMap::new(),
                stopping: false,
            }),
            provider: Mutex::new(None),
            observability: crate::observability::ObservabilityRuntime::disabled(None),
            boot: format!("boot-{}", uuid::Uuid::now_v7()),
            #[cfg(test)]
            fail_after: Mutex::new(None),
        }
    }
    pub(crate) fn configure(&self, config: ProviderConfig) -> Result<(), ToolError> {
        let url = url::Url::parse(&config.base_url).map_err(|_| invalid())?;
        if !url.username().is_empty() || url.password().is_some() || url.query().is_some() {
            return Err(invalid());
        }
        let profile = ProviderRegistry::adapter_from_config(config.clone()).model_runtime_profile();
        let binding = ProviderBinding {
            mode: config.mode.as_str().into(),
            endpoint: config.base_url.clone(),
            model: config.model.clone(),
            profile,
        };
        *self.provider.lock().unwrap() = Some(RunProvider {
            binding,
            make: Arc::new(move || ProviderRegistry::adapter_from_config(config.clone())),
        });
        Ok(())
    }
    #[cfg(test)]
    pub(crate) fn configure_fake(
        &self,
        make: impl Fn() -> Box<dyn ModelAdapter + Send> + Send + Sync + 'static,
    ) {
        let profile = make().model_runtime_profile();
        *self.provider.lock().unwrap() = Some(RunProvider {
            binding: ProviderBinding {
                mode: "fixture".into(),
                endpoint: "fixture".into(),
                model: "fixture".into(),
                profile,
            },
            make: Arc::new(make),
        });
    }
    fn point(&self, _point: &'static str) -> Result<(), ToolError> {
        // The MU10 subprocess harness kills the test Host here, with all real
        // file/SQLite commits and locks still live. Never compiled into a server.
        #[cfg(test)]
        if std::env::var("MU10_KILL_POINT").as_deref() == Ok(_point) {
            crate::tests::mu10_tests::kill_barrier();
        }
        #[cfg(test)]
        if *self.fail_after.lock().unwrap() == Some(_point) {
            return Err(fault("INJECTED_CRASH", "unavailable"));
        }
        Ok(())
    }
    fn by_key(&self, owner: &str, key: &str) -> Result<Option<Admission>, ToolError> {
        self.state.lock().unwrap().control.connection.query_row(&format!("SELECT {COLUMNS} FROM run_admissions WHERE owner_user_id=? AND client_request_id=?"),params![owner,key], read_row).optional().map_err(|_| storage())
    }
    fn by_turn(&self, owner: &str, turn: &str) -> Result<Admission, ToolError> {
        self.state
            .lock()
            .unwrap()
            .control
            .connection
            .query_row(
                &format!(
                    "SELECT {COLUMNS} FROM run_admissions WHERE owner_user_id=? AND turn_id=?"
                ),
                params![owner, turn],
                read_row,
            )
            .optional()
            .map_err(|_| storage())?
            .ok_or_else(missing)
    }
    fn rows(&self) -> Result<Vec<Admission>, ToolError> {
        let state = self.state.lock().unwrap();
        let mut q = state.control.connection.prepare(&format!("SELECT {COLUMNS} FROM run_admissions WHERE dispatch_state IN ('preparing','queued','claimed') ORDER BY rowid")).map_err(|_| storage())?;
        let rows = q.query_map([], read_row).map_err(|_| storage())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|_| storage())
    }
    fn update(&self, row: &Admission, stage: &str, key_closed: bool) -> Result<(), ToolError> {
        let mut state = self.state.lock().unwrap();
        #[cfg(test)]
        let _timing = crate::tests::mu10_tests::measure("control_commit");
        state.control.connection.execute("UPDATE run_admissions SET dispatch_state=?,key_closed=MAX(key_closed,?),unsaved=0 WHERE owner_user_id=? AND turn_id=?",params![stage,key_closed,row.owner,row.turn]).map_err(|_| storage())?;
        if matches!(stage, "settled" | "admission_failed") {
            state.streams.remove(&(row.owner.clone(), row.turn.clone()));
        }
        Ok(())
    }
    fn mark_unsaved(&self, row: &Admission) {
        // Even if this metadata write fails, the nonterminal row still owns its chat/workspace.
        let state = self.state.lock().unwrap();
        if let Some(stream) = state.streams.get(&(row.owner.clone(), row.turn.clone())) {
            if stream.snapshot().persistence_state == "pending" {
                stream.finish(None, Some(json!({"error_code":"TURN_UNSAVED"})));
            }
        }
        let _ = state.control.connection.execute(
            "UPDATE run_admissions SET unsaved=1 WHERE owner_user_id=? AND turn_id=?",
            params![row.owner, row.turn],
        );
    }
    fn repeat(
        &self,
        access: &Authorization,
        context: &AuthorizedContext,
        row: &Admission,
        normalized: &Value,
    ) -> Result<Value, ToolError> {
        if row.closed {
            return Err(closed());
        }
        access
            .library
            .lock()
            .unwrap()
            .authorize(&row.owner, &row.publication)
            .map_err(|_| missing())?;
        if row.state == "preparing" {
            return Err(preparing());
        }
        let user = context.user.lock().unwrap();
        let saved = frozen_input(&user, row)?.ok_or_else(closed)?;
        if &saved.normalized != normalized {
            return Err(fault("REQUEST_KEY_CONFLICT", "conflict"));
        }
        Ok(descriptor(row))
    }
    pub(crate) fn admit(
        &self,
        access: &Arc<Authorization>,
        context: &AuthorizedContext,
        workspace: &str,
        input: &Value,
        now: &str,
    ) -> Result<Value, ToolError> {
        let key = input["client_request_id"]
            .as_str()
            .filter(|s| !s.is_empty() && s.len() <= 128)
            .ok_or_else(invalid)?;
        let normalized = normalize(workspace, input)?;
        if let Some(row) = self.by_key(context.user_id(), key)? {
            return self.repeat(access, context, &row, &normalized);
        }
        let provider = self
            .provider
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| fault("PROVIDER_NOT_CONFIGURED", "unavailable"))?;
        let mut user = context.user.lock().unwrap();
        // Another admission can have finished while this request waited for the private authority.
        #[cfg(test)]
        let _timing = crate::tests::mu10_tests::measure("user_lock");
        if let Some(row) = self.by_key(context.user_id(), key)? {
            drop(user);
            return self.repeat(access, context, &row, &normalized);
        }
        let scene = access.workspaces.lock().unwrap().scene(
            context.user_id(),
            workspace,
            &user,
            &access.library,
        )?;
        let scene = scene.lock().unwrap();
        let stamp: WorkspaceStamp = serde_json::from_value(input.clone()).map_err(|_| invalid())?;
        scene.check(&stamp)?;
        if input.get("published_book_ref").is_some_and(|v| {
            !v.is_null() && v != &json!(scene.workspace.publication.as_ref().unwrap().reference)
        }) {
            return Err(fault("PUBLICATION_BINDING_MISMATCH", "conflict"));
        }
        let chat = input["session_id"].as_str().ok_or_else(invalid)?;
        if scene.workspace.selected_chat.as_deref() != Some(chat) {
            return Err(fault("CHAT_SELECTION_CHANGED", "conflict"));
        }
        let session = user
            .agent_history
            .sessions
            .iter()
            .find(|s| s.id == chat && s.book_id == scene.workspace.book.base.book_id)
            .ok_or_else(missing)?;
        let head = session.turns.last().map(|t| t.turn_id.clone());
        if input.get("expected_chat_head").is_some() && input["expected_chat_head"] != json!(head) {
            return Err(fault("CHAT_HEAD_CHANGED", "conflict"));
        }
        if session
            .turns
            .iter()
            .any(|t| t.status == AgentAssistantStatus::PendingAssistant)
        {
            return Err(busy());
        }
        let ordinal = session
            .turns
            .last()
            .map_or(0, |t| t.user_turn_ordinal)
            .checked_add(1)
            .ok_or_else(invalid)?;
        // A failed preparation may keep a closed admission without appending a History turn.
        // Allocate here so a new request at that same chat ordinal cannot collide with its tombstone.
        let turn = format!("turn_{}", uuid::Uuid::now_v7());
        let validated = validate_agent_input(&mut user, &scene.workspace, &input.to_string())
            .map_err(reply_error)?;
        let row = Admission {
            owner: context.user_id().into(),
            key: key.into(),
            turn,
            chat: chat.into(),
            workspace: workspace.into(),
            generation: scene.workspace.generation,
            publication: scene
                .workspace
                .publication
                .as_ref()
                .unwrap()
                .reference
                .clone(),
            state: "preparing".into(),
            cancel: false,
            closed: false,
            unsaved: false,
        };
        {
            let mut state = self.state.lock().unwrap();
            if state.stopping {
                return Err(fault("SERVICE_STOPPING", "unavailable"));
            }
            if state
                .deleting
                .contains(&(row.owner.clone(), row.chat.clone()))
            {
                return Err(busy());
            }
            #[cfg(test)]
            let _timing = crate::tests::mu10_tests::measure("control_commit");
            let tx = state
                .control
                .connection
                .transaction()
                .map_err(|_| storage())?;
            let occupied:bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM run_admissions WHERE owner_user_id=? AND (chat_session_id=? OR workspace_id=?) AND dispatch_state IN ('preparing','queued','claimed'))",params![row.owner,row.chat,row.workspace],|r|r.get(0)).map_err(|_|storage())?;
            if occupied {
                return Err(busy());
            }
            let (total, own):(u64,u64) = tx.query_row("SELECT count(*),COALESCE(sum(owner_user_id=?),0) FROM run_admissions WHERE dispatch_state IN ('preparing','queued','claimed')",[&row.owner],|r|Ok((r.get(0)?,r.get(1)?))).map_err(|_|storage())?;
            if total >= self.limits.queued_runs as u64 || own >= self.limits.user_queued_runs as u64
            {
                return Err(fault("RUN_QUEUE_FULL", "rate_limit"));
            }
            tx.execute("INSERT INTO run_admissions(owner_user_id,client_request_id,turn_id,chat_session_id,workspace_id,workspace_generation,book_id,publication_id,dispatch_state) VALUES(?,?,?,?,?,?,?,?,'preparing')",params![row.owner,row.key,row.turn,row.chat,row.workspace,row.generation,row.publication.book_id,row.publication.publication_id]).map_err(|_|storage())?;
            tx.commit().map_err(|_| storage())?;
        }
        self.point("preparing")?;
        let result: Result<(), ToolError> = (|| {
            let ValidatedAgentInput {
                request,
                message,
                display_user,
                question_anchor_lid,
                question_quote,
                presentation_follow_up,
                mut agent_message,
                presentation_context,
            } = validated;
            let mut session = user.agent_history.sessions.iter().find(|s| s.id == chat)
                .ok_or_else(missing)?.clone();
            let turn_ref = append_pending_agent_turn(
                &mut session,
                display_user,
                question_anchor_lid,
                question_quote.clone(),
                presentation_follow_up.clone(),
                request["teaching_ref"].as_str().map(str::to_owned),
                request["goal_id"].as_str(),
                request["goal_action"].as_str(),
                Some(row.publication.clone()),
                Some(&row.turn),
                now,
            )?;
            let private = PrivateBookContext {
                user: &user,
                book: &scene.workspace.book,
                book_dir: &scene.workspace.book_dir,
                messages: &session.messages,
                selected_chat: Some(chat),
            };
            let teaching = teaching::freeze_prepare(&private, &turn_ref, &request, now)?;
            if let Some(text) = presentation_context {
                agent_message.push_str(&text);
            }
            let goal = session
                .turns
                .last()
                .and_then(|t| t.goal_ref.as_ref())
                .and_then(|r| session.goals.iter().find(|g| g.id == r.id))
                .cloned();
            let frozen = FrozenTurn {
                version: 1,
                owner: row.owner.clone(),
                workspace: row.workspace.clone(),
                generation: row.generation,
                effect_revision: scene.revision,
                chat: row.chat.clone(),
                turn: row.turn.clone(),
                ordinal,
                head,
                publication: row.publication.clone(),
                normalized,
                reader: runtime::run_context::ReaderInputSnapshot::capture(
                    &scene.workspace.book,
                    &scene.workspace.reader,
                    &message,
                ),
                confirmation: user.agent_history.confirmation_id(chat).map(str::to_owned),
                provider: provider.binding,
                message,
                agent_message,
                messages: session.messages.clone(),
                quote: question_quote,
                presentation: presentation_follow_up,
                teaching,
                goal,
                now: now.into(),
                preparation_complete: false,
            };
            session.turns.last_mut().unwrap().domain.scene = Some(session_event::ReadingScene::capture(&scene.workspace));
            session.turns.last_mut().unwrap().admission_input = Some(frozen);
            if user.session_store.is_some() {
                let user = &mut *user;
                user.session_store.as_mut().unwrap().accept(&mut user.agent_history, session, now)?;
            } else {
                let mut candidate = user.agent_history.clone();
                *candidate.sessions.iter_mut().find(|s| s.id == chat).unwrap() = session;
                save_agent_history_path(&user.history_path, &candidate)?;
                user.agent_history = candidate;
            }
            self.point("pending")?;
            self.complete_preparation(&mut user, &row)?;
            Ok(())
        })();
        drop(scene);
        match result {
            Ok(()) => Ok(descriptor(&self.by_turn(&row.owner, &row.turn)?)),
            Err(e) if e.error_code == "INJECTED_CRASH" => Err(e),
            Err(e) => {
                if user.session_store.is_some() {
                    let user = &mut *user;
                    if user.session_store.as_mut().unwrap().settle(&mut user.agent_history, &row.chat).is_err() {
                        self.mark_unsaved(&row);
                        return Err(storage());
                    }
                }
                let has_input = frozen_input(&user, &row)?.is_some();
                if find_turn(&user, &row).is_some() {
                    if self
                        .end_pending(&mut user, &row, "ADMISSION_FAILED", false)
                        .is_err()
                    {
                        self.mark_unsaved(&row);
                        return Err(storage());
                    }
                }
                self.update(&row, "admission_failed", !has_input)?;
                Err(e)
            }
        }
    }
    fn complete_preparation(
        &self,
        user: &mut user_runtime::UserRuntime,
        row: &Admission,
    ) -> Result<(), ToolError> {
        let frozen = frozen_input(user, row)?
            .ok_or_else(|| fault("RUN_INPUT_MISSING", "conflict"))?;
        if frozen.version != 1 {
            return Err(fault("RUN_INPUT_INCOMPATIBLE", "conflict"));
        }
        // Each original event is checked/reused individually before the private completion flag.
        let mut learning = user.learning_store()?;
        for event in &frozen.teaching.events {
            learning.append_teaching(event)?;
            self.point("teaching_receipt")?;
        }
        if !frozen.preparation_complete {
            if let Some(store) = &mut user.session_store {
                store.append(&mut user.agent_history, &row.chat, &frozen.now, Some(&row.turn),
                    session_event::EventBody::TurnPrepared { teaching: frozen.teaching.clone() })?;
            } else {
            let mut candidate = user.agent_history.clone();
            find_turn_mut(&mut candidate, row)
                .unwrap()
                .admission_input
                .as_mut()
                .unwrap()
                .preparation_complete = true;
            save_agent_history_path(&user.history_path, &candidate)?;
            user.agent_history = candidate;
            }
        }
        session_runtime::link_teaching(user, &frozen.turn_ref(), &frozen.now)?;
        self.point("prepared")?;
        {
            let mut state = self.state.lock().unwrap();
            let tx = state.control.connection.transaction().map_err(|_|storage())?;
            let changed = tx.execute("UPDATE run_admissions SET dispatch_state='queued',unsaved=0 WHERE owner_user_id=? AND turn_id=? AND dispatch_state='preparing'",params![row.owner,row.turn]).map_err(|_|storage())?;
            if changed == 1 {
                crate::admin_usage::record(&tx, &row.owner, "question", &row.turn, frozen.now.parse().map_err(|_|invalid())?)?;
            }
            tx.commit().map_err(|_|storage())?;
            if changed == 1 {
                if let Some(stream) = state.streams.get(&(row.owner.clone(), row.turn.clone())) {
                    stream.dispatch_state("queued");
                }
            }
        }
        self.point("queued")?;
        Ok(())
    }
    fn end_pending(
        &self,
        user: &mut user_runtime::UserRuntime,
        row: &Admission,
        code: &str,
        cancelled: bool,
    ) -> Result<(), ToolError> {
        if let Some(store) = &mut user.session_store {
            store.settle(&mut user.agent_history, &row.chat)?;
        }
        let turn = find_turn(user, row).ok_or_else(missing)?;
        if turn.status != AgentAssistantStatus::PendingAssistant {
            return Ok(());
        }
        let reference = AgentTurnRef {
            session_id: row.chat.clone(),
            turn_id: row.turn.clone(),
            user_turn_ordinal: turn.user_turn_ordinal,
        };
        let frozen = frozen_input(user, row)?;
        let mut messages = user
            .agent_history
            .sessions
            .iter()
            .find(|s| s.id == row.chat)
            .unwrap()
            .messages
            .clone();
        if let Some(f) = &frozen {
            // A claimed Run may have installed a compaction checkpoint containing run-local tools.
            // Interrupted/cancelled queued work resumes the frozen chat prefix and original question.
            if user.session_store.is_none() {
                messages = f.messages.clone();
                messages.push(Message::user(f.agent_message.clone()));
            } else if messages == f.messages {
                messages.push(Message::user(f.agent_message.clone()));
            }
        }
        let summary = user.session_store.as_ref().map(|store| {
            session_runtime::close_interrupted(&mut messages);
            session_runtime::interrupted_summary(&store.logs[&row.chat], &row.turn)
        }).flatten();
        finalize_user_agent_turn(
            user,
            &reference,
            if cancelled {
                AgentAssistantStatus::Cancelled
            } else {
                AgentAssistantStatus::Failed
            },
            None,
            Some(AgentTurnError {
                error_code: code.into(),
                category: if cancelled {
                    "cancelled"
                } else {
                    "interrupted"
                }
                .into(),
                message: code.into(),
            }),
            summary,
            &messages,
            &multi_user_host::now().to_string(),
        )?;
        if let Some(stream) = self
            .state
            .lock()
            .unwrap()
            .streams
            .get(&(row.owner.clone(), row.turn.clone()))
            .cloned()
        {
            let turn = find_turn(user, row).unwrap();
            stream.finish(Some(json!({"turn_id":turn.turn_id,"status":turn.status,"error":turn.error,"outcome":turn.outcome})),None);
        }
        Ok(())
    }
}
fn find_turn<'a>(
    user: &'a user_runtime::UserRuntime,
    row: &Admission,
) -> Option<&'a AgentChatTurn> {
    user.agent_history
        .sessions
        .iter()
        .find(|s| s.id == row.chat)
        .and_then(|s| s.turns.iter().find(|t| t.turn_id == row.turn))
}

/// Execution uses the raw prefix plus its checkpoint. The model projection is
/// installed by the existing runtime, so it must not be compacted twice here.
fn frozen_input(user: &user_runtime::UserRuntime, row: &Admission) -> Result<Option<FrozenTurn>, ToolError> {
    if let Some(store) = &user.session_store {
        let Some(log) = store.logs.get(&row.chat) else { return Ok(None); };
        let Some(input) = log.projection.inputs.get(&row.turn) else { return Ok(None); };
        let messages = match &input.messages {
            session_event::HistoryPosition::Committed { history_through_seq } =>
                log.at(*history_through_seq)?.session.ok_or_else(missing)?.messages,
        };
        Ok(Some(input.clone().map_messages(|_| messages)))
    } else {
        Ok(find_turn(user, row).and_then(|t| t.admission_input.clone()))
    }
}
fn find_turn_mut<'a>(
    history: &'a mut AgentHistory,
    row: &Admission,
) -> Option<&'a mut AgentChatTurn> {
    history
        .sessions
        .iter_mut()
        .find(|s| s.id == row.chat)
        .and_then(|s| s.turns.iter_mut().find(|t| t.turn_id == row.turn))
}
fn descriptor(row: &Admission) -> Value {
    json!({"book_id":row.publication.book_id,"workspace_id":row.workspace,"workspace_generation":row.generation,"published_book_ref":row.publication,"session_id":row.chat,"turn_id":row.turn,"dispatch_state":row.state,"persistence_state":if row.unsaved{"failed"}else{"saved"}})
}
fn reply_error(reply: Reply) -> ToolError {
    let v: Value = serde_json::from_str(&reply.body).unwrap_or_default();
    fault(
        v["error_code"].as_str().unwrap_or("RUN_INPUT_REJECTED"),
        "validation",
    )
}
fn normalize(workspace: &str, input: &Value) -> Result<Value, ToolError> {
    let message = input["message"]
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(invalid)?;
    let mut v = json!({"workspace_id":workspace,"message":message,"display_user":input["display_user"].as_str().unwrap_or(message)});
    for key in [
        "session_id",
        "generation",
        "expected_revision",
        "attachment_id",
        "published_book_ref",
        "expected_chat_head",
        "question_anchor_lid",
        "question_quote",
        "presentation_follow_up",
        "teaching_ref",
        "goal_id",
        "goal_action",
        "retry_of",
    ] {
        v[key] = input[key].clone();
    }
    Ok(v)
}
impl RunAdmissions {
    /// Startup reconciliation never calls a Provider or the original prepare function.
    pub(crate) fn recover(&self, access: &Arc<Authorization>) -> Result<(), ToolError> {
        for row in self
            .rows()?
            .into_iter()
            .filter(|r| !matches!(r.state.as_str(), "settled" | "admission_failed"))
        {
            let handle = access
                .users
                .lock()
                .unwrap()
                .get_for_recovery(&row.owner, &multi_user_host::now().to_string())?;
            let mut user = handle.lock().unwrap();
            let Some(turn) = find_turn(&user, &row) else {
                self.update(&row, "admission_failed", true)?;
                continue;
            };
            if turn.status != AgentAssistantStatus::PendingAssistant {
                self.repair_saved(&mut user, &row)?;
                continue;
            }
            let result = (|| {
                if row.state == "claimed" {
                    self.end_pending(&mut user, &row, "INTERRUPTED", false)?;
                    return self.update(&row, "settled", false);
                }
                if row.cancel {
                    self.end_pending(&mut user, &row, "AGENT_RUN_CANCELLED", true)?;
                    return self.update(&row, "settled", false);
                }
                let frozen = frozen_input(&user, &row)?;
                let Some(frozen) = frozen else {
                    self.end_pending(&mut user, &row, "RUN_INPUT_MISSING", false)?;
                    return self.update(&row, "admission_failed", true);
                };
                if let Err(e) = validate_frozen(&user, &row, &frozen) {
                    self.end_pending(&mut user, &row, &e.error_code, false)?;
                    return self.update(&row, "settled", false);
                }
                if is_sensitive_memory_confirmation(&frozen.message) {
                    self.end_pending(&mut user, &row, "SENSITIVE_CONFIRMATION_EXPIRED", false)?;
                    return self.update(&row, "settled", false);
                }
                if access
                    .library
                    .lock()
                    .unwrap()
                    .authorize(&row.owner, &row.publication)
                    .is_err()
                {
                    self.end_pending(&mut user, &row, "RUN_PERMISSION_REVOKED", false)?;
                    return self.update(&row, "settled", false);
                }
                self.complete_preparation(&mut user, &row)
            })();
            if let Err(e) = result {
                self.mark_unsaved(&row);
                return Err(e);
            }
        }
        Ok(())
    }
    /// Reserve a user share before loading private state; never hold this lock during execution.
    pub(crate) fn run_one(&self, access: &Arc<Authorization>) -> Result<bool, ToolError> {
        let row = {
            let mut state = self.state.lock().unwrap();
            if state.stopping || state.reserved.len() >= self.limits.active_runs {
                return Ok(false);
            }
            let rows = {
                let mut query = state.control.connection.prepare(&format!("SELECT {COLUMNS} FROM run_admissions WHERE dispatch_state='queued' AND unsaved=0 ORDER BY rowid")).map_err(|_|storage())?;
                let rows = query
                    .query_map([], read_row)
                    .map_err(|_| storage())?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|_| storage())?;
                rows
            };
            let mut users = BTreeMap::new();
            for row in rows {
                if state
                    .reserved
                    .contains(&(row.owner.clone(), row.turn.clone()))
                    || state
                        .reserved
                        .iter()
                        .filter(|(owner, _)| owner == &row.owner)
                        .count()
                        >= self.limits.user_active_runs
                {
                    continue;
                }
                users.entry(row.owner.clone()).or_insert(row);
            }
            let next = users
                .keys()
                .find(|owner| state.last_owner.as_ref().is_none_or(|last| *owner > last))
                .or_else(|| users.keys().next())
                .cloned();
            let Some(owner) = next else {
                return Ok(false);
            };
            let row = users.remove(&owner).unwrap();
            state.last_owner = Some(owner);
            state.reserved.insert((row.owner.clone(), row.turn.clone()));
            row
        };
        let _active = ActiveGuard(self, (row.owner.clone(), row.turn.clone()));
        let handle = access
            .users
            .lock()
            .unwrap()
            .get_for_recovery(&row.owner, &multi_user_host::now().to_string())?;
        let frozen = {
            let mut user = handle.lock().unwrap();
            let row = self.by_turn(&row.owner, &row.turn)?;
            if row.state != "queued" {
                return Ok(false);
            }
            if row.cancel {
                if let Err(e) = self.end_pending(&mut user, &row, "AGENT_RUN_CANCELLED", true) {
                    self.mark_unsaved(&row);
                    return Err(e);
                }
                self.update(&row, "settled", false)?;
                return Ok(true);
            }
            let check = (|| {
                let frozen = frozen_input(&user, &row)?
                    .ok_or_else(|| fault("RUN_INPUT_MISSING", "conflict"))?;
                validate_frozen(&user, &row, &frozen)?;
                if !frozen.preparation_complete {
                    return Err(fault("RUN_INPUT_INCOMPLETE", "conflict"));
                }
                user.agent_history
                    .expire_confirmations(std::time::Instant::now());
                if is_sensitive_memory_confirmation(&frozen.message)
                    && (frozen.confirmation.is_none()
                        || frozen.confirmation.as_deref()
                            != user.agent_history.confirmation_id(&row.chat))
                {
                    return Err(pending_confirmation::required());
                }
                access
                    .library
                    .lock()
                    .unwrap()
                    .authorize(&row.owner, &row.publication)
                    .map_err(|_| fault("RUN_PERMISSION_REVOKED", "permission"))?;
                frozen.teaching.resume(&user)?;
                Ok(frozen)
            })();
            match check {
                Ok(f) => f,
                Err(e) => {
                    if self
                        .end_pending(&mut user, &row, &e.error_code, false)
                        .is_err()
                    {
                        self.mark_unsaved(&row);
                        return Err(storage());
                    }
                    self.update(&row, "settled", false)?;
                    return Ok(true);
                }
            }
        };
        let provider = self.provider.lock().unwrap().clone();
        let Some(provider) = provider.filter(|p| p.binding == frozen.provider) else {
            let mut user = handle.lock().unwrap();
            if let Err(e) = self.end_pending(&mut user, &row, "PROVIDER_BINDING_UNAVAILABLE", false)
            {
                self.mark_unsaved(&row);
                return Err(e);
            }
            self.update(&row, "settled", false)?;
            return Ok(true);
        };
        let prepared = frozen.prepared(access)?;
        let mut port = workspace_registry::NetworkRunPort::resume(
            access.clone(),
            handle.clone(),
            prepared.scope.clone(),
            frozen.turn_ref(),
            frozen.effect_revision,
        )?;
        let cancellation = CancellationToken::default();
        let stream;
        {
            let mut state = self.state.lock().unwrap();
            if state.stopping
                || state
                    .finishing
                    .contains(&(row.owner.clone(), row.turn.clone()))
                || state
                    .deleting
                    .contains(&(row.owner.clone(), row.chat.clone()))
            {
                return Ok(false);
            }
            let changed=state.control.connection.execute("UPDATE run_admissions SET dispatch_state='claimed',boot_id=?,attempt=attempt+1 WHERE owner_user_id=? AND turn_id=? AND dispatch_state='queued' AND cancel_requested=0 AND unsaved=0",params![self.boot,row.owner,row.turn]).map_err(|_|storage())?;
            if changed != 1 {
                return Ok(false);
            }
            stream = state
                .streams
                .entry((row.owner.clone(), row.turn.clone()))
                .or_insert_with(|| {
                    agent_stream::RunStream::queued(
                        agent_stream::RunDescriptor {
                            book_id: row.publication.book_id.clone(),
                            session_id: row.chat.clone(),
                            turn_id: row.turn.clone(),
                        },
                        &self.boot,
                        self.limits.event_bytes,
                    )
                })
                .clone();
            stream.started();
            state.active.insert(
                (row.owner.clone(), row.turn.clone()),
                Active {
                    cancellation: cancellation.clone(),
                    stream: stream.clone(),
                },
            );
        }
        self.point("claimed")?;
        port.observe(&stream);
        let user_port = NetworkUserPort {
            user: handle.clone(),
            access: access.clone(),
            owner: row.owner.clone(),
            publication: row.publication.clone(),
        };
        let provider_adapter = (provider.make)();
        // Every nested model purpose inherits this server-owned run attribution.
        let spend_scope = runtime::model_spend::ChargeScope::ReaderRun {
            user_id: row.owner.clone(), run_ref: row.turn.clone(),
        };
        provider_adapter.set_spend_context(spend_scope.clone(), Some(access.spend.port(
            spend_scope, row.publication.clone(), cancellation.clone(),
        )));
        let adapter = crate::service_limits::LimitedAdapter {
            inner: provider_adapter.as_ref(),
            resources: access.resources.clone(),
            owner: row.owner.clone(),
            cancellation: cancellation.clone(),
            usage: Mutex::new(Default::default()),
            authorization: Some((access, &row.publication)),
            stream: Some(stream.clone()),
        };
        let observation = self.observability.start_run_with_input(
            &prepared.scope.book.base.book_id,
            &prepared.turn_ref.session_id,
            || json!({"message":prepared.message,"messages":prepared.messages}),
        );
        let sink: Arc<dyn runtime::run_events::RunEventSink> = match &observation {
            Some(run) => Arc::new(agent_run::RunEventFanout {
                stream: stream.clone(),
                observation: run.sink(),
            }),
            None => stream.clone(),
        };
        let mut finished = agent_run::execute_model(
            &adapter,
            &prepared,
            cancellation,
            Some(&stream),
            Some(sink),
            observation.as_ref().map(|run| run.event_anchor()).unwrap_or_else(std::time::Instant::now),
            |adapter, prepared, context| {
                run_precommitted_with_ports(&user_port, &mut port, adapter, prepared, context)
            },
        );
        finished.summary.usage = Some(adapter.usage.lock().unwrap().clone());
        let saved = self.save_finished(&handle, &prepared, &finished);
        if let Some(run) = observation {
            let outcome = finished.result.as_ref().ok();
            let error = finished.result.as_ref().err().map(|error| error.error_code.as_str())
                .or_else(|| outcome.filter(|outcome| crate::observability::lifecycle::delivery_failed(outcome))
                    .map(|_| "ANSWER_DELIVERY_FAILED"));
            run.finish(
                outcome,
                finished.cancelled,
                if saved.is_ok() { runtime::observation::PersistenceState::Saved }
                else { runtime::observation::PersistenceState::Failed },
                error,
            );
        }
        match saved {
            Ok(view) => {
                stream.finish(Some(view), None);
                self.point("terminal")?;
                self.update(&row, "settled", false)?;
            }
            Err(_) => {
                stream.finish(None, Some(json!({
                    "error_code":"TURN_UNSAVED",
                    "message":"本次运行结果尚未保存，请重试保存。",
                    "execution_error":finished.result.as_ref().err().map(|error| json!({
                        "error_code":error.error_code,"category":error.category,"message":error.message
                    }))
                })));
                self.mark_unsaved(&row);
                self.state.lock().unwrap().unsaved.insert(
                    (row.owner.clone(), row.turn.clone()),
                    Unsaved {
                        user: handle,
                        prepared,
                        finished,
                        stream,
                    },
                );
            }
        }
        Ok(true)
    }
    fn repair_saved(
        &self,
        user: &mut crate::user_runtime::UserRuntime,
        row: &Admission,
    ) -> Result<(), ToolError> {
        let turn = find_turn(user, row).ok_or_else(missing)?;
        let reference = AgentTurnRef {
            session_id: row.chat.clone(),
            turn_id: row.turn.clone(),
            user_turn_ordinal: turn.user_turn_ordinal,
        };
        let now = frozen_input(user, row)?
            .as_ref()
            .map(|f| f.now.clone())
            .ok_or_else(|| fault("RUN_INPUT_MISSING", "conflict"))?;
        teaching::record_user_delivery(user, &reference, &now)?;
        session_runtime::link_teaching(user, &reference, &now)?;
        let cursors = agent_history_review_cursors(&user.agent_history);
        user.store.reconcile_review_jobs(&cursors, &now)?;
        self.record_completed(user, &reference)?;
        self.update(row, "settled", false)
    }
    fn save_finished(
        &self,
        handle: &UserHandle,
        prepared: &PreparedAgentChat,
        finished: &FinishedRun,
    ) -> Result<Value, ToolError> {
        let mut user = handle.lock().unwrap();
        #[cfg(test)]
        let _timing = crate::tests::mu10_tests::measure("user_lock");
        let (status, outcome, error) = match &finished.result {
            Ok(outcome) => (AgentAssistantStatus::Completed, Some(outcome.clone()), None),
            Err(e) => (
                if finished.cancelled {
                    AgentAssistantStatus::Cancelled
                } else {
                    AgentAssistantStatus::Failed
                },
                None,
                Some(AgentTurnError {
                    error_code: e.error_code.clone(),
                    category: e.category.clone(),
                    message: agent_run::saved_error_message(e),
                }),
            ),
        };
        let already_saved = user
            .agent_history
            .sessions
            .iter()
            .find(|s| s.id == prepared.turn_ref.session_id)
            .and_then(|s| {
                s.turns
                    .iter()
                    .find(|t| t.turn_id == prepared.turn_ref.turn_id)
            })
            .is_some_and(|t| t.status != AgentAssistantStatus::PendingAssistant);
        if !already_saved {
            finalize_user_agent_turn(
                &mut user,
                &prepared.turn_ref,
                status,
                outcome,
                error,
                Some(finished.summary.clone()),
                &finished.messages,
                &prepared.now,
            )?;
        }
        teaching::record_delivery(
            &prepared.scope.private_user(&user),
            &prepared.turn_ref,
            &prepared.now,
        )?;
        session_runtime::link_teaching(&mut user, &prepared.turn_ref, &prepared.now)?;
        let cursors = agent_history_review_cursors(&user.agent_history);
        user.store.reconcile_review_jobs(&cursors, &prepared.now)?;
        self.record_completed(&user, &prepared.turn_ref)?;
        let t = user
            .agent_history
            .sessions
            .iter()
            .find(|s| s.id == prepared.turn_ref.session_id)
            .and_then(|s| {
                s.turns
                    .iter()
                    .find(|t| t.turn_id == prepared.turn_ref.turn_id)
            })
            .ok_or_else(missing)?;
        Ok(json!(turn_view(&prepared.scope.book, t)))
    }
    fn record_completed(&self, user: &user_runtime::UserRuntime, reference: &AgentTurnRef) -> Result<(),ToolError> {
        let session = user.agent_history.sessions.iter().find(|s|s.id==reference.session_id).ok_or_else(missing)?;
        let turn = session.turns.iter().find(|t|t.turn_id==reference.turn_id).ok_or_else(missing)?;
        if crate::admin_usage::delivered(session,turn) {
            crate::admin_usage::record(&self.state.lock().unwrap().control.connection, user.user_id().ok_or_else(missing)?, "completed", &reference.turn_id, crate::multi_user_host::now())?;
        }
        Ok(())
    }
    pub(crate) fn lookup(
        &self,
        context: &AuthorizedContext,
        key: &str,
    ) -> Result<Value, ToolError> {
        let row = self.by_key(context.user_id(), key)?.ok_or_else(missing)?;
        self.status(context, &row.turn)
    }
    pub(crate) fn status(
        &self,
        context: &AuthorizedContext,
        turn: &str,
    ) -> Result<Value, ToolError> {
        let row = self.by_turn(context.user_id(), turn)?;
        if row.closed {
            return Err(closed());
        }
        let mut value = descriptor(&row);
        if let Ok(saved) = context.turn(turn) {
            value["turn"] = saved["turn"].clone();
        }
        let state = self.state.lock().unwrap();
        if let Some(stream) = state.streams.get(&(context.user_id().into(), turn.into())) {
            value["snapshot"] = json!(stream.snapshot());
        }
        if let Some(run) = state.unsaved.get(&(context.user_id().into(), turn.into())) {
            value["snapshot"] = json!(run.stream.snapshot());
            value["persistence_state"] = json!("failed");
        }
        Ok(value)
    }
    pub(crate) fn stream(
        &self,
        context: &AuthorizedContext,
        turn: &str,
    ) -> Option<Arc<agent_stream::RunStream>> {
        let mut state = self.state.lock().unwrap();
        let row = state
            .control
            .connection
            .query_row(
                &format!(
                    "SELECT {COLUMNS} FROM run_admissions WHERE owner_user_id=? AND turn_id=?"
                ),
                params![context.user_id(), turn],
                read_row,
            )
            .ok()?;
        if row.closed || !matches!(row.state.as_str(), "queued" | "claimed" | "preparing") {
            return None;
        }
        if let Some(run) = state.unsaved.get(&(row.owner.clone(), row.turn.clone())) {
            return Some(run.stream.clone());
        }
        Some(
            state
                .streams
                .entry((row.owner.clone(), row.turn.clone()))
                .or_insert_with(|| {
                    let stream = agent_stream::RunStream::queued(
                        agent_stream::RunDescriptor {
                            book_id: row.publication.book_id.clone(),
                            session_id: row.chat.clone(),
                            turn_id: row.turn.clone(),
                        },
                        &self.boot,
                        self.limits.event_bytes,
                    );
                    if row.state == "preparing" {
                        stream.dispatch_state("preparing");
                    }
                    if row.unsaved {
                        stream.finish(None, Some(json!({"error_code":"TURN_UNSAVED"})));
                    }
                    stream
                })
                .clone(),
        )
    }
    /// Account disable persists cancel_requested in the account transaction first.
    /// Queued work uses run_one/end_pending; active work uses the normal cancel token.
    /// No reader workspace or impersonated Principal is needed.
    pub(crate) fn cancel_disabled_user(&self, owner: &str) {
        let state = self.state.lock().unwrap();
        for ((user, _), active) in &state.active {
            if user == owner {
                active.cancellation.cancel();
                active.stream.cancelling();
            }
        }
    }

    pub(crate) fn cancel(
        &self,
        context: &AuthorizedContext,
        turn: &str,
    ) -> Result<Value, ToolError> {
        let row = {
            let state = self.state.lock().unwrap();
            let row = state
                .control
                .connection
                .query_row(
                    &format!(
                        "SELECT {COLUMNS} FROM run_admissions WHERE owner_user_id=? AND turn_id=?"
                    ),
                    params![context.user_id(), turn],
                    read_row,
                )
                .optional()
                .map_err(|_| storage())?
                .ok_or_else(missing)?;
            if row.closed {
                return Err(closed());
            }
            if matches!(row.state.as_str(), "preparing" | "queued" | "claimed") {
                state.control.connection.execute("UPDATE run_admissions SET cancel_requested=1 WHERE owner_user_id=? AND turn_id=?",params![context.user_id(),turn]).map_err(|_|storage())?;
                if let Some(active) = state.active.get(&(context.user_id().into(), turn.into())) {
                    active.cancellation.cancel();
                    active.stream.cancelling();
                }
            }
            row
        };
        if row.state == "queued" && !row.unsaved {
            let mut user = context.user.lock().unwrap();
            if let Err(e) = self.end_pending(&mut user, &row, "AGENT_RUN_CANCELLED", true) {
                self.mark_unsaved(&row);
                return Err(e);
            }
            self.update(&row, "settled", false)?;
        }
        self.status(context, turn)
    }
    pub(crate) fn retry_save(
        &self,
        context: &AuthorizedContext,
        turn: &str,
    ) -> Result<Value, ToolError> {
        let row = self.by_turn(context.user_id(), turn)?;
        let key = (row.owner.clone(), row.turn.clone());
        let unsaved = {
            let mut state = self.state.lock().unwrap();
            if state.reserved.contains(&key) || !state.finishing.insert(key.clone()) {
                return Err(busy());
            }
            state.unsaved.remove(&key)
        };
        let _finish = FinishGuard {
            runs: self,
            key: key.clone(),
        };
        if let Some(run) = unsaved {
            match self.save_finished(&run.user, &run.prepared, &run.finished) {
                Ok(view) => {
                    run.stream.finish(Some(view), None);
                    self.update(&row, "settled", false)?;
                }
                Err(e) => {
                    self.state.lock().unwrap().unsaved.insert(key, run);
                    return Err(e);
                }
            }
        } else {
            let mut user = context.user.lock().unwrap();
            if find_turn(&user, &row)
                .is_some_and(|t| t.status != AgentAssistantStatus::PendingAssistant)
            {
                self.repair_saved(&mut user, &row)?;
            } else if row.unsaved {
                self.end_pending(
                    &mut user,
                    &row,
                    if row.cancel {
                        "AGENT_RUN_CANCELLED"
                    } else {
                        "INTERRUPTED"
                    },
                    row.cancel,
                )?;
                self.update(
                    &row,
                    if row.state == "preparing" {
                        "admission_failed"
                    } else {
                        "settled"
                    },
                    false,
                )?;
            } else {
                return Err(fault("TURN_NOT_UNSAVED", "conflict"));
            }
        }
        self.status(context, turn)
    }
    pub(crate) fn delete_chat(
        &self,
        access: &Authorization,
        context: &AuthorizedContext,
        chat: &str,
    ) -> Result<Value, ToolError> {
        let key = (context.user_id().to_owned(), chat.to_owned());
        {
            let mut state = self.state.lock().unwrap();
            let busy:bool=state.control.connection.query_row("SELECT EXISTS(SELECT 1 FROM run_admissions WHERE owner_user_id=? AND chat_session_id=? AND dispatch_state IN ('preparing','queued','claimed'))",params![context.user_id(),chat],|r|r.get(0)).map_err(|_|storage())?;
            if busy || !state.deleting.insert(key.clone()) {
                return Err(self::busy());
            }
        }
        let _delete = DeleteGuard { runs: self, key };
        let mut user = context.user.lock().unwrap();
        if !user.agent_history.sessions.iter().any(|s| s.id == chat) {
            return Err(missing());
        }
        if user
            .agent_history
            .sessions
            .iter()
            .filter(|s| s.id == chat)
            .flat_map(|s| &s.turns)
            .any(|t| t.status == AgentAssistantStatus::PendingAssistant)
        {
            return Err(busy());
        }
        if user.session_store.is_some() {
            let user = &mut *user;
            let result = user.session_store.as_mut().unwrap().delete(&mut user.agent_history, chat);
            if result.is_err() && !user.agent_history.sessions.iter().any(|s| s.id == chat) {
                access.workspaces.lock().unwrap().clear_chat(context.user_id(), chat)?;
            }
            result?;
        } else {
            let mut candidate = user.agent_history.clone();
            candidate.sessions.retain(|s| s.id != chat);
            candidate.active_by_book.retain(|_, id| id != chat);
            candidate.pending_confirmations.remove(chat);
            candidate.pending_memory_ops.remove(chat);
            candidate.pending_governance_mutations.remove(chat);
            save_agent_history_path(&user.history_path, &candidate)?;
            user.agent_history = candidate;
        }
        self.point("chat_deleted")?;
        self.state.lock().unwrap().control.connection.execute("UPDATE run_admissions SET key_closed=1 WHERE owner_user_id=? AND chat_session_id=?",params![context.user_id(),chat]).map_err(|_|storage())?;
        access
            .workspaces
            .lock()
            .unwrap()
            .clear_chat(context.user_id(), chat)?;
        Ok(json!({"deleted":true,"session_id":chat}))
    }
    pub(crate) fn is_stopping(&self) -> bool {
        self.state.lock().unwrap().stopping
    }
    pub(crate) fn stop(&self) {
        let mut state = self.state.lock().unwrap();
        state.stopping = true;
        for active in state.active.values() {
            active.cancellation.cancel();
            active.stream.cancelling();
        }
    }
}
struct ActiveGuard<'a>(&'a RunAdmissions, (String, String));
impl Drop for ActiveGuard<'_> {
    fn drop(&mut self) {
        let mut state = self.0.state.lock().unwrap();
        // A pre-claim capacity/storage retry still owns the same queued observation stream.
        if state.active.remove(&self.1).is_some() {
            state.streams.remove(&self.1);
        }
        state.reserved.remove(&self.1);
    }
}
struct FinishGuard<'a> {
    runs: &'a RunAdmissions,
    key: (String, String),
}
impl Drop for FinishGuard<'_> {
    fn drop(&mut self) {
        self.runs.state.lock().unwrap().finishing.remove(&self.key);
    }
}
struct DeleteGuard<'a> {
    runs: &'a RunAdmissions,
    key: (String, String),
}
impl Drop for DeleteGuard<'_> {
    fn drop(&mut self) {
        self.runs.state.lock().unwrap().deleting.remove(&self.key);
    }
}
fn validate_frozen(
    user: &user_runtime::UserRuntime,
    row: &Admission,
    f: &FrozenTurn,
) -> Result<(), ToolError> {
    let session = user
        .agent_history
        .sessions
        .iter()
        .find(|s| s.id == row.chat)
        .ok_or_else(missing)?;
    let valid = f.version == 1
        && f.owner == row.owner
        && f.workspace == row.workspace
        && f.generation == row.generation
        && f.chat == row.chat
        && f.turn == row.turn
        && f.publication == row.publication
        && session
            .turns
            .last()
            .is_some_and(|t| t.turn_id == row.turn && t.user_turn_ordinal == f.ordinal)
        && session.turns.iter().rev().nth(1).map(|t| &t.turn_id) == f.head.as_ref()
        && (user.session_store.is_some() || session.messages == f.messages);
    if valid {
        Ok(())
    } else {
        Err(fault("RUN_INPUT_INCOMPLETE", "conflict"))
    }
}
