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
    previous_design_ref: Option<String>,
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

    fn complete_structured(&self, request: CompletionRequest) -> Result<Value, AdapterError> {
        if let Some(guidance)=super::tutor_loop_tests::fixture_coach_guidance(&request) { return Ok(guidance); }
        if let Some(review)=super::tutor_loop_tests::fixture_delivery_review(&request) { return Ok(review); }
        unreachable!("frozen local replay never calls another helper model")
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
                vec![call("design","tutor.step",{
                    let output=self.output(plan);
                    let target=&output["select"]["move"]["target"];
                    json!({"operation":"design","operation_id":"work-allocation","design":{
                        "target":target,"organizing_model":"Track application work separately from packet processing before deciding which work moves to another component.",
                        "representation":{"kind":"text","reason":"This local work-allocation distinction can be established in the existing example."},
                        "blocks":[{"block_id":"work-allocation","relation":"Moving packet processing to the NIC does not remove application work.",
                            "purpose":target["expected_performance"],"source_bindings":target["source_bindings"]}],
                        "connections":[],"focus_block_refs":["work-allocation"],"basis_refs":output["select"]["move"].get("interpretation_refs").cloned().unwrap_or(json!([])),
                        "open_question_refs":[],"previous_ref":self.previous_design_ref,"revision_reason":"Preserve the learner's assisted distinction and examine what remains."}})
                })],
            ),
            n if n == 3 + usize::from(has_evidence) => (
                None,
                vec![call(
                    "select",
                    "tutor.step",
                    {
                        let mut request=self.output(plan)["select"].clone();
                        request["move"]["design_ref"]=super::tutor_continuation_tests::accepted_design(plan,"design")["design_ref"].clone();
                        request["move"]["block_refs"]=json!(["work-allocation"]);
                        super::tutor_loop_tests::fixture_participation(&mut request["move"]);
                        request
                    },
                )],
            ),
            n if n == 4 + usize::from(has_evidence) => (
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
            previous_design_ref:run.tutor.as_ref().unwrap()["current_design"]["design_ref"].as_str().map(str::to_owned),
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
fn om3_open_plan_analysis_keeps_multiple_options_without_a_scoring_contract() {
    let (_root, mut state) = super::teaching_design_tests::fixture();
    let request = "帮我分析同一路线怎样缩短总时间；可以少停，也可以在安全条件下快一点，必要的休息要保留。先给比较框架。";
    let run = prepare_agent_chat(&mut state, &json!({"message":request}).to_string(), "om3").unwrap();
    let fact = format!("usage:{}:user:{}", run.tutor.as_ref().unwrap()["binding"]["tutor_session_id"].as_str().unwrap(), run.turn_ref.turn_id);
    let mut design = super::teaching_design_tests::design_request("om3-open-plan", None);
    design["design"]["basis_refs"] = json!([fact]);
    design["design"]["organizing_model"] = json!("相同路程下比较行驶时间与停留时间，保留用户提出的必要休息与安全条件。");
    design["design"]["revision_reason"] = json!("当前开放任务需要比较理由和可兼用的方案；先给框架，再由任务需要决定是否提问。");
    let mut selection = runtime::tutor::spec().parameters["examples"].as_array().unwrap().iter()
        .find(|example| example["move"]["move_id"] == "compare-journey-options").unwrap().clone();
    selection["move"]["responds_to"] = json!({"fact_refs":[fact],"disposition":"addressed"});
    let report = super::tutor_building_blocks_tests::om3_replay(&mut state, run, design, selection, "open-plan");
    let movement = &report["selection"]["payload"]["move"];
    assert_eq!(movement["participation"]["choice"], "support");
    assert!(movement["assessment"].is_null());
    assert!(movement["response_expectation"].is_null());
    assert_eq!(report["review_input"]["learner_grounding"]["expressions"][0]["text"], request);
    assert_eq!(report["review_audit"]["payload"]["current_design"]["design"]["basis_refs"], json!([fact]));
    assert!(report["review_input"]["learner_grounding"]["expressions"].as_array().unwrap().iter()
        .any(|expression| expression["fact_ref"] == fact && expression["text"] == request),
        "the compact review must retain the real expression behind the design basis");
    assert!(report["delivery"]["payload"]["answer"].as_str().unwrap().contains("Both changes may be combined"));
}

#[test]
fn om3_current_depth_request_overrides_history_without_rewriting_preference() {
    use memory::{Applicability, CreateProfileFact, FactSource, PreferenceClaim, ProfileScope, Sensitivity};
    let (_root, mut state) = super::teaching_design_tests::fixture();
    let prior = prepare_agent_chat(&mut state, r#"{"message":"平时请用简单例子，少推导公式。"}"#, "2026-10-20T08:00:00Z").unwrap();
    let preference = state.user.store.create_profile_fact(CreateProfileFact {
        scope: ProfileScope::Book { book_id: state.workspace.book.base.book_id.clone() },
        applicability: Applicability::Any,
        payload: ProfilePayload::ExplanationPreference(PreferenceClaim { key:"depth".into(), value:"平时请用简单例子，少推导公式。".into() }),
        source: FactSource::UserStated,
        evidence: vec![memory::EvidenceRef::Turn { session_id:prior.turn_ref.session_id.clone(), turn_id:prior.turn_ref.turn_id.clone() }],
        confidence: None, sensitivity: Sensitivity::Normal, valid_until: None,
    }, "2026-10-20T08:00:00Z").unwrap();
    super::tutor_loop_tests::finish(&mut state, &prior.turn_ref, &super::tutor_loop_tests::outcome(None));
    let request = "这次我想深入推导，请直接解释 v = d/T 的来由和 T > 0 的条件，不用先做题。";
    let run = prepare_agent_chat(&mut state, &json!({"message":request}).to_string(), "2026-10-20T08:01:00Z").unwrap();
    assert_eq!(run.tutor.as_ref().unwrap()["confirmed_preferences"][0]["fact_id"], preference.fact_id);
    let preferences = run.tutor.as_ref().unwrap()["confirmed_preferences"].clone();
    let fact = format!("usage:{}:user:{}", run.tutor.as_ref().unwrap()["binding"]["tutor_session_id"].as_str().unwrap(), run.turn_ref.turn_id);
    let mut design = super::teaching_design_tests::design_request("om3-deeper-now", None);
    design["design"]["basis_refs"] = json!([fact]);
    design["design"]["revision_reason"] = json!("当前明确要求深入推导，优先于历史少推导偏好；本次改变讲解深度，保留原偏好。");
    let mut selection = super::teaching_design_tests::select_request("om3-derivation", "", false);
    selection["move"]["kind"] = json!("explain");
    selection["move"]["interaction_intent"] = json!("direct_explanation");
    selection["move"]["intent_origin"] = json!("current_request");
    selection["move"]["intent_quote"] = json!(request);
    selection["move"]["responds_to"] = json!({"fact_refs":[fact],"disposition":"addressed"});
    selection["move"]["participation"] = json!({"choice":"conclude","reason":"当前要求直接推导；给出完整关系与条件后收束。"});
    selection["move"]["delivery_text"] = json!("把平均速度 v 定义为在同一总时间 T 内走完同一总路程 d 的恒定速度。恒定速度下路程是 vT，因此 vT = d。当 T > 0 时，两边除以 T 得 v = d/T。T 为零时不能作这个除法；总时间包括出发至到达之间的停留。");
    let report = super::tutor_building_blocks_tests::om3_replay(&mut state, run, design, selection, "current-depth");
    assert_eq!(report["review_input"]["frozen_input"]["confirmed_preferences"], preferences);
    assert_eq!(report["delivery"]["payload"]["move"]["intent_origin"], "current_request");
    assert_eq!(report["delivery"]["payload"]["move"]["intent_quote"], request);
    assert!(report["delivery"]["payload"]["move"]["response_expectation"].is_null());
    let next = prepare_agent_chat(&mut state, r#"{"message":"继续。"}"#, "2026-10-20T08:02:00Z").unwrap();
    assert_eq!(next.tutor.unwrap()["confirmed_preferences"], preferences,
        "a one-turn depth request must not mutate or add a long-term preference");
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
