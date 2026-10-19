use super::*;
use crate::presentation_author::{AuthorRequest, AuthorResult, PreviewImage};
use serde_json::{json, Value};

fn saved_source_read(messages: &[Message], id: &str) -> Value {
    let result: Value = serde_json::from_str(tool_result(messages, id)).unwrap();
    result["continuation"]["arguments"].clone()
}

#[test]
fn cc1_tool_policy_changes_preserve_sent_message_prefix() {
    let b = book();
    let registry = resident_tool_registry();
    let profile = ModelRuntimeProfile::fallback("cc1", ProviderToolProtocol::Native);
    let mut fragments = ContextFragmentLedger::default();
    let mut exposure = ToolExposureState::default();
    let mut messages = new_session();
    messages.push(Message::user("Explain the observed passage, then prepare a presentation."));
    let mut plans = Vec::new();
    for step in 0..3 {
        if step == 2 { exposure.activate("presentation.author"); }
        let (_, plan) = build_sample_request(
            &messages, &mut fragments, &b, &mut ActiveToolResultLedger::default(), None,
            COMPACTION_CONSUMPTION_WRAPPER, &registry, &profile, ToolPermissions::default(),
            &exposure, ArtifactExposureContext::no_overlay(),
            if step == 0 { EvidenceState::Unlocated } else { EvidenceState::KnownLids },
            &[], None, false, None,
        ).unwrap();
        plans.push(plan);
        let id = format!("cc1-{step}");
        messages.push(Message { provider_continuation: None, role: Role::Assistant, content: None,
            tool_calls: vec![call(&id, "book.text", r#"{"lid":"1.1"}"#)], tool_call_id: None });
        messages.push(Message { provider_continuation: None, role: Role::Tool, content: Some("observed passage".into()),
            tool_calls: vec![], tool_call_id: Some(id) });
    }
    assert!(plans[1].ordered_messages().iter().any(|m| m.content.as_deref().is_some_and(|s| s.contains("Source presentation:"))));
    assert!(plans[2].tools.iter().any(|t| t.name == "presentation.author"));
    for pair in plans.windows(2) {
        assert_eq!(pair[0].instructions, pair[1].instructions, "tool policy rewrote the first system message");
        let (before, _) = crate::native_chat_request_projection("cc1", &pair[0]);
        let (after, _) = crate::native_chat_request_projection("cc1", &pair[1]);
        let prior = before["messages"].as_array().unwrap();
        assert_eq!(prior.as_slice(), &after["messages"].as_array().unwrap()[..prior.len()], "native changed sent history");
        let before = crate::react_chat_request_projection("cc1", &pair[0]);
        let after = crate::react_chat_request_projection("cc1", &pair[1]);
        // ReAct's tool protocol message necessarily changes when tools change.
        let prior = before["messages"].as_array().unwrap();
        assert_eq!(&prior[1..], &after["messages"].as_array().unwrap()[1..prior.len()]);
    }
}

#[test]
fn cc1_open_presentation_goal_seeds_capability_without_old_turn_authority() {
    use crate::goal::{GoalStatus, ResidentGoal};
    for (label, status, request, unavailable, expected) in [
        ("open", GoalStatus::Open, "制作演示页", false, true),
        ("completed", GoalStatus::Completed, "制作演示页", false, false),
        ("cancelled", GoalStatus::Cancelled, "制作演示页", false, false),
        ("content", GoalStatus::Open, "解释这个概念", false, false),
        ("unavailable", GoalStatus::Open, "制作演示页", true, false),
    ] {
        let b = book();
        let mut port = AuthoringPort::new(&b, &format!("cc1-goal-{label}"));
        if unavailable { port.unavailable_reason = Some("probe_failed"); }
        let adapter = RequestPlanRecordingAdapter::new(vec![turn_final("No page has been delivered.")], vec![]);
        let mut context = RunContext::new(new_session(), OuterConfig { max_turns: Some(1), ..Default::default() }, adapter.model_runtime_profile());
        let mut goal = ResidentGoal::new("g".into(), "old-turn".into(), request.into());
        goal.status = status;
        context.goal = Some(goal);
        let _ = execute(&mut port, &b, &adapter, &mut context);
        let plans = adapter.seen_plans.borrow();
        let first = plans.first().expect("first model decision");
        assert_eq!(first.tools.iter().any(|t| t.name == "presentation.author"), expected, "{label}");
        assert!(!first.tools.iter().any(|t| t.name == "source.present" || t.name == "reader.gotoLid"), "old Goal restored current-turn authority");
        assert_eq!((port.writes, port.deliveries), (0, 0));
    }
}

#[test]
fn teaching_presentation_exposure_waits_for_accepted_design() {
    let b = book();
    let registry = resident_tool_registry();
    let profile = ModelRuntimeProfile::fallback("teaching-presentation-exposure", ProviderToolProtocol::Native);
    let mut exposure = ToolExposureState::default();
    // A page request or tool search may already have activated this capability.
    exposure.activate("presentation.author");
    let accepted = |kind| json!({"status":"active","current_design":{
        "design_ref":"design:accepted","design":{"representation":{"kind":kind}}
    }});
    for (label, tutor, expected) in [
        ("before design", Some(json!({"status":"active","current_design":null})), false),
        ("text design", Some(accepted("text")), false),
        ("accepted presentation", Some(accepted("presentation")), true),
        ("outside teaching", Some(json!({"status":"active","outside_current_request":true})), true),
        ("ordinary presentation", None, true),
    ] {
        let mut messages = new_session();
        messages.push(Message::user("Embedding 在哪里用，怎么用？给我一个演示页。"));
        let (tools, plan) = build_sample_request(
            &messages, &mut ContextFragmentLedger::default(), &b,
            &mut ActiveToolResultLedger::default(), None, COMPACTION_CONSUMPTION_WRAPPER,
            &registry, &profile, ToolPermissions::default(), &exposure,
            ArtifactExposureContext::no_overlay(), EvidenceState::KnownLids,
            &[], None, false, tutor.as_ref(),
        ).unwrap();
        assert!(tools.is_visible("presentation.author"), "{label}: existing pages remain readable");
        let author = plan.tools.iter().find(|tool| tool.name == "presentation.author").unwrap();
        let operations = &author.parameters["properties"]["operation"]["enum"];
        if expected {
            assert_eq!(operations, &crate::presentation_author::spec().parameters["properties"]["operation"]["enum"], "{label}");
        } else {
            assert_eq!(operations, &json!(["read", "search"]), "{label}");
            assert_eq!(author.parameters["anyOf"].as_array().unwrap().len(), 2);
        }
        assert_eq!(selection_state(&plan).is_some_and(|state| state["active"] == true), expected, "{label}");
        assert_eq!(plan.instruction_assets.iter().any(|asset|
            asset.asset_id.starts_with("resident-agent.presentation.phase.")), expected, "{label}");
    }
}

#[test]
fn teaching_presentation_inspection_before_design_keeps_mutations_blocked() {
    for kind in [None, Some("text")] {
        for mutation in [
            json!({"operation":"prepare","phase":"local","needs":[]}),
            // The real repair attempted write with assets/based_on. Report the
            // unavailable operation before misleading the caller into deleting fields.
            json!({"operation":"write","title":"Unplanned","html":"<p>Unplanned</p>","readable_content":"Unplanned",
                "assets":[],"based_on":{"presentation_id":"already-delivered","revision":5}}),
            json!({"operation":"deliver","candidate_id":"c1"}),
        ] {
            let b = book();
            let mut port = AuthoringPort::new(&b, &format!("inspect-before-design-{}-{}", kind.unwrap_or("none"), mutation["operation"].as_str().unwrap()));
            let reference = json!({"presentation_id":"already-delivered","revision":5});
            let adapter = RequestPlanRecordingAdapter::new(vec![
                discover(),
                author("read", json!({"operation":"read","reference":reference,"file":"readable_content"})),
                author("search", json!({"operation":"search","reference":reference,"query":"comparison"})),
                author("mutation", mutation.clone()),
                turn_final("No revised page was delivered."),
            ], vec![]);
            let mut context = RunContext::new(new_session(), OuterConfig { max_turns:Some(5), ..Default::default() }, adapter.model_runtime_profile());
            context.tutor = Some(json!({"status":"active","learner_context":{},"current_design":kind.map(|kind|
                json!({"design_ref":"design:text","design":{"representation":{"kind":kind}}}))}));
            let _ = execute(&mut port, &b, &adapter, &mut context);
            assert_eq!(port.inspections, ["read", "search"], "{kind:?}: both existing-page operations must reach the host");
            assert_eq!((port.writes, port.deliveries), (0, 0), "inspection must not grant mutation authority");
            assert!(context.presentation_authoring.is_none(), "prepare must remain blocked");
            assert!(tool_result(&context.messages, "mutation").contains("PRESENTATION_ARGUMENTS_INVALID"));
            let plans = adapter.seen_plans.borrow();
            let feedback = plans[4].input.iter().find(|message| message.tool_call_id.as_deref() == Some("mutation")).unwrap();
            let feedback: Value = serde_json::from_str(feedback.content.as_deref().unwrap()).unwrap();
            let message = feedback["model_body"]["message"].as_str().unwrap();
            assert!(message.starts_with(&format!("operation={} is unavailable.", mutation["operation"].as_str().unwrap())), "{message}");
            assert!(message.contains("tutor.step") && message.contains("operation=design") && message.contains("design.representation.kind=presentation"), "{message}");
            assert!(!message.contains("$.assets") && !message.contains("$.based_on"), "{message}");
            assert!(plans.iter().all(|plan| !plan.instruction_assets.iter().any(|asset|
                asset.asset_id.starts_with("resident-agent.presentation.phase."))));
        }
    }
}

#[test]
fn teaching_presentation_reuses_focused_design_and_preserves_review() {
    let b = book();
    let registry = resident_tool_registry();
    let profile = ModelRuntimeProfile::fallback("teaching-local-authoring", ProviderToolProtocol::Native);
    let tutor = json!({"status":"active","current_design":{"design_ref":"design:clock","design":{
        "target":{"learning_focus":"Elapsed time","expected_performance":"Include a stopped interval"},
        "focus_block_refs":["clock"],"blocks":[
            {"block_id":"clock","relation":"The clock continues during the stop.","purpose":"Count the whole interval."},
            {"block_id":"later-ratio","relation":"UNFOCUSED_RELATION"}],
        "representation":{"kind":"presentation","reason":"Keep the same clock visible.","observation":"The stopped interval remains in elapsed time."}
    }}});
    let binding: crate::presentation::PresentationTeachingBinding = serde_json::from_value(json!({
        "design_ref":"design:clock","block_refs":["clock"],"regions":[{
            "region_id":"clock-face","block_refs":["clock"],"state_fields":["elapsed"],
            "observation":"The stopped interval remains in elapsed time."}]
    })).unwrap();
    let review = crate::presentation_author::PresentationAuthoringContext {
        phase:PresentationPhase::Review, framework:Some("SECOND_WHOLE_LESSON".into()),
        focus:Some("Check the corrected clock reading and narrow-screen labels.".into()),
        needs:vec![crate::agent_prompt::PresentationNeed::State], teaching_binding:Some(binding.clone()),
    };
    let mut global = review.clone(); global.phase = PresentationPhase::Global;
    let mut stale = review.clone(); stale.teaching_binding.as_mut().unwrap().design_ref = "design:old".into();
    let mut exposure = ToolExposureState::default(); exposure.activate("presentation.author");
    for (label, authoring, expected_phase) in [
        ("initial",None,"local"),("global prepare",Some(&global),"local"),
        ("review",Some(&review),"review"),("new design",Some(&stale),"local"),
    ] {
        let (_, plan) = build_sample_request(
            &new_session(), &mut ContextFragmentLedger::default(), &b,
            &mut ActiveToolResultLedger::default(), None, COMPACTION_CONSUMPTION_WRAPPER,
            &registry, &profile, ToolPermissions::default(), &exposure,
            ArtifactExposureContext::no_overlay(), EvidenceState::KnownLids,
            &[], authoring, false, Some(&tutor),
        ).unwrap();
        assert_eq!(selection_state(&plan).unwrap()["phase"], expected_phase, "{label}");
        let context = design(&plan).unwrap();
        let framework: Value = serde_json::from_str(context["framework"].as_str().unwrap()).unwrap();
        assert_eq!(framework["target"], tutor["current_design"]["design"]["target"]);
        assert_eq!(framework["blocks"].as_array().unwrap().len(),1);
        assert_eq!(framework["blocks"][0]["block_id"],"clock");
        assert_eq!(framework["representation"]["observation"],"The stopped interval remains in elapsed time.");
        assert!(!context.to_string().contains("SECOND_WHOLE_LESSON"),"{label}");
        assert!(!context.to_string().contains("UNFOCUSED_RELATION"),"{label}");
        assert!(!plan.instruction_assets.iter().any(|asset|asset.asset_id == "resident-agent.presentation.phase.global"));
        if label == "review" {
            assert_eq!(context["focus"],review.focus.as_deref().unwrap());
            assert_eq!(context["needs"],json!(["state"]));
            assert_eq!(context["teaching_binding"],serde_json::to_value(&binding).unwrap());
        }
    }
    assert!(focused_teaching_authoring(None,Some(&review)).is_none());
    let mut outside = tutor; outside["outside_current_request"] = json!(true);
    assert!(focused_teaching_authoring(Some(&outside),Some(&review)).is_none());
}

#[test]
fn teaching_presentation_prepare_keeps_local_scope_in_run_state() {
    let b = book();
    let mut port = AuthoringPort::new(&b,"teaching-local-prepare");
    let adapter = RequestPlanRecordingAdapter::new(vec![
        prepare("global","global","A NEW WHOLE LESSON","Expand every section",json!(["state"])),
        prepare("review","review","ANOTHER WHOLE LESSON","Verify the repaired clock label",json!(["state"])),
        turn_final("The page is not delivered yet."),
    ],vec![]);
    let mut context = RunContext::new(new_session(),OuterConfig {max_turns:Some(3),..Default::default()},adapter.model_runtime_profile());
    context.tutor = Some(json!({"status":"active","current_design":{"design_ref":"design:clock","design":{
        "target":{"learning_focus":"Elapsed time"},"focus_block_refs":["clock"],
        "blocks":[{"block_id":"clock","relation":"Count stopped time."}],
        "representation":{"kind":"presentation","observation":"The stopped interval remains visible."}
    }},"learner_context":{}}));
    let _ = execute(&mut port,&b,&adapter,&mut context);
    let plans = adapter.seen_plans.borrow();
    for (id, plan) in [("global", &plans[1]), ("review", &plans[2])] {
        let result = plan.input.iter().find(|message| message.tool_call_id.as_deref() == Some(id)).unwrap();
        let result: Value = serde_json::from_str(result.content.as_deref().unwrap()).unwrap();
        assert_eq!(result["model_body"]["status"], "authoring_context_prepared", "{id}: {result}");
    }
    assert_eq!(selection_state(&plans[0]).unwrap()["phase"],"local");
    assert_eq!(design(&plans[1]).unwrap()["phase"],"local");
    assert_eq!(design(&plans[1]).unwrap()["focus"],"The stopped interval remains visible.");
    let current = design(&plans[2]).unwrap();
    assert_eq!(current["phase"],"review");
    assert_eq!(current["focus"],"Verify the repaired clock label");
    assert!(!current["framework"].as_str().unwrap().contains("WHOLE LESSON"));
    assert_eq!((port.writes,port.deliveries),(0,0));
}

#[test]
fn presentation_to4_prepare_example_applies_at_next_sampling() {
    let b = book();
    let mut port = AuthoringPort::new(&b, "to4-prepare");
    let input = crate::presentation_author::spec().parameters["examples"].as_array().unwrap()
        .iter().find(|e| e["operation"] == "prepare").unwrap().clone();
    let adapter = RequestPlanRecordingAdapter::new(vec![discover(), author("prepare", input.clone()), turn_final("Design prepared")], vec![]);
    let mut context = RunContext::new(new_session(), OuterConfig::default(), adapter.model_runtime_profile());
    execute(&mut port, &b, &adapter, &mut context).unwrap();
    let plans = adapter.seen_plans.borrow();
    let mut expected = input.clone();
    expected.as_object_mut().unwrap().remove("operation");
    assert_eq!(design(&plans[2]).unwrap(), expected);
    assert_eq!(selection_state(&plans[2]).unwrap()["needs"], input["needs"]);
    assert_eq!((port.writes, port.deliveries), (0, 0));
}

#[test]
fn ex13_guidance_changes_append_after_results_and_keep_provider_prefix() {
    let b = book();
    let mut port = AuthoringPort::new(&b, "ex13-prefix");
    let adapter = RequestPlanRecordingAdapter::new(vec![
        discover(),
        prepare("local", "local", "FRAME_A", "FOCUS_A", json!(["state"])),
        prepare("same", "local", "FRAME_A", "FOCUS_A", json!(["state", "state"])),
        prepare("design-only", "local", "FRAME_B", "FOCUS_B", json!(["state"])),
        prepare("needs-only", "local", "FRAME_B", "FOCUS_B", json!(["editing"])),
        write("write"),
        author("preview", json!({"operation":"preview","candidate_id":"c1"})),
        prepare("review", "review", "FRAME_B", "WHOLE_WORK", json!([])),
        author("deliver", json!({"operation":"deliver","candidate_id":"c1"})),
        write("new-work"),
        turn_final("Delivered one page; another draft remains."),
    ], vec![]);
    let mut context = RunContext::new(new_session(), OuterConfig::default(), adapter.model_runtime_profile());
    execute(&mut port, &b, &adapter, &mut context).unwrap();
    assert_eq!((port.writes, port.deliveries), (2, 1));
    let plans = adapter.seen_plans.borrow();
    // Optional local recording uses the same requests asserted below, never a model call.
    if let Some(directory) = std::env::var_os("EX13_REQUEST_RECORDING_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        for (index, plan) in plans.iter().enumerate() {
            let (native, _) = crate::native_chat_request_projection("ex13-controlled", plan);
            let react = crate::react_chat_request_projection("ex13-controlled", plan);
            let images = native["messages"].as_array().unwrap().last()
                .map(|m| &m["content"]).filter(|content| content.is_array()).cloned().unwrap_or(json!([]));
            let recording = json!({"instructions":plan.instructions,"instruction_assets":plan.instruction_assets,
                "runtime_profile":plan.runtime_profile,"tools":plan.tools.iter().map(|t| json!({"name":t.name,"description":t.description,"parameters":t.parameters})).collect::<Vec<_>>(),"messages":plan.input,"images":images,"usage":null});
            for (kind, value) in [("request", recording), ("native", native), ("react", react)] {
                std::fs::write(directory.join(format!("{kind}-{index:02}.json")), serde_json::to_vec_pretty(&value).unwrap()).unwrap();
            }
        }
    }
    for plan in &plans[2..] {
        assert_eq!(plan.instructions, plans[1].instructions, "phase/needs/delivery rewrote common instructions");
    }
    let expected = [
        (1, "global", json!([]), 2), (2, "local", json!(["state"]), 3),
        (3, "local", json!(["state"]), 3), (4, "local", json!(["state"]), 3),
        (5, "local", json!(["editing"]), 4), (6, "local", json!(["editing"]), 4),
        (7, "local", json!(["editing"]), 4), (8, "review", json!([]), 5),
        (9, "inactive", Value::Null, 6), (10, "global", json!([]), 7),
    ];
    for (index, phase, needs, events) in expected {
        let plan = &plans[index];
        let messages = plan.ordered_messages();
        let guidance: Vec<_> = messages.iter().enumerate().filter(|(_, m)| m.content.as_deref().is_some_and(|s| s.starts_with("sampling_guidance.v1\n"))).collect();
        assert_eq!(guidance.len(), events, "duplicate or missing selection at sampling {index}");
        let state = selection_state(plan).unwrap();
        assert_eq!(state["phase"].as_str().unwrap_or("inactive"), phase);
        assert_eq!(state["needs"], needs);
        assert_eq!(state["active"], phase != "inactive");
        assert!(!plan.instructions.contains("Presentation phase:"));
        for m in messages.iter().filter(|m| m.role == Role::System) {
            let text = m.content.as_deref().unwrap_or_default();
            assert!(!text.contains("FRAME_") && !text.contains("FOCUS_"), "design was elevated to instructions");
        }
        if [1, 2, 5, 8, 9, 10].contains(&index) {
            let last_tool = messages.iter().rposition(|m| m.role == Role::Tool).unwrap();
            assert!(guidance.last().unwrap().0 > last_tool, "guidance precedes its tool result");
        }
    }
    assert_eq!(design(&plans[4]).unwrap()["framework"], "FRAME_B");
    assert!(design(&plans[9]).is_none());
    assert!(design(&plans[10]).is_none());
    let history = serde_json::to_string(&context.messages).unwrap();
    assert!(!history.contains("sampling_guidance.v1") && !history.contains("runtime_state_snapshot.v1"), "run-local projections leaked into persisted messages");
    for (offset, pair) in plans[1..].windows(2).enumerate() {
        let mut expected_before = pair[0].clone();
        if offset + 1 == 6 {
            // EX13.2 deliberately changes this one call after its first saved
            // receipt sampling; phase changes must still preserve everything else.
            let saved = pair[1].input.iter().flat_map(|m| &m.tool_calls).find(|c| c.id == "write").unwrap();
            let args: Value = serde_json::from_str(&saved.arguments).unwrap();
            assert!(args.get("html").is_none());
            assert!(args.get("saved_source").is_none());
            assert_eq!(saved_source_read(&pair[1].input, "write")["candidate_id"], "c1");
            expected_before.input.iter_mut().flat_map(|m| &mut m.tool_calls).find(|c| c.id == "write").unwrap().arguments = saved.arguments.clone();
            expected_before.input.iter_mut().find(|m| m.tool_call_id.as_deref() == Some("write")).unwrap().content =
                pair[1].input.iter().find(|m| m.tool_call_id.as_deref() == Some("write")).unwrap().content.clone();
        }
        let (before, _) = crate::native_chat_request_projection("ex13-controlled", &expected_before);
        let (after, _) = crate::native_chat_request_projection("ex13-controlled", &pair[1]);
        assert_eq!(before["tools"], after["tools"]);
        let before = before["messages"].as_array().unwrap();
        let after = after["messages"].as_array().unwrap();
        // Preview image attachments are ephemeral observations, tested separately.
        let prefix_len = before.len() - usize::from(!pair[0].preview_images.is_empty());
        assert_eq!(before[..prefix_len], after[..prefix_len], "native rewrote an existing message");
        let before = crate::react_chat_request_projection("ex13-controlled", &expected_before);
        let after = crate::react_chat_request_projection("ex13-controlled", &pair[1]);
        let before = before["messages"].as_array().unwrap();
        assert_eq!(before[..prefix_len + 1], after["messages"].as_array().unwrap()[..prefix_len + 1], "ReAct moved tail guidance");
    }
}

#[test]
fn ex12_scroll_history_keeps_candidate_environment_and_action() {
    let args = json!({"operation":"preview","candidate_id":"c","viewport":{"width":320,"height":420,"input":"touch"},"actions":[{"kind":"scroll","y":1234}],"read_selector":"#end"});
    assert_eq!(compact_tool_locator_arguments("presentation.author", &args.to_string()), args);
    let receipt = historical_tool_receipt("presentation.author", &args.to_string(), r#"{"status":"preview_environment_recorded"}"#, &book());
    assert_eq!(receipt.locator_args, args);
}

#[test]
fn ex13_saved_source_projection_keeps_first_observation_and_raw_history() {
    let b = book();
    let mut port = AuthoringPort::new(&b, "ex13-saved-source");
    let source = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/performance/presentation-staged-authoring-ex12-4/mechanism-staged-catalog");
    let writes: Vec<AssistantTurn> = [13, 18].into_iter().map(|n| {
        let recorded: Value = serde_json::from_slice(&std::fs::read(source.join(format!("response-{n:02}.json"))).unwrap()).unwrap();
        let original = &recorded["tool_calls"][0];
        let mut args: Value = serde_json::from_str(original["arguments"].as_str().unwrap()).unwrap();
        // This controlled port has no book source observations; recorded source
        // bindings are verified separately by the ledger and real storage tests.
        args["source_ref_ids"] = json!([]);
        author(original["id"].as_str().unwrap(), args)
    }).collect();
    let original_calls: Vec<_> = writes.iter().map(|t| t.tool_calls[0].clone()).collect();
    let adapter = RequestPlanRecordingAdapter::new(vec![
        discover(), writes[0].clone(),
        author("preview-first", json!({"operation":"preview","candidate_id":"c1"})),
        writes[1].clone(),
        author("preview-second", json!({"operation":"preview","candidate_id":"c2"})),
        prepare("review-saved", "review", "", "check both works", json!([])),
        turn_final("Two saved drafts remain."),
    ], vec![]);
    let mut context = RunContext::new(new_session(), OuterConfig::default(), adapter.model_runtime_profile());
    execute(&mut port, &b, &adapter, &mut context).unwrap();
    let plans = adapter.seen_plans.borrow();
    if let Some(directory) = std::env::var_os("EX13_REQUEST_RECORDING_DIR") {
        let directory = std::path::PathBuf::from(directory).join("saved-source");
        std::fs::create_dir_all(&directory).unwrap();
        for (index, plan) in plans.iter().enumerate() {
            let (native, _) = crate::native_chat_request_projection("ex13-controlled", plan);
            let react = crate::react_chat_request_projection("ex13-controlled", plan);
            let images = native["messages"].as_array().unwrap().last()
                .map(|m| &m["content"]).filter(|c| c.is_array()).cloned().unwrap_or(json!([]));
            let request = json!({"instructions":plan.instructions,"instruction_assets":plan.instruction_assets,
                "runtime_profile":plan.runtime_profile,"tools":plan.tools.iter().map(|t| json!({"name":t.name,"description":t.description,"parameters":t.parameters})).collect::<Vec<_>>(),"messages":plan.input,"images":images,"usage":null});
            for (kind, value) in [("request", request), ("native", native), ("react", react)] {
                std::fs::write(directory.join(format!("{kind}-{index:02}.json")), serde_json::to_vec_pretty(&value).unwrap()).unwrap();
            }
        }
    }
    let args = |messages: &[Message], id: &str| messages.iter().flat_map(|m| &m.tool_calls)
        .find(|c| c.id == id).unwrap().arguments.clone();
    for (index, original) in original_calls.iter().enumerate() {
        let first = 2 + index * 2;
        assert_eq!(args(&plans[first].input, &original.id), original.arguments);
        let compact = args(&plans[first + 1].input, &original.id);
        let value: Value = serde_json::from_str(&compact).unwrap();
        assert!(value.get("html").is_none() && value.get("readable_content").is_none());
        assert!(value.get("saved_source").is_none());
        assert_eq!(saved_source_read(&plans[first + 1].input, &original.id), json!({"operation":"read","candidate_id":format!("c{}",index+1),"file":"index.html"}));
        for plan in &plans[first + 1..] {
            assert_eq!(args(&plan.input, &original.id), compact);
            assert_eq!(plan.input.iter().filter(|m| m.tool_call_id.as_deref() == Some(&original.id)).count(), 1);
        }
        assert_eq!(args(&context.messages, &original.id), original.arguments);
    }
    assert_eq!(context.presentation_candidates.len(), 2);
    assert_eq!(port.deliveries, 0);
}

struct AuthoringPort {
    store: MemoryStore,
    reader: Reader,
    writes: usize,
    deliveries: usize,
    inspections: Vec<&'static str>,
    recorded_preview: Option<AuthorResult>,
    unavailable_reason: Option<&'static str>,
    fail_next_write: bool,
}

#[test]
fn tool_contract_invalid_arguments_stop_before_host_and_can_be_corrected() {
    let b = book();
    let mut port = AuthoringPort::new(&b, "tool-contract-gate");
    let adapter = RequestPlanRecordingAdapter::new(vec![
        discover(),
        author("invalid", json!({"operation":"write","title":"Page","html":"<p>x</p>","readable_content":"Text","state_contract":"not an object"})),
        write("corrected"),
        turn_final("Draft saved."),
    ], vec![]);
    let mut context = RunContext::new(new_session(), OuterConfig::default(), adapter.model_runtime_profile());
    execute(&mut port, &b, &adapter, &mut context).unwrap();
    assert_eq!(port.writes, 1, "invalid request must never reach host storage");
    let result = context.messages.iter().find(|m| m.tool_call_id.as_deref() == Some("invalid")).unwrap();
    assert!(result.content.as_deref().unwrap().contains("PRESENTATION_ARGUMENTS_INVALID"));
    // Persisted messages hold receipts; correction details live in the next model projection.
    let plans = adapter.seen_plans.borrow();
    let feedback = plans[2].input.iter().find(|m| m.tool_call_id.as_deref() == Some("invalid")).unwrap();
    assert!(feedback.content.as_deref().unwrap().contains("state_contract"));
}

#[test]
fn presentation_teaching_focus_rejects_scope_expansion_before_authoring_mutations() {
    for operation in ["prepare", "write", "patch", "write-region"] {
        let b = book();
        let mut port = AuthoringPort::new(&b, &format!("teaching-focus-{operation}"));
        let source = json!({"source_id":b.base.book_id,"source_revision":b.source_fingerprint(),"start_lid":"1.1","end_lid":"1.1"});
        let current_design = json!({"design_ref":"design:focused","design":{
            "target":{"learning_focus":"Count elapsed time for the trip","expected_performance":"Identify the departure-to-arrival interval",
                "capability":"explanation","source_bindings":[source],"object_refs":[]},
            "representation":{"kind":"presentation","reason":"The learner needs a visible interval.","observation":"The clock continues during a stop."},
            "organizing_model":"The whole-trip average uses total distance and all elapsed time.",
            "blocks":[
                {"block_id":"elapsed-time","relation":"The clock continues during a stop.","purpose":"Include stopped minutes.","source_bindings":[source]},
                {"block_id":"distance","relation":"Distance is the amount travelled.","purpose":"Keep distance distinct from time.","source_bindings":[source]},
                {"block_id":"average-speed","relation":"Average speed divides distance by total time.","purpose":"Return to the whole-trip comparison.","source_bindings":[source]}],
            "connections":[],"focus_block_refs":["elapsed-time","distance"],"basis_refs":[],"open_question_refs":[],
            "revision_reason":"Make the quantities clear before returning to their ratio."}});
        let focused = json!({"design_ref":"design:focused","block_refs":["elapsed-time"],"regions":[{
            "region_id":"clock","block_refs":["elapsed-time"],"state_fields":[],"observation":"The clock covers the stopped interval too."}]});
        let mut outside = focused.clone();
        if operation == "write-region" {
            // This relationship is in focus but was not selected for this page.
            outside["regions"][0]["block_refs"] = json!(["distance"]);
        } else {
            // The block exists in the design, so identity checks alone cannot catch this expansion.
            outside["block_refs"] = json!(["average-speed"]);
            outside["regions"] = json!([]);
        }
        let valid_write = json!({"operation":"write","title":"The trip clock","html":"<p id=\"clock\">The clock keeps going during the stop.</p>",
            "readable_content":"The clock keeps going during the stop.","teaching_binding":focused});
        let invalid = match operation {
            "prepare" => json!({"operation":"prepare","phase":"local","framework":"The departure-to-arrival clock","focus":"Elapsed minutes",
                "needs":[],"teaching_binding":outside}),
            "patch" => json!({"operation":"patch","candidate_id":"c1","edits":[{"old_text":"clock","new_text":"average speed"}],"teaching_binding":outside}),
            _ => {
                let mut request = valid_write.clone();
                request["teaching_binding"] = outside;
                request
            }
        };
        let turns = if operation == "patch" {
            vec![discover(), author("valid-write", valid_write), author("outside-scope", invalid)]
        } else {
            vec![discover(), author("outside-scope", invalid), author("valid-write", valid_write)]
        };
        let max_turns = turns.len();
        let adapter = RequestPlanRecordingAdapter::new(turns, vec![]);
        let mut context = RunContext::new(new_session(), OuterConfig { max_turns:Some(max_turns), ..Default::default() }, adapter.model_runtime_profile());
        context.tutor = Some(json!({"status":"active","current_design":current_design,"learner_context":{}}));
        let unfinished = execute(&mut port, &b, &adapter, &mut context).unwrap();
        assert_eq!(unfinished.warning.as_deref().unwrap(),"TEACHING_INVALID");
        assert!(!unfinished.answer.as_ref().unwrap().contains("预算终答"),
            "the fixture stops after authoring without fabricating a selected teaching move: {unfinished:?}");
        assert_eq!((port.writes, port.deliveries), (1, 0), "{operation}: only the valid page may mutate host storage");
        assert_eq!(context.presentation_candidates.len(), 1, "{operation}: rejected authoring must not register a candidate");
        assert!(context.presentation_authoring.is_none(), "a rejected prepare must not replace the working authoring scope");
        assert!(tool_result(&context.messages, "outside-scope").contains("TEACHING_INVALID"), "{operation}");
        let plans = adapter.seen_plans.borrow();
        assert_eq!(plans.len(), max_turns + 1, "one small fallback is sampled without selecting the unfinished teaching move");
        if operation != "patch" {
            let correction = plans[2].input.iter().find(|message|message.tool_call_id.as_deref()==Some("outside-scope")).unwrap();
            let expected = if operation == "write-region" { "本页已选择的关系" } else { "当前教学焦点" };
            assert!(correction.content.as_deref().unwrap().contains(expected), "{operation}: the model needs the concrete scope correction");
        }
    }
}

impl AuthoringPort {
    fn new(book: &Book, label: &str) -> Self {
        Self {
            store: MemoryStore::open(tmp(label)).unwrap(),
            reader: Reader::new(book, 1),
            writes: 0,
            deliveries: 0,
            inspections: Vec::new(),
            recorded_preview: None,
            unavailable_reason: None,
            fail_next_write: false,
        }
    }
}
impl ResidentStatePort for AuthoringPort {
    fn presentation_unavailable_reason(&self) -> Option<&'static str> {
        self.unavailable_reason
    }
    fn submit_private<R>(&mut self, f: impl FnOnce(&mut MemoryStore) -> R) -> Result<R, ToolError> {
        Ok(f(&mut self.store))
    }
    fn read_live_reader<R>(&mut self, f: impl FnOnce(&Reader) -> R) -> Result<R, ToolError> {
        Ok(f(&self.reader))
    }
    fn apply_reader<R>(
        &mut self,
        f: impl FnOnce(&mut MemoryStore, &mut Reader) -> R,
    ) -> Result<R, ToolError> {
        Ok(f(&mut self.store, &mut self.reader))
    }
    fn reader_input(
        &mut self,
        book: &Book,
        question: &str,
    ) -> crate::run_context::ReaderInputSnapshot {
        crate::run_context::ReaderInputSnapshot::capture(book, &self.reader, question)
    }
    fn author_presentation(
        &mut self,
        request: AuthorRequest,
        bindings: &[SourceBinding],
        _: &[Message],
        _: &crate::run_context::CancellationToken,
    ) -> Result<AuthorResult, ToolError> {
        if self.fail_next_write && matches!(request, AuthorRequest::Write { .. }) {
            self.unavailable_reason = Some("execution_cleanup_failed");
            return Err(ToolError { error_code: "PRESENTATION_SANDBOX_UNAVAILABLE".into(),
                category: "unavailable".into(), message: "Presentation execution is unavailable".into() });
        }
        let mut result = AuthorResult {
            body: Value::Null,
            images: vec![],
            previewed_candidate: None,
            delivered: None,
        };
        match request {
            AuthorRequest::Prepare { .. } => panic!("prepare reached host storage"),
            AuthorRequest::Read { .. } => {
                self.inspections.push("read");
                result.body = json!({"status":"read","text":"Compare the query vector with each item vector."});
            }
            AuthorRequest::Search { .. } => {
                self.inspections.push("search");
                result.body = json!({"status":"matches","matches":[{"text":"comparison","offset":0}]});
            }
            AuthorRequest::Write { source_ref_ids, .. } => {
                crate::presentation_author::bindings_for(&source_ref_ids, bindings)?;
                self.writes += 1;
                result.body =
                    json!({"status":"candidate_saved", "candidate_id":format!("c{}",self.writes)});
            }
            AuthorRequest::Patch { .. } => {
                self.writes += 1;
                result.body =
                    json!({"status":"candidate_saved", "candidate_id":format!("c{}",self.writes)});
            }
            AuthorRequest::Preview { candidate_id, viewport, .. } => {
                if let Some(recorded) = self.recorded_preview.take() {
                    assert_eq!(recorded.body["candidate_id"], candidate_id);
                    return Ok(recorded);
                }
                result.body =
                    json!({"status":"preview_ready_for_inspection", "candidate_id":candidate_id});
                result.previewed_candidate = Some(candidate_id.clone());
                result.images.push(PreviewImage {
                    caption: format!("{candidate_id} actual fixture observation"),
                    png_base64: "png".into(),
                    candidate_id: Some(candidate_id),
                    environment_name: Some(viewport.map(crate::presentation_preview::preview_environment_name).unwrap_or_else(|| "desktop".into())),
                });
            }
            AuthorRequest::Deliver { .. } => {
                self.deliveries += 1;
                result.body = json!({"status":"version_saved"});
                result.delivered = Some(crate::presentation::PresentationRef {
                    presentation_id: "saved".into(),
                    revision: self.deliveries as u32,
                });
            }
            _ => panic!("unexpected host operation: {request:?}"),
        }
        Ok(result)
    }
}
fn discover() -> AssistantTurn {
    turn_calls(vec![call(
        "discover",
        "tool.search",
        r#"{"task":"rich explanation","required_capabilities":["presentation_authoring"],"scope":"passage","operation":"explain","effect_mode":"read_only","max_results":1}"#,
    )])
}
fn author(id: &str, args: Value) -> AssistantTurn {
    turn_calls(vec![call(id, "presentation.author", &args.to_string())])
}
fn prepare(id: &str, phase: &str, framework: &str, focus: &str, needs: Value) -> AssistantTurn {
    author(
        id,
        json!({"operation":"prepare","phase":phase,"framework":framework,"focus":focus,"needs":needs}),
    )
}
fn write(id: &str) -> AssistantTurn {
    author(
        id,
        json!({"operation":"write","title":"example","html":"<p>Example</p>","readable_content":"Example"}),
    )
}
fn execute(
    port: &mut AuthoringPort,
    b: &Book,
    adapter: &dyn ModelAdapter,
    context: &mut RunContext,
) -> Result<OuterOutcome, ToolError> {
    run_context(
        b,
        port,
        adapter,
        context,
        &default_profile_snapshot(b, &port.store, "t0"),
        &ResidentTurnResources::default(),
        None,
        &mut EphemeralCompactionCheckpointSink::default(),
        "Explain the relationship",
        "t0",
    )
}

#[test]
fn presentation_unavailable_discovery_reports_host_reason_and_preserves_answer() {
    let b = book();
    let mut port = AuthoringPort::new(&b, "presentation-unavailable-discovery");
    port.unavailable_reason = Some("probe_failed");
    let explanation = "演示环境当前不可用，页面尚未交付；这里是已经读到的章节骨架。";
    let adapter = RequestPlanRecordingAdapter::new(vec![discover(), turn_final(explanation)], vec![]);
    let mut context = RunContext::new(new_session(), OuterConfig { max_turns: Some(1), ..Default::default() }, adapter.model_runtime_profile());
    context.goal = Some(crate::goal::ResidentGoal::new("g".into(), "t1".into(), "制作演示页".into()));
    let out = execute(&mut port, &b, &adapter, &mut context).unwrap();
    let feedback: Value = serde_json::from_str(&tool_result(&context.messages, "discover")).unwrap();
    assert_eq!(feedback["activated"], json!([]));
    assert_eq!(feedback["blocked"][0]["reason"], "presentation_unavailable");
    assert!(feedback["blocked"][0]["message"].as_str().unwrap().contains("probe_failed"));
    assert_eq!(out.answer.as_deref(), Some(explanation));
    assert_eq!(out.warning.as_deref(), Some("PRESENTATION_SANDBOX_UNAVAILABLE"));
    assert!(out.incomplete && context.goal.as_ref().unwrap().objective_gap(0, 0).is_some());
    assert_eq!((port.writes, port.deliveries), (0, 0));
}

#[test]
fn presentation_unavailable_after_write_preserves_finalization_explanation() {
    let b = book();
    let mut port = AuthoringPort::new(&b, "presentation-unavailable-write");
    port.fail_next_write = true;
    let explanation = "演示执行环境不可用，页面没有交付。已有原文支持的解释仍可阅读。";
    let adapter = RequestPlanRecordingAdapter::new(vec![discover(), write("failed-write"), turn_final(explanation)], vec![]);
    let mut context = RunContext::new(new_session(), OuterConfig { max_turns: Some(2), ..Default::default() }, adapter.model_runtime_profile());
    context.goal = Some(crate::goal::ResidentGoal::new("g".into(), "t1".into(), "制作演示页".into()));
    let out = execute(&mut port, &b, &adapter, &mut context).unwrap();
    assert!(tool_result(&context.messages, "failed-write").contains("PRESENTATION_SANDBOX_UNAVAILABLE"));
    assert_eq!(out.answer.as_deref(), Some(explanation));
    assert_eq!(out.warning.as_deref(), Some("PRESENTATION_SANDBOX_UNAVAILABLE"));
    assert!(out.incomplete && context.delivered_presentations.is_empty());
    assert!(adapter.seen_plans.borrow().last().unwrap().tools.is_empty());
}

#[test]
fn presentation_unavailable_plain_final_does_not_reask_for_delivery() {
    let b = book();
    let mut port = AuthoringPort::new(&b, "presentation-unavailable-final");
    port.unavailable_reason = Some("not_configured");
    let explanation = "演示环境尚未配置，演示页未交付。";
    let adapter = RequestPlanRecordingAdapter::new(vec![turn_final(explanation), turn_final(explanation)], vec![]);
    let mut context = RunContext::new(new_session(), OuterConfig::default(), adapter.model_runtime_profile());
    context.goal = Some(crate::goal::ResidentGoal::new("g".into(), "t1".into(), "制作演示页".into()));
    let out = execute(&mut port, &b, &adapter, &mut context).unwrap();
    assert_eq!(adapter.seen_plans.borrow().len(), 1);
    assert_eq!(out.answer.as_deref(), Some(explanation));
    assert_eq!(out.warning.as_deref(), Some("PRESENTATION_SANDBOX_UNAVAILABLE"));
    assert!(out.incomplete && context.goal.as_ref().unwrap().result_refs.is_empty());
}
fn design(plan: &AgentRequestPlan) -> Option<Value> {
    let messages = plan.ordered_messages();
    let fragments: Vec<_> = messages
        .iter()
        .filter(|m| {
            m.content
                .as_deref()
                .is_some_and(|s| s.contains("key=presentation.authoring_context\n"))
        })
        .collect();
    if selection_state(plan).is_some_and(|s| s["design_active"] == false) {
        return None;
    }
    fragments.last().map(|message| {
        assert_eq!(message.role, Role::User, "framework must remain task data");
        assert!(!plan
            .instructions
            .contains("key=presentation.authoring_context"));
        let text = message.content.as_deref().unwrap();
        serde_json::from_str(text.lines().last().unwrap()).unwrap()
    })
}
fn selection_state(plan: &AgentRequestPlan) -> Option<Value> {
    plan.input.iter().rev().find_map(|m| {
        m.content.as_deref().filter(|s| s.starts_with("presentation_guidance_state.v1\n"))
            .map(|s| serde_json::from_str(s.lines().nth(1).unwrap()).unwrap())
    })
}
fn phase(plan: &AgentRequestPlan, name: &str, references: &[&str]) {
    let ids: Vec<_> = plan
        .instruction_assets
        .iter()
        .map(|asset| asset.asset_id.as_str())
        .collect();
    assert_eq!(ids.iter().filter(|id| id.contains(".phase.")).count(), 1);
    assert!(ids.contains(&format!("resident-agent.presentation.phase.{name}").as_str()));
    assert_eq!(
        ids.iter()
            .filter(|id| **id == "resident-agent.skill.presentation-method")
            .count(),
        1
    );
    let actual: Vec<_> = ids
        .iter()
        .filter_map(|id| id.strip_prefix("resident-agent.presentation.reference."))
        .collect();
    assert_eq!(actual, references);
    let selected = plan.input.iter().filter_map(|m| m.content.as_deref())
        .filter(|text| text.starts_with("sampling_guidance.v1\n")).collect::<Vec<_>>().join("\n");
    assert!(selected.contains("subsequent model sampling"));
    assert!(selected.contains("source.present"));
}

#[test]
fn ex12_stage_round_trip_replaces_design_and_preserves_candidate_inspection() {
    let b = book();
    let mut port = AuthoringPort::new(&b, "ex12-stages");
    let adapter = RequestPlanRecordingAdapter::new(
        vec![
            discover(),
            prepare("local-a", "local", "FRAME_A", "FOCUS_A", json!(["konva"])),
            write("write-a"),
            author(
                "preview-a",
                json!({"operation":"preview","candidate_id":"c1"}),
            ),
            prepare("global-b", "global", "FRAME_B", "REVISED_ORDER", json!([])),
            prepare(
                "local-b",
                "local",
                "FRAME_B",
                "FOCUS_B",
                json!(["editing", "continuous_scene"]),
            ),
            author(
                "patch-b",
                json!({"operation":"patch","candidate_id":"c1","edits":[{"old_text":"Example","new_text":"Revised"}]}),
            ),
            prepare(
                "review-b",
                "review",
                "FRAME_B",
                "WHOLE_WORK",
                json!(["state"]),
            ),
            author(
                "early-deliver",
                json!({"operation":"deliver","candidate_id":"c2"}),
            ),
            author(
                "preview-b",
                json!({"operation":"preview","candidate_id":"c2"}),
            ),
            author(
                "deliver-b",
                json!({"operation":"deliver","candidate_id":"c2"}),
            ),
            author(
                "new-work",
                json!({"operation":"prepare","phase":"local","needs":[]}),
            ),
            turn_final("Done."),
        ],
        vec![],
    );
    let mut context = RunContext::new(
        new_session(),
        OuterConfig::default(),
        adapter.model_runtime_profile(),
    );
    let out = execute(&mut port, &b, &adapter, &mut context).unwrap();
    assert!(!out.incomplete);
    assert_eq!((port.writes, port.deliveries), (2, 1));
    assert!(context.evidence_ledger.bindings().is_empty());
    assert_eq!(
        context.presentation_candidates,
        BTreeSet::from(["c1".into()])
    );
    assert!(context.inspected_presentations.contains("c1"));
    assert!(context.inspected_presentations.contains("c2"));
    assert!(tool_result(&context.messages, "early-deliver")
        .contains("PRESENTATION_INSPECTION_REQUIRED"));
    assert!(out.answer_view.unwrap().parts.iter().any(|p| matches!(p, AgentAnswerPart::Presentation {presentation_id,..} if presentation_id == "saved")));
    let plans = adapter.seen_plans.borrow();
    phase(&plans[1], "global", &[]);
    phase(&plans[2], "local", &["konva"]);
    assert_eq!(design(&plans[2]).unwrap()["framework"], "FRAME_A");
    assert_eq!(plans[4].preview_images.len(), 1);
    assert_eq!(
        plans[4].preview_images[0].candidate_id.as_deref(),
        Some("c1")
    );
    phase(&plans[5], "global", &[]);
    assert_eq!(design(&plans[5]).unwrap()["framework"], "FRAME_B");
    phase(
        &plans[6],
        "local",
        &["editing", "state", "continuous-scene"],
    );
    assert_eq!(design(&plans[6]).unwrap()["focus"], "FOCUS_B");
    phase(&plans[8], "review", &["state"]);
    assert_eq!(
        plans[10].preview_images[0].candidate_id.as_deref(),
        Some("c2")
    );
    assert!(plans[11].instruction_assets.iter().all(|a| !a.asset_id.contains(".phase.") && !a.asset_id.contains(".reference.")));
    assert!(design(&plans[11]).is_none(), "delivery clears design");
    phase(&plans[12], "local", &[]);
    let new_work = design(&plans[12]).unwrap();
    assert!(new_work["framework"].is_null());
    assert!(new_work["focus"].is_null());
    assert!(!tool_result(&context.messages, "local-a").contains("FRAME_A"));
}

#[test]
fn ex12_simple_local_can_deliver_without_framework_and_plain_answer_stays_plain() {
    let b = book();
    let mut port = AuthoringPort::new(&b, "ex12-simple");
    let adapter = RequestPlanRecordingAdapter::new(
        vec![
            discover(),
            author(
                "local",
                json!({"operation":"prepare","phase":"local","focus":"Correct the caption","needs":["editing"]}),
            ),
            write("small-edit"),
            author(
                "preview",
                json!({"operation":"preview","candidate_id":"c1"}),
            ),
            author(
                "deliver",
                json!({"operation":"deliver","candidate_id":"c1"}),
            ),
            turn_final("Done."),
        ],
        vec![],
    );
    let mut context = RunContext::new(
        new_session(),
        OuterConfig::default(),
        adapter.model_runtime_profile(),
    );
    execute(&mut port, &b, &adapter, &mut context).unwrap();
    assert_eq!(port.deliveries, 1);
    assert!(context.presentation_authoring.is_none());
    phase(&adapter.seen_plans.borrow()[2], "local", &["editing"]);
    assert!(design(&adapter.seen_plans.borrow()[2]).unwrap()["framework"].is_null());
    let plain = RequestPlanRecordingAdapter::new(vec![turn_final("A short explanation.")], vec![]);
    let mut next = RunContext::new(
        new_session(),
        OuterConfig::default(),
        plain.model_runtime_profile(),
    );
    execute(&mut port, &b, &plain, &mut next).unwrap();
    assert!(plain.seen_plans.borrow()[0]
        .instruction_assets
        .iter()
        .all(|a| !a.asset_id.contains("presentation")));
}

#[test]
fn ex12_identical_prepares_do_not_reset_no_progress_or_complete_a_goal() {
    let b = book();
    let mut port = AuthoringPort::new(&b, "ex12-no-progress");
    let adapter = RequestPlanRecordingAdapter::new(
        vec![
            discover(),
            prepare(
                "first",
                "local",
                "FRAME",
                "FOCUS",
                json!(["state", "editing"]),
            ),
            prepare(
                "same-reordered",
                "local",
                "FRAME",
                "FOCUS",
                json!(["editing", "state"]),
            ),
            prepare(
                "same-duplicates",
                "local",
                "FRAME",
                "FOCUS",
                json!(["state", "state", "editing"]),
            ),
            turn_final("Still preparing."),
        ],
        vec![],
    );
    let mut context = RunContext::new(
        new_session(),
        OuterConfig::default(),
        adapter.model_runtime_profile(),
    );
    context.goal = Some(crate::goal::ResidentGoal::new(
        "g".into(),
        "t1".into(),
        "制作演示页".into(),
    ));
    let out = execute(&mut port, &b, &adapter, &mut context).unwrap();
    assert!(out.incomplete);
    assert_eq!(out.warning.as_deref(), Some("AGENT_NO_PROGRESS"));
    assert!(adapter.seen_plans.borrow().last().unwrap().tools.is_empty());
    let plans = adapter.seen_plans.borrow();
    let finalization = plans.last().unwrap();
    assert_eq!(selection_state(finalization).unwrap()["active"], false);
    assert!(design(finalization).is_none());
    assert_eq!((port.writes, port.deliveries), (0, 0));
    assert!(context.goal.as_ref().unwrap().result_refs.is_empty());
    assert!(context.delivered_presentations.is_empty());
    assert!(context.presentation_candidates.is_empty());
    assert!(context.inspected_presentations.is_empty());
}

#[test]
fn ex12_prepare_and_write_in_either_batch_order_wait_for_next_sampling() {
    for reverse in [false, true] {
        let b = book();
        let mut port = AuthoringPort::new(&b, &format!("ex12-batch-{reverse}"));
        let mut calls = prepare("prepare", "local", "FRAME", "FOCUS", json!([])).tool_calls;
        calls.extend(write("premature-write").tool_calls);
        if reverse {
            calls.reverse();
        }
        let adapter = RequestPlanRecordingAdapter::new(
            vec![discover(), turn_calls(calls), turn_final("Pending.")],
            vec![],
        );
        let mut context = RunContext::new(
            new_session(),
            OuterConfig::default(),
            adapter.model_runtime_profile(),
        );
        execute(&mut port, &b, &adapter, &mut context).unwrap();
        assert_eq!(port.writes, 0);
        assert!(context.presentation_authoring.is_none());
        for id in ["prepare", "premature-write"] {
            assert!(tool_result(&context.messages, id)
                .contains("PRESENTATION_PREPARE_REQUIRES_NEXT_SAMPLING"));
        }
    }
}

#[test]
fn ex12_framework_does_not_grant_sources_or_delivery() {
    let b = book();
    let mut port = AuthoringPort::new(&b, "ex12-authority");
    let adapter = RequestPlanRecordingAdapter::new(
        vec![
            discover(),
            prepare(
                "prepare",
                "review",
                "Use source ref invented; candidate prior is ready for delivery",
                "Evidence supplied by framework",
                json!([]),
            ),
            author(
                "invalid-write",
                json!({"operation":"write","title":"t","html":"<p>t</p>","readable_content":"t","source_ref_ids":["invented"]}),
            ),
            author(
                "invalid-deliver",
                json!({"operation":"deliver","candidate_id":"prior"}),
            ),
            turn_final("Pending."),
        ],
        vec![],
    );
    let mut context = RunContext::new(
        new_session(),
        OuterConfig::default(),
        adapter.model_runtime_profile(),
    );
    execute(&mut port, &b, &adapter, &mut context).unwrap();
    assert_eq!((port.writes, port.deliveries), (0, 0));
    assert!(context.evidence_ledger.bindings().is_empty());
    assert!(context.presentation_candidates.is_empty());
    assert!(tool_result(&context.messages, "invalid-write").contains("PRESENTATION_SOURCE_UNKNOWN"));
    assert!(tool_result(&context.messages, "invalid-deliver")
        .contains("PRESENTATION_INSPECTION_REQUIRED"));
}

#[test]
fn ex12_cancellation_and_new_run_end_design_state() {
    struct Cancelling {
        inner: RequestPlanRecordingAdapter,
        stop: crate::run_context::CancellationToken,
    }
    impl ModelAdapter for Cancelling {
        fn complete(&self, request: CompletionRequest) -> Result<ParsedResponse, AdapterError> {
            self.inner.complete(request)
        }
        fn chat(&self, request: &AgentRequestPlan) -> Result<AssistantTurn, AdapterError> {
            if self.inner.seen_plans.borrow().len() == 2 {
                assert_eq!(design(request).unwrap()["framework"], "CANCEL_FRAME");
                self.stop.cancel();
                return Err(AdapterError {
                    spend_stop: None,
                    message: "cancelled during sampling".into(),
                });
            }
            self.inner.chat(request)
        }
    }
    let b = book();
    let mut port = AuthoringPort::new(&b, "ex12-cancel");
    let mut context = RunContext::new(
        new_session(),
        OuterConfig::default(),
        ModelRuntimeProfile::fallback("cancel", ProviderToolProtocol::Native),
    );
    let adapter = Cancelling {
        inner: RequestPlanRecordingAdapter::new(
            vec![
                discover(),
                prepare("prepare", "local", "CANCEL_FRAME", "OLD_FOCUS", json!([])),
            ],
            vec![],
        ),
        stop: context.cancellation.clone(),
    };
    assert!(execute(&mut port, &b, &adapter, &mut context).is_err());
    assert!(context.presentation_authoring.is_none());
    context.close_cancelled_tool_calls();
    let next_adapter =
        RequestPlanRecordingAdapter::new(vec![discover(), turn_final("New task.")], vec![]);
    let mut next = RunContext::new(
        context.messages,
        OuterConfig::default(),
        next_adapter.model_runtime_profile(),
    );
    execute(&mut port, &b, &next_adapter, &mut next).unwrap();
    assert!(next.presentation_authoring.is_none());
    let plans = next_adapter.seen_plans.borrow();
    phase(&plans[1], "global", &[]);
    assert!(
        design(&plans[1]).is_none(),
        "past prepare calls cannot reactivate state"
    );
}

#[test]
fn ex12_current_framework_survives_mid_turn_compaction() {
    check_framework_survives_mid_turn_compaction("local", json!(["editing"]));
}

fn check_framework_survives_mid_turn_compaction(selected_phase: &str, needs: Value) {
    let media_selected = needs.as_array().is_some_and(|items| items.iter().any(|n| n == "manim"));
    let b = book();
    let mut port = AuthoringPort::new(&b, "ex12-compaction");
    let messages = completed_history("OLD_COMPLETED_HISTORY", 35_000);
    let snapshot = default_profile_snapshot(&b, &port.store, "t0");
    // Discovery activates this schema after the initial pressure measurement.
    // Reserve it explicitly so the working material, not tool activation,
    // remains the event that triggers this compaction fixture.
    let author_schema_tokens = crate::model_runtime::estimate_text_tokens(
        &crate::presentation_author::spec().parameters.to_string(),
    );
    // Discovery also activates the compiled method directory. Its growth must
    // not replace working material as the cause of this fixture's compaction.
    let catalog_tokens = crate::model_runtime::estimate_text_tokens(
        &crate::presentation_methods::directory().text,
    );
    let profile = tune_profile_just_above_initial_pressure(
        &b,
        &snapshot,
        &port.reader,
        &messages,
        "Explain the relationship",
        compaction_profile("ex12-compaction"),
        (if media_selected { 20_000 } else { 10_000 }) + author_schema_tokens + catalog_tokens,
    );
    let local = prepare(
        "latest",
        selected_phase,
        "CURRENT_FRAME",
        "CURRENT_FOCUS",
        needs,
    );
    // Pressure comes from preserved current-turn conversation, not the brief design record.
    let mut next = write("pressure-after-selection");
    next.text = Some("Current working material. ".repeat(if media_selected { 6_000 } else { 2_000 }));
    let adapter = AutoCompactionAdapter::new(
        profile,
        vec![discover(), local, write("saved-before-pressure"), next, turn_final("Continue later.")],
    );
    let mut context = RunContext::new(
        messages,
        OuterConfig::default(),
        adapter.model_runtime_profile(),
    );
    let mut goal = crate::goal::ResidentGoal::new("g".into(), "t1".into(), "Explain the full relationship".into());
    goal.apply_update(serde_json::from_value(json!({"operation":"working", "focus":"verify the local explanation", "next_move":"expand remaining content", "items":[
        {"id":"prototype","description":"Key relationship prototype","status":"completed"},
        {"id":"expand","description":"Remaining explanation and exercises","status":"in_progress"},
        {"id":"review","description":"Read the whole work against requirements","status":"pending"}
    ]})).unwrap(), "t1", "Explain the full relationship").unwrap();
    context.goal = Some(goal.clone());
    let mut sink = EphemeralCompactionCheckpointSink::default();
    run_context(
        &b,
        &mut port,
        &adapter,
        &mut context,
        &snapshot,
        &ResidentTurnResources::default(),
        None,
        &mut sink,
        "Explain the relationship",
        "t0",
    )
    .unwrap();
    assert!(sink.installed.is_some());
    assert!(adapter
        .compaction_requests
        .borrow()
        .iter()
        .any(|r| r.phase == CompactionPhase::MidTurn));
    let sampled = adapter.seen_messages.borrow();
    assert!(
        serde_json::to_string(&sampled[3])
            .unwrap()
            .contains("OLD_COMPLETED_HISTORY"),
        "compaction must happen after the selected guidance was sampled"
    );
    let after = serde_json::to_string(&sampled[4]).unwrap();
    assert!(!after.contains("OLD_COMPLETED_HISTORY"));
    assert!(after.contains(CONTEXT_COMPACTION_ITEM_VERSION));
    let current_goal = sampled[4].iter().rev().filter_map(|m| m.content.as_deref())
        .find(|s| s.contains("key=agent.resident_goal\n")).unwrap();
    assert!(current_goal.contains("Explain the full relationship"));
    assert!(current_goal.contains(&serde_json::to_string(&goal.working.items).unwrap()));
    assert_eq!(context.goal, Some(goal), "framework changes and compaction must not rewrite requirements or the plan");
    let current = sampled[4]
        .iter()
        .rev()
        .find(|m| {
            m.content
                .as_deref()
                .is_some_and(|s| s.contains("key=presentation.authoring_context\n"))
        })
        .unwrap();
    assert_eq!(current.role, Role::User);
    assert!(current
        .content
        .as_deref()
        .unwrap()
        .contains("CURRENT_FRAME"));
    assert!(after.contains(&format!("Presentation phase: {selected_phase}")));
    assert_eq!(after.matches(&format!("Presentation phase: {selected_phase}")).count(), 1, "compaction lost or duplicated active guidance");
    if selected_phase == "local" {
        assert!(sampled[4].iter().filter_map(|m| m.content.as_deref())
            .any(|s| s.contains(include_str!("../../../skills/presentation/phases/local.md").trim())), "compaction lost local visual methods");
        ex14::record_value("local-compaction", "messages.json", &json!(*sampled));
    }
    if selected_phase == "review" {
        assert!(after.contains("观察关系真正发生的位置"));
        assert!(after.contains("prerequisite actions plus target actions"));
        let current = sampled[4].iter().rev().filter_map(|m| m.content.as_deref())
            .find(|s| s.starts_with("presentation_guidance_state.v1\n")).unwrap();
        assert!(current.contains(if media_selected { "manim" } else { "continuous_scene" }));
        if media_selected {
            let manim = include_str!("../../../skills/presentation/references/manim.md").trim();
            assert_eq!(sampled[4].iter().filter_map(|m| m.content.as_deref())
                .filter(|s| s.contains(manim)).count(), 1, "compaction lost or duplicated Manim guidance");
            ex14::record_value("media-compaction", "messages.json", &json!(*sampled));
        } else {
            ex14::record_value("compaction", "messages.json", &json!(*sampled));
        }
    }
    assert_eq!(sampled[3][0].content, sampled[4][0].content, "compaction rebuilt different common instructions");
    let saved = sampled[4].iter().flat_map(|m| &m.tool_calls).find(|c| c.id == "saved-before-pressure").unwrap();
    let args: Value = serde_json::from_str(&saved.arguments).unwrap();
    assert!(args.get("html").is_none());
    assert!(args.get("saved_source").is_none());
    assert_eq!(saved_source_read(&sampled[4], "saved-before-pressure")["candidate_id"], "c1");
    assert_eq!(sampled[4].iter().filter(|m| m.tool_call_id.as_deref() == Some("saved-before-pressure")).count(), 1);
    let raw = context.messages.iter().flat_map(|m| &m.tool_calls).find(|c| c.id == "saved-before-pressure").unwrap();
    assert_eq!(serde_json::from_str::<Value>(&raw.arguments).unwrap()["html"], "<p>Example</p>");
    let checkpoint = sink.installed.as_ref().unwrap();
    for request in adapter.compaction_requests.borrow().iter() {
        for id in &request.required_source_ids {
            assert!(checkpoint.semantic.source_coverage.iter().any(|coverage| &coverage.source_item_id == id));
        }
    }
}

#[path = "presentation_ex14_guidance_tests.rs"]
mod ex14;

#[test]
fn ex12_omitted_fields_clear_previous_work_before_delivery() {
    let b = book();
    let mut port = AuthoringPort::new(&b, "ex12-replace");
    let adapter = RequestPlanRecordingAdapter::new(
        vec![
            discover(),
            prepare(
                "old-work",
                "local",
                "OLD_FRAME",
                "OLD_FOCUS",
                json!(["manim"]),
            ),
            author(
                "different-work",
                json!({"operation":"prepare","phase":"local","needs":[]}),
            ),
            write("new-page"),
            turn_final("Draft remains."),
        ],
        vec![],
    );
    let mut context = RunContext::new(
        new_session(),
        OuterConfig::default(),
        adapter.model_runtime_profile(),
    );
    execute(&mut port, &b, &adapter, &mut context).unwrap();
    let plans = adapter.seen_plans.borrow();
    phase(&plans[2], "local", &["state", "continuous-scene", "manim"]);
    phase(&plans[3], "local", &[]);
    let replaced = design(&plans[3]).unwrap();
    assert!(replaced["framework"].is_null());
    assert!(replaced["focus"].is_null());
    assert_eq!((port.writes, port.deliveries), (1, 0));
}


#[test]
fn presentation_omitted_environments_report_remaining_groups_and_recover_after_observation() {
    let b=book();
    let mut port=AuthoringPort::new(&b,"presentation-omitted-recovery");
    // The supported batch preview path can return more observations than the
    // active image budget fits. Keep desktop last, while both earlier groups
    // are partially or wholly omitted by the actual projection.
    let images=(0..60).map(|step| {
        let environment=if step<20 {"narrow-content"} else if step<59 {"short-content"} else {"desktop-content"};
        PreviewImage {caption:format!("c1 {environment} observation {step}"),png_base64:"png".into(),
            candidate_id:Some("c1".into()),environment_name:Some(environment.into())}
    }).collect();
    port.recorded_preview=Some(AuthorResult {body:json!({"status":"preview_ready_for_inspection","candidate_id":"c1"}),
        images,previewed_candidate:Some("c1".into()),delivered:None});
    let adapter=RequestPlanRecordingAdapter::new(vec![
        discover(),write("write"),
        author("preview-many",json!({"operation":"preview","candidate_id":"c1"})),
        author("blocked-both",json!({"operation":"deliver","candidate_id":"c1"})),
        author("recover-narrow",json!({"operation":"preview","candidate_id":"c1","viewport":{"width":320,"height":420,"input":"touch"}})),
        author("blocked-short",json!({"operation":"deliver","candidate_id":"c1"})),
        author("recover-short",json!({"operation":"preview","candidate_id":"c1","viewport":{"width":640,"height":240,"input":"touch"}})),
        author("deliver",json!({"operation":"deliver","candidate_id":"c1"})),turn_final("The page is ready."),
    ],vec![]);
    let mut context=RunContext::new(new_session(),OuterConfig::default(),adapter.model_runtime_profile());
    context.unobserved_preview_environments.insert(("other-candidate".into(),"other-environment".into()));
    let outcome=execute(&mut port,&b,&adapter,&mut context).unwrap();
    assert_eq!(port.deliveries,1,"omitted groups must block delivery until every current-candidate group is observed");
    assert!(!outcome.incomplete);
    assert!(outcome.answer_view.unwrap().parts.iter().any(|part|matches!(part,AgentAnswerPart::Presentation {..})));
    assert!(!context.unobserved_preview_environments.iter().any(|(candidate,_)|candidate=="c1"));
    assert!(context.unobserved_preview_environments.contains(&("other-candidate".into(),"other-environment".into())),"recovery must not clear another candidate's observation debt");
    let plans=adapter.seen_plans.borrow();
    assert!(!plans[3].preview_images.is_empty() && plans[3].preview_images.len()<60);
    assert!(plans[3].input.iter().any(|message|message.content.as_deref().is_some_and(|text|text.contains("images omitted by the active image budget"))));
    let correction=|index:usize,id:&str| {
        let message=plans[index].input.iter().find(|message|message.tool_call_id.as_deref()==Some(id)).unwrap();
        let result:Value=serde_json::from_str(message.content.as_deref().unwrap()).unwrap();
        assert_eq!(result["model_body"]["error_code"],"PRESENTATION_INSPECTION_REQUIRED");
        result["model_body"]["message"].as_str().unwrap().to_owned()
    };
    let both=correction(4,"blocked-both");
    assert!(both.contains("narrow-content") && both.contains("short-content"));
    assert!(!both.contains("desktop-content") && !both.contains("other-environment"));
    let remaining=correction(6,"blocked-short");
    assert!(remaining.contains("short-content"));
    assert!(!remaining.contains("narrow-content") && !remaining.contains("other-environment"));
}

#[test]
fn presentation_restore_failure_loads_state_contract_without_replanning() {
    for phase_name in ["local", "review"] {
        let b = book();
        let mut port = AuthoringPort::new(&b, &format!("restore-guidance-{phase_name}"));
        port.recorded_preview = Some(AuthorResult {
            body: json!({"status":"preview_failed","candidate_id":"c1",
                "reading":{"page_state":{"page":2,"mode":"lexical"}},
                "errors":[{"kind":"state_restore_mismatch","message":"The button-driven result was lost after reopening."}]}),
            images: vec![], previewed_candidate: None, delivered: None,
        });
        let adapter = RequestPlanRecordingAdapter::new(vec![
            discover(),
            prepare("prepare", phase_name, "ACCEPTED_CONTENT", "REPAIR_CURRENT_PAGE", json!([])),
            write("write"),
            author("preview", json!({"operation":"preview","candidate_id":"c1"})),
            turn_final("Draft remains."),
        ], vec![]);
        let mut context = RunContext::new(new_session(), OuterConfig::default(), adapter.model_runtime_profile());
        execute(&mut port, &b, &adapter, &mut context).unwrap();
        let plans = adapter.seen_plans.borrow();
        phase(&plans[3], phase_name, &[]);
        phase(&plans[4], phase_name, &["state"]);
        let mut expected = design(&plans[3]).unwrap();
        expected["needs"] = json!(["state"]);
        assert_eq!(design(&plans[4]).unwrap(), expected, "a restore failure must not replace the page task");
        assert_eq!((port.writes, port.deliveries), (1, 0));
    }
}


#[test]
fn presentation_sampling_pressure_rebases_runtime_context_without_dropping_turn_evidence() {
    let b=book(); let registry=resident_tool_registry();
    let profile=ModelRuntimeProfile::fallback("pressure",ProviderToolProtocol::Native);
    let mut fragments=ContextFragmentLedger::default();
    let mut exposure=ToolExposureState::default(); exposure.activate("presentation.author");
    let mut results=ActiveToolResultLedger::default();
    let mut raw=new_session(); raw.push(Message::user("ORIGINAL_USER_DOUBT"));
    let mut previous_count=0; let mut retired=false;
    for step in 0..12 {
        let state=format!("CURRENT_STATE_{step} {}", "具体用户困惑与已经给过的帮助。".repeat(1300));
        fragments.upsert(ContextFragment::new("agent.learner_context",FragmentScope::Dynamic,Role::System,state,FragmentSensitivity::Private)).unwrap();
        let before=raw.clone();
        let (_,plan)=build_sample_request(&raw,&mut fragments,&b,&mut results,None,COMPACTION_CONSUMPTION_WRAPPER,&registry,&profile,
            ToolPermissions::default(),&exposure,ArtifactExposureContext::no_overlay(),EvidenceState::Unlocated,&[],None,false,None).unwrap();
        let count=plan.input.iter().filter(|message|message.content.as_deref().is_some_and(|text|text.starts_with("runtime_state_snapshot.v1\n"))).count();
        assert_eq!(raw,before);
        assert!(plan.active_context.fits);
        if previous_count>1 && count==1 {
            let all=serde_json::to_string(&plan.ordered_messages()).unwrap();
            assert!(all.contains(&format!("CURRENT_STATE_{step}")));
            assert!(!all.contains("CURRENT_STATE_0"));
            assert!(all.contains("ORIGINAL_USER_DOUBT") && all.contains("OBSERVED_TOOL_RESULT_0"));
            assert!(all.contains("resident-agent.presentation.phase.global"));
            retired=true; break;
        }
        previous_count=count;
        raw.push(Message {provider_continuation:None,role:Role::Assistant,content:None,tool_calls:vec![call(&format!("read-{step}"),"book.text",r#"{"lid":"1.1"}"#)],tool_call_id:None});
        raw.push(Message {provider_continuation:None,role:Role::Tool,content:Some(format!("OBSERVED_TOOL_RESULT_{step}")),tool_calls:vec![],tool_call_id:Some(format!("read-{step}"))});
    }
    assert!(retired,"real request pressure must invoke snapshot retirement before exhausting the model context");
}
