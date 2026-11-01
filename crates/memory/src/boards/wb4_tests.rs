use super::tests::{answer, document};
use super::*;
use serde_json::json;

type Authored = (
    Vec<BoardItem>,
    Vec<BoardLink>,
    Vec<BoardExplanationMapping>,
    Vec<BoardRoute>,
);

fn follow_up() -> BoardFollowUp {
    BoardFollowUp {
        note_refs: None,
        board_id: "board-1".into(),
        board_revision: 1,
        item_id: Some("answer-1".into()),
        block_id: Some("block-answer-1".into()),
        presentation: None,
        intent: None,
        regenerate_from: None,
    }
}

fn authored(part: &str, revision: u32) -> Authored {
    let source = BoardContentRef {
        item_id: "answer-1".into(),
        block_id: Some("block-answer-1".into()),
        content_revision: 1,
    };
    let mut explanation = answer();
    explanation.item_id = format!("answer-{part}");
    explanation.readable_content = format!("Complete explanation for {part}.");
    explanation.position = BoardPosition { x: 720.0, y: 320.0 };
    explanation.target = BoardTarget::Answer {
        session_id: "chat-2".into(),
        turn_id: "turn-2".into(),
        question: "Explain the example in complete parts".into(),
        answer_parts: vec![json!({"kind":"markdown","text":explanation.readable_content})],
    };
    explanation.explanation_blocks = vec![BoardExplanationBlock {
        block_id: format!("explanation-{part}"),
        markdown: format!("The worked detail for {part}."),
        answer_parts: vec![
            json!({"kind":"markdown","text":format!("The worked detail for {part}.")}),
        ],
    }];
    let mut drawing = answer();
    drawing.item_id = format!("drawing-{part}");
    drawing.position = BoardPosition {
        x: 1160.0,
        y: 320.0,
    };
    drawing.target = BoardTarget::Drawing {
        session_id: "chat-2".into(),
        turn_id: "turn-2".into(),
        based_on: vec![source.clone()],
        graphic: BoardGraphic::Text {
            text: format!("A diagram for {part}"),
        },
    };
    let visual = BoardVisualTarget::Item {
        item_id: drawing.item_id.clone(),
    };
    let detail = BoardContentRef {
        item_id: explanation.item_id.clone(),
        block_id: Some(format!("explanation-{part}")),
        content_revision: revision,
    };
    (
        vec![explanation, drawing],
        vec![BoardLink {
            note_relation_id: None, reason: None,
            link_id: format!("relation-{part}"),
            from: BoardVisualTarget::Item {
                item_id: "answer-1".into(),
            },
            to: visual.clone(),
            label: format!("explained by {part}"),
            based_on: vec![source],
        }],
        vec![BoardExplanationMapping {
            mapping_id: format!("mapping-{part}"),
            explanation: detail.clone(),
            visual: visual.clone(),
        }],
        vec![BoardRoute {
            route_id: format!("route-{part}"),
            title: format!("Walk through {part}"),
            steps: vec![BoardRouteStep {
                step_id: format!("step-{part}"),
                explanation: Some(detail),
                reveal_targets: vec![visual],
                focus_targets: vec![BoardVisualTarget::Link {
                    link_id: format!("relation-{part}"),
                }],
                transition: BoardTransition::RevealText,
                duration_ms: 400,
            }],
        }],
    )
}

fn append(
    store: &BoardStore,
    expected_revision: u32,
    commit_id: &str,
    content: Authored,
) -> Result<BoardDelivery, ToolError> {
    store.append_authored_part(
        &follow_up(),
        expected_revision,
        "chat-2",
        "turn-2",
        commit_id,
        content.0,
        vec![],
        content.1,
        content.2,
        content.3,
        "part-committed",
    )
}

fn append_terminal(
    store: &BoardStore,
    expected_revision: u32,
    content: Authored,
) -> Result<BoardDelivery, ToolError> {
    store.append_authored_delivery(
        &follow_up(),
        expected_revision,
        "chat-2",
        "turn-2",
        content.0,
        vec![],
        content.1,
        content.2,
        content.3,
        "completed",
    )
}

#[test]
fn two_parts_of_one_turn_reopen_with_ordered_receipts_and_complete_graphs() {
    let temp = tempfile::tempdir().unwrap();
    let store = BoardStore::open(temp.path(), None, false).unwrap();
    let initial = store.create(document(None)).unwrap();
    let first = append(&store, 1, "board:turn-2:1", authored("one", 2)).unwrap();
    let after_first = store.read("board-1").unwrap();
    let second = append(&store, 2, "board:turn-2:2", authored("two", 3)).unwrap();
    assert_eq!(first.commit_id.as_deref(), Some("board:turn-2:1"));
    assert_eq!(second.commit_id.as_deref(), Some("board:turn-2:2"));
    assert_eq!((first.before_revision, first.after_revision), (1, 2));
    assert_eq!((second.before_revision, second.after_revision), (2, 3));
    assert_eq!(first.item_ids, vec!["answer-one", "drawing-one"]);
    assert_eq!(second.item_ids, vec!["answer-two", "drawing-two"]);
    assert_eq!(first.block_ids, vec!["block-answer-one", "explanation-one"]);
    assert_eq!(
        second.block_ids,
        vec!["block-answer-two", "explanation-two"]
    );
    let saved = store.read("board-1").unwrap();
    assert_eq!(saved.revision, 3);
    assert_eq!(saved.items.len(), 5);
    assert_eq!(saved.items[0], initial.items[0]);
    assert_eq!(saved.items[..3], after_first.items);
    assert_eq!(saved.links.len(), 2);
    assert_eq!(saved.mappings.len(), 2);
    assert_eq!(saved.routes.len(), 2);
    assert_eq!(saved.links[0], after_first.links[0]);
    assert_eq!(saved.mappings[0], after_first.mappings[0]);
    assert_eq!(saved.routes[0], after_first.routes[0]);
    assert_eq!(
        resolve_explanation_block(&saved.items[3], "block-answer-two")
            .unwrap()
            .markdown,
        "Complete explanation for two."
    );
    assert_eq!(
        resolve_explanation_block(&saved.items[3], "explanation-two")
            .unwrap()
            .markdown,
        "The worked detail for two."
    );
    let reopened = BoardStore::open(temp.path(), None, false).unwrap();
    assert_eq!(reopened.read("board-1").unwrap(), saved);
    assert_eq!(reopened.read_revision("board-1", 2).unwrap(), after_first);
    assert_eq!(
        reopened
            .deliveries_for_turn("board-1", "chat-2", "turn-2")
            .unwrap(),
        vec![first, second]
    );
    for (session_id, turn_id) in [("chat-other", "turn-2"), ("chat-2", "turn-other")] {
        assert!(reopened
            .deliveries_for_turn("board-1", session_id, turn_id)
            .unwrap()
            .is_empty());
    }
}

#[test]
fn a_part_retry_returns_its_own_receipt_after_later_parts_and_visible_deletion() {
    let temp = tempfile::tempdir().unwrap();
    let store = BoardStore::open(temp.path(), None, false).unwrap();
    store.create(document(None)).unwrap();
    let first = append(&store, 1, "candidate-one", authored("one", 2)).unwrap();
    let second = append(&store, 2, "candidate-two", authored("two", 3)).unwrap();
    let saved = store.read("board-1").unwrap();
    let empty = || (vec![], vec![], vec![], vec![]);
    assert_eq!(append(&store, 1, "candidate-one", empty()).unwrap(), first);
    assert_eq!(append(&store, 1, "candidate-two", empty()).unwrap(), second);
    assert_eq!(store.read("board-1").unwrap(), saved);
    let mut removed = saved;
    removed.items.clear();
    let removed = store.update(removed, 3, "user-removed-content").unwrap();
    assert!(removed.links.is_empty() && removed.mappings.is_empty() && removed.routes.is_empty());
    let reopened = BoardStore::open(temp.path(), None, false).unwrap();
    assert_eq!(
        append(&reopened, 1, "candidate-one", empty()).unwrap(),
        first
    );
    assert_eq!(
        append(&reopened, 2, "candidate-two", empty()).unwrap(),
        second
    );
    assert_eq!(reopened.read("board-1").unwrap(), removed);
    assert_eq!(
        reopened
            .deliveries_for_turn("board-1", "chat-2", "turn-2")
            .unwrap(),
        vec![first, second]
    );
    assert_eq!(
        append(&reopened, 4, "candidate-three", authored("three", 5))
            .unwrap_err()
            .error_code,
        "BOARD_NOT_FOUND"
    );
    assert_eq!(reopened.read("board-1").unwrap(), removed);
    reopened.remove("board-1", 4).unwrap();
    assert_eq!(
        append(&reopened, 1, "candidate-one", empty())
            .unwrap_err()
            .error_code,
        "BOARD_NOT_FOUND"
    );
}

#[test]
fn a_fresh_part_after_a_drag_preserves_all_existing_positions() {
    let temp = tempfile::tempdir().unwrap();
    let store = BoardStore::open(temp.path(), None, false).unwrap();
    store.create(document(None)).unwrap();
    append(&store, 1, "candidate-one", authored("one", 2)).unwrap();
    let prepared_before_drag = authored("two", 3);
    let mut moved = store.read("board-1").unwrap();
    moved.items[0].position = BoardPosition { x: 144.5, y: 91.25 };
    moved.items[2].position = BoardPosition {
        x: 2100.5,
        y: -81.25,
    };
    let moved = store.update(moved, 2, "user-dragged").unwrap();
    assert_eq!(
        append(&store, 2, "candidate-two", prepared_before_drag)
            .unwrap_err()
            .error_code,
        "BOARD_REVISION_CONFLICT"
    );
    assert_eq!(store.read("board-1").unwrap(), moved);
    let mut refreshed = authored("two", 4);
    refreshed.0[0].position = BoardPosition { x: 720.0, y: 760.0 };
    refreshed.0[1].position = BoardPosition {
        x: 1160.0,
        y: 760.0,
    };
    let requested_positions: Vec<_> = refreshed
        .0
        .iter()
        .map(|item| item.position.clone())
        .collect();
    append(&store, 3, "candidate-two", refreshed).unwrap();
    let saved = store.read("board-1").unwrap();
    assert_eq!(saved.items[..3], moved.items);
    assert_eq!(saved.items[3].position, requested_positions[0]);
    assert_eq!(saved.items[4].position, requested_positions[1]);
    assert_eq!(saved.links[0], moved.links[0]);
    assert_eq!(saved.mappings[0], moved.mappings[0]);
    assert_eq!(saved.routes[0], moved.routes[0]);
}

#[test]
fn a_failed_part_publish_exposes_no_partial_graph_or_receipt_and_can_retry() {
    let temp = tempfile::tempdir().unwrap();
    let store = BoardStore::open(temp.path(), None, false).unwrap();
    store.create(document(None)).unwrap();
    let first = append(&store, 1, "candidate-one", authored("one", 2)).unwrap();
    let saved = store.read("board-1").unwrap();
    let blocker = store
        .board_path("board-1")
        .unwrap()
        .join("head.replace.tmp");
    std::fs::create_dir(&blocker).unwrap();
    assert!(append(&store, 2, "candidate-two", authored("two", 3)).is_err());
    assert_eq!(store.read("board-1").unwrap(), saved);
    assert!(store.read_revision("board-1", 3).is_err());
    assert_eq!(
        store
            .deliveries_for_turn("board-1", "chat-2", "turn-2")
            .unwrap(),
        vec![first.clone()]
    );
    std::fs::remove_dir(&blocker).unwrap();
    let second = append(&store, 2, "candidate-two", authored("two", 3)).unwrap();
    let reopened = BoardStore::open(temp.path(), None, false).unwrap();
    let saved = reopened.read("board-1").unwrap();
    assert_eq!(saved.revision, 3);
    assert_eq!(saved.items.len(), 5);
    assert_eq!(saved.links.len(), 2);
    assert_eq!(saved.mappings.len(), 2);
    assert_eq!(saved.routes.len(), 2);
    assert_eq!(saved.deliveries, vec![first, second]);
}

#[test]
fn a_terminal_append_cannot_duplicate_a_turn_that_already_delivered_parts() {
    let temp = tempfile::tempdir().unwrap();
    let store = BoardStore::open(temp.path(), None, false).unwrap();
    store.create(document(None)).unwrap();
    append(&store, 1, "candidate-one", authored("one", 2)).unwrap();
    let saved = store.read("board-1").unwrap();
    assert_eq!(
        store
            .append_delivery(
                &follow_up(),
                "chat-2",
                "turn-2",
                authored("final", 3).0,
                "completed"
            )
            .unwrap_err()
            .error_code,
        "INVALID_BOARD"
    );
    assert_eq!(
        append_terminal(&store, 2, authored("final", 3))
            .unwrap_err()
            .error_code,
        "INVALID_BOARD"
    );
    assert_eq!(store.read("board-1").unwrap(), saved);
}

#[test]
fn legacy_terminal_receipts_deserialize_and_retry_but_reject_new_parts() {
    for use_authored_terminal in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let store = BoardStore::open(temp.path(), None, false).unwrap();
        store.create(document(None)).unwrap();
        let receipt = if use_authored_terminal {
            append_terminal(&store, 1, authored("legacy", 2)).unwrap()
        } else {
            store
                .append_delivery(
                    &follow_up(),
                    "chat-2",
                    "turn-2",
                    authored("legacy", 2).0,
                    "completed",
                )
                .unwrap()
        };
        assert!(receipt.commit_id.is_none());
        assert!(serde_json::to_value(&receipt)
            .unwrap()
            .get("commit_id")
            .is_none());
        let snapshot_path = store
            .board_path("board-1")
            .unwrap()
            .join("revisions/2.json");
        let mut legacy: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&snapshot_path).unwrap()).unwrap();
        legacy["deliveries"][0]
            .as_object_mut()
            .unwrap()
            .remove("commit_id");
        std::fs::write(&snapshot_path, serde_json::to_vec(&legacy).unwrap()).unwrap();
        let reopened = BoardStore::open(temp.path(), None, false).unwrap();
        let saved = reopened.read("board-1").unwrap();
        assert_eq!(saved.deliveries, vec![receipt.clone()]);
        assert_eq!(
            reopened
                .deliveries_for_turn("board-1", "chat-2", "turn-2")
                .unwrap(),
            vec![receipt.clone()]
        );
        assert_eq!(
            reopened
                .append_delivery(&follow_up(), "chat-2", "turn-2", vec![], "retry")
                .unwrap(),
            receipt
        );
        assert_eq!(
            append_terminal(&reopened, 1, (vec![], vec![], vec![], vec![])).unwrap(),
            receipt
        );
        assert_eq!(
            append(&reopened, 2, "candidate-next", authored("next", 3))
                .unwrap_err()
                .error_code,
            "INVALID_BOARD"
        );
        assert_eq!(reopened.read("board-1").unwrap(), saved);
    }
}

#[test]
fn an_empty_commit_id_is_rejected_without_publishing_content() {
    let temp = tempfile::tempdir().unwrap();
    let store = BoardStore::open(temp.path(), None, false).unwrap();
    let initial = store.create(document(None)).unwrap();
    assert_eq!(
        append(&store, 1, "", authored("one", 2))
            .unwrap_err()
            .error_code,
        "INVALID_BOARD"
    );
    assert_eq!(store.read("board-1").unwrap(), initial);
    assert!(store
        .deliveries_for_turn("board-1", "chat-2", "turn-2")
        .unwrap()
        .is_empty());
}
