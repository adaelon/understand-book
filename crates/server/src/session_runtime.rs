//! Durable boundaries for a Resident run; all writes use its original chat.
use crate::session_event::{EventBody, MessageRevision, TurnFinished};
use crate::*;

pub(crate) fn append(
    user: &mut user_runtime::UserRuntime,
    reference: &AgentTurnRef,
    at: &str,
    body: EventBody,
) -> Result<(), ToolError> {
    #[cfg(test)]
    if matches!(&body, EventBody::TurnFinished(_)) {
        kill_point("before_terminal");
    }
    #[cfg(test)]
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
    #[cfg(test)]
    kill_point(point);
    Ok(())
}

#[cfg(test)]
fn kill_point(point: &str) {
    if !point.is_empty() && std::env::var("JL4_KILL_POINT").as_deref() == Ok(point) {
        crate::tests::mu10_tests::kill_barrier();
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
    let at = multi_user_host::now().to_string();
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
        &multi_user_host::now().to_string(),
        EventBody::CheckpointInstalled {
            checkpoint: checkpoint.clone(),
        },
    )
}

pub(crate) fn close_interrupted(messages: &mut Vec<Message>) {
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
            &multi_user_host::now().to_string(),
            Some(&turn),
            EventBody::TurnFinished(body),
        )?;
    }
    Ok(())
}

pub(crate) fn interrupted_summary(
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

fn deliver(user: &mut user_runtime::UserRuntime, reference: &AgentTurnRef, id: &str,
    effect: crate::session_event::DeliveredEffect) -> Result<(), ToolError> {
    if current_turn(user, reference)?.domain.effects.iter().any(|e| e.effect_id == id
        && serde_json::to_value(&e.effect).ok() == serde_json::to_value(&effect).ok()) { return Ok(()); }
    append(user, reference, &multi_user_host::now().to_string(), EventBody::EffectDelivered { effect_id: id.into(), effect })
}

pub(crate) fn deliver_effects(user: &mut user_runtime::UserRuntime, reference: &AgentTurnRef,
    effects: &[AgentEffect]) -> Result<(), ToolError> {
    if user.session_store.is_none() { return Ok(()); }
    user.session_store.as_mut().unwrap().settle(&mut user.agent_history, &reference.session_id)?;
    link_teaching(user, reference, &multi_user_host::now().to_string())?;
    for effect in effects {
        deliver(user, reference, &runtime::orchestrator::effect_id(effect),
            crate::session_event::DeliveredEffect::Reader { effect: effect.clone() })?;
    }
    Ok(())
}

/// Read original receipts only. Neither current Tutor control nor a new learning judgment is involved.
pub(crate) fn link_teaching(user: &mut user_runtime::UserRuntime, reference: &AgentTurnRef, at: &str) -> Result<(), ToolError> {
    if user.session_store.is_none() { return Ok(()); }
    user.session_store.as_mut().unwrap().settle(&mut user.agent_history, &reference.session_id)?;
    let learning = user.learning_store()?;
    let mut sessions = BTreeSet::new();
    for prefix in ["binding", "response"] {
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
