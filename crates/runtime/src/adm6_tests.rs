use super::*;
use crate::model_spend::SpendStop;

struct SpendAdapter {
    chats: RefCell<VecDeque<AssistantTurn>>,
    calls: Cell<usize>,
    reason: SpendStop,
}
impl SpendAdapter {
    fn new(chats: Vec<AssistantTurn>, reason: SpendStop) -> Self {
        Self {
            chats: RefCell::new(chats.into()),
            calls: Cell::new(0),
            reason,
        }
    }
}
impl ModelAdapter for SpendAdapter {
    fn complete(&self, _: CompletionRequest) -> Result<ParsedResponse, AdapterError> {
        self.calls.set(self.calls.get() + 1);
        Err(self.reason.into())
    }
    fn complete_structured(&self, _: CompletionRequest) -> Result<serde_json::Value, AdapterError> {
        self.calls.set(self.calls.get() + 1);
        Err(self.reason.into())
    }
    fn chat(&self, _: &AgentRequestPlan) -> Result<AssistantTurn, AdapterError> {
        self.calls.set(self.calls.get() + 1);
        self.chats
            .borrow_mut()
            .pop_front()
            .ok_or_else(|| self.reason.into())
    }
}

#[test]
fn adm6_outer_repair_query_synthesis_and_finalization_stop_without_more_tools() {
    for purpose in ["outer", "repair", "query", "synthesize", "finalization"] {
        for reason in [
            SpendStop::MissingScope,
            SpendStop::RateUnavailable,
            SpendStop::InsufficientAllowance,
            SpendStop::AllowanceExpired,
            SpendStop::StorageUnavailable,
            SpendStop::PermissionRevoked,
            SpendStop::ReconciliationRequired,
        ] {
            let b = book();
            let script = match purpose {
                "outer" => vec![],
                "repair" => vec![turn_final("Unregistered source [[source:missing]]")],
                "query" => vec![turn_calls(vec![
                    call(
                        "query",
                        "book.query",
                        r#"{"query":"command","intent":"definition","targets":["command"],"obligations":[{"requirement":"define"}],"anchor_lid":"1.1"}"#,
                    ),
                    call(
                        "after-stop",
                        "reader.note",
                        r#"{"lid":"1.1","text":"must not execute"}"#,
                    ),
                ])],
                "synthesize" => vec![turn_calls(vec![
                    call(
                        "synthesize",
                        "book.synthesize",
                        r#"{"lids":["1.1"],"task":"explain this passage"}"#,
                    ),
                    call(
                        "after-stop",
                        "reader.note",
                        r#"{"lid":"1.1","text":"must not execute"}"#,
                    ),
                ])],
                _ => vec![turn_calls(vec![call("manifest", "book.manifest", "{}")])],
            };
            let adapter = SpendAdapter::new(script, reason);
            let mut store = MemoryStore::open(tmp("adm6-purposes")).unwrap();
            let mut reader = Reader::new(&b, DEFAULT_RADIUS);
            let mut messages = new_session();
            let result = run(
                &b,
                &mut store,
                &mut reader,
                &adapter,
                &mut messages,
                "解释当前这一段 1.1 的 command",
                "t0",
                OuterConfig {
                    max_turns: (purpose == "finalization").then_some(1),
                    ..Default::default()
                },
            );
            let error = result.unwrap_err();
            assert_eq!(error.error_code, reason.code(), "{purpose}");
            assert_eq!(error.category, "model_spend");
            assert_eq!(
                adapter.calls.get(),
                if purpose == "outer" { 1 } else { 2 },
                "{purpose}"
            );
            assert!(!messages
                .iter()
                .any(|m| m.tool_call_id.as_deref() == Some("after-stop")));
            assert!(store
                .recall(&memory::RecallQuery {
                    text: Some("must not execute".into()),
                    ..Default::default()
                })
                .is_empty());
        }
    }
}

#[test]
fn adm6_compaction_stop_keeps_original_history_and_checkpoint() {
    let b = book();
    let mut messages = completed_history("adm6-original", 12_000);
    messages.push(Message::user("current question"));
    let original = serde_json::to_vec(&messages).unwrap();
    let adapter = SpendAdapter::new(vec![], SpendStop::InsufficientAllowance);
    let mut checkpoint = None;
    let mut sink = EphemeralCompactionCheckpointSink::default();
    let error = maybe_auto_compact(
        CompactionPhase::MidTurn,
        ActiveContextBudget {
            estimated_input_tokens: 90_000,
            reserved_tokens: 12_000,
            pressure_tokens: 102_000,
            high_watermark_tokens: 96_000,
            target_input_tokens: 84_000,
            over_high_watermark: true,
            fits: true,
        },
        &b,
        &mut messages,
        &adapter,
        &compaction_profile("adm6"),
        &ContextFragmentLedger::default(),
        &[],
        &mut checkpoint,
        &mut sink,
    )
    .unwrap_err();
    assert_eq!(error.error_code, SpendStop::InsufficientAllowance.code());
    assert_eq!(adapter.calls.get(), 1);
    assert!(checkpoint.is_none() && sink.installed.is_none());
    assert_eq!(serde_json::to_vec(&messages).unwrap(), original);
}

#[test]
fn adm6_tutor_and_observation_keep_typed_reason_without_text_matching() {
    let inner = SpendAdapter::new(vec![], SpendStop::ReconciliationRequired);
    let adapter = crate::run_events::ObservedAdapter {
        inner: &inner,
        events: Default::default(),
        cancellation: Default::default(),
        runtime_profile: inner.model_runtime_profile(),
    };
    let error = crate::tutor::evaluate(
        &adapter,
        inner.model_runtime_profile(),
        serde_json::json!({}),
    )
    .unwrap_err();
    assert_eq!(error.spend_stop, Some(SpendStop::ReconciliationRequired));
    assert_eq!(inner.calls.get(), 1);
    let activities = adapter.events.activities();
    assert_eq!(activities[0].name, "tutor_assessment");
    assert_eq!(
        activities[0].error_code.as_deref(),
        Some(SpendStop::ReconciliationRequired.code())
    );
    let ordinary = AdapterError {
        spend_stop: None,
        message: "ALLOWANCE_INSUFFICIENT".into(),
    }
    .into_tool_error();
    assert_eq!(ordinary.error_code, "PROVIDER_ERROR");
}

#[test]
fn adm6_tutor_tool_stops_run_before_next_tool_and_leaves_response_unassessed() {
    struct TutorPort<'a>(BorrowedResidentState<'a>);
    impl ResidentStatePort for TutorPort<'_> {
        fn submit_private<R>(
            &mut self,
            f: impl FnOnce(&mut MemoryStore) -> R,
        ) -> Result<R, ToolError> {
            self.0.submit_private(f)
        }
        fn read_live_reader<R>(&mut self, f: impl FnOnce(&Reader) -> R) -> Result<R, ToolError> {
            self.0.read_live_reader(f)
        }
        fn apply_reader<R>(
            &mut self,
            f: impl FnOnce(&mut MemoryStore, &mut Reader) -> R,
        ) -> Result<R, ToolError> {
            self.0.apply_reader(f)
        }
        fn reader_input(&mut self, b: &Book, q: &str) -> crate::run_context::ReaderInputSnapshot {
            self.0.reader_input(b, q)
        }
        fn tutor_assessment_input(&mut self, _: &str) -> Result<serde_json::Value, ToolError> {
            Ok(serde_json::json!({"existing":null}))
        }
        fn tutor_assessment_accept(
            &mut self,
            _: &str,
            _: serde_json::Value,
        ) -> Result<serde_json::Value, ToolError> {
            panic!("stopped assessment must not be accepted")
        }
    }
    let b = book();
    let mut store = MemoryStore::open(tmp("adm6-tutor")).unwrap();
    let snapshot = default_profile_snapshot(&b, &store, "t0");
    let mut reader = Reader::new(&b, DEFAULT_RADIUS);
    let adapter = SpendAdapter::new(
        vec![turn_calls(vec![
            call(
                "assess",
                "tutor.step",
                r#"{"operation":"assess","action_ref":"original-response"}"#,
            ),
            call(
                "after-stop",
                "reader.note",
                r#"{"lid":"1.1","text":"must not execute"}"#,
            ),
        ])],
        SpendStop::InsufficientAllowance,
    );
    let mut context = RunContext::new(
        new_session(),
        OuterConfig::default(),
        adapter.model_runtime_profile(),
    );
    context.tutor = Some(serde_json::json!({"status":"active"}));
    let error = run_context(
        &b,
        &mut TutorPort(BorrowedResidentState {
            store: &mut store,
            reader: &mut reader,
        }),
        &adapter,
        &mut context,
        &snapshot,
        &ResidentTurnResources::default(),
        None,
        &mut EphemeralCompactionCheckpointSink::default(),
        "评价我的回答",
        "t0",
    )
    .unwrap_err();
    assert_eq!(error.error_code, "ALLOWANCE_INSUFFICIENT");
    assert_eq!(adapter.calls.get(), 2);
    assert_eq!(context.trace.len(), 1);
    context.close_stopped_tool_calls(&error.error_code, &error.category, &error.message);
    let closing: serde_json::Value =
        serde_json::from_str(tool_result(&context.messages, "after-stop")).unwrap();
    assert_eq!(closing["error_code"], "ALLOWANCE_INSUFFICIENT");
    assert!(store
        .recall(&memory::RecallQuery {
            text: Some("must not execute".into()),
            ..Default::default()
        })
        .is_empty());
}
