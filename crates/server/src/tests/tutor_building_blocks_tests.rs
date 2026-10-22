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
        let mut movement = json!({"operation":"select","move":{
            "move_id":"fixture-probe","object_ids":[],"capability":"explanation",
            "kind":if direct {"explain"} else {"question"},
            "interaction_intent":if direct {"direct_explanation"} else {"guided_inquiry"},
            "intent_origin":if direct {"current_request"} else {"session_default"},
            "intent_quote":if direct {case["request"].clone()} else {Value::Null},
            "prompt":"fixture transport probe","actions":[],
            "target":{"learning_focus":"shared borrowing","expected_performance":"Distinguish ownership from borrowed access",
                "capability":"explanation","source_bindings":[{"start_lid":"1.1","end_lid":"1.1"}],"object_refs":[]}
        }});
        let mut design=super::teaching_design_tests::design_request("fixture-design",None);
        design["design"]["target"]=movement["move"]["target"].clone();
        design["design"]["organizing_model"]=json!("Borrowing provides access while ownership determines who controls the value.");
        design["design"]["blocks"][0]["relation"]=json!("Shared borrowing grants read access without transferring ownership.");
        design["design"]["blocks"][0]["purpose"]=json!("Distinguish the right to read from owning the value.");
        design["design"]["blocks"][0]["block_id"]=json!("borrowed-access");
        design["design"]["focus_block_refs"]=json!(["borrowed-access"]);
        movement["move"]["design_ref"]=json!(format!("design:{}:fixture-design",run.turn_ref.turn_id));
        movement["move"]["block_refs"]=json!(["borrowed-access"]);
        super::tutor_loop_tests::fixture_participation(&mut movement["move"]);
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
                    provider_continuation:None,text:None,usage_total_tokens:Some(1),
                    tool_calls:vec![ToolCall{id:"design".into(),name:"tutor.step".into(),arguments:design.to_string()}],
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

/// Fixed teaching choices exercise real prepare/design/select/review/storage.
/// The adapter records actual requests; its acceptance is not a semantic verdict.
pub(super) fn om3_replay(
    state: &mut AppState,
    run: agent_run::PreparedAgentChat,
    design: Value,
    mut selection: Value,
    name: &str,
) -> Value {
    let turn = run.turn_ref.clone();
    let frozen = run.tutor.as_ref().unwrap().clone();
    let design_ref = format!("design:{}:{}", turn.turn_id, design["operation_id"].as_str().unwrap());
    selection["move"]["design_ref"] = json!(design_ref);
    let body = selection["move"]["delivery_text"].as_str().unwrap().to_owned();
    let call = |id: &str, name: &str, arguments: Value| AssistantTurn {
        provider_continuation: None, text: None, usage_total_tokens: Some(1),
        tool_calls: vec![ToolCall { id: id.into(), name: name.into(), arguments: arguments.to_string() }],
    };
    let samples = Arc::new(Mutex::new(Vec::new()));
    let adapter = super::tutor_continuation_tests::TutorRequestRecorder {
        samples: samples.clone(), directory: None,
        inner: Box::new(ChatStubAdapter::scripted(vec![
            call("read", "book.text", json!({"lid":"1.1","end_lid":"1.1"})),
            call("design", "tutor.step", design),
            call("select", "tutor.step", selection),
            AssistantTurn { provider_continuation: None, text: Some(body.clone()), usage_total_tokens: Some(1), tool_calls: vec![] },
        ])),
    };
    let result = agent_run::execute_prepared(
        &agent_run::BorrowedAppPort(RefCell::new(state)), &adapter, run, Default::default());
    assert_eq!(result.reply.status, 200, "{}", result.reply.body);
    let outcome: Value = serde_json::from_str(&result.reply.body).unwrap();
    assert_eq!(outcome["incomplete"], false, "{outcome}");
    let captured = samples.lock().unwrap();
    let reviews: Vec<_> = captured.iter().filter(|sample| sample["kind"] == "structured_completion").collect();
    assert_eq!(reviews.len(), 1, "the scripted selection must reach the real private review once");
    let review: Value = serde_json::from_str(reviews[0]["input"][0]["content"].as_str().unwrap()).unwrap();
    assert_eq!(review["frozen_input"]["current_request"], frozen["current_request"]);
    assert_eq!(review["frozen_input"]["confirmed_preferences"], frozen["confirmed_preferences"]);
    assert!(review["learner_view"]["learning_opportunity"]["continuity"].is_object(),
        "review must receive the same continuity evidence used to choose support");
    assert!(captured[0]["input"].to_string().contains(frozen["current_request"].as_str().unwrap()),
        "the actual first main-model request must carry the current words");
    assert_eq!(review["proposed_delivery"]["text"], body);
    assert_eq!(review["current_design"]["design_ref"], design_ref);
    let store = state.user.learning_store().unwrap();
    let deliveries = store.teaching_deliveries(&turn.session_id, &turn.turn_id).unwrap();
    assert_eq!(deliveries.len(), 1);
    assert_eq!(deliveries[0].payload["answer"], body);
    let selected = store.teaching_event(&deliveries[0].causal_refs[0]).unwrap();
    assert_eq!(selected.payload["move"]["delivery_text"], body);
    assert!(store.teaching_turn_events(&deliveries[0].binding.tutor_session_id, &turn.turn_id).unwrap()
        .iter().all(|event| event.kind != memory::teaching::TeachingFact::LearnerAction),
        "teacher support does not create a learner submission");
    let report = json!({
        "prepare": frozen, "main_input": captured[0]["input"],
        "main_instructions": captured[0]["instructions"],
        "review_input": review, "review_instructions": reviews[0]["instructions"],
        "delivery": deliveries[0], "selection": selected, "accepted_design": store.teaching_event(&design_ref).unwrap(),
        "outcome": outcome,
    });
    if let Ok(directory) = std::env::var("OM3_OUTPUT") {
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(Path::new(&directory).join(format!("{name}.json")),
            serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    }
    report
}

#[test]
fn om3_operational_barrier_delivers_a_method_without_a_new_test() {
    let (_root, mut state) = super::teaching_design_tests::fixture();
    let request = "我不会填行程表，简单些，先示范在哪里写距离、时间和停车，不要先考我。";
    let run = prepare_agent_chat(&mut state, &json!({"message":request}).to_string(), "om3").unwrap();
    let fact = format!("usage:{}:user:{}", run.tutor.as_ref().unwrap()["binding"]["tutor_session_id"].as_str().unwrap(), run.turn_ref.turn_id);
    let mut design = super::teaching_design_tests::design_request("om3-method", None);
    design["design"]["basis_refs"] = json!([fact]);
    design["design"]["revision_reason"] = json!("当前请求要先示范填表；给出可以开始的操作，不从不会操作推断概念错误。");
    let mut selected = runtime::tutor::spec().parameters["examples"].as_array().unwrap().iter()
        .find(|example| example["move"]["move_id"] == "start-journey-table").unwrap().clone();
    selected["move"]["interaction_intent"] = json!("direct_explanation");
    selected["move"]["intent_origin"] = json!("current_request");
    selected["move"]["intent_quote"] = json!("简单些，先示范在哪里写距离、时间和停车，不要先考我");
    selected["move"]["responds_to"] = json!({"fact_refs":[fact],"disposition":"addressed"});
    let report = om3_replay(&mut state, run, design, selected, "operational-barrier");
    let movement = &report["selection"]["payload"]["move"];
    assert_eq!(movement["participation"]["choice"], "support");
    assert!(movement["response_expectation"].is_null());
    assert!(movement["assessment"].is_null());
    assert_eq!(report["review_input"]["current_design"]["design"]["basis_refs"], json!([fact]));
    assert!(report["review_input"]["learner_grounding"]["interpretations"].as_array().unwrap().is_empty());
    assert!(state.user.learning_store().unwrap().learner_projection(&state.workspace.book.base.book_id).unwrap()
        .0.unwrap().rows.is_empty(), "a worked method supplies no independent-performance evidence");
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
        source_references: RefCell<Vec<(String,String)>>,
        stage: std::cell::Cell<usize>,
    }
    impl RecordedTeaching {
        fn rebound(&self, plan: &AgentRequestPlan) -> Value {
            let mut serialized = self.output.to_string();
            if self.source_references.borrow().is_empty() {
                for (index, source) in self.sources.iter().enumerate() {
                    let call_id = format!("source-{index}");
                    let result = plan.input.iter().rev().find(|m| m.tool_call_id.as_deref() == Some(&call_id)).unwrap();
                    let body: Value = serde_json::from_str(result.content.as_deref().unwrap()).unwrap();
                    let actual = body["model_body"]["source_ref_id"].as_str().unwrap_or_else(||panic!("current source was not accepted: {body}"));
                    self.source_references.borrow_mut().push((source["source_ref_id"].as_str().unwrap().into(),actual.into()));
                }
            }
            for (old,new) in self.source_references.borrow().iter() { serialized=serialized.replace(old,new); }
            serde_json::from_str(&serialized).unwrap()
        }
    }
    impl ModelAdapter for RecordedTeaching {
        fn complete(&self, _: CompletionRequest) -> Result<ParsedResponse, AdapterError> { unreachable!("no model helpers in replay") }
        fn complete_structured(&self, request: CompletionRequest) -> Result<Value, AdapterError> {
            if let Some(guidance)=super::tutor_loop_tests::fixture_coach_guidance(&request) { return Ok(guidance); }
            if let Some(review)=super::tutor_loop_tests::fixture_delivery_review(&request) { return Ok(review); }
            unreachable!("no other model helpers in replay")
        }
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
                2 => {
                    let output=self.rebound(plan);
                    let target=&output["select"]["move"]["target"];
                    (None,vec![call("design","tutor.step",json!({"operation":"design","operation_id":"frozen-borrowing","design":{
                        "target":target,"organizing_model":"Borrowing provides access to a value while its owner keeps ownership.",
                        "representation":{"kind":"text","reason":"The frozen explanation compares the expressions and their types."},
                        "blocks":[{"block_id":"borrowed-access","relation":"Creating a shared reference does not transfer the referenced value.",
                            "purpose":target["expected_performance"],"source_bindings":target["source_bindings"]}],
                        "connections":[],"focus_block_refs":["borrowed-access"],"basis_refs":[],"open_question_refs":[],
                        "previous_ref":null,"revision_reason":"Bind the frozen explanation to its source relationship."}}))])
                },
                3 => {
                    let output=self.rebound(plan);
                    let mut request=output["select"].clone();
                    request["move"]["design_ref"]=super::tutor_continuation_tests::accepted_design(plan,"design")["design_ref"].clone();
                    request["move"]["block_refs"]=json!(["borrowed-access"]);
                    request["move"]["delivery_text"]=output["answer"].clone();
                    super::tutor_loop_tests::fixture_participation(&mut request["move"]);
                    (None,vec![call("select","tutor.step",request)])
                },
                4 => (Some(self.rebound(plan)["answer"].as_str().unwrap().to_string()), vec![]),
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
            let adapter = RecordedTeaching { output, sources: packet["presented_sources"].as_array().unwrap().clone(), source_references:Default::default(), stage: Default::default() };
            let (root, mut state) = fixture();
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
            save(root.path(), "replay.json", &json!({"http_status":result.reply.status,"outcome":outcome,"facts":facts,
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
            images: Vec::new(),
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
                images: Vec::new(),
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
