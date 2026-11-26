use super::*;

#[derive(Debug, Default)]
pub struct SessionProjection {
    pub active_model_projection: Option<runtime::active_projection::ActiveModelProjection>,
    pub through_seq: u64,
    pub session: Option<AgentChatSession>,
    pub inputs: BTreeMap<String, run_input::FrozenTurn<HistoryPosition>>,
    /// Supplemental facts keep their event and turn identities for recap consumers.
    pub facts: Vec<SessionEvent>,
    pub accepted_seq: BTreeMap<String, u64>,
    pub turn_seq: BTreeMap<String, u64>,
    pub turn_started_at: BTreeMap<String, String>,
    pub turn_updated_at: BTreeMap<String, String>,
    pub goal_seq: BTreeMap<String, u64>,
}

fn invalid(message: &str) -> ToolError {
    agent_history_internal(message)
}

impl SessionProjection {
    pub fn validate(&self, event: &SessionEvent) -> Result<(), ToolError> {
        if event.version != VERSION || event.seq != self.through_seq + 1 {
            return Err(invalid(
                "unsupported session event version or non-contiguous sequence",
            ));
        }
        let baseline = matches!(
            event.body,
            EventBody::SessionCreated(_)
        );
        if baseline != self.session.is_none() || (baseline && event.turn_id.is_some()) {
            return Err(invalid("session log must start with exactly one baseline"));
        }
        match &event.body {
            EventBody::SessionCreated(c) => {
                if c.session_id.is_empty() || (c.book_id.is_empty() && c.learning_workspace_id.is_some()) {
                    return Err(invalid("empty session identity"));
                }
            }
            EventBody::TurnAccepted(a) => {
                let s = self.session.as_ref().unwrap();
                validate_agent_turn(&a.turn)?;
                if a.turn.admission_input.is_some()
                    || a.turn.status != AgentAssistantStatus::PendingAssistant
                    || event.turn_id.as_deref() != Some(&a.turn.turn_id)
                    || a.history_through_seq != self.through_seq
                    || s.turns.iter().any(|t| {
                        t.turn_id == a.turn.turn_id
                            || t.status == AgentAssistantStatus::PendingAssistant
                    })
                    || s.turns
                        .last()
                        .is_some_and(|t| t.user_turn_ordinal >= a.turn.user_turn_ordinal)
                {
                    return Err(invalid("invalid accepted turn or frozen history position"));
                }
                if let Some(input) = &a.input {
                    if input.messages
                        != (HistoryPosition::Committed {
                            history_through_seq: a.history_through_seq,
                        })
                    {
                        return Err(invalid(
                            "accepted input must reference its committed history position",
                        ));
                    }
                }
                validate_goals(s, &a.goals, a.turn.goal_ref.as_ref())?;
            }
            EventBody::ExplanationRetried(attempt) => {
                let turn = self.turn(event)?;
                if self.session.as_ref().unwrap().turns.last().map(|head| &head.turn_id) != Some(&turn.turn_id)
                    || !crate::interaction_input::explanation::can_retry(turn, attempt) {
                    return Err(invalid("explanation retry must reopen the same failed expression at the chat head"));
                }
            }
            EventBody::TurnFinished(f) => {
                let mut turn = self.turn(event)?.clone();
                if turn.status != AgentAssistantStatus::PendingAssistant
                    || f.status == AgentAssistantStatus::PendingAssistant
                {
                    return Err(invalid(
                        "finished event requires a pending turn and a terminal result",
                    ));
                }
                finish(&mut turn, f);
                validate_agent_turn(&turn)?;
                validate_goals(
                    self.session.as_ref().unwrap(),
                    &f.goals,
                    f.goal_ref.as_ref(),
                )?;
                if let Some(revision) = &f.history_revision {
                    self.validate_revision(revision)?;
                }
            }
            EventBody::BoardAuthoringUpdated(candidate) => {
                let turn = self.turn(event)?;
                if (turn.status != AgentAssistantStatus::PendingAssistant
                    && !crate::board_author::confirmation_update(turn.domain.board_authoring.as_ref(), candidate))
                    || turn.domain.board_context.as_ref().is_none_or(|context| context.follow_up.board_id != candidate.board_id)
                    || turn.domain.board_authoring.as_ref().is_some_and(|old| old.candidate_revision > candidate.candidate_revision)
                { return Err(invalid("board candidate requires its pending accepted turn")); }
            }
            EventBody::BoardDelivered(link) => {
                let turn = self.turn(event)?;
                let Some(context) = turn.domain.board_context.as_ref() else {
                    return Err(invalid("board delivery requires its accepted target"));
                };
                // Validate the recorded receipt without applying today's
                // eligibility rule to answers saved by an earlier release.
                if !crate::board_delivery::has_completed_answer(turn)
                    || turn.board_delivery.is_some()
                    || link.delivery.is_some() == link.error.is_some()
                {
                    return Err(invalid("board delivery requires an unlinked successful answer"));
                }
                if let Some(delivery) = &link.delivery {
                    if delivery.board_id != context.follow_up.board_id
                        || delivery.parent_item_id != context.follow_up.item_id
                        || delivery.origin_turn_id != turn.turn_id
                        || delivery.origin_session_id != self.session.as_ref().unwrap().id
                    {
                        return Err(invalid("board receipt does not belong to the accepted target and turn"));
                    }
                }
            }
            EventBody::BoardCommitted(part) => {
                crate::session_runtime::validate_board_commit(
                    &self.session.as_ref().unwrap().id, self.turn(event)?, part,
                )?;
            }
            EventBody::HistoryRevised(revision) => self.validate_revision(revision)?,
            EventBody::CheckpointInstalled { checkpoint } => {
                validate_persisted_checkpoint(&self.session.as_ref().unwrap().messages, checkpoint)
                    .map_err(|e| invalid(&e.message))?;
            }
            EventBody::ModelProjectionUpdated { projection, input_from } => {
                if self.turn(event)?.status != AgentAssistantStatus::PendingAssistant {
                    return Err(invalid("model projection requires a pending turn"));
                }
                if let Some(projection) = projection {
                    if *input_from > 0 && self.active_model_projection.as_ref().is_none_or(|previous|
                        previous.input.len() < *input_from
                            || previous.provider_identity != projection.provider_identity
                            || previous.runtime_profile != projection.runtime_profile
                            || previous.instructions != projection.instructions)
                    {
                        return Err(invalid("model projection revision has no matching retained prefix"));
                    }
                    let session = self.session.as_ref().unwrap();
                    if projection.history_message_count != session.messages.len()
                        || projection.checkpoint_window_id.as_deref()
                            != session.compaction_checkpoint.as_ref().map(|checkpoint| checkpoint.window_id.as_str())
                    {
                        return Err(invalid("model projection does not describe the committed history window"));
                    }
                }
            }
            EventBody::TurnPrepared { .. } => {
                self.turn(event)?;
                if !self.inputs.contains_key(event.turn_id.as_ref().unwrap()) && self.turn(event)?.domain.local_action.is_none() {
                    return Err(invalid("prepared turn has no frozen input"));
                }
            }
            EventBody::SourcesBound { bindings } => {
                let mut turn = self.turn(event)?.clone();
                turn.source_bindings = bindings.clone();
                validate_agent_turn(&turn)?;
                if bindings
                    .iter()
                    .any(|b| b.book_id != turn.published_book_ref.as_ref().map(|p|p.book_id.as_str()).unwrap_or(&self.session.as_ref().unwrap().book_id)
                        && (b.publication_id.as_deref().is_none_or(str::is_empty)
                            || b.source_revision.as_deref().is_none_or(str::is_empty)))
                {
                    return Err(invalid("source belongs to another book"));
                }
            }
            EventBody::GoalUpdated { goal } => validate_goals(
                self.session.as_ref().unwrap(),
                std::slice::from_ref(goal),
                None,
            )?,
            EventBody::WorkspaceGoalUpdated { goal } => {
                let turn = self.turn(event)?;
                if self.session.as_ref().unwrap().learning_workspace_id.is_none()
                    || goal.text.trim().is_empty() || goal.revision == 0
                    || turn.domain.workspace_goal_update.as_ref().is_some_and(|old| old.revision >= goal.revision) {
                    return Err(invalid("invalid accepted workspace goal revision"));
                }
            }
            EventBody::EffectDelivered { effect_id, .. } => {
                let turn = self.turn(event)?;
                if effect_id.is_empty() || turn.domain.effects.iter().any(|e| e.effect_id == *effect_id && e.disposition.is_some()) {
                    return Err(invalid("cannot replace a disposed effect"));
                }
            }
            EventBody::DispositionStarted(started) => {
                let turn = self.turn(event)?;
                if !turn.domain.effects.iter().any(|e| e.effect_id == started.effect_id &&
                    (e.disposition.is_none() || e.retained_object_for_undo(&started.action).is_some())) {
                    return Err(invalid("effect is absent or already has a disposition"));
                }
            }
            EventBody::EffectDisposed(receipt) => {
                if !self.turn(event)?.domain.effects.iter().any(|e| e.disposition.as_ref().is_some_and(|d|
                    d.started.disposition_id == receipt.disposition_id && d.receipt.is_none())) {
                    return Err(invalid("disposition is absent or already completed"));
                }
            }
            EventBody::SessionUpdated { .. } => {}
            _ => {
                self.turn(event)?;
            }
        }
        if let EventBody::TurnFinished(f) = &event.body {
            let turn = self.turn(event)?;
            if f.source_bindings
                .iter()
                .any(|b| b.book_id != turn.published_book_ref.as_ref().map(|p|p.book_id.as_str()).unwrap_or(&self.session.as_ref().unwrap().book_id)
                    && (b.publication_id.as_deref().is_none_or(str::is_empty)
                        || b.source_revision.as_deref().is_none_or(str::is_empty)))
            {
                return Err(invalid("source belongs to another book"));
            }
        }
        Ok(())
    }

    fn turn(&self, e: &SessionEvent) -> Result<&AgentChatTurn, ToolError> {
        self.session
            .as_ref()
            .and_then(|s| {
                s.turns
                    .iter()
                    .find(|t| Some(&t.turn_id) == e.turn_id.as_ref())
            })
            .ok_or_else(|| invalid("event references an unknown turn"))
    }

    fn validate_revision(&self, r: &MessageRevision) -> Result<(), ToolError> {
        if r.from > self.session.as_ref().unwrap().messages.len() {
            return Err(invalid("history revision starts beyond committed messages"));
        }
        Ok(())
    }

    /// Call only after validation and durable append. No I/O, provider or domain writes.
    pub fn apply(&mut self, event: SessionEvent) {
        self.through_seq = event.seq;
        match &event.body {
            EventBody::TurnAccepted(a) => {
                self.accepted_seq.insert(a.turn.turn_id.clone(), event.seq);
                self.turn_seq.insert(a.turn.turn_id.clone(), event.seq);
                self.turn_started_at.insert(a.turn.turn_id.clone(), event.at.clone());
                self.turn_updated_at.insert(a.turn.turn_id.clone(), event.at.clone());
                for goal in &a.goals { self.goal_seq.insert(goal.id.clone(), event.seq); }
            }
            EventBody::TurnFinished(f) => {
                self.turn_seq.insert(event.turn_id.as_ref().unwrap().clone(), event.seq);
                self.turn_updated_at.insert(event.turn_id.as_ref().unwrap().clone(), event.at.clone());
                for goal in &f.goals { self.goal_seq.insert(goal.id.clone(), event.seq); }
            }
            EventBody::GoalUpdated { goal } => { self.goal_seq.insert(goal.id.clone(), event.seq); }
            _ => {}
        }
        match &event.body {
            EventBody::SessionCreated(c) => {
                self.session = Some(AgentChatSession {
                    follow_up_origin: c.follow_up_origin.clone(),
                    id: c.session_id.clone(),
                    learning_workspace_id: c.learning_workspace_id.clone(),
                    book_id: c.book_id.clone(),
                    title: c.title.clone(),
                    created_at: c.created_at.clone(),
                    updated_at: event.at.clone(),
                    messages: c.messages.clone(),
                    turns: vec![],
                    goals: vec![],
                    compaction_checkpoint: None,
                })
            }
            _ => {
                let session = self.session.as_mut().unwrap();
                session.updated_at = event.at.clone();
                match &event.body {
                    EventBody::SessionUpdated { title } => session.title = title.clone(),
                    EventBody::TurnAccepted(a) => {
                        let mut turn = a.turn.clone();
                        if turn.domain.interaction.as_ref().is_some_and(|action| action.course.is_some()) {
                            turn.domain.course_retry_context = a.input.as_ref()
                                .and_then(|input| input.normalized.get("course_context")).cloned();
                        }
                        session.turns.push(turn);
                        session.title = a.title.clone();
                        upsert_goals(session, &a.goals);
                        if let Some(input) = &a.input {
                            self.inputs.insert(a.turn.turn_id.clone(), input.clone());
                        }
                    }
                    EventBody::MessageAppended { messages } => {
                        session.messages.extend(messages.iter().cloned())
                    }
                    EventBody::ExplanationRetried(attempt) => {
                        self.active_model_projection = None;
                        crate::interaction_input::explanation::reset_failed(session, attempt.clone());
                    }
                    EventBody::HistoryRevised(r) => {
                        if self.active_model_projection.as_ref().is_some_and(|projection|
                            r.from < projection.history_message_count) {
                            self.active_model_projection = None;
                        }
                        revise(session, r);
                    }
                    EventBody::CheckpointInstalled { checkpoint } => {
                        session.compaction_checkpoint = Some(checkpoint.clone());
                        self.active_model_projection = None;
                    }
                    EventBody::ModelProjectionUpdated { projection, input_from } => {
                        self.active_model_projection = projection.as_ref().map(|projection| {
                            let mut restored = projection.clone();
                            if *input_from > 0 {
                                let mut input = self.active_model_projection.as_ref().unwrap().input[..*input_from].to_vec();
                                input.extend(restored.input);
                                restored.input = input;
                            }
                            restored
                        });
                    }
                    EventBody::GoalUpdated { goal } => {
                        upsert_goals(session, std::slice::from_ref(goal))
                    }
                    EventBody::WorkspaceGoalUpdated { goal } => {
                        session.turns.iter_mut().find(|turn| Some(&turn.turn_id) == event.turn_id.as_ref()).unwrap()
                            .domain.workspace_goal_update = Some(goal.clone());
                        self.facts.push(event.clone());
                    }
                    EventBody::TurnFinished(f) => {
                        finish(
                            session
                                .turns
                                .iter_mut()
                                .find(|t| Some(&t.turn_id) == event.turn_id.as_ref())
                                .unwrap(),
                            f,
                        );
                        upsert_goals(session, &f.goals);
                        if let Some(r) = &f.history_revision {
                            if self.active_model_projection.as_ref().is_some_and(|projection|
                                r.from < projection.history_message_count) {
                                self.active_model_projection = None;
                            }
                            revise(session, r);
                        }
                        // Interrupted/failed execution has no accepted sampled
                        // continuation. Its receipts remain in business history.
                        if f.status == AgentAssistantStatus::Failed
                            && !f.error.as_ref().is_some_and(|error| error.category == "model_spend")
                        {
                            self.active_model_projection = None;
                        }
                    }
                    EventBody::TurnPrepared { teaching } => {
                        if let Some(input) = self.inputs.get_mut(event.turn_id.as_ref().unwrap()) {
                            input.set_prepared(teaching.clone());
                        } else {
                            session.turns.iter_mut().find(|t| Some(&t.turn_id) == event.turn_id.as_ref()).unwrap()
                                .domain.local_preparation = Some(teaching.clone());
                        }
                    }
                    EventBody::SourcesBound { bindings } => {
                        session
                            .turns
                            .iter_mut()
                            .find(|t| Some(&t.turn_id) == event.turn_id.as_ref())
                            .unwrap()
                            .source_bindings = bindings.clone();
                        self.facts.push(event.clone());
                    }
                    EventBody::TeachingLinked(link) => {
                        let turn = session.turns.iter_mut().find(|t| Some(&t.turn_id) == event.turn_id.as_ref()).unwrap();
                        turn.domain.teaching.push(link.clone());
                        self.facts.push(event.clone());
                    }
                    EventBody::BoardAuthoringUpdated(candidate) => {
                        session.turns.iter_mut().find(|t| Some(&t.turn_id) == event.turn_id.as_ref()).unwrap()
                            .domain.board_authoring = Some(candidate.clone());
                    }
                    EventBody::BoardDelivered(link) => {
                        let turn = session.turns.iter_mut().find(|t| Some(&t.turn_id) == event.turn_id.as_ref()).unwrap();
                        turn.board_delivery = link.delivery.clone();
                        turn.board_delivery_error = link.error.clone();
                        self.facts.push(event.clone());
                    }
                    EventBody::BoardCommitted(part) => {
                        let turn = session.turns.iter_mut().find(|t| Some(&t.turn_id) == event.turn_id.as_ref()).unwrap();
                        if !turn.domain.board_commits.iter().any(|old| old.delivery.commit_id == part.delivery.commit_id) {
                            turn.domain.board_commits.push(part.clone());
                        }
                        self.facts.push(event.clone());
                    }
                    EventBody::EffectDelivered { effect_id, effect } => {
                        let turn = session.turns.iter_mut().find(|t| Some(&t.turn_id) == event.turn_id.as_ref()).unwrap();
                        if let Some(old) = turn.domain.effects.iter_mut().find(|e| e.effect_id == *effect_id) {
                            old.effect = effect.clone();
                        } else {
                            turn.domain.effects.push(EffectRecord { effect_id: effect_id.clone(), effect: effect.clone(), disposition: None });
                        }
                        self.facts.push(event.clone());
                    }
                    EventBody::DispositionStarted(started) => {
                        let turn = session.turns.iter_mut().find(|t| Some(&t.turn_id) == event.turn_id.as_ref()).unwrap();
                        turn.domain.effects.iter_mut().find(|e| e.effect_id == started.effect_id).unwrap().disposition =
                            Some(EffectDisposition { started: started.clone(), receipt: None });
                        self.facts.push(event.clone());
                    }
                    EventBody::EffectDisposed(receipt) => {
                        let turn = session.turns.iter_mut().find(|t| Some(&t.turn_id) == event.turn_id.as_ref()).unwrap();
                        let disposition = turn.domain.effects.iter_mut().filter_map(|e| e.disposition.as_mut())
                            .find(|d| d.started.disposition_id == receipt.disposition_id).unwrap();
                        disposition.receipt = Some(receipt.clone());
                        self.facts.push(event.clone());
                    }
                    EventBody::ActivityRecorded { .. } => self.facts.push(event.clone()),
                    _ => unreachable!(),
                }
            }
        }
    }

    pub fn fold(&mut self, event: SessionEvent) -> Result<(), ToolError> {
        self.validate(&event)?;
        self.apply(event);
        Ok(())
    }
}

fn validate_goals(
    s: &AgentChatSession,
    goals: &[runtime::goal::ResidentGoal],
    reference: Option<&AgentGoalRef>,
) -> Result<(), ToolError> {
    let mut ids = HashSet::new();
    for goal in goals {
        if !ids.insert(&goal.id)
            || goal.id.is_empty()
            || goal.revision == 0
            || goal.origin_turn_id.is_empty()
            || s.goals
                .iter()
                .any(|g| g.id == goal.id && g.revision > goal.revision)
        {
            return Err(invalid("invalid goal identity or revision"));
        }
    }
    if reference.is_some_and(|r| {
        !goals
            .iter()
            .chain(s.goals.iter())
            .any(|g| g.id == r.id && g.revision >= r.revision)
    }) {
        return Err(invalid("turn references an unknown goal revision"));
    }
    Ok(())
}

fn upsert_goals(s: &mut AgentChatSession, goals: &[runtime::goal::ResidentGoal]) {
    for goal in goals {
        if let Some(current) = s.goals.iter_mut().find(|g| g.id == goal.id) {
            *current = goal.clone();
        } else {
            s.goals.push(goal.clone());
        }
    }
}

fn revise(s: &mut AgentChatSession, r: &MessageRevision) {
    if s.messages[r.from..] != r.suffix && !r.suffix.starts_with(&s.messages[r.from..]) {
        s.compaction_checkpoint = None;
    }
    s.messages.truncate(r.from);
    s.messages.extend(r.suffix.iter().cloned());
}

fn finish(t: &mut AgentChatTurn, f: &TurnFinished) {
    t.status = f.status;
    t.outcome = f.outcome.clone();
    t.error = f.error.clone();
    t.run_summary = f.run_summary.clone();
    t.source_bindings = f.source_bindings.clone();
    t.delivery_diagnostics = f.delivery_diagnostics.clone();
    t.goal_ref = f.goal_ref.clone();
}
