//! WB2 acceptance: real network runs and private files, with only the model controlled.
use super::mu5_tests::Fixture;
use super::*;
use memory::{learning_evidence::LearningEvidence, teaching::TeachingEvent};
use runtime::presentation::{PresentationOwner, PresentationRef};
use std::cell::Cell;
use std::sync::Condvar;

const BACKEND: &str = "127.0.0.1:4220";
const CONTROL: &str = "127.0.0.1:4221";
const ATTACHMENT: &str = "wb2-seed";
const NOW: &str = "2026-10-25T09:00:00Z";
const SOURCE_QUOTE: &str = "平均速度是总路程除以总时间。";
const LEARNER_QUOTE: &str = "我明白了，慢的那段花的时间更多，所以不能把两个速度直接相加再除以二。";
const ANSWER_MARKER: &str = "慢段权重的完整解释";

#[derive(Default)]
struct ModelGate {
    blocked: bool,
    waiting: bool,
    completed: usize,
}

#[derive(Clone, Default)]
struct ModelControl {
    gate: Arc<(Mutex<ModelGate>, Condvar)>,
    requests: Arc<Mutex<Vec<Value>>>,
}

impl ModelControl {
    fn view(&self) -> Value {
        let gate = self.gate.0.lock().unwrap();
        json!({"blocked":gate.blocked,"waiting":gate.waiting,"completed":gate.completed,
            "requests":self.requests.lock().unwrap().clone()})
    }

    fn release(&self) {
        self.gate.0.lock().unwrap().blocked = false;
        self.gate.1.notify_all();
    }
}

struct BoardProbe {
    stage: Cell<usize>,
    control: ModelControl,
}

impl ModelAdapter for BoardProbe {
    fn complete(&self, _: CompletionRequest) -> Result<ParsedResponse, AdapterError> {
        Err(AdapterError {
            message: "WB2 does not synthesize unrequested text".into(),
            spend_stop: None,
        })
    }

    fn complete_structured(&self, request: CompletionRequest) -> Result<Value, AdapterError> {
        if let Some(review) = super::tutor_loop_tests::fixture_delivery_review(&request) {
            return Ok(review);
        }
        if request
            .system
            .starts_with("你是 Understand Book 的教学教练。")
        {
            let input: Value = serde_json::from_str(&request.user).unwrap();
            if input["trigger"] == "after_turn"
                && input["teaching_context"]["status"] == "observing"
            {
                let observations: Vec<_> = input["observation_facts"].as_array().into_iter().flatten()
                    .filter(|fact| fact["already_interpreted"] != true)
                    .map(|fact| json!({"fact_ref":fact["fact_ref"],"decision":"no_new_information",
                        "reason":"The browser fixture preserves the seeded learner expression without inferring new mastery."}))
                    .collect();
                return Ok(
                    json!({"next_step":{"focus":"Keep the completed explanation",
                    "participation":{"choice":"conclude","reason":"The requested explanation is delivered."}},
                    "observations":observations,"reason":"No additional understanding judgment is seeded by this model."}),
                );
            }
        }
        if let Some(guidance) = super::tutor_loop_tests::fixture_coach_guidance(&request) {
            return Ok(guidance);
        }
        Err(AdapterError {
            message: "WB2 has no background profile completion".into(),
            spend_stop: None,
        })
    }

    fn chat(&self, request: &AgentRequestPlan) -> Result<AssistantTurn, AdapterError> {
        let stage = self.stage.get();
        self.stage.set(stage + 1);
        let messages = request.ordered_messages();
        self.control
            .requests
            .lock()
            .unwrap()
            .push(json!({"stage":stage,"messages":messages}));
        let tool = |id: &str, name: &str, arguments: Value| AssistantTurn {
            provider_continuation: None,
            text: None,
            tool_calls: vec![runtime::ToolCall {
                id: id.into(),
                name: name.into(),
                arguments: arguments.to_string(),
            }],
            usage_total_tokens: Some(1),
        };
        match stage {
            0 => return Ok(tool("wb2-read", "book.text", json!({"lid":"1.1"}))),
            1 => {
                return Ok(tool(
                    "wb2-source",
                    "source.present",
                    json!({"quote":SOURCE_QUOTE}),
                ))
            }
            2 => {}
            _ => {
                return Err(AdapterError {
                    message: format!(
                        "WB2 scripted answer required unexpected repair: {}",
                        json!(messages.last())
                    ),
                    spend_stop: None,
                })
            }
        }
        let source = messages
            .iter()
            .rev()
            .find(|m| m.tool_call_id.as_deref() == Some("wb2-source"))
            .and_then(|m| m.content.as_deref())
            .and_then(|s| serde_json::from_str::<Value>(s).ok())
            .map(|value| value.get("model_body").cloned().unwrap_or(value))
            .unwrap_or(Value::Null);
        let source_ref = source["source_ref_id"]
            .as_str()
            .ok_or_else(|| AdapterError {
                message: format!("WB2 requires a real source.present result: {source}"),
                spend_stop: None,
            })?;
        {
            let (lock, changed) = &*self.control.gate;
            let mut gate = lock.lock().unwrap();
            gate.waiting = gate.blocked;
            changed.notify_all();
            let (mut gate, timeout) = changed
                .wait_timeout_while(gate, Duration::from_secs(120), |g| g.blocked)
                .unwrap();
            gate.waiting = false;
            if timeout.timed_out() {
                return Err(AdapterError {
                    message: "WB2 browser did not release the pending answer".into(),
                    spend_stop: None,
                });
            }
            gate.completed += 1;
        }
        Ok(AssistantTurn {
            provider_continuation: None,
            text: Some(format!("### {ANSWER_MARKER}\n\n这次解释针对发问时选中的知识对象。平均速度要用总路程除以总时间，不能因为两段路程相等就直接平均速度。\n\n去程 60 米、速度 3 m/s，需要 20 秒；回程 60 米、速度 1 m/s，需要 60 秒。总路程 120 米除以总时间 80 秒，得到 **1.5 m/s**。慢段占了四分之三的时间，因此它的权重更大。\n\n只有各段用时相同，速度的算术平均才与全程平均速度一致。记住先累计路程与用时，再做一次除法，就能区分等路程与等时间两种条件。 [[source:{source_ref}]]")),
            tool_calls: vec![], usage_total_tokens: Some(1),
        })
    }
}

fn seed_run(f: &Fixture, scene: &Value) -> crate::workspace_registry::NetworkRunPort {
    let principal = f
        .access
        .auth
        .authenticate(&f.a, crate::multi_user_host::now())
        .unwrap();
    let context = f
        .access
        .context(principal, crate::multi_user_host::now())
        .unwrap();
    let turn_id = format!("turn-{}", uuid::Uuid::now_v7());
    let chat = scene["selected_chat"].as_str().unwrap();
    let ordinal = {
        let mut user = context.user.lock().unwrap();
        let mut session = user
            .agent_history
            .sessions
            .iter()
            .find(|s| s.id == chat)
            .unwrap()
            .clone();
        let ordinal = session.turns.last().map_or(1, |t| t.user_turn_ordinal + 1);
        session.turns.push(serde_json::from_value(json!({"turn_id":turn_id,"user_turn_ordinal":ordinal,
            "user":LEARNER_QUOTE,"status":"pending_assistant","published_book_ref":scene["published_book_ref"],
            "question_anchor_lid":null,"question_quote":null})).unwrap());
        let user = &mut *user;
        user.session_store
            .as_mut()
            .unwrap()
            .accept(&mut user.agent_history, session, NOW)
            .unwrap();
        ordinal
    };
    crate::workspace_registry::NetworkRunPort::capture(
        f.access.clone(),
        context,
        scene["workspace_id"].as_str().unwrap(),
        serde_json::from_value(Fixture::stamp(scene, ATTACHMENT)).unwrap(),
        AgentTurnRef {
            session_id: chat.into(),
            turn_id,
            user_turn_ordinal: ordinal,
        },
        ChatStubAdapter::scripted(vec![]).model_runtime_profile(),
        LEARNER_QUOTE,
    )
    .unwrap()
}

fn seed() -> (Fixture, tempfile::TempDir, Value) {
    seed_with_presentation(|_| {})
}

/// Shared real publication/private-store setup for the WB3 browser acceptance.
/// Only the seed presentation changes; subsequent product writes use real routes.
pub(super) fn seed_with_presentation(
    transform: fn(&mut runtime::presentation::PresentationContent),
) -> (Fixture, tempfile::TempDir, Value) {
    let f = Fixture::new();
    // Reuse the representative WB1 material and experiment, then publish them
    // before creating any network-private references to their immutable version.
    let (material_root, local, local_metadata) = super::board_browser_host::fixture();
    let map_path = material_root.path().join("teaching/versions/v1/map.json");
    let mut map: Value = serde_json::from_slice(&std::fs::read(&map_path).unwrap()).unwrap();
    let mut second = map["objects"]["objects"][0].clone();
    second["ref"]["object_id"] = json!("equal-time");
    second["meaning"] = json!("等时间条件：何时可以直接平均速度");
    second["aliases"] = json!(["等时间的算术平均"]);
    second["conditions"] = json!(["两段经历相同的时间，而不是相同的路程"]);
    map["objects"]["active_refs"]
        .as_array_mut()
        .unwrap()
        .push(second["ref"].clone());
    map["objects"]["objects"]
        .as_array_mut()
        .unwrap()
        .push(second);
    std::fs::write(map_path, serde_json::to_vec_pretty(&map).unwrap()).unwrap();
    let publication = {
        let mut library = f.access.library.lock().unwrap();
        let reference = library.publish(material_root.path()).unwrap().reference;
        library.grant("A", &reference).unwrap();
        reference
    };
    let learning = f.ok(
        &f.a,
        "POST",
        "/api/learning-workspaces",
        json!({
            "goal":{"text":"用完整板书理解平均速度与等时间条件"},"members":[publication,f.y]
        }),
    );
    let learning_id = learning["learning_workspace_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut scene = f.ok(&f.a, "POST", "/api/workspaces", json!({
        "published_book_ref":publication,"learning_workspace_id":learning_id,"attachment_id":ATTACHMENT
    }));
    scene = f.action(&f.a, &scene, ATTACHMENT, "chat/new", json!({}));
    let port = seed_run(&f, &scene);
    let local_reference: PresentationRef =
        serde_json::from_value(local_metadata["reference"].clone()).unwrap();
    let local_owner = PresentationOwner {
        book_id: local.workspace.book.base.book_id.clone(),
        session_id: local_metadata["session_id"].as_str().unwrap().into(),
    };
    let mut content = crate::presentation_store::PresentationStore::for_user(&local.user)
        .unwrap()
        .read_version(&local_owner, &local_reference)
        .unwrap()
        .content;
    transform(&mut content);
    for binding in &mut content.source_bindings {
        binding.publication_id = Some(publication.publication_id.clone());
        binding.source_revision = Some(port.scope.reading_book().source_fingerprint().into());
    }
    let sources = content.source_bindings.clone();
    let seed_answer = format!("先把两段路程与用时分别相加，再计算全程平均速度。去程与回程各 60 米，用时分别是 20 秒与 60 秒，因此平均速度为 1.5 m/s。 [[source:{}]]", sources[0].source_ref_id);
    let handle = f.access.users.lock().unwrap().get("A", NOW).unwrap();
    let reference;
    {
        let mut user = handle.lock().unwrap();
        let private = port.scope.private_user(&user);
        let candidate = private
            .create_presentation_candidate(
                &port.scope.chat_session_id,
                &port.scope.turn_id,
                None,
                content,
            )
            .unwrap();
        reference = private
            .persist_presentation_candidate(
                &port.scope.chat_session_id,
                &port.scope.turn_id,
                &candidate.candidate_id,
            )
            .unwrap();
        let mut session = user
            .agent_history
            .sessions
            .iter()
            .find(|s| s.id == port.scope.chat_session_id)
            .unwrap()
            .clone();
        let turn = session.turns.last_mut().unwrap();
        turn.status = AgentAssistantStatus::Completed;
        turn.source_bindings = sources.clone();
        let mut outcome = super::presentation_store_tests::outcome(&reference);
        outcome.answer = Some(seed_answer.clone());
        outcome
            .answer_view
            .as_mut()
            .unwrap()
            .parts
            .insert(0, AgentAnswerPart::Markdown { text: seed_answer });
        outcome.source_bindings = sources;
        turn.outcome = Some(outcome);
        turn.domain
            .effects
            .push(crate::session_event::EffectRecord {
                effect_id: format!("presentation:{}:1", reference.presentation_id),
                disposition: None,
                effect: crate::session_event::DeliveredEffect::Presentation {
                    reference: reference.clone(),
                },
            });
        let turn_ref = AgentTurnRef {
            session_id: session.id.clone(),
            turn_id: port.scope.turn_id.clone(),
            user_turn_ordinal: turn.user_turn_ordinal,
        };
        crate::session_runtime::finish(&mut user, &turn_ref, &session, NOW).unwrap();

        let mut learning = user
            .learning_store_for_source(Some(&learning_id), &publication.book_id)
            .unwrap();
        let before = learning.state().unwrap();
        learning
            .mutate(
                &memory::learning::TutorMutation {
                    operation_id: "wb2-enable".into(),
                    expected_revision: before.control.revision,
                    action: memory::learning::TutorAction::SetEnabled { enabled: true },
                },
                NOW,
            )
            .unwrap();
        teaching::start_request(&port.scope.private_user(&user), NOW).unwrap();
        let state = learning.state().unwrap();
        let tutor = state
            .sessions
            .get(state.control.current_tutor_session_id.as_ref().unwrap())
            .unwrap();
        let fact: TeachingEvent = serde_json::from_value(json!({
            "event_id":"wb2-learner-expression","binding":{
                "tutor_session_id":tutor.id,"session_revision":tutor.revision,"control_revision":state.control.revision,
                "source_id":publication.book_id,"source_revision":port.scope.reading_book().source_fingerprint(),"map_revision":"v1",
                "chat_session_id":port.scope.chat_session_id,"turn_id":port.scope.turn_id},
            "kind":"usage_observed","causal_refs":[],"occurred_at":NOW,
            "payload":{"usage":{"kind":"user_message"},"response":LEARNER_QUOTE}
        })).unwrap();
        learning.append_teaching(&fact).unwrap();
        let evidence: LearningEvidence = serde_json::from_value(json!({
            "evidence_id":"wb2-understanding","action_ref":fact.event_id,"session_id":tutor.id,
            "source_id":publication.book_id,"source_revision":port.scope.reading_book().source_fingerprint(),"map_revision":"v1",
            "nature":"hypothesis","fact_refs":[fact.event_id],"teaching_implication":"继续比较等路程与等时间的不同条件。",
            "label":"平均速度与慢段权重","capability":"explanation","assistance_refs":[],"attempt":0,
            "status":"unassessed","interpretation":"用户已经用自己的话指出慢段占用更多时间。",
            "learner_quote":LEARNER_QUOTE,"interpreter_version":"wb2-fixture","source_quotes":[],
            "target":{"learning_focus":"平均速度与慢段权重","expected_performance":"解释等路程时为什么不能直接平均速度",
                "capability":"explanation","source_bindings":[{"source_id":publication.book_id,
                    "source_revision":port.scope.reading_book().source_fingerprint(),"start_lid":"1.1","end_lid":"1.1"}],
                "object_refs":[{"source_id":publication.book_id,"object_id":"speed","object_revision":1}]}
        })).unwrap();
        learning.append_evidence(&evidence).unwrap();
    }
    // Initial boards are created through the real product routes. No control
    // endpoint performs the later automatic answer retention under test.
    let mut boards = Vec::new();
    for (title, object_id) in [
        ("平均速度 · 发问时的画板 A", "speed"),
        ("等时间条件 · 等待时的画板 B", "equal-time"),
    ] {
        scene = f.action(
            &f.a,
            &scene,
            ATTACHMENT,
            "boards/create",
            json!({"title":title}),
        );
        let mut board = scene["result"].clone();
        scene = f.action(&f.a, &scene, ATTACHMENT, "boards/import", json!({
            "board_id":board["board_id"],"board_revision":board["revision"],
            "source":{"kind":"prebuild","object_id":object_id,"object_revision":1,"map_revision":"v1"}
        }));
        board = scene["result"].clone();
        if object_id == "speed" {
            for source in [
                json!({"kind":"answer","session_id":port.scope.chat_session_id,"turn_id":port.scope.turn_id}),
                json!({"kind":"understanding","evidence_id":"wb2-understanding"}),
                json!({"kind":"prebuild","object_id":"equal-time","object_revision":1,"map_revision":"v1"}),
            ] {
                scene = f.action(&f.a, &scene, ATTACHMENT, "boards/import", json!({
                    "board_id":board["board_id"],"board_revision":board["revision"],"source":source
                }));
                board = scene["result"].clone();
            }
        }
        boards.push(board);
    }
    let item = |board: &Value, kind: &str, object: Option<&str>| {
        board["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|i| {
                i["target"]["kind"] == kind
                    && object.is_none_or(|id| i["target"]["object_id"] == id)
            })
            .unwrap()["item_id"]
            .clone()
    };
    let metadata = json!({
        "backend":format!("http://{BACKEND}"),"control":format!("http://{CONTROL}"),
        "cookie":format!("{}={}",crate::auth::COOKIE,f.a),"csrf":crate::auth::csrf(&f.a),
        "login":{"user_id":"A","password":"fixture-only-password"},"root":f.root.path(),
        "publication":publication,"second_publication":f.y,"learning_workspace_id":learning_id,"workspace":scene,
        "session_id":port.scope.chat_session_id,"seed_turn_id":port.scope.turn_id,"presentation":reference,
        "evidence_id":"wb2-understanding","learner_quote":LEARNER_QUOTE,"answer_marker":ANSWER_MARKER,
        "board_a":boards[0],"board_b":boards[1],
        "target_a_item_id":item(&boards[0],"prebuild",Some("speed")),
        "target_b_item_id":item(&boards[0],"prebuild",Some("equal-time")),
        "understanding_item_id":item(&boards[0],"understanding",None),
        "board_b_item_id":item(&boards[1],"prebuild",Some("equal-time"))
    });
    (f, material_root, metadata)
}

#[test]
#[ignore = "WB2 real network browser fixture; POST :4221/stop to finish"]
fn board_follow_up_browser_host() {
    let (f, _material_root, metadata) = seed();
    let model = ModelControl::default();
    let adapter_control = model.clone();
    f.access.runs.configure_fake(move || {
        Box::new(BoardProbe {
            stage: Cell::new(0),
            control: adapter_control.clone(),
        })
    });
    let server = crate::multi_user_host::start_with_access(
        BACKEND.parse().unwrap(),
        Arc::new(crate::multi_user_host::Site::new("https://reader.example").unwrap()),
        f.access.clone(),
    )
    .unwrap();
    let control = tiny_http::Server::http(CONTROL).unwrap();
    let output = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp/board-wb2");
    std::fs::create_dir_all(&output).unwrap();
    std::fs::write(
        output.join("fixture.json"),
        serde_json::to_vec_pretty(&metadata).unwrap(),
    )
    .unwrap();
    println!("WB2_BROWSER_READY http://{BACKEND} control=http://{CONTROL}");
    let deadline = std::time::Instant::now() + Duration::from_secs(3600);
    while std::time::Instant::now() < deadline {
        let Some(request) = control.recv_timeout(Duration::from_secs(1)).unwrap() else {
            continue;
        };
        let value = match request.url() {
            "/fixture" => metadata.clone(),
            "/model/block" => {
                let mut gate = model.gate.0.lock().unwrap();
                assert!(
                    !gate.waiting,
                    "Release the active model before arming another answer"
                );
                gate.blocked = true;
                model.requests.lock().unwrap().clear();
                json!({"blocked":true})
            }
            "/model/release" => {
                model.release();
                model.view()
            }
            "/model/requests" => model.view(),
            "/stop" => {
                model.release();
                request
                    .respond(tiny_http::Response::from_string("stopped"))
                    .unwrap();
                break;
            }
            _ => json!({"error":"unknown WB2 test control"}),
        };
        request
            .respond(
                tiny_http::Response::from_string(value.to_string()).with_header(
                    tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap(),
                ),
            )
            .unwrap();
    }
    model.release();
    server.shutdown();
}
