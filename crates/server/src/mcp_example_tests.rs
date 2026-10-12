use super::*;
include!("../../runtime/src/testdata/book_example_adapter.rs");

#[test]
fn mcp_contract_local_advertised_examples_use_real_dispatch() {
    let book = Book::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../book-tool-contracts/fixtures/example-book")).unwrap();
    let reader = Reader::new(&book, DEFAULT_RADIUS);
    let mut app = AppState {
        services: crate::service_state::ServiceState { desktop_host: false, reader_only: false, library_root: None, adapter: Box::new(ExampleAdapter::default()) },
        user: crate::user_runtime::UserRuntime::local(MemoryStore::open(tmp("to2-local-examples")).unwrap(), None, crate::AgentHistory::default(), None),
        mcp_artifact_read_port: None,
        visitor_sessions: VisitorSessions::default(),
        workspace: crate::reader_workspace::ReaderWorkspace::local(std::env::temp_dir(), book.into(), reader, new_session(), None),
    };
    let tools = tools_list_result();
    let mut count = 0;
    for tool in tools["tools"].as_array().unwrap() {
        let Some(examples) = tool["inputSchema"]["examples"].as_array() else { continue };
        let name = tool["name"].as_str().unwrap();
        assert!(!name.contains('.'));
        for input in examples {
            let reply = dispatch_mcp_tool(&mut app, name, input.clone(), "1000");
            assert_eq!(reply.status, 200, "{name}: {}", reply.body);
            let result: Value = serde_json::from_str(&reply.body).unwrap();
            assert!(result.get("error_code").is_none(), "{name}: {result}");
            match name {
                "book_query" | "book_synthesize" => {
                    let lids: Vec<_> = result["citations"].as_array().unwrap().iter().map(|c| c["lid"].as_str().unwrap()).collect();
                    assert_eq!(lids, vec!["1.1", "1.2"], "{result}");
                }
                "book_text" => assert_eq!(result["text"], app.workspace.book.text("1.1", Some("1.2")).unwrap()),
                "book_concept" => assert_eq!(result["returned_count"], 2),
                "book_search_text" => assert_eq!(result["occurrences"][0]["start_lid"], if input["order"] == "document" { "1.1" } else { "1.2" }),
                "book_context" => assert!(!result["items"].as_array().unwrap().is_empty()),
                "book_structure" => assert_eq!(result["available"], true, "{result}"),
                _ => unreachable!("{name}"),
            }
            count += 1;
        }
    }
    assert_eq!(count, 8);
    // MCP must resolve the advertised alias, not a Resident tool name.
    assert_eq!(dispatch_mcp_tool(&mut app, "book.text", json!({"lid":"1.1"}), "1000").status, 404);
}
