//! Bounded response-fit replay: model text is frozen; tools and storage are real.
use super::*;
use memory::learning_evidence::LearningEvidence;
use memory::teaching::{TeachingEvent, TeachingFact};

fn directory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp/tutor-response-fit-20261013")
}

fn read(path: impl AsRef<Path>) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn save(path: impl AsRef<Path>, value: &Value) {
    std::fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

fn rebound(value: &Value, binding: &Value) -> Value {
    let mut text = value.to_string();
    for (token, key) in [
        ("$SOURCE_ID", "source_id"),
        ("$SOURCE_REVISION", "source_revision"),
        ("$TUTOR_SESSION", "tutor_session_id"),
        ("$CHAT_SESSION", "chat_session_id"),
        ("$CURRENT_TURN", "turn_id"),
    ] {
        text = text.replace(token, binding[key].as_str().unwrap());
    }
    serde_json::from_str(&text).unwrap()
}

fn text(value: &Value) -> Value {
    value.get("text").unwrap_or(value).clone()
}

fn seed(case: &Value) -> (tempfile::TempDir, AppState, agent_run::PreparedAgentChat) {
    let source = case["original_text"].as_str().unwrap();
    let mut base = sample_base();
    base.book_id = "response-fit-fixture".into();
    base.graph_nodes.clear();
    base.graph_edges.clear();
    for node in &mut base.lid_nodes {
        node.span = Span {
            start: 0,
            end: source.encode_utf16().count(),
        };
    }
    let (root, mut state) =
        super::tutor_loop_tests::fixture_with_book(false, Book::new(base, source), source);
    let control = state
        .user
        .learning_store()
        .unwrap()
        .state()
        .unwrap()
        .control;
    let reply = post(&mut state, "/tutor/mutate", &json!({
        "operation_id":"response-fit-mode", "expected_revision":control.revision,
        "action":{"kind":"set_mode", "session_id":control.current_tutor_session_id,"mode":"guided_inquiry"}
    }).to_string());
    assert_eq!(reply.status, 200, "{}", reply.body);
    let run = prepare_agent_chat(
        &mut state,
        &json!({"message":case["user_request"]}).to_string(),
        "response-fit",
    )
    .unwrap_or_else(|r| panic!("{}", r.body));
    let binding = &run.tutor.as_ref().unwrap()["binding"];
    let mut store = state.user.learning_store().unwrap();
    for imported in case["events"].as_array().unwrap() {
        let mut event = rebound(imported, binding);
        // Local envelopes retain captured content/help conditions. Production
        // session and control identities are not imported into the test store.
        event["binding"] = binding.clone();
        event["causal_refs"] = json!([]);
        if event["payload"]["move"]["target"].is_object() {
            event["payload"]["move"]["capability"] =
                event["payload"]["move"]["target"]["capability"].clone();
        }
        let event: TeachingEvent = serde_json::from_value(event).unwrap();
        store.append_teaching(&event).unwrap();
    }
    for imported in case["context"]["learner_context"]["interpretations"]
        .as_array()
        .unwrap()
        .iter()
        .rev()
    {
        let prior = rebound(imported, binding);
        let evidence: LearningEvidence = serde_json::from_value(json!({
            "evidence_id":prior["evidence_ref"], "action_ref":prior["fact_refs"][0],
            "delivery_ref":null, "session_id":binding["tutor_session_id"],
            "source_id":binding["source_id"], "source_revision":binding["source_revision"],
            "map_revision":null, "object_id":null, "object_revision":null,
            "nature":"hypothesis", "target":prior["target"], "fact_refs":prior["fact_refs"],
            "teaching_implication":text(&prior["teaching_implication"]),
            "label":prior["target"]["learning_focus"], "capability":prior["target"]["capability"],
            "assistance_refs":prior["assistance_refs"], "attempt":prior["attempt"],
            "assessment_ref":null, "status":"unassessed", "interpretation":text(&prior["interpretation"]),
            "learner_quote":text(&prior["learner_quote"]), "interpreter_version":"captured-fixture",
            "supersedes":null, "correction":prior["correction"], "source_quotes":[]
        })).unwrap();
        store.append_evidence(&evidence).unwrap();
    }
    store
        .rebuild_learner_projection(&state.workspace.book.base.book_id)
        .unwrap();
    drop(store);
    (root, state, run)
}

struct Replay {
    output: Value,
    sources: Vec<Value>,
    stage: std::cell::Cell<usize>,
}

impl Replay {
    fn output(&self, plan: &AgentRequestPlan) -> Value {
        let mut text = self.output.to_string();
        for (index, source) in self.sources.iter().enumerate() {
            let id = format!("source-{index}");
            let result = plan
                .input
                .iter()
                .find(|m| m.tool_call_id.as_deref() == Some(&id))
                .unwrap();
            let body: Value = serde_json::from_str(result.content.as_deref().unwrap()).unwrap();
            let actual = body["model_body"]["source_ref_id"].as_str().unwrap();
            let old = source["source_ref_id"].as_str().unwrap();
            text = text.replace(
                &format!("[[source:{old}]]"),
                &format!("[[source:{actual}]]"),
            );
        }
        serde_json::from_str(&text).unwrap()
    }
}

impl ModelAdapter for Replay {
    fn complete(&self, _: CompletionRequest) -> Result<ParsedResponse, AdapterError> {
        unreachable!("frozen local replay never calls a helper model")
    }

    fn chat(&self, plan: &AgentRequestPlan) -> Result<AssistantTurn, AdapterError> {
        let stage = self.stage.get();
        self.stage.set(stage + 1);
        let call = |id: &str, name: &str, arguments: Value| ToolCall {
            id: id.into(),
            name: name.into(),
            arguments: arguments.to_string(),
        };
        let has_evidence = self.output["evidence"].is_object();
        let (text, tool_calls) = match stage {
            0 => (
                None,
                vec![call(
                    "read",
                    "book.text",
                    json!({"lid":"1.1","end_lid":"1.1"}),
                )],
            ),
            1 => (
                None,
                self.sources
                    .iter()
                    .enumerate()
                    .map(|(i, source)| {
                        call(
                            &format!("source-{i}"),
                            "source.present",
                            json!({"quote":source["quote"]}),
                        )
                    })
                    .collect(),
            ),
            2 if has_evidence => (
                None,
                vec![call(
                    "evidence",
                    "tutor.step",
                    self.output(plan)["evidence"].clone(),
                )],
            ),
            n if n == 2 + usize::from(has_evidence) => (
                None,
                vec![call(
                    "select",
                    "tutor.step",
                    self.output(plan)["select"].clone(),
                )],
            ),
            n if n == 3 + usize::from(has_evidence) => (
                Some(self.output(plan)["answer"].as_str().unwrap().into()),
                vec![],
            ),
            _ => {
                return Err(AdapterError {
                    message: format!("Frozen response required a repair: {:?}", plan.input.last()),
                    spend_stop: None,
                })
            }
        };
        Ok(AssistantTurn {
            provider_continuation: None,
            text,
            tool_calls,
            usage_total_tokens: None,
        })
    }
}

fn replay(case: &Value, output: &Value) -> Value {
    let (_root, mut state, run) = seed(case);
    let turn = run.turn_ref.clone();
    let binding = run.tutor.as_ref().unwrap()["binding"].clone();
    let output = rebound(output, &binding);
    let evidence_id = output["evidence"]["operation_id"]
        .as_str()
        .map(|id| format!("interpretation:{id}"));
    let supersedes = output["evidence"]["supersedes"].as_str().map(str::to_owned);
    let expected_answer = output["answer"].as_str().unwrap().to_string();
    let samples = Arc::new(std::sync::Mutex::new(Vec::new()));
    let adapter = super::tutor_continuation_tests::TutorRequestRecorder {
        samples: samples.clone(),
        directory: None,
        inner: Box::new(Replay {
            output,
            sources: case["presented_sources"].as_array().unwrap().clone(),
            stage: Default::default(),
        }),
    };
    let result = agent_run::execute_prepared(
        &agent_run::BorrowedAppPort(RefCell::new(&mut state)),
        &adapter,
        run,
        Default::default(),
    );
    let saved = state
        .user
        .agent_history
        .sessions
        .iter()
        .find(|s| s.id == turn.session_id)
        .unwrap()
        .turns
        .last()
        .unwrap();
    let store = state.user.learning_store().unwrap();
    let facts = store
        .teaching_turn_events(binding["tutor_session_id"].as_str().unwrap(), &turn.turn_id)
        .unwrap();
    let deliveries: Vec<_> = facts
        .iter()
        .filter(|f| f.kind == TeachingFact::MessageDelivered && f.event_id.starts_with("delivery:"))
        .collect();
    let evidence = evidence_id.and_then(|id| store.evidence(&id).ok());
    if let Some(evidence) = &evidence {
        assert_eq!(
            evidence.status,
            memory::assessment::AssessmentStatus::Unassessed
        );
        assert_eq!(
            evidence.nature,
            memory::learning_evidence::EvidenceNature::Hypothesis
        );
        assert!(evidence.assessment_ref.is_none());
        for fact_id in &evidence.fact_refs {
            let fact = store.teaching_event(fact_id).unwrap();
            for help in fact.payload["assistance_refs"]
                .as_array()
                .into_iter()
                .flatten()
            {
                assert!(evidence
                    .assistance_refs
                    .contains(&help.as_str().unwrap().to_string()));
            }
        }
    }
    let projection = store
        .learner_projection(&state.workspace.book.base.book_id)
        .unwrap()
        .0
        .unwrap();
    assert!(
        projection.rows.is_empty(),
        "a hypothesis must not become measured mastery"
    );
    json!({"http_status":result.reply.status,"reply":result.reply.body,"outcome":saved.outcome,
        "delivery_count":deliveries.len(),"deliveries":deliveries,"evidence":evidence,
        "superseded":supersedes.map(|id|store.evidence_is_superseded(&id).unwrap()),
        "frozen_answer":expected_answer,"requests":*samples.lock().unwrap(),"external_provider_calls":0})
}

#[test]
fn tutor_response_fit_revises_hypothesis_without_erasing_help_or_claiming_mastery() {
    let target = json!({"learning_focus":"work allocation","expected_performance":"Explain which work moves and which work remains.",
        "capability":"explanation","object_refs":[],"source_bindings":[{"source_id":"$SOURCE_ID","source_revision":"$SOURCE_REVISION","start_lid":"1.1","end_lid":"1.1"}]});
    let prior = json!({"evidence_ref":"interpretation:old", "target":target,"fact_refs":["old-user"],
        "interpretation":{"text":"Application work may be confused with CPU work."},
        "teaching_implication":{"text":"Separate the two workloads."},"learner_quote":{"text":"Does application work shrink?"},
        "assistance_refs":[],"attempt":0,"correction":null});
    let event = |id: &str, kind: &str, payload: Value| json!({"event_id":id,"kind":kind,"payload":payload,"causal_refs":[],"occurred_at":"fixture","binding":{}});
    let answer = "The application work remains. Packet processing moves to the NIC. [[source:frozen-source-0]]";
    let case = json!({"original_text":"Packet processing moves to the NIC; application work remains.",
        "user_request":"With the example, I see packet processing moves but application work remains.",
        "context":{"learner_context":{"interpretations":[prior]}},
        "events":[event("old-user","usage_observed",json!({"usage":{"kind":"user_message"},"response":"Does application work shrink?"})),
            event("seen-help","help_displayed",json!({"text":"Separate packet processing from application work."})),
            event("current-user","learner_action",json!({"action":"chat_response","response":"With the example, I see packet processing moves but application work remains.","attempt":2,"assistance_refs":["seen-help"]}))],
        "presented_sources":[{"source_ref_id":"frozen-source-0","quote":"Packet processing moves to the NIC; application work remains."}]});
    let output = json!({"evidence":{"operation":"evidence","nature":"hypothesis","operation_id":"revised","target":target,
        "fact_refs":["current-user"],"supersedes":"interpretation:old","interpretation":"The supplied example supported the distinction; independent transfer is unknown.",
        "teaching_implication":"Retain this distinction and examine another allocation."},
        "select":{"operation":"select","move":{"move_id":"fit","object_ids":[],"capability":"explanation","kind":"explain",
            "interaction_intent":"guided_inquiry","intent_origin":"session_default","prompt":answer,"actions":[],"target":target,"interpretation_refs":["interpretation:revised"]}},"answer":answer});
    let result = replay(&case, &output);
    assert_eq!(result["http_status"], 200, "{result}");
    assert_eq!(result["outcome"]["incomplete"], false, "{result}");
    assert_eq!(result["delivery_count"], 1);
    assert_eq!(result["superseded"], true);
    assert_eq!(result["evidence"]["learner_quote"], case["user_request"]);
    assert_eq!(result["evidence"]["assistance_refs"], json!(["seen-help"]));
    assert!(!result["outcome"]["answer_view"]["sources"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[test]
#[ignore = "requires private frozen packets/model outputs; no provider calls"]
fn tutor_response_fit_replay_frozen_outputs() {
    let fixture = read(directory().join("fixtures.json"));
    let mut failures = Vec::new();
    for variant in ["before", "after"] {
        for case in fixture["cases"].as_array().unwrap() {
            let dir = directory().join(variant).join(case["id"].as_str().unwrap());
            let result = replay(case, &read(dir.join("output.json")));
            save(dir.join("replay.json"), &result);
            if variant == "after" {
                if result["http_status"] != 200
                    || result["outcome"]["incomplete"] != false
                    || result["delivery_count"] != 1
                {
                    failures.push(format!("{variant}/{}: see {}", case["id"], dir.join("replay.json").display()));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
