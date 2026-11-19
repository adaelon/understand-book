//! SE8 live experiment driver. Explicitly invoked with the existing budget relay.
use super::*;
use base64::Engine as _;
use runtime::provider_stream::{ModelDelta, ModelObserver};
use std::{path::{Path, PathBuf}, sync::atomic::{AtomicUsize, Ordering::SeqCst}, time::Instant};

pub fn save(root: &Path, name: &str, value: &Value) {
    std::fs::write(root.join(name), serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

pub struct Recorded {
    inner: Box<dyn ModelAdapter + Send>, root: PathBuf,
    profile: runtime::ModelRuntimeProfile, calls: AtomicUsize, output_tokens: Option<u32>,
    purpose: std::sync::Mutex<String>,
}
impl ModelAdapter for Recorded {
    fn model_runtime_profile(&self) -> runtime::ModelRuntimeProfile { self.profile.clone() }
    fn set_run_cancellation(&self, token: CancellationToken) { self.inner.set_run_cancellation(token); }
    fn set_model_purpose(&self, purpose: &str) {
        *self.purpose.lock().unwrap() = purpose.into();
        self.inner.set_model_purpose(purpose);
    }
    fn complete(&self, req: CompletionRequest) -> Result<ParsedResponse, runtime::AdapterError> { self.inner.complete(req) }
    fn complete_structured(&self, req: CompletionRequest) -> Result<Value, runtime::AdapterError> {
        self.complete_structured_observed(req, &mut runtime::provider_stream::ignore)
    }
    fn complete_structured_observed(&self, mut req: CompletionRequest, observer: &mut dyn ModelObserver) -> Result<Value, runtime::AdapterError> {
        if let Some(limit) = self.output_tokens { req.output_token_limit = Some(limit); }
        let n = self.calls.fetch_add(1, SeqCst);
        save(&self.root, &format!("request-{n:03}.json"), &json!({"kind":"structured","purpose":self.purpose.lock().unwrap().clone(),"system":req.system,"user":req.user,"images":req.images,"output_token_limit":req.output_token_limit}));
        let start = Instant::now(); let mut usage = None;
        let result = self.inner.complete_structured_observed(req, &mut |delta: ModelDelta| {
            if let ModelDelta::Usage(value) = &delta { usage = Some(value.clone()); }
            observer.observe(delta);
        });
        save(&self.root, &format!("response-{n:03}.json"), &json!({"response":result.as_ref().ok(),"usage":usage,"error":result.as_ref().err().map(|e|&e.message),"elapsed_ms":start.elapsed().as_millis()}));
        result
    }
    fn chat(&self, req: &runtime::AgentRequestPlan) -> Result<runtime::AssistantTurn, runtime::AdapterError> {
        self.chat_observed(req, &mut runtime::provider_stream::ignore)
    }
    fn chat_observed(&self, req: &runtime::AgentRequestPlan, observer: &mut dyn ModelObserver) -> Result<runtime::AssistantTurn, runtime::AdapterError> {
        let mut req = req.clone();
        if let Some(limit) = self.output_tokens { req.output_token_limit = Some(limit); }
        let n = self.calls.fetch_add(1, SeqCst);
        let images: Vec<_> = req.preview_images.iter().enumerate().map(|(i,image)| {
            let file = format!("image-{n:03}-{i:02}.png");
            std::fs::write(self.root.join(&file), base64::engine::general_purpose::STANDARD.decode(&image.png_base64).unwrap()).unwrap();
            json!({"file":file,"caption":image.caption,"candidate_id":image.candidate_id,"environment":image.environment_name})
        }).collect();
        save(&self.root, &format!("request-{n:03}.json"), &json!({"kind":"chat","instructions":req.instructions,"instruction_assets":req.instruction_assets,"messages":req.ordered_messages(),"images":images,"runtime_profile":req.runtime_profile,"output_token_limit":req.output_token_limit,"tools":req.tools.iter().map(|t|json!({"name":t.name,"description":t.description,"parameters":t.parameters})).collect::<Vec<_>>()}));
        let start = Instant::now(); let mut usage = None;
        let result = self.inner.chat_observed(&req, &mut |delta: ModelDelta| {
            if let ModelDelta::Usage(value) = &delta { usage = Some(value.clone()); }
            observer.observe(delta);
        });
        save(&self.root, &format!("response-{n:03}.json"), &json!({"text":result.as_ref().ok().and_then(|t|t.text.as_ref()),"tool_calls":result.as_ref().ok().map(|t|&t.tool_calls),"usage":usage,"error":result.as_ref().err().map(|e|&e.message),"elapsed_ms":start.elapsed().as_millis()}));
        result
    }
}

#[test]
#[ignore = "SE8 paid paired experiment; invoke only through the frozen run.py budget relay"]
fn se8_live_task() {
    let root = PathBuf::from(std::env::var_os("SE8_RUN_DIR").expect("SE8_RUN_DIR"));
    let input = PathBuf::from(std::env::var_os("SE8_INPUT").expect("SE8_INPUT"));
    let relay = std::env::var("SE8_RELAY").expect("SE8_RELAY");
    assert!(relay.starts_with("http://127.0.0.1:"));
    std::fs::create_dir(&root).expect("new evidence directory");
    let mut state = state_named(&format!("se8-{}", root.parent().unwrap().file_name().unwrap().to_string_lossy()));
    state.user.store = MemoryStore::open(root.join("memory.json")).unwrap();
    state.user.history_path = Some(root.join("history.json"));
    let message = std::fs::read_to_string(&input).unwrap();
    let mut prepared = prepare_agent_chat(&mut state, &json!({"message":message}).to_string(), "2026-10-16T04:00:00Z")
        .unwrap_or_else(|r| panic!("{}",r.body));
    let turn = prepared.turn_ref.clone();
    if let Some(base) = std::env::var_os("SE8_BASE_VERSION") {
        let mut version: runtime::presentation::AgentPresentation = serde_json::from_str(&std::fs::read_to_string(base).unwrap()).unwrap();
        version.owner.session_id = turn.session_id.clone();
        version.created_by_turn_id = turn.turn_id.clone();
        let target = root.join("history.presentations/versions").join(&version.reference.presentation_id);
        std::fs::create_dir_all(&target).unwrap();
        save(&target, &format!("{}.json",version.reference.revision), &json!(version));
        save(&root,"imported-base.json",&json!(version));
    }
    let config = ProviderConfig::from_values("native","local-recording-relay",relay,"deepseek-v4-flash").unwrap();
    let mut profile = runtime::ModelRuntimeCatalog::default().resolve("deepseek-v4-flash",runtime::ProviderToolProtocol::Native,None);
    // Omission preserves the production per-request limits (including Author).
    // Archived experiments can still explicitly request their frozen override.
    let output_tokens: Option<u32> = std::env::var("SE8_OUTPUT_TOKENS").ok().map(|value| value.parse().unwrap());
    if let Some(limit) = output_tokens { profile.output_reserve_tokens = limit; }
    let adapter = Recorded {inner:ProviderRegistry::adapter_from_config_with_runtime_profile(config,Some(profile.clone())),profile,root:root.clone(),calls:AtomicUsize::new(0),output_tokens,purpose:std::sync::Mutex::new("resident".into())};
    prepared.scope.provider_binding = adapter.profile.clone();
    save(&root,"input.json",&json!({"message":message,"catalog":runtime::presentation_methods::catalog(),"profile":adapter.profile}));
    let app = BorrowedAppPort(std::cell::RefCell::new(&mut state));
    let started = Instant::now();
    let report = crate::agent_run::execute_prepared(&app,&adapter,prepared,CancellationToken::default());
    std::fs::write(root.join("outcome.json"),&report.reply.body).unwrap();
    save(&root,"run.json",&json!({"http_status":report.reply.status,"model_calls":adapter.calls.load(SeqCst),"elapsed_ms":started.elapsed().as_millis()}));
    app.with_app(|s| {
        save(&root,"history-export.json",&json!(s.user.agent_history));
        save(&root,"observations.json",&json!(s.private_context().presentation_method_observations(&turn.session_id,&turn.turn_id).unwrap()));
        if let Ok(outcome) = serde_json::from_str::<OuterOutcome>(&report.reply.body) {
            for part in outcome.answer_view.into_iter().flat_map(|v|v.parts) {
                if let AgentAnswerPart::Presentation {presentation_id,revision} = part {
                    let reference = runtime::presentation::PresentationRef {presentation_id,revision};
                    let version = s.private_context().read_presentation(&turn.session_id,&reference).unwrap();
                    save(&root,"version.json",&json!(version));
                    save(&root,"content.json",&json!(version.content));
                }
            }
        }
    });
    // A failed or incomplete generation is an experiment result, not a runner assertion.
}
