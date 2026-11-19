//! Real HTTP recovery checks, including spend identities and cancellation.
use super::*;
use model_spend::{
    ChargeScope, ModelSpendPort, SendEvidence, SendIdentity, SendOutcome, SpendStop,
};
use serde_json::{json, Value};
use std::{
    collections::VecDeque,
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

#[derive(Default)]
struct Ledger {
    starts: Mutex<Vec<(SendIdentity, Value)>>,
    finishes: Mutex<Vec<SendOutcome>>,
    reject_attempt: Option<u32>,
    fail_finish: bool,
}
impl ModelSpendPort for Ledger {
    fn before_send(&self, id: &SendIdentity, request: &Value) -> Result<(), SpendStop> {
        if self.reject_attempt == Some(id.attempt) {
            return Err(SpendStop::InsufficientAllowance);
        }
        self.starts
            .lock()
            .unwrap()
            .push((id.clone(), request.clone()));
        Ok(())
    }
    fn after_send(&self, _: &SendIdentity, outcome: &SendOutcome) -> Result<(), SpendStop> {
        self.finishes.lock().unwrap().push(outcome.clone());
        if self.fail_finish {
            Err(SpendStop::StorageUnavailable)
        } else {
            Ok(())
        }
    }
}

struct Fixture {
    url: String,
    requests: Arc<Mutex<Vec<(Instant, Value)>>>,
    stop: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl Fixture {
    fn new(replies: Vec<String>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let captured = requests.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let worker = std::thread::spawn(move || {
            let mut replies: VecDeque<_> = replies.into();
            while !stopped.load(Ordering::Acquire) {
                let (mut socket, _) = match listener.accept() {
                    Ok(value) => value,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(2));
                        continue;
                    }
                    Err(error) => panic!("fixture accept: {error}"),
                };
                socket.set_nonblocking(false).unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut reader = BufReader::new(&mut socket);
                let mut length = 0;
                loop {
                    let mut line = String::new();
                    assert_ne!(reader.read_line(&mut line).unwrap(), 0);
                    if line == "\r\n" {
                        break;
                    }
                    if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                        length = value.trim().parse().unwrap();
                    }
                }
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                captured
                    .lock()
                    .unwrap()
                    .push((Instant::now(), serde_json::from_slice(&body).unwrap()));
                drop(reader);
                let reply = replies.pop_front().unwrap_or_else(|| {
                    response(400, "", json!({"error":{"code":"unexpected_request"}}))
                });
                let _ = socket.write_all(reply.as_bytes());
            }
        });
        Self {
            url,
            requests,
            stop,
            worker: Some(worker),
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.worker.take().unwrap().join().unwrap();
    }
}
fn response(status: u16, headers: &str, body: Value) -> String {
    let body = body.to_string();
    format!("HTTP/1.1 {status} Fixture\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n{body}", body.len())
}
fn success(messages: bool) -> String {
    response(
        200,
        "",
        if messages {
            json!({"type":"message","role":"assistant","content":[{"type":"text","text":"{\"ok\":true}"}],"stop_reason":"end_turn","usage":{"input_tokens":10,"output_tokens":5,"cache_read_input_tokens":0,"cache_creation_input_tokens":0}})
        } else {
            json!({"choices":[{"message":{"content":"{\"ok\":true}"},"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15}})
        },
    )
}
fn adapter(fixture: &Fixture, ledger: Arc<Ledger>, timeout: Duration) -> NativeAdapter {
    let adapter = NativeAdapter::from_config_with_timeout(
        ProviderConfig { options: Default::default(),
            agent_reasoning_effort: Default::default(),
            mode: ProviderMode::Native,
            api_key: "fixture-key".into(),
            base_url: fixture.url.clone(),
            model: "fixture-model".into(),
        },
        timeout,
    );
    adapter.set_spend_context(
        ChargeScope::ReaderRun {
            user_id: "reader".into(),
            run_ref: "provider-retry".into(),
        },
        Some(ledger),
    );
    adapter
}
fn send(adapter: &NativeAdapter, messages: bool) -> Result<Value, AdapterError> {
    adapter.post_provider_request(json!({"model":"fixture-model","messages":[{"role":"user","content":"fixture"}],"max_tokens":100}), &mut provider_stream::ignore, "structured", if messages { crate::model_settings::ApiProtocol::DeepseekMessages } else { crate::model_settings::ApiProtocol::OpenaiChatCompletions })
}

#[test]
fn provider_retry_recovers_429_and_503_with_separate_spend_receipts() {
    for messages in [false, true] {
        let f = Fixture::new(vec![
            response(429, "", json!({"error":{"code":"rate_limit_exceeded"}})),
            response(503, "", json!({"error":{"message":"overloaded"}})),
            success(messages),
        ]);
        let ledger = Arc::new(Ledger::default());
        let adapter = adapter(&f, ledger.clone(), Duration::from_secs(15));
        let value = send(&adapter, messages).unwrap();
        assert_eq!(value["choices"][0]["message"]["content"], "{\"ok\":true}");
        let starts = ledger.starts.lock().unwrap();
        let finishes = ledger.finishes.lock().unwrap();
        assert_eq!(starts.len(), 3);
        assert_eq!(finishes.len(), 3);
        assert_eq!(f.requests.lock().unwrap().len(), 3);
        for (i, (id, request)) in starts.iter().enumerate() {
            assert_eq!(id.attempt, i as u32 + 1);
            assert_eq!(id.logical_call_id, starts[0].0.logical_call_id);
            assert_eq!(request, &starts[0].1);
            assert_eq!(finishes[i].evidence, SendEvidence::Response);
            assert_eq!(finishes[i].succeeded, i == 2);
        }
        assert_ne!(starts[0].0.call_id, starts[1].0.call_id);
        assert_ne!(starts[1].0.call_id, starts[2].0.call_id);
        assert!(finishes[0].usage.is_none());
        assert_eq!(finishes[2].usage.as_ref().unwrap().total_tokens, Some(15));
        assert!(
            value["usage"]["total_tokens"].is_null(),
            "unknown failed attempts must not become free usage"
        );
    }
}

#[test]
fn provider_retry_stops_after_three_retries() {
    let wire = response(503, "", json!({"error":{"message":"overloaded"}}));
    let f = Fixture::new(vec![wire; 4]);
    let ledger = Arc::new(Ledger::default());
    let adapter = adapter(&f, ledger.clone(), Duration::from_secs(20));
    let error = send(&adapter, false).unwrap_err();
    assert!(error.message.contains("status code 503"));
    assert_eq!(f.requests.lock().unwrap().len(), 4);
    assert_eq!(ledger.finishes.lock().unwrap().len(), 4);
}

#[test]
fn provider_retry_does_not_retry_auth_parameters_or_exhausted_balance() {
    for (status, body) in [
        (400, json!({"error":{"code":"invalid_request"}})),
        (401, json!({"error":{"code":"invalid_api_key"}})),
        (402, json!({"error":{"message":"Insufficient Balance"}})),
        (403, json!({"error":{"code":"permission_error"}})),
        (422, json!({"error":{"code":"invalid_parameters"}})),
        (429, json!({"error":{"code":"insufficient_quota"}})),
        (429, json!({"error":{"type":"insufficient_balance"}})),
    ] {
        let f = Fixture::new(vec![response(status, "Retry-After: 1\r\n", body)]);
        let ledger = Arc::new(Ledger::default());
        assert!(send(&adapter(&f, ledger.clone(), Duration::from_secs(10)), false).is_err());
        assert_eq!(f.requests.lock().unwrap().len(), 1, "status={status}");
        assert_eq!(ledger.finishes.lock().unwrap().len(), 1);
    }
}

#[test]
fn provider_retry_honors_retry_after_and_records_usage_on_http_error() {
    let f = Fixture::new(vec![
        response(
            500,
            "Retry-After: 2\r\n",
            json!({"error":{"message":"server error"},"usage":{"prompt_tokens":7,"completion_tokens":3,"total_tokens":10}}),
        ),
        success(false),
    ]);
    let ledger = Arc::new(Ledger::default());
    let value = send(&adapter(&f, ledger.clone(), Duration::from_secs(10)), false).unwrap();
    let requests = f.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert!(requests[1].0.duration_since(requests[0].0) >= Duration::from_secs(2));
    assert_eq!(value["usage"]["total_tokens"], 25);
    assert_eq!(
        ledger.finishes.lock().unwrap()[0]
            .usage
            .as_ref()
            .unwrap()
            .total_tokens,
        Some(10)
    );
}

#[test]
fn provider_retry_respects_total_budget_and_never_shortens_long_retry_after() {
    for headers in ["Retry-After: 60\r\n", ""] {
        let f = Fixture::new(vec![response(
            429,
            headers,
            json!({"error":{"code":"rate_limit_exceeded"}}),
        )]);
        let ledger = Arc::new(Ledger::default());
        let started = Instant::now();
        let error = send(&adapter(&f, ledger, Duration::from_millis(400)), false).unwrap_err();
        assert!(error.message.contains("剩余时限"), "{error:?}");
        assert!(
            error.message.contains("status code 429"),
            "retain the actual failure"
        );
        assert!(started.elapsed() < Duration::from_secs(1));
        assert_eq!(f.requests.lock().unwrap().len(), 1);
    }
}

#[test]
fn provider_retry_cancel_during_wait_does_not_send_again() {
    let f = Fixture::new(vec![response(
        429,
        "Retry-After: 60\r\n",
        json!({"error":{"code":"rate_limit_exceeded"}}),
    )]);
    let token = run_context::CancellationToken::default();
    let ledger = Arc::new(Ledger::default());
    let adapter = adapter(&f, ledger.clone(), Duration::from_secs(120));
    adapter.set_run_cancellation(token.clone());
    std::thread::scope(|scope| {
        let worker = scope.spawn(move || send(&adapter, false));
        let deadline = Instant::now() + Duration::from_secs(3);
        while ledger.finishes.lock().unwrap().is_empty() {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        let start = Instant::now();
        token.cancel();
        assert!(worker
            .join()
            .unwrap()
            .unwrap_err()
            .message
            .contains("cancelled"));
        assert!(start.elapsed() < Duration::from_millis(500));
    });
    assert_eq!(f.requests.lock().unwrap().len(), 1);
    assert_eq!(ledger.finishes.lock().unwrap().len(), 1);
}

#[test]
fn provider_retry_later_attempts_share_the_original_deadline() {
    let f = Fixture::new(vec![
        response(
            503,
            "",
            json!({"error":{"message":"overloaded"}})
        );
        4
    ]);
    let ledger = Arc::new(Ledger::default());
    let started = Instant::now();
    let error = send(&adapter(&f, ledger, Duration::from_millis(1800)), false).unwrap_err();
    assert!(error.message.contains("剩余时限"), "{error:?}");
    assert_eq!(f.requests.lock().unwrap().len(), 2);
    assert!(
        started.elapsed() < Duration::from_millis(1800),
        "backoff reset the request deadline"
    );
}

#[test]
fn provider_retry_spend_denial_or_receipt_failure_stops_further_sends() {
    for fail_finish in [false, true] {
        let f = Fixture::new(vec![response(
            503,
            "",
            json!({"error":{"message":"overloaded"}}),
        )]);
        let ledger = Arc::new(Ledger {
            reject_attempt: Some(2),
            fail_finish,
            ..Default::default()
        });
        let error = send(&adapter(&f, ledger.clone(), Duration::from_secs(10)), false).unwrap_err();
        assert_eq!(
            error.spend_stop,
            Some(if fail_finish {
                SpendStop::StorageUnavailable
            } else {
                SpendStop::InsufficientAllowance
            })
        );
        assert_eq!(f.requests.lock().unwrap().len(), 1);
        assert_eq!(ledger.finishes.lock().unwrap().len(), 1);
    }
}

#[test]
fn provider_retry_react_uses_the_same_recovery_path() {
    let f = Fixture::new(vec![
        response(502, "", json!({"error":{"message":"gateway error"}})),
        success(false),
    ]);
    let adapter = ProviderRegistry::adapter_from_config_with_timeout(
        ProviderConfig { options: Default::default(),
            agent_reasoning_effort: Default::default(),
            mode: ProviderMode::ReAct,
            api_key: "fixture-key".into(),
            base_url: f.url.clone(),
            model: "fixture-model".into(),
        },
        Duration::from_secs(10),
    );
    let value = adapter
        .complete_structured(CompletionRequest {
            system: "fixture".into(),
            user: "fixture".into(),
            followup_user: None,
            images: vec![],
            output_token_limit: Some(100),
            reasoning_effort: None,
        })
        .unwrap();
    assert_eq!(value, json!({"ok":true}));
    assert_eq!(f.requests.lock().unwrap().len(), 2);
}
