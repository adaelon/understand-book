//! Prepared lessons use the shared-release boundary and the existing private
//! learning database. No model call is needed to open or progress a lesson.
use crate::{
    authorization::{missing, Authorization, AuthorizedContext},
    workspace_publication::Manifest,
};
use memory::course::{CourseAction, CourseMutation, InquiryCourse};
use read_tools::{Book, ToolError};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use std::collections::HashMap;

fn invalid(message: &str) -> ToolError {
    crate::user_storage_paths::error("COURSE_INVALID", "validation", message)
}
fn storage() -> ToolError {
    crate::user_storage_paths::error("COURSE_UNAVAILABLE", "unavailable", "课程暂时无法读取")
}

pub use reader_service::course_context::freeze_chat;

pub(crate) fn validate_sources(
    db: &Connection,
    course: Option<&InquiryCourse>,
) -> Result<(), ToolError> {
    let Some(course) = course else { return Ok(()) };
    for lesson in &course.lessons {
        let source = &lesson.summary.source;
        let directory: Option<String> = db
            .query_row(
                "SELECT directory FROM book_publications WHERE book_id=? AND publication_id=?",
                params![
                    source.publication.book_id,
                    source.publication.publication_id
                ],
                |r| r.get(0),
            )
            .optional()
            .map_err(|_| storage())?;
        let book = Book::load(&directory.ok_or_else(|| invalid("课程引用的资料版本不存在"))?)
            .map_err(|_| invalid("课程来源无法读取"))?;
        if book.source_fingerprint() != source.source_revision {
            return Err(invalid("课程来源与发布资料版本不一致"));
        }
        if book
            .text(&source.start_lid, Some(&source.end_lid))?
            .trim()
            .is_empty()
        {
            return Err(invalid("课程来源为空"));
        }
        let start = book
            .source_span(&source.start_lid)
            .ok_or_else(missing)?
            .start;
        let end = book.source_span(&source.end_lid).ok_or_else(missing)?.end;
        for lid in &lesson.assessment.source_lids {
            let span = book.source_span(lid).ok_or_else(missing)?;
            if span.start < start || span.end > end {
                return Err(invalid("任务判断依据不在本课声明的来源范围内"));
            }
        }
    }
    Ok(())
}

pub(crate) fn request(
    access: &Authorization,
    context: &AuthorizedContext,
    owner: &str,
    workspace: &str,
    method: &str,
    query: &HashMap<String, String>,
    input: Value,
    now: i64,
) -> Result<Value, ToolError> {
    let metadata = access.learning_workspace_metadata(owner, workspace)?;
    if method != "GET" && method != "POST" {
        return Err(invalid("不支持的课程操作"));
    }
    if method == "POST" && (metadata.archived || !query.is_empty()) {
        return Err(invalid("请先恢复工作区再学习课程"));
    }
    let source_window = query.get("source").is_some_and(|value| value == "1") && query.contains_key("run_id");
    if method == "GET" && (!input.is_null() || query.keys().any(|key| {
        key != "run_id" && !(source_window && matches!(key.as_str(), "source" | "anchor_lid" | "direction" | "include_outline"))
    })) {
        return Err(invalid("无效的课程查询"));
    }
    let (release_id, version, course) = {
        let control = access.admin.control.lock().unwrap();
        let reference:Option<(String,u32)>=control.connection.query_row("SELECT release_id,version FROM workspace_release_instances WHERE owner_user_id=? AND learning_workspace_id=?",params![owner,workspace],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(|_|storage())?;
        let (id, version) = reference.ok_or_else(missing)?;
        let raw: String = control
            .connection
            .query_row(
                "SELECT manifest_json FROM workspace_releases WHERE release_id=? AND version=?",
                params![id, version],
                |r| r.get(0),
            )
            .map_err(|_| storage())?;
        let manifest: Manifest = serde_json::from_str(&raw).map_err(|_| storage())?;
        (id, version, manifest.inquiry_course)
    };
    let user = context.user.lock().unwrap();
    let mut store = user.learning_store_for_workspace(Some(workspace))?;
    if method == "GET" {
        if let Some(id) = query.get("run_id") {
            let run = store.course_run(id)?;
            let source = &run.lesson.summary.source;
            let publication = access
                .library
                .lock()
                .unwrap()
                .load(owner, &source.publication)?;
            if source_window {
                if run.source_excerpt.is_none() || publication.book.source_fingerprint() != source.source_revision {
                    return Err(missing());
                }
                let range = crate::EvidenceRange { start_lid: source.start_lid.clone(), end_lid: source.end_lid.clone(), ranges: vec![] };
                let options = json!({"anchor_lid":query.get("anchor_lid"),"direction":query.get("direction"),
                    "include_outline":query.get("include_outline").is_none_or(|value| value != "false")});
                return crate::board_api::text_source_window(&publication.book, &format!("{id}:source"), &range, Some(&source.publication), &options);
            }
            return Ok(json!(store.course_view(id)?));
        }
        // Catalogs do not expose frozen private tasks, answer keys or solutions.
        return Ok(
            json!({"release_id":release_id,"release_version":version,"course":course.map(|c|c.overview()),"progress":store.course_progress()?}),
        );
    }
    let mutation: CourseMutation =
        serde_json::from_value(input).map_err(|_| invalid("课程操作格式无效"))?;
    if let Some(saved) = store.course_receipt(&mutation)? {
        access
            .library
            .lock()
            .unwrap()
            .load(owner, &saved.summary.source.publication)?;
        return Ok(json!(saved));
    }
    let mut source_text = None;
    let start = if let CourseAction::Start { lesson_id } = &mutation.action {
        let lesson = course
            .as_ref()
            .and_then(|c| c.lessons.iter().find(|l| &l.summary.id == lesson_id))
            .ok_or_else(missing)?;
        access
            .library
            .lock()
            .unwrap()
            .load(owner, &lesson.summary.source.publication)?;
        Some((release_id.as_str(), version, lesson))
    } else {
        let run = store.course_run(
            mutation
                .run_id
                .as_deref()
                .ok_or_else(|| invalid("缺少课程记录"))?,
        )?;
        let publication = access
            .library
            .lock()
            .unwrap()
            .load(owner, &run.lesson.summary.source.publication)?;
        if matches!(mutation.action, CourseAction::Source) {
            source_text = Some(publication.book.text(
                &run.lesson.summary.source.start_lid,
                Some(&run.lesson.summary.source.end_lid),
            )?);
        }
        None
    };
    let result = store.mutate_course(
        &mutation,
        start,
        metadata.goal.revision,
        &now.to_string(),
        source_text.as_deref()
    )?;
    // Mutation receipts remain exact replays; async reviews are read separately.
    Ok(json!(result))
}
