//! Real loopback transport checks: deadlines must interrupt socket reads, not
//! merely stop waiting on an otherwise still-running model worker.
use super::*;
use model_spend::{ChargeScope, ModelSpendPort, SendEvidence, SendIdentity, SendOutcome, SpendStop};
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    sync::{mpsc, Arc, Mutex},
    time::{Duration, Instant},
};

#[derive(Default)]
struct Ledger {
    starts: Mutex<Vec<SendIdentity>>,
    finishes: Mutex<Vec<SendOutcome>>,
}
impl ModelSpendPort for Ledger {
    fn before_send(&self, identity: &SendIdentity, _: &Value) -> Result<(), SpendStop> {
        self.starts.lock().unwrap().push(identity.clone());
        Ok(())
    }
    fn after_send(&self, _: &SendIdentity, outcome: &SendOutcome) -> Result<(), SpendStop> {
        self.finishes.lock().unwrap().push(outcome.clone());
        Ok(())
    }
}

fn consume_request(stream: &mut TcpStream) {
    stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    let mut reader = BufReader::new(stream);
    let mut length = 0;
    loop {
        let mut line = String::new();
        assert_ne!(reader.read_line(&mut line).unwrap(), 0);
        if line == "\r\n" { break; }
        if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
            length = value.trim().parse().unwrap();
        }
    }
    reader.read_exact(&mut vec![0; length]).unwrap();
}

fn stalled_response(wire: &'static str) -> (String, mpsc::Sender<()>, std::thread::JoinHandle<bool>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (release, held) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        consume_request(&mut socket);
        socket.write_all(wire.as_bytes()).unwrap();
        // Keep the upstream open until the client has returned. Without the
        // timeout fix, this bounded fallback lets the test fail instead of hang.
        if held.recv_timeout(Duration::from_secs(3)).is_err() { return false; }
        let mut byte = [0];
        match socket.read(&mut byte) {
            Ok(0) => true,
            Err(error) if matches!(error.kind(), std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::ConnectionAborted) => true,
            _ => false,
        }
    });
    (url, release, worker)
}

fn config(mode: ProviderMode, url: String) -> ProviderConfig {
    ProviderConfig { agent_reasoning_effort: Default::default(), mode, api_key: "fixture-key".into(), base_url: url, model: "fixture-model".into() }
}
fn request() -> CompletionRequest {
    CompletionRequest {
        followup_user: None, images: Vec::new(), system: "fixture".into(), user: "fixture".into(),
        output_token_limit: Some(100), reasoning_effort: None,
    }
}
fn charge(adapter: &dyn ModelAdapter, ledger: Arc<Ledger>) {
    adapter.set_spend_context(ChargeScope::ReaderTask { user_id: "reader".into(), task_ref: "task".into() }, Some(ledger));
}

#[test]
fn provider_timeout_before_headers_does_not_retry_or_lose_charge_outcome() {
    for mode in [ProviderMode::Native, ProviderMode::ReAct] {
        let (url, release, worker) = stalled_response("");
        let adapter = ProviderRegistry::adapter_from_config_with_timeout(config(mode, url), Duration::from_millis(250));
        let ledger = Arc::new(Ledger::default());
        charge(adapter.as_ref(), ledger.clone());
        let start = Instant::now();
        let error = adapter.complete_structured(request()).unwrap_err();
        let elapsed = start.elapsed();
        let _ = release.send(());
        assert!(worker.join().unwrap(), "timed-out transport must close its socket");
        assert!(elapsed >= Duration::from_millis(200), "request failed before its deadline: {elapsed:?}; {error:?}");
        assert!(elapsed < Duration::from_millis(1500), "elapsed={elapsed:?}; {error:?}");
        assert_eq!(ledger.starts.lock().unwrap().len(), 1, "timeout must not start another billed request");
        let finishes = ledger.finishes.lock().unwrap();
        assert_eq!(finishes.len(), 1);
        assert_eq!(finishes[0].evidence, SendEvidence::OutcomeUnknown);
        assert!(finishes[0].usage.is_none());
        assert!(!finishes[0].succeeded);
    }
}

#[test]
fn provider_timeout_covers_stalled_body_and_keeps_partial_usage() {
    const OPENAI: &str = concat!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"{\"}}],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":5,\"total_tokens\":15}}\n\n"
    );
    const MESSAGES: &str = concat!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
        "data: {\"type\":\"message_start\",\"message\":{\"id\":\"fixture\",\"type\":\"message\",\"role\":\"assistant\",\"model\":\"fixture-model\",\"content\":[],\"usage\":{\"input_tokens\":10,\"output_tokens\":5,\"cache_read_input_tokens\":0,\"cache_creation_input_tokens\":0}}}\n\n"
    );
    for (mode, messages) in [(ProviderMode::Native, false), (ProviderMode::ReAct, false), (ProviderMode::Native, true)] {
        let (url, release, worker) = stalled_response(if messages { MESSAGES } else { OPENAI });
        let cfg = config(mode, url);
        let ledger = Arc::new(Ledger::default());
        let start = Instant::now();
        let error = if messages {
            let adapter = NativeAdapter::from_config_with_timeout(cfg, Duration::from_millis(250));
            charge(&adapter, ledger.clone());
            adapter.post_provider_request(json!({"model":"fixture-model", "messages":[], "max_tokens":100}), &mut provider_stream::ignore, "structured", true).unwrap_err()
        } else {
            let adapter = ProviderRegistry::adapter_from_config_with_timeout(cfg, Duration::from_millis(250));
            charge(adapter.as_ref(), ledger.clone());
            adapter.complete_structured(request()).unwrap_err()
        };
        let elapsed = start.elapsed();
        let _ = release.send(());
        assert!(worker.join().unwrap(), "response reader must release its socket");
        assert!(elapsed >= Duration::from_millis(200), "body failed before its deadline: {elapsed:?}; {error:?}");
        assert!(elapsed < Duration::from_millis(1500), "elapsed={elapsed:?}; {error:?}");
        assert_eq!(ledger.starts.lock().unwrap().len(), 1);
        let finishes = ledger.finishes.lock().unwrap();
        assert_eq!(finishes.len(), 1);
        assert_eq!(finishes[0].evidence, SendEvidence::Response);
        assert_eq!(finishes[0].usage.as_ref().unwrap().total_tokens, Some(15));
        assert!(!finishes[0].succeeded);
    }
}

#[test]
fn provider_timeout_connect_budget_does_not_cut_off_healthy_longer_stream() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let worker = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        consume_request(&mut socket);
        socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n").unwrap();
        for content in ["{", "\"ok\":", "true}"] {
            std::thread::sleep(Duration::from_millis(80));
            let frame = json!({"choices":[{"index":0,"delta":{"content":content}}]});
            write!(socket, "data: {frame}\n\n").unwrap();
        }
        socket.write_all(b"data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n").unwrap();
    });
    let adapter = ProviderRegistry::adapter_from_config_with_timeouts(config(ProviderMode::Native, url), ProviderRequestTimeouts {
        request: Duration::from_secs(2), connect: Duration::from_millis(50),
    });
    assert_eq!(adapter.complete_structured(request()).unwrap(), json!({"ok":true}));
    worker.join().unwrap();
}
