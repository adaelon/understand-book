//! Real HTTP fixtures for output-budget retries, streaming rollback and settlement.
use crate::{
    model_spend::{ChargeScope, ModelSpendPort, SendIdentity, SendOutcome, SpendStop},
    provider_stream::ModelDelta,
    run_context::CancellationToken,
    AgentRequestPlan, CompletionRequest, Message, ModelAdapter, ProviderConfig, ProviderRegistry,
    ToolSpec,
};
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

const INITIAL_LIMIT: u32 = 131_072;
const RETRY_LIMIT: u32 = 393_216;

#[derive(Default)]
struct Ledger {
    attempts: Mutex<Vec<(SendIdentity, Value)>>,
    finishes: Mutex<Vec<(SendIdentity, SendOutcome)>>,
    reject_attempt: Option<u32>,
    fail_finish: bool,
    cancel_on_finish: Option<CancellationToken>,
}

impl ModelSpendPort for Ledger {
    fn before_send(&self, id: &SendIdentity, request: &Value) -> Result<(), SpendStop> {
        self.attempts.lock().unwrap().push((id.clone(), request.clone()));
        if self.reject_attempt == Some(id.attempt) {
            return Err(SpendStop::InsufficientAllowance);
        }
        Ok(())
    }

    fn after_send(&self, id: &SendIdentity, outcome: &SendOutcome) -> Result<(), SpendStop> {
        self.finishes.lock().unwrap().push((id.clone(), outcome.clone()));
        if let Some(cancellation) = &self.cancel_on_finish {
            cancellation.cancel();
        }
        if self.fail_finish {
            Err(SpendStop::StorageUnavailable)
        } else {
            Ok(())
        }
    }
}

fn accept(listener: &TcpListener) -> TcpStream {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        match listener.accept() {
            Ok((socket, _)) => {
                socket.set_nonblocking(false).unwrap();
                return socket;
            }
            Err(error)
                if error.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < deadline =>
            {
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(error) => panic!("fixture accept: {error}"),
        }
    }
}

fn consume(socket: &mut TcpStream) -> Value {
    socket.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let mut reader = BufReader::new(socket);
    let mut length = 0;
    loop {
        let mut line = String::new();
        assert!(reader.read_line(&mut line).unwrap() > 0, "missing request headers");
        if line == "\r\n" {
            break;
        }
        if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
            length = value.trim().parse().unwrap();
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).unwrap();
    serde_json::from_slice(&body).unwrap()
}

fn response(body: &str, sse: bool) -> String {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        if sse { "text/event-stream" } else { "application/json" },
        body.len(),
    )
}

fn fixture(wires: Vec<String>) -> (String, std::thread::JoinHandle<Vec<Value>>) {
    fixture_with_disconnects(wires.into_iter().map(Some).collect())
}

fn fixture_with_disconnects(wires: Vec<Option<String>>) -> (String, std::thread::JoinHandle<Vec<Value>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let worker = std::thread::spawn(move || {
        wires.into_iter().map(|wire| {
            let mut socket = accept(&listener);
            let request = consume(&mut socket);
            // A cancellation may close the client after reading a complete usage snapshot.
            if let Some(wire) = wire {
                let _ = socket.write_all(wire.as_bytes());
            }
            request
        }).collect()
    });
    (url, worker)
}

fn adapter(mode: &str, url: String, ledger: Arc<Ledger>) -> Box<dyn ModelAdapter + Send> {
    let adapter = ProviderRegistry::adapter_from_config_with_timeout(
        ProviderConfig::from_values(mode, "fixture-key", url, "deepseek-v4-flash").unwrap(),
        Duration::from_secs(3),
    );
    adapter.set_spend_context(
        ChargeScope::ReaderRun { user_id: "reader".into(), run_ref: "output-retry".into() },
        Some(ledger),
    );
    adapter
}

fn request() -> CompletionRequest {
    CompletionRequest {
        followup_user: None,
        images: vec![],
        system: "Return the requested result.".into(),
        user: "Test the output retry.".into(),
        output_token_limit: Some(INITIAL_LIMIT),
        reasoning_effort: Some("low".into()),
    }
}

fn usage(output: u32) -> Value {
    json!({
        "prompt_tokens": 10,
        "completion_tokens": output,
        "completion_tokens_details": {"reasoning_tokens": output.saturating_sub(1)},
        "prompt_cache_hit_tokens": 3,
        "total_tokens": 10 + output,
    })
}

fn json_reply(content: &str, reason: &str, output: u32) -> String {
    response(&json!({
        "choices": [{"index": 0, "message": {"content": content}, "finish_reason": reason}],
        "usage": usage(output),
    }).to_string(), false)
}

fn frame(value: Value) -> String {
    format!("data: {value}\n\n")
}

fn sse_reply(delta: Value, reason: &str, output: u32) -> String {
    let body = frame(json!({"choices": [{"index": 0, "delta": delta, "finish_reason": null}]}))
        + &frame(json!({"choices": [], "usage": usage(1)}))
        + &frame(json!({"choices": [{"index": 0, "delta": {}, "finish_reason": reason}]}))
        // The provider's final usage frame follows finish_reason, including length.
        + &frame(json!({"choices": [], "usage": usage(output)}))
        + "data: [DONE]\n\n";
    response(&body, true)
}

fn assert_two_attempts(ledger: &Ledger, first_output: u32, second_output: u32) {
    let attempts = ledger.attempts.lock().unwrap();
    let finishes = ledger.finishes.lock().unwrap();
    assert_eq!(attempts.len(), 2);
    assert_eq!(finishes.len(), 2);
    assert_eq!(attempts[0].0.logical_call_id, attempts[1].0.logical_call_id);
    assert_ne!(attempts[0].0.call_id, attempts[1].0.call_id);
    assert_eq!((attempts[0].0.attempt, attempts[1].0.attempt), (1, 2));
    assert_eq!(attempts[0].1["max_tokens"], INITIAL_LIMIT);
    assert_eq!(attempts[1].1["max_tokens"], RETRY_LIMIT);
    for (index, output) in [first_output, second_output].into_iter().enumerate() {
        assert_eq!(finishes[index].0.call_id, attempts[index].0.call_id);
        let usage = finishes[index].1.usage.as_ref().unwrap();
        assert_eq!(usage.input_tokens, Some(10));
        assert_eq!(usage.output_tokens, Some(output));
        assert_eq!(usage.total_tokens, Some(10 + output));
    }
}

#[test]
fn output_retry_json_native_and_react_structured_and_chat_expand_once() {
    for mode in ["native", "react"] {
        for entry in ["structured", "chat"] {
            let content = if entry == "structured" { r#"{"ok":true}"# }
                else if mode == "react" { r#"{"final":"complete answer"}"# }
                else { "complete answer" };
            let (url, worker) = fixture(vec![
                json_reply("unfinished", "length", INITIAL_LIMIT),
                json_reply(content, "stop", 5),
            ]);
            let ledger = Arc::new(Ledger::default());
            let adapter = adapter(mode, url, ledger.clone());
            let mut deltas = vec![];
            if entry == "structured" {
                let value = adapter.complete_structured_observed(request(), &mut |delta| deltas.push(delta)).unwrap();
                assert_eq!(value, json!({"ok": true}));
            } else {
                let mut plan = AgentRequestPlan::for_ad_hoc(adapter.model_runtime_profile(), &[Message::user("test")], &[]);
                plan.output_token_limit = Some(INITIAL_LIMIT);
                let turn = adapter.chat_observed(&plan, &mut |delta| deltas.push(delta)).unwrap();
                assert_eq!(turn.text.as_deref(), Some("complete answer"));
                assert_eq!(turn.usage_total_tokens, Some(INITIAL_LIMIT + 25));
            }
            let requests = worker.join().unwrap();
            assert_eq!(requests.len(), 2);
            assert_eq!(requests[0]["messages"], requests[1]["messages"]);
            assert_two_attempts(&ledger, INITIAL_LIMIT, 5);
            assert_eq!(deltas.iter().filter(|delta| matches!(delta, ModelDelta::RetryReset)).count(), 1);
            assert!(!deltas.iter().any(|delta| matches!(delta, ModelDelta::OutputLimitReached { .. })));
            let last_usage = deltas.iter().filter_map(|delta| match delta {
                ModelDelta::Usage(usage) => Some(usage), _ => None,
            }).last().unwrap();
            assert_eq!(last_usage.total_tokens, Some(INITIAL_LIMIT + 25));
            assert_eq!(last_usage.output_tokens, Some(INITIAL_LIMIT + 5));
        }
    }
}

#[test]
fn output_retry_sse_reads_usage_tail_and_replaces_snapshots_across_reset() {
    let (url, worker) = fixture(vec![
        sse_reply(json!({"content": "{\"unfinished\":"}), "length", INITIAL_LIMIT),
        sse_reply(json!({"content": "{\"ok\":true}"}), "stop", 5),
    ]);
    let ledger = Arc::new(Ledger::default());
    let adapter = adapter("native", url, ledger.clone());
    let mut visible = String::new();
    let mut snapshots = vec![];
    let mut reset_count = 0;
    let value = adapter.complete_structured_observed(request(), &mut |delta| match delta {
        ModelDelta::Text(text) => visible.push_str(&text),
        ModelDelta::RetryReset => { visible.clear(); reset_count += 1; }
        ModelDelta::Usage(usage) => snapshots.push(usage),
        _ => {}
    }).unwrap();
    worker.join().unwrap();
    assert_eq!(value, json!({"ok": true}));
    assert_eq!(visible, r#"{"ok":true}"#);
    assert_eq!(reset_count, 1);
    assert_two_attempts(&ledger, INITIAL_LIMIT, 5);
    assert!(snapshots.iter().any(|usage| usage.total_tokens == Some(INITIAL_LIMIT + 10)), "missing first attempt's final usage after length");
    let final_usage = snapshots.last().unwrap();
    assert_eq!(final_usage.input_tokens, Some(20));
    assert_eq!(final_usage.output_tokens, Some(INITIAL_LIMIT + 5));
    assert_eq!(final_usage.total_tokens, Some(INITIAL_LIMIT + 25));
    assert_eq!(final_usage.reasoning_output_tokens, Some(INITIAL_LIMIT + 3));
    assert_eq!(final_usage.cached_input_tokens, Some(6));
}

#[test]
fn output_retry_sse_discards_truncated_tool_arguments_before_new_attempt() {
    let (url, worker) = fixture(vec![
        sse_reply(json!({"tool_calls": [{"index": 0, "id": "old", "type": "function", "function": {"name": "inspect", "arguments": "{\"value\":\"unfinished"}}]}), "length", INITIAL_LIMIT),
        sse_reply(json!({"tool_calls": [{"index": 0, "id": "new", "type": "function", "function": {"name": "inspect", "arguments": "{\"value\":\"complete\"}"}}]}), "tool_calls", 5),
    ]);
    let ledger = Arc::new(Ledger::default());
    let adapter = adapter("native", url, ledger.clone());
    let mut plan = AgentRequestPlan::for_ad_hoc(adapter.model_runtime_profile(), &[Message::user("inspect")], &[ToolSpec {
        name: "inspect".into(), description: "Inspect a value".into(),
        parameters: json!({"type": "object", "properties": {"value": {"type": "string"}}, "required": ["value"]}),
    }]);
    plan.output_token_limit = Some(INITIAL_LIMIT);
    let mut visible_arguments = String::new();
    let turn = adapter.chat_observed(&plan, &mut |delta| match delta {
        ModelDelta::ToolArguments { arguments, .. } => visible_arguments.push_str(&arguments),
        ModelDelta::RetryReset => visible_arguments.clear(),
        _ => {}
    }).unwrap();
    worker.join().unwrap();
    assert_eq!(turn.tool_calls.len(), 1);
    assert_eq!(turn.tool_calls[0].id, "new");
    assert_eq!(turn.tool_calls[0].name, "inspect");
    assert_eq!(turn.tool_calls[0].arguments, r#"{"value":"complete"}"#);
    assert_eq!(visible_arguments, r#"{"value":"complete"}"#);
    assert_eq!(turn.usage_total_tokens, Some(INITIAL_LIMIT + 25));
    assert_two_attempts(&ledger, INITIAL_LIMIT, 5);
}

#[test]
fn output_retry_second_length_stops_without_third_transport() {
    for mode in ["native", "react"] {
        let (url, worker) = fixture(vec![
            json_reply("unfinished", "length", INITIAL_LIMIT),
            json_reply("still unfinished", "length", RETRY_LIMIT),
        ]);
        let ledger = Arc::new(Ledger::default());
        let adapter = adapter(mode, url, ledger.clone());
        let error = adapter.complete_structured(request()).unwrap_err();
        worker.join().unwrap();
        assert!(error.message.contains("length"), "{error:?}");
        assert_two_attempts(&ledger, INITIAL_LIMIT, RETRY_LIMIT);
    }
}

#[test]
fn output_retry_missing_attempt_usage_keeps_the_logical_total_unknown() {
    for missing_attempt in [0, 1] {
        let mut wires = vec![
            json_reply("unfinished", "length", INITIAL_LIMIT),
            json_reply("complete", "stop", 5),
        ];
        wires[missing_attempt] = response(&json!({"choices":[{
            "message":{"content": if missing_attempt == 0 {"unfinished"} else {"complete"}},
            "finish_reason": if missing_attempt == 0 {"length"} else {"stop"},
        }]}).to_string(), false);
        let (url, worker) = fixture(wires);
        let ledger = Arc::new(Ledger::default());
        let adapter = adapter("native", url, ledger.clone());
        let plan = AgentRequestPlan::for_ad_hoc(adapter.model_runtime_profile(), &[Message::user("test")], &[]);
        let mut final_usage = None;
        let turn = adapter.chat_observed(&plan, &mut |delta| {
            if let ModelDelta::Usage(usage) = delta { final_usage = Some(usage); }
        }).unwrap();
        worker.join().unwrap();
        assert_eq!(turn.text.as_deref(), Some("complete"));
        assert_eq!(turn.usage_total_tokens, None);
        let usage = final_usage.unwrap();
        assert_eq!(usage.total_tokens, None);
        assert_eq!(usage.input_tokens, None);
        assert_eq!(usage.output_tokens, None);
        let finishes = ledger.finishes.lock().unwrap();
        assert!(finishes[missing_attempt].1.usage.is_none());
        assert!(finishes[1 - missing_attempt].1.usage.is_some());
    }
}

#[test]
fn output_retry_respects_an_explicit_smaller_evaluation_budget() {
    let (url, worker) = fixture(vec![json_reply("unfinished", "length", 8_000)]);
    let ledger = Arc::new(Ledger::default());
    let adapter = adapter("native", url, ledger.clone());
    let mut request = request();
    request.output_token_limit = Some(8_000);
    let mut resets = 0;
    let mut limits = vec![];
    let error = adapter.complete_structured_observed(request, &mut |delta| {
        if matches!(delta, ModelDelta::RetryReset) { resets += 1; }
        if let ModelDelta::OutputLimitReached { total_tokens } = delta { limits.push(total_tokens); }
    }).unwrap_err();
    assert!(error.message.contains("length"));
    let requests = worker.join().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0]["max_tokens"], 8_000);
    assert_eq!(ledger.attempts.lock().unwrap().len(), 1);
    assert_eq!(ledger.finishes.lock().unwrap().len(), 1);
    assert_eq!(resets, 0);
    assert_eq!(limits, [Some(8_010)]);
}

#[test]
fn output_retry_transport_retries_keep_independent_budget_and_settlement() {
    let (url, worker) = fixture_with_disconnects(vec![
        None,
        Some(json_reply("unfinished", "length", INITIAL_LIMIT)),
        Some(json_reply(r#"{"ok":true}"#, "stop", 5)),
    ]);
    let ledger = Arc::new(Ledger::default());
    let adapter = adapter("native", url, ledger.clone());
    let mut deltas = vec![];
    let value = adapter.complete_structured_observed(request(), &mut |delta| deltas.push(delta)).unwrap();
    assert_eq!(value, json!({"ok": true}));
    assert_eq!(worker.join().unwrap().len(), 3);
    let attempts = ledger.attempts.lock().unwrap();
    let finishes = ledger.finishes.lock().unwrap();
    assert_eq!(attempts.len(), 3);
    assert_eq!(finishes.len(), 3);
    for (index, (identity, body)) in attempts.iter().enumerate() {
        assert_eq!(identity.attempt, index as u32 + 1);
        assert_eq!(identity.logical_call_id, attempts[0].0.logical_call_id);
        assert_eq!(body["max_tokens"], if index < 2 { INITIAL_LIMIT } else { RETRY_LIMIT });
        assert_eq!(finishes[index].0.call_id, identity.call_id);
    }
    assert!(finishes[0].1.usage.is_none());
    assert_eq!(finishes[1].1.usage.as_ref().unwrap().total_tokens, Some(INITIAL_LIMIT + 10));
    assert_eq!(finishes[2].1.usage.as_ref().unwrap().total_tokens, Some(15));
    assert_eq!(deltas.iter().filter(|delta| matches!(delta, ModelDelta::RetryReset)).count(), 1);
    let final_usage = deltas.iter().filter_map(|delta| match delta {
        ModelDelta::Usage(usage) => Some(usage), _ => None,
    }).last().unwrap();
    assert_eq!(final_usage.total_tokens, None, "the disconnected attempt may have incurred unreported usage");
}

#[test]
fn output_retry_cancellation_denial_and_failed_settlement_stop_before_resend() {
    for stop in ["cancel", "deny", "settlement"] {
        let (url, worker) = fixture(vec![json_reply("unfinished", "length", INITIAL_LIMIT)]);
        let cancellation = CancellationToken::default();
        let ledger = Arc::new(Ledger {
            reject_attempt: (stop == "deny").then_some(2),
            fail_finish: stop == "settlement",
            cancel_on_finish: (stop == "cancel").then_some(cancellation.clone()),
            ..Default::default()
        });
        let adapter = adapter("native", url, ledger.clone());
        adapter.set_run_cancellation(cancellation);
        let error = adapter.complete_structured(request()).unwrap_err();
        assert_eq!(worker.join().unwrap().len(), 1);
        let attempts = ledger.attempts.lock().unwrap();
        assert_eq!(attempts.len(), if stop == "deny" { 2 } else { 1 });
        let finishes = ledger.finishes.lock().unwrap();
        assert_eq!(finishes.len(), 1);
        assert_eq!(finishes[0].1.usage.as_ref().unwrap().total_tokens, Some(INITIAL_LIMIT + 10));
        match stop {
            "deny" => assert_eq!(error.spend_stop, Some(SpendStop::InsufficientAllowance)),
            "settlement" => assert_eq!(error.spend_stop, Some(SpendStop::StorageUnavailable)),
            _ => assert!(error.message.contains("cancel") || error.message.contains("取消"), "{error:?}"),
        }
    }
}
