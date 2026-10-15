use super::*;
use crate::{
    agent_prompt::{policy_modules_for_tools, policy_modules_for_tools_with_presentation},
    AgentRequestPlan, Message, ModelRuntimeProfile, ProviderToolProtocol,
};

fn request(phase: PresentationPhase, needs: &[PresentationNeed]) -> AgentRequestPlan {
    let tools = [crate::presentation_author::spec()];
    let guidance = PresentationGuidance {
        phase,
        needs: needs.to_vec(),
    };
    let modules = policy_modules_for_tools_with_presentation(&tools, Some(&guidance));
    AgentRequestPlan::for_agent_turn_with_modules(
        ModelRuntimeProfile::fallback("ex12-test", ProviderToolProtocol::Native),
        &[
            Message::system(crate::agent_prompt::BASE_INSTRUCTIONS),
            Message::user("Explain this relationship."),
        ],
        &tools,
        &modules,
    )
}

fn reference_names(plan: &AgentRequestPlan) -> Vec<&str> {
    plan.instruction_assets
        .iter()
        .filter_map(|asset| {
            asset
                .asset_id
                .strip_prefix("resident-agent.presentation.reference.")
        })
        .collect()
}

fn assert_shared_contract(plan: &AgentRequestPlan, phase: &str) {
    let text = &plan.instructions;
    // Each normative paragraph is supplied once, not copied by phase or policy.
    for paragraph in common_body(COMMON).split("\n\n").skip(1) {
        assert_eq!(
            text.matches(paragraph).count(),
            1,
            "duplicated/missing core: {paragraph}"
        );
    }
    assert!(!text.contains("name: presentation-method"));
    for contract in [
        "source.present",
        "button[data-source-ref]",
        "exact version",
        "Candidate IDs are valid only in the run",
        "self-contained",
        "asset_refs",
        "320×420 touch",
        "640×240 touch",
        "960×720 mouse",
        "subsequent model sampling",
        "old previews do not transfer",
        "independently substitute",
        "read_selector",
    ] {
        assert!(
            text.contains(contract),
            "missing shared contract: {contract}"
        );
    }
    let phases: Vec<_> = plan
        .instruction_assets
        .iter()
        .filter_map(|a| {
            a.asset_id
                .strip_prefix("resident-agent.presentation.phase.")
        })
        .collect();
    assert_eq!(phases, [phase]);
    let ids: std::collections::HashSet<_> = plan
        .instruction_assets
        .iter()
        .map(|a| &a.asset_id)
        .collect();
    assert_eq!(ids.len(), plan.instruction_assets.len());
    for asset in plan
        .instruction_assets
        .iter()
        .filter(|a| a.asset_id.contains("presentation"))
    {
        assert_eq!(asset.revision, REVISION);
    }
    for obsolete in [
        "ex11.v11",
        "Begin the page with one question sentence",
        "Δw=-ηL′",
        "semantic_state:2,transition_progress:0.35",
        "single compact result text",
    ] {
        assert!(
            !text.contains(obsolete),
            "obsolete general instruction: {obsolete}"
        );
    }
}

#[test]
fn ordinary_answer_never_loads_presentation_even_with_supplied_context() {
    let guidance = PresentationGuidance {
        phase: PresentationPhase::Local,
        needs: vec![PresentationNeed::Manim],
    };
    for modules in [
        policy_modules_for_tools(&[]),
        policy_modules_for_tools_with_presentation(&[], Some(&guidance)),
    ] {
        let plan = AgentRequestPlan::for_agent_turn_with_modules(
            ModelRuntimeProfile::fallback("ex12-test", ProviderToolProtocol::Native),
            &[Message::user("Explain this quotation.")],
            &[],
            &modules,
        );
        assert!(!plan.instructions.contains("Presentation method"));
        assert!(!plan
            .instruction_assets
            .iter()
            .any(|a| a.asset_id.contains("presentation")));
    }
}

#[test]
fn global_design_gets_directory_but_no_engineering_references() {
    let plan = request(
        PresentationPhase::Global,
        &[PresentationNeed::Manim, PresentationNeed::Konva],
    );
    assert_shared_contract(&plan, "global");
    assert!(reference_names(&plan).is_empty());
    assert!(plan
        .instructions
        .contains("Presentation capability directory"));
    // Include serialized tool definitions: moving long guidance into a schema
    // would still leak it into actual requests.
    let tool_text = plan
        .tools
        .iter()
        .map(|tool| format!("{} {}", tool.description, tool.parameters))
        .collect::<Vec<_>>()
        .join("\n");
    let wire_text = format!("{} {tool_text}", plan.instructions);
    for implementation in [
        "requestVideoFrameCallback",
        "HAVE_CURRENT_DATA",
        "hitFunc",
        "registerStateRestorer",
        "160px-high",
        "180s timeout",
    ] {
        assert!(
            !wire_text.contains(implementation),
            "global leaked {implementation}"
        );
    }
}

#[test]
fn static_local_and_local_edit_do_not_require_interaction_or_seek() {
    for needs in [
        vec![],
        vec![PresentationNeed::Editing, PresentationNeed::StaticPlot],
    ] {
        let plan = request(PresentationPhase::Local, &needs);
        assert_shared_contract(&plan, "local");
        for interactive in [
            "presentationScene",
            "registerStateReader",
            "requestVideoFrameCallback",
            "seek(",
            "position slider",
        ] {
            assert!(
                !plan.instructions.contains(interactive),
                "static leaked {interactive}"
            );
        }
        if needs.is_empty() {
            assert!(reference_names(&plan).is_empty());
        } else {
            assert_eq!(reference_names(&plan), ["editing", "static-plot"]);
            assert!(plan
                .instructions
                .contains("old_text must appear exactly once"));
            assert!(plan.instructions.contains("plt, fig, ax"));
        }
    }
}

#[test]
fn continuous_scene_brings_state_contract_once_and_no_media() {
    let plan = request(
        PresentationPhase::Local,
        &[
            PresentationNeed::ContinuousScene,
            PresentationNeed::State,
            PresentationNeed::ContinuousScene,
        ],
    );
    assert_shared_contract(&plan, "local");
    assert_eq!(reference_names(&plan), ["state", "continuous-scene"]);
    for contract in [
        "registerStateReader",
        "registerStateRestorer",
        "saved.values.page",
        "state_contract",
        "JSON types",
        "read-only window.presentation.initialState",
        "commitState()",
        "semantic_state",
        "transition_progress",
        "ONE OBJECT",
        "playing=false",
        "move backward, then repeat",
        "Zero-magnitude",
        "same paused positioning path",
    ] {
        assert!(
            plan.instructions.contains(contract),
            "missing scene contract: {contract}"
        );
    }
    assert!(!plan.instructions.contains("requestVideoFrameCallback"));
    assert!(!plan.instructions.contains("hitFunc"));
}

#[test]
fn media_local_includes_decoding_paused_restore_and_resource_contracts() {
    let plan = request(PresentationPhase::Local, &[PresentationNeed::Manim]);
    assert_shared_contract(&plan, "local");
    assert_eq!(
        reference_names(&plan),
        ["state", "continuous-scene", "manim"]
    );
    for contract in [
        "PresentationAnimation(Scene)",
        "asset_ref",
        "poster_path",
        "no autoplay",
        "requestVideoFrameCallback",
        "HAVE_CURRENT_DATA",
        "Promise<void>",
        "saved.values.page",
        "160px-high",
        "cancel the playback callback",
        "same logical fraction",
        "active_demo",
    ] {
        assert!(
            plan.instructions.contains(contract),
            "missing media contract: {contract}"
        );
    }
    assert!(!plan.instructions.contains("hitFunc"));
}

#[test]
fn konva_does_not_imply_timeline_and_custom_state_is_explicit() {
    let plan = request(PresentationPhase::Local, &[PresentationNeed::Konva]);
    assert_shared_contract(&plan, "local");
    assert_eq!(reference_names(&plan), ["konva"]);
    for contract in [
        "hitFunc",
        "getRelativePointerPosition",
        "ResizeObserver",
        "44px",
        "libraries:[",
    ] {
        assert!(
            plan.instructions.contains(contract),
            "missing Konva contract: {contract}"
        );
    }
    assert!(!plan.instructions.contains("presentationScene"));
    let interactive = request(
        PresentationPhase::Local,
        &[PresentationNeed::Konva, PresentationNeed::State],
    );
    assert_eq!(reference_names(&interactive), ["state", "konva"]);
}

#[test]
fn review_replaces_local_responsibility_and_keeps_selected_contracts() {
    let plan = request(
        PresentationPhase::Review,
        &[PresentationNeed::Manim, PresentationNeed::Editing],
    );
    assert_shared_contract(&plan, "review");
    assert_eq!(
        reference_names(&plan),
        ["editing", "state", "continuous-scene", "manim"]
    );
    assert!(plan
        .instructions
        .contains("重复前提、概念跳跃、对象含义变化或图文分离"));
    assert!(!plan.instructions.contains("Presentation phase: local"));
}

#[test]
fn selection_order_and_provider_protocol_do_not_change_guidance() {
    let forward = request(
        PresentationPhase::Local,
        &[PresentationNeed::Manim, PresentationNeed::Editing],
    );
    let reverse = request(
        PresentationPhase::Local,
        &[
            PresentationNeed::Editing,
            PresentationNeed::Manim,
            PresentationNeed::State,
        ],
    );
    assert_eq!(forward.instructions, reverse.instructions);
    assert_eq!(forward.instruction_assets, reverse.instruction_assets);
    let guidance = PresentationGuidance {
        phase: PresentationPhase::Local,
        needs: vec![PresentationNeed::Manim, PresentationNeed::Editing],
    };
    let react = AgentRequestPlan::for_agent_turn_with_modules(
        ModelRuntimeProfile::fallback("ex12-test", ProviderToolProtocol::ReAct),
        &[
            Message::system(crate::agent_prompt::BASE_INSTRUCTIONS),
            Message::user("Explain this relationship."),
        ],
        &forward.tools,
        &policy_modules_for_tools_with_presentation(&forward.tools, Some(&guidance)),
    );
    assert_eq!(react.instructions, forward.instructions);
    assert_eq!(react.instruction_assets, forward.instruction_assets);
}

#[test]
fn unphased_resident_preserves_all_contracts_without_claiming_phase_switching() {
    let tools = [crate::presentation_author::spec()];
    let actual = policy_modules_for_tools(&tools);
    assert_eq!(
        actual,
        policy_modules_for_tools_with_presentation(&tools, None)
    );
    assert!(actual.iter().all(|m| !m.asset_id.contains(".phase.")));
    for (_, name, _) in REFERENCES {
        assert!(actual
            .iter()
            .any(|m| m.asset_id == format!("resident-agent.presentation.reference.{name}")));
    }
    assert_eq!(
        actual
            .iter()
            .filter(|m| m.asset_id == "resident-agent.skill.presentation-method")
            .count(),
        1
    );
    assert!(tools[0].parameters["properties"]["operation"]["enum"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("prepare")));
}

#[test]
fn common_frontmatter_is_removed_for_both_line_endings() {
    assert_eq!(common_body("---\nname: test\n---\nbody"), "body");
    assert_eq!(common_body("---\r\nname: test\r\n---\r\nbody"), "body");
}

#[test]
fn common_modules_are_identical_across_sampling_selections() {
    let mut stable = None;
    for guidance in [
        None,
        Some(PresentationGuidance { phase: PresentationPhase::Global, needs: vec![] }),
        Some(PresentationGuidance { phase: PresentationPhase::Local, needs: vec![PresentationNeed::Manim] }),
        Some(PresentationGuidance { phase: PresentationPhase::Review, needs: vec![PresentationNeed::Editing] }),
    ] {
        let (selected, common): (Vec<_>, Vec<_>) = modules(guidance.as_ref()).into_iter().partition(is_sampling_module);
        assert_eq!(selected, selected_modules(guidance.as_ref()));
        assert_eq!(common, *stable.get_or_insert(common.clone()));
        assert!(common.iter().all(|m| !m.text.contains("Presentation phase:")));
    }
}
