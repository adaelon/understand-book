use super::*;

#[test]
fn jl7_startup_ignores_old_snapshot_and_recovers_only_jsonl_selection() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("agent-history.json");
    // Even an unreadable old format must not block the new chat store.
    let old = b"old chat bytes, deliberately not valid JSON";
    std::fs::write(&path, old).unwrap();
    let backup = agent_history_backup_path(&path);
    std::fs::write(&backup, b"old backup").unwrap();
    let (mut history, store) = load_chat_storage(&Some(path.clone())).unwrap();
    let mut store = store.unwrap();
    assert!(history.sessions.is_empty());
    assert!(store.paths.root.is_dir());
    store.ensure_book(&mut history, "book", "2026-10-01T12:00:00Z").unwrap();
    let id = history.active_by_book["book"].clone();
    std::fs::write(&store.paths.selection, br#"{"active_by_book":{"book":"old-chat"}}"#).unwrap();
    let (reopened, _) = load_chat_storage(&Some(path.clone())).unwrap();
    assert_eq!(reopened.sessions.len(), 1);
    assert_eq!(reopened.active_by_book["book"], id);
    assert_eq!(std::fs::read(&path).unwrap(), old);
    assert_eq!(std::fs::read(&backup).unwrap(), b"old backup");
}

fn setup() -> (tempfile::TempDir, SessionStore, AgentHistory) {
    let dir = tempfile::tempdir().unwrap();
    let paths = SessionPaths::from_history(&dir.path().join("agent-history.json"));
    std::fs::create_dir_all(&paths.root).unwrap();
    let (store, history) = SessionStore::open(paths).unwrap();
    (dir, store, history)
}

#[test]
fn session_store_colon_identity_selection_restart_and_delete() {
    let (_dir, mut store, mut history) = setup();
    let first = new_agent_session("book", "2026-10-01T12:34:56+08:00", 0);
    let id = first.id.clone();
    store.create(&mut history, first).unwrap();
    let other = new_agent_session("book", "2026-10-01T12:35:56+08:00", 1);
    let other_id = other.id.clone();
    store.create(&mut history, other).unwrap();
    store.select(&mut history, "book", &id).unwrap();
    let path = store.paths.session(&id);
    assert!(!path.file_name().unwrap().to_str().unwrap().contains(':'));
    let filename = path
        .file_stem()
        .unwrap()
        .to_str()
        .unwrap()
        .strip_prefix("s_")
        .unwrap();
    assert_eq!(
        String::from_utf8(URL_SAFE_NO_PAD.decode(filename).unwrap()).unwrap(),
        id
    );
    let (mut store, mut reopened) = SessionStore::open(store.paths).unwrap();
    assert_eq!(reopened.active_by_book["book"], id);
    let other_bytes = std::fs::read(store.paths.session(&other_id)).unwrap();
    store.delete(&mut reopened, &id).unwrap();
    assert!(!path.exists());
    assert_eq!(reopened.active_by_book["book"], other_id);
    assert_eq!(
        std::fs::read(store.paths.session(&other_id)).unwrap(),
        other_bytes
    );
    let (_, reopened) = SessionStore::open(store.paths).unwrap();
    assert_eq!(reopened.sessions.len(), 1);
    assert_eq!(reopened.active_by_book["book"], other_id);
}

#[test]
fn session_store_missing_or_stale_selection_uses_existing_recent_chat() {
    let (_dir, mut store, mut history) = setup();
    for (n, now) in [
        "server-start",
        "2026-10-01T00:00:00+08:00",
        "2026-09-30T18:00:00Z",
        "1893456000",
    ]
    .iter()
    .enumerate()
    {
        store
            .create(&mut history, new_agent_session("book", now, n))
            .unwrap();
    }
    let expected = history.sessions[3].id.clone();
    let (_, h) = SessionStore::open(store.paths.clone()).unwrap();
    assert_eq!(h.active_by_book["book"], expected);
    std::fs::write(
        &store.paths.selection,
        br#"{"active_by_book":{"book":"deleted","wrong-book":"deleted"}}"#,
    )
    .unwrap();
    let (_, h) = SessionStore::open(store.paths).unwrap();
    assert_eq!(h.active_by_book.len(), 1);
    assert_eq!(h.active_by_book["book"], expected);
}

#[test]
fn session_store_two_users_same_id_eviction_and_original_presentation_root() {
    let dir = tempfile::tempdir().unwrap();
    let mut registry = user_registry::UserRegistry::open(dir.path()).unwrap();
    for owner in ["A", "B"] {
        registry.create_user(owner).unwrap();
        let paths = registry.paths(owner).unwrap();
        assert_eq!(
            paths.presentations,
            paths.history.with_extension("presentations")
        );
        std::fs::create_dir_all(&paths.sessions).unwrap();
        std::fs::create_dir_all(&paths.presentations).unwrap();
        std::fs::write(paths.presentations.join("original-page"), owner).unwrap();
        let h = registry.get(owner, "2026-10-01T00:00:00Z").unwrap();
        let mut user = h.lock().unwrap();
        let mut session = new_agent_session("book", "time:with:colons", 0);
        session.title = owner.into();
        let user = &mut *user;
        user.session_store
            .as_mut()
            .unwrap()
            .create(&mut user.agent_history, session)
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(user.presentation_root().unwrap().join("original-page"))
                .unwrap(),
            owner
        );
        assert!(!paths.history.exists());
    }
    assert_eq!(
        registry
            .evict_idle(
                std::time::Instant::now() + user_registry::USER_IDLE_TTL + Duration::from_secs(1)
            )
            .unwrap(),
        2
    );
    for owner in ["A", "B"] {
        let h = registry.get(owner, "2026-10-01T01:00:00Z").unwrap();
        assert_eq!(h.lock().unwrap().agent_history.sessions[0].title, owner);
    }
}

#[test]
fn session_store_selection_write_failure_preserves_previous_selection_and_log() {
    let (_dir, mut store, mut history) = setup();
    store
        .create(&mut history, new_agent_session("book", "t0", 0))
        .unwrap();
    let id = history.sessions[0].id.clone();
    store.select(&mut history, "book", &id).unwrap();
    store
        .create(&mut history, new_agent_session("book", "t1", 1))
        .unwrap();
    let other = history.sessions[1].id.clone();
    std::fs::create_dir(agent_history_temporary_path(&store.paths.selection)).unwrap();
    assert!(store.select(&mut history, "book", &other).is_err());
    assert_eq!(history.active_by_book["book"], id);
    let (_, h) = SessionStore::open(store.paths).unwrap();
    assert_eq!(h.active_by_book["book"], id);
    assert_eq!(h.sessions.len(), 2);
}
