//! Execute existing reader commands once, with a durable intent and result.
use crate::session_event::*;
use crate::*;

#[derive(Deserialize)]
pub(crate) struct Request {
    pub session_id: String,
    pub turn_id: String,
    pub effect_id: String,
    pub action: DispositionAction,
}

pub(crate) enum Start {
    Existing(EffectDisposition),
    Execute(AgentTurnRef, EffectRecord),
}

fn failure(code: &str, message: &str) -> ToolError {
    ToolError {
        error_code: code.into(),
        category: "conflict".into(),
        message: message.into(),
    }
}

fn append(
    user: &mut user_runtime::UserRuntime,
    turn: &AgentTurnRef,
    at: &str,
    body: EventBody,
) -> Result<(), ToolError> {
    session_runtime::append(user, turn, at, body)
}

pub(crate) fn start(
    user: &mut user_runtime::UserRuntime,
    workspace: &reader_workspace::ReaderWorkspace,
    input: &Value,
    at: &str,
) -> Result<Start, ToolError> {
    let request: Request = serde_json::from_value(input.clone())
        .map_err(|_| failure("INVALID_EFFECT_REQUEST", "Invalid effect request"))?;
    let store = user.session_store.as_mut().ok_or_else(|| {
        failure(
            "SESSION_EVENT_WRITE_REQUIRED",
            "Effect dispositions require the new session log",
        )
    })?;
    store.settle(&mut user.agent_history, &request.session_id)?;
    let session = user
        .agent_history
        .sessions
        .iter()
        .find(|s| s.id == request.session_id && s.book_id == workspace.book.base.book_id)
        .ok_or_else(authorization::missing)?;
    let turn = session
        .turns
        .iter()
        .find(|t| t.turn_id == request.turn_id)
        .ok_or_else(authorization::missing)?;
    let reference = AgentTurnRef {
        session_id: session.id.clone(),
        turn_id: turn.turn_id.clone(),
        user_turn_ordinal: turn.user_turn_ordinal,
    };
    if turn.status == AgentAssistantStatus::PendingAssistant {
        return Err(failure(
            "CHAT_BUSY",
            "Wait for the run to finish before handling its effects",
        ));
    }
    let record = turn
        .domain
        .effects
        .iter()
        .find(|e| e.effect_id == request.effect_id)
        .cloned();
    let retained = record.as_ref().and_then(|e| e.retained_object_for_undo(&request.action)).map(str::to_owned);
    if let Some(disposition) = record.as_ref().and_then(|e| e.disposition.as_ref()).filter(|_| retained.is_none()) {
        if disposition.started.action != request.action {
            return Err(failure(
                "EFFECT_ALREADY_HANDLED",
                "This effect already has a disposition",
            ));
        }
        if disposition.receipt.is_some() {
            return Ok(Start::Existing(disposition.clone()));
        }
        let mut disposition = disposition.clone();
        // Only a known result object or in-place promotion proves a keep succeeded.
        if let Some(receipt) = reconcile(user, record.as_ref().unwrap(), &session.book_id) {
            append(
                user,
                &reference,
                at,
                EventBody::EffectDisposed(receipt.clone()),
            )?;
            disposition.receipt = Some(receipt);
        }
        return Ok(Start::Existing(disposition));
    }
    let mut effect = match &record {
        Some(EffectRecord {
            effect: DeliveredEffect::Reader { effect },
            ..
        }) => effect.clone(),
        Some(_) => {
            return Err(failure(
                "EFFECT_ACTION_UNSUPPORTED",
                "This delivery has no keep or undo command",
            ))
        }
        None => return Err(authorization::missing()),
    };
    if let Some(retained) = retained {
        if let AgentEffect::Note { mem_id, .. } | AgentEffect::Highlight { mem_id, .. } = &mut effect {
            *mem_id = retained;
        }
    }
    let allowed = matches!(
        (&effect, &request.action),
        (
            AgentEffect::Note { .. } | AgentEffect::Highlight { .. },
            DispositionAction::Keep | DispositionAction::Undo
        ) | (
            AgentEffect::Goto { .. }
                | AgentEffect::Layout { .. }
                | AgentEffect::PaperMinimap { .. },
            DispositionAction::Undo
        ) | (
            AgentEffect::LayoutProposal { .. } | AgentEffect::PaperMinimapProposal { .. },
            DispositionAction::Keep | DispositionAction::Dismiss
        )
    );
    if !allowed {
        return Err(failure(
            "EFFECT_ACTION_UNSUPPORTED",
            "Unsupported effect disposition",
        ));
    }
    if turn.published_book_ref.as_ref() != workspace.publication.as_ref().map(|p| &p.reference) {
        return Err(failure(
            "PUBLICATION_BINDING_MISMATCH",
            "Open the original publication",
        ));
    }
    if !matches!(
        effect,
        AgentEffect::Note { .. } | AgentEffect::Highlight { .. }
    ) {
        let scene =
            turn.domain.scene.as_ref().ok_or_else(|| {
                failure("WORKSPACE_STALE", "Original reading scene is unavailable")
            })?;
        if scene.workspace_id != workspace.id
            || scene.generation != workspace.generation
            || workspace.selected_chat.as_deref() != Some(&request.session_id)
        {
            return Err(failure(
                "WORKSPACE_STALE",
                "The original reading scene has changed",
            ));
        }
    }
    let started = DispositionStarted {
        disposition_id: format!("disposition:{}", uuid::Uuid::now_v7()),
        effect_id: request.effect_id.clone(),
        action: request.action.clone(),
        result_object_id: (matches!(effect, AgentEffect::Highlight { .. })
            && request.action == DispositionAction::Keep)
            .then(|| format!("mem_{}", uuid::Uuid::now_v7())),
    };
    append(
        user,
        &reference,
        at,
        EventBody::DispositionStarted(started.clone()),
    )?;
    Ok(Start::Execute(
        reference,
        EffectRecord {
            effect_id: request.effect_id,
            effect: DeliveredEffect::Reader { effect },
            disposition: Some(EffectDisposition {
                started,
                receipt: None,
            }),
        },
    ))
}

fn object_id(effect: &AgentEffect) -> Option<String> {
    match effect {
        AgentEffect::Note { mem_id, .. } | AgentEffect::Highlight { mem_id, .. } => {
            Some(mem_id.clone())
        }
        AgentEffect::LayoutProposal { proposal } => Some(proposal.proposal_id.clone()),
        AgentEffect::PaperMinimapProposal { proposal } => Some(proposal.proposal_id.clone()),
        AgentEffect::PaperMinimap { effect } => Some(effect.effect_id.clone()),
        AgentEffect::Layout { effect } => Some(format!("layout:{}", effect.after.rev)),
        AgentEffect::Goto { after_anchor, .. } => Some(after_anchor.clone()),
    }
}

fn receipt(record: &EffectRecord, result: Result<Option<String>, ToolError>) -> DispositionReceipt {
    let DeliveredEffect::Reader { effect } = &record.effect else {
        unreachable!()
    };
    let (result_object_id, error) = match result {
        Ok(id) => (id, None),
        Err(e) => (
            None,
            Some(AgentTurnError {
                error_code: e.error_code,
                category: e.category,
                message: e.message,
            }),
        ),
    };
    DispositionReceipt {
        disposition_id: record
            .disposition
            .as_ref()
            .unwrap()
            .started
            .disposition_id
            .clone(),
        original_object_id: object_id(effect),
        result_object_id,
        error,
    }
}

fn reconcile(
    user: &user_runtime::UserRuntime,
    record: &EffectRecord,
    book: &str,
) -> Option<DispositionReceipt> {
    let started = &record.disposition.as_ref()?.started;
    if started.action != DispositionAction::Keep {
        return None;
    }
    let DeliveredEffect::Reader { effect } = &record.effect else {
        return None;
    };
    let id = match effect {
        AgentEffect::Highlight { .. } => started.result_object_id.as_ref()?,
        AgentEffect::Note { mem_id, .. } => mem_id,
        _ => return None,
    };
    user.store
        .recall(&RecallQuery {
            book_id: Some(book.into()),
            layer: Some("long_term".into()),
            ..Default::default()
        })
        .iter()
        .any(|r| r.mem_id == *id)
        .then(|| receipt(record, Ok(Some(id.clone()))))
}

pub(crate) fn execute(
    user: &mut user_runtime::UserRuntime,
    workspace: &mut reader_workspace::ReaderWorkspace,
    record: &EffectRecord,
    at: &str,
) -> DispositionReceipt {
    let started = &record.disposition.as_ref().unwrap().started;
    let DeliveredEffect::Reader { effect } = &record.effect else {
        unreachable!()
    };
    let result = (|| match (effect, &started.action) {
        (AgentEffect::Note { mem_id, .. }, DispositionAction::Keep) => {
            workspace_client::memory(
                user,
                workspace,
                "memory/promote",
                &json!({"mem_id":mem_id}),
                at,
            )?;
            Ok(Some(mem_id.clone()))
        }
        (AgentEffect::Highlight { mem_id, .. }, DispositionAction::Keep) => {
            let original = user
                .store
                .recall(&RecallQuery {
                    book_id: Some(workspace.book.base.book_id.clone()),
                    ..Default::default()
                })
                .into_iter()
                .find(|r| r.mem_id == *mem_id)
                .ok_or_else(authorization::missing)?;
            let saved = user.store.save(
                SaveInput {
                    mem_id: started.result_object_id.clone(),
                    mem_type: original.mem_type,
                    layer: "long_term".into(),
                    book_id: original.book_id,
                    anchor: original.anchor,
                    content: original.content,
                    range: original.range,
                    selection_context: original.selection_context,
                    note_placement: original.note_placement,
                    note: original.note,
                    citations: Some(original.citations),
                    source_session_id: original.source_session_id,
                },
                at,
            )?;
            Ok(Some(saved.mem_id))
        }
        (
            AgentEffect::Note { mem_id, .. } | AgentEffect::Highlight { mem_id, .. },
            DispositionAction::Undo,
        ) => {
            workspace_client::memory(
                user,
                workspace,
                "memory/delete",
                &json!({"mem_id":mem_id}),
                at,
            )?;
            Ok(None)
        }
        (
            AgentEffect::Goto {
                before_anchor,
                after_anchor,
            },
            DispositionAction::Undo,
        ) => {
            if workspace.reader.state().viewport.anchor_lid != *after_anchor {
                return Err(failure("WORKSPACE_STALE", "Reading position has changed"));
            }
            workspace
                .reader
                .goto_lid(&workspace.book, &mut user.store, before_anchor, at)?;
            Ok(Some(workspace.reader.state().viewport.anchor_lid))
        }
        (AgentEffect::Layout { effect }, DispositionAction::Undo) => {
            let applied = workspace.reader.undo_layout_effect(effect)?;
            Ok(Some(format!("layout:{}", applied.after.rev)))
        }
        (AgentEffect::LayoutProposal { proposal }, DispositionAction::Keep) => {
            let applied = workspace.reader.apply_layout_proposal(
                &workspace.book,
                &proposal.proposal_id,
                proposal.base_layout_rev,
            )?;
            Ok(Some(format!("layout:{}", applied.after.rev)))
        }
        (AgentEffect::LayoutProposal { .. }, DispositionAction::Dismiss) => Ok(None),
        (AgentEffect::PaperMinimap { effect }, DispositionAction::Undo) => {
            let rev = workspace.reader.paper_minimap_state().rev;
            let applied =
                workspace
                    .reader
                    .undo_paper_minimap_effect_by_id(&effect.effect_id, rev, at)?;
            Ok(Some(applied.effect_id))
        }
        (AgentEffect::PaperMinimapProposal { proposal }, DispositionAction::Keep) => {
            let applied = workspace.reader.apply_paper_minimap_proposal(
                &workspace.book,
                &proposal.proposal_id,
                &proposal.base_map_rev,
                proposal.base_state_rev,
                at,
            )?;
            Ok(Some(applied.effect_id))
        }
        (AgentEffect::PaperMinimapProposal { proposal }, DispositionAction::Dismiss) => {
            workspace.reader.dismiss_paper_minimap_proposal(
                &proposal.proposal_id,
                &proposal.base_map_rev,
                proposal.base_state_rev,
            )?;
            Ok(None)
        }
        _ => Err(failure(
            "EFFECT_ACTION_UNSUPPORTED",
            "Unsupported effect disposition",
        )),
    })();
    receipt(record, result)
}

pub(crate) fn complete(
    user: &mut user_runtime::UserRuntime,
    reference: &AgentTurnRef,
    record: &EffectRecord,
    receipt: DispositionReceipt,
    at: &str,
) -> Result<EffectDisposition, ToolError> {
    append(
        user,
        reference,
        at,
        EventBody::EffectDisposed(receipt.clone()),
    )?;
    Ok(EffectDisposition {
        started: record.disposition.as_ref().unwrap().started.clone(),
        receipt: Some(receipt),
    })
}

pub(crate) fn route(
    state: &mut AppState,
    input: &Value,
    at: &str,
) -> Result<EffectDisposition, ToolError> {
    match start(&mut state.user, &state.workspace, input, at)? {
        Start::Existing(disposition) => Ok(disposition),
        Start::Execute(reference, record) => {
            let before = state.workspace.reader.clone();
            let receipt = execute(&mut state.user, &mut state.workspace, &record, at);
            let overlay = state
                .workspace
                .reader
                .paper_minimap_state()
                .saved_user_overlay;
            if overlay != before.paper_minimap_state().saved_user_overlay {
                if let Some(path) = paper_minimap_overlay_path(&state.workspace.session_path) {
                    if let Err(error) = save_saved_paper_minimap_overlay(&path, &overlay) {
                        state.workspace.reader = before;
                        return Err(error);
                    }
                }
            }
            if matches!(
                record.effect,
                DeliveredEffect::Reader {
                    effect: AgentEffect::Goto { .. }
                }
            ) && receipt.error.is_none()
            {
                if let Some(path) = &state.workspace.session_path {
                    let key = session_dir_key(&path_string(&state.workspace.book_dir));
                    let mut session =
                        load_session(&state.workspace.session_path).unwrap_or_else(|| {
                            SessionState {
                                current_book_dir: key.clone(),
                                books: BTreeMap::new(),
                            }
                        });
                    session.current_book_dir = key.clone();
                    session.books.insert(
                        key,
                        SessionBookProgress {
                            top_lid: state.workspace.reader.viewport().top_lid,
                        },
                    );
                    let bytes = serde_json::to_vec(&session)
                        .map_err(|e| agent_history_internal(e.to_string()))?;
                    if let Err(e) = std::fs::write(path, bytes) {
                        state.workspace.reader = before;
                        return Err(agent_history_internal(e.to_string()));
                    }
                }
            }
            complete(&mut state.user, &reference, &record, receipt, at)
        }
    }
}
