use memory::*;

fn statement(session: &str, key: &str, value: &str) -> CreateProfileFact {
    CreateProfileFact {
        scope: ProfileScope::Book {
            book_id: "book".into(),
        },
        applicability: Applicability::Any,
        payload: ProfilePayload::ExplanationPreference(PreferenceClaim {
            key: key.into(),
            value: value.into(),
        }),
        source: FactSource::UserStated,
        evidence: vec![EvidenceRef::Turn {
            session_id: session.into(),
            turn_id: "turn".into(),
        }],
        confidence: None,
        sensitivity: Sensitivity::Normal,
        valid_until: None,
    }
}

fn job(store: &mut MemoryStore, session: &str) -> ReviewJob {
    store
        .reconcile_review_jobs(
            &[ReviewSessionCursor {
                learning_workspace_id: None,
                turns: Vec::new(),
                session_id: session.into(),
                book_id: "book".into(),
                latest_user_turn_ordinal: 1,
            }],
            "10",
        )
        .unwrap();
    let id = store
        .review_state()
        .review_jobs
        .iter()
        .find(|job| job.session_id == session)
        .unwrap()
        .job_id
        .clone();
    store.claim_review_job(&id, "11").unwrap()
}

fn revision(
    session: &str,
    key: &str,
    value: &str,
    time: u64,
    previous: &[ProfileFact],
) -> ReviewFactCandidate {
    ReviewFactCandidate::revision(
        statement(session, key, value),
        previous.iter().map(|old| old.fact_id.clone()).collect(),
        Some(time),
    )
    .unwrap()
}

fn snapshot(store: &MemoryStore) -> ReaderProfileSnapshot {
    store.project_reader_profile_snapshot(&SnapshotRequest::current(SnapshotContext {
        book_id: Some("book".into()),
        ..Default::default()
    }))
}

#[test]
fn ordinary_statement_revises_a_correction_and_persists_one_current_fact() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("memory.json");
    let mut store = MemoryStore::open(&path).unwrap();
    let a = store
        .create_profile_fact(statement("a", "style", "A"), "1")
        .unwrap();
    let b = store
        .correct_profile_fact(&a.fact_id, statement("b", "style", "B"), "2")
        .unwrap();
    let unrelated = store
        .create_profile_fact(statement("other", "language", "Chinese"), "2")
        .unwrap();
    let job = job(&mut store, "c");
    let reviewed = vec![b.clone(), unrelated.clone()];
    let output = revision("c", "synonym_for_style", "C", 3_000, &[b.clone()]);
    let before = store.projection_revision();
    let result = store
        .commit_review_result(
            &job.job_id,
            &["turn".into()],
            &reviewed,
            &[output],
            &[],
            "90",
        )
        .unwrap();
    let c = store
        .profile_facts()
        .iter()
        .find(|fact| result.added_fact_ids.contains(&fact.fact_id))
        .unwrap();
    assert_eq!(c.payload, statement("", "style", "C").payload);
    assert_eq!(c.observed_at_ms, Some(3_000));
    assert_eq!(c.updated_at, "90");
    assert_eq!(c.supersedes, vec![b.fact_id.clone()]);
    assert_eq!(
        snapshot(&store)
            .injected_fact_ids()
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>(),
        [c.fact_id.clone(), unrelated.fact_id.clone()]
            .into_iter()
            .collect()
    );
    assert!(store
        .profile_facts()
        .iter()
        .filter(|fact| [a.fact_id.clone(), b.fact_id.clone()].contains(&fact.fact_id))
        .all(|fact| fact.status == FactStatus::Superseded));
    assert_eq!(store.projection_revision(), before + 1);
    assert_eq!(store.review_state().reviewed_through["c"], 1);
    let reopened = MemoryStore::open(path).unwrap();
    assert_eq!(reopened.profile_facts(), store.profile_facts());
    assert_eq!(snapshot(&reopened), snapshot(&store));
    let before = store.document_revision();
    assert!(
        store
            .commit_review_result(&job.job_id, &["turn".into()], &reviewed, &[], &[], "100")
            .unwrap()
            .already_completed
    );
    assert_eq!(store.document_revision(), before);
}

#[test]
fn corrected_fact_has_no_permanent_authority_over_a_later_user_statement() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = MemoryStore::open(dir.path().join("memory.json")).unwrap();
    let a = store
        .create_profile_fact(statement("a", "style", "A"), "1")
        .unwrap();
    store
        .correct_profile_fact(&a.fact_id, statement("b", "style", "B"), "2")
        .unwrap();
    let c = store
        .create_profile_fact(statement("c", "style", "C"), "3")
        .unwrap();
    assert!(c.supersedes.is_empty());
    assert_eq!(snapshot(&store).injected_fact_ids(), vec![c.fact_id]);
}

#[test]
fn an_explicit_revision_link_always_supersedes_its_target_even_when_value_is_retained() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = MemoryStore::open(dir.path().join("memory.json")).unwrap();
    let old = store
        .create_profile_fact(statement("old", "style", "A"), "1")
        .unwrap();
    let job = job(&mut store, "new");
    let output = revision("new", "style", "A", 2_000, &[old.clone()]);
    let result = store
        .commit_review_result(
            &job.job_id,
            &["turn".into()],
            &[old.clone()],
            &[output],
            &[],
            "30",
        )
        .unwrap();
    assert_eq!(snapshot(&store).injected_fact_ids(), result.added_fact_ids);
    assert_eq!(
        store
            .profile_facts()
            .iter()
            .find(|fact| fact.fact_id == old.fact_id)
            .unwrap()
            .status,
        FactStatus::Superseded
    );
}

#[test]
fn older_review_finishing_last_cannot_undo_newer_statement_with_or_without_target() {
    for linked in [true, false] {
        let dir = tempfile::tempdir().unwrap();
        let mut store = MemoryStore::open(dir.path().join("memory.json")).unwrap();
        let a = store
            .create_profile_fact(statement("a", "style", "A"), "1")
            .unwrap();
        let early_job = job(&mut store, "early");
        let late_job = job(&mut store, "late");
        let reviewed = vec![a.clone()];
        let newer = revision("late", "style", "C", 4_000, &reviewed);
        let result = store
            .commit_review_result(
                &late_job.job_id,
                &["turn".into()],
                &reviewed,
                &[newer],
                &[],
                "20",
            )
            .unwrap();
        let older = revision(
            "early",
            "style",
            "B",
            3_000,
            if linked { &reviewed } else { &[] },
        );
        let result_old = store
            .commit_review_result(
                &early_job.job_id,
                &["turn".into()],
                &reviewed,
                &[older],
                &[],
                "30",
            )
            .unwrap();
        assert!(result_old.added_fact_ids.is_empty());
        assert_eq!(snapshot(&store).injected_fact_ids(), result.added_fact_ids);
        assert_eq!(store.review_state().reviewed_through["early"], 1);
    }
}

#[test]
fn newer_review_can_replace_an_intermediate_revision_committed_during_extraction() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = MemoryStore::open(dir.path().join("memory.json")).unwrap();
    let a = store
        .create_profile_fact(statement("a", "style", "A"), "1")
        .unwrap();
    let first = job(&mut store, "first");
    let second = job(&mut store, "second");
    let reviewed = vec![a];
    let b = revision("first", "style", "B", 2_000, &reviewed);
    let first_result = store
        .commit_review_result(&first.job_id, &["turn".into()], &reviewed, &[b], &[], "20")
        .unwrap();
    let c = revision("second", "style", "C", 3_000, &reviewed);
    let second_result = store
        .commit_review_result(&second.job_id, &["turn".into()], &reviewed, &[c], &[], "30")
        .unwrap();
    let current = store
        .profile_facts()
        .iter()
        .find(|fact| second_result.added_fact_ids.contains(&fact.fact_id))
        .unwrap();
    assert!(current.supersedes.contains(&first_result.added_fact_ids[0]));
    assert_eq!(
        snapshot(&store).injected_fact_ids(),
        second_result.added_fact_ids
    );
}

#[test]
fn inference_cannot_replace_user_statement_and_invalid_batch_is_atomic() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("memory.json");
    let mut store = MemoryStore::open(&path).unwrap();
    let old = store
        .create_profile_fact(statement("a", "style", "A"), "1")
        .unwrap();
    let job = job(&mut store, "new");
    let mut inferred = statement("new", "style", "B");
    inferred.source = FactSource::AgentInferred;
    inferred.confidence = Some(Confidence::High);
    let bad =
        ReviewFactCandidate::revision(inferred, vec![old.fact_id.clone()], Some(4_000)).unwrap();
    let good = revision("new", "unrelated", "new fact", 4_000, &[]);
    let before = std::fs::read(&path).unwrap();
    let revision_before = store.document_revision();
    assert!(store
        .commit_review_result(
            &job.job_id,
            &["turn".into()],
            &[old.clone()],
            &[good, bad],
            &[],
            "30"
        )
        .is_err());
    assert_eq!(std::fs::read(path).unwrap(), before);
    assert_eq!(store.document_revision(), revision_before);
    assert_eq!(snapshot(&store).injected_fact_ids(), vec![old.fact_id]);
    assert_eq!(
        store.review_state().review_jobs[0].status,
        ReviewJobStatus::Running
    );
}

#[test]
fn unknown_target_scope_change_and_forgotten_target_do_not_mutate_profile() {
    for case in ["unknown", "scope", "forgotten"] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("memory.json");
        let mut store = MemoryStore::open(&path).unwrap();
        let old = store
            .create_profile_fact(statement("a", "style", "A"), "1")
            .unwrap();
        let job = job(&mut store, "new");
        let mut input = statement("new", "style", "B");
        if case == "scope" {
            input.scope = ProfileScope::Global;
        }
        let output =
            ReviewFactCandidate::revision(input, vec![old.fact_id.clone()], Some(3_000)).unwrap();
        if case == "forgotten" {
            store.forget_profile_fact(&old.fact_id, "20").unwrap();
        }
        let reviewed = if case == "unknown" { vec![] } else { vec![old] };
        let before = std::fs::read(&path).unwrap();
        assert!(
            store
                .commit_review_result(
                    &job.job_id,
                    &["turn".into()],
                    &reviewed,
                    &[output],
                    &[],
                    "30"
                )
                .is_err(),
            "{case}"
        );
        assert_eq!(std::fs::read(path).unwrap(), before, "{case}");
    }
}

#[test]
fn no_profile_candidates_preserves_durable_preference() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = MemoryStore::open(dir.path().join("memory.json")).unwrap();
    let old = store
        .create_profile_fact(statement("a", "style", "A"), "1")
        .unwrap();
    let job = job(&mut store, "temporary");
    let before = snapshot(&store);
    store
        .commit_review_result(&job.job_id, &["turn".into()], &[old], &[], &[], "30")
        .unwrap();
    assert_eq!(snapshot(&store), before);
}

#[test]
fn confirmed_inference_is_not_overwritten_by_background_inference() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = MemoryStore::open(dir.path().join("memory.json")).unwrap();
    let mut inferred = statement("old", "style", "A");
    inferred.source = FactSource::AgentInferred;
    inferred.confidence = Some(Confidence::Medium);
    let old = store.create_profile_fact(inferred, "1").unwrap();
    let old = store.confirm_profile_fact(&old.fact_id, "2").unwrap();
    assert_eq!(old.observed_at_ms, Some(2_000));
    let job = job(&mut store, "new");
    let mut inferred = statement("new", "style", "B");
    inferred.source = FactSource::AgentInferred;
    inferred.confidence = Some(Confidence::High);
    let new = ReviewFactCandidate::revision(inferred, vec![], Some(3_000)).unwrap();
    let result = store
        .commit_review_result(
            &job.job_id,
            &["turn".into()],
            &[old.clone()],
            &[new],
            &[],
            "30",
        )
        .unwrap();
    assert!(result.added_fact_ids.is_empty());
    assert_eq!(snapshot(&store).injected_fact_ids(), vec![old.fact_id]);
}

#[test]
fn a_batch_is_settled_by_source_order_and_missing_new_time_cannot_overwrite() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = MemoryStore::open(dir.path().join("memory.json")).unwrap();
    let old = store
        .create_profile_fact(statement("old", "style", "A"), "1")
        .unwrap();
    store
        .reconcile_review_jobs(
            &[ReviewSessionCursor {
                learning_workspace_id: None,
                turns: vec![],
                session_id: "batch".into(),
                book_id: "book".into(),
                latest_user_turn_ordinal: 2,
            }],
            "10",
        )
        .unwrap();
    let id = store.review_state().review_jobs[0].job_id.clone();
    store.claim_review_job(&id, "11").unwrap();
    let older = revision("batch", "style", "B", 2_000, &[old.clone()]);
    let mut newer = statement("batch", "style", "C");
    newer.evidence = vec![EvidenceRef::Turn {
        session_id: "batch".into(),
        turn_id: "turn-2".into(),
    }];
    let newer =
        ReviewFactCandidate::revision(newer, vec![old.fact_id.clone()], Some(3_000)).unwrap();
    store
        .commit_review_result(
            &id,
            &["turn".into(), "turn-2".into()],
            &[old],
            &[newer, older],
            &[],
            "30",
        )
        .unwrap();
    let current = store.resolve_profile_facts(&ProfileResolutionContext {
        book_id: Some("book".into()),
        ..Default::default()
    });
    assert_eq!(current.len(), 1);
    assert_eq!(current[0].payload, statement("", "style", "C").payload);
    let job = job(&mut store, "undated");
    let undated = ReviewFactCandidate::revision(
        statement("undated", "style", "D"),
        vec![current[0].fact_id.clone()],
        None,
    )
    .unwrap();
    assert!(store
        .commit_review_result(
            &job.job_id,
            &["turn".into()],
            &current,
            &[undated],
            &[],
            "40"
        )
        .unwrap()
        .added_fact_ids
        .is_empty());
    assert_eq!(
        snapshot(&store).injected_fact_ids(),
        vec![current[0].fact_id.clone()]
    );
}

#[test]
fn assertion_time_formats_and_missing_legacy_fields_are_supported() {
    let expected = Some(1_799_452_800_000);
    assert_eq!(profile_timestamp_ms("1799452800"), expected);
    assert_eq!(profile_timestamp_ms("1799452800000"), expected);
    assert_eq!(profile_timestamp_ms("2027-01-09T00:00:00Z"), expected);
    assert_eq!(profile_timestamp_ms("unknown"), None);
    let dir = tempfile::tempdir().unwrap();
    let mut store = MemoryStore::open(dir.path().join("memory.json")).unwrap();
    let fact = store
        .create_profile_fact(statement("a", "style", "A"), "1")
        .unwrap();
    let mut value = serde_json::to_value(fact).unwrap();
    value.as_object_mut().unwrap().remove("observed_at_ms");
    assert_eq!(
        serde_json::from_value::<ProfileFact>(value)
            .unwrap()
            .observed_at_ms,
        None
    );
}
