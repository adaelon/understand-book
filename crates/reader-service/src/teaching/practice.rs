//! Host checks and help release for retrieval practice (ADR-0178). The model
//! review judges soundness and shortcuts; these checks keep the contract,
//! pointers and assistance accounting exact.
use super::*;
use runtime::practice::{check_progression, level, named_target, PracticeEntry, PracticeHintKind, PracticeOutcome, PracticeSpec};

fn invalid(message: impl Into<String>) -> TeachingSelectionPreflightError {
    memory::teaching::invalid(message).into()
}

/// The saved idea at its exact revision; absence or a stale reference rejects the move.
fn saved_idea(state: &PrivateBookContext<'_>, reference: &memory::NoteRef) -> Result<String, TeachingSelectionPreflightError> {
    let view = state.user.store.personal_note(reference)
        .map_err(|_| invalid("练习引用的笔记版本不存在或不可用"))?;
    Ok(view.value.record.content)
}

pub(super) fn validate_select(
    state: &PrivateBookContext<'_>,
    current_action: &Value,
    movement: &TeachingMove,
    object_labels: &[String],
    ranges: &[EvidenceRange],
) -> Result<(), TeachingSelectionPreflightError> {
    let requested = serde_json::from_value::<runtime::tutor_continuation::ResolvedInteraction>(current_action.clone())
        .is_ok_and(|action| action.requests_practice());
    let Some(practice) = &movement.practice else {
        if requested && movement.kind == "question" {
            return Err(invalid("练习请求中的题目须附 move.practice：私有解答、评分合同与逐级提示"));
        }
        return Ok(());
    };
    practice.validate()?;
    let actions = |name: &str| movement.actions.iter().any(|a| a == name);
    if movement.kind != "question" || movement.assessment.is_none()
        || movement.explanation.as_deref().is_none_or(|s| s.trim().is_empty())
        || !actions("submit") || !actions("revise") || !actions("request_hint") || !actions("reveal") || !actions("skip") {
        return Err(invalid("练习题须为 kind=question，带冻结评分、私有完整解答，并允许提交、改答、提示、查看解答与跳过"));
    }
    let rule = movement.assessment.as_ref().map(|a| &a.rule);
    match practice.entry {
        // Card assembly is graded deterministically: the chosen card set must equal the premises.
        PracticeEntry::Board => match rule {
            Some(ScoringRule::ChoiceSet { choices, correct })
                if choices.iter().map(String::as_str).collect::<std::collections::BTreeSet<_>>()
                    == practice.cards.iter().map(|c| c.id.as_str()).collect()
                && correct.iter().map(String::as_str).collect::<std::collections::BTreeSet<_>>() == practice.premise_ids() => {}
            _ => return Err(invalid("画板组装题的评分须为 choice_set：选项为全部卡片，正确答案恰为前提卡片")),
        },
        // The learner's submitted page state is the answer; it needs an open rubric over that state.
        PracticeEntry::Presentation => if movement.presentation.is_none() || !matches!(rule, Some(ScoringRule::Open { .. })) {
            return Err(invalid("教具目标题须引用本轮已交付的演示版本，并以开放评分标准判断提交的页面状态"));
        },
        _ => {}
    }
    if movement.hint.is_some() || movement.continuation.is_some() || movement.response_expectation.is_some() {
        return Err(invalid("练习题的提示只放在 practice.hints；不附普通邀请或下一步菜单，作答后再决定后续"));
    }
    let public = [movement.prompt.as_str(), movement.delivery_text.as_deref().unwrap_or("")];
    let mut labels: Vec<&str> = object_labels.iter().map(String::as_str).collect();
    if let Some(target) = &movement.target { labels.push(&target.learning_focus); }
    if let Some(label) = named_target(&public, &labels) {
        return Err(invalid(format!("练习题面不得点名所练关系：公开文字包含“{label}”。请改为只呈现新情境")));
    }
    if let Some(reference) = &practice.revisit_note {
        let idea = saved_idea(state, reference)?;
        let idea = idea.trim();
        // Long notes may be quoted in part; a short saved idea must appear verbatim.
        if idea.chars().count() <= 200 && !public.iter().any(|text| text.contains(idea)) {
            return Err(invalid("旧想法回访须在题面逐字引用该笔记版本的原话"));
        }
    }
    for hint in &practice.hints {
        if let Some(reference) = &hint.note_ref { saved_idea(state, reference)?; }
        for lid in &hint.source_lids { preflight_target_read(state, lid, lid, ranges)?; }
    }
    validate_progression(state, movement, practice)
}

/// Selection rules over earlier problems on this material (ADR-0178 §6): interleave,
/// switch after two misses, follow the difficulty readout, use a fresh situation.
fn validate_progression(state: &PrivateBookContext<'_>, movement: &TeachingMove, practice: &PracticeSpec) -> Result<(), TeachingSelectionPreflightError> {
    let target = movement.target.as_ref().ok_or_else(|| invalid("练习须明确所练关系 move.target"))?;
    let records = history(state, &state.book.base.book_id, 30)?;
    // A chapter can support several relationships. Shared source ranges alone
    // do not identify which relationship a problem practices.
    let overlaps = |record: &PracticeRecord| record.target.as_ref().is_some_and(|saved|
        targets_overlap(state, saved, target)
            && (normalized(&saved.learning_focus) == normalized(&target.learning_focus)
                || saved.object_refs.iter().any(|reference| target.object_refs.contains(reference))));
    if practice.entry != PracticeEntry::NoteRevisit && records.first().is_some_and(&overlaps) {
        return Err(invalid("上一道练习刚练过这个关系；请换一个关系（交错练习）"));
    }
    let same: Vec<&PracticeRecord> = records.iter().filter(|r| overlaps(r)).collect();
    let concluded: Vec<&&PracticeRecord> = same.iter().filter(|r| r.outcome.scored()).collect();
    if concluded.len() >= 2 && concluded[..2].iter().all(|r| matches!(r.outcome, PracticeOutcome::Missed | PracticeOutcome::Partial)) {
        return Err(invalid("这个关系连续两次未答对；先换一个关系，需要时转为讲解"));
    }
    if let (Some(previous), Some(distance)) = (concluded.first(), practice.distance) {
        if let Some(previous_level) = previous.level {
            check_progression(previous_level, previous.outcome, level(practice.mode, distance))?;
        }
    }
    let prompt = normalized(&movement.prompt);
    if same.iter().any(|r| normalized(r.delivery.payload["move"]["prompt"].as_str().unwrap_or_default()) == prompt) {
        return Err(invalid("重练同一关系须换一个新情境，不能重复原题"));
    }
    Ok(())
}

fn normalized(text: &str) -> String { text.split_whitespace().collect::<Vec<_>>().join(" ") }

/// An explicit application request promises a fresh case in the learner-visible question.
pub(super) fn validate_application<'a>(context: &'a Value, movement: &TeachingMove, sources: impl Iterator<Item=&'a str>) -> Result<(), TeachingSelectionPreflightError> {
    if context["application_requested"] != true { return Ok(()); }
    let prompt = movement.prompt.trim();
    let case = movement.practice.as_ref().and_then(|p| p.application_situation.as_deref())
        .map(str::trim).filter(|text| !text.is_empty() && prompt.contains(*text) && prompt != *text)
        .ok_or_else(|| invalid("应用题须在 move.prompt 中给出新情境，并就它提出问题。在 practice.application_situation 逐字引用情境条件，保留让读者决定的问题，再提交完整题目。"))?;
    let original = &context["current_action"];
    let case = normalized(case);
    if sources.chain(original["source_excerpt"].as_str()).chain(original["answer_text"].as_str())
        .chain(crate::course_context::application_material(&context["inquiry_course"]["lesson"]))
        .any(|text| normalized(text).contains(&case)) {
        return Err(invalid("应用题的情境与已读原文或先前讲解重复。请用新的具体条件，让学习者自己运用同一关系，再提交题目。"));
    }
    Ok(())
}

/// One delivered practice problem as later selection sees it.
pub(super) struct PracticeRecord {
    pub delivery: TeachingEvent,
    pub target: Option<TeachingTarget>,
    pub level: Option<u8>,
    pub outcome: PracticeOutcome,
    pub skipped: bool,
    pub solution_seen: bool,
    pub hints_used: Vec<PracticeHintKind>,
    pub check: Option<String>,
}

/// Newest first. Outcomes come from the actual facts; a problem whose check found
/// it questionable says nothing about the learner and counts as open.
pub(super) fn history(state: &PrivateBookContext<'_>, source: &str, limit: u32) -> Result<Vec<PracticeRecord>, ToolError> {
    let store = state.user.learning_store_for_workspace(state.learning_workspace_id)?;
    let mut records = Vec::new();
    for delivery in store.practice_deliveries(source, limit)? {
        let (mut shown, mut revealed, mut skipped) = (Vec::new(), false, false);
        let mut outcome = PracticeOutcome::Open;
        let mut hints_at_success = None;
        for fact in store.teaching_activity_events(&delivery.event_id)? {
            match fact.kind {
                TeachingFact::HelpDisplayed => match serde_json::from_value::<PracticeHintKind>(fact.payload["hint_kind"].clone()) {
                    Ok(kind) => shown.push(kind),
                    Err(_) => revealed |= fact.payload["action"] == "reveal",
                },
                TeachingFact::LearnerAction => match fact.payload["action"].as_str() {
                    Some("submit" | "revise") if hints_at_success.is_none() => match store.assessment(&fact.event_id)?.map(|assessment| assessment.status) {
                        Some(AssessmentStatus::Correct) => {
                            outcome = PracticeOutcome::correct(&shown, revealed);
                            hints_at_success = Some(shown.clone());
                        }
                        Some(AssessmentStatus::Incorrect) => outcome = PracticeOutcome::Missed,
                        Some(AssessmentStatus::Partial) => outcome = PracticeOutcome::Partial,
                        _ => {}
                    },
                    Some("skip") => skipped = true,
                    _ => {}
                },
                _ => {}
            }
        }
        let check = store.practice_check_verdict(&delivery.event_id)?;
        if check.as_deref() == Some("questionable") { outcome = PracticeOutcome::Open; }
        else if outcome == PracticeOutcome::Open {
            if skipped { outcome = PracticeOutcome::Skipped; }
            else if revealed { outcome = PracticeOutcome::Revealed; }
        }
        let public = &delivery.payload["move"]["practice"];
        let level = serde_json::from_value(public["mode"].clone()).ok()
            .zip(serde_json::from_value(public["distance"].clone()).ok()).map(|(mode, distance)| level(mode, distance));
        let target = serde_json::from_value(delivery.payload["move"]["target"].clone()).unwrap_or(None);
        records.push(PracticeRecord { hints_used: hints_at_success.unwrap_or(shown), delivery, target, level, outcome, check, skipped, solution_seen: revealed });
    }
    Ok(records)
}

/// Bounded model view of recent practice on this material.
pub(super) fn history_view(state: &PrivateBookContext<'_>, chat: &str) -> Result<Value, ToolError> {
    Ok(json!(history(state, &state.book.base.book_id, 8)?.iter().map(|r| json!({
        "delivery_ref":r.delivery.event_id,"learning_focus":r.target.as_ref().map(|t| &t.learning_focus),
        "source_bindings":r.target.as_ref().map(|t| &t.source_bindings),"mode":r.delivery.payload["move"]["practice"]["mode"],
        "distance":r.delivery.payload["move"]["practice"]["distance"],"level":r.level,"outcome":r.outcome,
        "skipped":r.skipped,"solution_seen":r.solution_seen,"same_chat":r.delivery.binding.chat_session_id == chat,
        "hints_used":r.hints_used,"check":r.check,"occurred_at":r.delivery.occurred_at,
        "prompt":preview(&r.delivery.payload["move"]["prompt"],300)})).collect::<Vec<_>>()))
}

/// The next unreleased hint. A retried operation returns the hint it first released.
pub(super) fn next_hint(selected: &TeachingEvent, earlier: &[TeachingEvent], operation: &str) -> Result<Option<Value>, ToolError> {
    let Some(practice) = selected.payload["move"].get("practice").filter(|v| !v.is_null()) else { return Ok(None); };
    let practice: PracticeSpec = serde_json::from_value(practice.clone()).map_err(|e| memory::teaching::invalid(e.to_string()))?;
    let released: Vec<&TeachingEvent> = earlier.iter()
        .filter(|e| e.kind == TeachingFact::HelpDelivered && e.payload["action"] == "request_hint").collect();
    if let Some(previous) = released.iter().find(|e| e.event_id == format!("help:{operation}")) {
        return Ok(Some(previous.payload.clone()));
    }
    let hint = practice.next_hint(released.len()).ok_or_else(|| memory::teaching::invalid("这道题没有更多提示，可以查看解答"))?;
    Ok(Some(json!({"action":"request_hint","text":hint.text,"hint_kind":hint.kind,"hint_index":released.len(),
        "note_ref":hint.note_ref,"source_lids":hint.source_lids})))
}

/// The committed solution a correct answer releases. Later revisions count it as help.
pub(super) fn concluded_solution(selected: &TeachingEvent) -> Option<Value> {
    selected.payload["move"]["practice"].is_object().then_some(())?;
    let text = selected.payload["move"]["explanation"].as_str().filter(|s| !s.trim().is_empty())?;
    Some(json!({"action":"reveal","text":text,"reason":"concluded"}))
}

pub(super) fn release_concluded_solution(state: &PrivateBookContext<'_>, delivery: &TeachingEvent,
    action: &TeachingEvent, now: &str) -> Result<Option<Value>, ToolError> {
    let selected = event(state, &delivery.causal_refs[0])?;
    let Some(mut help) = concluded_solution(&selected) else { return Ok(None); };
    help["delivery_ref"] = json!(delivery.event_id);
    let operation = action.event_id.strip_prefix("action:").unwrap_or(&action.event_id);
    let offered = append(state, &delivery.binding, format!("help:{operation}"), TeachingFact::HelpDelivered,
        vec![action.event_id.clone()], help.clone(), now)?;
    help["event_id"] = json!(offered.event_id);
    Ok(Some(help))
}

/// Displayed help that surrenders solution information. Pointing at a given in
/// the problem does not, so it stays out of the frozen assistance conditions.
pub(super) fn is_assistance(help: &TeachingEvent) -> bool {
    serde_json::from_value::<PracticeHintKind>(help.payload["hint_kind"].clone())
        .map_or(true, PracticeHintKind::is_assistance)
}

/// Public move view: entry, mode and hint count, never hint texts.
pub(super) fn redact(public_move: &mut Value) {
    if let Some(practice) = public_move.get("practice").filter(|v| !v.is_null())
        .and_then(|v| serde_json::from_value::<PracticeSpec>(v.clone()).ok()) {
        public_move["practice"] = practice.public_view();
    }
}
