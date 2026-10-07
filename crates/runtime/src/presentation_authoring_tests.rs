use super::*;
use crate::presentation_author::{AuthorRequest, AuthorResult, PreviewImage};
use serde_json::{json, Value};

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
        (1, "global", json!([]), 1), (2, "local", json!(["state"]), 2),
        (3, "local", json!(["state"]), 2), (4, "local", json!(["state"]), 2),
        (5, "local", json!(["editing"]), 3), (6, "local", json!(["editing"]), 3),
        (7, "local", json!(["editing"]), 3), (8, "review", json!([]), 4),
        (9, "inactive", Value::Null, 5), (10, "global", json!([]), 6),
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
            assert_eq!(args["saved_source"]["candidate_id"], "c1");
            expected_before.input.iter_mut().flat_map(|m| &mut m.tool_calls).find(|c| c.id == "write").unwrap().arguments = saved.arguments.clone();
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
        assert_eq!(value["saved_source"]["candidate_id"], format!("c{}", index + 1));
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
}
impl AuthoringPort {
    fn new(book: &Book, label: &str) -> Self {
        Self {
            store: MemoryStore::open(tmp(label)).unwrap(),
            reader: Reader::new(book, 1),
            writes: 0,
            deliveries: 0,
        }
    }
}
impl ResidentStatePort for AuthoringPort {
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
        let mut result = AuthorResult {
            body: Value::Null,
            images: vec![],
            previewed_candidate: None,
            delivered: None,
        };
        match request {
            AuthorRequest::Prepare { .. } => panic!("prepare reached host storage"),
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
            AuthorRequest::Preview { candidate_id, .. } => {
                result.body =
                    json!({"status":"preview_ready_for_inspection", "candidate_id":candidate_id});
                result.previewed_candidate = Some(candidate_id.clone());
                result.images.push(PreviewImage {
                    caption: format!("{candidate_id} actual fixture observation"),
                    png_base64: "png".into(),
                    candidate_id: Some(candidate_id),
                    environment_name: Some("desktop".into()),
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
    assert!(plan.instructions.contains("subsequent model sampling"));
    assert!(plan.instructions.contains("source.present"));
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
    let b = book();
    let mut port = AuthoringPort::new(&b, "ex12-compaction");
    let messages = completed_history("OLD_COMPLETED_HISTORY", 35_000);
    let snapshot = default_profile_snapshot(&b, &port.store, "t0");
    let profile = tune_profile_just_above_initial_pressure(
        &b,
        &snapshot,
        &port.reader,
        &messages,
        "Explain the relationship",
        compaction_profile("ex12-compaction"),
        10_000,
    );
    let local = prepare(
        "latest",
        "local",
        "CURRENT_FRAME",
        "CURRENT_FOCUS",
        json!(["editing"]),
    );
    // Pressure comes from preserved current-turn conversation, not the brief design record.
    let mut next = write("pressure-after-selection");
    next.text = Some("Current working material. ".repeat(2_000));
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
    assert!(after.contains("Presentation phase: local"));
    assert_eq!(after.matches("Presentation phase: local").count(), 1, "compaction lost or duplicated active guidance");
    assert_eq!(sampled[3][0].content, sampled[4][0].content, "compaction rebuilt different common instructions");
    let saved = sampled[4].iter().flat_map(|m| &m.tool_calls).find(|c| c.id == "saved-before-pressure").unwrap();
    let args: Value = serde_json::from_str(&saved.arguments).unwrap();
    assert!(args.get("html").is_none());
    assert_eq!(args["saved_source"]["candidate_id"], "c1");
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
