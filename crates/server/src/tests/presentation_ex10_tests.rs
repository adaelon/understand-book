//! EX10 only: fixed resource assembly before the existing author write contract.
use super::*;
use std::sync::atomic::Ordering::SeqCst;

const MARKER: &str = "<!--EX10_KONVA_10.7.0-->";
fn evidence() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/performance/ex10-svg-konva")
}
fn assemble(html: &str, konva: bool) -> String {
    if !konva { return html.to_owned(); }
    let library = std::fs::read_to_string(evidence().join("vendor/konva-10.7.0.min.js")).unwrap();
    html.replace(MARKER, &format!("<script>{library}</script>"))
}
fn save_json(path: impl AsRef<std::path::Path>, value: &Value) {
    std::fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

#[test]
fn ex10_assembly_preserves_scene_and_pins_only_fixed_resource() {
    let raw = format!("{MARKER}<script>scene()</script>");
    assert_eq!(assemble(&raw, false), raw);
    let assembled = assemble(&raw, true);
    assert!(!assembled.contains(MARKER));
    assert!(assembled.ends_with("</script><script>scene()</script>"));
    assert!(assembled.len() < 1024 * 1024);
    assert_eq!(assemble("<script src='other.js'></script>", true), "<script src='other.js'></script>");
}

#[test]
#[ignore = "EX10 fixed Konva through author, three real previews and persisted version"]
fn ex10_preflight() {
    let (_temp, mut state, turn) = setup();
    let root = evidence().join("preflight");
    let app = BorrowedAppPort(std::cell::RefCell::new(&mut state));
    let scope = app.with_app(|state| crate::run_scope::RunScope::capture(state, &turn, "test", None, None));
    let mut port = RuntimeStatePort { scope: &scope, port: &app, turn_ref: &turn, previewed: Default::default(), animations: Default::default(), plots: Default::default() };
    let raw = std::fs::read_to_string(evidence().join("preflight-scene.html")).unwrap();
    let html = assemble(&raw, true);
    std::fs::write(root.join("assembled.html"), &html).unwrap();
    let cancellation = CancellationToken::default();
    let written = port.author_presentation(AuthorRequest::Write {
        new_object: false,
        libraries: vec![],
        based_on: None, title: "拖动点与线段".into(), html,
        readable_content: "拖动蓝点，线段和位置读数同步变化。".into(),
        state_contract: json!({"x":"point horizontal coordinate in scene units"}),
        initial_state: json!({"x":80}), asset_refs:vec![], source_ref_ids:vec![], assumptions:vec![],
    }, &[], &[], &cancellation).unwrap();
    save_json(root.join("write.json"), &written.body);
    let candidate_id = written.body["candidate_id"].as_str().unwrap().to_owned();
    for (name, viewport) in REQUIRED_PREVIEW_ENVIRONMENTS {
        let result = port.author_presentation(AuthorRequest::Preview {
            candidate_id:candidate_id.clone(), width:None, viewport:Some(viewport),
            read_selector:Some("#readout".into()), actions:vec![],
        }, &[], &[], &cancellation).unwrap();
        save_json(root.join(format!("preview-{name}.json")), &result.body);
        assert!(matches!(result.body["status"].as_str(),Some("preview_ready_for_inspection" | "preview_environment_recorded")), "{}",result.body);
    }
    let result = port.author_presentation(AuthorRequest::Deliver {candidate_id}, &[], &[], &cancellation).unwrap();
    save_json(root.join("deliver.json"), &result.body);
    let reference = result.delivered.unwrap();
    app.with_app(|state| {
        let outcome = OuterOutcome {
            answer:Some("拖动点与线段".into()),answer_view:Some(AgentAnswerView {parts:vec![AgentAnswerPart::Presentation {presentation_id:reference.presentation_id.clone(),revision:reference.revision}],sources:vec![]}),
            incomplete:false,warning:None,turns:1,tokens_spent:0,effects:vec![],trace:vec![],profile_usage:Default::default(),memory_updates:vec![],source_bindings:vec![],delivery_diagnostics:None,request_audit:Default::default(),
        };
        finalize_agent_turn_completed(state,&turn,&outcome,&state.workspace.messages.clone(),"2026-09-27T15:00:00Z").unwrap();
        let version = state.private_context().read_presentation(&turn.session_id,&reference).unwrap();
        save_json(root.join("content.json"), &serde_json::to_value(version.content).unwrap());
        let response = crate::presentation_api::route(&state.private_context(),&json!({"session_id":turn.session_id,"turn_id":turn.turn_id,"reference":reference}).to_string(),false);
        assert_eq!(response.status,200,"{}",response.body);
        std::fs::write(root.join("view.json"),response.body).unwrap();
    });
}

struct AssemblyRun {
    root: std::path::PathBuf,
    konva: bool,
    writes: usize,
    started: std::time::Instant,
}
thread_local! {
    static ASSEMBLY_RUN: std::cell::RefCell<Option<AssemblyRun>> = const { std::cell::RefCell::new(None) };
}

// Executed inside Author Write; never changes the model's returned tool call/history.
pub(crate) fn assemble_write(raw: &str) -> String {
    ASSEMBLY_RUN.with(|slot| {
        let mut slot = slot.borrow_mut();
        let Some(run) = slot.as_mut() else { return raw.to_owned(); };
        let index = run.writes;
        run.writes += 1;
        let before = std::time::Instant::now();
        let html = assemble(raw, run.konva);
        let assembly_us = before.elapsed().as_micros();
        std::fs::write(run.root.join(format!("write-{index:02}-raw.html")),raw).unwrap();
        std::fs::write(run.root.join(format!("write-{index:02}-assembled.html")),&html).unwrap();
        save_json(run.root.join(format!("write-{index:02}-assembly.json")),&json!({
            "raw_bytes":raw.len(),"assembled_bytes":html.len(),"assembly_us":assembly_us,
            "elapsed_ms":run.started.elapsed().as_millis(),"marker_count":raw.matches(MARKER).count()
        }));
        html
    })
}

#[test]
fn ex10_write_assembles_candidate_without_expanding_request() {
    let (temp, mut state, turn) = setup();
    let raw = format!("{MARKER}{}",working());
    assert!(raw.contains(MARKER));
    struct FixedWrite(String);
    impl ModelAdapter for FixedWrite {
        fn complete(&self, _: CompletionRequest) -> Result<ParsedResponse,runtime::AdapterError> { unreachable!() }
        fn chat(&self, _: &runtime::AgentRequestPlan) -> Result<runtime::AssistantTurn,runtime::AdapterError> {
            Ok(runtime::AssistantTurn { provider_continuation: None, text:None,usage_total_tokens:Some(0),tool_calls:vec![runtime::ToolCall {
                id:"write-1".into(),name:"presentation.author".into(),arguments:self.0.clone(),
            }] })
        }
    }
    let arguments = json!({"operation":"write","title":"实验","html":raw,"readable_content":"实验",
        "state_contract":{},"initial_state":{},"asset_refs":[],"source_ref_ids":[],"assumptions":[]}).to_string();
    let adapter = Ex10Adapter {
        inner:Ed3RecordingAdapter {inner:Box::new(FixedWrite(arguments.clone())),baseline:false,evidence:temp.path().to_owned(),requests:Default::default()},
        konva:true,started:std::time::Instant::now(),
    };
    let plan = runtime::AgentRequestPlan::for_agent_turn(adapter.model_runtime_profile(),&[],&[]);
    let returned = adapter.chat(&plan).unwrap();
    assert_eq!(returned.tool_calls[0].arguments,arguments);
    let request: AuthorRequest = serde_json::from_str(&returned.tool_calls[0].arguments).unwrap();
    ASSEMBLY_RUN.with(|slot| *slot.borrow_mut() = Some(AssemblyRun {
        root:temp.path().to_owned(),konva:true,writes:0,started:std::time::Instant::now(),
    }));
    let app = BorrowedAppPort(std::cell::RefCell::new(&mut state));
    let scope = app.with_app(|state| crate::run_scope::RunScope::capture(state, &turn, "test", None, None));
    let mut port = RuntimeStatePort { scope: &scope, port:&app,turn_ref:&turn,previewed:Default::default(),animations:Default::default(),plots:Default::default() };
    let result = port.author_presentation(request, &[], &[], &CancellationToken::default()).unwrap();
    ASSEMBLY_RUN.with(|slot| assert_eq!(slot.borrow_mut().take().unwrap().writes,1));
    assert_eq!(returned.tool_calls[0].arguments,arguments);
    app.with_app(|state| {
        let candidate = state.private_context().read_presentation_candidate(&turn.session_id,result.body["candidate_id"].as_str().unwrap()).unwrap();
        assert_eq!(candidate.content.content_files["index.html"],assemble(&raw,true));
    });
    assert_eq!(assemble_write(&raw),raw);
}

struct Ex10Adapter {
    inner: Ed3RecordingAdapter,
    konva: bool,
    started: std::time::Instant,
}
impl ModelAdapter for Ex10Adapter {
    fn model_runtime_profile(&self) -> runtime::ModelRuntimeProfile { self.inner.model_runtime_profile() }
    fn set_run_cancellation(&self, token: CancellationToken) { self.inner.set_run_cancellation(token); }
    fn complete(&self, request: CompletionRequest) -> Result<ParsedResponse,runtime::AdapterError> { self.inner.complete(request) }
    fn complete_structured(&self, request: CompletionRequest) -> Result<Value,runtime::AdapterError> { self.inner.complete_structured(request) }
    fn chat(&self, request: &runtime::AgentRequestPlan) -> Result<runtime::AssistantTurn,runtime::AdapterError> {
        let mut request = request.clone();
        for name in ["common.md", if self.konva {"C-api.md"} else {"B-api.md"}] {
            request.instructions.push_str("\n\n");
            request.instructions.push_str(&std::fs::read_to_string(evidence().join(name)).unwrap());
        }
        let ordinal = self.inner.requests.load(SeqCst);
        save_json(self.inner.evidence.join(format!("timing-{ordinal:02}-start.json")), &json!({
            "elapsed_ms":self.started.elapsed().as_millis(),"output_token_limit":request.output_token_limit,
            "runtime_profile":request.runtime_profile,
        }));
        let completion = self.inner.chat(&request);
        save_json(self.inner.evidence.join(format!("timing-{ordinal:02}-end.json")), &json!({
            "elapsed_ms":self.started.elapsed().as_millis(),"error":completion.as_ref().err().map(|error|format!("{error:?}")),
        }));
        completion
    }
}

#[test]
#[ignore = "EX10 one independent Resident run; caller enforces alternating order and stop gate"]
fn ex10_agent_comparison() {
    let condition = std::env::var("EX10_CONDITION").expect("B or C");
    assert!(matches!(condition.as_str(),"B"|"C"));
    let root = std::path::PathBuf::from(std::env::var("EX10_RUN_DIR").expect("EX10_RUN_DIR"));
    // Never overwrite an original sample.
    std::fs::create_dir(&root).unwrap();
    let input: Value = serde_json::from_str(&std::fs::read_to_string(evidence().join("input.json")).unwrap()).unwrap();
    save_json(root.join("input.json"), &input);
    let message = input["message"].as_str().unwrap();
    let mut state = state_named(&format!("ex10-{condition}"));
    state.user.history_path = Some(root.join("history.json"));
    let config = ProviderConfig::from_env().unwrap();
    let model = config.model.clone();
    let started = std::time::Instant::now();
    let adapter = Ex10Adapter {
        inner:Ed3RecordingAdapter {inner:ProviderRegistry::adapter_from_config(config),baseline:false,evidence:root.clone(),requests:Default::default()},
        konva:condition=="C",started,
    };
    let prepared = prepare_agent_chat(&mut state,&json!({"message":message}).to_string(),"2026-09-27T15:00:00Z")
        .unwrap_or_else(|reply|panic!("prepare: {}",reply.body));
    let turn_ref = prepared.turn_ref.clone();
    let app = BorrowedAppPort(std::cell::RefCell::new(&mut state));
    ASSEMBLY_RUN.with(|slot| *slot.borrow_mut() = Some(AssemblyRun {
        root:root.clone(),konva:condition=="C",writes:0,started,
    }));
    let report = crate::agent_run::execute_prepared(&app,&adapter,prepared,CancellationToken::default());
    let writes = ASSEMBLY_RUN.with(|slot| slot.borrow_mut().take().unwrap().writes);
    std::fs::write(root.join("outcome.json"),&report.reply.body).unwrap();
    save_json(root.join("summary.json"),&json!({"condition":condition,"model":model,"status":report.reply.status,
        "elapsed_ms":started.elapsed().as_millis(),"model_requests":adapter.inner.requests.load(SeqCst),"writes":writes}));
    if report.reply.status == 200 {
        let outcome: OuterOutcome = serde_json::from_str(&report.reply.body).unwrap();
        if let Some(reference) = outcome.answer_view.and_then(|view|view.parts.into_iter().find_map(|part|match part {
            AgentAnswerPart::Presentation {presentation_id,revision} => Some(runtime::presentation::PresentationRef {presentation_id,revision}), _=>None,
        })) {
            app.with_app(|state| {
                let version = state.private_context().read_presentation(&turn_ref.session_id,&reference).unwrap();
                save_json(root.join("content.json"), &serde_json::to_value(version.content).unwrap());
                let response = crate::presentation_api::route(&state.private_context(),&json!({"session_id":turn_ref.session_id,"turn_id":turn_ref.turn_id,"reference":reference}).to_string(),false);
                std::fs::write(root.join("view.json"),response.body).unwrap();
            });
        }
    }
}
