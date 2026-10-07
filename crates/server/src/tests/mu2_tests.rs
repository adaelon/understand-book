use super::*;
use crate::user_registry::{UserRegistry, USER_IDLE_TTL};
use std::time::Instant;

fn note(id: &str, content: &str) -> SaveInput {
    SaveInput {
        mem_id: Some(id.into()),
        mem_type: "note".into(),
        layer: "long_term".into(),
        book_id: "book-legacy".into(),
        anchor: Anchor {
            lid: Some("1.1".into()),
            concept: None,
        },
        content: content.into(),
        range: None,
        selection_context: None,
        note_placement: None,
        citations: None,
        source_session_id: Some("session-legacy".into()),
    }
}

#[test]
fn mu2_same_ids_are_isolated_and_two_handles_keep_both_writes_after_reload() {
    let dir = tempfile::tempdir().unwrap();
    let mut registry = UserRegistry::open(dir.path()).unwrap();
    registry.create_user("A").unwrap();
    registry.create_user("B").unwrap();
    let a = registry.get("A", "1").unwrap();
    let a2 = registry.get("A", "1").unwrap();
    let b = registry.get("B", "1").unwrap();
    assert!(Arc::ptr_eq(&a, &a2));
    assert!(!Arc::ptr_eq(&a, &b));
    let mut fixture = state_named("mu2-history");
    let book_id = fixture.workspace.book.base.book_id.clone();
    let turn = precommit_agent_turn(
        &mut fixture,
        &book_id,
        "question".into(),
        None,
        None,
        None,
        "1",
    )
    .unwrap();
    let session = fixture
        .user
        .agent_history
        .sessions
        .iter_mut()
        .find(|s| s.id == turn.session_id)
        .unwrap();
    session.id = "session-legacy".into();
    session.turns[0].turn_id = "turn-legacy".into();
    session.turns[0].status = AgentAssistantStatus::Failed;
    session.turns[0].error = Some(AgentTurnError {
        error_code: "AGENT_RUN_INTERRUPTED".into(),
        category: "interrupted".into(),
        message: "fixture".into(),
    });
    fixture
        .user
        .agent_history
        .active_by_book
        .insert(book_id, "session-legacy".into());
    for (handle, text) in [(&a, "A-private"), (&b, "B-private")] {
        let mut user = handle.lock().unwrap();
        user.store.save(note("note-legacy", text), "1").unwrap();
        let mut session = fixture.user.agent_history.sessions[0].clone();
        session.title = text.into();
        let paths = user.session_store.as_ref().unwrap().paths.clone();
        let mut log = crate::session_log::SessionLog::open(paths.session(&session.id)).unwrap();
        for event in crate::session_event::tests::sample_events(&session) { log.append(event).unwrap(); }
        let (store, history) = crate::session_store::SessionStore::open(paths).unwrap();
        user.session_store = Some(store); user.agent_history = history;
        user.learning_store()
            .unwrap()
            .mutate(
                &memory::learning::TutorMutation {
                    operation_id: "op-legacy".into(),
                    expected_revision: 0,
                    action: memory::learning::TutorAction::SetEnabled {
                        enabled: text == "A-private",
                    },
                },
                "1",
            )
            .unwrap();
        assert!(user.presentation_root().unwrap().starts_with(
            dir.path()
                .canonicalize()
                .unwrap()
                .join("users")
                .join(user.user_id().unwrap())
        ));
        assert_eq!(
            user.intent_store().unwrap().root(),
            registry.paths(user.user_id().unwrap()).unwrap().private
        );
    }
    let t1 = std::thread::spawn(move || {
        a.lock()
            .unwrap()
            .store
            .save(note("note-window-1", "one"), "2")
            .unwrap()
    });
    let t2 = std::thread::spawn(move || {
        a2.lock()
            .unwrap()
            .store
            .save(note("note-window-2", "two"), "2")
            .unwrap()
    });
    t1.join().unwrap();
    t2.join().unwrap();
    drop(b);
    assert_eq!(
        registry.evict_idle(Instant::now() + USER_IDLE_TTL).unwrap(),
        2
    );
    for (owner, count, text, enabled) in [("A", 3, "A-private", true), ("B", 1, "B-private", false)]
    {
        let user = registry.get(owner, "3").unwrap();
        let user = user.lock().unwrap();
        let records = user.store.recall(&RecallQuery::default());
        assert_eq!(records.len(), count);
        assert_eq!(
            records
                .iter()
                .find(|r| r.mem_id == "note-legacy")
                .unwrap()
                .content,
            text
        );
        assert_eq!(user.agent_history.sessions[0].id, "session-legacy");
        assert_eq!(
            user.agent_history.sessions[0].turns[0].turn_id,
            "turn-legacy"
        );
        assert_eq!(user.agent_history.sessions[0].title, text);
        assert_eq!(
            user.learning_store()
                .unwrap()
                .state()
                .unwrap()
                .control
                .enabled,
            enabled
        );
    }
}

#[test]
fn mu2_bad_user_or_corrupt_private_store_never_falls_back() {
    let root = tempfile::tempdir().unwrap();
    let mut registry = UserRegistry::open(root.path()).unwrap();
    registry.create_user("A").unwrap();
    registry.create_user("B").unwrap();
    let a = registry.get("A", "1").unwrap();
    a.lock()
        .unwrap()
        .store
        .save(note("note-legacy", "A-sentinel"), "1")
        .unwrap();
    let ap = registry.paths("A").unwrap();
    let before = std::fs::read(&ap.memory).unwrap();
    let bp = registry.paths("B").unwrap();
    std::fs::create_dir_all(bp.memory.parent().unwrap()).unwrap();
    std::fs::write(&bp.memory, "corrupt").unwrap();
    assert!(registry.get("B", "1").is_err());
    assert!(registry.get("missing", "1").is_err());
    assert!(registry.get("../A", "1").is_err());
    assert_eq!(std::fs::read(ap.memory).unwrap(), before);
    assert!(!bp.history.exists());
    assert!(!bp.learning.exists());
    std::fs::remove_file(&bp.memory).unwrap();
    let db = rusqlite::Connection::open(&bp.learning).unwrap();
    db.pragma_update(None, "user_version", 999).unwrap();
    drop(db);
    let before = std::fs::read(&bp.learning).unwrap();
    assert!(registry.get("B", "1").is_err());
    assert_eq!(std::fs::read(bp.learning).unwrap(), before);
}

fn pending_profile(state: &mut AppState, operation: &str) {
    let reply = post_profile(
        state,
        0,
        json!({"kind":"remember", "operation_id":operation,
        "evidence_text":"Remember my medical preference",
        "fact":profile_fact_draft("book", "health", "SENSITIVE_MU2_ONLY", "normal")}),
    );
    assert_eq!(reply.status, 200, "{}", reply.body);
    assert!(reply.body.contains("needs_sensitive_confirmation"));
}

#[test]
fn mu2_confirmation_expires_and_reloaded_history_requires_new_confirmation() {
    let mut state = state_named("mu2-expiry");
    pending_profile(&mut state, "op-old");
    let session = state.workspace.selected_chat.clone().unwrap();
    let deadline = state.user.agent_history.pending_confirmations[&session].expires_at;
    pending_profile(&mut state, "op-old");
    assert_eq!(
        state.user.agent_history.pending_confirmations[&session].expires_at,
        deadline
    );
    state.user.agent_history.expire_confirmations(deadline);
    assert!(state
        .user
        .agent_history
        .pending_governance_mutations
        .is_empty());
    let reply = post(&mut state, "/agent/chat", r#"{"message":"确认保存"}"#);
    assert!(
        reply.body.contains("SENSITIVE_CONFIRMATION_EXPIRED"),
        "{}",
        reply.body
    );
    pending_profile(&mut state, "op-new");
    let durable = serde_json::to_string(&state.user.agent_history).unwrap();
    assert!(!durable.contains("SENSITIVE_MU2_ONLY"));
    state.user.agent_history = serde_json::from_str(&durable).unwrap();
    let reply = post(&mut state, "/agent/chat", r#"{"message":"确认保存"}"#);
    assert!(reply.body.contains("SENSITIVE_CONFIRMATION_EXPIRED"));
    assert!(state.user.store.profile_facts().is_empty());
}

#[test]
fn mu2_prepared_confirmation_cannot_consume_replacement_operation() {
    let mut state = state_named("mu2-confirmation-identity");
    pending_profile(&mut state, "op-old");
    let prepared = prepare_agent_chat(&mut state, r#"{"message":"确认保存"}"#, "1")
        .unwrap_or_else(|r| panic!("{}", r.body));
    let session = state.workspace.selected_chat.clone().unwrap();
    let deadline = state.user.agent_history.pending_confirmations[&session].expires_at;
    state.user.agent_history.expire_confirmations(deadline);
    pending_profile(&mut state, "op-new");
    let new_id = state
        .user
        .agent_history
        .confirmation_id(&session)
        .unwrap()
        .to_string();
    let mut context = runtime::run_context::RunContext::new(
        prepared.messages.clone(),
        OuterConfig::default(),
        prepared.scope.provider_binding.clone(),
    );
    let port = crate::agent_run::BorrowedAppPort(RefCell::new(&mut state));
    let error = run_precommitted_agent_chat(&port, &UnconfiguredAdapter, &prepared, &mut context)
        .unwrap_err();
    assert_eq!(error.error_code, "SENSITIVE_CONFIRMATION_EXPIRED");
    drop(port);
    assert_eq!(
        state.user.agent_history.confirmation_id(&session),
        Some(new_id.as_str())
    );
    assert!(state.user.store.profile_facts().is_empty());
}

#[test]
fn mu2_idle_eviction_pins_confirmation_handles_and_flush_failures() {
    let root = tempfile::tempdir().unwrap();
    let mut registry = UserRegistry::open(root.path()).unwrap();
    registry.create_user("A").unwrap();
    let handle = registry.get("A", "1").unwrap();
    assert_eq!(
        registry.evict_idle(Instant::now() + USER_IDLE_TTL).unwrap(),
        0
    );
    let mut fixture = state_named("mu2-idle");
    pending_profile(&mut fixture, "op-idle");
    let now = Instant::now();
    {
        let mut user = handle.lock().unwrap();
        user.agent_history = fixture.user.agent_history.clone();
        user.last_access = now - USER_IDLE_TTL;
    }
    drop(handle);
    assert_eq!(registry.evict_idle(now).unwrap(), 0);
    assert_eq!(
        registry
            .evict_idle(now + crate::pending_confirmation::CONFIRMATION_TTL)
            .unwrap(),
        1
    );
    let handle = registry.get("A", "2").unwrap();
    let memory_path = registry.paths("A").unwrap().memory;
    {
        let mut user = handle.lock().unwrap();
        assert!(user.agent_history.pending_governance_mutations.is_empty());
        user.store.enqueue_read("book-legacy", "1.1", "2").unwrap();
        user.last_access = now - USER_IDLE_TTL;
    }
    // A supported IO failure: a directory blocks the atomic temporary file.
    let temporary = memory_path.with_extension("replace.tmp");
    std::fs::create_dir(&temporary).unwrap();
    drop(handle);
    assert!(registry.evict_idle(now).is_err());
    std::fs::remove_dir(&temporary).unwrap();
    let handle = registry.get("A", "3").unwrap();
    assert_eq!(handle.lock().unwrap().store.pending_read_count(), 1);
    handle.lock().unwrap().last_access = now - USER_IDLE_TTL;
    drop(handle);
    assert_eq!(registry.evict_idle(now).unwrap(), 1);
    let handle = registry.get("A", "4").unwrap();
    assert_eq!(
        handle.lock().unwrap().store.read_lids("book-legacy"),
        vec!["1.1"]
    );
}

#[test]
fn mu2_capacity_and_pending_turns_prevent_unflushed_runtime_replacement() {
    let root = tempfile::tempdir().unwrap();
    let mut users = UserRegistry::open(root.path()).unwrap();
    let mut handles = Vec::new();
    for n in 0..crate::user_registry::MAX_LOADED_USERS {
        let id = format!("user-{n}");
        users.create_user(&id).unwrap();
        handles.push(users.get(&id, "1").unwrap());
    }
    users.create_user("overflow").unwrap();
    assert!(
        matches!(users.get("overflow", "1"), Err(e) if e.error_code == "USER_RUNTIME_CAPACITY")
    );
    let mut fixture = state_named("mu2-pending");
    let book = fixture.workspace.book.base.book_id.clone();
    precommit_agent_turn(
        &mut fixture,
        &book,
        "question".into(),
        None,
        None,
        None,
        "1",
    )
    .unwrap();
    handles[0].lock().unwrap().agent_history = fixture.user.agent_history;
    let now = Instant::now() + USER_IDLE_TTL;
    drop(handles);
    assert_eq!(
        users.evict_idle(now).unwrap(),
        crate::user_registry::MAX_LOADED_USERS - 1
    );
    assert!(users.get("overflow", "2").is_ok());
    assert_eq!(
        users
            .get("user-0", "2")
            .unwrap()
            .lock()
            .unwrap()
            .agent_history
            .sessions[0]
            .turns[0]
            .status,
        AgentAssistantStatus::PendingAssistant
    );
}
