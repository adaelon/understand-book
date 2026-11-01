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

fn authored(revision: u32) -> Authored {
    let source = BoardContentRef {
        item_id: "answer-1".into(),
        block_id: Some("block-answer-1".into()),
        content_revision: 1,
    };
    let mut explanation = answer();
    explanation.item_id = "answer-2".into();
    explanation.readable_content =
        "The exact full answer.\n\nA unique stopping-time explanation.".into();
    explanation.position = BoardPosition { x: 820.0, y: 310.0 };
    explanation.target = BoardTarget::Answer {
        session_id: "chat-2".into(),
        turn_id: "turn-2".into(),
        question: "Draw the complete explanation".into(),
        answer_parts: vec![json!({"kind":"markdown","text":explanation.readable_content})],
    };
    explanation.explanation_blocks = vec![BoardExplanationBlock {
        block_id: "stopping-time".into(),
        markdown: "A unique stopping-time explanation.".into(),
        answer_parts: vec![json!({"kind":"markdown","text":"A unique stopping-time explanation."})],
    }];
    let mut drawing = answer();
    drawing.item_id = "drawing-1".into();
    drawing.position = BoardPosition {
        x: 1260.0,
        y: 310.0,
    };
    drawing.target = BoardTarget::Drawing {
        session_id: "chat-2".into(),
        turn_id: "turn-2".into(),
        based_on: vec![source.clone()],
        graphic: BoardGraphic::Path {
            points: vec![
                BoardPoint { x: 0.0, y: 0.0 },
                BoardPoint { x: 120.0, y: 90.0 },
            ],
            color: "#2563eb".into(),
            width: 3.0,
        },
    };
    let visual = BoardVisualTarget::Item {
        item_id: "drawing-1".into(),
    };
    let block = BoardContentRef {
        item_id: "answer-2".into(),
        block_id: Some("stopping-time".into()),
        content_revision: revision,
    };
    (
        vec![explanation, drawing],
        vec![BoardLink {
            note_relation_id: None, reason: None,
            link_id: "relation-1".into(),
            from: BoardVisualTarget::Item {
                item_id: "answer-1".into(),
            },
            to: visual.clone(),
            label: "includes stopped time".into(),
            based_on: vec![source],
        }],
        vec![BoardExplanationMapping {
            mapping_id: "mapping-1".into(),
            explanation: block.clone(),
            visual: visual.clone(),
        }],
        vec![BoardRoute {
            route_id: "route-1".into(),
            title: "Why stopped time counts".into(),
            steps: vec![BoardRouteStep {
                step_id: "draw-stopped-time".into(),
                explanation: Some(block),
                reveal_targets: vec![visual],
                focus_targets: vec![BoardVisualTarget::Link {
                    link_id: "relation-1".into(),
                }],
                transition: BoardTransition::Draw,
                duration_ms: 600,
            }],
        }],
    )
}

fn append(store: &BoardStore, expected: u32, parts: Authored) -> Result<BoardDelivery, ToolError> {
    store.append_authored_delivery(
        &follow_up(),
        expected,
        "chat-2",
        "turn-2",
        parts.0,
        vec![],
        parts.1,
        parts.2,
        parts.3,
        "later",
    )
}

#[test]
fn authored_graph_blocks_and_playback_reopen_without_rewriting_the_full_answer() {
    let temp = tempfile::tempdir().unwrap();
    let store = BoardStore::open(temp.path(), None, false).unwrap();
    let original = store.create(document(None)).unwrap();
    let delivery = append(&store, 1, authored(2)).unwrap();
    assert_eq!(delivery.block_ids, vec!["block-answer-2", "stopping-time"]);
    let saved = store.read("board-1").unwrap();
    assert_eq!(saved.items[0], original.items[0]);
    assert_eq!(
        saved.items[1].position,
        BoardPosition { x: 820.0, y: 310.0 }
    );
    assert_eq!(
        resolve_explanation_block(&saved.items[1], "block-answer-2")
            .unwrap()
            .markdown,
        saved.items[1].readable_content
    );
    assert_eq!(
        resolve_explanation_block(&saved.items[1], "stopping-time")
            .unwrap()
            .markdown,
        "A unique stopping-time explanation."
    );
    let view = BoardView {
        selected_item_id: Some("drawing-1".into()),
        reading_item_id: Some("answer-2".into()),
        selected_block_id: Some("stopping-time".into()),
        playback: Some(BoardPlaybackState {
            route_id: "route-1".into(),
            step_id: Some("draw-stopped-time".into()),
            following: false,
            show_all: false,
        }),
        ..Default::default()
    };
    store.save_view("board-1", &view).unwrap();
    let reopened = BoardStore::open(temp.path(), None, false).unwrap();
    assert_eq!(reopened.read("board-1").unwrap(), saved);
    assert_eq!(reopened.view("board-1").unwrap(), view);
    let hits = reopened
        .search("physics", Some("release-1"), "stopping-time", 10)
        .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].block_id.as_deref(), Some("stopping-time"));
    assert_eq!(reopened.read_revision("board-1", 1).unwrap(), original);
}

#[test]
fn authored_projection_conflicts_after_layout_changes_and_keeps_the_new_user_position() {
    let temp = tempfile::tempdir().unwrap();
    let store = BoardStore::open(temp.path(), None, false).unwrap();
    let mut moved = store.create(document(None)).unwrap();
    moved.items[0].position.x = 444.0;
    let moved = store.update(moved, 1, "moved").unwrap();
    assert_eq!(
        append(&store, 1, authored(2)).unwrap_err().error_code,
        "BOARD_REVISION_CONFLICT"
    );
    assert_eq!(store.read("board-1").unwrap(), moved);
    append(&store, 2, authored(3)).unwrap();
    assert_eq!(store.read("board-1").unwrap().items[0].position.x, 444.0);
    let mut stale_move = moved;
    stale_move.items[0].position.y = 777.0;
    assert_eq!(
        store.update(stale_move, 2, "stale").unwrap_err().error_code,
        "BOARD_REVISION_CONFLICT"
    );
    assert_eq!(store.read("board-1").unwrap().links.len(), 1);
}

#[test]
fn removing_authored_targets_cleans_links_mappings_routes_and_saved_selection() {
    let temp = tempfile::tempdir().unwrap();
    let store = BoardStore::open(temp.path(), None, false).unwrap();
    store.create(document(None)).unwrap();
    let receipt = append(&store, 1, authored(2)).unwrap();
    store
        .save_view(
            "board-1",
            &BoardView {
                selected_item_id: Some("drawing-1".into()),
                reading_item_id: Some("answer-2".into()),
                selected_block_id: Some("stopping-time".into()),
                playback: Some(BoardPlaybackState {
                    route_id: "route-1".into(),
                    step_id: Some("draw-stopped-time".into()),
                    following: true,
                    show_all: false,
                }),
                ..Default::default()
            },
        )
        .unwrap();
    let mut candidate = store.read("board-1").unwrap();
    candidate.items.retain(|item| item.item_id == "answer-1");
    let saved = store.update(candidate, 2, "removed").unwrap();
    assert!(saved.links.is_empty() && saved.mappings.is_empty() && saved.routes.is_empty());
    assert_eq!(store.view("board-1").unwrap(), BoardView::default());
    let replay = append(&store, 1, (vec![], vec![], vec![], vec![])).unwrap();
    assert_eq!(replay, receipt);
    assert_eq!(store.read("board-1").unwrap(), saved);
    assert_eq!(store.read_revision("board-1", 2).unwrap().mappings.len(), 1);
}

#[test]
fn deleted_source_remains_historical_provenance_without_a_dangling_live_link() {
    let temp = tempfile::tempdir().unwrap();
    let store = BoardStore::open(temp.path(), None, false).unwrap();
    store.create(document(None)).unwrap();
    append(&store, 1, authored(2)).unwrap();
    let mut candidate = store.read("board-1").unwrap();
    candidate.items.retain(|item| item.item_id != "answer-1");
    let saved = store.update(candidate, 2, "removed-source").unwrap();
    assert!(saved.links.is_empty());
    assert_eq!(saved.mappings.len(), 1);
    assert!(saved.routes[0].steps[0].focus_targets.is_empty());
    assert_eq!(saved.routes[0].steps[0].reveal_targets.len(), 1);
    store.validate_candidate(&saved).unwrap();
    assert_eq!(
        store.read_revision("board-1", 1).unwrap().items[0].item_id,
        "answer-1"
    );
}

#[test]
fn invalid_block_revision_geometry_and_visual_refs_never_publish_a_partial_graph() {
    let temp = tempfile::tempdir().unwrap();
    let store = BoardStore::open(temp.path(), None, false).unwrap();
    let initial = store.create(document(None)).unwrap();
    let mut invalid_block = authored(2);
    invalid_block.2[0].explanation.block_id = Some("missing-block".into());
    assert_eq!(
        append(&store, 1, invalid_block).unwrap_err().error_code,
        "INVALID_BOARD"
    );
    let mut future = authored(3);
    assert_eq!(
        append(&store, 1, future.clone()).unwrap_err().error_code,
        "INVALID_BOARD"
    );
    future.2[0].explanation.content_revision = 1;
    assert_eq!(
        append(&store, 1, future).unwrap_err().error_code,
        "INVALID_BOARD"
    );
    let mut path = authored(2);
    if let BoardTarget::Drawing {
        graphic: BoardGraphic::Path { points, .. },
        ..
    } = &mut path.0[1].target
    {
        points[0].x = f64::INFINITY;
    }
    assert_eq!(
        append(&store, 1, path).unwrap_err().error_code,
        "INVALID_BOARD"
    );
    let mut visual = authored(2);
    visual.2[0].visual = BoardVisualTarget::PresentationAnchor {
        item_id: "answer-1".into(),
        reference: NotePresentationRef {
            presentation_id: "missing-demo".into(),
            revision: 1,
        },
        anchor_id: "region".into(),
    };
    assert_eq!(
        append(&store, 1, visual).unwrap_err().error_code,
        "INVALID_BOARD"
    );
    assert_eq!(store.read("board-1").unwrap(), initial);
}

#[test]
fn layout_edits_cannot_rewrite_fine_blocks_or_authored_relations() {
    let temp = tempfile::tempdir().unwrap();
    let store = BoardStore::open(temp.path(), None, false).unwrap();
    store.create(document(None)).unwrap();
    append(&store, 1, authored(2)).unwrap();
    let saved = store.read("board-1").unwrap();
    let mut changed = saved.clone();
    changed.items[1].explanation_blocks[0].markdown = "replacement explanation".into();
    assert_eq!(
        store.update(changed, 2, "edit").unwrap_err().error_code,
        "INVALID_BOARD"
    );
    let mut changed = saved.clone();
    changed.links[0].label = "replacement relation".into();
    assert_eq!(
        store.update(changed, 2, "edit").unwrap_err().error_code,
        "INVALID_BOARD"
    );
    let mut moved = saved.clone();
    moved.items[2].position.y += 100.0;
    let moved = store.update(moved, 2, "drag").unwrap();
    assert_eq!(moved.links, saved.links);
    assert_eq!(moved.mappings, saved.mappings);
    assert_eq!(moved.routes, saved.routes);
}

#[test]
fn old_documents_and_views_keep_whole_answer_addresses_without_new_fields() {
    let temp = tempfile::tempdir().unwrap();
    let store = BoardStore::open(temp.path(), None, false).unwrap();
    store.create(document(None)).unwrap();
    let path = store
        .board_path("board-1")
        .unwrap()
        .join("revisions/1.json");
    let mut legacy: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    for key in ["links", "mappings", "routes"] {
        legacy.as_object_mut().unwrap().remove(key);
    }
    let item = legacy["items"][0].as_object_mut().unwrap();
    item.remove("explanation_blocks");
    item.remove("block_id");
    std::fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
    let loaded = store.read("board-1").unwrap();
    assert!(loaded.links.is_empty() && loaded.items[0].explanation_blocks.is_empty());
    assert_eq!(
        resolve_explanation_block(&loaded.items[0], "block-answer-1")
            .unwrap()
            .markdown,
        loaded.items[0].readable_content
    );
    let legacy_view: BoardView =
        serde_json::from_value(json!({"x":0,"y":0,"zoom":1,"selected_item_id":null})).unwrap();
    assert_eq!(legacy_view, BoardView::default());
}
