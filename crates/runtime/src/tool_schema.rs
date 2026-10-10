//! Model-visible constraints for existing operation-based tools.
use crate::ToolSpec;
use serde_json::{json, Map, Value};

/// Keep field descriptions once, and close each operation over its own fields.
fn operations(schema: &mut Value, tag: &str, variants: &[(&str, &[&str], &[&str])]) {
    let branches: Vec<_> = variants
        .iter()
        .map(|(name, required, optional)| {
            let mut properties = Map::new();
            properties.insert(tag.into(), json!({"enum":[name]}));
            for field in required.iter().chain(optional.iter()) {
                assert!(
                    schema["properties"].get(*field).is_some(),
                    "undeclared tool field: {field}"
                );
                properties.insert((*field).into(), json!({}));
            }
            let required: Vec<_> = std::iter::once(tag)
                .chain(required.iter().copied())
                .collect();
            json!({"properties":properties,"required":required,"additionalProperties":false})
        })
        .collect();
    schema["anyOf"] = json!(branches);
}

pub(crate) fn complete_spec(mut tool: ToolSpec) -> ToolSpec {
    let schema = &mut tool.parameters;
    match tool.name.as_str() {
        "goal.update" => {
            operations(
                schema,
                "operation",
                &[
                    (
                        "working",
                        &["focus", "next_move"],
                        &["open_questions", "items"],
                    ),
                    ("refine", &["interpretation", "requirements"], &[]),
                    (
                        "revise",
                        &[
                            "basis_turn_id",
                            "basis_quote",
                            "interpretation",
                            "requirements",
                        ],
                        &[],
                    ),
                ],
            );
            tool.description.push_str(" Use working in a separate call to save focus, next_move or plan items; refine/revise do not accept these fields.");
            nullable(schema, "items");
        }
        "reader.layout.apply" => {
            schema["properties"]["actions"]["items"] =
                book_tool_contracts::schema_value::<read_tools::ReaderLayoutAction>();
        }
        "reader.paper_minimap.apply" => {
            let mut commands = book_tool_contracts::schema_value::<reader::PaperMinimapCommand>();
            // These commands belong to the human UI and are always forbidden for Agent.
            for variant in commands["oneOf"].as_array_mut().unwrap() {
                if variant["properties"]["scope"]["enum"] == json!(["session"]) {
                    variant["properties"]["action"]["oneOf"]
                        .as_array_mut()
                        .unwrap()
                        .retain(|action| {
                            !["set_presentation", "update_viewport", "set_selected_lid"]
                                .iter()
                                .any(|kind| action["properties"]["kind"]["enum"] == json!([kind]))
                        });
                }
            }
            schema["properties"]["commands"]["items"] = commands;
        }
        "tutor.step" => {
            operations(
                schema,
                "operation",
                &[
                    ("material", &[], &["object_id", "after"]),
                    ("trace", &[], &["event_id", "after"]),
                    ("select", &["move"], &[]),
                    ("outside", &[], &[]),
                    ("assess", &["action_ref"], &[]),
                    (
                        "understanding",
                        &[],
                        &["object_id", "evidence_ref", "after"],
                    ),
                    (
                        "evidence",
                        &[
                            "action_ref",
                            "object_id",
                            "operation_id",
                            "response_quote",
                            "sources",
                            "interpretation",
                        ],
                        &["supersedes"],
                    ),
                    ("evidence", &["nature","operation_id","target","fact_refs","interpretation","teaching_implication"], &["supersedes"]),
                ],
            );
            schema["anyOf"].as_array_mut().unwrap().last_mut().unwrap()["properties"]["nature"] = json!({"enum":["hypothesis"]});
            let movement = &mut schema["properties"]["move"];
            let description = movement["properties"]["assessment"]["description"].clone();
            movement["properties"]["assessment"] =
                book_tool_contracts::schema_value::<memory::assessment::AssessmentContract>();
            movement["properties"]["assessment"]["description"] = description;
            movement["additionalProperties"] = json!(false);
            for field in [
                "intent_quote",
                "preference_ref",
                "presentation",
                "hint",
                "explanation",
                "assessment",
            ] {
                nullable(movement, field);
            }
            movement["allOf"] = json!([
                {"if":{"required":["actions"],"properties":{"actions":{"contains":{"enum":["request_hint"]}}}},"then":{"required":["hint"],"properties":{"hint":{"type":"string","minLength":1}}}},
                {"if":{"required":["actions"],"properties":{"actions":{"contains":{"enum":["reveal"]}}}},"then":{"required":["explanation"],"properties":{"explanation":{"type":"string","minLength":1}}}}
            ]);
        }
        "presentation.author" => {
            operations(
                schema,
                "operation",
                &[
                    ("prepare", &["phase", "needs"], &["framework", "focus"]),
                    ("render_plot", &["code"], &["data", "size"]),
                    ("render_animation", &["code"], &["data", "size", "cues"]),
                    (
                        "read",
                        &[],
                        &["reference", "candidate_id", "file", "offset", "length"],
                    ),
                    (
                        "search",
                        &["query"],
                        &["reference", "candidate_id", "file", "offset", "max_matches"],
                    ),
                    (
                        "patch",
                        &["edits"],
                        &[
                            "reference",
                            "candidate_id",
                            "title",
                            "readable_content",
                            "state_contract",
                            "initial_state",
                        ],
                    ),
                    (
                        "write",
                        &["title", "html", "readable_content"],
                        &[
                            "based_on",
                            "new_object",
                            "state_contract",
                            "asset_refs",
                            "libraries",
                            "source_ref_ids",
                            "assumptions",
                            "initial_state",
                        ],
                    ),
                    (
                        "preview",
                        &["candidate_id"],
                        &["read_selector", "width", "viewport", "actions"],
                    ),
                    ("deliver", &["candidate_id"], &[]),
                ],
            );
            schema["allOf"] = json!([
                {"if":{"properties":{"operation":{"enum":["read","search","patch"]}},"required":["operation"]},"then":{"oneOf":[{"required":["reference"],"properties":{"reference":{"type":"object"}}},{"required":["candidate_id"],"properties":{"candidate_id":{"type":"string"}}}]}},
                {"not":{"required":["width","viewport"],"properties":{"width":{"type":"integer"},"viewport":{"type":"object"}}}},
                {"if":{"required":["new_object"],"properties":{"new_object":{"enum":[true]}}},"then":{"not":{"required":["based_on"],"properties":{"based_on":{"type":"object"}}}}}
            ]);
            operations(
                &mut schema["properties"]["actions"]["items"],
                "kind",
                &[
                    ("click", &["selector"], &[]),
                    ("key", &["key"], &["selector"]),
                    ("seek", &["semantic_state", "transition_progress"], &[]),
                    ("scroll", &["y"], &[]),
                ],
            );
            schema["properties"]["source_ref_ids"]["description"]=json!("write: list every source ref used in HTML or readable_content. patch preserves existing source bindings; use write to change them.");
            for field in [
                "framework",
                "focus",
                "size",
                "reference",
                "candidate_id",
                "file",
                "length",
                "max_matches",
                "title",
                "readable_content",
                "state_contract",
                "based_on",
                "read_selector",
                "width",
                "viewport",
            ] {
                nullable(schema, field);
            }
            nullable(&mut schema["properties"]["actions"]["items"], "selector");
        }
        _ => {}
    }
    tool
}

/// Optional Serde fields accept an explicit null. A field shared with a required
/// operation (e.g. write.title vs patch.title) stays non-null in that operation.
fn nullable(schema: &mut Value, field: &str) {
    let original = schema["properties"][field].clone();
    let field_type = original["type"].as_str().expect("nullable field has a simple type");
    schema["properties"][field]["type"] = json!([field_type, "null"]);
    if let Some(branches) = schema.get_mut("anyOf").and_then(Value::as_array_mut) {
        for branch in branches {
            if branch["required"]
                .as_array()
                .is_some_and(|fields| fields.contains(&json!(field)))
            {
                branch["properties"][field] = original.clone();
            }
        }
    }
}
