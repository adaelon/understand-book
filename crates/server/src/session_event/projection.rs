use super::*;

#[derive(Debug, Default)]
pub(crate) struct SessionProjection {
    pub through_seq: u64,
    pub session: Option<AgentChatSession>,
    pub inputs: BTreeMap<String, run_admission::FrozenTurn<HistoryPosition>>,
    /// Supplemental facts keep their event and turn identities for recap consumers.
    pub facts: Vec<SessionEvent>,
    pub accepted_seq: BTreeMap<String, u64>,
    pub turn_seq: BTreeMap<String, u64>,
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
                if c.session_id.is_empty() || c.book_id.is_empty() {
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
            EventBody::HistoryRevised(revision) => self.validate_revision(revision)?,
            EventBody::CheckpointInstalled { checkpoint } => {
                validate_persisted_checkpoint(&self.session.as_ref().unwrap().messages, checkpoint)
                    .map_err(|e| invalid(&e.message))?;
            }
            EventBody::TurnPrepared { .. } => {
                self.turn(event)?;
                if !self.inputs.contains_key(event.turn_id.as_ref().unwrap()) {
                    return Err(invalid("prepared turn has no frozen input"));
                }
            }
            EventBody::SourcesBound { bindings } => {
                let mut turn = self.turn(event)?.clone();
                turn.source_bindings = bindings.clone();
                validate_agent_turn(&turn)?;
                if bindings
                    .iter()
                    .any(|b| b.book_id != self.session.as_ref().unwrap().book_id)
                {
                    return Err(invalid("source belongs to another book"));
                }
            }
            EventBody::GoalUpdated { goal } => validate_goals(
                self.session.as_ref().unwrap(),
                std::slice::from_ref(goal),
                None,
            )?,
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
            if f.source_bindings
                .iter()
                .any(|b| b.book_id != self.session.as_ref().unwrap().book_id)
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
                for goal in &a.goals { self.goal_seq.insert(goal.id.clone(), event.seq); }
            }
            EventBody::TurnFinished(f) => {
                self.turn_seq.insert(event.turn_id.as_ref().unwrap().clone(), event.seq);
                for goal in &f.goals { self.goal_seq.insert(goal.id.clone(), event.seq); }
            }
            EventBody::GoalUpdated { goal } => { self.goal_seq.insert(goal.id.clone(), event.seq); }
            _ => {}
        }
        match &event.body {
            EventBody::SessionCreated(c) => {
                self.session = Some(AgentChatSession {
                    id: c.session_id.clone(),
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
                        session.turns.push(a.turn.clone());
                        session.title = a.title.clone();
                        upsert_goals(session, &a.goals);
                        if let Some(input) = &a.input {
                            self.inputs.insert(a.turn.turn_id.clone(), input.clone());
                        }
                    }
                    EventBody::MessageAppended { messages } => {
                        session.messages.extend(messages.iter().cloned())
                    }
                    EventBody::HistoryRevised(r) => revise(session, r),
                    EventBody::CheckpointInstalled { checkpoint } => {
                        session.compaction_checkpoint = Some(checkpoint.clone())
                    }
                    EventBody::GoalUpdated { goal } => {
                        upsert_goals(session, std::slice::from_ref(goal))
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
                            revise(session, r);
                        }
                    }
                    EventBody::TurnPrepared { teaching } => {
                        let input = self
                            .inputs
                            .get_mut(event.turn_id.as_ref().unwrap())
                            .unwrap();
                        input.set_prepared(teaching.clone());
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
