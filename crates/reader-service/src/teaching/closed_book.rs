use super::*;
use memory::closed_book::{ClosedBookAnswerOrigin, ClosedBookCourseOrigin, ClosedBookPractice};

fn store(state: &PrivateBookContext<'_>) -> Result<memory::learning::LearningStore, ToolError> {
    state
        .user
        .learning_store_for_source(state.learning_workspace_id, &state.book.base.book_id)
}

pub(super) fn read(
    state: &PrivateBookContext<'_>,
    chat: &str,
) -> Result<Option<ClosedBookPractice>, ToolError> {
    let saved = store(state)?.closed_book(chat, &state.book.base.book_id)?;
    Ok(saved)
}

fn applicable(
    state: &PrivateBookContext<'_>,
    value: &ClosedBookPractice,
) -> Result<bool, ToolError> {
    let store = store(state)?;
    if value.source_revision != state.book.source_fingerprint()
        || value.goal_revision != store.state()?.workspace_goal_revision
    {
        return Ok(false);
    }
    if value.course {
        let run = store.course_run(&value.owner_id)?;
        Ok(run.progress.status == memory::course::CourseStatus::Active
            && value.course_origin.as_ref().is_none_or(|origin| origin.matches_run(&run)))
    } else {
        Ok(store
            .state()?
            .sessions
            .get(&value.owner_id)
            .is_some_and(|s| s.status == TutorSessionStatus::Active))
    }
}

pub(super) fn active(
    state: &PrivateBookContext<'_>,
    chat: &str,
) -> Result<Option<ClosedBookPractice>, ToolError> {
    match read(state, chat)? {
        Some(value) if value.status == "active" && applicable(state, &value)? => Ok(Some(value)),
        _ => Ok(None),
    }
}

fn practice_question(turn: &AgentChatTurn) -> &str {
    let authored = turn.user_authored_text().trim();
    if !authored.is_empty() {
        return authored;
    }
    turn.domain.interaction.as_ref().map(|action| action.accepted_request.trim())
        .filter(|request| !request.is_empty()).unwrap_or("当前学习重点")
}

fn neutral_followup(focus: &str, english: bool) -> Option<&'static str> {
    match (focus, english) {
        ("apply_case", false) => Some("只看这道题给出的新情境，你会作出什么判断？请用其中一个条件说明理由。"),
        ("apply_case", true) => Some("What do you conclude in this new case? Use a detail from it to explain your judgment."),
        ("explain_reason", false) => Some("你是怎样得出这个判断的？请把题目里的条件和你的结论连起来。"),
        ("explain_reason", true) => Some("How did you reach that judgment? Connect a condition in the question to your conclusion."),
        _ => None,
    }
}

/// Only the display receipt counts. Re-entering after looking at the book
/// collects any additional comparisons opened while the practice was ended.
fn displayed_support(
    store: &memory::learning::LearningStore,
    value: &ClosedBookPractice,
) -> Result<Option<Vec<String>>, ToolError> {
    let origin = if let Some(answer) = &value.answer_origin {
        answer.turn_id.clone()
    } else if value.course_origin.as_ref().is_some_and(|origin| origin.stage == memory::course::CourseStage::Explanation) {
        let run = store.course_run(&value.owner_id)?;
        store.course_explanation_delivery(&run)?.event_id
    } else {
        return Ok(None);
    };
    let refs = store.displayed_teach_backs(&value.binding(""), &origin)?;
    Ok((!refs.is_empty()).then_some(refs))
}

/// Rehydrate the teacher side privately. The public practice projection only
/// contains delivery IDs; the same text is used for question design and review.
pub(super) fn displayed_reasoning(
    state: &PrivateBookContext<'_>,
    practice: &ClosedBookPractice,
) -> Result<String, ToolError> {
    let Some(refs) = &practice.support_delivery_refs else { return Ok(String::new()); };
    let store = store(state)?;
    let mut shown = Vec::new();
    for id in refs {
        let fact = store.teaching_event(id)?;
        let card: runtime::teach_back::TeachBackComparison =
            serde_json::from_value(fact.payload["comparison"].clone()).map_err(|e| invalid(e.to_string()))?;
        let graph = &card.book_graph;
        for node in &graph.nodes {
            let mut lines = vec![node.label.clone()];
            lines.extend(node.condition.iter().map(|condition| format!("Condition: {condition}")));
            lines.extend(node.evidence.iter().map(|quote| quote.quote.clone()));
            shown.push(lines.join("\n"));
        }
        for edge in &graph.edges {
            let label = |id: &str| graph.nodes.iter().find(|node| node.id == id).map(|node| node.label.as_str());
            let (Some(from), Some(to)) = (label(&edge.from), label(&edge.to)) else {
                return Err(invalid("已展示的书侧推理缺少连线端点"));
            };
            let relation = json!(edge.relation_type);
            let mut lines = vec![format!("{from} —{}→ {to}", relation.as_str().unwrap_or_default())];
            lines.extend(edge.condition.iter().map(|condition| format!("Condition: {condition}")));
            lines.extend(edge.evidence.iter().map(|quote| quote.quote.clone()));
            shown.push(lines.join("\n"));
        }
        for row in card.rows {
            let mut lines = vec![row.summary];
            lines.extend(row.divergence_condition);
            shown.push(lines.join("\n"));
        }
        for side in card.side_notes {
            let mut lines = vec![side.text];
            lines.extend(side.evidence.into_iter().map(|quote| quote.quote));
            shown.push(lines.join("\n"));
        }
        shown.extend(card.follow_up.map(|follow_up| follow_up.question));
    }
    let mut distinct = std::collections::HashSet::new();
    shown.retain(|text| distinct.insert(text.clone()));
    Ok(shown.join("\n"))
}

pub(super) fn route(
    state: &PrivateBookContext<'_>,
    input: &Value,
    now: &str,
) -> Result<Value, ToolError> {
    control(state, input, now, None)
}

fn control(
    state: &PrivateBookContext<'_>,
    input: &Value,
    now: &str,
    first_decision: Option<&Value>,
) -> Result<Value, ToolError> {
    let chat = input["chat_session_id"]
        .as_str()
        .ok_or_else(|| invalid("缺少练习所属对话"))?;
    let session = state
        .user
        .agent_history
        .sessions
        .iter()
        .find(|s| {
            s.id == chat && s.matches_scope(state.learning_workspace_id, &state.book.base.book_id)
        })
        .ok_or_else(|| invalid("请回到原学习对话"))?;
    let mut store = store(state)?;
    let previous = store.closed_book(chat, &state.book.base.book_id)?;
    let action = input["action"].as_str().unwrap_or("state");
    if action == "state" {
        let mut view = previous;
        if let Some(value) = &mut view {
            value.can_resume = value.completion_reason.is_empty() && applicable(state, value)?;
            if value.status == "active" && !value.can_resume {
                value.status = "paused".into();
            }
        }
        return Ok(json!(view));
    }
    let operation = input["operation_id"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| invalid("缺少练习操作身份"))?;
    // The write may already have committed even when its HTTP response was lost.
    if let Some(saved) = store.closed_book_operation(operation)? {
        if saved.chat_session_id != chat || saved.source_id != state.book.base.book_id {
            return Err(invalid("练习操作已用于另一目标"));
        }
        return Ok(json!(saved));
    }
    let revision = input["revision"].as_u64().unwrap_or(0) as u32;
    if previous.as_ref().map_or(0, |s| s.revision) != revision {
        return Err(memory::closed_book::revision_conflict());
    }
    let mut value = if action == "start" {
        if let Some(saved) = &previous {
            if saved.status == "active" && applicable(state, saved)? {
                return Ok(json!(saved));
            }
        }
        let course = input["course_run_id"].as_str();
        let requested = input["target"].as_str().unwrap_or("").trim();
        let answer_origin = if course.is_none() {
            input["answer_turn_id"].as_str().map(|id| -> Result<ClosedBookAnswerOrigin, ToolError> {
                let turn = session.turns.iter().find(|turn| turn.turn_id == id
                    && turn.status == AgentAssistantStatus::Completed)
                    .ok_or_else(|| invalid("请从已完成的讲解开始练习"))?;
                let outcome = turn.outcome.as_ref().filter(|outcome| !outcome.incomplete)
                    .ok_or_else(|| invalid("这段讲解尚未完整交付"))?;
                let answer = outcome.answer_view.as_ref().map(|view| view.parts.iter()
                    .filter_map(AgentAnswerPart::text).collect::<Vec<_>>().join("\n\n"))
                    .filter(|text| !text.trim().is_empty()).or_else(|| outcome.answer.clone())
                    .filter(|text| !text.trim().is_empty()).ok_or_else(|| invalid("这段回答没有可练习的讲解"))?;
                Ok(ClosedBookAnswerOrigin { turn_id: id.into(), question: practice_question(turn).into(), answer })
            }).transpose()?
        } else { None };
        let (owner, target, course_origin) = if let Some(id) = course {
            let run = store.course_run(id)?;
            if run.progress.status != memory::course::CourseStatus::Active
                || store
                    .state()?
                    .workspace_goal_revision
                    .is_some_and(|revision| revision != run.progress.goal_revision)
                || run.lesson.summary.source.publication.book_id != state.book.base.book_id
                || run.lesson.summary.source.source_revision != state.book.source_fingerprint()
            {
                return Err(invalid("请先恢复当前课程"));
            }
            let origin = ClosedBookCourseOrigin::from_run(&run);
            (id.into(), origin.question.clone(), Some(origin))
        } else {
            let target = if let Some(origin) = &answer_origin {
                origin.question.clone()
            } else if requested.is_empty() {
                session
                    .turns
                    .iter()
                    .rev()
                    .find(|t| t.status == AgentAssistantStatus::Completed)
                    .map(|t| practice_question(t).to_owned())
                    .unwrap_or_else(|| "当前学习重点".into())
            } else {
                requested.into()
            };
            let current = store.state()?;
            let owner = current
                .control
                .current_tutor_session_id
                .as_ref()
                .and_then(|id| current.sessions.get(id))
                .filter(|s| s.status == TutorSessionStatus::Active)
                .map(|s| s.id.clone());
            let owner = if let Some(id) = owner {
                id
            } else {
                let next = store.mutate(
                    &memory::learning::TutorMutation {
                        operation_id: format!("closed-book-start:{operation}"),
                        expected_revision: current.control.revision,
                        action: memory::learning::TutorAction::Start {
                            user_intent: target.clone(),
                            explicit_constraints: vec![],
                            material_scope: vec![memory::learning::TutorMaterial {
                                source_id: state.book.base.book_id.clone(),
                                scope_refs: vec![],
                                role: memory::learning::TutorMaterialRole::Primary,
                            }],
                            default_teaching_intent: None,
                            learning_purpose: None,
                        },
                    },
                    now,
                )?;
                next.control.current_tutor_session_id.unwrap()
            };
            (owner, target, None)
        };
        ClosedBookPractice {
            id: operation.into(),
            owner_id: owner,
            course: course.is_some(),
            chat_session_id: chat.into(),
            source_id: state.book.base.book_id.clone(),
            source_revision: state.book.source_fingerprint().into(),
            goal_revision: store.state()?.workspace_goal_revision,
            course_origin,
            answer_origin,
            support_delivery_refs: None,
            transfer_condition: None,
            result_ref: None,
            revision,
            status: "active".into(),
            can_resume: true,
            target,
            criteria: String::new(),
            attempt: 1,
            assisted: false,
            needs_retry: false,
            visible_turn_ids: vec![],
            draft: String::new(),
            completion_reason: String::new(),
            learner_quote: String::new(),
        }
    } else {
        previous.ok_or_else(|| invalid("尚未开始闭卷练习"))?
    };
    match action {
        "start" => {
            value.support_delivery_refs = displayed_support(&store, &value)?;
            if let Some(decision) = first_decision.filter(|_| value.requires_transfer()) {
                validate_transfer_case(&value, &displayed_reasoning(state, &value)?, decision)?;
            }
        }
        "pause" => {
            value.status = "paused".into();
        }
        "exit" => {
            value.status = "ended".into();
            value.needs_retry = !value.criteria.is_empty() && value.completion_reason.is_empty();
        }
        "resume" => {
            if !value.completion_reason.is_empty() || !applicable(state, &value)? {
                return Err(invalid("请为当前内容开始新练习"));
            }
            if value.status == "ended" {
                value.support_delivery_refs = displayed_support(&store, &value)?;
            }
            value.status = "active".into();
        }
        "draft" => {
            value.draft = input["draft"].as_str().unwrap_or("").into();
        }
        "help" => {
            if value.status != "active" {
                return Err(invalid("请先恢复练习"));
            }
            value.assisted = true;
        }
        _ => return Err(invalid("不支持的练习操作")),
    }
    if let Some(draft) = input["draft"].as_str() {
        value.draft = draft.into();
    }
    Ok(json!(store.save_closed_book(value, operation, now)?))
}

pub(crate) const CLOSED_BOOK_DECISION_INVALID: &str = "CLOSED_BOOK_DECISION_INVALID";

// Only model-authored proposals rejected before persistence can be revised.
pub(crate) fn invalid_decision(message: &str) -> ToolError {
    crate::user_storage_paths::error(CLOSED_BOOK_DECISION_INVALID, "validation", message)
}

fn previously_shown(practice: &ClosedBookPractice, displayed: &str, quote: &str) -> bool {
    practice.answer_origin.as_ref().is_some_and(|o| o.question.contains(quote) || o.answer.contains(quote))
        || practice.course_origin.as_ref().is_some_and(|o| o.question.contains(quote))
        || (!displayed.is_empty() && displayed.contains(quote))
}

fn validate_transfer_case<'a>(
    practice: &ClosedBookPractice, displayed: &str, decision: &'a Value,
) -> Result<&'a str, ToolError> {
    let condition = decision["new_case"].as_str().unwrap_or("").trim();
    let plain = condition.replace("**", "");
    let premise = plain.trim_start_matches('>').trim();
    if premise.is_empty() || Some(premise) == decision["text"].as_str().map(str::trim)
        || previously_shown(practice, displayed, premise)
        || practice.transfer_condition.as_deref() == Some(condition) {
        return Err(invalid_decision("请在 new_case 单独给出需要重新判断的完整新情境，在 text 写要回答的问题。"));
    }
    Ok(condition)
}

pub(super) fn propose(
    state: &PrivateBookContext<'_>,
    binding: &TeachingBinding,
    request: &Value,
    now: &str,
) -> Result<Value, ToolError> {
    let decision = &request["closed_book"];
    let kind = decision["action"].as_str().unwrap_or("");
    let text = decision["text"]
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| invalid_decision("text 需要填写交付给学习者的问题或反馈"))?;
    let learner = state
        .user
        .agent_history
        .sessions
        .iter()
        .find(|s| s.id == binding.chat_session_id)
        .and_then(|s| s.turns.iter().find(|t| t.turn_id == binding.turn_id))
        .ok_or_else(|| invalid("缺少本轮用户输入"))?;
    if !matches!(
        kind,
        "start"
            | "clarify"
            | "question"
            | "followup"
            | "help"
            | "retry"
            | "complete"
            | "pause"
            | "exit"
    ) {
        return Err(invalid_decision("请选择追问、帮助、独立重试、完成或离开"));
    }
    let current = if kind == "start" {
        let quote = decision["learner_quote"].as_str().unwrap_or("").trim();
        if quote.is_empty() || !learner.user_authored_text().contains(quote) {
            return Err(invalid(
                "开启需要本轮明确要求闭卷练习的用户原话、目标与完成条件",
            ));
        }
        if active(state, &binding.chat_session_id)?.is_some() {
            return Err(invalid("已经开启，请接续当前尝试"));
        }
        let previous = read(state, &binding.chat_session_id)?;
        let bound = bound(state, &binding.turn_id)?.ok_or_else(|| invalid("缺少当前教学现场"))?;
        let course = &bound.payload["inquiry_course"]["lesson"]["progress"]["run_id"];
        serde_json::from_value(control(state,&json!({"action":"start","chat_session_id":binding.chat_session_id,
            "operation_id":format!("start:{}",binding.turn_id),"revision":previous.map_or(0,|v|v.revision),
            "course_run_id":course,"target":decision["target"]}),now,Some(decision))?).map_err(|e|invalid(e.to_string()))?
    } else {
        active(state, &binding.chat_session_id)?.ok_or_else(|| invalid("闭卷练习已暂停"))?
    };
    let (english, shown) = if kind != "start" {
        let frozen = bound(state, &binding.turn_id)?.ok_or_else(|| invalid("缺少当前教学现场"))?;
        if frozen.payload["closed_book"]["id"] != current.id
            || frozen.payload["closed_book"]["revision"] != current.revision
        {
            return Err(invalid("练习已切换或暂停，请按当前尝试继续"));
        }
        (frozen.payload["response_language"] == "en",
            frozen.payload["closed_book"]["displayed_reasoning"].as_str().unwrap_or_default().to_owned())
    } else { (false, String::new()) };
    let leaving = matches!(kind, "pause" | "exit");
    if kind == "clarify" && !current.criteria.is_empty() {
        return Err(invalid_decision("目标已确定，请沿当前目标追问，或结束后再改变目标"));
    }
    if current.criteria.is_empty()
        && !leaving
        && !matches!(kind, "clarify" | "start")
        && ((!current.requires_transfer() && decision["criteria"]
            .as_str()
            .is_none_or(|s| s.trim().is_empty()))
            || !matches!(kind, "question" | "start"))
    {
        return Err(invalid_decision("首问需要确定具体目标和完成条件"));
    }
    if !current.criteria.is_empty() && kind == "question" {
        return Err(invalid_decision("已有目标，请继续追问或使用 retry 开始同目标新尝试"));
    }
    if current.needs_retry && !matches!(kind, "retry" | "help" | "pause" | "exit") {
        return Err(invalid_decision("离开查阅后需要同目标新尝试"));
    }
    if kind == "retry" && decision["provides_help"] == true {
        return Err(invalid_decision("独立重试只提供新题必需条件，不能同时给出解法"));
    }
    let unverified_question = current.requires_transfer() && current.transfer_condition.is_none()
        && !current.visible_turn_ids.is_empty() && !current.criteria.is_empty();
    if kind == "retry" && !current.assisted && !current.needs_retry && !unverified_question {
        let quote = decision["learner_quote"].as_str().unwrap_or("").trim();
        if quote.is_empty() || !learner.user_authored_text().contains(quote) {
            return Err(invalid_decision("本次尝试未受助。先前尝试的帮助不影响本次独立性；回答充分时应完成，有必要缺口则追问。只有本轮用户要求换题时才能 retry，并引用该请求原话。"));
        }
    }
    let mut accepted = decision.clone();
    let mut transfer_condition = None;
    let mut requires_new_attempt = false;
    if current.requires_transfer() {
        if matches!(kind, "question" | "retry" | "start") {
            // The spoken start validates before saving the start receipt.
            let condition = if kind == "start" { decision["new_case"].as_str().unwrap_or("").trim() }
                else { validate_transfer_case(&current, &shown, decision)? };
            // Both the learner and the assessor get the case as supplied in one place.
            accepted["text"] = json!(runtime::closed_book_review::displayed_question(condition, text.trim()));
            accepted["criteria"] = json!(text.trim());
            transfer_condition = Some(condition.to_owned());
        } else if kind == "followup" && decision["provides_help"] != true {
            if let Some(prompt) = decision["followup_focus"].as_str().and_then(|focus| neutral_followup(focus, english)) {
                accepted["text"] = json!(prompt);
            } else {
                // A bespoke correction can disclose a decisive connection, even with a false help flag.
                accepted["provides_help"] = json!(true);
            }
        } else if kind == "complete" {
            if current.transfer_condition.is_none() {
                return Err(invalid_decision("这道题未记录新情境，先用 retry 出一道同目标变式，再判断独立完成。"));
            }
            let quote = decision["learner_quote"].as_str().unwrap_or("").trim();
            if !current.assisted && previously_shown(&current, &shown, quote) {
                return Err(invalid_decision("请对照本题的 transfer_condition，引用学习者运用新条件所得的判断。复述刚讲过的答案时应追问该条件如何改变结论。"));
            }
        }
    }
    if kind == "complete" {
        let quote = decision["learner_quote"].as_str().unwrap_or("").trim();
        if (current.assisted && !current.requires_transfer())
            || current.visible_turn_ids.is_empty()
            || decision["provides_help"] == true
            || quote.is_empty()
            || !learner.user_authored_text().contains(quote)
            || decision["reason"]
                .as_str()
                .is_none_or(|s| s.trim().is_empty())
        {
            return Err(invalid_decision(
                "完成需要当前独立回答的确切原话与理由；受助尝试请先用 retry 换一个未被代答的问题",
            ));
        }
        if current.assisted {
            // A valid learner quote can still repeat the correction just given.
            accepted["action"] = json!("followup");
            accepted["text"] = json!(if english {
                "This answer is recorded with the help you received. You can try a new case independently when you are ready."
            } else {
                "这次作答已和刚才获得的帮助一起保留。准备好后，可以换一道新题独立试一次。"
            });
            accepted["provides_help"] = json!(true);
        } else if let Some(case) = current.transfer_condition.as_ref().filter(|_| current.requires_transfer()) {
            let review = request.get("transfer_review").ok_or_else(|| crate::user_storage_paths::error(
                runtime::closed_book_review::REQUIRED, "validation", "此作答需要对照刚讲过的答案与当前变式核实独立性。"))?;
            let question = runtime::closed_book_review::displayed_question(case, &current.criteria);
            let verdict = runtime::closed_book_review::verified(review, &question, learner.user_authored_text());
            if verdict.is_ok() {
                accepted["transfer_comparison"] = review["comparison"].clone();
                if review["learner_quote"].is_string() { accepted["learner_quote"] = review["learner_quote"].clone(); }
                accepted["reason"] = review["reason"].clone();
            }
            match verdict {
                Ok("independent") => {}
                Ok("repeated_task") => {
                    accepted["action"] = json!("followup");
                    accepted["provides_help"] = json!(true);
                    accepted["text"] = json!(if english {
                        "The previous explanation already gave the answer to this question. We can try a different case to see how you apply it independently."
                    } else {
                        "刚才的讲解已经给出了这道题的答案。这次按有讲解帮助记录。可以换一道需要重新判断的题，再独立试一次。"
                    });
                }
                Ok("wrong_target") => {
                    requires_new_attempt = true;
                    accepted["action"] = json!("followup");
                    accepted["provides_help"] = json!(false);
                    accepted["text"] = json!(if english {
                        "This question tested a different topic. Your response is saved. Start a new attempt to practice the point from the original explanation."
                    } else {
                        "这道题偏离了刚才讲解的重点。你的作答已保留，可以重新出题，练习原先选中的内容。"
                    });
                }
                Ok(_) => {
                    accepted["action"] = json!("followup");
                    accepted["provides_help"] = json!(false);
                    accepted["text"] = json!(neutral_followup("apply_case", english));
                }
                Err(_) => {
                    accepted["action"] = json!("followup");
                    accepted["provides_help"] = json!(false);
                    accepted["reason"] = json!("Independent assessment remains unconfirmed.");
                    accepted["text"] = json!(if english {
                        "Your answer is saved. I couldn’t confirm independent completion this time. You can explain how the facts in this case support your answer, or ask for a new case."
                    } else {
                        "你的作答已保留。这次还没能确认独立完成。可以再说说本题中的条件怎样支持你的判断，也可以要求换一道题。"
                    });
                }
            }
        }
    }
    let id = format!("closed-book-proposal:{}", binding.turn_id);
    append(
        state,
        &current.binding(&binding.turn_id),
        id.clone(),
        TeachingFact::ClosedBookProposed,
        vec![],
        json!({"practice_id":current.id, "transfer_condition":transfer_condition, "requires_new_attempt":requires_new_attempt,
        "revision":current.revision,"decision":accepted,"response":learner.user_authored_text()}),
        now,
    )?;
    Ok(
        json!({"accepted":true,"delivery_draft":{"text":accepted["text"],"moves":[{"kind":"explain","prompt":accepted["text"],"actions":[]}],"delivery_move_refs":[]}}),
    )
}

pub(super) fn delivered(
    user: &crate::user_runtime::UserRuntime,
    workspace: Option<&str>,
    binding: &TeachingBinding,
    now: &str,
) -> Result<(), ToolError> {
    let mut store = user.learning_store_for_workspace(workspace)?;
    let Ok(proposal) = store.teaching_event(&format!("closed-book-proposal:{}", binding.turn_id))
    else {
        return Ok(());
    };
    if store
        .teaching_event(&format!("closed-book:delivery:{}", binding.turn_id))
        .is_ok()
    {
        return Ok(());
    }
    let Some(mut value) = store.closed_book(&binding.chat_session_id, &binding.source_id)? else {
        return Ok(());
    };
    if value.status != "active"
        || proposal.payload["practice_id"] != value.id
        || proposal.payload["revision"] != value.revision
    {
        return Ok(());
    }
    let view = store.state()?;
    if value.goal_revision != view.workspace_goal_revision {
        return Ok(());
    }
    let owner_active = if value.course {
        let run = store.course_run(&value.owner_id)?;
        run.progress.status == memory::course::CourseStatus::Active
            && value.course_origin.as_ref().is_none_or(|origin| origin.matches_run(&run))
    } else {
        view.sessions
            .get(&value.owner_id)
            .is_some_and(|s| s.status == TutorSessionStatus::Active)
    };
    if !owner_active {
        return Ok(());
    }
    let decision = &proposal.payload["decision"];
    let kind = decision["action"].as_str().unwrap_or("");
    if matches!(kind, "question" | "start") {
        value.visible_turn_ids.clear();
        value.target = decision["target"]
            .as_str()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(&value.target)
            .into();
        value.criteria = decision["criteria"].as_str().unwrap_or("").into();
    }
    if kind == "retry" {
        value.attempt += 1;
        value.visible_turn_ids.clear();
        value.assisted = false;
        value.needs_retry = false;
    }
    if value.requires_transfer() && matches!(kind, "question" | "retry" | "start") {
        value.transfer_condition = proposal.payload["transfer_condition"].as_str().map(str::to_owned);
        value.criteria = decision["criteria"].as_str().unwrap_or("").into();
    }
    value.assisted |= kind == "help" || decision["provides_help"] == true;
    if proposal.payload["requires_new_attempt"] == true { value.needs_retry = true; }
    value.visible_turn_ids.push(binding.turn_id.clone());
    if kind == "retry"
        || proposal.payload["response"]
            .as_str()
            .is_some_and(|text| text.trim() == value.draft.trim())
    {
        value.draft.clear();
    }
    if kind == "complete" {
        value.status = "complete".into();
        value.result_ref = Some(format!("closed-book:delivery:{}", binding.turn_id));
        value.completion_reason = decision["reason"].as_str().unwrap_or("").into();
        value.learner_quote = decision["learner_quote"].as_str().unwrap_or("").into();
    }
    if kind == "pause" {
        value.status = "paused".into();
    }
    if kind == "exit" {
        value.status = "ended".into();
        value.needs_retry = !value.criteria.is_empty();
    }
    store.save_closed_book(value, &format!("delivery:{}", binding.turn_id), now)?;
    Ok(())
}
