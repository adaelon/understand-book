use super::*;
use crate::session_event::tests::{accepted_events, legacy_session, sample_events};

fn open(dir: &tempfile::TempDir) -> SessionLog {
    SessionLog::open(dir.path().join("session.jsonl")).unwrap()
}

#[test]
fn session_log_reopen_and_position_keep_frozen_prefix() {
    let dir = tempfile::tempdir().unwrap();
    let mut log = open(&dir);
    for event in accepted_events() {
        log.append(event).unwrap();
    }
    let before = log
        .frozen_messages(&HistoryPosition::Committed {
            history_through_seq: 1,
        })
        .unwrap();
    let id = log.projection.session.as_ref().unwrap().turns[0]
        .turn_id
        .clone();
    let e = log.next_event(
        "later",
        Some(&id),
        EventBody::MessageAppended {
            messages: vec![Message::user("later")],
        },
    );
    log.append(e.clone()).unwrap();
    let bytes = std::fs::read(&log.path).unwrap();
    let mut reopened = open(&dir);
    assert_eq!(
        reopened.projection.session.as_ref().unwrap().messages.len(),
        before.len() + 1
    );
    assert_eq!(
        reopened
            .frozen_messages(&HistoryPosition::Committed {
                history_through_seq: 1
            })
            .unwrap(),
        before
    );
    reopened.append(e).unwrap();
    assert_eq!(std::fs::read(&log.path).unwrap(), bytes);
    assert!(reopened.at(4).is_err());
}

#[test]
fn session_log_torn_tail_is_read_only_then_repaired_by_writer() {
    for tail in [
        b"{\"version\":".as_slice(),
        "{\"text\":\"半".as_bytes(),
        &[0xe5, 0x8d],
    ] {
        let dir = tempfile::tempdir().unwrap();
        let mut log = open(&dir);
        let events = accepted_events();
        log.append(events[0].clone()).unwrap();
        let prefix = std::fs::read(&log.path).unwrap();
        OpenOptions::new()
            .append(true)
            .open(&log.path)
            .unwrap()
            .write_all(tail)
            .unwrap();
        let mut reopened = open(&dir);
        assert_eq!(reopened.projection.through_seq, 1);
        assert_eq!(
            std::fs::metadata(&log.path).unwrap().len(),
            (prefix.len() + tail.len()) as u64
        );
        reopened.append(events[1].clone()).unwrap();
        assert_eq!(open(&dir).projection.through_seq, 2);
        assert!(std::fs::read(&log.path).unwrap().starts_with(&prefix));
    }
}

#[test]
fn session_log_complete_corruption_is_not_a_torn_tail() {
    for bad in ["not-json\n".to_string(), serde_json::to_string(&SessionEvent::new(9, "now", None, EventBody::SessionUpdated { title: "bad".into() })).unwrap() + "\n", "{\"version\":99,\"seq\":2,\"at\":\"now\",\"kind\":\"session.updated\",\"payload\":{\"title\":\"bad\"}}\n".into()] {
        let dir = tempfile::tempdir().unwrap(); let mut log = open(&dir);
        log.append(accepted_events()[0].clone()).unwrap();
        OpenOptions::new().append(true).open(&log.path).unwrap().write_all(bad.as_bytes()).unwrap();
        let original = std::fs::read(&log.path).unwrap();
        let error = SessionLog::open(log.path.clone()).err().unwrap();
        assert!(error.message.contains("byte="));
        assert!(error.message.contains("session.jsonl"));
        assert_eq!(std::fs::read(&log.path).unwrap(), original);
    }
}

#[test]
fn session_log_partial_write_and_sync_failure_do_not_advance_or_duplicate() {
    for full in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let mut log = open(&dir);
        let events = accepted_events();
        log.append(events[0].clone()).unwrap();
        let failed = log.append_with(events[1].clone(), |file, bytes| {
            file.write_all(if full {
                bytes
            } else {
                &bytes[..bytes.len() / 2]
            })?;
            Err(std::io::Error::other("injected write/sync failure"))
        });
        assert!(failed.is_err());
        assert_eq!(log.projection.through_seq, 1);
        assert!(log.projection.session.as_ref().unwrap().turns.is_empty());
        let different = log.next_event(
            "different",
            None,
            EventBody::SessionUpdated {
                title: "other".into(),
            },
        );
        assert!(log.append(different).is_err());
        log.append(events[1].clone()).unwrap();
        assert_eq!(log.projection.through_seq, 2);
        assert_eq!(open(&dir).projection.session.unwrap().turns.len(), 1);
        assert_eq!(
            std::fs::read_to_string(&log.path).unwrap().lines().count(),
            2
        );
    }
}

#[test]
fn jl4_uncertain_terminal_retry_adopts_exact_record_without_duplicate() {
    use crate::session_store::{SessionPaths,SessionStore};
    for full in [false,true] {
        let dir=tempfile::tempdir().unwrap();
        let path=dir.path().join("agent-history.json");
        let paths=SessionPaths::from_history(&path);
        std::fs::create_dir_all(&paths.root).unwrap();
        let mut session=legacy_session();
        let reference=AgentTurnRef {session_id:session.id.clone(),turn_id:session.turns[0].turn_id.clone(),user_turn_ordinal:session.turns[0].user_turn_ordinal};
        let outcome=session.turns[0].outcome.take();
        session.turns[0].status=AgentAssistantStatus::PendingAssistant;
        let mut log=SessionLog::open(paths.session(&session.id)).unwrap();
        for event in sample_events(&session) { log.append(event).unwrap(); }
        let before_seq = log.projection.through_seq;
        let (store,history)=SessionStore::open(paths).unwrap();
        let memory=MemoryStore::open(dir.path().join("memory.json")).unwrap();
        let mut user=user_runtime::UserRuntime::local(memory,Some(path),history,None);
        user.session_store=Some(store);
        let log=user.session_store.as_mut().unwrap().logs.get_mut(&reference.session_id).unwrap();
        let terminal=log.next_event("done",Some(&reference.turn_id),EventBody::TurnFinished(TurnFinished {
            status:AgentAssistantStatus::Completed,outcome:outcome.clone(),error:None,run_summary:None,
            source_bindings:vec![],delivery_diagnostics:None,goal_ref:None,goals:vec![],history_revision:None,
        }));
        assert!(log.append_with(terminal,|file,bytes|{
            file.write_all(if full {bytes}else{&bytes[..bytes.len()/2]})?;
            Err(std::io::Error::other("uncertain terminal sync"))
        }).is_err());
        let messages=user.agent_history.sessions[0].messages.clone();
        finalize_user_agent_turn(&mut user,&reference,AgentAssistantStatus::Completed,outcome,None,None,&messages,"done").unwrap();
        let log=&user.session_store.as_ref().unwrap().logs[&reference.session_id];
        assert_eq!(std::fs::read_to_string(&log.path).unwrap().lines().count() as u64,before_seq+1);
        assert_eq!(user.agent_history.sessions[0].turns[0].status,AgentAssistantStatus::Completed);
        assert_eq!(SessionLog::open(log.path.clone()).unwrap().projection.through_seq,before_seq+1);
    }
}

#[test]
fn session_log_growth_only_writes_the_new_record_and_sanitizes_before_append() {
    let dir = tempfile::tempdir().unwrap();
    let mut log = open(&dir);
    let mut s = legacy_session();
    s.messages[1].content = Some("long history ".repeat(5000));
    let id = s.turns[0].turn_id.clone();
    for event in sample_events(&s) { log.append(event).unwrap(); }
    let prefix = std::fs::read(&log.path).unwrap();
    let mut call = Message::user("");
    call.role = runtime::Role::Assistant;
    call.tool_calls.push(runtime::ToolCall {
        id: "author".into(),
        name: "presentation.author".into(),
        arguments: r#"{"operation":"create","html":"DO_NOT_PERSIST_HTML"}"#.into(),
    });
    log.append(log.next_event(
        "later",
        Some(&id),
        EventBody::MessageAppended {
            messages: vec![call],
        },
    ))
    .unwrap();
    let after = std::fs::read(&log.path).unwrap();
    assert!(after.starts_with(&prefix));
    assert!(after.len() - prefix.len() < 2000);
    assert!(!String::from_utf8(after)
        .unwrap()
        .contains("DO_NOT_PERSIST_HTML"));
}

#[test]
fn session_log_goal_updates_and_finished_goal_reference_commit_together() {
    let dir = tempfile::tempdir().unwrap();
    let mut log = open(&dir);
    let mut events = accepted_events();
    let EventBody::TurnAccepted(a) = &mut events[1].body else {
        unreachable!()
    };
    let mut goal = runtime::goal::ResidentGoal::new(
        "goal-1".into(),
        a.turn.turn_id.clone(),
        a.turn.user.clone(),
    );
    a.turn.goal_ref = Some(AgentGoalRef {
        id: goal.id.clone(),
        revision: 1,
    });
    a.goals = vec![goal.clone()];
    let id = a.turn.turn_id.clone();
    for e in events {
        log.append(e).unwrap();
    }
    let requirements = goal.requirements.clone();
    let items = serde_json::json!([
        {"id":"prototype","description":"Key relationship prototype","status":"completed"},
        {"id":"review","description":"Review the whole page","status":"in_progress"}
    ]);
    goal.apply_update(serde_json::from_value(serde_json::json!({
        "operation":"working", "focus":"review", "next_move":"preview", "items":items
    })).unwrap(), &id, "continue").unwrap();
    assert_eq!(goal.revision, 2);
    assert_eq!(crate::session_store::changed_goals(log.projection.session.as_ref().unwrap(), &[goal.clone()]), vec![goal.clone()]);
    log.append(log.next_event(
        "progress",
        Some(&id),
        EventBody::GoalUpdated { goal: goal.clone() },
    ))
    .unwrap();
    assert!(crate::session_store::changed_goals(log.projection.session.as_ref().unwrap(), &[goal.clone()]).is_empty());
    assert_eq!(open(&dir).projection.session.unwrap().goals[0], goal, "GoalUpdated must recover every work item");
    let mut stale = goal.clone();
    stale.revision = 1;
    assert!(log
        .append(log.next_event("stale", Some(&id), EventBody::GoalUpdated { goal: stale }))
        .is_err());
    goal.revision = 3;
    goal.status = runtime::goal::GoalStatus::Completed;
    goal.working.items[1].status = runtime::goal::GoalWorkItemStatus::Completed;
    let final_goal = goal.clone();
    let original = legacy_session().turns.remove(0);
    let terminal = TurnFinished {
        status: AgentAssistantStatus::Completed,
        outcome: original.outcome,
        error: None,
        run_summary: None,
        source_bindings: vec![],
        delivery_diagnostics: None,
        goal_ref: Some(AgentGoalRef {
            id: goal.id.clone(),
            revision: 3,
        }),
        goals: vec![goal],
        history_revision: None,
    };
    log.append(log.next_event("done", Some(&id), EventBody::TurnFinished(terminal)))
        .unwrap();
    let s = open(&dir).projection.session.unwrap();
    assert_eq!(s.goals[0].status, runtime::goal::GoalStatus::Completed);
    assert_eq!(s.goals[0], final_goal, "TurnFinished must restore the final plan with the goal revision");
    assert_eq!(s.goals[0].requirements, requirements);
    assert_eq!(
        s.turns[0].goal_ref.as_ref().unwrap().revision,
        s.goals[0].revision
    );
}

#[test]
fn ex13_session_log_old_goal_records_default_to_an_empty_work_plan() {
    let dir = tempfile::tempdir().unwrap();
    let mut log = open(&dir);
    for event in accepted_events() { log.append(event).unwrap(); }
    let session = log.projection.session.as_ref().unwrap();
    let id = session.turns[0].turn_id.clone();
    let goal = runtime::goal::ResidentGoal::new("old-goal".into(), id.clone(), "Make a page".into());
    let event = log.next_event("old", Some(&id), EventBody::GoalUpdated { goal: goal.clone() });
    let mut old = serde_json::to_value(event).unwrap();
    old["payload"]["goal"]["working"].as_object_mut().unwrap().remove("items");
    writeln!(OpenOptions::new().append(true).open(&log.path).unwrap(), "{}", old).unwrap();
    assert_eq!(open(&dir).projection.session.unwrap().goals, vec![goal]);
}
