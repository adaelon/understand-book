//! MCP9: real HTTP, SQLite, published sources and an opt-in external client fixture.
use super::*;
use std::{path::Path, time::Instant};

fn publish(f: &Fixture, user: &str, id: &str, text: &str) -> PublishedBookRef {
    let input = tempfile::tempdir().unwrap();
    let mut base = base_schema::sample_base();
    base.book_id = id.into();
    base.graph_nodes.clear();
    base.graph_edges.clear();
    for node in &mut base.lid_nodes {
        node.span.end = text.encode_utf16().count();
    }
    fs::write(
        input.path().join("base.json"),
        serde_json::to_vec(&base).unwrap(),
    )
    .unwrap();
    fs::write(input.path().join("source.txt"), text).unwrap();
    let mut library = f.access.library.lock().unwrap();
    let reference = library.publish(input.path()).unwrap().reference;
    library.grant(user, &reference).unwrap();
    reference
}

fn admin(f: &Fixture, cookie: &str, method: &str, path: &str, payload: Value) -> Value {
    let request = ureq::request(
        method,
        &format!("{}{}", f.server.as_ref().unwrap().url, path),
    )
    .set("Host", "reader.example")
    .set("Origin", "https://reader.example")
    .set("Cookie", &format!("{}={cookie}", crate::auth::COOKIE))
    .set("X-CSRF-Token", &crate::auth::csrf(&cookie));
    let response = if method == "GET" {
        request.call()
    } else {
        request.send_json(payload)
    }
    .unwrap();
    response.into_json().unwrap()
}

#[test]
fn mcp9_two_accounts_purchase_recovery_and_source_evidence() {
    let mut f = Fixture::new();
    f.control.set_reader_admin("alice", true).unwrap();
    let cookie = f
        .access
        .auth
        .login("alice", "fixture-password", None, multi_user_host::now())
        .unwrap();
    let bob = publish(
        &f,
        "bob",
        "bob-notes",
        "Bob has a separate reading allowance.",
    );
    for (client, expected) in [(0, &f.reference), (2, &bob)] {
        assert_eq!(f.rpc(client, init())["result"]["protocolVersion"], PROTOCOL);
        let directory = f.tool(client, "book_list", json!({}));
        assert_eq!(
            directory["structuredContent"]["items"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            directory["structuredContent"]["items"][0]["published_book_ref"],
            json!(expected)
        );
    }
    f.remaining(2);
    let empty = f.tool(
        0,
        "book_search_text",
        json!({"published_book_ref":f.reference,"query":"absent-term"}),
    );
    assert_eq!(empty["structuredContent"]["data"]["occurrences"], json!([]));
    let found = f.tool(
        1,
        "book_search_text",
        json!({"published_book_ref":f.reference,"query":"needle"}),
    );
    let hit = &found["structuredContent"]["data"]["occurrences"][0];
    let args =
        json!({"published_book_ref":f.reference,"lid":hit["start_lid"],"end_lid":hit["end_lid"]});
    assert_eq!(
        f.tool(0, "book_text", args.clone())["structuredContent"]["error_code"],
        "quota_exhausted"
    );
    let purchase = json!({"operation_id":"mcp9-purchase","pack_count":1,"paid_at":multi_user_host::now(),"channel":"test","note":"isolated fixture; no real payment"});
    let receipt = admin(
        &f,
        &cookie,
        "POST",
        "/api/admin/users/alice/mcp/packs",
        purchase.clone(),
    );
    let first = f.tool(0, "book_text", args);
    let next = &first["structuredContent"]["next_calls"][0];
    let second = f.tool(1, next["name"].as_str().unwrap(), next["arguments"].clone());
    assert_eq!(
        first["structuredContent"]["usage"]["charged_from"]["pack_id"],
        receipt["call_pack"]["pack_id"]
    );
    assert_eq!(second["structuredContent"]["usage"]["paid_remaining"], 998);
    // Lost-response recovery after subsequent use returns the original receipt, without reissuing units.
    assert_eq!(
        admin(
            &f,
            &cookie,
            "GET",
            "/api/admin/operations/mcp9-purchase",
            json!({})
        ),
        receipt
    );
    assert_eq!(
        admin(
            &f,
            &cookie,
            "POST",
            "/api/admin/users/alice/mcp/packs",
            purchase
        ),
        receipt
    );
    assert_eq!(
        admin(&f, &cookie, "GET", "/api/admin/users/alice/mcp", json!({}))["paid_remaining"],
        998
    );
    let bob_read = f.tool(
        2,
        "book_text",
        json!({"published_book_ref":bob,"lid":"1.1"}),
    );
    assert_eq!(
        bob_read["structuredContent"]["data"]["text"],
        "Bob has a separate reading allowance."
    );
    assert_eq!(
        bob_read["structuredContent"]["usage"]["free_remaining"],
        999
    );
    assert_eq!(bob_read["structuredContent"]["usage"]["paid_remaining"], 0);
    assert_eq!(
        f.tool(2, "book_text", f.text_args())["structuredContent"]["error_code"],
        "material_unavailable"
    );
    f.access
        .library
        .lock()
        .unwrap()
        .revoke("alice", &f.reference)
        .unwrap();
    let denied = f.tool(0, next["name"].as_str().unwrap(), next["arguments"].clone());
    assert_eq!(
        denied["structuredContent"]["error_code"],
        "material_unavailable"
    );
    assert!(denied["structuredContent"].get("data").is_none());
    assert_eq!(
        f.tool(0, "book_list", json!({}))["structuredContent"]["items"],
        json!([])
    );
    assert_eq!(f.count(), 5);
    let rows: Vec<(String, String, i64)> = f.control.connection.prepare(
        "SELECT user_id,debit_source,count(*) FROM mcp_usage GROUP BY user_id,debit_source ORDER BY user_id,debit_source"
    ).unwrap().query_map([], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap().map(Result::unwrap).collect();
    assert_eq!(
        rows,
        vec![
            ("alice".into(), "call_pack".into(), 2),
            ("alice".into(), "free_month".into(), 2),
            ("bob".into(), "free_month".into(), 1)
        ]
    );
    f.no_private_state();
    if let Ok(path) = std::env::var("MCP9_SOURCE_EVIDENCE") {
        let base: Value =
            serde_json::from_slice(&fs::read(f._input.path().join("base.json")).unwrap()).unwrap();
        fs::write(
            path,
            serde_json::to_vec_pretty(&json!({
                "published_book_ref":f.reference,"tree":base["lid_nodes"],
                "source_text":fs::read_to_string(f._input.path().join("source.txt")).unwrap(),
                "pages":[first["structuredContent"],second["structuredContent"]]
            }))
            .unwrap(),
        )
        .unwrap();
    }
}

#[test]
fn mcp9_small_concurrent_reads_measure_bytes_and_ledger() {
    let f = Fixture::new();
    f.access
        .library
        .lock()
        .unwrap()
        .grant("bob", &f.reference)
        .unwrap();
    let mut total = 0;
    for clients in [1, 4, 8] {
        let barrier = Arc::new(std::sync::Barrier::new(clients));
        let started = Instant::now();
        let handles: Vec<_> = (0..clients).map(|i| {
            let (url, token, args, barrier) = (f.url(), f.tokens[if i % 2 == 0 {0} else {2}].clone(), f.text_args(), barrier.clone());
            std::thread::spawn(move || {
                barrier.wait();
                let start = Instant::now();
                let response = post(&url, &token, json!({"jsonrpc":"2.0","id":i,"method":"tools/call","params":{"name":"book_text","arguments":args}}));
                assert_eq!(response.status(), 200);
                let raw = response.into_string().unwrap();
                let value: Value = serde_json::from_str(&raw).unwrap();
                assert_eq!(value["result"]["isError"], false);
                assert_eq!(value["result"]["structuredContent"]["usage"]["charged_units"], 1);
                assert_eq!(value["result"]["structuredContent"]["data"]["text"].as_str().unwrap().chars().count(), 8000);
                (start.elapsed().as_micros(), raw.len())
            })
        }).collect();
        let mut results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        results.sort();
        total += clients as i64;
        assert_eq!(f.count(), total);
        println!(
            "MCP9_METRIC {}",
            json!({"clients":clients,"wall_us":started.elapsed().as_micros(),"median_us":results[clients/2].0,"max_us":results[clients-1].0,"response_bytes":results.iter().map(|r|r.1).sum::<usize>(),"calls":total})
        );
    }
    f.no_private_state();
}

/// Run explicitly with MCP9_AGENT_DIR; the external runner creates STOP after the client exits.
#[test]
#[ignore = "external Agent acceptance; requires MCP9_AGENT_DIR and a separately launched client"]
fn mcp9_external_agent_fixture() {
    let dir = std::path::PathBuf::from(std::env::var("MCP9_AGENT_DIR").expect("MCP9_AGENT_DIR"));
    fs::create_dir_all(&dir).unwrap();
    let mut f = Fixture::new();
    f.control.set_reader_admin("alice", true).unwrap();
    let cookie = f
        .access
        .auth
        .login("alice", "fixture-password", None, multi_user_host::now())
        .unwrap();
    f.access
        .library
        .lock()
        .unwrap()
        .revoke("alice", &f.reference)
        .unwrap();
    let text = format!("雨水实验记录。北池起始有 37 升水。😀\n{}\n实验结束时，北池有 19 升水。南池没有参与本次实验。", "本段记录观察过程，天气晴朗，池壁完整，没有其他进水。\n".repeat(330));
    let reference = publish(&f, "alice", "rainwater-notes", &text);
    f.remaining(0);
    admin(
        &f,
        &cookie,
        "POST",
        "/api/admin/users/alice/mcp/gifts",
        json!({"operation_id":"agent-gift","units":12,"reason":"MCP9 isolated Agent acceptance"}),
    );
    f.access
        .library
        .lock()
        .unwrap()
        .grant("bob", &reference)
        .unwrap();
    f.access.mcp_usage.status("bob").unwrap();
    f.control
        .connection
        .execute(
            "UPDATE mcp_free_months SET used_units=granted_units WHERE user_id='bob'",
            [],
        )
        .unwrap();
    for (name, index) in [("reader", 0), ("exhausted", 2)] {
        fs::write(dir.join(format!("{name}-mcp.json")), serde_json::to_vec(&json!({"mcpServers":{"beta":{
            "type":"http","url":f.url(),"headers":{"Host":"reader.example","Authorization":format!("Bearer {}",f.tokens[index])}
        }}})).unwrap()).unwrap();
    }
    fs::write(dir.join("READY"), "ready").unwrap();
    let started = Instant::now();
    while !dir.join("STOP").exists() && started.elapsed().as_secs() < 900 {
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    assert!(
        dir.join("STOP").exists(),
        "external client did not finish in 15 minutes"
    );
    let usage = f.access.mcp_usage.status("alice").unwrap();
    let calls: Vec<Value> = f.control.connection.prepare("SELECT user_id,tool,units,debit_source FROM mcp_usage ORDER BY completed_at,call_id")
        .unwrap().query_map([], |r| Ok(json!({"user_id":r.get::<_,String>(0)?,"tool":r.get::<_,String>(1)?,"units":r.get::<_,i64>(2)?,"debit_source":r.get::<_,String>(3)?}))).unwrap().map(Result::unwrap).collect();
    fs::write(dir.join("evidence.json"), serde_json::to_vec_pretty(&json!({"protocol":PROTOCOL,"reference":reference,"source_text":text,"gift_units":12,"usage":usage,"calls":calls})).unwrap()).unwrap();
    f.no_private_state();
    // Credentials are confined to the fixture directory and expire with its temporary database.
    for name in ["reader-mcp.json", "exhausted-mcp.json"] {
        fs::remove_file(Path::new(&dir).join(name)).unwrap();
    }
}
