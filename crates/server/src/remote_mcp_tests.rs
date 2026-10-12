use super::*;
use crate::{
    control_store::ControlStore,
    multi_user_host::{self, RunningMultiUserServer, Site},
    published_library::PublishedBookRef,
    user_registry::UserRegistry,
};
use std::{fs, sync::Arc};

#[path = "mcp_contract_tests.rs"]
mod mcp_contract;

#[path = "mcp_acceptance_tests.rs"]
mod mcp_acceptance;

struct Fixture {
    server: Option<RunningMultiUserServer>,
    access: Arc<Authorization>,
    control: ControlStore,
    tokens: Vec<String>,
    reference: PublishedBookRef,
    root: tempfile::TempDir,
    _input: tempfile::TempDir,
}
impl Fixture {
    fn new() -> Self {
        Self::with_example_book(false)
    }

    fn with_example_book(examples: bool) -> Self {
        let root = tempfile::tempdir().unwrap();
        let input = tempfile::tempdir().unwrap();
        let users = UserRegistry::open(root.path()).unwrap();
        let mut control = ControlStore::open(users.writer()).unwrap();
        control
            .provision_password("alice", "fixture-password", true)
            .unwrap();
        control
            .provision_password("bob", "fixture-password", true)
            .unwrap();
        let access = Arc::new(Authorization::new(users).unwrap());
        let now = multi_user_host::now();
        let mut tokens = vec![];
        for user in ["alice", "alice", "bob"] {
            let cookie = access
                .auth
                .login(user, "fixture-password", None, now)
                .unwrap();
            let principal = access.auth.authenticate(&cookie, now).unwrap();
            tokens.push(
                access
                    .mcp_access
                    .create(&principal, "HTTP fixture", now)
                    .unwrap()
                    .token,
            );
        }
        let source = format!("needle 中文😀 {}", "甲".repeat(8_020));
        let mut base = base_schema::sample_base();
        for node in &mut base.lid_nodes {
            node.span.end = source.encode_utf16().count();
        }
        fs::write(
            input.path().join("base.json"),
            serde_json::to_vec(&base).unwrap(),
        )
        .unwrap();
        fs::write(input.path().join("source.txt"), source).unwrap();
        if examples {
            fs::write(input.path().join("base.json"), include_str!("../../book-tool-contracts/fixtures/example-book/base.json")).unwrap();
            fs::write(input.path().join("source.txt"), include_str!("../../book-tool-contracts/fixtures/example-book/source.txt")).unwrap();
            fs::write(input.path().join("book_structure.json"), include_str!("../../book-tool-contracts/fixtures/example-book/book_structure.json")).unwrap();
        }
        let reference = if examples {
            // Restore the recorded publication and its registry row so tools/list
            // examples can be called byte-for-byte, including the published ref.
            let manifest: crate::published_library::PublicationManifest = serde_json::from_str(include_str!("../../book-tool-contracts/fixtures/example-book/publication.json")).unwrap();
            let reference = manifest.reference.clone();
            let manifest_json = serde_json::to_string(&manifest).unwrap();
            fs::write(input.path().join("publication.json"), &manifest_json).unwrap();
            control.connection.execute("INSERT INTO book_publications(book_id,publication_id,directory,manifest) VALUES(?,?,?,?)",
                rusqlite::params![reference.book_id, reference.publication_id, input.path().to_str().unwrap(), manifest_json]).unwrap();
            access.library.lock().unwrap().set_default(&reference).unwrap();
            reference
        } else { access
            .library
            .lock()
            .unwrap()
            .publish(input.path())
            .unwrap()
            .reference };
        access
            .library
            .lock()
            .unwrap()
            .grant("alice", &reference)
            .unwrap();
        let server = multi_user_host::start_with_access(
            "127.0.0.1:0".parse().unwrap(),
            Arc::new(Site::new("https://reader.example").unwrap()),
            access.clone(),
        )
        .unwrap();
        Self {
            server: Some(server),
            access,
            control,
            tokens,
            reference,
            root,
            _input: input,
        }
    }
    fn url(&self) -> String {
        format!("{}/mcp", self.server.as_ref().unwrap().url)
    }
    fn headers(&self) -> Headers {
        Headers(vec![
            ("Host".into(), "reader.example".into()),
            ("Authorization".into(), format!("Bearer {}", self.tokens[0])),
            ("Content-Type".into(), "application/json".into()),
            (
                "Accept".into(),
                "application/json, text/event-stream".into(),
            ),
            ("MCP-Protocol-Version".into(), PROTOCOL.into()),
        ])
    }
    fn rpc(&self, client: usize, msg: Value) -> Value {
        let response = post(&self.url(), &self.tokens[client], msg);
        assert_eq!(response.status(), 200);
        assert_eq!(response.header("Cache-Control"), Some("no-store"));
        assert!(response.header("Set-Cookie").is_none());
        response.into_json().unwrap()
    }
    fn tool(&self, client: usize, name: &str, args: Value) -> Value {
        self.rpc(client, json!({"jsonrpc":"2.0","id":"same-client-id","method":"tools/call","params":{"name":name,"arguments":args}}))["result"].clone()
    }
    fn text_args(&self) -> Value {
        json!({"published_book_ref":self.reference,"lid":"1.1"})
    }
    fn count(&self) -> i64 {
        self.control
            .connection
            .query_row("SELECT count(*) FROM mcp_usage", [], |r| r.get(0))
            .unwrap()
    }
    fn remaining(&self, n: i64) {
        self.access.mcp_usage.status("alice").unwrap();
        self.control
            .connection
            .execute(
                "UPDATE mcp_free_months SET used_units=granted_units-? WHERE user_id='alice'",
                [n],
            )
            .unwrap();
    }
    fn no_private_state(&self) {
        assert!(!self.root.path().join("users").exists());
        for table in ["reader_workspaces", "model_call_charges"] {
            let count: i64 = self
                .control
                .connection
                .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
                .unwrap();
            assert_eq!(count, 0, "{table}");
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.take().unwrap().shutdown();
    }
}
fn post(url: &str, token: &str, msg: Value) -> ureq::Response {
    let result = ureq::post(url)
        .set("Host", "reader.example")
        .set("Authorization", &format!("Bearer {token}"))
        .set("Accept", "application/json, text/event-stream")
        .set("MCP-Protocol-Version", PROTOCOL)
        .send_json(msg);
    match result {
        Ok(r) | Err(ureq::Error::Status(_, r)) => r,
        Err(err) => panic!("{err}"),
    }
}
fn init() -> Value {
    json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2099-01-01","capabilities":{},"clientInfo":{"name":"protocol-fixture","version":"1"}}})
}
fn body(reply: &HttpReply) -> Value {
    serde_json::from_slice(&reply.body).unwrap()
}

#[test]
fn remote_mcp_http_initialize_discover_read_continue_and_charge() {
    let f = Fixture::new();
    let initialized = ureq::post(&f.url())
        .set("Host", "reader.example")
        .set("Authorization", &format!("Bearer {}", f.tokens[0]))
        .set("Accept", "application/json, text/event-stream")
        .send_json(init())
        .unwrap();
    assert!(initialized.header("Mcp-Session-Id").is_none());
    assert_eq!(
        initialized.into_json::<Value>().unwrap()["result"]["protocolVersion"],
        PROTOCOL
    );
    let notified = post(
        &f.url(),
        &f.tokens[0],
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
    );
    assert_eq!(notified.status(), 202);
    assert_eq!(notified.into_string().unwrap(), "");
    let listed = f.rpc(0, json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}));
    let names: Vec<_> = listed["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["name"].as_str().unwrap())
        .collect();
    assert_eq!(names.len(), 11);
    assert!(names.contains(&"mcp_usage") && !names.contains(&"book_query"));
    let directory = f.tool(0, "book_list", json!({}));
    assert_eq!(
        directory["structuredContent"]["items"][0]["published_book_ref"],
        json!(f.reference)
    );
    assert_eq!(
        f.tool(2, "book_list", json!({}))["structuredContent"]["items"],
        json!([])
    );
    assert_eq!(
        f.tool(0, "mcp_usage", json!({}))["structuredContent"]["usage"]["free_remaining"],
        1000
    );
    assert_eq!(f.count(), 0);
    let first = f.tool(0, "book_text", f.text_args());
    assert_eq!(first["isError"], false);
    let data = &first["structuredContent"];
    assert_eq!(data["usage"]["free_remaining"], 999);
    assert_eq!(data["usage"]["charged_units"], 1);
    assert_eq!(data["usage"]["charged_from"]["kind"], "free_month");
    assert_eq!(data["data"]["text"].as_str().unwrap().chars().count(), 8000);
    assert!(data["sources"][0]["source_url"]
        .as_str()
        .unwrap()
        .starts_with("https://reader.example/?"));
    assert_eq!(
        serde_json::from_str::<Value>(first["content"][0]["text"].as_str().unwrap()).unwrap(),
        *data
    );
    let next = &data["next_calls"][0];
    let second = f.tool(1, next["name"].as_str().unwrap(), next["arguments"].clone());
    assert_eq!(second["structuredContent"]["usage"]["free_remaining"], 998);
    assert_eq!(
        second["structuredContent"]["data"]["source_range"]["start"],
        data["data"]["source_range"]["end"]
    );
    let empty = f.tool(
        0,
        "book_search_text",
        json!({"published_book_ref":f.reference,"query":"no-such-word"}),
    );
    assert_eq!(empty["isError"], false);
    assert_eq!(empty["structuredContent"]["usage"]["free_remaining"], 997);
    // Reusing JSON-RPC ids or successfully rereading is a new call.
    assert_eq!(
        f.tool(0, "book_text", f.text_args())["structuredContent"]["usage"]["free_remaining"],
        996
    );
    assert_eq!(f.count(), 4);
    f.no_private_state();
}

#[test]
fn remote_mcp_identity_origin_methods_versions_and_envelopes() {
    let f = Fixture::new();
    let site = Site::new("https://reader.example").unwrap();
    let run = |method: &str, h: &Headers, input: &str| {
        multi_user_host::dispatch(
            &f.access,
            &site,
            method,
            "/mcp",
            h,
            input,
            multi_user_host::now(),
        )
    };
    let ping = json!({"jsonrpc":"2.0","id":0,"method":"ping"}).to_string();
    assert_eq!(run("POST", &f.headers(), &ping).status, 200);
    for (key, value, status) in [
        ("Host", "other.example", 403),
        ("Origin", "https://other.example", 403),
        ("Authorization", "Bearer invalid", 401),
        ("MCP-Protocol-Version", "2025-03-26", 400),
        ("Content-Type", "text/plain", 415),
        ("Accept", "application/json", 406),
        ("Accept", "application/json, text/event-stream;q=0", 406),
    ] {
        let mut h = f.headers();
        h.0.retain(|(k, _)| k != key);
        h.0.push((key.into(), value.into()));
        assert_eq!(run("POST", &h, &ping).status, status, "{key}");
    }
    let mut same_origin = f.headers();
    same_origin
        .0
        .push(("Origin".into(), "https://reader.example".into()));
    assert_eq!(run("POST", &same_origin, &ping).status, 200);
    let mut cookie_only = f.headers();
    cookie_only.0.retain(|(k, _)| k != "Authorization");
    let cookie = f
        .access
        .auth
        .login("alice", "fixture-password", None, multi_user_host::now())
        .unwrap();
    cookie_only
        .0
        .push(("Cookie".into(), format!("{}={cookie}", crate::auth::COOKIE)));
    assert_eq!(run("POST", &cookie_only, &ping).status, 401);
    let mut missing_version = f.headers();
    missing_version
        .0
        .retain(|(k, _)| k != "MCP-Protocol-Version");
    assert_eq!(run("POST", &missing_version, &ping).status, 400);
    assert_eq!(
        run("POST", &missing_version, &init().to_string()).status,
        200
    );
    for method in ["GET", "DELETE", "PUT"] {
        assert_eq!(run(method, &f.headers(), "").status, 405);
    }
    let mut unsupported = f.headers();
    unsupported
        .0
        .retain(|(key, _)| key != "MCP-Protocol-Version");
    unsupported
        .0
        .push(("MCP-Protocol-Version".into(), "unsupported".into()));
    assert_eq!(run("GET", &unsupported, "").status, 400);
    for (input, code) in [
        ("{", -32700),
        ("[]", -32600),
        (r#"{"jsonrpc":"1.0","id":1,"method":"ping"}"#, -32600),
        (r#"{"jsonrpc":"2.0","id":null,"method":"ping"}"#, -32600),
        (r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#, -32602),
    ] {
        assert_eq!(
            body(&run("POST", &f.headers(), input))["error"]["code"],
            code
        );
    }
    assert_eq!(
        f.rpc(0, json!({"jsonrpc":"2.0","id":1,"method":"missing"}))["error"]["code"],
        -32601
    );
    assert_eq!(
        f.rpc(
            0,
            json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"book_query"}})
        )["error"]["code"],
        -32602
    );
    let notification = json!({"jsonrpc":"2.0","method":"tools/call","params":{"name":"book_text","arguments":f.text_args()}});
    let reply = run("POST", &f.headers(), &notification.to_string());
    assert_eq!(reply.status, 202);
    assert!(reply.body.is_empty());
    assert_eq!(
        run(
            "POST",
            &f.headers(),
            r#"{"jsonrpc":"2.0","id":1,"result":{}}"#
        )
        .status,
        202
    );
    assert_eq!(f.count(), 0);
    let get = ureq::get(&f.url())
        .set("Host", "reader.example")
        .set("Authorization", &format!("Bearer {}", f.tokens[0]))
        .call();
    let Err(ureq::Error::Status(405, response)) = get else {
        panic!("GET must reject")
    };
    assert_eq!(response.header("Allow"), Some("POST"));
    let response = post(&f.url(), "invalid", init());
    assert_eq!(response.status(), 401);
    assert!(response
        .header("WWW-Authenticate")
        .unwrap()
        .starts_with("Bearer"));
}

#[test]
fn remote_mcp_failed_reads_authorization_and_ledger_never_deliver_or_charge() {
    let f = Fixture::new();
    for (client, name, args, code) in [
        (0, "book_text", json!({}), "invalid_arguments"),
        (
            0,
            "mcp_usage",
            json!({"user_id":"bob"}),
            "invalid_arguments",
        ),
        (2, "book_text", f.text_args(), "material_unavailable"),
        (
            0,
            "book_paper_metadata",
            json!({"published_book_ref":f.reference}),
            "material_unavailable",
        ),
    ] {
        let result = f.tool(client, name, args);
        assert_eq!(result["isError"], true);
        assert_eq!(result["structuredContent"]["error_code"], code);
        assert!(result["structuredContent"].get("data").is_none());
    }
    let invalid_lid = f.tool(
        0,
        "book_text",
        json!({"published_book_ref":f.reference,"lid":"404.1"}),
    );
    assert_eq!(invalid_lid["isError"], true);
    assert_eq!(f.count(), 0);
    assert_eq!(
        f.access.mcp_usage.status("alice").unwrap().free_remaining,
        1000
    );
    f.control.connection.execute_batch("CREATE TRIGGER reject_mcp_debit BEFORE INSERT ON mcp_usage BEGIN SELECT RAISE(ABORT,'fixture'); END;").unwrap();
    let failed = f.tool(0, "book_text", f.text_args());
    assert_eq!(failed["isError"], true);
    assert_eq!(
        failed["structuredContent"]["cause_code"],
        "MCP_USAGE_STORAGE_UNAVAILABLE"
    );
    assert!(!failed.to_string().contains("needle"));
    assert_eq!(
        f.access.mcp_usage.status("alice").unwrap().free_remaining,
        1000
    );
    assert_eq!(f.count(), 0);
    f.control
        .connection
        .execute_batch("DROP TRIGGER reject_mcp_debit")
        .unwrap();
    assert_eq!(f.tool(0, "book_text", f.text_args())["isError"], false);
    f.access
        .library
        .lock()
        .unwrap()
        .revoke("alice", &f.reference)
        .unwrap();
    assert_eq!(
        f.tool(0, "book_text", f.text_args())["structuredContent"]["error_code"],
        "material_unavailable"
    );
    assert_eq!(f.count(), 1);
    f.control
        .connection
        .execute(
            "UPDATE mcp_credentials SET revoked_at=?",
            [multi_user_host::now()],
        )
        .unwrap();
    assert_eq!(post(&f.url(), &f.tokens[0], init()).status(), 401);
    f.no_private_state();
}

#[test]
fn remote_mcp_exhaustion_skips_load_and_keeps_free_tools_available() {
    let f = Fixture::new();
    f.remaining(0);
    assert_eq!(f.access.library.lock().unwrap().resident_usage().0, 0);
    let result = f.tool(0, "book_text", f.text_args());
    assert_eq!(result["isError"], true);
    assert_eq!(result["structuredContent"]["error_code"], "quota_exhausted");
    assert_eq!(result["structuredContent"]["retryable"], false);
    assert_eq!(result["structuredContent"]["usage"]["free_remaining"], 0);
    assert!(result["structuredContent"]["usage"]["free_resets_at"].is_string());
    assert_eq!(f.access.library.lock().unwrap().resident_usage().0, 0);
    assert_eq!(f.tool(0, "book_list", json!({}))["isError"], false);
    assert_eq!(f.tool(0, "mcp_usage", json!({}))["isError"], false);
    assert_eq!(
        f.rpc(0, json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}))["result"]["tools"]
            .as_array()
            .unwrap()
            .len(),
        11
    );
    assert_eq!(f.count(), 0);
}

#[test]
fn remote_mcp_http_two_clients_compete_for_last_call() {
    let f = Fixture::new();
    f.remaining(1);
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let handles: Vec<_>=(0..2).map(|i| {
        let (url,token,args,barrier)=(f.url(),f.tokens[i].clone(),f.text_args(),barrier.clone());
        std::thread::spawn(move || {
            barrier.wait();
            post(&url,&token,json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"book_text","arguments":args}})).into_json::<Value>().unwrap()
        })
    }).collect();
    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(
        results
            .iter()
            .filter(|v| v["result"]["isError"] == false)
            .count(),
        1
    );
    let failed = results
        .iter()
        .find(|v| v["result"]["isError"] == true)
        .unwrap();
    assert_eq!(
        failed["result"]["structuredContent"]["error_code"],
        "quota_exhausted"
    );
    assert!(!failed.to_string().contains("needle"));
    assert_eq!(f.count(), 1);
    assert_eq!(
        f.access.mcp_usage.status("alice").unwrap().free_remaining,
        0
    );
}

#[test]
fn remote_mcp_http_free_then_paid_and_purchase_restores_access() {
    let mut f = Fixture::new();
    f.control.set_reader_admin("alice", true).unwrap();
    let now = multi_user_host::now();
    let cookie = f
        .access
        .auth
        .login("alice", "fixture-password", None, now)
        .unwrap();
    let headers = Headers(vec![
        ("Host".into(), "reader.example".into()),
        ("Origin".into(), "https://reader.example".into()),
        ("Cookie".into(), format!("{}={cookie}", crate::auth::COOKIE)),
        ("X-CSRF-Token".into(), crate::auth::csrf(&cookie)),
    ]);
    let buy = |op: &str| {
        let reply=multi_user_host::dispatch(&f.access,&Site::new("https://reader.example").unwrap(),"POST","/api/admin/users/alice/mcp/packs",&headers,
            &json!({"operation_id":op,"pack_count":1,"paid_at":now,"channel":"fixture","note":"confirmed fixture"}).to_string(),now);
        assert_eq!(
            reply.status,
            200,
            "{}",
            String::from_utf8_lossy(&reply.body)
        );
        body(&reply)
    };
    let bought = buy("buy-first");
    f.remaining(1);
    let free = f.tool(0, "book_text", f.text_args());
    assert_eq!(
        free["structuredContent"]["usage"]["charged_from"]["kind"],
        "free_month"
    );
    assert_eq!(free["structuredContent"]["usage"]["paid_remaining"], 1000);
    let paid = f.tool(1, "book_text", f.text_args());
    assert_eq!(
        paid["structuredContent"]["usage"]["charged_from"]["pack_id"],
        bought["call_pack"]["pack_id"]
    );
    assert_eq!(paid["structuredContent"]["usage"]["paid_remaining"], 999);
    f.control
        .connection
        .execute("UPDATE mcp_call_packs SET used_units=total_units", [])
        .unwrap();
    assert_eq!(
        f.tool(0, "book_text", f.text_args())["structuredContent"]["error_code"],
        "quota_exhausted"
    );
    buy("buy-next");
    assert_eq!(
        f.tool(0, "book_text", f.text_args())["structuredContent"]["usage"]["paid_remaining"],
        999
    );
    assert_eq!(f.count(), 3);
}
