use super::*;

struct PauseAfterFirstSample<'a> {
    inner: BorrowedResidentState<'a>,
    checks: usize,
}
impl ResidentStatePort for PauseAfterFirstSample<'_> {
    fn tutor_active(&mut self) -> Result<bool, ToolError> { self.checks += 1; Ok(self.checks == 1) }
    fn submit_private<R>(&mut self, operation: impl FnOnce(&mut MemoryStore) -> R) -> Result<R, ToolError> { self.inner.submit_private(operation) }
    fn read_live_reader<R>(&mut self, operation: impl FnOnce(&Reader) -> R) -> Result<R, ToolError> { self.inner.read_live_reader(operation) }
    fn apply_reader<R>(&mut self, operation: impl FnOnce(&mut MemoryStore, &mut Reader) -> R) -> Result<R, ToolError> { self.inner.apply_reader(operation) }
    fn reader_input(&mut self, book: &Book, question: &str) -> crate::run_context::ReaderInputSnapshot { self.inner.reader_input(book, question) }
}

#[test]
fn tutor_pause_overrides_frozen_context_at_next_sample() {
    let b = book();
    let mut store = MemoryStore::open(tmp("tutor-frozen-pause")).unwrap();
    let mut reader = Reader::new(&b, DEFAULT_RADIUS);
    let snapshot = default_profile_snapshot(&b, &store, "t0");
    let adapter = RequestPlanRecordingAdapter::new(vec![turn_calls(vec![call("manifest", "book.manifest", "{}")]), turn_final("已暂停。")], vec![]);
    let mut context = RunContext::new(new_session(), OuterConfig::default(), adapter.model_runtime_profile());
    context.tutor = Some(serde_json::json!({"status":"active", "learner_context":{"current_request":"PAUSE_FROZEN_SENTINEL"}}));
    context.tutor_resolved = true;
    let mut state = PauseAfterFirstSample { inner: BorrowedResidentState { store: &mut store, reader: &mut reader }, checks: 0 };
    run_context(&b, &mut state, &adapter, &mut context, &snapshot, &ResidentTurnResources::default(), None,
        &mut EphemeralCompactionCheckpointSink::default(), "继续学习", "t0").unwrap();
    let plans = adapter.seen_plans.borrow();
    assert_eq!(plans.len(), 2);
    let text = serde_json::to_string(&plans[1].ordered_messages()).unwrap();
    assert_eq!(text.matches("PAUSE_FROZEN_SENTINEL").count(), 1);
    assert!(text.rfind("Current Tutor status: paused").unwrap() > text.rfind("Current Tutor status: active").unwrap());
    assert!(!plans[1].tools.iter().any(|tool| tool.name == "tutor.step"));
    assert!(!plans[1].instruction_assets.iter().any(|asset| asset.asset_id == "resident-agent.tutor.select"));
}

#[test]
fn tutor_tc2a_navigation_discovery_loads_workflow_on_next_sample() {
    let b = guided_read_book();
    let mut store = MemoryStore::open(tmp("tc2a-navigation-discovery")).unwrap();
    let mut reader = Reader::new(&b, 1);
    let adapter = RequestPlanRecordingAdapter::new(vec![
        turn_calls(vec![call("discover", "tool.search", r#"{"task":"find the route through the section","required_capabilities":["navigation_plan"],"scope":"section","operation":"navigate","effect_mode":"read_only","max_results":4}"#)]),
        turn_final("路线能力已找到。"),
    ], vec![]);
    run(&b, &mut store, &mut reader, &adapter, &mut new_session(), "梳理这里的关系", "t0", OuterConfig::default()).unwrap();
    let plans = adapter.seen_plans.borrow();
    assert_eq!(plans.len(), 2);
    assert!(!plans[0].instructions.contains("When establishing a new explicit guided-reading route"));
    assert!(plans[0].instructions.contains("navigation_plan"));
    assert!(plans[1].instructions.contains("When establishing a new explicit guided-reading route"));
    assert!(plans[1].tools.iter().any(|tool| tool.name == "book.guide_path"));
}

#[test]
fn tutor_tc2a_presentation_guidance_follows_discovery_phase_and_delivery() {
    let b = book();
    let tutor = serde_json::json!({"status":"active","learner_context":{"current_request":""}});
    let registry = resident_tool_registry().with_tutor_context(Some(&tutor));
    let mut exposure = ToolExposureState::default();
    exposure.activate_tutor();
    let profile = ModelRuntimeProfile::fallback("tc2a-presentation", ProviderToolProtocol::Native);
    let sample = |state: &ToolExposureState, authoring: Option<&crate::presentation_author::PresentationAuthoringContext>, finished| {
        build_sample_request(&new_session(), &mut ContextFragmentLedger::default(), &b,
            &ActiveToolResultLedger::default(), None, COMPACTION_CONSUMPTION_WRAPPER, &registry, &profile,
            ToolPermissions::default(), state, ArtifactExposureContext::no_overlay(), EvidenceState::UserProvided,
            &[], authoring, finished, Some(&tutor)).unwrap()
    };
    let (initial_exposure, initial) = sample(&exposure, None, false);
    assert!(!initial.instruction_assets.iter().any(|a| a.asset_id == "resident-agent.tutor.presentation"));
    search_and_activate(r#"{"task":"explain with an interactive scene","required_capabilities":["presentation_authoring"],"scope":"passage","operation":"explain","effect_mode":"read_only","max_results":1}"#,
        &ToolExposureContext { content_profile:b.content_profile_id(), permissions:ToolPermissions::default(),
            evidence_state:EvidenceState::UserProvided, artifact:ArtifactExposureContext::no_overlay() },
        &initial_exposure, &registry, &mut exposure).unwrap();
    for (phase, expected) in [(PresentationPhase::Global,"Teaching presentation planning"),
        (PresentationPhase::Local,"Teaching presentation construction"), (PresentationPhase::Review,"Teaching presentation review")] {
        let authoring = crate::presentation_author::PresentationAuthoringContext { phase,framework:None,focus:None,needs:vec![] };
        let (_, plan) = sample(&exposure, Some(&authoring), false);
        assert!(plan.tools.iter().any(|t| t.name == "presentation.author"));
        assert!(serde_json::to_string(&plan.ordered_messages()).unwrap().contains(expected));
        assert!(plan.instruction_assets.iter().any(|a| a.asset_id == "resident-agent.tutor.select"));
    }
    let (_, delivered) = sample(&exposure, None, true);
    assert!(!delivered.instruction_assets.iter().any(|a| a.asset_id == "resident-agent.tutor.presentation"));
    assert!(delivered.instruction_assets.iter().any(|a| a.asset_id == "resident-agent.tutor.select"));
}

struct ExpandingCompactor { inner: AutoCompactionAdapter }
impl ModelAdapter for ExpandingCompactor {
    fn complete(&self, _: CompletionRequest) -> Result<ParsedResponse, AdapterError> { unreachable!() }
    fn chat(&self, _: &AgentRequestPlan) -> Result<AssistantTurn, AdapterError> { unreachable!() }
    fn complete_structured(&self, request: CompletionRequest) -> Result<serde_json::Value, AdapterError> {
        let mut draft = self.inner.complete_structured(request)?;
        let mut ids = Vec::new();
        let items = (0..8).map(|i| {
            let mut item = draft["active_goal"][0].clone();
            let id = format!("expanded.{i}");
            ids.push(id.clone());
            item["item_id"] = serde_json::json!(id);
            item["text"] = serde_json::json!("oversized summary ".repeat(180));
            item
        }).collect::<Vec<_>>();
        draft["active_goal"] = serde_json::json!(items);
        for coverage in draft["source_coverage"].as_array_mut().unwrap() {
            coverage["target_item_ids"] = serde_json::json!(ids);
        }
        Ok(draft)
    }
}

#[test]
fn compaction_adoption_uses_actual_budget_and_remembers_no_benefit() {
    for (case, expands, current_chars, expected_error) in [
        ("useful-above-soft-target", false, 80_000, None),
        ("valid-no-benefit", true, 80_000, None),
        ("neither-fits", false, 500_000, Some(ACTIVE_CONTEXT_EXHAUSTED)),
    ] {
        let b = book();
        let mut profile = compaction_profile(case);
        profile.compaction.high_watermark_ratio = 0.02;
        let normal = AutoCompactionAdapter::new(profile.clone(), vec![]);
        let expanding = ExpandingCompactor { inner: AutoCompactionAdapter::new(profile.clone(), vec![]) };
        let adapter: &dyn ModelAdapter = if expands { &expanding } else { &normal };
        let mut messages = vec![Message::system("base"), Message::user("old ".repeat(3_000)), Message::user("current ".repeat(current_chars / 8))];
        let original = messages.clone();
        let ledger = ContextFragmentLedger::default();
        let body_ledger = ActiveToolResultLedger::default();
        let project = |raw: &[Message], checkpoint: Option<&CompactionCheckpoint>| {
            let projected = messages_with_context_fragments(raw, &ledger, &b, &body_ledger, checkpoint, COMPACTION_CONSUMPTION_WRAPPER).map_err(compaction_error)?;
            Ok::<_, ToolError>(ActiveContextBudget::from_plan(&AgentRequestPlan::for_agent_turn(profile.clone(), &projected, &[])))
        };
        let budget = project(&messages, None).unwrap();
        assert!(budget.over_high_watermark);
        let mut checkpoint = None;
        let mut attempted = None;
        let mut sink = EphemeralCompactionCheckpointSink::default();
        let result = maybe_auto_compact(CompactionPhase::MidTurn, budget, &b, &mut messages, adapter, &profile,
            &ledger, &[], &mut checkpoint, &mut sink, &mut attempted, |raw, candidate| project(raw, Some(candidate)));
        if let Some(error) = expected_error {
            assert_eq!(result.unwrap_err().error_code, error);
            assert!(sink.installed.is_none());
        } else if expands {
            assert!(!result.unwrap());
            assert_eq!(messages, original);
            assert!(sink.installed.is_none());
            assert!(!maybe_auto_compact(CompactionPhase::MidTurn, budget, &b, &mut messages, adapter, &profile,
                &ledger, &[], &mut checkpoint, &mut sink, &mut attempted, |raw, candidate| project(raw, Some(candidate))).unwrap());
            assert_eq!(expanding.inner.compaction_requests.borrow().len(), 1);
        } else {
            assert!(result.unwrap());
            assert!(sink.installed.is_some());
            let after = project(&messages, checkpoint.as_ref()).unwrap();
            assert!(after.fits && after.over_high_watermark);
            assert!(after.estimated_input_tokens < budget.estimated_input_tokens);
        }
    }
}

#[test]
fn tutor_turn_context_appears_once_across_five_samples() {
    let b = book_leaves(4);
    let mut store = MemoryStore::open(tmp("tutor-frozen-five-samples")).unwrap();
    let mut reader = Reader::new(&b, DEFAULT_RADIUS);
    let snapshot = default_profile_snapshot(&b, &store, "t0");
    let mut turns = (0..4).map(|i| turn_calls(vec![call(&format!("manifest-{i}"), "book.text", &format!(r#"{{"lid":"1.{}"}}"#, i+1))])).collect::<Vec<_>>();
    turns.push(turn_final("教学内容。"));
    let adapter = RequestPlanRecordingAdapter::new(turns, vec![]);
    let mut context = RunContext::new(new_session(), OuterConfig { max_turns: Some(8), ..Default::default() }, adapter.model_runtime_profile());
    context.tutor = Some(serde_json::json!({"status":"active", "learner_context":{"current_request":"FROZEN_LEARNER_SENTINEL"}}));
    context.tutor_resolved = true;
    run_context(&b, &mut BorrowedResidentState {store: &mut store, reader: &mut reader}, &adapter,
        &mut context, &snapshot, &ResidentTurnResources::default(), None,
        &mut EphemeralCompactionCheckpointSink::default(), "读取 1.1、1.2、1.3 和 1.4，继续学习", "t0").unwrap();
    let plans = adapter.seen_plans.borrow();
    assert_eq!(plans.len(), 5);
    for (index, plan) in plans.iter().enumerate() {
        let text = serde_json::to_string(&plan.ordered_messages()).unwrap();
        assert_eq!(text.matches("FROZEN_LEARNER_SENTINEL").count(), 1, "sample {index}");
    }
}

#[test]
fn tiny_eligible_history_does_not_abort_or_regenerate_compaction() {
    let b = book();
    let mut store = MemoryStore::open(tmp("compaction-short-old-turn")).unwrap();
    let mut reader = Reader::new(&b, DEFAULT_RADIUS);
    let mut profile = compaction_profile("short-old-turn");
    profile.compaction.high_watermark_ratio = 0.02;
    let adapter = AutoCompactionAdapter::new(profile, vec![
        turn_calls(vec![call("manifest", "book.manifest", "{}")]),
        turn_final("继续讲解。"),
    ]);
    let mut messages = new_session();
    messages.push(Message::user("继续学习"));
    run(&b, &mut store, &mut reader, &adapter, &mut messages, "开始给我讲讲1.4吧", "t0", OuterConfig::default()).unwrap();
    assert_eq!(adapter.seen_messages.borrow().len(), 2);
    assert!(adapter.compaction_requests.borrow().is_empty(), "fixed checkpoint overhead already exceeds the old message");
    assert_eq!(messages[1].content.as_deref(), Some("继续学习"));
}

#[test]
fn compaction_candidate_retains_large_current_body_and_may_miss_soft_target() {
    let b = book();
    let profile = compaction_profile("current-body-comparison");
    let adapter = AutoCompactionAdapter::new(profile.clone(), vec![]);
    let mut messages = new_session();
    messages.push(Message::user("old question ".repeat(500)));
    messages.push(Message { role: Role::Assistant, content: Some("old answer ".repeat(500)), tool_calls: vec![], tool_call_id: None, provider_continuation: None });
    messages.push(Message::user("current question"));
    messages.push(Message { role: Role::Assistant, content: None,
        tool_calls: vec![call("read", "book.text", "{\"lid\":\"1\"}")], tool_call_id: None, provider_continuation: None });
    messages.push(Message { role: Role::Tool, content: Some("current source body ".repeat(10_000)), tool_call_id: Some("read".into()), tool_calls: vec![], provider_continuation: None });
    let prepared = prepare_history_compaction(&b, &messages, CompactionPhase::MidTurn, vec![], vec![], vec![], BTreeMap::new()).unwrap();
    let checkpoint = compact_with_adapter(&adapter, &profile, &prepared, CompactionLimits {
        generation_input_limit_tokens: 100_000, target_active_tokens: 100,
    }).expect("generation returns a validated candidate; adoption uses the actual request budget");
    let projected = messages_with_context_fragments(&messages, &ContextFragmentLedger::default(), &b,
        &ActiveToolResultLedger::default(), Some(&checkpoint), COMPACTION_CONSUMPTION_WRAPPER).unwrap();
    assert!(projected.iter().any(|m| m.tool_call_id.as_deref() == Some("read") && m.content.as_ref().unwrap().contains("current source body")));
}
