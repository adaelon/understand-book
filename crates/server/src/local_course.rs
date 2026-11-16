//! Local prepared courses reuse the same private course runs as published courses.
use crate::{reader_workspace::ReaderWorkspace, user_runtime::UserRuntime};
use memory::course::{CourseAction, CourseMutation, InquiryCourse};
use read_tools::ToolError;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;

fn invalid(message: &str) -> ToolError { crate::user_storage_paths::error("COURSE_INVALID", "validation", message) }

#[derive(Deserialize)]
struct Artifact { version: String, source_id: String, source_revision: String, course: InquiryCourse }

fn load(workspace: &ReaderWorkspace) -> Result<Option<InquiryCourse>, ToolError> {
    let file = workspace.book_dir.join("inquiry_course.json");
    if !file.exists() { return Ok(None); }
    let bytes = std::fs::read(file).map_err(|_| invalid("本地课程文件无法读取"))?;
    let mut artifact: Artifact = serde_json::from_slice(&bytes).map_err(|e| invalid(&format!("本地课程格式无效：{e}")))?;
    let book = &workspace.book;
    if artifact.version != "course_artifact.v1" || artifact.source_id != book.base.book_id || artifact.source_revision != book.source_fingerprint() {
        return Err(invalid("本地课程与当前原文版本不一致"));
    }
    for lesson in &mut artifact.course.lessons {
        let source = &mut lesson.summary.source;
        if source.source_revision != artifact.source_revision || source.publication.book_id != artifact.source_id { return Err(invalid("课程来源不一致")); }
        source.publication.publication_id = "local".into();
        let start = book.source_span(&source.start_lid).ok_or_else(|| invalid("课程来源起点不存在"))?.start;
        let end = book.source_span(&source.end_lid).ok_or_else(|| invalid("课程来源终点不存在"))?.end;
        if start >= end || lesson.assessment.source_lids.iter().any(|lid| book.source_span(lid).is_none_or(|s| s.start < start || s.end > end)) {
            return Err(invalid("课程解题依据超出声明的原文范围"));
        }
    }
    artifact.course.validate()?;
    Ok(Some(artifact.course))
}

pub(crate) fn request(user: &UserRuntime, workspace: &ReaderWorkspace, book_id: &str, method: &str, query: &HashMap<String,String>, input: Value, now: &str) -> Result<Value,ToolError> {
    if workspace.publication.is_some() || book_id != workspace.book.base.book_id { return Err(invalid("阅读资料已切换，请重新打开课程")); }
    let release = format!("local-course:{book_id}");
    let mut store = user.learning_store_for_workspace(None)?;
    let check = |source: &memory::course::CourseSource| -> Result<(),ToolError> {
        if source.publication.book_id == book_id && source.publication.publication_id == "local" && source.source_revision == workspace.book.source_fingerprint() { Ok(()) }
        else { Err(invalid("这次课程记录属于另一份资料或原文版本")) }
    };
    if method == "GET" {
        if let Some(id) = query.get("run_id") {
            let run = store.course_run(id)?;
            let source = &run.lesson.summary.source;
            check(source)?;
            if query.get("source").is_some_and(|s| s == "1") {
                if run.source_excerpt.is_none() { return Err(invalid("请先打开本课原文")); }
                let range = crate::EvidenceRange { start_lid: source.start_lid.clone(), end_lid: source.end_lid.clone(), ranges: vec![] };
                return crate::board_api::text_source_window(&workspace.book, &format!("{id}:source"), &range, None,
                    &json!({"anchor_lid":query.get("anchor_lid"),"direction":query.get("direction"),"include_outline":query.get("include_outline").is_none_or(|v|v!="false")}));
            }
            return Ok(json!(store.course_view(id)?));
        }
        let course = load(workspace)?;
        let progress: Vec<_> = store.course_progress()?.into_iter().filter(|p|p.release_id == release).collect();
        return Ok(json!({"release_id":release,"release_version":1,"course":course.map(|c|c.overview()),"progress":progress}));
    }
    if method != "POST" { return Err(invalid("不支持的课程操作")); }
    let mutation: CourseMutation = serde_json::from_value(input).map_err(|e| invalid(&format!("课程操作格式无效：{e}")))?;
    if let Some(saved) = store.course_receipt(&mutation)? { check(&saved.summary.source)?; return Ok(json!(saved)); }
    let mut course = None;
    let mut source_text = None;
    if matches!(mutation.action, CourseAction::Start { .. }) { course = load(workspace)?; }
    else {
        let run = store.course_run(mutation.run_id.as_deref().ok_or_else(||invalid("缺少课程记录"))?)?;
        check(&run.lesson.summary.source)?;
        if matches!(mutation.action, CourseAction::Source) { source_text = Some(workspace.book.text(&run.lesson.summary.source.start_lid, Some(&run.lesson.summary.source.end_lid))?); }
    }
    let start = if let CourseAction::Start { lesson_id } = &mutation.action {
        Some((release.as_str(), 1, course.as_ref().and_then(|c|c.lessons.iter().find(|l|&l.summary.id==lesson_id)).ok_or_else(||invalid("本课不存在"))?))
    } else { None };
    Ok(json!(store.mutate_course(&mutation,start,1,now,source_text.as_deref())?))
}
