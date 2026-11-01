use super::*;
use crate::boards::tests::{answer, document};

fn topic(id: &str, title: &str) -> BoardTopicInput {
    BoardTopicInput {
        item_id: id.into(),
        title: title.into(),
        style: BoardTopicStyle::default(),
        based_on: vec![],
        source_bindings: vec![],
    }
}

fn create_map() -> BoardEditOperation {
    BoardEditOperation::CreateMindMap {
        map: BoardMindMap {
            map_id: "map-1".into(),
            root_item_id: "root".into(),
            layout: BoardMindMapLayout::Balanced,
            anchor: BoardPosition { x: 80.0, y: 90.0 },
            branch_sides: BTreeMap::new(),
        },
        root: topic("root", "Rust 切片"),
    }
}

fn insert(parent: &str, id: &str, index: usize) -> BoardEditOperation {
    BoardEditOperation::InsertTopic {
        parent_item_id: parent.into(),
        index,
        topic: topic(id, id),
    }
}

fn rename(id: &str, title: &str) -> BoardEditOperation {
    BoardEditOperation::UpdateTopic {
        item_id: id.into(),
        title: Some(title.into()),
        style: None,
    }
}

fn seeded(store: &BoardStore) -> BoardDocument {
    let initial = store.create(document(None)).unwrap();
    store
        .edit(
            &initial.board_id,
            initial.revision,
            vec![
                create_map(),
                insert("root", "read", 0),
                insert("root", "write", 1),
                insert("read", "slice", 0),
            ],
            BoardEditActor::Human,
            "Create Rust map",
            "t1",
        )
        .unwrap()
}

#[test]
fn old_board_and_view_deserialize_without_mind_map_fields() {
    let mut value = serde_json::to_value(document(None)).unwrap();
    value.as_object_mut().unwrap().remove("mind_maps");
    value.as_object_mut().unwrap().remove("edit_history");
    let old: BoardDocument = serde_json::from_value(value).unwrap();
    assert!(old.mind_maps.is_empty());
    assert_eq!(old.edit_history, BoardEditHistory::default());
    let view: BoardView =
        serde_json::from_value(serde_json::json!({"x":0,"y":0,"zoom":1,"selected_item_id":null}))
            .unwrap();
    assert!(view.folded_topic_ids.is_empty());
    assert!(view.selected_item_ids.is_empty());
    assert!(!view.mind_map_outline);
}

#[test]
fn atomic_tree_edits_reject_cycles_duplicate_parents_and_orphans() {
    let temp = tempfile::tempdir().unwrap();
    let store = BoardStore::open(temp.path(), None, false).unwrap();
    let base = seeded(&store);
    let cycle = BoardEditOperation::MoveSubtree {
        item_id: "read".into(),
        parent_item_id: "slice".into(),
        index: 0,
    };
    assert!(store
        .edit(
            &base.board_id,
            base.revision,
            vec![rename("write", "must not persist"), cycle],
            BoardEditActor::Human,
            "bad",
            "t2"
        )
        .is_err());
    assert_eq!(store.read(&base.board_id).unwrap(), base);
    let mut duplicate = base.clone();
    children_mut(&mut duplicate, "write")
        .unwrap()
        .push("slice".into());
    assert!(validate_mind_maps(&duplicate).is_err());
    let mut orphan = base.clone();
    children_mut(&mut orphan, "read").unwrap().clear();
    assert!(validate_mind_maps(&orphan).is_err());
    let mut two_maps = base.clone();
    two_maps.mind_maps.push(BoardMindMap {
        map_id: "map-2".into(),
        ..base.mind_maps[0].clone()
    });
    assert!(validate_mind_maps(&two_maps).is_err());
}

#[test]
fn move_keeps_branch_identity_and_sides_copy_retains_sources_and_internal_links() {
    let temp = tempfile::tempdir().unwrap();
    let store = BoardStore::open(temp.path(), None, false).unwrap();
    let base = seeded(&store);
    let original_sides = base.mind_maps[0].branch_sides.clone();
    let mut copied_topic = topic("borrow", "借用");
    copied_topic.source_bindings = answer().source_bindings;
    copied_topic.based_on = vec![BoardContentRef {
        item_id: "answer-1".into(),
        block_id: None,
        content_revision: 1,
    }];
    let source = store
        .edit(
            &base.board_id,
            base.revision,
            vec![
                BoardEditOperation::MoveSubtree {
                    item_id: "read".into(),
                    parent_item_id: "root".into(),
                    index: 1,
                },
                BoardEditOperation::InsertTopic {
                    parent_item_id: "read".into(),
                    index: 1,
                    topic: copied_topic.clone(),
                },
                BoardEditOperation::PutLink {
                    link: BoardLink {
                        link_id: "internal".into(),
                        from: BoardVisualTarget::Item {
                            item_id: "slice".into(),
                        },
                        to: BoardVisualTarget::Item {
                            item_id: "borrow".into(),
                        },
                        label: "borrows".into(),
                        based_on: vec![],
                        note_relation_id: None,
                        reason: None,
                    },
                },
            ],
            BoardEditActor::Human,
            "Organize",
            "t2",
        )
        .unwrap();
    assert_eq!(children(&source, "root").unwrap(), ["write", "read"]);
    assert_eq!(source.mind_maps[0].branch_sides, original_sides);
    let copied = store
        .edit(
            &base.board_id,
            source.revision,
            vec![BoardEditOperation::CopySubtree {
                item_id: "read".into(),
                parent_item_id: "write".into(),
                index: 0,
                id_prefix: "copy".into(),
            }],
            BoardEditActor::Human,
            "Copy",
            "t3",
        )
        .unwrap();
    assert_eq!(
        children(&copied, "copy-read").unwrap(),
        ["copy-slice", "copy-borrow"]
    );
    let borrow = copied
        .items
        .iter()
        .find(|item| item.item_id == "copy-borrow")
        .unwrap();
    assert_eq!(borrow.source_bindings, copied_topic.source_bindings);
    assert!(
        matches!(&borrow.target, BoardTarget::Topic { based_on, .. } if based_on == &copied_topic.based_on)
    );
    let link = copied
        .links
        .iter()
        .find(|link| link.link_id == "copy-internal")
        .unwrap();
    assert_eq!(
        link.from,
        BoardVisualTarget::Item {
            item_id: "copy-slice".into()
        }
    );
    assert_eq!(
        link.to,
        BoardVisualTarget::Item {
            item_id: "copy-borrow".into()
        }
    );
    assert_eq!(copied.deliveries, source.deliveries);
}

#[test]
fn delete_undo_redo_reopen_restore_links_mappings_routes_and_stable_ids() {
    let temp = tempfile::tempdir().unwrap();
    let store = BoardStore::open(temp.path(), None, false).unwrap();
    let base = seeded(&store);
    let explanation = BoardContentRef {
        item_id: "answer-1".into(),
        block_id: Some("block-answer-1".into()),
        content_revision: 1,
    };
    let edge = BoardVisualTarget::TopicEdge {
        child_item_id: "slice".into(),
    };
    let related = store
        .edit(
            &base.board_id,
            base.revision,
            vec![
                BoardEditOperation::PutLink {
                    link: BoardLink {
                        link_id: "relation".into(),
                        from: BoardVisualTarget::Item {
                            item_id: "read".into(),
                        },
                        to: BoardVisualTarget::Item {
                            item_id: "write".into(),
                        },
                        label: "compare".into(),
                        based_on: vec![],
                        note_relation_id: None,
                        reason: None,
                    },
                },
                BoardEditOperation::PutMapping {
                    mapping: BoardExplanationMapping {
                        mapping_id: "mapping".into(),
                        explanation: explanation.clone(),
                        visual: edge.clone(),
                    },
                },
                BoardEditOperation::PutRoute {
                    route: BoardRoute {
                        route_id: "route".into(),
                        title: "Read".into(),
                        steps: vec![BoardRouteStep {
                            step_id: "step".into(),
                            explanation: None,
                            reveal_targets: vec![edge],
                            focus_targets: vec![],
                            transition: BoardTransition::Instant,
                            duration_ms: 0,
                        }],
                    },
                },
            ],
            BoardEditActor::Human,
            "Connect",
            "t2",
        )
        .unwrap();
    let removed = store
        .edit(
            &base.board_id,
            related.revision,
            vec![BoardEditOperation::RemoveSubtree {
                item_id: "read".into(),
            }],
            BoardEditActor::Human,
            "Delete branch",
            "t3",
        )
        .unwrap();
    assert!(removed.links.is_empty() && removed.mappings.is_empty() && removed.routes.is_empty());
    assert_eq!(children(&removed, "root").unwrap(), ["write"]);
    let reopened = BoardStore::open(temp.path(), None, false).unwrap();
    let restored = reopened
        .undo(&base.board_id, removed.revision, "t4")
        .unwrap();
    assert_eq!(restored.items, related.items);
    assert_eq!(restored.links, related.links);
    assert_eq!(restored.mappings, related.mappings);
    assert_eq!(restored.routes, related.routes);
    assert_eq!(restored.mind_maps, related.mind_maps);
    assert_eq!(restored.edit_history.redo_stack, [removed.revision]);
    let redone = reopened
        .redo(&base.board_id, restored.revision, "t5")
        .unwrap();
    assert_eq!(redone.items, removed.items);
    assert_eq!(redone.links, removed.links);
    let undone = reopened
        .undo(&base.board_id, redone.revision, "t6")
        .unwrap();
    let edited = reopened
        .edit(
            &base.board_id,
            undone.revision,
            vec![rename("write", "新的人工修改")],
            BoardEditActor::Human,
            "Rename",
            "t7",
        )
        .unwrap();
    assert!(edited.edit_history.redo_stack.is_empty());
    assert!(reopened
        .redo(&base.board_id, edited.revision, "t8")
        .is_err());
}

#[test]
fn ai_edit_is_one_transaction_undo_preserves_manual_work_and_delivery_retry() {
    let temp = tempfile::tempdir().unwrap();
    let store = BoardStore::open(temp.path(), None, false).unwrap();
    let base = seeded(&store);
    let manual = store
        .edit(
            &base.board_id,
            base.revision,
            vec![rename("write", "我整理的可写权限")],
            BoardEditActor::Human,
            "Rename",
            "t2",
        )
        .unwrap();
    let follow_up = BoardFollowUp { note_refs:None,
        board_id: base.board_id.clone(),
        board_revision: manual.revision,
        item_id: Some("read".into()),
        block_id: None,
        presentation: None,
        intent: None,
        regenerate_from: None,
    };
    let mut explanation = answer();
    explanation.item_id = "ai-answer".into();
    let operations = vec![insert("read", "example", 1), rename("read", "只读切片")];
    let receipt = store
        .append_authored_part_with_edits(
            &follow_up,
            manual.revision,
            "chat-1",
            "turn-1",
            "candidate-1",
            vec![explanation.clone()],
            vec![],
            vec![],
            vec![],
            vec![],
            operations.clone(),
            "t3",
        )
        .unwrap();
    let after = store.read(&base.board_id).unwrap();
    assert_eq!(after.revision, manual.revision + 1);
    assert_eq!(after.edit_history.undo_stack.last(), Some(&after.revision));
    let undone = store.undo(&base.board_id, after.revision, "t4").unwrap();
    assert_eq!(undone.items, manual.items);
    assert_eq!(undone.deliveries, vec![receipt.clone()]);
    assert_eq!(
        store
            .append_authored_part_with_edits(
                &follow_up,
                manual.revision,
                "chat-1",
                "turn-1",
                "candidate-1",
                vec![explanation],
                vec![],
                vec![],
                vec![],
                vec![],
                operations,
                "t5"
            )
            .unwrap(),
        receipt
    );
    assert_eq!(store.read(&base.board_id).unwrap(), undone);
    let redone = store.redo(&base.board_id, undone.revision, "t6").unwrap();
    assert_eq!(redone.items, after.items);
    assert_eq!(redone.deliveries.len(), 1);
}

#[test]
fn semantic_rebase_preserves_other_branch_and_rejects_actual_dependencies() {
    let temp = tempfile::tempdir().unwrap();
    let store = BoardStore::open(temp.path(), None, false).unwrap();
    let base = seeded(&store);
    let operations = vec![insert("read", "example", 1)];
    let mut latest = apply_board_edits(&base, &[rename("write", "人工可写分支")], "t2").unwrap();
    latest
        .items
        .iter_mut()
        .find(|item| item.item_id == "read")
        .unwrap()
        .position
        .x += 200.0;
    assert!(!board_edits_conflict(&base, &latest, &operations, &[]).unwrap());
    let projected = apply_board_edits(&latest, &operations, "t3").unwrap();
    assert_eq!(
        projected
            .items
            .iter()
            .find(|item| item.item_id == "write")
            .unwrap()
            .title,
        "人工可写分支"
    );
    assert!(board_edits_conflict(&base, &latest, &operations, &["write".into()]).unwrap());
    let same_parent = apply_board_edits(&base, &[insert("read", "human", 0)], "t2").unwrap();
    assert!(board_edits_conflict(&base, &same_parent, &operations, &[]).unwrap());
    let deleted = apply_board_edits(
        &base,
        &[BoardEditOperation::RemoveSubtree {
            item_id: "read".into(),
        }],
        "t2",
    )
    .unwrap();
    assert!(board_edits_conflict(&base, &deleted, &operations, &[]).unwrap());
    let same_title = apply_board_edits(&base, &[rename("read", "人工只读标题")], "t2").unwrap();
    assert!(board_edits_conflict(&base, &same_title, &[rename("read", "AI标题")], &[]).unwrap());
    let copy = BoardEditOperation::CopySubtree {
        item_id: "read".into(),
        parent_item_id: "write".into(),
        index: 0,
        id_prefix: "copy".into(),
    };
    assert!(
        board_edits_conflict(&base, &same_title, &[copy.clone()], &[]).unwrap(),
        "copying must keep the observed source wording"
    );
    let source_children_changed =
        apply_board_edits(&base, &[insert("read", "new-source-child", 1)], "t2").unwrap();
    assert!(
        board_edits_conflict(&base, &source_children_changed, &[copy], &[]).unwrap(),
        "copying must keep the observed source hierarchy"
    );
}

#[test]
fn stale_revision_cannot_write_and_view_changes_do_not_create_content_revisions() {
    let temp = tempfile::tempdir().unwrap();
    let store = BoardStore::open(temp.path(), None, false).unwrap();
    let base = seeded(&store);
    let edited = store
        .edit(
            &base.board_id,
            base.revision,
            vec![rename("read", "Read")],
            BoardEditActor::Human,
            "Rename",
            "t2",
        )
        .unwrap();
    assert!(store
        .edit(
            &base.board_id,
            base.revision,
            vec![rename("read", "Stale")],
            BoardEditActor::Human,
            "Rename",
            "t3"
        )
        .is_err());
    assert!(store.undo(&base.board_id, base.revision, "t3").is_err());
    let view = BoardView {
        folded_topic_ids: vec!["read".into(), "deleted".into()],
        selected_item_ids: vec!["read".into(), "write".into(), "deleted".into()],
        mind_map_outline: true,
        ..BoardView::default()
    };
    let saved = store.save_view(&base.board_id, &view).unwrap();
    assert_eq!(saved.folded_topic_ids, ["read"]);
    assert_eq!(saved.selected_item_ids, ["read", "write"]);
    assert!(store.view(&base.board_id).unwrap().mind_map_outline);
    assert_eq!(store.read(&base.board_id).unwrap(), edited);
}

#[test]
fn projected_note_relationships_remain_owned_by_notebook() {
    let mut base = apply_board_edits(&document(None), &[create_map()], "t1").unwrap();
    let link = BoardLink {
        link_id: "note-projection".into(),
        from: BoardVisualTarget::Item {
            item_id: "root".into(),
        },
        to: BoardVisualTarget::Item {
            item_id: "answer-1".into(),
        },
        label: "shared".into(),
        based_on: vec![],
        note_relation_id: Some("relation".into()),
        reason: Some("Shared notebook".into()),
    };
    base.links.push(link.clone());
    assert!(apply_board_edits(
        &base,
        &[BoardEditOperation::RemoveLink {
            link_id: link.link_id.clone()
        }],
        "t2"
    )
    .is_err());
    let mut forged = link;
    forged.note_relation_id = None;
    assert!(
        apply_board_edits(&base, &[BoardEditOperation::PutLink { link: forged }], "t2").is_err()
    );
}

#[test]
fn note_revision_dependencies_detect_separate_edits_and_undo_keeps_current_note_text() {
    use crate::workspace_notes::{WorkspaceNote, WorkspaceNoteAuthor, WorkspaceNoteStore};
    let temp = tempfile::tempdir().unwrap();
    let notes = WorkspaceNoteStore::open(temp.path(), None, false).unwrap();
    notes
        .create(WorkspaceNote {
            note_id: "shared".into(),
            learning_workspace_id: None,
            title: "Shared note".into(),
            content: "Original wording".into(),
            revision: 1,
            created_at: "t0".into(),
            updated_at: "t0".into(),
            author: WorkspaceNoteAuthor::User,
            updated_by: WorkspaceNoteAuthor::User,
            origin: None,
            source_bindings: vec![],
        })
        .unwrap();
    let store = BoardStore::open(temp.path(), None, false).unwrap();
    let mut initial = document(None);
    let mut note_item = answer();
    note_item.item_id = "note-card".into();
    note_item.target = BoardTarget::Note {
        note_id: "shared".into(),
        missing: false,
    };
    initial.items.push(note_item);
    let original = store.create(initial).unwrap();
    let before = store
        .edit(
            &original.board_id,
            original.revision,
            vec![create_map()],
            BoardEditActor::Human,
            "Create",
            "t1",
        )
        .unwrap();
    let note_ids = vec!["note-card".into()];
    assert_eq!(
        store.note_revisions(&before, &note_ids).unwrap()["shared"],
        1
    );
    let renamed = store
        .edit(
            &before.board_id,
            before.revision,
            vec![rename("root", "Changed topic")],
            BoardEditActor::Human,
            "Rename",
            "t2",
        )
        .unwrap();
    notes
        .update(
            "shared",
            1,
            "Shared note",
            "A newer note body",
            WorkspaceNoteAuthor::User,
            "t3",
        )
        .unwrap();
    let latest = store.read(&before.board_id).unwrap();
    assert_eq!(
        latest.revision, renamed.revision,
        "a notebook edit has a separate revision clock"
    );
    assert_eq!(
        store.note_revisions(&latest, &note_ids).unwrap()["shared"],
        2
    );
    assert!(
        board_edits_conflict(&before, &latest, &[insert("root", "child", 0)], &note_ids).unwrap()
    );
    let undone = store
        .undo(&before.board_id, renamed.revision, "t4")
        .unwrap();
    assert_eq!(
        undone
            .items
            .iter()
            .find(|item| item.item_id == "note-card")
            .unwrap()
            .readable_content,
        "A newer note body"
    );
    assert_eq!(notes.read("shared").unwrap().revision, 2);
}
