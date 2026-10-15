//! MCP-specific result composition; canonical content remains owned by its producer.
use book_tool_contracts::{contract_for, BookToolId};
use read_tools::output_schema::schema_for;
use serde_json::{json, Value};

pub(crate) fn reference_schema() -> Value {
    schema_for::<crate::published_library::PublishedBookRef>()
}

fn candidate_calls(id: BookToolId, schema: &mut Value, remote: bool) {
    let collection = match id {
        BookToolId::SearchText => "occurrences",
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

pub(crate) fn local(id: BookToolId) -> Value {
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

pub(crate) fn local_annotations(id: BookToolId) -> Value {
    if id == BookToolId::Guide {
        return json!({"readOnlyHint":false,"destructiveHint":true,"idempotentHint":false,"openWorldHint":true});
    }
    if contract_for(id).aliases.resident.is_some() {
        return runtime::tool_registry::book_annotations(id);
    }
    free_read_annotations()
}

pub(crate) fn free_read_annotations() -> Value {
    json!({"readOnlyHint":true,"destructiveHint":false,"idempotentHint":true,"openWorldHint":false})
}

pub(crate) fn paid_read_annotations() -> Value {
    // Immutable content, but every successful call appends consumption and debits credits.
    json!({"readOnlyHint":false,"destructiveHint":false,"idempotentHint":false,"openWorldHint":false})
}

pub(crate) fn usage_schema() -> Value {
    let mut schema = schema_for::<crate::mcp_usage::Usage>();
    let fields = json!({
        "charged_units":{"type":"integer","minimum":0,"maximum":1},
        "charged_from":{"anyOf":[schema_for::<crate::mcp_usage::DebitSource>(),{"type":"null"}]},
        "top_up_url":{"type":["string","null"]},"top_up_instruction":{"type":"string"}
    });
    for (key, field) in fields.as_object().unwrap() {
        schema["properties"][key] = field.clone();
        schema["required"].as_array_mut().unwrap().push(json!(key));
    }
    schema
}

pub(crate) fn usage_result() -> Value {
    json!({"type":"object","additionalProperties":false,"required":["usage"],"properties":{"usage":usage_schema()}})
}

fn next_calls(name: &str, input: &Value) -> Value {
    json!({"type":"array","items":{"type":"object","additionalProperties":false,"required":["name","arguments"],
        "properties":{"name":{"const":name},"arguments":input}}})
}

pub(crate) fn directory(input: &Value) -> Value {
    json!({"type":"object","additionalProperties":false,"required":["items","complete","next_calls","usage"],"properties":{
        "items":{"type":"array","items":{"type":"object","additionalProperties":false,
            "required":["published_book_ref","name","is_default","available_tools"],"properties":{
            "published_book_ref":reference_schema(),"name":{"type":"string"},"is_default":{"type":"boolean"},
            "available_tools":{"type":"array","items":{"type":"string"}}}}},
        "complete":{"type":"boolean"},"next_calls":next_calls("book_list",input),"usage":usage_schema()
    }})
}

/// Every array that McpRead::bounded can page, with its original JSON Pointer.
/// Nullable object branches are traversed; arrays of arrays and nested item
/// arrays retain their own item schema instead of becoming untyped JSON.
fn collection_schemas(schema: &Value, path: &str, out: &mut Vec<Value>) {
    if schema["type"] == "array" {
        out.push(
            json!({"properties":{"data_path":{"type":"string","pattern":format!("^{path}$")},
            "data":schema,"total_items":{"type":"integer","minimum":0}}}),
        );
        collection_schemas(&schema["items"], &format!("{path}/[0-9]+"), out);
    }
    if let Some(properties) = schema["properties"].as_object() {
        for (key, child) in properties {
            let key = key.replace('~', "~0").replace('/', "~1");
            collection_schemas(child, &format!("{path}/{key}"), out);
        }
    }
    for kind in ["oneOf", "anyOf", "allOf"] {
        for child in schema[kind].as_array().into_iter().flatten() {
            collection_schemas(child, path, out);
        }
    }
}

pub(crate) fn remote(id: BookToolId, input: &Value) -> Value {
    let mut data = if id == BookToolId::Text {
        schema_for::<read_tools::TextPage>()
    } else {
        read_tools::output_schema::book_output_schema(id).expect("remote read result owner")
    };
    candidate_calls(id, &mut data, true);
    let source = json!({"type":"object","additionalProperties":false,
    "required":["published_book_ref","start_lid","end_lid","source_range","source_url"],"properties":{
        "published_book_ref":reference_schema(),"start_lid":{"type":"string"},"end_lid":{"type":"string"},
        "source_range":schema_for::<base_schema::Span>(),"source_url":{"type":"string"}
    }});
    let mut schema = json!({"type":"object","additionalProperties":false,
    "required":["published_book_ref","data","sources","complete","next_calls","usage"],"properties":{
        "published_book_ref":reference_schema(),"data":data,"sources":{"type":"array","items":source},
        "complete":{"type":"boolean"},"next_calls":next_calls(contract_for(id).aliases.mcp.unwrap(),input),"usage":usage_schema()
    }});
    if id != BookToolId::Text {
        let mut branches = vec![
            json!({"properties":{"data_path":{"const":""},"data":data,"total_items":{"type":"null"}}}),
        ];
        collection_schemas(&data, "", &mut branches);
        schema["properties"]["data"] = json!({"type":["object","array"]});
        schema["properties"]["data_path"] = json!({"type":"string"});
        schema["properties"]["total_items"] = json!({"type":["integer","null"],"minimum":0});
        schema["required"]
            .as_array_mut()
            .unwrap()
            .extend([json!("data_path"), json!("total_items")]);
        schema["anyOf"] = json!(branches);
    }
    schema["description"] = json!("Successful remote delivery only. complete describes remaining transport/native pages; candidate counts describe selection before transport. data_path identifies the returned collection. Each successful call, including repeats and empty pages, consumes one unit. isError diagnostics remain outside this success schema.");
    schema
}
