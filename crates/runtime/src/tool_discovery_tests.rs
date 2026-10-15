use super::*;
use crate::orchestrator::resident_tool_registry;
use serde_json::{json, Value};

fn fixture() -> (ToolRegistry, ModelRuntimeProfile, ToolExposureContext) {
    (
        resident_tool_registry(),
        crate::ModelRuntimeCatalog::default().resolve(
            "test",
            crate::ProviderToolProtocol::Native,
            None,
        ),
        ToolExposureContext {
            content_profile: ContentProfileId::TechnicalLearning,
            permissions: ToolPermissions::default(),
            evidence_state: EvidenceState::Unlocated,
            artifact: ArtifactExposureContext::no_overlay(),
        },
    )
}

fn request(capability: &str, scope: &str, operation: &str) -> Value {
    json!({"task":"Explain the requested material", "required_capabilities":[capability],
        "scope":scope, "operation":operation, "effect_mode":"read_only", "max_results":3})
}

fn scenarios() -> Vec<(&'static str, Value)> {
    let mut intent = request("reader_write", "passage", "mutate_reader");
    intent["effect_mode"] = json!("reader_mutation_explicitly_requested");
    vec![
        (
            "matched",
            request("structural_index", "document", "summarize"),
        ),
        (
            "evidence_required",
            request("source_presentation", "passage", "read_source"),
        ),
        (
            "locator_required",
            request("source_read", "passage", "read_source"),
        ),
        (
            "effect_not_authorized",
            request("reader_write", "passage", "mutate_reader"),
        ),
        ("intent_required", intent),
        (
            "profile_mismatch",
            request("source_read", "document", "read_source"),
        ),
        ("no_result", request("discovery", "corpus", "compare")),
    ]
}

#[test]
fn tool_search_to7_snapshot() {
    let (registry, model, context) = fixture();
    let book = read_tools::Book::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../book-tool-contracts/fixtures/example-book"
    ))
    .unwrap();
    let policy = registry.registration("tool.search").unwrap().output_policy;
    let mut snapshots = serde_json::Map::new();
    for (name, input) in scenarios() {
        let mut state = ToolExposureState::default();
        let plan = ToolExposurePlan::build(&registry, &model, &context, &state);
        let outcome =
            search_and_activate(&input.to_string(), &context, &plan, &registry, &mut state)
                .unwrap();
        let result = serde_json::to_value(&outcome).unwrap();
        let delivered = crate::tool_result::project_tool_result(
            "tool.search",
            &input.to_string(),
            &result.to_string(),
            policy,
            48 * 1024,
            &book,
        );
        assert!(
            !delivered.truncated,
            "normal scenario {name} must preserve recovery"
        );
        assert_eq!(delivered.model_body, result);
        snapshots.insert(
            name.into(),
            json!({"input":input,"result":result,"delivered":delivered.model_body}),
        );
    }
    if let Ok(path) = std::env::var("TO7_SNAPSHOT") {
        std::fs::write(
            path,
            serde_json::to_vec_pretty(&json!({
                "spec":registry.registration("tool.search").unwrap().spec,
                "scenarios":snapshots
            }))
            .unwrap(),
        )
        .unwrap();
    }
}

#[test]
fn tool_search_to7_budget_observation() {
    let (registry, model, context) = fixture();
    let book = read_tools::Book::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../book-tool-contracts/fixtures/example-book"
    ))
    .unwrap();
    let policy = registry.registration("tool.search").unwrap().output_policy;
    let mut state = ToolExposureState::default();
    let plan = ToolExposurePlan::build(&registry, &model, &context, &state);
    let mut input = request("memory_read", "passage", "explain");
    input["required_capabilities"] = json!([
        "semantic_evidence",
        "synthesis",
        "memory_read",
        "profile_read",
        "presentation_authoring"
    ]);
    input["max_results"] = json!(6);
    let outcome =
        search_and_activate(&input.to_string(), &context, &plan, &registry, &mut state).unwrap();
    let raw = serde_json::to_value(&outcome).unwrap();
    let delivered = crate::tool_result::project_tool_result(
        "tool.search",
        &input.to_string(),
        &raw.to_string(),
        policy,
        48 * 1024,
        &book,
    );
    assert!(
        serde_json::to_vec(&delivered.model_body).unwrap().len() <= policy.max_model_body_bytes
    );
    if let Ok(directory) = std::env::var("TO7_REQUESTS") {
        std::fs::write(std::path::Path::new(&directory).join("budget-observation.json"), serde_json::to_vec_pretty(&json!({
            "input":input,"raw":raw,"delivered":delivered.model_body,"truncated":delivered.truncated,
            "continuation":delivered.continuation,"limit":policy.max_model_body_bytes
        })).unwrap()).unwrap();
    }
}

#[test]
fn tool_search_to11_bounded_discovery_refines_through_the_real_parser() {
    let (registry, model, context) = fixture();
    let book = read_tools::Book::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../book-tool-contracts/fixtures/example-book"
    ))
    .unwrap();
    let mut state = ToolExposureState::default();
    let plan = ToolExposurePlan::build(&registry, &model, &context, &state);
    let mut input = request("semantic_evidence", "passage", "explain");
    input["required_capabilities"] = json!([
        "semantic_evidence",
        "synthesis",
        "memory_read",
        "profile_read",
        "presentation_authoring",
        "source_presentation"
    ]);
    input["max_results"] = json!(6);
    let raw = serde_json::to_value(
        search_and_activate(&input.to_string(), &context, &plan, &registry, &mut state).unwrap(),
    )
    .unwrap();
    let policy = registry.registration("tool.search").unwrap().output_policy;
    let mut snapshots = Vec::new();
    for budget in [4096, 2500] {
        let draft = crate::tool_result::project_tool_result(
            "tool.search",
            &input.to_string(),
            &raw.to_string(),
            policy,
            budget,
            &book,
        );
        assert!(draft.truncated);
        assert!(draft.model_body_json().len() <= budget);
        for field in ["unmet_capabilities", "blocked", "activated"] {
            assert_eq!(draft.model_body[field], raw[field]);
        }
        if let Some(crate::tool_result::ToolContinuation::RefineCall {
            tool, arguments, ..
        }) = &draft.continuation
        {
            assert_eq!(tool, "tool.search");
            registry
                .registration(tool)
                .unwrap()
                .validate_arguments(&arguments.to_string())
                .unwrap();
            search_and_activate(
                &arguments.to_string(),
                &context,
                &plan,
                &registry,
                &mut state,
            )
            .unwrap();
        }
        snapshots.push(json!({"budget":budget,"input":input,"raw":raw,"delivered":draft.model_body,"continuation":draft.continuation}));
    }
    // With no blocked capability, a dropped match yields a real smaller request.
    input["required_capabilities"].as_array_mut().unwrap().pop();
    let raw = serde_json::to_value(
        search_and_activate(&input.to_string(), &context, &plan, &registry, &mut state).unwrap(),
    )
    .unwrap();
    let draft = crate::tool_result::project_tool_result(
        "tool.search",
        &input.to_string(),
        &raw.to_string(),
        policy,
        1800,
        &book,
    );
    let Some(crate::tool_result::ToolContinuation::RefineCall {
        tool, arguments, ..
    }) = &draft.continuation
    else {
        panic!("smaller discovery required")
    };
    assert_eq!(arguments["max_results"], 1);
    assert_eq!(
        arguments["required_capabilities"].as_array().unwrap().len(),
        1
    );
    registry
        .registration(tool)
        .unwrap()
        .validate_arguments(&arguments.to_string())
        .unwrap();
    search_and_activate(
        &arguments.to_string(),
        &context,
        &plan,
        &registry,
        &mut state,
    )
    .unwrap();
    snapshots.push(json!({"budget":1800,"input":input,"raw":raw,"delivered":draft.model_body,"continuation":draft.continuation}));
    if let Ok(path) = std::env::var("TO11_DISCOVERY") {
        std::fs::write(path, serde_json::to_vec_pretty(&snapshots).unwrap()).unwrap();
    }
}

#[test]
fn tool_search_to7_public_recovery_is_capability_scoped_and_executable() {
    let (registry, model, context) = fixture();
    for (name, input) in scenarios() {
        let mut state = ToolExposureState::default();
        let plan = ToolExposurePlan::build(&registry, &model, &context, &state);
        let outcome =
            search_and_activate(&input.to_string(), &context, &plan, &registry, &mut state)
                .unwrap();
        let value = serde_json::to_value(&outcome).unwrap();
        for internal in ["task_need", "capability_plan", "request_audit"] {
            assert!(value.get(internal).is_none(), "{name}: {internal}");
        }
        let serialized = value.to_string();
        for entry in &plan.entries {
            if entry.disposition == ToolExposureDisposition::Hidden {
                assert!(
                    !serialized.contains(&entry.name),
                    "{name}: leaked {}",
                    entry.name
                );
            }
        }
        if name == "matched" {
            assert_eq!(value["unmet_capabilities"], json!([]));
            assert_eq!(value["blocked"], json!([]));
            let card = registry.routing_card("book.structure").unwrap();
            assert_eq!(value["matches"][0]["use_when"], json!(card.use_when));
            assert_eq!(value["matches"][0]["avoid_when"], json!(card.avoid_when));
            continue;
        }
        let reason = match name {
            "intent_required" => "explicit_effect_intent_required",
            "no_result" => "no_matching_capability",
            other => other,
        };
        assert_eq!(
            value["unmet_capabilities"], input["required_capabilities"],
            "{name}"
        );
        assert_eq!(value["blocked"][0]["reason"], reason, "{name}");
        assert!(outcome.matches.is_empty(), "{name}");
        assert_eq!(
            state.activated_names().count(),
            0,
            "suggestions must not activate tools"
        );
        for block in &outcome.blocked {
            assert_eq!(block.capability, outcome.unmet_capabilities[0]);
            assert!(!block.message.is_empty());
            if name != "no_result" {
                assert!(!block.next_searches.is_empty(), "{name}");
            }
            for recovery in &block.next_searches {
                let args = serde_json::to_string(recovery).unwrap();
                registry
                    .registration("tool.search")
                    .unwrap()
                    .validate_arguments(&args)
                    .unwrap();
                let mut recovery_state = state.clone();
                let recovered =
                    search_and_activate(&args, &context, &plan, &registry, &mut recovery_state)
                        .unwrap();
                assert!(!recovered.matches.is_empty(), "{name}: {args}");
                assert!(recovered.unmet_capabilities.is_empty());
                assert!(recovered
                    .matches
                    .iter()
                    .all(|matched| matched.effect_mode == ToolEffect::ReadOnly));
            }
        }
    }
}

#[test]
fn tool_search_to7_recovery_tracks_evidence_permissions_and_sampling() {
    let (registry, model, mut context) = fixture();
    context.evidence_state = EvidenceState::CurrentAnchorOnly;
    let mut state = ToolExposureState::default();
    let plan = ToolExposurePlan::build(&registry, &model, &context, &state);
    let input = request("source_presentation", "passage", "read_source").to_string();
    let outcome = search_and_activate(&input, &context, &plan, &registry, &mut state).unwrap();
    assert_eq!(
        outcome.blocked[0].next_searches[0].required_capabilities,
        vec![ToolCapability::SourceRead]
    );

    context.evidence_state = EvidenceState::KnownLids;
    let plan = ToolExposurePlan::build(&registry, &model, &context, &state);
    let outcome = search_and_activate(&input, &context, &plan, &registry, &mut state).unwrap();
    assert_eq!(outcome.matches[0].name, "source.present");
    assert!(outcome.blocked.is_empty());

    context.permissions.allow_source_presentation = false;
    context.permissions.allow_reader_read = false;
    context.permissions.allow_memory_write = false;
    let plan = ToolExposurePlan::build(&registry, &model, &context, &state);
    for input in [
        input,
        request("memory_write", "document", "explain").to_string(),
    ] {
        let outcome = search_and_activate(&input, &context, &plan, &registry, &mut state).unwrap();
        assert_eq!(outcome.blocked[0].reason, "permission_denied");
        assert!(outcome.blocked[0].next_searches.is_empty());
        let json = serde_json::to_string(&outcome).unwrap();
        assert!(!json.contains("source.present") && !json.contains("memory.save"));
    }
    let outcome = search_and_activate(
        &request("reader_write", "passage", "mutate_reader").to_string(),
        &context,
        &plan,
        &registry,
        &mut state,
    )
    .unwrap();
    assert!(outcome
        .blocked
        .iter()
        .all(|block| block.next_searches.is_empty()));
    assert!(!serde_json::to_string(&outcome)
        .unwrap()
        .contains("reader.state"));

    context.evidence_state = EvidenceState::Unlocated;
    let mut plan = ToolExposurePlan::build(&registry, &model, &context, &state);
    plan.exclude_for_sampling(&["book.structure", "book.search_text"]);
    let outcome = search_and_activate(
        &request("source_read", "passage", "read_source").to_string(),
        &context,
        &plan,
        &registry,
        &mut state,
    )
    .unwrap();
    assert_eq!(outcome.blocked[0].reason, "locator_required");
    assert!(
        outcome.blocked[0].next_searches.is_empty(),
        "excluded tools cannot be recommended"
    );

    let mut tiny_model = model;
    tiny_model.tool_schema_budget_bytes = 1;
    let plan = ToolExposurePlan::build(&registry, &tiny_model, &context, &state);
    let outcome = search_and_activate(
        &request("source_read", "passage", "read_source").to_string(),
        &context,
        &plan,
        &registry,
        &mut state,
    )
    .unwrap();
    let json = serde_json::to_string(&outcome).unwrap();
    assert_eq!(outcome.blocked[0].reason, "capability_unavailable");
    assert!(outcome.blocked[0].next_searches.is_empty());
    for internal in ["runtime_hidden", "schema_budget", "sampling_excluded"] {
        assert!(!json.contains(internal));
    }
}

#[test]
fn tool_search_to7_result_limit_is_not_a_capability_failure() {
    let (registry, model, context) = fixture();
    let mut state = ToolExposureState::default();
    let plan = ToolExposurePlan::build(&registry, &model, &context, &state);
    let mut input = request("structural_index", "document", "summarize");
    input["required_capabilities"] = json!(["structural_index", "memory_read"]);
    input["max_results"] = json!(1);
    let outcome =
        search_and_activate(&input.to_string(), &context, &plan, &registry, &mut state).unwrap();
    assert_eq!(outcome.matches.len(), 1);
    assert_eq!(outcome.blocked.len(), 1);
    assert_eq!(outcome.blocked[0].reason, "result_limit");
    let recovered = search_and_activate(
        &serde_json::to_string(&outcome.blocked[0].next_searches[0]).unwrap(),
        &context,
        &plan,
        &registry,
        &mut state,
    )
    .unwrap();
    assert!(recovered.unmet_capabilities.is_empty());
}

#[test]
fn tool_search_to7_examples_preserve_runtime_authority() {
    let (registry, model, mut context) = fixture();
    context.evidence_state = EvidenceState::CurrentAnchorOnly;
    let spec = &registry.registration("tool.search").unwrap().spec;
    let examples = spec.parameters["examples"].as_array().unwrap();
    assert_eq!(examples.len(), 4);
    for example in examples {
        let mut state = ToolExposureState::default();
        let plan = ToolExposurePlan::build(&registry, &model, &context, &state);
        let outcome =
            search_and_activate(&example.to_string(), &context, &plan, &registry, &mut state)
                .unwrap();
        if example["effect_mode"] == "read_only" {
            assert!(
                !outcome.matches.is_empty(),
                "{example}: {:?}",
                outcome.blocked
            );
        } else {
            assert!(
                outcome.matches.is_empty(),
                "model example is not user authority"
            );
            assert_eq!(outcome.blocked[0].reason, "explicit_effect_intent_required");
            seed_turn_tool_activations(
                &classify_turn_intent("记一条笔记"),
                &registry,
                &context,
                &mut state,
            );
            let plan = ToolExposurePlan::build(&registry, &model, &context, &state);
            let outcome =
                search_and_activate(&example.to_string(), &context, &plan, &registry, &mut state)
                    .unwrap();
            assert_eq!(outcome.matches.len(), 1);
            assert_eq!(outcome.matches[0].name, "reader.note");
        }
    }
    let mut state = ToolExposureState::default();
    let plan = ToolExposurePlan::build(&registry, &model, &context, &state);
    let mut input = request("structural_index", "document", "summarize");
    input["task"] = json!("先梳理全书的论证主线，再说明各部分如何连接；不要跳转页面或保存笔记。");
    let outcome =
        search_and_activate(&input.to_string(), &context, &plan, &registry, &mut state).unwrap();
    assert_eq!(outcome.matches[0].name, "book.structure");
    assert!(outcome
        .matches
        .iter()
        .all(|matched| matched.effect_mode == ToolEffect::ReadOnly));
}
