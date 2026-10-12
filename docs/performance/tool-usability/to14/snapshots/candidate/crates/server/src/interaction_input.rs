//! Shared input contract and source resolution before either host accepts a run.
use crate::*;
use runtime::tutor_continuation::{AnswerRef, EntryAction, TeachingChoice, InteractionAction, InteractionResolution, ResolvedInteraction};
use memory::teaching::TeachingSourceBinding;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum InteractionInput {
    Typed,
    Selection,
    Answer { action: EntryAction, reference: AnswerRef },
    Choice(TeachingChoice),
}

pub(crate) fn invalid(message: &str) -> ToolError {
    user_storage_paths::error("INTERACTION_INVALID", "validation", message)
}
pub(crate) fn unavailable() -> ToolError {
    user_storage_paths::error("INTERACTION_UNAVAILABLE", "unavailable", "此阅读操作暂不可用，请继续使用输入框提问")
}

fn local_normalized(input: &Value) -> Result<Value, ToolError> {
    run_admission::normalize("local", input)
}

/// Read the original accepted identity before inspecting the current head or
/// source. A pending/failed operation is returned, never executed a second time.
pub(crate) fn local_repeat(state: &AppState, body: &str) -> Result<Option<Reply>, Reply> {
    let input = body_value(body)?;
    InteractionInput::parse(&input).map_err(|e|err_reply(&e))?;
    let Some(key) = input["client_request_id"].as_str() else { return Ok(None); };
    let normalized = local_normalized(&input).map_err(|e|err_reply(&e))?;
    for session in &state.user.agent_history.sessions {
        for turn in &session.turns {
            if let Some(old) = turn.domain.local_action.as_ref().filter(|a|a.client_request_id == key) {
                if old.normalized != normalized {
                    return Err(err_reply(&user_storage_paths::error("REQUEST_KEY_CONFLICT","conflict","同一请求身份不能改变操作内容")));
                }
                resume_local_facts(&state.user, turn).map_err(|e| err_reply(&e))?;
                return Ok(Some(ok_json(&json!({"book_id":session.book_id,"session_id":session.id,"turn_id":turn.turn_id}))));
            }
        }
    }
    Ok(None)
}

/// Repairs facts only. An accepted local run is never restarted by this path.
pub(crate) fn resume_local_facts(user: &user_runtime::UserRuntime, turn: &AgentChatTurn) -> Result<(), ToolError> {
    if let Some(event) = &turn.domain.choice_event { user.learning_store()?.append_teaching(event)?; }
    if let Some(prepared) = &turn.domain.local_preparation { prepared.resume(user)?; }
    Ok(())
}

#[cfg(test)]
thread_local! { pub(crate) static LOCAL_FAIL_AFTER: std::cell::Cell<Option<&'static str>> = const { std::cell::Cell::new(None) }; }
#[cfg(test)]
pub(crate) fn local_point(point: &str) -> Result<(), ToolError> {
    if LOCAL_FAIL_AFTER.with(|p| p.get() == Some(point)) {
        return Err(user_storage_paths::error("INJECTED_CRASH", "unavailable", point));
    }
    Ok(())
}

pub(crate) fn prepare_local(state: &mut AppState, turn: &AgentTurnRef, request: &Value, now: &str) -> Result<Option<Value>, ToolError> {
    let original = state.user.agent_history.sessions.iter().find(|s| s.id == turn.session_id).unwrap()
        .turns.iter().find(|t| t.turn_id == turn.turn_id).unwrap();
    if let Some(event) = &original.domain.choice_event { state.user.learning_store()?.append_teaching(event)?; }
    let frozen = teaching::freeze_prepare(&state.private_context(), turn, request, original.domain.interaction.as_ref(), now)?;
    if state.user.session_store.is_some() {
        session_runtime::append(&mut state.user, turn, now, session_event::EventBody::TurnPrepared { teaching:frozen.clone() })?;
    } else {
        let mut candidate = state.user.agent_history.clone();
        candidate.sessions.iter_mut().find(|s| s.id == turn.session_id).unwrap().turns.iter_mut()
            .find(|t| t.turn_id == turn.turn_id).unwrap().domain.local_preparation = Some(frozen.clone());
        commit_agent_history_candidate(state, candidate)?;
    }
    #[cfg(test)]
    local_point("snapshot")?;
    for event in &frozen.events {
        state.user.learning_store()?.append_teaching(event)?;
        #[cfg(test)]
        local_point("teaching_receipt")?;
    }
    #[cfg(test)]
    local_point("prepared")?;
    Ok(frozen.context)
}

pub(crate) fn choice_event(user: &user_runtime::UserRuntime, interaction: Option<&ResolvedInteraction>, turn: &AgentTurnRef, now: &str)
    -> Result<Option<memory::teaching::TeachingEvent>, ToolError> {
    let Some(ResolvedInteraction { action: InteractionAction::UiChoice { teaching_choice, .. }, .. }) = interaction else { return Ok(None); };
    let original = user.learning_store()?.teaching_event(&teaching_choice.delivery_ref)?;
    let mut binding = original.binding;
    binding.turn_id = turn.turn_id.clone();
    binding.chat_session_id = turn.session_id.clone();
    Ok(Some(memory::teaching::TeachingEvent { event_id:format!("chosen:{}", turn.turn_id), binding,
        kind:memory::teaching::TeachingFact::ContinuationChosen, causal_refs:vec![teaching_choice.delivery_ref.clone()],
        payload:json!(memory::teaching::ContinuationChosen { delivery_ref:teaching_choice.delivery_ref.clone(),
            option_id:teaching_choice.option_id.clone(), accepted_turn_id:turn.turn_id.clone() }), occurred_at:now.into() }))
}

pub(crate) fn related_goal(user: &user_runtime::UserRuntime, chat: Option<&str>, interaction: Option<&ResolvedInteraction>) -> Option<String> {
    let original = interaction?.answer_ref.as_ref()?;
    let session = user.agent_history.sessions.iter().find(|s| Some(s.id.as_str()) == chat)?;
    let goal = session.turns.iter().find(|t| t.turn_id == original.turn_id)?.goal_ref.as_ref()?;
    session.goals.iter().find(|g| g.id == goal.id && matches!(g.status, runtime::goal::GoalStatus::Open | runtime::goal::GoalStatus::Completed))
        .map(|g| g.id.clone())
}

pub(crate) fn local_domain(state: &AppState, input: &Value, action: &ResolvedInteraction) -> Result<session_event::TurnDomain, ToolError> {
    let key = input["client_request_id"].as_str().filter(|s|!s.is_empty() && s.len()<=128)
        .ok_or_else(||invalid("阅读操作缺少请求身份"))?;
    let session = state.user.agent_history.sessions.iter().find(|s|Some(s.id.as_str()) == state.workspace.selected_chat.as_deref())
        .ok_or_else(||invalid("请先打开聊天"))?;
    if input["session_id"] != session.id {
        return Err(user_storage_paths::error("CHAT_SELECTION_CHANGED","conflict","聊天已切换，请返回原聊天或重新发起操作"));
    }
    if !input.as_object().unwrap().contains_key("expected_chat_head")
        || input["expected_chat_head"] != json!(session.turns.last().map(|t|&t.turn_id)) {
        return Err(user_storage_paths::error("CHAT_HEAD_CHANGED","conflict","聊天已变化，请重新发起操作"));
    }
    if session.turns.iter().any(|t|t.status == AgentAssistantStatus::PendingAssistant) {
        return Err(user_storage_paths::error("AGENT_RUN_BUSY","conflict","请等待当前回答完成"));
    }
    Ok(session_event::TurnDomain { interaction:Some(action.clone()),
        local_action:Some(session_event::LocalActionInput { client_request_id:key.into(),normalized:local_normalized(input)? }),
        scene:Some(session_event::ReadingScene::capture(&state.workspace)),..Default::default() })
}

impl InteractionInput {
    pub(crate) fn resolve(&self, user: &user_runtime::UserRuntime,
        workspace: &reader_workspace::ReaderWorkspace, input: &Value, quote: Option<&AskQuote>)
        -> Result<Option<InteractionResolution>, ToolError> {
        if *self == Self::Typed { return Ok(None); }
        self.validate_reference(user, workspace)?;
        let mut resolved = ResolvedInteraction {
            action: InteractionAction::UiEntry { entry_action: EntryAction::ExplainSelection },
            user_supplement: input["message"].as_str().unwrap_or_default().into(),
            accepted_request: "帮我弄懂这段".into(), answer_ref: None, answer_text: None,
            source_excerpt: None, source_bindings: vec![], target: None,
            expected_chat_head: input["expected_chat_head"].as_str().map(str::to_owned),
            presentation_follow_up: None,
        };
        match self {
            Self::Choice(choice) => {
                let saved = user.learning_store()?.teaching_event(&choice.delivery_ref)?;
                Self::Answer { action:EntryAction::ContinueAnswer, reference:AnswerRef { turn_id:saved.binding.turn_id.clone(), excerpt:None } }
                    .validate_reference(user, workspace)?;
                let private = PrivateBookContext { user, book:&workspace.book, book_dir:&workspace.book_dir,
                    messages:&workspace.messages, selected_chat:workspace.selected_chat.as_deref() };
                let view = teaching::continuation_view(&private, &saved)?.ok_or_else(||invalid("原交付没有建议集合"))?;
                use runtime::tutor_continuation::ContinuationAvailability;
                match view.availability {
                    ContinuationAvailability::Eligible => {},
                    ContinuationAvailability::Consumed { .. } => return Err(user_storage_paths::error("INTERACTION_ALREADY_ACCEPTED", "conflict", "这组建议已接纳，请查看原任务")),
                    ContinuationAvailability::Stale => return Err(user_storage_paths::error("INTERACTION_STALE", "conflict", "原建议已过期，请按当前现场继续")),
                    _ => return Err(unavailable()),
                }
                if input["expected_chat_head"] != saved.binding.turn_id {
                    return Err(user_storage_paths::error("CHAT_HEAD_CHANGED", "conflict", "选择必须保留原建议的聊天位置"));
                }
                let option = view.continuation.options.iter().find(|o| o.option_id == choice.option_id).unwrap();
                resolved.action = InteractionAction::UiChoice { teaching_choice:choice.clone(), move_kind:option.move_kind };
                resolved.accepted_request = option.label.clone();
                resolved.answer_ref = Some(AnswerRef { turn_id:saved.binding.turn_id.clone(), excerpt:None });
                let outcome = user.agent_history.sessions.iter().find(|s| s.id == saved.binding.chat_session_id).unwrap()
                    .turns.iter().find(|t| t.turn_id == saved.binding.turn_id).unwrap().outcome.as_ref().unwrap();
                resolved.answer_text = Some(outcome.answer_view.as_ref()
                    .map(|v| v.parts.iter().filter_map(|p| if let AgentAnswerPart::Markdown { text } = p { Some(text.as_str()) } else { None }).collect::<Vec<_>>().join("\n\n"))
                    .unwrap_or_else(|| outcome.answer.clone().unwrap_or_default()));
                resolved.target = Some(serde_json::from_value(saved.payload["move"]["target"].clone()).map_err(|_|invalid("原建议目标不可读"))?);
                resolved.source_bindings = resolved.target.as_ref().unwrap().source_bindings.clone();
                if let Some(receipt) = input.get("presentation_follow_up").filter(|v| !v.is_null()) {
                    if receipt["session_id"] != saved.binding.chat_session_id || receipt["turn_id"] != saved.binding.turn_id
                        || receipt["reference"] != saved.payload["move"]["presentation"] {
                        return Err(invalid("演示现场必须属于原建议的同一版本"));
                    }
                }
            },
            Self::Selection => {
                if input["source_id"] != workspace.book.base.book_id
                    || input["source_revision"] != workspace.book.source_fingerprint() {
                    return Err(user_storage_paths::error("INTERACTION_STALE", "conflict", "原文来源已变化，请重新选择"));
                }
                let quote = quote.filter(|q| q.status == Some(SelectionResolution::Resolved)
                    && q.ranges.as_ref().is_some_and(|r| !r.is_empty()))
                    .ok_or_else(|| invalid("直接帮助需要已核验的完整选区"))?;
                resolved.source_excerpt = quote.resolved_quote.clone();
                resolved.source_bindings = verified_question_evidence(&workspace.book, Some(quote)).iter()
                    .map(|range| TeachingSourceBinding { source_id: workspace.book.base.book_id.clone(),
                        source_revision: workspace.book.source_fingerprint().into(),
                        start_lid: range.start_lid.clone(), end_lid: range.end_lid.clone() }).collect();
            },
            Self::Answer { action, reference } => {
                let turn = user.agent_history.sessions.iter().find(|s| Some(s.id.as_str()) == workspace.selected_chat.as_deref())
                    .unwrap().turns.iter().find(|t| t.turn_id == reference.turn_id).unwrap();
                let outcome = turn.outcome.as_ref().unwrap();
                resolved.action = InteractionAction::UiEntry { entry_action: *action };
                resolved.accepted_request = if *action == EntryAction::ContinueAnswer { "继续理解这段" } else { "这一步没跟上" }.into();
                resolved.answer_ref = Some(reference.clone());
                resolved.answer_text = Some(reference.excerpt.clone().unwrap_or_else(|| outcome.answer_view.as_ref()
                    .map(|v| v.parts.iter().filter_map(|p| if let AgentAnswerPart::Markdown { text } = p { Some(text.as_str()) } else { None }).collect::<Vec<_>>().join("\n\n"))
                    .unwrap_or_else(|| outcome.answer.clone().unwrap_or_default())));
                if let Some(original) = &turn.domain.interaction {
                    resolved.source_bindings = original.source_bindings.clone();
                    resolved.source_excerpt = original.source_excerpt.clone();
                    resolved.target = original.target.clone();
                } else {
                    // Historical ordinary answers have no action snapshot. Verify their
                    // stored quote against the current book; never infer a new viewport.
                    let original_binding = match user.learning_store()?.teaching_event(&format!("binding:{}", turn.turn_id)) {
                        Ok(event) => Some(event),
                        Err(error) if error.error_code == "TEACHING_INVALID" => None,
                        Err(error) => return Err(error),
                    };
                    if original_binding.as_ref().is_some_and(|e| e.binding.source_revision != workspace.book.source_fingerprint()) {
                        return Err(user_storage_paths::error("INTERACTION_STALE","conflict","原回答的来源版本已变化"));
                    }
                    let stored = parse_question_quote(&json!({"question_quote":turn.question_quote}), &workspace.book)
                        .map_err(|_| invalid("原回答的原文范围已失效"))?;
                    resolved.source_bindings = verified_question_evidence(&workspace.book, stored.as_ref()).iter()
                        .map(|r| TeachingSourceBinding { source_id: workspace.book.base.book_id.clone(),
                            source_revision: workspace.book.source_fingerprint().into(),start_lid:r.start_lid.clone(),end_lid:r.end_lid.clone() }).collect();
                    resolved.source_excerpt = stored.and_then(|q| q.resolved_quote);
                    if resolved.source_bindings.is_empty() {
                        if let Some(original) = original_binding {
                            let mut excerpts = Vec::new();
                            for binding in &turn.source_bindings {
                                if binding.book_id != workspace.book.base.book_id { return Err(invalid("接续范围包含其他材料，请明确选择原文")); }
                                let source = workspace.book.resolve_source(&binding.evidence_range,"zh-CN",Some(&binding.evidence_text_digest))
                                    .map_err(|_|invalid("原回答的引用来源已失效"))?;
                                excerpts.push(source.highlighted_quote);
                                resolved.source_bindings.push(TeachingSourceBinding { source_id:binding.book_id.clone(),
                                    source_revision:original.binding.source_revision.clone(),
                                    start_lid:binding.evidence_range.start_lid.clone(),end_lid:binding.evidence_range.end_lid.clone() });
                            }
                            if !excerpts.is_empty() { resolved.source_excerpt=Some(excerpts.join("\n\n")); }
                        }
                    }
                }
            },
            _ => unreachable!(),
        }
        if resolved.source_bindings.iter().any(|s| s.source_id != workspace.book.base.book_id
            || s.source_revision != workspace.book.source_fingerprint()) {
            return Err(user_storage_paths::error("INTERACTION_STALE", "conflict", "原回答的来源已变化，请重新选择原文"));
        }
        let private = PrivateBookContext { user, book:&workspace.book, book_dir:&workspace.book_dir,
            messages:&workspace.messages, selected_chat:workspace.selected_chat.as_deref() };
        let helps = teaching::pending_activity_help(&private, &resolved)?;
        if !helps.is_empty() { return Ok(Some(InteractionResolution::ActivityHelp { delivery_refs: helps })); }
        if matches!(self, Self::Answer { action:EntryAction::ContinueAnswer, .. }) {
            let view = user.learning_store()?.state()?;
            let current = view.control.current_tutor_session_id.as_ref().and_then(|id|view.sessions.get(id));
            if !view.control.enabled || tutor_api::tutor_source_readiness(&workspace.book,&workspace.book_dir)["status"] != "ready"
                || current.is_some_and(|s| s.status != memory::learning::TutorSessionStatus::Active
                    || !s.material_scope.iter().any(|m|m.source_id == workspace.book.base.book_id)) {
                return Err(user_storage_paths::error("INTERACTION_UNAVAILABLE", "conflict", "请先开启连续教学，并在当前材料显式开始或恢复学习"));
            }
            if resolved.source_bindings.is_empty() { return Err(invalid("原回答没有可接续的原文范围，请重新选择原文")); }
        }
        Ok(Some(InteractionResolution::RunInput { interaction:resolved }))
    }
    /// Preserves original strings. Normalization must not erase payload differences.
    pub(crate) fn parse(input: &Value) -> Result<Self, ToolError> {
        let present = |key: &str| input.get(key).is_some_and(|v| !v.is_null());
        let parse = |key: &str| input.get(key).cloned().unwrap_or(Value::Null);
        let action: Option<EntryAction> = serde_json::from_value(parse("entry_action"))
            .map_err(|_| invalid("未知阅读操作"))?;
        let answer: Option<AnswerRef> = serde_json::from_value(parse("answer_ref"))
            .map_err(|_| invalid("回答引用格式无效"))?;
        let choice: Option<TeachingChoice> = serde_json::from_value(parse("teaching_choice"))
            .map_err(|_| invalid("建议选择格式无效"))?;
        if present("resolved_interaction") || present("current_action") {
            return Err(invalid("解析结果由服务端生成"));
        }
        if action.is_some() && choice.is_some() {
            return Err(invalid("阅读操作与建议选择不能同时提交"));
        }
        let typed_action = action.is_some() || choice.is_some();
        if typed_action && ["teaching_ref", "note_mem_id", "goal_action", "goal_id", "retry_of"]
            .iter().any(|key| present(key)) {
            return Err(invalid("阅读操作不能混用原活动回应、笔记或任务替换元数据"));
        }
        let result = match (action, answer, choice) {
            (None, None, None) => Self::Typed,
            (Some(EntryAction::ExplainSelection), None, None) if input["question_quote"].is_object() => Self::Selection,
            (Some(action @ (EntryAction::ClarifyAnswer | EntryAction::ContinueAnswer)), Some(reference), None)
                if !present("question_quote") && !present("question_anchor_lid") => {
                    if reference.turn_id.trim().is_empty() || reference.excerpt.as_deref().is_some_and(|s| s.trim().is_empty()) {
                        return Err(invalid("回答标识和所选摘录不能为空"));
                    }
                    Self::Answer { action, reference }
                },
            (None, None, Some(choice)) if !present("question_quote") && !present("question_anchor_lid") => {
                if choice.delivery_ref.trim().is_empty() || choice.option_id.trim().is_empty() {
                    return Err(invalid("建议交付标识与选项不能为空"));
                }
                Self::Choice(choice)
            },
            _ => return Err(invalid("操作与原文选区或回答引用不匹配")),
        };
        let message = input["message"].as_str().ok_or_else(|| invalid("需 message 用户消息文本"))?;
        if result == Self::Typed && message.trim().is_empty() {
            return Err(invalid("请输入问题或选择一个阅读操作"));
        }
        if matches!(result, Self::Answer { .. } | Self::Choice(_))
            && !input["expected_chat_head"].as_str().is_some_and(|s| !s.trim().is_empty()) {
            return Err(invalid("回答接续和建议选择需保留点击时的聊天位置"));
        }
        Ok(result)
    }

    /// Validate stored ownership before reporting that the new execution path is closed.
    pub(crate) fn validate_reference(&self, user: &user_runtime::UserRuntime,
        workspace: &reader_workspace::ReaderWorkspace) -> Result<(), ToolError> {
        let chat = workspace.selected_chat.as_deref();
        match self {
            Self::Answer { reference, .. } => {
                let turn = user.agent_history.sessions.iter()
                    .find(|s| Some(s.id.as_str()) == chat && s.book_id == workspace.book.base.book_id)
                    .and_then(|s| s.turns.iter().find(|t| t.turn_id == reference.turn_id))
                    .ok_or_else(|| invalid("原回答不属于当前聊天"))?;
                let outcome = turn.outcome.as_ref().filter(|o| turn.status == AgentAssistantStatus::Completed
                    && !crate::observability::lifecycle::delivery_failed(o)
                    && !turn.delivery_diagnostics.as_ref().and_then(|d| d.repair.as_ref()).is_some_and(|d| !d.issues.is_empty()))
                    .ok_or_else(|| invalid("只能接续已成功交付的回答"))?;
                let texts: Vec<&str> = outcome.answer_view.as_ref().map(|v| v.parts.iter().filter_map(|p|
                    if let AgentAnswerPart::Markdown { text } = p { Some(text.as_str()) } else { None }).collect())
                    .unwrap_or_else(|| outcome.answer.as_deref().into_iter().collect());
                if texts.iter().all(|s| s.trim().is_empty()) || reference.excerpt.as_ref()
                    .is_some_and(|excerpt| !texts.iter().any(|s| s.contains(excerpt.as_str()))) {
                    return Err(invalid("摘录必须来自原回答的连续公开文字"));
                }
            },
            Self::Choice(choice) => {
                let event = user.learning_store()?.teaching_event(&choice.delivery_ref)
                    .map_err(|e| if e.error_code == "TEACHING_INVALID" { invalid("建议交付不存在") } else { e })?;
                if event.kind != memory::teaching::TeachingFact::MessageDelivered
                    || Some(event.binding.chat_session_id.as_str()) != chat
                    || event.binding.source_id != workspace.book.base.book_id {
                    return Err(invalid("建议不属于当前聊天的已交付回答"));
                }
                let continuation: runtime::tutor_continuation::TeachingContinuation =
                    serde_json::from_value(event.payload["move"]["continuation"].clone())
                        .map_err(|_| invalid("原交付没有建议集合"))?;
                continuation.validate().map_err(|_| invalid("原建议集合无效"))?;
                if !continuation.options.iter().any(|o| o.option_id == choice.option_id) {
                    return Err(invalid("所选建议不存在"));
                }
            },
            _ => {},
        }
        Ok(())
    }
}
