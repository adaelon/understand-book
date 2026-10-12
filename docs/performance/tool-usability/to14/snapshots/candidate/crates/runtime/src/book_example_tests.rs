use super::*;
use crate as runtime;
use serde_json::{json, Value};
include!("testdata/book_example_adapter.rs");

fn example_book() -> Book {
    Book::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../book-tool-contracts/fixtures/example-book")).unwrap()
}

#[test]
fn advertised_book_examples_dispatch_and_bind_only_observed_source() {
    let book = example_book();
    let adapter = ExampleAdapter::default();
    let mut ledger = TurnEvidenceLedger::default();
    let specs = tool_specs();
    let source = specs.iter().find(|s| s.name == "source.present").unwrap().parameters["examples"][0].clone();
    assert_eq!(ledger.present(&book, &source.to_string()).unwrap_err().error_code, "SOURCE_NOT_OBSERVED");
    let mut count = 0;
    for spec in &specs {
        let Some(id) = book_tool_contracts::from_resident_alias(&spec.name) else { continue };
        let Some(examples) = spec.parameters["examples"].as_array() else { continue };
        for input in examples {
            let (body, effect) = dispatch_resident_book_tool(id, input.clone(), &book, &adapter);
            assert!(effect.is_none());
            let result: Value = serde_json::from_str(&body).unwrap();
            assert!(result.get("error_code").is_none(), "{}: {result}", spec.name);
            match id {
                BookToolId::Text => {
                    assert_eq!(result["text"], book.text("1.1", Some("1.2")).unwrap());
                    observe_tool_evidence(&mut ledger, &spec.name, &input.to_string(), &body, &book);
                }
                BookToolId::Concept => {
                    assert_eq!(result["returned_count"], 2);
                    assert_eq!(result["truncated"], false);
                }
                BookToolId::Query | BookToolId::Synthesize => {
                    let lids: Vec<_> = result["citations"].as_array().unwrap().iter().map(|c| c["lid"].as_str().unwrap()).collect();
                    assert_eq!(lids, vec!["1.1", "1.2"], "{result}");
                    if id == BookToolId::Synthesize {
                        assert_eq!(result["source_lids"], json!(["1.1","1.2"]));
                    }
                }
                BookToolId::SearchText => {
                    assert_eq!(result["occurrences"][0]["start_lid"], if input["order"] == "document" { "1.1" } else { "1.2" });
                    if input["order"] == "document" {
                        let mut continuation = input.clone();
                        continuation["cursor"] = result["next_cursor"].clone();
                        let (next, _) = dispatch_resident_book_tool(id, continuation, &book, &adapter);
                        let next: Value = serde_json::from_str(&next).unwrap();
                        assert_eq!(next["occurrences"][0]["start_lid"], "1.2");
                    }
                }
                BookToolId::Context => assert!(result["items"].as_array().unwrap().len() > 0),
                BookToolId::Structure => assert_eq!(result["available"], true, "{result}"),
                _ => unreachable!(),
            }
            count += 1;
        }
    }
    assert_eq!(count, 8);
    ledger.present(&book, &source.to_string()).unwrap();
    let binding = &ledger.bindings()[0];
    assert_eq!(binding.evidence_range.start_lid, "1.1");
    assert_eq!(book.resolve_source(&binding.evidence_range, SOURCE_PRESENTATION_LOCALE, None).unwrap().highlighted_quote, source["quote"]);
    let text_input = &specs.iter().find(|s| s.name == "book.text").unwrap().parameters["examples"][0];
    let fake = FakeAdapter::new(vec![
        turn_calls(vec![call("read-example", "book.text", &text_input.to_string())]),
        turn_calls(vec![call("present-example", "source.present", &source.to_string())]),
        turn_final(&format!("The first condition excludes equality.[[source:{}]]", binding.source_ref_id)),
    ], vec![]);
    let mut store = MemoryStore::open(tmp("to2-source-example")).unwrap();
    let mut reader = Reader::new(&book, DEFAULT_RADIUS);
    let mut messages = new_session();
    let out = run(&book, &mut store, &mut reader, &fake, &mut messages,
        "Read 1.1 through 1.2 and cite the first condition.", "t0", OuterConfig::default()).unwrap();
    assert_eq!(out.source_bindings.len(), 1);
    assert_eq!(out.source_bindings[0].evidence_range, binding.evidence_range);
    assert!(!out.incomplete, "{:?}", out.warning);
    let requests = adapter.requests.lock().unwrap();
    assert!(requests.iter().any(|r| r.user.contains("greater than 10") && r.user.contains("at least 10")));
}
