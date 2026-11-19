//! Explicit learning selections. Course, practice and chat records remain authoritative.
use crate::{
    reader_workspace::ReaderWorkspace, user_runtime::UserRuntime,
};
use base_schema::PublishedBookRef;
use memory::course::CourseStatus;
use read_tools::ToolError;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningTaskSelection {
    pub learning_workspace_id: Option<String>,
    pub published_book_ref: PublishedBookRef,
    pub source_revision: String,
    pub course_run_id: Option<String>,
    pub chat_session_id: Option<String>,
    #[serde(default)]
    pub closed_book_id: Option<String>,
    pub selected_at: i64,
    #[serde(default)]
    pub conversation_turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_question: Option<memory::NoteRef>,
    #[serde(default)]
    pub continue_conversation: bool,
}

impl LearningTaskSelection {
    pub fn same_target(&self, other: &Self) -> bool {
        self.learning_workspace_id == other.learning_workspace_id
            && self.published_book_ref == other.published_book_ref
            && if self.external_question.is_some() || other.external_question.is_some() {
                self.external_question == other.external_question
            } else if self.course_run_id.is_some() || other.course_run_id.is_some() {
                self.course_run_id == other.course_run_id
            } else {
                self.chat_session_id == other.chat_session_id
            }
    }
}

#[derive(Serialize)]
pub struct LearningTaskView {
    learning_workspace_id: Option<String>,
    published_book_ref: PublishedBookRef,
    course_run_id: Option<String>,
    chat_session_id: Option<String>,
    selected_at: i64,
    kind: &'static str,
    title: String,
    status: &'static str,
    lesson_id: Option<String>,
    stage: Option<String>,
    closed_book_id: Option<String>,
    question: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    note_ref: Option<memory::NoteRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_lid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_ref: Option<Value>,
    message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    course_context: Option<CourseTaskContext>,
}

#[derive(Serialize)]
pub struct CourseTaskContext {
    pub release_id: String,
    pub release_version: u32,
    pub collection_kind: &'static str,
    pub collection_title: String,
    pub unit_title: String,
    pub lesson_number: u32,
    pub lesson_count: u32,
}

impl LearningTaskView {
    fn new(task: &LearningTaskSelection) -> Self {
        Self {
            learning_workspace_id: task.learning_workspace_id.clone(),
            published_book_ref: task.published_book_ref.clone(),
            course_run_id: task.course_run_id.clone(),
            chat_session_id: task.chat_session_id.clone(),
            selected_at: task.selected_at,
            kind: if task.course_run_id.is_some() {
                "course"
            } else if task.closed_book_id.is_some() {
                "closed_book"
            } else {
                "conversation"
            },
            title: task.published_book_ref.book_id.clone(),
            status: "active",
            lesson_id: None,
            stage: None,
            closed_book_id: task.closed_book_id.clone(),
            question: None,
            note_ref: task.external_question.clone(),
            source_lid: None,
            source_ref: None,
            message: None,
            course_context: None,
        }
    }
    pub fn unavailable(mut self, message: &str) -> Self {
        self.status = "unavailable";
        self.message = Some(message.into());
        self
    }
}

fn invalid(message: &str) -> ToolError {
    crate::user_storage_paths::error("LEARNING_TASK_INVALID", "validation", message)
}

fn publication(workspace: &ReaderWorkspace) -> PublishedBookRef {
    workspace
        .publication
        .as_ref()
        .map(|p| p.reference.clone())
        .unwrap_or_else(|| PublishedBookRef {
            book_id: workspace.book.base.book_id.clone(),
            publication_id: "local".into(),
        })
}

pub fn selections(user: &UserRuntime) -> &[LearningTaskSelection] {
    user.session_store
        .as_ref()
        .map_or(&[], |store| store.learning_tasks.as_slice())
}

pub fn select(
    user: &mut UserRuntime,
    workspace: &ReaderWorkspace,
    input: &Value,
    now: &str,
) -> Result<Value, ToolError> {
    let scope = workspace.learning_workspace_id.as_deref();
    let reference = publication(workspace);
    let chat_id = input["chat_session_id"]
        .as_str()
        .or(workspace.selected_chat.as_deref());
    let chat = chat_id
        .map(|id| {
            user.agent_history
                .sessions
                .iter()
                .find(|s| s.id == id && s.matches_scope(scope, &reference.book_id))
                .ok_or_else(|| invalid("请回到任务原来的学习对话"))
        })
        .transpose()?;
    if chat_id.is_some() && chat_id != workspace.selected_chat.as_deref() {
        return Err(invalid("请先打开任务原来的学习对话"));
    }
    let store = user.learning_store_for_source(scope, &reference.book_id)?;
    let closed = chat
        .map(|chat| store.closed_book(&chat.id, &reference.book_id))
        .transpose()?
        .flatten();
    let closed = closed.filter(|value| {
        value.completion_reason.is_empty()
            && matches!(value.status.as_str(), "active" | "paused" | "ended")
            && value.source_revision == workspace.book.source_fingerprint()
    });
    let course_id = input["course_run_id"].as_str().or_else(|| {
        closed
            .as_ref()
            .filter(|value| value.course)
            .map(|value| value.owner_id.as_str())
    });
    if let Some(id) = course_id {
        let run = store.course_run(id)?;
        if run.lesson.summary.source.publication != reference
            || run.lesson.summary.source.source_revision != workspace.book.source_fingerprint()
            || workspace
                .learning_goal
                .as_ref()
                .is_some_and(|goal| goal.revision != run.progress.goal_revision)
            || matches!(
                run.progress.status,
                CourseStatus::Completed | CourseStatus::Skipped
            )
        {
            return Err(invalid("请先打开原课程中尚未完成的任务"));
        }
    }
    let latest = chat.and_then(|chat| chat.turns.last()).filter(|turn| {
        turn.published_book_ref
            .as_ref()
            .is_none_or(|r| r == &reference)
    });
    let continue_conversation = input["continue_conversation"].as_bool().unwrap_or(false);
    if course_id.is_none()
        && closed.is_none()
        && !latest.is_some_and(|turn| {
            continue_conversation
                || turn.socratic_question().is_some()
                || turn.status == crate::AgentAssistantStatus::PendingAssistant
        })
    {
        return Err(invalid("当前对话没有待继续的学习任务"));
    }
    let selection = LearningTaskSelection {
        learning_workspace_id: scope.map(str::to_owned),
        published_book_ref: reference,
        source_revision: workspace.book.source_fingerprint().to_owned(),
        course_run_id: course_id.map(str::to_owned),
        chat_session_id: chat_id.map(str::to_owned),
        closed_book_id: closed
            .as_ref()
            .filter(|value| !value.course || Some(value.owner_id.as_str()) == course_id)
            .map(|v| v.id.clone()),
        selected_at: now
            .parse()
            .unwrap_or_else(|_| crate::service_error::now()),
        external_question: None,
        conversation_turn_id: latest.map(|turn| turn.turn_id.clone()),
        continue_conversation,
    };
    let view = project(
        user,
        &selection,
        workspace.learning_goal.as_ref().map(|g| g.revision),
        |_| Ok(None),
    )?
    .ok_or_else(|| invalid("当前任务已结束"))?;
    if view.status == "unavailable" {
        return Err(invalid(
            view.message.as_deref().unwrap_or("当前任务不可继续"),
        ));
    }
    user.session_store
        .as_mut()
        .ok_or_else(|| {
            crate::user_storage_paths::error(
                "LEARNING_TASK_STORAGE_UNAVAILABLE",
                "unavailable",
                "当前学习现场没有持久对话存储",
            )
        })?
        .select_learning_task(&user.agent_history, selection)?;
    Ok(json!({ "task": view }))
}

pub fn local_list(
    user: &UserRuntime,
    workspace: &ReaderWorkspace,
) -> Result<Value, ToolError> {
    let reference = publication(workspace);
    let mut tasks = Vec::new();
    let mut external_visible = None;
    for selection in selections(user).iter().filter(|task| {
        task.learning_workspace_id == workspace.learning_workspace_id
            && task.published_book_ref == reference
    }) {
        if selection.external_question.is_some() {
            let visible = match external_visible {
                Some(visible) => visible,
                None => {
                    let scope = workspace.learning_workspace_id.as_deref();
                    let store = user.learning_store_for_workspace(scope)?;
                    let visible = store.open_closed_book_chats(scope.is_none().then_some(&reference.book_id))?.is_empty();
                    external_visible = Some(visible);
                    visible
                }
            };
            if !visible { continue; }
        }
        if let Some(view) = project(
            user,
            selection,
            workspace.learning_goal.as_ref().map(|goal| goal.revision),
            |_| Ok(None),
        )? {
            tasks.push(
                if selection.source_revision != workspace.book.source_fingerprint() {
                    view.unavailable("原材料内容已更新，请从当前材料重新开始")
                } else {
                    view
                },
            );
        }
    }
    Ok(json!({ "tasks": tasks }))
}

pub fn project(
    user: &UserRuntime,
    task: &LearningTaskSelection,
    goal_revision: Option<u32>,
    resolve_course: impl FnOnce(&memory::course_resumption::CourseResumption) -> Result<Option<CourseTaskContext>, ToolError>,
) -> Result<Option<LearningTaskView>, ToolError> {
    let store = user.learning_store_for_source(
        task.learning_workspace_id.as_deref(),
        &task.published_book_ref.book_id,
    )?;
    let mut view = LearningTaskView::new(task);
    if let Some(reference) = &task.external_question {
        let note = match user.store.personal_note(reference) {
            Ok(note) => note,
            Err(error) if error.category == "not_found" => return Ok(None),
            Err(error) => return Err(error),
        };
        if note.value.context.book_id.as_deref() != Some(task.published_book_ref.book_id.as_str())
            || note.value.context.workspace_id != task.learning_workspace_id {
            return Ok(None);
        }
        view.kind = "external_question";
        view.title = note.value.record.content.chars().take(100).collect();
        view.source_lid = note.value.context.lid;
        view.source_ref = view.source_lid.as_ref().map(|lid| {
            let mut source = json!({"book_id":task.published_book_ref.book_id,"source_revision":task.source_revision,"lid":lid});
            if let Some(range) = note.value.record.selection_context.as_ref().and_then(|s|s.ranges.first()) {
                source["range"] = json!(range.range);
            }
            source
        });
        view.question = Some(note.value.record.content);
        return Ok(Some(view));
    }
    let run = if let Some(id) = task.course_run_id.as_deref() {
        match store.course_resumption(id)? {
            Some(run) => Some(run),
            None => return Ok(Some(view.unavailable("原课程记录已不存在"))),
        }
    } else {
        None
    };
    if let Some(run) = &run {
        if matches!(
            run.progress.status,
            CourseStatus::Completed | CourseStatus::Skipped
        ) {
            return Ok(None);
        }
        view.title = run.summary.question.clone();
        view.course_context = resolve_course(run)?;
        view.lesson_id = Some(run.progress.lesson_id.clone());
        view.stage = serde_json::to_value(run.progress.stage)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned));
        view.status = if run.progress.status == CourseStatus::Paused {
            "paused"
        } else {
            "active"
        };
        if run.summary.source.publication != task.published_book_ref
            || run.summary.source.source_revision != task.source_revision
            || goal_revision.is_some_and(|revision| revision != run.progress.goal_revision)
        {
            return Ok(Some(
                view.unavailable("原课程目标或材料版本已更新，请打开当前课程继续"),
            ));
        }
        view.question = Some(run.question.clone());
    }
    let chat = task.chat_session_id.as_ref().and_then(|id| {
        user.agent_history.sessions.iter().find(|chat| {
            chat.id == *id
                && chat.matches_scope(
                    task.learning_workspace_id.as_deref(),
                    &task.published_book_ref.book_id,
                )
        })
    });
    if let Some(chat) = chat {
        if let Some(closed) = store.closed_book(&chat.id, &task.published_book_ref.book_id)? {
            let belongs = if let Some(run) = &run {
                closed.course && closed.owner_id == run.progress.run_id
            } else {
                !closed.course
                    && task
                        .closed_book_id
                        .as_ref()
                        .is_some_and(|id| *id == closed.id)
            };
            if belongs
                && closed.completion_reason.is_empty()
                && matches!(closed.status.as_str(), "active" | "paused" | "ended")
            {
                let state = store.state()?;
                let applicable = closed.source_revision == task.source_revision
                    && closed.goal_revision == state.workspace_goal_revision
                    && run.as_ref().map_or_else(
                        || {
                            state.sessions.get(&closed.owner_id).is_some_and(|s| {
                                s.status == memory::learning::TutorSessionStatus::Active
                            })
                        },
                        |run| {
                            closed.course_origin.as_ref().is_none_or(|origin| {
                                origin.round_index == run.round_index
                                    && origin.stage == run.progress.stage
                                    && origin.decision_phase == run.decision_phase
                            })
                        },
                    );
                if applicable {
                    view.kind = "closed_book";
                    view.closed_book_id = Some(closed.id);
                    view.title = closed.target;
                    view.status = if closed.status == "active" && view.status == "active" {
                        "active"
                    } else {
                        "paused"
                    };
                    view.question = closed.visible_turn_ids.iter().rev().find_map(|id| {
                        chat.turns
                            .iter()
                            .find(|turn| turn.turn_id == *id)
                            .and_then(|turn| turn.socratic_question())
                            .map(str::to_owned)
                    });
                    return Ok(Some(view));
                } else if run.is_none() {
                    return Ok(Some(
                        view.unavailable("原练习目标或材料已更新，请回到原对话查看"),
                    ));
                }
            } else if belongs && run.is_none() {
                return Ok(None);
            }
        }
    }
    if run.is_some() {
        if task.closed_book_id.is_some() && chat.is_none() {
            return Ok(Some(
                view.unavailable("原练习对话已不存在，请打开课程选择下一步"),
            ));
        }
        view.closed_book_id = None;
        if chat.is_none() {
            view.chat_session_id = None;
        }
        return Ok(Some(view));
    }
    if task.closed_book_id.is_some() {
        return Ok(Some(view.unavailable("原练习或对话已不存在")));
    }
    let Some(chat) = chat else {
        return Ok(None);
    };
    let Some(turn) = chat.turns.last().filter(|turn| {
        turn.published_book_ref
            .as_ref()
            .is_none_or(|r| r == &task.published_book_ref)
    }) else {
        return Ok(None);
    };
    let question = turn.socratic_question();
    if question.is_none()
        && turn.status != crate::AgentAssistantStatus::PendingAssistant
        && !(task.continue_conversation
            && task.conversation_turn_id.as_deref() == Some(turn.turn_id.as_str()))
    {
        return Ok(None);
    }
    view.title = if chat.title.trim().is_empty() {
        turn.user.chars().take(120).collect()
    } else {
        chat.title.clone()
    };
    view.question = question.map(str::to_owned);
    Ok(Some(view))
}

