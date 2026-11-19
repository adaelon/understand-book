//! Anthropic Messages codec, separate from DeepSeek's tool-update extension.
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
    if !path.ends_with("/messages") {
        url.set_path(&format!(
            "{}/messages",
            if path.is_empty() { "/v1" } else { path }
        ));
    }
    Ok(url.into())
}

fn content(value: &Value) -> Result<Vec<Value>, AdapterError> {
    if let Some(text) = value.as_str() {
        return Ok(if text.is_empty() {
            Vec::new()
        } else {
            vec![json!({"type":"text","text":text})]
        });
    }
    if value.is_null() {
        return Ok(Vec::new());
    }
    let parts = value
        .as_array()
        .ok_or_else(|| error("Messages content must be text or parts"))?;
    let mut result = Vec::new();
    for part in parts {
        result.push(match part["type"].as_str() {
            Some("text") => json!({"type":"text","text":part["text"]}),
            Some("image_url") => {
                let image = part["image_url"]["url"].as_str().ok_or_else(|| error("Messages image is missing its URL"))?;
                if let Some(data) = image.strip_prefix("data:") {
                    let (media_type, encoded) = data.split_once(";base64,").ok_or_else(|| error("Messages image data URL must contain base64 data"))?;
                    json!({"type":"image","source":{"type":"base64","media_type":media_type,"data":encoded}})
                } else {
                    json!({"type":"image","source":{"type":"url","url":image}})
                }
            }
            kind => return Err(error(format!("Messages unsupported content {kind:?}"))),
        });
    }
    Ok(result)
}

fn append_message(messages: &mut Vec<Value>, role: &str, blocks: Vec<Value>) {
    if blocks.is_empty() {
        return;
    }
    if let Some(last) = messages.last_mut().filter(|last| last["role"] == role) {
        last["content"].as_array_mut().unwrap().extend(blocks);
    } else {
        messages.push(json!({"role":role,"content":blocks}));
    }
}

fn continuation_state(blocks: &[Value]) -> Vec<Value> {
    blocks
        .iter()
        .map(|block| {
            let mut saved = block.clone();
            if saved["type"] == "text" {
                saved["_text_bytes"] = json!(saved["text"].as_str().unwrap_or("").len());
                saved.as_object_mut().unwrap().remove("text");
            } else if saved["type"] == "tool_use" {
                saved.as_object_mut().unwrap().remove("input");
            }
            saved
        })
        .collect()
}

fn tool_block(call: &Value) -> Result<Value, AdapterError> {
    let arguments = call["function"]["arguments"]
        .as_str()
        .ok_or_else(|| error("Messages tool arguments are not text"))?;
    let input: Value = serde_json::from_str(arguments).map_err(error)?;
    if !input.is_object() {
        return Err(error("Messages tool arguments must be an object"));
    }
    Ok(json!({"type":"tool_use","id":call["id"],"name":call["function"]["name"],"input":input}))
}

fn replay_blocks(message: &Value, blocks: &[Value]) -> Result<Vec<Value>, AdapterError> {
    let text = message["content"].as_str().unwrap_or("");
    let calls = message["tool_calls"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let text_bytes: usize = blocks
        .iter()
        .filter(|block| block["type"] == "text")
        .map(|block| {
            block["_text_bytes"]
                .as_u64()
                .map(|n| n as usize)
                .unwrap_or_else(|| block["text"].as_str().unwrap_or("").len())
        })
        .sum();
    let preserve_segments = text_bytes == text.len();
    let mut offset = 0;
    let mut text_written = false;
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    for block in blocks {
        match block["type"].as_str() {
            Some("thinking" | "redacted_thinking") => result.push(block.clone()),
            Some("text") => {
                let mut updated = block.clone();
                updated.as_object_mut().unwrap().remove("_text_bytes");
                if preserve_segments {
                    let length = block["_text_bytes"]
                        .as_u64()
                        .map(|n| n as usize)
                        .unwrap_or_else(|| block["text"].as_str().unwrap_or("").len());
                    let end = offset + length;
                    updated["text"] = json!(text
                        .get(offset..end)
                        .ok_or_else(|| error("Messages saved text boundary is invalid"))?);
                    offset = end;
                } else if !text_written {
                    updated["text"] = json!(text);
                } else {
                    continue;
                }
                text_written = true;
                if !updated["text"].as_str().unwrap_or("").is_empty() {
                    result.push(updated);
                }
            }
            Some("tool_use") => {
                if let Some(call) = calls.iter().find(|call| call["id"] == block["id"]) {
                    result.push(tool_block(call)?);
                    seen.insert(call["id"].as_str().unwrap_or("").to_string());
                }
            }
            kind => {
                return Err(error(format!(
                    "Messages unsupported saved content {kind:?}"
                )))
            }
        }
    }
    if !text_written && !text.is_empty() {
        result.push(json!({"type":"text","text":text}));
    }
    for call in calls {
        if !seen.contains(call["id"].as_str().unwrap_or("")) {
            result.push(tool_block(&call)?);
        }
    }
    Ok(result)
}

pub(crate) fn project_request(chat: Value) -> Result<Value, AdapterError> {
    let mut system = Vec::new();
    let mut messages = Vec::new();
    for message in chat["messages"]
        .as_array()
        .ok_or_else(|| error("Messages request is missing messages"))?
    {
        let role = message["role"]
            .as_str()
            .ok_or_else(|| error("Messages message is missing its role"))?;
        if matches!(role, "system" | "developer") {
            system.extend(content(&message["content"])?);
            continue;
        }
        if role == "tool" {
            let id = message["tool_call_id"]
                .as_str()
                .filter(|id| !id.is_empty())
                .ok_or_else(|| error("Messages tool result is missing tool_use_id"))?;
            let mut result = json!({"type":"tool_result","tool_use_id":id,"content":content(&message["content"])?});
            let error_result = message["content"]
                .as_str()
                .and_then(|text| serde_json::from_str::<Value>(text).ok())
                .is_some_and(|value| {
                    value["error_code"].is_string()
                        || (value["version"] == crate::tool_result::TOOL_RESULT_ENVELOPE_VERSION
                            && value["status"] == "error")
                });
            if message["is_error"] == true || error_result {
                result["is_error"] = json!(true);
            }
            append_message(&mut messages, "user", vec![result]);
            continue;
        }
        if !matches!(role, "user" | "assistant") {
            return Err(error(format!("Messages unsupported role {role}")));
        }
        if role == "assistant" {
            if let Some(native) = message["protocol_state"].as_array() {
                append_message(&mut messages, role, replay_blocks(message, native)?);
                continue;
            }
        }
        let mut blocks = content(&message["content"])?;
        if let Some(calls) = message["tool_calls"].as_array() {
            for call in calls {
                blocks.push(tool_block(call)?);
            }
        }
        append_message(&mut messages, role, blocks);
    }
    let mut body = json!({"model":chat["model"],"messages":messages,
        "max_tokens":chat["max_tokens"],"stream":chat["stream"].as_bool().unwrap_or(true)});
    if let Some(temperature) = chat.get("temperature") {
        body["temperature"] = temperature.clone();
    }
    if let Some(effort) = chat.get("reasoning_effort") {
        body["output_config"]["effort"] = effort.clone();
    }
    if let Some(tools) = chat["tools"].as_array().filter(|tools| !tools.is_empty()) {
        let mut projected = Vec::new();
        for tool in tools {
            if tool["type"] != "function" {
                return Err(error("Messages only accepts registered function tools"));
            }
            let function = &tool["function"];
            let mut declaration = json!({"name":function["name"],"description":function["description"],"input_schema":function["parameters"]});
            if let Some(strict) = function.get("strict") {
                declaration["strict"] = strict.clone();
            }
            projected.push(declaration);
        }
        body["tools"] = Value::Array(projected);
        let choice = chat.get("tool_choice").cloned().unwrap_or(json!("auto"));
        body["tool_choice"] = match choice.as_str() {
            Some("none") => json!({"type":"none"}),
            Some("auto") => json!({"type":"auto"}),
            Some("required") => json!({"type":"any"}),
            None if choice["type"] == "function" => {
                json!({"type":"tool","name":choice["function"]["name"]})
            }
            _ => return Err(error("Messages unsupported tool choice")),
        };
        if body["tool_choice"]["type"] != "none" {
            if let Some(parallel) = chat["parallel_tool_calls"].as_bool() {
                body["tool_choice"]["disable_parallel_tool_use"] = json!(!parallel);
            }
        }
    }
    match chat["response_format"]["type"].as_str() {
        Some("json_schema") => body["output_config"]["format"] = json!({"type":"json_schema","schema":chat["response_format"]["json_schema"]["schema"]}),
        // Messages has no JSON-object wire mode. Preserve the runtime's text
        // JSON contract and its parser rather than inventing a response schema.
        Some("json_object") => system.push(json!({"type":"text","text":"Return exactly one JSON object and no Markdown code fence."})),
        None | Some("text") => {}
        _ => return Err(error("Messages unsupported output format")),
    }
    if !system.is_empty() {
        body["system"] = Value::Array(system);
    }
    Ok(body)
}

#[derive(Default)]
struct Usage {
    input: Option<u32>,
    output: Option<u32>,
    read: Option<u32>,
    created: Option<u32>,
}
impl Usage {
    fn update(&mut self, value: &Value) -> Result<(), AdapterError> {
        let map = value
            .as_object()
            .ok_or_else(|| error("Messages usage is not an object"))?;
        for (name, target) in [
            ("input_tokens", &mut self.input),
            ("output_tokens", &mut self.output),
            ("cache_read_input_tokens", &mut self.read),
            ("cache_creation_input_tokens", &mut self.created),
        ] {
            if let Some(value) = map.get(name) {
                *target = Some(
                    value
                        .as_u64()
                        .and_then(|n| u32::try_from(n).ok())
                        .ok_or_else(|| error(format!("Messages invalid {name}")))?,
                );
            }
        }
        Ok(())
    }
    fn snapshot(&self) -> ModelUsage {
        // The three input buckets are disjoint. A missing bucket is unknown,
        // including on a compatibility endpoint; never report it as free.
        let input_tokens = self
            .input
            .zip(self.read)
            .zip(self.created)
            .map(|((input, read), created)| input.saturating_add(read).saturating_add(created));
        ModelUsage {
            input_tokens,
            output_tokens: self.output,
            cached_input_tokens: self.read,
            cache_creation_input_tokens: self.created,
            total_tokens: input_tokens
                .zip(self.output)
                .map(|(i, o)| i.saturating_add(o)),
            ..Default::default()
        }
    }
    fn observe(&self, observer: &mut dyn ModelObserver) {
        let snapshot = self.snapshot();
        if snapshot.has_any_value() {
            observer.observe(ModelDelta::Usage(snapshot));
        }
    }
}
pub(crate) fn model_usage(value: &Value) -> Option<ModelUsage> {
    let mut usage = Usage::default();
    usage.update(value.get("usage")?).ok()?;
    let snapshot = usage.snapshot();
    snapshot.has_any_value().then_some(snapshot)
}

struct Block {
    native: Value,
    closed: bool,
    arguments: Option<String>,
}
impl Block {
    fn new(native: Value) -> Result<Self, AdapterError> {
        match native["type"].as_str() {
            Some("text") if native["text"].is_string() => {}
            Some("thinking") if native["thinking"].is_string() => {}
            Some("redacted_thinking") if native["data"].is_string() => {}
            Some("tool_use")
                if native["id"].as_str().is_some_and(|s| !s.is_empty())
                    && native["name"].as_str().is_some_and(|s| !s.is_empty())
                    && native["input"].is_object() => {}
            kind => {
                return Err(error(format!(
                    "Messages unsupported or invalid content block {kind:?}"
                )))
            }
        }
        Ok(Self {
            native,
            closed: false,
            arguments: None,
        })
    }
    fn observe(&self, index: usize, observer: &mut dyn ModelObserver) {
        if self.native["type"] == "tool_use" {
            observer.observe(ModelDelta::ToolArguments {
                index,
                id: self.native["id"].as_str().unwrap().into(),
                name: self.native["name"].as_str().unwrap().into(),
                arguments: self
                    .arguments
                    .clone()
                    .unwrap_or_else(|| self.native["input"].to_string()),
            });
        }
    }
    fn delta(
        &mut self,
        index: usize,
        delta: &Value,
        observer: &mut dyn ModelObserver,
    ) -> Result<(), AdapterError> {
        let field = match (self.native["type"].as_str(), delta["type"].as_str()) {
            (Some("text"), Some("text_delta")) => "text",
            (Some("thinking"), Some("thinking_delta")) => "thinking",
            (Some("thinking"), Some("signature_delta")) => "signature",
            (Some("tool_use"), Some("input_json_delta")) => {
                let fragment = delta["partial_json"]
                    .as_str()
                    .ok_or_else(|| error("Messages tool delta is not text"))?;
                self.arguments
                    .get_or_insert_with(String::new)
                    .push_str(fragment);
                self.observe(index, observer);
                return Ok(());
            }
            (_, Some("citations_delta")) => {
                return Err(error(
                    "Messages native citations are not enabled for this request",
                ))
            }
            _ => {
                return Err(error(format!(
                    "Messages unsupported content delta {}",
                    delta["type"]
                )))
            }
        };
        let fragment = delta[field]
            .as_str()
            .ok_or_else(|| error(format!("Messages {field} delta is not text")))?;
        self.native[field] = json!(format!(
            "{}{}",
            self.native[field].as_str().unwrap_or(""),
            fragment
        ));
        if field == "text" {
            observer.observe(ModelDelta::Text(fragment.into()));
        }
        Ok(())
    }
}

fn normalize(
    message: &Value,
    blocks: BTreeMap<usize, Block>,
    usage: &Usage,
    stop: &str,
) -> Result<Value, AdapterError> {
    let finish = match stop {
        "end_turn" | "stop_sequence" => "stop",
        "tool_use" => "tool_calls",
        "max_tokens" => "length",
        "refusal" => return Err(error("Messages refused the request")),
        "model_context_window_exceeded" => {
            return Err(error(
                "Messages model context window exceeded; reduce the configured context budget",
            ))
        }
        _ => return Err(error(format!("Messages unfinished response: {stop}"))),
    };
    let mut text = String::new();
    let mut calls = Vec::new();
    let mut native = Vec::new();
    for (_, mut block) in blocks {
        if !block.closed {
            return Err(error("Messages response has an unclosed content block"));
        }
        match block.native["type"].as_str().unwrap() {
            "text" => text.push_str(block.native["text"].as_str().unwrap()),
            "tool_use" => {
                let arguments = block
                    .arguments
                    .unwrap_or_else(|| block.native["input"].to_string());
                let parsed = serde_json::from_str::<Value>(&arguments)
                    .ok()
                    .filter(Value::is_object);
                if parsed.is_none() && finish != "length" {
                    return Err(error("Messages returned incomplete tool arguments"));
                }
                block.native["input"] = parsed.unwrap_or(Value::Null);
                calls.push(json!({"id":block.native["id"],"type":"function","function":{"name":block.native["name"],"arguments":arguments}}));
            }
            "thinking" => {
                if finish != "length"
                    && !block.native["signature"]
                        .as_str()
                        .is_some_and(|s| !s.is_empty())
                {
                    return Err(error(
                        "Messages thinking block is missing its replay signature",
                    ));
                }
            }
            _ => {}
        }
        native.push(block.native);
    }
    if !calls.is_empty() && !matches!(finish, "tool_calls" | "length") {
        return Err(error(
            "Messages returned tools without a tool_use stop reason",
        ));
    }
    if finish == "tool_calls" && calls.is_empty() {
        return Err(error("Messages tool_use stop reason has no tool calls"));
    }
    if finish == "stop" && text.is_empty() {
        return Err(error("Messages returned no answer"));
    }
    Ok(
        json!({"id":message["id"],"model":message["model"],"choices":[{"index":0,
        "message":{"role":"assistant","content":text,"tool_calls":calls,"protocol_state":continuation_state(&native)},"finish_reason":finish}],
        "usage":usage.snapshot().as_provider_usage()}),
    )
}

fn read_json(
    value: Value,
    cancellation: &CancellationToken,
    observer: &mut dyn ModelObserver,
) -> Result<Value, AdapterError> {
    let mut usage = Usage::default();
    if let Some(raw) = value.get("usage") {
        usage.update(raw)?;
        usage.observe(observer);
    }
    cancellation.check().map_err(|e| error(e.message))?;
    if !value["error"].is_null() {
        return Err(error(format!("Messages error: {}", value["error"])));
    }
    let mut blocks = BTreeMap::new();
    for (index, native) in value["content"]
        .as_array()
        .ok_or_else(|| error("Messages response is missing content"))?
        .iter()
        .enumerate()
    {
        let mut block = Block::new(native.clone())?;
        if block.native["type"] == "text" {
            observer.observe(ModelDelta::Text(
                block.native["text"].as_str().unwrap().into(),
            ));
        }
        block.observe(index, observer);
        block.closed = true;
        blocks.insert(index, block);
    }
    let result = normalize(
        &value,
        blocks,
        &usage,
        value["stop_reason"]
            .as_str()
            .ok_or_else(|| error("Messages response is missing stop_reason"))?,
    )?;
    observer.observe(ModelDelta::Finish(
        result["choices"][0]["finish_reason"]
            .as_str()
            .unwrap()
            .into(),
    ));
    Ok(result)
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
        read_json(response.into_json().map_err(error)?, cancellation, observer)
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
    let mut message = None;
    let mut blocks: BTreeMap<usize, Block> = BTreeMap::new();
    let mut usage = Usage::default();
    let mut stop = None;
    loop {
        cancellation.check().map_err(|e| error(e.message))?;
        let mut line = String::new();
        if reader.read_line(&mut line).map_err(error)? == 0 {
            return Err(error("Messages stream ended before message_stop"));
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
        let kind = event["type"]
            .as_str()
            .ok_or_else(|| error("Messages stream event is missing its type"))?;
        if kind == "error" {
            if let Some(raw) = event.get("usage") {
                usage.update(raw)?;
                usage.observe(observer);
            }
            return Err(error(format!("Messages stream error: {}", event["error"])));
        }
        if kind == "message_start" {
            if message.is_some() {
                return Err(error("Messages duplicate message_start"));
            }
            if !event["message"].is_object() {
                return Err(error("Messages message_start is missing message"));
            }
            if let Some(raw) = event["message"].get("usage") {
                usage.update(raw)?;
                usage.observe(observer);
            }
            message = Some(event["message"].clone());
            continue;
        }
        if !matches!(
            kind,
            "content_block_start"
                | "content_block_delta"
                | "content_block_stop"
                | "message_delta"
                | "message_stop"
        ) {
            continue;
        }
        if message.is_none() {
            return Err(error("Messages content precedes message_start"));
        }
        match kind {
            "content_block_start" | "content_block_delta" | "content_block_stop" => {
                if stop.is_some() {
                    return Err(error("Messages content follows stop_reason"));
                }
                let index = event["index"]
                    .as_u64()
                    .and_then(|n| usize::try_from(n).ok())
                    .ok_or_else(|| error("Messages block is missing index"))?;
                if kind == "content_block_start" {
                    if blocks.contains_key(&index) {
                        return Err(error("Messages duplicate content block index"));
                    }
                    let block = Block::new(event["content_block"].clone())?;
                    if block.native["type"] == "text" {
                        let text = block.native["text"].as_str().unwrap();
                        if !text.is_empty() {
                            observer.observe(ModelDelta::Text(text.into()));
                        }
                    }
                    block.observe(index, observer);
                    blocks.insert(index, block);
                } else {
                    let block = blocks
                        .get_mut(&index)
                        .ok_or_else(|| error("Messages delta/stop has no matching block"))?;
                    if block.closed {
                        return Err(error("Messages delta/stop follows a closed block"));
                    }
                    if kind == "content_block_delta" {
                        block.delta(index, &event["delta"], observer)?;
                    } else {
                        block.closed = true;
                    }
                }
            }
            "message_delta" => {
                if let Some(raw) = event.get("usage") {
                    usage.update(raw)?;
                    usage.observe(observer);
                }
                if let Some(reason) = event["delta"]["stop_reason"].as_str() {
                    stop = Some(reason.to_string());
                }
            }
            "message_stop" => {
                let result = normalize(
                    message.as_ref().unwrap(),
                    blocks,
                    &usage,
                    stop.as_deref()
                        .ok_or_else(|| error("Messages message_stop is missing stop_reason"))?,
                )?;
                observer.observe(ModelDelta::Finish(
                    result["choices"][0]["finish_reason"]
                        .as_str()
                        .unwrap()
                        .into(),
                ));
                return Ok(result);
            }
            _ => unreachable!(),
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
    fn usage() -> Value {
        json!({"input_tokens":10,"output_tokens":1,"cache_read_input_tokens":7,"cache_creation_input_tokens":3})
    }

    #[test]
    fn provider_messages_projects_system_tools_images_and_signatures() {
        let native = json!([{"type":"thinking","thinking":"private","signature":"signed"},{"type":"tool_use","id":"t1","name":"read","input":{}}]);
        let body=project_request(json!({"model":"claude","max_tokens":100,"messages":[
            {"role":"system","content":"teach"},{"role":"user","content":[{"type":"text","text":"look"},{"type":"image_url","image_url":{"url":"data:image/png;base64,AA=="}}]},
            {"role":"assistant","content":null,"protocol_state":native,"tool_calls":[{"id":"t1","function":{"name":"read","arguments":"{}"}}]},
            {"role":"tool","tool_call_id":"t1","content":"failed","is_error":true},{"role":"user","content":"retry"}],
            "tools":[{"type":"function","function":{"name":"read","description":"Read","parameters":{"type":"object"}}}],
            "parallel_tool_calls":false,"tool_choice":"auto","response_format":{"type":"json_object"}})).unwrap();
        assert_eq!(body["system"][0]["text"], "teach");
        assert_eq!(
            body["messages"][0]["content"][1]["source"]["media_type"],
            "image/png"
        );
        assert_eq!(body["messages"][1]["content"][0]["signature"], "signed");
        assert_eq!(body["messages"][2]["content"][0]["is_error"], true);
        assert_eq!(body["messages"][2]["content"][1]["text"], "retry");
        assert_eq!(body["tool_choice"]["disable_parallel_tool_use"], true);
        assert!(body.get("thinking").is_none());
        assert!(body.get("response_format").is_none());
        assert!(body["tools"][0].get("tool_addition").is_none());
    }

    #[test]
    fn provider_messages_stream_preserves_signatures_tools_and_cumulative_usage() {
        let data = stream(vec![
            json!({"type":"message_start","message":{"id":"m","usage":usage()}}),
            json!({"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":""}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"private"}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"signed"}}),
            json!({"type":"content_block_stop","index":0}),
            json!({"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"t1","name":"read","input":{}}}),
            json!({"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"x\":"}}),
            json!({"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"1}"}}),
            json!({"type":"content_block_stop","index":1}),
            json!({"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":5}}),
            json!({"type":"message_delta","delta":{},"usage":{"output_tokens":6}}),
            json!({"type":"message_stop"}),
        ]);
        let mut events = Vec::new();
        let value = read_sse(
            data.as_bytes(),
            &CancellationToken::default(),
            &mut |delta| events.push(delta),
        )
        .unwrap();
        let message = &value["choices"][0]["message"];
        assert_eq!(message["protocol_state"][0]["signature"], "signed");
        assert_eq!(
            message["tool_calls"][0]["function"]["arguments"],
            "{\"x\":1}"
        );
        assert_eq!(value["usage"]["prompt_tokens"], 20);
        assert_eq!(value["usage"]["completion_tokens"], 6);
        assert_eq!(value["usage"]["total_tokens"], 26);
        assert!(!events
            .iter()
            .any(|delta| matches!(delta, ModelDelta::Text(_))));
    }

    #[test]
    fn provider_messages_rejects_missing_stop_refusal_and_partial_tool_arguments() {
        let partial = stream(vec![
            json!({"type":"message_start","message":{"usage":usage()}}),
        ]);
        assert!(read_sse(
            partial.as_bytes(),
            &CancellationToken::default(),
            &mut crate::provider_stream::ignore
        )
        .is_err());
        let refused = json!({"content":[{"type":"text","text":"no"}],"stop_reason":"refusal","usage":usage()});
        assert!(read_json(
            refused,
            &CancellationToken::default(),
            &mut crate::provider_stream::ignore
        )
        .is_err());
        let mut blocks = BTreeMap::new();
        blocks.insert(
            0,
            Block {
                native: json!({"type":"tool_use","id":"t","name":"read","input":{}}),
                closed: true,
                arguments: Some("{".into()),
            },
        );
        assert!(normalize(&json!({}), blocks, &Usage::default(), "tool_use").is_err());
        let truncated = json!({"content":[],"stop_reason":"max_tokens","usage":usage()});
        assert_eq!(
            read_json(
                truncated,
                &CancellationToken::default(),
                &mut crate::provider_stream::ignore
            )
            .unwrap()["choices"][0]["finish_reason"],
            "length"
        );
    }

    #[test]
    fn provider_messages_json_schema_and_endpoint_are_native() {
        let body=project_request(json!({"model":"claude","max_tokens":12,"messages":[{"role":"user","content":"json"}],
            "response_format":{"type":"json_schema","json_schema":{"name":"result","schema":{"type":"object","properties":{"ok":{"type":"boolean"}},"required":["ok"],"additionalProperties":false}}}})).unwrap();
        assert_eq!(
            body["output_config"]["format"]["schema"]["required"],
            json!(["ok"])
        );
        assert_eq!(
            endpoint("https://api.anthropic.com", "m").unwrap(),
            "https://api.anthropic.com/v1/messages"
        );
        assert_eq!(
            endpoint("https://gateway.test/v1/messages?version=x", "m").unwrap(),
            "https://gateway.test/v1/messages?version=x"
        );
        let usage = model_usage(&json!({"usage":{"input_tokens":10,"output_tokens":2}})).unwrap();
        assert_eq!(usage.input_tokens, None);
        assert_eq!(usage.output_tokens, Some(2));
    }

    #[test]
    fn provider_messages_replay_keeps_signatures_without_reviving_retired_bodies() {
        let native = vec![
            json!({"type":"thinking","thinking":"private","signature":"signed"}),
            json!({"type":"text","text":"long original answer"}),
            json!({"type":"tool_use","id":"t","name":"old_name","input":{"long":"original arguments"}}),
        ];
        let state = continuation_state(&native);
        let serialized = serde_json::to_string(&state).unwrap();
        assert!(!serialized.contains("long original answer"));
        assert!(!serialized.contains("original arguments"));
        let body = project_request(json!({"model":"m","max_tokens":12,"messages":[{"role":"assistant","content":"summary",
            "protocol_state":state,"tool_calls":[{"id":"t","function":{"name":"new_name","arguments":"{}"}}]}]})).unwrap();
        let blocks = &body["messages"][0]["content"];
        assert_eq!(blocks[0]["signature"], "signed");
        assert_eq!(blocks[1]["text"], "summary");
        assert!(blocks[1].get("_text_bytes").is_none());
        assert_eq!(blocks[2]["name"], "new_name");
        assert_eq!(blocks[2]["input"], json!({}));
    }

    #[test]
    fn provider_messages_marks_runtime_tool_errors() {
        let body=project_request(json!({"model":"m","max_tokens":12,"messages":[
            {"role":"tool","tool_call_id":"t","content":json!({"version":crate::tool_result::TOOL_RESULT_ENVELOPE_VERSION,"status":"error","model_body":{"error_code":"LID_NOT_FOUND"}}).to_string()}
        ]})).unwrap();
        assert_eq!(body["messages"][0]["content"][0]["is_error"], true);
    }
}
