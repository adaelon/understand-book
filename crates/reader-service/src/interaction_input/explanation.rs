//! The expression belongs to one delivered answer and retains its identity across attempts.
use super::*;
use crate::session_event::ExplanationAttempt;
use runtime::teach_back::{TeachBackComparison, TeachBackReply, TeachBackReplyKind};

pub fn attempt(input: &Value) -> Result<Option<ExplanationAttempt>, ToolError> {
    if input["entry_action"] != "explain_back" { return Ok(None); }
    let id = |key: &str| input[key].as_str().filter(|s| !s.trim().is_empty() && s.len() <= 128)
        .ok_or_else(|| invalid("解释提交缺少有效的表达或操作标识"));
    let expression_id = id("expression_id")?;
    if id("client_request_id")? != expression_id {
        return Err(invalid("提交与重试须沿用同一表达标识"));
    }
    if !input["retry_of"].is_null() { id("retry_of")?; }
    Ok(Some(ExplanationAttempt { expression_id: expression_id.into(), operation_id: id("operation_id")?.into() }))
}

pub fn can_retry(turn: &AgentChatTurn, attempt: &ExplanationAttempt) -> bool {
    turn.status == AgentAssistantStatus::Failed
        && turn.domain.teach_back_attempts.first().is_some_and(|first| first.expression_id == attempt.expression_id)
        && !turn.domain.teach_back_attempts.contains(attempt)
}

/// The frozen request remains authoritative for the expression, source answer and scene.
/// Only the operation and current chat head are new on an explicit retry.
pub fn retry_replay(turn: &AgentChatTurn, normalized: &Value, original: &Value) -> Result<bool, ToolError> {
    let attempt = attempt(normalized)?.ok_or_else(|| invalid("只能重试讲后表达"))?;
    let conflict = || user_storage_paths::error("REQUEST_KEY_CONFLICT", "conflict", "同一表达不能改变原话、原讲解或提交身份");
    if normalized["retry_of"] != turn.turn_id || normalized["expected_chat_head"] != turn.turn_id
        || turn.domain.teach_back_attempts.first().is_none_or(|a| a.expression_id != attempt.expression_id || a == &attempt)
        || normalized.as_object().unwrap().iter().any(|(key,value)| {
            !matches!(key.as_str(), "operation_id" | "expected_chat_head" | "retry_of" | "attachment_id" | "generation" | "expected_revision" | "response_language") && original.get(key) != Some(value)
        })
        || original.as_object().unwrap().keys().any(|key| key != "response_language" && normalized.get(key).is_none()) {
        return Err(conflict());
    }
    if turn.domain.teach_back_attempts.contains(&attempt) { return Ok(true); }
    if !can_retry(turn, &attempt) { return Err(invalid("只能重试生成失败的原表达")); }
    Ok(false)
}

pub(crate) fn reset_failed(session: &mut AgentChatSession, attempt: ExplanationAttempt) {
    let turn = session.turns.last_mut().unwrap();
    if let Some(length) = turn.domain.teach_back_history_len.filter(|n| *n < session.messages.len()) {
        session.messages.truncate(length);
        if session.compaction_checkpoint.as_ref().is_some_and(|checkpoint|
            validate_persisted_checkpoint(&session.messages, checkpoint).is_err()) {
            session.compaction_checkpoint = None;
        }
    }
    turn.status = AgentAssistantStatus::PendingAssistant;
    turn.error = None;
    turn.run_summary = None;
    turn.outcome = None;
    turn.delivery_diagnostics = None;
    turn.source_bindings.clear();
    turn.domain.teach_back_attempts.push(attempt);
}

pub fn reopen(user: &mut user_runtime::UserRuntime, input: &Value, now: &str) -> Result<AgentTurnRef, ToolError> {
    let attempt = attempt(input)?.ok_or_else(|| invalid("缺少表达标识"))?;
    let session = user.agent_history.sessions.iter().find(|s| input["session_id"] == s.id)
        .ok_or_else(|| invalid("原表达不属于当前聊天"))?;
    let head = session.turns.last().filter(|turn| input["retry_of"] == turn.turn_id && can_retry(turn, &attempt))
        .ok_or_else(|| invalid("请在原聊天的失败表达上重试"))?;
    let reference = AgentTurnRef { session_id: session.id.clone(), turn_id: head.turn_id.clone(), user_turn_ordinal: head.user_turn_ordinal };
    if user.session_store.is_some() {
        session_runtime::append(user, &reference, now, session_event::EventBody::ExplanationRetried(attempt))?;
    } else {
        let mut candidate = user.agent_history.clone();
        let session = candidate.sessions.iter_mut().find(|s| s.id == reference.session_id).unwrap();
        reset_failed(session, attempt);
        session.updated_at = now.into();
        save_agent_history_path(&user.history_path, &candidate)?;
        user.agent_history = candidate;
    }
    Ok(reference)
}

/// Public eligibility uses the same cited bindings as admission. Source content and
/// revision checks occur when the reader actually submits the expression.
fn cited<'a>(book: &Book, turn: &'a AgentChatTurn) -> Option<Vec<&'a SourceBinding>> {
    if turn.status != AgentAssistantStatus::Completed || turn.video_question.is_some() || turn.music_question.is_some()
        || turn.domain.interaction.as_ref().is_some_and(|a| a.requests_practice())
        || turn.source_bindings.iter().any(|b| b.book_id != book.base.book_id || b.media.is_some() || b.music.is_some()
            || b.publication_id.as_ref().is_some_and(|id| turn.published_book_ref.as_ref().is_none_or(|p| &p.publication_id != id)))
        || turn.published_book_ref.as_ref().is_some_and(|p| p.book_id != book.base.book_id) {
        return None;
    }
    let outcome = turn.outcome.as_ref()?;
    let view = outcome.answer_view.as_ref()?;
    if outcome.incomplete || crate::observability::lifecycle::delivery_failed(outcome)
        || turn.delivery_diagnostics.as_ref().and_then(|d| d.repair.as_ref()).is_some_and(|d| !d.issues.is_empty())
        || view.sources.is_empty()
        || !view.parts.iter().any(|part| matches!(part, AgentAnswerPart::Markdown { text } if !text.trim().is_empty()))
        || view.parts.iter().any(|part| matches!(part, AgentAnswerPart::TeachBackComparison { .. } | AgentAnswerPart::MusicExample { .. })) {
        return None;
    }
    view.sources.iter().map(|source| turn.source_bindings.iter().find(|binding|
        binding.source_ref_id == source.source_ref_id && binding.book_id == book.base.book_id && binding.valid_evidence()
            && binding.evidence_range.is_some()
            && binding.publication_id.as_ref().is_none_or(|id| turn.published_book_ref.as_ref().is_some_and(|p| &p.publication_id == id))
            && binding.source_revision.as_deref().is_none_or(|revision| revision == book.source_fingerprint())
    )).collect()
}

pub(crate) fn eligible(book: &Book, turn: &AgentChatTurn) -> bool { cited(book, turn).is_some() }

pub(crate) fn resolve(user: &user_runtime::UserRuntime, workspace: &reader_workspace::ReaderWorkspace,
    input: &Value, reference: &AnswerRef, resolved: &mut ResolvedInteraction) -> Result<(), ToolError> {
    if media_purpose_scope(workspace, input).is_some() { return Err(invalid("讲后对照仅适用于普通文本书的讲解")); }
    let session = user.agent_history.sessions.iter().find(|s| Some(s.id.as_str()) == workspace.selected_chat.as_deref()).unwrap();
    let turn = session.turns.iter().find(|turn| turn.turn_id == reference.turn_id).unwrap();
    let bindings = cited(&workspace.book, turn).ok_or_else(|| invalid("请选择已交付、带有本书原文出处的讲解"))?;
    let store = user.learning_store_for_source(workspace.learning_workspace_id.as_deref(), &workspace.book.base.book_id)?;
    if store.closed_book(&session.id, &workspace.book.base.book_id)?.is_some_and(|v| matches!(v.status.as_str(), "active" | "paused")) {
        return Err(invalid("请先离开闭卷练习，再查看讲后对照"));
    }
    // Primary citations in historical answers store a digest but no full source
    // revision. Their accepted teaching binding pins the original publication.
    let original = teaching::bound_user(user, workspace.learning_workspace_id.as_deref(), &turn.turn_id)?;
    let book = &workspace.book;
    if input.get("source_id").filter(|v| !v.is_null()).is_some_and(|v| v != book.base.book_id.as_str())
        || input.get("source_revision").filter(|v| !v.is_null()).is_some_and(|v| v != book.source_fingerprint())
        || original.as_ref().is_some_and(|e| e.binding.source_id != book.base.book_id || e.binding.source_revision != book.source_fingerprint())
        || (original.is_none() && bindings.iter().any(|b| b.source_revision.is_none())
            && turn.domain.interaction.as_ref().is_none_or(|a| a.source_bindings.is_empty() || a.source_bindings.iter()
                .any(|b| b.source_id != book.base.book_id || b.source_revision != book.source_fingerprint())))
        || turn.published_book_ref.as_ref().zip(workspace.publication.as_ref()).is_some_and(|(p, current)| p != &current.reference) {
        return Err(user_storage_paths::error("INTERACTION_STALE", "conflict", "原讲解的来源版本已变化或无法核对"));
    }
    let mut sources = Vec::new();
    // The reader's selected paragraph is a more precise anchor than a long
    // delivered range, but only when its characters were actually cited.
    for selected in verified_question_evidence(book, turn.question_quote.as_ref()).iter().flat_map(|r| &r.ranges) {
        let covered = bindings.iter().filter_map(|b| b.evidence_range.as_ref()).any(|citation| {
            if citation.ranges.is_empty() {
                book.source_span(&citation.start_lid).zip(book.source_span(&citation.end_lid)).zip(book.source_span(&selected.lid))
                    .is_some_and(|((first, last), leaf)| first.start <= leaf.start && leaf.end <= last.end)
            } else {
                citation.ranges.iter().any(|r| r.lid == selected.lid && r.range.start <= selected.range.start && r.range.end >= selected.range.end)
            }
        });
        if covered && !sources.iter().any(|s: &TeachingSourceBinding| s.start_lid == selected.lid) {
            sources.push(TeachingSourceBinding { source_id: book.base.book_id.clone(), source_revision: book.source_fingerprint().into(),
                start_lid: selected.lid.clone(), end_lid: selected.lid.clone() });
        }
    }
    for binding in bindings {
        let range = binding.evidence_range.as_ref().unwrap();
        book.resolve_source(range, "zh-CN", Some(&binding.evidence_text_digest))
            .map_err(|_| invalid("原讲解引用的文字已无法核对"))?;
        let source = TeachingSourceBinding { source_id: book.base.book_id.clone(), source_revision: book.source_fingerprint().into(),
            start_lid: range.start_lid.clone(), end_lid: range.end_lid.clone() };
        if !sources.contains(&source) { sources.push(source); }
    }
    resolved.action = InteractionAction::UiEntry { entry_action: EntryAction::ExplainBack };
    let submitted = original.as_ref().and_then(|e| e.payload["current_request"].as_str())
        .unwrap_or_else(|| turn.user_authored_text());
    // The existing “teach me this part” button stored its full localized
    // instruction in the supplement. It supplies no concrete question.
    let guided_default = turn.domain.interaction.as_ref().is_some_and(|action|
        action.action == InteractionAction::UiEntry { entry_action: EntryAction::LearnWithPurpose }
            && action.learning_purpose == Some(LearningPurpose::Understand))
        && matches!(submitted,
            "我还没有具体困惑。请先从所选内容中选出一个值得理解的重点，说明它是什么、为什么重要，再带我逐步理解。"
            | "I do not have a specific question yet. Choose one key idea from the selected content, explain what it is and why it matters, then guide me through it.");
    resolved.accepted_request = if guided_default { String::new() } else { submitted.into() };
    resolve_reply(&store, input, &session.id, book, &turn.turn_id, None, resolved)?;
    resolved.answer_ref = Some(reference.clone());
    resolved.source_bindings = sources;
    Ok(())
}

fn resolve_reply(store: &memory::learning::LearningStore, input: &Value, chat: &str, book: &Book,
    origin: &str, course: Option<&memory::course::CourseRun>, resolved: &mut ResolvedInteraction) -> Result<(), ToolError> {
    if !input["teach_back_reply"].is_null() {
        let reply: TeachBackReply = serde_json::from_value(input["teach_back_reply"].clone())
            .map_err(|_| invalid("请选择对照中的关系行和纠正方式"))?;
        let parent = store.teaching_event(&reply.round_id)?;
        if parent.kind != memory::teaching::TeachingFact::TeachBackDelivered
            || parent.binding.chat_session_id != chat
            || parent.binding.source_id != book.base.book_id
            || parent.binding.source_revision != book.source_fingerprint()
            || parent.payload["origin_turn_id"] != origin {
            return Err(invalid("所选对照不属于这段讲解或当前来源版本"));
        }
        if store.teaching_event_sequence(&format!("display:{}", reply.round_id))?.is_none() {
            return Err(invalid("请先查看这轮对照，再针对其中一行回应"));
        }
        let card: TeachBackComparison = serde_json::from_value(parent.payload["comparison"].clone())
            .map_err(|e| invalid(&format!("已保存的对照无法读取：{e}")))?;
        if !card.rows.iter().any(|row| row.row_id == reply.row_id) {
            return Err(invalid("这轮对照没有所选关系行"));
        }
        if reply.kind == TeachBackReplyKind::FollowUp {
            // The main chat follows its head. The course form follows its latest reflection,
            // including after an intervening chat question or closed-book attempt.
            let current = match course {
                Some(run) => run.expressions.iter().rev().find(|e|e.activity == "reflection" && e.round_index == Some(run.round_index))
                    .is_some_and(|e|e.job_ref == reply.round_id),
                None => input["expected_chat_head"] == parent.binding.turn_id,
            };
            if card.follow_up.as_ref().is_none_or(|question| question.row_id != reply.row_id)
                || (input["retry_of"].is_null() && !current) {
                return Err(invalid("请回复当前对照的追问，或直接开始新的提问"));
            }
        }
        resolved.teach_back_reply = Some(reply);
    }
    Ok(())
}

/// Bind an explanation to the already released round before either model sees it.
pub(crate) fn resolve_course(user: &user_runtime::UserRuntime, workspace: &reader_workspace::ReaderWorkspace,
    input: &Value, resolved: &mut ResolvedInteraction) -> Result<(), ToolError> {
    use memory::course::{CourseExplanationOrigin, CourseStage, CourseStatus};
    let context = &input["course_context"];
    let lesson = &context["lesson"];
    let chat = workspace.selected_chat.as_deref().ok_or_else(||invalid("请先打开课程聊天"))?;
    if context["surface"] != "course" || media_purpose_scope(workspace,input).is_some() {
        return Err(invalid("请在本书课程的讲解步骤提交解释"));
    }
    let store = user.learning_store_for_workspace(workspace.learning_workspace_id.as_deref())?;
    let book = &workspace.book;
    if store.closed_book(chat, &book.base.book_id)?.is_some_and(|p|matches!(p.status.as_str(),"active"|"paused")) {
        return Err(invalid("请先离开闭卷练习，再查看讲后对照"));
    }
    // An explicit retry keeps the original binding even if a different tab has since advanced.
    if let Some(turn_id) = input["retry_of"].as_str() {
        if let Some(saved) = user.agent_history.sessions.iter().find(|s| s.id == chat)
            .and_then(|s| s.turns.iter().find(|t| t.turn_id == turn_id)).and_then(|t| t.domain.interaction.as_ref())
            .filter(|a| a.course.as_ref().is_some_and(|c| lesson["progress"]["run_id"] == c.run_id)) {
            *resolved = saved.clone();
            return Ok(());
        }
    }
    let run_id = lesson["progress"]["run_id"].as_str().ok_or_else(||invalid("请重新打开本课讲解"))?;
    let run = store.course_run(run_id)?;
    if run.progress.stage != CourseStage::Explanation || run.progress.status != CourseStatus::Active
        || run.support_visit.is_some() || context["view_revision"] != run.progress.revision
        || store.state()?.workspace_goal_revision.is_some_and(|revision| revision != run.progress.goal_revision) {
        return Err(invalid("课程现场已变化，请重新打开本轮讲解"));
    }
    let delivery = store.course_explanation_delivery(&run)?;
    let source = &run.lesson.summary.source;
    if input["source_id"].as_str().is_some_and(|id|id != book.base.book_id)
        || input["source_revision"].as_str().is_some_and(|id|id != book.source_fingerprint()) {
        return Err(invalid("课程来源已变化，请重新打开本轮讲解"));
    }
    resolved.action = InteractionAction::UiEntry { entry_action: EntryAction::ExplainBack };
    // Only the visible prompt identifies the task in phase one. The explanation and rubric stay out.
    resolved.accepted_request = lesson["reflection_prompt"].as_str().unwrap_or(&run.lesson.summary.question).to_owned();
    let binding = |first:&str,last:&str| TeachingSourceBinding { source_id:book.base.book_id.clone(),
        source_revision:book.source_fingerprint().into(),start_lid:first.into(),end_lid:last.into() };
    let span = book.source_span(&source.start_lid).zip(book.source_span(&source.end_lid))
        .ok_or_else(||invalid("本课讲解的来源已不可定位"))?;
    for lid in &run.lesson.assessment.source_lids {
        if book.source_span(lid).is_some_and(|p| span.0.start <= p.start && p.end <= span.1.end) {
            resolved.source_bindings.push(binding(lid,lid));
        }
    }
    resolved.source_bindings.push(binding(&source.start_lid,&source.end_lid));
    resolve_reply(&store,input,chat,book,&delivery.event_id,Some(&run),resolved)?;
    resolved.course = Some(CourseExplanationOrigin {run_id:run.progress.run_id, lesson_id:run.progress.lesson_id,
        stage:CourseStage::Explanation, round_index:run.round_index, delivery_ref:delivery.event_id});
    Ok(())
}
