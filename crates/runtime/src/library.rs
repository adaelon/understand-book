//! Resident cross-book tools. Public material reads and private learning share the Run owner.
use crate::ToolSpec;
use base_schema::PublishedBookRef;
use book_tool_contracts::{input_schema, BookToolId};
use read_tools::{Book, ToolError};
use serde_json::{json, Value};
use std::sync::Arc;

pub const READ_OPERATIONS: [(&str, BookToolId); 5] = [
    ("text", BookToolId::Text), ("search_text", BookToolId::SearchText),
    ("concept", BookToolId::Concept), ("structure", BookToolId::Structure),
    ("context", BookToolId::Context),
];

pub struct LibrarySource {
    pub reference: PublishedBookRef,
    pub book: Arc<Book>,
    pub title: String,
}

pub struct LibraryReadResult {
    pub body: Value,
    pub source: Option<LibrarySource>,
}

pub fn unavailable() -> ToolError {
    ToolError { error_code: "LIBRARY_UNAVAILABLE".into(), category: "not_found".into(),
        message: "This reading session has no authorized published library".into() }
}

pub fn reference_schema() -> Value {
    json!({"type":"object","properties":{
        "book_id":{"type":"string","minLength":1},"publication_id":{"type":"string","minLength":1}},
        "required":["book_id","publication_id"],"additionalProperties":false})
}

pub fn list_spec() -> ToolSpec {
    ToolSpec { name: "library.list".into(),
        description: "Find materials in this run's learning workspace that are currently authorized for this reader when a specific knowledge gap needs supplementary reading. Returns names, exact published_book_ref values and available operations. Copy the reference into library.read. The current book and Reader position stay unchanged. Directory entries are not evidence; follow next_calls for further pages.".into(),
        parameters: json!({"type":"object","properties":{"after":reference_schema(),
            "limit":{"type":"integer","minimum":1,"maximum":50}},"additionalProperties":false,
            "examples":[{}]}) }
}

pub fn read_spec() -> ToolSpec {
    let variants: Vec<Value> = READ_OPERATIONS.iter().map(|(operation, id)| {
        let mut schema = input_schema(*id);
        schema.as_object_mut().unwrap().remove("examples");
        schema["properties"]["operation"] = json!({"type":"string","enum":[operation]});
        schema["properties"]["published_book_ref"] = reference_schema();
        if schema.get("required").is_none() { schema["required"] = json!([]); }
        schema["required"].as_array_mut().unwrap().extend([json!("operation"),json!("published_book_ref")]);
        if *id != BookToolId::Text {
            schema["properties"]["page"] = json!({"type":"object","properties":{
                "path":{"type":"string"},"offset":{"type":"integer","minimum":0},
                "limit":{"type":"integer","minimum":1,"maximum":50}},"required":["path"],"additionalProperties":false});
        }
        schema
    }).collect();
    ToolSpec { name: "library.read".into(),
        description: "Read a selected material from this run's frozen workspace members while current account authorization remains valid, without switching books. Choose structure/concept/search_text/context to locate relevant passages, then text to read evidence. Use the same canonical Book parameters with operation and published_book_ref. Only actually returned source text supports an explanation. Follow returned next_calls unchanged. Consult this reader's understanding of the selected material; do not treat another book's LIDs as current-book navigation targets.".into(),
        parameters: json!({"type":"object","oneOf":variants,"examples":[
            {"operation":"structure","published_book_ref":{"book_id":"material","publication_id":"version"}},
            {"operation":"text","published_book_ref":{"book_id":"material","publication_id":"version"},"lid":"1.1"}]}) }
}

pub fn learning_spec() -> ToolSpec {
    let text = json!({"type":"string","minLength":1});
    ToolSpec { name:"library.learning".into(), description:"Read or update this reader's private understanding of a reference material within this run's learning workspace. Understanding and superseding evidence stay in this workspace. Read before supplementing an explanation. observe records an unassessed hypothesis from an exact quote of the CURRENT user's words and source_ref_ids registered this turn for that publication. Use supersedes to revise an earlier hypothesis. Assistant explanations, UI action labels and mere reading are not evidence of mastery. Never assign a score. Actual delivered help is recorded automatically after commit.".into(),
        parameters:json!({"type":"object","oneOf":[
            {"type":"object","properties":{"operation":{"type":"string","enum":["read"]},"published_book_ref":reference_schema(),
                "after":{"type":"integer","minimum":0}},"required":["operation","published_book_ref"],"additionalProperties":false},
            {"type":"object","properties":{"operation":{"type":"string","enum":["observe"]},"published_book_ref":reference_schema(),
                "operation_id":text,"source_ref_ids":{"type":"array","items":text,"minItems":1,"uniqueItems":true},
                "learner_quote":text,"learning_focus":text,"expected_performance":text,"capability":text,
                "interpretation":text,"teaching_implication":text,"supersedes":text},
                "required":["operation","published_book_ref","operation_id","source_ref_ids","learner_quote","learning_focus","expected_performance","capability","interpretation","teaching_implication"],"additionalProperties":false}
        ],"examples":[{"operation":"read","published_book_ref":{"book_id":"material","publication_id":"version"}}]}) }
}
