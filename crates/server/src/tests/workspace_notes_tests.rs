use super::mu5_tests::Fixture;
use super::*;
use runtime::run_context::ResidentStatePort;

#[test]
fn workspace_notes_account_notebook_works_without_material_and_checks_scope_and_revision() {
    let f = Fixture::new();
    let scope = f.ok(
        &f.a,
        "POST",
        "/api/learning-workspaces",
        json!({"goal":{"text":"记录自己的想法"}}),
    )["learning_workspace_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let path = |operation: &str| format!("/api/learning-workspaces/{scope}/notes/{operation}");
    let note = f.ok(&f.a,"POST",&path("create"),json!({"title":"一个想法","content":"独立于书籍","learning_workspace_id":"untrusted-other-scope","author":"assistant"}));
    assert_eq!(note["learning_workspace_id"], scope);
    assert_eq!(note["author"], "user");
    let second = f.ok(
        &f.a,
        "POST",
        &path("create"),
        json!({"title":"一个例子","content":"这个例子支持想法"}),
    );
    let relation = f.ok(&f.a,"POST",&path("relation_save"),json!({"from_note_id":second["note_id"],"to_note_id":note["note_id"],"label":"支持","reason":"它把抽象想法变成了具体情境"}));
    let edited = f.ok(&f.a,"POST",&path("update"),json!({"note_id":note["note_id"],"note_revision":1,"title":"修改后的想法","content":"补充解释"}));
    assert_eq!(edited["note_id"], note["note_id"]);
    assert_eq!(edited["revision"], 2);
    let stale = f.call(&f.a,"POST",&path("update"),json!({"note_id":note["note_id"],"note_revision":1,"title":"过期窗口","content":"不应覆盖"}));
    assert_eq!(stale.0, 409);
    assert_eq!(stale.1["error_code"], "NOTE_REVISION_CONFLICT");
    assert_eq!(
        f.ok(
            &f.a,
            "POST",
            &path("read"),
            json!({"note_id":note["note_id"]})
        ),
        edited
    );
    assert_eq!(
        f.call(
            &f.b,
            "POST",
            &path("read"),
            json!({"note_id":note["note_id"]})
        )
        .0,
        404
    );
    let other = f.ok(
        &f.a,
        "POST",
        "/api/learning-workspaces",
        json!({"goal":{"text":"另外的学习目标"}}),
    )["learning_workspace_id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(
        f.call(
            &f.a,
            "POST",
            &format!("/api/learning-workspaces/{other}/notes/read"),
            json!({"note_id":note["note_id"]})
        )
        .0,
        404
    );
    assert_eq!(
        f.ok(&f.a, "POST", &path("list"), json!({}))["relations"][0],
        relation
    );
    f.ok(
        &f.a,
        "POST",
        &path("delete"),
        json!({"note_id":note["note_id"],"note_revision":2}),
    );
    let notebook = f.ok(&f.a, "POST", &path("list"), json!({}));
    assert_eq!(notebook["notes"].as_array().unwrap().len(), 1);
    assert!(notebook["relations"].as_array().unwrap().is_empty());
    f.ok(
        &f.a,
        "POST",
        &format!("/api/learning-workspaces/{scope}/archive"),
        json!({"archived":true}),
    );
    assert_eq!(f.ok(&f.a, "POST", &path("list"), json!({})), notebook);
    assert_eq!(
        f.call(
            &f.a,
            "POST",
            &path("create"),
            json!({"title":"不能写入","content":"已归档"})
        )
        .0,
        409
    );
}

#[test]
fn workspace_notes_resident_windows_and_agent_share_the_same_workspace_notebook() {
    let f = Fixture::new();
    let scope = f.ok(
        &f.a,
        "POST",
        "/api/learning-workspaces",
        json!({"goal":{"text":"跨书连接想法"},"members":[f.x,f.y]}),
    )["learning_workspace_id"]
        .clone();
    let first = f.ok(
        &f.a,
        "POST",
        "/api/workspaces",
        json!({"published_book_ref":f.x,"attachment_id":"first","learning_workspace_id":scope}),
    );
    let second = f.ok(
        &f.a,
        "POST",
        "/api/workspaces",
        json!({"published_book_ref":f.y,"attachment_id":"second","learning_workspace_id":scope}),
    );
    let note = f.action(
        &f.a,
        &first,
        "first",
        "notes/create",
        json!({"title":"我的理解","content":"这是用户原文"}),
    )["result"]
        .clone();
    assert_eq!(
        f.action(
            &f.a,
            &second,
            "second",
            "notes/read",
            json!({"note_id":note["note_id"]})
        )["result"],
        note
    );
    let second = f.action(&f.a, &second, "second", "chat/new", json!({}));
    let mut agent = f.run(&second, "second");
    assert!(agent.workspace_notes_available());
    let list = agent
        .workspace_notes(json!({"action":"list","query":"用户","limit":1}))
        .unwrap();
    assert_eq!(list["notes"][0]["note_id"], note["note_id"]);
    let edited = agent.workspace_notes(json!({"action":"update","note_id":note["note_id"],"expected_revision":1,"title":"整理后的理解","content":"Agent 补充了推理"})).unwrap();
    assert_eq!(edited["author"], "user");
    assert_eq!(edited["updated_by"], "assistant");
    let long_text = "你".repeat(9000);
    let created = agent
        .workspace_notes(json!({"action":"create","title":"Agent 的补充","content":long_text}))
        .unwrap();
    assert_eq!(created["author"], "assistant");
    let read = agent
        .workspace_notes(json!({"action":"read","note_id":created["note_id"],"length":1000}))
        .unwrap();
    assert_eq!(
        read["note"]["content"].as_str().unwrap().chars().count(),
        1000
    );
    assert_eq!(read["next_offset"], 1000);
    assert_eq!(read["total_characters"], 9000);
    let relation = agent.workspace_notes(json!({"action":"relation_save","from_note_id":created["note_id"],"to_note_id":note["note_id"],"label":"补充","reason":"增加了原来尚未展开的推理"})).unwrap();
    let latest = f.action(
        &f.a,
        &first,
        "first",
        "notes/read",
        json!({"note_id":note["note_id"]}),
    );
    assert_eq!(latest["result"], edited);
    let read = agent
        .workspace_notes(json!({"action":"read","note_id":note["note_id"]}))
        .unwrap();
    assert_eq!(read["relations"][0], relation);
    assert_eq!(agent.workspace_notes(json!({"action":"update","note_id":note["note_id"],"expected_revision":1,"title":"旧版本","content":"不覆盖新内容"})).unwrap_err().error_code,"NOTE_REVISION_CONFLICT");
    let no_scope = f.create(&f.a, &f.x, "unscoped");
    let no_scope = f.action(&f.a, &no_scope, "unscoped", "chat/new", json!({}));
    assert!(!f.run(&no_scope, "unscoped").workspace_notes_available());
}

#[test]
fn workspace_notes_imports_preserve_delivered_sources_and_reading_authorship() {
    let (_root, mut state, metadata) = super::board_browser_host::fixture();
    state.workspace.learning_workspace_id = Some("notebook-imports".into());
    for session in &mut state.user.agent_history.sessions {
        session.learning_workspace_id = Some("notebook-imports".into());
    }
    let call = |state: &mut AppState, operation: &str, input: Value| {
        let reply = crate::route(
            state,
            Req {
                method: "POST",
                url: &format!("/notes/{operation}"),
                body: &input.to_string(),
                now: "2026-10-27",
            },
        );
        assert_eq!(reply.status, 200, "{}", reply.body);
        serde_json::from_str::<Value>(&reply.body).unwrap()
    };
    let answer = call(
        &mut state,
        "import",
        json!({"source":metadata["sources"]["answer"]}),
    );
    assert_eq!(answer["author"], "assistant");
    assert_eq!(answer["origin"]["turn_id"], metadata["turn_id"]);
    assert!(answer["content"]
        .as_str()
        .unwrap()
        .contains("慢的路段占用了更多时间"));
    assert!(!answer["source_bindings"].as_array().unwrap().is_empty());
    let viewport = state.workspace.reader.viewport().top_lid.clone();
    let window = call(&mut state, "source.read", json!({
        "note_id": answer["note_id"], "source_ref_id": answer["source_bindings"][0]["source_ref_id"]
    }));
    assert!(window["segments"].as_array().unwrap().iter()
        .any(|segment| !segment["highlights"].as_array().unwrap().is_empty()));
    assert_eq!(state.workspace.reader.viewport().top_lid, viewport);
    let original_turn = state.user.agent_history.sessions[0].turns[0].clone();
    state.workspace.learning_workspace_id = Some("another-workspace".into());
    assert!(crate::workspace_notes_api::route(
        &state.user,
        &state.workspace,
        "import",
        &json!({"source":metadata["sources"]["answer"]}),
        "now"
    )
    .is_err());
    state.workspace.learning_workspace_id = Some("notebook-imports".into());
    assert_eq!(
        state.user.agent_history.sessions[0].turns[0].turn_id,
        original_turn.turn_id
    );

    let placement = memory::NoteBodyPlacement::LidBlock {
        source_fingerprint: state.workspace.book.source_fingerprint().into(),
        lid: "1.1".into(),
    };
    let make_input = |content: &str, structured: bool| SaveInput {
        mem_id: None,
        mem_type: "note".into(),
        layer: "long_term".into(),
        book_id: state.workspace.book.base.book_id.clone(),
        anchor: Anchor {
            lid: Some("1.1".into()),
            concept: None,
        },
        content: content.into(),
        range: None,
        selection_context: None,
        note_placement: structured.then_some(placement.clone()),
        note: structured.then_some(memory::NoteData {
            material: memory::NoteMaterial {
                book_id: state.workspace.book.base.book_id.clone(),
                publication_id: None,
            },
            association: memory::NoteAssociation::BodyPlacement {
                placement: placement.clone(),
            },
            retained_excerpt: None,
            source_bindings: vec![],
        }),
        citations: None,
        source_session_id: None,
    };
    let structured_input = make_input("用户自己的阅读理解", true);
    let legacy_input = make_input("旧笔记的原文", false);
    let structured = state.user.store.save(structured_input, "now").unwrap();
    let legacy = state.user.store.save(legacy_input, "now").unwrap();
    let note = call(
        &mut state,
        "import",
        json!({"source":{"kind":"reading_note","mem_id":structured.mem_id}}),
    );
    assert_eq!(note["author"], "user");
    assert_eq!(note["content"], structured.content);
    assert_eq!(note["origin"]["mem_id"], structured.mem_id);
    let old = call(
        &mut state,
        "import",
        json!({"source":{"kind":"reading_note","mem_id":legacy.mem_id}}),
    );
    assert_eq!(old["author"], "unknown");
    assert_eq!(old["content"], legacy.content);
    state.user.agent_history.sessions.clear();
    let source = call(
        &mut state,
        "source.resolve",
        json!({"note_id":answer["note_id"],"source_ref_id":answer["source_bindings"][0]["source_ref_id"]}),
    );
    assert_eq!(source["stale"], false);
    assert_eq!(source["can_open_in_reader"], true);
}

#[test]
fn workspace_notes_imports_restore_original_publication_for_implicit_sources() {
    let f = Fixture::new();
    let original = f.create(&f.a, &f.x, "original-note-source");
    let original = f.action(&f.a, &original, "original-note-source", "chat/new", json!({}));
    let port = f.run(&original, "original-note-source");
    let handle = f.access.users.lock().unwrap().get("A", "now").unwrap();
    let make_binding = |reference: &base_schema::PublishedBookRef, id: &str| {
        let publication = f.access.library.lock().unwrap().load("A", reference).unwrap();
        let range = EvidenceRange { start_lid: "1.1".into(), end_lid: "1.1".into(), ranges: vec![] };
        let resolved = publication.book.resolve_source(&range, "zh-CN", None).unwrap();
        SourceBinding {
            source_ref_id: id.into(), book_id: reference.book_id.clone(),
            publication_id: None, source_revision: Some(publication.book.source_fingerprint().into()),
            evidence_range: Some(range), evidence_text_digest: resolved.evidence_text_digest,
            media: None, music: None, label_snapshot: resolved.label, preview_snapshot: resolved.preview,
        }
    };
    let implicit = make_binding(&f.x, "ordinary-source");
    let mut explicit = make_binding(&f.y, "explicit-other-book");
    explicit.publication_id = Some(f.y.publication_id.clone());
    let turn = {
        let mut user = handle.lock().unwrap();
        let mut session = user.agent_history.sessions.iter()
            .find(|session| session.id == port.scope.chat_session_id).unwrap().clone();
        assert!(session.learning_workspace_id.is_none());
        let current = &mut session.turns[0];
        assert_eq!(current.published_book_ref.as_ref(), Some(&f.x));
        current.status = AgentAssistantStatus::Completed;
        current.source_bindings = vec![implicit, explicit];
        current.outcome = Some(OuterOutcome {
            answer: Some("原始阅读回合的解释 [[source:ordinary-source]]".into()), answer_view: None,
            incomplete: false, warning: None, turns: 1, tokens_spent: 0, effects: vec![], trace: vec![],
            profile_usage: Default::default(), memory_updates: vec![], source_bindings: current.source_bindings.clone(),
            delivery_diagnostics: None, request_audit: Default::default(), teaching_delivery_move_refs: vec![],
        });
        let turn = AgentTurnRef {
            session_id: session.id.clone(), turn_id: current.turn_id.clone(), user_turn_ordinal: current.user_turn_ordinal,
        };
        reader_service::tests::host_calls::session_runtime::finish(&mut user, &turn, &session, "finished").unwrap();
        turn
    };
    drop(port);
    let original = f.get(&f.a, &original);
    let reading = f.action(&f.a, &original, "original-note-source", "memory/save", json!({
        "type":"note", "content":"把原始回答记到阅读笔记", "note":{"association":{
            "kind":"answer", "session_id":turn.session_id, "turn_id":turn.turn_id,
        }},
    }))["result"]["record"].clone();
    assert!(reading["note"]["source_bindings"][0]["publication_id"].is_null());
    let scope = f.ok(&f.a, "POST", "/api/learning-workspaces", json!({
        "goal":{"text":"复用原始阅读笔记"}, "members":[f.x,f.y],
    }))["learning_workspace_id"].as_str().unwrap().to_owned();
    let records = f.ok(&f.a, "GET", &format!("/api/me/learning-records/{}", turn.session_id), Value::Null);
    f.ok(&f.a, "POST", &format!("/api/learning-workspaces/{scope}/records"), json!({
        "session_id":turn.session_id, "through_seq":records["through_seq"], "goal_revision":1,
    }));
    let open = |reference: &base_schema::PublishedBookRef, attachment: &str| f.ok(&f.a, "POST", "/api/workspaces", json!({
        "published_book_ref":reference, "learning_workspace_id":scope, "attachment_id":attachment,
    }));
    let other = open(&f.y, "other-note-book");
    let imported = f.action(&f.a, &other, "other-note-book", "notes/import", json!({
        "source":{"kind":"answer", "session_id":turn.session_id, "turn_id":turn.turn_id},
    }))["result"].clone();
    assert_eq!(imported["origin"]["publication_id"], f.x.publication_id);
    assert_eq!(imported["source_bindings"][0]["publication_id"], f.x.publication_id);
    assert_eq!(imported["source_bindings"][1]["publication_id"], f.y.publication_id);

    let current = open(&f.x, "reading-note-book");
    let imported_reading = f.action(&f.a, &current, "reading-note-book", "notes/import", json!({
        "source":{"kind":"reading_note", "mem_id":reading["mem_id"]},
    }))["result"].clone();
    assert_eq!(imported_reading["source_bindings"][0]["publication_id"], f.x.publication_id);

    // Existing notebook files retain the original publication in origin already.
    let retained = {
        let user = handle.lock().unwrap();
        let store = reader_service::tests::host_calls::workspace_notes_api::scoped_store(&user, Some(&scope)).unwrap();
        let mut note: memory::workspace_notes::WorkspaceNote = serde_json::from_value(imported.clone()).unwrap();
        note.note_id = "note-before-publication-fix".into();
        note.source_bindings[0].publication_id = None;
        store.create(note).unwrap()
    };
    for note_id in [imported["note_id"].as_str().unwrap(), imported_reading["note_id"].as_str().unwrap(), &retained.note_id] {
        let input = json!({"note_id":note_id,"source_ref_id":"ordinary-source"});
        let resolved = f.action(&f.a, &other, "other-note-book", "notes/source.resolve", input.clone());
        assert_eq!(resolved["result"]["stale"], false);
        assert!(resolved["result"]["source_url"].as_str().unwrap().contains(&f.x.publication_id));
        let read = f.action(&f.a, &other, "other-note-book", "notes/source.read", input.clone());
        assert_eq!(read["result"]["published_book_ref"], json!(f.x));
        assert!(read["result"]["segments"].as_array().unwrap().iter()
            .any(|segment| !segment["highlights"].as_array().unwrap().is_empty()));
        let opened = f.action(&f.a, &other, "other-note-book", "notes/source.open", input);
        assert!(opened["result"]["source_url"].as_str().unwrap().contains(&f.x.publication_id));
        assert_eq!(f.get(&f.a, &other)["published_book_ref"], json!(f.y));
    }
    let user = handle.lock().unwrap();
    let store = reader_service::tests::host_calls::workspace_notes_api::scoped_store(&user, Some(&scope)).unwrap();
    assert_eq!(store.read(&retained.note_id).unwrap(), retained, "source reads must not rewrite saved revisions");
}
