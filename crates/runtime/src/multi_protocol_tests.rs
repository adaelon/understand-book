//! Real TCP coverage of the shared adapter entry point and per-send settlement.
//! These fixtures exercise wire contracts; they do not claim real-service acceptance.
use crate::{
    model_settings::{ApiProtocol, AuthMethod, ModelBinding, ModelCapabilities, ProviderOptions},
    model_spend::{ChargeScope, ModelSpendPort, SendEvidence, SendIdentity, SendOutcome, SpendStop},
    provider_stream::{self, ModelDelta},
    AgentRequestPlan, CompletionRequest, Message, ModelAdapter, ProviderConfig, ProviderMode,
    ProviderRegistry, Role, ToolSpec,
};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, VecDeque},
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex},
    time::Duration,
};

const PROTOCOLS: [ApiProtocol; 7] = [
    ApiProtocol::OpenaiResponses, ApiProtocol::AnthropicMessages,
    ApiProtocol::GeminiGenerateContent, ApiProtocol::GeminiInteractions,
    ApiProtocol::OllamaChat, ApiProtocol::CohereV2Chat, ApiProtocol::BedrockConverse,
];
const ANSWER: &str = "Page 1 explains the source.";
const OUTPUT_LIMIT: u32 = 128;

#[derive(Debug, Clone)]
struct CapturedRequest {
    target: String,
    headers: BTreeMap<String, String>,
    body: Value,
}

struct Fixture {
    url: String,
    requests: Arc<Mutex<Vec<CapturedRequest>>>,
    stop: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl Fixture {
    fn new(responses: Vec<String>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let captured = requests.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let worker = std::thread::spawn(move || {
            let mut responses: VecDeque<_> = responses.into();
            while !stopped.load(Ordering::Acquire) {
                let (mut socket, _) = match listener.accept() {
                    Ok(socket) => socket,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(2)); continue;
                    }
                    Err(error) => panic!("protocol fixture accept: {error}"),
                };
                socket.set_nonblocking(false).unwrap();
                socket.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
                let mut reader = BufReader::new(&mut socket);
                let mut line = String::new();
                assert!(reader.read_line(&mut line).unwrap() > 0);
                let target = line.split_whitespace().nth(1).unwrap().to_owned();
                let mut headers = BTreeMap::new();
                loop {
                    line.clear();
                    assert!(reader.read_line(&mut line).unwrap() > 0);
                    if line == "\r\n" { break; }
                    let (key, value) = line.split_once(':').unwrap();
                    headers.insert(key.to_ascii_lowercase(), value.trim().to_owned());
                }
                let length: usize = headers.get("content-length").unwrap().parse().unwrap();
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                captured.lock().unwrap().push(CapturedRequest {
                    target, headers, body: serde_json::from_slice(&body).unwrap(),
                });
                drop(reader);
                let response = responses.pop_front().unwrap_or_else(|| wire(400, "application/json", &json!({"error":{"message":"Unexpected additional model send"}}).to_string()));
                let _ = socket.write_all(response.as_bytes());
            }
        });
        Self { url, requests, stop, worker: Some(worker) }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.worker.take().unwrap().join().unwrap();
    }
}

#[derive(Default)]
struct Ledger {
    starts: Mutex<Vec<(SendIdentity, Value)>>,
    finishes: Mutex<Vec<(SendIdentity, SendOutcome)>>,
}
impl ModelSpendPort for Ledger {
    fn before_send(&self, identity: &SendIdentity, request: &Value) -> Result<(), SpendStop> {
        self.starts.lock().unwrap().push((identity.clone(), request.clone())); Ok(())
    }
    fn after_send(&self, identity: &SendIdentity, outcome: &SendOutcome) -> Result<(), SpendStop> {
        self.finishes.lock().unwrap().push((identity.clone(), outcome.clone())); Ok(())
    }
}

fn wire(status: u16, content_type: &str, body: &str) -> String {
    format!("HTTP/1.1 {status} Fixture\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len())
}
fn sse(values: Vec<Value>) -> String {
    values.into_iter().map(|v| format!("data: {v}\n\n")).collect()
}

fn adapter(protocol: ApiProtocol, f: &Fixture, ledger: Arc<Ledger>) -> Box<dyn ModelAdapter + Send> {
    let auth = match protocol {
        ApiProtocol::OllamaChat => AuthMethod::None,
        ApiProtocol::AnthropicMessages | ApiProtocol::GeminiGenerateContent | ApiProtocol::GeminiInteractions => AuthMethod::ApiKey,
        _ => AuthMethod::Bearer,
    };
    let config = ProviderConfig {
        mode: ProviderMode::Native,
        api_key: if auth == AuthMethod::None { "".into() } else { "fixture-key".into() },
        base_url: f.url.clone(), model: "fixture-model".into(), agent_reasoning_effort: Default::default(),
        options: ProviderOptions {
            api_protocol: protocol, auth,
            binding: Some(ModelBinding { service_id: "fixture-service".into(), model_config_id: protocol.as_str().into(), revision: 1 }),
            output_token_limit: Some(OUTPUT_LIMIT),
            capabilities: ModelCapabilities { native_tools: Some(true), json_object: Some(true), ..Default::default() },
            ..Default::default()
        },
    };
    let adapter = ProviderRegistry::adapter_from_config_with_timeout(config, Duration::from_secs(4));
    adapter.set_spend_context(ChargeScope::ReaderRun { user_id: "reader".into(), run_ref: "multi-protocol".into() }, Some(ledger));
    adapter
}

fn tools() -> Vec<ToolSpec> {
    vec![ToolSpec { name: "read".into(), description: "Read a page of the source.".into(),
        parameters: json!({"type":"object","properties":{"page":{"type":"integer"}},"required":["page"],"additionalProperties":false}) }]
}
fn plan(adapter: &dyn ModelAdapter, messages: &[Message]) -> AgentRequestPlan {
    AgentRequestPlan::for_ad_hoc(adapter.model_runtime_profile(), messages, &tools())
}
fn structured_request() -> CompletionRequest {
    CompletionRequest { system: "Return exactly one JSON object with ok=true.".into(), user: "Report readiness.".into(),
        images: vec![], followup_user: None, output_token_limit: Some(10_000), reasoning_effort: None }
}

/// `text == None` means a tool request; otherwise a completed text response.
fn success(protocol: ApiProtocol, text: Option<&str>) -> String {
    match protocol {
        ApiProtocol::OpenaiResponses => {
            let item = match text {
                Some(text) => json!({"type":"message","id":"msg-1","role":"assistant","content":[{"type":"output_text","text":text,"annotations":[]}]}),
                None => json!({"type":"function_call","id":"fc-1","call_id":"call-1","name":"read","arguments":"{\"page\":1}"}),
            };
            let mut events = vec![json!({"type":"response.output_item.added","output_index":0,"item":if text.is_some() { json!({"type":"message","id":"msg-1","role":"assistant","content":[]}) } else { json!({"type":"function_call","id":"fc-1","call_id":"call-1","name":"read","arguments":""}) }})];
            if let Some(text) = text {
                events.push(json!({"type":"response.output_text.delta","output_index":0,"content_index":0,"item_id":"msg-1","delta":text}));
            } else {
                events.push(json!({"type":"response.function_call_arguments.delta","output_index":0,"item_id":"fc-1","delta":"{\"page\":"}));
                events.push(json!({"type":"response.function_call_arguments.delta","output_index":0,"item_id":"fc-1","delta":"1}"}));
            }
            events.push(json!({"type":"response.completed","response":{"id":"resp-1","status":"completed","output":[item],"usage":{"input_tokens":10,"output_tokens":3,"total_tokens":13}}}));
            wire(200, "text/event-stream", &sse(events))
        }
        ApiProtocol::AnthropicMessages => {
            let block = if text.is_some() { json!({"type":"text","text":""}) } else { json!({"type":"tool_use","id":"call-1","name":"read","input":{}}) };
            let delta = match text { Some(text) => json!({"type":"text_delta","text":text}), None => json!({"type":"input_json_delta","partial_json":"{\"page\":1}"}) };
            let events = vec![
                json!({"type":"message_start","message":{"id":"msg-1","type":"message","role":"assistant","model":"fixture-model","content":[],"stop_reason":null,"usage":{"input_tokens":10,"output_tokens":0,"cache_read_input_tokens":0,"cache_creation_input_tokens":0}}}),
                json!({"type":"content_block_start","index":0,"content_block":block}),
                json!({"type":"content_block_delta","index":0,"delta":delta}),
                json!({"type":"content_block_stop","index":0}),
                json!({"type":"message_delta","delta":{"stop_reason":if text.is_some(){"end_turn"}else{"tool_use"},"stop_sequence":null},"usage":{"output_tokens":3}}),
                json!({"type":"message_stop"}),
            ];
            wire(200, "text/event-stream", &sse(events))
        }
        ApiProtocol::OllamaChat => {
            let message = match text {
                Some(text) => json!({"role":"assistant","content":text}),
                None => json!({"role":"assistant","content":"","tool_calls":[{"function":{"name":"read","arguments":{"page":1}}}]}),
            };
            let body = format!("{}\n", json!({"message":message,"done":true,"done_reason":"stop","prompt_eval_count":10,"eval_count":3}));
            wire(200, "application/x-ndjson", &body)
        }
        ApiProtocol::CohereV2Chat => {
            let mut events = vec![json!({"type":"message-start","id":"msg-1","delta":{"message":{"role":"assistant","content":[]}}})];
            if let Some(text) = text {
                events.extend([
                    json!({"type":"content-start","index":0,"delta":{"message":{"content":{"type":"text","text":""}}}}),
                    json!({"type":"content-delta","index":0,"delta":{"message":{"content":{"text":text}}}}),
                    json!({"type":"content-end","index":0}),
                ]);
            } else {
                events.extend([
                    json!({"type":"tool-call-start","index":0,"delta":{"message":{"tool_calls":{"id":"call-1","type":"function","function":{"name":"read","arguments":"{\"page\":1}"}}}}}),
                    json!({"type":"tool-call-end","index":0}),
                ]);
            }
            events.push(json!({"type":"message-end","delta":{"finish_reason":if text.is_some(){"COMPLETE"}else{"TOOL_CALL"},"usage":{"tokens":{"input_tokens":10,"output_tokens":3},"billed_units":{"input_tokens":7,"output_tokens":3}}}}));
            wire(200, "text/event-stream", &sse(events))
        }
        ApiProtocol::BedrockConverse => {
            let block = match text { Some(text) => json!({"text":text}), None => json!({"toolUse":{"toolUseId":"call-1","name":"read","input":{"page":1}}}) };
            wire(200, "application/json", &json!({"output":{"message":{"role":"assistant","content":[block]}},"stopReason":if text.is_some(){"end_turn"}else{"tool_use"},"usage":{"inputTokens":10,"outputTokens":3,"totalTokens":13}}).to_string())
        }
        ApiProtocol::GeminiGenerateContent | ApiProtocol::GeminiInteractions => gemini_success(protocol, text),
        _ => unreachable!(),
    }
}

fn gemini_success(protocol: ApiProtocol, text: Option<&str>) -> String {
    match protocol {
        ApiProtocol::GeminiGenerateContent => {
            let part = match text { Some(text) => json!({"text":text}), None => json!({"functionCall":{"id":"call-1","name":"read","args":{"page":1}},"thoughtSignature":"fixture-signature"}) };
            wire(200, "text/event-stream", &sse(vec![json!({"candidates":[{"index":0,"content":{"role":"model","parts":[part]},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":10,"candidatesTokenCount":3,"totalTokenCount":13}})]))
        }
        ApiProtocol::GeminiInteractions => {
            let step = match text { Some(text) => json!({"type":"model_output","content":[{"type":"text","text":text}]}), None => json!({"type":"function_call","id":"call-1","name":"read","arguments":{"page":1}}) };
            wire(200, "application/json", &json!({"id":"interaction-1","status":if text.is_some(){"completed"}else{"requires_action"},"steps":[step],"usage":{"total_input_tokens":10,"total_output_tokens":3,"total_tokens":13}}).to_string())
        }
        _ => unreachable!(),
    }
}

fn assert_wire(protocol: ApiProtocol, requests: &[CapturedRequest], call_id: &str) {
    assert_eq!(requests.len(), 3, "{protocol:?}: tool + answer + structured should send exactly three requests");
    for request in requests {
        let path = request.target.split('?').next().unwrap();
        let expected = match protocol {
            ApiProtocol::OpenaiResponses => "/v1/responses",
            ApiProtocol::AnthropicMessages => "/v1/messages",
            ApiProtocol::GeminiGenerateContent => "/v1beta/models/fixture-model:streamGenerateContent",
            ApiProtocol::GeminiInteractions => "/v1beta/interactions",
            ApiProtocol::OllamaChat => "/api/chat",
            ApiProtocol::CohereV2Chat => "/v2/chat",
            ApiProtocol::BedrockConverse => "/model/fixture-model/converse-stream",
            _ => unreachable!(),
        };
        assert_eq!(path, expected, "selected protocol must determine the endpoint");
        match protocol {
            ApiProtocol::OllamaChat => assert!(!request.headers.contains_key("authorization") && !request.headers.contains_key("x-api-key")),
            ApiProtocol::AnthropicMessages => assert_eq!(request.headers.get("x-api-key").map(String::as_str), Some("fixture-key")),
            ApiProtocol::GeminiGenerateContent | ApiProtocol::GeminiInteractions => assert_eq!(request.headers.get("x-goog-api-key").map(String::as_str), Some("fixture-key")),
            _ => assert_eq!(request.headers.get("authorization").map(String::as_str), Some("Bearer fixture-key")),
        }
        let limit = match protocol {
            ApiProtocol::OpenaiResponses => &request.body["max_output_tokens"],
            ApiProtocol::GeminiGenerateContent => &request.body["generationConfig"]["maxOutputTokens"],
            ApiProtocol::GeminiInteractions => &request.body["generation_config"]["max_output_tokens"],
            ApiProtocol::OllamaChat => &request.body["options"]["num_predict"],
            ApiProtocol::BedrockConverse => &request.body["inferenceConfig"]["maxTokens"],
            _ => &request.body["max_tokens"],
        };
        assert_eq!(limit, &json!(OUTPUT_LIMIT), "{protocol:?}: configured cap also limits explicit completion budgets");
    }
    let second = &requests[1].body;
    let structured = &requests[2].body;
    match protocol {
        ApiProtocol::OpenaiResponses => {
            assert_eq!(requests[0].body["store"], false);
            assert_eq!(requests[0].body["tools"][0]["name"], "read");
            let input = second["input"].as_array().unwrap();
            assert!(input.iter().any(|i| i["type"] == "function_call" && i["call_id"] == call_id));
            assert!(input.iter().any(|i| i["type"] == "function_call_output" && i["call_id"] == call_id));
            assert_eq!(structured["text"]["format"]["type"], "json_object");
        }
        ApiProtocol::AnthropicMessages => {
            assert!(!requests[0].body["system"].is_null());
            assert_eq!(requests[0].body["tools"][0]["input_schema"]["required"], json!(["page"]));
            let blocks: Vec<_> = second["messages"].as_array().unwrap().iter().flat_map(|m| m["content"].as_array().unwrap()).collect();
            assert!(blocks.iter().any(|b| b["type"] == "tool_use" && b["id"] == call_id));
            assert!(blocks.iter().any(|b| b["type"] == "tool_result" && b["tool_use_id"] == call_id));
        }
        ApiProtocol::OllamaChat => {
            let messages = second["messages"].as_array().unwrap();
            let assistant = messages.iter().find(|m| m["role"] == "assistant").unwrap();
            assert_eq!(assistant["tool_calls"][0]["function"]["arguments"], json!({"page":1}));
            let tool = messages.iter().find(|m| m["role"] == "tool").unwrap();
            assert_eq!(tool["tool_name"], "read"); assert!(tool.get("tool_call_id").is_none());
            assert_eq!(structured["format"], "json");
        }
        ApiProtocol::CohereV2Chat => {
            let tool = second["messages"].as_array().unwrap().iter().find(|m| m["role"] == "tool").unwrap();
            assert_eq!(tool["tool_call_id"], call_id); assert_eq!(tool["content"][0]["type"], "text");
            assert_eq!(structured["response_format"]["type"], "json_object"); assert!(structured.get("tools").is_none());
        }
        ApiProtocol::GeminiGenerateContent => {
            let parts: Vec<_> = second["contents"].as_array().unwrap().iter().flat_map(|m| m["parts"].as_array().unwrap()).collect();
            assert!(parts.iter().any(|p| p["functionCall"]["name"] == "read" && p["thoughtSignature"] == "fixture-signature"));
            assert!(parts.iter().any(|p| p["functionResponse"]["name"] == "read"));
            assert_eq!(structured["generationConfig"]["responseMimeType"], "application/json");
        }
        ApiProtocol::GeminiInteractions => {
            assert_eq!(requests[0].body["store"], false);
            assert!(second["input"].as_array().unwrap().iter().any(|item| item["type"] == "function_result" && item["call_id"] == call_id));
            assert!(second.get("previous_interaction_id").is_none());
            assert_eq!(structured["response_format"]["mime_type"], "application/json");
        }
        ApiProtocol::BedrockConverse => {
            assert!(requests[0].body.get("model").is_none());
            assert_eq!(requests[0].body["toolConfig"]["tools"][0]["toolSpec"]["inputSchema"]["json"]["required"], json!(["page"]));
            let blocks: Vec<_> = second["messages"].as_array().unwrap().iter().flat_map(|m| m["content"].as_array().unwrap()).collect();
            assert!(blocks.iter().any(|b| b["toolUse"]["toolUseId"] == call_id));
            assert!(blocks.iter().any(|b| b["toolResult"]["toolUseId"] == call_id));
        }
        _ => unreachable!(),
    }
}

#[test]
fn multi_protocol_tool_round_trip_and_structured_completion_share_send_and_ledger() {
    for protocol in PROTOCOLS {
        let fixture = Fixture::new(vec![success(protocol, None), success(protocol, Some(ANSWER)), success(protocol, Some("{\"ok\":true}"))]);
        let ledger = Arc::new(Ledger::default());
        let adapter = adapter(protocol, &fixture, ledger.clone());
        let mut messages = vec![Message::system("Use source pages to answer."), Message::user("Read page 1, then explain it.")];
        let first = adapter.chat_observed(&plan(adapter.as_ref(), &messages), &mut provider_stream::ignore).unwrap_or_else(|e| panic!("{protocol:?} tool request: {}", e.message));
        assert_eq!(first.tool_calls.len(), 1, "{protocol:?}");
        let call = first.tool_calls[0].clone();
        assert!(!call.id.is_empty()); assert_eq!(call.name, "read");
        assert_eq!(serde_json::from_str::<Value>(&call.arguments).unwrap(), json!({"page":1}));
        messages.push(Message { role: Role::Assistant, content: first.text, tool_calls: first.tool_calls, tool_call_id: None, provider_continuation: first.provider_continuation });
        messages.push(Message { role: Role::Tool, content: Some("{\"page\":1,\"text\":\"fixture source\"}".into()), tool_calls: vec![], tool_call_id: Some(call.id.clone()), provider_continuation: None });
        let mut events = Vec::new();
        let answer = adapter.chat_observed(&plan(adapter.as_ref(), &messages), &mut |event| events.push(event)).unwrap_or_else(|e| panic!("{protocol:?} tool result: {}", e.message));
        assert_eq!(answer.text.as_deref(), Some(ANSWER)); assert!(answer.tool_calls.is_empty());
        let result = adapter.complete_structured_observed(structured_request(), &mut provider_stream::ignore).unwrap_or_else(|e| panic!("{protocol:?} structured: {}", e.message));
        assert_eq!(result, json!({"ok":true}));
        let requests = fixture.requests.lock().unwrap();
        assert_wire(protocol, &requests, &call.id);
        let starts = ledger.starts.lock().unwrap(); let finishes = ledger.finishes.lock().unwrap();
        assert_eq!(starts.len(), 3, "{protocol:?}"); assert_eq!(finishes.len(), 3, "{protocol:?}");
        for index in 0..3 {
            assert_eq!(starts[index].0.attempt, 1);
            assert_eq!(starts[index].0.call_id, finishes[index].0.call_id);
            assert_eq!(starts[index].1, requests[index].body, "ledger must receive the final wire request");
            assert!(finishes[index].1.succeeded); assert_eq!(finishes[index].1.evidence, SendEvidence::Response);
            let usage = finishes[index].1.usage.as_ref().unwrap();
            assert_eq!(usage.input_tokens, Some(10)); assert_eq!(usage.output_tokens, Some(3)); assert_eq!(usage.total_tokens, Some(13));
            if protocol == ApiProtocol::CohereV2Chat { assert_eq!(usage.billed_input_tokens, Some(7)); }
        }
        assert_ne!(starts[0].0.logical_call_id, starts[1].0.logical_call_id);
        assert_ne!(starts[1].0.logical_call_id, starts[2].0.logical_call_id);
        if !matches!(protocol, ApiProtocol::GeminiInteractions | ApiProtocol::BedrockConverse) {
            assert!(events.iter().any(|event| matches!(event, ModelDelta::Text(text) if text.contains("Page 1"))), "{protocol:?}: streamed answer reached the observer");
        }
    }
}

#[test]
fn multi_protocol_auth_failure_is_one_failed_send_without_protocol_fallback() {
    for protocol in PROTOCOLS {
        let fixture = Fixture::new(vec![wire(401, "application/json", "{\"error\":{\"message\":\"Invalid fixture credential\"}}")]);
        let ledger = Arc::new(Ledger::default());
        let adapter = adapter(protocol, &fixture, ledger.clone());
        let error = adapter.complete_structured(structured_request()).unwrap_err();
        assert!(error.message.contains("401"), "{protocol:?}: {}", error.message);
        assert_eq!(fixture.requests.lock().unwrap().len(), 1, "{protocol:?}");
        assert_eq!(ledger.starts.lock().unwrap().len(), 1);
        let finishes = ledger.finishes.lock().unwrap(); assert_eq!(finishes.len(), 1); assert!(!finishes[0].1.succeeded);
    }
}

#[test]
fn multi_protocol_interrupted_stream_is_not_a_completed_answer_or_tool_call() {
    for protocol in PROTOCOLS.into_iter().filter(|p| *p != ApiProtocol::BedrockConverse) {
        let (content_type, body) = match protocol {
            ApiProtocol::OpenaiResponses => ("text/event-stream", sse(vec![
                json!({"type":"response.output_item.added","output_index":0,"item":{"type":"message","id":"msg-1","role":"assistant","content":[]}}),
                json!({"type":"response.output_text.delta","output_index":0,"item_id":"msg-1","content_index":0,"delta":"half"}),
            ])),
            ApiProtocol::AnthropicMessages => ("text/event-stream", sse(vec![
                json!({"type":"message_start","message":{"id":"msg-1","role":"assistant","content":[],"usage":{"input_tokens":10,"output_tokens":0}}}),
                json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}),
                json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"half"}}),
            ])),
            ApiProtocol::OllamaChat => ("application/x-ndjson", format!("{}\n", json!({"message":{"role":"assistant","content":"half"},"done":false}))),
            ApiProtocol::CohereV2Chat => ("text/event-stream", sse(vec![
                json!({"type":"message-start","id":"msg-1","delta":{"message":{"role":"assistant","content":[]}}}),
                json!({"type":"content-start","index":0,"delta":{"message":{"content":{"type":"text","text":""}}}}),
                json!({"type":"content-delta","index":0,"delta":{"message":{"content":{"text":"half"}}}}),
            ])),
            ApiProtocol::GeminiGenerateContent => ("text/event-stream", sse(vec![json!({"candidates":[{"index":0,"content":{"role":"model","parts":[{"text":"half"}]}}]})])),
            ApiProtocol::GeminiInteractions => ("text/event-stream", sse(vec![
                json!({"event_type":"step.start","index":0,"step":{"type":"model_output","content":[]}}),
                json!({"event_type":"step.delta","index":0,"delta":{"type":"text","text":"half"}}),
            ])),
            _ => unreachable!(),
        };
        let fixture = Fixture::new(vec![wire(200, content_type, &body)]);
        let ledger = Arc::new(Ledger::default()); let adapter = adapter(protocol, &fixture, ledger.clone());
        let mut events = Vec::new();
        let result = adapter.chat_observed(&plan(adapter.as_ref(), &[Message::user("Read page 1.")]), &mut |event| events.push(event));
        assert!(events.iter().any(|event| matches!(event, ModelDelta::Text(text) if text == "half")), "{protocol:?}: fixture must reach a valid partial answer before disconnecting");
        assert!(result.is_err(), "{protocol:?}: an interrupted stream cannot succeed");
        assert_eq!(fixture.requests.lock().unwrap().len(), 1, "{protocol:?}: no automatic replay of a partial answer");
        assert_eq!(ledger.starts.lock().unwrap().len(), 1);
        let finishes = ledger.finishes.lock().unwrap(); assert_eq!(finishes.len(), 1); assert!(!finishes[0].1.succeeded);
    }
}
