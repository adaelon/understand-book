//! Candidates remain private until an observed whole answer or explicit local part commits.
use crate::*;
use memory::boards::*;
use runtime::board_author::{AuthorRequest, AuthorResult};
use runtime::presentation::{PresentationOwner, PresentationRef};
use runtime::run_context::CancellationToken;

#[path = "board_author_edits.rs"]
mod edits;
#[path = "board_author_confirmation.rs"]
mod confirmation;
pub(crate) use confirmation::{candidate_confirmation, candidate_preview, confirmation_update};
#[path = "board_author_regeneration.rs"]
mod regeneration;
pub(crate) use regeneration::{RegenerationContext, regeneration_context, regeneration_public_context};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Candidate {
    pub board_id: String,
    pub base_board_revision: u32,
    pub candidate_revision: u32,
    pub candidate_id: String,
    pub items: Vec<BoardItem>,
    #[serde(default)]
    pub update_items: Vec<BoardItem>,
    #[serde(default)]
    pub operations: Vec<BoardEditOperation>,
    #[serde(default)]
    pub read_item_ids: Vec<String>,
    /// Exact semantic base, including resolved shared-note text. Kept private;
    /// it is not another model-facing copy of the board.
    #[serde(default)]
    pub edit_base: Option<BoardDocument>,
    #[serde(default)]
    pub conflict: Option<Value>,
    #[serde(default)]
    pub note_revisions: std::collections::BTreeMap<String, u32>,
    #[serde(default)]
    pub regeneration: Option<Box<RegenerationContext>>,
    #[serde(default)]
    pub requires_confirmation: bool,
    #[serde(default)]
    pub confirmed: bool,
    #[serde(default)]
    pub dismissed: bool,
    pub links: Vec<BoardLink>,
    pub mappings: Vec<BoardExplanationMapping>,
    pub routes: Vec<BoardRoute>,
    /// Author-owned source text for each stable block. Rendered blocks contain
    /// compiled Markdown and cannot reconstruct source markers or boundaries.
    #[serde(default)]
    pub explanation_blocks: Vec<runtime::board_author::ExplanationBlock>,
    pub delivery_text: String,
    pub answer_parts: Vec<Value>,
    pub previewed: bool,
    pub staged: bool,
    #[serde(default)]
    pub source_bindings: Vec<SourceBinding>,
    #[serde(default)]
    #[serde(alias = "commit_review")]
    pub commit_selection: Option<runtime::board_author::SelectedDelivery>,
    #[serde(default)]
    pub commit_requested_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CommittedPart {
    pub delivery: BoardDelivery,
    pub candidate_revision: u32,
    pub delivery_text: String,
    pub answer_parts: Vec<Value>,
    pub source_bindings: Vec<SourceBinding>,
    pub teaching_delivery_move_refs: Vec<String>,
    pub committed_at: String,
    #[serde(default)]
    pub delivery_scope: Option<Value>,
}

pub(crate) trait Storage {
    fn source_workspace(&self, reference: Option<&base_schema::PublishedBookRef>) -> Result<reader_workspace::ReaderWorkspace, ToolError>;
    fn with_user<R>(
        &self,
        operation: impl FnOnce(&mut user_runtime::UserRuntime) -> Result<R, ToolError>,
    ) -> Result<R, ToolError>;
}

struct LocalStorage<'a, P> {
    port: &'a P,
    scope: &'a run_scope::RunScope,
}
impl<P: agent_run::AppStatePort> Storage for LocalStorage<'_, P> {
    fn source_workspace(&self, reference: Option<&base_schema::PublishedBookRef>) -> Result<reader_workspace::ReaderWorkspace, ToolError> {
        if reference.is_some_and(|reference| self.scope.publication.as_ref().map(|p| &p.reference) != Some(reference)) {
            return Err(authorization::missing());
        }
        Ok(source_workspace(self.scope, self.scope.publication.clone()))
    }
    fn with_user<R>(
        &self,
        operation: impl FnOnce(&mut user_runtime::UserRuntime) -> Result<R, ToolError>,
    ) -> Result<R, ToolError> {
        self.port.with_app(|state| {
            self.scope.check_owner(state)?;
            operation(&mut state.user)
        })
    }
}

impl<P: agent_run::AppStatePort> agent_run::RuntimeStatePort<'_, P> {
    pub(crate) fn author_board_candidate(
        &self,
        request: AuthorRequest,
        bindings: &[SourceBinding],
        messages: &[Message],
        cancellation: &CancellationToken,
    ) -> Result<AuthorResult, ToolError> {
        let frozen = self
            .scope
            .board_context
            .as_ref()
            .ok_or_else(runtime::board_author::unavailable)?;
        execute(
            &LocalStorage {
                port: self.port,
                scope: self.scope,
            },
            frozen,
            self.turn_ref,
            request,
            bindings,
            messages,
            cancellation,
        )
    }
}

/// Source tools use the accepted run's immutable source scope, never the active Reader.
pub(crate) fn source_workspace(scope: &run_scope::RunScope, publication: Option<std::sync::Arc<published_library::PublishedBook>>) -> reader_workspace::ReaderWorkspace {
    let book = publication.as_ref().map_or_else(|| scope.reading_book().clone(), |p| p.book.clone());
    let directory = publication.as_ref().map_or_else(|| scope.book_dir.clone(), |p| p.directory.clone());
    let reader = reader::Reader::new(&book, reader::DEFAULT_RADIUS);
    let mut workspace = reader_workspace::ReaderWorkspace::local(directory, book, reader, vec![], None);
    workspace.learning_workspace_id = scope.learning_workspace_id.clone();
    workspace.learning_members = scope.members.clone();
    workspace.publication = publication;
    workspace.selected_chat = Some(scope.chat_session_id.clone());
    workspace
}

fn error(message: impl Into<String>) -> ToolError {
    runtime::board_author::error("BOARD_AUTHORING_INVALID", message)
}
pub(crate) fn reserved_answer_item_id(turn_id: &str) -> String {
    format!("item-board-{turn_id}")
}

fn next_answer_item_id(turn: &AgentChatTurn) -> String {
    let base = reserved_answer_item_id(&turn.turn_id);
    if turn.domain.board_commits.is_empty() {
        base
    } else {
        format!("{base}-part-{}", turn.domain.board_commits.len() + 1)
    }
}

fn presentation_item_id(answer_item_id: &str, reference: &memory::NotePresentationRef) -> String {
    // Reversible encoding keeps the exact presentation identity legal as a
    // board ID. Reordering a candidate's presentation list cannot rename it.
    let exact_id = reference
        .presentation_id
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!(
        "{answer_item_id}-presentation-{exact_id}-r{}",
        reference.revision
    )
}

/// Recover only an already-written board receipt. This never publishes a draft.
pub(crate) fn repair_commits(
    user: &mut user_runtime::UserRuntime,
    reference: &AgentTurnRef,
    _now: &str,
) -> Result<Vec<BoardDelivery>, ToolError> {
    if let Some(store) = user.session_store.as_mut() {
        store.settle(&mut user.agent_history, &reference.session_id)?;
    }
    let current = user
        .agent_history
        .sessions
        .iter()
        .find(|session| session.id == reference.session_id)
        .and_then(|session| {
            session
                .turns
                .iter()
                .find(|turn| turn.turn_id == reference.turn_id)
        })
        .ok_or_else(|| error("Original board part turn is unavailable"))?
        .clone();
    let Some(frozen) = current.domain.board_context.as_ref() else {
        return Ok(vec![]);
    };
    let intent = current
        .domain
        .board_authoring
        .as_ref()
        .filter(|candidate| candidate.commit_selection.is_some());
    if intent.is_none() && current.domain.board_commits.is_empty() {
        return Ok(vec![]);
    }
    let store = board_api::scoped_store(user, frozen.learning_workspace_id.as_deref())?;
    let receipts = match store.deliveries_for_turn(
        &frozen.follow_up.board_id,
        &reference.session_id,
        &reference.turn_id,
    ) {
        Ok(receipts) => receipts
            .into_iter()
            .filter(|receipt| receipt.commit_id.is_some())
            .collect::<Vec<_>>(),
        Err(error) if error.error_code == "BOARD_NOT_FOUND" => current
            .domain
            .board_commits
            .iter()
            .map(|part| part.delivery.clone())
            .collect(),
        Err(error) => return Err(error),
    };
    for delivery in &receipts {
        let retained = current
            .domain
            .board_commits
            .iter()
            .find(|part| part.delivery.commit_id == delivery.commit_id);
        let part = if let Some(part) = retained {
            part.clone()
        } else {
            let candidate = intent
                .filter(|candidate| Some(&candidate.candidate_id) == delivery.commit_id.as_ref())
                .ok_or_else(|| {
                    error("A durable board part is missing its frozen selected candidate")
                })?;
            let review = candidate.commit_selection.as_ref().expect("intent checked");
            CommittedPart {
                delivery: delivery.clone(),
                candidate_revision: candidate.candidate_revision,
                delivery_text: candidate.delivery_text.clone(),
                answer_parts: candidate.answer_parts.clone(),
                source_bindings: candidate.source_bindings.clone(),
                teaching_delivery_move_refs: review.teaching_delivery_move_refs.clone(),
                committed_at: candidate
                    .commit_requested_at
                    .clone()
                    .ok_or_else(|| error("A board commit intent is missing its saved time"))?,
                delivery_scope: review.delivery_scope.clone(),
            }
        };
        session_runtime::record_board_commit(user, reference, &part)?;
        let presentations = part
            .answer_parts
            .iter()
            .filter(|part| part["kind"] == "presentation")
            .map(|part| {
                serde_json::from_value::<PresentationRef>(
                    json!({"presentation_id":part["presentation_id"],"revision":part["revision"]}),
                )
                .map_err(|error| crate::board_author::error(error.to_string()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        session_runtime::deliver_presentations(
            user,
            reference,
            &presentations,
            &part.committed_at,
        )?;
        teaching::record_board_local_delivery(
            user,
            reference,
            part.delivery.commit_id.as_deref().expect("part receipt"),
            &part.committed_at,
        )?;
    }
    Ok(receipts)
}

pub(crate) fn same_retained_identity(left: &BoardItem, right: &BoardItem) -> bool {
    left.item_id == right.item_id
        || matches!((&left.target, &right.target),
        (BoardTarget::Note { note_id: left, .. }, BoardTarget::Note { note_id: right, .. }) if left == right)
        || matches!((&left.target, &right.target),
        (BoardTarget::Understanding { evidence_id: left, .. }, BoardTarget::Understanding { evidence_id: right, .. }) if left == right)
}

fn read_current(
    user: &user_runtime::UserRuntime,
    frozen: &board_api::FrozenBoardFollowUp,
) -> Result<(BoardStore, BoardDocument), ToolError> {
    let (store, board) = read_current_document(user, frozen)?;
    if frozen.follow_up.item_id.as_ref().is_some_and(|id| !board.items.iter().any(|item| &item.item_id == id)) {
        return Err(error(
            "The selected item was removed; open a current board item before authoring",
        ));
    }
    if frozen.follow_up.item_id.as_ref().is_some_and(|id| board.items.iter().any(|item|
        &item.item_id == id && matches!(item.target, BoardTarget::Note { missing: true, .. }))) {
        return Err(error("The selected workspace note was deleted; choose another note before authoring"));
    }
    Ok((store, board))
}
fn read_current_document(
    user: &user_runtime::UserRuntime,
    frozen: &board_api::FrozenBoardFollowUp,
) -> Result<(BoardStore, BoardDocument), ToolError> {
    let store = board_api::scoped_store(user, frozen.learning_workspace_id.as_deref())?;
    let board = store.read(&frozen.follow_up.board_id)?;
    if board.book_id != frozen.book_id
        || board.learning_workspace_id != frozen.learning_workspace_id
        || board.publication_id.as_ref() != frozen.publication.as_ref().map(|p| &p.publication_id)
    {
        return Err(authorization::missing());
    }
    Ok((store, board))
}
fn turn<'a>(
    user: &'a user_runtime::UserRuntime,
    reference: &AgentTurnRef,
) -> Result<&'a AgentChatTurn, ToolError> {
    user.agent_history
        .sessions
        .iter()
        .find(|s| s.id == reference.session_id)
        .and_then(|s| s.turns.iter().find(|t| t.turn_id == reference.turn_id))
        .filter(|t| t.status == AgentAssistantStatus::PendingAssistant)
        .ok_or_else(|| error("Board authoring requires its current pending turn"))
}
fn selected_candidate(
    user: &user_runtime::UserRuntime,
    reference: &AgentTurnRef,
    revision: u32,
) -> Result<Candidate, ToolError> {
    turn(user, reference)?.domain.board_authoring.as_ref().filter(|c| c.candidate_revision == revision).cloned()
        .ok_or_else(|| error("Read the current candidate_revision; the requested candidate is unavailable or was replaced"))
}
fn project(board: &BoardDocument, candidate: &Candidate) -> Result<BoardDocument, ToolError> {
    if board.board_id != candidate.board_id || board.revision != candidate.base_board_revision {
        return Err(runtime::board_author::error("BOARD_REVISION_CONFLICT", "The board changed after this candidate was prepared; read, prepare and preview the new layout"));
    }
    let mut projected = board.clone();
    for update in &candidate.update_items {
        let retained = projected.items.iter_mut().find(|item| item.item_id == update.item_id)
            .ok_or_else(|| error("The updated board item is unavailable"))?;
        *retained = update.clone();
    }
    projected.items.extend(candidate.items.clone());
    projected.links.extend(candidate.links.clone());
    projected.mappings.extend(candidate.mappings.clone());
    projected.routes.extend(candidate.routes.clone());
    if candidate.operations.is_empty() { projected.revision += 1; }
    else { projected = apply_board_edits(&projected, &candidate.operations, &board.updated_at)?; }
    if let Some(regeneration)=&candidate.regeneration { regeneration::require_preserved(regeneration,board,&projected)?; }
    Ok(projected)
}
pub(crate) fn validate_current(store: &BoardStore, board: &BoardDocument, candidate: &Candidate) -> Result<BoardDocument, ToolError> {
    if candidate.note_revisions != store.note_revisions(board, &candidate.read_item_ids)? {
        return Err(runtime::board_author::error("BOARD_EDIT_CONFLICT", "A shared note used by this suggestion changed. Keep the reader's current content and redo the suggestion from that note."));
    }
    project(board, candidate)
}
fn readable(parts: &[AgentAnswerPart]) -> String {
    parts
        .iter()
        .filter_map(|part| match part {
            AgentAnswerPart::Markdown { text } => Some(text.as_str()),
            AgentAnswerPart::MusicExample { example } => Some(example.label.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}
pub(crate) fn next_position(
    board: &BoardDocument,
    parent: Option<&BoardItem>,
    size: &BoardSize,
) -> BoardPosition {
    let x = parent.map_or(80.0, |item| item.position.x + item.size.width + 72.0);
    let mut y = parent.map_or(80.0, |item| item.position.y);
    loop {
        let next = board
            .items
            .iter()
            .filter(|item| {
                x < item.position.x + item.size.width + 16.0
                    && x + size.width + 16.0 > item.position.x
                    && y < item.position.y + item.size.height + 16.0
                    && y + size.height + 16.0 > item.position.y
            })
            .map(|item| item.position.y + item.size.height + 32.0)
            .max_by(f64::total_cmp);
        match next {
            Some(next) => y = next,
            None => return BoardPosition { x, y },
        }
    }
}
fn require_declared_anchors(
    user: &user_runtime::UserRuntime,
    board: &BoardDocument,
) -> Result<(), ToolError> {
    let mut targets = Vec::new();
    for link in &board.links {
        targets.extend([&link.from, &link.to]);
    }
    for mapping in &board.mappings {
        targets.push(&mapping.visual);
    }
    for route in &board.routes {
        for step in &route.steps {
            targets.extend(step.focus_targets.iter());
            targets.extend(step.reveal_targets.iter());
        }
    }
    for target in targets {
        let BoardVisualTarget::PresentationAnchor {
            item_id,
            reference,
            anchor_id,
        } = target
        else {
            continue;
        };
        let item = board
            .items
            .iter()
            .find(|item| &item.item_id == item_id)
            .ok_or_else(|| error("Presentation item unavailable"))?;
        let BoardTarget::Presentation { receipt, source_book_id } = &item.target else {
            return Err(error("An anchor requires a retained presentation"));
        };
        let owner = PresentationOwner {
            book_id: source_book_id.clone().unwrap_or_else(||board.book_id.clone()),
            session_id: receipt.session_id.clone(),
        };
        let reference = runtime::presentation::PresentationRef {
            presentation_id: reference.presentation_id.clone(),
            revision: reference.revision,
        };
        let version = presentation_store::PresentationStore::for_user(user)?
            .read_version(&owner, &reference)?;
        let declared = version
            .content
            .teaching_binding
            .as_ref()
            .is_some_and(|binding| {
                binding
                    .regions
                    .iter()
                    .any(|region| &region.region_id == anchor_id)
            })
            || version
                .content
                .action_contract
                .iter()
                .any(|action| match action {
                    runtime::presentation_interact::PresentationActionContract::Highlight {
                        region_id,
                    }
                    | runtime::presentation_interact::PresentationActionContract::Reveal {
                        region_id,
                    }
                    | runtime::presentation_interact::PresentationActionContract::Annotate {
                        region_id,
                    } => region_id == anchor_id,
                    _ => false,
                });
        if !declared {
            return Err(error(format!(
                "Presentation anchor {anchor_id} is not declared in the exact retained version"
            )));
        }
    }
    Ok(())
}

pub(crate) fn execute(
    storage: &impl Storage,
    frozen: &board_api::FrozenBoardFollowUp,
    reference: &AgentTurnRef,
    request: AuthorRequest,
    bindings: &[SourceBinding],
    messages: &[Message],
    cancellation: &CancellationToken,
) -> Result<AuthorResult, ToolError> {
    cancellation.check()?;
    // A board is retained independently of the active Reader and final chat.
    // Give implicit current-book sources their exact publication before prepare,
    // revise or an incremental commit can persist them, including unscoped runs.
    let mut published_bindings = bindings.to_vec();
    if let Some(publication) = &frozen.publication {
        for binding in &mut published_bindings {
            if binding.publication_id.is_none() && binding.book_id == publication.book_id {
                binding.publication_id = Some(publication.publication_id.clone());
            }
        }
    }
    let bindings = published_bindings.as_slice();
    let now = crate::multi_user_host::now().to_string();
    storage.with_user(|user| repair_commits(user, reference, &now).map(|_| ()))?;
    let source_workspaces = match &request {
        AuthorRequest::Catalog { published_book_ref, .. } => vec![storage.source_workspace(published_book_ref.as_ref())?],
        AuthorRequest::SourceRead { source, published_book_ref } => vec![storage.source_workspace(
            if matches!(source["kind"].as_str(), Some("answer" | "presentation")) { None } else { published_book_ref.as_ref() })?],
        AuthorRequest::Prepare { imports, .. } => imports.iter().map(|entry| storage.source_workspace(
            if matches!(entry.source["kind"].as_str(), Some("answer" | "presentation")) { None } else { entry.published_book_ref.as_ref() })).collect::<Result<Vec<_>, _>>()?,
        _ => vec![],
    };
    match request {
        AuthorRequest::Plan { .. } => Err(runtime::board_author::error("BOARD_PLAN_RUNTIME_REQUIRED", "Expression planning is handled by the current runtime before board authoring.")),
        AuthorRequest::Catalog { kind, query, after, limit, .. } => storage.with_user(|user| {
            read_current(user, frozen)?;
            let body = board_api::catalog(user, &source_workspaces[0], &json!({"kind":kind,"query":query,"after":after,"limit":limit.unwrap_or(20)}))?;
            Ok(AuthorResult::value(json!({"status":"board_catalog","learning_workspace_id":frozen.learning_workspace_id,
                "book_id":source_workspaces[0].book.base.book_id,"published_book_ref":source_workspaces[0].publication.as_ref().map(|p|&p.reference),
                "items":body["items"],"next":body["next"],"total":body["total"],
                "notice":"Original source identities and snapshots only. Use source_read before collecting needed entries through prepare.imports; no learner mastery is inferred."})))
        }),
        AuthorRequest::SourceRead { source, .. } => storage.with_user(|user| {
            read_current(user, frozen)?;
            let mut source: board_api::BoardImportSource = serde_json::from_value(source).map_err(|e| error(e.to_string()))?;
            let source_publication = match &source {
                board_api::BoardImportSource::Note { .. } | board_api::BoardImportSource::PersonalNote { .. } => None,
                board_api::BoardImportSource::Answer { session_id, turn_id } | board_api::BoardImportSource::Presentation { session_id, turn_id, .. } =>
                    crate::workspace_record_context::permitted_turn(user, frozen.learning_workspace_id.as_deref(), &frozen.book_id, session_id, turn_id)?.published_book_ref,
                _ => source_workspaces[0].publication.as_ref().map(|p|p.reference.clone()),
            };
            let item = board_api::import_item(user, &source_workspaces[0], source.clone(), &now)?;
            if let (board_api::BoardImportSource::Presentation { receipt, .. }, BoardTarget::Presentation { receipt: saved, .. }) = (&mut source, &item.target) {
                *receipt = Some(saved.clone());
            }
            Ok(AuthorResult::value(json!({"status":"board_source","source":source,"item":item,
                "published_book_ref":source_publication,"retained":false})))
        }),
        AuthorRequest::Read { collection, offset, length, board_revision, candidate_revision } => storage.with_user(|user| {
            let (store, mut board) = read_current_document(user, frozen)?;
            let changed = edits::refresh(user, frozen, reference, &store, &board, &now)?;
            board_api::resolve_note_items(user,&mut board,true);
            let active_turn = turn(user, reference)?;
            let current = active_turn.domain.board_authoring.as_ref();
            let understanding_items = crate::board_understanding::items(user, frozen, &reference.session_id, &reference.turn_id, &now)?
                .into_iter().map(|item| board.items.iter().find(|old| same_retained_identity(old, &item)).cloned().unwrap_or(item)).collect::<Vec<_>>();
            let understanding_refs = understanding_items.iter().map(|item| BoardContentRef {
                item_id: item.item_id.clone(), block_id: None,
                content_revision: if board.items.iter().any(|old| old.item_id == item.item_id) { board.revision } else { board.revision + 1 },
            }).collect::<Vec<_>>();
            let conditions = crate::board_understanding::formal_condition_refs(&board, &understanding_items);
            let parent = frozen.follow_up.item_id.as_ref().and_then(|id| board.items.iter().find(|item| &item.item_id == id));
            let size = BoardSize { width: 320.0, height: 220.0 };
            let body = json!({"status":"board_read","board":board,
                "reserved_answer_item_id":next_answer_item_id(active_turn),
                "committed":active_turn.domain.board_commits.iter().map(|part|&part.delivery).collect::<Vec<_>>(),
                "answer_position":next_position(&board, parent, &size),"answer_size":size,
                "candidate_revision":current.map(|c| c.candidate_revision).unwrap_or(0),
                "proposed_content_revision":board.revision+1,"understanding_items":understanding_items,"understanding_refs":understanding_refs,"formal_condition_refs":conditions,
                "candidate":current.map(edits::public_candidate),
                "candidate_id":current.map(|candidate|&candidate.candidate_id),
                "candidate_change":if changed {Some(if current.is_some_and(|candidate|candidate.conflict.is_some()) {"pending_changes"} else {"rebased"})} else {None},
                "notice":"Retained source references are historical. Use source.present for new claims. understanding_items are actual current-turn evidence; use the exact understanding_refs for each existing or proposed item and content revision. Keep learner_quote separate from interpretation. Follow next_calls for omitted records; read collection=layout to inspect all retained item positions without long answer text."});
            let mut result = AuthorResult::value(runtime::board_author::read_page(body,collection,offset,length,board_revision,candidate_revision)?);
            result.invalidates_delivery = changed;
            Ok(result)
        }),
        AuthorRequest::Prepare { base_board_revision, blocks, drawings, mut operations, read_item_ids, update_drawings, layout: layout_updates, imports, presentations, links, mappings, routes } => storage.with_user(|user| {
            let (store, board) = read_current(user, frozen)?;
            if board.revision != base_board_revision { return Err(runtime::board_author::error("BOARD_REVISION_CONFLICT", "Read the current board before preparing an update")); }
            if read_item_ids.iter().any(|id| !board.items.iter().any(|item| &item.item_id == id)) {
                return Err(error("read_item_ids must name observed retained items, not proposed or missing topics"));
            }
            let active_turn = turn(user, reference)?;
            if active_turn.domain.board_authoring.as_ref().is_some_and(|candidate|candidate.conflict.is_some()) {
                return Err(runtime::board_author::error("BOARD_EDIT_PENDING_READER", "The conflicting suggestion is retained for the reader. Finish with a short explanation and let the reader choose redo from current content; this run cannot replace the human edit with another candidate."));
            }
            let candidate_revision = active_turn.domain.board_authoring.as_ref().map_or(1, |old| old.candidate_revision + 1);
            let question = active_turn.user.clone();
            if blocks.is_empty() || blocks.iter().any(|block| block.block_id.trim().is_empty() || block.markdown.trim().is_empty()) {
                return Err(error("Provide nonempty explicit explanation blocks"));
            }
            let delivery_text = blocks.iter().map(|block| block.markdown.as_str()).collect::<Vec<_>>().join("\n\n");
            let raw_explanation_blocks = blocks.clone();
            let mut compiled = runtime::orchestrator::compile_presentation_text(&delivery_text, bindings, messages)
                .map_err(|issues| error(format!("Explanation failed the ordinary answer compiler: {}", json!(issues))))?;
            for reference in presentations {
                if !compiled.parts.iter().any(|part|matches!(part,AgentAnswerPart::Presentation{presentation_id,revision}
                    if presentation_id==&reference.presentation_id && revision==&reference.revision)) {
                    compiled.parts.push(AgentAnswerPart::Presentation{presentation_id:reference.presentation_id,revision:reference.revision});
                }
            }
            let explanation_blocks = blocks.into_iter().map(|block| {
                let view = runtime::orchestrator::compile_presentation_text(&block.markdown, bindings, messages)
                    .map_err(|issues| error(format!("Block {} failed the ordinary answer compiler: {}", block.block_id, json!(issues))))?;
                Ok(BoardExplanationBlock { block_id: block.block_id, markdown: readable(&view.parts), answer_parts: view.parts.iter().map(|part| json!(part)).collect() })
            }).collect::<Result<Vec<_>, ToolError>>()?;
            let parent = frozen.follow_up.item_id.as_ref().and_then(|id| board.items.iter().find(|item| &item.item_id == id));
            let item_id = next_answer_item_id(active_turn);
            let size = BoardSize { width: 320.0, height: 220.0 };
            let mut items = vec![BoardItem { item_id: item_id.clone(), reply_to_item_id: frozen.follow_up.item_id.clone(),
                block_id: Some(format!("block-{item_id}")), explanation_blocks,
                title: question.chars().take(100).collect(), readable_content: readable(&compiled.parts),
                position: next_position(&board, parent, &size), size, created_at: now.clone(), source_bindings: note_api::bindings(bindings),
                target: BoardTarget::Answer { session_id: reference.session_id.clone(), turn_id: reference.turn_id.clone(), question,
                    answer_parts: compiled.parts.iter().map(|part| json!(part)).collect() } }];
            items.extend(drawings.into_iter().map(|drawing| BoardItem { item_id: drawing.item_id,
                reply_to_item_id: frozen.follow_up.item_id.clone(), block_id: None, explanation_blocks: vec![],
                title: drawing.title, readable_content: match &drawing.graphic { BoardGraphic::Text { text } | BoardGraphic::Shape { text, .. } => text.clone(), BoardGraphic::Path { .. } => String::new() },
                position: drawing.position, size: drawing.size, created_at: now.clone(), source_bindings: note_api::bindings(bindings),
                target: BoardTarget::Drawing { graphic: drawing.graphic, session_id: reference.session_id.clone(), turn_id: reference.turn_id.clone(), based_on: drawing.based_on } }));
            let mut update_items = Vec::new();
            for drawing in update_drawings {
                let mut item = board.items.iter().find(|item| item.item_id == drawing.item_id).cloned()
                    .ok_or_else(|| error("The drawing selected for update is unavailable"))?;
                let BoardTarget::Drawing { graphic, based_on, .. } = &mut item.target else {
                    return Err(error("Only native drawings can change text or graphics; retained answers and learner quotations are immutable"));
                };
                *graphic = drawing.graphic;
                *based_on = drawing.based_on;
                item.title = drawing.title;
                item.readable_content = match graphic { BoardGraphic::Text { text } | BoardGraphic::Shape { text, .. } => text.clone(), BoardGraphic::Path { .. } => String::new() };
                item.position = drawing.position; item.size = drawing.size;
                if update_items.iter().any(|old: &BoardItem| old.item_id == item.item_id) { return Err(error("Each existing drawing can be updated once per candidate")); }
                update_items.push(item);
            }
            for update in layout_updates {
                if board.items.iter().any(|item| item.item_id == update.item_id && matches!(item.target, BoardTarget::Topic { .. })) {
                    return Err(error("Topic geometry comes from the shared layout. Use set_mind_map for the root anchor or layout mode."));
                }
                if let Some(item) = items.iter_mut().find(|item| item.item_id == update.item_id) {
                    // The reserved answer is part of this candidate, so a failed
                    // preview can move it away from the derived topic layout.
                    item.position = update.position; item.size = update.size;
                } else if let Some(item) = update_items.iter_mut().find(|item| item.item_id == update.item_id) {
                    item.position = update.position; item.size = update.size;
                } else {
                    let mut item = board.items.iter().find(|item| item.item_id == update.item_id).cloned()
                        .ok_or_else(|| error("The layout item is not retained or authored by this prepare. Use the current reserved_answer_item_id to position its answer."))?;
                    item.position = update.position; item.size = update.size; update_items.push(item);
                }
            }
            for (import, workspace) in imports.into_iter().zip(&source_workspaces) {
                let source: board_api::BoardImportSource = serde_json::from_value(import.source).map_err(|e| error(e.to_string()))?;
                let mut retained = board.items.clone(); retained.extend(items.clone());
                let mut imported = match source {
                    board_api::BoardImportSource::Answer { session_id, turn_id } => board_api::answer_with_presentations(user, workspace, &session_id, &turn_id, &retained, &now)?,
                    source => vec![board_api::import_item(user, workspace, source, &now)?],
                };
                let mut source_layout = board.clone(); source_layout.items = retained;
                let mut source_parent = None;
                for (index, mut item) in imported.drain(..).enumerate() {
                    if matches!(&item.target, BoardTarget::Note { .. }) && source_layout.items.iter().any(|old| same_retained_identity(old, &item)) {
                        return Err(error("This note is already on the board. Reuse its existing item identity instead of importing another copy"));
                    }
                    if index == 0 {
                        item.item_id = import.item_id.clone(); item.position = import.position.clone(); item.size = import.size.clone();
                        source_parent = Some(item.clone());
                    } else {
                        if let BoardTarget::Presentation { receipt, .. } = &item.target { item.item_id = presentation_item_id(&import.item_id, &receipt.reference); }
                        item.position = next_position(&source_layout, source_parent.as_ref(), &item.size);
                    }
                    item.reply_to_item_id = frozen.follow_up.item_id.clone();
                    source_layout.items.push(item.clone()); items.push(item);
                }
            }
            let mut layout = board.clone();
            for update in &update_items { *layout.items.iter_mut().find(|item| item.item_id == update.item_id).expect("existing item checked") = update.clone(); }
            layout.items.extend(items.clone());
            for mut item in board_api::retained_presentation_items(user,frozen,&reference.session_id,&reference.turn_id,
                &compiled.parts,&layout.items,&now)? {
                if let BoardTarget::Presentation{receipt,..}=&item.target {
                    item.item_id=presentation_item_id(&item_id,&receipt.reference);
                }
                item.position=next_position(&layout,parent,&item.size);
                layout.items.push(item.clone()); items.push(item);
            }
            for mut item in crate::board_understanding::items(user, frozen, &reference.session_id, &reference.turn_id, &now)? {
                if layout.items.iter().any(|old| same_retained_identity(old, &item)) { continue; }
                item.position = next_position(&layout, parent, &item.size);
                layout.items.push(item.clone()); items.push(item);
            }
            for operation in &mut operations {
                if let BoardEditOperation::CreateMindMap { root: topic, .. } | BoardEditOperation::InsertTopic { topic, .. } = operation {
                    topic.source_bindings = note_api::bindings(bindings);
                    if frozen.requires_conversion_confirmation && topic.based_on.is_empty() {
                        if let Some(item_id) = &frozen.follow_up.item_id {
                            topic.based_on.push(BoardContentRef { item_id:item_id.clone(),block_id:frozen.follow_up.block_id.clone(),content_revision:frozen.follow_up.board_revision });
                        }
                    }
                }
            }
            let mut candidate = Candidate { board_id: board.board_id.clone(), base_board_revision, candidate_revision,
                candidate_id: format!("board:{}:{candidate_revision}", reference.turn_id), items, update_items, links, mappings, routes,
                operations, read_item_ids, edit_base:Some(board.clone()), conflict:None, note_revisions:Default::default(),
                regeneration:frozen.regeneration.clone(),
                requires_confirmation:frozen.requires_conversion_confirmation,confirmed:false,dismissed:false,
                explanation_blocks:raw_explanation_blocks, delivery_text, answer_parts: compiled.parts.iter().map(|part| json!(part)).collect(), previewed: false, staged: false,
                source_bindings:bindings.to_vec(),commit_selection:None,commit_requested_at:None };
            candidate.read_item_ids = edits::dependencies(&board, &candidate);
            candidate.note_revisions = store.note_revisions(&board, &candidate.read_item_ids)?;
            let projected = project(&board, &candidate)?;
            edits::require_scope(&board, &projected, frozen, &candidate)?;
            store.validate_candidate(&projected)?;
            require_declared_anchors(user, &projected)?;
            session_runtime::update_board_authoring(user, reference, &now, candidate.clone())?;
            let mut result = AuthorResult::value(json!({"status":"candidate_prepared","candidate_id":candidate.candidate_id,
                "candidate_revision":candidate_revision,"base_board_revision":base_board_revision,"reserved_answer_item_id":item_id,
                "proposed_content_revision":projected.revision,
                "explanation_refs":candidate.items[0].explanation_blocks.iter().map(|block| BoardContentRef {
                    item_id:item_id.clone(),block_id:Some(block.block_id.clone()),content_revision:projected.revision
                }).collect::<Vec<_>>(),
                "next":"Run preview, inspect its actual image in the next sampling, repair any clipping/overlap, then deliver this exact candidate_revision."}));
            result.invalidates_delivery = true;
            Ok(result)
        }),
        AuthorRequest::Revise { candidate_revision, blocks, drawings, operations } => storage.with_user(|user| {
            let active_turn = turn(user, reference)?;
            if active_turn.domain.board_commits.iter().any(|part| part.candidate_revision == candidate_revision) {
                return Err(runtime::board_author::error("BOARD_CANDIDATE_COMMITTED", "This candidate is already saved. Prepare a new part to extend the board; saved explanations remain immutable."));
            }
            let mut candidate = selected_candidate(user, reference, candidate_revision)?;
            let (store, board) = read_current(user, frozen)?;
            validate_current(&store, &board, &candidate)?;
            if blocks.is_empty() && drawings.is_empty() && operations.is_none() {
                return Err(error("Revise needs explanation blocks, drawings or a replacement semantic operation batch"));
            }
            let mut content_changed = false;
            if let Some(mut operations) = operations {
                for operation in &mut operations {
                    if let BoardEditOperation::CreateMindMap { root: topic, .. } | BoardEditOperation::InsertTopic { topic, .. } = operation {
                        topic.source_bindings = note_api::bindings(&candidate.source_bindings);
                    }
                }
                content_changed |= operations != candidate.operations;
                candidate.operations = operations;
            }
            if !blocks.is_empty() {
                if candidate.explanation_blocks.is_empty() {
                    return Err(error("This older candidate has no original explanation blocks. Prepare its complete explanation and drawings before revising blocks."));
                }
                let mut changed = std::collections::BTreeSet::new();
                for block in blocks {
                    if !changed.insert(block.block_id.clone()) {
                        return Err(error(format!("Explanation block {} can be revised only once per call", block.block_id)));
                    }
                    let original = candidate.explanation_blocks.iter_mut().find(|old| old.block_id == block.block_id)
                        .ok_or_else(|| error(format!("Unknown candidate explanation block_id: {}", block.block_id)))?;
                    if block.markdown.trim().is_empty() {
                        return Err(error(format!("Explanation block {} needs nonempty Markdown", block.block_id)));
                    }
                    content_changed |= original.markdown != block.markdown;
                    *original = block;
                }
                // Old source markers stay bound when a later sampling supplies
                // only newly observed sources. The original blocks are compiled,
                // never the readable text shown by BoardCanvas.
                for binding in bindings {
                    if !candidate.source_bindings.iter().any(|old| old.source_ref_id == binding.source_ref_id) {
                        candidate.source_bindings.push(binding.clone());
                    }
                }
                candidate.delivery_text = candidate.explanation_blocks.iter().map(|block| block.markdown.as_str()).collect::<Vec<_>>().join("\n\n");
                let mut compiled = runtime::orchestrator::compile_presentation_text(&candidate.delivery_text, &candidate.source_bindings, messages)
                    .map_err(|issues| error(format!("Explanation failed the ordinary answer compiler: {}", json!(issues))))?;
                // Explicit presentations are separate retained items and parts,
                // so editing a text block must not silently remove them.
                for part in &candidate.answer_parts {
                    if let Ok(presentation @ AgentAnswerPart::Presentation { .. }) = serde_json::from_value::<AgentAnswerPart>(part.clone()) {
                        if !compiled.parts.contains(&presentation) { compiled.parts.push(presentation); }
                    }
                }
                let explanation_blocks = candidate.explanation_blocks.iter().map(|block| {
                    let view = runtime::orchestrator::compile_presentation_text(&block.markdown, &candidate.source_bindings, messages)
                        .map_err(|issues| error(format!("Block {} failed the ordinary answer compiler: {}", block.block_id, json!(issues))))?;
                    Ok(BoardExplanationBlock { block_id:block.block_id.clone(), markdown:readable(&view.parts), answer_parts:view.parts.iter().map(|part| json!(part)).collect() })
                }).collect::<Result<Vec<_>, ToolError>>()?;
                let answer = candidate.items.first_mut().ok_or_else(|| error("The candidate explanation item is unavailable"))?;
                let BoardTarget::Answer { answer_parts, .. } = &mut answer.target else {
                    return Err(error("The candidate explanation item is unavailable"));
                };
                *answer_parts = compiled.parts.iter().map(|part| json!(part)).collect();
                answer.explanation_blocks = explanation_blocks;
                answer.readable_content = readable(&compiled.parts);
                answer.source_bindings = note_api::bindings(&candidate.source_bindings);
                candidate.answer_parts = answer_parts.clone();
            }
            let mut changed = std::collections::BTreeSet::new();
            for drawing in drawings {
                if !changed.insert(drawing.item_id.clone()) {
                    return Err(error(format!("Drawing {} can be revised only once per call", drawing.item_id)));
                }
                let item = candidate.items.iter_mut().chain(candidate.update_items.iter_mut()).find(|item| item.item_id == drawing.item_id)
                    .ok_or_else(|| error(format!("Unknown candidate drawing item_id: {}. Revise only drawings already present in this candidate or its updates.", drawing.item_id)))?;
                let BoardTarget::Drawing { graphic, based_on, .. } = &mut item.target else {
                    return Err(error(format!("Candidate item {} is not a native drawing; retained answers and learner quotations are immutable", drawing.item_id)));
                };
                content_changed |= *graphic != drawing.graphic || *based_on != drawing.based_on || item.title != drawing.title
                    || item.position != drawing.position || item.size != drawing.size;
                *graphic = drawing.graphic;
                *based_on = drawing.based_on;
                item.title = drawing.title;
                item.readable_content = match graphic { BoardGraphic::Text { text } | BoardGraphic::Shape { text, .. } => text.clone(), BoardGraphic::Path { .. } => String::new() };
                item.position = drawing.position;
                item.size = drawing.size;
            }
            if !content_changed {
                return Err(runtime::board_author::error("BOARD_CANDIDATE_UNCHANGED", "The supplied blocks and drawings are identical to the current candidate. No new revision was created; continue with the current candidate or provide a content change."));
            }
            candidate.candidate_revision += 1;
            candidate.candidate_id = format!("board:{}:{}", reference.turn_id, candidate.candidate_revision);
            candidate.previewed = false;
            candidate.staged = false;
            candidate.commit_selection = None;
            candidate.commit_requested_at = None;
            candidate.conflict = None;
            candidate.read_item_ids = edits::dependencies(&board, &candidate);
            candidate.note_revisions = store.note_revisions(&board, &candidate.read_item_ids)?;
            let projected = project(&board, &candidate)?;
            edits::require_scope(&board, &projected, frozen, &candidate)?;
            store.validate_candidate(&projected)?;
            require_declared_anchors(user, &projected)?;
            session_runtime::update_board_authoring(user, reference, &now, candidate.clone())?;
            let item_id = &candidate.items[0].item_id;
            let mut result = AuthorResult::value(json!({"status":"candidate_revised","candidate_id":candidate.candidate_id,
                "candidate_revision":candidate.candidate_revision,"base_board_revision":candidate.base_board_revision,"reserved_answer_item_id":item_id,
                "proposed_content_revision":projected.revision,
                "explanation_refs":candidate.items[0].explanation_blocks.iter().map(|block| BoardContentRef {
                    item_id:item_id.clone(),block_id:Some(block.block_id.clone()),content_revision:projected.revision
                }).collect::<Vec<_>>(),
                "next":"Preview and inspect this revised candidate, then select its candidate_revision for teaching review before saving it."}));
            result.invalidates_delivery = true;
            Ok(result)
        }),
        AuthorRequest::Preview { candidate_revision, viewport } => {
            let (candidate, projected) = storage.with_user(|user| {
                selected_candidate(user, reference, candidate_revision)?;
                let (store, board) = read_current_document(user, frozen)?;
                edits::refresh(user, frozen, reference, &store, &board, &now)?;
                let candidate = turn(user, reference)?.domain.board_authoring.clone().expect("candidate checked");
                if candidate.conflict.is_some() { return Ok((candidate, None)); }
                let projected = project(&board, &candidate)?;
                Ok((candidate, Some(projected)))
            })?;
            let Some(projected) = projected else {
                let mut result = AuthorResult::value(json!({"status":"candidate_pending_changes","candidate_id":candidate.candidate_id,
                    "candidate_revision":candidate.candidate_revision,"conflict":candidate.conflict,
                    "next":"The reader's current content is preserved. Offer to redo this suggestion from the current selected subtree; do not overwrite it."}));
                result.invalidates_delivery = true;
                return Ok(result);
            };
            let candidate_revision = candidate.candidate_revision;
            // Chromium work must never hold the user/session lock.
            let candidate_item_ids = edits::preview_ids(&projected, &candidate);
            let report = crate::board_preview::preview(&candidate.candidate_id, &projected, &candidate_item_ids, viewport, cancellation)?;
            cancellation.check()?;
            let valid = report.errors.is_empty() && !report.observations.is_empty()
                && report.observations.iter().all(|observation| observation.issues.is_empty()
                    && observation.dom["board"]["ready"] == true);
            let mut result = AuthorResult::value(json!({"status":if valid {"preview_ready_for_inspection"} else {"preview_failed"},
                "candidate_id":candidate.candidate_id,"candidate_revision":candidate_revision,"base_board_revision":candidate.base_board_revision,
                "requires_confirmation":candidate.requires_confirmation,
                "environment":report.environment,"environment_name":report.environment_name,"errors":report.errors,
                "observations":report.observations.iter().map(|observation| json!({"dom":observation.dom,"layout":observation.layout,"issues":observation.issues})).collect::<Vec<_>>() }));
            for observation in &report.observations {
                result.images.push(runtime::presentation_author::PreviewImage { caption: format!("Native board candidate {} / {}", candidate.candidate_revision, report.environment_name),
                    png_base64: observation.screenshot_png_base64.clone(), candidate_id: Some(candidate.candidate_id.clone()), environment_name: Some(report.environment_name.clone()) });
            }
            storage.with_user(|user| {
                let mut current = selected_candidate(user, reference, candidate_revision)?;
                let (store, board) = read_current(user, frozen)?; validate_current(&store, &board, &current)?;
                current.previewed = valid; current.staged = false;
                session_runtime::update_board_authoring(user, reference, &now, current)
            })?;
            if valid { result.previewed_candidate = Some(candidate.candidate_id); }
            result.invalidates_delivery = true;
            Ok(result)
        }
        AuthorRequest::Deliver { candidate_revision } => storage.with_user(|user| {
            if !turn(user,reference)?.domain.board_commits.is_empty() {
                return Err(runtime::board_author::error("BOARD_PART_COMMIT_REQUIRED", "This turn already has committed board parts. Commit this additional complete part, then finish with references to the retained parts."));
            }
            let mut candidate = selected_candidate(user, reference, candidate_revision)?;
            if candidate.requires_confirmation {
                return Err(runtime::board_author::error("BOARD_CONVERSION_COMMIT_REQUIRED", "Use commit after the actual preview and teaching review to offer this conversion for the reader's confirmation. The original board remains intact until they accept."));
            }
            if !candidate.previewed { return Err(runtime::board_author::error("BOARD_PREVIEW_REQUIRED", "The exact candidate requires a successful actual BoardCanvas preview")); }
            let (store, board) = read_current(user, frozen)?; validate_current(&store, &board, &candidate)?;
            candidate.staged = true;
            session_runtime::update_board_authoring(user, reference, &now, candidate.clone())?;
            let mut result = AuthorResult::value(json!({"status":"delivery_staged","candidate_id":candidate.candidate_id,
                "candidate_revision":candidate_revision,"board_id":candidate.board_id,"base_board_revision":candidate.base_board_revision,
                "delivery_text":candidate.delivery_text,"next":"Finish the answer. The exact prepared explanation will pass through the normal answer compiler and commit with the observed drawings only when this turn completes successfully."}));
            result.delivery_text = Some(candidate.delivery_text);
            Ok(result)
        }),
        AuthorRequest::Commit { candidate_revision, selected_delivery } => storage.with_user(|user| {
            if let Some(part)=turn(user,reference)?.domain.board_commits.iter().find(|part|part.candidate_revision==candidate_revision) {
                let mut result=AuthorResult::value(json!({"status":"board_part_committed","candidate_revision":candidate_revision,
                    "candidate_id":part.delivery.commit_id,"delivery":part.delivery,"already_committed":true}));
                result.committed=Some(part.delivery.clone());return Ok(result);
            }
            let mut candidate=selected_candidate(user,reference,candidate_revision)?;
            if !candidate.previewed {return Err(runtime::board_author::error("BOARD_PREVIEW_REQUIRED","The exact board part needs a successful actual preview before commit"));}
            let selection=selected_delivery.ok_or_else(||runtime::board_author::error("BOARD_TEACHING_SELECTION_REQUIRED","Commit requires the runtime's inspected and selected board part"))?;
            let (store,board)=read_current(user,frozen)?;
            validate_current(&store,&board,&candidate)?;
            // Persist the exact selection before the board write. A lost session
            // acknowledgment can then repair this receipt without reinterpreting
            // a newer teaching selection or creating a different board item.
            candidate.commit_selection=Some(selection);
            candidate.commit_requested_at=Some(now.clone());
            session_runtime::update_board_authoring(user,reference,&now,candidate.clone())?;
            if candidate.requires_confirmation && !candidate.confirmed {
                return Ok(AuthorResult::value(json!({"status":"candidate_confirmation_required","candidate_id":candidate.candidate_id,
                    "candidate_revision":candidate.candidate_revision,"board_id":candidate.board_id,
                    "next":"The conversion preview is ready for the reader. Finish with a short notice asking them to inspect and accept it in the board. Do not claim it was saved; the old drawings stay intact."})));
            }
            let delivery=store.append_authored_part_with_edits(&frozen.follow_up,candidate.base_board_revision,&reference.session_id,&reference.turn_id,
                &candidate.candidate_id,candidate.items.clone(),candidate.update_items.clone(),candidate.links.clone(),candidate.mappings.clone(),candidate.routes.clone(),candidate.operations.clone(),&now)?;
            repair_commits(user,reference,&now)?;
            let mut result=AuthorResult::value(json!({"status":"board_part_committed","candidate_id":candidate.candidate_id,
                "candidate_revision":candidate_revision,"delivery":delivery,"already_committed":false,
                "next":"This complete part is now saved. Continue the original request. Read the updated board and new reserved answer item ID before preparing another part; finish by referring to the committed parts."}));
            result.committed=Some(delivery);result.invalidates_delivery=true;Ok(result)
        }),
    }
}

/// Called only after the ordinary final answer has been durably accepted.
pub(crate) fn staged_items(
    candidate: &Candidate,
    final_parts: &[AgentAnswerPart],
) -> Result<Vec<BoardItem>, ToolError> {
    let expected: Vec<AgentAnswerPart> = candidate
        .answer_parts
        .iter()
        .cloned()
        .map(serde_json::from_value)
        .collect::<Result<_, _>>()
        .map_err(|e| error(e.to_string()))?;
    if readable(&expected) != readable(final_parts) {
        return Err(error("The compiled final answer differs from the observed board explanation; no candidate content was published"));
    }
    Ok(candidate.items.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    const NOW: &str = "2026-10-25T10:00:00Z";

    fn setup() -> (
        tempfile::TempDir,
        AppState,
        crate::agent_run::PreparedAgentChat,
        Value,
    ) {
        let (root, mut state, metadata) = crate::tests::board_browser_host::fixture();
        let create = board_api::route(
            &state.user,
            &state.workspace,
            "create",
            &json!({"title":"Native candidate"}),
            NOW,
        )
        .unwrap();
        let board = board_api::route(&state.user, &state.workspace, "import", &json!({"board_id":create["board_id"],"board_revision":create["revision"],"source":metadata["sources"]["prebuild"]}), NOW).unwrap();
        let prepared = prepare_agent_chat(&mut state, &json!({"message":"把所选条件画成板书", "client_request_id":"native-author-test", "session_id":metadata["session_id"],
            "expected_chat_head":metadata["turn_id"], "board_follow_up":{"board_id":board["board_id"],"board_revision":board["revision"],"item_id":board["items"][0]["item_id"]}}).to_string(), NOW)
            .unwrap_or_else(|reply| panic!("{}", reply.body));
        (root, state, prepared, board)
    }

    fn prepared_request(board: &Value, turn: &AgentTurnRef) -> AuthorRequest {
        let revision = board["revision"].as_u64().unwrap();
        serde_json::from_value(json!({"operation":"prepare","base_board_revision":revision,
            "blocks":[{"block_id":"condition","markdown":"先分别看每一段的用时，再把它们相加。"}],
            "drawings":[{"item_id":"drawing-condition","title":"时间相加","position":{"x":900,"y":420},"size":{"width":260,"height":160},
                "graphic":{"kind":"shape","shape":"rectangle","text":"总时间 = 第一段 + 第二段","color":"#177e77"},"based_on":[{"item_id":board["items"][0]["item_id"],"block_id":null,"content_revision":revision}]}],
            "mappings":[{"mapping_id":"condition-map","explanation":{"item_id":reserved_answer_item_id(&turn.turn_id),"block_id":"condition","content_revision":revision+1},"visual":{"kind":"item","item_id":"drawing-condition"}}],
            "routes":[{"route_id":"route-condition","title":"看时间","steps":[{"step_id":"time","explanation":{"item_id":reserved_answer_item_id(&turn.turn_id),"block_id":"condition","content_revision":revision+1},
                "reveal_targets":[{"kind":"item","item_id":"drawing-condition"}],"focus_targets":[{"kind":"item","item_id":"drawing-condition"}],"transition":"draw","duration_ms":800}]}]})).unwrap()
    }

    #[test]
    fn board_source_publication_unscoped_authoring_persists_exact_sources_before_commit() {
        let (_root, mut state, mut prepared, board) = setup();
        assert!(prepared.scope.learning_workspace_id.is_none());
        let publication = base_schema::PublishedBookRef {
            book_id: state.workspace.book.base.book_id.clone(), publication_id: "original-publication".into(),
        };
        let mut document: BoardDocument = serde_json::from_value(board.clone()).unwrap();
        document.publication_id = Some(publication.publication_id.clone());
        let saved = state.user.history_path.as_ref().unwrap().parent().unwrap()
            .join("boards/unscoped").join(&document.board_id).join("revisions")
            .join(format!("{}.json", document.revision));
        std::fs::write(&saved, serde_json::to_vec(&document).unwrap()).unwrap();
        std::fs::write(saved.parent().unwrap().parent().unwrap().join("head.json"),
            serde_json::to_vec(&BoardSummary::from(&document)).unwrap()).unwrap();
        let frozen = prepared.scope.board_context.as_mut().unwrap();
        frozen.publication = Some(publication.clone());
        let mut binding: SourceBinding = serde_json::from_value(board["items"][0]["source_bindings"][0].clone()).unwrap();
        binding.publication_id = None;
        let mut explicit = binding.clone();
        explicit.source_ref_id = "explicit-original".into();
        explicit.publication_id = Some("another-exact-publication".into());
        let mut foreign = binding.clone();
        foreign.source_ref_id = "foreign-original".into();
        foreign.book_id = "another-book".into();
        foreign.publication_id = Some("foreign-publication".into());
        let sources = vec![binding.clone(), explicit.clone(), foreign.clone()];
        state.user.agent_history.sessions.iter_mut().find(|s| s.id == prepared.turn_ref.session_id).unwrap()
            .turns.iter_mut().find(|t| t.turn_id == prepared.turn_ref.turn_id).unwrap()
            .domain.board_context = Some(frozen.clone());
        let mut request = serde_json::to_value(prepared_request(&board, &prepared.turn_ref)).unwrap();
        request["blocks"][0]["markdown"] = json!(format!("原文依据。[[source:{}]]", binding.source_ref_id));
        let port = crate::agent_run::BorrowedAppPort(RefCell::new(&mut state));
        let storage = LocalStorage { port: &port, scope: &prepared.scope };
        let frozen = prepared.scope.board_context.as_ref().unwrap();
        let cancel = CancellationToken::default();
        execute(&storage, frozen, &prepared.turn_ref, serde_json::from_value(request).unwrap(), &sources, &[], &cancel).unwrap();
        storage.with_user(|user| {
            let mut candidate = selected_candidate(user, &prepared.turn_ref, 1)?;
            assert_eq!(candidate.source_bindings[0].publication_id.as_deref(), Some("original-publication"));
            assert_eq!(candidate.source_bindings[1], explicit);
            assert_eq!(candidate.source_bindings[2], foreign);
            assert!(candidate.items.iter().all(|item| item.source_bindings[0].publication_id.as_deref() == Some("original-publication")));
            candidate.previewed = true;
            session_runtime::update_board_authoring(user, &prepared.turn_ref, NOW, candidate)?;
            Ok(())
        }).unwrap();
        execute(&storage, frozen, &prepared.turn_ref, AuthorRequest::Commit {
            candidate_revision: 1,
            selected_delivery: Some(runtime::board_author::SelectedDelivery { teaching_delivery_move_refs: vec![], delivery_scope: None }),
        }, &sources, &[], &cancel).unwrap();
        storage.with_user(|user| {
            let reopened = board_api::scoped_store(user, None)?.read(&document.board_id)?;
            let retained = reopened.items.iter().find(|item| item.item_id == reserved_answer_item_id(&prepared.turn_ref.turn_id)).unwrap();
            assert_eq!(retained.source_bindings[0].publication_id.as_deref(), Some("original-publication"));
            Ok(())
        }).unwrap();
        assert!(sources[0].publication_id.is_none(), "the caller's evidence is unchanged");
    }

    #[test]
    fn repeated_prepare_preserves_exact_presentation_mapping_across_reordering_and_parts() {
        let (_root, mut state, prepared, board) = setup();
        let original = state
            .user
            .agent_history
            .sessions
            .iter()
            .find(|session| session.id == prepared.turn_ref.session_id)
            .unwrap()
            .turns
            .iter()
            .filter_map(|turn| turn.outcome.as_ref())
            .filter_map(|outcome| outcome.answer_view.as_ref())
            .flat_map(|view| &view.parts)
            .find_map(|part| match part {
                AgentAnswerPart::Presentation {
                    presentation_id,
                    revision,
                } => Some(PresentationRef {
                    presentation_id: presentation_id.clone(),
                    revision: *revision,
                }),
                _ => None,
            })
            .unwrap();
        let owner = PresentationOwner {
            book_id: state.workspace.book.base.book_id.clone(),
            session_id: prepared.turn_ref.session_id.clone(),
        };
        let content = presentation_store::PresentationStore::for_user(&state.user)
            .unwrap()
            .read_version(&owner, &original)
            .unwrap()
            .content;
        let mut references = vec![];
        for based_on in [None, None] {
            let candidate = state
                .private_context()
                .create_presentation_candidate(
                    &prepared.turn_ref.session_id,
                    &prepared.turn_ref.turn_id,
                    based_on,
                    content.clone(),
                )
                .unwrap();
            references.push(
                state
                    .private_context()
                    .persist_presentation_candidate(
                        &prepared.turn_ref.session_id,
                        &prepared.turn_ref.turn_id,
                        &candidate.candidate_id,
                    )
                    .unwrap(),
            );
        }
        let candidate = state
            .private_context()
            .create_presentation_candidate(
                &prepared.turn_ref.session_id,
                &prepared.turn_ref.turn_id,
                Some(references[0].clone()),
                content,
            )
            .unwrap();
        let next_version = state
            .private_context()
            .persist_presentation_candidate(
                &prepared.turn_ref.session_id,
                &prepared.turn_ref.turn_id,
                &candidate.candidate_id,
            )
            .unwrap();
        let port = crate::agent_run::BorrowedAppPort(RefCell::new(&mut state));
        let storage = LocalStorage {
            port: &port,
            scope: &prepared.scope,
        };
        let frozen = prepared.scope.board_context.as_ref().unwrap();
        let cancel = CancellationToken::default();
        let request = |revision: Value, presentations: Vec<PresentationRef>, mapping: Value| {
            serde_json::from_value::<AuthorRequest>(json!({
            "operation":"prepare","base_board_revision":revision,"blocks":[{"block_id":"condition","markdown":"对照两个实验现场。"}],
            "drawings":[],"presentations":presentations,"mappings":mapping})).unwrap()
        };
        execute(
            &storage,
            frozen,
            &prepared.turn_ref,
            request(board["revision"].clone(), references.clone(), json!([])),
            &[],
            &[],
            &cancel,
        )
        .unwrap();
        let first = storage
            .with_user(|user| selected_candidate(user, &prepared.turn_ref, 1))
            .unwrap();
        let find_item = |candidate: &Candidate, reference: &PresentationRef| {
            candidate.items.iter().find(|item|matches!(&item.target,
            BoardTarget::Presentation{receipt,..} if receipt.reference.presentation_id==reference.presentation_id && receipt.reference.revision==reference.revision)).unwrap().item_id.clone()
        };
        let first_id = find_item(&first, &references[0]);
        let other_id = find_item(&first, &references[1]);
        let mapping = json!([{"mapping_id":"new-presentation-map","explanation":{"item_id":first.items[0].item_id,
            "block_id":"condition","content_revision":board["revision"].as_u64().unwrap()+1},
            "visual":{"kind":"item","item_id":first_id}}]);
        execute(
            &storage,
            frozen,
            &prepared.turn_ref,
            request(
                board["revision"].clone(),
                vec![references[1].clone(), references[0].clone()],
                mapping.clone(),
            ),
            &[],
            &[],
            &cancel,
        )
        .unwrap();
        let second = storage
            .with_user(|user| selected_candidate(user, &prepared.turn_ref, 2))
            .unwrap();
        assert_eq!(find_item(&second, &references[0]), first_id);
        assert_eq!(find_item(&second, &references[1]), other_id);
        assert_eq!(
            json!(second.mappings),
            mapping,
            "the actual referenced new item survives a new prepare and reordered inputs"
        );
        storage
            .with_user(|user| {
                let mut candidate = selected_candidate(user, &prepared.turn_ref, 2)?;
                candidate.previewed = true;
                session_runtime::update_board_authoring(user, &prepared.turn_ref, NOW, candidate)
            })
            .unwrap();
        execute(
            &storage,
            frozen,
            &prepared.turn_ref,
            AuthorRequest::Commit {
                candidate_revision: 2,
                selected_delivery: Some(runtime::board_author::SelectedDelivery {
                    teaching_delivery_move_refs: vec![],
                    delivery_scope: None,
                }),
            },
            &[],
            &[],
            &cancel,
        )
        .unwrap();
        let current = storage
            .with_user(|user| Ok(read_current(user, frozen)?.1))
            .unwrap();
        let mut next_request = request(
            json!(current.revision),
            vec![next_version.clone()],
            json!([]),
        );
        if let AuthorRequest::Prepare { blocks, .. } = &mut next_request {
            blocks[0].block_id = "next-condition".into();
        }
        execute(
            &storage,
            frozen,
            &prepared.turn_ref,
            next_request,
            &[],
            &[],
            &cancel,
        )
        .unwrap();
        let next = storage
            .with_user(|user| selected_candidate(user, &prepared.turn_ref, 3))
            .unwrap();
        let next_id = find_item(&next, &next_version);
        assert!(next_id.starts_with(&format!(
            "{}-part-2-presentation-",
            reserved_answer_item_id(&prepared.turn_ref.turn_id)
        )));
        assert_ne!(next_id, first_id);
        assert!(
            current.items.iter().any(|item| item.item_id == first_id),
            "the previous committed scene keeps its original identity"
        );
    }

    #[test]
    fn committed_parts_keep_stable_ids_and_repair_a_lost_session_acknowledgment() {
        let (_root, mut state, prepared, board) = setup();
        let port = crate::agent_run::BorrowedAppPort(RefCell::new(&mut state));
        let storage = LocalStorage {
            port: &port,
            scope: &prepared.scope,
        };
        let frozen = prepared.scope.board_context.as_ref().unwrap();
        let cancel = CancellationToken::default();
        let read = || AuthorRequest::Read {
            collection: None,
            offset: 0,
            length: None,
            board_revision: None,
            candidate_revision: None,
        };
        let review = || runtime::board_author::SelectedDelivery {
            teaching_delivery_move_refs: vec![],
            delivery_scope: None,
        };
        execute(
            &storage,
            frozen,
            &prepared.turn_ref,
            prepared_request(&board, &prepared.turn_ref),
            &[],
            &[],
            &cancel,
        )
        .unwrap();
        storage
            .with_user(|user| {
                let mut candidate = selected_candidate(user, &prepared.turn_ref, 1)?;
                candidate.previewed = true;
                session_runtime::update_board_authoring(user, &prepared.turn_ref, NOW, candidate)
            })
            .unwrap();
        let first = execute(
            &storage,
            frozen,
            &prepared.turn_ref,
            AuthorRequest::Commit {
                candidate_revision: 1,
                selected_delivery: Some(review()),
            },
            &[],
            &[],
            &cancel,
        )
        .unwrap();
        assert_eq!(first.body["status"], "board_part_committed");
        assert!(
            first.delivery_text.is_none(),
            "local delivery cannot trigger whole-turn finalization"
        );
        let receipt = first.committed.unwrap();
        let repeated = execute(
            &storage,
            frozen,
            &prepared.turn_ref,
            AuthorRequest::Commit {
                candidate_revision: 1,
                selected_delivery: None,
            },
            &[],
            &[],
            &cancel,
        )
        .unwrap();
        assert_eq!(repeated.committed, Some(receipt.clone()));
        let observed = execute(
            &storage,
            frozen,
            &prepared.turn_ref,
            read(),
            &[],
            &[],
            &cancel,
        )
        .unwrap()
        .body;
        assert_eq!(observed["committed"].as_array().unwrap().len(), 1);
        assert_eq!(
            observed["reserved_answer_item_id"],
            format!(
                "{}-part-2",
                reserved_answer_item_id(&prepared.turn_ref.turn_id)
            )
        );
        let second_id = observed["reserved_answer_item_id"]
            .as_str()
            .unwrap()
            .to_owned();
        execute(&storage,frozen,&prepared.turn_ref,serde_json::from_value(json!({"operation":"prepare",
            "base_board_revision":observed["board"]["revision"],"blocks":[{"block_id":"second-condition","markdown":"现在把第二个条件连接到已经交付的第一部分。"}],"drawings":[]})).unwrap(),&[],&[],&cancel).unwrap();
        // Simulate the supported failure window after the authoritative board
        // write but before the session could acknowledge its committed receipt.
        let second = storage
            .with_user(|user| {
                let mut candidate = selected_candidate(user, &prepared.turn_ref, 2)?;
                assert_eq!(candidate.items[0].item_id, second_id);
                candidate.previewed = true;
                candidate.commit_selection = Some(review());
                candidate.commit_requested_at = Some(NOW.into());
                session_runtime::update_board_authoring(
                    user,
                    &prepared.turn_ref,
                    NOW,
                    candidate.clone(),
                )?;
                let (store, _) = read_current(user, frozen)?;
                let delivery = store.append_authored_part(
                    &frozen.follow_up,
                    candidate.base_board_revision,
                    &prepared.turn_ref.session_id,
                    &prepared.turn_ref.turn_id,
                    &candidate.candidate_id,
                    candidate.items,
                    candidate.update_items,
                    candidate.links,
                    candidate.mappings,
                    candidate.routes,
                    NOW,
                )?;
                assert_eq!(
                    turn(user, &prepared.turn_ref)?.domain.board_commits.len(),
                    1
                );
                Ok(delivery)
            })
            .unwrap();
        let repaired = execute(
            &storage,
            frozen,
            &prepared.turn_ref,
            read(),
            &[],
            &[],
            &cancel,
        )
        .unwrap()
        .body;
        assert_eq!(repaired["committed"], json!([receipt, second]));
        assert_eq!(repaired["candidate_revision"], 2);
        assert_eq!(
            repaired["reserved_answer_item_id"],
            format!(
                "{}-part-3",
                reserved_answer_item_id(&prepared.turn_ref.turn_id)
            )
        );
        let saved = storage
            .with_user(|user| Ok(read_current(user, frozen)?.1))
            .unwrap();
        storage
            .with_user(|user| repair_commits(user, &prepared.turn_ref, "2026-10-25T11:00:00Z"))
            .unwrap();
        assert_eq!(
            storage
                .with_user(|user| Ok(read_current(user, frozen)?.1))
                .unwrap(),
            saved
        );
        assert_eq!(
            execute(
                &storage,
                frozen,
                &prepared.turn_ref,
                AuthorRequest::Deliver {
                    candidate_revision: 2
                },
                &[],
                &[],
                &cancel
            )
            .err()
            .unwrap()
            .error_code,
            "BOARD_PART_COMMIT_REQUIRED"
        );
    }

    #[test]
    fn read_pages_keep_geometry_exact_and_reject_a_changed_formal_board() {
        let (_root, mut state, prepared, board) = setup();
        let port = crate::agent_run::BorrowedAppPort(RefCell::new(&mut state));
        let storage = LocalStorage {
            port: &port,
            scope: &prepared.scope,
        };
        let frozen = prepared.scope.board_context.as_ref().unwrap();
        let request: AuthorRequest =
            serde_json::from_value(json!({"operation":"read","collection":"layout",
            "offset":0,"length":1,"board_revision":board["revision"]}))
            .unwrap();
        let page = execute(
            &storage,
            frozen,
            &prepared.turn_ref,
            request.clone(),
            &[],
            &[],
            &CancellationToken::default(),
        )
        .unwrap();
        assert_eq!(
            page.body["records"][0]["item_id"],
            board["items"][0]["item_id"]
        );
        assert_eq!(
            page.body["records"][0]["position"],
            board["items"][0]["position"]
        );
        assert_eq!(page.body["records"][0]["size"], board["items"][0]["size"]);
        storage
            .with_user(|user| {
                let (store, mut current) = read_current(user, frozen)?;
                let revision = current.revision;
                current.items[0].position.x += 125.5;
                store.update(current, revision, NOW)?;
                Ok(())
            })
            .unwrap();
        let error = execute(
            &storage,
            frozen,
            &prepared.turn_ref,
            request,
            &[],
            &[],
            &CancellationToken::default(),
        )
        .err()
        .unwrap();
        assert_eq!(error.error_code, "BOARD_REVISION_CONFLICT");
    }

    #[test]
    fn candidate_is_private_durable_and_requires_its_exact_preview() {
        let (_root, mut state, prepared, board) = setup();
        let request = prepared_request(&board, &prepared.turn_ref);
        let port = crate::agent_run::BorrowedAppPort(RefCell::new(&mut state));
        let storage = LocalStorage {
            port: &port,
            scope: &prepared.scope,
        };
        let frozen = prepared.scope.board_context.as_ref().unwrap();
        let result = execute(
            &storage,
            frozen,
            &prepared.turn_ref,
            request.clone(),
            &[],
            &[],
            &CancellationToken::default(),
        )
        .unwrap();
        assert_eq!(result.body["candidate_revision"], 1);
        storage
            .with_user(|user| {
                let (_, current) = read_current(user, frozen)?;
                assert_eq!(
                    json!(current),
                    board,
                    "prepare must not change the formal board"
                );
                let reloaded = load_agent_history(&user.history_path)?;
                let saved = reloaded
                    .sessions
                    .iter()
                    .find(|s| s.id == prepared.turn_ref.session_id)
                    .unwrap()
                    .turns
                    .iter()
                    .find(|t| t.turn_id == prepared.turn_ref.turn_id)
                    .unwrap();
                assert_eq!(
                    saved
                        .domain
                        .board_authoring
                        .as_ref()
                        .unwrap()
                        .candidate_revision,
                    1
                );
                Ok(())
            })
            .unwrap();
        assert_eq!(
            execute(
                &storage,
                frozen,
                &prepared.turn_ref,
                AuthorRequest::Deliver {
                    candidate_revision: 1
                },
                &[],
                &[],
                &CancellationToken::default()
            )
            .err()
            .unwrap()
            .error_code,
            "BOARD_PREVIEW_REQUIRED"
        );
        let revised = execute(
            &storage,
            frozen,
            &prepared.turn_ref,
            request,
            &[],
            &[],
            &CancellationToken::default(),
        )
        .unwrap();
        assert_eq!(revised.body["candidate_revision"], 2);
        assert!(execute(
            &storage,
            frozen,
            &prepared.turn_ref,
            AuthorRequest::Preview {
                candidate_revision: 1,
                viewport: None
            },
            &[],
            &[],
            &CancellationToken::default()
        )
        .is_err());
    }

    #[test]
    fn changed_base_and_unbound_source_are_rejected_before_a_candidate_is_saved() {
        let (_root, mut state, prepared, board) = setup();
        let mut request = prepared_request(&board, &prepared.turn_ref);
        if let AuthorRequest::Prepare { blocks, .. } = &mut request {
            blocks[0].markdown = "这是书中的条件。 [[source:unobserved]]".into();
        }
        let port = crate::agent_run::BorrowedAppPort(RefCell::new(&mut state));
        let storage = LocalStorage {
            port: &port,
            scope: &prepared.scope,
        };
        let frozen = prepared.scope.board_context.as_ref().unwrap();
        assert!(execute(
            &storage,
            frozen,
            &prepared.turn_ref,
            request,
            &[],
            &[],
            &CancellationToken::default()
        )
        .is_err());
        let mut request = prepared_request(&board, &prepared.turn_ref);
        if let AuthorRequest::Prepare {
            base_board_revision,
            ..
        } = &mut request
        {
            *base_board_revision -= 1;
        }
        assert_eq!(
            execute(
                &storage,
                frozen,
                &prepared.turn_ref,
                request,
                &[],
                &[],
                &CancellationToken::default()
            )
            .err()
            .unwrap()
            .error_code,
            "BOARD_REVISION_CONFLICT"
        );
        storage
            .with_user(|user| {
                assert!(turn(user, &prepared.turn_ref)?
                    .domain
                    .board_authoring
                    .is_none());
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn decimal_path_survives_private_candidate_and_formal_snapshot_storage() {
        let (_root, mut state, prepared, board) = setup();
        let mut input = serde_json::to_value(prepared_request(&board, &prepared.turn_ref)).unwrap();
        input["drawings"][0]["position"] = json!({"x":900.25,"y":420.5});
        input["drawings"][0]["size"] = json!({"width":260.75,"height":160.125});
        input["drawings"][0]["graphic"] = json!({"kind":"path","points":[{"x":0.5,"y":1.25},{"x":120.75,"y":60.125}],"color":"#177e77","width":2.5});
        let request: AuthorRequest = serde_json::from_str(&input.to_string()).unwrap();
        let port = crate::agent_run::BorrowedAppPort(RefCell::new(&mut state));
        let storage = LocalStorage {
            port: &port,
            scope: &prepared.scope,
        };
        let frozen = prepared.scope.board_context.as_ref().unwrap();
        execute(
            &storage,
            frozen,
            &prepared.turn_ref,
            request,
            &[],
            &[],
            &CancellationToken::default(),
        )
        .unwrap();
        storage
            .with_user(|user| {
                let loaded = load_agent_history(&user.history_path)?;
                let candidate = loaded
                    .sessions
                    .iter()
                    .find(|session| session.id == prepared.turn_ref.session_id)
                    .unwrap()
                    .turns
                    .iter()
                    .find(|turn| turn.turn_id == prepared.turn_ref.turn_id)
                    .unwrap()
                    .domain
                    .board_authoring
                    .as_ref()
                    .unwrap();
                let expected = candidate
                    .items
                    .iter()
                    .find(|item| matches!(item.target, BoardTarget::Drawing { .. }))
                    .unwrap()
                    .clone();
                assert_eq!(
                    expected.position,
                    BoardPosition {
                        x: 900.25,
                        y: 420.5
                    }
                );
                assert_eq!(
                    expected.size,
                    BoardSize {
                        width: 260.75,
                        height: 160.125
                    }
                );
                let BoardTarget::Drawing {
                    graphic: BoardGraphic::Path { points, width, .. },
                    ..
                } = &expected.target
                else {
                    panic!()
                };
                assert_eq!(*width, 2.5);
                assert_eq!(
                    points[1],
                    BoardPoint {
                        x: 120.75,
                        y: 60.125
                    }
                );
                // Exercise the immutable storage representation, independently of
                // the preview and final-delivery gates tested by the browser flow.
                let (boards, _) = read_current(user, frozen)?;
                let receipt = boards.append_authored_delivery(
                    &frozen.follow_up,
                    candidate.base_board_revision,
                    &prepared.turn_ref.session_id,
                    &prepared.turn_ref.turn_id,
                    candidate.items.clone(),
                    candidate.update_items.clone(),
                    candidate.links.clone(),
                    candidate.mappings.clone(),
                    candidate.routes.clone(),
                    NOW,
                )?;
                let persisted =
                    boards.read_revision(&candidate.board_id, receipt.after_revision)?;
                assert_eq!(
                    persisted
                        .items
                        .iter()
                        .find(|item| item.item_id == expected.item_id),
                    Some(&expected)
                );
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn fine_blocks_cannot_survive_a_different_final_answer() {
        let (_root, mut state, prepared, board) = setup();
        let port = crate::agent_run::BorrowedAppPort(RefCell::new(&mut state));
        let storage = LocalStorage {
            port: &port,
            scope: &prepared.scope,
        };
        let frozen = prepared.scope.board_context.as_ref().unwrap();
        execute(
            &storage,
            frozen,
            &prepared.turn_ref,
            prepared_request(&board, &prepared.turn_ref),
            &[],
            &[],
            &CancellationToken::default(),
        )
        .unwrap();
        storage
            .with_user(|user| {
                let candidate = selected_candidate(user, &prepared.turn_ref, 1)?;
                assert!(staged_items(
                    &candidate,
                    &[AgentAnswerPart::Markdown {
                        text: "已被修正为另一个回答".into()
                    }]
                )
                .is_err());
                let parts = candidate
                    .answer_parts
                    .iter()
                    .cloned()
                    .map(serde_json::from_value)
                    .collect::<Result<Vec<_>, _>>()
                    .unwrap();
                assert_eq!(staged_items(&candidate, &parts)?.len(), 2);
                Ok(())
            })
            .unwrap();
    }
}

#[cfg(test)]
#[path = "board_author_learning_tests.rs"]
mod learning_tests;

#[cfg(test)]
#[path = "board_author_revise_tests.rs"]
mod revise_tests;

#[cfg(test)]
#[path = "board_author_edit_tests.rs"]
mod edit_tests;
