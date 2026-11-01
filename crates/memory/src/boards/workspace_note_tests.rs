use super::*;
use crate::workspace_notes::{WorkspaceNote, WorkspaceNoteAuthor, WorkspaceNoteRelation, WorkspaceNoteStore};

const NOW: &str = "2026-10-27T10:00:00Z";

fn note(id: &str) -> WorkspaceNote {
    WorkspaceNote {
        note_id: id.into(), learning_workspace_id: Some("learning".into()),
        title: format!("My {id}"), content: "My original explanation".into(), revision: 1,
        created_at: NOW.into(), updated_at: NOW.into(), author: WorkspaceNoteAuthor::User,
        updated_by: WorkspaceNoteAuthor::User,
        origin: None, source_bindings: vec![],
    }
}

fn card(id: &str) -> BoardItem {
    let mut item = super::tests::answer();
    item.item_id = format!("card-{id}");
    item.title = "The import preview".into();
    item.readable_content = "A transient imported body".into();
    item.source_bindings.clear();
    item.target = BoardTarget::Note { note_id: id.into(), missing: false };
    item
}

#[test]
fn shared_note_references_refresh_across_books_and_layout_saves_without_rewriting_answers() {
    let root = tempfile::tempdir().unwrap();
    let notes = WorkspaceNoteStore::open(root.path(), Some("learning"), false).unwrap();
    notes.create(note("one")).unwrap();
    let boards = BoardStore::open(root.path(), Some("learning"), false).unwrap();
    let mut first = super::tests::document(Some("learning"));
    first.items.push(card("one"));
    let mut first = boards.create(first).unwrap();
    let retained_answer = first.items[0].clone();
    let mut second = super::tests::document(Some("learning"));
    second.board_id = "board-two".into();
    second.book_id = "music".into();
    second.items = vec![card("one")];
    boards.create(second).unwrap();

    notes.update("one", 1, "Revised understanding", "A more precise explanation", WorkspaceNoteAuthor::User, NOW).unwrap();
    // An already-open board can save its layout after the shared note changed.
    first.items[1].position.x = 800.0;
    let saved = boards.update(first, 1, NOW).unwrap();
    assert_eq!(saved.items[0], retained_answer);
    assert_eq!(saved.items[1].position.x, 800.0);
    for (id, index) in [("board-1", 1), ("board-two", 0)] {
        let reopened = boards.read(id).unwrap();
        assert_eq!(reopened.items[index].title, "Revised understanding");
        assert_eq!(reopened.items[index].readable_content, "A more precise explanation");
        assert_eq!(boards.read_revision(id, 1).unwrap().items[index].readable_content, "A more precise explanation");
    }
    let path = root.path().join("boards/workspaces/learning/board-1/revisions/2.json");
    let stored: BoardDocument = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert!(stored.items[1].readable_content.is_empty());
    assert!(stored.items[1].source_bindings.is_empty());
    assert_eq!(stored.items[0], retained_answer);
    assert_eq!(boards.search("physics", Some("release-1"), "precise", 20).unwrap()[0].kind, BoardSearchKind::Note);
}

#[test]
fn shared_relations_follow_notebook_edits_and_deleted_notes_never_restore_stale_text() {
    let root = tempfile::tempdir().unwrap();
    let notes = WorkspaceNoteStore::open(root.path(), Some("learning"), false).unwrap();
    notes.create(note("one")).unwrap();
    notes.create(note("two")).unwrap();
    let relation = notes.upsert_relation(WorkspaceNoteRelation {
        relation_id: "rel-example".into(), from_note_id: "one".into(), to_note_id: "two".into(),
        label: "provides an example".into(), reason: "The practice log makes this principle concrete".into(),
        revision: 1, created_at: NOW.into(), updated_at: NOW.into(), author: WorkspaceNoteAuthor::User,
    }, None, NOW).unwrap();
    let boards = BoardStore::open(root.path(), Some("learning"), false).unwrap();
    let mut board = super::tests::document(Some("learning"));
    board.items = vec![card("one"), card("two")];
    let mut board = boards.create(board).unwrap();
    assert_eq!(board.links[0].note_relation_id.as_deref(), Some("rel-example"));
    assert_eq!(board.links[0].reason.as_deref(), Some("The practice log makes this principle concrete"));
    board.items[0].position.y = 600.0;
    let saved = boards.update(board, 1, NOW).unwrap();
    assert_eq!(saved.links.len(), 1, "layout saving must not duplicate the projected relation");
    let raw: BoardDocument = serde_json::from_slice(&std::fs::read(root.path().join("boards/workspaces/learning/board-1/revisions/2.json")).unwrap()).unwrap();
    assert!(raw.links.is_empty(), "the notebook is the only durable relation owner");
    let mut changed = relation;
    changed.label = "limits the claim".into();
    changed.reason = "The example only holds under this condition".into();
    notes.upsert_relation(changed, Some(1), NOW).unwrap();
    assert_eq!(boards.read("board-1").unwrap().links[0].label, "limits the claim");

    notes.remove("one", 1).unwrap();
    for revision in [1, 2] {
        let missing = boards.read_revision("board-1", revision).unwrap();
        assert!(matches!(missing.items[0].target, BoardTarget::Note { missing: true, .. }));
        assert!(missing.items[0].readable_content.is_empty());
        assert!(missing.links.is_empty());
    }
    // Saving a stale, previously hydrated board cannot resurrect its content.
    let saved = boards.update(saved, 2, NOW).unwrap();
    assert!(matches!(saved.items[0].target, BoardTarget::Note { missing: true, .. }));
    assert!(saved.items[0].readable_content.is_empty());
    assert!(saved.links.is_empty());
}

#[test]
fn native_note_links_cannot_create_a_second_relationship_truth() {
    let root = tempfile::tempdir().unwrap();
    let notes = WorkspaceNoteStore::open(root.path(), Some("learning"), false).unwrap();
    notes.create(note("one")).unwrap();
    notes.create(note("two")).unwrap();
    let boards = BoardStore::open(root.path(), Some("learning"), false).unwrap();
    let mut board = super::tests::document(Some("learning"));
    board.items = vec![card("one"), card("two")];
    board.links.push(BoardLink { link_id: "divergent-relation".into(),
        from: BoardVisualTarget::Item { item_id: "card-one".into() },
        to: BoardVisualTarget::Item { item_id: "card-two".into() }, label: "related".into(),
        based_on: vec![], note_relation_id: None, reason: None });
    assert!(boards.create(board).unwrap_err().message.contains("notebook relations"));
}
