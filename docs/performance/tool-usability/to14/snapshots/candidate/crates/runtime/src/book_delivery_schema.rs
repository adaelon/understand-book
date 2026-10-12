//! The model receives an envelope, not the complete domain result.
use super::*;
use read_tools::output_schema::schema_for;

fn delivery_schema(search: bool) -> Value {
    if search {
        return json!({"type":"object","additionalProperties":false,
            "properties":{"returned_count":{"type":"integer","minimum":0},"section_counts_omitted":{"type":"boolean"}},
            "required":["returned_count","section_counts_omitted"]});
    }
    json!({"type":"object","additionalProperties":false,"required":["status"],"properties":{
        "status":{"const":"partial"},
        "collections":{"type":"object","additionalProperties":{"type":"object","additionalProperties":false,
            "properties":{"total":{"type":"integer","minimum":0},"delivered":{"type":"integer","minimum":0}},"required":["total","delivered"]}},
        "read_lids":{"type":"array","items":{"type":"string"}},
        "omitted_fields":{"type":"array","items":{"type":"string"}},
        "recovery":{"type":"string"}
    }})
}

fn add_delivery(schema: &mut Value, delivery: &Value) {
    if schema.get("properties").is_some() {
        schema["properties"]["delivery"] = delivery.clone();
        schema["required"]
            .as_array_mut()
            .unwrap()
            .push(json!("delivery"));
    } else {
        for key in ["oneOf", "anyOf"] {
            for branch in schema
                .get_mut(key)
                .and_then(Value::as_array_mut)
                .into_iter()
                .flatten()
            {
                add_delivery(branch, delivery);
            }
        }
    }
}

fn omitted_schema() -> Value {
    json!({"type":"object","additionalProperties":false,"required":["projection","delivery"],"properties":{
        "projection":{"const":"semantic_budget_exceeded"},
        "delivery":{"type":"object","additionalProperties":false,"required":["status"],
            "properties":{"status":{"const":"omitted"},"reason":{"type":"string"}}},
        "available":{"type":"boolean"},"error_code":{"type":"string"},"category":{"type":"string"},
        "required_capability":{"type":"string"},"profile_id":{"type":"string"},"profile_version":{"type":"string"}
    }})
}

/// Error details are execution-path diagnostics; only the common error identity
/// is shared. They are deliberately outside the successful domain schema.
fn error_schema() -> Value {
    json!({"type":"object","required":["error_code"],"properties":{
        "error_code":{"type":"string"},"category":{"type":"string"},"message":{"type":"string"},
        "required_capability":{"type":"string"},"delivery":delivery_schema(false)
    },"additionalProperties":true,
        "description":"Execution diagnostic. Additional fields are owned by the failing validator or capability gate; they are not domain content."})
}

pub fn book_envelope_schema(tool: &str) -> Option<Value> {
    let id = book_tool_contracts::from_resident_alias(tool)?;
    let full = crate::book_output_schema::canonical(id)?;
    let mut partial = full.clone();
    if id != BookToolId::Text {
        add_delivery(&mut partial, &delivery_schema(id == BookToolId::SearchText));
    }
    let summary = json!({"type":"object","additionalProperties":false,"required":["projection","message"],
        "properties":{"projection":{"enum":["budget_exhausted","bounded_summary"]},"message":{"type":"string"}}});
    let mut base = schema_for::<ToolResultEnvelope>();
    base["properties"]["version"] = json!({"const":TOOL_RESULT_ENVELOPE_VERSION});
    base["properties"]["receipt"]["properties"]["tool"] = json!({"const":tool});
    base["properties"]["receipt"]["properties"]["locator_args"] = json!({"type":"object",
        "description":"Locator-only subset retained by the historical receipt; never source text."});
    base["description"] = json!("Resident delivered result. status/truncated describe the initial model delivery; candidate counts and query status inside model_body retain domain selection/support meaning. After sampling, the active-turn ledger can evict model_body to null while retaining the original status and receipt. Null never supplies fresh evidence. Receipt evidence lists only accepted delivered text.");
    base["oneOf"] = json!([
        {"properties":{"status":{"const":"ok"},"truncated":{"const":false},"model_body":{"anyOf":[full,{"type":"null"}]}}},
        {"properties":{"status":{"const":"partial"},"model_body":{"anyOf":[partial,omitted_schema(),summary,{"type":"null"}]}}},
        {"properties":{"status":{"const":"error"},"model_body":{"anyOf":[error_schema(),omitted_schema(),{"type":"null"}]}}}
    ]);
    // Native and ReAct both serialize the same envelope. Continuations always
    // target an existing Book input contract, including cross-tool recovery.
    let mut calls = Vec::new();
    for contract in book_tool_contracts::contracts()
        .iter()
        .filter(|c| c.aliases.resident.is_some())
    {
        for kind in ["tool_cursor", "next_call", "refine_call"] {
            let mut properties = json!({"kind":{"const":kind},"tool":{"const":contract.aliases.resident.unwrap()},
                "arguments":book_tool_contracts::input_schema(contract.id)});
            let mut required = vec!["kind", "tool", "arguments"];
            let note = match kind {
                "next_call" => Some("reason"),
                "refine_call" => Some("guidance"),
                _ => None,
            };
            if let Some(note) = note {
                properties[note] = json!({"type":"string"});
                required.push(note);
            }
            calls.push(json!({"type":"object","properties":properties,"required":required,"additionalProperties":false}));
        }
    }
    base["properties"]["continuation"] = json!({"oneOf":calls});
    Some(base)
}
