//! Opt-in offline replay of a locally saved provider request. Never calls a model.
use super::*;
use crate::{ProviderContinuation, ToolCall};

#[test]
#[ignore = "requires a private saved request via UB_CONTEXT_CACHE_REQUEST"]
fn messages_saved_request_cache_measurement() {
    let path = std::env::var("UB_CONTEXT_CACHE_REQUEST").unwrap();
    let before: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let model = before["model"].as_str().unwrap();
    let mut used = HashSet::new();
    let names: BTreeMap<_, _> = crate::tool_registry::ToolHandlerId::ALL
        .iter()
        .enumerate()
        .map(|(i, h)| {
            (
                crate::provider_tool_name(h.canonical_name(), i, &mut used),
                h.canonical_name().to_owned(),
            )
        })
        .collect();
    let canonical = |name: &str| names.get(name).cloned().unwrap_or_else(|| name.into());
    let mut declarations = Vec::new();
    for tool in before["tools"].as_array().unwrap() {
        declarations.push(ToolDeclaration {
            spec: ToolSpec {
                name: canonical(tool["name"].as_str().unwrap()),
                description: tool["description"].as_str().unwrap().into(),
                parameters: tool["input_schema"].clone(),
            },
            defer_loading: tool["defer_loading"] == true,
        });
    }
    let mut input = vec![Message::system(before["system"].as_str().unwrap())];
    let mut effective = before["system"].as_str().unwrap().to_owned();
    let mut updates = Vec::new();
    let mut snapshot_bytes = 0;
    let mut delta_bytes = 0;
    for message in before["messages"].as_array().unwrap() {
        let blocks = message["content"].as_array().unwrap();
        match message["role"].as_str().unwrap() {
            "system" => {
                for block in blocks {
                    match block["type"].as_str().unwrap() {
                        "text" => {
                            let snapshot = block["text"].as_str().unwrap();
                            let delta = snapshot
                                .strip_prefix(&effective)
                                .expect("recorded snapshots must be exact cumulative prefixes")
                                .strip_prefix("\n\n")
                                .unwrap();
                            let context = Message::system(delta);
                            assert!(
                                is_runtime_context(&context),
                                "only host context may be moved out of cumulative system updates"
                            );
                            snapshot_bytes += snapshot.len();
                            delta_bytes += delta.len();
                            input.push(context);
                            effective = snapshot.to_owned();
                        }
                        kind @ ("tool_addition" | "tool_removal") => {
                            let name = canonical(block["tool"]["name"].as_str().unwrap());
                            updates.push(ToolUpdate {
                                message_index: input.len() - 1,
                                added: if kind == "tool_addition" {
                                    vec![name.clone()]
                                } else {
                                    vec![]
                                },
                                removed: if kind == "tool_removal" {
                                    vec![name]
                                } else {
                                    vec![]
                                },
                            });
                        }
                        other => panic!("unexpected system block: {other}"),
                    }
                }
            }
            "user" => {
                for block in blocks {
                    match block["type"].as_str().unwrap() {
                        "text" => input.push(Message::user(block["text"].as_str().unwrap())),
                        "tool_result" => input.push(Message {
                            role: Role::Tool,
                            content: Some(block["content"].as_str().unwrap().into()),
                            tool_call_id: Some(block["tool_use_id"].as_str().unwrap().into()),
                            tool_calls: vec![],
                            provider_continuation: None,
                        }),
                        other => panic!("unexpected user block: {other}"),
                    }
                }
            }
            "assistant" => {
                let content = blocks
                    .iter()
                    .filter_map(|b| (b["type"] == "text").then(|| b["text"].as_str().unwrap()))
                    .collect::<String>();
                let reasoning = blocks
                    .iter()
                    .filter_map(|b| {
                        (b["type"] == "thinking").then(|| b["thinking"].as_str().unwrap())
                    })
                    .collect::<String>();
                let tool_calls = blocks
                    .iter()
                    .filter(|b| b["type"] == "tool_use")
                    .map(|b| ToolCall {
                        id: b["id"].as_str().unwrap().into(),
                        name: canonical(b["name"].as_str().unwrap()),
                        arguments: b["input"].to_string(),
                    })
                    .collect();
                input.push(Message {
                    role: Role::Assistant,
                    content: (!content.is_empty()).then_some(content),
                    tool_calls,
                    tool_call_id: None,
                    provider_continuation: Some(ProviderContinuation {
                        model: model.into(),
                        reasoning_content: reasoning,
                        messages_content: blocks.clone(),
                    }),
                });
            }
            other => panic!("unexpected role: {other}"),
        }
    }
    let mut request = AgentRequestPlan::for_ad_hoc(
        crate::ModelRuntimeCatalog::default().resolve(
            "fixture",
            crate::ProviderToolProtocol::Native,
            None,
        ),
        &input,
        &declarations
            .iter()
            .map(|d| d.spec.clone())
            .collect::<Vec<_>>(),
    );
    request.output_token_limit = before["max_tokens"].as_u64().map(|n| n as u32);
    request.parallel_tool_calls = before["tool_choice"]["disable_parallel_tool_use"] != true;
    request.tool_history = Some(ToolHistoryProjection {
        declarations,
        updates,
    });
    let (mut after, _) = request_projection(crate::AgentReasoningEffort::default(),
        model,
        &request,
        MessagesCapabilities {
            tool_update: ToolUpdateMode::AdditionOnly,
            system_in_history: true,
        },
    )
    .unwrap();
    // Keep the original model/sampling settings for a controlled live replay.
    for (key, value) in before.as_object().unwrap() {
        if !matches!(key.as_str(), "system" | "messages" | "tools") {
            after[key] = value.clone();
        }
    }
    let non_context = |body: &Value| -> Vec<Value> {
        body["messages"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|m| {
                m["content"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter_map(move |b| {
                        if m["role"] == "system" && b["type"] == "text" {
                            return None;
                        }
                        if b["type"] == "text"
                            && is_runtime_context(&Message::system(
                                b["text"].as_str().unwrap_or_default(),
                            ))
                        {
                            return None;
                        }
                        Some(json!({"role":m["role"],"block":b}))
                    })
            })
            .collect()
    };
    assert!(
        non_context(&before) == non_context(&after),
        "tool calls, results, reasoning and conversation blocks must remain exact"
    );
    assert!(
        before["tools"] == after["tools"],
        "tool declarations must remain exact"
    );
    let before_bytes = serde_json::to_vec(&before).unwrap().len();
    let after_bytes = serde_json::to_vec(&after).unwrap().len();
    assert!(after_bytes < before_bytes);
    let report = json!({"full_request_bytes_before":before_bytes,"full_request_bytes_after":after_bytes,
        "cumulative_snapshot_bytes_before":snapshot_bytes,"runtime_delta_bytes_after":delta_bytes,
        "non_context_blocks_exact":true,"tool_declarations_exact":true,
        "measurement":"offline serialization only; no token/currency or output-quality claim"});
    if let Ok(path) = std::env::var("UB_CONTEXT_CACHE_MEASUREMENT") {
        std::fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    }
    if let Ok(path) = std::env::var("UB_CONTEXT_CACHE_REPROJECTED") {
        std::fs::write(path, serde_json::to_vec(&after).unwrap()).unwrap();
    }
    println!("{report}");
}
