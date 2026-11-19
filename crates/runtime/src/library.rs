//! Resident cross-book tools. Public material reads and private learning share the Run owner.
use crate::ToolSpec;
use base_schema::PublishedBookRef;
use book_tool_contracts::{input_schema, BookToolId};
use read_tools::{Book, ToolError};
use serde_json::{json, Value};
use std::sync::Arc;

pub(crate) const WORKSPACE_CATALOG_POLICY: &str = "Workspace materials:
- The following directory is the first page of this run's frozen workspace materials, filtered by this reader's current authorization. It is metadata, not observed source evidence. Names, publication identities and video positions come from the directory; never invent a missing material or source position. Follow library.list next_calls only when the needed material is not on this page.
- book.* reads only the currently open material. For related content across this workspace use library.search; it reports whether semantic retrieval was available and returns publication-bound text reads. To use another textbook, handout or video, copy its exact published_book_ref into library.read and library.learning. Use structure, search or search_text to locate relevant passages, then text to read them. Bounded retrieval is not an exhaustive overview. Preserve the original task and Reader position. A failed search in the current book does not establish that the whole workspace lacks the answer.
- For a video, its saved transcript is readable through library.read; use video.search with the same published_book_ref for visual or teaching-role locators. Read only the relevant source. When actual frames or sound are necessary, discover source_read with scope=passage and operation=read_source for video.read_frames or video.review_audio, then follow the returned locations. Transcript text alone does not establish unseen visuals or unheard audio.
- For published music, use music.read with its published_book_ref for notation, events, part roles and provenance; pure music has no spoken transcript. Only music.interact applied browser receipts establish player actions, and prepared analysis is not a main-model audio listen.
- To show a textbook passage or video transcript location, call source.present with the exact observed quote and the same published_book_ref; Runtime retains that publication and its actual source location. For observed frames or audio use their returned source targets. If a required tool is absent from this sampling, use tool.search; discover corpus source_read for library reads and corpus memory_read for the learner's existing understanding. Directory entries and search hits alone cannot support a source citation.";

pub const READ_OPERATIONS: [(&str, BookToolId); 6] = [
    ("search", BookToolId::Search),
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

/// Mathematical follow-up reads keep the source publication in review and budget projections.
pub(crate) fn scope_math_read_calls(body: &mut Value, reference: &PublishedBookRef) {
    for item in body.get_mut("math_context").and_then(Value::as_array_mut).into_iter().flatten() {
        for call in item["next_calls"].as_array_mut().into_iter().flatten() {
            call.as_object_mut().unwrap().remove("tool");
            call["name"] = json!("library.read");
            call["arguments"]["operation"] = json!("text");
            call["arguments"]["published_book_ref"] = json!(reference);
        }
    }
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

pub fn search_spec() -> ToolSpec {
    ToolSpec { name: "library.search".into(),
        description: "Locate relevant passages across this run's frozen learning-workspace materials, filtered by current account authorization. Uses lexical relevance and, when configured, semantic retrieval; the result reports the actual retrieval status. published_book_refs may only narrow the workspace. Results are bounded candidates, never exhaustive coverage or observed source evidence. Follow each hit's library.read text call before explaining or citing. For an explicit selection read it directly; for one ordinary article prefer book.search without embeddings.".into(),
        parameters: json!({"type":"object","properties":{
            "query":{"type":"string","minLength":1,"maxLength":4096},
            "limit":{"type":"integer","minimum":1,"maximum":20},
            "published_book_refs":{"type":"array","items":reference_schema(),"minItems":1,"uniqueItems":true}},
            "required":["query"],"additionalProperties":false,
            "examples":[{"query":"why does the model fail on unseen examples","limit":6}]}) }
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
