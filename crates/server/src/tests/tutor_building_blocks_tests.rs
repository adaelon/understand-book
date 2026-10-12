//! Fixed real-source Tutor scenarios reused for TB1/TB2 comparisons.
use super::*;
use runtime::model_spend::{ChargeScope, ModelSpendPort, SendIdentity, SendOutcome, SpendStop};
use runtime::provider_stream::ModelDelta;
use std::sync::Mutex;

const SOURCE: &str =
    include_str!("../../../../docs/performance/tutor-building-blocks-tb1/source.txt");
const SCENARIOS: &str =
    include_str!("../../../../docs/performance/tutor-building-blocks-tb1/scenarios.json");
const WINDOW: u64 = 1_000_000;
const TOKEN_LIMIT: u64 = 3_000_000;

fn save(directory: &Path, name: &str, value: &impl serde::Serialize) {
    std::fs::write(
        directory.join(name),
        serde_json::to_vec_pretty(value).unwrap(),
    )
    .unwrap();
}

pub(super) fn fixture() -> (tempfile::TempDir, AppState) {
    let mut base = sample_base();
    base.book_id = "tb1-mastering-rust-borrowing-v1".into();
    base.graph_nodes.clear();
    base.graph_edges.clear();
    for node in &mut base.lid_nodes {
        node.span = Span {
            start: 0,
            end: SOURCE.encode_utf16().count(),
        };
    }
    let (root, mut state) =
        super::tutor_loop_tests::fixture_with_book(false, Book::new(base, SOURCE), SOURCE);
    let learning = state.user.learning_store().unwrap().state().unwrap();
    let reply = post(
        &mut state,
        "/tutor/mutate",
        &json!({
            "operation_id":"tb1-mode","expected_revision":learning.control.revision,
            "action":{"kind":"set_mode","session_id":learning.control.current_tutor_session_id,
                "mode":"guided_inquiry"}
        })
        .to_string(),
    );
    assert_eq!(reply.status, 200, "{}", reply.body);
    (root, state)
}

fn input(case: &Value) -> Value {
    json!({"message":format!("{}\n{}",case["background"].as_str().unwrap(),case["request"].as_str().unwrap())})
}

#[test]
fn tutor_tb1_fixture_replays_distinct_starts_and_source() {
    let cases: Value = serde_json::from_str(SCENARIOS).unwrap();
    assert_eq!(cases["cases"][0]["request"], cases["cases"][1]["request"]);
    assert_ne!(
        cases["cases"][0]["background"],
        cases["cases"][1]["background"]
    );
    for case in cases["cases"].as_array().unwrap() {
        let (_root, mut state) = fixture();
        let request = input(case);
        let run = prepare_agent_chat(&mut state, &request.to_string(), "tb1")
            .unwrap_or_else(|r| panic!("{}", r.body));
        let context = run.tutor.as_ref().unwrap();
        assert_eq!(context["status"], "active");
        assert_eq!(context["default_teaching_intent"], "guided_inquiry");
        assert_eq!(
            context["learner_context"]["current_request"],
            request["message"]
        );
        assert!(context["learner_context"]["interpretations"]
            .as_array()
            .unwrap()
            .is_empty());
        // The production source tool must return the complete frozen excerpt.
        let samples = Arc::new(Mutex::new(Vec::new()));
        let direct = case["id"] == "direct-explanation";
        let movement = json!({"operation":"select","move":{
            "move_id":"fixture-probe","object_ids":[],"capability":"explanation",
            "kind":if direct {"explain"} else {"question"},
            "interaction_intent":if direct {"direct_explanation"} else {"guided_inquiry"},
            "intent_origin":if direct {"current_request"} else {"session_default"},
            "intent_quote":if direct {case["request"].clone()} else {Value::Null},
            "prompt":"fixture transport probe","actions":[],
            "target":{"learning_focus":"shared borrowing","expected_performance":"Distinguish ownership from borrowed access",
                "capability":"explanation","source_bindings":[{"start_lid":"1.1","end_lid":"1.1"}],"object_refs":[]}
        }});
        let adapter = super::tutor_continuation_tests::TutorRequestRecorder {
            samples: samples.clone(),
            directory: None,
            inner: Box::new(ChatStubAdapter::scripted(vec![
                AssistantTurn {
                    provider_continuation: None,
                    text: None,
                    usage_total_tokens: Some(1),
                    tool_calls: vec![ToolCall {
                        id: "read".into(),
                        name: "book.text".into(),
                        arguments: json!({"lid":"1.1","end_lid":"1.1"}).to_string(),
                    }],
                },
                AssistantTurn {
                    provider_continuation: None,
                    text: None,
                    usage_total_tokens: Some(1),
                    tool_calls: vec![ToolCall {
                        id: "select".into(),
                        name: "tutor.step".into(),
                        arguments: movement.to_string(),
                    }],
                },
                AssistantTurn {
                    provider_continuation: None,
                    text: Some("fixture transport probe".into()),
                    usage_total_tokens: Some(1),
                    tool_calls: vec![],
                },
            ])),
        };
        let result = agent_run::execute_prepared(
            &agent_run::BorrowedAppPort(RefCell::new(&mut state)),
            &adapter,
            run,
            Default::default(),
        );
        assert_eq!(result.reply.status, 200, "{}", result.reply.body);
        let captured = samples.lock().unwrap();
        assert!(captured
            .iter()
            .flat_map(|r| r["input"].as_array().unwrap())
            .any(|m| m["role"] == json!(runtime::Role::Tool)
                && m["content"]
                    .as_str()
                    .and_then(|c| serde_json::from_str::<Value>(c).ok())
                    .is_some_and(|body| body["model_body"]["text"]
                        .as_str()
                        .is_some_and(|text| text.trim() == SOURCE.trim()))));
    }
}

#[derive(Default)]
struct Totals {
    calls: usize,
    used: u64,
    stopped: bool,
}

// Replay frozen model output through real tools/storage. This checks engineering
// acceptance, not the model's tool planning or a live Provider connection.
#[test]
fn tutor_tb2_sol_outputs_replay_through_real_delivery() {
    struct RecordedTeaching {
        output: Value,
        sources: Vec<Value>,
        stage: std::cell::Cell<usize>,
    }
    impl RecordedTeaching {
        fn rebound(&self, plan: &AgentRequestPlan) -> Value {
            let mut serialized = self.output.to_string();
            for (index, source) in self.sources.iter().enumerate() {
                let call_id = format!("source-{index}");
                let result = plan.input.iter().find(|m| m.tool_call_id.as_deref() == Some(&call_id)).unwrap();
                let body: Value = serde_json::from_str(result.content.as_deref().unwrap()).unwrap();
                let actual = body["model_body"]["source_ref_id"].as_str().unwrap();
                serialized = serialized.replace(source["source_ref_id"].as_str().unwrap(), actual);
            }
            serde_json::from_str(&serialized).unwrap()
        }
    }
    impl ModelAdapter for RecordedTeaching {
        fn complete(&self, _: CompletionRequest) -> Result<ParsedResponse, AdapterError> { unreachable!("no model helpers in replay") }
        fn chat(&self, plan: &AgentRequestPlan) -> Result<AssistantTurn, AdapterError> {
            let stage = self.stage.get();
            self.stage.set(stage + 1);
            let call = |id: &str, name: &str, arguments: Value| ToolCall {
                id: id.into(), name: name.into(), arguments: arguments.to_string(),
            };
            let (text, tool_calls) = match stage {
                0 => (None, vec![call("read", "book.text", json!({"lid":"1.1","end_lid":"1.1"}))]),
                1 => (None, self.sources.iter().enumerate().map(|(index, source)|
                    call(&format!("source-{index}"), "source.present", json!({"quote":source["quote"]}))).collect()),
                2 => (None, vec![call("select", "tutor.step", self.rebound(plan)["select"].clone())]),
                3 => (Some(self.rebound(plan)["answer"].as_str().unwrap().to_string()), vec![]),
                _ => panic!("frozen output required an unexpected repair: {:?}", plan.input.last()),
            };
            Ok(AssistantTurn { provider_continuation: None, text, tool_calls, usage_total_tokens: None })
        }
    }
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/performance/tutor-building-blocks-tb2/sol");
    let cases: Value = serde_json::from_str(SCENARIOS).unwrap();
    for variant in ["before", "after", "prerequisite-repair"] {
        for case in cases["cases"].as_array().unwrap() {
            if variant == "prerequisite-repair" && case["id"] != "starting-assignment" { continue; }
            let dir = directory.join(variant).join(case["id"].as_str().unwrap());
            let packet: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("input.json")).unwrap()).unwrap();
            let output: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("output.json")).unwrap()).unwrap();
            let adapter = RecordedTeaching { output, sources: packet["presented_sources"].as_array().unwrap().clone(), stage: Default::default() };
            let (_root, mut state) = fixture();
            let run = prepare_agent_chat(&mut state, &input(case).to_string(), "tb2-sol").unwrap_or_else(|r| panic!("{}", r.body));
            let session = run.tutor.as_ref().unwrap()["binding"]["tutor_session_id"].as_str().unwrap().to_string();
            let turn = run.turn_ref.clone();
            let result = agent_run::execute_prepared(&agent_run::BorrowedAppPort(RefCell::new(&mut state)), &adapter, run, Default::default());
            assert_eq!(result.reply.status, 200, "{variant}/{}: {}", case["id"], result.reply.body);
            let saved = state.user.agent_history.sessions.iter().find(|s| s.id == turn.session_id).unwrap().turns.last().unwrap();
            let outcome = saved.outcome.as_ref().unwrap();
            assert!(!outcome.incomplete, "{variant}/{}", case["id"]);
            assert!(!outcome.answer_view.as_ref().unwrap().sources.is_empty());
            assert!(!outcome.answer.as_ref().unwrap().contains("source_ref_"));
            let store = state.user.learning_store().unwrap();
            let facts = store.teaching_turn_events(&session, &turn.turn_id).unwrap();
            assert_eq!(facts.iter().filter(|f| f.kind == memory::teaching::TeachingFact::MoveSelected).count(), 1);
            assert_eq!(facts.iter().filter(|f| f.kind == memory::teaching::TeachingFact::MessageDelivered).count(), 1);
            assert!(store.evidence_page(&state.workspace.book.base.book_id, 0, 100).unwrap().is_empty());
            save(&dir, "replay.json", &json!({"http_status":result.reply.status,"outcome":outcome,"facts":facts,
                "external_provider_calls":0,"source_identity_rebound_from_exact_quotes":true}));
        }
    }
}
struct Budget {
    total: Arc<Mutex<Totals>>,
    calls: Mutex<usize>,
    pending: Mutex<u64>,
    directory: PathBuf,
}
impl ModelSpendPort for Budget {
    fn before_send(&self, _: &SendIdentity, request: &Value) -> Result<(), SpendStop> {
        let mut total = self.total.lock().unwrap();
        let mut calls = self.calls.lock().unwrap();
        let output = request["max_tokens"]
            .as_u64()
            .ok_or(SpendStop::ReconciliationRequired)?;
        let reserve = WINDOW + output;
        if total.stopped || total.calls >= 60 || *calls >= 12 || total.used + reserve > TOKEN_LIMIT
        {
            return Err(SpendStop::InsufficientAllowance);
        }
        *calls += 1;
        total.calls += 1;
        total.used += reserve;
        *self.pending.lock().unwrap() = reserve;
        Ok(())
    }
    fn after_send(&self, identity: &SendIdentity, outcome: &SendOutcome) -> Result<(), SpendStop> {
        let mut total = self.total.lock().unwrap();
        let index = *self.calls.lock().unwrap();
        let reserve = *self.pending.lock().unwrap();
        let usage = outcome.usage.as_ref().and_then(|u| u.total_tokens);
        if let Some(tokens) = usage {
            total.used = total.used - reserve + u64::from(tokens);
        }
        total.stopped = usage.is_none() || !outcome.succeeded;
        save(
            &self.directory,
            &format!("transport-{index:02}.json"),
            &json!({
                "identity":identity,"outcome":outcome,"reserved_tokens":reserve,
                "cumulative_tokens_or_reserved":total.used,"total_http_calls":total.calls,"stopped":total.stopped
            }),
        );
        if total.stopped {
            Err(SpendStop::ReconciliationRequired)
        } else {
            Ok(())
        }
    }
}

#[test]
fn tutor_tb1_budget_stops_before_overspend_and_on_unknown_usage() {
    let root = tempfile::tempdir().unwrap();
    let total = Arc::new(Mutex::new(Totals::default()));
    let budget = Budget {
        total: total.clone(),
        calls: Mutex::new(0),
        pending: Mutex::new(0),
        directory: root.path().into(),
    };
    let id = SendIdentity {
        call_id: "test".into(),
        logical_call_id: "test".into(),
        attempt: 1,
        scope: None,
        purpose: "test".into(),
        model: "test".into(),
        provider: "test".into(),
    };
    assert_eq!(
        budget.before_send(&id, &json!({})),
        Err(SpendStop::ReconciliationRequired)
    );
    total.lock().unwrap().used = TOKEN_LIMIT - WINDOW;
    assert_eq!(
        budget.before_send(&id, &json!({"max_tokens":32768})),
        Err(SpendStop::InsufficientAllowance)
    );
    total.lock().unwrap().used = 0;
    *budget.calls.lock().unwrap() = 12;
    assert_eq!(
        budget.before_send(&id, &json!({"max_tokens":32768})),
        Err(SpendStop::InsufficientAllowance)
    );
    *budget.calls.lock().unwrap() = 0;
    budget
        .before_send(&id, &json!({"max_tokens":32768}))
        .unwrap();
    budget
        .after_send(
            &id,
            &SendOutcome {
                evidence: runtime::model_spend::SendEvidence::Response,
                usage: Some(runtime::provider_stream::ModelUsage {
                    total_tokens: Some(100),
                    ..Default::default()
                }),
                succeeded: true,
            },
        )
        .unwrap();
    assert_eq!(total.lock().unwrap().used, 100);
    budget
        .before_send(&id, &json!({"max_tokens":32768}))
        .unwrap();
    assert_eq!(
        budget.after_send(
            &id,
            &SendOutcome {
                evidence: runtime::model_spend::SendEvidence::OutcomeUnknown,
                usage: None,
                succeeded: false
            }
        ),
        Err(SpendStop::ReconciliationRequired)
    );
    assert_eq!(
        budget.before_send(&id, &json!({"max_tokens":32768})),
        Err(SpendStop::InsufficientAllowance)
    );
    assert_eq!(total.lock().unwrap().used, 1_032_868);
}

// Wire snapshots retain public content/tools; private continuation reasoning is counted, not published.
fn public_wire(mut value: Value) -> Value {
    fn redact(v: &mut Value) {
        match v {
            Value::Object(fields) => {
                if let Some(private) = fields.remove("reasoning_content") {
                    fields.insert(
                        "redacted_reasoning_chars".into(),
                        json!(private.as_str().map_or(0, |s| s.chars().count())),
                    );
                }
                for child in fields.values_mut() {
                    redact(child);
                }
            }
            Value::Array(values) => {
                for child in values {
                    redact(child);
                }
            }
            _ => {}
        }
    }
    redact(&mut value);
    value
}

struct WireRecorder {
    inner: Box<dyn ModelAdapter + Send>,
    directory: PathBuf,
    calls: std::cell::Cell<usize>,
}
impl WireRecorder {
    fn observe<T>(
        &self,
        run: impl FnOnce(&mut dyn runtime::provider_stream::ModelObserver) -> T,
    ) -> T {
        let index = self.calls.get();
        self.calls.set(index + 1);
        run(&mut |delta| match delta {
            ModelDelta::Request(body) => save(
                &self.directory,
                &format!("wire-request-{index:02}.json"),
                &public_wire((*body).clone()),
            ),
            ModelDelta::Response(body) => save(
                &self.directory,
                &format!("wire-response-{index:02}.json"),
                &public_wire((*body).clone()),
            ),
            ModelDelta::Usage(usage) => {
                save(&self.directory, &format!("usage-{index:02}.json"), &usage)
            }
            _ => {}
        })
    }
}
impl ModelAdapter for WireRecorder {
    fn model_runtime_profile(&self) -> runtime::ModelRuntimeProfile {
        self.inner.model_runtime_profile()
    }
    fn complete(&self, mut request: CompletionRequest) -> Result<ParsedResponse, AdapterError> {
        // Query helpers may omit a limit; keep the same finite output allowance
        // as this experiment's ordinary chat requests without changing production.
        request.output_token_limit.get_or_insert(32_768);
        self.observe(|observer| self.inner.complete_observed(request, observer))
    }
    fn complete_structured(&self, mut request: CompletionRequest) -> Result<Value, AdapterError> {
        request.output_token_limit.get_or_insert(32_768);
        self.observe(|observer| self.inner.complete_structured_observed(request, observer))
    }
    fn chat(&self, request: &AgentRequestPlan) -> Result<AssistantTurn, AdapterError> {
        // The tools-disabled terminal opportunity may omit an output limit too.
        // Bound the actual experimental request without rewriting its plan.
        let mut bounded = request.clone();
        bounded.output_token_limit.get_or_insert(32_768);
        self.observe(|observer| self.inner.chat_observed(&bounded, observer))
    }
}

#[test]
fn tutor_tb2_terminal_request_is_bounded_before_provider_dispatch() {
    struct Probe(u32);
    impl ModelAdapter for Probe {
        fn complete(&self, _: CompletionRequest) -> Result<ParsedResponse, AdapterError> {
            unreachable!()
        }
        fn chat(&self, request: &AgentRequestPlan) -> Result<AssistantTurn, AdapterError> {
            assert!(request.tools.is_empty());
            assert_eq!(request.output_token_limit, Some(self.0));
            Err(AdapterError { message: "terminal probe reached".into(), spend_stop: None })
        }
    }
    let root = tempfile::tempdir().unwrap();
    for (requested, expected) in [(None, 32768), (Some(1024), 1024)] {
        let recorder = WireRecorder {
            inner: Box::new(Probe(expected)), directory: root.path().into(), calls: Default::default(),
        };
        let mut request = AgentRequestPlan::for_ad_hoc(
            runtime::ModelRuntimeProfile::fallback("tb2-terminal", runtime::ProviderToolProtocol::Native),
            &[runtime::Message::user("Deliver the selected teaching text.")], &[],
        );
        request.output_token_limit = requested;
        assert_eq!(recorder.chat(&request).unwrap_err().message, "terminal probe reached");
        assert_eq!(request.output_token_limit, requested, "do not mutate the captured plan");
    }
}

#[test]
fn tutor_tb1_auxiliary_request_is_bounded_before_provider_dispatch() {
    struct Probe(u32);
    impl ModelAdapter for Probe {
        fn complete(&self, request: CompletionRequest) -> Result<ParsedResponse, AdapterError> {
            assert_eq!(request.output_token_limit, Some(self.0));
            assert_eq!(request.user, "query evidence");
            Err(AdapterError {
                message: "probe reached".into(),
                spend_stop: None,
            })
        }
        fn complete_structured(&self, request: CompletionRequest) -> Result<Value, AdapterError> {
            assert_eq!(request.output_token_limit, Some(self.0));
            Ok(json!({"candidate_fits":[],"plan_gate":"sufficient","probes":[]}))
        }
        fn chat(&self, _: &AgentRequestPlan) -> Result<AssistantTurn, AdapterError> {
            unreachable!()
        }
    }
    let root = tempfile::tempdir().unwrap();
    for (requested, expected) in [(None, 32768), (Some(1024), 1024)] {
        let recorder = WireRecorder {
            inner: Box::new(Probe(expected)),
            directory: root.path().into(),
            calls: Default::default(),
        };
        let result = recorder.complete(CompletionRequest {
            output_token_limit: requested,
            reasoning_effort: None,
            system: "query".into(),
            user: "query evidence".into(),
        });
        assert_eq!(result.unwrap_err().message, "probe reached");
        let outer = super::tutor_continuation_tests::TutorRequestRecorder {
            inner: Box::new(recorder),
            samples: Default::default(),
            directory: None,
        };
        // Query results without an answer field must keep the structured protocol
        // through both recording layers, rather than enter ParsedResponse parsing.
        let result = outer
            .complete_structured(CompletionRequest {
                output_token_limit: requested,
                reasoning_effort: None,
                system: "query".into(),
                user: "query evidence".into(),
            })
            .unwrap();
        assert_eq!(
            result,
            json!({"candidate_fits":[],"plan_gate":"sufficient","probes":[]})
        );
    }
}

#[test]
#[ignore = "TB1 four fixed real-model samples; fresh TB1_OUTPUT and original provider environment required"]
fn tutor_tb1_live_fixed_baseline() {
    let directory = PathBuf::from(std::env::var("TB1_OUTPUT").unwrap());
    assert!(!directory.exists(), "preserve every previous run");
    std::fs::create_dir_all(&directory).unwrap();
    let config = ProviderConfig::from_env().unwrap();
    assert_eq!(config.model, "deepseek-v4-flash");
    assert_eq!(config.mode.as_str(), "native");
    let mut cases: Value = serde_json::from_str(SCENARIOS).unwrap();
    if let Some(id) = std::env::var("TB1_CASE").ok().filter(|id| !id.is_empty()) {
        cases["cases"]
            .as_array_mut()
            .unwrap()
            .retain(|case| case["id"] == id);
        assert_eq!(
            cases["cases"].as_array().unwrap().len(),
            1,
            "unknown fixed case"
        );
    }
    let prior_output = std::env::var("TB1_PRIOR_OUTPUT")
        .ok()
        .filter(|path| !path.is_empty());
    let prior_budget: Value = prior_output
        .as_ref()
        .map(|path| {
            serde_json::from_slice(&std::fs::read(Path::new(path).join("budget.json")).unwrap())
                .unwrap()
        })
        .unwrap_or(Value::Null);
    assert_ne!(
        prior_budget["stopped"], true,
        "unknown usage cannot be reset for a follow-up"
    );
    save(
        &directory,
        "configuration.json",
        &json!({"model":config.model,"provider":config.mode.as_str(),
        "base_url":config.base_url,"temperature":0,"repetitions":1,"scenario_count":cases["cases"].as_array().unwrap().len(),
        "token_limit":TOKEN_LIMIT,"reserve_input_tokens":WINDOW,"http_calls_per_case":12,"http_calls_total":60,
        "request_timeout_seconds":120,"material_version":"tb1.mastering-rust.borrowing.v1",
        "common_code_baseline":"93b476da49b005def3f52b7b95bdaff4867e6e7b","cases":cases,
        "auxiliary_output_token_limit":32768,"prior_output":prior_output,"prior_budget":prior_budget,
        "reader_origin":"scripted; no human participant or post-delivery learner response"}),
    );
    let total = Arc::new(Mutex::new(Totals {
        calls: prior_budget["http_calls"].as_u64().unwrap_or(0) as usize,
        used: prior_budget["tokens_or_reserved"].as_u64().unwrap_or(0),
        stopped: false,
    }));
    for case in cases["cases"].as_array().unwrap() {
        let dir = directory.join(case["id"].as_str().unwrap());
        std::fs::create_dir(&dir).unwrap();
        let (_root, mut state) = fixture();
        // Prepare freezes the service adapter's model profile; use the real configured
        // adapter here too, rather than inheriting the fixture's unknown-model profile.
        state.services.adapter = ProviderRegistry::adapter_from_config_with_timeout(
            config.clone(),
            Duration::from_secs(120),
        );
        state.services.adapter.set_spend_context(
            ChargeScope::OperatorTask {
                task_ref: format!("tb1-{}", case["id"].as_str().unwrap()),
            },
            Some(Arc::new(Budget {
                total: total.clone(),
                calls: Mutex::new(0),
                pending: Mutex::new(0),
                directory: dir.clone(),
            })),
        );
        let request = input(case);
        save(&dir, "input.json", &request);
        let run = prepare_agent_chat(&mut state, &request.to_string(), "tb1")
            .unwrap_or_else(|r| panic!("{}", r.body));
        save(&dir, "learner-context.json", &run.tutor);
        let session = run.tutor.as_ref().unwrap()["binding"]["tutor_session_id"]
            .as_str()
            .unwrap()
            .to_string();
        let turn_ref = run.turn_ref.clone();
        let inner = std::mem::replace(
            &mut state.services.adapter,
            Box::new(ChatStubAdapter::scripted(vec![])),
        );
        let samples = Arc::new(Mutex::new(Vec::new()));
        let adapter = super::tutor_continuation_tests::TutorRequestRecorder {
            samples: samples.clone(),
            directory: Some(dir.clone()),
            inner: Box::new(WireRecorder {
                inner,
                directory: dir.clone(),
                calls: Default::default(),
            }),
        };
        let started = std::time::Instant::now();
        let result = agent_run::execute_prepared(
            &agent_run::BorrowedAppPort(RefCell::new(&mut state)),
            &adapter,
            run,
            Default::default(),
        );
        let elapsed = started.elapsed().as_millis();
        let saved = state
            .user
            .agent_history
            .sessions
            .iter()
            .find(|s| s.id == turn_ref.session_id)
            .unwrap()
            .turns
            .iter()
            .find(|t| t.turn_id == turn_ref.turn_id)
            .unwrap();
        let store = state.user.learning_store().unwrap();
        save(
            &dir,
            "result.json",
            &json!({"http_status":result.reply.status,"reply":serde_json::from_str::<Value>(&result.reply.body).unwrap_or(json!(result.reply.body)),
            "turn":saved,"elapsed_ms":elapsed,"provider_logical_calls":samples.lock().unwrap().len(),
            "facts":store.teaching_events(&session,0,100).unwrap(),
            "evidence":store.evidence_page(&state.workspace.book.base.book_id,0,100).unwrap(),
            "learner_projection":store.learner_projection(&state.workspace.book.base.book_id).unwrap(),
            "messages":state.workspace.messages.iter().map(|m| {let mut v=serde_json::to_value(m).unwrap();v.as_object_mut().unwrap().remove("provider_continuation");v}).collect::<Vec<_>>()}),
        );
        if let Some(answer) = saved.outcome.as_ref().and_then(|o| o.answer.as_ref()) {
            std::fs::write(dir.join("answer.md"), answer).unwrap();
        }
        println!(
            "TB1 {} status={} elapsed_ms={elapsed}",
            case["id"], result.reply.status
        );
    }
    let total = total.lock().unwrap();
    save(
        &directory,
        "budget.json",
        &json!({"http_calls":total.calls,"tokens_or_reserved":total.used,
        "stopped":total.stopped,"token_limit":TOKEN_LIMIT,"monetary_cost":null}),
    );
}
