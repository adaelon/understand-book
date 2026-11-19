//! Stateless Gemini Interactions with local step history and thought signatures.
//! https://aistudio.google.com/docs/function-calling#stateless-function-calling
//! https://aistudio.google.com/docs/streaming
use crate::{
    gemini_generate::{
        arguments, content_parts, count, error, observe_result, read_sse_events, required_string,
        split_current_text,
    },
    provider_stream::{ModelDelta, ModelObserver, ModelUsage},
    run_context::CancellationToken,
    AdapterError,
};
use serde_json::{json, Value};
use std::{collections::BTreeMap, io::Read};

pub(crate) fn endpoint(base_url: &str, _model: &str) -> Result<String, AdapterError> {
    let mut url = url::Url::parse(base_url).map_err(error)?;
    let base = url.path().trim_end_matches('/');
    if !base.ends_with("/interactions") {
        url.set_path(&format!(
            "{}/interactions",
            if base.is_empty() { "/v1beta" } else { base }
        ));
    }
    Ok(url.into())
}

fn content(value: &Value) -> Result<Vec<Value>, AdapterError> {
    content_parts(value)?
        .into_iter()
        .map(|part| {
            if let Some(text) = part["text"].as_str() {
                return Ok(json!({"type":"text","text":text}));
            }
            if let Some(image) = part.get("inlineData") {
                return Ok(
                    json!({"type":"image","mime_type":image["mimeType"],"data":image["data"]}),
                );
            }
            if let Some(image) = part.get("fileData") {
                return Ok(json!({"type":"image","uri":image["fileUri"]}));
            }
            Err(error("Gemini Interactions unsupported input content"))
        })
        .collect()
}

fn function_step(call: &Value) -> Result<Value, AdapterError> {
    Ok(
        json!({"type":"function_call","id":required_string(&call["id"],"call ID")?,
        "name":required_string(&call["function"]["name"],"function name")?,"arguments":arguments(call)?}),
    )
}

fn assistant_steps(message: &Value) -> Result<Vec<Value>, AdapterError> {
    let calls: Vec<&Value> = message["tool_calls"]
        .as_array()
        .into_iter()
        .flatten()
        .collect();
    let Some(metadata) = message["protocol_state"]["steps"].as_array() else {
        let mut steps = Vec::new();
        let blocks = content(&message["content"])?;
        if !blocks.is_empty() {
            steps.push(json!({"type":"model_output","content":blocks}));
        }
        for call in calls {
            steps.push(function_step(call)?);
        }
        return Ok(steps);
    };
    let lengths: Vec<usize> = metadata
        .iter()
        .filter(|step| step["type"] == "model_output")
        .flat_map(|step| step["lengths"].as_array().into_iter().flatten())
        .map(|length| length.as_u64().unwrap_or(0) as usize)
        .collect();
    let (texts, _) = split_current_text(message["content"].as_str().unwrap_or(""), &lengths);
    let mut texts = texts.into_iter();
    let mut steps = Vec::new();
    let mut used = std::collections::BTreeSet::new();
    for descriptor in metadata {
        match descriptor["type"].as_str() {
            Some("thought") => steps.push(descriptor.clone()),
            Some("model_output") => {
                let count = descriptor["lengths"].as_array().map_or(0, Vec::len);
                let content: Vec<Value> = texts
                    .by_ref()
                    .take(count)
                    .filter(|text| !text.is_empty())
                    .map(|text| json!({"type":"text","text":text}))
                    .collect();
                if !content.is_empty() {
                    steps.push(json!({"type":"model_output","content":content}));
                }
            }
            Some("function_call") => {
                if let Some(call) = calls
                    .iter()
                    .find(|call| call["id"] == descriptor["call_id"])
                {
                    used.insert(required_string(&call["id"], "call ID")?.to_owned());
                    steps.push(function_step(call)?);
                }
            }
            _ => return Err(error("Gemini Interactions saved step metadata is invalid")),
        }
    }
    let content: Vec<Value> = texts
        .filter(|text| !text.is_empty())
        .map(|text| json!({"type":"text","text":text}))
        .collect();
    if !content.is_empty() {
        steps.push(json!({"type":"model_output","content":content}));
    }
    for call in calls {
        if !used.contains(required_string(&call["id"], "call ID")?) {
            steps.push(function_step(call)?);
        }
    }
    Ok(steps)
}

pub(crate) fn project_request(chat: Value) -> Result<Value, AdapterError> {
    let mut input = Vec::new();
    let mut instructions = Vec::new();
    let mut calls: BTreeMap<String, String> = BTreeMap::new();
    for message in chat["messages"]
        .as_array()
        .ok_or_else(|| error("Gemini Interactions request is missing messages"))?
    {
        match message["role"].as_str() {
            Some("system" | "developer") => {
                let text = message["content"]
                    .as_str()
                    .ok_or_else(|| error("Gemini Interactions system instructions must be text"))?;
                instructions.push(text);
            }
            Some("user") => {
                input.push(json!({"type":"user_input","content":content(&message["content"])?}))
            }
            Some("assistant") => {
                input.extend(assistant_steps(message)?);
                for call in message["tool_calls"].as_array().into_iter().flatten() {
                    calls.insert(
                        required_string(&call["id"], "call ID")?.into(),
                        required_string(&call["function"]["name"], "function name")?.into(),
                    );
                }
            }
            Some("tool") => {
                let id = required_string(&message["tool_call_id"], "tool result ID")?;
                let name = calls.get(id).ok_or_else(|| {
                    error(format!(
                        "Gemini Interactions tool result has no matching call: {id}"
                    ))
                })?;
                let result = message["content"].as_str().unwrap_or("");
                let mut step =
                    json!({"type":"function_result","call_id":id,"name":name,"result":result});
                if let Ok(value) = serde_json::from_str::<Value>(result) {
                    if value.get("error_code").is_some() || value["is_error"] == true {
                        step["is_error"] = json!(true);
                    }
                }
                input.push(step);
            }
            role => {
                return Err(error(format!(
                    "Gemini Interactions unsupported message role: {role:?}"
                )))
            }
        }
    }
    let mut body = json!({"model":chat["model"],"input":input,"store":false,"stream":chat.get("stream").cloned().unwrap_or(json!(true)),"generation_config":{}});
    if !instructions.is_empty() {
        body["system_instruction"] = json!(instructions.join("\n\n"));
    }
    for (source, target) in [
        ("max_tokens", "max_output_tokens"),
        ("temperature", "temperature"),
        ("top_p", "top_p"),
        ("stop", "stop_sequences"),
        ("reasoning_effort", "thinking_level"),
    ] {
        if let Some(value) = chat.get(source) {
            body["generation_config"][target] = value.clone();
        }
    }
    let tools: Vec<Value> = chat["tools"].as_array().into_iter().flatten().map(|tool| json!({
        "type":"function","name":tool["function"]["name"],"description":tool["function"]["description"],
        "parameters":tool["function"]["parameters"],
    })).collect();
    if !tools.is_empty() {
        body["tools"] = json!(tools);
        let choice = &chat["tool_choice"];
        body["generation_config"]["tool_choice"] =
            if let Some(name) = choice["function"]["name"].as_str() {
                json!({"allowed_tools":{"mode":"any","tools":[name]}})
            } else {
                json!(match choice.as_str() {
                    Some("required") => "any",
                    Some("none") => "none",
                    _ => "auto",
                })
            };
    }
    match chat["response_format"]["type"].as_str() {
        Some("json_object") => {
            body["response_format"] = json!({"type":"text","mime_type":"application/json"})
        }
        Some("json_schema") => {
            body["response_format"] = json!({"type":"text","mime_type":"application/json","schema":chat["response_format"]["json_schema"]["schema"]})
        }
        _ => {}
    }
    Ok(body)
}

pub(crate) fn model_usage(value: &Value) -> Option<ModelUsage> {
    let usage = value
        .get("usage")
        .or_else(|| value.get("interaction")?.get("usage"))?;
    let thoughts = count(&usage["total_thought_tokens"]);
    let snapshot = ModelUsage {
        input_tokens: count(&usage["total_input_tokens"]).map(|tokens| {
            tokens.saturating_add(count(&usage["total_tool_use_tokens"]).unwrap_or(0))
        }),
        // Interactions total_output_tokens excludes total_thought_tokens.
        output_tokens: count(&usage["total_output_tokens"])
            .map(|tokens| tokens.saturating_add(thoughts.unwrap_or(0))),
        reasoning_output_tokens: thoughts,
        cached_input_tokens: count(&usage["total_cached_tokens"]),
        total_tokens: count(&usage["total_tokens"]),
        ..Default::default()
    };
    snapshot.has_any_value().then_some(snapshot)
}

fn normalize(interaction: &Value) -> Result<Value, AdapterError> {
    let status = interaction["status"]
        .as_str()
        .ok_or_else(|| error("Gemini Interactions stream ended without a terminal status"))?;
    if !matches!(status, "completed" | "requires_action") {
        // The API's incomplete status is not exclusively an output limit. Do not
        // retry safety/unknown failures by pretending they are MAX_TOKENS.
        return Err(error(format!(
            "Gemini Interactions did not complete: {status}; {}",
            interaction["errors"]
        )));
    }
    let steps = interaction["steps"]
        .as_array()
        .ok_or_else(|| error("Gemini Interactions response is missing steps"))?;
    let mut text = String::new();
    let mut calls = Vec::new();
    let mut replay = Vec::new();
    for step in steps {
        match step["type"].as_str() {
            Some("model_output") => {
                let mut lengths = Vec::new();
                for block in step["content"]
                    .as_array()
                    .ok_or_else(|| error("Gemini Interactions output is missing content"))?
                {
                    if block["type"] != "text" {
                        return Err(error(
                            "Gemini Interactions returned unsupported non-text output",
                        ));
                    }
                    let fragment = block["text"]
                        .as_str()
                        .ok_or_else(|| error("Gemini Interactions text block is missing text"))?;
                    text.push_str(fragment);
                    lengths.push(fragment.len());
                }
                replay.push(json!({"type":"model_output","lengths":lengths}));
            }
            Some("thought") => replay.push(step.clone()),
            Some("function_call") => {
                let id = required_string(&step["id"], "function call ID")?;
                let name = required_string(&step["name"], "function name")?;
                if !step["arguments"].is_object() {
                    return Err(error(
                        "Gemini Interactions function arguments must be a complete object",
                    ));
                }
                calls.push(json!({"id":id,"type":"function","function":{"name":name,"arguments":step["arguments"].to_string()}}));
                replay.push(json!({"type":"function_call","call_id":id}));
            }
            // Some deployments include the input in the returned timeline.
            Some("user_input" | "function_result") => {}
            kind => {
                return Err(error(format!(
                    "Gemini Interactions unsupported response step: {kind:?}"
                )))
            }
        }
    }
    if (status == "requires_action") != !calls.is_empty() {
        return Err(error(
            "Gemini Interactions status does not match its function calls",
        ));
    }
    if calls.is_empty() && text.is_empty() {
        return Err(error(
            "Gemini Interactions returned no answer or function calls",
        ));
    }
    let usage = model_usage(interaction)
        .map(|usage| usage.as_provider_usage())
        .unwrap_or(Value::Null);
    Ok(
        json!({"choices":[{"message":{"role":"assistant","content":if text.is_empty(){Value::Null}else{json!(text)},
        "tool_calls":calls,"protocol_state":{"steps":replay}},"finish_reason":if status=="requires_action"{"tool_calls"}else{"stop"}}],"usage":usage}),
    )
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
        let interaction: Value = response.into_json().map_err(error)?;
        cancellation.check().map_err(|err| error(err.message))?;
        if let Some(usage) = model_usage(&interaction) {
            observer.observe(ModelDelta::Usage(usage));
        }
        let result = normalize(&interaction)?;
        observe_result(&result, observer, true);
        Ok(result)
    }
}

struct Step {
    value: Value,
    closed: bool,
    arguments: String,
    argument_deltas: bool,
    tool_index: Option<usize>,
}

impl Step {
    fn start(
        value: Value,
        tool_index: usize,
        observer: &mut dyn ModelObserver,
    ) -> Result<Self, AdapterError> {
        let mut step = Self {
            value,
            closed: false,
            arguments: String::new(),
            argument_deltas: false,
            tool_index: None,
        };
        match step.value["type"].as_str() {
            Some("model_output") => {
                if step.value.get("content").is_none() {
                    step.value["content"] = json!([]);
                }
                for block in step.value["content"]
                    .as_array()
                    .ok_or_else(|| error("Gemini Interactions invalid output content"))?
                {
                    if block["type"] != "text" {
                        return Err(error("Gemini Interactions unsupported output content"));
                    }
                    if let Some(text) = block["text"].as_str().filter(|text| !text.is_empty()) {
                        observer.observe(ModelDelta::Text(text.into()));
                    }
                }
            }
            Some("thought") => {}
            Some("function_call") => {
                required_string(&step.value["id"], "function call ID")?;
                required_string(&step.value["name"], "function name")?;
                step.tool_index = Some(tool_index);
                if let Some(args) = step.value.get("arguments") {
                    if !args.is_object() {
                        return Err(error(
                            "Gemini Interactions invalid initial function arguments",
                        ));
                    }
                    step.arguments = args.to_string();
                }
            }
            kind => {
                return Err(error(format!(
                    "Gemini Interactions unsupported streaming step: {kind:?}"
                )))
            }
        }
        Ok(step)
    }
    fn delta(
        &mut self,
        delta: &Value,
        observer: &mut dyn ModelObserver,
    ) -> Result<(), AdapterError> {
        if self.closed {
            return Err(error("Gemini Interactions delta after step.stop"));
        }
        match (self.value["type"].as_str(), delta["type"].as_str()) {
            (Some("model_output"), Some("text")) => {
                let text = delta["text"]
                    .as_str()
                    .ok_or_else(|| error("Gemini Interactions text delta is missing text"))?;
                let blocks = self.value["content"].as_array_mut().unwrap();
                if let Some(last) = blocks.last_mut().filter(|last| last["type"] == "text") {
                    let mut combined = last["text"].as_str().unwrap_or("").to_owned();
                    combined.push_str(text);
                    last["text"] = json!(combined);
                } else {
                    blocks.push(json!({"type":"text","text":text}));
                }
                if !text.is_empty() {
                    observer.observe(ModelDelta::Text(text.into()));
                }
            }
            (Some("thought"), Some("thought_signature")) => {
                self.value["signature"] =
                    json!(required_string(&delta["signature"], "thought signature")?);
            }
            (Some("thought"), Some("thought_summary")) => {
                if !delta["content"].is_object() {
                    return Err(error("Gemini Interactions invalid thought summary"));
                }
                if self.value.get("summary").is_none() {
                    self.value["summary"] = json!([]);
                }
                self.value["summary"]
                    .as_array_mut()
                    .ok_or_else(|| error("Gemini Interactions invalid thought summary list"))?
                    .push(delta["content"].clone());
            }
            (Some("function_call"), Some("arguments_delta")) => {
                let fragment = delta["arguments"]
                    .as_str()
                    .ok_or_else(|| error("Gemini Interactions missing arguments delta"))?;
                if !self.argument_deltas {
                    self.arguments.clear();
                    self.argument_deltas = true;
                }
                self.arguments.push_str(fragment);
                observer.observe(ModelDelta::ToolArguments {
                    index: self.tool_index.unwrap(),
                    id: self.value["id"].as_str().unwrap().into(),
                    name: self.value["name"].as_str().unwrap().into(),
                    arguments: self.arguments.clone(),
                });
            }
            pair => {
                return Err(error(format!(
                    "Gemini Interactions unsupported step delta: {pair:?}"
                )))
            }
        }
        Ok(())
    }
    fn close(&mut self) -> Result<(), AdapterError> {
        if self.closed {
            return Err(error("Gemini Interactions repeated step.stop"));
        }
        if self.tool_index.is_some() {
            let args: Value = if self.arguments.is_empty() {
                json!({})
            } else {
                serde_json::from_str(&self.arguments).map_err(error)?
            };
            if !args.is_object() {
                return Err(error(
                    "Gemini Interactions function arguments must be an object",
                ));
            }
            self.value["arguments"] = args;
        }
        self.closed = true;
        Ok(())
    }
}

pub(crate) fn read_sse(
    reader: impl Read,
    cancellation: &CancellationToken,
    observer: &mut dyn ModelObserver,
) -> Result<Value, AdapterError> {
    let mut steps: BTreeMap<usize, Step> = BTreeMap::new();
    let mut interaction: Option<Value> = None;
    let mut tool_count = 0;
    read_sse_events(reader, cancellation, |event| {
        if interaction.is_some() {
            return Err(error("Gemini Interactions event after completion"));
        }
        let index = || {
            event["index"]
                .as_u64()
                .and_then(|index| usize::try_from(index).ok())
                .ok_or_else(|| error("Gemini Interactions step is missing index"))
        };
        match event["event_type"].as_str() {
            Some("interaction.created" | "interaction.status_update") => {}
            Some("step.start") => {
                let index = index()?;
                if steps.contains_key(&index) {
                    return Err(error("Gemini Interactions repeated step index"));
                }
                let step = Step::start(event["step"].clone(), tool_count, observer)?;
                if step.tool_index.is_some() {
                    tool_count += 1;
                }
                steps.insert(index, step);
            }
            Some("step.delta") => steps
                .get_mut(&index()?)
                .ok_or_else(|| error("Gemini Interactions delta before step.start"))?
                .delta(&event["delta"], observer)?,
            Some("step.stop") => {
                steps
                    .get_mut(&index()?)
                    .ok_or_else(|| error("Gemini Interactions stop before step.start"))?
                    .close()?;
                if let Some(usage) = model_usage(&event) {
                    observer.observe(ModelDelta::Usage(usage));
                }
            }
            Some("interaction.completed") => {
                if let Some(usage) = model_usage(&event) {
                    observer.observe(ModelDelta::Usage(usage));
                }
                if steps.values().any(|step| !step.closed) {
                    return Err(error(
                        "Gemini Interactions completed with an unfinished step",
                    ));
                }
                let mut final_value = event["interaction"].clone();
                final_value["steps"] = json!(steps
                    .values()
                    .map(|step| step.value.clone())
                    .collect::<Vec<_>>());
                interaction = Some(final_value);
            }
            Some("error") => return Err(error(format!("Gemini Interactions: {}", event["error"]))),
            kind => {
                return Err(error(format!(
                    "Gemini Interactions unsupported stream event: {kind:?}"
                )))
            }
        }
        Ok(())
    })?;
    let interaction = interaction
        .ok_or_else(|| error("Gemini Interactions stream ended before interaction.completed"))?;
    let result = normalize(&interaction)?;
    observe_result(&result, observer, false);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn event(kind: &str, mut value: Value) -> String {
        value["event_type"] = json!(kind);
        format!("event: {kind}\ndata: {value}\n\n")
    }
    fn streamed_call() -> String {
        event(
            "interaction.created",
            json!({"interaction":{"id":"response","status":"in_progress"}}),
        ) + &event("step.start", json!({"index":0,"step":{"type":"thought"}}))
            + &event(
                "step.delta",
                json!({"index":0,"delta":{"type":"thought_signature","signature":"opaque"}}),
            )
            + &event("step.stop", json!({"index":0}))
            + &event(
                "step.start",
                json!({"index":1,"step":{"type":"function_call","id":"call-a","name":"read"}}),
            )
            + &event(
                "step.start",
                json!({"index":2,"step":{"type":"function_call","id":"call-b","name":"read"}}),
            )
            + &event(
                "step.delta",
                json!({"index":1,"delta":{"type":"arguments_delta","arguments":"{\"lid\":"}}),
            )
            + &event(
                "step.delta",
                json!({"index":2,"delta":{"type":"arguments_delta","arguments":"{\"lid\":\"p2\"}"}}),
            )
            + &event(
                "step.delta",
                json!({"index":1,"delta":{"type":"arguments_delta","arguments":"\"p1\"}"}}),
            )
            + &event("step.stop", json!({"index":1}))
            + &event("step.stop", json!({"index":2}))
            + &event(
                "interaction.completed",
                json!({"interaction":{"id":"response","status":"requires_action","usage":{
            "total_input_tokens":7,"total_output_tokens":23,"total_thought_tokens":49,"total_tokens":79}}}),
            )
            + "event: done\ndata: [DONE]\n\n"
    }
    #[test]
    fn gemini_interactions_stream_assembles_parallel_calls_and_replays_thoughts() {
        let response = read_sse(
            streamed_call().as_bytes(),
            &CancellationToken::default(),
            &mut crate::provider_stream::ignore,
        )
        .unwrap();
        let message = &response["choices"][0]["message"];
        assert_eq!(
            message["tool_calls"][0]["function"]["arguments"],
            "{\"lid\":\"p1\"}"
        );
        assert_eq!(message["tool_calls"][1]["id"], "call-b");
        assert_eq!(response["usage"]["completion_tokens"], 72);
        assert_eq!(response["usage"]["total_tokens"], 79);
        let request=project_request(json!({"model":"gemini-test","max_tokens":2048,"messages":[
            {"role":"system","content":"cite the source"},{"role":"user","content":"read two pages"},message,
            {"role":"tool","tool_call_id":"call-a","content":"page one"},
            {"role":"tool","tool_call_id":"call-b","content":"page two"}]})).unwrap();
        assert_eq!(request["store"], false);
        assert!(request.get("previous_interaction_id").is_none());
        assert_eq!(
            request["input"][1],
            json!({"type":"thought","signature":"opaque"})
        );
        assert_eq!(request["input"][4]["call_id"], "call-a");
        assert_eq!(request["input"][5]["name"], "read");
        assert_eq!(request["generation_config"]["max_output_tokens"], 2048);
    }
    #[test]
    fn gemini_interactions_projects_images_json_schema_and_endpoint_query() {
        let body=project_request(json!({"model":"gemini-test","messages":[{"role":"user","content":[
            {"type":"image_url","image_url":{"url":"data:image/png;base64,aW1hZ2U="}}]}],
            "response_format":{"type":"json_schema","json_schema":{"schema":{"type":"object","properties":{"answer":{"type":"string"}}}}}})).unwrap();
        assert_eq!(
            body["input"][0]["content"][0],
            json!({"type":"image","mime_type":"image/png","data":"aW1hZ2U="})
        );
        assert_eq!(
            body["response_format"]["schema"]["properties"]["answer"]["type"],
            "string"
        );
        assert_eq!(
            endpoint("https://example.com/v1beta?quota=p", "m").unwrap(),
            "https://example.com/v1beta/interactions?quota=p"
        );
    }
    #[test]
    fn gemini_interactions_stream_text_not_thoughts_and_usage_is_snapshot() {
        let wire = event(
            "step.start",
            json!({"index":0,"step":{"type":"model_output","content":[{"type":"text","text":"A"}]}}),
        ) + &event(
            "step.delta",
            json!({"index":0,"delta":{"type":"text","text":"B"}}),
        ) + &event(
            "step.stop",
            json!({"index":0,"usage":{"total_input_tokens":3,"total_output_tokens":1,"total_tokens":4}}),
        ) + &event(
            "interaction.completed",
            json!({"interaction":{"status":"completed","usage":{"total_input_tokens":3,"total_output_tokens":2,"total_tokens":5}}}),
        );
        let mut events = Vec::new();
        let result = read_sse(
            wire.as_bytes(),
            &CancellationToken::default(),
            &mut |event| events.push(event),
        )
        .unwrap();
        assert_eq!(result["choices"][0]["message"]["content"], "AB");
        assert_eq!(result["usage"]["total_tokens"], 5);
        assert_eq!(
            events
                .iter()
                .filter_map(|event| if let ModelDelta::Text(t) = event {
                    Some(t.as_str())
                } else {
                    None
                })
                .collect::<Vec<_>>(),
            vec!["A", "B"]
        );
    }
    #[test]
    fn gemini_interactions_rejects_disconnect_partial_arguments_failed_and_incomplete() {
        let open = event(
            "step.start",
            json!({"index":0,"step":{"type":"function_call","id":"c","name":"read"}}),
        ) + &event(
            "step.delta",
            json!({"index":0,"delta":{"type":"arguments_delta","arguments":"{\"lid\":"}}),
        );
        assert!(read_sse(
            open.as_bytes(),
            &CancellationToken::default(),
            &mut crate::provider_stream::ignore
        )
        .is_err());
        let unfinished = open.clone()
            + &event(
                "interaction.completed",
                json!({"interaction":{"status":"requires_action"}}),
            );
        assert!(read_sse(
            unfinished.as_bytes(),
            &CancellationToken::default(),
            &mut crate::provider_stream::ignore
        )
        .is_err());
        let invalid = open + &event("step.stop", json!({"index":0}));
        assert!(read_sse(
            invalid.as_bytes(),
            &CancellationToken::default(),
            &mut crate::provider_stream::ignore
        )
        .is_err());
        for status in ["failed", "cancelled", "incomplete", "requires_continuation"] {
            assert!(normalize(&json!({"status":status,"steps":[]})).is_err());
        }
        assert!(normalize(&json!({"status":"requires_action","steps":[]})).is_err());
    }
    #[test]
    fn gemini_interactions_cancellation_prevents_reading() {
        let token = CancellationToken::default();
        token.cancel();
        assert!(read_sse(
            streamed_call().as_bytes(),
            &token,
            &mut crate::provider_stream::ignore
        )
        .is_err());
    }
    #[test]
    fn gemini_interactions_replay_does_not_restore_retired_facts() {
        let response=normalize(&json!({"status":"requires_action","steps":[{"type":"thought","signature":"sig"},
            {"type":"model_output","content":[{"type":"text","text":"long retired answer"}]},
            {"type":"function_call","id":"c","name":"read","arguments":{"text":"long retired argument"}}]})).unwrap();
        let mut message = response["choices"][0]["message"].clone();
        assert!(!message["protocol_state"]
            .to_string()
            .contains("long retired"));
        message["content"] = json!("short");
        message["tool_calls"][0]["function"]["arguments"] = json!("{\"text\":\"retired\"}");
        let request = project_request(json!({"model":"m","messages":[message]})).unwrap();
        assert!(!request.to_string().contains("long retired"));
        assert_eq!(request["input"][0]["signature"], "sig");
        assert_eq!(request["input"][1]["content"][0]["text"], "short");
        assert_eq!(request["input"][2]["arguments"]["text"], "retired");
    }
    #[test]
    fn gemini_interactions_usage_includes_tool_prompt_once() {
        let usage=model_usage(&json!({"usage":{"total_input_tokens":10,"total_tool_use_tokens":4,"total_output_tokens":2,"total_thought_tokens":3,"total_tokens":19}})).unwrap();
        assert_eq!(usage.input_tokens, Some(14));
        assert_eq!(usage.output_tokens, Some(5));
        assert_eq!(usage.total_tokens, Some(19));
    }
}
