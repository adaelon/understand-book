//! Ollama's native chat protocol. One NDJSON object is one complete wire chunk;
//! tool arguments are JSON objects, not OpenAI string fragments.
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
        message: format!("Ollama: {}", message.to_string()),
    }
}

pub(crate) fn endpoint(base_url: &str, _model: &str) -> Result<String, AdapterError> {
    let mut url = url::Url::parse(base_url).map_err(error)?;
    let path = url.path().trim_end_matches('/');
    let path = if path.ends_with("/api/chat") {
        path.to_owned()
    } else if path.ends_with("/api") {
        format!("{path}/chat")
    } else {
        format!("{path}/api/chat")
    };
    url.set_path(&path);
    Ok(url.to_string())
}

fn content(value: &Value) -> Result<(String, Vec<Value>), AdapterError> {
    if value.is_null() {
        return Ok((String::new(), vec![]));
    }
    if let Some(text) = value.as_str() {
        return Ok((text.into(), vec![]));
    }
    let mut text = String::new();
    let mut images = Vec::new();
    for part in value
        .as_array()
        .ok_or_else(|| error("message content must be text or content blocks"))?
    {
        match part["type"].as_str() {
            Some("text") => text.push_str(
                part["text"]
                    .as_str()
                    .ok_or_else(|| error("invalid text block"))?,
            ),
            Some("image_url") => {
                let uri = part["image_url"]["url"]
                    .as_str()
                    .ok_or_else(|| error("image URL is missing"))?;
                let (prefix, data) = uri
                    .split_once(',')
                    .ok_or_else(|| error("native images require a base64 data URI"))?;
                if !prefix.starts_with("data:image/")
                    || !prefix.ends_with(";base64")
                    || data.is_empty()
                {
                    return Err(error("native images require a base64 image data URI"));
                }
                images.push(json!(data));
            }
            kind => return Err(error(format!("unsupported content block {kind:?}"))),
        }
    }
    Ok((text, images))
}

pub(crate) fn project_request(chat_body: Value) -> Result<Value, AdapterError> {
    let mut messages = Vec::new();
    let mut tool_names = BTreeMap::<String, String>::new();
    for message in chat_body["messages"]
        .as_array()
        .ok_or_else(|| error("messages are missing"))?
    {
        let role = message["role"]
            .as_str()
            .ok_or_else(|| error("message role is missing"))?;
        let (text, images) = content(&message["content"])?;
        let mut native = json!({"role": role, "content": text});
        if !images.is_empty() {
            native["images"] = json!(images);
        }
        if role == "assistant" {
            if let Some(thinking) = message["protocol_state"]["thinking"].as_str() {
                native["thinking"] = json!(thinking);
            }
            if let Some(calls) = message["tool_calls"].as_array() {
                let mut native_calls = Vec::new();
                for (index, call) in calls.iter().enumerate() {
                    let id = call["id"]
                        .as_str()
                        .ok_or_else(|| error("tool call ID is missing"))?;
                    let name = call["function"]["name"]
                        .as_str()
                        .ok_or_else(|| error("tool name is missing"))?;
                    let arguments: Value = match &call["function"]["arguments"] {
                        Value::String(text) => serde_json::from_str(text).map_err(error)?,
                        value => value.clone(),
                    };
                    if !arguments.is_object() {
                        return Err(error("tool arguments must be an object"));
                    }
                    tool_names.insert(id.into(), name.into());
                    native_calls.push(json!({"type":"function","function":{"index":index,"name":name,"arguments":arguments}}));
                }
                native["tool_calls"] = json!(native_calls);
            }
        } else if role == "tool" {
            let id = message["tool_call_id"]
                .as_str()
                .ok_or_else(|| error("tool result ID is missing"))?;
            let name = tool_names
                .get(id)
                .ok_or_else(|| error(format!("tool result {id} has no preceding call")))?;
            native["tool_name"] = json!(name);
        }
        messages.push(native);
    }
    let mut body = json!({"model": chat_body["model"], "messages":messages,"stream":chat_body["stream"].as_bool().unwrap_or(true)});
    let choice = chat_body["tool_choice"].as_str().unwrap_or("auto");
    if choice != "auto" && choice != "none" {
        return Err(error("native chat does not support forced tool choice"));
    }
    if choice != "none"
        && chat_body["tools"]
            .as_array()
            .is_some_and(|tools| !tools.is_empty())
    {
        body["tools"] = chat_body["tools"].clone();
    }
    let mut options = serde_json::Map::new();
    for (from, to) in [
        ("max_tokens", "num_predict"),
        ("temperature", "temperature"),
        ("top_p", "top_p"),
        ("seed", "seed"),
        ("stop", "stop"),
    ] {
        if let Some(value) = chat_body.get(from) {
            options.insert(to.into(), value.clone());
        }
    }
    if !options.is_empty() {
        body["options"] = Value::Object(options);
    }
    match chat_body["response_format"]["type"].as_str() {
        None | Some("text") => {}
        Some("json_object") => body["format"] = json!("json"),
        Some("json_schema") => {
            let schema = &chat_body["response_format"]["json_schema"]["schema"];
            if !schema.is_object() {
                return Err(error("JSON Schema is missing"));
            }
            body["format"] = schema.clone();
        }
        kind => return Err(error(format!("unsupported response format {kind:?}"))),
    }
    // Ollama thinking values depend on the selected model. Forward only an
    // explicitly requested intensity; absence leaves the model's own default.
    if let Some(effort) = chat_body.get("reasoning_effort") {
        body["think"] = effort.clone();
    }
    Ok(body)
}

fn count(value: &Value) -> Option<u32> {
    value.as_u64().and_then(|n| u32::try_from(n).ok())
}

pub(crate) fn model_usage(value: &Value) -> Option<ModelUsage> {
    let input_tokens = count(&value["prompt_eval_count"]);
    let output_tokens = count(&value["eval_count"]);
    let usage = ModelUsage {
        input_tokens,
        output_tokens,
        cached_input_tokens: count(&value["prompt_eval_cached_count"]),
        total_tokens: input_tokens
            .zip(output_tokens)
            .map(|(a, b)| a.saturating_add(b)),
        ..Default::default()
    };
    usage.has_any_value().then_some(usage)
}

#[derive(Default)]
struct ResponseState {
    text: String,
    thinking: String,
    calls: Vec<Value>,
    usage: Option<ModelUsage>,
    finish: Option<&'static str>,
}

impl ResponseState {
    fn accept(
        &mut self,
        value: &Value,
        observer: &mut dyn ModelObserver,
    ) -> Result<(), AdapterError> {
        if !value["error"].is_null() {
            return Err(error(format!("provider error: {}", value["error"])));
        }
        if self.finish.is_some() {
            return Err(error("content after final done frame"));
        }
        if let Some(usage) = model_usage(value) {
            observer.observe(ModelDelta::Usage(usage.clone()));
            self.usage = Some(usage);
        }
        if let Some(text) = value["message"]["content"]
            .as_str()
            .filter(|s| !s.is_empty())
        {
            self.text.push_str(text);
            observer.observe(ModelDelta::Text(text.into()));
        }
        if let Some(text) = value["message"]["thinking"].as_str() {
            self.thinking.push_str(text);
        }
        if let Some(calls) = value["message"]["tool_calls"].as_array() {
            for native in calls {
                let name = native["function"]["name"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| error("tool name is missing"))?;
                let arguments = &native["function"]["arguments"];
                if !arguments.is_object() {
                    return Err(error("tool arguments are incomplete or not an object"));
                }
                let id = format!("ollama_{}", uuid::Uuid::now_v7());
                let arguments = arguments.to_string();
                observer.observe(ModelDelta::ToolArguments {
                    index: self.calls.len(),
                    id: id.clone(),
                    name: name.into(),
                    arguments: arguments.clone(),
                });
                self.calls.push(json!({"id":id,"type":"function","function":{"name":name,"arguments":arguments}}));
            }
        }
        if value["done"] == true {
            let reason = match value["done_reason"].as_str() {
                Some("length") => "length",
                Some("stop") => {
                    if self.calls.is_empty() {
                        "stop"
                    } else {
                        "tool_calls"
                    }
                }
                other => {
                    return Err(error(format!(
                        "unsupported or missing done reason {other:?}"
                    )))
                }
            };
            self.finish = Some(reason);
            observer.observe(ModelDelta::Finish(reason.into()));
        }
        Ok(())
    }

    fn finish(self) -> Result<Value, AdapterError> {
        let reason = self
            .finish
            .ok_or_else(|| error("stream ended before a final done frame"))?;
        let mut message = json!({"role":"assistant","content":self.text,"tool_calls":self.calls});
        if !self.thinking.is_empty() {
            message["protocol_state"] = json!({"thinking":self.thinking});
        }
        Ok(
            json!({"choices":[{"index":0,"message":message,"finish_reason":reason}],"usage":self.usage.map(|usage|usage.as_provider_usage())}),
        )
    }
}

fn read_ndjson(
    reader: impl Read,
    cancellation: &CancellationToken,
    observer: &mut dyn ModelObserver,
) -> Result<Value, AdapterError> {
    let mut reader = BufReader::new(reader);
    let mut state = ResponseState::default();
    loop {
        cancellation.check().map_err(|e| error(e.message))?;
        let mut line = String::new();
        let count = reader.read_line(&mut line).map_err(error)?;
        cancellation.check().map_err(|e| error(e.message))?;
        if count == 0 {
            break;
        }
        if line.trim().is_empty() {
            continue;
        }
        state.accept(
            &serde_json::from_str::<Value>(&line).map_err(error)?,
            observer,
        )?;
    }
    state.finish()
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
        == "application/x-ndjson";
    let result = if streaming {
        read_ndjson(response.into_reader(), cancellation, observer)
    } else {
        cancellation.check().map_err(|e| error(e.message))?;
        let value: Value = response.into_json().map_err(error)?;
        cancellation.check().map_err(|e| error(e.message))?;
        let mut state = ResponseState::default();
        state.accept(&value, observer)?;
        state.finish()
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

    #[test]
    fn provider_ollama_projection_pairs_tools_and_inline_images() {
        let body = project_request(json!({"model":"local-model","stream":true,"max_tokens":200,"temperature":0,"messages":[
            {"role":"user","content":[{"type":"text","text":"image"},{"type":"image_url","image_url":{"url":"data:image/png;base64,YQ=="}}]},
            {"role":"assistant","content":null,"protocol_state":{"thinking":"plan"},"tool_calls":[{"id":"call-1","function":{"name":"read","arguments":"{\"page\":1}"}}]},
            {"role":"tool","tool_call_id":"call-1","content":"page text"}
        ]})).unwrap();
        assert_eq!(body["messages"][0]["images"], json!(["YQ=="]));
        assert_eq!(
            body["messages"][1]["tool_calls"][0]["function"]["arguments"],
            json!({"page":1})
        );
        assert_eq!(body["messages"][1]["thinking"], "plan");
        assert_eq!(body["messages"][2]["tool_name"], "read");
        assert!(body["messages"][2].get("tool_call_id").is_none());
        assert_eq!(body["options"]["num_predict"], 200);
        assert_eq!(
            endpoint("http://localhost:11434/api?x=1", "m").unwrap(),
            "http://localhost:11434/api/chat?x=1"
        );
    }

    #[test]
    fn provider_ollama_format_and_no_tools_choice_are_projected() {
        let body = project_request(json!({"messages":[],"tool_choice":"none","tools":[{}],"response_format":{"type":"json_schema","json_schema":{"schema":{"type":"object"}}}})).unwrap();
        assert!(body.get("tools").is_none());
        assert_eq!(body["format"], json!({"type":"object"}));
        assert!(project_request(
            json!({"messages":[{"role":"tool","tool_call_id":"missing","content":"x"}]})
        )
        .is_err());
    }

    #[test]
    fn provider_ollama_ndjson_tool_roundtrip_and_native_counts() {
        let wire = concat!(
            "{\"message\":{\"thinking\":\"plan\",\"content\":\"\"},\"done\":false}\n",
            "{\"message\":{\"tool_calls\":[{\"function\":{\"name\":\"read\",\"arguments\":{\"page\":1}}}]},\"done\":false}\n",
            "{\"message\":{\"content\":\"\"},\"done\":true,\"done_reason\":\"stop\",\"prompt_eval_count\":20,\"eval_count\":5,\"prompt_eval_cached_count\":8}\n"
        );
        let result =
            read_ndjson(wire.as_bytes(), &CancellationToken::default(), &mut ignore).unwrap();
        assert_eq!(result["choices"][0]["finish_reason"], "tool_calls");
        let mut message = result["choices"][0]["message"].clone();
        let id = message["tool_calls"][0]["id"].clone();
        message["role"] = json!("assistant");
        let projected = project_request(
            json!({"messages":[message,{"role":"tool","tool_call_id":id,"content":"ok"}]}),
        )
        .unwrap();
        assert_eq!(projected["messages"][1]["tool_name"], "read");
        assert_eq!(result["usage"]["total_tokens"], 25);
        assert_eq!(result["usage"]["prompt_tokens_details"]["cached_tokens"], 8);
    }

    #[test]
    fn provider_ollama_rejects_interruption_and_preserves_length() {
        let incomplete = "{\"message\":{\"content\":\"half\"},\"done\":false}\n";
        assert!(read_ndjson(
            incomplete.as_bytes(),
            &CancellationToken::default(),
            &mut ignore
        )
        .is_err());
        let length =
            "{\"message\":{\"content\":\"half\"},\"done\":true,\"done_reason\":\"length\"}\n";
        assert_eq!(
            read_ndjson(
                length.as_bytes(),
                &CancellationToken::default(),
                &mut ignore
            )
            .unwrap()["choices"][0]["finish_reason"],
            "length"
        );
        let failed = "{\"error\":\"model unavailable\"}\n";
        assert!(read_ndjson(
            failed.as_bytes(),
            &CancellationToken::default(),
            &mut ignore
        )
        .is_err());
    }
}
