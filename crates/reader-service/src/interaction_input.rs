//! Shared input contract and source resolution before either host accepts a run.
use crate::*;
use runtime::tutor_continuation::{AnswerRef, EntryAction, TeachingChoice, InteractionAction, InteractionResolution, ResolvedInteraction};
use memory::teaching::TeachingSourceBinding;
use memory::learning::LearningPurpose;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum InteractionInput {
    Typed,
    Selection,
    GuideChapter { chapter_lid: String },
    LearnWithPurpose { purpose: LearningPurpose, chapter_lid: Option<String> },
    ApplyLesson,
    Answer { action: EntryAction, reference: AnswerRef },
    Choice(TeachingChoice),
    /// Practice from one exact saved idea against the current material.
    RevisitNote { note_ref: memory::NoteRef },
}

pub(crate) fn invalid(message: &str) -> ToolError {
    user_storage_paths::error("INTERACTION_INVALID", "validation", message)
}
pub(crate) fn unavailable() -> ToolError {
    user_storage_paths::error("INTERACTION_UNAVAILABLE", "unavailable", "此阅读操作暂不可用，请继续使用输入框提问")
}

fn local_normalized(input: &Value) -> Result<Value, ToolError> {
    run_input::normalize("local", input)
}

/// Read the original accepted identity before inspecting the current head or
/// source. A pending/failed operation is returned, never executed a second time.
pub(crate) fn local_repeat(state: &mut AppState, body: &str) -> Result<Option<Reply>, Reply> {
    let input = body_value(body)?;
    InteractionInput::parse(&input).map_err(|e|err_reply(&e))?;
    let Some(key) = input["client_request_id"].as_str() else { return Ok(None); };
    crate::board_delivery::repair(&mut state.user, &crate::service_error::now().to_string()).map_err(|e| err_reply(&e))?;
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
    if let Some(event) = &turn.domain.choice_event { user.learning_store_for_workspace(turn.domain.scene.as_ref().and_then(|s|s.learning_workspace_id.as_deref()))?.append_teaching(event)?; }
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
    if let Some(event) = &original.domain.choice_event { state.user.learning_store_for_workspace(original.domain.scene.as_ref().and_then(|s|s.learning_workspace_id.as_deref()))?.append_teaching(event)?; }
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
        state.user.learning_store_for_workspace(frozen.learning_workspace_id.as_deref())?.append_teaching(event)?;
        #[cfg(test)]
        local_point("teaching_receipt")?;
    }
    crate::teaching_coach::ensure_turn(&state.user, frozen.learning_workspace_id.as_deref(), turn, frozen.context.as_ref(), now)?;
    #[cfg(test)]
    local_point("prepared")?;
    Ok(frozen.context)
}

pub fn choice_event(user: &user_runtime::UserRuntime, interaction: Option<&ResolvedInteraction>, turn: &AgentTurnRef, now: &str)
    -> Result<Option<memory::teaching::TeachingEvent>, ToolError> {
    let Some(ResolvedInteraction { action: InteractionAction::UiChoice { teaching_choice, .. }, .. }) = interaction else { return Ok(None); };
    let scope = user.agent_history.sessions.iter().find(|s|s.id == turn.session_id).and_then(|s|s.learning_workspace_id.as_deref());
    let original = user.learning_store_for_workspace(scope)?.teaching_event(&teaching_choice.delivery_ref)?;
    let mut binding = original.binding;
    binding.turn_id = turn.turn_id.clone();
    binding.chat_session_id = turn.session_id.clone();
    Ok(Some(memory::teaching::TeachingEvent { event_id:format!("chosen:{}", turn.turn_id), binding,
        kind:memory::teaching::TeachingFact::ContinuationChosen, causal_refs:vec![teaching_choice.delivery_ref.clone()],
        payload:json!(memory::teaching::ContinuationChosen { delivery_ref:teaching_choice.delivery_ref.clone(),
            option_id:teaching_choice.option_id.clone(), accepted_turn_id:turn.turn_id.clone() }), occurred_at:now.into() }))
}

pub fn related_goal(user: &user_runtime::UserRuntime, chat: Option<&str>, interaction: Option<&ResolvedInteraction>) -> Option<String> {
    let original = interaction?.answer_ref.as_ref()?;
    let session = user.agent_history.sessions.iter().find(|s| Some(s.id.as_str()) == chat)?;
    let goal = session.turns.iter().find(|t| t.turn_id == original.turn_id)?.goal_ref.as_ref()?;
    session.goals.iter().find(|g| g.id == goal.id && matches!(g.status, runtime::goal::GoalStatus::Open | runtime::goal::GoalStatus::Completed))
        .map(|g| g.id.clone())
}

pub(crate) fn local_domain(state: &AppState, input: &Value, action: &ResolvedInteraction) -> Result<session_event::TurnDomain, ToolError> {
    local_domain_for_request(state, input, Some(action))
}

pub(crate) fn local_domain_for_request(state: &AppState, input: &Value, action: Option<&ResolvedInteraction>) -> Result<session_event::TurnDomain, ToolError> {
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
    Ok(session_event::TurnDomain { interaction:action.cloned(),
        local_action:Some(session_event::LocalActionInput { client_request_id:key.into(),normalized:local_normalized(input)? }),
        scene:Some(session_event::ReadingScene::capture(&state.workspace)),..Default::default() })
}

impl InteractionInput {
    pub(crate) fn resolve(&self, user: &user_runtime::UserRuntime,
        workspace: &reader_workspace::ReaderWorkspace, input: &Value, quote: Option<&AskQuote>)
        -> Result<Option<InteractionResolution>, ToolError> {
        if *self == Self::Typed { return Ok(None); }
        self.validate_reference(user, workspace)?;
        let answer_practice = matches!(self, Self::Answer { action: EntryAction::ContinueAnswer, .. })
            && input["learning_purpose"] == "practice";
        let mut has_reference_sources = false;
        let mut has_media_scope = false;
        let mut resolved = ResolvedInteraction {
            action: InteractionAction::UiEntry { entry_action: EntryAction::ExplainSelection },
            user_supplement: input["message"].as_str().unwrap_or_default().into(),
            accepted_request: "帮我弄懂这段".into(), answer_ref: None, answer_text: None,
            source_excerpt: None, source_bindings: vec![], target: None,
            expected_chat_head: input["expected_chat_head"].as_str().map(str::to_owned),
            presentation_follow_up: None,
            learning_purpose: answer_practice.then_some(LearningPurpose::Practice),
            note_ref: None,
        };
        if answer_practice || matches!(self, Self::LearnWithPurpose { purpose: LearningPurpose::Practice, .. } | Self::ApplyLesson | Self::RevisitNote { .. }) {
            if tutor_api::tutor_source_readiness(&workspace.book, &workspace.book_dir)["status"] != "ready" {
                return Err(user_storage_paths::error("INTERACTION_UNAVAILABLE", "conflict", "请等待当前材料准备完成"));
            }
        }
        if let Self::LearnWithPurpose { purpose, .. } = self {
            resolved.learning_purpose = Some(*purpose);
            if !matches!(*purpose, LearningPurpose::Discover | LearningPurpose::Practice) {
                if tutor_api::tutor_source_readiness(&workspace.book, &workspace.book_dir)["status"] != "ready" {
                    return Err(user_storage_paths::error("INTERACTION_UNAVAILABLE", "conflict", "请等待当前材料准备完成"));
                }
            }
        }
        match self {
            Self::ApplyLesson => {
                let source: memory::course::CourseSource = serde_json::from_value(
                    input["course_context"]["lesson"]["summary"]["source"].clone()
                ).map_err(|_| invalid("请打开当前课程后再出应用题"))?;
                if input["source_id"] != source.publication.book_id || input["source_revision"] != source.source_revision {
                    return Err(user_storage_paths::error("INTERACTION_STALE", "conflict", "本课来源已变化，请重新打开课程"));
                }
                resolved.action = InteractionAction::UiEntry { entry_action: EntryAction::ApplyLesson };
                resolved.learning_purpose = Some(LearningPurpose::Practice);
                resolved.accepted_request = "围绕本课刚讲的关系，给我一道新情境应用题".into();
                resolved.source_bindings = vec![TeachingSourceBinding {
                    source_id: source.publication.book_id, source_revision: source.source_revision,
                    start_lid: source.start_lid, end_lid: source.end_lid,
                }];
            },
            Self::Choice(choice) => {
                let saved = user.learning_store_for_workspace(workspace.learning_workspace_id.as_deref())?.teaching_event(&choice.delivery_ref)?;
                Self::Answer { action:EntryAction::ContinueAnswer, reference:AnswerRef { turn_id:saved.binding.turn_id.clone(), excerpt:None } }
                    .validate_reference(user, workspace)?;
                let private = PrivateBookContext { user, book:&workspace.book, book_dir:&workspace.book_dir,
                    learning_workspace_id:workspace.learning_workspace_id.as_deref(),
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
                    .map(|v| v.parts.iter().filter_map(AgentAnswerPart::text).collect::<Vec<_>>().join("\n\n"))
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
            Self::GuideChapter { chapter_lid } | Self::LearnWithPurpose { chapter_lid: Some(chapter_lid), .. } => {
                if input["source_id"] != workspace.book.base.book_id
                    || input["source_revision"] != workspace.book.source_fingerprint() {
                    return Err(user_storage_paths::error("INTERACTION_STALE", "conflict", "原文来源已变化，请重新选择章节"));
                }
                // Purpose entry can name a real subsection; the older chapter
                // action keeps its chapter-only contract.
                let title = if resolved.learning_purpose.is_some() {
                    container_title(&workspace.book, chapter_lid, true)?
                } else { chapter_title(&workspace.book, chapter_lid)? };
                let section = workspace.book.base.lid_nodes.iter().any(|node|
                    node.lid == *chapter_lid && node.kind == base_schema::NodeKind::Section);
                let scope = if section { format!("小节「{title}」") } else { format!("《{title}》这一章") };
                resolved.action = InteractionAction::UiEntry { entry_action: if resolved.learning_purpose.is_some() {
                    EntryAction::LearnWithPurpose
                } else { EntryAction::GuideChapter } };
                resolved.accepted_request = resolved.learning_purpose.map(|purpose| purpose_request(purpose, &scope))
                    .unwrap_or_else(||format!("带我读《{title}》这一章"));
                resolved.source_bindings = vec![TeachingSourceBinding {
                    source_id: workspace.book.base.book_id.clone(),
                    source_revision: workspace.book.source_fingerprint().into(),
                    start_lid: chapter_lid.clone(), end_lid: chapter_lid.clone(),
                }];
            },
            Self::Selection | Self::LearnWithPurpose { chapter_lid: None, .. } => {
                if input["source_id"] != workspace.book.base.book_id
                    || input["source_revision"] != workspace.book.source_fingerprint() {
                    return Err(user_storage_paths::error("INTERACTION_STALE", "conflict", "原文来源已变化，请重新选择"));
                }
                if let Some(purpose) = resolved.learning_purpose.filter(|_| input.get("question_quote").is_none_or(Value::is_null)) {
                    if let Some(scope) = media_purpose_scope(workspace, input) {
                        // Media input locates the learning scope; it is not text
                        // evidence. The run must read the prepared source before
                        // constructing a source-grounded teaching target.
                        resolved.action = InteractionAction::UiEntry { entry_action: EntryAction::LearnWithPurpose };
                        resolved.accepted_request = purpose_request(purpose, scope);
                    } else {
                        let first = workspace.book.base.lid_nodes.iter().min_by_key(|node|node.span.start)
                            .ok_or_else(||invalid("当前材料尚无可定位的正文"))?;
                        let last = workspace.book.base.lid_nodes.iter().max_by_key(|node|node.span.end).unwrap();
                        resolved.action = InteractionAction::UiEntry { entry_action: EntryAction::LearnWithPurpose };
                        resolved.accepted_request = purpose_request(purpose, "当前材料");
                        resolved.source_bindings = vec![TeachingSourceBinding { source_id:workspace.book.base.book_id.clone(),
                            source_revision:workspace.book.source_fingerprint().into(), start_lid:first.lid.clone(),end_lid:last.lid.clone() }];
                    }
                } else {
                    let quote = quote.filter(|q| q.status == Some(SelectionResolution::Resolved)
                        && q.ranges.as_ref().is_some_and(|r| !r.is_empty()))
                        .ok_or_else(|| invalid("直接帮助需要已核验的完整选区"))?;
                    resolved.source_excerpt = quote.resolved_quote.clone();
                    if let Some(purpose) = resolved.learning_purpose {
                        resolved.action = InteractionAction::UiEntry { entry_action: EntryAction::LearnWithPurpose };
                        resolved.accepted_request = purpose_request(purpose, "所选内容");
                    }
                    resolved.source_bindings = verified_question_evidence(&workspace.book, Some(quote)).iter()
                        .map(|range| TeachingSourceBinding { source_id: workspace.book.base.book_id.clone(),
                            source_revision: workspace.book.source_fingerprint().into(),
                            start_lid: range.start_lid.clone(), end_lid: range.end_lid.clone() }).collect();
                }
            },
            Self::Answer { action, reference } => {
                let turn = user.agent_history.sessions.iter().find(|s| Some(s.id.as_str()) == workspace.selected_chat.as_deref())
                    .unwrap().turns.iter().find(|t| t.turn_id == reference.turn_id).unwrap();
                let outcome = turn.outcome.as_ref().unwrap();
                has_reference_sources = turn.source_bindings.iter().any(|s| s.publication_id.is_some());
                let media_source = media_purpose_scope(workspace, input).is_some();
                let original_binding = if media_source || turn.domain.interaction.is_none() {
                    match user.learning_store_for_workspace(workspace.learning_workspace_id.as_deref())?
                        .teaching_event(&format!("binding:{}", turn.turn_id)) {
                        Ok(event) => Some(event),
                        Err(error) if error.error_code == "TEACHING_INVALID" => None,
                        Err(error) => return Err(error),
                    }
                } else { None };
                if original_binding.as_ref().is_some_and(|event| event.binding.source_id != workspace.book.base.book_id
                    || event.binding.source_revision != workspace.book.source_fingerprint()) {
                    return Err(user_storage_paths::error("INTERACTION_STALE", "conflict", "原回答的来源版本已变化"));
                }
                has_media_scope = media_source
                    && (turn.video_question.is_some() || turn.music_question.is_some()
                        || turn.domain.interaction.as_ref().is_some_and(|action| action.learning_purpose.is_some())
                        || original_binding.is_some()
                        || turn.source_bindings.iter().any(|source| source.media.is_some() || source.music.is_some()));
                resolved.action = InteractionAction::UiEntry { entry_action: *action };
                resolved.accepted_request = if answer_practice { "根据这段回答给我一道应用题" }
                    else if *action == EntryAction::ContinueAnswer { "继续理解这段" } else { "这一步没跟上" }.into();
                resolved.answer_ref = Some(reference.clone());
                resolved.answer_text = Some(reference.excerpt.clone().unwrap_or_else(|| outcome.answer_view.as_ref()
                    .map(|v| v.parts.iter().filter_map(AgentAnswerPart::text).collect::<Vec<_>>().join("\n\n"))
                    .unwrap_or_else(|| outcome.answer.clone().unwrap_or_default())));
                if let Some(original) = &turn.domain.interaction {
                    resolved.source_bindings = original.source_bindings.clone();
                    resolved.source_excerpt = original.source_excerpt.clone();
                    resolved.target = original.target.clone();
                } else {
                    // Historical ordinary answers have no action snapshot. Verify their
                    // stored quote against the current book; never infer a new viewport.
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
                                if binding.publication_id.is_some() { continue; }
                                if binding.book_id != workspace.book.base.book_id { return Err(invalid("接续范围包含其他材料，请明确选择原文")); }
                                let range = binding.evidence_range.as_ref().ok_or_else(unavailable)?;
                                let source = workspace.book.resolve_source(range,"zh-CN",Some(&binding.evidence_text_digest))
                                    .map_err(|_|invalid("原回答的引用来源已失效"))?;
                                excerpts.push(source.highlighted_quote);
                                resolved.source_bindings.push(TeachingSourceBinding { source_id:binding.book_id.clone(),
                                    source_revision:original.binding.source_revision.clone(),
                                    start_lid:range.start_lid.clone(),end_lid:range.end_lid.clone() });
                            }
                            if !excerpts.is_empty() { resolved.source_excerpt=Some(excerpts.join("\n\n")); }
                        }
                    }
                }
            },
            Self::RevisitNote { note_ref } => {
                if input["source_id"] != workspace.book.base.book_id
                    || input["source_revision"] != workspace.book.source_fingerprint() {
                    return Err(user_storage_paths::error("INTERACTION_STALE", "conflict", "原文来源已变化，请重新打开材料"));
                }
                // The saved idea is the learner's own words; the material scope
                // supplies the source the problem and its solution are grounded in.
                user.store.personal_note(note_ref).map_err(|_| invalid("这条想法的版本不存在或已删除"))?;
                let first = workspace.book.base.lid_nodes.iter().min_by_key(|node|node.span.start)
                    .ok_or_else(||invalid("当前材料尚无可定位的正文"))?;
                let last = workspace.book.base.lid_nodes.iter().max_by_key(|node|node.span.end).unwrap();
                resolved.action = InteractionAction::UiEntry { entry_action: EntryAction::RevisitNote };
                resolved.accepted_request = if input["understanding_ref"].is_null() {
                    "用我这条想法练一练"
                } else {
                    "换个例子练习"
                }.into();
                resolved.note_ref = Some(note_ref.clone());
                resolved.source_bindings = vec![TeachingSourceBinding { source_id:workspace.book.base.book_id.clone(),
                    source_revision:workspace.book.source_fingerprint().into(), start_lid:first.lid.clone(),end_lid:last.lid.clone() }];
            },
            _ => unreachable!(),
        }
        if let Some(reference)=input.get("understanding_ref").filter(|v|!v.is_null()) {
            let evidence=crate::knowledge_tree::evidence(user,reference)?;
            if reference["workspace_id"].as_str()!=workspace.learning_workspace_id.as_deref()
                || evidence["source_id"]!=workspace.book.base.book_id || evidence["source_revision"]!=workspace.book.source_fingerprint()
                || evidence["superseded"]==true { return Err(invalid("理解依据或学习范围已变化，请刷新理解记录后继续")); }
            let target: memory::teaching::TeachingTarget=serde_json::from_value(evidence["target"].clone()).map_err(|_|invalid("这条历史记录尚无可接续的知识关系"))?;
            if !target.source_bindings.is_empty() {
                resolved.source_bindings=target.source_bindings.clone();
            }
            if matches!(self, Self::Answer { action:EntryAction::ContinueAnswer, .. }) {
                resolved.accepted_request=if answer_practice {"换个例子练习"} else {"接着这一点"}.into();
            }
            resolved.target=Some(target);
        }
        if resolved.source_bindings.iter().any(|s| s.source_id != workspace.book.base.book_id
            || s.source_revision != workspace.book.source_fingerprint()) {
            return Err(user_storage_paths::error("INTERACTION_STALE", "conflict", "原回答的来源已变化，请重新选择原文"));
        }
        let private = PrivateBookContext { user, book:&workspace.book, book_dir:&workspace.book_dir,
                    learning_workspace_id:workspace.learning_workspace_id.as_deref(),
            messages:&workspace.messages, selected_chat:workspace.selected_chat.as_deref() };
        // A chapter is a navigation scope, not a claim that the user selected
        // every pending activity inside it for more help.
        // Explicit answer practice requests a new problem on the original scope.
        let helps = if resolved.requests_practice() || matches!(self, Self::GuideChapter { .. } | Self::LearnWithPurpose { .. }) { vec![] }
            else { teaching::pending_activity_help(&private, &resolved)? };
        if !helps.is_empty() && resolved.user_supplement.trim().is_empty() {
            return Ok(Some(InteractionResolution::ActivityHelp { delivery_refs: helps }));
        }
        if matches!(self, Self::Answer { action:EntryAction::ContinueAnswer, .. }) && !has_reference_sources {
            if resolved.source_bindings.is_empty() && !has_media_scope { return Err(invalid("原回答没有可接续的原文范围，请重新选择原文")); }
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
        let purpose: Option<LearningPurpose> = serde_json::from_value(parse("learning_purpose"))
            .map_err(|_| invalid("未知学习目的"))?;
        if purpose.is_some() && action != Some(EntryAction::LearnWithPurpose)
            && !(matches!(action, Some(EntryAction::ContinueAnswer | EntryAction::ApplyLesson)) && purpose == Some(LearningPurpose::Practice)) {
            return Err(invalid("学习目的只用于目的选择，或围绕原回答进行应用练习"));
        }
        if present("resolved_interaction") || present("current_action") || present("guide_chapter_lid") {
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
        if present("chapter_lid") && !matches!(action, Some(EntryAction::GuideChapter | EntryAction::LearnWithPurpose)) {
            return Err(invalid("章节定位只用于章节导读操作"));
        }
        if present("note_ref") != (action == Some(EntryAction::RevisitNote)) {
            return Err(invalid("想法回访需要且只接受一个确切笔记版本"));
        }
        let result = match (action, answer, choice) {
            (Some(EntryAction::ApplyLesson), None, None)
                if purpose == Some(LearningPurpose::Practice) && input["course_context"].is_object()
                    && !present("question_quote") && !present("question_anchor_lid") =>
                Self::ApplyLesson,
            (Some(EntryAction::LearnWithPurpose), None, None) => {
                let purpose = purpose.ok_or_else(||invalid("请选择本次学习目的"))?;
                let chapter_lid = input["chapter_lid"].as_str().map(str::to_owned);
                let chapter = chapter_lid.as_ref().is_some_and(|s| !s.trim().is_empty());
                let selection = input["question_quote"].is_object();
                if (chapter && selection) || (present("chapter_lid") && !chapter)
                    || (present("question_quote") && !selection) || (!selection && present("question_anchor_lid")) {
                    return Err(invalid("学习目的的章节和选区不能同时提交；未选择时使用当前材料"));
                }
                Self::LearnWithPurpose { purpose, chapter_lid }
            },
            (Some(EntryAction::GuideChapter), None, None)
                if !present("question_quote") && !present("question_anchor_lid") => {
                    let chapter_lid = input["chapter_lid"].as_str().filter(|s| !s.trim().is_empty())
                        .ok_or_else(|| invalid("章节导读需要规范章节标识"))?;
                    Self::GuideChapter { chapter_lid: chapter_lid.into() }
                },
            (Some(EntryAction::RevisitNote), None, None)
                if !present("question_quote") && !present("question_anchor_lid") => {
                    let note_ref: memory::NoteRef = serde_json::from_value(parse("note_ref"))
                        .map_err(|_| invalid("笔记引用格式无效"))?;
                    if note_ref.note_id.trim().is_empty() || !note_ref.revision.is_some_and(|n| n > 0) {
                        return Err(invalid("想法回访需要确切笔记版本"));
                    }
                    Self::RevisitNote { note_ref }
                },
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
                    .find(|s| Some(s.id.as_str()) == chat && s.matches_scope(workspace.learning_workspace_id.as_deref(), &workspace.book.base.book_id))
                    .and_then(|s| s.turns.iter().find(|t| t.turn_id == reference.turn_id))
                    .ok_or_else(|| invalid("原回答不属于当前聊天"))?;
                let outcome = turn.outcome.as_ref().filter(|o| turn.status == AgentAssistantStatus::Completed
                    && !crate::observability::lifecycle::delivery_failed(o)
                    && !turn.delivery_diagnostics.as_ref().and_then(|d| d.repair.as_ref()).is_some_and(|d| !d.issues.is_empty()))
                    .ok_or_else(|| invalid("只能接续已成功交付的回答"))?;
                let texts = crate::note_api::answer_text_blocks(outcome);
                if texts.iter().all(|s| s.trim().is_empty()) || reference.excerpt.as_ref()
                    .is_some_and(|excerpt| !texts.iter().any(|s| s.contains(excerpt.as_str()))) {
                    return Err(invalid("摘录必须来自原回答的连续公开文字"));
                }
            },
            Self::Choice(choice) => {
                let event = user.learning_store_for_workspace(workspace.learning_workspace_id.as_deref())?.teaching_event(&choice.delivery_ref)
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

fn media_purpose_scope(workspace: &reader_workspace::ReaderWorkspace, input: &Value) -> Option<&'static str> {
    if workspace.book_dir.join("video_source_manifest.json").is_file() {
        Some(if input.get("video_draft_id").is_some_and(|v| !v.is_null()) { "当前视频片段" } else { "当前视频" })
    } else if workspace.book_dir.join("music_source_manifest.json").is_file() {
        Some(if input.get("music_question").is_some_and(|v| !v.is_null()) { "当前音乐片段" } else { "当前音乐" })
    } else { None }
}

fn purpose_request(purpose: LearningPurpose, scope: &str) -> String {
    match purpose {
        LearningPurpose::Discover => format!("先了解{scope}的重点：用三到五条简短要点概览主要内容及关系，附原文出处，并指出一个适合继续深入的切入点。先给概览，不展开长篇讲解，也不出题。"),
        LearningPurpose::Understand => format!("带我读懂{scope}"),
        LearningPurpose::Exam => format!("围绕{scope}复习备考"),
        LearningPurpose::Explore => format!("围绕{scope}探索一个问题"),
        LearningPurpose::Practice => format!("围绕{scope}练一练"),
    }
}

/// Canonical chapter topology supplies the container; its source heading supplies the name.
pub(crate) fn chapter_title(book: &Book, lid: &str) -> Result<String, ToolError> {
    container_title(book, lid, false)
}

pub(crate) fn container_title(book: &Book, lid: &str, allow_section: bool) -> Result<String, ToolError> {
    let node = book.base.lid_nodes.iter().find(|n| n.lid == lid && (n.kind == base_schema::NodeKind::Chapter
        || (allow_section && n.kind == base_schema::NodeKind::Section)))
        .ok_or_else(|| invalid(if allow_section { "学习范围需要当前材料的章节或小节节点" } else { "章节导读需要当前材料的规范章节点" }))?;
    if let Some(title) = book.manifest().tree.into_iter().find(|entry| entry.lid == node.lid).and_then(|entry| entry.title) {
        let title = title.trim();
        let title = if title.starts_with('#') { title.trim_start_matches('#').trim().trim_end_matches('#').trim() } else { title };
        if !title.is_empty() { return Ok(title.to_owned()); }
    }
    let label = book.source_label(&EvidenceRange { start_lid: node.lid.clone(), end_lid: node.lid.clone(), ranges: vec![] }, "zh-CN")?;
    Ok(label.strip_prefix("章节 · ").or_else(|| label.strip_prefix("小节 · ")).unwrap_or(&label).to_owned())
}

#[cfg(test)]
mod chapter_title_tests {
    use super::*;
    use base_schema::{LidNode, NodeKind, Span};

    #[test]
    fn chapter_title_uses_source_heading_instead_of_lid_number() {
        let title = "第1章 一本书怎样成为可以交互的阅读环境";
        for heading in [title.to_owned(), format!("## {title} ##")] {
            let source = format!("{heading}\n这里是正文。\n");
            let heading_end = heading.encode_utf16().count() + 1;
            let source_end = source.encode_utf16().count();
            let mut base = base_schema::sample_base();
            base.graph_nodes.clear();
            base.graph_edges.clear();
            base.lid_nodes = vec![
                LidNode { lid:"5".into(),path:vec![5],kind:NodeKind::Chapter,span:Span {start:0,end:source_end},
                    children:vec!["5.heading".into(),"5.1".into()] },
                LidNode { lid:"5.heading".into(),path:vec![5,0],kind:NodeKind::Paragraph,span:Span {start:0,end:heading_end},children:vec![] },
                LidNode { lid:"5.1".into(),path:vec![5,1],kind:NodeKind::Paragraph,span:Span {start:heading_end,end:source_end},children:vec![] },
            ];
            let book = Book::new(base,&source);
            assert_eq!(chapter_title(&book,"5").unwrap(),title);
            assert!(chapter_title(&book,"5.heading").is_err(),"a heading leaf is not a chapter container");
        }
    }
}
