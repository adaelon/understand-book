//! Replay saved, hydrated coach inputs through the real execution path.
//! cargo run -p runtime --example coach_replay -- --dry-run INPUT_DIR NEW_OUTPUT_DIR
//! Use --live explicitly to send paid requests with the configured provider.
use runtime::{AdapterError, AgentRequestPlan, AssistantTurn, CompletionRequest, ModelAdapter, ParsedResponse};
use runtime::provider_stream::{ModelDelta, ModelObserver, ModelUsage};
use serde_json::{json, Value};
use std::{cell::RefCell, path::PathBuf, time::{Duration, Instant, SystemTime, UNIX_EPOCH}};

struct Recorder {
    inner: Box<dyn ModelAdapter + Send>,
    directory: PathBuf,
    live: bool,
    calls: RefCell<Vec<Value>>,
}
impl ModelAdapter for Recorder {
    fn model_runtime_profile(&self) -> runtime::ModelRuntimeProfile { self.inner.model_runtime_profile() }
    fn set_model_purpose(&self, purpose: &str) { self.inner.set_model_purpose(purpose); }
    fn complete(&self, _: CompletionRequest) -> Result<ParsedResponse, AdapterError> { unreachable!() }
    fn chat(&self, _: &AgentRequestPlan) -> Result<AssistantTurn, AdapterError> { unreachable!() }
    fn complete_structured_observed(&self, request: CompletionRequest, observer: &mut dyn ModelObserver) -> Result<Value, AdapterError> {
        let ordinal = self.calls.borrow().len() + 1;
        assert!(ordinal <= 10, "six frozen jobs permit at most ten paid calls after free stage start");
        std::fs::write(self.directory.join(format!("request-{ordinal}.json")), serde_json::to_vec_pretty(&request).unwrap()).unwrap();
        let started_ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();
        let started = Instant::now();
        let mut usage = ModelUsage::default();
        let result = if self.live {
            self.inner.complete_structured_observed(request.clone(), &mut |delta| {
                match &delta {
                    ModelDelta::Usage(value) => usage = value.clone(),
                    ModelDelta::Request(value) => { std::fs::write(self.directory.join(format!("wire-request-{ordinal}.json")), serde_json::to_vec_pretty(value.as_ref()).unwrap()).unwrap(); }
                    ModelDelta::Response(value) => { std::fs::write(self.directory.join(format!("wire-response-{ordinal}.json")), serde_json::to_vec_pretty(value.as_ref()).unwrap()).unwrap(); }
                    _ => {}
                }
                observer.observe(delta);
            })
        } else { Err(AdapterError {spend_stop:None,message:"dry run: request captured without provider execution".into()}) };
        self.calls.borrow_mut().push(json!({"ordinal":ordinal,"started_ms":started_ms,"elapsed_ms":started.elapsed().as_millis(),
            "system_bytes":request.system.len(),"input_bytes":request.user.len(),"correction":request.followup_user.is_some(),
            "usage":usage,"error":result.as_ref().err().map(|e|&e.message),"live":self.live}));
        result
    }
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert!(args.len()>=4 && matches!(args[1].as_str(),"--live"|"--dry-run"), "expected --live|--dry-run INPUT_DIR NEW_OUTPUT_DIR [ORIGINAL_REQUEST ...]");
    let input_dir = PathBuf::from(&args[2]); let output = PathBuf::from(&args[3]);
    assert!(!output.exists(), "choose a new output directory to avoid repeating a paid run");
    std::fs::create_dir_all(&output).unwrap();
    let live = args[1] == "--live";
    let config = if live { runtime::ProviderConfig::from_env().expect("configured provider") }
        else { runtime::ProviderConfig::from_values("native","unused","https://api.deepseek.com/v1","deepseek-flash").unwrap() };
    let adapter = Recorder {inner:runtime::ProviderRegistry::adapter_from_config_with_timeout(config,Duration::from_secs(180)),
        directory:output.clone(),live,calls:RefCell::new(vec![])};
    let mut rows=vec![];
    let jobs: Vec<u32> = if args.len() == 4 { vec![1,14,21,28,34,36] }
        else { args[4..].iter().map(|v|v.parse().unwrap()).collect() };
    for n in jobs {
        let input: Value = serde_json::from_slice(&std::fs::read(input_dir.join(format!("input-{n}.json"))).unwrap()).unwrap();
        let before = adapter.calls.borrow().len();
        let result = runtime::tutor_coach::execute_coach(&adapter,input);
        let row = json!({"original_request":n,"calls":adapter.calls.borrow().len()-before,
            "result":result.as_ref().ok(),"error":result.as_ref().err().map(|e|&e.message)});
        std::fs::write(output.join(format!("job-{n}.json")),serde_json::to_vec_pretty(&row).unwrap()).unwrap();
        println!("job {n}: calls={}, valid={}",row["calls"],result.is_ok());
        rows.push(row);
        std::fs::write(output.join("run.json"),serde_json::to_vec_pretty(&json!({"live":live,"jobs":rows,"calls":*adapter.calls.borrow()})).unwrap()).unwrap();
    }
}
