use super::*;
#[path = "presentation_ex10_tests.rs"]
pub(crate) mod ex10;
#[path = "presentation_ex11_tests.rs"]
mod ex11;
#[path = "presentation_animation_tests.rs"]
mod animation;
use crate::agent_run::{AppStatePort, BorrowedAppPort, RuntimeStatePort};
use runtime::{
    presentation_author::*,
    presentation_preview::{PreviewAction, REQUIRED_PREVIEW_ENVIRONMENTS},
    run_context::{CancellationToken, ResidentStatePort},
};

fn setup() -> (tempfile::TempDir, AppState, AgentTurnRef) {
    let root = tempfile::tempdir().unwrap();
    let mut state = state_named("rp4-authoring");
    state.history_path = Some(root.path().join("history.json"));
    let book = state.book.base.book_id.clone();
    let turn = precommit_agent_turn(
        &mut state,
        &book,
        "interactive example".into(),
        None,
        None,
        None,
        "2026-09-17T00:00:00Z",
    )
    .unwrap();
    (root, state, turn)
}

#[test]
fn presentation_ex6_model_preview_json_deserializes() {
    let request: AuthorRequest = serde_json::from_str(r#"{"operation":"preview","candidate_id":"c1","actions":[{"kind":"seek","semantic_state":1,"transition_progress":0.5}]}"#).unwrap();
    assert!(matches!(request, AuthorRequest::Preview { actions, .. } if matches!(actions.as_slice(), [PreviewAction::Seek { semantic_state: 1, transition_progress }] if transition_progress.as_f64() == Some(0.5))));
}
fn write(html: &str) -> AuthorRequest {
    AuthorRequest::Write {
        libraries: vec![],
        based_on: None,
        state_contract: json!({}),
        title: "参数实验".into(),
        html: html.into(),
        readable_content: "已找到两处证据，共需三处。补齐后为一。".into(),
        asset_refs: vec![],
        source_ref_ids: vec![],
        assumptions: vec![],
        initial_state: json!({"count":2}),
    }
}
fn candidate(port: &mut impl ResidentStatePort, html: &str) -> String {
    port.author_presentation(write(html), &[], &[], &CancellationToken::default())
        .unwrap()
        .body["candidate_id"]
        .as_str()
        .unwrap()
        .into()
}
fn working() -> String {
    std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/presentation-author.html"),
    )
    .unwrap()
}

#[test]
#[ignore = "requires installed Chromium/Edge"]
fn selected_result_reads_live_state_after_actions() {
    let (_root, mut state, turn) = setup();
    let app = BorrowedAppPort(std::cell::RefCell::new(&mut state));
    let mut port = RuntimeStatePort { port: &app, turn_ref: &turn, previewed: Default::default(), animations: Default::default(), plots: Default::default() };
    let html = r#"<input id="eta" type="number" value="1.2"><button id="next">Next</button><p id="result"></p><script>
      let k=4,w=0;const eta=document.getElementById('eta'),result=document.getElementById('result');
      function render(){w=0;for(let i=0;i<k;i++)w=w-2*Number(eta.value)*(w-2);const next=w-2*Number(eta.value)*(w-2);result.textContent=`eta=${eta.value}; k=${k}; w=${w.toFixed(6)}; next k=${k+1}; w=${next.toFixed(6)}`;}
      document.getElementById('next').onclick=()=>{k++;render()};
      window.presentation.registerStateReader(()=>({values:{eta:Number(eta.value),k},visible_step:String(k)}));render();
    </script>"#;
    let id = candidate(&mut port, html);
    let result = port.author_presentation(AuthorRequest::Preview {
        candidate_id: id.clone(), read_selector: Some("#result".into()), width: None, viewport: None,
        actions: vec![PreviewAction::Click { selector: "#next".into() }],
    }, &[], &[], &CancellationToken::default()).unwrap();
    assert_eq!(result.body["candidate_id"], id);
    let reading = &result.body["reading"];
    assert_eq!(reading["text"], "eta=1.2; k=5; w=12.756480; next k=6; w=-13.059072");
    assert_eq!(reading["page_state"], json!({"values":{"eta":1.2,"k":5},"visible_step":"5"}));
    assert_eq!(reading["controls"][0]["value"], "1.2");
    assert_eq!(reading["action_step"], 1);
    assert_eq!(result.images.len(), 2);
}

#[test]
fn presentation_plot_renders_real_image_and_freezes_assets_in_candidate() {
    let (_root, mut state, turn) = setup();
    let app = BorrowedAppPort(std::cell::RefCell::new(&mut state));
    let mut port = RuntimeStatePort {
        port: &app,
        turn_ref: &turn,
        previewed: Default::default(),
        animations: Default::default(), plots: Default::default(),
    };
    let cancellation = CancellationToken::default();
    let plot = port.author_presentation(
        AuthorRequest::RenderPlot {
            code: "ax.plot(data['x'], data['y'], marker='o', label='实验'); ax.set_xlabel('时间 (秒)'); ax.set_ylabel('温度 (°C)'); ax.legend()".into(),
            data: json!({"x":[0,1,2],"y":[20,25,30]}),
            size: Some(PlotSize { width: 640, height: 400 }),
        }, &[], &[], &cancellation,
    ).unwrap();
    assert_eq!(plot.body["mime"], "image/svg+xml");
    assert_eq!(plot.body["width"], 640);
    assert!(plot.images[0].png_base64.starts_with("iVBOR"));
    let asset_ref = plot.body["asset_ref"].as_str().unwrap();
    let asset_path = plot.body["asset_path"].as_str().unwrap();
    let html = format!("<h1>温度变化</h1><img src=\"{asset_path}\" alt=\"时间与温度折线图\">");
    let saved = port
        .author_presentation(
            AuthorRequest::Write {
                libraries: vec![],
                based_on: None,
                state_contract: json!({}),
                title: "温度变化".into(),
                html,
                readable_content:
                    "时间从 0 到 2 秒，温度从 20 升至 30 摄氏度；数据为本次示例输入。".into(),
                asset_refs: vec![asset_ref.into()],
                source_ref_ids: vec![],
                assumptions: vec![],
                initial_state: json!({}),
            },
            &[],
            &[],
            &cancellation,
        )
        .unwrap();
    let candidate = app
        .with_app(|state| {
            state.read_presentation_candidate(
                &turn.session_id,
                saved.body["candidate_id"].as_str().unwrap(),
            )
        })
        .unwrap();
    assert!(candidate.content.content_files[asset_path].contains("时间 (秒)"));
    assert!(candidate.content.content_files[&format!("plots/{asset_ref}.py")].contains("ax.plot"));
    assert!(candidate.content.content_files[&format!("plots/{asset_ref}.json")].contains("\"y\""));
    if let Ok(directory) = std::env::var("ED2_EVIDENCE_DIR") {
        use base64::Engine as _;
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            std::path::Path::new(&directory).join("plot.svg"),
            &candidate.content.content_files[asset_path],
        )
        .unwrap();
        std::fs::write(
            std::path::Path::new(&directory).join("plot.png"),
            base64::engine::general_purpose::STANDARD
                .decode(&plot.images[0].png_base64)
                .unwrap(),
        )
        .unwrap();
    }
    let error = port
        .author_presentation(
            AuthorRequest::RenderPlot {
                code: "raise ValueError('bad axis')".into(),
                data: json!({}),
                size: None,
            },
            &[],
            &[],
            &cancellation,
        )
        .err()
        .unwrap();
    assert_eq!(error.error_code, "PRESENTATION_PLOT_FAILED");
    assert!(error.message.contains("bad axis"));
}

#[test]
fn presentation_plot_cancellation_stops_running_python() {
    let cancellation = CancellationToken::default();
    let stop = cancellation.clone();
    let stopper = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(300));
        stop.cancel();
    });
    let started = std::time::Instant::now();
    let result = crate::presentation_plot::render(
        "import time; time.sleep(10)".into(),
        json!({}),
        None,
        &cancellation,
    );
    stopper.join().unwrap();
    assert_eq!(result.err().unwrap().error_code, "AGENT_RUN_CANCELLED");
    assert!(started.elapsed() < std::time::Duration::from_secs(5));
}

#[test]
#[ignore = "requires installed Chromium/Edge and plot Python"]
fn presentation_plot_previews_and_reopens_the_same_version_asset() {
    let (_root, mut state, turn) = setup();
    let app = BorrowedAppPort(std::cell::RefCell::new(&mut state));
    let mut port = RuntimeStatePort {
        port: &app,
        turn_ref: &turn,
        previewed: Default::default(),
        animations: Default::default(), plots: Default::default(),
    };
    let cancellation = CancellationToken::default();
    let plot = port.author_presentation(AuthorRequest::RenderPlot {
        code: "ax.plot(data['x'], data['y'], marker='o', label='实验'); ax.set_xlabel('时间 (秒)'); ax.set_ylabel('温度 (°C)'); ax.legend()".into(),
        data: json!({"x":[0,1,2],"y":[20,25,30]}), size: Some(PlotSize { width: 640, height: 400 }),
    }, &[], &[], &cancellation).unwrap();
    let asset_ref = plot.body["asset_ref"].as_str().unwrap().to_string();
    let path = plot.body["asset_path"].as_str().unwrap().to_string();
    let candidate_id = port
        .author_presentation(
            AuthorRequest::Write {
                libraries: vec![],
                based_on: None,
                state_contract: json!({}),
                title: "温度变化".into(),
                html: format!("<h1>温度变化</h1><img src=\"{path}\" alt=\"时间与温度折线图\">"),
                readable_content:
                    "横轴时间（秒），纵轴温度（摄氏度）；示例数据点为 (0,20)、(1,25)、(2,30)。"
                        .into(),
                asset_refs: vec![asset_ref.clone()],
                source_ref_ids: vec![],
                assumptions: vec![],
                initial_state: json!({}),
            },
            &[],
            &[],
            &cancellation,
        )
        .unwrap()
        .body["candidate_id"]
        .as_str()
        .unwrap()
        .to_string();
    let preview = port
        .author_presentation(
            AuthorRequest::Preview {
                read_selector: None,
                candidate_id: candidate_id.clone(),
                width: None,
                viewport: None,
                actions: vec![],
            },
            &[],
            &[],
            &cancellation,
        )
        .unwrap();
    assert_eq!(
        preview.body["status"], "preview_ready_for_inspection",
        "{}",
        preview.body
    );
    assert!(preview.images[0].png_base64.starts_with("iVBOR"));
    let reference = port
        .author_presentation(
            AuthorRequest::Deliver { candidate_id },
            &[],
            &[],
            &cancellation,
        )
        .unwrap()
        .delivered
        .unwrap();
    let original = app
        .with_app(|state| state.read_presentation(&turn.session_id, &reference))
        .unwrap();
    let mut reopened = state_named("rp4-authoring");
    reopened.history_path = app.with_app(|state| state.history_path.clone());
    reopened.agent_history = app.with_app(|state| state.agent_history.clone());
    let disk = reopened
        .read_presentation(&turn.session_id, &reference)
        .unwrap();
    assert_eq!(
        disk.content.content_files[&path],
        original.content.content_files[&path]
    );
    assert!(disk.content.content_files[&format!("plots/{asset_ref}.py")].contains("ax.plot"));
    let next_plot = port
        .author_presentation(
            AuthorRequest::RenderPlot {
                code: "ax.plot(data['x'], data['y']); ax.set_xlabel('时间 (秒)')".into(),
                data: json!({"x":[0,1,2],"y":[20,22,24]}),
                size: Some(PlotSize {
                    width: 640,
                    height: 400,
                }),
            },
            &[],
            &[],
            &cancellation,
        )
        .unwrap();
    let next_ref = next_plot.body["asset_ref"].as_str().unwrap().to_string();
    let next_path = next_plot.body["asset_path"].as_str().unwrap().to_string();
    let next_id = port
        .author_presentation(
            AuthorRequest::Write {
                libraries: vec![],
                based_on: Some(reference.clone()),
                state_contract: json!({}),
                title: "修订温度变化".into(),
                html: format!(
                    "<h1>修订温度变化</h1><img src=\"{next_path}\" alt=\"时间与温度折线图\">"
                ),
                readable_content: "修订数据点为 (0,20)、(1,22)、(2,24)。".into(),
                asset_refs: vec![next_ref],
                source_ref_ids: vec![],
                assumptions: vec![],
                initial_state: json!({}),
            },
            &[],
            &[],
            &cancellation,
        )
        .unwrap()
        .body["candidate_id"]
        .as_str()
        .unwrap()
        .to_string();
    let next_candidate = app
        .with_app(|state| state.read_presentation_candidate(&turn.session_id, &next_id))
        .unwrap();
    assert!(next_candidate
        .content
        .content_files
        .contains_key(&next_path));
    assert!(!next_candidate.content.content_files.contains_key(&path));
    let next_preview = port
        .author_presentation(
            AuthorRequest::Preview {
                read_selector: None,
                candidate_id: next_id.clone(),
                width: None,
                viewport: None,
                actions: vec![],
            },
            &[],
            &[],
            &cancellation,
        )
        .unwrap();
    assert_eq!(
        next_preview.body["status"], "preview_ready_for_inspection",
        "{}",
        next_preview.body
    );
    let next_version = port
        .author_presentation(
            AuthorRequest::Deliver {
                candidate_id: next_id,
            },
            &[],
            &[],
            &cancellation,
        )
        .unwrap()
        .delivered
        .unwrap();
    assert_eq!(next_version.revision, reference.revision + 1);
    assert_eq!(
        app.with_app(|state| state.read_presentation(&turn.session_id, &reference))
            .unwrap()
            .content
            .content_files[&path],
        disk.content.content_files[&path]
    );
}

#[test]
#[ignore = "real configured model, plot Python and browser; writes isolated evidence"]
fn presentation_plot_real_resident_route() {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering::SeqCst};
    let browser_baseline = std::env::var("ED2_BROWSER_BASELINE").is_ok();
    let root = std::path::PathBuf::from(
        std::env::var("ED2_REAL_EVIDENCE_DIR").expect("set ED2_REAL_EVIDENCE_DIR"),
    );
    std::fs::create_dir_all(&root).unwrap();
    let mut state = state_named("ed2-live-plot");
    state.history_path = Some(root.join("history.json"));
    let config = ProviderConfig::from_env().unwrap();
    let model = config.model.clone();
    let adapter = FaultInjectionAdapter {
        inner: ProviderRegistry::adapter_from_config(config),
        inject_failure: false,
        writes: AtomicUsize::new(0),
        saw_failure: AtomicBool::new(false),
        saw_images: AtomicBool::new(false),
        evidence: root.clone(),
    };
    let message = if browser_baseline {
        "请现场制作并交付一张静态中文折线图解释这组示例数据：时间（秒）0、1、2；温度（摄氏度）20、25、30。用浏览器内联 SVG/HTML 绘图，不调用 Python render_plot。图中要有坐标单位、图例和中文标签。完成真实预览与交付；readable_content 写明三个数据点、单位和示例假设。无需书内检索，也无需浏览器滑块。"
    } else {
        "请现场制作并交付一张静态中文折线图解释这组示例数据：时间（秒）0、1、2；温度（摄氏度）20、25、30。请使用 presentation.author 的 render_plot，让 Matplotlib 生成有坐标单位、图例和中文标签的 SVG。看到实际图片后，用返回的 asset_path 放进页面 img，write 时传入 asset_refs，完成真实预览与交付。readable_content 写明三个数据点、单位和示例假设。无需书内检索，也无需浏览器滑块。"
    };
    let request = json!({"message":message});
    let prepared = prepare_agent_chat(&mut state, &request.to_string(), "2026-09-25T00:00:00Z")
        .unwrap_or_else(|r| panic!("prepare: {}", r.body));
    let turn_ref = prepared.turn_ref.clone();
    let app = BorrowedAppPort(std::cell::RefCell::new(&mut state));
    let started = std::time::Instant::now();
    let report =
        crate::agent_run::execute_prepared(&app, &adapter, prepared, CancellationToken::default());
    std::fs::write(root.join("outcome.json"), &report.reply.body).unwrap();
    std::fs::write(root.join("summary.json"), serde_json::to_vec_pretty(&json!({"model":model,"browser_baseline":browser_baseline,"elapsed_ms":started.elapsed().as_millis(),"status":report.reply.status})).unwrap()).unwrap();
    assert_eq!(report.reply.status, 200, "{}", report.reply.body);
    assert!(adapter.saw_images.load(SeqCst));
    let outcome: OuterOutcome = serde_json::from_str(&report.reply.body).unwrap();
    let reference = outcome
        .answer_view
        .unwrap()
        .parts
        .into_iter()
        .find_map(|part| match part {
            AgentAnswerPart::Presentation {
                presentation_id,
                revision,
            } => Some(runtime::presentation::PresentationRef {
                presentation_id,
                revision,
            }),
            _ => None,
        })
        .expect("model did not deliver a presentation");
    app.with_app(|state| {
        let version = state.read_presentation(&turn_ref.session_id, &reference).unwrap();
        if browser_baseline {
            assert!(!version.content.content_files.keys().any(|path| path.ends_with(".py")));
            assert!(version.content.content_files["index.html"].contains("<svg"));
        } else {
            assert!(version.content.content_files.keys().any(|path| path.ends_with(".svg")));
            assert!(version.content.content_files.keys().any(|path| path.ends_with(".py")));
        }
        assert!(version.content.readable_content.contains("摄氏度"));
        let response = crate::presentation_api::route(state, &json!({"session_id":turn_ref.session_id,"turn_id":turn_ref.turn_id,"reference":reference}).to_string(), false);
        assert_eq!(response.status, 200, "{}", response.body);
        std::fs::write(root.join("view.json"), response.body).unwrap();
    });
}

/// A fixed natural-request comparison. ED3_CONDITION=B removes only the new
/// presentation method module from the provider request; production has no toggle.
struct Ed3RecordingAdapter {
    inner: Box<dyn ModelAdapter + Send>,
    baseline: bool,
    evidence: std::path::PathBuf,
    requests: std::sync::atomic::AtomicUsize,
}

impl ModelAdapter for Ed3RecordingAdapter {
    fn model_runtime_profile(&self) -> runtime::ModelRuntimeProfile {
        self.inner.model_runtime_profile()
    }
    fn set_run_cancellation(&self, token: CancellationToken) {
        self.inner.set_run_cancellation(token);
    }
    fn complete(
        &self,
        request: CompletionRequest,
    ) -> Result<ParsedResponse, runtime::AdapterError> {
        self.inner.complete(request)
    }
    fn complete_structured(
        &self,
        request: CompletionRequest,
    ) -> Result<Value, runtime::AdapterError> {
        self.inner.complete_structured(request)
    }
    fn chat(
        &self,
        request: &runtime::AgentRequestPlan,
    ) -> Result<runtime::AssistantTurn, runtime::AdapterError> {
        let mut request = request.clone();
        if self.baseline {
            let marker = "Presentation method (ed3.v1):";
            if let Some(start) = request.instructions.find(marker) {
                let end = request.instructions[start..]
                    .find("\n\n")
                    .map(|offset| start + offset + 2)
                    .unwrap_or(request.instructions.len());
                request.instructions.replace_range(start..end, "");
                request
                    .instruction_assets
                    .retain(|asset| asset.asset_id != "resident-agent.skill.presentation-method");
            }
        }
        let ordinal = self
            .requests
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        for (index, image) in request.preview_images.iter().enumerate() {
            std::fs::write(
                self.evidence
                    .join(format!("image-{ordinal:02}-{index:02}.base64")),
                &image.png_base64,
            )
            .unwrap();
        }
        std::fs::write(self.evidence.join(format!("request-{ordinal:02}.json")), serde_json::to_vec_pretty(&json!({
            "instructions": request.instructions,
            "instruction_assets": request.instruction_assets,
            "messages": request.input,
            "tools": request.tools.iter().map(|tool| json!({"name":tool.name,"description":tool.description,"parameters":tool.parameters})).collect::<Vec<_>>(),
            "images": request.preview_images.iter().map(|image| &image.caption).collect::<Vec<_>>()
        })).unwrap()).unwrap();
        let turn = self.inner.chat(&request)?;
        std::fs::write(
            self.evidence.join(format!("response-{ordinal:02}.json")),
            serde_json::to_vec_pretty(&json!({
                "text": turn.text, "tool_calls": turn.tool_calls, "usage": turn.usage_total_tokens
            }))
            .unwrap(),
        )
        .unwrap();
        Ok(turn)
    }
}

#[test]
#[ignore = "real configured model and browser; writes ED3 B/C evidence"]
fn presentation_method_natural_request_comparison() {
    let condition = std::env::var("ED3_CONDITION").expect("ED3_CONDITION=B or C");
    assert!(matches!(condition.as_str(), "B" | "C"));
    let task = std::env::var("ED3_TASK").expect("ED3_TASK=plain, position or evidence");
    let message = match task.as_str() {
        "plain" => "原文说：‘梯度下降每次沿当前梯度的反方向更新参数。’这句话是什么意思？",
        "position" => "学习率大一点，为什么会来回跳，甚至越来越远？让我能看出来。这里的简化材料是 L(w)=(w-2)^2，更新规则 w_{t+1}=w_t-2η(w_t-2)，初值 w_0=0；只在这个一维模型内解释。",
        "evidence" => "原文说：‘这次调查观察到两组学生的阅读时长不同。’我猜是新教学法造成的。哪些是原文说的，哪些只是我的推测？",
        _ => panic!("unknown ED3_TASK"),
    };
    let root =
        std::path::PathBuf::from(std::env::var("ED3_EVIDENCE_DIR").expect("ED3_EVIDENCE_DIR"));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("input.json"),
        serde_json::to_vec_pretty(&json!({"condition":condition,"task":task,"message":message}))
            .unwrap(),
    )
    .unwrap();
    let mut state = state_named(&format!("ed3-{condition}-{task}"));
    state.history_path = Some(root.join("history.json"));
    let config = ProviderConfig::from_env().unwrap();
    let model = config.model.clone();
    let adapter = Ed3RecordingAdapter {
        inner: ProviderRegistry::adapter_from_config(config),
        baseline: condition == "B",
        evidence: root.clone(),
        requests: Default::default(),
    };
    let prepared = prepare_agent_chat(
        &mut state,
        &json!({"message":message}).to_string(),
        "2026-09-25T00:00:00Z",
    )
    .unwrap_or_else(|reply| panic!("prepare: {}", reply.body));
    let turn_ref = prepared.turn_ref.clone();
    let started = std::time::Instant::now();
    let app = BorrowedAppPort(std::cell::RefCell::new(&mut state));
    let report =
        crate::agent_run::execute_prepared(&app, &adapter, prepared, CancellationToken::default());
    std::fs::write(root.join("outcome.json"), &report.reply.body).unwrap();
    std::fs::write(root.join("summary.json"), serde_json::to_vec_pretty(&json!({
        "condition":condition,"task":task,"model":model,"status":report.reply.status,
        "elapsed_ms":started.elapsed().as_millis(),"model_requests":adapter.requests.load(std::sync::atomic::Ordering::SeqCst)
    })).unwrap()).unwrap();
    if report.reply.status == 200 {
        let outcome: OuterOutcome = serde_json::from_str(&report.reply.body).unwrap();
        if let Some(reference) = outcome.answer_view.and_then(|view| {
            view.parts.into_iter().find_map(|part| match part {
                AgentAnswerPart::Presentation {
                    presentation_id,
                    revision,
                } => Some(runtime::presentation::PresentationRef {
                    presentation_id,
                    revision,
                }),
                _ => None,
            })
        }) {
            app.with_app(|state| {
                let response = crate::presentation_api::route(state, &json!({"session_id":turn_ref.session_id,"turn_id":turn_ref.turn_id,"reference":reference}).to_string(), false);
                std::fs::write(root.join("view.json"), response.body).unwrap();
            });
        }
    }
    assert_eq!(report.reply.status, 200, "{}", report.reply.body);
}

/// EX2 compares the frozen ED3 skill with an experimental skill on the same Runtime.
/// The adapter changes only the loaded instruction body and its revision for C.
struct Ex2SkillAdapter {
    inner: Ed3RecordingAdapter,
    baseline: bool,
    loaded: std::sync::atomic::AtomicBool,
}

impl ModelAdapter for Ex2SkillAdapter {
    fn model_runtime_profile(&self) -> runtime::ModelRuntimeProfile { self.inner.model_runtime_profile() }
    fn set_run_cancellation(&self, token: CancellationToken) { self.inner.set_run_cancellation(token); }
    fn complete(&self, request: CompletionRequest) -> Result<ParsedResponse, runtime::AdapterError> { self.inner.complete(request) }
    fn complete_structured(&self, request: CompletionRequest) -> Result<Value, runtime::AdapterError> { self.inner.complete_structured(request) }
    fn chat(&self, request: &runtime::AgentRequestPlan) -> Result<runtime::AssistantTurn, runtime::AdapterError> {
        use std::sync::atomic::Ordering::SeqCst;
        let mut request = request.clone();
        let marker = "Presentation method (ed3.v1):";
        if request.instructions.contains(marker) {
            self.loaded.store(true, SeqCst);
            if !self.baseline {
                let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
                let current = std::fs::read_to_string(root.join("skills/presentation/SKILL.md")).unwrap();
                let current = current.split_once("\n---\n").unwrap().1.trim();
                let candidate = std::fs::read_to_string(root.join("docs/performance/explorable-explanation-ex2-skill-v7.md")).unwrap();
                let candidate = candidate.split_once("\n---\n").unwrap().1.trim();
                request.instructions = request.instructions.replacen(current, candidate, 1);
                let asset = request.instruction_assets.iter_mut().find(|a| a.asset_id == "resident-agent.skill.presentation-method").unwrap();
                asset.revision = "ex2.v7".into();
            }
        }
        self.inner.chat(&request)
    }
}

#[test]
#[ignore = "real configured model and browser; writes EX2 B/C evidence"]
fn presentation_method_ex2_comparison() {
    use std::sync::atomic::Ordering::SeqCst;
    let condition = std::env::var("EX2_CONDITION").expect("EX2_CONDITION=B or C");
    assert!(matches!(condition.as_str(), "B" | "C"));
    let task = std::env::var("EX2_TASK").expect("EX2_TASK from frozen EX0 input");
    let root = std::path::PathBuf::from(std::env::var("EX2_EVIDENCE_DIR").expect("EX2_EVIDENCE_DIR"));
    std::fs::create_dir_all(&root).unwrap();
    let repo = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let inputs: Value = serde_json::from_str(&std::fs::read_to_string(repo.join("docs/performance/explorable-explanation-ex0-inputs.json")).unwrap()).unwrap();
    let message = inputs["tasks"].as_array().unwrap().iter().find(|item| item["id"] == task).and_then(|item| item["message"].as_str()).expect("frozen task id");
    std::fs::write(root.join("input.json"),serde_json::to_vec_pretty(&json!({"condition":condition,"task":task,"message":message})).unwrap()).unwrap();
    let mut state = state_named(&format!("ex2-{condition}-{task}"));
    state.history_path = Some(root.join("history.json"));
    let config = ProviderConfig::from_env().unwrap();
    let model = config.model.clone();
    let adapter = Ex2SkillAdapter {
        inner: Ed3RecordingAdapter { inner: ProviderRegistry::adapter_from_config(config), baseline:false, evidence:root.clone(), requests:Default::default() },
        baseline:condition == "B",
        loaded:Default::default(),
    };
    let prepared = prepare_agent_chat(&mut state,&json!({"message":message}).to_string(),"2026-09-27T00:00:00Z")
        .unwrap_or_else(|reply| panic!("prepare: {}", reply.body));
    let turn_ref = prepared.turn_ref.clone();
    let started = std::time::Instant::now();
    let app = BorrowedAppPort(std::cell::RefCell::new(&mut state));
    let report = crate::agent_run::execute_prepared(&app,&adapter,prepared,CancellationToken::default());
    std::fs::write(root.join("outcome.json"),&report.reply.body).unwrap();
    std::fs::write(root.join("summary.json"),serde_json::to_vec_pretty(&json!({
        "condition":condition,"task":task,"model":model,"status":report.reply.status,
        "elapsed_ms":started.elapsed().as_millis(),"model_requests":adapter.inner.requests.load(SeqCst),"skill_loaded":adapter.loaded.load(SeqCst)
    })).unwrap()).unwrap();
    if report.reply.status == 200 {
        let outcome: OuterOutcome = serde_json::from_str(&report.reply.body).unwrap();
        if let Some(reference) = outcome.answer_view.and_then(|view| view.parts.into_iter().find_map(|part| match part {
            AgentAnswerPart::Presentation { presentation_id, revision } => Some(runtime::presentation::PresentationRef { presentation_id, revision }),
            _ => None,
        })) {
            app.with_app(|state| {
                let response = crate::presentation_api::route(state,&json!({"session_id":turn_ref.session_id,"turn_id":turn_ref.turn_id,"reference":reference}).to_string(),false);
                std::fs::write(root.join("view.json"),response.body).unwrap();
            });
        }
    }
}

#[test]
#[ignore = "real configured model and browser; writes G6 natural-request evidence"]
fn resident_goal_g6_natural_chapter_request() {
    let scenario = std::env::var("G6_SCENARIO").unwrap_or_else(|_| "chapter".into());
    let root = std::path::PathBuf::from(std::env::var("G6_EVIDENCE_DIR").expect("G6_EVIDENCE_DIR"));
    let book_dir = std::path::PathBuf::from(std::env::var("G6_BOOK_DIR").expect("G6_BOOK_DIR"));
    std::fs::create_dir_all(&root).unwrap();
    let book = Book::load(book_dir.to_str().unwrap()).expect("G6 book must load");
    let mut state = state_named("g6-natural-chapter");
    state.book_dir = book_dir;
    state.book = book.into();
    state.reader = Reader::new(&state.book, DEFAULT_RADIUS);
    state.reader.goto_lid(&state.book, &mut state.store, "1.11", "2026-09-25T00:00:00Z").unwrap();
    state.history_path = Some(root.join("history.json"));
    let config = ProviderConfig::from_env().unwrap();
    let model = config.model.clone();
    let adapter = Ed3RecordingAdapter {
        inner: ProviderRegistry::adapter_from_config(config),
        baseline: false,
        evidence: root.clone(),
        requests: Default::default(),
    };
    let message = match scenario.as_str() {
        "chapter" => "可以把这一章的内容富文本演示给我看吗，我想看一下整体",
        "short" => "原文说‘中心化编排由一个 Manager 统一调度多个子 Agent’，这句话是什么意思？",
        "text_only" => "只用文字概述多 Agent 协作这一节，不需要演示页。",
        _ => panic!("unknown G6_SCENARIO"),
    };
    std::fs::write(root.join("input.json"), serde_json::to_vec_pretty(&json!({
        "scenario":scenario, "message": message, "book_id":state.book.base.book_id, "reader_anchor":"1.11", "model":model
    })).unwrap()).unwrap();
    let prepared = prepare_agent_chat(&mut state, &json!({"message":message}).to_string(), "2026-09-25T00:00:00Z")
        .unwrap_or_else(|reply| panic!("prepare: {}", reply.body));
    let turn_ref = prepared.turn_ref.clone();
    let started = std::time::Instant::now();
    let app = BorrowedAppPort(std::cell::RefCell::new(&mut state));
    let report = crate::agent_run::execute_prepared(&app, &adapter, prepared, CancellationToken::default());
    std::fs::write(root.join("outcome.json"), &report.reply.body).unwrap();
    let (goal, stop_reason) = app.with_app(|state| {
        let session = state.agent_history.sessions.iter().find(|session| session.id == turn_ref.session_id).unwrap();
        let goal = session.goals.iter().find(|goal| goal.origin_turn_id == turn_ref.turn_id).cloned();
        let stop = session.turns.iter().find(|turn| turn.turn_id == turn_ref.turn_id)
            .and_then(|turn| turn.error.as_ref().map(|error| error.error_code.clone()).or_else(|| turn.outcome.as_ref().and_then(|outcome| outcome.warning.clone())));
        (goal, stop)
    });
    std::fs::write(root.join("summary.json"), serde_json::to_vec_pretty(&json!({
        "model":model, "http_status":report.reply.status, "elapsed_ms":started.elapsed().as_millis(),
        "model_requests":adapter.requests.load(std::sync::atomic::Ordering::SeqCst),
        "goal":goal, "stop_reason":stop_reason
    })).unwrap()).unwrap();
    if report.reply.status == 200 {
        let outcome: OuterOutcome = serde_json::from_str(&report.reply.body).unwrap();
        if let Some(reference) = outcome.answer_view.and_then(|view| view.parts.into_iter().find_map(|part| match part {
            AgentAnswerPart::Presentation { presentation_id, revision } => Some(runtime::presentation::PresentationRef { presentation_id, revision }),
            _ => None,
        })) {
            app.with_app(|state| {
                let response = crate::presentation_api::route(state, &json!({"session_id":turn_ref.session_id,"turn_id":turn_ref.turn_id,"reference":reference}).to_string(), false);
                std::fs::write(root.join("view.json"), response.body).unwrap();
            });
        }
    }
}

#[test]
fn presentation_author_requires_preview_and_current_source_bindings() {
    let (_root, mut state, turn) = setup();
    let app = BorrowedAppPort(std::cell::RefCell::new(&mut state));
    let mut port = RuntimeStatePort {
        port: &app,
        turn_ref: &turn,
        previewed: Default::default(),
        animations: Default::default(), plots: Default::default(),
    };
    let id = candidate(&mut port, &working());
    let error = port
        .author_presentation(
            AuthorRequest::Deliver { candidate_id: id },
            &[],
            &[],
            &CancellationToken::default(),
        )
        .err()
        .unwrap();
    assert!(error.message.contains("Preview"));
    let mut request = write(&working());
    if let AuthorRequest::Write { source_ref_ids, .. } = &mut request {
        source_ref_ids.push("invented".into());
    }
    assert_eq!(
        port.author_presentation(request, &[], &[], &CancellationToken::default())
            .err()
            .unwrap()
            .error_code,
        "PRESENTATION_SOURCE_UNKNOWN"
    );
    let cancellation = CancellationToken::default();
    cancellation.cancel();
    assert_eq!(
        port.author_presentation(write(&working()), &[], &[], &cancellation)
            .err()
            .unwrap()
            .error_code,
        "AGENT_RUN_CANCELLED"
    );
}

#[test]
#[ignore = "requires installed Chromium/Edge"]
fn presentation_author_browser_correction_and_private_delivery() {
    let (_root, mut state, turn) = setup();
    let app = BorrowedAppPort(std::cell::RefCell::new(&mut state));
    let mut port = RuntimeStatePort {
        port: &app,
        turn_ref: &turn,
        previewed: Default::default(),
        animations: Default::default(), plots: Default::default(),
    };
    let cancellation = CancellationToken::default();
    let broken = candidate(
        &mut port,
        &format!(
            "{}<script>throw new Error('RP4_BROKEN')</script>",
            working()
        ),
    );
    let failure = port
        .author_presentation(
            AuthorRequest::Preview {
                read_selector: None,
                width: None,
                viewport: None,
                candidate_id: broken.clone(),
                actions: vec![],
            },
            &[],
            &[],
            &cancellation,
        )
        .unwrap();
    assert_eq!(failure.body["status"], "preview_failed");
    assert!(failure.body.to_string().contains("RP4_BROKEN"));
    assert!(
        !failure.images.is_empty(),
        "failed preview screenshots must be available to the model"
    );
    assert!(failure
        .images
        .iter()
        .all(|image| image.caption.contains("preview_failed")
            && image.candidate_id.as_deref() == Some(broken.as_str())));
    assert!(port
        .author_presentation(
            AuthorRequest::Deliver {
                candidate_id: broken
            },
            &[],
            &[],
            &cancellation
        )
        .is_err());
    let fixed = candidate(&mut port, &working());
    let preview = port
        .author_presentation(
            AuthorRequest::Preview {
                read_selector: None,
                width: None,
                viewport: None,
                candidate_id: fixed.clone(),
                actions: vec![PreviewAction::Click {
                    selector: "#add".into(),
                }],
            },
            &[],
            &[],
            &cancellation,
        )
        .unwrap();
    assert_eq!(
        preview.body["status"], "preview_ready_for_inspection",
        "{}",
        preview.body
    );
    assert!(preview.body["observations"][0]["dom"]["text"]
        .as_str()
        .unwrap()
        .contains("0.666666"));
    assert!(preview.body["observations"][1]["dom"]["text"]
        .as_str()
        .unwrap()
        .contains("1"));
    assert_eq!(preview.images.len(), 2);
    assert!(preview
        .images
        .iter()
        .all(|i| i.png_base64.starts_with("iVBOR")));
    let saved = port
        .author_presentation(
            AuthorRequest::Deliver {
                candidate_id: fixed,
            },
            &[],
            &[],
            &cancellation,
        )
        .unwrap()
        .delivered
        .unwrap();
    app.with_app(|state| {
        assert!(state.read_presentation(&turn.session_id, &saved).is_ok());
        let reply = crate::presentation_api::route(
            state,
            &json!({"session_id":turn.session_id,"turn_id":turn.turn_id,"reference":saved})
                .to_string(),
            false,
        );
        assert_ne!(reply.status, 200, "saved is not committed");
    });
}

#[test]
#[ignore = "requires installed Chromium/Edge"]
fn presentation_author_requires_three_explicit_environment_receipts() {
    let (_root, mut state, turn) = setup();
    let app = BorrowedAppPort(std::cell::RefCell::new(&mut state));
    let mut port = RuntimeStatePort {
        port: &app,
        turn_ref: &turn,
        previewed: Default::default(),
        animations: Default::default(), plots: Default::default(),
    };
    let candidate_id = candidate(
        &mut port,
        &format!(
            "<style>button{{min-width:44px;min-height:44px}}</style>{}",
            working()
        ),
    );
    let cancellation = CancellationToken::default();
    for (index, (name, viewport)) in REQUIRED_PREVIEW_ENVIRONMENTS.iter().enumerate() {
        let result = port
            .author_presentation(
                AuthorRequest::Preview {
                read_selector: None,
                    candidate_id: candidate_id.clone(),
                    width: None,
                    viewport: Some(*viewport),
                    actions: vec![PreviewAction::Click {
                        selector: "#add".into(),
                    }],
                },
                &[],
                &[],
                &cancellation,
            )
            .unwrap();
        assert_eq!(result.body["environment_name"], *name);
        assert!(!result.images.is_empty());
        assert!(result
            .images
            .iter()
            .all(
                |image| image.candidate_id.as_deref() == Some(candidate_id.as_str())
                    && image.environment_name.as_deref() == Some(*name)
            ));
        assert_eq!(
            result.body["environment"],
            serde_json::to_value(viewport).unwrap()
        );
        assert_eq!(
            result.body["status"],
            if index + 1 == REQUIRED_PREVIEW_ENVIRONMENTS.len() {
                "preview_ready_for_inspection"
            } else {
                "preview_environment_recorded"
            },
            "{}",
            result.body,
        );
    }
    assert!(port
        .author_presentation(
            AuthorRequest::Deliver { candidate_id },
            &[],
            &[],
            &cancellation,
        )
        .unwrap()
        .delivered
        .is_some());
}

#[test]
#[ignore = "requires installed Chromium/Edge"]
fn presentation_ex1_gold_uses_three_real_preview_environments() {
    let (_root, mut state, turn) = setup();
    let app = BorrowedAppPort(std::cell::RefCell::new(&mut state));
    let mut port = RuntimeStatePort {
        port: &app,
        turn_ref: &turn,
        previewed: Default::default(),
        animations: Default::default(), plots: Default::default(),
    };
    let html = std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/performance/ex1-learning-rate.html"),
    )
    .unwrap();
    let cancellation = CancellationToken::default();
    let written = port
        .author_presentation(
            AuthorRequest::Write {
                libraries: vec![],
                based_on: None,
                state_contract: json!({
                    "eta":"fixed learning rate 0.1..1.1",
                    "semantic_state":"completed iteration 0..5",
                    "transition_progress":"visual interpolation in [0,1)",
                    "reveal_state":"next-step reveal for current iteration",
                    "prediction":"same, cross or null"
                }),
                title: "同样的损失，不同的路".into(),
                html,
                readable_content: "一维模型 L(w)=(w−2)²，从 w₀=0 出发。η=0.2 同侧接近，η=0.8 交替接近，η=1.1 交替远离；过渡中的位置是视觉插值，数值只取真实迭代端点。".into(),
                asset_refs: vec![],
                source_ref_ids: vec![],
                assumptions: vec!["仅适用给定的一维二次模型".into()],
                initial_state: json!({"eta":0.8,"semantic_state":0,"transition_progress":0,"reveal_state":false,"prediction":null}),
            },
            &[],
            &[],
            &cancellation,
        )
        .unwrap();
    let candidate_id = written.body["candidate_id"].as_str().unwrap().to_string();
    for (index, (name, viewport)) in REQUIRED_PREVIEW_ENVIRONMENTS.iter().enumerate() {
        let result = port
            .author_presentation(
                AuthorRequest::Preview {
                read_selector: None,
                    candidate_id: candidate_id.clone(),
                    width: None,
                    viewport: Some(*viewport),
                    actions: vec![
                        PreviewAction::Click { selector: "#next".into() },
                        PreviewAction::Click { selector: "#predict-cross".into() },
                        PreviewAction::Click { selector: "#reveal".into() },
                    ],
                },
                &[],
                &[],
                &cancellation,
            )
            .unwrap();
        assert_eq!(result.body["environment_name"], *name);
        assert_eq!(result.body["observations"].as_array().unwrap().len(), 4);
        assert!(result.body["observations"][3]["dom"]["text"].as_str().unwrap().contains("1.28"));
        assert_eq!(result.body["status"], if index + 1 == REQUIRED_PREVIEW_ENVIRONMENTS.len() {
            "preview_ready_for_inspection"
        } else {
            "preview_environment_recorded"
        }, "{}", result.body);
    }
    assert!(port
        .author_presentation(AuthorRequest::Deliver { candidate_id }, &[], &[], &cancellation)
        .unwrap()
        .delivered
        .is_some());
}

#[test]
#[ignore = "requires installed Chromium/Edge"]
fn presentation_ex6_seeks_and_observes_the_same_frozen_scene() {
    let (_root, mut state, turn) = setup();
    let app = BorrowedAppPort(std::cell::RefCell::new(&mut state));
    let mut port = RuntimeStatePort {
        port: &app,
        turn_ref: &turn,
        previewed: Default::default(),
        animations: Default::default(), plots: Default::default(),
    };
    let html = std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/performance/ex1-learning-rate.html"),
    )
    .unwrap();
    let cancellation = CancellationToken::default();
    let candidate_id = port.author_presentation(
        AuthorRequest::Write {
            libraries: vec![],
            based_on: None,
            state_contract: json!({"eta":"fixed learning rate 0.1..1.1","semantic_state":"completed iteration 0..5","transition_progress":"visual interpolation in [0,1)"}),
            title: "同样的损失，不同的路".into(),
            html,
            readable_content: "一维模型 L(w)=(w−2)² 的真实步数和过渡中间帧。".into(),
            asset_refs: vec![],
            source_ref_ids: vec![],
            assumptions: vec!["仅适用给定的一维二次模型".into()],
            initial_state: json!({"eta":0.8,"semantic_state":0,"transition_progress":0}),
        },
        &[], &[], &cancellation,
    ).unwrap().body["candidate_id"].as_str().unwrap().to_string();
    let seek = |semantic_state, transition_progress| PreviewAction::Seek { semantic_state, transition_progress: json!(transition_progress) };
    let desktop = port.author_presentation(
        AuthorRequest::Preview {
            read_selector: Some("#readout".into()),
            candidate_id: candidate_id.clone(), width: None,
            viewport: Some(REQUIRED_PREVIEW_ENVIRONMENTS[2].1),
            actions: vec![seek(1, 0.5),seek(5, 0.0),seek(0, 0.0),seek(1, 0.5)],
        }, &[], &[], &cancellation,
    ).unwrap();
    let observations = desktop.body["observations"].as_array().unwrap();
    assert_eq!(desktop.body["reading"]["action_step"], 4);
    assert_eq!(desktop.body["reading"]["scene"]["semantic_state"], 1);
    assert_eq!(desktop.body["reading"]["scene"]["transition_progress"], 0.5);
    assert_eq!(desktop.body["reading"]["scene"]["playing"], false);
    assert!(desktop.body["reading"]["text"].as_str().unwrap().contains("真实第 1 步"));
    assert_eq!(observations.len(), 5);
    for index in [1, 4] {
        assert_eq!(observations[index]["scene"]["target"]["semantic_state"], 1);
        assert_eq!(observations[index]["scene"]["target"]["transition_progress"], 0.5);
        assert_eq!(observations[index]["scene"]["actual"], observations[index]["scene"]["after_capture"]);
        assert_eq!(observations[index]["scene"]["actual"]["playing"], false);
        assert!(observations[index]["dom"]["text"].as_str().unwrap().contains("过渡 50%"));
    }
    assert_eq!(observations[2]["scene"]["actual"]["semantic_state"], 5);
    assert_eq!(observations[3]["scene"]["actual"]["semantic_state"], 0);
    let pixels = |image: &runtime::presentation_author::PreviewImage| {
        use base64::Engine as _;
        let png = base64::engine::general_purpose::STANDARD.decode(&image.png_base64).unwrap();
        let mut reader = png::Decoder::new(std::io::Cursor::new(png)).read_info().unwrap();
        let mut pixels = vec![0; reader.output_buffer_size()];
        let frame = reader.next_frame(&mut pixels).unwrap();
        pixels.truncate(frame.buffer_size());
        pixels
    };
    let first = pixels(&desktop.images[1]);
    let repeated = pixels(&desktop.images[4]);
    assert_eq!(first.len(), repeated.len());
    // Chromium varied one antialias pixel across identical captures here; a stale path changes many pixels.
    let changed_bytes = first.iter().zip(&repeated).filter(|(a, b)| a != b).count();
    assert!(changed_bytes <= 24, "repeated middle frame changed {changed_bytes} pixel bytes after rollback");
    assert_eq!(desktop.body["status"], "preview_environment_recorded");
    for (_, viewport) in REQUIRED_PREVIEW_ENVIRONMENTS.iter().take(2) {
        let result = port.author_presentation(
            AuthorRequest::Preview {
                read_selector: None,
                candidate_id: candidate_id.clone(), width: None, viewport: Some(*viewport),
                actions: vec![seek(1, 0.5)],
            }, &[], &[], &cancellation,
        ).unwrap();
        assert_eq!(result.body["observations"][1]["scene"]["actual"]["transition_progress"], 0.5);
        assert!(result.body["errors"].as_array().unwrap().is_empty());
    }
    assert!(port.author_presentation(
        AuthorRequest::Deliver { candidate_id }, &[], &[], &cancellation,
    ).unwrap().delivered.is_some());
}

#[test]
#[ignore = "requires installed Chromium/Edge"]
fn presentation_ex6_reports_page_that_ignores_seek_and_keeps_playing() {
    let (_root, mut state, turn) = setup();
    let app = BorrowedAppPort(std::cell::RefCell::new(&mut state));
    let mut port = RuntimeStatePort {
        port: &app,
        turn_ref: &turn,
        previewed: Default::default(),
        animations: Default::default(), plots: Default::default(),
    };
    let id = candidate(&mut port, r#"<p>场景</p><script>
      window.presentationScene={seek(){},snapshot(){return {semantic_state:0,transition_progress:0,playing:true}}};
    </script>"#);
    let result = port.author_presentation(
        AuthorRequest::Preview {
                read_selector: None,
            candidate_id: id,
            width: None,
            viewport: Some(REQUIRED_PREVIEW_ENVIRONMENTS[2].1),
            actions: vec![PreviewAction::Seek { semantic_state: 1, transition_progress: json!(0.5) }],
        }, &[], &[], &CancellationToken::default(),
    ).unwrap();
    assert_eq!(result.body["status"], "preview_failed");
    let problems = result.body["errors"].as_array().unwrap();
    assert!(problems.iter().any(|problem| problem["kind"] == "scene_position_mismatch"));
    assert!(problems.iter().any(|problem| problem["kind"] == "scene_not_paused"));
    assert_eq!(result.images.len(), 2, "the failed frame remains visible for repair");
}

#[test]
#[ignore = "real configured model and browser; writes EX6 evidence"]
fn presentation_ex6_real_model_receives_middle_frame_and_revises() {
    let root = std::path::PathBuf::from(std::env::var("EX6_EVIDENCE_DIR").expect("EX6_EVIDENCE_DIR"));
    std::fs::create_dir_all(&root).unwrap();
    let message = "请制作一页很小的可交互说明，解释 L(w)=(w-2)^2、w₀=0、wₜ₊₁=wₜ-2η(wₜ-2) 在 η=0.8 时如何跨过最优点。页面只需一条位置轴、一颗沿真实迭代间过渡的点、步数/过渡读数和播放按钮。请在页面提供 window.presentationScene.seek({semantic_state,transition_progress}) 与 snapshot()；seek 停止播放并立即重建该位置，snapshot 返回语义步数、过渡进度和 playing。先写候选，在桌面 preview 用 seek 定位真实第 1 步、过渡 50%，读实际截图和观察值；根据该中途帧修订一次可见标记或文字，再预览新候选的三种必需视口，最后交付。数值只按给定递推计算，不引用书外材料。";
    std::fs::write(root.join("input.json"), serde_json::to_vec_pretty(&json!({"message":message})).unwrap()).unwrap();
    let mut state = state_named("ex6-real-middle-frame");
    state.history_path = Some(root.join("history.json"));
    let config = ProviderConfig::from_env().unwrap();
    let model = config.model.clone();
    let adapter = Ed3RecordingAdapter {
        inner: ProviderRegistry::adapter_from_config(config),
        baseline: false,
        evidence: root.clone(),
        requests: Default::default(),
    };
    let prepared = prepare_agent_chat(&mut state, &json!({"message":message}).to_string(), "2026-09-27T00:00:00Z")
        .unwrap_or_else(|reply| panic!("prepare: {}", reply.body));
    let turn_ref = prepared.turn_ref.clone();
    let started = std::time::Instant::now();
    let app = BorrowedAppPort(std::cell::RefCell::new(&mut state));
    let report = crate::agent_run::execute_prepared(&app, &adapter, prepared, CancellationToken::default());
    std::fs::write(root.join("outcome.json"), &report.reply.body).unwrap();
    std::fs::write(root.join("summary.json"), serde_json::to_vec_pretty(&json!({
        "model":model,"elapsed_ms":started.elapsed().as_millis(),"status":report.reply.status,
        "model_requests":adapter.requests.load(std::sync::atomic::Ordering::SeqCst)
    })).unwrap()).unwrap();
    assert_eq!(report.reply.status, 200, "{}", report.reply.body);
    let outcome: Value = serde_json::from_str(&report.reply.body).unwrap();
    let mut seek_ordinal = None;
    let mut revised = false;
    for ordinal in 0..adapter.requests.load(std::sync::atomic::Ordering::SeqCst) {
        let response: Value = serde_json::from_str(&std::fs::read_to_string(root.join(format!("response-{ordinal:02}.json"))).unwrap()).unwrap();
        for call in response["tool_calls"].as_array().unwrap() {
            if call["name"] != "presentation.author" { continue; }
            let args: Value = serde_json::from_str(call["arguments"].as_str().unwrap()).unwrap();
            if args["operation"] == "preview" && args["actions"].as_array().is_some_and(|actions| actions.iter().any(|action| action["kind"] == "seek" && action["semantic_state"] == 1 && action["transition_progress"] == 0.5)) {
                seek_ordinal = Some(ordinal);
            }
            if args["operation"] == "write" && seek_ordinal.is_some_and(|at| ordinal > at) {
                revised = true;
            }
        }
    }
    assert!(seek_ordinal.is_some(), "model never requested a middle frame");
    assert!(revised, "model did not revise after observing the middle frame");
    let image_received = (0..adapter.requests.load(std::sync::atomic::Ordering::SeqCst)).any(|ordinal| {
        let path = root.join(format!("request-{ordinal:02}.json"));
        let request: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        request["images"].as_array().unwrap().iter().any(|caption| caption.as_str().unwrap().contains("Target scene: semantic step 1, transition 0.500"))
    });
    assert!(image_received, "middle-frame screenshot was not sent to the model");
    let reference = outcome["answer_view"]["parts"].as_array().unwrap().iter()
        .find(|part| part["kind"] == "presentation").expect("model did not deliver a presentation");
    app.with_app(|state| {
        let response = crate::presentation_api::route(state, &json!({"session_id":turn_ref.session_id,"turn_id":turn_ref.turn_id,"reference":{"presentation_id":reference["presentation_id"],"revision":reference["revision"]}}).to_string(), false);
        assert_eq!(response.status, 200, "{}", response.body);
        std::fs::write(root.join("view.json"), response.body).unwrap();
    });
}

#[test]
#[ignore = "real configured model and browser; writes selected-result diagnostic evidence"]
fn selected_result_real_model_reads_and_explains() {
    let root = std::path::PathBuf::from(std::env::var("EX2_READ_EVIDENCE_DIR").expect("EX2_READ_EVIDENCE_DIR"));
    std::fs::create_dir_all(&root).unwrap();
    let message = "请做一页很小的交互说明：L(w)=(w-2)^2，w0=0，w(k+1)=wk-2η(wk-2)。固定 η=1.2，初始显示 k=4，按钮前进一步。页面用程序按递推生成当前步和下一步的读数，均显示六位小数，提供 state reader 返回实际 η 和 k。把这些读数放在同一结果区域，使用 presentation.author preview 的 read_selector，在点击一次之后读取它，再根据工具返回的 reading 解释当前步和下一步为何在最优点两侧，并在最终回答逐项写明 η、k、w。最终文字里的具体数值必须来自该 reading。预览三种所需视口后交付，不引用书外材料。";
    std::fs::write(root.join("input.json"), serde_json::to_vec_pretty(&json!({"message":message,"kind":"directed_capability_diagnostic","runs":1})).unwrap()).unwrap();
    let mut state = state_named("ex2-selected-result");
    state.history_path = Some(root.join("history.json"));
    let config = ProviderConfig::from_env().unwrap();
    let model = config.model.clone();
    let adapter = Ed3RecordingAdapter { inner: ProviderRegistry::adapter_from_config(config), baseline: false, evidence: root.clone(), requests: Default::default() };
    let prepared = prepare_agent_chat(&mut state, &json!({"message":message}).to_string(), "2026-09-27T00:00:00Z")
        .unwrap_or_else(|reply| panic!("prepare: {}", reply.body));
    let turn_ref = prepared.turn_ref.clone();
    let started = std::time::Instant::now();
    let app = BorrowedAppPort(std::cell::RefCell::new(&mut state));
    let report = crate::agent_run::execute_prepared(&app, &adapter, prepared, CancellationToken::default());
    std::fs::write(root.join("outcome.json"), &report.reply.body).unwrap();
    let requests = adapter.requests.load(std::sync::atomic::Ordering::SeqCst);
    std::fs::write(root.join("summary.json"), serde_json::to_vec_pretty(&json!({"model":model,"elapsed_ms":started.elapsed().as_millis(),"status":report.reply.status,"model_requests":requests})).unwrap()).unwrap();
    assert_eq!(report.reply.status, 200, "{}", report.reply.body);
    let outcome: Value = serde_json::from_str(&report.reply.body).unwrap();
    let received = (0..requests).any(|ordinal| {
        let request: Value = serde_json::from_str(&std::fs::read_to_string(root.join(format!("request-{ordinal:02}.json"))).unwrap()).unwrap();
        request["messages"].as_array().unwrap().iter().any(|message| {
            if message["role"] != "Tool" { return false; }
            let body: Value = serde_json::from_str(message["content"].as_str().unwrap_or("")).unwrap_or(Value::Null);
            let reading = &body["model_body"]["reading"];
            body["status"] == "ok" && reading["action_step"] == 1 && reading["page_state"]["values"]["k"] == 5 && reading["text"].as_str().is_some_and(|text| text.contains("12.756480") && text.contains("13.059072"))
        })
    });
    assert!(received, "model did not receive the complete post-click page reading");
    let reference = outcome["answer_view"]["parts"].as_array().unwrap().iter().find(|part| part["kind"] == "presentation").expect("model did not deliver");
    app.with_app(|state| {
        let response = crate::presentation_api::route(state, &json!({"session_id":turn_ref.session_id,"turn_id":turn_ref.turn_id,"reference":{"presentation_id":reference["presentation_id"],"revision":reference["revision"]}}).to_string(), false);
        assert_eq!(response.status, 200);
        std::fs::write(root.join("view.json"), response.body).unwrap();
    });
    let answer = outcome["answer"].as_str().unwrap_or("");
    assert!(answer.contains("12.756480") && answer.contains("13.059072"), "verify final numerical claims: {answer}");
}

#[test]
#[ignore = "requires installed Chromium/Edge"]
fn presentation_author_browser_dynamic_semantics_and_cancel() {
    let (_root, mut state, turn) = setup();
    let app = BorrowedAppPort(std::cell::RefCell::new(&mut state));
    let mut port = RuntimeStatePort {
        port: &app,
        turn_ref: &turn,
        previewed: Default::default(),
        animations: Default::default(), plots: Default::default(),
    };
    let cancellation = CancellationToken::default();
    let id = candidate(
        &mut port,
        "<button id='bad' onclick=\"this.textContent='[[source:invented]]'\">运行</button>",
    );
    let result = port
        .author_presentation(
            AuthorRequest::Preview {
                read_selector: None,
                width: None,
                viewport: None,
                candidate_id: id.clone(),
                actions: vec![PreviewAction::Click {
                    selector: "#bad".into(),
                }],
            },
            &[],
            &[],
            &cancellation,
        )
        .unwrap();
    assert_eq!(result.body["status"], "preview_failed");
    assert!(port
        .author_presentation(
            AuthorRequest::Deliver { candidate_id: id },
            &[],
            &[],
            &cancellation
        )
        .is_err());
    let id = candidate(&mut port, "<h1>停止测试</h1><script>while(true){}</script>");
    let stop = cancellation.clone();
    let stopper = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(700));
        stop.cancel();
    });
    let started = std::time::Instant::now();
    let error = port
        .author_presentation(
            AuthorRequest::Preview {
                read_selector: None,
                width: None,
                viewport: None,
                candidate_id: id,
                actions: vec![],
            },
            &[],
            &[],
            &cancellation,
        )
        .err()
        .unwrap();
    stopper.join().unwrap();
    assert!(
        error.message.contains("AGENT_RUN_CANCELLED"),
        "{}",
        error.message
    );
    assert!(started.elapsed() < std::time::Duration::from_secs(10));
}

/// Real model owns generation, actions, repair and delivery. Only the first generated
/// candidate is deliberately corrupted, after generation and before actual execution.
struct FaultInjectionAdapter {
    inner: Box<dyn ModelAdapter + Send>,
    inject_failure: bool,
    writes: std::sync::atomic::AtomicUsize,
    saw_failure: std::sync::atomic::AtomicBool,
    saw_images: std::sync::atomic::AtomicBool,
    evidence: std::path::PathBuf,
}
impl ModelAdapter for FaultInjectionAdapter {
    fn model_runtime_profile(&self) -> runtime::ModelRuntimeProfile {
        self.inner.model_runtime_profile()
    }
    fn set_run_cancellation(&self, cancellation: CancellationToken) {
        self.inner.set_run_cancellation(cancellation);
    }
    fn complete(&self, req: CompletionRequest) -> Result<ParsedResponse, runtime::AdapterError> {
        self.inner.complete(req)
    }
    fn complete_structured(&self, req: CompletionRequest) -> Result<Value, runtime::AdapterError> {
        self.inner.complete_structured(req)
    }
    fn chat(
        &self,
        request: &runtime::AgentRequestPlan,
    ) -> Result<runtime::AssistantTurn, runtime::AdapterError> {
        use std::sync::atomic::Ordering::SeqCst;
        for message in &request.input {
            if message
                .content
                .as_deref()
                .is_some_and(|s| s.contains("RP4_INJECTED_RUNTIME_FAILURE"))
            {
                self.saw_failure.store(true, SeqCst);
            }
        }
        if !request.preview_images.is_empty() {
            self.saw_images.store(true, SeqCst);
            for (index, image) in request.preview_images.iter().enumerate() {
                std::fs::write(
                    self.evidence.join(format!(
                        "write-{}-step-{index}.base64",
                        self.writes.load(SeqCst)
                    )),
                    &image.png_base64,
                )
                .unwrap();
            }
        }
        let mut turn = self.inner.chat(request)?;
        {
            use std::io::Write;
            let mut log = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(self.evidence.join("calls.jsonl"))
                .unwrap();
            writeln!(log,"{}",json!({"tools":turn.tool_calls,"text":turn.text,"images":request.preview_images.len()})).unwrap();
        }

        for call in &mut turn.tool_calls {
            if call.name != "presentation.author" {
                continue;
            }
            let mut args: Value = serde_json::from_str(&call.arguments).unwrap();
            if self.inject_failure
                && args["operation"] == "write"
                && args["html"].is_string()
                && self.writes.fetch_add(1, SeqCst) == 0
            {
                args["html"] = json!(format!(
                    "{}<script>throw new Error('RP4_INJECTED_RUNTIME_FAILURE')</script>",
                    args["html"].as_str().unwrap()
                ));
                call.arguments = args.to_string();
            }
        }
        Ok(turn)
    }
}

#[test]
#[ignore = "real configured model plus installed browser; writes isolated evidence"]
fn presentation_author_real_model_repairs_and_delivers() {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering::SeqCst};
    let root = std::path::PathBuf::from(
        std::env::var("RP4_EVIDENCE_DIR").expect("set isolated RP4_EVIDENCE_DIR"),
    );
    std::fs::create_dir_all(&root).unwrap();
    let mut state = state_named("rp4-live-model");
    state.history_path = Some(root.join("history.json"));
    let config = ProviderConfig::from_env().unwrap();
    let model = config.model.clone();
    let adapter = FaultInjectionAdapter {
        inner: ProviderRegistry::adapter_from_config(config),
        inject_failure: true,
        writes: AtomicUsize::new(0),
        saw_failure: AtomicBool::new(false),
        saw_images: AtomicBool::new(false),
        evidence: root.clone(),
    };
    let request = json!({"message":"请现场制作一个中文交互页面，帮助我比较找到证据与忠实使用证据。采用我给定的实验假设：共需三处证据，起初找到两处，召回率2/3；加入无关材料不改变召回率；补齐第三处后为1。用两张并排卡片说明区别，并提供‘加入无关材料’和‘补齐证据’两个按钮、联动数值和图形。所有规则是本问题提供的假设，不需要书内检索。请实际操作两个按钮，检查运行错误及截图；发现错误则修改后重跑，完成后交付可交互内容。"});
    let prepared = prepare_agent_chat(&mut state, &request.to_string(), "2026-09-17T00:00:00Z")
        .unwrap_or_else(|r| panic!("prepare: {}", r.body));
    let turn_ref = prepared.turn_ref.clone();
    let app = BorrowedAppPort(std::cell::RefCell::new(&mut state));
    let started = std::time::Instant::now();
    let report =
        crate::agent_run::execute_prepared(&app, &adapter, prepared, CancellationToken::default());
    std::fs::write(root.join("outcome.json"), &report.reply.body).unwrap();
    std::fs::write(root.join("summary.json"),serde_json::to_vec_pretty(&json!({"model":model,"elapsed_ms":started.elapsed().as_millis(),"writes":adapter.writes.load(SeqCst),"saw_failure":adapter.saw_failure.load(SeqCst),"saw_images":adapter.saw_images.load(SeqCst),"http_status":report.reply.status})).unwrap()).unwrap();
    assert_eq!(report.reply.status, 200, "{}", report.reply.body);
    assert!(adapter.writes.load(SeqCst) >= 2, "model did not rewrite");
    assert!(adapter.saw_failure.load(SeqCst));
    assert!(adapter.saw_images.load(SeqCst));
    let outcome: OuterOutcome = serde_json::from_str(&report.reply.body).unwrap();
    let reference = outcome
        .answer_view
        .unwrap()
        .parts
        .into_iter()
        .find_map(|p| match p {
            AgentAnswerPart::Presentation {
                presentation_id,
                revision,
            } => Some(runtime::presentation::PresentationRef {
                presentation_id,
                revision,
            }),
            _ => None,
        })
        .expect("no delivered presentation");
    app.with_app(|state| {
        let version = state.read_presentation(&turn_ref.session_id,&reference).unwrap();
        assert!(!version.content.content_files[&version.content.entrypoint].contains("RP4_INJECTED_RUNTIME_FAILURE"));
        let response = crate::presentation_api::route(state,&json!({"session_id":turn_ref.session_id,"turn_id":turn_ref.turn_id,"reference":reference}).to_string(),false);
        assert_eq!(response.status,200,"{}",response.body);
        std::fs::write(root.join("view.json"),&response.body).unwrap();
    });
    let history = std::fs::read_to_string(root.join("history.json")).unwrap();
    assert!(!history.contains("<script>"));
    assert!(!history.contains("iVBOR"));
}

#[test]
#[ignore = "serves the already completed real model run for a bounded browser mount check"]
fn presentation_author_mount_host() {
    let root = std::path::PathBuf::from(std::env::var("RP4_EVIDENCE_DIR").unwrap());
    let mut state = state_named("rp4-mount");
    state.history_path = Some(root.join("history.json"));
    state.agent_history = load_agent_history(&state.history_path).unwrap();
    let session = state.agent_history.sessions.last().unwrap();
    let turn = session.turns.last().unwrap();
    let fixture = json!({"session_id":session.id,"turn_id":turn.turn_id,"outcome":turn.outcome});
    let server = tiny_http::Server::http("127.0.0.1:4175").unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(300);
    println!("RP4_MOUNT_READY");
    while std::time::Instant::now() < deadline {
        let Some(mut request) = server.recv_timeout(Duration::from_secs(1)).unwrap() else {
            continue;
        };
        let path = request.url().to_string();
        if path == "/stop" {
            request
                .respond(tiny_http::Response::from_string("stopped"))
                .unwrap();
            break;
        }
        let reply = if path == "/fixture" {
            ok_json(&fixture)
        } else {
            let mut body = String::new();
            request.as_reader().read_to_string(&mut body).unwrap();
            post(&mut state, &path, &body)
        };
        request
            .respond(
                tiny_http::Response::from_string(reply.body)
                    .with_status_code(reply.status)
                    .with_header(
                        tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap(),
                    ),
            )
            .unwrap();
    }
}
