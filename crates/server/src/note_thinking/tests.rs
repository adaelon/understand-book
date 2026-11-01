use super::*;
use memory::teaching::{TeachingBinding, TeachingEvent, TeachingFact, TeachingTarget};

fn fixture() -> (tempfile::TempDir, UserRuntime, String, LearningEvidence) {
    let root = tempfile::tempdir().unwrap();
    let mut user = UserRuntime::local(
        memory::MemoryStore::open(root.path().join("memory.json")).unwrap(),
        None,
        crate::AgentHistory::default(),
        None,
    );
    let note = user
        .store
        .create_personal_note(
            serde_json::from_value(
                json!({"content":"停车的时候没运动，这段时间应该不算吧？","authorship":"user"}),
            )
            .unwrap(),
            "note",
            "2026-11-01T01:00:00Z",
        )
        .unwrap();
    let reference = NoteRef {
        note_id: note.note_id.clone(),
        revision: Some(1),
    };
    let b = TeachingBinding {
        tutor_session_id: "stage".into(),
        session_revision: 1,
        control_revision: 1,
        source_id: "book".into(),
        source_revision: "v1".into(),
        map_revision: None,
        chat_session_id: "chat".into(),
        turn_id: "turn".into(),
    };
    let text = "我觉得停车时没有走距离，所以不用计入总时间。";
    let mut turn: crate::AgentChatTurn =
        serde_json::from_value(json!({"turn_id":"turn","user":text})).unwrap();
    turn.domain.personal_notes = Some(json!({"references":[reference]}));
    let session:crate::AgentChatSession=serde_json::from_value(json!({"id":"chat","learning_workspace_id":"scope-a","book_id":"book","title":"这段行程的时间","created_at":"1","updated_at":"2","turns":[turn],"messages":[]})).unwrap();
    user.agent_history.sessions.push(session);
    user.store
        .record_note_discussion(memory::NoteDiscussion {
            reference: reference.clone(),
            turn: memory::NoteTurnRef {
                session_id: "chat".into(),
                turn_id: "turn".into(),
            },
            created_at: "2026-11-01T01:02:00Z".into(),
            expression: Some(text.into()),
        })
        .unwrap();
    let mut store = user.learning_store_for_workspace(Some("scope-a")).unwrap();
    store
        .append_teaching(&TeachingEvent {
            event_id: "expression".into(),
            binding: b.clone(),
            kind: TeachingFact::UsageObserved,
            causal_refs: vec![],
            payload: json!({"usage":{"kind":"user_message"},"response":text}),
            occurred_at: "2026-11-01T01:02:00Z".into(),
        })
        .unwrap();
    let mut target:TeachingTarget=serde_json::from_value(json!({"learning_focus":"总时间包含停车时间","expected_performance":"区分运动时间与全程时间","capability":"explanation","source_bindings":[{"source_id":"book","source_revision":"v1","start_lid":"1.1","end_lid":"1.1"}]})).unwrap();
    store.bind_focus(&mut target, &b, "focus:elapsed").unwrap();
    let e:LearningEvidence=serde_json::from_value(json!({"evidence_id":"observation","action_ref":"expression","session_id":"stage","source_id":"book","source_revision":"v1","nature":"hypothesis","target":target,"fact_refs":["expression"],"teaching_implication":"用出发到到达的时间线解释停车为何计入","label":target.learning_focus,"capability":"explanation","assistance_refs":[],"attempt":0,"status":"unassessed","interpretation":"将是否移动作为全程时间的边界","learner_quote":text,"interpreter_version":"resident.hypothesis.v2","source_quotes":[],"understanding":{"difficulties":["仍把停止移动看成行程计时结束"],"unconfirmed":["改变停车时长后能否判断平均速度"]},"note_refs":[reference]})).unwrap();
    store.append_evidence(&e).unwrap();
    (root, user, note.note_id, e)
}

#[test]
fn note_thinking_links_exact_versions_and_corrections_without_editing_user_text() {
    let (_root, mut user, id, e) = fixture();
    let initial = page(&user, &json!({"note_id":id})).unwrap();
    assert_eq!(
        initial["current_understanding"][0]["difficulties"][0],
        "仍把停止移动看成行程计时结束"
    );
    assert_eq!(initial["timeline"].as_array().unwrap().len(), 3);
    let request = json!({"note_id":id,"workspace_id":"scope-a","evidence_ref":e.evidence_id});
    let action = start(&user, &request).unwrap();
    assert_eq!(action["note_ref"]["revision"], 1);
    assert_eq!(action["target"]["focus_ref"], "focus:elapsed");
    assert_eq!(action["context"]["workspace_id"], "scope-a");
    let corrected=evidence(&user,&json!({"note_id":id,"workspace_id":"scope-a","evidence_ref":e.evidence_id,"operation_id":"correction","text":"我只是在试探这个说法，不是确定不算"}),true).unwrap();
    let next = page(&user, &json!({"note_id":id})).unwrap();
    assert!(next["current_understanding"][0]["difficulties"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(
        next["current_understanding"][0]["evidence_ref"],
        corrected["evidence_ref"]
    );
    assert_eq!(
        user.store
            .personal_note(&NoteRef {
                note_id: id.clone(),
                revision: None
            })
            .unwrap()
            .current_revision,
        1
    );
    assert_eq!(next["timeline"][0]["kind"], "correction");
    let second = user
        .store
        .create_personal_note(
            serde_json::from_value(json!({"content":"一个不同的想法","authorship":"user"}))
                .unwrap(),
            "second",
            "3",
        )
        .unwrap();
    assert!(evidence(
        &user,
        &json!({"note_id":second.note_id,"workspace_id":"scope-a","evidence_ref":e.evidence_id}),
        false
    )
    .is_err());
    let version = user
        .store
        .revise_personal_note(
            &id,
            1,
            serde_json::from_value(
                json!({"content":"现在想研究加速度，而不是这次行程的平均速度","authorship":"user"}),
            )
            .unwrap(),
            "edit",
            "2026-11-01T01:05:00Z",
        )
        .unwrap();
    assert_eq!(version.current_revision, 2);
    let changed = page(&user, &json!({"note_id":id})).unwrap();
    assert_eq!(
        changed["current_understanding"][0]["note_refs"][0]["revision"],
        1
    );
    assert!(
        start(&user, &request).is_err(),
        "the superseded judgment cannot be resumed as current"
    );
}

#[test]
fn note_thinking_attribution_rejects_unoffered_notes_and_wrong_version_quotes() {
    let (_root, user, id, _) = fixture();
    let binding = user
        .learning_store_for_workspace(Some("scope-a"))
        .unwrap()
        .teaching_event("expression")
        .unwrap()
        .binding;
    let exact = json!([{"reference":{"note_id":id,"revision":1},"quote":"停车的时候没运动"}]);
    assert_eq!(
        crate::personal_note_context::evidence_notes(&user, &binding, Some(&exact))
            .unwrap()
            .len(),
        1
    );
    let wrong = json!([{"reference":{"note_id":id,"revision":1},"quote":"停车计入总时间"}]);
    assert!(crate::personal_note_context::evidence_notes(&user, &binding, Some(&wrong)).is_err());
    assert!(crate::personal_note_context::evidence_notes(
        &user,
        &binding,
        Some(&json!([{ "reference":{"note_id":id,"revision":2},"quote":"停车的时候没运动"}]))
    )
    .is_err());
}

#[test]
fn note_thinking_saved_or_copied_text_alone_never_creates_understanding() {
    let (_root, mut user, _, _) = fixture();
    for authorship in [
        "user",
        "assistant_draft",
        "adopted_assistant",
        "material_excerpt",
    ] {
        let note = user
            .store
            .create_personal_note(
                serde_json::from_value(
                    json!({"content":"平均速度等于总距离除以总时间","authorship":authorship}),
                )
                .unwrap(),
                authorship,
                "1",
            )
            .unwrap();
        let view = page(&user, &json!({"note_id":note.note_id})).unwrap();
        assert!(view["current_understanding"].as_array().unwrap().is_empty());
        assert_eq!(view["timeline"].as_array().unwrap().len(), 1);
    }
}

#[test]
#[ignore = "Exports actual API views for browser acceptance; UNDERSTANDING_OUTPUT selects directory"]
fn note_thinking_export_browser_fixture() {
    let (_root, mut user, id, e) = fixture();
    user.store.revise_personal_note(&id,1,serde_json::from_value(json!({"title":"停车的时候没运动，这段时间应该不算吧？","content":"我原来觉得，只有移动时才应该计时。现在我想把整段行程画出来，再看看停车到底属于哪一段。","authorship":"user","based_on_turn":{"session_id":"chat","turn_id":"turn"}})).unwrap(),"browser-version","2026-11-01T01:05:00Z").unwrap();
    let request = json!({"note_id":id,"workspace_id":"scope-a","evidence_ref":e.evidence_id});
    let initial = page(&user, &json!({"note_id":id})).unwrap();
    let original_evidence = evidence(&user, &request, false).unwrap();
    let action = start(&user, &request).unwrap();
    let versions = user.store.personal_note_history(&id, None, 20).unwrap();
    let note = user
        .store
        .personal_note(&NoteRef {
            note_id: id.clone(),
            revision: None,
        })
        .unwrap();
    evidence(&user,&json!({"note_id":id,"workspace_id":"scope-a","evidence_ref":e.evidence_id,"operation_id":"browser-correction","text":"我是在求证，还没有确定停车不算。请保留为待确认。"}),true).unwrap();
    let output = json!({"note":note,"history":{"revisions":versions,"discussions":user.store.note_discussions(&id)},
        "thinking":initial,"evidence":original_evidence,"action":action,"corrected":page(&user,&json!({"note_id":id})).unwrap()});
    let directory =
        std::path::PathBuf::from(std::env::var("UNDERSTANDING_OUTPUT").expect("output directory"));
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(
        directory.join("browser-fixture.json"),
        serde_json::to_vec_pretty(&output).unwrap(),
    )
    .unwrap();
}

#[test]
#[ignore = "Measures retained-note reads at the accepted PN scale; UNDERSTANDING_OUTPUT selects directory"]
fn note_thinking_read_cost_at_one_thousand_notes() {
    let (root, mut user, id, seed) = fixture();
    let path = root.path().join("memory.json");
    let history = std::mem::take(&mut user.agent_history);
    drop(user);
    let mut document: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let template = document["notes"][0].clone();
    let notes: Vec<_> = (0..1000)
        .map(|n| {
            let mut note = template.clone();
            let name = if n == 0 {
                id.clone()
            } else {
                format!("scale-note-{n}")
            };
            note["note_id"] = json!(name);
            note["current_revision"] = json!(3);
            note["revisions"] = json!((1..=3)
                .map(|revision| {
                    let mut v = template["revisions"][0].clone();
                    v["revision"] = json!(revision);
                    v["record"]["mem_id"] = json!(name);
                    v["record"]["content"] = json!(format!(
                        "{} 第 {revision} 次表述",
                        v["record"]["content"].as_str().unwrap().repeat(20)
                    ));
                    v
                })
                .collect::<Vec<_>>());
            note
        })
        .collect();
    document["notes"] = json!(notes);
    std::fs::write(&path, serde_json::to_vec(&document).unwrap()).unwrap();
    let mut user = UserRuntime::local(
        memory::MemoryStore::open(&path).unwrap(),
        None,
        history,
        None,
    );
    user.store
        .revise_personal_note(
            &id,
            3,
            serde_json::from_value(
                json!({"content":"现在我开始按行程的起止理解总时间。","authorship":"user"}),
            )
            .unwrap(),
            "scale-edit",
            "2026-11-01T01:09:00Z",
        )
        .unwrap();
    let mut connection = rusqlite::Connection::open(root.path().join("learning.db")).unwrap();
    let tx = connection.transaction().unwrap();
    let template_fact = user
        .learning_store_for_workspace(Some("scope-a"))
        .unwrap()
        .teaching_event(&seed.action_ref)
        .unwrap();
    for n in 1..1000 {
        for revision in 1..=3 {
            let mut fact = template_fact.clone();
            fact.event_id = format!("scale-fact-{n}-{revision}");
            fact.binding.turn_id = fact.event_id.clone();
            tx.execute("INSERT INTO teaching_trace(learning_workspace_id,event_id,session_id,event) VALUES(?,?,?,?)",rusqlite::params!["scope-a",fact.event_id,fact.binding.tutor_session_id,serde_json::to_string(&fact).unwrap()]).unwrap();
            let mut e = seed.clone();
            e.evidence_id = format!("scale-evidence-{n}-{revision}");
            e.action_ref = fact.event_id.clone();
            e.fact_refs = vec![fact.event_id];
            e.note_refs = vec![NoteRef {
                note_id: format!("scale-note-{n}"),
                revision: Some(revision),
            }];
            e.target.as_mut().unwrap().focus_ref = Some(format!("scale-focus-{n}"));
            tx.execute("INSERT INTO learning_evidence(learning_workspace_id,evidence_id,source_id,evidence) VALUES(?,?,?,?)",rusqlite::params!["scope-a",e.evidence_id,e.source_id,serde_json::to_string(&e).unwrap()]).unwrap();
            tx.execute("INSERT INTO understanding_note_evidence(note_id,revision,learning_workspace_id,evidence_id) VALUES(?,?,?,?)",rusqlite::params![e.note_refs[0].note_id,revision,"scope-a",e.evidence_id]).unwrap();
        }
    }
    tx.commit().unwrap();
    drop(connection);
    let mut elapsed = Vec::new();
    for _ in 0..5 {
        let start = std::time::Instant::now();
        let view = page(&user, &json!({"note_id":id})).unwrap();
        elapsed.push(start.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(view["current_revision"], 4);
        assert_eq!(view["current_understanding"].as_array().unwrap().len(), 1);
        assert_eq!(view["timeline"].as_array().unwrap().len(), 6);
    }
    let report = json!({"notes":1000,"revisions":3001,"learning_evidence":2998,"thinking_read_ms":elapsed,"model_calls_on_read":0,"build":"debug","note":"First read followed by four warm reads; private fixture, no model adapter"});
    let directory =
        std::path::PathBuf::from(std::env::var("UNDERSTANDING_OUTPUT").expect("output directory"));
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(
        directory.join("read-cost.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    eprintln!("{report}");
}
