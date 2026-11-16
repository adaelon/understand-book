use super::*;
use serde_json::json;

#[test]
fn teacher_corrections_tools_have_closed_operation_contracts() {
    for status in ["practice","observing","course"] {
        let registry=crate::orchestrator::resident_tool_registry().with_tutor_context(Some(&json!({"status":status})));
        let tool=registry.registration("tutor.step").unwrap();
        for request in [json!({"operation":"corrections","correction_action":"search","queries":["平均速度"]}),
            json!({"operation":"corrections","correction_action":"read","correction_ref":"ref","after":768}),
            json!({"operation":"corrections","correction_action":"record","operation_id":"record","correction":{
                "previous_event_ref":"actual-ref","mistake_quote":"old words","corrected_claim":"specific correction","applies_when":"equal distances",
                "topics":["speed"],"source_quotes":[{"lid":"1.1","quote":"source words"}],"supersedes":[]}}),
            json!({"operation":"corrections","correction_action":"use","operation_id":"use","correction_ref":"ref","decision":"apply","reason":"The same equal-distance condition."})] {
            tool.validate_arguments(&request.to_string()).unwrap_or_else(|e|panic!("{request}: {e:?}"));
        }
        for request in [json!({"operation":"corrections","correction_action":"record"}),
            json!({"operation":"trace","queries":["speed"]}),json!({"operation":"corrections","correction_action":"use","correction_ref":"ref"})] {
            assert!(tool.validate_arguments(&request.to_string()).is_err(),"{request}");
        }
    }
}

#[test]
fn course_tutor_uses_the_course_mainline_without_generic_design_or_grading_tools() {
    for status in ["course", "observing"] {
        let context=json!({"status":status,"inquiry_course":{"lesson":{"progress":{"run_id":"course:1","stage":"prediction"}}},"socratic_dialogue":{"question":"What changed?","purpose":{"learner_inference":"compare two bounds"}}});
        let registry=crate::orchestrator::resident_tool_registry().with_tutor_context(Some(&context));
        let tool=registry.registration("tutor.step").unwrap();
        assert_eq!(tool.spec.parameters["properties"]["operation"]["enum"],json!(["trace","understanding","evidence","corrections","closed_book"]));
        for operation in ["design","select","assess"] {
            assert!(tool.validate_arguments(&json!({"operation":operation}).to_string()).is_err());
        }
        let modules=crate::tutor::instruction_modules(&context,&[],None);
        assert!(modules.iter().any(|module|module.asset_id=="resident-agent.tutor.course"));
        assert!(!modules.iter().any(|module|matches!(module.asset_id.as_str(),"resident-agent.tutor.design"|"resident-agent.tutor.select"|"resident-agent.tutor.socratic")));
        let instructions=crate::tutor::control_instructions(&context);
        assert!(instructions.contains("saved socratic_dialogue"));
        assert!(instructions.contains(if status=="course" {"Course tutoring is active"} else {"Course tutoring is not active now"}));
    }
}

#[test]
fn ranked_search_delivery_matches_its_closed_output_schema() {
    let registry = crate::orchestrator::resident_tool_registry();
    let registration = registry.registration("book.search").unwrap();
    let book = read_tools::Book::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../book-tool-contracts/fixtures/example-book")).unwrap();
    let input = json!({"query":"threshold activation conditions","limit":4});
    let raw = serde_json::to_string(&book.search_ranked("threshold activation conditions", None, 4).unwrap()).unwrap();
    let project = |limit| crate::tool_result::project_tool_result("book.search", &input.to_string(), &raw,
        ToolOutputPolicy { result_policy:ToolResultPolicy::EvidenceProjection,max_model_body_bytes:limit }, limit, &book);
    let complete = project(100_000);
    assert!(!complete.truncated);
    let schema = registration.output_schema().unwrap();
    let complete_schema = &schema["oneOf"][0]["properties"]["model_body"]["anyOf"][0];
    validate_schema(complete_schema, &complete.model_body, "$").unwrap();
    let mut invalid = complete.model_body.clone(); invalid["unadvertised"] = json!(true);
    assert!(validate_schema(complete_schema, &invalid, "$").is_err());
    let partial = project(serde_json::to_vec(&complete.model_body).unwrap().len()-1);
    assert!(partial.truncated);
    let partial_schema = &schema["oneOf"][1]["properties"]["model_body"];
    validate_schema(partial_schema, &partial.model_body, "$").unwrap();
    let omitted = project(32);
    validate_schema(partial_schema, &omitted.model_body, "$").unwrap();
}

#[test]
fn presentation_read_error_explains_the_complete_operation_without_hiding_cause() {
    let registry = crate::orchestrator::resident_tool_registry();
    let registration = registry.registration("presentation.author").unwrap();
    for input in [
        json!({"operation":"read","candidate_id":"candidate-1","libraries":null,"based_on":null}),
        json!({"operation":"read","reference":"candidate-1"}),
        json!({"operation":"read","candidate_id":"candidate-1","based_on":null}),
    ] {
        let original = validate_schema(&registration.spec.parameters, &input, "$").unwrap_err();
        let error = registration.validate_arguments(&input.to_string()).unwrap_err();
        assert!(error.message.starts_with(&original.message));
        assert!(error.message.contains("omit unrelated fields even when null"));
        let selected_schema = error.message.split("Selected operation schema: ").nth(1).unwrap()
            .split(" Use exactly one target:").next().unwrap();
        let schema: Value = serde_json::from_str(selected_schema).unwrap();
        assert_eq!(schema["properties"]["operation"]["enum"], json!(["read"]));
        assert!(schema["properties"].get("libraries").is_none());
        assert!(schema["properties"].get("based_on").is_none());
        assert!(error.message.contains("candidate_id as the string returned by write/patch"));
    }
    registration.validate_arguments(&json!({"operation":"read","candidate_id":"candidate-1","offset":0,"length":200}).to_string()).unwrap();
}

#[test]
fn video_tool_specs_examples_survive_real_provider_projection() {
    let registry = crate::orchestrator::resident_tool_registry();
    let specs: Vec<_> = ["video.read_frames", "video.search", "video.review_audio"]
        .iter().map(|name| registry.registration(name).unwrap().spec.clone()).collect();
    let profile = crate::ModelRuntimeCatalog::default().resolve("test", crate::ProviderToolProtocol::Native, None);
    let plan = crate::AgentRequestPlan::for_agent_turn(profile, &[crate::Message::user("Explain this video frame")], &specs);
    let (native, _) = crate::native_chat_request_projection("test", &plan);
    let react = crate::react_chat_request_projection("test", &plan);
    for (index, spec) in specs.iter().enumerate() {
        let examples = spec.parameters["examples"].as_array().expect("video input examples");
        assert!(!examples.is_empty());
        for example in examples {
            validate_schema(&spec.parameters, example, "$").unwrap();
            registry.registration(&spec.name).unwrap().validate_arguments(&example.to_string()).unwrap();
            match spec.name.as_str() {
                "video.read_frames" => { serde_json::from_value::<crate::video::ReadFrames>(example.clone()).unwrap(); },
                "video.search" => { serde_json::from_value::<read_tools::video_search::VideoSearch>(example.clone()).unwrap(); },
                "video.review_audio" => { serde_json::from_value::<crate::video_audio::ReviewAudio>(example.clone()).unwrap(); },
                _ => unreachable!(),
            }
        }
        assert_eq!(native["tools"][index]["function"]["parameters"], spec.parameters);
    }
    let advertised: Vec<_> = specs.iter().map(|s| json!({"name":s.name,"description":s.description,"parameters":s.parameters})).collect();
    assert!(react["messages"][0]["content"].as_str().unwrap().contains(&serde_json::to_string_pretty(&advertised).unwrap()));
    let location = &specs[0].parameters["properties"]["location"];
    assert!(location["properties"]["end_ms"]["description"].as_str().unwrap().to_ascii_lowercase().contains("exclusive"));
    assert_eq!(&specs[2].parameters["properties"]["location"], location);
}

#[test]
fn to5_examples_follow_final_schema_and_provider_requests() {
    let registry = crate::orchestrator::resident_tool_registry();
    let names = ["goal.update", "reader.note", "reader.highlight", "memory.save"];
    let specs: Vec<_> = names.iter().map(|name| registry.registration(name).unwrap().spec.clone()).collect();
    let profile = crate::ModelRuntimeCatalog::default().resolve("test", crate::ProviderToolProtocol::Native, None);
    let tasks = json!({
        "origin":"解释这一段并保存一条笔记",
        "revision":"改成只解释，不要保存笔记",
        "note":"把‘先核对前提再下结论’记为这一段的笔记",
        "explain":"只解释选中的原文，不保存笔记、高亮或记忆",
        "context":"我有 Rust 基础，读这一段时请先给例子"
    });
    let plan = crate::AgentRequestPlan::for_agent_turn(profile, &[crate::Message::user(tasks["note"].as_str().unwrap())], &specs);
    let (native, _) = crate::native_chat_request_projection("test", &plan);
    let react = crate::react_chat_request_projection("test", &plan);
    if let Ok(path) = std::env::var("TO5_SPEC_SNAPSHOT") {
        std::fs::write(path, serde_json::to_vec_pretty(&json!({"specs":specs,"tasks":tasks,"native":native,"react":react})).unwrap()).unwrap();
    }
    for (index, spec) in specs.iter().enumerate() {
        let examples = spec.parameters["examples"].as_array().expect("advertise TO5 inputs");
        assert_eq!(examples.len(), [3, 1, 2, 4][index], "{}", spec.name);
        for (i, input) in examples.iter().enumerate() {
            assert!(!examples[..i].contains(input));
            validate_schema(&spec.parameters, input, "$").unwrap();
            registry.registration(&spec.name).unwrap().validate_arguments(&input.to_string()).unwrap();
            if spec.name == "goal.update" {
                serde_json::from_value::<crate::goal::GoalUpdate>(input.clone()).unwrap();
            }
        }
        assert_eq!(native["tools"][index]["function"]["parameters"], spec.parameters);
    }
    let advertised: Vec<_> = specs.iter().map(|s| json!({"name":s.name,"description":s.description,"parameters":s.parameters})).collect();
    assert!(react["messages"][0]["content"].as_str().unwrap().contains(&serde_json::to_string_pretty(&advertised).unwrap()));
}

#[test]
fn presentation_to4_examples_follow_final_schema_parser_and_provider_requests() {
    let registry = crate::orchestrator::resident_tool_registry();
    let registration = registry.registration("presentation.author").unwrap();
    let spec = &registration.spec;
    let profile = crate::ModelRuntimeCatalog::default().resolve("test", crate::ProviderToolProtocol::Native, None);
    let plan = crate::AgentRequestPlan::for_agent_turn(profile, &[crate::Message::user("Revise the existing presentation")], &[spec.clone()]);
    let (native, _) = crate::native_chat_request_projection("test", &plan);
    let react = crate::react_chat_request_projection("test", &plan);
    if let Ok(path) = std::env::var("TO4_SPEC_SNAPSHOT") {
        std::fs::write(path, serde_json::to_vec_pretty(&json!({"spec":spec,"native":native,"react":react})).unwrap()).unwrap();
    }
    let examples = spec.parameters["examples"].as_array().expect("advertise Presentation inputs");
    let mut operations = std::collections::BTreeSet::new();
    for (index, input) in examples.iter().enumerate() {
        assert!(!examples[..index].contains(input), "duplicate Presentation example");
        validate_schema(&spec.parameters, input, "$").unwrap();
        registration.validate_arguments(&input.to_string()).unwrap();
        serde_json::from_value::<crate::presentation_author::AuthorRequest>(input.clone()).unwrap();
        operations.insert(input["operation"].as_str().unwrap());
    }
    assert_eq!(operations.into_iter().collect::<Vec<_>>(), vec!["deliver", "patch", "prepare", "preview", "read", "read_method", "render_animation", "render_plot", "search", "write"]);
    assert_eq!(native["tools"][0]["function"]["parameters"], spec.parameters);
    assert!(react["messages"][0]["content"].as_str().unwrap().contains(&serde_json::to_string_pretty(&json!([spec])).unwrap()));
    for operation in ["read", "search", "patch"] {
        for field in ["reference", "candidate_id"] {
            assert!(examples.iter().any(|e| e["operation"] == operation && e.get(field).is_some()), "{operation}: {field}");
        }
    }
    for (operation, field) in [("write", "based_on"), ("write", "new_object"), ("preview", "width"), ("preview", "viewport")] {
        assert!(examples.iter().any(|e| e["operation"] == operation && e.get(field).is_some()), "{operation}: {field}");
    }
}

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
    // Includes the metadata-only revision without copying the retained body.
    assert_eq!(count, 81);
}

#[test]
fn tutor_examples_follow_final_state_schema_and_provider_requests() {
    let action = |kind: &str| json!({"status":"practice","current_action":{
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
    let mut native_action = action("explain_selection");
    native_action["native_board_candidate_pending"] = json!(true);
    let choice = |kind: &str| {
        let mut context = action("continue_answer");
        context["current_action"]["action"] = json!({"origin":"ui_choice","teaching_choice":{"delivery_ref":"delivery:fixture","option_id":"next"},"move_kind":kind});
        context
    };
    let cases = [
        ("practice", json!({"status":"practice"}), 17),
        ("native_board_pending", json!({"status":"practice","native_board_candidate_pending":true}), 13),
        ("native_board_action", native_action, 11),
        ("active_response", json!({"status":"practice","learner_context":{"pending_assessment_refs":["action:actual-open"],"uninterpreted_fact_refs":["usage:actual-user"]}}), 17),
        ("observing", json!({"status":"observing"}), 2),
        ("observing_expression", json!({"status":"observing","learner_context":{"uninterpreted_fact_refs":["usage:actual-user"]}}), 2),
        ("reference", json!({"status":"reference"}), 1),
        ("explain_selection", action("explain_selection"), 13),
        ("clarify_answer", action("clarify_answer"), 17),
        ("continue_answer", action("continue_answer"), 17),
        ("supplement", supplement, 17),
        ("guided_default", guided, 17),
        ("confirmed_preference", preferred, 17),
        ("choice_explain", choice("explain"), 17),
        ("choice_compare", choice("compare"), 17),
        ("choice_observe", choice("observe"), 17),
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
            if context["native_board_candidate_pending"] == true && input["operation"] == "select" {
                assert_eq!(input["move"]["board_candidate_revision"], 1);
                assert!(input["move"].get("delivery_text").is_none());
                assert!(input.get("delivery_move_refs").is_none());
                assert!(input["move"].get("design_ref").is_none());
                assert!(input["move"].get("block_refs").is_none());
                assert!(input["move"].get("target").is_none());
            }
            if context["current_action"].is_object() && context["native_board_candidate_pending"] != true && input["operation"] == "select" {
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
    let mut unavailable = serde_json::Map::new();
    for status in ["paused", "inactive", "unready"] {
        let context = json!({"status":status});
        assert!(crate::orchestrator::resident_tool_registry().with_tutor_context(Some(&context)).registration("tutor.step").is_none());
        unavailable.insert(status.into(), json!({"context":context,"spec":null}));
    }
    if let Ok(path) = std::env::var("TO3_STATE_SNAPSHOT") {
        std::fs::write(path, serde_json::to_vec_pretty(&snapshots).unwrap()).unwrap();
    }
    if let Ok(path) = std::env::var("TOOL_TUTOR_SNAPSHOT") {
        let mut states: serde_json::Map<String, serde_json::Value> = snapshots.iter()
            .map(|(name, snapshot)| (name.clone(), json!({"context":snapshot["context"],"spec":snapshot["spec"]})))
            .collect();
        states.extend(unavailable);
        std::fs::write(path, serde_json::to_vec_pretty(&states).unwrap()).unwrap();
    }
}

#[test]
fn model_tutor_examples_keep_native_selection_and_short_text_repairs() {
    for native_board in [false, true] {
        let mut context = json!({"status":"practice","native_board_candidate_pending":native_board});
        let spec = crate::tutor::model_spec_for_context(&context).unwrap();
        let examples = spec.parameters["examples"].as_array().unwrap();
        assert!(examples.iter().any(|example| example["operation"] == "select"));
        if !native_board {
            assert!(examples.iter().any(|example| example["operation"] == "revise" && example["move_changes"].is_object()));
        }
        for example in examples { validate_schema(&spec.parameters, example, "$").unwrap(); }
        context["current_design"] = json!({"design_ref":"actual-new-design"});
        context["learner_context"] = json!({"uninterpreted_fact_refs":["actual-new-expression"]});
        let next = crate::tutor::model_spec_for_context(&context).unwrap();
        assert_eq!(serde_json::to_value(spec).unwrap(), serde_json::to_value(next).unwrap(),
            "live references must not change the declaration or examples");
    }
}

#[test]
fn tutor_schema_examples_stay_stable_when_actual_references_change() {
    for status in ["practice", "observing", "reference"] {
        let context = |suffix: &str| json!({"status":status,"learner_context":{
            "pending_assessment_refs":[format!("action:open-{suffix}")],
            "uninterpreted_fact_refs":[format!("usage:learner-{suffix}")]}});
        let earlier = context("earlier");
        let current = context("current");
        let first = crate::tutor::spec_for_context(&earlier).unwrap();
        let next = crate::tutor::spec_for_context(&current).unwrap();
        assert_eq!(serde_json::to_value(&first).unwrap(), serde_json::to_value(&next).unwrap(),
            "actual event IDs must not change the advertised contract in {status}");
        assert_ne!(crate::tutor::learner_view(&earlier), crate::tutor::learner_view(&current));
        assert_eq!(crate::tutor::learner_view(&current)["uninterpreted_fact_refs"],
            current["learner_context"]["uninterpreted_fact_refs"]);
        assert!(!next.parameters.to_string().contains("usage:learner-current"));
        assert!(!next.parameters.to_string().contains("action:open-current"));

        let without_refs = crate::tutor::spec_for_context(&json!({"status":status})).unwrap();
        assert_eq!(serde_json::to_value(&next).unwrap(), serde_json::to_value(&without_refs).unwrap(),
            "gaining or consuming a learner fact must not change the advertised contract in {status}");
        let examples = without_refs.parameters["examples"].as_array().unwrap();
        assert!(!examples.iter().any(|example| example["operation"] == "assess" || example["nature"] == "hypothesis"),
            "stable examples must not fabricate available learner facts");
        if status == "observing" {
            assert_eq!(next.parameters["properties"]["operation"]["enum"], json!(["trace", "understanding", "evidence", "corrections", "closed_book"]));
            assert!(next.parameters["properties"].get("move").is_none());
        } else if status == "reference" {
            assert_eq!(next.parameters["properties"]["operation"]["enum"], json!(["trace"]));
            assert!(next.parameters["properties"].get("move").is_none());
        }
    }
}

#[test]
fn tutor_intent_origins_follow_the_frozen_mode_action_and_preferences() {
    let start=json!({"status":"practice","default_teaching_intent":null,
        "current_request":"请继续本次学习，结合当前阅读位置和已有回应，选择一个有依据的教学动作。",
        "user_intent":"先讲解一个关系，再根据我的回应继续。","confirmed_preferences":[]});
    let registry=crate::orchestrator::resident_tool_registry().with_tutor_context(Some(&start));
    let registration=registry.registration("tutor.step").unwrap();
    let properties=&registration.spec.parameters["properties"]["move"]["properties"];
    assert_eq!(properties["intent_origin"]["enum"],json!(["current_request","neutral"]),
        "having a saved goal must not advertise an absent session mode");
    assert!(properties["intent_origin"]["description"].as_str().unwrap().contains("kind=explain remains valid"));
    let mut selection=registration.spec.parameters["examples"].as_array().unwrap().iter()
        .find(|example|example["operation"]=="select").unwrap().clone();
    selection["move"]["kind"]=json!("explain");
    selection["move"]["interaction_intent"]=json!("neutral");
    selection["move"]["intent_origin"]=json!("session_default");
    assert!(registration.validate_arguments(&selection.to_string()).is_err());
    selection["move"]["intent_origin"]=json!("neutral");
    registration.validate_arguments(&selection.to_string()).unwrap();
    for mode in ["direct_explanation","guided_inquiry"] {
        let mut context=start.clone();
        context["default_teaching_intent"]=json!(mode);
        let registry=crate::orchestrator::resident_tool_registry().with_tutor_context(Some(&context));
        let registration=registry.registration("tutor.step").unwrap();
        assert_eq!(registration.spec.parameters["properties"]["move"]["properties"]["intent_origin"]["enum"],
            json!(["current_request","session_default","neutral"]));
        selection["move"]["intent_origin"]=json!("session_default");
        selection["move"]["interaction_intent"]=json!(mode);
        registration.validate_arguments(&selection.to_string()).unwrap();
    }
    let action=json!({"action":{"origin":"ui_entry","entry_action":"clarify_answer"},"user_supplement":"",
        "accepted_request":"Clarify the prior answer","answer_ref":null,"answer_text":null,
        "source_excerpt":null,"source_bindings":[],"target":null,"expected_chat_head":null,"presentation_follow_up":null});
    let mut context=start.clone();
    context["current_action"]=action;
    context["confirmed_preferences"]=json!([{"fact_id":"actual-preference","preference":"Compare first"}]);
    let tool=crate::tutor::spec_for_context(&context).unwrap();
    assert_eq!(tool.parameters["properties"]["move"]["properties"]["intent_origin"]["enum"],
        json!(["current_request","current_action","confirmed_preference","neutral"]));
    context["current_action"]["action"]["entry_action"]=json!("explain_selection");
    let tool=crate::tutor::spec_for_context(&context).unwrap();
    assert!(tool.parameters["properties"]["move"]["properties"].get("intent_origin").is_none(),
        "a program-owned explanation action still binds intent on the host");
    assert_eq!(tool.parameters["properties"]["move"]["properties"]["kind"]["enum"],json!(["explain"]));
}

#[test]
fn tutor_learner_view_examples_follow_available_facts_and_current_permissions() {
    for status in ["practice", "observing", "reference", "paused", "inactive", "unready"] {
        for (assessment, hypothesis) in [(false, false), (true, false), (false, true), (true, true)] {
            let context = json!({"status":status,"learner_context":{
                "pending_assessment_refs":if assessment { vec!["action:actual-open"] } else { vec![] },
                "uninterpreted_fact_refs":if hypothesis { vec!["usage:actual-user"] } else { vec![] },
            }});
            let view = crate::tutor::learner_view(&context);
            let expect_assessment = assessment && status == "practice";
            let expect_hypothesis = false; // Ordinary expression capture belongs to the background observer.
            if !expect_assessment && !expect_hypothesis {
                assert!(view.get("tool_input_examples").is_none(), "no applicable examples in {status}");
                continue;
            }
            let published = &view["tool_input_examples"];
            assert_eq!(published["tool"], "tutor.step");
            let notice = published["notice"].as_str().unwrap();
            assert!(notice.contains("not learner facts or evidence"));
            assert!(notice.contains("actual material read this turn"));
            assert!(notice.contains("placeholders, never event IDs"));
            assert!(notice.contains("learner's original words"));
            let examples = published["examples"].as_array().unwrap();
            assert_eq!(examples.len(), usize::from(expect_assessment) + usize::from(expect_hypothesis));
            assert_eq!(examples.iter().any(|input| input["operation"] == "assess"), expect_assessment);
            assert_eq!(examples.iter().any(|input| input["nature"] == "hypothesis"), expect_hypothesis);
            let registry = crate::orchestrator::resident_tool_registry().with_tutor_context(Some(&context));
            let registration = registry.registration("tutor.step").unwrap();
            for input in examples {
                validate_schema(&registration.spec.parameters, input, "$").unwrap();
                registration.validate_arguments(&input.to_string()).unwrap();
                let mut actual_request = input.clone();
                if input["operation"] == "assess" {
                    assert_eq!(input["action_ref"], "<pending_assessment_action_ref>");
                    actual_request["action_ref"] = context["learner_context"]["pending_assessment_refs"][0].clone();
                } else {
                    assert_eq!(input["nature"], "hypothesis");
                    assert_eq!(input["fact_refs"], json!(["<learner_expression_fact_ref>"]));
                    actual_request["fact_refs"] = context["learner_context"]["uninterpreted_fact_refs"].clone();
                }
                registration.validate_arguments(&actual_request.to_string()).unwrap();
            }
            let empty = json!({"status":status,"learner_context":{
                "pending_assessment_refs":[],"uninterpreted_fact_refs":[],
            }});
            assert!(crate::tutor::learner_view(&empty).get("tool_input_examples").is_none(),
                "consumed facts remove examples from the current view");
            assert_eq!(serde_json::to_value(&registration.spec).unwrap(),
                serde_json::to_value(crate::tutor::spec_for_context(&empty).unwrap()).unwrap(),
                "availability belongs in the appended learner view, not the tool prefix");
        }
    }
}

#[test]
fn tutor_neutral_request_can_invite_without_rewriting_learner_intent_or_forcing_every_turn() {
    let context = json!({"status":"practice","current_request":"I still mix moving time up with the whole journey."});
    let registry = crate::orchestrator::resident_tool_registry().with_tutor_context(Some(&context));
    let registration = registry.registration("tutor.step").unwrap();
    let examples = registration.spec.parameters["examples"].as_array().unwrap();
    for choice in ["support", "invite", "conclude"] {
        let request = examples.iter().find(|e| e["move"]["participation"]["choice"]==choice).unwrap();
        registration.validate_arguments(&request.to_string()).unwrap();
        let _:crate::tutor::TeachingParticipation=serde_json::from_value(request["move"]["participation"].clone()).unwrap();
        assert_eq!(request["move"]["interaction_intent"],"neutral","teacher initiative is not learner-requested guided inquiry");
        assert_eq!(request["move"]["intent_origin"],"neutral");
        assert_eq!(request["move"]["response_expectation"].is_object(),choice=="invite");
    }
    let mut unchosen=examples.iter().find(|e|e["operation"]=="select").unwrap().clone();
    unchosen["move"].as_object_mut().unwrap().remove("participation");
    let mut historical=unchosen["move"].clone();
    historical.as_object_mut().unwrap().remove("target"); // Historical object-only moves predate source targets.
    assert!(serde_json::from_value::<crate::tutor::TeachingMove>(historical).is_ok(),"old persisted moves remain readable");
    assert!(registration.validate_arguments(&unchosen.to_string()).is_err(),"a new selection must decide who supplies the next inference");
}

#[test]
fn tutor_participation_rationale_stays_private_while_the_invitation_remains_public() {
    let request=crate::tutor::spec().parameters["examples"].as_array().unwrap().iter()
        .find(|e|e["move"]["response_expectation"].is_object()).unwrap().clone();
    let public:Value=serde_json::from_str(&crate::tutor::public_arguments(&request.to_string())).unwrap();
    assert_eq!(public["move"]["response_expectation"],request["move"]["response_expectation"]);
    assert_eq!(public["move"]["prompt"],request["move"]["prompt"]);
    assert!(public["move"].get("participation").is_none());
    assert!(public["move"].get("delivery_text").is_none(), "a prepared draft is public only when delivered");
    assert!(!public.to_string().contains(request["move"]["participation"]["reason"].as_str().unwrap()));
    for malformed in [json!({"operation":"select","participation":request["move"]["participation"]}),
        json!({"operation":"select","move":request["move"].to_string()})] {
        assert!(!crate::tutor::public_arguments(&malformed.to_string()).contains("participation"));
    }
}

#[test]
fn tutor_coach_binding_and_adjustment_stay_private_in_tool_arguments() {
    let mut request = crate::tutor::spec().parameters["examples"].as_array().unwrap().iter()
        .find(|example| example["move"]["response_expectation"].is_object()).unwrap().clone();
    request["move"]["coach_ref"] = json!("coach:source-turn");
    request["move"]["coach_adjustment"] = json!({"reason":"PRIVATE_COACH_ADJUSTMENT", "fact_refs":["user:current"]});
    let public:Value = serde_json::from_str(&crate::tutor::public_arguments(&request.to_string())).unwrap();
    assert_eq!(public["move"]["response_expectation"], request["move"]["response_expectation"]);
    assert_eq!(public["move"]["prompt"], request["move"]["prompt"]);
    for field in ["coach_ref", "coach_adjustment"] { assert!(public["move"].get(field).is_none()); }
    for input in [request.clone(),
        json!({"operation":"select","coach_ref":request["move"]["coach_ref"],"coach_adjustment":request["move"]["coach_adjustment"]}),
        json!({"operation":"select","move":request["move"].to_string()})] {
        let public = crate::tutor::public_arguments(&input.to_string());
        assert!(!public.contains("coach:source-turn") && !public.contains("PRIVATE_COACH_ADJUSTMENT"));
    }
}

#[test]
fn tutor_new_support_allocates_help_and_an_optional_remaining_inference() {
    let context = json!({"status":"practice"});
    let registry = crate::orchestrator::resident_tool_registry().with_tutor_context(Some(&context));
    let registration = registry.registration("tutor.step").unwrap();
    let mut request = registration.spec.parameters["examples"].as_array().unwrap().iter()
        .find(|e| e["move"]["participation"]["choice"] == "support").unwrap().clone();
    registration.validate_arguments(&request.to_string()).unwrap();
    request["move"]["participation"]["learner_inference"] = Value::Null;
    registration.validate_arguments(&request.to_string()).unwrap();
    assert!(request["move"].get("response_expectation").is_none(), "unfinished work is not a forced invitation");
    for field in ["support", "learner_inference"] {
        let mut incomplete = request.clone();
        incomplete["move"]["participation"].as_object_mut().unwrap().remove(field);
        assert!(registration.validate_arguments(&incomplete.to_string()).is_err());
    }
    request["move"].as_object_mut().unwrap().remove("delivery_text");
    assert!(registration.validate_arguments(&request.to_string()).is_err(), "new teaching delivers prepared content rather than an outline");
    let historical = json!({"choice":"support", "reason":"The learner needs this premise."});
    assert!(serde_json::from_value::<crate::tutor::TeachingParticipation>(historical).is_ok(), "existing recorded decisions stay readable");
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
        for remote_field in ["published_book_ref", "page"] {
            assert!(registration.spec.parameters["properties"].get(remote_field).is_none(), "{name}.{remote_field}");
        }
    }
    check("book.text", json!({"lid":"1.1","start_utf16":4}), true);
    check("book.text", json!({"lid":"1.1","start_utf16":-1}), false);
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
        json!({"operation":"write","title":"Page","html":"<p>x</p>"}),
        json!({"operation":"write","title":"Page","html":"<p>x</p>","readable_content":"Text","new_object":true,"based_on":{"presentation_id":"p1","revision":1}}),
        json!({"operation":"write","title":"Page","html":"<p>x</p>","readable_content":"Text","based_on":"candidate-id"}),
        json!({"operation":"patch","candidate_id":"c1","edits":[{"old_text":"old","new_text":"new"}],"source_ref_ids":["source-observed"]}),
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
    let registry = crate::orchestrator::resident_tool_registry();
    let examples = registry.registration("goal.update").unwrap().spec.parameters["examples"].as_array().unwrap();
    for example in examples {
        let invalid_fields: &[(&str, Value)] = match example["operation"].as_str().unwrap() {
            "working" => &[("interpretation", json!("Explain")), ("requirements", json!([])), ("basis_turn_id", json!("t1")), ("basis_quote", json!("quote"))],
            "refine" => &[("focus", json!("Read")), ("next_move", json!("Explain")), ("open_questions", json!([])), ("items", json!([])), ("basis_turn_id", json!("t1")), ("basis_quote", json!("quote"))],
            "revise" => &[("focus", json!("Read")), ("next_move", json!("Explain")), ("open_questions", json!([])), ("items", json!([]))],
            _ => unreachable!(),
        };
        for (field, value) in invalid_fields {
            let mut input = example.clone();
            input[*field] = value.clone();
            check("goal.update", input.clone(), false);
            assert!(serde_json::from_value::<crate::goal::GoalUpdate>(input).is_err());
        }
    }
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
fn tutor_revision_contract_accepts_only_local_text_edits_and_evidence_resubmission() {
    let registry=crate::orchestrator::resident_tool_registry().with_tutor_context(Some(&json!({"status":"practice"})));
    let registration=registry.registration("tutor.step").unwrap();
    let evidence_only=json!({"operation":"revise","draft_ref":"draft:current","edits":[]});
    registration.validate_arguments(&evidence_only.to_string()).unwrap();
    let replacement=json!({"operation":"revise","draft_ref":"draft:current","edits":[
        {"field":"delivery_text","paragraph":2,"new_text":""},
        {"field":"public_prompt","paragraph":1,"new_text":"Can a later route select the retained B?"}
    ]});
    registration.validate_arguments(&replacement.to_string()).unwrap();
    let mut invalid=vec![
        json!({"operation":"revise"}),
        json!({"operation":"revise","draft_ref":"draft:current"}),
        json!({"operation":"revise","draft_ref":"","edits":[]}),
        json!({"operation":"revise","draft_ref":"draft:current","edits":null}),
    ];
    for field in ["field","paragraph","new_text"] {
        let mut request=replacement.clone();
        request["edits"][0].as_object_mut().unwrap().remove(field);
        invalid.push(request);
    }
    for (field,value) in [("field",json!("participation")),("paragraph",json!(0)),("paragraph",json!(-1)),
        ("paragraph",json!(1.5)),("paragraph",json!("1")),("paragraph",Value::Null),
        ("old_text",json!("Unsupported detail.")),("new_text",Value::Null),("path",json!("move.target"))] {
        let mut request=replacement.clone();
        request["edits"][0][field]=value;
        invalid.push(request);
    }
    invalid.push(json!({"operation":"revise","draft_ref":"draft:current","edits":[
        {"field":"delivery_text","old_text":"Unsupported detail.","new_text":"Corrected detail."}
    ]}));
    for (field,value) in [("finish",json!(true)),("move",json!({})),("delivery_move_refs",json!([]))] {
        let mut request=evidence_only.clone();
        request[field]=value;
        invalid.push(request);
    }
    for request in invalid {
        assert!(registration.validate_arguments(&request.to_string()).is_err(),"{request}");
    }
}

#[test]
fn tutor_revision_vocabulary_stays_stable_and_requires_active_text_mode() {
    let plain=json!({"status":"practice"});
    let with_draft=json!({"status":"practice","draft_ref":"draft:current","open_issues":[{"issue":"Missing source evidence"}]});
    for model_facing in [false,true] {
        let schema=|context:&Value| if model_facing {crate::tutor::model_spec_for_context(context)} else {crate::tutor::spec_for_context(context)};
        let first=schema(&plain).unwrap();
        let next=schema(&with_draft).unwrap();
        assert_eq!(serde_json::to_value(first).unwrap(),serde_json::to_value(next).unwrap(),
            "actual draft availability and refs belong in private state, not a changing tool vocabulary");
        for context in [json!({"status":"observing"}),json!({"status":"reference"}),json!({"status":"practice","native_board_candidate_pending":true})] {
            let spec=schema(&context).unwrap();
            assert!(!spec.parameters["properties"]["operation"]["enum"].as_array().unwrap().contains(&json!("revise")));
            assert!(spec.parameters["properties"].get("draft_ref").is_none());
            assert!(spec.parameters["properties"].get("edits").is_none());
            assert!(validate_schema(&spec.parameters,&json!({"operation":"revise","draft_ref":"draft:current","edits":[]}),"$").is_err());
        }
    }
    assert!(crate::tutor::execution_guidance(&plain)["text_revision"].is_object());
    assert!(crate::tutor::execution_guidance(&json!({"status":"practice","native_board_candidate_pending":true}))["text_revision"].is_null());
}

#[test]
fn tutor_revision_edit_text_stays_private_in_public_arguments() {
    let request=json!({"operation":"revise","draft_ref":"draft:current","edits":[
        {"field":"delivery_text","paragraph":2,"new_text":"PRIVATE_REPLACEMENT"},
        {"field":"public_prompt","paragraph":1,"new_text":"PRIVATE_NEW_INVITATION"}
    ]});
    let public:Value=serde_json::from_str(&crate::tutor::public_arguments(&request.to_string())).unwrap();
    assert_eq!(public,json!({"operation":"revise","draft_ref":"draft:current","edits":[{"field":"delivery_text","paragraph":2},{"field":"public_prompt","paragraph":1}]}));
    for malformed in [
        json!({"operation":"revise","edits":"PRIVATE_MALFORMED_PATCH"}),
        json!({"operation":"revise","edits":{"paragraph":2,"new_text":"PRIVATE_REPLACEMENT"}}),
        json!({"operation":"revise","edits":[{"field":"PRIVATE_INVALID_FIELD","paragraph":2,"new_text":"PRIVATE_REPLACEMENT"}]}),
        json!({"operation":"revise","edits":[{"field":"delivery_text","paragraph":"PRIVATE_INVALID_NUMBER","new_text":"PRIVATE_REPLACEMENT"}]}),
        json!({"operation":"revise","edits":[{"field":"delivery_text","old_text":"PRIVATE_OLD_DRAFT","new_text":"PRIVATE_REPLACEMENT"}]}),
    ] {
        assert!(!crate::tutor::public_arguments(&malformed.to_string()).contains("PRIVATE_"));
    }
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
    let mut input = json!({"operation":"select","move":{"move_id":"m1","object_ids":["o1"],"capability":"evaluation","kind":"question","prompt":"Compare","interaction_intent":"guided_inquiry","intent_origin":"current_request","intent_quote":"Compare"}});
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
    let registry = crate::orchestrator::resident_tool_registry();
    let spec = &registry.registration("memory.save").unwrap().spec;
    let examples = spec.parameters["examples"].as_array().unwrap();
    let types: std::collections::BTreeSet<_> = examples.iter().map(|e| e["type"].as_str().unwrap()).collect();
    assert_eq!(types.into_iter().collect::<Vec<_>>(), vec!["context", "highlight", "position", "qa"]);
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
