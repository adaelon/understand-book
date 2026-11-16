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

pub(super) fn route(
    state: &PrivateBookContext<'_>,
    input: &Value,
    now: &str,
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
                Ok(ClosedBookAnswerOrigin { turn_id: id.into(), question: turn.user_authored_text().into(), answer })
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
                    .map(|t| t.user_authored_text().to_owned())
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
        "start" => {}
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
        .ok_or_else(|| invalid("缺少交付给学习者的问题或反馈"))?;
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
        return Err(invalid("请选择追问、帮助、独立重试、完成或离开"));
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
        serde_json::from_value(route(state,&json!({"action":"start","chat_session_id":binding.chat_session_id,
            "operation_id":format!("start:{}",binding.turn_id),"revision":previous.map_or(0,|v|v.revision),
            "course_run_id":course,"target":decision["target"]}),now)?).map_err(|e|invalid(e.to_string()))?
    } else {
        active(state, &binding.chat_session_id)?.ok_or_else(|| invalid("闭卷练习已暂停"))?
    };
    if kind != "start" {
        let frozen = bound(state, &binding.turn_id)?.ok_or_else(|| invalid("缺少当前教学现场"))?;
        if frozen.payload["closed_book"]["id"] != current.id
            || frozen.payload["closed_book"]["revision"] != current.revision
        {
            return Err(invalid("练习已切换或暂停，请按当前尝试继续"));
        }
    }
    let leaving = matches!(kind, "pause" | "exit");
    if kind == "clarify" && !current.criteria.is_empty() {
        return Err(invalid("目标已确定，请沿当前目标追问，或结束后再改变目标"));
    }
    if current.criteria.is_empty()
        && !leaving
        && !matches!(kind, "clarify" | "start")
        && (decision["criteria"]
            .as_str()
            .is_none_or(|s| s.trim().is_empty())
            || !matches!(kind, "question" | "start"))
    {
        return Err(invalid("首问需要确定具体目标和完成条件"));
    }
    if !current.criteria.is_empty() && kind == "question" {
        return Err(invalid("已有目标，请继续追问或使用 retry 开始同目标新尝试"));
    }
    if current.needs_retry && !matches!(kind, "retry" | "help" | "pause" | "exit") {
        return Err(invalid("离开查阅后需要同目标新尝试"));
    }
    if kind == "retry" && decision["provides_help"] == true {
        return Err(invalid("独立重试只提供新题必需条件，不能同时给出解法"));
    }
    if kind == "retry" && !current.assisted && !current.needs_retry {
        let quote = decision["learner_quote"].as_str().unwrap_or("").trim();
        if quote.is_empty() || !learner.user_authored_text().contains(quote) {
            return Err(invalid("本次尝试未受助。先前尝试的帮助不影响本次独立性；回答充分时应完成，有必要缺口则追问。只有本轮用户要求换题时才能 retry，并引用该请求原话。"));
        }
    }
    if kind == "complete" {
        let quote = decision["learner_quote"].as_str().unwrap_or("").trim();
        if current.assisted
            || current.visible_turn_ids.is_empty()
            || decision["provides_help"] == true
            || quote.is_empty()
            || !learner.user_authored_text().contains(quote)
            || decision["reason"]
                .as_str()
                .is_none_or(|s| s.trim().is_empty())
        {
            return Err(invalid(
                "完成需要当前独立回答的确切原话与理由；受助尝试请先用 retry 换一个未被代答的问题",
            ));
        }
    }
    let id = format!("closed-book-proposal:{}", binding.turn_id);
    append(
        state,
        &current.binding(&binding.turn_id),
        id.clone(),
        TeachingFact::ClosedBookProposed,
        vec![],
        json!({"practice_id":current.id,
        "revision":current.revision,"decision":decision,"response":learner.user_authored_text()}),
        now,
    )?;
    Ok(
        json!({"accepted":true,"delivery_draft":{"text":text,"moves":[{"kind":"explain","prompt":text,"actions":[]}],"delivery_move_refs":[]}}),
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
    value.assisted |= kind == "help" || decision["provides_help"] == true;
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
