//! Paid, opt-in end-to-end text delivery measurement. No model or tool result is scripted.
use super::*;
use crate::agent_stream::{RunDescriptor, RunEvent, RunStream};
use runtime::model_spend::{ChargeScope, ModelSpendPort, SendIdentity, SendOutcome, SpendStop};
use runtime::provider_stream::{ModelDelta, ModelObserver};
use runtime::run_events::{RunEventSink, RuntimeEvent};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Instant;

pub const MAX_HTTP_REQUESTS: usize = 24;
pub const SOURCE: &str = "过拟合与正则化\n训练误差是在参与拟合的数据上计算的误差；验证误差是在未用于拟合参数的数据上计算的误差，用于估计对同类新样本的泛化。过拟合是模型过度适应训练数据的偶然细节，训练误差低并不保证验证误差低。例如模型 A 的训练均方误差为 0.01、验证均方误差为 0.30；模型 B 的训练均方误差为 0.08、验证均方误差为 0.12。在数据划分和度量相同的前提下，B 在这份验证集上泛化更好，A 表现出更明显的训练与验证差距。\nL2 正则化在经验训练损失上加参数平方和惩罚。若预测为 f_w(x)，常见目标是 J(w)= (1/n)∑[f_w(x_i)-y_i]^2 + λ∑w_j^2，其中 λ≥0 调节惩罚强度，是否惩罚偏置须按模型约定。λ=0.01 表示参数平方和的权重为 0.01，并不是把训练误差直接减去 0.01。优化时，模型在拟合训练样本与保持较小权重之间取舍；对单个参数 w_j，惩罚项的梯度为 2λw_j。正则化可能降低方差、改善泛化，但不会保证每份验证集上的误差都下降；λ 太大可能造成欠拟合，通常用验证数据选择 λ。";
pub const QUESTIONS: [&str; 2] = [
    "请结合当前这篇材料，用纯文字讲清楚什么是过拟合，以及为什么模型 A 训练误差 0.01、验证误差 0.30，而模型 B 训练误差 0.08、验证误差 0.12 时，更应该选 B。请直接讲解，不用展示页面，也暂时不要出题。",
    "接着刚才的解释，继续用文字和公式讲清楚 L2 正则化怎么缓解过拟合：写出 λ=0.01 的训练目标，解释惩罚项及其梯度，最后说明它为什么不能保证验证误差一定下降。不要展示页面，也暂时不要出题。",
];

pub fn save(directory: &Path, name: &str, value: &Value) {
    std::fs::write(directory.join(name), serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

/// This port deliberately retains no request body. NativeAdapter calls it for every HTTP attempt,
/// including automatic retries and private coach/review calls.
pub struct RequestBudget {
    directory: PathBuf,
    start: Instant,
    attempts: Mutex<Vec<(Instant, Value)>>,
    denied: AtomicU32,
}
impl RequestBudget {
    fn count(&self) -> usize { self.attempts.lock().unwrap().len() }
    fn persist(&self, attempts: &[(Instant, Value)]) {
        save(&self.directory, "http-attempts.json", &json!({
            "limit": MAX_HTTP_REQUESTS, "denied": self.denied.load(Ordering::Relaxed),
            "attempts": attempts.iter().map(|(_, value)| value).collect::<Vec<_>>()
        }));
    }
}
impl ModelSpendPort for RequestBudget {
    fn before_send(&self, identity: &SendIdentity, _request: &Value) -> Result<(), SpendStop> {
        let mut attempts = self.attempts.lock().unwrap();
        if attempts.len() >= MAX_HTTP_REQUESTS {
            self.denied.fetch_add(1, Ordering::Relaxed);
            self.persist(&attempts);
            return Err(SpendStop::InsufficientAllowance);
        }
        attempts.push((Instant::now(), json!({
            "call_id": identity.call_id, "logical_call_id": identity.logical_call_id,
            "attempt": identity.attempt, "purpose": identity.purpose, "model": identity.model,
            "started_at_unix_ms": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis(),
            "started_ms": self.start.elapsed().as_secs_f64() * 1000.0
        })));
        self.persist(&attempts);
        Ok(())
    }
    fn after_send(&self, identity: &SendIdentity, outcome: &SendOutcome) -> Result<(), SpendStop> {
        let mut attempts = self.attempts.lock().unwrap();
        let (start, value) = attempts.iter_mut().find(|(_, value)| value["call_id"] == identity.call_id).unwrap();
        value["duration_ms"] = json!(start.elapsed().as_secs_f64() * 1000.0);
        value["outcome"] = json!(outcome);
        self.persist(&attempts);
        Ok(())
    }
}

/// Explicit local content capture, separate from spending. This contains fixture content only;
/// no provider credentials, HTTP headers or provider configuration are serialized.
pub struct CaptureSink { directory: PathBuf }
impl RunEventSink for CaptureSink {
    fn captures_content(&self) -> bool { true }
    fn emit(&self, _event: RuntimeEvent) {}
    fn activity_input(&self, id: u32, input: Value) {
        save(&self.directory, &format!("request-{id:03}.json"), &input);
    }
    fn activity_output(&self, id: u32, output: Value) {
        save(&self.directory, &format!("response-{id:03}.json"), &output);
    }
}

/// Transparent transport observation: every method delegates to NativeAdapter with its normal
/// profile, reasoning, tool choice and limits. Request/response deltas still reach the host.
pub struct CapturedNative {
    native: runtime::NativeAdapter,
    sink: CaptureSink,
    sequence: AtomicU32,
}
impl CapturedNative {
    fn capture<T>(
        &self, observer: &mut dyn ModelObserver,
        call: impl FnOnce(&mut dyn ModelObserver) -> Result<T, AdapterError>,
    ) -> Result<T, AdapterError> {
        let id = self.sequence.fetch_add(1, Ordering::Relaxed) + 1;
        let start = Instant::now();
        let mut first_text_ms = None;
        let mut usage = None;
        let result = call(&mut |delta: ModelDelta| {
            match &delta {
                ModelDelta::Request(body) if self.sink.captures_content() => self.sink.activity_input(id, (**body).clone()),
                ModelDelta::Response(body) if self.sink.captures_content() => self.sink.activity_output(id, (**body).clone()),
                ModelDelta::Text(text) if !text.is_empty() && first_text_ms.is_none() => {
                    first_text_ms = Some(start.elapsed().as_secs_f64() * 1000.0);
                }
                ModelDelta::Usage(snapshot) => usage = Some(snapshot.clone()),
                _ => {}
            }
            observer.observe(delta);
        });
        save(&self.sink.directory, &format!("call-{id:03}.json"), &json!({
            "duration_ms": start.elapsed().as_secs_f64() * 1000.0,
            "provider_first_text_ms": first_text_ms, "usage": usage,
            "succeeded": result.is_ok(), "error": result.as_ref().err().map(|error| &error.message),
            "metric_note": "Provider text may be private or a protocol fragment; it is not public answer latency."
        }));
        result
    }
}
impl ModelAdapter for CapturedNative {
    fn set_spend_context(&self, scope: ChargeScope, port: Option<Arc<dyn ModelSpendPort>>) { self.native.set_spend_context(scope, port); }
    fn set_model_purpose(&self, purpose: &str) { self.native.set_model_purpose(purpose); }
    fn set_run_cancellation(&self, cancellation: runtime::run_context::CancellationToken) { self.native.set_run_cancellation(cancellation); }
    fn model_runtime_profile(&self) -> runtime::ModelRuntimeProfile { self.native.model_runtime_profile() }
    fn stream_text_is_structured(&self) -> bool { self.native.stream_text_is_structured() }
    fn complete_observed(&self, request: CompletionRequest, observer: &mut dyn ModelObserver) -> Result<ParsedResponse, AdapterError> {
        self.capture(observer, |observer| self.native.complete_observed(request, observer))
    }
    fn complete_structured_observed(&self, request: CompletionRequest, observer: &mut dyn ModelObserver) -> Result<Value, AdapterError> {
        self.capture(observer, |observer| self.native.complete_structured_observed(request, observer))
    }
    fn chat_observed(&self, request: &AgentRequestPlan, observer: &mut dyn ModelObserver) -> Result<AssistantTurn, AdapterError> {
        self.capture(observer, |observer| self.native.chat_observed(request, observer))
    }
    fn complete(&self, request: CompletionRequest) -> Result<ParsedResponse, AdapterError> {
        self.complete_observed(request, &mut runtime::provider_stream::ignore)
    }
    fn complete_structured(&self, request: CompletionRequest) -> Result<Value, AdapterError> {
        self.complete_structured_observed(request, &mut runtime::provider_stream::ignore)
    }
    fn chat(&self, request: &AgentRequestPlan) -> Result<AssistantTurn, AdapterError> {
        self.chat_observed(request, &mut runtime::provider_stream::ignore)
    }
}

pub fn markdown(view: &Value) -> String {
    view["parts"].as_array().into_iter().flatten()
        .filter(|part| part["kind"] == "markdown")
        .filter_map(|part| part["text"].as_str()).collect::<Vec<_>>().join("\n")
}
pub struct StopReader(Arc<AtomicBool>);
impl Drop for StopReader {
    fn drop(&mut self) { self.0.store(true, Ordering::Release); }
}

#[test]
#[ignore = "Paid live Native provider: requires credentials and fresh WORKSPACE_CACHE_LIVE_OUTPUT; at most 24 HTTP attempts"]
fn workspace_cache_live_public_text_delivery() {
    let directory = PathBuf::from(std::env::var("WORKSPACE_CACHE_LIVE_OUTPUT").expect("Set a fresh WORKSPACE_CACHE_LIVE_OUTPUT"));
    std::fs::create_dir_all(&directory).unwrap();
    assert!(std::fs::read_dir(&directory).unwrap().next().is_none(), "Use an empty output directory");
    let mut provider = ProviderConfig::from_env().unwrap_or_else(|_| panic!("Configure the normal Native provider credentials before opting in"));
    provider.mode = runtime::ProviderMode::Native;
    let budget = Arc::new(RequestBudget { directory: directory.clone(), start: Instant::now(), attempts: Mutex::new(Vec::new()), denied: AtomicU32::new(0) });
    let scope = ChargeScope::OperatorTask { task_ref: "workspace-cache-live-public-text".into() };
    let adapter = CapturedNative { native: runtime::NativeAdapter::from_config(provider.clone()), sink: CaptureSink { directory: directory.clone() }, sequence: AtomicU32::new(0) };
    adapter.set_spend_context(scope.clone(), Some(budget.clone()));
    let mut base = sample_base();
    base.book_id = "workspace-cache-live-overfitting".into();
    base.graph_nodes.clear(); base.graph_edges.clear();
    for node in &mut base.lid_nodes { node.span = Span { start: 0, end: SOURCE.encode_utf16().count() }; }
    let (_root, mut state) = super::tutor_loop_tests::fixture_with_book(false, Book::new(base, SOURCE), SOURCE);
    // Admission freezes this profile; it must match the live execution adapter.
    state.services.adapter = Box::new(runtime::NativeAdapter::from_config(provider));
    state.services.adapter.set_spend_context(scope, Some(budget.clone()));
    super::session_runtime_tests::enable_jsonl(&mut state);
    assert_eq!(route_agent_new(&mut state, "workspace-cache-live").status, 200);
    save(&directory, "configuration.json", &json!({
        "profile": adapter.model_runtime_profile(), "source": SOURCE, "questions": QUESTIONS,
        "max_http_requests": MAX_HTTP_REQUESTS, "model_transport": "live NativeAdapter for all model calls",
        "metric": "time_to_first_public_answer_patch_ms",
        "boundary": "before prepare_agent_chat through first nonempty Markdown answer.patch; includes inline pre-coach, private review, server select, outer/finalization; excludes browser rendering and post-turn coach",
        "state": "temporary fixture book, JSONL history and isolated learning store"
    }));
    let mut turns = Vec::new();
    for (index, question) in QUESTIONS.iter().enumerate() {
        let number = index + 1;
        let started = Instant::now();
        let request = json!({"message": question});
        let prepared = match prepare_agent_chat(&mut state, &request.to_string(), &format!("workspace-cache-live-{number}")) {
            Ok(prepared) => prepared,
            Err(reply) => {
                let failure = json!({"turn":number,"admission_failed":true,"http_status":reply.status,"reply":reply.body});
                save(&directory, &format!("turn-{number:02}.json"), &failure);
                turns.push(failure); break;
            }
        };
        let turn = prepared.turn_ref.clone();
        let tutor = prepared.tutor.clone();
        save(&directory, &format!("input-{number:02}.json"), &json!({"request":request,"tutor":tutor,"provider_profile":prepared.scope.provider_binding}));
        let stream_offset_ms = started.elapsed().as_secs_f64() * 1000.0;
        let stream = RunStream::new(RunDescriptor { book_id: state.workspace.book.base.book_id.clone(), session_id: turn.session_id.clone(), turn_id: turn.turn_id.clone() });
        let done = Arc::new(AtomicBool::new(false));
        let (report, events) = std::thread::scope(|threads| {
            let reader_done = done.clone();
            let stream = &stream;
            let reader = threads.spawn(move || {
                let mut events: Vec<RunEvent> = Vec::new();
                let mut after = Some(0);
                loop {
                    let stopping = reader_done.load(Ordering::Acquire);
                    let (page, _) = stream.read_after(after, Duration::from_millis(25));
                    if let Some(last) = page.last() { after = Some(last.seq); }
                    events.extend(page);
                    if stopping { return events; }
                }
            });
            let stop = StopReader(done);
            let report = agent_run::execute_prepared_with_stream_for_test(
                &agent_run::BorrowedAppPort(RefCell::new(&mut state)), &adapter, prepared, Default::default(), stream,
            );
            drop(stop);
            (report, reader.join().unwrap())
        });
        let first = events.iter().find(|event| event.event_type == "answer.patch"
            && event.payload["operation"] != "discard"
            && !markdown(&event.payload["view"]).trim().is_empty());
        let saved = state.user.agent_history.sessions.iter().find(|session| session.id == turn.session_id).unwrap()
            .turns.iter().find(|saved| saved.turn_id == turn.turn_id).unwrap();
        let deliveries = state.user.learning_store().unwrap().teaching_deliveries(&turn.session_id, &turn.turn_id).unwrap();
        let answer = saved.outcome.as_ref().and_then(|outcome| outcome.answer_view.as_ref())
            .map(|view| markdown(&json!(view))).unwrap_or_default();
        let runtime_incomplete = saved.outcome.as_ref().is_none_or(|outcome| outcome.incomplete);
        let delivered = report.reply.status == 200 && first.is_some() && !answer.trim().is_empty() && !deliveries.is_empty();
        let record = json!({
            "turn":number, "http_status":report.reply.status, "reply":serde_json::from_str::<Value>(&report.reply.body).unwrap_or(json!(report.reply.body)),
            "delivered":delivered, "runtime_incomplete":runtime_incomplete,
            "time_to_first_public_answer_patch_ms":first.filter(|_| delivered).map(|event| stream_offset_ms + event.elapsed_ms),
            "prepare_and_capture_ms":stream_offset_ms, "first_observed_markdown_patch":first,
            "elapsed_including_post_coach_ms":started.elapsed().as_secs_f64() * 1000.0,
            "http_attempts_total":budget.count(), "logical_model_calls_total":adapter.sequence.load(Ordering::Relaxed),
            "saved_turn":saved, "deliveries":deliveries
        });
        save(&directory, &format!("turn-{number:02}.json"), &record);
        save(&directory, &format!("events-{number:02}.json"), &json!(events));
        std::fs::write(directory.join(format!("answer-{number:02}.md")), answer).unwrap();
        turns.push(record);
        println!("WORKSPACE_CACHE_LIVE turn={number} delivered={delivered} http_attempts={}", budget.count());
        if !delivered || runtime_incomplete || budget.denied.load(Ordering::Relaxed) > 0 { break; }
    }
    let complete = turns.len() == QUESTIONS.len() && turns.iter().all(|turn| turn["delivered"] == true && turn["runtime_incomplete"] == false)
        && budget.denied.load(Ordering::Relaxed) == 0;
    save(&directory, "complete.json", &json!({
        "complete":complete, "successful_deliveries":turns.iter().filter(|turn| turn["delivered"] == true).count(),
        "turns":turns, "http_attempts":budget.count(),
        "logical_model_calls":adapter.sequence.load(Ordering::Relaxed), "budget_denials":budget.denied.load(Ordering::Relaxed)
    }));
    assert!(complete, "Live conversation incomplete; inspect output artifacts. The HTTP budget was not extended.");
}

#[test]
fn workspace_cache_live_budget_stops_before_25th_http_attempt() {
    let directory = tempfile::tempdir().unwrap();
    let budget = RequestBudget { directory: directory.path().into(), start: Instant::now(), attempts: Mutex::new(Vec::new()), denied: AtomicU32::new(0) };
    for number in 0..MAX_HTTP_REQUESTS {
        let identity = SendIdentity { call_id: number.to_string(), logical_call_id: "same-logical-call-with-retries".into(),
            attempt: number as u32, scope: None, purpose: "budget-unit-test".into(), model: "no-model-requested".into(), provider: "unused".into() };
        budget.before_send(&identity, &json!({"private_body":"must_not_be_recorded"})).unwrap();
        budget.after_send(&identity, &SendOutcome { evidence: runtime::model_spend::SendEvidence::NotSent, usage: None, succeeded: false }).unwrap();
    }
    let identity = SendIdentity { call_id: "25".into(), logical_call_id: "another-logical-call".into(), attempt: 0,
        scope: None, purpose: "budget-unit-test".into(), model: "no-model-requested".into(), provider: "unused".into() };
    assert_eq!(budget.before_send(&identity, &Value::Null), Err(SpendStop::InsufficientAllowance));
    assert_eq!(budget.count(), MAX_HTTP_REQUESTS);
    let log = std::fs::read_to_string(directory.path().join("http-attempts.json")).unwrap();
    assert!(!log.contains("must_not_be_recorded"));
}
