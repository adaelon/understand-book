use super::*;
use crate::{
    auth::AuthService,
    control_store::{ControlStore, ServiceWriter},
    mcp_access::McpAccess,
};
use base_schema::{LidNode, NodeKind, Span};
use std::{
    collections::{BTreeMap, VecDeque},
    fs,
    path::Path,
};

const NOW: i64 = 1_790_000_000;
const PASSWORD: &str = "fixture-only-password";

struct Fixture {
    library: Mutex<PublishedLibrary>,
    access: McpAccess,
    control: ControlStore,
    tokens: BTreeMap<String, String>,
    root: tempfile::TempDir,
    input: tempfile::TempDir,
    a: PublishedBookRef,
    b: PublishedBookRef,
}
impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let input = tempfile::tempdir().unwrap();
        let writer = ServiceWriter::acquire(root.path()).unwrap();
        let mut control = ControlStore::open(writer.clone()).unwrap();
        let auth = AuthService::new(ControlStore::open(writer.clone()).unwrap());
        let access = McpAccess::new(ControlStore::open(writer.clone()).unwrap());
        let mut tokens = BTreeMap::new();
        for user in ["alice", "bob"] {
            control.provision_password(user, PASSWORD, true).unwrap();
            let cookie = auth.login(user, PASSWORD, None, NOW).unwrap();
            let principal = auth.authenticate(&cookie, NOW).unwrap();
            tokens.insert(
                user.into(),
                access
                    .create(&principal, "Reader Agent", NOW)
                    .unwrap()
                    .token,
            );
        }
        let mut library = PublishedLibrary::new(ControlStore::open(writer).unwrap());
        material(&input.path().join("a"), "alpha", true);
        material(&input.path().join("b"), "beta", false);
        let a = library.publish(&input.path().join("a")).unwrap().reference;
        let b = library.publish(&input.path().join("b")).unwrap().reference;
        library.grant("alice", &a).unwrap();
        library.grant("bob", &b).unwrap();
        Self {
            library: Mutex::new(library),
            access,
            control,
            tokens,
            root,
            input,
            a,
            b,
        }
    }
    fn call(&self, user: &str, name: &str, args: Value) -> Result<Value, ToolError> {
        let identity = self
            .access
            .authenticate(Some(&format!("Bearer {}", self.tokens[user])))?;
        McpRead::new(&self.library, "https://reader.example")
            .unwrap()
            .call(&identity, name, args)
    }
    fn next(&self, user: &str, call: &Value) -> Result<Value, ToolError> {
        self.call(
            user,
            call["name"].as_str().unwrap(),
            call["arguments"].clone(),
        )
    }
}
pub(crate) fn material(path: &Path, id: &str, enriched: bool) {
    fs::create_dir_all(path).unwrap();
    let mut source = "\n\n".to_string();
    let mut base = base_schema::sample_base();
    base.book_id = id.into();
    let mut nodes = Vec::new();
    for i in 1..=60 {
        let start = source.encode_utf16().count();
        source.push_str(&format!(
            "needle {i} 中文😀 {}\n\n",
            if i == 1 {
                "甲".repeat(16_005)
            } else {
                "content".into()
            }
        ));
        nodes.push(LidNode {
            lid: format!("1.{i}"),
            path: vec![1, i],
            kind: NodeKind::Paragraph,
            span: Span {
                start,
                end: source.encode_utf16().count(),
            },
            children: vec![],
        });
    }
    base.lid_nodes = vec![LidNode {
        lid: "1".into(),
        path: vec![1],
        kind: NodeKind::Chapter,
        span: Span {
            start: 0,
            end: source.encode_utf16().count(),
        },
        children: nodes.iter().map(|n| n.lid.clone()).collect(),
    }];
    base.lid_nodes.extend(nodes);
    base.graph_nodes[0].occurrences = (1..=60).map(|i| format!("1.{i}")).collect();
    fs::write(path.join("base.json"), serde_json::to_vec(&base).unwrap()).unwrap();
    fs::write(path.join("source.txt"), source).unwrap();
    if enriched {
        // Paper fixtures enter through the same source-readiness gate as published materials.
        let book = read_tools::Book::load(path.to_str().unwrap()).unwrap();
        fs::create_dir_all(path.join(".build/source-reconciliation")).unwrap();
        fs::write(
            path.join(".build/source-reconciliation/report.json"),
            json!({"version":"source_reconciliation_report.v1","book_id":id,"unresolved":[]})
                .to_string(),
        )
        .unwrap();
        let unavailable = json!({"status":"unavailable","reason":"text-only test material"});
        fs::write(
            path.join("source_manifest.json"),
            json!({"version":"source_manifest.v2","book_id":id,
            "canonical_source":{"path":"source.txt","sha256":book.source_fingerprint()},
            "capabilities":{"view_pdf":unavailable,"project_lid_to_pdf":unavailable,
                "project_ranges_to_pdf":unavailable,"resolve_pdf_selection":unavailable}})
            .to_string(),
        )
        .unwrap();
        let header = json!({"book_id":id,"book_version":"v1","profile_id":"paper","profile_version":"paper_v0",
            "core_schema_version":"v1","generated_at":"2026-10-11T00:00:00Z"});
        let anchored = json!({"text":"A built explanation", "evidence_lids":["1.1"]});
        let spine: Vec<_> = (1..=60)
            .map(|i| {
                json!({"lid":format!("1.{i}"),"role":"foundation","summary":anchored,
            "key_stop_ids":[],"depends_on":[]})
            })
            .collect();
        let threads: Vec<_> = (1..=60).map(|i|json!({"id":format!("t{i}"),"name":format!("Thread {i}"),"summary":anchored,
            "lids":if i == 1 { (1..=60).map(|i|format!("1.{i}")).collect::<Vec<_>>() } else { vec!["1.1".into()] },"key_stop_ids":[]})).collect();
        fs::write(
            path.join("book_structure.json"),
            json!({"header":header,"spine":spine,"throughlines":threads,"key_stops":[]})
                .to_string(),
        )
        .unwrap();
        fs::write(path.join("paper_metadata.json"), json!({"header":header,"title":{"value":"Fixture Paper","source":"front_matter","evidence_lids":["1.1"],"confidence":1.0},
            "references":{"value":(1..=60).map(|i|json!({"raw":format!("Reference {i}"),"identifiers":null})).collect::<Vec<_>>(),
                "source":"paper_text","evidence_lids":["1.60"],"confidence":1.0}}).to_string()).unwrap();
        fs::write(path.join("paper_lexicon.json"), json!({"header":header,"entries":(1..=60).map(|i|json!({
            "term":format!("term{i}"),"term_type":"term","occurrences_lids":[format!("1.{i}")],"defined_at_lid":format!("1.{i}"),"aliases":[]})).collect::<Vec<_>>()}).to_string()).unwrap();
    }
}

#[test]
fn mcp_read_catalog_isolates_accounts_pages_versions_and_avoids_private_or_resident_state() {
    let f = Fixture::new();
    let metadata_path = f.input.path().join("a/paper_metadata.json");
    let mut metadata: Value = serde_json::from_slice(&fs::read(&metadata_path).unwrap()).unwrap();
    metadata["title"]["value"] = json!("Updated publication metadata");
    fs::write(metadata_path, metadata.to_string()).unwrap();
    let second = {
        let mut library = f.library.lock().unwrap();
        let second = library
            .publish(&f.input.path().join("a"))
            .unwrap()
            .reference;
        library.grant("alice", &second).unwrap();
        second
    };
    let first = f.call("alice", "book_list", json!({"limit":1})).unwrap();
    assert_eq!(first["items"][0]["published_book_ref"], json!(f.a));
    assert_eq!(first["items"][0]["name"], "Fixture Paper");
    assert_eq!(
        first["items"][0]["available_tools"]
            .as_array()
            .unwrap()
            .len(),
        9
    );
    assert_eq!(first["items"][0]["is_default"], false);
    assert_eq!(first["complete"], false);
    let last = f.next("alice", &first["next_calls"][0]).unwrap();
    assert_eq!(last["items"][0]["published_book_ref"], json!(second));
    assert_eq!(last["complete"], true);
    assert_eq!(last["items"][0]["is_default"], true);
    assert_eq!(last["items"][0]["name"], "Updated publication metadata");
    let bob = f.call("bob", "book_list", json!({})).unwrap();
    assert_eq!(bob["items"][0]["published_book_ref"], json!(f.b));
    assert_eq!(bob["items"].as_array().unwrap().len(), 1);
    assert_eq!(f.library.lock().unwrap().resident_usage(), (0, 0));
    for (user, reference) in [("alice", &f.b), ("bob", &f.a)] {
        assert_eq!(
            f.call(
                user,
                "book_text",
                json!({"published_book_ref":reference,"lid":"1"})
            )
            .unwrap_err()
            .error_code,
            "MCP_MATERIAL_UNAVAILABLE"
        );
    }
    for reference in [&f.a, &second] {
        let result = f
            .call(
                "alice",
                "book_text",
                json!({"published_book_ref":reference,"lid":"1.2"}),
            )
            .unwrap();
        assert_eq!(result["published_book_ref"], json!(reference));
        assert_eq!(result["sources"][0]["published_book_ref"], json!(reference));
        let url = url::Url::parse(result["sources"][0]["source_url"].as_str().unwrap()).unwrap();
        assert_eq!(url.origin().ascii_serialization(), "https://reader.example");
        assert_eq!(
            url.query_pairs()
                .find(|(key, _)| key == "publication_id")
                .unwrap()
                .1,
            reference.publication_id
        );
        let metadata = f
            .call(
                "alice",
                "book_paper_metadata",
                json!({"published_book_ref":reference}),
            )
            .unwrap();
        assert_eq!(
            metadata["data"]["title"]["value"],
            if reference == &f.a {
                "Fixture Paper"
            } else {
                "Updated publication metadata"
            }
        );
    }
    assert!(!f.root.path().join("users").exists());
    assert_eq!(
        f.control
            .connection
            .query_row("SELECT count(*) FROM reader_workspaces", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[test]
fn mcp_read_text_pages_keep_exact_source_and_recheck_cached_grants_on_continuation() {
    let mut f = Fixture::new();
    let pinned = f.library.lock().unwrap().load("alice", &f.a).unwrap();
    let expected = pinned.book.text("1.1", Some("1.60")).unwrap();
    let mut result = f
        .call(
            "alice",
            "book_text",
            json!({"published_book_ref":f.a,"lid":"1.1","end_lid":"1.60"}),
        )
        .unwrap();
    let first = result.clone();
    let mut text = String::new();
    let mut position = result["data"]["requested_range"]["start"].as_u64().unwrap();
    loop {
        assert_eq!(result["data"]["source_range"]["start"], position);
        let fragment = result["data"]["text"].as_str().unwrap();
        assert!(fragment.chars().count() <= 8_000);
        position += fragment.encode_utf16().count() as u64;
        assert_eq!(result["data"]["source_range"]["end"], position);
        assert_eq!(
            result["sources"][0]["source_range"],
            result["data"]["source_range"]
        );
        let url = url::Url::parse(result["sources"][0]["source_url"].as_str().unwrap()).unwrap();
        assert_eq!(
            url.query_pairs()
                .find(|(key, _)| key == "end_utf16")
                .unwrap()
                .1,
            position.to_string()
        );
        text.push_str(fragment);
        if result["complete"] == true {
            break;
        }
        result = f.next("alice", &result["next_calls"][0]).unwrap();
    }
    assert_eq!(text, expected);
    f.library.lock().unwrap().revoke("alice", &f.a).unwrap();
    assert_eq!(
        f.next("alice", &first["next_calls"][0])
            .unwrap_err()
            .error_code,
        "MCP_MATERIAL_UNAVAILABLE"
    );
    assert_eq!(
        f.call("alice", "book_list", json!({})).unwrap()["items"],
        json!([])
    );
    f.library.lock().unwrap().grant("alice", &f.a).unwrap();
    f.control.set_user_disabled("alice", true).unwrap();
    assert_eq!(
        f.next("alice", &first["next_calls"][0])
            .unwrap_err()
            .error_code,
        "MCP_AUTH_REQUIRED"
    );
}

fn reassemble(f: &Fixture, name: &str, args: Value) -> Value {
    let first = f.call("alice", name, args).unwrap();
    let mut data = first["data"].clone();
    let mut pending: VecDeque<_> = first["next_calls"]
        .as_array()
        .unwrap()
        .iter()
        .cloned()
        .collect();
    let mut pages = 0;
    while let Some(call) = pending.pop_front() {
        let result = f.next("alice", &call).unwrap();
        if name == "book_paper_metadata" {
            assert!(
                !result["sources"].as_array().unwrap().is_empty(),
                "metadata page must retain enclosing evidence"
            );
        }
        let path = result["data_path"].as_str().unwrap();
        let target = data.pointer_mut(path).unwrap().as_array_mut().unwrap();
        assert_eq!(
            target.len(),
            call["arguments"]["page"]["offset"].as_u64().unwrap() as usize
        );
        assert!(result["data"].as_array().unwrap().len() <= MAX_PAGE_ITEMS);
        target.extend(result["data"].as_array().unwrap().iter().cloned());
        pending.extend(result["next_calls"].as_array().unwrap().iter().cloned());
        pages += 1;
        assert!(pages < 100);
    }
    assert!(pages > 0, "fixture must exercise continuation");
    data
}

#[test]
fn mcp_read_to8_nested_candidate_calls_remain_executable() {
    let f = Fixture::new();
    let args = json!({"published_book_ref":f.a,"query":"needle","limit":1});
    let first = f.call("alice", "book_concept", args.clone()).unwrap();
    assert_eq!(first["complete"], false);
    assert!(first["next_calls"].as_array().unwrap().iter().any(|c| c["arguments"]["page"]["path"] == "/candidates/0/next_calls"));
    let data = reassemble(&f, "book_concept", args);
    let candidate = &data["candidates"][0];
    let calls = candidate["next_calls"].as_array().unwrap();
    assert_eq!(calls.len(), 60);
    for (lid, call) in candidate["occurrences"].as_array().unwrap().iter().zip(calls) {
        assert_eq!(call["arguments"]["lid"], *lid);
        assert_eq!(call["arguments"]["published_book_ref"], json!(f.a));
    }
    let last = f.next("alice", calls.last().unwrap()).unwrap();
    assert!(last["data"]["text"].as_str().unwrap().starts_with("needle 60 "));
    assert_eq!(last["sources"][0]["start_lid"], "1.60");
    assert_eq!(last["sources"][0]["published_book_ref"], json!(f.a));
}

#[test]
fn mcp_read_to9_context_pagination_and_missing_structure_are_independent() {
    let f = Fixture::new();
    let mut samples = Vec::new();
    // The chapter has 60 children. The transport page size is 20, independent of k.
    for k in [0, 10, 40, 60] {
        let args = json!({"published_book_ref":f.b,"lid":"1","granularity":"far","k":k});
        let first = f.call("bob", "book_context", args.clone()).unwrap();
        assert_eq!(first["data"]["total_candidates"], 60);
        assert_eq!(first["data"]["returned_count"], k);
        assert_eq!(first["data"]["truncated"], k < 60);
        assert_eq!(first["complete"], k <= 20);
        let mut items = first["data"]["items"].as_array().unwrap().clone();
        let mut pending = VecDeque::from(first["next_calls"].as_array().unwrap().clone());
        let mut pages = Vec::new();
        while let Some(next) = pending.pop_front() {
            let page = f.next("bob", &next).unwrap();
            assert_eq!(page["data_path"], "/items");
            assert_eq!(page["total_items"], k);
            assert_eq!(page["published_book_ref"], json!(f.b));
            items.extend(page["data"].as_array().unwrap().iter().cloned());
            pending.extend(page["next_calls"].as_array().unwrap().iter().cloned());
            pages.push(page);
            assert!(pages.len() <= 2);
        }
        assert_eq!(items.len(), k as usize);
        for (i, item) in items.iter().enumerate() {
            assert_eq!(item["lid"], format!("1.{}", i + 1));
            let next = &item["next_calls"][0];
            assert_eq!(next["name"], "book_text");
            assert_eq!(next["arguments"]["published_book_ref"], json!(f.b));
        }
        if let Some(last) = items.last() {
            let read = f.next("bob", &last["next_calls"][0]).unwrap();
            assert!(read["data"]["text"].as_str().unwrap().contains(&format!("needle {k}")));
        }
        samples.push(json!({"input":args,"first":first,"pages":pages,"reconstructed_count":items.len()}));
    }
    let list = f.call("bob", "book_list", json!({})).unwrap();
    let available = list["items"][0]["available_tools"].as_array().unwrap();
    assert!(available.contains(&json!("book_context")));
    assert!(!available.contains(&json!("book_structure")));
    let missing = f.call("bob", "book_structure", json!({"published_book_ref":f.b})).unwrap_err();
    assert_eq!(missing.error_code, "MCP_CAPABILITY_UNAVAILABLE");
    if let Ok(path) = std::env::var("TO9_REMOTE") {
        fs::write(path, serde_json::to_vec_pretty(&json!({
            "samples":samples,"list":list,"missing_structure":missing,
        })).unwrap()).unwrap();
    }
}

#[test]
fn mcp_read_nested_structure_and_paper_collections_reassemble_without_omissions() {
    let f = Fixture::new();
    let publication = f.library.lock().unwrap().load("alice", &f.a).unwrap();
    let book = &publication.book;
    for (name, expected) in [
        ("book_structure", json!(book.structure(None).unwrap())),
        ("book_guide_path", json!(book.guide_path(None).unwrap())),
        (
            "book_paper_metadata",
            json!(book.paper_metadata_projection()),
        ),
        ("book_paper_lexicon", json!(book.paper_lexicon_projection())),
        (
            "book_paper_reading_guide",
            json!(book.paper_reading_guide(None, None).unwrap()),
        ),
    ] {
        assert_eq!(
            reassemble(&f, name, json!({"published_book_ref":f.a})),
            expected,
            "{name}"
        );
    }
    // Collection reconstruction compares the remote shape, including version-bound calls.
    let mut expected = json!(book.concept_candidates("command", None, None).unwrap());
    for candidate in expected["candidates"].as_array_mut().unwrap() {
        candidate["next_calls"] = json!(candidate["occurrences"].as_array().unwrap().iter()
            .map(|lid| json!({"name":"book_text","arguments":{"lid":lid,"published_book_ref":f.a}}))
            .collect::<Vec<_>>());
    }
    assert_eq!(
        reassemble(
            &f,
            "book_concept",
            json!({"published_book_ref":f.a,"query":"command"})
        ),
        expected
    );
    let scoped = f
        .call(
            "alice",
            "book_structure",
            json!({"published_book_ref":f.a,"at":"1.1"}),
        )
        .unwrap();
    assert_eq!(scoped["data"]["spine_unit"]["lid"], "1.1");
    let first = f
        .call("alice", "book_structure", json!({"published_book_ref":f.a}))
        .unwrap();
    f.library.lock().unwrap().revoke("alice", &f.a).unwrap();
    assert_eq!(
        f.next("alice", &first["next_calls"][0])
            .unwrap_err()
            .error_code,
        "MCP_MATERIAL_UNAVAILABLE"
    );
}

#[test]
fn mcp_read_search_context_empty_results_and_remote_parameter_validation() {
    let f = Fixture::new();
    // Search has independent occurrence-batch and collection (including section-count) continuations.
    for page_size in [3, 50] {
        let mut pending = VecDeque::from([json!({"name":"book_search_text",
            "arguments":{"published_book_ref":f.a,"query":"needle","page_size":page_size}})]);
        let mut ordinals = Vec::new();
        let mut calls = 0;
        while let Some(call) = pending.pop_front() {
            let result = f.next("alice", &call).unwrap();
            let occurrences = if result["data_path"] == "" {
                assert_eq!(result["data"]["total_occurrences"], 60);
                Some(&result["data"]["occurrences"])
            } else if result["data_path"] == "/occurrences" {
                Some(&result["data"])
            } else {
                None
            };
            if let Some(occurrences) = occurrences {
                assert!(!result["sources"].as_array().unwrap().is_empty());
                ordinals.extend(
                    occurrences
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|v| v["ordinal"].as_u64().unwrap()),
                );
            }
            pending.extend(result["next_calls"].as_array().unwrap().iter().cloned());
            calls += 1;
            assert!(calls < 100);
        }
        ordinals.sort_unstable();
        assert_eq!(ordinals.len(), 60);
        assert!(ordinals.windows(2).all(|pair| pair[0] + 1 == pair[1]));
    }
    let empty = f
        .call(
            "alice",
            "book_search_text",
            json!({"published_book_ref":f.a,"query":"not-in-fixture"}),
        )
        .unwrap();
    assert_eq!(empty["data"]["occurrences"], json!([]));
    assert_eq!(empty["complete"], true);
    let context = f
        .call(
            "alice",
            "book_context",
            json!({"published_book_ref":f.a,"lid":"1.1","granularity":"near","k":3}),
        )
        .unwrap();
    assert_eq!(context["data"]["anchor"], "1.1");
    assert!(!context["sources"].as_array().unwrap().is_empty());
    for name in [
        "book_query",
        "book_synthesize",
        "book_guide",
        "book_manifest",
        "mcp_usage",
        "unknown",
    ] {
        assert_eq!(
            f.call("alice", name, json!({})).unwrap_err().error_code,
            "MCP_TOOL_UNAVAILABLE"
        );
    }
    for (name, args) in [
        ("book_text", json!({"lid":"1"})),
        (
            "book_text",
            json!({"published_book_ref":f.a,"lid":"1","user_id":"bob"}),
        ),
        (
            "book_text",
            json!({"published_book_ref":f.a,"lid":"1","start_utf16":-1}),
        ),
        (
            "book_text",
            json!({"published_book_ref":f.a,"lid":"1","page":{"path":""}}),
        ),
        (
            "book_structure",
            json!({"published_book_ref":f.a,"page":{"path":"/throughlines","offset":999}}),
        ),
        (
            "book_structure",
            json!({"published_book_ref":f.a,"page":{"path":"/available"}}),
        ),
        (
            "book_structure",
            json!({"published_book_ref":f.a,"page":{"path":"/throughlines","limit":0}}),
        ),
        (
            "book_structure",
            json!({"published_book_ref":f.a,"page":{"path":"/throughlines","limit":51}}),
        ),
        (
            "book_search_text",
            json!({"published_book_ref":f.a,"query":"needle","page_size":51}),
        ),
        ("book_list", json!({"limit":0})),
        ("book_list", json!({"user_id":"bob"})),
        (
            "book_list",
            json!({"after":{"book_id":"alpha","publication_id":f.a.publication_id,"extra":1}}),
        ),
    ] {
        assert_eq!(
            f.call("alice", name, args.clone()).unwrap_err().category,
            "validation",
            "{name}: {args}"
        );
    }
    for name in [
        "book_structure",
        "book_guide_path",
        "book_paper_metadata",
        "book_paper_lexicon",
        "book_paper_reading_guide",
    ] {
        assert_eq!(
            f.call("bob", name, json!({"published_book_ref":f.b}))
                .unwrap_err()
                .error_code,
            "MCP_CAPABILITY_UNAVAILABLE"
        );
    }
    let definitions = tool_definitions();
    assert_eq!(definitions.len(), 10);
    for tool in definitions.iter().skip(1) {
        assert_eq!(tool["inputSchema"]["additionalProperties"], false);
        assert!(tool["inputSchema"]["required"]
            .as_array()
            .unwrap()
            .contains(&json!("published_book_ref")));
    }
}

#[test]
fn mcp_read_to10_reverse_search_preserves_native_and_collection_continuations() {
    let f = Fixture::new();
    let input = json!({"published_book_ref":f.a,"query":"needle","match_mode":"exact","scope":{"within_lid":"1"},"order":"reverse_document","page_size":50});
    let mut pending = VecDeque::from([json!({"name":"book_search_text","arguments":input})]);
    let mut hits = Vec::new();
    let mut calls = Vec::new();
    while let Some(call) = pending.pop_front() {
        for key in [
            "published_book_ref",
            "query",
            "match_mode",
            "scope",
            "order",
            "page_size",
        ] {
            assert_eq!(call["arguments"][key], input[key]);
        }
        let result = f.next("alice", &call).unwrap();
        let occurrences = if result["data_path"] == "" {
            Some(&result["data"]["occurrences"])
        } else if result["data_path"] == "/occurrences" {
            Some(&result["data"])
        } else {
            None
        };
        if let Some(occurrences) = occurrences {
            let page: Vec<_> = occurrences
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v["ordinal"].as_u64().unwrap())
                .collect();
            assert!(page.windows(2).all(|p| p[0] == p[1] + 1));
            hits.extend(page);
        }
        pending.extend(result["next_calls"].as_array().unwrap().iter().cloned());
        calls.push(json!({"call":call,"result":result}));
        assert!(calls.len() < 20);
    }
    hits.sort_unstable();
    assert_eq!(hits, (1..=60).collect::<Vec<_>>());
    if let Ok(path) = std::env::var("TO10_REMOTE_SEARCH") {
        fs::write(path, serde_json::to_vec_pretty(&calls).unwrap()).unwrap();
    }
}
