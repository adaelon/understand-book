//! Resolve a frozen explanation, verify both private stages and deliver the comparison.
use crate::agent_run::{PreparedAgentChat, UserStatePort};
use read_tools::{Book, ToolError};
use runtime::orchestrator::{AgentAnswerPart, AgentAnswerView, OuterOutcome};
use runtime::run_context::RunContext;
use runtime::run_events::ActivityStatus;
use runtime::teach_back::{
    self, TeachBackComparison, TeachBackContinuation, TeachBackReply, TeachBackResult,
    TeachBackRoundContext, TeachBackStage,
};
use runtime::tutor_continuation::ResolvedInteraction;
use runtime::{Message, ModelAdapter, Role};
use serde_json::Value;

#[cfg(test)]
#[path = "tests/teach_back_run_tests.rs"]
mod tests;

pub(crate) fn requested(prepared: &PreparedAgentChat) -> bool {
    prepared
        .tutor
        .as_ref()
        .is_some_and(|tutor| tutor["current_action"]["action"]["entry_action"] == "explain_back")
}

fn anchors(book: &Book, interaction: &ResolvedInteraction) -> Result<Vec<String>, ToolError> {
    let bindings = &interaction.source_bindings;
    if bindings.iter().any(|source| {
        source.source_id != book.base.book_id || source.source_revision != book.source_fingerprint()
    }) {
        return Err(crate::user_storage_paths::error(
            "INTERACTION_STALE",
            "conflict",
            "原讲解的来源版本已变化，请重新打开讲解。",
        ));
    }
    // Prefer pinpoint citations. Reading the endpoints of a whole-section citation as
    // additional anchors would bury the graph neighbors of its cited core paragraph.
    let mut lids: Vec<String> = Vec::new();
    for source in bindings.iter().filter(|s| s.start_lid == s.end_lid) {
        if !lids.contains(&source.start_lid) {
            lids.push(source.start_lid.clone());
        }
    }
    let points: Vec<_> = lids
        .iter()
        .filter_map(|lid| book.source_span(lid))
        .collect();
    for source in bindings.iter().filter(|s| s.start_lid != s.end_lid) {
        let covers_point = book
            .source_span(&source.start_lid)
            .zip(book.source_span(&source.end_lid))
            .is_some_and(|(start, end)| {
                points
                    .iter()
                    .any(|p| start.start <= p.start && p.end <= end.end)
            });
        if !covers_point {
            for lid in [&source.start_lid, &source.end_lid] {
                if !lids.contains(lid) {
                    lids.push(lid.clone());
                }
            }
        }
    }
    Ok(lids)
}

struct EarlierRound {
    card: TeachBackComparison,
    first_round_id: String,
    first_turn_id: String,
    first_expression: String,
}

/// Read immutable facts by their admitted IDs. Switching rows or a later display
/// receipt cannot change what was available when this turn began.
fn read_accepted_round(
    user: &crate::user_runtime::UserRuntime,
    prepared: &PreparedAgentChat,
    reply: Option<&TeachBackReply>,
) -> Result<(Vec<String>, Option<EarlierRound>), ToolError> {
    let store =
        user.learning_store_for_workspace(prepared.scope.learning_workspace_id.as_deref())?;
    let binding = store
        .teaching_event(&format!("binding:{}", prepared.turn_ref.turn_id))?
        .binding;
    let input = store.teaching_event(&format!(
        "usage:{}:user:{}",
        binding.tutor_session_id, binding.turn_id
    ))?;
    let support =
        serde_json::from_value(input.payload["support_delivery_refs"].clone()).map_err(|e| {
            crate::agent_history_internal(format!("invalid frozen teach-back support: {e}"))
        })?;
    let Some(reply) = reply else {
        return Ok((support, None));
    };
    let parent = store.teaching_event(&reply.round_id)?;
    let card = serde_json::from_value(parent.payload["comparison"].clone())
        .map_err(|e| crate::agent_history_internal(format!("invalid delivered comparison: {e}")))?;
    let first_round_id = input.payload["first_round_id"]
        .as_str()
        .ok_or_else(|| crate::agent_history_internal("missing first teach-back round"))?;
    let first = store.evidence(&format!("interpretation:{first_round_id}"))?;
    let expression = store.teaching_event(&first.action_ref)?;
    Ok((
        support,
        Some(EarlierRound {
            card,
            first_round_id: first_round_id.to_owned(),
            first_turn_id: expression.binding.turn_id,
            first_expression: first.learner_quote,
        }),
    ))
}

/// A help-only submission returns no card. Its classification calls still count
/// towards the final ordinary answer's model usage.
pub(crate) fn execute(
    prepared: &PreparedAgentChat,
    port: &impl UserStatePort,
    adapter: &dyn ModelAdapter,
    context: &mut RunContext,
) -> Result<(Option<OuterOutcome>, usize, u32), ToolError> {
    execute_with(prepared, port, context, |stage, instructions, packet| {
        teach_back::sample_model(adapter, stage, instructions, packet)
            .map_err(runtime::AdapterError::into_tool_error)
    })
}

pub(crate) fn execute_with(
    prepared: &PreparedAgentChat,
    port: &impl UserStatePort,
    context: &mut RunContext,
    mut sample: impl FnMut(TeachBackStage, &str, &Value) -> Result<(String, u32), ToolError>,
) -> Result<(Option<OuterOutcome>, usize, u32), ToolError> {
    port.with_user(|u| prepared.scope.check_user(u))?;
    let interaction: ResolvedInteraction = serde_json::from_value(
        prepared.tutor.as_ref().expect("frozen explanation")["current_action"].clone(),
    )
    .map_err(|e| crate::agent_history_internal(format!("invalid accepted explanation: {e}")))?;
    let book = prepared.scope.reading_book();
    let anchors = anchors(book, &interaction)?;
    let expression = &interaction.user_supplement;
    let question = interaction.accepted_request.trim();
    let course_task = interaction.course.as_ref().map(|_| {
        let course = &prepared.tutor.as_ref().expect("frozen explanation")["inquiry_course"];
        let lesson = &course["lesson"];
        serde_json::json!({
            "introduction": lesson["introduction"], "concepts": lesson["concepts"],
            "displayed_explanation": lesson["explanation"], "saved_exploration": lesson["exploration"],
            "current_exploration": course["current_exploration"], "math_exploration": lesson["math_exploration"]
        })
    });
    let language = context
        .response_language
        .map(|language| language.fragment().content);
    let reply = interaction.teach_back_reply.as_ref();
    let (support, prior) = port.with_user(|user| read_accepted_round(user, prepared, reply))?;
    let continuation = reply
        .zip(prior.as_ref())
        .map(|(reply, prior)| TeachBackContinuation {
            kind: reply.kind,
            previous: &prior.card,
            row_id: &reply.row_id,
            first_expression: &prior.first_expression,
            first_turn_id: &prior.first_turn_id,
        });
    let mut active = None;
    let mut calls = 0;
    let mut tokens = 0u32;
    let result = teach_back::run(
        book,
        &anchors,
        (!question.is_empty()).then_some(question),
        course_task.as_ref(),
        &prepared.turn_ref.turn_id,
        expression,
        continuation,
        |stage| {
            if let Some(previous) = active.take() {
                context
                    .events
                    .finish(previous, ActivityStatus::Succeeded, None, None, None);
            }
            let (name, label) = match stage {
                TeachBackStage::OrganizeExpression => ("organize_expression", "正在整理你的表达"),
                TeachBackStage::ComparePassages => ("compare_passages", "正在核对书中原文与对照"),
            };
            active = Some(context.events.begin("teach_back", name, label, true));
        },
        |stage, instructions, packet: &Value| {
            context.cancellation.check()?;
            port.check_access()?;
            let localized;
            let instructions = if let Some(language) = &language {
                localized = format!("{instructions}\n\n{language}");
                &localized
            } else {
                instructions
            };
            let (text, usage) = sample(stage, instructions, packet)?;
            calls += 1;
            tokens = tokens.saturating_add(usage);
            context.cancellation.check()?;
            port.check_access()?;
            Ok(text)
        },
    );
    if let Some(activity) = active {
        let status = if context.cancellation.is_cancelled() {
            ActivityStatus::Cancelled
        } else if result.is_err() {
            ActivityStatus::Failed
        } else {
            ActivityStatus::Succeeded
        };
        context.events.finish(
            activity,
            status,
            None,
            result.as_ref().err().map(|e| e.error_code.clone()),
            None,
        );
    }
    // Keep just the visible comparison in conversational memory. Private requests,
    // raw proposals and diagnostics never become assistant messages.
    let (part, text) = match result? {
        TeachBackResult::OrdinaryChat => return Ok((None, calls, tokens)),
        TeachBackResult::NeedsRevision => {
            let text = match context.response_language {
                Some(runtime::response_language::ResponseLanguage::English) =>
                    "Your first explanation did not include that reason. Choose “Revise my explanation” on the comparison to save it as a new explanation.",
                _ => "这条理由在首次原话里找不到。请在对照中选择「改一下我的想法」，把它记作一次新表达。",
            }.to_owned();
            (AgentAnswerPart::Markdown { text: text.clone() }, text)
        }
        TeachBackResult::Comparison(mut comparison) => {
            comparison.support_delivery_refs = Some(support);
            comparison.continuation =
                reply
                    .zip(prior)
                    .map(|(reply, prior)| TeachBackRoundContext {
                        kind: reply.kind,
                        parent_round_id: reply.round_id.clone(),
                        first_round_id: prior.first_round_id,
                        row_id: reply.row_id.clone(),
                    });
            let text = comparison
                .rows
                .iter()
                .map(|row| row.summary.as_str())
                .chain(
                    comparison
                        .follow_up
                        .iter()
                        .map(|follow| follow.question.as_str()),
                )
                .collect::<Vec<_>>()
                .join("\n\n");
            (AgentAnswerPart::TeachBackComparison { comparison }, text)
        }
    };
    context.messages.push(Message::user(expression));
    context.messages.push(Message {
        role: Role::Assistant,
        content: Some(text.clone()),
        tool_calls: vec![],
        tool_call_id: None,
        provider_continuation: None,
    });
    let outcome = OuterOutcome {
        answer: Some(text),
        answer_view: Some(AgentAnswerView {
            parts: vec![part],
            sources: vec![],
        }),
        incomplete: false,
        warning: None,
        turns: calls,
        tokens_spent: tokens,
        effects: vec![],
        trace: vec![],
        profile_usage: Default::default(),
        memory_updates: vec![],
        teaching_delivery_move_refs: vec![],
        source_bindings: vec![],
        delivery_diagnostics: None,
        request_audit: Default::default(),
    };
    Ok((Some(outcome), calls, tokens))
}
