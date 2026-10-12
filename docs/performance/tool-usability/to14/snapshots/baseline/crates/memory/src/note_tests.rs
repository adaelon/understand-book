use super::*;

fn input(state: u32) -> SaveInput {
    SaveInput {
        mem_id: None,
        mem_type: "note".into(),
        layer: "long_term".into(),
        book_id: "book".into(),
        anchor: Anchor::default(),
        content: "same thought".into(),
        range: None,
        selection_context: None,
        note_placement: None,
        citations: None,
        source_session_id: Some("chat".into()),
        note: Some(NoteData {
            material: NoteMaterial {
                book_id: "book".into(),
                publication_id: Some("edition-1".into()),
            },
            association: NoteAssociation::Presentation {
                receipt: NotePresentationReceipt {
                    session_id: "chat".into(),
                    turn_id: "turn".into(),
                    reference: NotePresentationRef {
                        presentation_id: "page".into(),
                        revision: 2,
                    },
                    state_revision: state,
                    saved_state_ref: format!("state-{state}"),
                },
                title: "Recall".into(),
            },
            retained_excerpt: None,
            source_bindings: vec![],
        }),
    }
}

#[test]
fn rn1_association_identity_edit_placement_promote_and_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("memory.json");
    let mut store = MemoryStore::open(&path).unwrap();
    let first = store.save_note(input(1), "created").unwrap().record;
    let second = store.save_note(input(2), "later").unwrap().record;
    assert_ne!(first.mem_id, second.mem_id);
    assert_eq!(
        store.save_note(input(1), "retry").unwrap(),
        NoteSaveOutcome {
            status: NoteSaveStatus::Existing,
            record: first.clone()
        }
    );
    let mut version = input(1);
    if let NoteAssociation::Presentation { receipt, .. } =
        &mut version.note.as_mut().unwrap().association
    {
        receipt.reference.revision = 3;
    }
    assert_ne!(
        store.save_note(version, "version").unwrap().record.mem_id,
        first.mem_id
    );
    let mut publication = input(1);
    publication.note.as_mut().unwrap().material.publication_id = Some("edition-2".into());
    assert_ne!(
        store
            .save_note(publication, "publication")
            .unwrap()
            .record
            .mem_id,
        first.mem_id
    );
    let edited = store
        .replace(
            ReplaceInput {
                mem_id: first.mem_id.clone(),
                content: "edited".into(),
                selection_context: None,
            },
            "edit",
        )
        .unwrap();
    assert_eq!(edited.note, first.note);
    assert_eq!(edited.generated_at, first.generated_at);
    assert_eq!(edited.source_session_id, first.source_session_id);
    let moved = store
        .reanchor(ReanchorInput {
            mem_id: edited.mem_id,
            note_placement: NoteBodyPlacement::LidBlock {
                source_fingerprint: "existing-source".into(),
                lid: "1.2".into(),
            },
        })
        .unwrap();
    assert_eq!(moved.note, first.note);
    assert_eq!(moved.generated_at, first.generated_at);
    assert_eq!(moved.anchor.lid.as_deref(), Some("1.2"));
    let promoted = store
        .promote(PromoteInput {
            mem_id: moved.mem_id.clone(),
            from_layer: "long_term".into(),
            to_layer: "session".into(),
        })
        .unwrap();
    assert_eq!(promoted.note, first.note);
    assert_eq!(MemoryStore::open(path).unwrap().document, store.document);
}

#[test]
fn rn1_four_kinds_and_retained_content_admission() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = MemoryStore::open(dir.path().join("memory.json")).unwrap();
    let mut answer = input(1);
    answer.content.clear();
    answer.note.as_mut().unwrap().association = NoteAssociation::Answer {
        session_id: "chat".into(),
        turn_id: "turn".into(),
    };
    assert!(store.save_note(answer.clone(), "t0").is_err());
    answer.note.as_mut().unwrap().retained_excerpt = Some(NoteExcerpt::Assistant {
        text: "assistant excerpt".into(),
    });
    let saved = store.save_note(answer, "t0").unwrap().record;
    let edited = store
        .replace(
            ReplaceInput {
                mem_id: saved.mem_id,
                content: "".into(),
                selection_context: None,
            },
            "t1",
        )
        .unwrap();
    assert_eq!(
        edited.note.unwrap().retained_excerpt.unwrap().text(),
        "assistant excerpt"
    );
    let mut empty = input(1);
    empty.content.clear();
    assert!(store.save_note(empty, "t0").is_err());
    let mut selection = input(1);
    selection.anchor.lid = Some("1.1".into());
    selection.selection_context = Some(SelectionContext {
        status: SelectionResolution::Resolved,
        resolution_basis: None,
        raw_quote: "原文".into(),
        resolved_quote: "原文".into(),
        ranges: vec![SelectedRange {
            lid: "1.1".into(),
            range: TextRange { start: 0, end: 2 },
        }],
    });
    selection.note.as_mut().unwrap().association = NoteAssociation::Selection;
    selection.note.as_mut().unwrap().retained_excerpt = Some(NoteExcerpt::Original {
        text: "原文".into(),
    });
    let selected = store.save_note(selection, "t0").unwrap().record;
    let selected_edit = store
        .replace(
            ReplaceInput {
                mem_id: selected.mem_id,
                content: "我的想法".into(),
                selection_context: None,
            },
            "t1",
        )
        .unwrap();
    assert_eq!(selected_edit.selection_context, selected.selection_context);
    assert_eq!(selected_edit.note, selected.note);
    let mut reselected = selected.selection_context.clone().unwrap();
    reselected.raw_quote = "新原文".into();
    reselected.resolved_quote = "新原文".into();
    reselected.ranges[0].range.end = 3;
    let reselected = store
        .replace(
            ReplaceInput {
                mem_id: selected_edit.mem_id,
                content: "我的想法".into(),
                selection_context: Some(reselected),
            },
            "t2",
        )
        .unwrap();
    assert_eq!(
        reselected.note.unwrap().retained_excerpt.unwrap(),
        NoteExcerpt::Original {
            text: "新原文".into()
        }
    );
    let presentation = store.save_note(input(1), "t0").unwrap().record;
    assert!(store
        .replace(
            ReplaceInput {
                mem_id: presentation.mem_id.clone(),
                content: "edited".into(),
                selection_context: selected.selection_context
            },
            "t2"
        )
        .is_err());
    assert!(store
        .recall(&RecallQuery::default())
        .contains(&presentation));
    let mut placed = input(1);
    let placement = NoteBodyPlacement::LidBlock {
        source_fingerprint: "source".into(),
        lid: "1.1".into(),
    };
    placed.anchor.lid = Some("1.1".into());
    placed.note_placement = Some(placement.clone());
    placed.note.as_mut().unwrap().association = NoteAssociation::BodyPlacement { placement };
    let saved = store.save_note(placed, "t0").unwrap().record;
    let moved = store
        .reanchor(ReanchorInput {
            mem_id: saved.mem_id,
            note_placement: NoteBodyPlacement::LidBlock {
                source_fingerprint: "source".into(),
                lid: "1.2".into(),
            },
        })
        .unwrap();
    assert_eq!(saved.note, moved.note);
}

#[test]
fn rn1_failed_edits_collisions_and_writes_keep_original() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("memory.json");
    let mut store = MemoryStore::open(&path).unwrap();
    let first = store.save_note(input(1), "t0").unwrap().record;
    let mut other = input(1);
    other.content = "other".into();
    store.save_note(other, "t1").unwrap();
    let before = store.document.clone();
    let bytes = std::fs::read(&path).unwrap();
    let replace = |content: &str| ReplaceInput {
        mem_id: first.mem_id.clone(),
        content: content.into(),
        selection_context: None,
    };
    assert_eq!(
        store
            .replace(replace("other"), "t2")
            .unwrap_err()
            .error_code,
        "MEMORY_REPLACE_CONFLICT"
    );
    assert!(store.replace(replace(""), "t2").is_err());
    std::fs::create_dir(atomic_temporary_path(&path)).unwrap();
    assert!(store.replace(replace("changed"), "t2").is_err());
    assert!(store.save_note(input(2), "t2").is_err());
    assert!(store
        .reanchor(ReanchorInput {
            mem_id: first.mem_id,
            note_placement: NoteBodyPlacement::LidBlock {
                source_fingerprint: "source".into(),
                lid: "1.1".into()
            }
        })
        .is_err());
    assert_eq!(store.document, before);
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}

#[test]
fn rn1_v3_fixture_preserves_every_stored_field_and_failed_upgrade() {
    let original: serde_json::Value =
        serde_json::from_str(include_str!("../tests/fixtures/memory-v3.json")).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("memory.json");
    std::fs::write(&path, serde_json::to_vec(&original).unwrap()).unwrap();
    let before = std::fs::read(&path).unwrap();
    std::fs::create_dir(atomic_temporary_path(&path)).unwrap();
    assert!(MemoryStore::open(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    std::fs::remove_dir(atomic_temporary_path(&path)).unwrap();
    let store = MemoryStore::open(&path).unwrap();
    let expected: MemoryDocument = serde_json::from_value({
        let mut v = original;
        v["schema_version"] = 4.into();
        v
    })
    .unwrap();
    assert_eq!(store.document, expected);
    assert!(store.document.records.iter().all(|r| r.note.is_none()));
    assert_eq!(MemoryStore::open(path).unwrap().document, expected);
}


#[test]
fn rn4_note_navigation_survives_edits_reanchor_reopen_and_delete() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("memory.json");
    let mut store = MemoryStore::open(&path).unwrap();
    let first = store.save_note(input(1), "created").unwrap().record;
    let edited = store.replace(ReplaceInput { mem_id: first.mem_id.clone(), content: "edited".into(), selection_context: None }, "edited").unwrap();
    let moved = store.reanchor(ReanchorInput { mem_id: edited.mem_id.clone(), note_placement: NoteBodyPlacement::LidBlock { source_fingerprint: "existing-source".into(), lid: "1.2".into() } }).unwrap();
    let reopened = MemoryStore::open(&path).unwrap();
    for id in [&first.mem_id, &edited.mem_id, &moved.mem_id] {
        assert_eq!(reopened.current_note_record(id), Some(&moved));
    }
    let before = store.document.clone();
    std::fs::create_dir(atomic_temporary_path(&path)).unwrap();
    assert!(store.replace(ReplaceInput { mem_id: moved.mem_id.clone(), content: "failed".into(), selection_context: None }, "failed").is_err());
    assert_eq!(store.document, before);
    std::fs::remove_dir(atomic_temporary_path(&path)).unwrap();
    store.delete(&moved.mem_id).unwrap();
    assert!(MemoryStore::open(&path).unwrap().current_note_record(&first.mem_id).is_none());
}

#[test]
fn rn4_returning_to_prior_content_keeps_navigation_acyclic() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = MemoryStore::open(dir.path().join("memory.json")).unwrap();
    let first = store.save_note(input(1), "created").unwrap().record;
    let edited = store.replace(ReplaceInput { mem_id: first.mem_id.clone(), content: "edited".into(), selection_context: None }, "edited").unwrap();
    let restored = store.replace(ReplaceInput { mem_id: edited.mem_id.clone(), content: first.content.clone(), selection_context: None }, "restored").unwrap();
    assert_eq!(restored.mem_id, first.mem_id);
    assert_eq!(store.current_note_record(&edited.mem_id), Some(&restored));
    assert_eq!(store.current_note_record(&first.mem_id), Some(&restored));
}
