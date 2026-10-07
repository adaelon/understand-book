use super::*;

fn call(port: &mut impl ResidentStatePort, value: Value) -> Result<AuthorResult, ToolError> {
    let request = serde_json::from_value(value).expect("author tool contract");
    port.author_presentation(request, &[], &[], &CancellationToken::default())
}

#[test]
fn ex13_saved_candidates_remain_readable_across_patch_delivery_and_run_boundary() {
    let (_root, mut state, turn) = setup();
    let app = BorrowedAppPort(std::cell::RefCell::new(&mut state));
    let scope = app.with_app(|state| crate::run_scope::RunScope::capture(state, &turn, "test", None, None));
    let mut port = RuntimeStatePort { scope: &scope, port: &app, turn_ref: &turn,
        previewed: Default::default(), animations: Default::default(), plots: Default::default() };
    let first = candidate(&mut port, "<p>first</p>");
    let independent = candidate(&mut port, "<p>independent</p>");
    let patched = call(&mut port, json!({"operation":"patch","candidate_id":first,
        "edits":[{"old_text":"first","new_text":"second"}]})).unwrap();
    let second = patched.body["candidate_id"].as_str().unwrap().to_owned();
    let patched = call(&mut port, json!({"operation":"patch","candidate_id":second,
        "edits":[{"old_text":"second","new_text":"third"}]})).unwrap();
    let third = patched.body["candidate_id"].as_str().unwrap().to_owned();
    assert!(call(&mut port, json!({"operation":"deliver","candidate_id":third})).is_err());
    port.previewed.insert(third.clone(), std::collections::HashSet::from(["legacy".into()]));
    let delivered = call(&mut port, json!({"operation":"deliver","candidate_id":third})).unwrap();
    let candidates = [(first, "first"), (independent, "independent"), (second, "second"), (third, "third")];
    for (id, text) in &candidates {
        let read = call(&mut port, json!({"operation":"read","candidate_id":id,"file":"index.html"})).unwrap();
        assert_eq!(read.body["text"], format!("<p>{text}</p>"));
    }
    let mut later = turn.clone();
    later.turn_id = "different-run".into();
    let mut port = RuntimeStatePort { scope: &scope, port: &app, turn_ref: &later,
        previewed: Default::default(), animations: Default::default(), plots: Default::default() };
    for (id, _) in &candidates {
        assert!(call(&mut port, json!({"operation":"read","candidate_id":id})).err().unwrap().message.contains("different run"));
        assert!(call(&mut port, json!({"operation":"search","candidate_id":id,"query":"p"})).err().unwrap().message.contains("different run"));
        assert!(call(&mut port, json!({"operation":"patch","candidate_id":id,"edits":[{"old_text":"<p>","new_text":"<h1>"}]})).err().unwrap().message.contains("different run"));
        assert!(call(&mut port, json!({"operation":"deliver","candidate_id":id})).err().unwrap().message.contains("Preview"));
    }
    let read = call(&mut port, json!({"operation":"read","reference":delivered.delivered.unwrap(),"file":"index.html"})).unwrap();
    assert_eq!(read.body["text"], "<p>third</p>");
}

#[test]
fn presentation_edit_reads_complete_source_and_explicit_unicode_range() {
    let (_root, mut state, turn) = setup();
    let app = BorrowedAppPort(std::cell::RefCell::new(&mut state));
    let scope = app.with_app(|state| crate::run_scope::RunScope::capture(state, &turn, "test", None, None));
    let mut port = RuntimeStatePort { scope: &scope,
        port: &app,
        turn_ref: &turn,
        previewed: Default::default(),
        animations: Default::default(),
        plots: Default::default(),
    };
    let html = format!("{}<!--{}-->", working(), "函数位置🙂\n".repeat(5000));
    let id = candidate(&mut port, &html);
    let reference = app
        .with_app(|s| s.private_context().persist_presentation_candidate(&turn.session_id, &turn.turn_id, &id))
        .unwrap();
    let read = call(
        &mut port,
        json!({"operation":"read","reference":reference,"file":"index.html"}),
    )
    .unwrap();
    assert_eq!(read.body["text"], html);
    assert_eq!(read.body["next_offset"], Value::Null);
    assert!(read.body.get("readable_content").is_none());
    let part = call(&mut port, json!({"operation":"read","candidate_id":id,"file":"index.html","offset":5000,"length":121})).unwrap();
    assert_eq!(
        part.body["text"],
        html.chars().skip(5000).take(121).collect::<String>()
    );
    assert_eq!(part.body["end_offset"], 5121);
    assert_eq!(part.body["next_offset"], Value::Null);
}

#[test]
fn presentation_edit_search_then_patch_preserves_original_and_requires_new_preview() {
    let (_root, mut state, turn) = setup();
    let app = BorrowedAppPort(std::cell::RefCell::new(&mut state));
    let scope = app.with_app(|state| crate::run_scope::RunScope::capture(state, &turn, "test", None, None));
    let mut port = RuntimeStatePort { scope: &scope,
        port: &app,
        turn_ref: &turn,
        previewed: Default::default(),
        animations: Default::default(),
        plots: Default::default(),
    };
    let html = format!(
        "{}\n<script>function target() {{ return '旧值'; }}</script>",
        working()
    );
    let id = candidate(&mut port, &html);
    let original = app
        .with_app(|s| s.private_context().read_presentation_candidate(&turn.session_id, &id))
        .unwrap();
    let found = call(
        &mut port,
        json!({"operation":"search","candidate_id":id,"query":"function target"}),
    )
    .unwrap();
    let offset = found.body["matches"][0]["offset"].as_u64().unwrap() as usize;
    assert!(html
        .chars()
        .skip(offset)
        .collect::<String>()
        .starts_with("function target"));
    let patched = call(&mut port, json!({"operation":"patch","candidate_id":id,"edits":[{"old_text":"return '旧值'","new_text":"return '新值'"}]})).unwrap();
    let next_id = patched.body["candidate_id"].as_str().unwrap();
    assert_ne!(id, next_id);
    let next = app
        .with_app(|s| s.private_context().read_presentation_candidate(&turn.session_id, next_id))
        .unwrap();
    let mut expected = original.content.clone();
    expected.content_files.insert(
        "index.html".into(),
        html.replace("return '旧值'", "return '新值'"),
    );
    assert_eq!(next.content, expected);
    assert_eq!(
        app.with_app(|s| s.private_context().read_presentation_candidate(&turn.session_id, &id))
            .unwrap(),
        original
    );
    port.previewed.insert(
        id.clone(),
        std::collections::HashSet::from(["legacy".into()]),
    );
    let error = call(
        &mut port,
        json!({"operation":"deliver","candidate_id":next_id}),
    )
    .err()
    .expect("new candidate needs its own preview");
    assert!(error.message.to_lowercase().contains("preview"));
}

#[test]
fn presentation_edit_invalid_patches_are_atomic_and_search_is_pageable() {
    let (root, mut state, turn) = setup();
    let app = BorrowedAppPort(std::cell::RefCell::new(&mut state));
    let scope = app.with_app(|state| crate::run_scope::RunScope::capture(state, &turn, "test", None, None));
    let mut port = RuntimeStatePort { scope: &scope,
        port: &app,
        turn_ref: &turn,
        previewed: Default::default(),
        animations: Default::default(),
        plots: Default::default(),
    };
    let html = "<p>same same 唯一</p>";
    let id = candidate(&mut port, html);
    let before = std::fs::read_dir(root.path().join("history.presentations/candidates"))
        .unwrap()
        .count();
    for edits in [
        json!([]),
        json!([{"old_text":"","new_text":"x"}]),
        json!([{"old_text":"same","new_text":"x"}]),
        json!([{"old_text":"唯一","new_text":"changed"},{"old_text":"missing","new_text":"x"}]),
    ] {
        assert!(call(
            &mut port,
            json!({"operation":"patch","candidate_id":id,"edits":edits})
        )
        .is_err());
    }
    assert_eq!(
        std::fs::read_dir(root.path().join("history.presentations/candidates"))
            .unwrap()
            .count(),
        before
    );
    assert_eq!(
        app.with_app(|s| s.private_context().read_presentation_candidate(&turn.session_id, &id))
            .unwrap()
            .content
            .content_files["index.html"],
        html
    );
    let first = call(
        &mut port,
        json!({"operation":"search","candidate_id":id,"query":"same","max_matches":1}),
    )
    .unwrap();
    let second=call(&mut port,json!({"operation":"search","candidate_id":id,"query":"same","offset":first.body["next_offset"]})).unwrap();
    assert_eq!(first.body["matches"][0]["offset"], 3);
    assert_eq!(second.body["matches"][0]["offset"], 8);
    assert_eq!(second.body["next_offset"], Value::Null);
}

#[test]
#[ignore = "requires installed Chromium/Edge; replays a recorded EX11 page"]
fn presentation_edit_original_learning_page_preview_delivery() {
    let (_root, mut state, turn) = setup();
    let path=std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/performance/ex11-local-demonstrations/ex11.7/batch8/runs/learning-2/content.json");
    let original: runtime::presentation::PresentationContent =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let old = state.private_context().create_presentation_candidate(&turn.session_id, &turn.turn_id, None, original.clone())
        .unwrap();
    let reference = state.private_context().persist_presentation_candidate(&turn.session_id, &turn.turn_id, &old.candidate_id)
        .unwrap();
    let app = BorrowedAppPort(std::cell::RefCell::new(&mut state));
    let scope = app.with_app(|state| crate::run_scope::RunScope::capture(state, &turn, "test", None, None));
    let mut port = RuntimeStatePort { scope: &scope,
        port: &app,
        turn_ref: &turn,
        previewed: Default::default(),
        animations: Default::default(),
        plots: Default::default(),
    };
    let search = call(
        &mut port,
        json!({"operation":"search","reference":reference,"query":"window.presentationScene="}),
    )
    .unwrap();
    assert_eq!(search.body["matches"].as_array().unwrap().len(), 1);
    let old_text = "window.presentationScene={seek:seek, snapshot:snapshot};";
    let new_text="window.presentationScene={seek:function(o){return seek(o.semantic_state,o.transition_progress);}, snapshot:snapshot};";
    let patched=call(&mut port,json!({"operation":"patch","reference":reference,"edits":[{"old_text":old_text,"new_text":new_text}]})).unwrap();
    let id = patched.body["candidate_id"].as_str().unwrap();
    let candidate = app
        .with_app(|s| s.private_context().read_presentation_candidate(&turn.session_id, id))
        .unwrap();
    let mut expected = original.clone();
    expected
        .content_files
        .get_mut("index.html")
        .unwrap()
        .replace_range(
            original.content_files["index.html"].find(old_text).unwrap()
                ..original.content_files["index.html"].find(old_text).unwrap() + old_text.len(),
            new_text,
        );
    assert_eq!(
        candidate.content, expected,
        "only the requested API wrapper may change; Konva and other resources stay identical"
    );
    let mut previews = vec![];
    for (name, viewport) in REQUIRED_PREVIEW_ENVIRONMENTS {
        let preview=call(&mut port,json!({"operation":"preview","candidate_id":id,"viewport":viewport,"read_selector":"#readout",
            "actions":[{"kind":"seek","semantic_state":2,"transition_progress":0.35},{"kind":"seek","semantic_state":1,"transition_progress":0.2},{"kind":"seek","semantic_state":1,"transition_progress":0.2}]})).unwrap();
        assert_eq!(
            preview.body["errors"],
            json!([]),
            "{name}: {}",
            preview.body
        );
        previews.push(preview.body);
    }
    let delivered = call(&mut port, json!({"operation":"deliver","candidate_id":id})).unwrap();
    let next = delivered.delivered.unwrap();
    assert_eq!(next.presentation_id, reference.presentation_id);
    assert_eq!(next.revision, reference.revision + 1);
    assert_eq!(
        app.with_app(|s| s.private_context().read_presentation(&turn.session_id, &reference))
            .unwrap()
            .content,
        original
    );
    if let Ok(directory) = std::env::var("EX11_EDIT_EVIDENCE_DIR") {
        std::fs::create_dir_all(&directory).unwrap();
        let directory = std::path::PathBuf::from(directory);
        std::fs::write(
            directory.join("patched-content.json"),
            serde_json::to_vec_pretty(&candidate.content).unwrap(),
        )
        .unwrap();
        std::fs::write(
            directory.join("preview-results.json"),
            serde_json::to_vec_pretty(&previews).unwrap(),
        )
        .unwrap();
        std::fs::write(directory.join("patch.json"),serde_json::to_vec_pretty(&json!({"old_text":old_text,"new_text":new_text,"old_reference":reference,"new_reference":next})).unwrap()).unwrap();
    }
}
