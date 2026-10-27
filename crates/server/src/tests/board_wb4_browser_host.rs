//! WB4 controlled model over real publication, author, preview, run and board stores.
use super::*;
use std::cell::{Cell, RefCell};
use std::sync::Condvar;

const BACKEND: &str = "127.0.0.1:4230";
const CONTROL: &str = "127.0.0.1:4231";
const SOURCE_QUOTE: &str = "平均速度是总路程除以总时间。";
const FIRST: &str = "先把两段用时看清楚";
const SECOND: &str = "再辨认直接平均的条件";
const PAGE: &str = r##"<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>两段时间实验 · 第 __VERSION__ 版</title>
<style>*{box-sizing:border-box}body{margin:0;padding:18px;background:#f5f4ed;color:#233e40;font:15px/1.6 sans-serif}main{max-width:620px;margin:auto}h1{font-size:20px;line-height:1.35;margin:0 0 12px}p{margin:12px 0}label{display:block}input{width:100%;min-height:48px;accent-color:#18857e}output{font-weight:700}.result{border-left:3px solid #18857e;padding:8px 12px;background:#e6eee8}.edition{font-size:12px;color:#63776e}</style></head><body><main><h1>两段时间实验</h1><p>每段都是 60 米，去程速度是 3 m/s。改变回程速度，观察两段用时。</p><label>回程速度：<output id="speed-label"></output> m/s<input id="return-speed" aria-label="回程速度" type="range" min="1" max="3" step="0.5" value="1" data-presentation-parameter="return_speed"></label><p>去程 20 秒 · 回程 <output id="return-time"></output> 秒</p><p id="result" class="result"></p><p class="edition" id="edition">第 __VERSION__ 版 · __CONDITION__</p></main><script>
const model={return_speed:1,...window.presentation.initialState};const byId=id=>document.getElementById(id);
function render(){const seconds=60/model.return_speed;byId('return-speed').value=model.return_speed;byId('speed-label').textContent=model.return_speed;byId('return-time').textContent=seconds.toFixed(1);byId('result').textContent='全程平均速度：'+(120/(20+seconds)).toFixed(2)+' m/s';}
byId('return-speed').oninput=event=>{model.return_speed=Number(event.target.value);render();window.presentation.commitState();};
window.presentation.registerStateReader(()=>({values:{return_speed:model.return_speed},visible_step:'explore',observation:{summary:byId('result').textContent,state:{return_speed:model.return_speed},rendered:{return_time:60/model.return_speed,average_speed:120/(20+60/model.return_speed)}}}));
window.presentation.registerStateRestorer(scene=>{Object.assign(model,scene.values.page);render();});
window.presentation.registerActionHandler(async action=>{if(action.kind==='set_parameters'){model.return_speed=Number(action.values.return_speed);render();}});render();
</script></body></html>"##;

#[derive(Default)]
struct Gate {
    waiting: Option<String>,
    generation: usize,
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
        json!({"waiting":gate.waiting,"generation":gate.generation,"mode":*self.mode.lock().unwrap(),"requests":*self.requests.lock().unwrap()})
    }
    fn release(&self) {
        self.gate.0.lock().unwrap().generation += 1;
        self.gate.1.notify_all();
    }
    fn wait(&self, checkpoint: &str) -> Result<(), AdapterError> {
        let mut gate = self.gate.0.lock().unwrap();
        gate.waiting = Some(checkpoint.into());
        let generation = gate.generation;
        let (mut gate, timeout) = self
            .gate
            .1
            .wait_timeout_while(gate, Duration::from_secs(180), |g| {
                g.generation == generation
            })
            .unwrap();
        gate.waiting = None;
        if timeout.timed_out() {
            Err(fail(format!("Browser did not release {checkpoint}")))
        } else {
            Ok(())
        }
    }
}
fn fail(message: impl Into<String>) -> AdapterError {
    AdapterError {
        message: message.into(),
        spend_stop: None,
    }
}
fn call(id: &str, name: &str, args: Value) -> AssistantTurn {
    AssistantTurn {
        provider_continuation: None,
        text: None,
        tool_calls: vec![runtime::ToolCall {
            id: id.into(),
            name: name.into(),
            arguments: args.to_string(),
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
struct Probe {
    stage: Cell<usize>,
    mode: String,
    control: Control,
    read: RefCell<Value>,
    layout: RefCell<Vec<Value>>,
    layout_calls: Cell<usize>,
}
impl Probe {
    fn read_layout(
        &self,
        messages: &[runtime::Message],
        part: usize,
    ) -> Result<Option<AssistantTurn>, AdapterError> {
        let count = self.layout_calls.get();
        let args = if count == 0 {
            let read = self.read.borrow();
            read["next_calls"].as_array().and_then(|calls|calls.iter().find(|c|c["arguments"]["collection"]=="layout"))
                .map(|c|c["arguments"].clone()).unwrap_or_else(||json!({"operation":"read","collection":"layout","offset":0,"length":20,"board_revision":read["board"]["revision"]}))
        } else {
            let page = body(messages, &format!("part-{part}-layout-{}", count - 1));
            if page["status"] != "board_page" {
                return Err(fail(format!("Layout read failed: {page}")));
            }
            self.layout.borrow_mut().extend(
                page["records"]
                    .as_array()
                    .ok_or_else(|| fail("No layout records"))?
                    .iter()
                    .cloned(),
            );
            if page["complete"] == true {
                if Some(self.layout.borrow().len() as u64) != page["total"].as_u64() {
                    return Err(fail("Incomplete recovered layout"));
                }
                return Ok(None);
            }
            page["next_calls"]
                .as_array()
                .and_then(|calls| {
                    calls
                        .iter()
                        .find(|c| c["arguments"]["collection"] == "layout")
                })
                .map(|c| c["arguments"].clone())
                .ok_or_else(|| fail("Missing layout continuation"))?
        };
        self.layout_calls.set(count + 1);
        self.stage.set(self.stage.get() - 1);
        Ok(Some(call(
            &format!("part-{part}-layout-{count}"),
            "board.author",
            args,
        )))
    }
    fn write_presentation(
        &self,
        messages: &[runtime::Message],
        part: usize,
    ) -> Result<Value, AdapterError> {
        let source = body(messages, "present-source");
        let source_ref = source["source_ref_id"]
            .as_str()
            .ok_or_else(|| fail(format!("No real source receipt: {source}")))?;
        let mut args = json!({"operation":"write","title":format!("两段时间实验 · 第 {part} 版"),
            "html":PAGE.replace("__VERSION__",&part.to_string()).replace("__CONDITION__",if part==1{"先分别计算用时"}else{"直接平均需要各段用时相同"}),
            "readable_content":"每段路程 60 米，去程速度 3 m/s，回程速度 1 至 3 m/s。平均速度等于总路程除以总时间。只有各段用时相同，才可直接平均速度。",
            "state_contract":{"return_speed":"number from 1 to 3 in steps of 0.5"},"initial_state":{"return_speed":1},
            "source_ref_ids":[source_ref],"assumptions":["每段内部保持匀速"],
            "action_contract":[{"kind":"set_parameters","parameters":["return_speed"]},{"kind":"highlight","region_id":"result"}]});
        if part == 2 {
            args["based_on"] = body(messages, "presentation-1-deliver")["reference"].clone();
        }
        Ok(args)
    }
    fn prepare_board(
        &self,
        messages: &[runtime::Message],
        part: usize,
    ) -> Result<Value, AdapterError> {
        let read = self.read.borrow();
        let revision = read["board"]["revision"]
            .as_u64()
            .ok_or_else(|| fail("No board revision"))?;
        let answer = read["reserved_answer_item_id"]
            .as_str()
            .ok_or_else(|| fail("No reserved part answer"))?;
        let source = body(messages, "present-source");
        let source_ref = source["source_ref_id"].as_str().unwrap();
        let id = format!("drawing-{}-{part}", self.mode);
        let block = format!("part-{part}-explanation");
        let text = if part == 1 {
            format!("### {FIRST}\n\n去程 60 ÷ 3 = 20 秒，回程 60 ÷ 1 = 60 秒。虽然两段路程相同，慢的那段占用更多时间。你现在可以操作这一部分的实验，观察回程用时的变化。 [[source:{source_ref}]]")
        } else {
            format!("### {SECOND}\n\n全程平均速度要用总路程除以总时间。只有各段用时相同，速度的算术平均才成立；等路程并不能替代等时间。这里保留第一部分，并新增带条件说明的第二版实验。 [[source:{source_ref}]]")
        };
        let mut y = read["answer_position"]["y"].as_f64().unwrap()
            + read["answer_size"]["height"].as_f64().unwrap();
        for item in self.layout.borrow().iter() {
            y = y.max(
                item["position"]["y"].as_f64().unwrap() + item["size"]["height"].as_f64().unwrap(),
            );
        }
        let target = json!({"kind":"item","item_id":id});
        let explanation = json!({"item_id":answer,"block_id":block,"content_revision":revision+1});
        Ok(json!({"operation":"prepare","base_board_revision":revision,
            "blocks":[{"block_id":block,"markdown":text}],
            "presentations":[body(messages,&format!("presentation-{part}-deliver"))["reference"].clone()],
            "drawings":[{"item_id":id,"title":if part==1{FIRST}else{SECOND},"position":{"x":640.5,"y":y+32.25},"size":{"width":290.0,"height":150.5},
                "graphic":{"kind":"shape","shape":"rectangle","text":if part==1{"去程 20 秒\n回程 60 秒"}else{"直接平均的条件\n各段用时相同"},"color":"#18857e"},"based_on":[]}],
            "links":[],"mappings":[{"mapping_id":format!("mapping-{}-{part}",self.mode),"explanation":explanation,"visual":target}],
            "routes":[{"route_id":format!("route-{}-{part}",self.mode),"title":if part==1{FIRST}else{SECOND},"steps":[{"step_id":format!("step-{}-{part}",self.mode),"explanation":explanation,"reveal_targets":[target.clone()],"focus_targets":[target],"transition":"draw","duration_ms":650}]}]}))
    }
    fn advance(
        &self,
        stage: usize,
        messages: &[runtime::Message],
    ) -> Result<AssistantTurn, AdapterError> {
        if stage == 0 {
            return Ok(call("read-source", "book.text", json!({"lid":"1.1"})));
        }
        if stage == 1 {
            return Ok(call(
                "present-source",
                "source.present",
                json!({"quote":SOURCE_QUOTE}),
            ));
        }
        if stage == 2 {
            return Ok(call(
                "discover-presentation",
                "tool.search",
                json!({"task":"create an interactive HTML presentation with a range input and persistent experimental state","required_capabilities":["presentation_authoring"],"scope":"passage","operation":"explain","effect_mode":"read_only","max_results":3}),
            ));
        }
        if stage == 13 {
            let result = body(messages, "part-1-commit");
            if result["status"] != "board_part_committed" {
                return Err(fail(format!("First commit failed: {result}")));
            }
            self.control.wait("first-committed")?;
            return Ok(call(
                "part-1-retry",
                "board.author",
                json!({"operation":"commit","candidate_revision":body(messages,"part-1-prepare")["candidate_revision"]}),
            ));
        }
        if stage == 14 {
            let retry = body(messages, "part-1-retry");
            if retry["status"] != "board_part_committed" || retry["already_committed"] != true {
                return Err(fail(format!("Commit retry was not idempotent: {retry}")));
            }
            if retry["delivery"] != body(messages, "part-1-commit")["delivery"] {
                return Err(fail("Retry changed receipt"));
            }
        }
        if stage == 24 {
            let second = body(messages, "part-2-commit");
            if second["status"] != "board_part_committed" {
                return Err(fail(format!("Second commit failed: {second}")));
            }
            self.control.wait("second-committed")?;
            return Ok(AssistantTurn{provider_continuation:None,text:Some("两个部分都已保存在画板上：先分别计算时间，再比较直接平均所需的等时间条件。可以从已保存的部分继续阅读与操作。".into()),tool_calls:vec![],usage_total_tokens:Some(1)});
        }
        let (part, phase) = if (3..13).contains(&stage) {
            (1, stage - 3)
        } else if (14..24).contains(&stage) {
            (2, stage - 14)
        } else {
            return Err(fail(format!(
                "Unexpected WB4 sampling {stage}: {}",
                json!(messages.last())
            )));
        };
        let p = format!("presentation-{part}");
        let b = format!("part-{part}");
        match phase {
            0 => Ok(call(
                &format!("{p}-prepare"),
                "presentation.author",
                json!({"operation":"prepare","phase":"local","framework":"先把两段用时看清楚，再辨认速度直接平均的条件。","focus":if part==1{FIRST}else{SECOND},"needs":["state"]}),
            )),
            1 => Ok(call(
                &format!("{p}-write"),
                "presentation.author",
                self.write_presentation(messages, part)?,
            )),
            2..=4 => {
                let candidate = body(messages, &format!("{p}-write"));
                if candidate["status"] != "candidate_saved" {
                    return Err(fail(format!("Presentation write failed: {candidate}")));
                }
                let (_, viewport) =
                    runtime::presentation_preview::REQUIRED_PREVIEW_ENVIRONMENTS[phase - 2];
                Ok(call(
                    &format!("{p}-preview-{}", phase - 2),
                    "presentation.author",
                    json!({"operation":"preview","candidate_id":candidate["candidate_id"],"viewport":viewport,"read_selector":"#result","actions":[{"kind":"key","selector":"#return-speed","key":"ArrowRight"}]}),
                ))
            }
            5 => Ok(call(
                &format!("{p}-deliver"),
                "presentation.author",
                json!({"operation":"deliver","candidate_id":body(messages,&format!("{p}-write"))["candidate_id"]}),
            )),
            6 => {
                let saved = body(messages, &format!("{p}-deliver"));
                if saved["status"] != "version_saved" {
                    return Err(fail(format!("Presentation delivery failed: {saved}")));
                }
                self.layout.borrow_mut().clear();
                self.layout_calls.set(0);
                Ok(call(
                    &format!("{b}-read"),
                    "board.author",
                    json!({"operation":"read"}),
                ))
            }
            7 => {
                *self.read.borrow_mut() = body(messages, &format!("{b}-read"));
                if let Some(call) = self.read_layout(messages, part)? {
                    return Ok(call);
                }
                Ok(call(
                    &format!("{b}-prepare"),
                    "board.author",
                    self.prepare_board(messages, part)?,
                ))
            }
            8 => {
                let prepared = body(messages, &format!("{b}-prepare"));
                if prepared["status"] != "candidate_prepared" {
                    return Err(fail(format!("Board prepare failed: {prepared}")));
                }
                Ok(call(
                    &format!("{b}-preview"),
                    "board.author",
                    json!({"operation":"preview","candidate_revision":prepared["candidate_revision"],"viewport":{"width":1280,"height":900,"input":"mouse"}}),
                ))
            }
            // phase 9 intentionally observes the actual preview in this later sampling.
            9 => {
                let preview = body(messages, &format!("{b}-preview"));
                if preview["status"] != "preview_ready_for_inspection" {
                    return Err(fail(format!("Board preview failed: {preview}")));
                }
                if part == 2 && self.mode == "cancel" {
                    self.control.wait("second-candidate-previewed")?;
                }
                Ok(call(
                    &format!("{b}-commit"),
                    "board.author",
                    json!({"operation":"commit","candidate_revision":body(messages,&format!("{b}-prepare"))["candidate_revision"]}),
                ))
            }
            _ => Err(fail(format!("Invalid author phase {phase}"))),
        }
    }
}
impl ModelAdapter for Probe {
    fn complete(&self, _: CompletionRequest) -> Result<ParsedResponse, AdapterError> {
        Err(fail("WB4 uses real agent tools"))
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
                    .map(|fact| json!({"fact_ref":fact["fact_ref"],"decision":"no_new_information","reason":"Delivering an explanation does not establish learner understanding."})).collect();
                return Ok(
                    json!({"next_step":{"focus":"Keep the formally saved board parts","participation":{"choice":"conclude","reason":"The requested run has ended."}},"observations":observations,"reason":"No mastery inference from delivered support."}),
                );
            }
        }
        if let Some(guidance) = super::tutor_loop_tests::fixture_coach_guidance(&request) {
            return Ok(guidance);
        }
        Err(fail(
            "WB4 has no background or presentation-coach completion",
        ))
    }
    fn chat(&self, request: &AgentRequestPlan) -> Result<AssistantTurn, AdapterError> {
        let stage = self.stage.get();
        self.stage.set(stage + 1);
        let messages = request.ordered_messages();
        let output = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp/board-wb4/browser");
        std::fs::create_dir_all(&output).unwrap();
        for (index, image) in request.preview_images.iter().enumerate() {
            let png = base64::Engine::decode(
                &base64::engine::general_purpose::STANDARD,
                &image.png_base64,
            )
            .unwrap();
            std::fs::write(
                output.join(format!("{}-sampling-{stage}-{index}.png", self.mode)),
                png,
            )
            .unwrap();
        }
        self.control.requests.lock().unwrap().push(json!({"mode":self.mode,"stage":stage,"messages":messages,"preview_image_count":request.preview_images.len()}));
        self.advance(stage, &messages)
    }
}

#[test]
#[ignore = "WB4 real browser host; POST :4231/stop to finish"]
fn board_wb4_browser_host() {
    let (f, _material_root, mut metadata) =
        super::board_follow_up_browser_host::seed_with_presentation(|_| {});
    // This Windows fixture uses the same real BrowserPreview as the desktop.
    // It does not exercise or enable the production Linux service sandbox.
    let preview_probe = f.access.sandbox.configure_browser_for_test().unwrap();
    let output = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp/board-wb4");
    std::fs::create_dir_all(&output).unwrap();
    std::fs::write(
        output.join("presentation-browser-probe.json"),
        serde_json::to_vec_pretty(&preview_probe).unwrap(),
    )
    .unwrap();
    let capability = f.access.sandbox.capability();
    assert_eq!(capability["preview"], true);
    assert_eq!(capability["plot"], false);
    assert_eq!(capability["animation"], false);
    metadata["presentation_execution_environment"] = json!({
        "kind":"cfg_test_real_browser_preview",
        "production_linux_sandbox":false,
        "browser":preview_probe.browser,
        "capability":capability,
        "probe":"presentation-browser-probe.json"
    });
    metadata["backend"] = json!(format!("http://{BACKEND}"));
    metadata["control"] = json!(format!("http://{CONTROL}"));
    metadata["first_marker"] = json!(FIRST);
    metadata["second_marker"] = json!(SECOND);
    let metadata = Arc::new(metadata);
    let control = Control::default();
    *control.mode.lock().unwrap() = "cancel".into();
    let adapter_control = control.clone();
    f.access.runs.configure_fake(move || {
        Box::new(Probe {
            stage: Cell::new(0),
            mode: adapter_control.mode.lock().unwrap().clone(),
            control: adapter_control.clone(),
            read: RefCell::new(Value::Null),
            layout: RefCell::new(vec![]),
            layout_calls: Cell::new(0),
        })
    });
    let server = crate::multi_user_host::start_with_access(
        BACKEND.parse().unwrap(),
        Arc::new(crate::multi_user_host::Site::new("https://reader.example").unwrap()),
        f.access.clone(),
    )
    .unwrap();
    let listener = tiny_http::Server::http(CONTROL).unwrap();
    std::fs::write(
        output.join("fixture.json"),
        serde_json::to_vec_pretty(&*metadata).unwrap(),
    )
    .unwrap();
    println!("WB4_BROWSER_READY http://{BACKEND} control=http://{CONTROL}");
    let deadline = std::time::Instant::now() + Duration::from_secs(3600);
    while std::time::Instant::now() < deadline {
        let Some(request) = listener.recv_timeout(Duration::from_secs(1)).unwrap() else {
            continue;
        };
        let value = match request.url() {
            "/fixture" => (*metadata).clone(),
            "/model/requests" => control.view(),
            "/model/release" => {
                control.release();
                control.view()
            }
            path if path.starts_with("/model/mode/") => {
                *control.mode.lock().unwrap() = path.trim_start_matches("/model/mode/").into();
                control.view()
            }
            "/stop" => {
                control.release();
                request
                    .respond(tiny_http::Response::from_string("stopped"))
                    .unwrap();
                break;
            }
            _ => json!({"error":"unknown WB4 control"}),
        };
        request
            .respond(
                tiny_http::Response::from_string(value.to_string()).with_header(
                    tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap(),
                ),
            )
            .unwrap();
    }
    control.release();
    server.shutdown();
}
