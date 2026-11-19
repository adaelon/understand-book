use crate::*;
use std::sync::Arc;

const NOW: &str = "2026-10-26T04:00:00Z";
fn call(state: &AppState, action: &str, input: Value) -> Value {
    crate::board_api::route(&state.user, &state.workspace, action, &input, NOW).unwrap()
}
fn import(state: &AppState, board: &Value, source: &Value) -> Value {
    call(state, "import", json!({"board_id":board["board_id"],"board_revision":board["revision"],"source":source}))
}
fn fixture() -> (tempfile::TempDir, AppState) {
    let (root, mut state, _) = crate::tests::board_browser_host::fixture();
    let source = "命令将一个请求封装成对象。调用者通过命令执行工作，支持排队和撤销。";
    let mut base = base_schema::sample_base();
    for node in &mut base.lid_nodes { node.span = base_schema::Span { start: 0, end: source.encode_utf16().count() }; }
    for index in 0..65 {
        let mut node = base.graph_nodes[0].clone(); node.id = format!("concept:extra-{index}"); node.name = format!("额外概念 {index}"); base.graph_nodes.push(node);
    }
    let discourse = serde_json::from_value(json!([{"lid":"1.1","mode":"exposition","local_summary":"命令封装请求，调用者负责调度。","local_function":"definition","rhetorical_move":"chapter_setup","relations":[]}])).unwrap();
    let structure = serde_json::from_value(json!({
        "header":{"book_id":"sample-book","book_version":"v1","profile_id":"technical_learning","profile_version":"v0","core_schema_version":"v0","generated_at":"now"},
        "spine":[{"lid":"1","role":"foundation","summary":{"text":"用命令隔离请求与执行。","evidence_lids":["1.1"]},"key_stop_ids":["stop-1"],"depends_on":[]}],
        "throughlines":[{"id":"theme-1","name":"请求与执行","summary":{"text":"把请求保存下来再执行。","evidence_lids":["1.1"]},"lids":["1"],"key_stop_ids":["stop-1"]}],
        "key_stops":[{"id":"stop-1","lid":"1.1","type":"definition","title":"命令是什么","reason":{"text":"命令把请求封装成对象。","evidence_lids":["1.1"]}}]
    })).unwrap();
    state.workspace.book = Arc::new(Book::new(base, source).with_discourse_items(discourse).with_book_structure(Some(structure)));
    // This is the production shape that originally yielded an empty catalog.
    std::fs::remove_file(root.path().join("teaching_readiness.json")).unwrap();
    (root, state)
}

#[test]
fn wb5_prebuilt_artifacts_import_reopen_sources_and_follow_up_without_teaching_map() {
    let (_root, state) = fixture();
    let mut board = call(&state, "create", json!({"title":"已有预构建内容"}));
    for category in ["pass1", "discourse", "book_structure"] {
        let catalog = call(&state, "catalog", json!({"kind":"prebuild","category":category}));
        assert!(!catalog["items"].as_array().unwrap().is_empty(), "{category}");
        let candidates = catalog["items"].as_array().unwrap();
        for candidate in candidates.iter().take(if category == "book_structure" { 3 } else { 1 }) {
            board = import(&state, &board, &candidate["source"]);
            let item = board["items"].as_array().unwrap().last().unwrap();
            assert_eq!(item["target"]["artifact"], category);
            assert!(!item["readable_content"].as_str().unwrap().is_empty());
            assert!(item["target"]["snapshot"].is_object());
            if category == "discourse" {
                assert!(item["readable_content"].as_str().unwrap().contains("章节导入"));
                assert_eq!(item["target"]["snapshot"]["rhetorical_move"], "chapter_setup");
            }
            assert!(!item["source_bindings"].as_array().unwrap().is_empty());
            let resolved = call(&state, "source.resolve", json!({"board_id":board["board_id"],"item_id":item["item_id"],"source_ref_id":item["source_bindings"][0]["source_ref_id"]}));
            assert!(resolved.to_string().contains("命令"));
            let follow_up = serde_json::from_value(json!({"board_id":board["board_id"],"board_revision":board["revision"],"item_id":item["item_id"]})).unwrap();
            let frozen = crate::board_api::freeze_follow_up(&state.user, &state.workspace, &follow_up).unwrap();
            assert!(!frozen.source_bindings.is_empty());
            let related = call(&state, "related", json!({"source":candidate["source"]}));
            assert_eq!(related["items"][0]["board_id"], board["board_id"]);
        }
    }
    assert_eq!(board["items"].as_array().unwrap().len(), 5);
    assert_eq!(call(&state, "read", json!({"board_id":board["board_id"]}))["document"], board);
    let mut stale = call(&state, "catalog", json!({"kind":"prebuild","category":"pass1"}))["items"][0]["source"].clone();
    stale["source_revision"] = json!("older-source");
    assert!(crate::board_api::route(&state.user, &state.workspace, "import", &json!({"board_id":board["board_id"],"board_revision":board["revision"],"source":stale}), NOW).is_err());
}

#[test]
fn wb5_catalog_search_reaches_entries_beyond_first_page_and_keeps_categories_separate() {
    let (_root, state) = fixture();
    let first = call(&state, "catalog", json!({"kind":"prebuild","category":"pass1"}));
    assert_eq!(first["total"], 67); assert_eq!(first["items"].as_array().unwrap().len(), 60); assert_eq!(first["next"], 60);
    let second = call(&state, "catalog", json!({"kind":"prebuild","category":"pass1","after":first["next"]}));
    assert_eq!(second["items"].as_array().unwrap().len(), 7); assert!(second["next"].is_null());
    let found = call(&state, "catalog", json!({"kind":"prebuild","category":"pass1","query":"额外概念 64"}));
    assert_eq!(found["total"], 1); assert_eq!(found["items"][0]["source"]["entry_id"], "concept:extra-64");
    assert_eq!(call(&state, "catalog", json!({"kind":"prebuild","category":"discourse","query":"额外概念 64"}))["total"], 0);
}

#[test]
#[ignore = "real published AI Infra artifact acceptance; set UB_BOARD_PREBUILD_BOOK"]
fn wb5_real_ai_infra_prebuilt_artifacts() {
    let book_dir = std::env::var("UB_BOARD_PREBUILD_BOOK").unwrap();
    let (_root, mut state) = fixture();
    state.workspace.book = Arc::new(Book::load(&book_dir).unwrap());
    state.workspace.book_dir = PathBuf::from(&book_dir);
    let mut board = call(&state, "create", json!({"title":"AI Infra 预构建验收"}));
    let mut checks = Vec::new();
    for category in ["pass1", "discourse", "book_structure"] {
        let started = std::time::Instant::now();
        let catalog = call(&state, "catalog", json!({"kind":"prebuild","category":category}));
        let count = catalog["total"].as_u64().unwrap(); assert!(count > 0);
        let catalog_ms = started.elapsed().as_millis();
        let source = &catalog["items"][0]["source"];
        board = import(&state, &board, source);
        let item = board["items"].as_array().unwrap().last().unwrap();
        for binding in item["source_bindings"].as_array().unwrap() {
            call(&state, "source.resolve", json!({"board_id":board["board_id"],"item_id":item["item_id"],"source_ref_id":binding["source_ref_id"]}));
        }
        assert_eq!(call(&state, "read", json!({"board_id":board["board_id"]}))["document"], board);
        checks.push(json!({"category":category,"total":count,"catalog_ms":catalog_ms,"sources":item["source_bindings"].as_array().unwrap().len(),"title":item["title"]}));
    }
    let report = json!({"status":"passed","checks":checks,"items":board["items"].as_array().unwrap().len()});
    println!("{report}");
    if let Ok(path) = std::env::var("UB_BOARD_PREBUILD_REPORT") { std::fs::write(path, serde_json::to_string_pretty(&report).unwrap()).unwrap(); }
}

#[test]
#[ignore = "local browser acceptance with published artifacts; POST /test/shutdown to stop"]
fn wb5_prebuilt_browser_host() {
    let book_dir = std::env::var("UB_BOARD_PREBUILD_BOOK").unwrap();
    let (_root, mut state) = fixture();
    state.workspace.book = Arc::new(Book::load(&book_dir).unwrap());
    state.workspace.book_dir = PathBuf::from(&book_dir);
    let metadata = call(&state, "create", json!({"title":"AI Infra · 预构建内容"}));
    let output = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp/board-prebuild-20261026/browser-fixture.json");
    std::fs::write(output, serde_json::to_vec(&metadata).unwrap()).unwrap();
    let server = tiny_http::Server::http("127.0.0.1:4248").unwrap();
    println!("WB5_BROWSER_READY");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3600);
    while std::time::Instant::now() < deadline {
        let Some(mut request) = server.recv_timeout(std::time::Duration::from_secs(1)).unwrap() else { continue };
        let path = request.url().to_owned();
        if path == "/test/shutdown" && request.method().as_str() == "POST" {
            request.respond(tiny_http::Response::from_string("stopped")).unwrap(); break;
        }
        let mut body = String::new(); request.as_reader().read_to_string(&mut body).unwrap();
        let reply = if path == "/test/fixture" { ok_json(&metadata) }
        else { route(&mut state, Req { method:request.method().as_str(),url:&path,body:&body,now:NOW }) };
        request.respond(tiny_http::Response::from_string(reply.body).with_status_code(reply.status)
            .with_header(tiny_http::Header::from_bytes("Content-Type","application/json").unwrap())).unwrap();
    }
}
