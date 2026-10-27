//! DeepSeek Messages responses, normalized without discarding native replay blocks.
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
        message: message.to_string(),
    }
}

fn check(cancellation: &CancellationToken) -> Result<(), AdapterError> {
    cancellation.check().map_err(|e| error(e.message))
}

fn string<'a>(value: &'a Value, field: &str) -> Result<&'a str, AdapterError> {
    value
        .as_str()
        .ok_or_else(|| error(format!("DeepSeek Messages expected string {field}")))
}

#[derive(Default)]
struct Usage {
    uncached_input: Option<u32>,
    output: Option<u32>,
    cache_read: Option<u32>,
    cache_creation: Option<u32>,
}

impl Usage {
    fn update(&mut self, value: &Value) -> Result<(), AdapterError> {
        let fields = value
            .as_object()
            .ok_or_else(|| error("DeepSeek Messages usage is not an object"))?;
        for (name, target) in [
            ("input_tokens", &mut self.uncached_input),
            ("output_tokens", &mut self.output),
            ("cache_read_input_tokens", &mut self.cache_read),
            ("cache_creation_input_tokens", &mut self.cache_creation),
        ] {
            if let Some(value) = fields.get(name) {
                *target = Some(
                    value
                        .as_u64()
                        .and_then(|n| u32::try_from(n).ok())
                        .ok_or_else(|| error(format!("DeepSeek Messages invalid {name}")))?,
                );
            }
        }
        Ok(())
    }

    fn snapshot(&self) -> ModelUsage {
        // Messages input_tokens excludes both cache buckets. An omitted bucket
        // remains unknown; it must not silently become a zero-cost cache miss.
        let input_tokens = self
            .uncached_input
            .zip(self.cache_read)
            .zip(self.cache_creation)
            .map(|((input, read), created)| input.saturating_add(read).saturating_add(created));
        ModelUsage {
            input_tokens,
            output_tokens: self.output,
            cached_input_tokens: self.cache_read,
            cache_creation_input_tokens: self.cache_creation,
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

/// Read usage in a Messages response (including HTTP error bodies).
pub(crate) fn model_usage(value: &Value) -> Option<ModelUsage> {
    let mut usage = Usage::default();
    usage.update(value.get("usage")?).ok()?;
    let snapshot = usage.snapshot();
    snapshot.has_any_value().then_some(snapshot)
}

struct Block {
    native: Value,
    closed: bool,
    tool_index: Option<usize>,
    arguments: String,
    has_argument_delta: bool,
}

impl Block {
    fn new(
        native: Value,
        tool_index: usize,
        observer: &mut dyn ModelObserver,
    ) -> Result<Self, AdapterError> {
        if !native.is_object() {
            return Err(error("DeepSeek Messages content block is not an object"));
        }
        let mut block = Self {
            native,
            closed: false,
            tool_index: None,
            arguments: String::new(),
            has_argument_delta: false,
        };
        match block.native["type"].as_str() {
            Some("text") => {
                let text = string(&block.native["text"], "text")?;
                if !text.is_empty() {
                    observer.observe(ModelDelta::Text(text.into()));
                }
            }
            Some("thinking") => {
                string(&block.native["thinking"], "thinking")?;
                if let Some(signature) = block.native.get("signature") {
                    string(signature, "signature")?;
                }
            }
            Some("redacted_thinking") => {
                string(&block.native["data"], "redacted_thinking.data")?;
            }
            Some("tool_use") => {
                let id = string(&block.native["id"], "tool id")?;
                let name = string(&block.native["name"], "tool name")?;
                if id.is_empty() || name.is_empty() {
                    return Err(error("DeepSeek Messages empty tool identity"));
                }
                if !block.native["input"].is_object() {
                    return Err(error("DeepSeek Messages tool input is not an object"));
                }
                block.tool_index = Some(tool_index);
                block.arguments = block.native["input"].to_string();
                block.observe_tool(observer);
            }
            kind => {
                return Err(error(format!(
                    "DeepSeek Messages unsupported content block {kind:?}"
                )))
            }
        }
        Ok(block)
    }

    fn observe_tool(&self, observer: &mut dyn ModelObserver) {
        observer.observe(ModelDelta::ToolArguments {
            index: self.tool_index.expect("tool block index"),
            id: self.native["id"].as_str().unwrap().into(),
            name: self.native["name"].as_str().unwrap().into(),
            arguments: self.arguments.clone(),
        });
    }

    fn delta(
        &mut self,
        delta: &Value,
        observer: &mut dyn ModelObserver,
    ) -> Result<(), AdapterError> {
        let field = match (self.native["type"].as_str(), delta["type"].as_str()) {
            (Some("text"), Some("text_delta")) => "text",
            (Some("thinking"), Some("thinking_delta")) => "thinking",
            (Some("thinking"), Some("signature_delta")) => "signature",
            (Some("tool_use"), Some("input_json_delta")) => {
                let fragment = string(&delta["partial_json"], "partial_json")?;
                if fragment.is_empty() {
                    return Ok(());
                }
                if !self.has_argument_delta {
                    self.arguments.clear();
                    self.has_argument_delta = true;
                }
                self.arguments.push_str(fragment);
                self.observe_tool(observer);
                return Ok(());
            }
            _ => {
                return Err(error(format!(
                    "DeepSeek Messages unsupported delta {} for {}",
                    delta["type"], self.native["type"]
                )))
            }
        };
        let fragment = string(&delta[field], field)?;
        let mut assembled = self
            .native
            .get(field)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        assembled.push_str(fragment);
        self.native[field] = Value::String(assembled);
        if field == "text" && !fragment.is_empty() {
            observer.observe(ModelDelta::Text(fragment.into()));
        }
        Ok(())
    }

    fn close(&mut self) {
        self.closed = true;
        if self.tool_index.is_some() {
            // Keep malformed arguments for the execution validator. The Messages
            // replay envelope still requires object input, as the Harness does.
            self.native["input"] = serde_json::from_str::<Value>(&self.arguments)
                .ok()
                .filter(Value::is_object)
                .unwrap_or_else(|| json!({}));
        }
    }
}

fn finish_reason(value: &Value) -> Result<String, AdapterError> {
    match value.as_str() {
        Some("end_turn" | "stop_sequence") => Ok("stop".into()),
        Some("tool_use") => Ok("tool_calls".into()),
        Some("max_tokens") => Ok("length".into()),
        _ => Err(error(format!(
            "DeepSeek Messages unsupported stop reason {value}"
        ))),
    }
}

fn normalize(
    message: &Value,
    blocks: Vec<Block>,
    usage: &Usage,
    finish: String,
) -> Result<Value, AdapterError> {
    if blocks.iter().any(|block| !block.closed) {
        return Err(error("DeepSeek Messages has unclosed content blocks"));
    }
    if blocks.is_empty() && finish == "stop" {
        return Err(error("DeepSeek Messages returned no content"));
    }
    let mut text = String::new();
    let mut reasoning: Option<String> = None;
    let mut tool_calls = Vec::new();
    let mut native = Vec::with_capacity(blocks.len());
    for block in blocks {
        match block.native["type"].as_str() {
            Some("text") => text.push_str(block.native["text"].as_str().unwrap()),
            Some("thinking") => reasoning
                .get_or_insert_with(String::new)
                .push_str(block.native["thinking"].as_str().unwrap()),
            Some("tool_use") => tool_calls.push(json!({
                "id": block.native["id"], "type": "function",
                "function": {"name": block.native["name"], "arguments": block.arguments},
            })),
            _ => {}
        }
        native.push(block.native);
    }
    if !tool_calls.is_empty() && !matches!(finish.as_str(), "tool_calls" | "length") {
        return Err(error("DeepSeek Messages tool calls without tool finish"));
    }
    let mut normalized = json!({
        "role": "assistant", "content": if text.is_empty() { Value::Null } else { Value::String(text) },
        "tool_calls": tool_calls, "messages_content": native,
    });
    if let Some(reasoning) = reasoning {
        normalized["reasoning_content"] = Value::String(reasoning);
    }
    Ok(json!({
        "id": message["id"], "model": message["model"],
        "choices": [{"index": 0, "message": normalized, "finish_reason": finish}],
        "usage": usage.snapshot().as_provider_usage(),
    }))
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
    check(cancellation)?;
    if !value["error"].is_null() {
        return Err(error(format!(
            "DeepSeek Messages error: {}",
            value["error"]
        )));
    }
    let content = value["content"]
        .as_array()
        .ok_or_else(|| error("DeepSeek Messages missing content"))?;
    let mut blocks = Vec::with_capacity(content.len());
    let mut tool_count = 0;
    for native in content {
        let mut block = Block::new(native.clone(), tool_count, observer)?;
        tool_count += usize::from(block.tool_index.is_some());
        block.close();
        blocks.push(block);
    }
    let finish = finish_reason(&value["stop_reason"])?;
    let result = normalize(&value, blocks, &usage, finish.clone())?;
    check(cancellation)?;
    observer.observe(ModelDelta::Finish(finish));
    Ok(result)
}

pub(crate) fn read_response(
    response: ureq::Response,
    cancellation: &CancellationToken,
    observer: &mut dyn ModelObserver,
) -> Result<Value, AdapterError> {
    check(cancellation)?;
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
        let value = response.into_json().map_err(error)?;
        read_json(value, cancellation, observer)
    };
    if let Ok(value) = &result {
        observer.observe(ModelDelta::Response(Arc::new(value.clone())));
    }
    result
}

pub(crate) fn read_sse(
    reader: impl Read,
    cancellation: &CancellationToken,
    observer: &mut dyn ModelObserver,
) -> Result<Value, AdapterError> {
    let mut reader = BufReader::new(reader);
    let mut data = String::new();
    let mut message: Option<Value> = None;
    let mut blocks: Vec<Block> = Vec::new();
    let mut indices = BTreeMap::new();
    let mut tool_count = 0;
    let mut usage = Usage::default();
    let mut finish: Option<String> = None;
    loop {
        check(cancellation)?;
        let mut line = String::new();
        let count = reader.read_line(&mut line).map_err(error)?;
        check(cancellation)?;
        if count == 0 {
            return Err(error("DeepSeek Messages stream ended before message_stop"));
        }
        let line = line.trim_end_matches(['\r', '\n']);
        if !line.is_empty() {
            if let Some(value) = line.strip_prefix("data:") {
                if !data.is_empty() {
                    data.push('\n');
                }
                data.push_str(value.strip_prefix(' ').unwrap_or(value));
            }
            continue;
        }
        if data.is_empty() {
            continue;
        }
        let event: Value = serde_json::from_str(&std::mem::take(&mut data)).map_err(error)?;
        let kind = string(&event["type"], "event type")?;
        if kind == "error" || !event["error"].is_null() {
            if let Some(raw) = event.get("usage") {
                usage.update(raw)?;
                usage.observe(observer);
            }
            return Err(error(format!(
                "DeepSeek Messages stream error: {}",
                event["error"]
            )));
        }
        if kind == "message_start" {
            if message.is_some() {
                return Err(error("DeepSeek Messages duplicate message_start"));
            }
            if !event["message"].is_object() {
                return Err(error("DeepSeek Messages missing message"));
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
            // Messages permits additional non-content events (including ping).
            continue;
        }
        if message.is_none() {
            return Err(error("DeepSeek Messages event precedes message_start"));
        }
        match kind {
            "content_block_start" | "content_block_delta" | "content_block_stop" => {
                if finish.is_some() {
                    return Err(error("DeepSeek Messages content after stop reason"));
                }
                let index = event["index"]
                    .as_u64()
                    .and_then(|n| usize::try_from(n).ok())
                    .ok_or_else(|| error("DeepSeek Messages invalid block index"))?;
                if kind == "content_block_start" {
                    if indices.contains_key(&index) {
                        return Err(error("DeepSeek Messages duplicate block index"));
                    }
                    let block = Block::new(event["content_block"].clone(), tool_count, observer)?;
                    tool_count += usize::from(block.tool_index.is_some());
                    indices.insert(index, blocks.len());
                    blocks.push(block);
                } else {
                    let position = indices.get(&index).copied().ok_or_else(|| {
                        error("DeepSeek Messages delta/stop without an open block")
                    })?;
                    let block = &mut blocks[position];
                    if block.closed {
                        return Err(error("DeepSeek Messages delta/stop for a closed block"));
                    }
                    if kind == "content_block_delta" {
                        block.delta(&event["delta"], observer)?;
                    } else {
                        block.close();
                    }
                }
            }
            "message_delta" => {
                if let Some(raw) = event.get("usage") {
                    usage.update(raw)?;
                    usage.observe(observer);
                }
                if !event["delta"]["stop_reason"].is_null() {
                    finish = Some(finish_reason(&event["delta"]["stop_reason"])?);
                }
            }
            "message_stop" => {
                let finish = finish
                    .ok_or_else(|| error("DeepSeek Messages message_stop without stop reason"))?;
                let result = normalize(message.as_ref().unwrap(), blocks, &usage, finish.clone())?;
                check(cancellation)?;
                observer.observe(ModelDelta::Finish(finish));
                return Ok(result);
            }
            _ => unreachable!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(value: Value) -> String {
        format!(
            "event: {}\ndata: {}\n\n",
            value["type"].as_str().unwrap(),
            value
        )
    }
    fn start() -> String {
        frame(
            json!({"type":"message_start", "message":{"id":"m1","model":"deepseek-flash","usage":{"input_tokens":7,"output_tokens":0,"cache_read_input_tokens":90,"cache_creation_input_tokens":3}}}),
        )
    }
    fn block(index: usize, value: Value) -> String {
        frame(json!({"type":"content_block_start","index":index,"content_block":value}))
    }
    fn delta(index: usize, value: Value) -> String {
        frame(json!({"type":"content_block_delta","index":index,"delta":value}))
    }
    fn close(index: usize) -> String {
        frame(json!({"type":"content_block_stop","index":index}))
    }
    fn stop(reason: &str) -> String {
        frame(
            json!({"type":"message_delta","delta":{"stop_reason":reason},"usage":{"output_tokens":11}}),
        ) + &frame(json!({"type":"message_stop"}))
    }
    fn parse(wire: &str) -> Result<Value, AdapterError> {
        read_sse(wire.as_bytes(), &CancellationToken::default(), &mut |_| {})
    }

    #[test]
    fn interleaved_blocks_preserve_order_signatures_raw_arguments_and_usage() {
        let wire = start()
            + &block(
                8,
                json!({"type":"thinking","thinking":"plan ","signature":"sig-","opaque":{"version":1}}),
            )
            + &block(2, json!({"type":"text","text":"中"}))
            + &delta(8, json!({"type":"thinking_delta","thinking":"next"}))
            + &delta(2, json!({"type":"text_delta","text":"文"}))
            + &delta(8, json!({"type":"signature_delta","signature":"tail"}))
            + &close(2)
            + &close(8)
            + &block(
                9,
                json!({"type":"redacted_thinking","data":"opaque","provider_field":"keep"}),
            )
            + &close(9)
            + &block(
                10,
                json!({"type":"tool_use","id":"c1","name":"book.text","input":{}}),
            )
            + &block(
                11,
                json!({"type":"tool_use","id":"c2","name":"reader.state","input":{}}),
            )
            + &delta(
                10,
                json!({"type":"input_json_delta","partial_json":"{\"lid\":"}),
            )
            + &delta(11, json!({"type":"input_json_delta","partial_json":"{}"}))
            + &delta(
                10,
                json!({"type":"input_json_delta","partial_json":"\"1.1\"}"}),
            )
            + &close(11)
            + &close(10)
            + &stop("tool_use");
        let mut observed = Vec::new();
        let value = read_sse(wire.as_bytes(), &Default::default(), &mut |d| {
            observed.push(d)
        })
        .unwrap();
        let message = &value["choices"][0]["message"];
        assert_eq!(message["content"], "中文");
        assert_eq!(message["reasoning_content"], "plan next");
        assert_eq!(message["messages_content"][0]["signature"], "sig-tail");
        assert_eq!(message["messages_content"][0]["opaque"]["version"], 1);
        assert_eq!(message["messages_content"][2]["provider_field"], "keep");
        assert_eq!(message["tool_calls"][0]["id"], "c1");
        assert_eq!(
            message["tool_calls"][0]["function"]["arguments"],
            r#"{"lid":"1.1"}"#
        );
        assert_eq!(message["messages_content"][3]["input"]["lid"], "1.1");
        assert_eq!(value["usage"]["prompt_tokens"], 100);
        assert_eq!(value["usage"]["completion_tokens"], 11);
        assert_eq!(value["usage"]["total_tokens"], 111);
        let texts = observed
            .iter()
            .filter_map(|d| {
                if let ModelDelta::Text(s) = d {
                    Some(s.as_str())
                } else {
                    None
                }
            })
            .collect::<String>();
        assert_eq!(texts, "中文");
        assert!(observed.iter().any(|d| matches!(d, ModelDelta::ToolArguments { index: 1, id, arguments, .. } if id == "c2" && arguments == "{}")));
        assert_eq!(
            observed
                .iter()
                .filter(|d| matches!(d, ModelDelta::Finish(_)))
                .count(),
            1
        );
    }

    #[test]
    fn malformed_tool_json_remains_available_for_execution_validation() {
        for arguments in [r#"{"lid":"1.1""#, "[]", "null"] {
            let wire = start()
                + &block(
                    0,
                    json!({"type":"tool_use","id":"call","name":"book.text","input":{}}),
                )
                + &delta(
                    0,
                    json!({"type":"input_json_delta","partial_json":arguments}),
                )
                + &close(0)
                + &stop("tool_use");
            let value = parse(&wire).unwrap();
            assert_eq!(
                value["choices"][0]["message"]["tool_calls"][0]["function"]["arguments"],
                arguments
            );
            assert_eq!(
                value["choices"][0]["message"]["messages_content"][0]["input"],
                json!({})
            );
        }
        let wire = start()
            + &block(
                0,
                json!({"type":"tool_use","id":"call","name":"reader.state","input":{}}),
            )
            + &delta(0, json!({"type":"input_json_delta","partial_json":""}))
            + &close(0)
            + &stop("tool_use");
        assert_eq!(
            parse(&wire).unwrap()["choices"][0]["message"]["tool_calls"][0]["function"]
                ["arguments"],
            "{}"
        );
    }

    #[test]
    fn cumulative_usage_is_retained_when_a_later_stream_fails() {
        let wire = start()
            + &frame(json!({"type":"message_delta","delta":{},"usage":{"output_tokens":9}}));
        for tail in [
            "".to_owned(),
            frame(json!({"type":"error","error":{"type":"overloaded_error","message":"busy"}})),
        ] {
            let mut usage = None;
            assert!(read_sse(
                (wire.clone() + &tail).as_bytes(),
                &Default::default(),
                &mut |d| {
                    if let ModelDelta::Usage(u) = d {
                        usage = Some(u);
                    }
                }
            )
            .is_err());
            let usage = usage.unwrap();
            assert_eq!(usage.input_tokens, Some(100));
            assert_eq!(usage.output_tokens, Some(9));
            assert_eq!(usage.total_tokens, Some(109));
        }
        let value =
            json!({"usage":{"input_tokens":7,"output_tokens":11,"cache_read_input_tokens":90}});
        let usage = model_usage(&value).unwrap();
        assert_eq!(usage.input_tokens, None);
        assert_eq!(usage.total_tokens, None);
        assert_eq!(usage.cached_input_tokens, Some(90));
        assert_eq!(usage.cache_creation_input_tokens, None);
    }

    #[test]
    fn truncated_and_invalid_event_sequences_fail_without_finish() {
        let open = start() + &block(0, json!({"type":"text","text":"answer"}));
        for wire in [
            open.clone(),
            open.clone() + &stop("end_turn"),
            start() + &close(0),
            open.clone() + &close(0) + &delta(0, json!({"type":"text_delta","text":"late"})),
            open.clone() + &block(0, json!({"type":"text","text":"duplicate"})),
            open.clone() + &frame(json!({"type":"message_stop"})),
            open.clone()
                + &frame(json!({"type":"message_delta","delta":{"stop_reason":"refusal"}})),
            open + "data: {bad}\n\n",
        ] {
            let mut finished = false;
            assert!(read_sse(wire.as_bytes(), &Default::default(), &mut |d| {
                finished |= matches!(d, ModelDelta::Finish(_));
            })
            .is_err());
            assert!(!finished);
        }
    }

    #[test]
    fn cancellation_and_length_finish_preserve_observed_usage() {
        let wire = start()
            + &block(0, json!({"type":"thinking","thinking":""}))
            + &close(0)
            + &stop("max_tokens");
        let value = parse(&wire).unwrap();
        assert_eq!(value["choices"][0]["finish_reason"], "length");
        assert_eq!(value["choices"][0]["message"]["reasoning_content"], "");
        let cancellation = CancellationToken::default();
        let token = cancellation.clone();
        assert!(read_sse(wire.as_bytes(), &cancellation, &mut |_| token.cancel()).is_err());
    }

    #[test]
    fn nonstream_response_has_the_same_replay_and_usage_shape() {
        let value = json!({"id":"m1","model":"deepseek-flash","content":[
            {"type":"thinking","thinking":"reason","signature":"opaque"},
            {"type":"text","text":"answer"},
            {"type":"tool_use","id":"c1","name":"reader.state","input":{}}
        ],"stop_reason":"tool_use","usage":{"input_tokens":7,"cache_read_input_tokens":90,"cache_creation_input_tokens":3,"output_tokens":11}});
        let response = ureq::Response::new(200, "OK", &value.to_string()).unwrap();
        let mut observed = Vec::new();
        let result =
            read_response(response, &Default::default(), &mut |d| observed.push(d)).unwrap();
        assert_eq!(
            result["choices"][0]["message"]["messages_content"],
            value["content"]
        );
        assert_eq!(result["usage"]["total_tokens"], 111);
        assert!(matches!(observed.last(), Some(ModelDelta::Response(_))));
        assert!(observed
            .iter()
            .any(|d| matches!(d, ModelDelta::Finish(reason) if reason == "tool_calls")));
        assert!(observed
            .iter()
            .any(|d| matches!(d, ModelDelta::Text(text) if text == "answer")));
    }
}
