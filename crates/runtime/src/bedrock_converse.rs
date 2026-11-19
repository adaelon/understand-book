//! Native Bedrock Converse JSON and AWS EventStream codec.
//! Authentication and every actual send remain owned by the common provider loop.
use crate::{
    provider_stream::{ModelDelta, ModelObserver, ModelUsage},
    run_context::CancellationToken,
    AdapterError,
};
use base64::Engine;
use serde_json::{json, Value};
use std::{collections::BTreeMap, io::Read};

fn error(message: impl ToString) -> AdapterError {
    AdapterError {
        message: format!("Bedrock Converse: {}", message.to_string()),
        spend_stop: None,
    }
}
fn required<'a>(value: &'a Value, key: &str) -> Result<&'a str, AdapterError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| error(format!("missing {key}")))
}

pub(crate) fn endpoint(base_url: &str, model: &str) -> Result<String, AdapterError> {
    endpoint_with_stream(base_url, model, true)
}
pub(crate) fn endpoint_with_stream(
    base_url: &str,
    model: &str,
    stream: bool,
) -> Result<String, AdapterError> {
    let mut url = url::Url::parse(base_url).map_err(error)?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(error("endpoint must use HTTP(S)"));
    }
    let mut path = url
        .path_segments_mut()
        .map_err(|_| error("invalid endpoint"))?;
    path.pop_if_empty()
        .push("model")
        .push(model)
        .push(if stream {
            "converse-stream"
        } else {
            "converse"
        });
    drop(path);
    Ok(url.to_string())
}

fn content_blocks(content: &Value) -> Result<Vec<Value>, AdapterError> {
    match content {
        Value::Null => Ok(Vec::new()),
        Value::String(text) => Ok(if text.is_empty() {
            vec![]
        } else {
            vec![json!({"text":text})]
        }),
        Value::Array(parts) => parts
            .iter()
            .map(|part| match part["type"].as_str() {
                Some("text") => Ok(json!({"text":required(part,"text")?})),
                Some("image_url") => {
                    let data = required(&part["image_url"], "url")?;
                    let (kind, bytes) = data
                        .strip_prefix("data:image/")
                        .and_then(|s| s.split_once(";base64,"))
                        .ok_or_else(|| error("images require an inline base64 image"))?;
                    let format = match kind {
                        "jpg" | "jpeg" => "jpeg",
                        "png" => "png",
                        "gif" => "gif",
                        "webp" => "webp",
                        _ => return Err(error("unsupported image format")),
                    };
                    base64::engine::general_purpose::STANDARD
                        .decode(bytes)
                        .map_err(|_| error("invalid image base64"))?;
                    Ok(json!({"image":{"format":format,"source":{"bytes":bytes}}}))
                }
                _ => Err(error("unsupported message content block")),
            })
            .collect(),
        _ => Err(error("invalid message content")),
    }
}

pub(crate) fn project_request(chat_body: Value) -> Result<Value, AdapterError> {
    let messages = chat_body["messages"]
        .as_array()
        .ok_or_else(|| error("messages must be an array"))?;
    let mut system = Vec::new();
    let mut output = Vec::<Value>::new();
    for message in messages {
        let role = required(message, "role")?;
        if role == "system" || role == "developer" {
            let blocks = content_blocks(&message["content"])?;
            if blocks.iter().any(|b| b.get("text").is_none()) {
                return Err(error("system messages must contain text"));
            }
            system.extend(blocks);
            continue;
        }
        let (wire_role, mut blocks) = match role {
            "tool" => (
                "user",
                vec![
                    json!({"toolResult":{"toolUseId":required(message,"tool_call_id")?,"content":[{"text":message["content"].as_str().map(str::to_owned).unwrap_or_else(||message["content"].to_string())}]}}),
                ],
            ),
            "user" => ("user", content_blocks(&message["content"])?),
            "assistant" => {
                if let Some(blocks) = message["protocol_state"]["content"].as_array() {
                    ("assistant", blocks.clone())
                } else {
                    let mut blocks = content_blocks(&message["content"])?;
                    if let Some(calls) = message["tool_calls"].as_array() {
                        for call in calls {
                            let arguments = required(&call["function"], "arguments")?;
                            let input: Value = serde_json::from_str(arguments)
                                .map_err(|e| error(format!("invalid tool arguments: {e}")))?;
                            if !input.is_object() {
                                return Err(error("tool arguments must be a JSON object"));
                            }
                            blocks.push(json!({"toolUse":{"toolUseId":required(call,"id")?,"name":required(&call["function"],"name")?,"input":input}}));
                        }
                    }
                    ("assistant", blocks)
                }
            }
            _ => return Err(error("unsupported message role")),
        };
        if blocks.is_empty() {
            continue;
        }
        if let Some(previous) = output.last_mut().filter(|m| m["role"] == wire_role) {
            previous["content"]
                .as_array_mut()
                .unwrap()
                .append(&mut blocks);
        } else {
            output.push(json!({"role":wire_role,"content":blocks}));
        }
    }
    if chat_body["response_format"]["type"] == "json_object" {
        system
            .push(json!({"text":"Return exactly one valid JSON object, without Markdown fences."}));
    }
    let mut body =
        json!({"messages":output,"inferenceConfig":{"maxTokens":chat_body["max_tokens"]}});
    if !system.is_empty() {
        body["system"] = json!(system);
    }
    for (source, target) in [("temperature", "temperature"), ("top_p", "topP")] {
        if let Some(value) = chat_body.get(source) {
            body["inferenceConfig"][target] = value.clone();
        }
    }
    if let Some(stop) = chat_body.get("stop") {
        body["inferenceConfig"]["stopSequences"] = if stop.is_string() {
            json!([stop])
        } else {
            stop.clone()
        };
    }
    if let Some(tools) = chat_body["tools"]
        .as_array()
        .filter(|tools| !tools.is_empty())
    {
        let mut wire = Vec::new();
        for tool in tools {
            let function = &tool["function"];
            let mut spec = json!({"name":required(function,"name")?,"inputSchema":{"json":function["parameters"]}});
            if let Some(description) = function["description"].as_str().filter(|s| !s.is_empty()) {
                spec["description"] = json!(description);
            }
            wire.push(json!({"toolSpec":spec}));
        }
        body["toolConfig"] = json!({"tools":wire});
        match chat_body["tool_choice"].as_str() {
            Some("none") => {
                body.as_object_mut().unwrap().remove("toolConfig");
            }
            Some("required") => body["toolConfig"]["toolChoice"] = json!({"any":{}}),
            Some("auto") | None => {
                if let Some(name) = chat_body["tool_choice"]["function"]["name"].as_str() {
                    body["toolConfig"]["toolChoice"] = json!({"tool":{"name":name}});
                }
            }
            _ => return Err(error("unsupported tool choice")),
        }
    }
    if chat_body["response_format"]["type"] == "json_schema" {
        let source = &chat_body["response_format"]["json_schema"];
        let schema = source
            .get("schema")
            .ok_or_else(|| error("JSON Schema output requires schema"))?;
        let mut definition = json!({"schema":schema.to_string()});
        for key in ["name", "description"] {
            if let Some(value) = source.get(key) {
                definition[key] = value.clone();
            }
        }
        body["outputConfig"] =
            json!({"textFormat":{"type":"json_schema","structure":{"jsonSchema":definition}}});
    }
    // json_object has no Converse wire equivalent. The JSON instruction above
    // and the existing parser implement the configured text fallback.
    Ok(body)
}

pub(crate) fn model_usage(value: &Value) -> Option<ModelUsage> {
    let usage = value.get("usage")?;
    let count = |key: &str| usage[key].as_u64().and_then(|n| u32::try_from(n).ok());
    let snapshot = ModelUsage {
        input_tokens: count("inputTokens"),
        output_tokens: count("outputTokens"),
        total_tokens: count("totalTokens"),
        cached_input_tokens: count("cacheReadInputTokens"),
        cache_creation_input_tokens: count("cacheWriteInputTokens"),
        ..Default::default()
    };
    snapshot.has_any_value().then_some(snapshot)
}

fn finish_reason(reason: &str) -> Result<&'static str, AdapterError> {
    match reason {
        "end_turn" | "stop_sequence" => Ok("stop"),
        "tool_use" => Ok("tool_calls"),
        "max_tokens" => Ok("length"),
        "guardrail_intervened" | "content_filtered" => Ok("content_filter"),
        other => Err(error(format!("model stopped with {other}"))),
    }
}

fn normalize(
    content: Vec<Value>,
    reason: &str,
    usage: Option<ModelUsage>,
) -> Result<Value, AdapterError> {
    let mut text = String::new();
    let mut tools = Vec::new();
    for block in &content {
        if let Some(part) = block["text"].as_str() {
            text.push_str(part);
        }
        if let Some(tool) = block.get("toolUse") {
            let input = tool
                .get("input")
                .filter(|v| v.is_object())
                .ok_or_else(|| error("tool input is not an object"))?;
            tools.push(json!({"id":required(tool,"toolUseId")?,"type":"function","function":{"name":required(tool,"name")?,"arguments":input.to_string()}}));
        }
    }
    let reason = finish_reason(reason)?;
    if reason == "tool_calls" && tools.is_empty() {
        return Err(error("tool_use stop has no completed tool call"));
    }
    let mut result = json!({"choices":[{"message":{"role":"assistant","content":text,"tool_calls":tools,"protocol_state":{"content":content}},"finish_reason":reason}]});
    if let Some(usage) = usage {
        result["usage"] = usage.as_provider_usage();
    }
    Ok(result)
}

pub(crate) fn read_response(
    response: ureq::Response,
    cancellation: &CancellationToken,
    observer: &mut dyn ModelObserver,
) -> Result<Value, AdapterError> {
    let result = if response
        .header("Content-Type")
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        == "application/vnd.amazon.eventstream"
    {
        read_eventstream(response.into_reader(), cancellation, observer)
    } else {
        let value: Value = response.into_json().map_err(error)?;
        cancellation.check().map_err(|e| error(e.message))?;
        let usage = model_usage(&value);
        if let Some(usage) = &usage {
            observer.observe(ModelDelta::Usage(usage.clone()));
        }
        let blocks = value["output"]["message"]["content"]
            .as_array()
            .ok_or_else(|| error("response has no assistant content"))?;
        let reason = required(&value, "stopReason")?;
        let normalized = normalize(blocks.clone(), reason, usage)?;
        if let Some(text) = normalized["choices"][0]["message"]["content"]
            .as_str()
            .filter(|s| !s.is_empty())
        {
            observer.observe(ModelDelta::Text(text.into()));
        }
        observer.observe(ModelDelta::Finish(finish_reason(reason)?.into()));
        Ok(normalized)
    };
    if let Ok(value) = &result {
        observer.observe(ModelDelta::Response(std::sync::Arc::new(value.clone())));
    }
    result
}

#[derive(Default)]
struct Block {
    content: Value,
    arguments: String,
    stopped: bool,
}

/// Frame CRCs are part of the required AWS wire protocol, not stored fingerprints.
fn read_frame(
    reader: &mut impl Read,
) -> Result<Option<(BTreeMap<String, String>, Value)>, AdapterError> {
    let mut prelude = [0u8; 12];
    match reader.read(&mut prelude[..1]) {
        Ok(0) => return Ok(None),
        Ok(_) => {}
        Err(e) => return Err(error(e)),
    }
    reader
        .read_exact(&mut prelude[1..])
        .map_err(|e| error(format!("truncated EventStream prelude: {e}")))?;
    if crc32fast::hash(&prelude[..8]) != u32::from_be_bytes(prelude[8..12].try_into().unwrap()) {
        return Err(error("EventStream prelude CRC mismatch"));
    }
    let size = u32::from_be_bytes(prelude[..4].try_into().unwrap()) as usize;
    let headers_len = u32::from_be_bytes(prelude[4..8].try_into().unwrap()) as usize;
    if !(16..=25_165_824).contains(&size) || headers_len > size - 16 {
        return Err(error("invalid EventStream frame length"));
    }
    let mut rest = vec![0; size - 12];
    reader
        .read_exact(&mut rest)
        .map_err(|e| error(format!("truncated EventStream frame: {e}")))?;
    let mut crc = crc32fast::Hasher::new();
    crc.update(&prelude);
    crc.update(&rest[..rest.len() - 4]);
    if crc.finalize() != u32::from_be_bytes(rest[rest.len() - 4..].try_into().unwrap()) {
        return Err(error("EventStream message CRC mismatch"));
    }
    let mut headers = BTreeMap::new();
    let mut cursor = 0;
    while cursor < headers_len {
        let name_len = rest[cursor] as usize;
        cursor += 1;
        if cursor + name_len + 1 > headers_len {
            return Err(error("truncated EventStream header"));
        }
        let name = std::str::from_utf8(&rest[cursor..cursor + name_len])
            .map_err(error)?
            .to_owned();
        cursor += name_len;
        let kind = rest[cursor];
        cursor += 1;
        let len = match kind {
            0 | 1 => 0,
            2 => 1,
            3 => 2,
            4 => 4,
            5 | 8 => 8,
            9 => 16,
            6 | 7 => {
                if cursor + 2 > headers_len {
                    return Err(error("truncated EventStream header size"));
                }
                let n = u16::from_be_bytes(rest[cursor..cursor + 2].try_into().unwrap()) as usize;
                cursor += 2;
                n
            }
            _ => return Err(error("unknown EventStream header type")),
        };
        if cursor + len > headers_len {
            return Err(error("truncated EventStream header value"));
        }
        if kind == 7 {
            headers.insert(
                name,
                std::str::from_utf8(&rest[cursor..cursor + len])
                    .map_err(error)?
                    .to_owned(),
            );
        }
        cursor += len;
    }
    let value = serde_json::from_slice(&rest[headers_len..rest.len() - 4]).map_err(error)?;
    Ok(Some((headers, value)))
}

fn read_eventstream(
    mut reader: impl Read,
    cancellation: &CancellationToken,
    observer: &mut dyn ModelObserver,
) -> Result<Value, AdapterError> {
    let mut blocks = BTreeMap::<usize, Block>::new();
    let mut reason = None;
    let mut usage = None;
    let mut started = false;
    let mut metadata = false;
    loop {
        cancellation.check().map_err(|e| error(e.message))?;
        let Some((headers, value)) = read_frame(&mut reader)? else {
            break;
        };
        let kind = headers.get(":event-type").map(String::as_str).unwrap_or("");
        if headers
            .get(":message-type")
            .is_some_and(|v| v == "exception" || v == "error")
            || kind.ends_with("Exception")
        {
            return Err(error(format!(
                "{}: {}",
                if kind.is_empty() {
                    "stream error"
                } else {
                    kind
                },
                value["message"]
                    .as_str()
                    .unwrap_or("provider stream failed")
            )));
        }
        let index = || {
            value["contentBlockIndex"]
                .as_u64()
                .map(|n| n as usize)
                .ok_or_else(|| error("missing contentBlockIndex"))
        };
        match kind {
            "messageStart" => {
                if started || value["role"] != "assistant" {
                    return Err(error("invalid messageStart"));
                }
                started = true;
            }
            "contentBlockStart" => {
                if reason.is_some() || !started {
                    return Err(error("content started outside message"));
                }
                let index = index()?;
                if blocks.contains_key(&index) {
                    return Err(error("duplicate content block"));
                }
                let tool = &value["start"]["toolUse"];
                let content = if tool.is_object() {
                    json!({"toolUse":{"toolUseId":required(tool,"toolUseId")?,"name":required(tool,"name")?,"input":{}}})
                } else {
                    json!({})
                };
                blocks.insert(
                    index,
                    Block {
                        content,
                        ..Default::default()
                    },
                );
            }
            "contentBlockDelta" => {
                if reason.is_some() || !started {
                    return Err(error("content delta outside message"));
                }
                let index = index()?;
                let block = blocks.entry(index).or_insert_with(|| Block {
                    content: json!({}),
                    ..Default::default()
                });
                if block.stopped {
                    return Err(error("delta after block stop"));
                }
                let delta = &value["delta"];
                if let Some(text) = delta["text"].as_str() {
                    let current = block.content["text"].as_str().unwrap_or("").to_owned() + text;
                    block.content["text"] = json!(current);
                    observer.observe(ModelDelta::Text(text.into()));
                } else if let Some(arguments) = delta["toolUse"]["input"].as_str() {
                    if !block.content["toolUse"].is_object() {
                        return Err(error("tool delta has no start"));
                    }
                    block.arguments.push_str(arguments);
                    observer.observe(ModelDelta::ToolArguments {
                        index,
                        id: required(&block.content["toolUse"], "toolUseId")?.into(),
                        name: required(&block.content["toolUse"], "name")?.into(),
                        arguments: block.arguments.clone(),
                    });
                } else if let Some(reasoning) = delta.get("reasoningContent") {
                    if !block.content["reasoningContent"].is_object() {
                        block.content["reasoningContent"] = json!({});
                    }
                    for key in ["text", "signature"] {
                        if let Some(text) = reasoning[key].as_str() {
                            if !block.content["reasoningContent"]["reasoningText"].is_object() {
                                block.content["reasoningContent"]["reasoningText"] = json!({});
                            }
                            let current = block.content["reasoningContent"]["reasoningText"][key]
                                .as_str()
                                .unwrap_or("")
                                .to_owned()
                                + text;
                            block.content["reasoningContent"]["reasoningText"][key] =
                                json!(current);
                        }
                    }
                    if let Some(redacted) = reasoning["redactedContent"].as_str() {
                        let mut bytes = if let Some(previous) =
                            block.content["reasoningContent"]["redactedContent"].as_str()
                        {
                            base64::engine::general_purpose::STANDARD
                                .decode(previous)
                                .map_err(error)?
                        } else {
                            vec![]
                        };
                        bytes.extend(
                            base64::engine::general_purpose::STANDARD
                                .decode(redacted)
                                .map_err(error)?,
                        );
                        block.content["reasoningContent"]["redactedContent"] =
                            json!(base64::engine::general_purpose::STANDARD.encode(bytes));
                    }
                } else {
                    return Err(error("unsupported content delta"));
                }
            }
            "contentBlockStop" => {
                let block = blocks
                    .get_mut(&index()?)
                    .ok_or_else(|| error("stop for missing content block"))?;
                if block.stopped {
                    return Err(error("duplicate content stop"));
                }
                if block.content.get("toolUse").is_some() {
                    let input: Value = serde_json::from_str(&block.arguments)
                        .map_err(|e| error(format!("incomplete tool arguments: {e}")))?;
                    if !input.is_object() {
                        return Err(error("tool arguments must be an object"));
                    }
                    block.content["toolUse"]["input"] = input;
                }
                block.stopped = true;
            }
            "messageStop" => {
                if !started || reason.is_some() || blocks.values().any(|b| !b.stopped) {
                    return Err(error("message stopped with incomplete content"));
                }
                let stop = required(&value, "stopReason")?;
                observer.observe(ModelDelta::Finish(finish_reason(stop)?.into()));
                reason = Some(stop.to_owned());
            }
            "metadata" => {
                if reason.is_none() {
                    return Err(error("metadata before message stop"));
                }
                metadata = true;
                usage = model_usage(&value);
                if let Some(usage) = &usage {
                    observer.observe(ModelDelta::Usage(usage.clone()));
                }
            }
            _ => return Err(error(format!("unsupported EventStream event {kind}"))),
        }
    }
    let reason = reason.ok_or_else(|| error("stream ended before messageStop"))?;
    if !metadata {
        return Err(error("stream ended before metadata"));
    }
    normalize(
        blocks.into_values().map(|b| b.content).collect(),
        &reason,
        usage,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(kind: &str, body: Value) -> Vec<u8> {
        let mut headers = Vec::new();
        for (name, value) in [(":message-type", "event"), (":event-type", kind)] {
            headers.push(name.len() as u8);
            headers.extend_from_slice(name.as_bytes());
            headers.push(7);
            headers.extend_from_slice(&(value.len() as u16).to_be_bytes());
            headers.extend_from_slice(value.as_bytes());
        }
        let payload = serde_json::to_vec(&body).unwrap();
        let size = 16 + headers.len() + payload.len();
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&(size as u32).to_be_bytes());
        bytes.extend_from_slice(&(headers.len() as u32).to_be_bytes());
        bytes.extend_from_slice(&crc32fast::hash(&bytes).to_be_bytes());
        bytes.extend(headers);
        bytes.extend(payload);
        bytes.extend_from_slice(&crc32fast::hash(&bytes).to_be_bytes());
        bytes
    }
    fn stream(events: Vec<(&str, Value)>) -> Vec<u8> {
        events
            .into_iter()
            .flat_map(|(kind, body)| frame(kind, body))
            .collect()
    }
    #[test]
    fn bedrock_projection_round_trips_tool_ids_and_private_reasoning() {
        let body=project_request(json!({"max_tokens":128,"messages":[{"role":"system","content":"source"},{"role":"user","content":"read"},{"role":"assistant","content":null,"tool_calls":[{"id":"call1","function":{"name":"book_read","arguments":"{\"lid\":\"one\"}"}}]},{"role":"tool","tool_call_id":"call1","content":"passage"}],"tools":[{"function":{"name":"book_read","parameters":{"type":"object"}}}],"tool_choice":"required"})).unwrap();
        assert_eq!(
            body["messages"][1]["content"][0]["toolUse"]["toolUseId"],
            "call1"
        );
        assert_eq!(
            body["messages"][2]["content"][0]["toolResult"]["toolUseId"],
            "call1"
        );
        assert_eq!(body["toolConfig"]["toolChoice"], json!({"any":{}}));
        assert!(body.get("model").is_none());
        assert!(body.get("stream").is_none());
        let private = json!({"content":[{"reasoningContent":{"reasoningText":{"text":"private","signature":"sig"}}},{"text":"answer"}]});
        let body=project_request(json!({"max_tokens":12,"messages":[{"role":"assistant","content":"answer","protocol_state":private}]})).unwrap();
        assert_eq!(
            body["messages"][0]["content"][0]["reasoningContent"]["reasoningText"]["signature"],
            "sig"
        );
    }
    #[test]
    fn bedrock_stream_assembles_tools_by_index_and_preserves_usage() {
        let bytes = stream(vec![
            ("messageStart", json!({"role":"assistant"})),
            (
                "contentBlockStart",
                json!({"contentBlockIndex":0,"start":{"toolUse":{"toolUseId":"a","name":"read"}}}),
            ),
            (
                "contentBlockStart",
                json!({"contentBlockIndex":1,"start":{"toolUse":{"toolUseId":"b","name":"find"}}}),
            ),
            (
                "contentBlockDelta",
                json!({"contentBlockIndex":1,"delta":{"toolUse":{"input":"{\"q\":"}}}),
            ),
            (
                "contentBlockDelta",
                json!({"contentBlockIndex":0,"delta":{"toolUse":{"input":"{}"}}}),
            ),
            (
                "contentBlockDelta",
                json!({"contentBlockIndex":1,"delta":{"toolUse":{"input":"\"x\"}"}}}),
            ),
            ("contentBlockStop", json!({"contentBlockIndex":0})),
            ("contentBlockStop", json!({"contentBlockIndex":1})),
            ("messageStop", json!({"stopReason":"tool_use"})),
            (
                "metadata",
                json!({"usage":{"inputTokens":12,"outputTokens":8,"totalTokens":20,"cacheReadInputTokens":4}}),
            ),
        ]);
        let mut events = Vec::new();
        let result = read_eventstream(bytes.as_slice(), &CancellationToken::default(), &mut |e| {
            events.push(e)
        })
        .unwrap();
        assert_eq!(
            result["choices"][0]["message"]["tool_calls"][1]["function"]["arguments"],
            "{\"q\":\"x\"}"
        );
        assert_eq!(result["usage"]["prompt_tokens"], 12);
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, ModelDelta::Usage(_)))
                .count(),
            1
        );
    }
    #[test]
    fn bedrock_stream_rejects_truncation_corruption_and_incomplete_arguments() {
        let start = frame("messageStart", json!({"role":"assistant"}));
        assert!(
            read_eventstream(start.as_slice(), &CancellationToken::default(), &mut |_| {}).is_err()
        );
        let mut corrupt = start.clone();
        let last = corrupt.len() - 1;
        corrupt[last] ^= 1;
        assert!(read_eventstream(
            corrupt.as_slice(),
            &CancellationToken::default(),
            &mut |_| {}
        )
        .is_err());
        assert!(read_eventstream(
            &start[..start.len() - 2],
            &CancellationToken::default(),
            &mut |_| {}
        )
        .is_err());
        let incomplete = stream(vec![
            ("messageStart", json!({"role":"assistant"})),
            (
                "contentBlockStart",
                json!({"contentBlockIndex":0,"start":{"toolUse":{"toolUseId":"a","name":"read"}}}),
            ),
            (
                "contentBlockDelta",
                json!({"contentBlockIndex":0,"delta":{"toolUse":{"input":"{"}}}),
            ),
            ("contentBlockStop", json!({"contentBlockIndex":0})),
        ]);
        assert!(read_eventstream(
            incomplete.as_slice(),
            &CancellationToken::default(),
            &mut |_| {}
        )
        .is_err());
    }
    #[test]
    fn bedrock_normalizes_refusal_length_and_unknown_usage() {
        assert_eq!(
            normalize(vec![], "max_tokens", None).unwrap()["choices"][0]["finish_reason"],
            "length"
        );
        assert_eq!(
            normalize(vec![], "guardrail_intervened", None).unwrap()["choices"][0]["finish_reason"],
            "content_filter"
        );
        assert!(model_usage(&json!({"usage":{}})).is_none());
        assert_eq!(endpoint("https://bedrock-runtime.us-east-1.amazonaws.com","arn:aws:bedrock:us-east-1:1:inference-profile/a").unwrap(),"https://bedrock-runtime.us-east-1.amazonaws.com/model/arn:aws:bedrock:us-east-1:1:inference-profile%2Fa/converse-stream");
    }
}
