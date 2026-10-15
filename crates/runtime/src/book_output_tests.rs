use super::*;
use crate as runtime;
use serde_json::{json, Value};
include!("testdata/book_example_adapter.rs");

#[test]
fn to12_resident_results_export_real_dispatch_and_budget_envelopes() {
    let book = Book::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../book-tool-contracts/fixtures/example-book"
    ))
    .unwrap();
    let registry = resident_tool_registry();
    let adapter = ExampleAdapter::default();
    let mut schemas = serde_json::Map::new();
    let mut samples = Vec::new();
    for contract in book_tool_contracts::contracts() {
        let Some(name) = contract.aliases.resident else {
            continue;
        };
        let registration = registry.registration(name).unwrap();
        schemas.insert(name.into(),json!({"canonical":crate::book_output_schema::canonical(contract.id).unwrap(),
            "outputSchema":registration.output_schema().unwrap(),"annotations":registration.annotations()}));
        assert_eq!(registration.annotations()["readOnlyHint"], true);
        let mut inputs = book_tool_contracts::input_examples(contract.id);
        if inputs.is_empty() {
            inputs.push(json!({}));
        }
        if contract.id == BookToolId::SearchText {
            inputs.push(json!({"query":"missing-text"}));
        }
        if contract.id == BookToolId::Context {
            inputs.push(json!({"lid":"1","k":0}));
        }
        if contract.id == BookToolId::Concept {
            inputs.push(json!({"query":"missing-concept"}));
        }
        for input in inputs {
            let (raw, _) = dispatch_resident_book_tool(contract.id, input.clone(), &book, &adapter);
            let raw_value: Value = serde_json::from_str(&raw).unwrap();
            let is_error = raw_value.get("error_code").is_some();
            assert_eq!(is_error, input["query"] == "missing-concept", "{name}: {raw}");
            for limit in [100_000, 700, 100, 4] {
                let mut policy = registration.output_policy;
                policy.max_model_body_bytes = limit;
                let draft = crate::tool_result::project_tool_result(
                    name,
                    &input.to_string(),
                    &raw,
                    policy,
                    limit,
                    &book,
                );
                let receipt = tool_receipt(
                    name,
                    &input.to_string(),
                    &draft.model_body_json(),
                    &raw,
                    &book,
                );
                let envelope = draft.into_envelope(receipt);
                if name == "book.text" && limit == 100_000 {
                    let mut ledger = crate::tool_result::ActiveToolResultLedger::default();
                    ledger.insert("to12-evicted", envelope.clone());
                    ledger.mark_projected_fresh_results_sampled();
                    ledger.make_room_for(
                        crate::tool_result::ACTIVE_TURN_TOOL_MODEL_BODY_BUDGET_BYTES,
                    );
                    let mut messages = vec![Message {
                        role: Role::Tool,
                        content: None,
                        tool_calls: vec![],
                        tool_call_id: Some("to12-evicted".into()),
                        provider_continuation: None,
                    }];
                    ledger.project_messages(&mut messages);
                    let evicted: Value =
                        serde_json::from_str(messages[0].content.as_deref().unwrap()).unwrap();
                    assert!(evicted["model_body"].is_null());
                    assert_eq!(evicted["status"], "ok");
                    samples.push(json!({"tool":name,"evicted":true,"envelope":evicted}));
                }
                samples.push(json!({"tool":name,"input":input,"raw":raw_value,"error":is_error,"limit":limit,"envelope":envelope}));
            }
        }
    }
    let input = json!({"lid":"missing"});
    let (raw, _) = dispatch_resident_book_tool(BookToolId::Text, input.clone(), &book, &adapter);
    for limit in [100_000, 100, 4] {
        let mut policy = registry.registration("book.text").unwrap().output_policy;
        policy.max_model_body_bytes = limit;
        let draft = crate::tool_result::project_tool_result(
            "book.text",
            &input.to_string(),
            &raw,
            policy,
            limit,
            &book,
        );
        let receipt = tool_receipt(
            "book.text",
            &input.to_string(),
            &draft.model_body_json(),
            &raw,
            &book,
        );
        samples
            .push(json!({"tool":"book.text","error":true,"envelope":draft.into_envelope(receipt)}));
    }
    assert!(samples.iter().any(|s| s["envelope"]["status"] == "partial"));
    assert!(samples
        .iter()
        .any(|s| s["envelope"]["model_body"].is_null()));
    if let Ok(dir) = std::env::var("TO12_DIR") {
        std::fs::write(
            std::path::Path::new(&dir).join("resident.json"),
            serde_json::to_vec_pretty(&json!({"tools":schemas,"samples":samples})).unwrap(),
        )
        .unwrap();
    }
}
