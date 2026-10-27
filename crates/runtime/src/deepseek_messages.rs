//! Capability-gated DeepSeek Messages requests. Execution permissions remain in
//! the current request's tools; retained declarations only preserve context.
use crate::{AdapterError, AgentRequestPlan, Message, Role, ToolSpec};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashSet};

pub const TOOL_CHANGES_BETA: &str = "mid-conversation-tool-changes-2026-07-01";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ToolUpdateMode {
    AdditionOnly,
    InHistory,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolDeclaration {
    pub spec: ToolSpec,
    pub defer_loading: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolUpdate {
    /// Before request.input[index]; input.len() means after its last message.
    pub message_index: usize,
    pub added: Vec<String>,
    pub removed: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolHistoryProjection {
    pub declarations: Vec<ToolDeclaration>,
    pub updates: Vec<ToolUpdate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MessagesCapabilities {
    pub tool_update: ToolUpdateMode,
    pub system_in_history: bool,
}

/// No name-prefix or alias inheritance: capabilities belong to the exact route.
pub(crate) fn capabilities_from_listing(
    listing: &Value,
    model: &str,
) -> Option<MessagesCapabilities> {
    let entry = listing
        .get("data")?
        .as_array()?
        .iter()
        .find(|entry| entry["id"] == model)?;
    let capabilities = &entry["api_capabilities"]["anthropic_messages"];
    let tool_update = match capabilities["tool_update"].as_str()? {
        "addition-only" => ToolUpdateMode::AdditionOnly,
        "in-history" => ToolUpdateMode::InHistory,
        _ => return None,
    };
    if entry.get("output_modalities").is_some_and(|v| {
        !v.as_array()
            .is_some_and(|values| values.iter().any(|v| v == "text"))
    }) {
        return None;
    }
    Some(MessagesCapabilities {
        tool_update,
        system_in_history: capabilities["system_prompt_update"] == "in-history",
    })
}

pub(crate) fn capabilities_for_route(
    listing: &Value,
    base_url: &str,
    model: &str,
) -> Option<MessagesCapabilities> {
    // The official pricing/model contract explicitly serves these retired names
    // with deepseek-flash; the 2026-10-25 Messages/deferred live probe confirms it.
    // https://api-docs.deepseek.com/quick_start/pricing/
    // Never apply this provider-owned alias contract to proxies or override an
    // explicitly listed model's own capability (including leading-only).
    let exact_present = listing["data"]
        .as_array()
        .is_some_and(|entries| entries.iter().any(|entry| entry["id"] == model));
    if exact_present {
        return capabilities_from_listing(listing, model);
    }
    let official = url::Url::parse(base_url)
        .ok()
        .is_some_and(|url| url.origin().ascii_serialization() == "https://api.deepseek.com");
    if official && matches!(model, "deepseek-v4-flash" | "deepseek-v4-flash-vision-exp") {
        capabilities_from_listing(listing, "deepseek-flash")
    } else {
        None
    }
}

/// Derive only paths on the configured origin; never redirect credentials to a
/// different service. Official root and /v1 Chat endpoints share /anthropic/v1.
pub(crate) fn endpoint_urls(base_url: &str) -> (String, String) {
    let base = base_url.trim_end_matches('/');
    let parsed = url::Url::parse(base).ok();
    if parsed
        .as_ref()
        .is_some_and(|url| url.host_str() == Some("api.deepseek.com"))
    {
        let url = parsed.unwrap();
        let origin = url.origin().ascii_serialization();
        return (
            format!("{origin}/models"),
            format!("{origin}/anthropic/v1/messages"),
        );
    }
    (format!("{base}/models"), format!("{base}/messages"))
}

fn error(message: impl Into<String>) -> AdapterError {
    AdapterError {
        spend_stop: None,
        message: message.into(),
    }
}

fn tool_input(arguments: &str) -> Value {
    serde_json::from_str::<Value>(arguments)
        .ok()
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}))
}

/// Store only replay metadata. Text and arguments already live in Message;
/// keeping native copies would resurrect bodies that pressure retirement removed.
pub(crate) fn continuation_blocks(blocks: &[Value]) -> Vec<Value> {
    blocks
        .iter()
        .map(|block| {
            let mut metadata = block.clone();
            if let Some(fields) = metadata.as_object_mut() {
                match block["type"].as_str() {
                    Some("text") => {
                        fields.remove("text");
                        fields.insert(
                            "text_bytes".into(),
                            json!(block["text"].as_str().unwrap_or("").len()),
                        );
                    }
                    Some("thinking") => {
                        fields.remove("thinking");
                        fields.insert(
                            "thinking_bytes".into(),
                            json!(block["thinking"].as_str().unwrap_or("").len()),
                        );
                    }
                    Some("tool_use") => {
                        fields.remove("input");
                    }
                    _ => {}
                }
            }
            metadata
        })
        .collect()
}

fn replay_blocks(message: &Message, state: &crate::ProviderContinuation) -> Vec<Value> {
    let mut text_offset: usize = 0;
    let mut thinking_offset: usize = 0;
    state
        .messages_content
        .iter()
        .map(|block| {
            let mut expanded = block.clone();
            for (length_key, field, source, offset) in [
                (
                    "text_bytes",
                    "text",
                    message.content.as_deref().unwrap_or(""),
                    &mut text_offset,
                ),
                (
                    "thinking_bytes",
                    "thinking",
                    state.reasoning_content.as_str(),
                    &mut thinking_offset,
                ),
            ] {
                if let Some(length) = block[length_key]
                    .as_u64()
                    .and_then(|value| usize::try_from(value).ok())
                {
                    let end = offset.saturating_add(length);
                    expanded[field] = json!(source.get(*offset..end).unwrap_or(""));
                    expanded.as_object_mut().unwrap().remove(length_key);
                    *offset = end;
                }
            }
            expanded
        })
        .collect()
}

fn append_message(messages: &mut Vec<Value>, role: &str, blocks: Vec<Value>) {
    if blocks.is_empty() {
        return;
    }
    if let Some(previous) = messages
        .last_mut()
        .filter(|message| message["role"] == role)
    {
        previous["content"].as_array_mut().unwrap().extend(blocks);
    } else {
        messages.push(json!({"role":role, "content":blocks}));
    }
}

fn flush_updates(messages: &mut Vec<Value>, updates: &mut Vec<Value>) -> Result<(), AdapterError> {
    if updates.is_empty() {
        return Ok(());
    }
    if messages.last().is_none_or(|m| m["role"] != "user") {
        return Err(error(
            "Messages system/tool update needs a preceding user or tool-result turn",
        ));
    }
    // Keep each full system snapshot separate: the latest one supersedes prior
    // snapshots; tool additions are independent protocol updates.
    messages.append(updates);
    Ok(())
}

fn assistant_blocks(
    message: &Message,
    model: &str,
    names: &BTreeMap<String, String>,
) -> Vec<Value> {
    let continuation = message
        .provider_continuation
        .as_ref()
        .filter(|value| value.model == model);
    let replay = continuation
        .map(|state| replay_blocks(message, state))
        .unwrap_or_default();
    let mut blocks = Vec::new();
    let mut seen_calls = HashSet::new();
    let replay_text = replay
        .iter()
        .filter_map(|block| (block["type"] == "text").then(|| block["text"].as_str().unwrap_or("")))
        .collect::<String>();
    let preserve_text_blocks = replay_text == message.content.as_deref().unwrap_or("");
    let mut text_written = false;
    for block in &replay {
        match block["type"].as_str() {
            Some("thinking" | "redacted_thinking") => blocks.push(block.clone()),
            Some("text") if preserve_text_blocks => {
                blocks.push(block.clone());
                text_written = true;
            }
            Some("text") if !text_written => {
                if let Some(text) = message.content.as_ref().filter(|text| !text.is_empty()) {
                    blocks.push(json!({"type":"text","text":text}));
                }
                text_written = true;
            }
            Some("tool_use") => {
                if let Some(call) = message
                    .tool_calls
                    .iter()
                    .find(|call| block["id"] == call.id)
                {
                    let mut updated = block.clone();
                    updated["name"] = json!(names.get(&call.name).unwrap_or(&call.name));
                    updated["input"] = tool_input(&call.arguments);
                    blocks.push(updated);
                    seen_calls.insert(call.id.as_str());
                }
            }
            _ => {}
        }
    }
    if replay.is_empty() {
        if let Some(state) = continuation.filter(|state| !state.reasoning_content.is_empty()) {
            blocks.push(json!({"type":"thinking", "thinking":state.reasoning_content}));
        }
    }
    if !text_written {
        if let Some(text) = message.content.as_ref().filter(|text| !text.is_empty()) {
            blocks.push(json!({"type":"text","text":text}));
        }
    }
    for call in &message.tool_calls {
        if !seen_calls.contains(call.id.as_str()) {
            blocks.push(json!({"type":"tool_use","id":call.id,"name":names.get(&call.name).unwrap_or(&call.name),"input":tool_input(&call.arguments)}));
        }
    }
    blocks
}

fn image_blocks(content: &Value) -> Result<Vec<Value>, AdapterError> {
    if let Some(text) = content.as_str() {
        return Ok(vec![json!({"type":"text","text":text})]);
    }
    let mut blocks = Vec::new();
    for block in content
        .as_array()
        .ok_or_else(|| error("Messages image content must be an array"))?
    {
        if block["type"] == "text" {
            blocks.push(block.clone());
        } else if block["type"] == "image_url" {
            let data = block["image_url"]["url"]
                .as_str()
                .ok_or_else(|| error("Messages image URL is missing"))?;
            let (media, bytes) = data
                .strip_prefix("data:")
                .and_then(|data| data.split_once(";base64,"))
                .ok_or_else(|| error("Messages inline image requires base64 data"))?;
            blocks.push(
                json!({"type":"image","source":{"type":"base64","media_type":media,"data":bytes}}),
            );
        }
    }
    Ok(blocks)
}

pub(crate) fn uses_tool_updates(body: &Value) -> bool {
    body["messages"].as_array().is_some_and(|messages| {
        messages.iter().any(|message| {
            message["content"].as_array().is_some_and(|blocks| {
                blocks.iter().any(|block| {
                    matches!(
                        block["type"].as_str(),
                        Some("tool_addition" | "tool_removal")
                    )
                })
            })
        })
    })
}

/// Produces both the request and the map of currently executable tool names.
pub(crate) fn request_projection(
    model: &str,
    request: &AgentRequestPlan,
    capabilities: MessagesCapabilities,
) -> Result<(Value, BTreeMap<String, String>), AdapterError> {
    request_projection_inner(model, request, capabilities, false)
}

/// A completed response's tool batch is an input-usage anchor even before tools
/// execute. This projection is never sent; real requests still require results.
pub(crate) fn context_projection(
    model: &str,
    request: &AgentRequestPlan,
    capabilities: MessagesCapabilities,
) -> Result<Value, AdapterError> {
    request_projection_inner(model, request, capabilities, true).map(|value| value.0)
}

fn request_projection_inner(
    model: &str,
    request: &AgentRequestPlan,
    capabilities: MessagesCapabilities,
    allow_open_tail: bool,
) -> Result<(Value, BTreeMap<String, String>), AdapterError> {
    let fallback;
    let history = if let Some(history) = &request.tool_history {
        history
    } else {
        fallback = ToolHistoryProjection {
            declarations: request
                .tools
                .iter()
                .cloned()
                .map(|spec| ToolDeclaration {
                    spec,
                    defer_loading: false,
                })
                .collect(),
            updates: vec![],
        };
        &fallback
    };
    let mut used = HashSet::new();
    let mut names: BTreeMap<String, String> = crate::tool_registry::ToolHandlerId::ALL
        .iter()
        .enumerate()
        .map(|(index, handler)| {
            let name = handler.canonical_name();
            (
                name.to_owned(),
                crate::provider_tool_name(name, index, &mut used),
            )
        })
        .collect();
    for (index, declaration) in history.declarations.iter().enumerate() {
        if !names.contains_key(&declaration.spec.name) {
            names.insert(
                declaration.spec.name.clone(),
                crate::provider_tool_name(&declaration.spec.name, index, &mut used),
            );
        }
    }
    let executable_names = request
        .tools
        .iter()
        .filter_map(|tool| {
            names
                .get(&tool.name)
                .map(|name| (name.clone(), tool.name.clone()))
        })
        .collect();
    let declared_names: HashSet<&str> = history
        .declarations
        .iter()
        .map(|tool| tool.spec.name.as_str())
        .collect();
    if history
        .updates
        .iter()
        .any(|update| update.message_index > request.input.len())
    {
        return Err(error("Messages tool update refers to removed history"));
    }
    let tools: Vec<Value> = history.declarations.iter().map(|declaration| {
        let mut tool = json!({"name":names[&declaration.spec.name],"description":declaration.spec.description,"input_schema":declaration.spec.parameters});
        if declaration.defer_loading { tool["defer_loading"] = json!(true); }
        tool
    }).collect();
    let mut system = request.instructions.clone();
    let mut messages = Vec::new();
    let mut updates = Vec::new();
    let mut effective_system = system.clone();
    for index in 0..=request.input.len() {
        for update in history
            .updates
            .iter()
            .filter(|update| update.message_index == index)
        {
            let mut blocks = Vec::new();
            for name in &update.added {
                if !declared_names.contains(name.as_str()) {
                    return Err(error(format!(
                        "Messages tool addition has no declaration: {name}"
                    )));
                }
                blocks.push(json!({"type":"tool_addition","tool":{"type":"tool_reference","name":names[name]}}));
            }
            if capabilities.tool_update == ToolUpdateMode::InHistory {
                for name in &update.removed {
                    if !declared_names.contains(name.as_str()) {
                        return Err(error(format!(
                            "Messages tool removal has no declaration: {name}"
                        )));
                    }
                    blocks.push(json!({"type":"tool_removal","tool":{"type":"tool_reference","name":names[name]}}));
                }
            }
            if !blocks.is_empty() {
                updates.push(json!({"role":"system","content":blocks}));
            }
        }
        let Some(message) = request.input.get(index) else {
            break;
        };
        match message.role {
            Role::System => {
                let Some(text) = message.content.as_ref().filter(|text| !text.is_empty()) else {
                    continue;
                };
                if !effective_system.is_empty() {
                    effective_system.push_str("\n\n");
                }
                effective_system.push_str(text);
                if messages.is_empty() || !capabilities.system_in_history {
                    system = effective_system.clone();
                } else {
                    updates.push(json!({"role":"system","content":[{"type":"text","text":effective_system}]}));
                }
            }
            Role::User => append_message(
                &mut messages,
                "user",
                message
                    .content
                    .as_ref()
                    .filter(|text| !text.is_empty())
                    .map(|text| vec![json!({"type":"text","text":text})])
                    .unwrap_or_default(),
            ),
            Role::Tool => {
                let id = message
                    .tool_call_id
                    .as_ref()
                    .ok_or_else(|| error("Messages tool result is missing call id"))?;
                append_message(
                    &mut messages,
                    "user",
                    vec![
                        json!({"type":"tool_result","tool_use_id":id,"content":message.content.as_deref().unwrap_or("")}),
                    ],
                );
            }
            Role::Assistant => {
                flush_updates(&mut messages, &mut updates)?;
                append_message(
                    &mut messages,
                    "assistant",
                    assistant_blocks(message, model, &names),
                );
            }
        }
    }
    let mut image_messages = Vec::new();
    crate::append_preview_images(&mut image_messages, request);
    for message in image_messages {
        append_message(&mut messages, "user", image_blocks(&message["content"])?);
    }
    flush_updates(&mut messages, &mut updates)?;
    // Tool results must be the immediate next user turn and precede its text.
    let mut pending = HashSet::new();
    for message in &mut messages {
        let role = message["role"].as_str().unwrap_or("").to_owned();
        let blocks = message["content"].as_array_mut().unwrap();
        if role == "assistant" {
            if !pending.is_empty() {
                return Err(error("Messages history has unresolved tool calls"));
            }
            for block in blocks.iter().filter(|block| block["type"] == "tool_use") {
                if !pending.insert(block["id"].as_str().unwrap_or("").to_owned()) {
                    return Err(error("Messages duplicate tool call id"));
                }
            }
        } else if role == "user" {
            for block in blocks.iter().filter(|block| block["type"] == "tool_result") {
                if !pending.remove(block["tool_use_id"].as_str().unwrap_or("")) {
                    return Err(error("Messages tool result has no matching call"));
                }
            }
            if !pending.is_empty() {
                return Err(error("Messages tool calls need immediate results"));
            }
            blocks.sort_by_key(|block| block["type"] != "tool_result");
        }
    }
    if !pending.is_empty() && !allow_open_tail {
        return Err(error("Messages history ends with unresolved tool calls"));
    }
    let mut body = json!({"model":model,"messages":messages,"tools":tools,"temperature":0,
        "thinking":{"type":"enabled"},"output_config":{"effort":"high"},
        "tool_choice":{"type":request.tool_choice.as_provider_value(),"disable_parallel_tool_use":!request.parallel_tool_calls}});
    if !system.is_empty() {
        body["system"] = json!(system);
    }
    if let Some(limit) = request.output_token_limit {
        body["max_tokens"] = json!(limit);
    }
    Ok((body, executable_names))
}

pub(crate) fn completion_projection(
    model: &str,
    request: &crate::CompletionRequest,
) -> Result<Value, AdapterError> {
    let messages = crate::completion_user_messages(request).into_iter()
        .map(|message| Ok(json!({"role":"user","content":image_blocks(&message["content"])?})))
        .collect::<Result<Vec<_>, AdapterError>>()?;
    let mut body = json!({"model":model,"system":request.system,"messages":messages,"temperature":0});
    let effort = request.reasoning_effort.as_deref().unwrap_or("high");
    if effort == "off" {
        body["thinking"] = json!({"type":"disabled"});
    } else if matches!(effort, "low" | "high" | "max") {
        body["thinking"] = json!({"type":"enabled"});
        body["output_config"] = json!({"effort":effort});
    } else {
        return Err(error(format!(
            "Messages does not support reasoning effort {effort}"
        )));
    }
    if let Some(limit) = request.output_token_limit {
        body["max_tokens"] = json!(limit);
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ModelAdapter, NativeAdapter, ProviderConfig, ProviderContinuation, ProviderMode, ToolCall,
    };
    use std::{
        io::{Read, Write},
        net::{TcpListener, TcpStream},
        time::{Duration, Instant},
    };

    fn capabilities(mode: ToolUpdateMode) -> MessagesCapabilities {
        MessagesCapabilities {
            tool_update: mode,
            system_in_history: true,
        }
    }
    fn tool(name: &str) -> ToolSpec {
        ToolSpec {
            name: name.into(),
            description: format!("Use {name}"),
            parameters: json!({"type":"object","properties":{"value":{"type":"string"}},"required":["value"]}),
        }
    }
    fn plan(messages: &[Message], tools: &[ToolSpec]) -> AgentRequestPlan {
        AgentRequestPlan::for_ad_hoc(
            crate::ModelRuntimeCatalog::default().resolve(
                "fixture",
                crate::ProviderToolProtocol::Native,
                None,
            ),
            messages,
            tools,
        )
    }
    fn listing(model: &str) -> Value {
        json!({"data":[{"id":model,"api_capabilities":{"anthropic_messages":{"system_prompt_update":"in-history","tool_update":"addition-only"}}}]})
    }

    #[test]
    fn messages_capabilities_require_exact_model_and_explicit_tool_update() {
        assert_eq!(
            capabilities_from_listing(&listing("deepseek-flash"), "deepseek-flash"),
            Some(capabilities(ToolUpdateMode::AdditionOnly))
        );
        assert_eq!(
            capabilities_from_listing(&listing("deepseek-flash"), "deepseek-v4-flash"),
            None
        );
        for mode in ["leading-only", "unknown", ""] {
            let mut value = listing("deepseek-flash");
            value["data"][0]["api_capabilities"]["anthropic_messages"]["tool_update"] = json!(mode);
            assert_eq!(capabilities_from_listing(&value, "deepseek-flash"), None);
        }
        assert_eq!(
            endpoint_urls("https://api.deepseek.com/v1").1,
            "https://api.deepseek.com/anthropic/v1/messages"
        );
        assert_eq!(
            endpoint_urls("http://localhost:8000/v1").1,
            "http://localhost:8000/v1/messages"
        );
    }

    #[test]
    fn messages_only_official_documented_aliases_inherit_canonical_capability() {
        let mut models = listing("deepseek-flash");
        assert_eq!(
            capabilities_for_route(&models, "https://api.deepseek.com/v1", "deepseek-v4-flash"),
            Some(capabilities(ToolUpdateMode::AdditionOnly))
        );
        assert_eq!(
            capabilities_for_route(
                &models,
                "https://api.deepseek.com",
                "deepseek-v4-flash-vision-exp"
            ),
            Some(capabilities(ToolUpdateMode::AdditionOnly))
        );
        assert_eq!(
            capabilities_for_route(&models, "https://proxy.example/v1", "deepseek-v4-flash"),
            None
        );
        assert_eq!(
            capabilities_for_route(
                &models,
                "https://api.deepseek.com",
                "deepseek-unknown-flash"
            ),
            None
        );
        models["data"].as_array_mut().unwrap().push(json!({"id":"deepseek-v4-flash","api_capabilities":{"anthropic_messages":{"tool_update":"leading-only"}}}));
        assert_eq!(
            capabilities_for_route(&models, "https://api.deepseek.com", "deepseek-v4-flash"),
            None
        );
    }

    #[test]
    fn messages_addition_preserves_old_messages_and_defers_only_new_schema() {
        let old = plan(
            &[Message::system("base"), Message::user("first")],
            &[tool("read")],
        );
        let (before, _) =
            request_projection("fixture", &old, capabilities(ToolUpdateMode::AdditionOnly))
                .unwrap();
        let mut next = old.clone();
        next.input.push(Message {
            provider_continuation: None,
            role: Role::Assistant,
            content: Some("done".into()),
            tool_calls: vec![],
            tool_call_id: None,
        });
        next.input.push(Message::user("edit now"));
        next.tools.push(tool("edit"));
        next.tool_history = Some(ToolHistoryProjection {
            declarations: vec![
                ToolDeclaration {
                    spec: tool("read"),
                    defer_loading: false,
                },
                ToolDeclaration {
                    spec: tool("edit"),
                    defer_loading: true,
                },
            ],
            updates: vec![ToolUpdate {
                message_index: next.input.len(),
                added: vec!["edit".into()],
                removed: vec![],
            }],
        });
        let (after, names) =
            request_projection("fixture", &next, capabilities(ToolUpdateMode::AdditionOnly))
                .unwrap();
        assert_eq!(before["system"], after["system"]);
        assert_eq!(before["messages"][0], after["messages"][0]);
        assert_eq!(before["tools"][0], after["tools"][0]);
        assert_eq!(after["tools"][1]["defer_loading"], true);
        assert_eq!(after["messages"][3]["content"][0]["type"], "tool_addition");
        assert_eq!(after["messages"][3]["content"][0]["tool"]["name"], "edit");
        assert_eq!(names["edit"], "edit");
        assert!(uses_tool_updates(&after));
    }

    #[test]
    fn messages_replay_preserves_block_order_and_signatures_without_resurrecting_pruned_args() {
        let mut assistant = Message {
            provider_continuation: Some(ProviderContinuation {
                model: "fixture".into(),
                reasoning_content: "private".into(),
                messages_content: vec![
                    json!({"type":"thinking","thinking":"private","signature":"sig"}),
                    json!({"type":"text","text":"doing"}),
                    json!({"type":"tool_use","id":"c1","name":"read","input":{"value":"old large payload"}}),
                ],
            }),
            role: Role::Assistant,
            content: Some("doing".into()),
            tool_calls: vec![ToolCall {
                id: "c1".into(),
                name: "read".into(),
                arguments: r#"{"value":"locator"}"#.into(),
            }],
            tool_call_id: None,
        };
        let state = assistant.provider_continuation.as_mut().unwrap();
        state.messages_content = continuation_blocks(&state.messages_content);
        let stored = serde_json::to_string(&state.messages_content).unwrap();
        assert!(!stored.contains("old large payload"));
        assert!(!stored.contains("private"));
        assert!(!stored.contains("doing"));
        let result = Message {
            provider_continuation: None,
            role: Role::Tool,
            content: Some("ok".into()),
            tool_calls: vec![],
            tool_call_id: Some("c1".into()),
        };
        let request = plan(&[Message::user("read"), assistant, result], &[tool("read")]);
        let (body, _) = request_projection(
            "fixture",
            &request,
            capabilities(ToolUpdateMode::AdditionOnly),
        )
        .unwrap();
        assert_eq!(body["messages"][1]["content"][0]["signature"], "sig");
        assert_eq!(
            body["messages"][1]["content"][2]["input"]["value"],
            "locator"
        );
        assert_eq!(body["messages"][2]["content"][0]["tool_use_id"], "c1");
        let (other, _) = request_projection(
            "different-model",
            &request,
            capabilities(ToolUpdateMode::AdditionOnly),
        )
        .unwrap();
        assert_eq!(other["messages"][1]["content"][0]["type"], "text");
        assert!(!other.to_string().contains("sig"));
    }

    #[test]
    fn messages_system_updates_are_full_effective_snapshots_and_images_remain_pixels() {
        let mut request = plan(
            &[
                Message::system("base"),
                Message::system("stable"),
                Message::user("question"),
                Message::system("dynamic"),
            ],
            &[],
        );
        request
            .preview_images
            .push(crate::presentation_author::PreviewImage {
                caption: "preview".into(),
                png_base64: "cGl4ZWxz".into(),
                candidate_id: None,
                environment_name: None,
            });
        let (body, _) = request_projection(
            "fixture",
            &request,
            capabilities(ToolUpdateMode::AdditionOnly),
        )
        .unwrap();
        assert_eq!(body["system"], "base\n\nstable");
        assert_eq!(
            body["messages"][0]["content"][2]["source"]["data"],
            "cGl4ZWxz"
        );
        assert_eq!(
            body["messages"][1]["content"][0]["text"],
            "base\n\nstable\n\ndynamic"
        );
    }

    #[test]
    fn messages_meter_can_project_completed_tool_response_but_send_requires_results() {
        let request = plan(
            &[
                Message::user("read"),
                Message {
                    provider_continuation: None,
                    role: Role::Assistant,
                    content: None,
                    tool_calls: vec![ToolCall {
                        id: "open".into(),
                        name: "read".into(),
                        arguments: "{}".into(),
                    }],
                    tool_call_id: None,
                },
            ],
            &[tool("read")],
        );
        assert!(request_projection(
            "fixture",
            &request,
            capabilities(ToolUpdateMode::AdditionOnly)
        )
        .is_err());
        let metered = context_projection(
            "fixture",
            &request,
            capabilities(ToolUpdateMode::AdditionOnly),
        )
        .unwrap();
        assert_eq!(metered["messages"][1]["content"][0]["type"], "tool_use");
        assert!(crate::model_runtime::estimate_provider_request_input_tokens(&metered) > 0);
    }

    fn accept(listener: &TcpListener) -> TcpStream {
        listener.set_nonblocking(true).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match listener.accept() {
                Ok((stream, _)) => {
                    stream.set_nonblocking(false).unwrap();
                    stream
                        .set_read_timeout(Some(Duration::from_secs(5)))
                        .unwrap();
                    return stream;
                }
                Err(e)
                    if e.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < deadline =>
                {
                    std::thread::sleep(Duration::from_millis(2))
                }
                Err(e) => panic!("mock accept: {e}"),
            }
        }
    }
    fn read_http(stream: &mut TcpStream) -> (String, Value) {
        let mut bytes = Vec::new();
        let mut byte = [0];
        while !bytes.ends_with(b"\r\n\r\n") {
            stream.read_exact(&mut byte).unwrap();
            bytes.push(byte[0]);
        }
        let head = String::from_utf8(bytes).unwrap();
        let len = head
            .lines()
            .find_map(|line| {
                line.split_once(':')
                    .filter(|(key, _)| key.eq_ignore_ascii_case("content-length"))
                    .map(|(_, v)| v.trim().parse::<usize>().unwrap())
            })
            .unwrap_or(0);
        let mut body = vec![0; len];
        stream.read_exact(&mut body).unwrap();
        (
            head,
            if body.is_empty() {
                Value::Null
            } else {
                serde_json::from_slice(&body).unwrap()
            },
        )
    }
    fn respond(stream: &mut TcpStream, value: &Value) {
        let body = value.to_string();
        write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
    }

    #[test]
    fn messages_adapter_discovers_once_and_replays_native_tool_results_with_beta() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let mut stream = accept(&listener);
            let (head, _) = read_http(&mut stream);
            assert!(head.starts_with("GET /models "));
            respond(&mut stream, &listing("deepseek-flash"));
            drop(stream);
            let mut stream = accept(&listener);
            let (head, first) = read_http(&mut stream);
            assert!(head.starts_with("POST /messages "));
            assert!(!head.to_ascii_lowercase().contains("anthropic-beta"));
            assert!(first.get("stream_options").is_none());
            respond(
                &mut stream,
                &json!({"type":"message","content":[{"type":"thinking","thinking":"plan","signature":"opaque"},{"type":"tool_use","id":"c1","name":"read","input":{"value":"x"}}],"stop_reason":"tool_use","usage":{"input_tokens":10,"cache_read_input_tokens":20,"cache_creation_input_tokens":0,"output_tokens":4}}),
            );
            drop(stream);
            let mut stream = accept(&listener);
            let (head, second) = read_http(&mut stream);
            assert!(head.starts_with("POST /messages "));
            assert!(head.contains(TOOL_CHANGES_BETA));
            assert_eq!(second["messages"][0], first["messages"][0]);
            assert_eq!(second["messages"][1]["content"][0]["signature"], "opaque");
            assert_eq!(second["messages"][2]["content"][0]["tool_use_id"], "c1");
            assert_eq!(second["messages"][3]["content"][0]["type"], "tool_addition");
            respond(
                &mut stream,
                &json!({"type":"message","content":[{"type":"text","text":"done"}],"stop_reason":"end_turn","usage":{"input_tokens":2,"cache_read_input_tokens":30,"cache_creation_input_tokens":0,"output_tokens":1}}),
            );
        });
        let adapter = NativeAdapter::from_config(ProviderConfig {
            mode: ProviderMode::Native,
            api_key: "fixture-key".into(),
            base_url: base,
            model: "deepseek-flash".into(),
        });
        assert_eq!(
            adapter.tool_update_mode(),
            Some(ToolUpdateMode::AdditionOnly)
        );
        assert_eq!(
            adapter.tool_update_mode(),
            Some(ToolUpdateMode::AdditionOnly)
        );
        let mut request = plan(
            &[Message::system("base"), Message::user("read x")],
            &[tool("read")],
        );
        request.output_token_limit = Some(128);
        let mut wire = None;
        let first = adapter
            .chat_observed(&request, &mut |delta| {
                if let crate::provider_stream::ModelDelta::Request(value) = delta {
                    wire = Some(value);
                }
            })
            .unwrap();
        assert_eq!(
            wire.as_deref(),
            adapter.context_request_projection(&request).as_ref()
        );
        assert_eq!(first.usage_total_tokens, Some(34));
        request.input.push(Message {
            provider_continuation: first.provider_continuation,
            role: Role::Assistant,
            content: first.text,
            tool_calls: first.tool_calls,
            tool_call_id: None,
        });
        request.input.push(Message {
            provider_continuation: None,
            role: Role::Tool,
            content: Some("ok".into()),
            tool_calls: vec![],
            tool_call_id: Some("c1".into()),
        });
        request.tools.push(tool("edit"));
        request.tool_history = Some(ToolHistoryProjection {
            declarations: vec![
                ToolDeclaration {
                    spec: tool("read"),
                    defer_loading: false,
                },
                ToolDeclaration {
                    spec: tool("edit"),
                    defer_loading: true,
                },
            ],
            updates: vec![ToolUpdate {
                message_index: request.input.len(),
                added: vec!["edit".into()],
                removed: vec![],
            }],
        });
        assert_eq!(
            adapter.chat(&request).unwrap().text.as_deref(),
            Some("done")
        );
        server.join().unwrap();
    }

    #[test]
    fn messages_unlisted_alias_keeps_chat_completions() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let mut stream = accept(&listener);
            read_http(&mut stream);
            respond(&mut stream, &listing("deepseek-flash"));
            drop(stream);
            let mut stream = accept(&listener);
            let (head, body) = read_http(&mut stream);
            assert!(head.starts_with("POST /chat/completions "));
            assert_eq!(body["model"], "deepseek-v4-flash");
            respond(
                &mut stream,
                &json!({"choices":[{"message":{"content":"ok"},"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":1,"total_tokens":11}}),
            );
        });
        let adapter = NativeAdapter::from_config(ProviderConfig {
            mode: ProviderMode::Native,
            api_key: "fixture-key".into(),
            base_url: base,
            model: "deepseek-v4-flash".into(),
        });
        assert_eq!(adapter.tool_update_mode(), None);
        let mut request = plan(&[Message::user("hi")], &[]);
        request.output_token_limit = Some(128);
        assert_eq!(adapter.chat(&request).unwrap().text.as_deref(), Some("ok"));
        server.join().unwrap();
    }

    /// Explicit paid smoke test: synthetic content, <=4 single HTTP attempts,
    /// <=256 output tokens each, no change to the configured model or endpoint.
    #[test]
    #[ignore = "uses the configured official DeepSeek credential; explicit live validation only"]
    fn messages_live_deferred_protocol_probe() {
        let config = ProviderConfig::from_env().unwrap();
        assert_eq!(
            url::Url::parse(&config.base_url).unwrap().host_str(),
            Some("api.deepseek.com")
        );
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let directory = root.join("output/deepseek-harness-mechanisms-20261025/protocol-probe");
        std::fs::create_dir_all(&directory).unwrap();
        let mut attempts = 0;
        let mut evidence = Vec::new();
        let mut send = |adapter: &NativeAdapter,
                        request: &AgentRequestPlan|
         -> Result<Value, String> {
            assert!(attempts < 4, "live probe HTTP budget exhausted");
            let (mut body, _) = request_projection(
                &adapter.model,
                request,
                capabilities(ToolUpdateMode::AdditionOnly),
            )
            .map_err(|e| e.message)?;
            body["thinking"] = json!({"type":"disabled"});
            body.as_object_mut().unwrap().remove("output_config");
            body["max_tokens"] = json!(256);
            adapter
                .normalize_provider_body(&mut body, true)
                .map_err(|e| e.message)?;
            attempts += 1;
            let url = endpoint_urls(&adapter.base_url).1;
            let mut deltas = Vec::new();
            let result = match adapter.send_provider_request_once(&url, &body, true) {
                Ok(response) => crate::deepseek_messages_stream::read_response(
                    response,
                    &Default::default(),
                    &mut |delta| deltas.push(delta),
                )
                .map_err(|e| e.message),
                Err(error) => Err(crate::adapter_http_error_message("probe", error)),
            };
            evidence.push(json!({"attempt":attempts,"endpoint":url,"request":body,"tool_changes_beta":uses_tool_updates(&body),"response":result.as_ref().ok(),"error":result.as_ref().err()}));
            std::fs::write(
                directory.join("results.json"),
                serde_json::to_vec_pretty(&evidence).unwrap(),
            )
            .unwrap();
            result
        };
        let mut adapter =
            NativeAdapter::from_config_with_timeout(config.clone(), Duration::from_secs(45));
        let mut request = plan(
            &[
                Message::system(
                    "You are testing an API tool protocol. Follow the named tool exactly.",
                ),
                Message::user("Call cache_probe_a once with value=first. Do not answer with text."),
            ],
            &[tool("cache_probe_a")],
        );
        request.output_token_limit = Some(256);
        let first = match send(&adapter, &request) {
            Ok(value) => value,
            Err(_) => {
                let mut canonical = config;
                canonical.model = "deepseek-flash".into();
                adapter =
                    NativeAdapter::from_config_with_timeout(canonical, Duration::from_secs(45));
                assert_eq!(
                    adapter.tool_update_mode(),
                    Some(ToolUpdateMode::AdditionOnly)
                );
                send(&adapter, &request).expect("canonical Messages initial tool call")
            }
        };
        let append_result = |request: &mut AgentRequestPlan,
                             value: &Value,
                             model: &str,
                             expected: &str| {
            assert_eq!(value["choices"][0]["finish_reason"], "tool_calls");
            let message = &value["choices"][0]["message"];
            let calls = message["tool_calls"].as_array().unwrap();
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0]["function"]["name"], expected);
            let id = calls[0]["id"].as_str().unwrap().to_owned();
            request.input.push(Message {
                role: Role::Assistant,
                content: message["content"].as_str().map(str::to_owned),
                tool_call_id: None,
                tool_calls: vec![ToolCall {
                    id: id.clone(),
                    name: expected.into(),
                    arguments: calls[0]["function"]["arguments"].as_str().unwrap().into(),
                }],
                provider_continuation: Some(ProviderContinuation {
                    model: model.into(),
                    reasoning_content: message["reasoning_content"].as_str().unwrap_or("").into(),
                    messages_content: message["messages_content"].as_array().unwrap().clone(),
                }),
            });
            request.input.push(Message {
                provider_continuation: None,
                role: Role::Tool,
                content: Some("{\"ok\":true}".into()),
                tool_calls: vec![],
                tool_call_id: Some(id),
            });
        };
        append_result(&mut request, &first, &adapter.model, "cache_probe_a");
        request.input.push(Message::user("Now call the newly added cache_probe_b once with value=second. Do not answer with text."));
        request.tools.push(tool("cache_probe_b"));
        request.tool_history = Some(ToolHistoryProjection {
            declarations: vec![
                ToolDeclaration {
                    spec: tool("cache_probe_a"),
                    defer_loading: false,
                },
                ToolDeclaration {
                    spec: tool("cache_probe_b"),
                    defer_loading: true,
                },
            ],
            updates: vec![ToolUpdate {
                message_index: request.input.len(),
                added: vec!["cache_probe_b".into()],
                removed: vec![],
            }],
        });
        let second = send(&adapter, &request).expect("deferred tool activation accepted");
        append_result(&mut request, &second, &adapter.model, "cache_probe_b");
        request.input.push(Message::user(
            "The test is complete. Reply only DONE and call no tools.",
        ));
        request.tool_choice = crate::ToolChoice::None;
        let final_response = send(&adapter, &request)
            .expect("retained deferred activation and tool replay accepted");
        assert_eq!(final_response["choices"][0]["finish_reason"], "stop");
        assert_eq!(
            final_response["choices"][0]["message"]["content"]
                .as_str()
                .unwrap()
                .trim(),
            "DONE"
        );
    }
}
