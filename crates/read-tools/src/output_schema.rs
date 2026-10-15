//! Complete read result contracts. Delivery projections are owned by their surfaces.
use crate::*;
use book_tool_contracts::BookToolId;
use serde_json::{json, Value};

/// Output objects are closed; typed maps retain their generated value schema.
/// This does not change input parsing or serialization of the result types.
pub fn schema_for<T: schemars::JsonSchema>() -> Value {
    let mut schema = book_tool_contracts::schema_value::<T>();
    close_objects(&mut schema);
    schema
}

fn close_objects(schema: &mut Value) {
    if schema.get("properties").is_some() && schema.get("additionalProperties").is_none() {
        schema["additionalProperties"] = json!(false);
    }
    if let Some(properties) = schema.get_mut("properties").and_then(Value::as_object_mut) {
        for property in properties.values_mut() {
            close_objects(property);
        }
    }
    for key in ["items", "additionalProperties"] {
        if let Some(child) = schema.get_mut(key) {
            close_objects(child);
        }
    }
    for key in ["oneOf", "anyOf", "allOf"] {
        if let Some(children) = schema.get_mut(key).and_then(Value::as_array_mut) {
            for child in children {
                close_objects(child);
            }
        }
    }
}

pub fn text_schema() -> Value {
    let mut page = schema_for::<TextPage>();
    page["properties"]["lid"] = json!({"type":"string"});
    page["required"].as_array_mut().unwrap().push(json!("lid"));
    json!({"type":"object","oneOf":[
        {"type":"object","properties":{"lid":{"type":"string"},"text":{"type":"string"}},
         "required":["lid","text"],"additionalProperties":false}, page
    ]})
}

/// None means the result is owned by Runtime or by the visitor session surface.
pub fn book_output_schema(id: BookToolId) -> Option<Value> {
    Some(match id {
        BookToolId::Manifest => schema_for::<Manifest>(),
        BookToolId::Text => text_schema(),
        BookToolId::SearchText => schema_for::<SearchTextResult>(),
        BookToolId::Context => schema_for::<Context>(),
        BookToolId::Concept => schema_for::<ConceptCandidateSet>(),
        BookToolId::Structure => schema_for::<StructureProjection>(),
        BookToolId::GuidePath => schema_for::<GuidePath>(),
        BookToolId::PaperMetadata => schema_for::<PaperMetadataProjection>(),
        BookToolId::PaperLexicon => schema_for::<PaperLexiconProjection>(),
        BookToolId::PaperReadingGuide => schema_for::<PaperReadingGuide>(),
        BookToolId::Query | BookToolId::Synthesize | BookToolId::Guide => return None,
    })
}
