use super::*;

pub(crate) fn legacy_session() -> AgentChatSession {
    let history = serde_json::from_str(include_str!(
        "../../tests/fixtures/agent-history-pre-m.json"
    ))
    .unwrap();
    migrate_agent_history(history).unwrap().sessions.remove(0)
}

/// Build test facts through the same typed events accepted by the writer.
pub(crate) fn sample_events(session: &AgentChatSession) -> Vec<SessionEvent> {
    let mut events = vec![];
    let mut push = |turn: Option<&str>, body| {
        events.push(SessionEvent::new(events.len() as u64 + 1, &session.updated_at, turn, body));
    };
    push(None, EventBody::SessionCreated(SessionCreated {
        session_id: session.id.clone(), book_id: session.book_id.clone(),
        title: session.title.clone(), created_at: session.created_at.clone(), messages: vec![],
    }));
    for goal in &session.goals { push(None, EventBody::GoalUpdated { goal: goal.clone() }); }
    drop(push);
    for original in &session.turns {
        let mut turn = original.clone();
        turn.status = AgentAssistantStatus::PendingAssistant;
        turn.outcome = None; turn.error = None; turn.run_summary = None;
        let input = turn.admission_input.take().map(|f| f.map_messages(|_| HistoryPosition::Committed {
            history_through_seq: events.len() as u64,
        }));
        events.push(SessionEvent::new(events.len() as u64 + 1, &session.updated_at, Some(&turn.turn_id),
            EventBody::TurnAccepted(TurnAccepted { turn: turn.clone(), history_through_seq: events.len() as u64,
                input, goals: vec![], title: session.title.clone() })));
        if original.status != AgentAssistantStatus::PendingAssistant {
            events.push(SessionEvent::new(events.len() as u64 + 1, &session.updated_at, Some(&turn.turn_id),
                EventBody::TurnFinished(TurnFinished { status: original.status, outcome: original.outcome.clone(),
                    error: original.error.clone(), run_summary: original.run_summary.clone(),
                    source_bindings: original.source_bindings.clone(), delivery_diagnostics: original.delivery_diagnostics.clone(),
                    goal_ref: original.goal_ref.clone(), goals: vec![], history_revision: None })));
        }
    }
    events.push(SessionEvent::new(events.len() as u64 + 1, &session.updated_at, None,
        EventBody::HistoryRevised(MessageRevision { from: 0, suffix: session.messages.clone() })));
    if let Some(checkpoint) = &session.compaction_checkpoint {
        events.push(SessionEvent::new(events.len() as u64 + 1, &session.updated_at, None,
            EventBody::CheckpointInstalled { checkpoint: checkpoint.clone() }));
    }
    events
}

pub(crate) fn assert_event_roundtrip(session: &AgentChatSession) {
    let mut projection = SessionProjection::default();
    for event in sample_events(session) {
        let bytes = serde_json::to_vec(&event).unwrap();
        projection.fold(serde_json::from_slice(&bytes).unwrap()).unwrap();
    }
    assert_eq!(serde_json::to_value(projection.session.unwrap()).unwrap(), serde_json::to_value(session).unwrap());
}

pub(crate) fn assert_checkpoint_log_contract(session: &AgentChatSession) {
    let checkpoint = session.compaction_checkpoint.clone().unwrap();
    let mut baseline = session.clone();
    baseline.compaction_checkpoint = None;
    if baseline.turns.is_empty() {
        append_pending_agent_turn(
            &mut baseline,
            "current raw user text".into(),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            "accepted",
        )
        .unwrap();
    }
    let id = baseline.turns.last().unwrap().turn_id.clone();
    let dir = tempfile::tempdir().unwrap();
    let mut log =
        crate::session_log::SessionLog::open(dir.path().join("checkpoint.jsonl")).unwrap();
    for event in sample_events(&baseline) { log.append(event).unwrap(); }
    log.append(log.next_event(
        "checkpoint",
        None,
        EventBody::CheckpointInstalled {
            checkpoint: checkpoint.clone(),
        },
    ))
    .unwrap();
    let checkpoint_seq = log.projection.through_seq;
    let expected = log
        .frozen_messages(&HistoryPosition::Committed {
            history_through_seq: checkpoint_seq,
        })
        .unwrap();
    log.append(log.next_event(
        "append",
        Some(&id),
        EventBody::MessageAppended {
            messages: vec![Message::user("next question")],
        },
    ))
    .unwrap();
    assert_eq!(
        log.projection
            .session
            .as_ref()
            .unwrap()
            .compaction_checkpoint
            .as_ref(),
        Some(&checkpoint)
    );
    log.append(log.next_event(
        "revise",
        Some(&id),
        EventBody::HistoryRevised(MessageRevision {
            from: 1,
            suffix: vec![Message::user("failed question")],
        }),
    ))
    .unwrap();
    assert!(log
        .projection
        .session
        .as_ref()
        .unwrap()
        .compaction_checkpoint
        .is_none());
    let reopened = crate::session_log::SessionLog::open(log.path).unwrap();
    assert!(reopened
        .projection
        .session
        .as_ref()
        .unwrap()
        .compaction_checkpoint
        .is_none());
    assert_eq!(
        reopened
            .frozen_messages(&HistoryPosition::Committed {
                history_through_seq: checkpoint_seq
            })
            .unwrap(),
        expected
    );
}

pub(crate) fn accepted_events() -> Vec<SessionEvent> {
    let s = legacy_session();
    let mut turn = s.turns[0].clone();
    turn.status = AgentAssistantStatus::PendingAssistant;
    turn.outcome = None;
    vec![
        SessionEvent::new(
            1,
            &s.created_at,
            None,
            EventBody::SessionCreated(SessionCreated {
                session_id: s.id,
                book_id: s.book_id,
                title: s.title.clone(),
                created_at: s.created_at.clone(),
                messages: s.messages[..1].to_vec(),
            }),
        ),
        SessionEvent::new(
            2,
            &s.updated_at,
            Some(&turn.turn_id.clone()),
            EventBody::TurnAccepted(TurnAccepted {
                turn,
                history_through_seq: 1,
                input: None,
                goals: vec![],
                title: s.title,
            }),
        ),
    ]
}

#[test]
fn session_event_question_tool_result_and_terminal_match_legacy_view() {
    let mut s = legacy_session();
    let mut call = Message::user("");
    call.role = runtime::Role::Assistant;
    call.tool_calls.push(runtime::ToolCall {
        id: "call-text".into(),
        name: "book.text".into(),
        arguments: r#"{"lid":"1.1"}"#.into(),
    });
    call.provider_continuation = Some(runtime::ProviderContinuation {
        model: "deepseek".into(),
        reasoning_content: "private-protocol".into(),
    });
    let mut result =
        Message::user(r#"{"lid":"1.1","text":"A command packages a request as an object."}"#);
    result.role = runtime::Role::Tool;
    result.tool_call_id = Some("call-text".into());
    s.messages.splice(2..2, [call, result]);
    let mut p = SessionProjection::default();
    for e in accepted_events() {
        p.fold(e).unwrap();
    }
    let turn = &s.turns[0];
    p.fold(SessionEvent::new(
        3,
        &s.updated_at,
        Some(&turn.turn_id),
        EventBody::MessageAppended {
            messages: s.messages[1..].to_vec(),
        },
    ))
    .unwrap();
    p.fold(SessionEvent::new(
        4,
        &s.updated_at,
        Some(&turn.turn_id),
        EventBody::TurnFinished(TurnFinished {
            status: turn.status,
            outcome: turn.outcome.clone(),
            error: None,
            run_summary: None,
            source_bindings: vec![],
            delivery_diagnostics: None,
            goal_ref: None,
            goals: vec![],
            history_revision: None,
        }),
    ))
    .unwrap();
    assert_eq!(
        serde_json::to_value(p.session.unwrap()).unwrap(),
        serde_json::to_value(&s).unwrap()
    );
    assert_event_roundtrip(&s);
}

#[test]
fn session_event_failed_suffix_and_result_are_one_record() {
    let mut p = SessionProjection::default();
    for e in accepted_events() {
        p.fold(e).unwrap();
    }
    let id = p.session.as_ref().unwrap().turns[0].turn_id.clone();
    p.fold(SessionEvent::new(
        3,
        "t3",
        Some(&id),
        EventBody::MessageAppended {
            messages: vec![
                Message::user("question"),
                Message::user("failed transient result"),
            ],
        },
    ))
    .unwrap();
    let f = TurnFinished {
        status: AgentAssistantStatus::Failed,
        outcome: None,
        error: Some(AgentTurnError {
            error_code: "PROVIDER_ERROR".into(),
            category: "provider".into(),
            message: "failed".into(),
        }),
        run_summary: None,
        source_bindings: vec![],
        delivery_diagnostics: None,
        goal_ref: None,
        goals: vec![],
        history_revision: Some(MessageRevision {
            from: 2,
            suffix: vec![],
        }),
    };
    let mut invalid = f.clone();
    invalid.error = None;
    assert!(p
        .fold(SessionEvent::new(
            4,
            "t4",
            Some(&id),
            EventBody::TurnFinished(invalid)
        ))
        .is_err());
    assert_eq!(p.through_seq, 3);
    assert_eq!(p.session.as_ref().unwrap().messages.len(), 3);
    p.fold(SessionEvent::new(
        4,
        "t4",
        Some(&id),
        EventBody::TurnFinished(f),
    ))
    .unwrap();
    let s = p.session.unwrap();
    assert_eq!(s.messages.len(), 2);
    assert_eq!(s.turns[0].status, AgentAssistantStatus::Failed);
}

#[test]
fn session_event_preserves_teaching_presentation_and_review_watermarks() {
    let mut s = legacy_session();
    s.turns[0].teaching_ref = Some("teaching-turn-7".into());
    s.turns[0].presentation_follow_up = Some(runtime::presentation::PresentationFollowUp {
        session_id: s.id.clone(),
        turn_id: s.turns[0].turn_id.clone(),
        reference: runtime::presentation::PresentationRef {
            presentation_id: "p1".into(),
            revision: 3,
        },
        state_revision: 2,
        saved_state_ref: "p1/state/2".into(),
    });
    s.turns[0].user_turn_ordinal = 7;
    assert_event_roundtrip(&s);
    let before = agent_history_review_cursors(&AgentHistory {
        sessions: vec![s.clone()],
        ..Default::default()
    });
    let mut p = SessionProjection::default();
    for event in sample_events(&s) { p.fold(event).unwrap(); }
    let after = agent_history_review_cursors(&AgentHistory {
        sessions: vec![p.session.unwrap()],
        ..Default::default()
    });
    assert_eq!(before[0].session_id, after[0].session_id);
    assert_eq!(before[0].book_id, after[0].book_id);
    assert_eq!(
        before[0].latest_user_turn_ordinal,
        after[0].latest_user_turn_ordinal
    );
}
