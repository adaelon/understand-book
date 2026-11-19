use super::*;

fn request(state: &mut AppState, method: &str, path: &str, body: Value) -> Reply {
    route(
        state,
        Req {
            method,
            url: path,
            body: &body.to_string(),
            now: "104",
        },
    )
}
fn json_body(reply: Reply) -> Value {
    let value: Value = serde_json::from_str(&reply.body).unwrap();
    assert_eq!(reply.status, 200, "{value}");
    value
}
fn result(state: &mut AppState, token: &str, name: &str, args: Value) -> Reply {
    request(
        state,
        "POST",
        "/personal/call",
        json!({"token":token,"name":name,"arguments":args}),
    )
}
fn setup(label: &str) -> (tempfile::TempDir, AppState) {
    let directory = tempfile::tempdir().unwrap();
    let mut state =
        crate::tests::state_named(&format!("personal-{label}-{}", uuid::Uuid::now_v7()));
    let source = "X".repeat(100);
    state.workspace.book = Arc::new(Book::new(base_schema::sample_base(), &source));
    state.workspace.reader = reader::Reader::new(&state.workspace.book, reader::DEFAULT_RADIUS);
    fs::write(state.workspace.book_dir.join("source.txt"), source).unwrap();
    let history = directory.path().join("memory/agent-history.json");
    fs::create_dir_all(history.parent().unwrap()).unwrap();
    let (saved, store) = crate::session_store::load_chat_storage(&Some(history.clone())).unwrap();
    state.user = crate::user_runtime::UserRuntime::local(
        memory::MemoryStore::open(history.with_file_name("memory.json")).unwrap(),
        Some(history),
        saved,
        None,
    );
    state.user.session_store = store;
    state.services.library_root = Some(directory.path().join("books"));
    (directory, state)
}
fn enable(state: &mut AppState, write: bool, navigate: bool) -> String {
    json_body(request(
        state,
        "POST",
        "/personal/connection",
        json!({"action":"enable","write":write,"navigate":navigate}),
    ))["token"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[test]
fn personal_writes_survive_restart_and_retries() {
    let (_dir, mut state) = setup("persist");
    let source = include_str!("../../../../examples/quickstart/book.md");
    let offset = source[..source.find("知识图谱").unwrap()]
        .encode_utf16()
        .count();
    let end = source.encode_utf16().count();
    let mut base = base_schema::sample_base();
    base.lid_nodes[0].span.end = end;
    base.lid_nodes[0].children.push("1.2".into());
    base.lid_nodes[1].span.end = offset;
    let mut second = base.lid_nodes[1].clone();
    second.lid = "1.2".into();
    second.path = vec![1, 2];
    second.span = base_schema::Span { start: offset, end };
    base.lid_nodes.push(second);
    state.workspace.book = Arc::new(Book::new(base, source));
    state.workspace.reader = reader::Reader::new(&state.workspace.book, reader::DEFAULT_RADIUS);
    fs::write(state.workspace.book_dir.join("source.txt"), source).unwrap();
    let id = state.workspace.book.base.book_id.clone();
    let token = enable(&mut state, true, true);
    let found = json_body(result(
        &mut state,
        &token,
        "book_search_text",
        json!({"book_id":id,"query":"知识图谱"}),
    ));
    let at = &found["occurrences"][0]["ranges"][0]["source_ref"];
    assert_eq!(at["lid"], "1.2");
    let read = json_body(result(
        &mut state,
        &token,
        "book_text",
        json!({"book_id":id,"lid":at["lid"],"range":at["range"]}),
    ));
    assert_eq!(read["text"], "知识图谱");
    assert_eq!(&read["source_ref"], at);
    let write = json!({"source_ref":at,"text":"从原文回到问题","operation_id":"note-a"});
    let first = json_body(result(&mut state, &token, "note_add", write.clone()));
    assert_eq!(first["value"]["authorship"], "assistant_draft");
    let retry = json_body(result(&mut state, &token, "note_add", write));
    assert_eq!(retry["note_id"], first["note_id"]);
    let saved = json_body(result(
        &mut state,
        &token,
        "task_save",
        json!({"source_ref":at,"text":"为什么？","operation_id":"t-a"}),
    ));
    assert!(saved["task"]["external_question"]["note_id"].is_string());
    let highlighted = json_body(result(
        &mut state,
        &token,
        "highlight_add",
        json!({"source_ref":at,"operation_id":"h-a"}),
    ));
    assert!(highlighted["highlight_id"].is_string());
    assert_eq!(highlighted["operation_id"], "h-a");
    assert_eq!(&highlighted["source_ref"], at);
    let repeated = json_body(result(
        &mut state,
        &token,
        "highlight_add",
        json!({"source_ref":at,"operation_id":"h-a"}),
    ));
    assert_eq!(repeated["highlight_id"], highlighted["highlight_id"]);
    let history = state.user.history_path.clone();
    let (sessions, chats) = crate::session_store::load_chat_storage(&history).unwrap();
    state.user = crate::user_runtime::UserRuntime::local(
        memory::MemoryStore::open(history.as_ref().unwrap().with_file_name("memory.json")).unwrap(),
        history,
        sessions,
        None,
    );
    state.user.session_store = chats;
    let highlights = json_body(result(
        &mut state,
        &token,
        "highlight_list",
        json!({"book_id":id}),
    ));
    assert_eq!(
        highlights["highlights"][0]["highlight_id"],
        highlighted["highlight_id"]
    );
    assert_eq!(&highlights["highlights"][0]["source_ref"], at);
    let task = json_body(result(&mut state, &token, "task_list", json!({})));
    assert_eq!(task["tasks"][0]["kind"], "external_question");
    assert_eq!(task["tasks"][0]["question"], "为什么？");
    assert_eq!(&task["tasks"][0]["source_ref"], at);
    let local = learning_task_view(&state);
    assert_eq!(
        local["tasks"][0]["note_ref"],
        saved["task"]["external_question"]
    );
    assert_eq!(local["tasks"][0]["source_lid"], "1.2");
}
fn learning_task_view(state: &AppState) -> Value {
    crate::learning_task::local_list(&state.user, &state.workspace).unwrap()
}

#[test]
fn personal_connection_owns_scope_and_render_ack() {
    let (_dir, mut state) = setup("scope");
    let id = state.workspace.book.base.book_id.clone();
    let token = enable(&mut state, false, true);
    let metadata = json_body(result(
        &mut state,
        &token,
        "paper_metadata",
        json!({"book_id":id}),
    ));
    assert_eq!(metadata["book_id"], id);
    assert_ne!(
        result(
            &mut state,
            &token,
            "paper_metadata",
            json!({"book_id":"outside"})
        )
        .status,
        200
    );
    let text = json_body(result(
        &mut state,
        &token,
        "book_text",
        json!({"book_id":id,"lid":"1.1","range":{"start":2,"end":4}}),
    ));
    let source = &text["source_ref"];
    assert_eq!(text["text"], "XX");
    let invalid_source = json!({"book_id":id,"source_revision":"old","lid":"1.1"});
    assert_ne!(
        result(
            &mut state,
            &token,
            "reader_goto",
            json!({"source_ref":invalid_source})
        )
        .status,
        200
    );
    assert_ne!(
        result(
            &mut state,
            &token,
            "note_add",
            json!({"source_ref":source,"text":"x","operation_id":"r"})
        )
        .status,
        200
    );
    assert_ne!(
        result(
            &mut state,
            &token,
            "book_text",
            json!({"book_id":"outside","lid":"1.1"})
        )
        .status,
        200
    );
    let ticket = json_body(result(
        &mut state,
        &token,
        "reader_compare",
        json!({"source_ref":source,"other_source_ref":source}),
    ));
    assert!(ticket["displayed"].is_null());
    let pending = json_body(request(
        &mut state,
        "GET",
        "/personal/navigation",
        json!({}),
    ));
    assert_eq!(pending["pending"]["id"], ticket["id"]);
    assert_eq!(pending["pending"]["sources"][0]["text"], "XX");
    json_body(request(
        &mut state,
        "POST",
        "/personal/navigation",
        json!({"id":ticket["id"],"displayed":true}),
    ));
    let receipt = json_body(result(
        &mut state,
        &token,
        "reader_navigation_result",
        json!({"id":ticket["id"]}),
    ));
    assert_eq!(receipt["displayed"], true);
    let redacted = json_body(request(
        &mut state,
        "GET",
        "/personal/connection",
        json!({}),
    ));
    assert!(redacted.get("token").is_none());
    json_body(request(
        &mut state,
        "POST",
        "/personal/connection",
        json!({"action":"revoke"}),
    ));
    assert_ne!(
        result(&mut state, &token, "material_list", json!({})).status,
        200
    );
}

#[test]
fn reads_another_shared_book_without_moving_the_current_window() {
    let (root, mut state) = setup("second");
    let current = state.workspace.book.base.book_id.clone();
    let mut base = base_schema::sample_base();
    base.book_id = "second".into();
    let location = root.path().join("books/second");
    fs::create_dir_all(&location).unwrap();
    fs::write(
        location.join("base.json"),
        serde_json::to_vec(&base).unwrap(),
    )
    .unwrap();
    fs::write(location.join("source.txt"), "Y".repeat(100)).unwrap();
    let granted = json_body(request(
        &mut state,
        "POST",
        "/personal/connection",
        json!({"action":"enable","book_ids":[current,"second"],"write":true,"navigate":true}),
    ));
    let token = granted["token"].as_str().unwrap();
    let second = json_body(result(
        &mut state,
        token,
        "book_text",
        json!({"book_id":"second","lid":"1.1","range":{"start":0,"end":3}}),
    ));
    assert_eq!(second["text"], "YYY");
    let note = json_body(result(
        &mut state,
        token,
        "note_add",
        json!({"source_ref":second["source_ref"],"text":"第二本的笔记","operation_id":"other"}),
    ));
    assert_ne!(
        result(
            &mut state,
            token,
            "note_read",
            json!({"book_id":current,"note_id":note["note_id"]})
        )
        .status,
        200
    );
    let notes = json_body(result(
        &mut state,
        token,
        "note_list",
        json!({"book_id":"second"}),
    ));
    assert_eq!(notes["notes"][0]["note_id"], note["note_id"]);
    let other = json_body(result(
        &mut state,
        token,
        "book_text",
        json!({"book_id":current,"lid":"1.1"}),
    ));
    let comparison = json_body(result(
        &mut state,
        token,
        "reader_compare",
        json!({"source_ref":other["source_ref"],"other_source_ref":second["source_ref"]}),
    ));
    assert_eq!(comparison["sources"][1]["text"], "YYY");
    assert!(comparison["displayed"].is_null());
    assert_eq!(state.workspace.book.base.book_id, current);
    state.workspace.book = Arc::new(Book::load(location.to_str().unwrap()).unwrap());
    state.workspace.book_dir = location.canonicalize().unwrap();
    let scene = json_body(result(
        &mut state,
        token,
        "reader_scene",
        json!({"book_id":"second"}),
    ));
    assert_eq!(scene["visible"], true);
    let refreshed = json_body(request(
        &mut state,
        "POST",
        "/personal/connection",
        json!({"action":"enable","book_ids":["second"]}),
    ));
    assert_eq!(refreshed["book_ids"], json!(["second"]));
}

#[test]
fn active_and_paused_practice_hide_external_sources_and_questions() {
    let (_root, mut state) = setup("closed");
    let id = state.workspace.book.base.book_id.clone();
    let token = enable(&mut state, true, true);
    let source = json_body(result(
        &mut state,
        &token,
        "book_text",
        json!({"book_id":id,"lid":"1.1"}),
    ))["source_ref"]
        .clone();
    json_body(result(
        &mut state,
        &token,
        "task_save",
        json!({"source_ref":source,"text":"练习前的提示","operation_id":"practice-task"}),
    ));
    json_body(crate::tests::post(&mut state, "/agent/new", "{}"));
    let chat = state.workspace.selected_chat.clone().unwrap();
    let mut revision = 0;
    for action in ["start", "pause", "exit"] {
        let res=json_body(crate::tests::post(&mut state,"/tutor/closed-book",&json!({
            "action":action,"target":"解释原文", "revision":revision,"chat_session_id":chat,"operation_id":action
        }).to_string()));
        revision = res["revision"].as_i64().unwrap();
        // The external Agent has no selected chat and must still respect this material's attempt.
        state.workspace.selected_chat = None;
        let text = result(
            &mut state,
            &token,
            "book_text",
            json!({"book_id":id,"lid":"1.1"}),
        );
        let tasks = json_body(result(&mut state, &token, "task_list", json!({}))).to_string();
        if action == "exit" {
            assert_eq!(text.status, 200, "{}", text.body);
            assert!(tasks.contains("练习前的提示"));
        } else {
            assert_ne!(text.status, 200);
            assert!(!tasks.contains("练习前的提示"));
            let tools = json_body(result(&mut state, &token, "tools_list", json!({}))).to_string();
            assert!(!tools.contains("note_read"));
            assert_ne!(
                result(
                    &mut state,
                    &token,
                    "task_save",
                    json!({"source_ref":source,"text":"伪造独立答案","operation_id":"bad"})
                )
                .status,
                200
            );
        }
        state.workspace.selected_chat = Some(chat.clone());
    }
}

#[test]
fn paper_metadata_is_limited_to_shared_books_and_this_connections_import_receipts() {
    let (_directory, mut state) = setup("papers");
    let configured = json_body(request(
        &mut state,
        "POST",
        "/personal/connection",
        json!({"action":"enable","write":true,"manage":true}),
    ));
    let token = configured["token"].as_str().unwrap();
    let root = dir(&state).unwrap().to_path_buf();
    let grant = read_grant(&root).unwrap().unwrap();
    // Complete the catalog-import stage without needing a network provider.
    for id in ["older-paper", "new-paper"] {
        let receipt = crate::learning_extensions::finish(
            &mut state,
            profile(&root).unwrap(),
            crate::learning_extensions::Prepared::Import {
                book_id: Some(id.into()),
                pdf: None,
                source: Some(json!({"metadata":{"title":id,"id":id},"vault":"papers"})),
            },
            "t0",
        )
        .unwrap();
        if id == "new-paper" {
            record_extension_result(
                &root,
                &grant,
                &crate::state_library_root(&state),
                "agentero_import",
                &receipt,
            )
            .unwrap();
        }
    }
    let response = json_body(result(
        &mut state,
        token,
        "paper_metadata",
        json!({"book_id":"new-paper"}),
    ));
    assert_eq!(response["status"], "metadata_only");
    assert_eq!(response["source"]["metadata"]["title"], "new-paper");
    assert_ne!(
        result(
            &mut state,
            token,
            "paper_metadata",
            json!({"book_id":"older-paper"})
        )
        .status,
        200
    );
    assert_ne!(
        result(
            &mut state,
            token,
            "book_text",
            json!({"book_id":"new-paper","lid":"1.1"})
        )
        .status,
        200
    );
    let stolen = result(
        &mut state,
        token,
        "paper_import",
        json!({"book_id":"older-paper","path":"unused.pdf"}),
    );
    assert_eq!(
        serde_json::from_str::<Value>(&stolen.body).unwrap()["error_code"],
        "PERSONAL_ACCESS_DENIED"
    );
    state.services.library_root = Some(_directory.path().join("another-library"));
    assert_ne!(
        result(
            &mut state,
            token,
            "paper_metadata",
            json!({"book_id":"new-paper"})
        )
        .status,
        200
    );
}
