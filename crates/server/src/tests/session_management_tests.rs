use super::*;
use crate::session_store::{SessionPaths, SessionStore};

#[test]
fn session_management_delete_cancels_running_review_for_both_storage_modes() {
    for jsonl in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let mut state = state_named(if jsonl { "capacity-delete-jsonl" } else { "capacity-delete-memory" });
        if jsonl {
            let history_path = dir.path().join("agent-history.json");
            let paths = SessionPaths::from_history(&history_path);
            std::fs::create_dir_all(&paths.root).unwrap();
            let (store, history) = SessionStore::open(paths).unwrap();
            state.user.history_path = Some(history_path);
            state.user.session_store = Some(store);
            state.user.agent_history = history;
        }
        assert_eq!(route_agent_new(&mut state, "1").status, 200);
        let id = state.workspace.selected_chat.clone().unwrap();
        state.user.store.reconcile_review_jobs(&[memory::ReviewSessionCursor {
            learning_workspace_id: None, turns: vec![], session_id: id.clone(),
            book_id: state.workspace.book.base.book_id.clone(), latest_user_turn_ordinal: 1,
        }], "2").unwrap();
        let job = state.user.store.review_state().review_jobs[0].clone();
        state.user.store.claim_review_job(&job.job_id, "3").unwrap();
        let reply = route_agent_history_delete(&mut state, &json!({"session_id":id}).to_string(), "4");
        assert_eq!(reply.status, 200, "{}", reply.body);
        let review = state.user.store.review_state();
        assert_eq!(review.review_jobs[0].status, ReviewJobStatus::Cancelled);
        assert_eq!(review.review_jobs[0].attempts, 1);
        assert!(!review.reviewed_through.contains_key(&id));
        assert!(state.user.store.commit_review_result(&job.job_id, &["turn".into()], &[], &[], "5").is_err());
    }
}

#[test]
fn session_management_routes_reopen_preserve_transcript_and_existing_presentation() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = state_named("jl2-management");
    let history_path = dir.path().join("agent-history.json");
    let paths = SessionPaths::from_history(&history_path);
    std::fs::create_dir_all(&paths.root).unwrap();
    let (store, history) = SessionStore::open(paths).unwrap();
    state.user.history_path = Some(history_path.clone());
    state.user.session_store = Some(store);
    state.user.agent_history = history;
    assert_eq!(
        route_agent_new(&mut state, "2026-10-01T10:00:00Z").status,
        200
    );
    let first = state.workspace.selected_chat.clone().unwrap();
    let first_path = state
        .user
        .session_store
        .as_ref()
        .unwrap()
        .paths
        .session(&first);
    let first_bytes = std::fs::read(&first_path).unwrap();
    assert_eq!(
        route_agent_new(&mut state, "2026-10-01T10:01:00Z").status,
        200
    );
    let second = state.workspace.selected_chat.clone().unwrap();
    let selected = json!({"session_id":first}).to_string();
    assert_eq!(
        route_agent_history_select(&mut state, &selected).status,
        200
    );
    assert_eq!(std::fs::read(&first_path).unwrap(), first_bytes);
    let (history, store) =
        crate::session_store::load_chat_storage(&Some(history_path.clone())).unwrap();
    state.user.agent_history = history;
    state.user.session_store = store;
    assert_eq!(
        state.user.agent_history.active_by_book[&state.workspace.book.base.book_id],
        first
    );
    assert_eq!(
        route_agent_history_delete(&mut state, &selected, "now").status,
        200
    );
    assert_eq!(
        state.workspace.selected_chat.as_deref(),
        Some(second.as_str())
    );
    assert!(!first_path.exists());
    assert!(!history_path.exists());
    assert_eq!(
        state.user.presentation_root().unwrap(),
        history_path.with_extension("presentations")
    );
    assert_eq!(
        save_agent_history_path(&state.user.history_path, &state.user.agent_history)
            .unwrap_err()
            .error_code,
        "SESSION_EVENT_WRITE_REQUIRED"
    );
}

#[test]
fn session_management_pending_delete_preserves_session_and_selection() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = state_named("jl2-pending-delete");
    let history_path = dir.path().join("agent-history.json");
    let paths = SessionPaths::from_history(&history_path);
    std::fs::create_dir_all(&paths.root).unwrap();
    let mut session = new_agent_session(&state.workspace.book.base.book_id, "t0", 0);
    append_pending_agent_turn(
        &mut session,
        "pending".into(),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        "t1",
    )
    .unwrap();
    let id = session.id.clone();
    let mut log = crate::session_log::SessionLog::open(paths.session(&id)).unwrap();
    for event in crate::session_event::tests::sample_events(&session) { log.append(event).unwrap(); }
    let (store, history) = SessionStore::open(paths.clone()).unwrap();
    state.user.session_store = Some(store);
    state.user.agent_history = history;
    state.user.history_path = Some(history_path);
    let body = json!({"session_id":id}).to_string();
    assert_eq!(route_agent_history_select(&mut state, &body).status, 200);
    let bytes = std::fs::read(paths.session(&id)).unwrap();
    let reply = route_agent_history_delete(&mut state, &body, "now");
    assert!(reply.body.contains("CHAT_BUSY"));
    assert_eq!(std::fs::read(paths.session(&id)).unwrap(), bytes);
    assert_eq!(state.workspace.selected_chat.as_deref(), Some(id.as_str()));
}
