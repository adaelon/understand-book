use super::*;
use serde_json::json;

#[test]
fn optional_nulls_match_serde_without_weakening_required_fields() {
    check(
        "tutor.step",
        json!({"operation":"select","move":{"move_id":"m1","object_ids":["o1"],"capability":"explanation","kind":"observe","prompt":"Compare","interaction_intent":"neutral","intent_origin":"neutral","intent_quote":null,"assessment":null}}),
        true,
    );
    check(
        "reader.layout.apply",
        json!({"actions":[{"kind":"open_slot","slot_id":"agent","region":null}]}),
        true,
    );
    check(
        "presentation.author",
        json!({"operation":"read","reference":null,"candidate_id":"c1","file":null,"length":null}),
        true,
    );
    check(
        "presentation.author",
        json!({"operation":"read","reference":null,"candidate_id":null}),
        false,
    );
    check(
        "presentation.author",
        json!({"operation":"preview","candidate_id":null}),
        false,
    );
    check(
        "presentation.author",
        json!({"operation":"preview","candidate_id":"c1","width":null,"viewport":{"width":960,"height":720,"input":"mouse"}}),
        true,
    );
    check(
        "presentation.author",
        json!({"operation":"write","title":null,"html":"<p>x</p>","readable_content":"text"}),
        false,
    );
    check(
        "artifact.read",
        json!({"artifact_ref":"a1","record_refs":null,"cursor":"c1"}),
        true,
    );
    check(
        "goal.update",
        json!({"operation":"working","focus":"Read","next_move":"Explain","items":null}),
        true,
    );
}

fn check(tool: &str, input: Value, accepted: bool) {
    let registry = crate::orchestrator::resident_tool_registry();
    let registration = registry.registration(tool).unwrap();
    let schema = validate_schema(&registration.spec.parameters, &input, "$");
    assert_eq!(
        schema.is_ok(),
        accepted,
        "{tool} schema: {input}: {schema:?}"
    );
    assert_eq!(
        registration.validate_arguments(&input.to_string()).is_ok(),
        accepted,
        "{tool} runtime: {input}"
    );
}

#[test]
fn author_operations_require_complete_inputs() {
    for input in [
        json!({"operation":"write","title":"Page","readable_content":"Text"}),
        json!({"operation":"write","html":"<p>x</p>","readable_content":"Text"}),
        json!({"operation":"preview"}),
        json!({"operation":"prepare","phase":"local"}),
        json!({"operation":"read"}),
        json!({"operation":"read","candidate_id":"c1","reference":{"presentation_id":"p1","revision":1}}),
        json!({"operation":"preview","candidate_id":"c1","width":960,"viewport":{"width":960,"height":720,"input":"mouse"}}),
        json!({"operation":"preview","candidate_id":"c1","actions":[{"kind":"click"}]}),
    ] {
        check("presentation.author", input, false);
    }
    for input in [
        json!({"operation":"write","title":"Page","html":"<p>x</p>","readable_content":"Text"}),
        json!({"operation":"read","candidate_id":"c1"}),
        json!({"operation":"prepare","phase":"local","needs":[]}),
        json!({"operation":"preview","candidate_id":"c1","actions":[{"kind":"scroll","y":120}]}),
    ] {
        check("presentation.author", input, true);
    }
}

#[test]
fn goal_operations_never_silently_discard_plan_fields() {
    check(
        "goal.update",
        json!({"operation":"working","focus":"Read"}),
        false,
    );
    let mut refine = json!({"operation":"refine","interpretation":"Explain","requirements":[{"id":"r1","description":"Explain","basis_turn_id":"t1","verification":"content"}]});
    check("goal.update", refine.clone(), true);
    refine["items"] = json!([{"id":"p1","description":"Read","status":"pending"}]);
    check("goal.update", refine.clone(), false);
    assert!(serde_json::from_value::<crate::goal::GoalUpdate>(refine).is_err());
    check(
        "goal.update",
        json!({"operation":"working","focus":"Read","next_move":"Explain","items":[]}),
        true,
    );
}

#[test]
fn tutor_operations_require_their_payloads() {
    for input in [
        json!({"operation":"select"}),
        json!({"operation":"assess"}),
        json!({"operation":"evidence","action_ref":"a1"}),
    ] {
        check("tutor.step", input, false);
    }
    check("tutor.step", json!({"operation":"material"}), true);
    check(
        "tutor.step",
        json!({"operation":"assess","action_ref":"a1"}),
        true,
    );
    let mut input = json!({"operation":"select","move":{"move_id":"m1","object_ids":["o1"],"capability":"evaluation","kind":"question","prompt":"Compare","interaction_intent":"guided_inquiry","intent_origin":"current_request"}});
    check("tutor.step", input.clone(), true);
    input["move"]["assessment"] =
        json!({"source_lids":["1.1"],"source_sufficient":true,"feedback":"after_submit"});
    check("tutor.step", input, false);
    let mut hypothesis = json!({"operation":"evidence","nature":"hypothesis","operation_id":"h1","fact_refs":["usage:1"],
        "interpretation":"Needs the denominator connection","teaching_implication":"Explain the whole journey",
        "target":{"learning_focus":"speed","expected_performance":"explain total time","capability":"explanation",
            "source_bindings":[{"source_id":"book","source_revision":"v1","start_lid":"1.1","end_lid":"1.1"}],"object_refs":[]}});
    check("tutor.step",hypothesis.clone(),true);
    hypothesis.as_object_mut().unwrap().remove("fact_refs");
    check("tutor.step",hypothesis,false);
}

#[test]
fn note_is_only_exposed_through_reader_note() {
    check(
        "memory.save",
        json!({"type":"note","anchor_lid":"1.1","content":"Note"}),
        false,
    );
    check("reader.note", json!({"lid":"1.1","text":"Note"}), true);
}

#[test]
fn reader_commands_expose_nested_required_fields() {
    check(
        "reader.layout.apply",
        json!({"actions":[{"kind":"focus_slot"}]}),
        false,
    );
    check(
        "reader.layout.apply",
        json!({"actions":[{"kind":"focus_slot","slot_id":"agent"}]}),
        true,
    );
    check(
        "reader.paper_minimap.apply",
        json!({"base_state_rev":1,"reason":"Orient","commands":[{"scope":"session","action":{"kind":"focus_region"}}]}),
        false,
    );
    check(
        "reader.paper_minimap.apply",
        json!({"base_state_rev":1,"reason":"Orient","commands":[{"scope":"session","action":{"kind":"focus_region","region_id":"r1"}}]}),
        true,
    );
    check(
        "reader.paper_minimap.apply",
        json!({"base_state_rev":1,"reason":"Orient","commands":[{"scope":"session","action":{"kind":"set_presentation","presentation":"expanded"}}]}),
        false,
    );
}

#[test]
fn book_comparison_schema_requires_separate_targets() {
    let mut input = json!({"query":"Compare A and B","intent":"comparison","targets":["A and B"],"obligations":[{"requirement":"Differences"}],"anchor_lid":"1.1"});
    check("book.query", input.clone(), false);
    input["targets"] = json!(["A", "B"]);
    check("book.query", input, true);
}

#[test]
fn search_schemas_expose_existing_length_limits() {
    for (tool, limit) in [
        ("artifact.search", 512),
        ("book.search_text", 4096),
        ("book.concept", 4096),
    ] {
        check(tool, json!({"query":"x".repeat(limit+1)}), false);
        check(tool, json!({"query":"x".repeat(limit)}), true);
    }
}

#[test]
fn artifact_read_exposes_pointer_and_cursor_rules() {
    let schema = artifact_read_input_schema();
    assert_eq!(
        schema["properties"]["field_paths"]["items"]["pattern"],
        "^/(?:[^~]|~[01])*$"
    );
    check(
        "artifact.read",
        json!({"artifact_ref":"a1","record_refs":["r1"],"cursor":"c1"}),
        false,
    );
    check(
        "artifact.read",
        json!({"artifact_ref":"a1","field_paths":["/title"]}),
        true,
    );
}

#[test]
fn mcp_guide_schema_has_conditional_requirements() {
    let schema = input_schema(BookToolId::Guide);
    for input in [json!({}), json!({"action":"close"})] {
        assert!(validate_schema(&schema, &input, "$").is_err());
    }
    for input in [
        json!({"intent":"Explain"}),
        json!({"action":"close","session_id":"s1"}),
    ] {
        assert!(validate_schema(&schema, &input, "$").is_ok());
    }
}
