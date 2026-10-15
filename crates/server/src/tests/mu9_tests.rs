use super::*;
use crate::{
    control_store::{ControlStore, ServiceWriter},
    published_library::PublishedLibrary,
    reader_maintenance::*,
    user_registry::UserRegistry,
};
use std::fs;

struct MigrationFixture {
    _root: tempfile::TempDir,
    plan: MigrationPlan,
}
impl MigrationFixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let memory = root.path().join("old-memory");
        let private = root.path().join("old-private");
        let service = root.path().join("service");
        fs::create_dir_all(&memory).unwrap();
        fs::create_dir_all(private.join("accepted/intent/artifact")).unwrap();
        let book = root.path().join("book");
        fs::create_dir(&book).unwrap();
        let mut base = sample_base();
        base.book_id = "mu9-book".into();
        fs::write(book.join("base.json"), serde_json::to_vec(&base).unwrap()).unwrap();
        fs::write(book.join("source.txt"), "X".repeat(100) + "尾巴").unwrap();
        fs::write(
            book.join("unknown-build-note.txt"),
            "keep complete original package",
        )
        .unwrap();
        let writer = ServiceWriter::acquire(&service).unwrap();
        let mut control = ControlStore::open(writer.clone()).unwrap();
        for user in ["A", "B"] {
            control.create_user(user).unwrap();
        }
        let mut library = PublishedLibrary::new(ControlStore::open(writer).unwrap());
        let publication = library.publish(&book).unwrap().reference;
        let mut store = MemoryStore::open_private(memory.join("memory.json")).unwrap();
        store
            .save(
                SaveInput {
                    mem_id: Some("note-old".into()),
                    mem_type: "note".into(),
                    layer: "long_term".into(),
                    book_id: "mu9-book".into(),
                    anchor: Anchor {
                        lid: Some("1.1".into()),
                        concept: None,
                    },
                    content: r"原文 C:\books\中文 空格 保持不动".into(),
                    range: None,
                    selection_context: None,
                    note: None,
                    note_placement: None,
                    citations: None,
                    source_session_id: Some("chat-old".into()),
                },
                "1",
            )
            .unwrap();
        store
            .learning_store()
            .unwrap()
            .mutate(
                &memory::learning::TutorMutation {
                    operation_id: "learning-op-old".into(),
                    expected_revision: 0,
                    action: memory::learning::TutorAction::SetEnabled { enabled: true },
                },
                "1",
            )
            .unwrap();
        drop(store);
        let evidence = json!({"evidence_id":"evidence-old","action_ref":"action-old","delivery_ref":"turn-old","session_id":"chat-old","source_id":"mu9-book","source_revision":"source-old","map_revision":"map-old","object_id":"object-old","object_revision":1,"label":"旧证据","capability":"explain","assistance_refs":[],"attempt":1,"assessment_ref":null,"status":"unassessed","interpretation":"原解释","learner_quote":"原回答","interpreter_version":"fixture","supersedes":null,"correction":null,"source_quotes":{}});
        let db = rusqlite::Connection::open(memory.join("learning.db")).unwrap();
        db.execute("INSERT INTO learning_evidence(evidence_id,source_id,evidence) VALUES('evidence-old','mu9-book',?)",[evidence.to_string()]).unwrap();
        drop(db);
        let mut chat = new_agent_session("mu9-book", "1", 1);
        chat.id = "chat-old".into();
        chat.turns.push(serde_json::from_value(json!({"turn_id":"turn-old","user_turn_ordinal":1,"user":"问题原文", "status":"pending_assistant","question_anchor_lid":null,"question_quote":null})).unwrap());
        let history = AgentHistory {
            sessions: vec![chat],
            active_by_book: BTreeMap::from([("mu9-book".into(), "chat-old".into())]),
            ..Default::default()
        };
        fs::write(
            memory.join("agent-history.json"),
            serde_json::to_vec(&history).unwrap(),
        )
        .unwrap();
        fs::write(memory.join("session.json"),json!({"current_book_dir":r"C:\books\中文 空格","books":{r"\\?\C:\books\中文 空格":{"top_lid":"1.1"}}}).to_string()).unwrap();
        fs::create_dir_all(memory.join("agent-history.presentations/versions/pres-old")).unwrap();
        fs::write(memory.join("agent-history.presentations/versions/pres-old/1.json"),json!({"reference":{"presentation_id":"pres-old","revision":1},"candidate_id":"candidate-old","owner":{"book_id":"mu9-book","session_id":"chat-old"},"created_by_turn_id":"turn-old","based_on":null,"content":{"title":"旧演示","content_files":{"index.html":"<h1>原文</h1>"},"entrypoint":"index.html","readable_content":"原文","source_bindings":[],"assumptions":[],"state_contract":{},"initial_state":{"slider":2},"animation_assets":{}}}).to_string()).unwrap();
        fs::write(
            private.join("accepted/intent/artifact/result.txt"),
            "私人目标成果",
        )
        .unwrap();
        fs::write(memory.join("auxiliary.txt"), "unknown helper preserved").unwrap();
        Self {
            plan: MigrationPlan {
                operation_id: "import-one".into(),
                memory_dir: memory,
                private_dir: private,
                library_root: book.clone(),
                service_root: service,
                user_id: "A".into(),
                backup_dir: root.path().join("backup"),
                books: vec![BookMapping {
                    legacy_dir: r"C:\books\中文 空格".into(),
                    source_dir: book,
                    publication,
                }],
                reviewed_files: BTreeSet::from([
                    "memory/auxiliary.txt".into(),
                    "private/accepted/intent/artifact/result.txt".into(),
                ]),
            },
            _root: root,
        }
    }
}

#[test]
fn mu9_t60_preview_import_retry_and_interrupted_files_preserve_owner_ids_and_bytes() {
    let f = MigrationFixture::new();
    let p = &f.plan;
    let old_memory = fs::read(p.memory_dir.join("memory.json")).unwrap();
    let old_history = fs::read(p.memory_dir.join("agent-history.json")).unwrap();
    let preview = migration_preview(p).unwrap();
    assert_eq!(preview["counts"]["chats"], 1);
    assert_eq!(preview["counts"]["legacy_pending"], 1);
    assert!(!p.backup_dir.exists());
    assert!(!p.service_root.join("users/A").exists());
    assert!(migrate(p, false).is_err());
    assert!(migrate_inner(p, true, Some(2)).is_err());
    assert!(
        matches!(ServiceWriter::acquire(&p.service_root),Err(e) if e.error_code=="SERVICE_MAINTENANCE_INCOMPLETE")
    );
    assert_eq!(migrate(p, true).unwrap()["state"], "complete");
    assert_eq!(migrate(p, true).unwrap()["resumed"], true);
    let target = p.service_root.join("users/A");
    assert_eq!(
        fs::read(target.join("memory/memory.json")).unwrap(),
        old_memory
    );
    assert_eq!(
        fs::read(p.memory_dir.join("agent-history.json")).unwrap(),
        old_history
    );
    assert_eq!(
        fs::read_to_string(target.join("memory/auxiliary.txt")).unwrap(),
        "unknown helper preserved"
    );
    assert_eq!(
        fs::read_to_string(target.join("private/accepted/intent/artifact/result.txt")).unwrap(),
        "私人目标成果"
    );
    let h: Value =
        serde_json::from_slice(&fs::read(target.join("memory/agent-history.json")).unwrap())
            .unwrap();
    assert_eq!(h["sessions"][0]["id"], "chat-old");
    assert_eq!(h["sessions"][0]["turns"][0]["turn_id"], "turn-old");
    assert_eq!(
        h["sessions"][0]["turns"][0]["published_book_ref"],
        json!(p.books[0].publication)
    );
    let mut users = UserRegistry::open(&p.service_root).unwrap();
    let a = users.get("A", "2").unwrap();
    let b = users.get("B", "2").unwrap();
    assert_eq!(
        a.lock().unwrap().store.recall(&RecallQuery::default())[0].mem_id,
        "note-old"
    );
    assert!(b
        .lock()
        .unwrap()
        .store
        .recall(&RecallQuery::default())
        .is_empty());
    assert!(b.lock().unwrap().agent_history.sessions.is_empty());
    assert!(
        a.lock()
            .unwrap()
            .learning_store()
            .unwrap()
            .state()
            .unwrap()
            .control
            .enabled
    );
    assert_eq!(
        a.lock()
            .unwrap()
            .learning_store()
            .unwrap()
            .evidence("evidence-old")
            .unwrap()
            .delivery_ref.as_deref(),
        Some("turn-old")
    );
    assert!(
        !b.lock()
            .unwrap()
            .learning_store()
            .unwrap()
            .state()
            .unwrap()
            .control
            .enabled
    );
    assert!(a.lock().unwrap().agent_history.sessions.is_empty());
    let control = ControlStore::open(users.writer()).unwrap();
    let (count, checkpoint): (i64, String) = control
        .connection
        .query_row(
            "SELECT count(*),checkpoint FROM reader_workspaces WHERE owner_user_id='A'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(count, 1);
    assert_eq!(
        serde_json::from_str::<Value>(&checkpoint).unwrap()["reader"]["top_lid"],
        "1.1"
    );
}

#[test]
fn mu9_t60_conflicting_plan_unknown_files_and_committed_target_are_rejected() {
    let f = MigrationFixture::new();
    let p = &f.plan;
    let mut unknown = p.clone();
    unknown.reviewed_files.clear();
    assert_eq!(
        migration_preview(&unknown).unwrap()["pending_review"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert!(migrate(&unknown, true)
        .unwrap_err()
        .to_string()
        .contains("Review unknown"));
    assert!(!p.service_root.join("maintenance.json").exists());
    assert!(migrate_inner(p, true, Some(1)).is_err());
    let mut changed = p.clone();
    changed.user_id = "B".into();
    assert!(migrate(&changed, true)
        .unwrap_err()
        .to_string()
        .contains("plan conflict"));
    let file = p.service_root.join("users/A/memory/agent-history.json");
    fs::write(&file, "conflicting target").unwrap();
    assert!(migrate(p, true)
        .unwrap_err()
        .to_string()
        .contains("Target file conflict"));
    assert!(p.service_root.join("maintenance.json").exists());
}

#[test]
fn mu9_t60_resume_after_metadata_commit_and_reject_completed_target_change() {
    let f = MigrationFixture::new();
    let p = &f.plan;
    assert!(migrate_inner(p, true, Some(usize::MAX)).is_err());
    migrate(p, true).unwrap();
    migrate(p, true).unwrap();
    fs::write(
        p.service_root.join("users/A/memory/auxiliary.txt"),
        "changed",
    )
    .unwrap();
    assert!(migrate(p, true).is_err());
    assert!(!p.service_root.join("maintenance.json").exists());
}

#[test]
fn mu9_t61_all_maintenance_operations_share_the_service_writer_lock() {
    let f = MigrationFixture::new();
    let p = &f.plan;
    let _writer = ServiceWriter::acquire(&p.service_root).unwrap();
    for error in [
        migration_preview(p).unwrap_err(),
        migrate(p, true).unwrap_err(),
        backup_service(&p.service_root, &p.backup_dir).unwrap_err(),
        export_user(&p.service_root, "A", &p.backup_dir).unwrap_err(),
    ] {
        assert!(error.to_string().contains("SERVICE_WRITER_BUSY"), "{error}");
    }
    assert!(!p.backup_dir.exists());
}

#[test]
fn mu9_t62_real_snapshot_restore_keeps_wal_users_learning_presentations_and_publications() {
    let f = MigrationFixture::new();
    let p = &f.plan;
    migrate(p, true).unwrap();
    {
        let mut users = UserRegistry::open(&p.service_root).unwrap();
        let b = users.get("B", "2").unwrap();
        b.lock()
            .unwrap()
            .learning_store()
            .unwrap()
            .mutate(
                &memory::learning::TutorMutation {
                    operation_id: "B-op".into(),
                    expected_revision: 0,
                    action: memory::learning::TutorAction::SetEnabled { enabled: true },
                },
                "2",
            )
            .unwrap();
    }
    let snapshot = f._root.path().join("service-backup");
    // A read-only held connection keeps the final committed WAL present after backup.
    let wal = rusqlite::Connection::open(p.service_root.join("control.sqlite")).unwrap();
    wal.execute_batch("PRAGMA wal_autocheckpoint=0; INSERT INTO users(user_id) VALUES('wal-user')")
        .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(f._root.path(), fs::Permissions::from_mode(0o755)).unwrap();
    }
    backup_service(&p.service_root, &snapshot).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(f._root.path()).unwrap().permissions().mode() & 0o777,
            0o755
        );
        assert_eq!(
            fs::metadata(&snapshot).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }
    let restored = f._root.path().join("restored");
    restore_service(&snapshot, &restored).unwrap();
    let mut users = UserRegistry::open(&restored).unwrap();
    assert!(users.paths("wal-user").is_ok());
    for owner in ["A", "B"] {
        assert!(
            users
                .get(owner, "3")
                .unwrap()
                .lock()
                .unwrap()
                .learning_store()
                .unwrap()
                .state()
                .unwrap()
                .control
                .enabled
        );
    }
    let control = ControlStore::open(users.writer()).unwrap();
    let mut library = PublishedLibrary::new(control);
    let publication = library.load("A", &p.books[0].publication).unwrap();
    assert!(publication
        .directory()
        .starts_with(restored.canonicalize().unwrap()));
    assert_eq!(
        fs::read(publication.directory().join("source.txt")).unwrap(),
        fs::read(p.books[0].source_dir.join("source.txt")).unwrap()
    );
    assert_eq!(
        fs::read(
            restored.join("users/A/memory/agent-history.presentations/versions/pres-old/1.json")
        )
        .unwrap(),
        fs::read(
            p.memory_dir
                .join("agent-history.presentations/versions/pres-old/1.json")
        )
        .unwrap()
    );
    let a = users.get("A", "3").unwrap();
    let a = a.lock().unwrap();
    let presentation = crate::presentation_store::PresentationStore::for_user(&a)
        .unwrap()
        .read_version(
            &runtime::presentation::PresentationOwner {
                book_id: "mu9-book".into(),
                session_id: "chat-old".into(),
            },
            &runtime::presentation::PresentationRef {
                presentation_id: "pres-old".into(),
                revision: 1,
            },
        )
        .unwrap();
    assert_eq!(presentation.content.initial_state, json!({"slider":2}));
    assert_eq!(
        a.learning_store()
            .unwrap()
            .evidence("evidence-old")
            .unwrap()
            .session_id,
        "chat-old"
    );
    assert!(restore_service(&snapshot, &p.service_root).is_err());
    assert!(restore_service(&snapshot, &restored).is_err());
    assert!(backup_service(&p.service_root, &snapshot).is_err());
}

#[test]
fn mu9_t63_export_is_an_independent_single_user_root_and_original_backup_restores() {
    let f = MigrationFixture::new();
    let p = &f.plan;
    migrate(p, true).unwrap();
    let target = f._root.path().join("old-binary-export");
    export_user(&p.service_root, "A", &target).unwrap();
    assert!(!target.join("users").exists());
    assert!(!target.join("control.sqlite").exists());
    let store = MemoryStore::open_private(target.join("memory/memory.json")).unwrap();
    assert_eq!(store.recall(&RecallQuery::default())[0].mem_id, "note-old");
    assert!(
        store
            .learning_store()
            .unwrap()
            .state()
            .unwrap()
            .control
            .enabled
    );
    let session = load_session(&Some(target.join("memory/session.json"))).unwrap();
    assert!(Path::new(&session.current_book_dir)
        .canonicalize()
        .unwrap()
        .starts_with(target.canonicalize().unwrap()));
    assert!(Book::load(&session.current_book_dir).is_ok());
    assert_eq!(session.current_top_lid(), Some("1.1"));
    assert_eq!(
        fs::read(target.join("private/accepted/intent/artifact/result.txt")).unwrap(),
        fs::read(p.private_dir.join("accepted/intent/artifact/result.txt")).unwrap()
    );
    let before = f._root.path().join("pre-import-restored");
    restore_service(&p.backup_dir.join("snapshot"), &before).unwrap();
    let mut users = UserRegistry::open(&before).unwrap();
    assert!(users
        .get("A", "2")
        .unwrap()
        .lock()
        .unwrap()
        .store
        .recall(&RecallQuery::default())
        .is_empty());
    assert_eq!(
        fs::read(p.backup_dir.join("snapshot/memory/memory.json")).unwrap(),
        fs::read(p.memory_dir.join("memory.json")).unwrap()
    );
}

#[test]
fn mu9_invalid_source_mapping_schema_and_overlapping_roots_do_not_enter_maintenance() {
    let f = MigrationFixture::new();
    let p = &f.plan;
    let mut invalid = p.clone();
    invalid.backup_dir = p.service_root.join("backup");
    assert!(migrate(&invalid, true)
        .unwrap_err()
        .to_string()
        .contains("independent"));
    let mut invalid = p.clone();
    invalid.books[0].legacy_dir = "missing".into();
    assert!(migration_preview(&invalid)
        .unwrap_err()
        .to_string()
        .contains("Unmapped session"));
    fs::write(
        p.books[0].source_dir.join("source.txt"),
        "different content",
    )
    .unwrap();
    assert!(migrate(p, true)
        .unwrap_err()
        .to_string()
        .contains("source differs"));
    assert!(!p.service_root.join("maintenance.json").exists());
    let db = rusqlite::Connection::open(p.service_root.join("control.sqlite")).unwrap();
    db.pragma_update(None, "user_version", 999).unwrap();
    drop(db);
    assert!(migration_preview(p)
        .unwrap_err()
        .to_string()
        .contains("compatible"));
    assert!(!p.backup_dir.exists());
}

#[test]
fn mu9_t62_interrupted_restore_resumes_and_keeps_snapshot_immutable() {
    let f = MigrationFixture::new();
    let p = &f.plan;
    migrate(p, true).unwrap();
    let snapshot = f._root.path().join("service-backup");
    backup_service(&p.service_root, &snapshot).unwrap();
    let before = fs::read(snapshot.join("service/control.sqlite")).unwrap();
    let restored = f._root.path().join("restore-retry");
    assert!(restore_inner(&snapshot, &restored, true).is_err());
    assert!(
        matches!(ServiceWriter::acquire(&restored),Err(e) if e.error_code=="SERVICE_MAINTENANCE_INCOMPLETE")
    );
    restore_service(&snapshot, &restored).unwrap();
    let mut users = UserRegistry::open(&restored).unwrap();
    assert_eq!(
        users
            .get("A", "now")
            .unwrap()
            .lock()
            .unwrap()
            .store
            .recall(&RecallQuery::default())[0]
            .mem_id,
        "note-old"
    );
    assert_eq!(
        fs::read(snapshot.join("service/control.sqlite")).unwrap(),
        before
    );
}

#[test]
fn mu9_unknown_learning_source_and_existing_backup_are_rejected_before_writes() {
    let f = MigrationFixture::new();
    let p = &f.plan;
    fs::create_dir(&p.backup_dir).unwrap();
    fs::write(p.backup_dir.join("keep.txt"), "existing backup").unwrap();
    assert!(migrate(p, true)
        .unwrap_err()
        .to_string()
        .contains("new backup directory"));
    assert!(!p.service_root.join("maintenance.json").exists());
    let db = rusqlite::Connection::open(p.memory_dir.join("learning.db")).unwrap();
    db.execute(
        "UPDATE learning_evidence SET source_id='unmapped-source'",
        [],
    )
    .unwrap();
    drop(db);
    assert!(migration_preview(p)
        .unwrap_err()
        .to_string()
        .contains("Unmapped book_id"));
    assert_eq!(
        fs::read_to_string(p.backup_dir.join("keep.txt")).unwrap(),
        "existing backup"
    );
}

#[test]
fn mu9_learning_only_material_and_changed_pdf_cannot_be_silently_bound() {
    let f = MigrationFixture::new();
    let p = &f.plan;
    let store = MemoryStore::open_private(p.memory_dir.join("memory.json")).unwrap();
    store
        .learning_store()
        .unwrap()
        .mutate(
            &memory::learning::TutorMutation {
                operation_id: "learning-only".into(),
                expected_revision: 1,
                action: memory::learning::TutorAction::Start {
                    user_intent: "study another material".into(),
                    explicit_constraints: vec![],
                    material_scope: vec![memory::learning::TutorMaterial {
                        source_id: "learning-only-book".into(),
                        scope_refs: vec![],
                        role: memory::learning::TutorMaterialRole::Primary,
                    }],
                    default_teaching_intent: None,
                },
            },
            "3",
        )
        .unwrap();
    drop(store);
    assert!(migration_preview(p)
        .unwrap_err()
        .to_string()
        .contains("learning-only-book"));
    let f = MigrationFixture::new();
    let p = &f.plan;
    let book = Book::load(p.books[0].source_dir.to_str().unwrap()).unwrap();
    fs::write(
        p.books[0].source_dir.join("paper.pdf"),
        b"legacy PDF source",
    )
    .unwrap();
    fs::write(p.books[0].source_dir.join("source_manifest.json"),json!({"version":"source_manifest.v2","book_id":"mu9-book","canonical_source":{"path":"source.txt","sha256":book.source_fingerprint()},"original_pdf":{"path":"paper.pdf","sha256":sha256_hex(b"legacy PDF source")},"capabilities":{"view_pdf":{"status":"available"},"project_lid_to_pdf":{"status":"unavailable"}}}).to_string()).unwrap();
    assert!(migration_preview(p)
        .unwrap_err()
        .to_string()
        .contains("PDF binding differs"));
    assert!(!p.service_root.join("maintenance.json").exists());
}
