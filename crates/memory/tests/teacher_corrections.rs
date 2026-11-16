use memory::{
    learning::LearningStore,
    teacher_corrections::{CorrectionQuote, TeacherCorrection},
    teaching::{TeachingBinding, TeachingEvent, TeachingFact},
};
use serde_json::{json, Value};

fn binding(chat: &str, turn: &str) -> TeachingBinding {
    TeachingBinding {
        tutor_session_id: format!("stage-{chat}"),
        session_revision: 1,
        control_revision: 1,
        source_id: "speed".into(),
        source_revision: "v1".into(),
        map_revision: None,
        chat_session_id: chat.into(),
        turn_id: turn.into(),
    }
}
fn event(
    store: &mut LearningStore,
    binding: &TeachingBinding,
    id: &str,
    kind: TeachingFact,
    payload: Value,
) {
    store
        .append_teaching(&TeachingEvent {
            event_id: id.into(),
            binding: binding.clone(),
            kind,
            causal_refs: vec![],
            payload,
            occurred_at: "2026-11-13T00:00:00Z".into(),
        })
        .unwrap();
}
fn delivered(
    store: &mut LearningStore,
    binding: &TeachingBinding,
    id: &str,
    text: &str,
    incomplete: bool,
) {
    event(
        store,
        binding,
        id,
        TeachingFact::UsageObserved,
        json!({"usage":{"kind":"assistant_delivery"},"answer":text,"incomplete":incomplete}),
    );
}
fn draft(previous: &str) -> TeacherCorrection {
    TeacherCorrection {previous_event_ref:previous.into(),mistake_quote:"Use the mean of speeds.".into(),
        corrected_claim:"Equal distances require total distance divided by total time.".into(),
        applies_when:"Two equal-distance legs, possibly unequal times; not an equal-time arithmetic-mean rule.".into(),
        topics:vec!["average speed".into(),"等路程".into()],
        source_quotes:vec![CorrectionQuote{lid:"1.1".into(),quote:"Average speed is total distance divided by total time.".into()}],supersedes:vec![]}
}
fn search(store: &LearningStore, binding: &TeachingBinding, term: &str) -> Value {
    store
        .search_teacher_corrections(binding, &[term.into()], 0)
        .unwrap()
}
fn saved(
    store: &mut LearningStore,
    chat: &str,
    operation: &str,
    mut correction: TeacherCorrection,
) -> String {
    let old = binding(chat, &format!("wrong-{operation}"));
    let previous = format!("wrong-{chat}-{operation}");
    delivered(store, &old, &previous, &correction.mistake_quote, false);
    correction.previous_event_ref = previous;
    let corrected = binding(chat, operation);
    let result = store
        .record_teacher_correction(
            &corrected,
            operation,
            correction,
            "Why average speed?",
            "now",
        )
        .unwrap();
    delivered(
        store,
        &corrected,
        &format!("corrected-{chat}-{operation}"),
        "Use total distance divided by total time.",
        false,
    );
    result.event_id
}

#[test]
fn corrections_survive_long_history_new_chat_new_stage_and_reopen() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("learning.sqlite");
    let mut store = LearningStore::open(&path, false)
        .unwrap()
        .with_workspace(Some("workspace"));
    let id = saved(&mut store, "old-chat", "speed", draft("unused"));
    for i in 0..40 {
        let mut other = draft("unused");
        other.topics = vec!["colour".into()];
        other.corrected_claim = "The colour is blue.".into();
        other.applies_when = "Discussing paint.".into();
        other.source_quotes[0].quote = "The colour is blue.".into();
        // Request is controlled too: the unrelated records must not match speed.
        let b = binding("noise", &format!("noise-{i}"));
        delivered(
            &mut store,
            &b,
            &format!("noise-old-{i}"),
            &other.mistake_quote,
            false,
        );
        other.previous_event_ref = format!("noise-old-{i}");
        store
            .record_teacher_correction(&b, &format!("noise-{i}"), other, "Discuss paint", "now")
            .unwrap();
    }
    let current = binding("new-chat", "fresh-turn");
    assert_eq!(
        store.agent_working_state(&current).unwrap()["recent_turns"]
            .as_array()
            .map_or(0, Vec::len),
        0
    );
    drop(store);
    let store = LearningStore::open(&path, false)
        .unwrap()
        .with_workspace(Some("workspace"));
    let found = search(&store, &current, "average speed");
    assert_eq!(found["total"], 1);
    assert_eq!(found["candidates"][0]["correction_ref"], id);
    assert!(store
        .current_interpretations("speed", "v1", 0, 20)
        .unwrap()
        .is_empty());
}

#[test]
fn corrections_stay_inside_reader_workspace_source_and_revision() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("learning.sqlite");
    let mut store = LearningStore::open(&path, false)
        .unwrap()
        .with_workspace(Some("one"));
    saved(&mut store, "chat", "correction", draft("unused"));
    let current = binding("other", "fresh");
    let mut wrong = current.clone();
    wrong.source_id = "different".into();
    assert_eq!(search(&store, &wrong, "average speed")["total"], 0);
    wrong = current.clone();
    wrong.source_revision = "v2".into();
    assert_eq!(search(&store, &wrong, "average speed")["total"], 0);
    let other = LearningStore::open(&path, false)
        .unwrap()
        .with_workspace(Some("two"));
    assert_eq!(search(&other, &current, "average speed")["total"], 0);
    let reader = LearningStore::open(&temp.path().join("other-reader.sqlite"), false)
        .unwrap()
        .with_workspace(Some("one"));
    assert_eq!(search(&reader, &current, "average speed")["total"], 0);
}

#[test]
fn only_completed_delivery_activates_replacement_and_old_use_is_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let mut store = LearningStore::open(&temp.path().join("learning.sqlite"), false).unwrap();
    let old = saved(&mut store, "chat", "old", draft("unused"));
    let current = binding("chat", "replacement");
    let mut replacement = draft("wrong-chat-old");
    event(
        &mut store,
        &current,
        "board-part",
        TeachingFact::MessageDelivered,
        json!({"answer":"An earlier committed board part."}),
    );
    replacement.corrected_claim =
        "Use the harmonic mean only for equal distances and positive speeds.".into();
    replacement.supersedes = vec![old.clone()];
    let fresh = store
        .record_teacher_correction(
            &current,
            "replace",
            replacement.clone(),
            "Add the condition",
            "now",
        )
        .unwrap();
    assert_eq!(
        search(&store, &current, "average speed")["candidates"][0]["correction_ref"],
        old
    );
    delivered(&mut store, &current, "partial", "Interrupted draft", true);
    assert_eq!(
        search(&store, &current, "average speed")["candidates"][0]["correction_ref"],
        old
    );
    delivered(
        &mut store,
        &current,
        "complete",
        "Corrected explanation",
        false,
    );
    let found = search(&store, &current, "average speed");
    assert_eq!(found["total"], 1);
    assert_eq!(found["candidates"][0]["correction_ref"], fresh.event_id);
    assert!(store
        .use_teacher_correction(&current, "stale", &old, "apply", "same topic", "now")
        .is_err());
    assert_eq!(
        store
            .record_teacher_correction(
                &current,
                "replace",
                replacement,
                "Add the condition",
                "later"
            )
            .unwrap()
            .event_id,
        fresh.event_id
    );
}

#[test]
fn same_words_return_conditions_not_a_forced_application_or_learner_inference() {
    let temp = tempfile::tempdir().unwrap();
    let mut store = LearningStore::open(&temp.path().join("learning.sqlite"), false).unwrap();
    let id = saved(&mut store, "chat", "old", draft("unused"));
    let current = binding("new", "fresh");
    let found = search(&store, &current, "average speed");
    assert!(found["candidates"][0]["correction"]["applies_when"]
        .as_str()
        .unwrap()
        .contains("equal-distance"));
    let use_event = store
        .use_teacher_correction(
            &current,
            "reject",
            &id,
            "not_applicable",
            "This request specifies equal time intervals, not equal distances.",
            "now",
        )
        .unwrap();
    assert_eq!(use_event.payload["decision"], "not_applicable");
    assert!(store
        .current_interpretations("speed", "v1", 0, 20)
        .unwrap()
        .is_empty());
    assert!(store.state().unwrap().sessions.is_empty());
}

#[test]
fn fabricated_teacher_words_and_cross_source_replacements_are_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("learning.sqlite");
    let mut store = LearningStore::open(&path, false).unwrap();
    let b = binding("chat", "turn");
    delivered(&mut store, &b, "original", "Use total elapsed time.", false);
    assert!(store
        .record_teacher_correction(&b, "bad", draft("original"), "question", "now")
        .is_err());
    let id = saved(&mut store, "chat", "old", draft("unused"));
    let mut replacement = draft("wrong-chat-old");
    replacement.supersedes = vec!["not-an-offered-correction".into()];
    assert!(store
        .record_teacher_correction(&b, "bad-ref", replacement, "question", "now")
        .is_err());
    assert_eq!(
        search(&store, &b, "average speed")["candidates"][0]["correction_ref"],
        id
    );
}

#[test]
fn legacy_reviews_require_a_sourced_final_accept_and_support_contiguous_quotes() {
    let temp = tempfile::tempdir().unwrap();
    let mut store = LearningStore::open(&temp.path().join("learning.sqlite"), false).unwrap();
    let b = binding("legacy", "turn");
    event(
        &mut store,
        &b,
        "bad-draft",
        TeachingFact::AgentWorkReviewed,
        json!({"original_request":"average speed","proposed_delivery":{"text":"Use mean of speeds."},"review":{"decision":"revise","revision":"Use total distance / total time for equal distances."}}),
    );
    assert_eq!(search(&store, &b, "average speed")["total"], 0);
    let mut accepted = json!({"review":{"decision":"accept","agent_work":{"understanding":[{"statement":"Use total distance / total time.","sources":[{"source_id":"speed","source_revision":"v1","lid":"1.1","end_lid":"1.2","quote":"total distance / total time"}]}]}},
        "source_evidence":[{"source_id":"speed","source_revision":"v1","segments":[{"lid":"1.1","text":"total distance / ","source_range":{"start":0,"end":17}},{"lid":"1.2","text":"total time","source_range":{"start":17,"end":27}}]}]});
    event(
        &mut store,
        &b,
        "accepted",
        TeachingFact::AgentWorkReviewed,
        accepted.clone(),
    );
    assert_eq!(search(&store, &b, "average speed")["total"], 1);
    accepted["source_evidence"][0]["segments"][1]["source_range"]["start"] = json!(99);
    event(
        &mut store,
        &b,
        "gapped",
        TeachingFact::AgentWorkReviewed,
        accepted,
    );
    assert_eq!(search(&store, &b, "average speed")["total"], 0);
}

#[test]
fn relevance_is_filtered_before_pagination_and_supersession() {
    let temp = tempfile::tempdir().unwrap();
    let mut store = LearningStore::open(&temp.path().join("learning.sqlite"), false).unwrap();
    for i in 0..9 {
        saved(&mut store, "chat", &format!("old-{i}"), draft("unused"));
    }
    let b = binding("new", "fresh");
    let first = search(&store, &b, "average speed");
    assert_eq!(first["total"], 9);
    assert!(!first["candidates"].as_array().unwrap().is_empty());
    assert_eq!(
        first["next"].as_u64().unwrap() as usize,
        first["candidates"].as_array().unwrap().len()
    );
    assert!(first.to_string().len() < 4096);
    let second = store
        .search_teacher_corrections(
            &b,
            &["average speed".into()],
            first["next"].as_u64().unwrap() as usize,
        )
        .unwrap();
    assert_ne!(
        first["candidates"][0]["correction_ref"],
        second["candidates"][0]["correction_ref"]
    );
    assert_eq!(search(&store, &b, "unrelated")["total"], 0);
    for query in [
        "等路程 平均速度 2uv/(u+v)",
        "equal-distance average speed",
        "讲一下等路程平均速度",
    ] {
        assert_eq!(search(&store, &b, query)["total"], 9, "{query}");
    }
}

#[test]
fn large_correction_is_budgeted_and_exactly_readable_in_chunks() {
    let temp = tempfile::tempdir().unwrap();
    let mut store = LearningStore::open(&temp.path().join("learning.sqlite"), false).unwrap();
    let mut large = draft("unused");
    large.corrected_claim = "等路程时不能平均速度，需分别计算时间。".repeat(70);
    large.source_quotes[0].quote = "原文依据".repeat(390);
    let reference = saved(&mut store, "chat", "large", large.clone());
    let b = binding("new", "turn");
    let preview = search(&store, &b, "average speed");
    assert_eq!(preview["candidates"][0]["details_truncated"], true);
    assert!(preview.to_string().len() < 4096);
    let mut after = 0;
    let mut text = String::new();
    loop {
        let page = store
            .read_teacher_correction(&b, &reference, after)
            .unwrap();
        assert!(page.to_string().len() < 4096);
        text.push_str(page["chunk"].as_str().unwrap());
        let Some(next) = page["next"].as_u64() else {
            break;
        };
        after = next as usize;
    }
    let restored: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        restored["correction"]["source_quotes"][0]["quote"],
        large.source_quotes[0].quote
    );
    assert_eq!(
        restored["correction"]["corrected_claim"],
        large.corrected_claim
    );
}

#[test]
#[ignore = "isolated SQLite measurement before/after the source-kind index; run with --nocapture"]
fn teacher_corrections_query_cost() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("learning.sqlite");
    let mut store = LearningStore::open(&path, false).unwrap();
    // Establish the same baseline even after production gains the index.
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute_batch("DROP INDEX IF EXISTS teaching_trace_correction_source")
        .unwrap();
    for i in 0..1500 {
        // Ordinary conversations share one observation stage: distribute the
        // corrections through that history, rather than putting them all first.
        if i % 64 == 0 {
            saved(&mut store, "chat", &format!("fix-{i}"), draft("unused"));
        }
        event(
            &mut store,
            &binding("chat", &format!("noise-{i}")),
            &format!("noise-{i}"),
            TeachingFact::TurnBound,
            json!({"context":"Actual historical material and dialogue. ".repeat(220)}),
        );
    }
    let b = binding("new", "fresh");
    let measure = || {
        let mut hints = Vec::new();
        let mut searches = Vec::new();
        for _ in 0..12 {
            let start = std::time::Instant::now();
            assert_eq!(
                store.teacher_correction_hint(&b).unwrap()["record_count"],
                24
            );
            hints.push(start.elapsed().as_secs_f64() * 1000.0);
            let start = std::time::Instant::now();
            assert_eq!(search(&store, &b, "average speed")["total"], 24);
            searches.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        json!({"hint_ms":hints,"search_ms":searches})
    };
    let before = measure();
    db.execute_batch("CREATE INDEX teaching_trace_correction_source ON teaching_trace(learning_workspace_id,
        json_extract(event,'$.binding.source_id'),json_extract(event,'$.binding.source_revision'),json_extract(event,'$.kind'),json_extract(event,'$.binding.turn_id'))").unwrap();
    let after = measure();
    println!(
        "TEACHER_CORRECTION_QUERY_COST {}",
        json!({"historical_events":1500,"corrections":24,"before":before,"after":after})
    );
}
