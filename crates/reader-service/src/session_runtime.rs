//! Durable boundaries for a Resident run; all writes use its original chat.
use crate::session_event::{EventBody, MessageRevision, TurnFinished};
use crate::*;

#[cfg(test)]
#[path = "tests/board_commit_session_tests.rs"]
mod board_commit_tests;

pub fn append(
    user: &mut user_runtime::UserRuntime,
    reference: &AgentTurnRef,
    at: &str,
    body: EventBody,
) -> Result<(), ToolError> {
    #[cfg(any(test, feature = "test-support"))]
    if matches!(&body, EventBody::TurnFinished(_)) {
        kill_point("before_terminal");
    }
    #[cfg(any(test, feature = "test-support"))]
    let point = match &body {
        EventBody::ActivityRecorded { activity }
            if activity.kind == "tool"
                && activity.status == runtime::run_events::ActivityStatus::Running =>
        {
            "tool_start"
        }
        EventBody::MessageAppended { messages }
            if messages.iter().any(|m| m.role == runtime::Role::Tool) =>
        {
            "tool_result"
        }
        EventBody::CheckpointInstalled { .. } => "checkpoint",
        _ => "",
    };
    user.session_store.as_mut().unwrap().append(
        &mut user.agent_history,
        &reference.session_id,
        at,
        Some(&reference.turn_id),
        body,
    )?;
    #[cfg(any(test, feature = "test-support"))]
    kill_point(point);
    Ok(())
}

#[cfg(any(test, feature = "test-support"))]
fn kill_point(point: &str) {
    if !point.is_empty() && std::env::var("JL4_KILL_POINT").as_deref() == Ok(point) {
        crate::tests::kill_barrier();
    }
}

pub(crate) fn revision(before: &[Message], after: &[Message]) -> Option<MessageRevision> {
    let from = before.iter().zip(after).take_while(|(a, b)| a == b).count();
    (from != before.len() || from != after.len()).then(|| MessageRevision {
        from,
        suffix: after[from..].to_vec(),
    })
}

pub(crate) fn finish(
    user: &mut user_runtime::UserRuntime,
    reference: &AgentTurnRef,
    session: &AgentChatSession,
    at: &str,
) -> Result<(), ToolError> {
    user.session_store
        .as_mut()
        .unwrap()
        .settle(&mut user.agent_history, &reference.session_id)?;
    if current_turn(user, reference)?.status != AgentAssistantStatus::PendingAssistant { return Ok(()); }
    let turn = session.turns.iter().find(|t| t.turn_id == reference.turn_id).unwrap();
    let effects = turn.run_summary.as_ref().map(|s| s.effects.as_slice())
        .or_else(|| turn.outcome.as_ref().map(|o| o.effects.as_slice())).unwrap_or(&[]);
    deliver_effects(user, reference, effects)?;
    if !turn.source_bindings.is_empty() && current_turn(user, reference)?.source_bindings != turn.source_bindings {
        append(user, reference, at, EventBody::SourcesBound { bindings: turn.source_bindings.clone() })?;
    }
    if let Some(view) = turn.outcome.as_ref().and_then(|o| o.answer_view.as_ref()) {
        for part in &view.parts {
            if let AgentAnswerPart::Presentation { presentation_id, revision } = part {
                deliver(user, reference, &format!("presentation:{presentation_id}:{revision}"),
                    crate::session_event::DeliveredEffect::Presentation { reference: runtime::presentation::PresentationRef {
                        presentation_id: presentation_id.clone(), revision: *revision } })?;
            }
        }
    }
    let before = user.session_store.as_ref().unwrap().logs[&reference.session_id]
        .projection
        .session
        .as_ref()
        .unwrap();
    if before.turns.iter().any(|t| {
        t.turn_id == reference.turn_id && t.status != AgentAssistantStatus::PendingAssistant
    }) {
        return Ok(());
    }
    let turn = session
        .turns
        .iter()
        .find(|t| t.turn_id == reference.turn_id)
        .unwrap();
    let mut messages = session.messages.clone();
    clean(&mut messages);
    let body = TurnFinished {
        status: turn.status,
        outcome: turn.outcome.clone(),
        error: turn.error.clone(),
        run_summary: turn.run_summary.clone(),
        source_bindings: turn.source_bindings.clone(),
        delivery_diagnostics: turn.delivery_diagnostics.clone(),
        goal_ref: turn.goal_ref.clone(),
        goals: session_store::changed_goals(before, &session.goals),
        history_revision: revision(&before.messages, &messages),
    };
    append(user, reference, at, EventBody::TurnFinished(body))
}

pub(crate) fn clean(messages: &mut [Message]) {
    runtime::presentation_author::redact_history(messages);
    runtime::tool_exposure::redact_history(messages);
}

/// Commit the accepted model surface separately from the redacted business
/// transcript. A terminal turn then commits against this same history position.
pub fn model_projection(
    user: &mut user_runtime::UserRuntime,
    reference: &AgentTurnRef,
    messages: &[Message],
    projection: Option<&runtime::active_projection::ActiveModelProjection>,
) -> Result<(), ToolError> {
    if user.session_store.is_none() { return Ok(()); }
    progress(user, reference, messages, &[])?;
    let current = &user.session_store.as_ref().unwrap().logs[&reference.session_id]
        .projection.active_model_projection;
    if current.as_ref() == projection { return Ok(()); }
    let input_from = current.as_ref().zip(projection)
        .filter(|(old, new)| old.provider_identity == new.provider_identity
            && old.runtime_profile == new.runtime_profile && old.instructions == new.instructions)
        .map_or(0, |(old, new)| old.input.iter().zip(&new.input).take_while(|(a, b)| a == b).count());
    let projection = projection.cloned().map(|mut projection| {
        projection.input.drain(..input_from);
        projection
    });
    append(user, reference, &service_error::now().to_string(),
        EventBody::ModelProjectionUpdated { projection, input_from })
}

pub(crate) fn progress(
    user: &mut user_runtime::UserRuntime,
    reference: &AgentTurnRef,
    messages: &[Message],
    activities: &[runtime::run_events::RunActivity],
) -> Result<(), ToolError> {
    if user.session_store.is_none() {
        return Ok(());
    }
    user.session_store
        .as_mut()
        .unwrap()
        .settle(&mut user.agent_history, &reference.session_id)?;
    let at = service_error::now().to_string();
    let mut messages = messages.to_vec();
    clean(&mut messages);
    let log = &user.session_store.as_ref().unwrap().logs[&reference.session_id];
    let session = log.projection.session.as_ref().unwrap();
    if !session.turns.iter().any(|t| {
        t.turn_id == reference.turn_id && t.status == AgentAssistantStatus::PendingAssistant
    }) {
        return Err(agent_history_internal("run is no longer pending"));
    }
    if let Some(r) = revision(&session.messages, &messages) {
        let body = if r.from == session.messages.len() {
            EventBody::MessageAppended { messages: r.suffix }
        } else {
            EventBody::HistoryRevised(r)
        };
        append(user, reference, &at, body)?;
    }
    for activity in activities {
        let log = &user.session_store.as_ref().unwrap().logs[&reference.session_id];
        let last = log
            .projection
            .facts
            .iter()
            .rev()
            .find_map(|e| match &e.body {
                EventBody::ActivityRecorded { activity: old }
                    if e.turn_id.as_deref() == Some(&reference.turn_id)
                        && old.step_id == activity.step_id =>
                {
                    Some(old)
                }
                _ => None,
            });
        if last != Some(activity) {
            append(
                user,
                reference,
                &at,
                EventBody::ActivityRecorded {
                    activity: activity.clone(),
                },
            )?;
        }
    }
    Ok(())
}

pub(crate) fn checkpoint(
    user: &mut user_runtime::UserRuntime,
    reference: &AgentTurnRef,
    checkpoint: &CompactionCheckpoint,
    messages: &[Message],
) -> Result<(), ToolError> {
    // The checkpoint must describe the persistable prefix, including redaction.
    let mut clean_messages = messages.to_vec();
    clean(&mut clean_messages);
    validate_persisted_checkpoint(&clean_messages, checkpoint)
        .map_err(|e| agent_history_internal(e.message))?;
    progress(user, reference, &clean_messages, &[])?;
    append(
        user,
        reference,
        &service_error::now().to_string(),
        EventBody::CheckpointInstalled {
            checkpoint: checkpoint.clone(),
        },
    )
}

pub fn close_interrupted(messages: &mut Vec<Message>) {
    let mut pending = Vec::new();
    for message in messages.iter() {
        for call in &message.tool_calls {
            pending.push(call.id.clone());
        }
        if let Some(id) = &message.tool_call_id {
            pending.retain(|call| call != id);
        }
    }
    for id in pending {
        messages.push(Message { role: runtime::Role::Tool, content: Some(json!({"error_code":"INTERRUPTED","message":"Execution stopped before a result was saved"}).to_string()),
            tool_call_id: Some(id), tool_calls: vec![], provider_continuation: None });
    }
}

pub(crate) fn recover(
    store: &mut session_store::SessionStore,
    history: &mut AgentHistory,
    admitted: &BTreeSet<String>,
) -> Result<(), ToolError> {
    let pending = history
        .sessions
        .iter()
        .flat_map(|s| {
            s.turns
                .iter()
                .filter(|t| {
                    t.status == AgentAssistantStatus::PendingAssistant
                        && !admitted.contains(&t.turn_id)
                })
                .map(|t| (s.id.clone(), t.turn_id.clone()))
        })
        .collect::<Vec<_>>();
    for (id, turn) in pending {
        let session = history.sessions.iter().find(|s| s.id == id).unwrap();
        let current = session.turns.iter().find(|t| t.turn_id == turn).unwrap();
        let mut messages = session.messages.clone();
        close_interrupted(&mut messages);
        let body = TurnFinished {
            status: AgentAssistantStatus::Failed,
            outcome: None,
            error: Some(AgentTurnError {
                error_code: "INTERRUPTED".into(),
                category: "interrupted".into(),
                message: "The previous Reader process stopped before this run finished".into(),
            }),
            run_summary: interrupted_summary(&store.logs[&id], &turn),
            source_bindings: current.source_bindings.clone(),
            delivery_diagnostics: None,
            goal_ref: current.goal_ref.clone(),
            goals: vec![],
            history_revision: revision(&session.messages, &messages),
        };
        store.append(
            history,
            &id,
            &service_error::now().to_string(),
            Some(&turn),
            EventBody::TurnFinished(body),
        )?;
    }
    Ok(())
}

pub fn interrupted_summary(
    log: &session_log::SessionLog,
    turn: &str,
) -> Option<agent_run::AgentRunSummary> {
    let mut activities = BTreeMap::new();
    for e in &log.projection.facts {
        if e.turn_id.as_deref() == Some(turn) {
            if let EventBody::ActivityRecorded { activity } = &e.body {
                let mut activity = activity.clone();
                if activity.status == runtime::run_events::ActivityStatus::Running {
                    activity.status = runtime::run_events::ActivityStatus::Failed;
                    activity.error_code = Some("INTERRUPTED".into());
                }
                activities.insert(activity.step_id, activity);
            }
        }
    }
    Some(agent_run::AgentRunSummary {
        usage: None,
        effects: vec![],
        trace: vec![],
        activities: Some(activities.into_values().collect()),
        last_seq: None,
    })
}

fn current_turn<'a>(user: &'a user_runtime::UserRuntime, reference: &AgentTurnRef) -> Result<&'a AgentChatTurn, ToolError> {
    user.agent_history.sessions.iter().find(|s| s.id == reference.session_id)
        .and_then(|s| s.turns.iter().find(|t| t.turn_id == reference.turn_id))
        .ok_or_else(|| agent_history_internal("original turn missing"))
}

pub(crate) fn link_board_delivery(
    user: &mut user_runtime::UserRuntime,
    reference: &AgentTurnRef,
    at: &str,
    link: crate::session_event::BoardDelivered,
) -> Result<(), ToolError> {
    if let Some(store) = user.session_store.as_mut() {
        store.settle(&mut user.agent_history, &reference.session_id)?;
    }
    let turn = current_turn(user, reference)?;
    if turn.board_delivery.is_some()
        || (link.delivery.is_none() && turn.board_delivery_error == link.error)
    { return Ok(()); }
    if user.session_store.is_some() {
        return append(user, reference, at, EventBody::BoardDelivered(link));
    }
    let mut candidate = user.agent_history.clone();
    let session = candidate.sessions.iter_mut().find(|session| session.id == reference.session_id)
        .ok_or_else(|| agent_history_internal("original session missing"))?;
    let turn = session.turns.iter_mut().find(|turn| turn.turn_id == reference.turn_id)
        .ok_or_else(|| agent_history_internal("original turn missing"))?;
    turn.board_delivery = link.delivery;
    turn.board_delivery_error = link.error;
    session.updated_at = at.into();
    save_agent_history_path(&user.history_path, &candidate)?;
    user.agent_history = candidate;
    Ok(())
}

pub(crate) fn update_board_authoring(
    user: &mut user_runtime::UserRuntime, reference: &AgentTurnRef, at: &str,
    authoring: crate::board_author::Candidate,
) -> Result<(), ToolError> {
    if user.session_store.is_some() {
        return append(user, reference, at, EventBody::BoardAuthoringUpdated(authoring));
    }
    let current = current_turn(user, reference)?;
    if current.status != AgentAssistantStatus::PendingAssistant
        && !crate::board_author::confirmation_update(current.domain.board_authoring.as_ref(), &authoring) {
        return Err(agent_history_internal("board authoring requires a pending turn"));
    }
    let mut candidate = user.agent_history.clone();
    let session = candidate.sessions.iter_mut().find(|s| s.id == reference.session_id)
        .ok_or_else(|| agent_history_internal("original session missing"))?;
    let turn = session.turns.iter_mut().find(|t| t.turn_id == reference.turn_id)
        .ok_or_else(|| agent_history_internal("original turn missing"))?;
    turn.domain.board_authoring = Some(authoring);
    session.updated_at = at.into();
    save_agent_history_path(&user.history_path, &candidate)?;
    user.agent_history = candidate;
    Ok(())
}

/// A local unit is already in the board store. Its transcript receipt survives
/// any later run termination and is replayed without creating another unit.
pub(crate) fn validate_board_commit(
    session_id: &str, turn: &AgentChatTurn, part: &crate::board_author::CommittedPart,
) -> Result<(), ToolError> {
    let Some(context) = &turn.domain.board_context else {
        return Err(agent_history_internal("board commit requires its accepted target"));
    };
    let delivery = &part.delivery;
    if delivery.commit_id.as_deref().is_none_or(str::is_empty)
        || delivery.board_id != context.follow_up.board_id
        || delivery.parent_item_id != context.follow_up.item_id
        || delivery.origin_session_id != session_id || delivery.origin_turn_id != turn.turn_id
        || part.candidate_revision == 0 || part.committed_at.is_empty() {
        return Err(agent_history_internal("board commit does not belong to its accepted target and turn"));
    }
    if let Some(old) = turn.domain.board_commits.iter().find(|old| old.delivery.commit_id == delivery.commit_id) {
        if serde_json::to_value(old).map_err(|e| agent_history_internal(e.to_string()))?
            != serde_json::to_value(part).map_err(|e| agent_history_internal(e.to_string()))? {
            return Err(agent_history_internal("a board commit cannot be replaced with a different payload"));
        }
    }
    Ok(())
}

pub(crate) fn record_board_commit(
    user: &mut user_runtime::UserRuntime, reference: &AgentTurnRef,
    part: &crate::board_author::CommittedPart,
) -> Result<(), ToolError> {
    if let Some(store) = user.session_store.as_mut() {
        store.settle(&mut user.agent_history, &reference.session_id)?;
    }
    let turn = current_turn(user, reference)?;
    validate_board_commit(&reference.session_id, turn, part)?;
    if turn.domain.board_commits.iter().any(|old| old.delivery.commit_id == part.delivery.commit_id) { return Ok(()); }
    if user.session_store.is_some() {
        return append(user, reference, &part.committed_at, EventBody::BoardCommitted(part.clone()));
    }
    let mut candidate = user.agent_history.clone();
    let session = candidate.sessions.iter_mut().find(|session| session.id == reference.session_id)
        .ok_or_else(|| agent_history_internal("original session missing"))?;
    let turn = session.turns.iter_mut().find(|turn| turn.turn_id == reference.turn_id)
        .ok_or_else(|| agent_history_internal("original turn missing"))?;
    turn.domain.board_commits.push(part.clone());
    session.updated_at = part.committed_at.clone();
    save_agent_history_path(&user.history_path, &candidate)?;
    user.agent_history = candidate;
    Ok(())
}

pub fn board_commits(turn: &AgentChatTurn) -> Vec<memory::boards::BoardDelivery> {
    turn.domain.board_commits.iter().map(|part| part.delivery.clone()).collect()
}

/// Permit exact already-committed presentation versions before the whole run
/// finishes. The immutable effect identity makes commit repair idempotent.
pub(crate) fn deliver_presentations(
    user: &mut user_runtime::UserRuntime, reference: &AgentTurnRef,
    references: &[runtime::presentation::PresentationRef], now: &str,
) -> Result<(), ToolError> {
    if let Some(store) = user.session_store.as_mut() {
        store.settle(&mut user.agent_history, &reference.session_id)?;
        for presentation in references {
            deliver(user, reference, &format!("presentation:{}:{}", presentation.presentation_id, presentation.revision),
                crate::session_event::DeliveredEffect::Presentation { reference: presentation.clone() })?;
        }
        return Ok(());
    }
    let mut candidate = user.agent_history.clone();
    let session = candidate.sessions.iter_mut().find(|session| session.id == reference.session_id)
        .ok_or_else(|| agent_history_internal("original session missing"))?;
    let turn = session.turns.iter_mut().find(|turn| turn.turn_id == reference.turn_id)
        .ok_or_else(|| agent_history_internal("original turn missing"))?;
    for presentation in references {
        let effect_id = format!("presentation:{}:{}", presentation.presentation_id, presentation.revision);
        if !turn.domain.effects.iter().any(|effect| effect.effect_id == effect_id) {
            turn.domain.effects.push(crate::session_event::EffectRecord { effect_id,
                effect: crate::session_event::DeliveredEffect::Presentation { reference: presentation.clone() }, disposition: None });
        }
    }
    session.updated_at = now.into();
    save_agent_history_path(&user.history_path, &candidate)?;
    user.agent_history = candidate;
    Ok(())
}

fn deliver(user: &mut user_runtime::UserRuntime, reference: &AgentTurnRef, id: &str,
    effect: crate::session_event::DeliveredEffect) -> Result<(), ToolError> {
    if current_turn(user, reference)?.domain.effects.iter().any(|e| e.effect_id == id
        && serde_json::to_value(&e.effect).ok() == serde_json::to_value(&effect).ok()) { return Ok(()); }
    append(user, reference, &service_error::now().to_string(), EventBody::EffectDelivered { effect_id: id.into(), effect })
}

pub(crate) fn deliver_effects(user: &mut user_runtime::UserRuntime, reference: &AgentTurnRef,
    effects: &[AgentEffect]) -> Result<(), ToolError> {
    if user.session_store.is_none() { return Ok(()); }
    user.session_store.as_mut().unwrap().settle(&mut user.agent_history, &reference.session_id)?;
    link_teaching(user, reference, &service_error::now().to_string())?;
    for effect in effects {
        deliver(user, reference, &runtime::orchestrator::effect_id(effect),
            crate::session_event::DeliveredEffect::Reader { effect: effect.clone() })?;
    }
    Ok(())
}

/// Read original receipts only. Neither current Tutor control nor a new learning judgment is involved.
pub fn link_teaching(user: &mut user_runtime::UserRuntime, reference: &AgentTurnRef, at: &str) -> Result<(), ToolError> {
    if user.session_store.is_none() { return Ok(()); }
    user.session_store.as_mut().unwrap().settle(&mut user.agent_history, &reference.session_id)?;
    let learning_workspace_id = user.agent_history.sessions.iter().find(|s| s.id == reference.session_id)
        .and_then(|s| s.learning_workspace_id.as_deref());
    let learning = user.learning_store_for_workspace(learning_workspace_id)?;
    let mut sessions = BTreeSet::new();
    for source in &current_turn(user, reference)?.source_bindings {
        if source.publication_id.is_some() { sessions.insert(format!("observation:{}",source.book_id)); }
    }
    for prefix in ["binding", "response", "chosen"] {
        match learning.teaching_event(&format!("{prefix}:{}", reference.turn_id)) {
            Ok(event) => { sessions.insert(event.binding.tutor_session_id); }
            Err(e) if e.error_code == "TEACHING_INVALID" => {},
            Err(e) => return Err(e),
        }
    }
    let mut links: BTreeMap<(String, u64), Vec<String>> = BTreeMap::new();
    for session in sessions {
        for event in learning.teaching_turn_events(&session, &reference.turn_id)? {
            if event.binding.chat_session_id == reference.session_id {
                links.entry((session.clone(), u64::from(event.binding.session_revision))).or_default().push(event.event_id);
            }
        }
    }
    drop(learning);
    for ((session_id, revision), receipts) in links {
        let existing = &current_turn(user, reference)?.domain.teaching;
        let receipt_ids: Vec<_> = receipts.into_iter().filter(|id| !existing.iter().any(|l|
            l.session_id == session_id && l.revision == revision && l.receipt_ids.contains(id))).collect();
        if !receipt_ids.is_empty() {
            append(user, reference, at, EventBody::TeachingLinked(crate::session_event::TeachingLink { session_id, revision, receipt_ids }))?;
        }
    }
    Ok(())
}
