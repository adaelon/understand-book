use super::*;
use serde_json::json;

#[test]
fn advertised_examples_survive_provider_projection_and_real_validation() {
    let registry = crate::orchestrator::resident_tool_registry();
    let specs = crate::orchestrator::tool_specs();
    let profile = crate::ModelRuntimeCatalog::default().resolve("test", crate::ProviderToolProtocol::Native, None);
    let plan = crate::AgentRequestPlan::for_agent_turn(profile, &[crate::Message::user("Compare conditions")], &specs);
    let (native, names) = crate::native_chat_request_projection("test", &plan);
    let react = crate::react_chat_request_projection("test", &plan);
    let protocol = react["messages"][0]["content"].as_str().unwrap();
    let advertised = specs.iter().map(|s| json!({"name":s.name,"description":s.description,"parameters":s.parameters})).collect::<Vec<_>>();
    assert!(protocol.contains(&serde_json::to_string_pretty(&advertised).unwrap()));
    let mut count = 0;
    for (index, spec) in specs.iter().enumerate() {
        let Some(examples) = spec.parameters["examples"].as_array() else { continue };
        assert!(!examples.is_empty(), "{}", spec.name);
        assert_eq!(native["tools"][index]["function"]["parameters"], spec.parameters);
        assert_eq!(names[native["tools"][index]["function"]["name"].as_str().unwrap()], spec.name);
        for (i, input) in examples.iter().enumerate() {
            assert!(!examples[..i].contains(input), "duplicate {}", spec.name);
            validate_schema(&spec.parameters, input, "$").unwrap();
            registry.registration(&spec.name).unwrap().validate_arguments(&input.to_string()).unwrap();
            count += 1;
        }
    }
    assert_eq!(count, 19);
}

#[test]
fn tutor_examples_follow_final_state_schema_and_provider_requests() {
    let action = |kind: &str| json!({"status":"active","current_action":{
        "action":{"origin":"ui_entry","entry_action":kind},"user_supplement":"",
        "accepted_request":"UI request","answer_ref":null,"answer_text":null,
        "source_excerpt":null,"source_bindings":[],"target":null,
        "expected_chat_head":null,"presentation_follow_up":null}});
    let mut supplement = action("explain_selection");
    supplement["current_action"]["user_supplement"] = json!("Let me try first.");
    let mut guided = action("continue_answer");
    guided["default_teaching_intent"] = json!("guided_inquiry");
    let mut preferred = action("continue_answer");
    preferred["confirmed_preferences"] = json!([{"fact_id":"p","preference":"Compare first"}]);
    let choice = |kind: &str| {
        let mut context = action("continue_answer");
        context["current_action"]["action"] = json!({"origin":"ui_choice","teaching_choice":{"delivery_ref":"delivery:fixture","option_id":"next"},"move_kind":kind});
        context
    };
    let cases = [
        ("active", json!({"status":"active"}), 8),
        ("active_response", json!({"status":"active","learner_context":{"pending_assessment_refs":["action:actual-open"],"uninterpreted_fact_refs":["usage:actual-user"]}}), 10),
        ("observing", json!({"status":"observing"}), 2),
        ("observing_expression", json!({"status":"observing","learner_context":{"uninterpreted_fact_refs":["usage:actual-user"]}}), 3),
        ("reference", json!({"status":"reference"}), 1),
        ("explain_selection", action("explain_selection"), 6),
        ("clarify_answer", action("clarify_answer"), 6),
        ("continue_answer", action("continue_answer"), 8),
        ("supplement", supplement, 8),
        ("guided_default", guided, 8),
        ("confirmed_preference", preferred, 8),
        ("choice_explain", choice("explain"), 6),
        ("choice_compare", choice("compare"), 6),
        ("choice_observe", choice("observe"), 6),
    ];
    let mut snapshots = serde_json::Map::new();
    for (name, context, count) in cases {
        let registry = crate::orchestrator::resident_tool_registry().with_tutor_context(Some(&context));
        let registration = registry.registration("tutor.step").unwrap();
        let spec = &registration.spec;
        let examples = spec.parameters["examples"].as_array().unwrap();
        assert_eq!(examples.len(), count, "{name}");
        for (index, input) in examples.iter().enumerate() {
            assert!(!examples[..index].contains(input), "duplicate in {name}");
            validate_schema(&spec.parameters, input, "$").unwrap_or_else(|e| panic!("{name}: {input}: {e:?}"));
            registration.validate_arguments(&input.to_string()).unwrap();
            if input["operation"] == "assess" { assert_eq!(input["action_ref"], context["learner_context"]["pending_assessment_refs"][0]); }
            if input["nature"] == "hypothesis" { assert_eq!(input["fact_refs"][0], context["learner_context"]["uninterpreted_fact_refs"][0]); }
            if context["current_action"].is_object() && input["operation"] == "select" {
                assert!(input["move"].get("capability").is_none());
                assert!(input["move"]["target"].is_object());
                if crate::tutor::action_intent(&context).is_some() {
                    assert!(input["move"].get("intent_origin").is_none());
                    assert!(input["move"].get("interaction_intent").is_none());
                }
                if let Some((intent, "current_action")) = crate::tutor::action_intent(&context) {
                    let action: crate::tutor_continuation::ResolvedInteraction = serde_json::from_value(context["current_action"].clone()).unwrap();
                    assert!(action.validates_current_action(intent, input["move"]["kind"].as_str().unwrap()));
                }
            }
        }
        let profile = crate::ModelRuntimeCatalog::default().resolve("test", crate::ProviderToolProtocol::Native, None);
        let plan = crate::AgentRequestPlan::for_agent_turn(profile, &[crate::Message::user("Tutor examples")], &[spec.clone()]);
        let (native, names) = crate::native_chat_request_projection("test", &plan);
        assert_eq!(native["tools"][0]["function"]["parameters"], spec.parameters);
        assert_eq!(names[native["tools"][0]["function"]["name"].as_str().unwrap()], "tutor.step");
        let react = crate::react_chat_request_projection("test", &plan);
        assert!(react["messages"][0]["content"].as_str().unwrap().contains(&serde_json::to_string_pretty(&json!([spec])).unwrap()));
        snapshots.insert(name.into(), json!({"context":context,"spec":spec,"native":native,"react":react}));
    }
    for status in ["paused", "inactive", "unready"] {
        assert!(crate::orchestrator::resident_tool_registry().with_tutor_context(Some(&json!({"status":status}))).registration("tutor.step").is_none());
    }
    if let Ok(path) = std::env::var("TO3_STATE_SNAPSHOT") {
        std::fs::write(path, serde_json::to_vec_pretty(&snapshots).unwrap()).unwrap();
    }
}

#[test]
fn tutor_tc2a_misuse_and_help_have_valid_example_counterparts() {
    let spec = crate::tutor::spec();
    let examples = spec.parameters["examples"].as_array().unwrap();
    let selected = examples.iter().find(|e| e["operation"] == "select").unwrap();
    check("tutor.step", selected.clone(), true);
    check("tutor.step", json!({"operation":"select","move":"explain speed"}), false);
    check("tutor.step", json!({"operation":"select"}), false);
    let mut top_level = selected["move"].clone();
    top_level["operation"] = json!("select");
    check("tutor.step", top_level, false);
    let scored = examples.iter().find(|e| e["move"]["assessment"].is_object()).unwrap();
    check("tutor.step", scored.clone(), true);
    for actions in [None, Some(json!([])), Some(json!(["reveal"]))] {
        let mut invalid = scored.clone();
        invalid["move"].as_object_mut().unwrap().remove("actions");
        if let Some(actions) = actions { invalid["move"]["actions"] = actions; }
        check("tutor.step", invalid, false);
    }
    let helped = examples.iter().find(|e| e["move"]["hint"].is_string()).unwrap();
    check("tutor.step", helped.clone(), true);
    for field in ["hint", "explanation"] {
        let mut invalid = helped.clone();
        invalid["move"].as_object_mut().unwrap().remove(field);
        check("tutor.step", invalid, false);
    }
}

#[test]
fn advertised_example_validation_detects_projection_and_parser_drift() {
    let mut schema = input_schema(BookToolId::Text);
    let example = schema["examples"][0].clone();
    schema["properties"].as_object_mut().unwrap().remove("end_lid");
    assert!(validate_schema(&schema, &example, "$").is_err());
    let mut invalid = input_schema(BookToolId::Query)["examples"][0].clone();
    invalid["targets"] = json!(["alpha threshold", "alpha threshold"]);
    assert!(validate_schema(&input_schema(BookToolId::Query), &invalid, "$").is_ok());
    assert!(crate::orchestrator::resident_tool_registry().registration("book.query").unwrap().validate_arguments(&invalid.to_string()).is_err());
}

#[test]
fn book_common_semantics_and_routing_guidance_reach_the_resident_surface() {
    let registry = crate::orchestrator::resident_tool_registry();
    for contract in book_tool_contracts::contracts() {
        let Some(name) = contract.aliases.resident else { continue };
        let registration = registry.registration(name).unwrap();
        assert_eq!(registration.spec.parameters, input_schema(contract.id), "{name}");
        assert_eq!(registration.routing_card.use_when, vec![contract.use_when.to_string()]);
        assert_eq!(registration.routing_card.avoid_when, vec![contract.do_not_use_when.to_string()]);
        if let Some(properties) = registration.spec.parameters["properties"].as_object() {
            for (field, property) in properties {
                assert!(property["description"].as_str().is_some_and(|s| !s.is_empty()), "{name}.{field}");
            }
        }
        for remote_field in ["published_book_ref", "start_utf16", "page"] {
            assert!(registration.spec.parameters["properties"].get(remote_field).is_none(), "{name}.{remote_field}");
        }
    }
    for input in [json!({"lid":"1.1"}), json!({"lid":"1.1","k":0}), json!({"lid":"1.1","k":null,"granularity":null})] {
        check("book.context", input, true);
    }
    check("book.search_text", json!({"query":"alpha","scope":{"within_lid":"1","relative_to":{"lid":"1.2","direction":"before"}}}), true);
    check("book.search_text", json!({"query":"alpha","scope":{"relative_to":{"lid":"1.2"}}}), false);
}

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
