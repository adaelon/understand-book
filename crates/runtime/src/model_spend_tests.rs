use super::*;
use crate::{
    AgentRequestPlan, CompletionRequest, Message, ModelAdapter, ProviderConfig, ProviderRegistry,
};
use serde_json::json;
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[derive(Default)]
struct Ledger {
    starts: Mutex<Vec<(SendIdentity, Value)>>,
    finishes: Mutex<Vec<(SendIdentity, SendOutcome)>>,
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
    fn after_send(&self, id: &SendIdentity, outcome: &SendOutcome) -> Result<(), SpendStop> {
        self.finishes
            .lock()
            .unwrap()
            .push((id.clone(), outcome.clone()));
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
            Ok((s, _)) => return s,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(5))
            }
            Err(e) => panic!("accept: {e}"),
        }
    }
}
fn consume(stream: &mut TcpStream) -> Value {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut reader = BufReader::new(stream);
    let mut length = 0;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        if line == "\r\n" {
            break;
        }
        if let Some(v) = line.to_lowercase().strip_prefix("content-length:") {
            length = v.trim().parse().unwrap();
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).unwrap();
    serde_json::from_slice(&body).unwrap()
}
fn response(body: &str, status: u16, sse: bool) -> String {
    format!("HTTP/1.1 {status} Fixture\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",if sse {"text/event-stream"} else {"application/json"},body.len())
}
fn fixture(wires: Vec<Option<String>>) -> (String, std::thread::JoinHandle<Vec<Value>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let worker = std::thread::spawn(move || {
        wires
            .into_iter()
            .map(|wire| {
                let mut socket = accept(&listener);
                let body = consume(&mut socket);
                if let Some(wire) = wire {
                    socket.write_all(wire.as_bytes()).unwrap();
                }
                body
            })
            .collect()
    });
    (url, worker)
}
fn request() -> CompletionRequest {
    CompletionRequest {
        system: "test".into(),
        user: "test".into(),
        output_token_limit: Some(100),
        reasoning_effort: None,
    }
}
fn adapter(mode: &str, url: String, ledger: Arc<Ledger>) -> Box<dyn ModelAdapter + Send> {
    let adapter = ProviderRegistry::adapter_from_config_with_timeout(
        ProviderConfig::from_values(mode, "fixture-key", url, "fixture-model").unwrap(),
        Duration::from_secs(3),
    );
    adapter.set_spend_context(
        ChargeScope::ReaderRun {
            user_id: "reader".into(),
            run_ref: "turn".into(),
        },
        Some(ledger),
    );
    adapter
}
fn json_reply(content: &str) -> String {
    response(&json!({"choices":[{"message":{"content":content},"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":5,"prompt_cache_hit_tokens":3,"total_tokens":15}}).to_string(),200,false)
}

#[test]
fn adm4_transport_retry_has_two_identities_and_no_usage_cross_contamination() {
    for mode in ["native", "react"] {
        let (url, worker) = fixture(vec![None, Some(json_reply(r#"{"ok":true}"#))]);
        let ledger = Arc::new(Ledger::default());
        let adapter = adapter(mode, url, ledger.clone());
        assert_eq!(adapter.complete_structured(request()).unwrap()["ok"], true);
        let requests = worker.join().unwrap();
        assert_eq!(requests[0], requests[1]);
        let starts = ledger.starts.lock().unwrap();
        let finishes = ledger.finishes.lock().unwrap();
        assert_eq!(starts.len(), 2);
        assert_eq!(finishes.len(), 2);
        assert_ne!(starts[0].0.call_id, starts[1].0.call_id);
        assert_eq!(starts[0].0.logical_call_id, starts[1].0.logical_call_id);
        assert_eq!((starts[0].0.attempt, starts[1].0.attempt), (1, 2));
        assert_eq!(finishes[0].1.evidence, SendEvidence::OutcomeUnknown);
        assert!(finishes[0].1.usage.is_none());
        assert_eq!(finishes[1].1.usage.as_ref().unwrap().total_tokens, Some(15));
        assert_eq!(
            starts[0].0.scope,
            Some(ChargeScope::ReaderRun {
                user_id: "reader".into(),
                run_ref: "turn".into()
            })
        );
    }
}
#[test]
fn adm4_usage_snapshots_survive_truncation_error_status_and_cancellation() {
    for mode in ["truncated", "http-error", "cancelled", "complete"] {
        let first="data: {\"choices\":[],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":1,\"total_tokens\":11}}\n\n";
        let last="data: {\"choices\":[],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":5,\"total_tokens\":15}}\n\n";
        let wire = if mode == "http-error" {
            response(
                r#"{"error":{"message":"failed"},"usage":{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15}}"#,
                500,
                false,
            )
        } else {
            response(
                &(first.to_owned()
                    + last
                    + if mode == "complete" {
                        "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"{}\"},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n"
                    } else {
                        ""
                    }),
                200,
                true,
            )
        };
        let (url, worker) = fixture(vec![Some(wire)]);
        let ledger = Arc::new(Ledger::default());
        let adapter = adapter("native", url, ledger.clone());
        let cancellation = crate::run_context::CancellationToken::default();
        adapter.set_run_cancellation(cancellation.clone());
        let result = adapter.complete_structured_observed(request(), &mut |delta| {
            if mode == "cancelled" && matches!(delta, crate::provider_stream::ModelDelta::Usage(_))
            {
                cancellation.cancel();
            }
        });
        assert_eq!(result.is_ok(), mode == "complete");
        worker.join().unwrap();
        let finishes = ledger.finishes.lock().unwrap();
        assert_eq!(finishes.len(), 1);
        assert_eq!(
            finishes[0].1.usage.as_ref().unwrap().total_tokens,
            Some(if mode == "cancelled" { 11 } else { 15 })
        );
    }
}
#[test]
fn adm4_denial_missing_scope_and_failed_settlement_prevent_send_or_retry() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let ledger = Arc::new(Ledger {
        reject_attempt: Some(1),
        ..Default::default()
    });
    let a = adapter("native", url, ledger.clone());
    assert_eq!(
        a.complete_structured(request()).unwrap_err().message,
        SpendStop::InsufficientAllowance.code()
    );
    a.set_spend_context(
        ChargeScope::ReaderTask {
            user_id: String::new(),
            task_ref: "task".into(),
        },
        Some(ledger),
    );
    assert_eq!(
        a.complete_structured(request()).unwrap_err().message,
        SpendStop::MissingScope.code()
    );
    assert!(listener.accept().is_err());
    for failed_settlement in [false, true] {
        let (url, worker) = fixture(vec![None]);
        let ledger = Arc::new(Ledger {
            reject_attempt: Some(2),
            fail_finish: failed_settlement,
            ..Default::default()
        });
        let a = adapter("native", url, ledger.clone());
        let err = a.complete_structured(request()).unwrap_err();
        worker.join().unwrap();
        assert_eq!(
            err.message,
            if failed_settlement {
                SpendStop::StorageUnavailable
            } else {
                SpendStop::InsufficientAllowance
            }
            .code()
        );
        assert_eq!(ledger.starts.lock().unwrap().len(), 1);
    }
}
#[test]
fn adm4_all_entrypoints_and_nested_purposes_reach_same_port() {
    for mode in ["native", "react"] {
        for entry in ["chat", "complete", "structured"] {
            let content = match entry {
                "complete" => {
                    r#"{"sufficient":true,"answer":"ok","citations":[],"model_supplement":[]}"#
                }
                "chat" if mode == "react" => r#"{"final":"ok"}"#,
                "chat" => "ok",
                _ => r#"{"ok":true}"#,
            };
            let (url, worker) = fixture(vec![Some(json_reply(content))]);
            let ledger = Arc::new(Ledger::default());
            let adapter = adapter(mode, url, ledger.clone());
            let observed = crate::run_events::ObservedAdapter {
                inner: adapter.as_ref(),
                events: Default::default(),
                cancellation: Default::default(),
                runtime_profile: adapter.model_runtime_profile(),
            };
            let _scope = crate::run_events::purpose(
                &observed,
                if entry == "structured" {
                    "compaction"
                } else {
                    "query"
                },
            );
            match entry {
                "complete" => {
                    observed.complete(request()).unwrap();
                }
                "structured" => {
                    observed.complete_structured(request()).unwrap();
                }
                _ => {
                    let mut plan = AgentRequestPlan::for_ad_hoc(
                        adapter.model_runtime_profile(),
                        &[Message::user("test")],
                        &[],
                    );
                    plan.output_token_limit = Some(100);
                    plan.preview_images
                        .push(crate::presentation_author::PreviewImage {
                            caption: "image".into(),
                            png_base64: "test".into(),
                            candidate_id: None,
                            environment_name: None,
                        });
                    observed.chat(&plan).unwrap();
                }
            }
            worker.join().unwrap();
            let starts = ledger.starts.lock().unwrap();
            assert_eq!(starts.len(), 1);
            assert_eq!(
                starts[0].0.purpose,
                if entry == "structured" {
                    "compaction"
                } else {
                    "query"
                }
            );
            assert_eq!(starts[0].1["max_tokens"], 100);
            if entry == "chat" {
                assert!(starts[0]
                    .1
                    .to_string()
                    .contains("data:image/png;base64,test"));
            }
        }
    }
}

#[test]
fn adm6_native_and_react_preserve_typed_denial_before_any_http() {
    struct Denied(SpendStop);
    impl ModelSpendPort for Denied {
        fn before_send(&self, _: &SendIdentity, _: &Value) -> Result<(), SpendStop> {
            Err(self.0)
        }
        fn after_send(&self, _: &SendIdentity, _: &SendOutcome) -> Result<(), SpendStop> {
            panic!("denied call was sent")
        }
    }
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    for mode in ["native", "react"] {
        for reason in [
            SpendStop::MissingScope,
            SpendStop::RateUnavailable,
            SpendStop::InsufficientAllowance,
            SpendStop::AllowanceExpired,
            SpendStop::StorageUnavailable,
            SpendStop::PermissionRevoked,
            SpendStop::ReconciliationRequired,
        ] {
            let adapter = ProviderRegistry::adapter_from_config(
                ProviderConfig::from_values(
                    mode,
                    "fixture-key",
                    format!("http://{}", listener.local_addr().unwrap()),
                    "fixture-model",
                )
                .unwrap(),
            );
            adapter.set_spend_context(
                ChargeScope::ReaderRun {
                    user_id: "A".into(),
                    run_ref: "run".into(),
                },
                Some(Arc::new(Denied(reason))),
            );
            assert_eq!(
                adapter.complete(request()).unwrap_err().spend_stop,
                Some(reason)
            );
            assert_eq!(
                adapter
                    .complete_structured(request())
                    .unwrap_err()
                    .spend_stop,
                Some(reason)
            );
            let mut plan = AgentRequestPlan::for_ad_hoc(
                adapter.model_runtime_profile(),
                &[Message::user("observe preview")],
                &[],
            );
            plan.preview_images
                .push(crate::presentation_author::PreviewImage {
                    caption: "presentation preview".into(),
                    png_base64: "fixture".into(),
                    candidate_id: None,
                    environment_name: None,
                });
            assert_eq!(adapter.chat(&plan).unwrap_err().spend_stop, Some(reason));
        }
    }
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}
