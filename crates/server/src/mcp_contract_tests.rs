//! Public discovery and result contracts, exercised through the real HTTP fixture.
use super::*;
use std::collections::{BTreeMap, BTreeSet};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

fn discovery(f: &Fixture) -> Value {
    f.rpc(0, json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}))["result"]["tools"].clone()
}

#[test]
#[ignore = "one-time fixture capture; set TOOL_EXAMPLE_REFERENCE to an output path"]
fn mcp_contract_capture_example_publication() {
    let f = Fixture::with_example_book(true);
    let path = std::env::var("TOOL_EXAMPLE_REFERENCE").unwrap();
    let manifest = f.access.library.lock().unwrap().publish(f._input.path()).unwrap();
    let directory = std::path::Path::new(&path);
    std::fs::create_dir_all(directory).unwrap();
    std::fs::write(directory.join("publication.json"), serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
    std::fs::write(directory.join("publication-ref.json"), serde_json::to_vec_pretty(&manifest.reference).unwrap()).unwrap();
}

/// Explicit measurement entry point; ordinary tests never write snapshots.
#[test]
#[ignore = "set TOOL_CONTRACT_SNAPSHOT and run explicitly to capture published definitions"]
fn mcp_contract_export_tool_definitions() {
    let f = Fixture::new();
    let resident: Vec<_> = runtime::orchestrator::tool_specs()
        .into_iter()
        .map(|spec| json!({"name":spec.name,"description":spec.description,"inputSchema":spec.parameters}))
        .collect();
    let snapshot = json!({
        "resident": resident,
        "local_mcp": crate::mcp::tools_list_result()["tools"],
        "remote_mcp": discovery(&f),
    });
    let path = std::env::var("TOOL_CONTRACT_SNAPSHOT").expect("snapshot output path is required");
    std::fs::write(path, serde_json::to_vec_pretty(&snapshot).unwrap()).unwrap();
    assert_eq!(f.count(), 0);
}

#[test]
fn mcp_contract_initialize_and_all_tool_schemas_explain_the_reading_path() {
    let f = Fixture::new();
    let initialized = f.rpc(0, init());
    let instructions = initialized["result"]["instructions"].as_str().unwrap();
    for term in [
        "book_list",
        "available_tools",
        "book_search_text",
        "book_concept",
        "book_text",
        "sources",
        "UTF-16",
        "next_calls",
        "charged_units",
        "charged_from",
        "free_resets_at",
        "paid_next_expires_at",
        "call_packs",
        "quota_exhausted",
        "unauthenticated",
        "material_unavailable",
        "invalid_arguments",
        "retry_after_seconds",
        "top_up_instruction",
    ] {
        assert!(instructions.contains(term), "initialization omits {term}");
    }
    let tools = discovery(&f);
    let tools = tools.as_array().unwrap();
    let expected: BTreeSet<_> = [
        "book_list",
        "mcp_usage",
        "book_text",
        "book_search_text",
        "book_context",
        "book_concept",
        "book_structure",
        "book_guide_path",
        "book_paper_metadata",
        "book_paper_lexicon",
        "book_paper_reading_guide",
    ]
    .into_iter()
    .collect();
    assert_eq!(tools.len(), expected.len());
    assert_eq!(
        tools
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect::<BTreeSet<_>>(),
        expected
    );
    for tool in tools {
        let name = tool["name"].as_str().unwrap();
        let description = tool["description"].as_str().unwrap();
        let schema = &tool["inputSchema"];
        assert_eq!(schema["type"], "object", "{name}");
        assert_eq!(schema["additionalProperties"], false, "{name}");
        assert!(
            !description.contains("book.text"),
            "remote aliases only: {name}"
        );
        assert!(description.contains("usage"), "{name}");
        if name == "book_list" || name == "mcp_usage" {
            assert!(description.contains("Costs 0 calls"), "{name}");
            assert!(description.contains("exhausted"), "{name}");
            continue;
        }
        for term in [
            "Use when:",
            "Costs 1 call",
            "failures cost 0",
            "next_calls",
            "sources",
            "source_url",
            "source_range",
            "charged_units",
            "quota_exhausted",
        ] {
            assert!(description.contains(term), "{name} omits {term}");
        }
        assert_eq!(
            schema["properties"]["published_book_ref"]["required"],
            json!(["book_id", "publication_id"])
        );
        assert_eq!(
            schema["properties"]["published_book_ref"]["additionalProperties"],
            false
        );
        for (key, property) in schema["properties"].as_object().unwrap() {
            assert!(
                property["description"]
                    .as_str()
                    .is_some_and(|s| !s.is_empty()),
                "{name}.{key}"
            );
        }
        // Compare all shared semantics, including nested descriptions and defaults.
        let mut local = schema.clone();
        if let Some(examples) = local.get_mut("examples").and_then(Value::as_array_mut) {
            for example in examples {
                assert!(example.as_object_mut().unwrap().remove("published_book_ref").is_some());
            }
        }
        let properties = local["properties"].as_object_mut().unwrap();
        properties.remove("published_book_ref");
        properties.remove(if name == "book_text" {
            "start_utf16"
        } else {
            "page"
        });
        let required = local["required"].as_array_mut().unwrap();
        assert!(required.contains(&json!("published_book_ref")));
        required.retain(|v| v != "published_book_ref");
        let original =
            book_tool_contracts::input_schema(book_tool_contracts::from_mcp_alias(name).unwrap());
        if original.get("properties").is_none() {
            assert!(local["properties"].as_object().unwrap().is_empty());
            local.as_object_mut().unwrap().remove("properties");
        }
        if original.get("required").is_none() {
            local.as_object_mut().unwrap().remove("required");
        }
        assert_eq!(local, original, "{name}");
    }
    assert_eq!(f.count(), 0);
}

#[test]
fn mcp_contract_local_and_resident_keep_common_semantics_without_remote_fields() {
    let local = crate::mcp::tools_list_result();
    let resident = runtime::orchestrator::tool_specs();
    for contract in book_tool_contracts::contracts() {
        let schema = book_tool_contracts::input_schema(contract.id);
        let name = contract.aliases.mcp.unwrap();
        let tool = local["tools"].as_array().unwrap().iter().find(|t| t["name"] == name).unwrap();
        assert_eq!(tool["inputSchema"], schema, "{name}");
        let description = tool["description"].as_str().unwrap();
        assert!(description.contains(contract.use_when), "{name}");
        assert!(description.contains(&contract.do_not_use_when.replace("book.text", "book_text")), "{name}");
        assert!(!description.contains("book.text"), "{name}");
        for remote_term in ["published_book_ref", "start_utf16", "next_calls", "charged_units", "source_url"] {
            assert!(!tool.to_string().contains(remote_term), "{name}: {remote_term}");
        }
        if let Some(alias) = contract.aliases.resident {
            let spec = resident.iter().find(|s| s.name == alias).unwrap();
            assert_eq!(spec.parameters, schema, "{alias}");
            assert_eq!(spec.description, contract.description, "{alias}");
        }
    }
}

fn assert_usage(usage: &Value, charged: i64) {
    assert_eq!(usage["charged_units"], charged);
    for field in [
        "free_remaining",
        "paid_remaining",
        "free_granted",
        "free_used",
    ] {
        assert!(usage[field].as_i64().is_some_and(|n| n >= 0), "{field}");
    }
    let reset = OffsetDateTime::parse(usage["free_resets_at"].as_str().unwrap(), &Rfc3339).unwrap();
    assert_eq!(reset.offset().whole_hours(), 8);
    assert_eq!(reset.day(), 1);
    assert_eq!(reset.time().as_hms(), (0, 0, 0));
    assert!(usage["call_packs"].is_array());
    let purchase = url::Url::parse(usage["top_up_url"].as_str().unwrap()).unwrap();
    assert_eq!(purchase.path(), "/");
    assert_eq!(purchase.query(), Some("account=mcp"));
    for term in [
        "CNY 10",
        "1,000",
        "12 calendar months",
        "user",
        "administrator",
    ] {
        assert!(usage["top_up_instruction"].as_str().unwrap().contains(term));
    }
    if charged == 0 {
        assert!(usage["charged_from"].is_null());
    }
}

fn projection(result: &Value) -> &Value {
    let value = &result["structuredContent"];
    assert_eq!(
        serde_json::from_str::<Value>(result["content"][0]["text"].as_str().unwrap()).unwrap(),
        *value
    );
    value
}

#[test]
fn mcp_contract_remote_advertised_examples_execute_unchanged() {
    let f = Fixture::with_example_book(true);
    let tools = discovery(&f);
    let directory = f.tool(0, "book_list", json!({}));
    let reference = &projection(&directory)["items"][0]["published_book_ref"];
    assert_eq!(*reference, json!(f.reference));
    let mut count = 0;
    for tool in tools.as_array().unwrap() {
        let Some(examples) = tool["inputSchema"]["examples"].as_array() else { continue };
        let name = tool["name"].as_str().unwrap();
        for input in examples {
            assert_eq!(input["published_book_ref"], *reference);
            let result = f.tool(0, name, input.clone());
            assert_eq!(result["isError"], false, "{name}: {result}");
            let result = projection(&result);
            assert_usage(&result["usage"], 1);
            assert_eq!(result["published_book_ref"], *reference);
            match name {
                "book_text" => {
                    let source = include_str!("../../book-tool-contracts/fixtures/example-book/source.txt");
                    assert_eq!(result["data"]["text"], source.lines().take(2).collect::<Vec<_>>().join("\n") + "\n");
                    assert_eq!(result["sources"][0]["published_book_ref"], *reference);
                    assert_eq!(result["sources"][0]["start_lid"], "1.1");
                    assert_eq!(result["sources"][0]["end_lid"], "1.2");
                }
                "book_search_text" => assert_eq!(result["data"]["occurrences"][0]["start_lid"], if input["order"] == "document" { "1.1" } else { "1.2" }),
                "book_concept" => assert_eq!(result["data"]["returned_count"], 2),
                "book_context" => assert!(!result["data"]["items"].as_array().unwrap().is_empty()),
                "book_structure" => assert_eq!(result["data"]["available"], true),
                _ => unreachable!("{name}"),
            }
            count += 1;
        }
    }
    assert_eq!(count, 6);
    assert_eq!(f.count(), count);
    let denied = f.tool(2, "book_text", tools.as_array().unwrap().iter().find(|t| t["name"] == "book_text").unwrap()["inputSchema"]["examples"][0].clone());
    assert_eq!(denied["isError"], true);
    assert_eq!(f.count(), count);
    f.no_private_state();
}

#[test]
fn mcp_contract_discover_locate_read_and_cite_exact_pages() {
    let f = Fixture::new();
    let directory = f.tool(0, "book_list", json!({}));
    let item = &projection(&directory)["items"][0];
    let reference = &item["published_book_ref"];
    assert!(item["available_tools"]
        .as_array()
        .unwrap()
        .contains(&json!("book_search_text")));
    assert_usage(&projection(&directory)["usage"], 0);
    let search = f.tool(
        0,
        "book_search_text",
        json!({"published_book_ref":reference,"query":"needle"}),
    );
    assert_eq!(search["isError"], false);
    let hit = &projection(&search)["data"]["occurrences"][0];
    let mut result = f.tool(
        0,
        "book_text",
        json!({"published_book_ref":reference,"lid":hit["start_lid"],"end_lid":hit["end_lid"]}),
    );
    let mut combined = String::new();
    let mut previous_end = 0;
    for page in 0..2 {
        assert_eq!(result["isError"], false, "{result}");
        let value = projection(&result);
        assert_usage(&value["usage"], 1);
        assert_eq!(value["usage"]["free_remaining"], 998 - page);
        assert_eq!(
            value["usage"]["charged_from"],
            json!({"kind":"free_month","month_hk":value["usage"]["month_hk"]})
        );
        let source = &value["sources"][0];
        assert_eq!(source["published_book_ref"], *reference);
        assert_eq!(source["source_range"], value["data"]["source_range"]);
        assert_eq!(source["start_lid"], hit["start_lid"]);
        assert_eq!(source["source_range"]["start"], previous_end);
        previous_end = source["source_range"]["end"].as_u64().unwrap();
        let url = url::Url::parse(source["source_url"].as_str().unwrap()).unwrap();
        assert_eq!(url.origin().ascii_serialization(), "https://reader.example");
        let query: BTreeMap<_, _> = url.query_pairs().into_owned().collect();
        for key in ["book_id", "publication_id"] {
            assert_eq!(query[key], reference[key].as_str().unwrap());
        }
        assert_eq!(query["start_lid"], source["start_lid"].as_str().unwrap());
        assert_eq!(query["end_lid"], source["end_lid"].as_str().unwrap());
        assert_eq!(
            query["start_utf16"],
            source["source_range"]["start"].to_string()
        );
        assert_eq!(
            query["end_utf16"],
            source["source_range"]["end"].to_string()
        );
        combined.push_str(value["data"]["text"].as_str().unwrap());
        assert_eq!(value["complete"], page == 1);
        if page == 0 {
            let next = &value["next_calls"][0];
            assert_eq!(next["arguments"]["published_book_ref"], *reference);
            result = f.tool(0, next["name"].as_str().unwrap(), next["arguments"].clone());
        } else {
            assert_eq!(value["next_calls"], json!([]));
        }
    }
    assert_eq!(
        combined,
        fs::read_to_string(f._input.path().join("source.txt")).unwrap()
    );
    assert_eq!(f.count(), 3);
}

#[test]
fn mcp_contract_paid_expiry_and_exhausted_discovery_are_stable() {
    use crate::{
        admin_store::{AdminStore, Command},
        mcp_pack::{Mutation, Purchase},
    };
    let mut f = Fixture::new();
    f.control.set_reader_admin("alice", true).unwrap();
    let before = discovery(&f);
    let bought = AdminStore::new(ControlStore::open(f.control.writer.clone()).unwrap())
        .apply(
            "alice",
            "contract-pack",
            "alice",
            Command::Mcp(Mutation::Purchase(Purchase {
                pack_count: 1,
                paid_at: multi_user_host::now(),
                channel: "fixture".into(),
                external_ref: None,
                note: "contract fixture".into(),
            })),
            multi_user_host::now(),
        )
        .unwrap()
        .0;
    f.remaining(0);
    let paid = f.tool(0, "book_text", f.text_args());
    let usage = &projection(&paid)["usage"];
    assert_usage(usage, 1);
    assert_eq!(usage["paid_remaining"], 999);
    assert_eq!(
        usage["charged_from"],
        json!({"kind":"call_pack","pack_id":bought["call_pack"]["pack_id"]})
    );
    let pack = &usage["call_packs"][0];
    assert_eq!(pack["available_units"], 999);
    assert_eq!(usage["paid_next_expires_at"], pack["expires_at"]);
    for field in ["opened_at", "expires_at"] {
        let date = OffsetDateTime::parse(pack[field].as_str().unwrap(), &Rfc3339).unwrap();
        assert_eq!(date.offset().whole_hours(), 8);
    }
    f.control
        .connection
        .execute("UPDATE mcp_call_packs SET used_units=total_units", [])
        .unwrap();
    let exhausted = f.tool(0, "book_text", f.text_args());
    let failure = projection(&exhausted);
    assert_eq!(exhausted["isError"], true);
    assert_eq!(failure["error_code"], "quota_exhausted");
    assert_eq!(failure["retryable"], false);
    assert!(failure["next_step"]
        .as_str()
        .unwrap()
        .contains("Stop content requests"));
    assert_usage(&failure["usage"], 0);
    assert_eq!(failure["usage"]["free_remaining"], 0);
    assert_eq!(failure["usage"]["paid_remaining"], 0);
    assert_eq!(before, discovery(&f));
    for name in ["book_list", "mcp_usage"] {
        let free = f.tool(0, name, json!({}));
        assert_eq!(free["isError"], false);
        assert_usage(&projection(&free)["usage"], 0);
    }
    assert_eq!(f.count(), 1);
}

#[test]
fn mcp_contract_errors_give_actionable_recovery_without_evidence_or_charge() {
    let f = Fixture::new();
    for (client, name, args, code, retryable, next) in [
        (
            0,
            "book_text",
            json!({}),
            "invalid_arguments",
            false,
            "Correct arguments",
        ),
        (
            2,
            "book_text",
            f.text_args(),
            "material_unavailable",
            false,
            "book_list",
        ),
    ] {
        let result = f.tool(client, name, args);
        assert_eq!(result["isError"], true);
        assert_failure(projection(&result), code, retryable, next);
    }
    f.control.connection.execute_batch("CREATE TRIGGER reject_contract_debit BEFORE INSERT ON mcp_usage BEGIN SELECT RAISE(ABORT,'fixture'); END;").unwrap();
    let failed = f.tool(0, "book_text", f.text_args());
    assert_failure(
        projection(&failed),
        "service_unavailable",
        true,
        "Retry later",
    );
    let unauthenticated = post(&f.url(), "invalid", init());
    assert_eq!(unauthenticated.status(), 401);
    assert_failure(
        &unauthenticated.into_json::<Value>().unwrap(),
        "unauthenticated",
        false,
        "credential",
    );
    // Rate-limit mapping is deterministic; avoid manufacturing concurrent library pressure.
    let busy = failure(&error("LIBRARY_BUSY", "rate_limit", "fixture"));
    assert_failure(&busy, "service_busy", true, "Wait 60 seconds");
    assert_eq!(busy["retry_after_seconds"], 60);
    assert_eq!(f.count(), 0);
    assert_eq!(
        f.access.mcp_usage.status("alice").unwrap().free_remaining,
        1000
    );
}

fn assert_failure(value: &Value, code: &str, retryable: bool, next: &str) {
    assert_eq!(value["error_code"], code);
    assert!(value["cause_code"].is_string());
    assert!(value["message"].as_str().is_some_and(|s| !s.is_empty()));
    assert_eq!(value["retryable"], retryable);
    assert!(value["next_step"].as_str().unwrap().contains(next));
    assert_eq!(value["charged_units"], 0);
    assert!(value.get("data").is_none());
    assert!(value.get("sources").is_none());
}
