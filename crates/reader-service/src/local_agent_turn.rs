//! Bind an external process to an accepted Reader turn and its original book.
use crate::{
    acp_evidence::{AcpEvidence, READING_INSTRUCTIONS},
    acp_teaching,
    agent_run::{AppStatePort, PreparedAgentChat},
    local_acp, AppState, Reply, Req,
};
use read_tools::ToolError;
use runtime::{
    orchestrator::OuterOutcome,
    run_context::RunContext,
    run_events::{ActivityStatus, RunActivity},
    Message, Role,
};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

pub(crate) struct Selection {
    agent: local_acp::AgentConfig,
    profile: PathBuf,
    state: Arc<Mutex<AppState>>,
}

fn invalid(message: impl ToString) -> ToolError {
    crate::user_storage_paths::error(
        "LOCAL_AGENT_UNAVAILABLE",
        "validation",
        &message.to_string(),
    )
}

pub(crate) fn select(
    root: Option<&Path>,
    state: &Arc<Mutex<AppState>>,
    body: &str,
) -> Result<Option<Selection>, Reply> {
    let input = crate::body_value(body)?;
    let Some(id) = input.get("local_agent_id").filter(|v| !v.is_null()) else {
        return Ok(None);
    };
    let root = root.ok_or_else(|| crate::err_reply(&invalid("此入口未连接本机 Agent")))?;
    if input["scope"] == "personal" {
        return Err(crate::err_reply(&invalid(
            "请打开要讨论的材料，再使用本机 Agent",
        )));
    }
    let id = id
        .as_str()
        .ok_or_else(|| crate::err_reply(&invalid("请选择已配置的 Agent")))?;
    let agent = local_acp::resolve(root, id).map_err(|e| crate::err_reply(&invalid(e)))?;
    Ok(Some(Selection {
        agent,
        profile: root.to_owned(),
        state: state.clone(),
    }))
}

pub(crate) fn route(state: &Arc<Mutex<AppState>>, root: &Path, req: Req<'_>) -> Reply {
    if req.url.starts_with("/personal/") {
        return crate::personal_connection::route_host(state, req);
    }
    let result = match (req.method, req.url) {
        ("POST", "/local-agent/library") => {
            let input = match crate::body_value(req.body) {
                Ok(input) => input,
                Err(reply) => return reply,
            };
            let selected = input["selected_dir"]
                .as_str()
                .map(Path::new)
                .filter(|p| p.is_absolute());
            selected
                .ok_or_else(|| invalid("请选择本机书库的绝对路径"))
                .and_then(|selected| {
                    let mut state = state.lock().unwrap();
                    let library = desktop_support::library_settings::LibrarySettingsStore::new(
                        root.join("settings.json"),
                    )
                    .apply_selection(selected)
                    .map_err(invalid)?;
                    state.services.library_root = Some(library.clone());
                    Ok(json!({"library_root":library}))
                })
        }
        ("GET", "/local-agent/settings") => local_acp::settings(root).map_err(invalid),
        ("POST", "/local-agent/permission") => match crate::body_value(req.body) {
            Ok(input) => local_acp::permission_response(root, &input).map_err(invalid),
            Err(reply) => return reply,
        },
        ("POST", "/local-agent/settings") => match crate::body_value(req.body) {
            Ok(input) => local_acp::update(root, &input).map_err(invalid),
            Err(reply) => return reply,
        },
        ("POST", "/local-agent/extensions") => match crate::body_value(req.body) {
            Ok(input) => crate::learning_extensions::dispatch_local(
                state,
                root,
                input["name"].as_str().unwrap_or(""),
                &input["arguments"],
                req.now,
            ),
            Err(reply) => return reply,
        },
        _ => {
            return Reply {
                status: 404,
                body: "{}".into(),
            }
        }
    };
    match result {
        Ok(value) => crate::ok_json(&value),
        Err(error) => crate::err_reply(&error),
    }
}

impl Selection {
    pub(crate) fn identity(&self) -> crate::session_event::LocalAgentIdentity {
        crate::session_event::LocalAgentIdentity {
            id: self.agent.id.clone(),
            name: self.agent.name.clone(),
        }
    }

    fn skill_scope(prepared: &PreparedAgentChat) -> String {
        prepared
            .scope
            .learning_workspace_id
            .as_ref()
            .map(|id| format!("workspace:{id}"))
            .unwrap_or_else(|| format!("book:{}", prepared.scope.book_id()))
    }
    fn prompt(
        &self,
        port: &impl AppStatePort,
        prepared: &PreparedAgentChat,
        closed: bool,
    ) -> Result<String, ToolError> {
        let scope = &prepared.scope;
        let mut parts = vec![READING_INSTRUCTIONS.to_owned()];
        let book_id = scope.book_id();
        let viewport = scope
            .reader_input
            .as_ref()
            .map(|input| &input.state.viewport);
        parts.push(format!(
            "Reader material: {book_id}. Current viewport: {}",
            json!(viewport)
        ));
        if let Some(language) = prepared.response_language {
            parts.push(format!("Respond using language {}.", json!(language)));
        }
        let skill_scope = Self::skill_scope(prepared);
        let skill = crate::learning_extensions::skill_context(&self.profile, &skill_scope)
            .map_err(invalid)?;
        if !skill.is_empty() {
            parts.push(skill);
        }
        if !closed {
            let history = port.with_app(|state| {
                scope.check_owner(state)?;
                let chat = state.user.agent_history.sessions.iter().find(|s| s.id == scope.chat_session_id)
                    .ok_or_else(|| invalid("聊天记录已不可用"))?;
                let mut prior = chat.turns.iter().filter(|turn| turn.user_turn_ordinal < prepared.turn_ref.user_turn_ordinal)
                    .rev().take(6).map(|turn| json!({
                        "question":turn.user.chars().take(1200).collect::<String>(),
                        "answer":turn.outcome.as_ref().and_then(|out| out.answer.as_ref()).map(|t|t.chars().take(1800).collect::<String>())
                    })).collect::<Vec<_>>();
                prior.reverse();
                Ok::<_, ToolError>(prior)
            })?;
            if !history.is_empty() {
                parts.push(format!(
                    "Earlier conversation in this material, for context:\n{}",
                    json!(history)
                ));
            }
        }
        parts.push(acp_teaching::prompt(port, prepared)?);
        parts.push(prepared.agent_message.clone());
        Ok(parts.join("\n\n"))
    }

    pub(crate) fn run(
        &self,
        port: &impl AppStatePort,
        prepared: &PreparedAgentChat,
        context: &mut RunContext,
    ) -> Result<OuterOutcome, ToolError> {
        let book = prepared
            .scope
            .book
            .as_ref()
            .ok_or_else(|| invalid("请先打开材料"))?;
        let closed = acp_teaching::closed_book(prepared);
        let prompt = self.prompt(port, prepared, closed)?;
        let seed = crate::verified_question_evidence(book, prepared.scope.question_quote.as_ref());
        let evidence = AcpEvidence::start(book.clone(), seed, &prepared.message)?;
        if !closed {
            evidence.enable_actions(
                self.state.clone(),
                prepared.scope.clone(),
                prepared.now.clone(),
                context.cancellation.clone(),
            );
        }
        let mcp = evidence.mcp_server()?;
        let private_dir = if closed {
            Some(tempfile::tempdir_in(&self.profile).map_err(invalid)?)
        } else {
            None
        };
        let cwd = private_dir
            .as_ref()
            .map(|dir| dir.path().to_owned())
            .unwrap_or_else(|| {
                crate::learning_extensions::skill_workspace(
                    &self.profile,
                    &Self::skill_scope(prepared),
                )
            });
        std::fs::create_dir_all(&cwd).map_err(invalid)?;
        context
            .messages
            .push(Message::user(prepared.agent_message.clone()));
        if !closed {
            evidence.start_preview(context.events.clone());
        }
        let mut activity = context
            .events
            .begin("model", &self.agent.id, &self.agent.name, true);
        let mut tools: HashMap<String, RunActivity> = HashMap::new();
        let mut reported_tokens = None;
        let response = local_acp::run_interactive(
            &self.agent,
            &cwd,
            &[mcp],
            &prompt,
            &context.cancellation,
            |update| {
                if update["sessionUpdate"] == "reader_prompt_usage" {
                    let count = |field: &str| {
                        update["usage"][field]
                            .as_u64()
                            .and_then(|n| u32::try_from(n).ok())
                    };
                    reported_tokens = count("totalTokens").filter(|n| *n > 0);
                    if reported_tokens.is_some() {
                        activity.usage = Some(runtime::provider_stream::ModelUsage {
                            input_tokens: count("inputTokens"),
                            output_tokens: count("outputTokens"),
                            reasoning_output_tokens: count("thoughtTokens"),
                            cached_input_tokens: count("cachedReadTokens"),
                            cache_creation_input_tokens: count("cachedWriteTokens"),
                            total_tokens: reported_tokens,
                            ..Default::default()
                        });
                    }
                    return;
                }
                if closed {
                    return;
                }
                let id = update["toolCallId"].as_str().unwrap_or_default();
                match update["sessionUpdate"].as_str() {
                    Some("agent_message_chunk") => {
                        if let Some(text) = update.pointer("/content/text").and_then(Value::as_str)
                        {
                            evidence.push_text(text);
                        }
                    }
                    Some("tool_call" | "tool_call_update") if !id.is_empty() => {
                        tools.entry(id.to_owned()).or_insert_with(|| {
                            context.events.begin(
                                "tool",
                                "acp.tool",
                                update["title"].as_str().unwrap_or("Agent tool"),
                                true,
                            )
                        });
                        let status = match update["status"].as_str() {
                            Some("completed") => Some(ActivityStatus::Succeeded),
                            Some("failed") => Some(ActivityStatus::Failed),
                            _ => None,
                        };
                        if let Some(status) = status {
                            if let Some(step) = tools.remove(id) {
                                context.events.finish(step, status, None, None, None);
                            }
                        }
                    }
                    _ => {}
                }
            },
            &self.profile,
            if closed { "" } else { &prepared.scope.turn_id },
            &context.events,
        );
        context.effects.extend(evidence.action_effects());
        let result = response.map_err(invalid).and_then(|raw| {
            context.cancellation.check()?;
            let accepted = acp_teaching::answer(port, prepared, &raw)?;
            evidence.finish(&accepted)
        });
        let status = if context.cancellation.is_cancelled() {
            ActivityStatus::Cancelled
        } else if result.is_err() {
            ActivityStatus::Failed
        } else {
            ActivityStatus::Succeeded
        };
        for step in tools.into_values() {
            context.events.finish(
                step,
                if status == ActivityStatus::Succeeded {
                    ActivityStatus::NoResult
                } else {
                    status
                },
                None,
                None,
                None,
            );
        }
        context.events.finish(
            activity,
            status,
            reported_tokens,
            result.as_ref().err().map(|e| e.error_code.clone()),
            None,
        );
        let mut outcome = result?;
        if let Some(tokens) = reported_tokens {
            outcome.tokens_spent = tokens;
        }
        context.messages.push(Message {
            role: Role::Assistant,
            content: outcome.answer.clone(),
            tool_calls: Vec::new(),
            tool_call_id: None,
            provider_continuation: None,
        });
        Ok(outcome)
    }
}
