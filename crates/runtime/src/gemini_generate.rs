//! Gemini GenerateContent wire projection and response decoding.
//! Protocol reference: https://aistudio.google.com/api-reference/generate-content
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

pub(crate) fn error(message: impl ToString) -> AdapterError {
    AdapterError {
        spend_stop: None,
        message: message.to_string(),
    }
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
    let base = url.path().trim_end_matches('/');
    let method = if stream {
        "streamGenerateContent"
    } else {
        "generateContent"
    };
    let path = if let Some(prefix) = base
        .strip_suffix(":generateContent")
        .or_else(|| base.strip_suffix(":streamGenerateContent"))
    {
        format!("{prefix}:{method}")
    } else {
        let base = if base.is_empty() { "/v1beta" } else { base };
        let model = model.strip_prefix("models/").unwrap_or(model);
        if base.ends_with("/models") {
            format!("{base}/{model}:{method}")
        } else {
            format!("{base}/models/{model}:{method}")
        }
    };
    url.set_path(&path);
    let query: Vec<(String, String)> = url
        .query_pairs()
        .filter(|(key, _)| key != "alt")
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();
    url.set_query(None);
    if !query.is_empty() || stream {
        url.query_pairs_mut().extend_pairs(query);
    }
    if stream {
        url.query_pairs_mut().append_pair("alt", "sse");
    }
    Ok(url.into())
}

/// Convert only project-supported text and image content; remote URLs remain remote.
pub(crate) fn content_parts(content: &Value) -> Result<Vec<Value>, AdapterError> {
    if content.is_null() {
        return Ok(Vec::new());
    }
    if let Some(text) = content.as_str() {
        return Ok(vec![json!({"text":text})]);
    }
    let blocks = content
        .as_array()
        .ok_or_else(|| error("Gemini content must be text or content blocks"))?;
    blocks
        .iter()
        .map(|block| match block["type"].as_str() {
            Some("text") => block["text"]
                .as_str()
                .map(|text| json!({"text":text}))
                .ok_or_else(|| error("Gemini text block is missing text")),
            Some("image_url") => {
                let uri = block["image_url"]["url"]
                    .as_str()
                    .ok_or_else(|| error("Gemini image block is missing its URL"))?;
                if let Some(data) = uri.strip_prefix("data:") {
                    let (mime, bytes) = data
                        .split_once(";base64,")
                        .ok_or_else(|| error("Gemini inline image must be base64 encoded"))?;
                    Ok(json!({"inlineData":{"mimeType":mime,"data":bytes}}))
                } else {
                    Ok(json!({"fileData":{"fileUri":uri}}))
                }
            }
            kind => Err(error(format!(
                "Gemini unsupported input content block: {kind:?}"
            ))),
        })
        .collect()
}

pub(crate) fn arguments(call: &Value) -> Result<Value, AdapterError> {
    let value: Value = serde_json::from_str(
        call["function"]["arguments"]
            .as_str()
            .ok_or_else(|| error("Gemini tool call is missing JSON arguments"))?,
    )
    .map_err(error)?;
    if !value.is_object() {
        return Err(error("Gemini tool arguments must be an object"));
    }
    Ok(value)
}

pub(crate) fn required_string<'a>(value: &'a Value, field: &str) -> Result<&'a str, AdapterError> {
    value
        .as_str()
        .filter(|text| !text.is_empty())
        .ok_or_else(|| error(format!("Gemini missing {field}")))
}

fn push_content(contents: &mut Vec<Value>, role: &str, parts: Vec<Value>) {
    if parts.is_empty() {
        return;
    }
    // Parallel function results and consecutive user images belong to one turn.
    if let Some(last) = contents.last_mut().filter(|last| last["role"] == role) {
        last["parts"].as_array_mut().unwrap().extend(parts);
    } else {
        contents.push(json!({"role":role,"parts":parts}));
    }
}

/// Preserve part boundaries without storing a second copy of answer text. If
/// history has been compacted, use its current text and discard stale segments.
pub(crate) fn split_current_text(text: &str, lengths: &[usize]) -> (Vec<String>, bool) {
    let mut offset = 0usize;
    let mut parts = Vec::new();
    for length in lengths {
        let Some(end) = offset.checked_add(*length) else {
            return (vec![text.into()], false);
        };
        let Some(part) = text.get(offset..end) else {
            return (vec![text.into()], false);
        };
        parts.push(part.into());
        offset = end;
    }
    if offset == text.len() {
        (parts, true)
    } else {
        (vec![text.into()], false)
    }
}

fn assistant_parts(
    message: &Value,
) -> Result<(Vec<Value>, BTreeMap<String, (String, Option<String>)>), AdapterError> {
    let tool_calls: Vec<&Value> = message["tool_calls"]
        .as_array()
        .into_iter()
        .flatten()
        .collect();
    let Some(metadata) = message["protocol_state"]["parts"].as_array() else {
        let mut parts = content_parts(&message["content"])?;
        let mut calls = BTreeMap::new();
        for call in tool_calls {
            let id = required_string(&call["id"], "tool call ID")?;
            let name = required_string(&call["function"]["name"], "tool name")?;
            parts.push(json!({"functionCall":{"id":id,"name":name,"args":arguments(call)?}}));
            calls.insert(id.into(), (name.into(), Some(id.into())));
        }
        return Ok((parts, calls));
    };
    let lengths: Vec<usize> = metadata
        .iter()
        .filter(|part| part["kind"] == "text")
        .map(|part| part["length"].as_u64().unwrap_or(0) as usize)
        .collect();
    let (texts, same_segments) =
        split_current_text(message["content"].as_str().unwrap_or(""), &lengths);
    let mut texts = texts.into_iter();
    let mut parts = Vec::new();
    let mut calls = BTreeMap::new();
    for descriptor in metadata {
        match descriptor["kind"].as_str() {
            Some("private") => parts.push(descriptor["value"].clone()),
            Some("text") => {
                if let Some(text) = texts.next().filter(|text| !text.is_empty()) {
                    let mut part = json!({"text":text});
                    if same_segments {
                        if let Some(signature) = descriptor.get("thoughtSignature") {
                            part["thoughtSignature"] = signature.clone();
                        }
                    }
                    parts.push(part);
                }
            }
            Some("function_call") => {
                let Some(call) = tool_calls
                    .iter()
                    .find(|call| call["id"] == descriptor["call_id"])
                else {
                    continue;
                };
                let id = required_string(&call["id"], "tool call ID")?;
                let name = required_string(&call["function"]["name"], "tool name")?;
                let mut part = json!({"functionCall":{"name":name,"args":arguments(call)?}});
                let native_id = descriptor["native_id"].as_str().map(str::to_owned);
                if let Some(native_id) = &native_id {
                    part["functionCall"]["id"] = json!(native_id);
                }
                if let Some(signature) = descriptor.get("thoughtSignature") {
                    part["thoughtSignature"] = signature.clone();
                }
                calls.insert(id.into(), (name.into(), native_id));
                parts.push(part);
            }
            _ => return Err(error("Gemini saved part metadata is invalid")),
        }
    }
    // Canonical history owns any facts added or replaced after the response.
    for text in texts.filter(|text| !text.is_empty()) {
        parts.push(json!({"text":text}));
    }
    for call in tool_calls {
        let id = required_string(&call["id"], "tool call ID")?;
        if calls.contains_key(id) {
            continue;
        }
        let name = required_string(&call["function"]["name"], "tool name")?;
        parts.push(json!({"functionCall":{"id":id,"name":name,"args":arguments(call)?}}));
        calls.insert(id.into(), (name.into(), Some(id.into())));
    }
    Ok((parts, calls))
}

pub(crate) fn project_request(chat: Value) -> Result<Value, AdapterError> {
    let mut contents = Vec::new();
    let mut system = Vec::new();
    let mut calls: BTreeMap<String, (String, Option<String>)> = BTreeMap::new();
    for message in chat["messages"]
        .as_array()
        .ok_or_else(|| error("Gemini request is missing messages"))?
    {
        match message["role"].as_str() {
            Some("system" | "developer") => system.extend(content_parts(&message["content"])?),
            Some("user") => {
                push_content(&mut contents, "user", content_parts(&message["content"])?)
            }
            Some("assistant") => {
                let (parts, message_calls) = assistant_parts(message)?;
                calls.extend(message_calls);
                push_content(&mut contents, "model", parts);
            }
            Some("tool") => {
                let id = required_string(&message["tool_call_id"], "tool result ID")?;
                let (name, native_id) = calls.get(id).ok_or_else(|| {
                    error(format!("Gemini tool result has no matching call: {id}"))
                })?;
                let text = message["content"].as_str().unwrap_or("");
                let value = serde_json::from_str::<Value>(text).unwrap_or_else(|_| json!(text));
                let result = if value.is_object() {
                    value
                } else {
                    json!({"result":value})
                };
                let mut response = json!({"name":name,"response":result});
                if let Some(id) = native_id {
                    response["id"] = json!(id);
                }
                push_content(
                    &mut contents,
                    "user",
                    vec![json!({"functionResponse":response})],
                );
            }
            role => return Err(error(format!("Gemini unsupported message role: {role:?}"))),
        }
    }
    let mut body = json!({"contents":contents,"generationConfig":{"candidateCount":1}});
    if !system.is_empty() {
        body["systemInstruction"] = json!({"parts":system});
    }
    for (source, target) in [
        ("max_tokens", "maxOutputTokens"),
        ("temperature", "temperature"),
        ("top_p", "topP"),
        ("stop", "stopSequences"),
    ] {
        if let Some(value) = chat.get(source) {
            body["generationConfig"][target] = value.clone();
        }
    }
    if let Some(effort) = chat["reasoning_effort"].as_str() {
        body["generationConfig"]["thinkingConfig"] =
            json!({"thinkingLevel":effort.to_ascii_uppercase()});
    }
    match chat["response_format"]["type"].as_str() {
        Some("json_object") => {
            body["generationConfig"]["responseMimeType"] = json!("application/json")
        }
        Some("json_schema") => {
            body["generationConfig"]["responseMimeType"] = json!("application/json");
            body["generationConfig"]["responseJsonSchema"] =
                chat["response_format"]["json_schema"]["schema"].clone();
        }
        _ => {}
    }
    let tools: Vec<Value> = chat["tools"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|tool| {
            json!({
                "name":tool["function"]["name"],"description":tool["function"]["description"],
                "parametersJsonSchema":tool["function"]["parameters"],
            })
        })
        .collect();
    if !tools.is_empty() {
        body["tools"] = json!([{"functionDeclarations":tools}]);
        let choice = &chat["tool_choice"];
        let mut config = json!({"mode": match choice.as_str() {
            Some("none") => "NONE", Some("required") => "ANY", _ => "AUTO",
        }});
        if let Some(name) = choice["function"]["name"].as_str() {
            config = json!({"mode":"ANY","allowedFunctionNames":[name]});
        }
        body["toolConfig"] = json!({"functionCallingConfig":config});
    }
    Ok(body)
}

pub(crate) fn count(value: &Value) -> Option<u32> {
    value.as_u64().and_then(|count| u32::try_from(count).ok())
}

/// Gemini reports visible and thought tokens separately. Normalize their sum as
/// output, with thoughts retained as a subset, so the spend layer counts once.
pub(crate) fn model_usage(value: &Value) -> Option<ModelUsage> {
    let usage = value.get("usageMetadata")?;
    let thoughts = count(&usage["thoughtsTokenCount"]);
    let output = count(&usage["candidatesTokenCount"])
        .map(|tokens| tokens.saturating_add(thoughts.unwrap_or(0)));
    let snapshot = ModelUsage {
        input_tokens: count(&usage["promptTokenCount"]).map(|tokens| {
            tokens.saturating_add(count(&usage["toolUsePromptTokenCount"]).unwrap_or(0))
        }),
        output_tokens: output,
        reasoning_output_tokens: thoughts,
        cached_input_tokens: count(&usage["cachedContentTokenCount"]),
        total_tokens: count(&usage["totalTokenCount"]),
        ..Default::default()
    };
    snapshot.has_any_value().then_some(snapshot)
}

fn normalize(response: &Value) -> Result<Value, AdapterError> {
    if let Some(error_value) = response.get("error") {
        return Err(error(format!("Gemini: {error_value}")));
    }
    if let Some(reason) = response["promptFeedback"]["blockReason"]
        .as_str()
        .filter(|r| *r != "BLOCK_REASON_UNSPECIFIED")
    {
        return Err(error(format!("Gemini rejected the prompt: {reason}")));
    }
    let candidate = &response["candidates"][0];
    let finish = match candidate["finishReason"].as_str() {
        Some("STOP") => "stop",
        Some("MAX_TOKENS") => "length",
        Some(reason) => {
            return Err(error(format!(
                "Gemini response was not completed: {reason}"
            )))
        }
        None => return Err(error("Gemini stream ended without a finish reason")),
    };
    let parts = candidate["content"]["parts"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let mut text = String::new();
    let mut calls = Vec::new();
    let mut metadata = Vec::new();
    for part in &parts {
        if part["thought"] == true {
            metadata.push(json!({"kind":"private","value":part}));
        } else {
            if let Some(fragment) = part["text"].as_str() {
                text.push_str(fragment);
                let mut descriptor = json!({"kind":"text","length":fragment.len()});
                if let Some(signature) = part.get("thoughtSignature") {
                    descriptor["thoughtSignature"] = signature.clone();
                }
                metadata.push(descriptor);
            }
        }
        if let Some(call) = part.get("functionCall") {
            let name = required_string(&call["name"], "function name")?;
            let args = call.get("args").cloned().unwrap_or_else(|| json!({}));
            if !args.is_object() {
                return Err(error("Gemini function args must be an object"));
            }
            let id = call["id"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| format!("gemini-{}", uuid::Uuid::now_v7()));
            let mut descriptor = json!({"kind":"function_call","call_id":id});
            if let Some(native_id) = call.get("id") {
                descriptor["native_id"] = native_id.clone();
            }
            if let Some(signature) = part.get("thoughtSignature") {
                descriptor["thoughtSignature"] = signature.clone();
            }
            metadata.push(descriptor);
            calls.push(json!({"id":id,"type":"function","function":{"name":name,"arguments":args.to_string()}}));
        } else if part.get("text").is_none() && part.get("thoughtSignature").is_none() {
            return Err(error("Gemini returned unsupported non-text output"));
        } else if part.get("text").is_none() && part["thought"] != true {
            metadata.push(json!({"kind":"private","value":part}));
        }
    }
    let finish = if finish == "stop" && !calls.is_empty() {
        "tool_calls"
    } else {
        finish
    };
    if finish == "stop" && text.is_empty() {
        return Err(error("Gemini returned no answer or tool calls"));
    }
    let usage = model_usage(response)
        .map(|usage| usage.as_provider_usage())
        .unwrap_or(Value::Null);
    Ok(
        json!({"choices":[{"message":{"role":"assistant","content":if text.is_empty() {Value::Null}else{json!(text)},
        "tool_calls":calls,"protocol_state":{"parts":metadata}},"finish_reason":finish}],"usage":usage}),
    )
}

pub(crate) fn observe_result(value: &Value, observer: &mut dyn ModelObserver, emit_text: bool) {
    let message = &value["choices"][0]["message"];
    if emit_text {
        if let Some(text) = message["content"].as_str().filter(|text| !text.is_empty()) {
            observer.observe(ModelDelta::Text(text.into()));
        }
    }
    for (index, call) in message["tool_calls"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        observer.observe(ModelDelta::ToolArguments {
            index,
            id: call["id"].as_str().unwrap_or("").into(),
            name: call["function"]["name"].as_str().unwrap_or("").into(),
            arguments: call["function"]["arguments"].as_str().unwrap_or("").into(),
        });
    }
    observer.observe(ModelDelta::Finish(
        value["choices"][0]["finish_reason"]
            .as_str()
            .unwrap_or("")
            .into(),
    ));
    observer.observe(ModelDelta::Response(Arc::new(value.clone())));
}

pub(crate) fn read_response(
    response: ureq::Response,
    cancellation: &CancellationToken,
    observer: &mut dyn ModelObserver,
) -> Result<Value, AdapterError> {
    if response
        .header("Content-Type")
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        == "text/event-stream"
    {
        read_sse(response.into_reader(), cancellation, observer)
    } else {
        let response: Value = response.into_json().map_err(error)?;
        cancellation.check().map_err(|err| error(err.message))?;
        if let Some(usage) = model_usage(&response) {
            observer.observe(ModelDelta::Usage(usage));
        }
        let value = normalize(&response)?;
        observe_result(&value, observer, true);
        Ok(value)
    }
}

pub(crate) fn read_sse(
    reader: impl Read,
    cancellation: &CancellationToken,
    observer: &mut dyn ModelObserver,
) -> Result<Value, AdapterError> {
    let mut response = json!({"candidates":[{"content":{"role":"model","parts":[]}}]});
    read_sse_events(reader, cancellation, |frame| {
        if frame.get("error").is_some() {
            return Err(error(format!("Gemini: {}", frame["error"])));
        }
        if let Some(usage) = model_usage(&frame) {
            observer.observe(ModelDelta::Usage(usage));
            response["usageMetadata"] = frame["usageMetadata"].clone();
        }
        if frame.get("promptFeedback").is_some() {
            response["promptFeedback"] = frame["promptFeedback"].clone();
        }
        if let Some(candidate) = frame["candidates"].as_array().and_then(|c| c.first()) {
            if let Some(reason) = candidate.get("finishReason") {
                response["candidates"][0]["finishReason"] = reason.clone();
            }
            for part in candidate["content"]["parts"]
                .as_array()
                .into_iter()
                .flatten()
            {
                if part["thought"] != true {
                    if let Some(text) = part["text"].as_str().filter(|text| !text.is_empty()) {
                        observer.observe(ModelDelta::Text(text.into()));
                    }
                }
                response["candidates"][0]["content"]["parts"]
                    .as_array_mut()
                    .unwrap()
                    .push(part.clone());
            }
        }
        Ok(())
    })?;
    let value = normalize(&response)?;
    observe_result(&value, observer, false);
    Ok(value)
}

/// SSE framing shared by the two Gemini protocols; their terminal states are
/// checked by the distinct decoders, never inferred from EOF or [DONE].
pub(crate) fn read_sse_events(
    reader: impl Read,
    cancellation: &CancellationToken,
    mut frame: impl FnMut(Value) -> Result<(), AdapterError>,
) -> Result<(), AdapterError> {
    let mut reader = BufReader::new(reader);
    let mut data = String::new();
    let mut event = String::new();
    loop {
        cancellation.check().map_err(|err| error(err.message))?;
        let mut line = String::new();
        let read = reader.read_line(&mut line).map_err(error)?;
        if read == 0 || line.trim_end_matches(['\r', '\n']).is_empty() {
            if !data.is_empty() && data.trim() != "[DONE]" {
                let mut value: Value = serde_json::from_str(data.trim_end()).map_err(error)?;
                if !event.is_empty() && value.get("event_type").is_none() {
                    value["event_type"] = json!(event);
                }
                frame(value)?;
            }
            data.clear();
            event.clear();
            if read == 0 {
                break;
            }
        } else if let Some(value) = line.strip_prefix("data:") {
            let value = value.strip_prefix(' ').unwrap_or(value);
            data.push_str(value.trim_end_matches(['\r', '\n']));
            data.push('\n');
        } else if let Some(value) = line.strip_prefix("event:") {
            event = value.trim().into();
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(value: Value) -> String {
        format!("data: {value}\n\n")
    }
    #[test]
    fn gemini_generate_endpoint_keeps_query_and_encodes_model_path() {
        assert_eq!(endpoint("https://generativelanguage.googleapis.com/v1beta?quota=sample", "models/gemini-test").unwrap(),
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-test:streamGenerateContent?quota=sample&alt=sse");
        assert_eq!(
            endpoint(
                "https://example.com/v1/models/gemini-test:generateContent?alt=json",
                "ignored"
            )
            .unwrap(),
            "https://example.com/v1/models/gemini-test:streamGenerateContent?alt=sse"
        );
    }
    #[test]
    fn gemini_generate_projection_preserves_signed_calls_and_pairs_parallel_results() {
        let parts = json!([{"functionCall":{"name":"read","args":{"lid":"p1"}},"thoughtSignature":"opaque"},
            {"functionCall":{"id":"native-2","name":"read","args":{"lid":"p2"}}}]);
        let response =
            normalize(&json!({"candidates":[{"content":{"parts":parts},"finishReason":"STOP"}]}))
                .unwrap();
        let message = &response["choices"][0]["message"];
        let first_id = &message["tool_calls"][0]["id"];
        let body = project_request(json!({"model":"gemini-test","max_tokens":2048,"messages":[
            {"role":"system","content":"guide"}, {"role":"user","content":"read"},
            message,
            {"role":"tool","tool_call_id":first_id,"content":"one"},
            {"role":"tool","tool_call_id":"native-2","content":"{\"text\":\"two\"}"}]}))
        .unwrap();
        assert_eq!(body["contents"][1]["parts"], parts);
        assert_eq!(body["contents"][2]["parts"].as_array().unwrap().len(), 2);
        assert!(body["contents"][2]["parts"][0]["functionResponse"]
            .get("id")
            .is_none());
        assert_eq!(
            body["contents"][2]["parts"][1]["functionResponse"]["id"],
            "native-2"
        );
        assert_eq!(body["generationConfig"]["maxOutputTokens"], 2048);
    }
    #[test]
    fn gemini_generate_projects_images_and_json_separately_from_tools() {
        let body = project_request(json!({"messages":[{"role":"user","content":[{"type":"text","text":"Describe"},
            {"type":"image_url","image_url":{"url":"data:image/png;base64,aW1hZ2U="}}]}],"response_format":{"type":"json_object"},"max_tokens":50})).unwrap();
        assert_eq!(
            body["contents"][0]["parts"][1]["inlineData"]["mimeType"],
            "image/png"
        );
        assert_eq!(
            body["generationConfig"]["responseMimeType"],
            "application/json"
        );
        assert!(body.get("tools").is_none());
    }
    #[test]
    fn gemini_generate_stream_keeps_usage_snapshot_and_private_parts() {
        let wire = frame(
            json!({"candidates":[{"content":{"parts":[{"text":"private","thought":true,"thoughtSignature":"s"}]}}],
                "usageMetadata":{"promptTokenCount":10,"candidatesTokenCount":1,"thoughtsTokenCount":2,"totalTokenCount":13}}),
        ) + &frame(
            json!({"candidates":[{"content":{"parts":[{"text":"answer"}]},"finishReason":"STOP"}],
                "usageMetadata":{"promptTokenCount":10,"candidatesTokenCount":4,"thoughtsTokenCount":2,"totalTokenCount":16}}),
        );
        let mut events = Vec::new();
        let result = read_sse(
            wire.as_bytes(),
            &CancellationToken::default(),
            &mut |event| events.push(event),
        )
        .unwrap();
        assert_eq!(result["choices"][0]["message"]["content"], "answer");
        assert_eq!(result["usage"]["completion_tokens"], 6);
        assert_eq!(result["usage"]["total_tokens"], 16);
        assert_eq!(
            result["choices"][0]["message"]["protocol_state"]["parts"][0]["value"]
                ["thoughtSignature"],
            "s"
        );
        assert_eq!(
            events
                .iter()
                .filter_map(|event| if let ModelDelta::Text(t) = event {
                    Some(t.as_str())
                } else {
                    None
                })
                .collect::<Vec<_>>(),
            vec!["answer"]
        );
    }
    #[test]
    fn gemini_generate_rejects_disconnect_safety_and_orphan_tools() {
        let wire = frame(json!({"candidates":[{"content":{"parts":[{"text":"half"}]}}]}));
        assert!(read_sse(
            wire.as_bytes(),
            &CancellationToken::default(),
            &mut crate::provider_stream::ignore
        )
        .is_err());
        assert!(normalize(&json!({"promptFeedback":{"blockReason":"SAFETY"}})).is_err());
        assert!(normalize(&json!({"candidates":[{"finishReason":"SAFETY"}]})).is_err());
        assert!(project_request(
            json!({"messages":[{"role":"tool","tool_call_id":"lost","content":"x"}]})
        )
        .is_err());
        assert_eq!(normalize(&json!({"candidates":[{"finishReason":"MAX_TOKENS","content":{"parts":[{"text":"half"}]}}]})).unwrap()["choices"][0]["finish_reason"],"length");
    }
    #[test]
    fn gemini_generate_replay_does_not_restore_retired_text_or_arguments() {
        let response=normalize(&json!({"candidates":[{"content":{"parts":[{"text":"old long answer"},
            {"functionCall":{"id":"c","name":"read","args":{"text":"retired large body"}},"thoughtSignature":"sig"}]},"finishReason":"STOP"}]})).unwrap();
        let mut message = response["choices"][0]["message"].clone();
        assert!(!message["protocol_state"]
            .to_string()
            .contains("retired large body"));
        assert!(!message["protocol_state"]
            .to_string()
            .contains("old long answer"));
        message["content"] = json!("short");
        message["tool_calls"][0]["function"]["arguments"] = json!("{\"text\":\"retired\"}");
        let request = project_request(json!({"messages":[message]})).unwrap();
        assert!(!request.to_string().contains("retired large body"));
        assert!(!request.to_string().contains("old long answer"));
        assert_eq!(request["contents"][0]["parts"][0]["text"], "short");
        assert_eq!(
            request["contents"][0]["parts"][1]["functionCall"]["args"]["text"],
            "retired"
        );
        assert_eq!(
            request["contents"][0]["parts"][1]["thoughtSignature"],
            "sig"
        );
    }
    #[test]
    fn gemini_generate_nonstream_endpoint_and_tool_prompt_usage() {
        assert_eq!(
            endpoint_with_stream(
                "https://example.com/v1/models/m:streamGenerateContent?alt=sse&quota=x",
                "m",
                false
            )
            .unwrap(),
            "https://example.com/v1/models/m:generateContent?quota=x"
        );
        let usage=model_usage(&json!({"usageMetadata":{"promptTokenCount":10,"toolUsePromptTokenCount":4,"candidatesTokenCount":2,"thoughtsTokenCount":3,"totalTokenCount":19}})).unwrap();
        assert_eq!(usage.input_tokens, Some(14));
        assert_eq!(usage.output_tokens, Some(5));
        assert_eq!(usage.total_tokens, Some(19));
    }
}
