//! Explicit learning selections. Course, practice and chat records remain authoritative.
use crate::{
    control_store::ControlStore, published_library::PublishedLibrary,
    reader_workspace::ReaderWorkspace, user_runtime::UserRuntime,
};
use base_schema::PublishedBookRef;
use memory::course::CourseStatus;
use read_tools::ToolError;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct LearningTaskSelection {
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
    #[serde(default)]
    pub continue_conversation: bool,
}

impl LearningTaskSelection {
    pub fn same_target(&self, other: &Self) -> bool {
        self.learning_workspace_id == other.learning_workspace_id
            && self.published_book_ref == other.published_book_ref
            && if self.course_run_id.is_some() || other.course_run_id.is_some() {
                self.course_run_id == other.course_run_id
            } else {
                self.chat_session_id == other.chat_session_id
            }
    }
}

#[derive(Serialize)]
struct LearningTaskView {
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
    message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    course_context: Option<CourseTaskContext>,
}

#[derive(Serialize)]
struct CourseTaskContext {
    release_id: String,
    release_version: u32,
    collection_kind: &'static str,
    collection_title: String,
    unit_title: String,
    lesson_number: u32,
    lesson_count: u32,
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
            message: None,
            course_context: None,
        }
    }
    fn unavailable(mut self, message: &str) -> Self {
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

fn selections(user: &UserRuntime) -> &[LearningTaskSelection] {
    user.session_store
        .as_ref()
        .map_or(&[], |store| store.learning_tasks.as_slice())
}

pub(crate) fn select(
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
            .unwrap_or_else(|_| crate::multi_user_host::now()),
        conversation_turn_id: latest.map(|turn| turn.turn_id.clone()),
        continue_conversation,
    };
    let view = project(
        user,
        &selection,
        workspace.learning_goal.as_ref().map(|g| g.revision),
        None,
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

pub(crate) fn local_list(
    user: &UserRuntime,
    workspace: &ReaderWorkspace,
) -> Result<Value, ToolError> {
    let reference = publication(workspace);
    let mut tasks = Vec::new();
    for selection in selections(user).iter().filter(|task| {
        task.learning_workspace_id == workspace.learning_workspace_id
            && task.published_book_ref == reference
    }) {
        if let Some(view) = project(
            user,
            selection,
            workspace.learning_goal.as_ref().map(|goal| goal.revision),
            None,
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

pub(crate) fn network_list(
    user: &UserRuntime,
    owner: &str,
    control: &ControlStore,
    library: &Mutex<PublishedLibrary>,
) -> Result<Value, ToolError> {
    let mut tasks = Vec::new();
    for selection in selections(user) {
        let workspace = selection
            .learning_workspace_id
            .as_deref()
            .map(|id| control.learning_workspace(owner, id))
            .transpose();
        let (goal_revision, unavailable) = match workspace {
            Ok(Some(workspace)) if workspace.archived => (
                Some(workspace.goal.revision),
                Some("原学习工作区已归档，可恢复后继续"),
            ),
            Ok(Some(workspace))
                if !workspace.members.iter().any(|member| {
                    member.published_book_ref == selection.published_book_ref && member.readable
                }) =>
            {
                (Some(workspace.goal.revision), Some("原学习材料当前不可用"))
            }
            Ok(Some(workspace)) => (Some(workspace.goal.revision), None),
            Ok(None) => (None, None),
            Err(error) if error.category == "not_found" => (None, Some("原学习工作区已不存在")),
            Err(error) => return Err(error),
        };
        if let Some(view) = project(user, selection, goal_revision, Some(control))? {
            let unavailable = unavailable.or_else(|| {
                library
                    .lock()
                    .unwrap()
                    .authorize(owner, &selection.published_book_ref)
                    .err()
                    .map(|_| "原学习材料当前不可用")
            });
            tasks.push(match unavailable {
                Some(message) => view.unavailable(message),
                None => view,
            });
        }
    }
    Ok(json!({ "tasks": tasks }))
}

fn project(
    user: &UserRuntime,
    task: &LearningTaskSelection,
    goal_revision: Option<u32>,
    control: Option<&ControlStore>,
) -> Result<Option<LearningTaskView>, ToolError> {
    let store = user.learning_store_for_source(
        task.learning_workspace_id.as_deref(),
        &task.published_book_ref.book_id,
    )?;
    let mut view = LearningTaskView::new(task);
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
        view.course_context = control
            .map(|control| course_context(control, run))
            .transpose()?
            .flatten();
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

/// A saved run selects one immutable release. Only its organizational fields
/// cross the database boundary; lesson bodies and assessment content stay there.
fn course_context(
    control: &ControlStore,
    run: &memory::course_resumption::CourseResumption,
) -> Result<Option<CourseTaskContext>, ToolError> {
    let storage = |_| {
        crate::user_storage_paths::error(
            "LEARNING_TASK_STORAGE_UNAVAILABLE",
            "unavailable",
            "课程归属信息暂时无法读取",
        )
    };
    let row: Option<(String, String, u32, u32, u32, Option<String>)> = control.connection.query_row(
        "SELECT json_extract(r.manifest_json,'$.name'),
                json_extract(unit.value,'$.title'), CAST(lesson.key AS INTEGER)+1,
                json_array_length(r.manifest_json,'$.inquiry_course.lessons'),
                (SELECT count(DISTINCT json_extract(source.value,'$.summary.source.publication.book_id'))
                   FROM json_each(r.manifest_json,'$.inquiry_course.lessons') source),
                publication.directory
           FROM workspace_releases r
           JOIN json_each(r.manifest_json,'$.inquiry_course.lessons') lesson
           JOIN json_each(r.manifest_json,'$.inquiry_course.units') unit
             ON json_extract(unit.value,'$.id')=json_extract(lesson.value,'$.summary.unit_id')
           LEFT JOIN book_publications publication
             ON publication.book_id=?4 AND publication.publication_id=?5
          WHERE r.release_id=?1 AND r.version=?2 AND json_extract(lesson.value,'$.summary.id')=?3",
        params![run.progress.release_id,run.progress.release_version,run.progress.lesson_id,
            run.summary.source.publication.book_id,run.summary.source.publication.publication_id],
        |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?)),
    ).optional().map_err(storage)?;
    Ok(row.map(
        |(release_title, unit_title, lesson_number, lesson_count, books, directory)| {
            let collection_kind = if books == 1 { "book" } else { "course" };
            let collection_title = if books == 1 {
                directory
                    .as_deref()
                    .and_then(|path| crate::book_title::read(std::path::Path::new(path), None))
                    .unwrap_or(release_title)
            } else {
                release_title
            };
            CourseTaskContext {
                release_id: run.progress.release_id.clone(),
                release_version: run.progress.release_version,
                collection_kind,
                collection_title,
                unit_title,
                lesson_number,
                lesson_count,
            }
        },
    ))
}
