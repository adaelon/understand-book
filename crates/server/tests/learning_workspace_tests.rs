use base_schema::{PublishedBookRef, WorkspaceGoalInput};
use rusqlite::{params, Connection};
use server::control_store::{ControlStore, ServiceWriter};

fn goal(text: &str) -> WorkspaceGoalInput {
    WorkspaceGoalInput {
        text: text.into(),
        constraints: vec![],
    }
}

fn reference(book: &str, publication: &str) -> PublishedBookRef {
    PublishedBookRef {
        book_id: book.into(),
        publication_id: publication.into(),
    }
}

struct Fixture {
    root: tempfile::TempDir,
    control: ControlStore,
    db: Connection,
    first: PublishedBookRef,
    second: PublishedBookRef,
    ungranted: PublishedBookRef,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let writer = ServiceWriter::acquire(root.path()).unwrap();
        let mut control = ControlStore::open(writer).unwrap();
        control.create_user("A").unwrap();
        control.create_user("B").unwrap();
        let db = Connection::open(root.path().join("control.sqlite")).unwrap();
        db.pragma_update(None, "foreign_keys", "ON").unwrap();
        let first = reference("linear-algebra", "v1");
        let second = reference("deep-learning", "v2");
        let ungranted = reference("deep-learning", "v3");
        for item in [&first, &second, &ungranted] {
            db.execute(
                "INSERT INTO book_publications(book_id,publication_id,directory,manifest) VALUES(?,?,?,?)",
                params![item.book_id, item.publication_id, format!("{}/{}", item.book_id, item.publication_id), "{}"],
            ).unwrap();
        }
        for item in [&first, &second] {
            db.execute(
                "INSERT INTO book_grants(owner_user_id,book_id,publication_id) VALUES('A',?,?)",
                params![item.book_id, item.publication_id],
            )
            .unwrap();
        }
        Self {
            root,
            control,
            db,
            first,
            second,
            ungranted,
        }
    }
}

#[test]
fn w1_required_goal_and_exact_two_materials_survive_reopen() {
    let mut fixture = Fixture::new();
    for text in ["", " \t\n", "\u{3000}"] {
        assert_eq!(
            fixture
                .control
                .create_learning_workspace("A", goal(text), vec![], 1)
                .unwrap_err()
                .error_code,
            "WORKSPACE_GOAL_REQUIRED"
        );
    }
    assert!(fixture
        .control
        .list_learning_workspaces("A")
        .unwrap()
        .is_empty());
    let original = fixture
        .control
        .create_learning_workspace(
            "A",
            goal("学习深度学习"),
            vec![fixture.first.clone(), fixture.second.clone()],
            2,
        )
        .unwrap();
    assert_eq!(original.members.len(), 2);
    assert!(original.members.iter().all(|member| member.readable));
    let id = original.learning_workspace_id.clone();
    drop(fixture.control);
    drop(fixture.db);
    let writer = ServiceWriter::acquire(fixture.root.path()).unwrap();
    let control = ControlStore::open(writer).unwrap();
    assert_eq!(control.learning_workspace("A", &id).unwrap(), original);
    let refs: Vec<_> = original
        .members
        .iter()
        .map(|member| &member.published_book_ref)
        .collect();
    assert!(refs.contains(&&fixture.first));
    assert!(refs.contains(&&fixture.second));
}

#[test]
fn w1_goal_revisions_preserve_accepted_wording_and_constraints() {
    let mut fixture = Fixture::new();
    let first = fixture
        .control
        .create_learning_workspace(
            "A",
            WorkspaceGoalInput {
                text: " 学习深度学习 ".into(),
                constraints: vec!["每周四小时".into()],
            },
            vec![],
            1,
        )
        .unwrap();
    let id = &first.learning_workspace_id;
    let revised = fixture
        .control
        .revise_learning_workspace_goal(
            "A",
            id,
            WorkspaceGoalInput {
                text: "期末考试冲刺".into(),
                constraints: vec!["只剩三天".into(), "以考纲为准".into()],
            },
            2,
        )
        .unwrap();
    assert_eq!(revised.goal.revision, 2);
    assert_eq!(revised.goal.text, "期末考试冲刺");
    assert_eq!(revised.goal_history[0], first.goal);
    assert_eq!(revised.goal_history[1], revised.goal);
    assert_eq!(revised.goal_history[0].text, " 学习深度学习 ");
    assert_eq!(
        fixture
            .control
            .revise_learning_workspace_goal("A", id, goal(" "), 3)
            .unwrap_err()
            .error_code,
        "WORKSPACE_GOAL_REQUIRED"
    );
    assert_eq!(
        fixture.control.learning_workspace("A", id).unwrap(),
        revised
    );
}

#[test]
fn w1_materials_require_exact_grants_and_revoke_removes_readability() {
    let mut fixture = Fixture::new();
    let denied = fixture
        .control
        .create_learning_workspace(
            "A",
            goal("深度学习"),
            vec![fixture.first.clone(), fixture.ungranted.clone()],
            1,
        )
        .unwrap_err();
    assert_eq!(denied.error_code, "WORKSPACE_MATERIAL_NOT_AUTHORIZED");
    assert!(fixture
        .control
        .list_learning_workspaces("A")
        .unwrap()
        .is_empty());
    let workspace = fixture
        .control
        .create_learning_workspace("A", goal("深度学习"), vec![], 2)
        .unwrap();
    let id = &workspace.learning_workspace_id;
    fixture
        .control
        .add_learning_workspace_member("A", id, &fixture.second, 3)
        .unwrap();
    let repeated = fixture
        .control
        .add_learning_workspace_member("A", id, &fixture.second, 4)
        .unwrap();
    assert_eq!(repeated.members.len(), 1);
    assert_eq!(repeated.members[0].added_at, 3);
    assert_eq!(
        fixture
            .control
            .add_learning_workspace_member("A", id, &fixture.ungranted, 5)
            .unwrap_err()
            .error_code,
        "WORKSPACE_MATERIAL_NOT_AUTHORIZED"
    );
    fixture
        .db
        .execute(
            "DELETE FROM book_grants WHERE owner_user_id='A' AND book_id=? AND publication_id=?",
            params![fixture.second.book_id, fixture.second.publication_id],
        )
        .unwrap();
    let revoked = fixture.control.learning_workspace("A", id).unwrap();
    assert_eq!(revoked.members[0].published_book_ref, fixture.second);
    assert!(!revoked.members[0].readable);
    let removed = fixture
        .control
        .remove_learning_workspace_member("A", id, &fixture.second, 6)
        .unwrap();
    assert!(removed.members.is_empty());
}

#[test]
fn w1_other_account_cannot_read_or_mutate_workspace() {
    let mut fixture = Fixture::new();
    let original = fixture
        .control
        .create_learning_workspace("A", goal("期末考试冲刺"), vec![fixture.first.clone()], 1)
        .unwrap();
    let id = &original.learning_workspace_id;
    let errors = [
        fixture.control.learning_workspace("B", id).unwrap_err(),
        fixture
            .control
            .revise_learning_workspace_goal("B", id, goal("替换目标"), 2)
            .unwrap_err(),
        fixture
            .control
            .add_learning_workspace_member("B", id, &fixture.second, 2)
            .unwrap_err(),
        fixture
            .control
            .remove_learning_workspace_member("B", id, &fixture.first, 2)
            .unwrap_err(),
        fixture
            .control
            .set_learning_workspace_archived("B", id, true, 2)
            .unwrap_err(),
    ];
    assert!(errors
        .iter()
        .all(|error| error.error_code == "LEARNING_WORKSPACE_NOT_FOUND"));
    assert!(fixture
        .control
        .list_learning_workspaces("B")
        .unwrap()
        .is_empty());
    assert_eq!(
        fixture.control.learning_workspace("A", id).unwrap(),
        original
    );
}

#[test]
fn w1_archive_preserves_history_and_requires_restore_for_changes() {
    let mut fixture = Fixture::new();
    let original = fixture
        .control
        .create_learning_workspace("A", goal("深度学习"), vec![fixture.first.clone()], 1)
        .unwrap();
    let id = &original.learning_workspace_id;
    let archived = fixture
        .control
        .set_learning_workspace_archived("A", id, true, 2)
        .unwrap();
    assert!(archived.archived);
    assert_eq!(archived.goal_history, original.goal_history);
    assert_eq!(archived.members, original.members);
    assert_eq!(
        fixture.control.list_learning_workspaces("A").unwrap(),
        vec![archived]
    );
    assert_eq!(
        fixture
            .control
            .revise_learning_workspace_goal("A", id, goal("新目标"), 3)
            .unwrap_err()
            .error_code,
        "LEARNING_WORKSPACE_ARCHIVED"
    );
    assert_eq!(
        fixture
            .control
            .add_learning_workspace_member("A", id, &fixture.second, 3)
            .unwrap_err()
            .error_code,
        "LEARNING_WORKSPACE_ARCHIVED"
    );
    assert_eq!(
        fixture
            .control
            .remove_learning_workspace_member("A", id, &fixture.first, 3)
            .unwrap_err()
            .error_code,
        "LEARNING_WORKSPACE_ARCHIVED"
    );
    fixture
        .control
        .set_learning_workspace_archived("A", id, false, 4)
        .unwrap();
    let restored = fixture
        .control
        .revise_learning_workspace_goal("A", id, goal("期末冲刺"), 5)
        .unwrap();
    assert!(!restored.archived);
    assert_eq!(restored.goal.revision, 2);
}
