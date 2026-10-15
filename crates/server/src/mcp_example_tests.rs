use super::*;
include!("../../runtime/src/testdata/book_example_adapter.rs");

#[test]
fn mcp_contract_to12_local_wire_results_cover_every_book_tool() {
    let book = Book::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../book-tool-contracts/fixtures/example-book")).unwrap();
    let reader = Reader::new(&book, DEFAULT_RADIUS);
    let mut app = AppState {
        services: crate::service_state::ServiceState { desktop_host:false, reader_only:false, library_root:None, adapter:Box::new(ExampleAdapter::default()) },
        user: crate::user_runtime::UserRuntime::local(MemoryStore::open(tmp("to12-local")).unwrap(),None,crate::AgentHistory::default(),None),
        mcp_artifact_read_port:None, visitor_sessions:VisitorSessions::default(),
        workspace:crate::reader_workspace::ReaderWorkspace::local(std::env::temp_dir(),book.into(),reader,new_session(),None),
    };
    let tools = handle_jsonrpc_message(&mut app, &json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}).to_string(),"1000").unwrap()["result"]["tools"].clone();
    let mut cases = Vec::new();
    for contract in contracts() {
        let name = contract.aliases.mcp.unwrap();
        let inputs = book_tool_contracts::input_examples(contract.id);
        if !inputs.is_empty() { cases.extend(inputs.into_iter().map(|input|(name.to_string(),input))); }
        else {
            cases.push((name.into(), match contract.id {
                BookToolId::Guide => json!({"intent":"Explain the threshold conditions","anchor_lid":"1.1"}),
                _ => json!({}),
            }));
        }
    }
    cases.extend([
        ("book_search_text".into(),json!({"query":"missing-text"})),
        ("book_concept".into(),json!({"query":"missing-concept"})),
        ("book_context".into(),json!({"lid":"1","k":0})),
        ("book_text".into(),json!({"lid":"1.1","start_utf16":app.workspace.book.base.lid_nodes.iter().find(|n|n.lid=="1.1").unwrap().span.start})),
        ("book_query".into(),json!({"query":"invalid plan"})),
    ]);
    let mut samples = Vec::new();
    for (name,input) in cases {
        let reply = handle_jsonrpc_message(&mut app,&json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":name,"arguments":input}}).to_string(),"1001").unwrap()["result"].clone();
        assert_eq!(reply["isError"],input["query"] == "missing-concept","{name}: {reply}");
        assert_eq!(serde_json::from_str::<Value>(reply["content"][0]["text"].as_str().unwrap()).unwrap(),reply["structuredContent"]);
        if name == "book_guide" {
            let close = json!({"action":"close","session_id":reply["structuredContent"]["session_id"]});
            let closed = handle_jsonrpc_message(&mut app,&json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":name,"arguments":close}}).to_string(),"1002").unwrap()["result"].clone();
            assert_eq!(closed["isError"],false);
            samples.push(json!({"tool":name,"input":close,"reply":closed}));
        }
        samples.push(json!({"tool":name,"input":input,"reply":reply}));
    }
    let error = handle_jsonrpc_message(&mut app,&json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"book_text","arguments":{"lid":"missing"}}}).to_string(),"1003").unwrap()["result"].clone();
    assert_eq!(error["isError"],true);
    samples.push(json!({"tool":"book_text","reply":error}));
    for tool in tools.as_array().unwrap().iter().filter(|t|t["name"].as_str().unwrap().starts_with("book_")) {
        assert_eq!(tool["outputSchema"]["type"],"object");
        assert_eq!(tool["annotations"]["readOnlyHint"],tool["name"]!="book_guide");
    }
    if let Ok(dir) = std::env::var("TO12_DIR") {
        std::fs::write(std::path::Path::new(&dir).join("local.json"),serde_json::to_vec_pretty(&json!({"tools":tools,"samples":samples})).unwrap()).unwrap();
    }
}

#[test]
fn mcp_contract_to10_local_and_rest_consume_common_text_offset() {
    let book = Book::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../book-tool-contracts/fixtures/example-book"
    ))
    .unwrap();
    let expected = book.text("1.1", Some("1.2")).unwrap();
    let start = book
        .base
        .lid_nodes
        .iter()
        .find(|n| n.lid == "1.1")
        .unwrap()
        .span
        .start
        + 4;
    let reader = Reader::new(&book, DEFAULT_RADIUS);
    let mut app = AppState {
        services: crate::service_state::ServiceState {
            desktop_host: false,
            reader_only: false,
            library_root: None,
            adapter: Box::new(ExampleAdapter::default()),
        },
        user: crate::user_runtime::UserRuntime::local(
            MemoryStore::open(tmp("to10-local")).unwrap(),
            None,
            crate::AgentHistory::default(),
            None,
        ),
        mcp_artifact_read_port: None,
        visitor_sessions: VisitorSessions::default(),
        workspace: crate::reader_workspace::ReaderWorkspace::local(
            std::env::temp_dir(),
            book.into(),
            reader,
            new_session(),
            None,
        ),
    };
    let args = json!({"lid":"1.1","end_lid":"1.2","start_utf16":start});
    let reply = dispatch_mcp_tool(&mut app, "book_text", args.clone(), "1000");
    assert_eq!(reply.status, 200, "{}", reply.body);
    let body: Value = serde_json::from_str(&reply.body).unwrap();
    assert_eq!(
        body["text"],
        String::from_utf16(&expected.encode_utf16().skip(4).collect::<Vec<_>>()).unwrap()
    );
    assert_eq!(body["source_range"]["start"], start);
    let query = std::collections::HashMap::from([
        ("lid".into(), "1.1".into()),
        ("end".into(), "1.2".into()),
        ("start_utf16".into(), start.to_string()),
    ]);
    let input = crate::bind_rest_book_input(BookToolId::Text, &query)
        .unwrap_or_else(|_| panic!("REST input"));
    let rest =
        crate::route_canonical_readonly_book_tool(&app.workspace.book, BookToolId::Text, input);
    assert_eq!(serde_json::from_str::<Value>(&rest.body).unwrap(), body);
    if let Ok(path) = std::env::var("TO10_LOCAL") {
        std::fs::write(
            path,
            serde_json::to_vec_pretty(
                &json!({"tools":tools_list_result()["tools"],"input":args,"result":body}),
            )
            .unwrap(),
        )
        .unwrap();
    }
}

#[test]
fn mcp_contract_to9_local_context_counts_and_missing_structure() {
    let book = Book::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../book-tool-contracts/fixtures/example-book"))
        .unwrap().with_book_structure(None);
    let reader = Reader::new(&book, DEFAULT_RADIUS);
    let mut app = AppState {
        services: crate::service_state::ServiceState { desktop_host: false, reader_only: false, library_root: None, adapter: Box::new(ExampleAdapter::default()) },
        user: crate::user_runtime::UserRuntime::local(MemoryStore::open(tmp("to9-local")).unwrap(), None, crate::AgentHistory::default(), None),
        mcp_artifact_read_port: None,
        visitor_sessions: VisitorSessions::default(),
        workspace: crate::reader_workspace::ReaderWorkspace::local(std::env::temp_dir(), book.into(), reader, new_session(), None),
    };
    let mut samples = Vec::new();
    for k in [0, 1, 2, 10] {
        let input = json!({"lid":"1.1","granularity":"far","k":k});
        let reply = dispatch_mcp_tool(&mut app, "book_context", input.clone(), "1000");
        assert_eq!(reply.status, 200);
        let result: Value = serde_json::from_str(&reply.body).unwrap();
        assert_eq!(result["total_candidates"], 2);
        assert_eq!(result["returned_count"], k.min(2));
        assert_eq!(result["truncated"], k < 2);
        for item in result["items"].as_array().unwrap() {
            let next = &item["next_calls"][0];
            assert_eq!(next["name"], "book_text");
            let read = dispatch_mcp_tool(&mut app, "book_text", next["arguments"].clone(), "1000");
            assert_eq!(read.status, 200);
        }
        samples.push(json!({"input":input,"result":result}));
    }
    let reply = dispatch_mcp_tool(&mut app, "book_structure", json!({}), "1000");
    assert_eq!(reply.status, 200);
    let structure: Value = serde_json::from_str(&reply.body).unwrap();
    assert_eq!(structure["available"], false);
    assert_eq!(structure["warning"], "book_structure.json not attached");
    if let Ok(path) = std::env::var("TO9_LOCAL") {
        std::fs::write(path, serde_json::to_vec_pretty(&json!({"samples":samples,"structure":structure})).unwrap()).unwrap();
    }
}

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
    let mut handoffs = Vec::new();
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
            let collection = match name {
                "book_search_text" => Some("occurrences"),
                "book_concept" => Some("candidates"),
                "book_context" => Some("items"),
                _ => None,
            };
            if let Some(collection) = collection {
                for candidate in result[collection].as_array().unwrap() {
                    for next in candidate["next_calls"].as_array().unwrap() {
                        assert_eq!(next["name"], "book_text");
                        assert!(next.get("tool").is_none());
                        assert!(next["arguments"].get("published_book_ref").is_none());
                        let reply = dispatch_mcp_tool(&mut app, next["name"].as_str().unwrap(), next["arguments"].clone(), "1000");
                        assert_eq!(reply.status, 200);
                        let read: Value = serde_json::from_str(&reply.body).unwrap();
                        let args = &next["arguments"];
                        assert_eq!(read["text"], app.workspace.book.text(args["lid"].as_str().unwrap(), args["end_lid"].as_str()).unwrap());
                        handoffs.push(json!({"tool":name,"candidate":candidate,"call":next,"result":read}));
                    }
                }
            }
            count += 1;
        }
    }
    assert_eq!(count, 8);
    let reply = dispatch_mcp_tool(&mut app, "book_search_text", json!({"query":"10.\nThe beta"}), "1000");
    assert_eq!(reply.status, 200);
    let located: Value = serde_json::from_str(&reply.body).unwrap();
    let candidate = &located["occurrences"][0];
    let next = &candidate["next_calls"][0];
    assert_eq!(next["arguments"], json!({"lid":"1.1","end_lid":"1.2"}));
    let reply = dispatch_mcp_tool(&mut app, next["name"].as_str().unwrap(), next["arguments"].clone(), "1000");
    assert_eq!(reply.status, 200);
    let result: Value = serde_json::from_str(&reply.body).unwrap();
    assert_eq!(result["text"], app.workspace.book.text("1.1", Some("1.2")).unwrap());
    handoffs.push(json!({"tool":"book_search_text","candidate":candidate,"call":next,"result":result}));
    // MCP must resolve the advertised alias, not a Resident tool name.
    assert_eq!(dispatch_mcp_tool(&mut app, "book.text", json!({"lid":"1.1"}), "1000").status, 404);
    assert!(!handoffs.is_empty());
    if let Ok(path) = std::env::var("TO8_LOCAL") {
        std::fs::write(path, serde_json::to_vec_pretty(&json!({"tools":tools,"handoffs":handoffs})).unwrap()).unwrap();
    }
}
