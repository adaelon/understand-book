//! EX12.4 same-foundation live experiment. No production configuration switches.
use super::*;
#[path = "presentation_ex13_live_tests.rs"]
mod ex13_live;
use base64::Engine;
use runtime::agent_prompt::{PresentationGuidance, PresentationNeed, PresentationPhase};
use runtime::provider_stream::{ModelDelta, ModelObserver};
use std::{path::{Path, PathBuf}, sync::atomic::{AtomicUsize, Ordering::SeqCst}, time::Instant};

fn repo() -> PathBuf { PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..") }
fn evidence() -> PathBuf { repo().join("docs/performance/presentation-staged-authoring-ex12-4") }
fn read(path: impl AsRef<Path>) -> String { std::fs::read_to_string(path).unwrap() }
fn json_file(path: impl AsRef<Path>) -> Value { serde_json::from_str(&read(path)).unwrap() }
fn save(path: impl AsRef<Path>, value: &Value) {
    std::fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}
fn is_presentation(id: &str) -> bool {
    id.starts_with("resident-agent.presentation.") || matches!(id,
        "resident-agent.policy.presentation-authoring" | "resident-agent.skill.presentation-method")
}

fn experiment_request(request: &runtime::AgentRequestPlan, baseline: bool) -> runtime::AgentRequestPlan {
    let mut result = request.clone();
    if !result.tools.iter().any(|t| t.name == "presentation.author") { return result; }
    let scroll = read(evidence().join("shared-scroll.txt"));
    if baseline {
        // Remove only the currently injected presentation modules, preserving all
        // other policies, current task data, provider continuation and real tools.
        let needs = vec![PresentationNeed::Editing, PresentationNeed::StaticPlot,
            PresentationNeed::State, PresentationNeed::ContinuousScene,
            PresentationNeed::Konva, PresentationNeed::Manim];
        let mut texts = std::collections::BTreeSet::new();
        for phase in [PresentationPhase::Global, PresentationPhase::Local, PresentationPhase::Review] {
            for module in runtime::agent_prompt::policy_modules_for_tools_with_presentation(
                &request.tools, Some(&PresentationGuidance { phase, needs: needs.clone() })) {
                if is_presentation(&module.asset_id) { texts.insert(module.text); }
            }
        }
        for text in texts { result.instructions = result.instructions.replace(text.trim(), ""); }
        result.instruction_assets.retain(|a| !is_presentation(&a.asset_id));
        let skill = read(evidence().join("../presentation-staged-authoring-baseline/guidance-baseline/SKILL.md"));
        let skill = skill.split_once("\n---\n").or_else(|| skill.split_once("\r\n---\r\n")).unwrap().1.trim();
        for (id, text) in [("resident-agent.policy.presentation-authoring", read(evidence().join("baseline-policy.txt"))),
            ("resident-agent.skill.presentation-method", skill.into())] {
            result.instructions.push_str(&format!("\n\n{text}"));
            result.instruction_assets.push(runtime::InstructionAssetRef { asset_id: id.into(), revision: "ex11.v11".into() });
        }
    } else {
        // The same observation text is appended once in each arm.
        result.instructions = result.instructions.replace(&scroll, "");
    }
    result.instructions.push_str(&format!("\n\n{scroll}"));
    result.instruction_assets.push(runtime::InstructionAssetRef {
        asset_id: "experiment.ex12.shared-scroll".into(), revision: "ex12.v3".into() });
    result
}

struct RecordingAdapter {
    inner: Box<dyn ModelAdapter + Send>, baseline: bool, root: PathBuf,
    profile: runtime::ModelRuntimeProfile,
    chats: AtomicUsize, completions: AtomicUsize, started: Instant,
}
impl ModelAdapter for RecordingAdapter {
    fn model_runtime_profile(&self) -> runtime::ModelRuntimeProfile { self.profile.clone() }
    fn set_run_cancellation(&self, token: CancellationToken) { self.inner.set_run_cancellation(token); }
    fn complete(&self, req: CompletionRequest) -> Result<ParsedResponse, runtime::AdapterError> {
        self.inner.complete(req)
    }
    fn complete_structured(&self, req: CompletionRequest) -> Result<Value, runtime::AdapterError> {
        self.complete_structured_observed(req, &mut runtime::provider_stream::ignore)
    }
    fn complete_structured_observed(&self, req: CompletionRequest, observer: &mut dyn ModelObserver) -> Result<Value, runtime::AdapterError> {
        let n = self.completions.fetch_add(1, SeqCst);
        save(self.root.join(format!("completion-{n:02}-request.json")), &json!({"system":req.system,"user":req.user,"output_token_limit":req.output_token_limit,"reasoning_effort":req.reasoning_effort}));
        let start = Instant::now(); let mut usage = None;
        let result = self.inner.complete_structured_observed(req, &mut |delta: ModelDelta| {
            if let ModelDelta::Usage(value) = &delta { usage = Some(value.clone()); }
            observer.observe(delta);
        });
        save(self.root.join(format!("completion-{n:02}-response.json")), &json!({"response":result.as_ref().ok(),"error":result.as_ref().err().map(|e|&e.message),"usage":usage,"duration_ms":start.elapsed().as_millis()}));
        result
    }
    fn chat(&self, req: &runtime::AgentRequestPlan) -> Result<runtime::AssistantTurn, runtime::AdapterError> {
        self.chat_observed(req, &mut runtime::provider_stream::ignore)
    }
    fn chat_observed(&self, req: &runtime::AgentRequestPlan, observer: &mut dyn ModelObserver) -> Result<runtime::AssistantTurn, runtime::AdapterError> {
        let req = experiment_request(req, self.baseline);
        let n = self.chats.fetch_add(1, SeqCst);
        let mut images = Vec::new();
        for (i, image) in req.preview_images.iter().enumerate() {
            let name = format!("image-{n:02}-{i:02}.png");
            std::fs::write(self.root.join(&name), base64::engine::general_purpose::STANDARD.decode(&image.png_base64).unwrap()).unwrap();
            images.push(json!({"file":name,"caption":image.caption,"candidate_id":image.candidate_id,"environment":image.environment_name}));
        }
        save(self.root.join(format!("request-{n:02}.json")), &json!({
            "instructions":req.instructions,"instruction_assets":req.instruction_assets,"messages":req.input,
            "tools":req.tools.iter().map(|t|json!({"name":t.name,"description":t.description,"parameters":t.parameters})).collect::<Vec<_>>(),
            "images":images,"runtime_profile":req.runtime_profile,"output_token_limit":req.output_token_limit,
            "elapsed_ms":self.started.elapsed().as_millis()}));
        let start = Instant::now(); let mut usage = None;
        let result = self.inner.chat_observed(&req, &mut |delta: ModelDelta| {
            if let ModelDelta::Usage(value) = &delta { usage = Some(value.clone()); }
            observer.observe(delta);
        });
        save(self.root.join(format!("response-{n:02}.json")), &json!({
            "text":result.as_ref().ok().and_then(|t|t.text.as_ref()),
            "tool_calls":result.as_ref().ok().map(|t|&t.tool_calls),"usage":usage,
            "error":result.as_ref().err().map(|e|&e.message),"duration_ms":start.elapsed().as_millis()}));
        result
    }
}

fn setup_run(root: &Path, task: &str) -> AppState {
    let mut state = state_named(&format!("ex12-{task}-{}",root.file_name().unwrap().to_string_lossy()));
    // Memory's learning sidecar is parent-scoped; give each arm its own parent.
    state.user.store = MemoryStore::open(root.join("memory.json")).unwrap();
    let book_dir = repo().join("tmp/ex12-4/publication");
    state.workspace.book = Book::load(book_dir.to_str().unwrap()).unwrap().into();
    state.workspace.book_dir = book_dir;
    state.workspace.reader = Reader::new(&state.workspace.book, 20);
    state.workspace.reader.restore_top_lid(&state.workspace.book, "3.6.1");
    state.workspace.reader.select_annotation("3.6");
    state.user.history_path = Some(root.join("history.json"));
    if task == "mechanism" {
        std::fs::copy(evidence().join("history-before.json"), root.join("history.json")).unwrap();
        state.user.agent_history = load_agent_history(&state.user.history_path).unwrap();
        let session = &state.user.agent_history.sessions[0];
        state.workspace.messages = session.messages.clone();
        state.workspace.selected_chat = Some(session.id.clone());
        let version = json_file(evidence().join("../presentation-staged-authoring-baseline/previous-version-1.json"));
        let target = root.join("history.presentations/versions").join(version["reference"]["presentation_id"].as_str().unwrap());
        std::fs::create_dir_all(&target).unwrap();
        save(target.join("1.json"), &version);
    }
    state
}

#[test]
fn ex12_comparison_changes_only_guidance_and_keeps_shared_tools() {
    let tool = spec();
    for phase in [PresentationPhase::Global, PresentationPhase::Local, PresentationPhase::Review] {
        let modules = runtime::agent_prompt::policy_modules_for_tools_with_presentation(&[tool.clone()],
            Some(&PresentationGuidance { phase, needs:vec![PresentationNeed::Manim,PresentationNeed::Konva] }));
        let request = runtime::AgentRequestPlan::for_agent_turn_with_modules(
            runtime::ModelRuntimeCatalog::default().resolve("deepseek-v4-flash",runtime::ProviderToolProtocol::Native,None),
            &[Message::user("unchanged task")], &[tool.clone()], &modules);
        let old = experiment_request(&request, true);
        let new = experiment_request(&request, false);
        assert!(old.instructions.contains("Presentation method (ex11.v11)"));
        assert!(!old.instructions.contains("Presentation phase:"));
        assert!(!old.instructions.contains("Presentation engineering contract:"));
        assert!(new.instructions.contains("Presentation phase:"));
        assert_eq!(old.tools[0].parameters, new.tools[0].parameters);
        assert_eq!(old.tools[0].description, new.tools[0].description);
        assert_eq!(serde_json::to_value(&old.input).unwrap(),serde_json::to_value(&new.input).unwrap());
        for plan in [&old, &new] { assert_eq!(plan.instructions.matches("Long pages: preview actions").count(), 1); }
        assert_eq!(old.runtime_profile,new.runtime_profile);
        assert_eq!(old.output_token_limit,new.output_token_limit);
    }
}

#[test]
fn ex12_archived_profile_keeps_catalog_authoring_and_compaction_budgets() {
    let profile: runtime::ModelRuntimeProfile = serde_json::from_value(json_file(evidence().join("provider-profile.json"))).unwrap();
    assert_eq!(serde_json::to_value(&profile.resolution).unwrap(), json!("CatalogMatch"));
    let plan = runtime::AgentRequestPlan::for_agent_turn(profile.clone(), &[Message::user("original request")], &[spec()]);
    assert_eq!(plan.output_token_limit, Some(131_072));
    assert_eq!(runtime::compaction::compaction_output_token_limit(&profile), 65_536);
}

#[test]
#[ignore = "EX12 archived publication and version fixtures"]
fn ex12_original_history_and_version_are_readable() {
    let root = tempfile::tempdir().unwrap();
    let mut state = setup_run(root.path(), "mechanism");
    let session = &state.user.agent_history.sessions[0];
    assert_eq!(session.turns.len(),1);
    assert_eq!(session.messages.len(),53);
    let reference = runtime::presentation::PresentationRef {presentation_id:"presentation-1790851114402727998-8".into(),revision:1};
    let version = state.private_context().read_presentation(&session.id,&reference).unwrap();
    assert!(version.content.content_files.contains_key("index.html"));
    assert_eq!(state.workspace.reader.viewport().top_lid,"3.6.1");
    let input = json_file(evidence().join("inputs.json"))["mechanism"].clone();
    prepare_agent_chat(&mut state,&input.to_string(),"2026-10-02T05:00:00Z")
        .unwrap_or_else(|r|panic!("prepare: {}",r.body));
}

#[test]
#[ignore = "EX12.4 real native model, same inputs and production browser"]
fn ex12_live_comparison() {
    let task = std::env::var("EX12_TASK").unwrap();
    let arm = std::env::var("EX12_ARM").unwrap();
    assert!(matches!(arm.as_str(),"baseline"|"staged"));
    let root = PathBuf::from(std::env::var("EX12_RUN_DIR").unwrap());
    std::fs::create_dir(&root).unwrap();
    let follow_up = std::env::var("EX12_FOLLOW_UP_INPUT").ok();
    let input = follow_up.as_ref().map(json_file)
        .unwrap_or_else(|| json_file(evidence().join("inputs.json"))[&task].clone());
    assert!(input.is_object()); save(root.join("input.json"),&input);
    let mut state = setup_run(&root,&task);
    if follow_up.is_some() {
        // A correction is a separate retained run, never an overwrite or a new baseline sample.
        let base = PathBuf::from(std::env::var("EX12_CONTINUE_FROM").unwrap());
        std::fs::copy(base.join("history.json"), root.join("history.json")).unwrap();
        state.user.agent_history = load_agent_history(&state.user.history_path).unwrap();
        let session = state.user.agent_history.sessions.last().unwrap();
        state.workspace.messages = session.messages.clone();
        state.workspace.selected_chat = Some(session.id.clone());
        let version = json_file(base.join("version.json"));
        let reference = &version["reference"];
        let target = root.join("history.presentations/versions").join(reference["presentation_id"].as_str().unwrap());
        std::fs::create_dir_all(&target).unwrap();
        save(target.join(format!("{}.json",reference["revision"])), &version);
        save(root.join("continuation.json"), &json!({"base_run":base,"reference":reference,"kind":"observed-feedback"}));
    }
    let mut config = ProviderConfig::from_env().unwrap();
    config.mode = runtime::ProviderMode::Native;
    config.model = "deepseek-v4-flash".into();
    config.base_url = "https://api.deepseek.com".into();
    let profile: runtime::ModelRuntimeProfile = serde_json::from_value(json_file(evidence().join("provider-profile.json"))).unwrap();
    let adapter = RecordingAdapter {inner:ProviderRegistry::adapter_from_config_with_runtime_profile(config,Some(profile.clone())), profile,
        baseline:arm=="baseline",root:root.clone(),chats:AtomicUsize::new(0),completions:AtomicUsize::new(0),started:Instant::now()};
    let mut prepared = prepare_agent_chat(&mut state,&input.to_string(),"2026-10-02T05:00:00Z")
        .unwrap_or_else(|r|panic!("prepare: {}",r.body));
    prepared.scope.provider_binding = adapter.model_runtime_profile();
    assert_eq!(serde_json::to_value(adapter.model_runtime_profile()).unwrap(),json_file(evidence().join("provider-profile.json")));
    if task == "mechanism" {
        let original = json_file(evidence().join("../presentation-staged-authoring-baseline/original-turn.json"));
        prepared.scope.reader_input = serde_json::from_value(original["admission_input"]["reader"].clone()).unwrap();
        if follow_up.is_none() {
            prepared.agent_message = original["admission_input"]["agent_message"].as_str().unwrap().into();
        }
    }
    save(root.join("admitted.json"),&json!({"messages":prepared.messages,"message":prepared.agent_message,"reader":prepared.scope.reader_input,"profile":adapter.model_runtime_profile()}));
    let turn_ref = prepared.turn_ref.clone();
    let app = BorrowedAppPort(std::cell::RefCell::new(&mut state));
    let report = crate::agent_run::execute_prepared(&app,&adapter,prepared,CancellationToken::default());
    std::fs::write(root.join("outcome.json"),&report.reply.body).unwrap();
    save(root.join("summary.json"),&json!({"task":task,"arm":arm,"status":report.reply.status,
        "elapsed_ms":adapter.started.elapsed().as_millis(),"model_requests":adapter.chats.load(SeqCst),"structured_completions":adapter.completions.load(SeqCst)}));
    app.with_app(|state| {
        if let Ok(outcome) = serde_json::from_str::<OuterOutcome>(&report.reply.body) {
            for part in outcome.answer_view.into_iter().flat_map(|v|v.parts) {
                if let AgentAnswerPart::Presentation {presentation_id,revision} = part {
                    let reference = runtime::presentation::PresentationRef {presentation_id,revision};
                    let version = state.private_context().read_presentation(&turn_ref.session_id,&reference).unwrap();
                    save(root.join("version.json"),&serde_json::to_value(&version).unwrap());
                    save(root.join("content.json"),&serde_json::to_value(&version.content).unwrap());
                    let view = crate::presentation_api::route(&state.private_context(),&json!({"session_id":turn_ref.session_id,"turn_id":turn_ref.turn_id,"reference":reference}).to_string(),false);
                    std::fs::write(root.join("view.json"),view.body).unwrap();
                }
            }
        }
    });
    assert_eq!(report.reply.status,200,"provider/runtime failure retained: {}",report.reply.body);
}
