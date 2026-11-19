//! Cohere V2 chat projection and typed SSE events. Context tokens and billed
//! units deliberately remain separate in the shared usage record.
use crate::{
    provider_stream::{ModelDelta, ModelObserver, ModelUsage},
    run_context::CancellationToken,
    AdapterError,
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader, Read},
    sync::Arc,
};

fn error(message: impl ToString) -> AdapterError {
    AdapterError {
        spend_stop: None,
        message: format!("Cohere V2: {}", message.to_string()),
    }
}

pub(crate) fn endpoint(base_url: &str, _model: &str) -> Result<String, AdapterError> {
    let mut url = url::Url::parse(base_url).map_err(error)?;
    let path = url.path().trim_end_matches('/');
    let path = if path.ends_with("/v2/chat") {
        path.into()
    } else if path.ends_with("/v2") {
        format!("{path}/chat")
    } else {
        format!("{path}/v2/chat")
    };
    url.set_path(&path);
    Ok(url.to_string())
}

pub(crate) fn project_request(chat_body: Value) -> Result<Value, AdapterError> {
    let mut messages = Vec::new();
    for message in chat_body["messages"]
        .as_array()
        .ok_or_else(|| error("messages are missing"))?
    {
        let role = message["role"]
            .as_str()
            .ok_or_else(|| error("message role is missing"))?;
        let mut native = json!({"role":role});
        if !message["content"].is_null() {
            native["content"] = message["content"].clone();
        }
        if role == "assistant" {
            if let Some(calls) = message.get("tool_calls") {
                native["tool_calls"] = calls.clone();
            }
            if let Some(plan) = message["protocol_state"]["tool_plan"]
                .as_str()
                .filter(|s| !s.is_empty())
            {
                native["tool_plan"] = json!(plan);
            }
            if let Some(thinking) = message["protocol_state"]["thinking"]
                .as_str()
                .filter(|s| !s.is_empty())
            {
                let mut blocks = vec![json!({"type":"thinking","thinking":thinking})];
                if let Some(text) = message["content"].as_str().filter(|text| !text.is_empty()) {
                    blocks.push(json!({"type":"text","text":text}));
                } else if let Some(content) = message["content"].as_array() {
                    blocks.extend(content.iter().cloned());
                }
                native["content"] = json!(blocks);
            }
        } else if role == "tool" {
            let id = message["tool_call_id"]
                .as_str()
                .ok_or_else(|| error("tool result ID is missing"))?;
            native["tool_call_id"] = json!(id);
            if let Some(text) = message["content"].as_str() {
                // A text block accepts both plain-text results and JSON results.
                native["content"] = json!([{"type":"text","text":text}]);
            }
        }
        messages.push(native);
    }
    let mut body = json!({"model":chat_body["model"],"messages":messages,"stream":chat_body["stream"].as_bool().unwrap_or(true)});
    for field in [
        "max_tokens",
        "temperature",
        "seed",
        "frequency_penalty",
        "presence_penalty",
    ] {
        if let Some(value) = chat_body.get(field) {
            body[field] = value.clone();
        }
    }
    if let Some(value) = chat_body.get("top_p") {
        body["p"] = value.clone();
    }
    if let Some(value) = chat_body.get("stop") {
        body["stop_sequences"] = if value.is_string() {
            json!([value])
        } else {
            value.clone()
        };
    }
    let choice = chat_body["tool_choice"].as_str().unwrap_or("auto");
    if chat_body.get("tool_choice").is_some_and(Value::is_object)
        || !matches!(choice, "auto" | "none" | "required")
    {
        return Err(error(
            "named tool choice is not supported; use auto or required",
        ));
    }
    let tools = chat_body["tools"]
        .as_array()
        .filter(|tools| !tools.is_empty() && choice != "none");
    if let Some(tools) = tools {
        body["tools"] = json!(tools);
        if choice == "required" {
            body["tool_choice"] = json!("REQUIRED");
        }
    } else if choice == "required" {
        return Err(error("required tool choice needs at least one tool"));
    }
    if let Some(format) = chat_body
        .get("response_format")
        .filter(|format| format["type"] != "text")
    {
        if tools.is_some()
            || chat_body["documents"]
                .as_array()
                .is_some_and(|docs| !docs.is_empty())
        {
            return Err(error("response_format cannot be combined with tools or documents; use a separate structured completion"));
        }
        body["response_format"] = match format["type"].as_str() {
            Some("json_object") => json!({"type":"json_object"}),
            Some("json_schema") => {
                let schema = &format["json_schema"]["schema"];
                if !schema.is_object() {
                    return Err(error("JSON Schema is missing"));
                }
                json!({"type":"json_object","json_schema":schema})
            }
            kind => return Err(error(format!("unsupported response format {kind:?}"))),
        };
    }
    // OpenAI reasoning intensities have no corresponding Cohere intensity field.
    // An explicit intensity must not silently disappear from the request.
    if chat_body.get("reasoning_effort").is_some() {
        return Err(error(
            "reasoning_effort is unsupported; use the model's default reasoning configuration",
        ));
    }
    Ok(body)
}

fn count(value: &Value) -> Option<u32> {
    value
        .as_u64()
        .and_then(|n| u32::try_from(n).ok())
        .or_else(|| {
            let n = value.as_f64()?;
            (n.is_finite() && n >= 0.0 && n <= f64::from(u32::MAX) && n.fract() == 0.0)
                .then_some(n as u32)
        })
}

pub(crate) fn model_usage(value: &Value) -> Option<ModelUsage> {
    let usage = value
        .get("usage")
        .or_else(|| value.get("delta")?.get("usage"))?;
    let input_tokens = count(&usage["tokens"]["input_tokens"]);
    let output_tokens = count(&usage["tokens"]["output_tokens"]);
    let snapshot = ModelUsage {
        input_tokens,
        output_tokens,
        cached_input_tokens: count(&usage["cached_tokens"]),
        billed_input_tokens: count(&usage["billed_units"]["input_tokens"]),
        billed_output_tokens: count(&usage["billed_units"]["output_tokens"]),
        total_tokens: input_tokens
            .zip(output_tokens)
            .map(|(a, b)| a.saturating_add(b)),
        ..Default::default()
    };
    snapshot.has_any_value().then_some(snapshot)
}

fn finish_reason(value: &Value) -> Result<&'static str, AdapterError> {
    match value.as_str() {
        Some("COMPLETE" | "STOP_SEQUENCE") => Ok("stop"),
        Some("MAX_TOKENS") => Ok("length"),
        Some("TOOL_CALL") => Ok("tool_calls"),
        Some("ERROR" | "TIMEOUT") => Err(error(format!("generation ended with {value}"))),
        other => Err(error(format!(
            "unsupported or missing finish_reason {other:?}"
        ))),
    }
}

fn checked_call(value: &Value) -> Result<Value, AdapterError> {
    let id = value["id"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| error("tool call ID is missing"))?;
    let name = value["function"]["name"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| error("tool name is missing"))?;
    let arguments = value["function"]["arguments"]
        .as_str()
        .ok_or_else(|| error("tool arguments are missing"))?;
    if !serde_json::from_str::<Value>(arguments)
        .map_err(error)?
        .is_object()
    {
        return Err(error("tool arguments must be an object"));
    }
    Ok(json!({"id":id,"type":"function","function":{"name":name,"arguments":arguments}}))
}

fn normalize(
    value: Value,
    observer: &mut dyn ModelObserver,
    streamed: bool,
) -> Result<Value, AdapterError> {
    let usage = model_usage(&value);
    if !streamed {
        if let Some(usage) = &usage {
            observer.observe(ModelDelta::Usage(usage.clone()));
        }
    }
    if !value["error"].is_null() {
        return Err(error(format!("provider error: {}", value["error"])));
    }
    let reason = finish_reason(&value["finish_reason"])?;
    if !value["message"].is_object() {
        return Err(error("response message is missing"));
    }
    for field in ["content", "tool_calls"] {
        if value["message"]
            .get(field)
            .is_some_and(|v| !v.is_null() && !v.is_array())
        {
            return Err(error(format!("response message {field} must be an array")));
        }
    }
    let mut text = String::new();
    let mut thinking = String::new();
    for block in value["message"]["content"].as_array().into_iter().flatten() {
        match block["type"].as_str() {
            Some("text") => text.push_str(
                block["text"]
                    .as_str()
                    .ok_or_else(|| error("invalid text block"))?,
            ),
            Some("thinking") => thinking.push_str(
                block["thinking"]
                    .as_str()
                    .ok_or_else(|| error("invalid thinking block"))?,
            ),
            kind => return Err(error(format!("unsupported response content {kind:?}"))),
        }
    }
    let mut calls = Vec::new();
    for call in value["message"]["tool_calls"]
        .as_array()
        .into_iter()
        .flatten()
    {
        // Length truncation is dealt with by the common output retry lifecycle;
        // incomplete arguments are never delivered to the executor.
        calls.push(if reason == "length" {
            call.clone()
        } else {
            checked_call(call)?
        });
    }
    if reason == "tool_calls" && calls.is_empty() {
        return Err(error("TOOL_CALL finish without a tool call"));
    }
    if reason == "stop" && !calls.is_empty() {
        return Err(error("tool calls require TOOL_CALL finish"));
    }
    if !streamed {
        if !text.is_empty() {
            observer.observe(ModelDelta::Text(text.clone()));
        }
        for (index, call) in calls.iter().enumerate() {
            observer.observe(ModelDelta::ToolArguments {
                index,
                id: call["id"].as_str().unwrap_or("").into(),
                name: call["function"]["name"].as_str().unwrap_or("").into(),
                arguments: call["function"]["arguments"].as_str().unwrap_or("").into(),
            });
        }
        observer.observe(ModelDelta::Finish(reason.into()));
    }
    let mut message = json!({"role":"assistant","content":text,"tool_calls":calls});
    let plan = value["message"]["tool_plan"].as_str().unwrap_or("");
    if !thinking.is_empty() || !plan.is_empty() {
        message["protocol_state"] = json!({"thinking":thinking,"tool_plan":plan});
    }
    Ok(
        json!({"choices":[{"index":0,"message":message,"finish_reason":reason}],"usage":usage.map(|usage|usage.as_provider_usage())}),
    )
}

struct ContentBlock {
    kind: String,
    text: String,
    closed: bool,
}
struct ToolBlock {
    id: String,
    name: String,
    arguments: String,
    closed: bool,
}
impl ToolBlock {
    fn value(&self) -> Value {
        json!({"id":self.id,"type":"function","function":{"name":self.name,"arguments":self.arguments}})
    }
    fn observe(&self, index: usize, observer: &mut dyn ModelObserver) {
        observer.observe(ModelDelta::ToolArguments {
            index,
            id: self.id.clone(),
            name: self.name.clone(),
            arguments: self.arguments.clone(),
        });
    }
}

#[derive(Default)]
struct StreamState {
    started: bool,
    content: BTreeMap<usize, ContentBlock>,
    tools: BTreeMap<usize, ToolBlock>,
    plan: String,
    finish: Option<String>,
    usage: Value,
}
impl StreamState {
    fn accept(
        &mut self,
        value: Value,
        observer: &mut dyn ModelObserver,
    ) -> Result<(), AdapterError> {
        let kind = value["type"]
            .as_str()
            .ok_or_else(|| error("SSE event type is missing"))?;
        if self.finish.is_some() {
            return Err(error("event after message-end"));
        }
        if kind == "message-start" {
            if self.started {
                return Err(error("duplicate message-start"));
            }
            self.started = true;
            return Ok(());
        }
        if kind == "debug" {
            return Ok(());
        }
        if !self.started {
            return Err(error("stream has no message-start"));
        }
        let index = || {
            value["index"]
                .as_u64()
                .and_then(|n| usize::try_from(n).ok())
                .ok_or_else(|| error("event index is missing"))
        };
        let message = &value["delta"]["message"];
        match kind {
            "content-start" => {
                let index = index()?;
                if self.content.contains_key(&index) {
                    return Err(error("duplicate content index"));
                }
                let content = &message["content"];
                let kind = content["type"]
                    .as_str()
                    .ok_or_else(|| error("content type is missing"))?;
                let field = match kind {
                    "text" => "text",
                    "thinking" => "thinking",
                    _ => return Err(error(format!("unsupported content type {kind}"))),
                };
                let text = content[field].as_str().unwrap_or("").to_owned();
                if kind == "text" && !text.is_empty() {
                    observer.observe(ModelDelta::Text(text.clone()));
                }
                self.content.insert(
                    index,
                    ContentBlock {
                        kind: kind.into(),
                        text,
                        closed: false,
                    },
                );
            }
            "content-delta" => {
                let block = self
                    .content
                    .get_mut(&index()?)
                    .ok_or_else(|| error("content delta without start"))?;
                if block.closed {
                    return Err(error("content delta after end"));
                }
                let fragment = message["content"][&block.kind]
                    .as_str()
                    .ok_or_else(|| error("content delta text is missing"))?;
                block.text.push_str(fragment);
                if block.kind == "text" && !fragment.is_empty() {
                    observer.observe(ModelDelta::Text(fragment.into()));
                }
            }
            "content-end" => {
                let block = self
                    .content
                    .get_mut(&index()?)
                    .ok_or_else(|| error("content end without start"))?;
                if block.closed {
                    return Err(error("duplicate content end"));
                }
                block.closed = true;
            }
            "tool-plan-delta" => self.plan.push_str(
                message["tool_plan"]
                    .as_str()
                    .ok_or_else(|| error("tool plan is missing"))?,
            ),
            "tool-call-start" => {
                let index = index()?;
                if self.tools.contains_key(&index) {
                    return Err(error("duplicate tool index"));
                }
                let call = &message["tool_calls"];
                let id = call["id"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| error("tool ID is missing"))?;
                let name = call["function"]["name"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| error("tool name is missing"))?;
                let block = ToolBlock {
                    id: id.into(),
                    name: name.into(),
                    arguments: call["function"]["arguments"].as_str().unwrap_or("").into(),
                    closed: false,
                };
                block.observe(index, observer);
                self.tools.insert(index, block);
            }
            "tool-call-delta" => {
                let index = index()?;
                let block = self
                    .tools
                    .get_mut(&index)
                    .ok_or_else(|| error("tool delta without start"))?;
                if block.closed {
                    return Err(error("tool delta after end"));
                }
                block.arguments.push_str(
                    message["tool_calls"]["function"]["arguments"]
                        .as_str()
                        .ok_or_else(|| error("tool argument delta is missing"))?,
                );
                block.observe(index, observer);
            }
            "tool-call-end" => {
                let block = self
                    .tools
                    .get_mut(&index()?)
                    .ok_or_else(|| error("tool end without start"))?;
                if block.closed {
                    return Err(error("duplicate tool end"));
                }
                block.closed = true;
            }
            // Citations are provider metadata, not the application's verified LIDs.
            "citation-start" | "citation-end" => {}
            "message-end" => {
                if let Some(usage) = model_usage(&value) {
                    observer.observe(ModelDelta::Usage(usage));
                }
                self.usage = value["delta"]["usage"].clone();
                if let Some(message) = value["delta"]["error"].as_str().filter(|s| !s.is_empty()) {
                    return Err(error(message));
                }
                let reason = finish_reason(&value["delta"]["finish_reason"])?;
                if reason != "length"
                    && (self.content.values().any(|b| !b.closed)
                        || self.tools.values().any(|b| !b.closed))
                {
                    return Err(error(
                        "message ended with unfinished content or tool blocks",
                    ));
                }
                self.finish = value["delta"]["finish_reason"].as_str().map(str::to_owned);
                observer.observe(ModelDelta::Finish(reason.into()));
            }
            "error" => return Err(error(format!("stream error: {value}"))),
            _ => return Err(error(format!("unsupported SSE event {kind}"))),
        }
        Ok(())
    }

    fn finish(self, observer: &mut dyn ModelObserver) -> Result<Value, AdapterError> {
        let reason = self
            .finish
            .ok_or_else(|| error("stream ended before message-end"))?;
        let content: Vec<_> = self
            .content
            .into_values()
            .map(|b| {
                if b.kind == "text" {
                    json!({"type":"text","text":b.text})
                } else {
                    json!({"type":"thinking","thinking":b.text})
                }
            })
            .collect();
        let calls: Vec<_> = self.tools.into_values().map(|b| b.value()).collect();
        normalize(
            json!({"finish_reason":reason,"message":{"content":content,"tool_calls":calls,"tool_plan":self.plan},"usage":self.usage}),
            observer,
            true,
        )
    }
}

fn read_sse(
    reader: impl Read,
    cancellation: &CancellationToken,
    observer: &mut dyn ModelObserver,
) -> Result<Value, AdapterError> {
    let mut reader = BufReader::new(reader);
    let mut data = String::new();
    let mut state = StreamState::default();
    loop {
        cancellation.check().map_err(|e| error(e.message))?;
        let mut line = String::new();
        let count = reader.read_line(&mut line).map_err(error)?;
        cancellation.check().map_err(|e| error(e.message))?;
        if count == 0 {
            if !data.is_empty() {
                return Err(error("incomplete SSE event at end of stream"));
            }
            break;
        }
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            if !data.is_empty() {
                state.accept(
                    serde_json::from_str(&std::mem::take(&mut data)).map_err(error)?,
                    observer,
                )?;
            }
        } else if let Some(fragment) = line.strip_prefix("data:") {
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(fragment.strip_prefix(' ').unwrap_or(fragment));
        }
    }
    state.finish(observer)
}

pub(crate) fn read_response(
    response: ureq::Response,
    cancellation: &CancellationToken,
    observer: &mut dyn ModelObserver,
) -> Result<Value, AdapterError> {
    let streaming = response
        .header("Content-Type")
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        == "text/event-stream";
    let result = if streaming {
        read_sse(response.into_reader(), cancellation, observer)
    } else {
        cancellation.check().map_err(|e| error(e.message))?;
        let value = response.into_json().map_err(error)?;
        cancellation.check().map_err(|e| error(e.message))?;
        normalize(value, observer, false)
    };
    if let Ok(value) = &result {
        observer.observe(ModelDelta::Response(Arc::new(value.clone())));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider_stream::ignore;
    fn stream(events: Vec<Value>) -> String {
        events
            .into_iter()
            .map(|v| format!("data: {v}\n\n"))
            .collect()
    }

    #[test]
    fn provider_cohere_projection_separates_tools_from_json() {
        let base = json!({"model":"command-a","messages":[],"max_tokens":80,"stream":true,"tools":[{"type":"function","function":{"name":"read","parameters":{"type":"object"}}}],"tool_choice":"auto"});
        let wire = project_request(base.clone()).unwrap();
        assert!(wire.get("tool_choice").is_none());
        assert_eq!(wire["tools"], base["tools"]);
        let mut combined = base;
        combined["response_format"] = json!({"type":"json_object"});
        assert!(project_request(combined.clone())
            .unwrap_err()
            .message
            .contains("cannot be combined"));
        combined["tool_choice"] = json!("none");
        assert_eq!(
            project_request(combined).unwrap()["response_format"],
            json!({"type":"json_object"})
        );
        let schema = project_request(json!({"messages":[],"response_format":{"type":"json_schema","json_schema":{"schema":{"type":"object"}}}})).unwrap();
        assert_eq!(
            schema["response_format"],
            json!({"type":"json_object","json_schema":{"type":"object"}})
        );
        assert_eq!(
            endpoint("https://api.cohere.com/v2?version=1", "m").unwrap(),
            "https://api.cohere.com/v2/chat?version=1"
        );
    }

    #[test]
    fn provider_cohere_tool_result_and_private_plan_roundtrip() {
        let call = json!({"id":"a","type":"function","function":{"name":"read","arguments":"{}"}});
        let value = normalize(json!({"finish_reason":"TOOL_CALL","message":{"content":[{"type":"thinking","thinking":"think"}],"tool_plan":"inspect","tool_calls":[call]}}), &mut ignore, false).unwrap();
        let message = value["choices"][0]["message"].clone();
        let wire = project_request(
            json!({"messages":[message,{"role":"tool","tool_call_id":"a","content":"raw result"}]}),
        )
        .unwrap();
        assert_eq!(wire["messages"][0]["tool_plan"], "inspect");
        assert_eq!(wire["messages"][0]["content"][0]["thinking"], "think");
        assert_eq!(wire["messages"][1]["tool_call_id"], "a");
        assert_eq!(
            wire["messages"][1]["content"],
            json!([{"type":"text","text":"raw result"}])
        );
        assert!(wire["messages"][0].get("protocol_state").is_none());
    }

    #[test]
    fn provider_cohere_parallel_sse_calls_and_billed_units_stay_separate() {
        let events = vec![
            json!({"type":"message-start"}),
            json!({"type":"tool-call-start","index":1,"delta":{"message":{"tool_calls":{"id":"b","function":{"name":"second","arguments":""}}}}}),
            json!({"type":"tool-call-start","index":0,"delta":{"message":{"tool_calls":{"id":"a","function":{"name":"first","arguments":"{"}}}}}),
            json!({"type":"tool-call-delta","index":1,"delta":{"message":{"tool_calls":{"function":{"arguments":"{}"}}}}}),
            json!({"type":"tool-call-delta","index":0,"delta":{"message":{"tool_calls":{"function":{"arguments":"\"page\":2}"}}}}}),
            json!({"type":"tool-call-end","index":0}),
            json!({"type":"tool-call-end","index":1}),
            json!({"type":"message-end","delta":{"finish_reason":"TOOL_CALL","usage":{"tokens":{"input_tokens":100.0,"output_tokens":20},"billed_units":{"input_tokens":70,"output_tokens":18},"cached_tokens":25}}}),
        ];
        let value = read_sse(
            stream(events).as_bytes(),
            &CancellationToken::default(),
            &mut ignore,
        )
        .unwrap();
        assert_eq!(value["choices"][0]["message"]["tool_calls"][0]["id"], "a");
        assert_eq!(value["choices"][0]["message"]["tool_calls"][1]["id"], "b");
        assert_eq!(value["usage"]["prompt_tokens"], 100);
        assert_eq!(value["usage"]["total_tokens"], 120);
        assert_eq!(value["usage"]["billed_input_tokens"], 70);
        assert_eq!(value["usage"]["billed_output_tokens"], 18);
        let billed_only =
            model_usage(&json!({"usage":{"billed_units":{"input_tokens":3,"output_tokens":2}}}))
                .unwrap();
        assert_eq!(billed_only.input_tokens, None);
        assert_eq!(billed_only.total_tokens, None);
    }

    #[test]
    fn provider_cohere_stream_requires_finish_and_valid_complete_arguments() {
        let start = json!({"type":"message-start"});
        assert!(read_sse(
            stream(vec![start.clone()]).as_bytes(),
            &CancellationToken::default(),
            &mut ignore
        )
        .is_err());
        let unfinished = json!({"type":"tool-call-start","index":0,"delta":{"message":{"tool_calls":{"id":"x","function":{"name":"read","arguments":"{"}}}}});
        let end = json!({"type":"message-end","delta":{"finish_reason":"TOOL_CALL"}});
        assert!(read_sse(
            stream(vec![start.clone(), unfinished.clone(), end]).as_bytes(),
            &CancellationToken::default(),
            &mut ignore
        )
        .is_err());
        let length = json!({"type":"message-end","delta":{"finish_reason":"MAX_TOKENS"}});
        let result = read_sse(
            stream(vec![start, unfinished, length]).as_bytes(),
            &CancellationToken::default(),
            &mut ignore,
        )
        .unwrap();
        assert_eq!(result["choices"][0]["finish_reason"], "length");
        assert!(normalize(
            json!({"finish_reason":"ERROR","message":{"content":[]}}),
            &mut ignore,
            false
        )
        .is_err());
    }
}
