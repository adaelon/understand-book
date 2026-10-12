//! TO3: execute published requests against real Tutor storage and dispatch.
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

fn discrimination(state: &mut AppState) {
    let run = prepare(state);
    teaching::step(&state.private_context(), &run.turn_ref, json!({"operation":"material","object_id":"speed"}), &[], &[]).unwrap();
    teaching::step(&state.private_context(), &run.turn_ref, json!({"operation":"select","move":{
        "move_id":"discriminate","object_ids":["speed"],"capability":"evaluation","kind":"source_focus",
        "interaction_intent":"neutral","intent_origin":"neutral","prompt":"Which quantities does this source support?","actions":["submit"]}}), &[], &[range()]).unwrap();
    finish(state, &run.turn_ref, &outcome(None));
    let id = format!("delivery:{}:discriminate", run.turn_ref.turn_id);
    json_post(state, "/tutor/display", json!({"delivery_ref":id}));
    json_post(state, "/tutor/action", json!({"operation_id":"to3-discrimination","delivery_ref":id,
        "action":"submit","response":"The source supports total distance; colour is not supported."}));
}

pub(crate) struct PublishedExample {
    pub(crate) example: Value,
    pub(crate) stage: AtomicUsize,
    pub(crate) observed: Arc<std::sync::Mutex<Vec<Value>>>,
}
impl ModelAdapter for PublishedExample {
    fn complete(&self, _: CompletionRequest) -> Result<ParsedResponse, AdapterError> { unreachable!() }
    fn chat(&self, plan: &AgentRequestPlan) -> Result<AssistantTurn, AdapterError> {
        match self.stage.fetch_add(1, Ordering::SeqCst) {
            0 => Ok(call("book.text", json!({"lid":"1.1","end_lid":"1.1"}))),
            1 => {
                let spec = plan.tools.iter().find(|t| t.name == "tutor.step").unwrap();
                let input = spec.parameters["examples"].as_array().unwrap().iter().find(|e| {
                    e["operation"] == self.example["operation"] && e["move"]["move_id"] == self.example["move"]["move_id"]
                        && e["nature"] == self.example["nature"]
                }).expect("example must survive the actual sampled state").clone();
                self.observed.lock().unwrap().push(input.clone());
                Ok(call("tutor.step", input))
            }
            2 if self.example["operation"] != "select" && self.example["operation"] != "outside"
                && plan.tools.iter().find(|t| t.name == "tutor.step").is_some_and(|t| t.parameters["properties"].get("move").is_some()) =>
                Ok(call("tutor.step", json!({"operation":"outside"}))),
            _ => Ok(answer("Average speed uses total distance divided by total time.")),
        }
    }
}

#[test]
fn tutor_to3_published_examples_execute_through_resident_dispatch() {
    let canonical = runtime::tutor::spec();
    let examples = canonical.parameters["examples"].as_array().unwrap();
    let mut executed = 0;
    for example in examples.iter().filter(|e| e["operation"] != "assess") {
        let (_root, mut state) = fixture();
        if example["operation"] == "evidence" { discrimination(&mut state); }
        let observed = Arc::new(std::sync::Mutex::new(Vec::new()));
        state.services.adapter = Box::new(PublishedExample { example:example.clone(), stage:AtomicUsize::new(0), observed:observed.clone() });
        let result = json_post(&mut state, "/agent/chat", json!({"message":"Help me examine average speed."}));
        let published = observed.lock().unwrap()[0].clone();
        if example["nature"] != "hypothesis" { assert_eq!(published, *example); }
        assert_ne!(result["incomplete"], true, "{example}: {result}");
        let trace = result["trace"].as_array().unwrap();
        let tool = trace.iter().find(|t| t["tool"] == "tutor.step" || t["name"] == "tutor.step");
        // The durable records, not the model's final text, establish acceptance.
        let store = state.user.learning_store().unwrap();
        let session = store.state().unwrap().control.current_tutor_session_id.unwrap();
        let turn = state.user.agent_history.sessions[0].turns.last().unwrap();
        let events = store.teaching_turn_events(&session, &turn.turn_id).unwrap();
        match example["operation"].as_str().unwrap() {
            "select" => {
                let saved = events.iter().find(|e| e.kind == TeachingFact::MoveSelected).expect("selected example persisted");
                assert_eq!(saved.payload["move"]["move_id"], example["move"]["move_id"]);
                assert_eq!(saved.payload["move"]["target"]["source_bindings"][0]["source_id"], state.workspace.book.base.book_id);
                assert!(!store.teaching_deliveries(&state.user.agent_history.sessions[0].id, &turn.turn_id).unwrap().is_empty());
            }
            "evidence" => { store.evidence(&format!("interpretation:{}", example["operation_id"].as_str().unwrap())).unwrap(); }
            "outside" => assert!(events.iter().any(|e| e.payload["outside"] == true)),
            _ => assert!(tool.is_some(), "read operation must appear in trace: {result}"),
        }
        executed += 1;
    }
    assert_eq!(executed, 9);
}

#[test]
fn tutor_to3_assess_example_uses_original_open_response() {
    let (_root, mut state) = fixture();
    let spec = runtime::tutor::spec();
    let examples = spec.parameters["examples"].as_array().unwrap();
    let scored = examples.iter().find(|e| e["move"]["assessment"].is_object()).unwrap();
    let run = prepare(&mut state);
    let mut invalid = scored.clone();
    invalid["move"]["actions"] = json!([]);
    assert!(teaching::step(&state.private_context(), &run.turn_ref, invalid, &[], &[range()]).unwrap_err().message.contains("评分活动必须允许提交"));
    teaching::step(&state.private_context(), &run.turn_ref, scored.clone(), &[], &[range()]).unwrap();
    finish(&mut state, &run.turn_ref, &outcome(None));
    let id = format!("delivery:{}:explain-denominator", run.turn_ref.turn_id);
    json_post(&mut state, "/tutor/display", json!({"delivery_ref":id}));
    json_post(&mut state, "/tutor/action", json!({"operation_id":"to3-open","delivery_ref":id,"action":"submit","response":"Use total time, and total time must be positive."}));
    let example = examples.iter().find(|e| e["operation"] == "assess").unwrap();
    state.services.adapter = Box::new(ChatStubAdapter::scripted(vec![
        call("tutor.step", example.clone()),
        answer(&json!({"items":[{"id":"total-time","verdict":"supported","response_quote":"Use total time",
            "sources":[{"lid":"1.1","quote":"total time"}],"reason":"The response names total time."}]}).to_string()),
        call("tutor.step", json!({"operation":"outside"})), answer("The response meets the frozen criterion."),
    ]));
    json_post(&mut state, "/agent/chat", json!({"message":"Evaluate my response","teaching_ref":"action:to3-open"}));
    assert_eq!(state.user.learning_store().unwrap().assessment("action:to3-open").unwrap().unwrap().status, memory::assessment::AssessmentStatus::Correct);
}

#[test]
fn tutor_to3_observing_examples_keep_teaching_disabled() {
    let spec = runtime::tutor::spec();
    for example in spec.parameters["examples"].as_array().unwrap().iter().filter(|e|
        e["operation"] == "trace" || e["operation"] == "understanding" || e["nature"] == "hypothesis") {
        let (_root, mut state) = fixture();
        discrimination(&mut state);
        let revision = state.user.learning_store().unwrap().state().unwrap().control.revision;
        json_post(&mut state, "/tutor/mutate", json!({"operation_id":"to3-off","expected_revision":revision,"action":{"kind":"set_enabled","enabled":false}}));
        let observed = Arc::new(std::sync::Mutex::new(Vec::new()));
        state.services.adapter = Box::new(PublishedExample { example:example.clone(), stage:AtomicUsize::new(0), observed:observed.clone() });
        let result = json_post(&mut state, "/agent/chat", json!({"message":"I am asking about the denominator."}));
        let published = observed.lock().unwrap()[0].clone();
        if example["nature"] != "hypothesis" { assert_eq!(published, *example); }
        assert_eq!(result["incomplete"], false);
        let store = state.user.learning_store().unwrap();
        assert!(!store.state().unwrap().control.enabled);
        if example["nature"] == "hypothesis" {
            let evidence = store.evidence("interpretation:to3-hypothesis").unwrap();
            assert_eq!(evidence.fact_refs, vec![published["fact_refs"][0].as_str().unwrap().to_string()]);
            assert_eq!(evidence.learner_quote, "I am asking about the denominator.");
        }
        let chat = &state.user.agent_history.sessions[0];
        assert!(store.teaching_deliveries(&chat.id, &chat.turns.last().unwrap().turn_id).unwrap().is_empty());
    }
}

#[test]
fn tutor_to3_examples_keep_source_fact_help_and_pause_preconditions() {
    let (_root, mut state) = fixture();
    discrimination(&mut state);
    let run = prepare(&mut state);
    let spec = runtime::tutor::spec();
    let examples = spec.parameters["examples"].as_array().unwrap();
    let select = examples.iter().find(|e| e["move"]["hint"].is_string()).unwrap();
    assert!(teaching::step(&state.private_context(), &run.turn_ref, select.clone(), &[], &[]).is_err());
    teaching::step(&state.private_context(), &run.turn_ref, select.clone(), &[], &[range()]).unwrap();
    finish(&mut state, &run.turn_ref, &outcome(None));
    let delivery = format!("delivery:{}:compare-times", run.turn_ref.turn_id);
    json_post(&mut state, "/tutor/display", json!({"delivery_ref":delivery}));
    for action in ["request_hint", "reveal"] {
        let result = json_post(&mut state, "/tutor/action", json!({"operation_id":format!("to3-{action}"),"delivery_ref":delivery,"action":action}));
        assert!(result.to_string().contains(if action == "request_hint" { "Time equals distance" } else { "equal distances take different times" }));
    }
    let hypothesis = examples.iter().find(|e| e["nature"] == "hypothesis").unwrap();
    let mut false_fact = hypothesis.clone();
    false_fact["fact_refs"] = json!([delivery]);
    assert!(teaching::step(&state.private_context(), &run.turn_ref, false_fact, &[], &[range()]).is_err());
    let revision = state.user.learning_store().unwrap().state().unwrap().control.revision;
    json_post(&mut state, "/tutor/mutate", json!({"operation_id":"to3-pause","expected_revision":revision,"action":{"kind":"set_enabled","enabled":false}}));
    assert!(teaching::step(&state.private_context(), &run.turn_ref, select.clone(), &[], &[range()]).unwrap_err().message.contains("教学已暂停"));
}
