use super::*;
use serde_json::{json, Value};
include!("../../read-tools/testdata/math_book.rs");

fn read(book: &Book, args: Value) -> (String, String) {
    // Deterministic Book reads must never call a model.
    struct NoModel;
    impl ModelAdapter for NoModel {
        fn complete(&self, _: CompletionRequest) -> Result<ParsedResponse, AdapterError> {
            panic!("mathematical source reads must not call a model")
        }
        fn chat(&self, _: &AgentRequestPlan) -> Result<AssistantTurn, AdapterError> {
            panic!("mathematical source reads must not call a model")
        }
    }
    let (raw, effect) = dispatch_resident_book_tool(BookToolId::Text, args.clone(), book, &NoModel);
    assert!(effect.is_none());
    assert!(serde_json::from_str::<Value>(&raw)
        .unwrap()
        .get("error_code")
        .is_none());
    (args.to_string(), raw)
}

#[test]
fn math_structure_read_locators_feed_teaching_review_without_observing_unread_targets() {
    let book = optimization_math_book();
    let (arguments, raw) = read(&book, json!({"lid":"1.8","end_lid":"1.9"}));
    let body: Value = serde_json::from_str(&raw).unwrap();
    let next = body["math_context"][0]["next_calls"][0]["arguments"].to_string();
    let mut locators = TurnLocatorLedger::default();
    assert!(authorize_book_text(&next, &locators).is_err());
    locators.observe_tool_result("book.text", &raw, &book);
    assert!(authorize_book_text(&next, &locators).is_ok());
    let mut evidence = TurnEvidenceLedger::default();
    observe_tool_evidence(&mut evidence, "book.text", &arguments, &raw, &book);
    let exercise = evidence.teaching_review_sources(&book).unwrap();
    assert_eq!(
        exercise[0]["math_context"][0]["references"][1]["relation"],
        "asks_to_prove"
    );
    assert!(!exercise.to_string().contains("在假设6.1的条件下"));
    let (arguments, raw) = read(&book, serde_json::from_str(&next).unwrap());
    observe_tool_evidence(&mut evidence, "book.text", &arguments, &raw, &book);
    let with_premise = evidence.teaching_review_sources(&book).unwrap();
    assert!(with_premise.to_string().contains("在假设6.1的条件下"));
    let theorem = with_premise[0]["math_context"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["label"] == "定理 6.5")
        .unwrap();
    assert_eq!(theorem["references"][0]["resolution"], "missing");
    assert!(theorem["references"][0].get("target_lid").is_none());
    // Batch reads retain the same legal follow-up locations.
    let batch =
        crate::book_text_batch::read(r#"{"ranges":[{"lid":"1.8","end_lid":"1.9"}]}"#, &book);
    let mut batch_locators = TurnLocatorLedger::default();
    batch_locators.observe_tool_result("book.text_batch", &batch, &book);
    assert!(authorize_book_text(&next, &batch_locators).is_ok());
}

#[test]
fn math_structure_partial_delivery_and_continuation_preserve_actual_read_conditions() {
    let book = optimization_math_book();
    let (arguments, raw) = read(&book, json!({"lid":"1.1","end_lid":"1.3"}));
    let registry = resident_tool_registry();
    let policy = registry.registration("book.text").unwrap().output_policy;
    let draft =
        crate::tool_result::project_tool_result("book.text", &arguments, &raw, policy, 220, &book);
    assert!(draft.truncated);
    assert!(
        !draft.model_body.to_string().contains("局部极小点"),
        "{}",
        draft.model_body
    );
    let mut evidence = TurnEvidenceLedger::default();
    observe_tool_evidence(
        &mut evidence,
        "book.text",
        &arguments,
        &draft.model_body_json(),
        &book,
    );
    assert!(!evidence
        .teaching_review_sources(&book)
        .unwrap()
        .to_string()
        .contains("局部极小点"));
    let next = draft.model_body["next_start_utf16"].as_u64().unwrap();
    let (arguments, raw) = read(
        &book,
        json!({"lid":"1.1","end_lid":"1.3","start_utf16":next}),
    );
    observe_tool_evidence(&mut evidence, "book.text", &arguments, &raw, &book);
    let review = evidence.teaching_review_sources(&book).unwrap();
    assert_eq!(
        review[0]["math_context"][0]["assumptions"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        review[0]["math_context"][0]["missing_evidence_lids"],
        json!([])
    );
    assert!(review.to_string().contains("局部极小点"));
    assert!(review.to_string().contains("nabla f(x^{*})=0."));
}

#[test]
fn math_structure_reference_review_keeps_followup_calls_in_the_original_publication() {
    let book = optimization_math_book();
    let (arguments, raw) = read(&book, json!({"lid":"1.8","end_lid":"1.9"}));
    let mut evidence = TurnEvidenceLedger::default();
    observe_tool_evidence(&mut evidence, "book.text", &arguments, &raw, &book);
    let reference = base_schema::PublishedBookRef {
        book_id: book.base.book_id.clone(),
        publication_id: "edition-1".into(),
    };
    let mut sources = Vec::new();
    evidence
        .append_teaching_review_source(&book, Some(&reference), &mut sources)
        .unwrap();
    let next = &sources[0]["math_context"][0]["next_calls"][0];
    assert_eq!(next["name"], "library.read");
    assert_eq!(next["arguments"]["operation"], "text");
    assert_eq!(next["arguments"]["published_book_ref"], json!(reference));
    assert!(next.get("tool").is_none());
}

#[test]
fn math_structure_library_budget_keeps_source_prefix_and_scoped_followups() {
    let book = optimization_math_book();
    let (arguments, raw) = read(&book, json!({"lid":"1"}));
    let reference = base_schema::PublishedBookRef {
        book_id: book.base.book_id.clone(),
        publication_id: "edition-1".into(),
    };
    let mut args: Value = serde_json::from_str(&arguments).unwrap();
    args["operation"] = json!("text");
    args["published_book_ref"] = json!(reference);
    let mut data: Value = serde_json::from_str(&raw).unwrap();
    crate::library::scope_math_read_calls(&mut data, &reference);
    let raw = json!({"operation":"text","published_book_ref":reference,"data":data,"sources":[],"next_calls":[],"complete":true}).to_string();
    let registry = resident_tool_registry();
    let policy = registry.registration("library.read").unwrap().output_policy;
    let source = book.text("1", None).unwrap();
    let mut reads = 0;
    for budget in [700, 1400, raw.len() - 1, raw.len()] {
        let draft = crate::tool_result::project_tool_result(
            "library.read",
            &args.to_string(),
            &raw,
            policy,
            budget,
            &book,
        );
        assert!(draft.model_body_json().len() <= budget);
        let body = &draft.model_body;
        let Some(text) = body["data"]["text"].as_str() else {
            continue;
        };
        reads += 1;
        assert!(!text.is_empty() && source.starts_with(text));
        assert_eq!(body["published_book_ref"], json!(reference));
        for item in body["data"]["math_context"]
            .as_array()
            .into_iter()
            .flatten()
        {
            for call in item["next_calls"].as_array().unwrap() {
                assert_eq!(call["name"], "library.read");
                assert_eq!(call["arguments"]["published_book_ref"], json!(reference));
            }
            for quote in item["assumptions"].as_array().unwrap() {
                assert!(text.contains(quote["quote"].as_str().unwrap()));
            }
        }
    }
    assert!(
        reads >= 3,
        "optional annotations must not displace readable source"
    );
}
