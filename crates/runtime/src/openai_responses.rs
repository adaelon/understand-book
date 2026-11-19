//! OpenAI Responses wire codec. Local history is the authority (`store: false`).
use crate::{
    provider_stream::{ModelDelta, ModelObserver, ModelUsage},
    run_context::CancellationToken,
    AdapterError,
};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{BufRead, BufReader, Read},
    sync::Arc,
};

fn error(message: impl ToString) -> AdapterError {
    AdapterError {
        spend_stop: None,
        message: message.to_string(),
    }
}

pub(crate) fn endpoint(base_url: &str, _model: &str) -> Result<String, AdapterError> {
    let mut url = url::Url::parse(base_url).map_err(error)?;
    let path = url.path().trim_end_matches('/');
    if !path.ends_with("/responses") {
        url.set_path(&format!(
            "{}/responses",
            if path.is_empty() { "/v1" } else { path }
        ));
    }
    Ok(url.into())
}

fn content(value: &Value, assistant: bool) -> Result<Value, AdapterError> {
    // Synthetic history uses EasyInputMessage. Output-message content requires
    // the original response item's id/status, which only native replay has.
    let text_type = "input_text";
    if let Some(text) = value.as_str() {
        return Ok(json!([{"type":text_type,"text":text}]));
    }
    if value.is_null() {
        return Ok(json!([]));
    }
    let parts = value
        .as_array()
        .ok_or_else(|| error("Responses message content must be text or parts"))?;
    let mut result = Vec::new();
    for part in parts {
        result.push(match part["type"].as_str() {
            Some("text") => json!({"type":text_type,"text":part["text"]}),
            Some("image_url") if !assistant => {
                let image = &part["image_url"];
                let address = image["url"].as_str().ok_or_else(|| error("Responses image is missing its URL"))?;
                json!({"type":"input_image","image_url":address,"detail":image["detail"].as_str().unwrap_or("auto")})
            }
            kind => return Err(error(format!("Responses unsupported message content {kind:?}"))),
        });
    }
    Ok(Value::Array(result))
}

fn continuation_state(items: &[Value]) -> Vec<Value> {
    items
        .iter()
        .map(|item| {
            let mut saved = item.clone();
            if let Some(object) = saved.as_object_mut() {
                object.remove("created_by");
            }
            if saved["type"] == "message" {
                if let Some(parts) = saved["content"].as_array_mut() {
                    for part in parts {
                        if part["type"] == "output_text" {
                            part["_text_bytes"] = json!(part["text"].as_str().unwrap_or("").len());
                            part.as_object_mut().unwrap().remove("text");
                        }
                    }
                }
            } else if saved["type"] == "function_call" {
                saved.as_object_mut().unwrap().remove("arguments");
            }
            saved
        })
        .collect()
}

fn replay_items(message: &Value, items: &[Value]) -> Result<Vec<Value>, AdapterError> {
    let text = message["content"].as_str().unwrap_or("");
    let text_bytes: usize = items
        .iter()
        .filter_map(|item| item["content"].as_array())
        .flatten()
        .filter(|part| part["type"] == "output_text")
        .map(|part| {
            part["_text_bytes"]
                .as_u64()
                .map(|n| n as usize)
                .unwrap_or_else(|| part["text"].as_str().unwrap_or("").len())
        })
        .sum();
    let preserve_segments = text_bytes == text.len();
    let calls = message["tool_calls"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    let mut offset = 0;
    let mut text_written = false;
    for item in items {
        let mut restored = item.clone();
        if let Some(object) = restored.as_object_mut() {
            object.remove("created_by");
        }
        match item["type"].as_str() {
            Some("reasoning") => result.push(restored),
            Some("message") => {
                let mut parts = Vec::new();
                for part in item["content"]
                    .as_array()
                    .ok_or_else(|| error("Responses saved message is missing content"))?
                {
                    if part["type"] != "output_text" {
                        continue;
                    }
                    let mut updated = part.clone();
                    updated.as_object_mut().unwrap().remove("_text_bytes");
                    if preserve_segments {
                        let length = part["_text_bytes"]
                            .as_u64()
                            .map(|n| n as usize)
                            .unwrap_or_else(|| part["text"].as_str().unwrap_or("").len());
                        let end = offset + length;
                        updated["text"] = json!(text
                            .get(offset..end)
                            .ok_or_else(|| error("Responses saved text boundary is invalid"))?);
                        offset = end;
                    } else if !text_written {
                        updated["text"] = json!(text);
                        updated["annotations"] = json!([]);
                    } else {
                        continue;
                    }
                    text_written = true;
                    if !updated["text"].as_str().unwrap_or("").is_empty() {
                        parts.push(updated);
                    }
                }
                if !parts.is_empty() {
                    restored["content"] = Value::Array(parts);
                    result.push(restored);
                }
            }
            Some("function_call") => {
                if let Some(call) = calls.iter().find(|call| call["id"] == item["call_id"]) {
                    restored["name"] = call["function"]["name"].clone();
                    restored["arguments"] = call["function"]["arguments"].clone();
                    seen.insert(call["id"].as_str().unwrap_or("").to_string());
                    result.push(restored);
                }
            }
            kind => return Err(error(format!("Responses unsupported saved item {kind:?}"))),
        }
    }
    if !text_written && !text.is_empty() {
        result.push(json!({"role":"assistant","content":text}));
    }
    for call in &calls {
        if !seen.contains(call["id"].as_str().unwrap_or("")) {
            result.push(json!({"type":"function_call","call_id":call["id"],"name":call["function"]["name"],"arguments":call["function"]["arguments"]}));
        }
    }
    Ok(result)
}

/// Convert the runtime's normalized Chat request; private items have already
/// been checked against the frozen service, protocol and model by the caller.
pub(crate) fn project_request(chat: Value) -> Result<Value, AdapterError> {
    let messages = chat["messages"]
        .as_array()
        .ok_or_else(|| error("Responses request is missing messages"))?;
    let mut input = Vec::new();
    for message in messages {
        let role = message["role"]
            .as_str()
            .ok_or_else(|| error("Responses message is missing its role"))?;
        if role == "assistant" {
            if let Some(items) = message["protocol_state"].as_array() {
                // Keep item ordering: reasoning must precede the function call
                // that it belongs to. Re-projecting also would duplicate it.
                input.extend(replay_items(message, items)?);
                continue;
            }
        }
        if role == "tool" {
            let call_id = message["tool_call_id"]
                .as_str()
                .filter(|s| !s.is_empty())
                .ok_or_else(|| error("Responses tool result is missing call_id"))?;
            input.push(json!({"type":"function_call_output","call_id":call_id,"output":message["content"]}));
            continue;
        }
        if !matches!(role, "system" | "developer" | "user" | "assistant") {
            return Err(error(format!("Responses unsupported role {role}")));
        }
        let parts = content(&message["content"], role == "assistant")?;
        if parts.as_array().is_some_and(|parts| !parts.is_empty()) {
            input.push(json!({"type":"message","role":role,"content":parts}));
        }
        if let Some(calls) = message["tool_calls"].as_array() {
            for call in calls {
                input.push(json!({"type":"function_call","call_id":call["id"],
                    "name":call["function"]["name"],"arguments":call["function"]["arguments"]}));
            }
        }
    }
    let mut body = json!({"model":chat["model"],"input":input,"store":false,
        "stream":chat["stream"].as_bool().unwrap_or(true)});
    if let Some(limit) = chat.get("max_tokens") {
        body["max_output_tokens"] = limit.clone();
    }
    if let Some(temperature) = chat.get("temperature") {
        body["temperature"] = temperature.clone();
    }
    if let Some(effort) = chat.get("reasoning_effort") {
        body["reasoning"] = json!({"effort":effort});
    }
    if let Some(tools) = chat["tools"].as_array().filter(|tools| !tools.is_empty()) {
        let mut projected = Vec::new();
        for tool in tools {
            if tool["type"] != "function" {
                return Err(error("Responses only accepts registered function tools"));
            }
            let function = &tool["function"];
            projected.push(json!({"type":"function","name":function["name"],
                "description":function["description"],"parameters":function["parameters"],
                "strict":function["strict"].as_bool().unwrap_or(false)}));
        }
        body["tools"] = Value::Array(projected);
        if let Some(choice) = chat.get("tool_choice") {
            body["tool_choice"] = if choice.is_string() {
                choice.clone()
            } else {
                json!({"type":"function","name":choice["function"]["name"]})
            };
        }
        if let Some(parallel) = chat.get("parallel_tool_calls") {
            body["parallel_tool_calls"] = parallel.clone();
        }
    }
    if let Some(format) = chat.get("response_format") {
        body["text"] = json!({"format": match format["type"].as_str() {
            Some("json_object") => json!({"type":"json_object"}),
            Some("json_schema") => {
                let mut schema = format["json_schema"].clone();
                schema["type"] = json!("json_schema");
                schema
            }
            Some("text") => json!({"type":"text"}),
            _ => return Err(error("Responses unsupported output format")),
        }});
    }
    Ok(body)
}

pub(crate) fn model_usage(value: &Value) -> Option<ModelUsage> {
    let value = if value.get("response").is_some() {
        &value["response"]
    } else {
        value
    };
    let mut usage = crate::provider_stream::model_usage(value)?;
    if let Some(tokens) = value["usage"]["input_tokens_details"]["cache_write_tokens"]
        .as_u64()
        .and_then(|n| u32::try_from(n).ok())
    {
        usage.cache_creation_input_tokens = Some(tokens);
    }
    Some(usage)
}

fn normalized(value: &Value) -> Result<Value, AdapterError> {
    if !value["error"].is_null() {
        return Err(error(format!("Responses error: {}", value["error"])));
    }
    let limited = match value["status"].as_str() {
        Some("completed") => false,
        Some("incomplete") if value["incomplete_details"]["reason"] == "max_output_tokens" => true,
        status => {
            return Err(error(format!(
                "Responses did not complete: {status:?}; {}",
                value["incomplete_details"]
            )))
        }
    };
    let output = value["output"]
        .as_array()
        .ok_or_else(|| error("Responses response is missing output items"))?;
    let mut text = String::new();
    let mut calls = Vec::new();
    for item in output {
        match item["type"].as_str() {
            Some("message") => {
                for part in item["content"]
                    .as_array()
                    .ok_or_else(|| error("Responses message is missing content"))?
                {
                    match part["type"].as_str() {
                        Some("output_text") => text.push_str(
                            part["text"]
                                .as_str()
                                .ok_or_else(|| error("Responses output_text is not text"))?,
                        ),
                        Some("refusal") => {
                            return Err(error(format!(
                                "Responses refused the request: {}",
                                part["refusal"]
                            )))
                        }
                        kind => {
                            return Err(error(format!(
                                "Responses unsupported output content {kind:?}"
                            )))
                        }
                    }
                }
            }
            Some("function_call") => {
                let call_id = item["call_id"].as_str().filter(|s| !s.is_empty());
                let name = item["name"].as_str().filter(|s| !s.is_empty());
                let arguments = item["arguments"].as_str().unwrap_or("");
                if !limited
                    && (call_id.is_none()
                        || name.is_none()
                        || !serde_json::from_str::<Value>(arguments)
                            .is_ok_and(|args| args.is_object()))
                {
                    return Err(error("Responses returned an incomplete function call"));
                }
                calls.push(json!({"id":call_id.unwrap_or(""),"type":"function",
                    "function":{"name":name.unwrap_or(""),"arguments":arguments}}));
            }
            Some("reasoning") => {}
            kind => return Err(error(format!("Responses unsupported output item {kind:?}"))),
        }
    }
    let finish = if limited {
        "length"
    } else if calls.is_empty() {
        "stop"
    } else {
        "tool_calls"
    };
    if !limited && text.is_empty() && calls.is_empty() {
        return Err(error("Responses returned no answer or function call"));
    }
    Ok(
        json!({"id":value["id"],"model":value["model"],"choices":[{"index":0,
        "message":{"role":"assistant","content":text,"tool_calls":calls,"protocol_state":continuation_state(output)},"finish_reason":finish}],
        "usage":model_usage(value).unwrap_or_default().as_provider_usage()}),
    )
}

pub(crate) fn read_response(
    response: ureq::Response,
    cancellation: &CancellationToken,
    observer: &mut dyn ModelObserver,
) -> Result<Value, AdapterError> {
    cancellation.check().map_err(|e| error(e.message))?;
    let result = if response
        .header("Content-Type")
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .eq_ignore_ascii_case("text/event-stream")
    {
        read_sse(response.into_reader(), cancellation, observer)
    } else {
        let value: Value = response.into_json().map_err(error)?;
        if let Some(usage) = model_usage(&value) {
            observer.observe(ModelDelta::Usage(usage));
        }
        cancellation.check().map_err(|e| error(e.message))?;
        let normalized = normalized(&value)?;
        let message = &normalized["choices"][0]["message"];
        if let Some(text) = message["content"].as_str().filter(|text| !text.is_empty()) {
            observer.observe(ModelDelta::Text(text.into()));
        }
        for (index, call) in message["tool_calls"].as_array().unwrap().iter().enumerate() {
            observer.observe(ModelDelta::ToolArguments {
                index,
                id: call["id"].as_str().unwrap_or("").into(),
                name: call["function"]["name"].as_str().unwrap_or("").into(),
                arguments: call["function"]["arguments"].as_str().unwrap_or("").into(),
            });
        }
        observer.observe(ModelDelta::Finish(
            normalized["choices"][0]["finish_reason"]
                .as_str()
                .unwrap()
                .into(),
        ));
        Ok(normalized)
    };
    if let Ok(value) = &result {
        observer.observe(ModelDelta::Response(Arc::new(value.clone())));
    }
    result
}

fn read_sse(
    reader: impl Read,
    cancellation: &CancellationToken,
    observer: &mut dyn ModelObserver,
) -> Result<Value, AdapterError> {
    let mut reader = BufReader::new(reader);
    let mut data = String::new();
    let mut calls: BTreeMap<usize, Value> = BTreeMap::new();
    loop {
        cancellation.check().map_err(|e| error(e.message))?;
        let mut line = String::new();
        if reader.read_line(&mut line).map_err(error)? == 0 {
            return Err(error(
                "Responses stream ended before its terminal response event",
            ));
        }
        cancellation.check().map_err(|e| error(e.message))?;
        let line = line.trim_end_matches(['\r', '\n']);
        if !line.is_empty() {
            if let Some(fragment) = line.strip_prefix("data:") {
                if !data.is_empty() {
                    data.push('\n');
                }
                data.push_str(fragment.strip_prefix(' ').unwrap_or(fragment));
            }
            continue;
        }
        if data.is_empty() {
            continue;
        }
        let event: Value = serde_json::from_str(&std::mem::take(&mut data)).map_err(error)?;
        if let Some(usage) = model_usage(&event) {
            observer.observe(ModelDelta::Usage(usage));
        }
        match event["type"].as_str() {
            Some("response.output_text.delta") => {
                let delta = event["delta"]
                    .as_str()
                    .ok_or_else(|| error("Responses text delta is not text"))?;
                observer.observe(ModelDelta::Text(delta.into()));
            }
            Some("response.output_item.added") if event["item"]["type"] == "function_call" => {
                let index = event["output_index"]
                    .as_u64()
                    .ok_or_else(|| error("Responses item is missing output_index"))?
                    as usize;
                calls.insert(index, event["item"].clone());
            }
            Some(
                "response.function_call_arguments.delta" | "response.function_call_arguments.done",
            ) => {
                let index = event["output_index"]
                    .as_u64()
                    .ok_or_else(|| error("Responses function delta is missing output_index"))?
                    as usize;
                let call = calls
                    .get_mut(&index)
                    .ok_or_else(|| error("Responses function delta precedes its output item"))?;
                if event["item_id"] != call["id"] {
                    return Err(error("Responses function delta has a different item_id"));
                }
                let arguments = if event["type"] == "response.function_call_arguments.done" {
                    event["arguments"]
                        .as_str()
                        .ok_or_else(|| error("Responses function arguments are not text"))?
                        .to_string()
                } else {
                    format!(
                        "{}{}",
                        call["arguments"].as_str().unwrap_or(""),
                        event["delta"]
                            .as_str()
                            .ok_or_else(|| error("Responses function delta is not text"))?
                    )
                };
                call["arguments"] = json!(arguments);
                observer.observe(ModelDelta::ToolArguments {
                    index,
                    id: call["call_id"].as_str().unwrap_or("").into(),
                    name: call["name"].as_str().unwrap_or("").into(),
                    arguments,
                });
            }
            Some("response.completed" | "response.incomplete" | "response.failed") => {
                let value = normalized(&event["response"])?;
                observer.observe(ModelDelta::Finish(
                    value["choices"][0]["finish_reason"]
                        .as_str()
                        .unwrap()
                        .into(),
                ));
                return Ok(value);
            }
            Some("error") => return Err(error(format!("Responses stream error: {event}"))),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn stream(events: Vec<Value>) -> String {
        events
            .iter()
            .map(|event| format!("data: {event}\n\n"))
            .collect()
    }
    fn response(output: Value) -> Value {
        json!({"id":"r1","status":"completed","output":output,
        "usage":{"input_tokens":20,"output_tokens":7,"total_tokens":27,"input_tokens_details":{"cached_tokens":5},"output_tokens_details":{"reasoning_tokens":3}}})
    }

    #[test]
    fn provider_responses_projects_tools_images_and_stateless_replay() {
        let state = json!([{"type":"reasoning","id":"r","encrypted_content":"sealed","summary":[]},
            {"type":"function_call","id":"fc","call_id":"call1","name":"read","arguments":"{}"}]);
        let body = project_request(json!({"model":"reasoner","max_tokens":123,"messages":[
            {"role":"system","content":"Read"},{"role":"user","content":[{"type":"text","text":"look"},{"type":"image_url","image_url":{"url":"data:image/png;base64,AA==","detail":"high"}}]},
            {"role":"assistant","content":null,"protocol_state":state,"tool_calls":[{"id":"call1","type":"function","function":{"name":"read","arguments":"{}"}}]},
            {"role":"tool","tool_call_id":"call1","content":"found"}],
            "tools":[{"type":"function","function":{"name":"read","description":"Read","parameters":{"type":"object"}}}],
            "tool_choice":"auto","parallel_tool_calls":true,"response_format":{"type":"json_object"}})).unwrap();
        assert_eq!(body["store"], false);
        assert_eq!(body["max_output_tokens"], 123);
        assert!(body.get("temperature").is_none());
        assert_eq!(body["input"][1]["content"][1]["type"], "input_image");
        assert_eq!(body["input"][2]["encrypted_content"], "sealed");
        assert_eq!(
            body["input"][4],
            json!({"type":"function_call_output","call_id":"call1","output":"found"})
        );
        assert_eq!(body["tools"][0]["strict"], false);
        assert_eq!(body["text"]["format"]["type"], "json_object");
    }

    #[test]
    fn provider_responses_stream_keeps_parallel_ids_usage_and_reasoning() {
        let output = json!([{"type":"reasoning","encrypted_content":"sealed","summary":[]},
            {"type":"function_call","id":"fc1","call_id":"c1","name":"read","arguments":"{\"x\":1}"},
            {"type":"function_call","id":"fc2","call_id":"c2","name":"find","arguments":"{}"}]);
        let data = stream(vec![
            json!({"type":"response.output_item.added","output_index":1,"item":{"type":"function_call","id":"fc1","call_id":"c1","name":"read","arguments":""}}),
            json!({"type":"response.output_item.added","output_index":2,"item":{"type":"function_call","id":"fc2","call_id":"c2","name":"find","arguments":""}}),
            json!({"type":"response.function_call_arguments.delta","output_index":1,"item_id":"fc1","delta":"{\"x\":"}),
            json!({"type":"response.function_call_arguments.delta","output_index":2,"item_id":"fc2","delta":"{}"}),
            json!({"type":"response.function_call_arguments.delta","output_index":1,"item_id":"fc1","delta":"1}"}),
            json!({"type":"response.completed","response":response(output.clone())}),
        ]);
        let mut events = Vec::new();
        let value = read_sse(
            data.as_bytes(),
            &CancellationToken::default(),
            &mut |delta| events.push(delta),
        )
        .unwrap();
        assert_eq!(value["choices"][0]["finish_reason"], "tool_calls");
        assert_eq!(value["choices"][0]["message"]["tool_calls"][0]["id"], "c1");
        assert_eq!(
            value["choices"][0]["message"]["protocol_state"],
            Value::Array(continuation_state(output.as_array().unwrap()))
        );
        assert!(events.iter().any(|delta| matches!(delta,ModelDelta::ToolArguments {id,arguments,..} if id=="c1" && arguments=="{\"x\":1}")));
        let usage = model_usage(&value).unwrap();
        assert_eq!(usage.total_tokens, Some(27));
        assert_eq!(usage.reasoning_output_tokens, Some(3));
        assert_eq!(usage.cached_input_tokens, Some(5));
    }

    #[test]
    fn provider_responses_requires_terminal_and_rejects_refusal_or_invalid_calls() {
        let partial = stream(vec![
            json!({"type":"response.output_text.delta","delta":"half"}),
        ]);
        assert!(read_sse(
            partial.as_bytes(),
            &CancellationToken::default(),
            &mut crate::provider_stream::ignore
        )
        .is_err());
        assert!(normalized(&response(
            json!([{"type":"message","content":[{"type":"refusal","refusal":"no"}]}])
        ))
        .is_err());
        assert!(normalized(&response(
            json!([{"type":"function_call","call_id":"c","name":"read","arguments":"{"}])
        ))
        .is_err());
        let mut limited = response(json!([]));
        limited["status"] = json!("incomplete");
        limited["incomplete_details"] = json!({"reason":"max_output_tokens"});
        assert_eq!(
            normalized(&limited).unwrap()["choices"][0]["finish_reason"],
            "length"
        );
    }

    #[test]
    fn provider_responses_endpoint_preserves_version_query() {
        assert_eq!(
            endpoint(
                "https://example.test/openai/responses?api-version=2026-01-01",
                "m"
            )
            .unwrap(),
            "https://example.test/openai/responses?api-version=2026-01-01"
        );
        assert_eq!(
            endpoint("https://api.openai.com", "m").unwrap(),
            "https://api.openai.com/v1/responses"
        );
    }

    #[test]
    fn provider_responses_preserves_explicit_temperature() {
        let body = project_request(
            json!({"model":"m","temperature":0.0,"messages":[{"role":"user","content":"hello"}]}),
        )
        .unwrap();
        assert_eq!(body["temperature"], 0.0);
    }

    #[test]
    fn provider_responses_replay_uses_retired_history_and_remapped_tools() {
        let output = json!([
            {"type":"reasoning","id":"r","encrypted_content":"sealed","summary":[]},
            {"type":"message","id":"m","status":"completed","role":"assistant","phase":"commentary","created_by":"provider",
                "content":[{"type":"output_text","text":"long original answer","annotations":[]}]},
            {"type":"function_call","id":"f","call_id":"c","name":"old_name","arguments":"{\"long\":\"original payload\"}"}
        ]);
        let state = continuation_state(output.as_array().unwrap());
        let serialized = serde_json::to_string(&state).unwrap();
        assert!(!serialized.contains("long original answer"));
        assert!(!serialized.contains("original payload"));
        let body = project_request(json!({"model":"m","messages":[{"role":"assistant","content":"summary","protocol_state":state,
            "tool_calls":[{"id":"c","function":{"name":"new_name","arguments":"{}"}}]}]})).unwrap();
        assert_eq!(body["input"][1]["content"][0]["text"], "summary");
        assert_eq!(body["input"][1]["phase"], "commentary");
        assert!(body["input"][1].get("created_by").is_none());
        assert!(body["input"][1]["content"][0].get("_text_bytes").is_none());
        assert_eq!(body["input"][2]["arguments"], "{}");
        assert_eq!(body["input"][2]["name"], "new_name");
    }
}
