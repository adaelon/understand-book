//! Shared protocol behavior exercised through the real local HTTP transport.
use super::*;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant};

fn server(response: Value) -> (String, std::thread::JoinHandle<(String, Value)>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let worker = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut socket = loop {
            match listener.accept() {
                Ok((socket, _)) => break socket,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < deadline => std::thread::sleep(Duration::from_millis(2)),
                Err(error) => panic!("fixture did not receive a request: {error}"),
            }
        };
        socket.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        let mut reader = BufReader::new(&mut socket);
        let mut headers = String::new();
        let mut length = 0;
        loop {
            let mut line = String::new();
            assert!(reader.read_line(&mut line).unwrap() > 0);
            if line == "\r\n" { break; }
            if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") { length = value.trim().parse().unwrap(); }
            headers.push_str(&line);
        }
        let mut body = vec![0; length];
        reader.read_exact(&mut body).unwrap();
        drop(reader);
        let body = if body.is_empty() { Value::Null } else { serde_json::from_slice(&body).unwrap() };
        let response = response.to_string();
        write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}", response.len()).unwrap();
        (headers, body)
    });
    (url, worker)
}

fn success() -> Value { json!({"choices":[{"message":{"content":"{\"ok\":true}"},"finish_reason":"stop"}],"usage":{"prompt_tokens":5,"completion_tokens":3,"total_tokens":8}}) }
fn config(url: String, protocol: ApiProtocol) -> ProviderConfig {
    let mut config = ProviderConfig::from_values("native", "test-key", url, "deepseek-fixture").unwrap();
    config.options.api_protocol = protocol;
    config
}
fn completion() -> CompletionRequest {
    CompletionRequest { system:"Return JSON".into(),user:"fixture".into(),images:vec![],followup_user:None,output_token_limit:Some(512),reasoning_effort:Some("low".into()) }
}

#[test]
fn provider_explicit_chat_does_not_probe_deepseek_and_none_auth_omits_credentials() {
    let (url, worker) = server(success());
    let mut config = config(format!("{url}/v1?api-version=2026-01-01"), ApiProtocol::OpenaiChatCompletions);
    config.options.auth = AuthMethod::None;
    config.api_key.clear();
    config.options.headers.insert("x-request-purpose".into(), "test".into());
    config.options.output_token_limit = Some(128);
    let adapter = NativeAdapter::from_config(config);
    assert_eq!(adapter.tool_update_mode(), None);
    assert_eq!(adapter.complete_structured(completion()).unwrap(), json!({"ok":true}));
    let (headers, body) = worker.join().unwrap();
    assert!(headers.starts_with("POST /v1/chat/completions?api-version=2026-01-01 HTTP/1.1"), "{headers}");
    let headers = headers.to_ascii_lowercase();
    assert!(!headers.contains("authorization:") && !headers.contains("x-api-key:"));
    assert!(headers.contains("x-request-purpose: test"));
    assert_eq!(body["max_tokens"], 128);
    assert!(body.get("reasoning_effort").is_none());
    assert!(body.get("temperature").is_none());
}

#[test]
fn provider_chat_output_parameter_and_explicit_temperature_reach_the_wire() {
    let (url, worker) = server(success());
    let mut config = config(url, ApiProtocol::OpenaiChatCompletions);
    config.options.generation.output_token_parameter = Some(OutputTokenParameter::MaxCompletionTokens);
    config.options.generation.temperature = Some(0.0);
    config.options.output_token_limit = Some(99);
    assert_eq!(NativeAdapter::from_config(config).complete_structured(completion()).unwrap(), json!({"ok":true}));
    let (_, body) = worker.join().unwrap();
    assert_eq!(body["max_completion_tokens"], 99);
    assert!(body.get("max_tokens").is_none());
    assert_eq!(body["temperature"], 0.0);
}

#[test]
fn provider_explicit_messages_reports_missing_route_capability_without_chat_fallback() {
    let (url, worker) = server(json!({"data":[{"id":"deepseek-fixture"}]}));
    let adapter = NativeAdapter::from_config(config(url, ApiProtocol::DeepseekMessages));
    let error = adapter.complete_structured(completion()).unwrap_err();
    assert!(error.message.contains("未确认 DeepSeek Messages"), "{}", error.message);
    assert!(worker.join().unwrap().0.starts_with("GET /models HTTP/1.1"));
}

#[test]
fn provider_private_state_is_bound_to_service_protocol_endpoint_and_model() {
    let mut config = config("http://localhost:7654/v1".into(), ApiProtocol::OpenaiResponses);
    config.options.binding = Some(model_settings::ModelBinding {service_id:"service-a".into(),model_config_id:"model-a".into(),revision:1});
    let adapter = NativeAdapter::from_config(config);
    let mut message = Message::user("question");
    message.role = Role::Assistant;
    message.content = Some("delivered answer".into());
    message.tool_calls = vec![ToolCall { id:"call-1".into(), name:"book.text".into(), arguments:"{\"lid\":\"1\"}".into() }];
    message.provider_continuation = Some(ProviderContinuation {
        provider_identity:Some(adapter.continuation_identity(ApiProtocol::OpenaiResponses)),
        protocol_state:Some(json!([{"type":"reasoning","encrypted_content":"private"}])),
        model:"deepseek-fixture".into(),reasoning_content:String::new(),messages_content:vec![],
    });
    let mut plan = AgentRequestPlan::for_ad_hoc(adapter.model_runtime_profile(), &[message], &[]);
    assert!(matches!(adapter.request_history(&plan, ApiProtocol::OpenaiResponses), std::borrow::Cow::Borrowed(_)));
    plan.input[0].provider_continuation.as_mut().unwrap().provider_identity.as_mut().unwrap().service_id = Some("service-b".into());
    let projected = adapter.request_history(&plan, ApiProtocol::OpenaiResponses);
    assert!(projected.input[0].provider_continuation.is_none());
    assert_eq!(projected.input[0].content.as_deref(), Some("delivered answer"));
    assert_eq!(projected.input[0].tool_calls[0].id, "call-1");
    assert!(plan.input[0].provider_continuation.is_some());
    assert!(adapter.request_history(&plan, ApiProtocol::AnthropicMessages).input[0].provider_continuation.is_none());
}

#[test]
fn provider_header_override_and_image_capability_fail_before_transport() {
    let mut config = config("http://127.0.0.1:1".into(), ApiProtocol::OpenaiChatCompletions);
    config.options.headers.insert("Authorization".into(),"other-key".into());
    let adapter = NativeAdapter::from_config(config.clone());
    assert!(adapter.complete_structured(completion()).unwrap_err().message.contains("不能覆盖认证"));
    config.options.headers.clear();
    config.options.capabilities.images = Some(false);
    let adapter = NativeAdapter::from_config(config);
    let body = json!({"messages":[{"role":"user","content":[{"type":"image_url","image_url":{"url":"data:image/png;base64,AA=="}}]}]});
    assert!(adapter.validate_protocol_request(&body, ApiProtocol::OpenaiChatCompletions).unwrap_err().message.contains("不支持图像"));
}

#[test]
fn provider_streaming_tools_can_use_the_same_nonstreaming_tool_contract() {
    let mut config = config("http://localhost:7654".into(), ApiProtocol::BedrockConverse);
    config.options.capabilities.streaming_tools = Some(false);
    let adapter = NativeAdapter::from_config(config);
    let mut body = json!({"messages":[],"tools":[{"type":"function","function":{"name":"read","parameters":{"type":"object"}}}]});
    adapter.normalize_provider_body(&mut body, ApiProtocol::BedrockConverse).unwrap();
    assert_eq!(body["stream"], false);
    assert!(body.get("stream_options").is_none());
    assert!(protocol_endpoint(ApiProtocol::BedrockConverse, "https://bedrock-runtime.us-east-1.amazonaws.com", "model-id", false).unwrap().ends_with("/converse"));
}

#[test]
fn provider_billed_units_remain_distinct_and_missing_attempt_usage_stays_unknown() {
    let usage = provider_stream::model_usage(&json!({"usage":{"input_tokens":100,"output_tokens":20,"billed_input_tokens":75,"billed_output_tokens":18}})).unwrap();
    assert_eq!(usage.total_tokens, Some(120));
    assert_eq!(usage.billed_input_tokens, Some(75));
    assert_eq!(usage.billed_output_tokens, Some(18));
    assert_eq!(usage.plus(&usage).total_tokens, Some(240));
    assert_eq!(usage.plus(&usage).billed_input_tokens, Some(150));
    assert_eq!(usage.plus(&provider_stream::ModelUsage::default()).billed_input_tokens, None);
    assert_eq!(provider_stream::model_usage(&json!({"usage":usage.as_provider_usage()})), Some(usage));
}

#[test]
fn provider_model_settings_file_selects_a_model_and_invalid_explicit_settings_never_fall_back() {
    let path = std::env::temp_dir().join(format!("ub-model-settings-{}.json", uuid::Uuid::now_v7()));
    let mut settings = model_settings::ModelSettings::from_legacy(&config("http://localhost:7654/v1".into(), ApiProtocol::LegacyAuto));
    let mut second = settings.models[0].clone();
    second.id = "selected".into();
    second.model_id = "another-model".into();
    settings.models.push(second);
    std::fs::write(&path, serde_json::to_vec(&settings).unwrap()).unwrap();
    let read = || ProviderConfig::from_getter(|key| match key {
        "UNDERSTAND_BOOK_MODEL_SETTINGS" => Some(path.to_string_lossy().into_owned()),
        "UNDERSTAND_BOOK_MODEL_CONFIG_ID" => Some("selected".into()),
        "OPENCODE_API_KEY" => Some("fallback-key".into()),
        "OPENCODE_BASE_URL" => Some("http://fallback.invalid/v1".into()),
        "FLUID_LLM_MODEL" => Some("fallback-model".into()),
        _ => None,
    });
    let selected = read().unwrap();
    assert_eq!(selected.model, "another-model");
    assert_eq!(selected.options.binding.unwrap().model_config_id, "selected");
    std::fs::write(&path, "{invalid").unwrap();
    assert!(read().unwrap_err().message.contains("JSON"));
    std::fs::remove_file(&path).unwrap();
    assert!(read().unwrap_err().message.contains("无法读取"));
}
