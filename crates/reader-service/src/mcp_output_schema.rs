//! MCP-specific result composition; canonical content remains owned by its producer.
use book_tool_contracts::{contract_for, BookToolId};
use read_tools::output_schema::schema_for;
use serde_json::{json, Value};

pub fn reference_schema() -> Value {
    schema_for::<crate::published_library::PublishedBookRef>()
}

pub fn candidate_calls(id: BookToolId, schema: &mut Value, remote: bool) {
    if id == BookToolId::Text && schema.get("oneOf").is_some() {
        for branch in schema["oneOf"].as_array_mut().unwrap() {
            candidate_calls(id, branch, remote);
        }
        return;
    }
    let collection = match id {
        BookToolId::Text => "math_context",
        BookToolId::SearchText => "occurrences",
        BookToolId::Search => "hits",
        BookToolId::Concept => "candidates",
        BookToolId::Context => "items",
        _ => return,
    };
    let call = &mut schema["properties"][collection]["items"]["properties"]["next_calls"]["items"];
    call["properties"].as_object_mut().unwrap().remove("tool");
    call["properties"]["name"] = json!({"const":"book_text"});
    call["required"] = json!(["name", "arguments"]);
    if remote {
        call["properties"]["arguments"]["properties"]["published_book_ref"] = reference_schema();
        call["properties"]["arguments"]["required"]
            .as_array_mut()
            .unwrap()
            .push(json!("published_book_ref"));
    }
}

pub fn local(id: BookToolId) -> Value {
    let mut schema = runtime::book_output_schema::canonical(id).expect("Book result owner");
    if id == BookToolId::Guide {
        schema = json!({"type":"object","oneOf":[
            {"type":"object","additionalProperties":false,"required":["session_id","opened","guide"],
                "properties":{"session_id":{"type":"string"},"opened":{"type":"boolean"},"guide":schema}},
            {"type":"object","additionalProperties":false,"required":["ok","closed","session_id"],
                "properties":{"ok":{"const":true},"closed":{"const":true},"session_id":{"type":"string"}}}
        ]});
    }
    candidate_calls(id, &mut schema, false);
    // Tagged domain unions still advertise MCP's object root.
    schema["type"] = json!("object");
    schema
}

pub fn local_annotations(id: BookToolId) -> Value {
    if id == BookToolId::Guide {
        return json!({"readOnlyHint":false,"destructiveHint":true,"idempotentHint":false,"openWorldHint":true});
    }
    if contract_for(id).aliases.resident.is_some() {
        return runtime::tool_registry::book_annotations(id);
    }
    free_read_annotations()
}

pub fn free_read_annotations() -> Value {
    json!({"readOnlyHint":true,"destructiveHint":false,"idempotentHint":true,"openWorldHint":false})
}

