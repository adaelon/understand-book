use super::*;


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
            std::fs::read_to_string(user.test_presentation_root().unwrap().join("original-page"))
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
