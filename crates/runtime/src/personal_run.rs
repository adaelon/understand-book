//! Material-free Resident execution. Shares the model, Goal, tool contracts,
//! compaction, cancellation and persistence ports with reading turns.
use crate::auto_compaction::{ActiveContextBudget, CompactionCheckpointSink};
use crate::compaction::{CompactionCheckpoint, CompactionLimits, CompactionPhase};
use crate::orchestrator::{OuterOutcome, TraceStep};
use crate::run_context::{ResidentStatePort, RunContext};
use crate::{AgentRequestPlan, Message, ModelAdapter, Role};
use read_tools::ToolError;
use serde_json::{json, Value};

fn failure(code: &str, message: impl Into<String>) -> ToolError {
    ToolError {
        error_code: code.into(),
        category: "validation".into(),
        message: message.into(),
    }
}
pub fn run(
    state: &mut impl ResidentStatePort,
    adapter: &dyn ModelAdapter,
    context: &mut RunContext,
    question: &str,
    saved_checkpoint: Option<CompactionCheckpoint>,
    sink: &mut dyn CompactionCheckpointSink,
) -> Result<OuterOutcome, ToolError> {
    let notes = crate::personal_notes::context(state, &mut context.personal_note_reads, question)?;
    let fragment=Message::system(format!("This is a personal learning conversation with no Book or Reader scene. Respond to the user's actual thoughts; compare conditions, offer examples or questions when helpful. No material, LID, source evidence or mastery assessment is available merely because a note names it. Use the same Resident Goal and current request. Do not require a learning goal or a formal test. For a requested rewrite or synthesis, return a clearly identified draft for the user to adopt; never change their notes automatically.\n{}\n{}\npersonal_notes.v1\n{}",crate::personal_notes::POLICY,crate::goal::PLANNED_WORK_GUIDANCE,notes));
    let registry = crate::orchestrator::resident_tool_registry();
    let tools: Vec<_> = registry
        .registrations()
        .iter()
        .filter(|r| {
            matches!(
                r.spec.name.as_str(),
                "notes.read" | "notes.search" | "goal.update"
            ) || (state.web_available()
                && matches!(r.spec.name.as_str(), "web.search" | "web.fetch"))
        })
        .map(|r| r.spec.clone())
        .collect();
    context.messages.push(Message::user(question));
    let mut checkpoint = saved_checkpoint;
    let mut turns = 0;
    let mut spent = 0;
    let mut repeated = std::collections::BTreeMap::new();
    loop {
        context.cancellation.check()?;
        if context.config.turn_limit_reached(turns) {
            return Err(failure("TURN_LIMIT_EXCEEDED", "当前运行到达调用上限"));
        }
        let mut contexts = vec![
            fragment.clone(),
            Message::system(format!("Current Resident Goal: {}", json!(context.goal))),
        ];
        if let Some(language) = context.response_language {
            contexts.push(Message::system(language.fragment().content));
        }
        let project =
            |checkpoint: Option<&CompactionCheckpoint>| -> Result<Vec<Message>, ToolError> {
                if let Some(checkpoint) = checkpoint {
                    crate::compaction::project_compaction_checkpoint_messages(
                        &context.messages,
                        checkpoint,
                        &contexts,
                        crate::compaction::COMPACTION_CONSUMPTION_WRAPPER,
                    )
                    .map_err(|e| failure("COMPACTION_FAILED", format!("{e:?}")))
                } else {
                    let mut messages = context.messages.clone();
                    let at = usize::from(messages.first().is_some_and(|m| m.role == Role::System));
                    messages.splice(at..at, contexts.clone());
                    Ok(messages)
                }
            };
        let mut plan = AgentRequestPlan::for_agent_turn(
            context.runtime_profile.clone(),
            &project(checkpoint.as_ref())?,
            &tools,
        );
        let budget = ActiveContextBudget::from_plan(&plan);
        if budget.over_high_watermark {
            let prepared = crate::compaction::prepare_compaction(
                CompactionPhase::MidTurn,
                &context.messages,
                &context.messages,
                vec![],
                vec![],
                vec![],
                Default::default(),
            )
            .map_err(|e| failure("COMPACTION_FAILED", format!("{e:?}")))?;
            let candidate = crate::compaction::compact_with_adapter(
                adapter,
                &context.runtime_profile,
                &prepared,
                CompactionLimits {
                    target_active_tokens: budget.target_input_tokens,
                    ..Default::default()
                },
            )
            .map_err(|e| failure("COMPACTION_FAILED", format!("{e:?}")))?;
            plan = AgentRequestPlan::for_agent_turn(
                context.runtime_profile.clone(),
                &project(Some(&candidate))?,
                &tools,
            );
            if !plan.active_context.fits {
                return Err(failure(
                    "ACTIVE_CONTEXT_EXHAUSTED",
                    "上下文暂时无法缩减到可用范围",
                ));
            }
            sink.install(&candidate, &context.messages)
                .map_err(|e| failure("COMPACTION_FAILED", format!("{e:?}")))?;
            checkpoint = Some(candidate);
        }
        context
            .tool_declaration_history
            .project(&mut plan, adapter.tool_update_mode());
        sink.persist_progress(&context.messages, &context.events.activities())?;
        turns += 1;
        let _purpose = crate::run_events::purpose(adapter, "outer");
        let turn = adapter.chat(&plan).map_err(|e| e.into_tool_error())?;
        context.sampled_model_projection =
            crate::active_projection::ActiveModelProjection::capture_request(
                &plan,
                context.messages.len(),
                adapter.projection_identity().as_deref(),
                checkpoint.as_ref(),
                &context.tool_declaration_history,
            );
        context.cancellation.check()?;
        spent += turn.usage_total_tokens.unwrap_or(0);
        context.messages.push(Message {
            role: Role::Assistant,
            content: turn.text.clone(),
            tool_calls: turn.tool_calls.clone(),
            tool_call_id: None,
            provider_continuation: turn.provider_continuation,
        });
        if turn.tool_calls.is_empty() {
            let answer = turn
                .text
                .filter(|s| !s.trim().is_empty())
                .ok_or_else(|| failure("ANSWER_EMPTY", "本次讨论没有生成回答"))?;
            return Ok(OuterOutcome {
                answer: Some(answer.clone()),
                answer_view: Some(crate::orchestrator::AgentAnswerView {
                    parts: vec![crate::orchestrator::AgentAnswerPart::Markdown { text: answer }],
                    sources: vec![],
                }),
                incomplete: false,
                warning: None,
                turns,
                tokens_spent: spent,
                effects: vec![],
                trace: context.trace.clone(),
                profile_usage: Default::default(),
                memory_updates: vec![],
                teaching_delivery_move_refs: vec![],
                source_bindings: vec![],
                delivery_diagnostics: None,
                request_audit: Default::default(),
            });
        }
        for call in turn.tool_calls {
            context.cancellation.check()?;
            let count = repeated
                .entry((call.name.clone(), call.arguments.clone()))
                .or_insert(0);
            *count += 1;
            if *count > 3 {
                return Err(failure(
                    "AGENT_NO_PROGRESS",
                    "相同读取反复出现且没有进展，请换一种提问继续",
                ));
            }
            let activity = context.events.begin(
                "tool",
                &call.name,
                registry
                    .registration(&call.name)
                    .map(|r| r.activity_label())
                    .unwrap_or("工具"),
                true,
            );
            let result = (|| -> Result<Value, ToolError> {
                if *count > 3 {
                    return Err(failure(
                        "AGENT_NO_PROGRESS",
                        "相同读取已完成，请使用已有结果或改变查询",
                    ));
                }
                if !tools.iter().any(|tool| tool.name == call.name) {
                    return Err(failure("TOOL_NOT_EXPOSED", "当前讨论没有此材料能力"));
                }
                let input: Value = serde_json::from_str(&call.arguments)
                    .map_err(|e| failure("TOOL_ARGUMENTS_INVALID", e.to_string()))?;
                match call.name.as_str() {
                    "notes.search" => state.personal_note_search(input),
                    "notes.read" => {
                        state.submit_private(|store| store.read_personal_note_page(&input))?
                    }
                    "goal.update" => {
                        let mut goal = context
                            .goal
                            .clone()
                            .ok_or_else(|| failure("GOAL_UNAVAILABLE", "运行目标不可用"))?;
                        let id = goal.user_message_refs.last().cloned().unwrap_or_default();
                        let update = serde_json::from_value(input)
                            .map_err(|e| failure("GOAL_UPDATE_INVALID", e.to_string()))?;
                        goal.apply_update(update, &id, question)
                            .map_err(|e| failure("GOAL_UPDATE_INVALID", e))?;
                        state.persist_goal(&goal)?;
                        context.goal = Some(goal.clone());
                        Ok(json!(goal))
                    }
                    name => state.web_call(name, input, &context.cancellation),
                }
            })();
            let body = result.unwrap_or_else(|e| json!(e)).to_string();
            let (status, error, count) = crate::run_events::tool_result(&body);
            context.events.finish(activity, status, None, error, count);
            context.trace.push(TraceStep {
                model_tool_loop: Some(turns),
                tool: call.name,
                args: call.arguments,
                result_digest: body.chars().take(600).collect(),
                query_audit: None,
            });
            context.messages.push(Message {
                role: Role::Tool,
                content: Some(body),
                tool_calls: vec![],
                tool_call_id: Some(call.id),
                provider_continuation: None,
            });
        }
    }
}
