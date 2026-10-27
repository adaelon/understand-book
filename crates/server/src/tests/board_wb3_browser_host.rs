//! WB3 real browser host: controlled model, real author/preview/store/run routes.
use super::*;
use std::cell::{Cell, RefCell};
use std::sync::Condvar;

const BACKEND: &str = "127.0.0.1:4220";
const CONTROL: &str = "127.0.0.1:4221";
const SOURCE_QUOTE: &str = "平均速度是总路程除以总时间。";
const LEARNER_QUOTE: &str = "我以为两段路程相同就可以把速度直接平均。";
const ANSWER_MARKER: &str = "把路程与时间接起来";
const COMPARISON_MARKER: &str = "我的原话与正式条件对照";

#[derive(Default)]
struct Gate {
    blocked: bool,
    waiting: bool,
    completed: usize,
}

#[derive(Clone, Default)]
struct Control {
    gate: Arc<(Mutex<Gate>, Condvar)>,
    mode: Arc<Mutex<String>>,
    requests: Arc<Mutex<Vec<Value>>>,
}
impl Control {
    fn view(&self) -> Value {
        let gate = self.gate.0.lock().unwrap();
        json!({"blocked":gate.blocked,"waiting":gate.waiting,"completed":gate.completed,
            "mode":*self.mode.lock().unwrap(),"requests":*self.requests.lock().unwrap()})
    }
    fn release(&self) {
        self.gate.0.lock().unwrap().blocked = false;
        self.gate.1.notify_all();
    }
    fn wait(&self) -> Result<(), AdapterError> {
        let mut gate = self.gate.0.lock().unwrap();
        gate.waiting = gate.blocked;
        let (mut gate, timeout) = self
            .gate
            .1
            .wait_timeout_while(gate, Duration::from_secs(180), |g| g.blocked)
            .unwrap();
        gate.waiting = false;
        if timeout.timed_out() {
            return Err(fail("WB3 browser did not release inspected candidate"));
        }
        gate.completed += 1;
        Ok(())
    }
}
fn fail(message: impl Into<String>) -> AdapterError {
    AdapterError {
        message: message.into(),
        spend_stop: None,
    }
}
fn call(id: &str, name: &str, arguments: Value) -> AssistantTurn {
    AssistantTurn {
        provider_continuation: None,
        text: None,
        tool_calls: vec![runtime::ToolCall {
            id: id.into(),
            name: name.into(),
            arguments: arguments.to_string(),
        }],
        usage_total_tokens: Some(1),
    }
}
fn body(messages: &[runtime::Message], id: &str) -> Value {
    messages
        .iter()
        .rev()
        .find(|m| m.tool_call_id.as_deref() == Some(id))
        .and_then(|m| m.content.as_deref())
        .and_then(|s| serde_json::from_str::<Value>(s).ok())
        .map(|v| v.get("model_body").cloned().unwrap_or(v))
        .unwrap_or(Value::Null)
}
fn latest_fact(messages: &[runtime::Message]) -> Result<String, AdapterError> {
    for message in messages.iter().rev() {
        let Some(content) = message.content.as_deref() else {
            continue;
        };
        let Some(start) = content.rfind("Current learner view (private derived data") else {
            continue;
        };
        let Some(json_start) = content[start..].find('{') else {
            continue;
        };
        let value: Value = serde_json::Deserializer::from_str(&content[start + json_start..])
            .into_iter::<Value>()
            .next()
            .and_then(Result::ok)
            .unwrap_or(Value::Null);
        if let Some(expression) = value["learner_grounding"]["expressions"]
            .as_array()
            .and_then(|items| {
                items
                    .iter()
                    .rev()
                    .find(|item| item["text"] == LEARNER_QUOTE)
            })
        {
            if let Some(fact) = expression["fact_ref"].as_str() {
                return Ok(fact.into());
            }
        }
    }
    Err(fail(
        "WB3 requires an actual current learner fact in the runtime view",
    ))
}

struct Probe {
    stage: Cell<usize>,
    mode: String,
    control: Control,
    seed: Arc<Value>,
    read: RefCell<Value>,
    draft: RefCell<String>,
    layout_calls: Cell<usize>,
    layout: RefCell<Vec<Value>>,
}
impl Probe {
    fn read_layout(
        &self,
        messages: &[runtime::Message],
    ) -> Result<Option<AssistantTurn>, AdapterError> {
        let count = self.layout_calls.get();
        let arguments = if count == 0 {
            let read = self.read.borrow();
            read["next_calls"].as_array().and_then(|calls| calls.iter().find(|call| call["arguments"]["collection"] == "layout"))
                .map(|call| call["arguments"].clone()).unwrap_or_else(|| json!({"operation":"read","collection":"layout","offset":0,"length":20,"board_revision":read["board"]["revision"]}))
        } else {
            let page = body(messages, &format!("layout-page-{}", count - 1));
            if page["status"] != "board_page" || page["collection"] != "layout" {
                return Err(fail(format!("Expected real layout page: {page}")));
            }
            let records = page["records"]
                .as_array()
                .ok_or_else(|| fail("Layout page has no records"))?;
            self.layout.borrow_mut().extend(records.iter().cloned());
            if page["complete"] == true {
                let layout = self.layout.borrow();
                if Some(layout.len() as u64) != page["total"].as_u64() {
                    return Err(fail(
                        "Layout pagination did not recover every retained item",
                    ));
                }
                if self.mode != "native" {
                    for id in [
                        "drawing-native-time",
                        "drawing-native-total",
                        "drawing-native-condition",
                    ] {
                        if !layout.iter().any(|item| item["item_id"] == id) {
                            return Err(fail(format!(
                                "Next run failed to read retained native geometry {id}"
                            )));
                        }
                    }
                }
                return Ok(None);
            }
            page["next_calls"]
                .as_array()
                .and_then(|calls| {
                    calls
                        .iter()
                        .find(|call| call["arguments"]["collection"] == "layout")
                })
                .map(|call| call["arguments"].clone())
                .ok_or_else(|| fail("Incomplete layout has no exact continuation"))?
        };
        self.layout_calls.set(count + 1);
        // Keep the authored/interaction phase fixed while the model consumes
        // real collection pages in separate samplings.
        self.stage.set(self.stage.get() - 1);
        Ok(Some(call(
            &format!("layout-page-{count}"),
            "board.author",
            arguments,
        )))
    }

    fn prepare(&self, source: &Value) -> Result<Value, AdapterError> {
        let read = self.read.borrow();
        let revision = read["board"]["revision"]
            .as_u64()
            .ok_or_else(|| fail(format!("author read: {read}")))?;
        let answer = read["reserved_answer_item_id"]
            .as_str()
            .ok_or_else(|| fail("No reserved answer identity"))?;
        let current = read["board"]["items"].as_array().unwrap();
        let formal = current
            .iter()
            .find(|i| i["target"]["object_id"] == "speed")
            .ok_or_else(|| fail("Missing actual formal object"))?;
        let source_ref = source["source_ref_id"]
            .as_str()
            .ok_or_else(|| fail(format!("Real source missing: {source}")))?;
        let comparison = self.mode == "understanding";
        let formal_ref =
            json!({"item_id":formal["item_id"],"block_id":null,"content_revision":revision});
        let entries: Vec<(&str, String, &str)> = if comparison {
            vec![("learner-words",format!("### {COMPARISON_MARKER}\n\n你的原话是：“{LEARNER_QUOTE}” 这保留了你本次真实的表达。"),"我的原话\n等路程 → 直接平均？"),
                ("formal-condition",format!("### 正式条件\n\n书中的条件是“各段用时相同”。两段路程相同不能替代两段用时相同。 [[source:{source_ref}]]"),"正式条件\n各段用时相同"),
                ("interpretation", "### 这次理解判断\n\n这次表达提示你可能把等路程和等时间混在一起；这是基于这句原话的暂时解释，不是已经掌握的结论。可回到两段用时分别为 20 秒与 60 秒的例子检查。".into(),"暂时判断\n需要区分两种条件")]
        } else {
            vec![("time",format!("### {ANSWER_MARKER}\n\n先看每一段用时：去程 60 ÷ 3 = 20 秒，回程 60 ÷ 1 = 60 秒。相同的路程并没有产生相同的时间。"),"分别算时间\n去程 20 秒 · 回程 60 秒"),
                ("total",format!("### 把整体合起来\n\n总路程是 60 + 60 = 120 米，总时间是 20 + 60 = 80 秒。全程平均速度是 120 ÷ 80 = **1.5 m/s**。 [[source:{source_ref}]]"),"先相加，再相除\n120 米 ÷ 80 秒 = 1.5 m/s"),
                ("condition","### 什么时候可以直接平均\n\n只有各段用时相同，速度的算术平均才等于全程平均速度。慢段占用更多时间，因此本例不能直接用 (3 + 1) ÷ 2。".into(),"直接平均的条件\n各段用时相同")]
        };
        let prefix = if comparison { "comparison" } else { "native" };
        let understanding = read["understanding_items"]
            .as_array()
            .and_then(|a| a.first());
        let understanding_ref = understanding.and_then(|item| {
            read["understanding_refs"].as_array().and_then(|refs| {
                refs.iter()
                    .find(|reference| reference["item_id"] == item["item_id"])
            })
        });
        if comparison && understanding_ref.is_none() {
            return Err(fail(format!("Real new understanding absent: {read}")));
        }
        let blocks: Vec<_> = entries.iter().map(|(id,markdown,_)|json!({"block_id":format!("{prefix}-{id}"),"markdown":markdown})).collect();
        let mut drawing_y = read["answer_position"]["y"].as_f64().unwrap()
            + read["answer_size"]["height"].as_f64().unwrap();
        for item in self.layout.borrow().iter() {
            drawing_y = drawing_y.max(
                item["position"]["y"].as_f64().unwrap() + item["size"]["height"].as_f64().unwrap(),
            );
        }
        drawing_y += 32.0;
        let drawings: Vec<_> = entries.iter().enumerate().map(|(index,(id,_,text))| {
            let mut based_on = vec![formal_ref.clone()];
            if let Some(reference) = understanding_ref {based_on.push(reference.clone());}
            json!({"item_id":format!("drawing-{prefix}-{id}"),"title":text.lines().next().unwrap(),
                "position":{"x":index as f64*340.0,"y":drawing_y},"size":{"width":290,"height":180},
                "graphic":{"kind":"shape","shape":"rectangle","text":text,"color":if comparison {"#b77928"}else{"#177e77"}},"based_on":based_on})
        }).collect();
        let mut mappings: Vec<_> = entries.iter().map(|(id,_,_)|json!({"mapping_id":format!("mapping-{prefix}-{id}"),
            "explanation":{"item_id":answer,"block_id":format!("{prefix}-{id}"),"content_revision":revision+1},
            "visual":{"kind":"item","item_id":format!("drawing-{prefix}-{id}")}})).collect();
        let steps: Vec<_> = mappings.iter().enumerate().map(|(i,m)|json!({"step_id":format!("step-{prefix}-{i}"),"explanation":m["explanation"],
            "reveal_targets":[m["visual"]],"focus_targets":[m["visual"]],"transition":if i==0 {"draw"}else{"reveal_text"},"duration_ms":900})).collect();
        let links: Vec<_> = (0..2).map(|i|json!({"link_id":format!("link-{prefix}-{i}"),
            "from":mappings[i]["visual"],"to":mappings[i+1]["visual"],"label":if comparison {"对照"}else{"然后"},"based_on":[formal_ref]})).collect();
        if !comparison {
            let presentation = current
                .iter()
                .find(|i| i["target"]["kind"] == "presentation")
                .unwrap();
            mappings.push(json!({"mapping_id":"mapping-native-experiment","explanation":mappings[1]["explanation"],
                "visual":{"kind":"presentation_anchor","item_id":presentation["item_id"],"reference":presentation["target"]["receipt"]["reference"],"anchor_id":"result"}}));
        }
        *self.draft.borrow_mut() = entries
            .iter()
            .map(|(_, markdown, _)| markdown.as_str())
            .collect::<Vec<_>>()
            .join("\n\n");
        Ok(
            json!({"operation":"prepare","base_board_revision":revision,"blocks":blocks,"drawings":drawings,"links":links,"mappings":mappings,
            "routes":[{"route_id":format!("route-{prefix}"),"title":if comparison {COMPARISON_MARKER}else{ANSWER_MARKER},"steps":steps}]}),
        )
    }

    fn author(
        &self,
        stage: usize,
        messages: &[runtime::Message],
    ) -> Result<AssistantTurn, AdapterError> {
        let comparison = self.mode == "understanding";
        if stage == 0 {
            return Ok(call("read-source", "book.text", json!({"lid":"1.1"})));
        }
        if comparison && stage == 1 {
            return Ok(call(
                "new-evidence",
                "tutor.step",
                json!({"operation":"evidence","nature":"hypothesis","operation_id":"wb3-new-understanding",
                "target":{"learning_focus":"平均速度的适用条件","expected_performance":"区分等路程与等时间，说明何时可以直接平均速度","capability":"explanation",
                    "source_bindings":[{"source_id":self.seed["publication"]["book_id"],"source_revision":self.seed["board_a"]["source_revision"],"start_lid":"1.1","end_lid":"1.1"}]},
                "fact_refs":[latest_fact(messages)?],"interpretation":"本次原话把等路程与等时间条件混在一起，仍需用两段用时检验。",
                "teaching_implication":"把用户原话与正式知识点的等时间条件并排比较。"}),
            ));
        }
        let offset = if comparison { 1 } else { 0 };
        match stage - offset {
            1 => Ok(call(
                "present-source",
                "source.present",
                json!({"quote":SOURCE_QUOTE}),
            )),
            2 => Ok(call(
                "author-read",
                "board.author",
                json!({"operation":"read"}),
            )),
            3 => {
                *self.read.borrow_mut() = body(messages, "author-read");
                if let Some(next) = self.read_layout(messages)? {
                    return Ok(next);
                }
                Ok(call(
                    "author-prepare",
                    "board.author",
                    self.prepare(&body(messages, "present-source"))?,
                ))
            }
            4 => {
                let candidate = body(messages, "author-prepare");
                if candidate["status"] != "candidate_prepared" {
                    return Err(fail(format!("Prepare failed: {candidate}")));
                }
                Ok(call(
                    "author-preview",
                    "board.author",
                    json!({"operation":"preview","candidate_revision":candidate["candidate_revision"],"viewport":{"width":1280,"height":900,"input":"mouse"}}),
                ))
            }
            5 => {
                let preview = body(messages, "author-preview");
                if preview["status"] != "preview_ready_for_inspection" {
                    return Err(fail(format!("Preview failed: {preview}")));
                }
                self.control.wait()?;
                Ok(call(
                    "author-deliver",
                    "board.author",
                    json!({"operation":"deliver","candidate_revision":body(messages,"author-prepare")["candidate_revision"]}),
                ))
            }
            6 => {
                let delivery = body(messages, "author-deliver");
                if delivery["status"] != "delivery_staged" {
                    return Err(fail(format!("Delivery failed: {delivery}")));
                }
                Ok(AssistantTurn {
                    provider_continuation: None,
                    text: Some(self.draft.borrow().clone()),
                    tool_calls: vec![],
                    usage_total_tokens: Some(1),
                })
            }
            _ => Err(fail(format!(
                "Unexpected WB3 repair at {stage}: {}",
                json!(messages.last())
            ))),
        }
    }

    fn interact(
        &self,
        stage: usize,
        messages: &[runtime::Message],
    ) -> Result<AssistantTurn, AdapterError> {
        if stage == 0 {
            return Ok(call(
                "retained-discover",
                "tool.search",
                json!({"task":"operate retained presentation","required_capabilities":["presentation_interaction"],"scope":"passage","operation":"explain","effect_mode":"read_only","max_results":1}),
            ));
        }
        if stage == 1 {
            return Ok(call(
                "retained-read",
                "board.author",
                json!({"operation":"read"}),
            ));
        }
        if stage == 2 {
            *self.read.borrow_mut() = body(messages, "retained-read");
            if let Some(next) = self.read_layout(messages)? {
                return Ok(next);
            }
        }
        let read = self.read.borrow();
        let item = read["board"]["items"]
            .as_array()
            .and_then(|a| a.iter().find(|i| i["target"]["kind"] == "presentation"))
            .ok_or_else(|| fail(format!("Missing retained presentation: {read}")))?;
        let target = json!({"board_id":read["board"]["board_id"],"item_id":item["item_id"],"board_revision":read["board"]["revision"]});
        let reference = item["target"]["receipt"]["reference"].clone();
        match stage {
            2 => Ok(call(
                "retained-observe",
                "presentation.interact",
                json!({"operation":"observe","reference":reference,"board_target":target}),
            )),
            3 | 4 => {
                let prior = body(
                    messages,
                    if stage == 3 {
                        "retained-observe"
                    } else {
                        "retained-set"
                    },
                );
                if !matches!(prior["status"].as_str(), Some("observed" | "applied")) {
                    return Err(fail(format!("Real browser receipt failed: {prior}")));
                }
                Ok(call(
                    if stage == 3 {
                        "retained-set"
                    } else {
                        "retained-apply"
                    },
                    "presentation.interact",
                    json!({"operation":"act","reference":reference,"board_target":target,
                    "based_on":prior["saved_state_ref"],"action":if stage==3 {json!({"kind":"set_parameters","values":{"return_speed":2}})}else{json!({"kind":"invoke","action_id":"compare"})}}),
                ))
            }
            5 => {
                let result = body(messages, "retained-apply");
                if result["status"] != "applied" {
                    return Err(fail(format!("Apply failed: {result}")));
                }
                Ok(AssistantTurn{provider_continuation:None,text:Some("### 新对话继续旧演示\n\n已根据真实画面回执将回程速度设为 2 m/s，并显示比较结论。回程用时变为 30 秒，总时间 50 秒，全程平均速度为 2.4 m/s。原演示内容版本保持不变。".into()),tool_calls:vec![],usage_total_tokens:Some(1)})
            }
            _ => Err(fail("Unexpected retained-interaction repair")),
        }
    }
}

impl ModelAdapter for Probe {
    fn complete(&self, _: CompletionRequest) -> Result<ParsedResponse, AdapterError> {
        Err(fail("WB3 uses only real agent tools"))
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
                let observations:Vec<_>=input["observation_facts"].as_array().into_iter().flatten().filter(|f|f["already_interpreted"]!=true)
                    .map(|f|json!({"fact_ref":f["fact_ref"],"decision":"no_new_information","reason":"Only the explicitly recorded learner expression is interpreted."})).collect();
                return Ok(
                    json!({"next_step":{"focus":"Keep the completed board explanation","participation":{"choice":"conclude","reason":"The requested explanation is delivered."}},
                    "observations":observations,"reason":"Do not infer mastery from delivered help."}),
                );
            }
        }
        if let Some(guidance) = super::tutor_loop_tests::fixture_coach_guidance(&request) {
            return Ok(guidance);
        }
        Err(fail("WB3 has no background profile completion"))
    }
    fn chat(&self, request: &AgentRequestPlan) -> Result<AssistantTurn, AdapterError> {
        let stage = self.stage.get();
        self.stage.set(stage + 1);
        let messages = request.ordered_messages();
        if !request.preview_images.is_empty() {
            let output = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp/board-wb3/browser");
            std::fs::create_dir_all(&output).unwrap();
            for (index, image) in request.preview_images.iter().enumerate() {
                let png = base64::Engine::decode(
                    &base64::engine::general_purpose::STANDARD,
                    &image.png_base64,
                )
                .unwrap();
                std::fs::write(
                    output.join(format!("candidate-{}-{index}.png", self.mode)),
                    png,
                )
                .unwrap();
            }
        }
        self.control.requests.lock().unwrap().push(json!({"mode":self.mode,"stage":stage,"messages":messages,"preview_image_count":request.preview_images.len()}));
        if self.mode == "interaction" {
            self.interact(stage, &messages)
        } else {
            self.author(stage, &messages)
        }
    }
}

fn seed_presentation(content: &mut runtime::presentation::PresentationContent) {
    let entry = content.content_files.get_mut(&content.entrypoint).unwrap();
    *entry = entry.replace(
        "<p id=\"result\">",
        "<p id=\"result\" data-presentation-explanation=\"result\">",
    );
    *entry=entry.replace("window.presentation.registerStateReader",r#"window.presentation.registerActionHandler(async action=>{
      if(action.kind==='set_parameters'){model.return_speed=Number(action.values.return_speed);render();}
      else if(action.kind==='invoke'&&action.action_id==='compare'){model.revealed=true;render();}
    });
window.presentation.registerStateReader"#);
    content
        .action_contract
        .push(serde_json::from_value(json!({"kind":"highlight","region_id":"result"})).unwrap());
}

#[test]
#[ignore = "WB3 real network/browser fixture; POST :4221/stop to finish"]
fn board_wb3_browser_host() {
    let (f, _material_root, mut metadata) =
        super::board_follow_up_browser_host::seed_with_presentation(seed_presentation);
    metadata["answer_marker"] = json!(ANSWER_MARKER);
    metadata["comparison_marker"] = json!(COMPARISON_MARKER);
    metadata["new_learner_quote"] = json!(LEARNER_QUOTE);
    metadata["new_evidence_id"] = json!("interpretation:wb3-new-understanding");
    let metadata = Arc::new(metadata);
    let model = Control::default();
    *model.mode.lock().unwrap() = "native".into();
    let adapter_control = model.clone();
    let seed = metadata.clone();
    f.access.runs.configure_fake(move || {
        Box::new(Probe {
            stage: Cell::new(0),
            mode: adapter_control.mode.lock().unwrap().clone(),
            control: adapter_control.clone(),
            seed: seed.clone(),
            read: RefCell::new(Value::Null),
            draft: RefCell::new(String::new()),
            layout_calls: Cell::new(0),
            layout: RefCell::new(Vec::new()),
        })
    });
    let server = crate::multi_user_host::start_with_access(
        BACKEND.parse().unwrap(),
        Arc::new(crate::multi_user_host::Site::new("https://reader.example").unwrap()),
        f.access.clone(),
    )
    .unwrap();
    let control = tiny_http::Server::http(CONTROL).unwrap();
    let output = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp/board-wb3");
    std::fs::create_dir_all(&output).unwrap();
    std::fs::write(
        output.join("fixture.json"),
        serde_json::to_vec_pretty(&*metadata).unwrap(),
    )
    .unwrap();
    println!("WB3_BROWSER_READY http://{BACKEND} control=http://{CONTROL}");
    let deadline = std::time::Instant::now() + Duration::from_secs(3600);
    while std::time::Instant::now() < deadline {
        let Some(request) = control.recv_timeout(Duration::from_secs(1)).unwrap() else {
            continue;
        };
        let value = match request.url() {
            "/fixture" => (*metadata).clone(),
            "/model/block" => {
                let mut gate = model.gate.0.lock().unwrap();
                assert!(!gate.waiting);
                gate.blocked = true;
                json!({"blocked":true})
            }
            "/model/release" => {
                model.release();
                model.view()
            }
            "/model/requests" => model.view(),
            path if path.starts_with("/model/mode/") => {
                *model.mode.lock().unwrap() = path.trim_start_matches("/model/mode/").into();
                model.view()
            }
            "/stop" => {
                model.release();
                request
                    .respond(tiny_http::Response::from_string("stopped"))
                    .unwrap();
                break;
            }
            _ => json!({"error":"unknown WB3 test control"}),
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
