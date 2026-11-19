//! Cloud workspace/catalog context for shared task projections.
use crate::{control_store::ControlStore, published_library::PublishedLibrary, user_runtime::UserRuntime};
use read_tools::ToolError;
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};
use std::sync::Mutex;
pub use reader_service::learning_task::*;

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
        if let Some(view) = project(user, selection, goal_revision, |run| course_context(control, run))? {
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
