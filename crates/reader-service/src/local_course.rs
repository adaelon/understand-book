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

fn load(workspace: &ReaderWorkspace, trial: Option<&str>) -> Result<Option<InquiryCourse>, ToolError> {
    let directory = match trial {
        Some(revision) => workspace.book_dir.join(".build/course-trials").join(revision),
        None => workspace.book_dir.clone(),
    };
    let file = directory.join("inquiry_course.json");
    if !file.exists() {
        return if trial.is_some() { Err(invalid("找不到此版本的试课")) } else { Ok(None) };
    }
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
    let trial = query.get("trial").map(String::as_str);
    let release = match trial {
        Some(revision) if crate::video_media::safe_segment(revision) => format!("local-course-trial:{book_id}:{revision}"),
        Some(_) => return Err(invalid("试课版本标识无效")),
        None => format!("local-course:{book_id}"),
    };
    let mut store = user.learning_store_for_workspace(None)?;
    let check = |release_id: &str, source: &memory::course::CourseSource| -> Result<(),ToolError> {
        if release_id != release { return Err(invalid("这次学习记录属于另一个课程版本")); }
        if source.publication.book_id == book_id && source.publication.publication_id == "local" && source.source_revision == workspace.book.source_fingerprint() { Ok(()) }
        else { Err(invalid("这次课程记录属于另一份资料或原文版本")) }
    };
    if method == "GET" {
        if let Some(id) = query.get("run_id") {
            let run = store.course_run(id)?;
            let source = &run.lesson.summary.source;
            check(&run.progress.release_id, source)?;
            if query.get("source").is_some_and(|s| s == "1") {
                if run.source_excerpt.is_none() { return Err(invalid("请先打开本课原文")); }
                let range = crate::EvidenceRange { start_lid: source.start_lid.clone(), end_lid: source.end_lid.clone(), ranges: vec![] };
                return crate::board_api::text_source_window(&workspace.book, &format!("{id}:source"), &range, None,
                    &json!({"anchor_lid":query.get("anchor_lid"),"direction":query.get("direction"),"include_outline":query.get("include_outline").is_none_or(|v|v!="false")}));
            }
            return Ok(json!(store.course_view(id)?));
        }
        let course = load(workspace, trial)?;
        let progress: Vec<_> = store.course_progress()?.into_iter().filter(|p|p.release_id == release).collect();
        return Ok(json!({"release_id":release,"release_version":1,"course":course.map(|c|c.overview()),"progress":progress}));
    }
    if method != "POST" { return Err(invalid("不支持的课程操作")); }
    let mutation: CourseMutation = serde_json::from_value(input).map_err(|e| invalid(&format!("课程操作格式无效：{e}")))?;
    if let Some(saved) = store.course_receipt(&mutation)? { check(&saved.progress.release_id, &saved.summary.source)?; return Ok(json!(saved)); }
    let mut course = None;
    let mut source_text = None;
    if matches!(mutation.action, CourseAction::Start { .. }) { course = load(workspace, trial)?; }
    else {
        let run = store.course_run(mutation.run_id.as_deref().ok_or_else(||invalid("缺少课程记录"))?)?;
        check(&run.progress.release_id, &run.lesson.summary.source)?;
        if matches!(mutation.action, CourseAction::Source) { source_text = Some(workspace.book.text(&run.lesson.summary.source.start_lid, Some(&run.lesson.summary.source.end_lid))?); }
    }
    let start = if let CourseAction::Start { lesson_id } = &mutation.action {
        Some((release.as_str(), 1, course.as_ref().and_then(|c|c.lessons.iter().find(|l|&l.summary.id==lesson_id)).ok_or_else(||invalid("本课不存在"))?))
    } else { None };
    let saved = store.mutate_course(&mutation,start,1,now,source_text.as_deref())?;
    check(&saved.progress.release_id, &saved.summary.source)?;
    Ok(json!(saved))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn trial_courses_keep_versions_receipts_and_resumed_runs_separate() {
        let root = tempfile::tempdir().unwrap();
        let book = read_tools::Book::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../book-tool-contracts/fixtures/example-book")).unwrap();
        let reader = reader::Reader::new(&book, reader::DEFAULT_RADIUS);
        let workspace = ReaderWorkspace::local(root.path().into(), book.into(), reader, vec![], None);
        let book_id = &workspace.book.base.book_id;
        let revision = workspace.book.source_fingerprint();
        let user = UserRuntime::local(memory::MemoryStore::open(root.path().join("memory.json")).unwrap(),
            None, crate::AgentHistory::default(), None);
        let mut course: Value = serde_json::from_str(include_str!("../../../docs/examples/inquiry-course/ai-infra-resource-bounds.v1.course.json")).unwrap();
        let source = &mut course["lessons"][0]["summary"]["source"];
        source["publication"]["book_id"] = json!(book_id);
        source["source_revision"] = json!(revision);
        source["start_lid"] = json!("1.1");
        source["end_lid"] = json!("1.2");
        course["lessons"][0]["assessment"]["source_lids"] = json!(["1.1"]);
        for (subdir, question) in [("", "已发布课程"), (".build/course-trials/first", "第一版样课"), (".build/course-trials/revised", "修改后样课")] {
            let dir = root.path().join(subdir);
            fs::create_dir_all(&dir).unwrap();
            course["lessons"][0]["summary"]["question"] = json!(question);
            let artifact = json!({"version":"course_artifact.v1", "source_id":book_id, "source_revision":revision, "course":course});
            fs::write(dir.join("inquiry_course.json"), serde_json::to_vec(&artifact).unwrap()).unwrap();
        }
        let send = |trial: Option<&str>, run: Option<&str>, action: Value| {
            let mut query = HashMap::new();
            if let Some(trial) = trial { query.insert("trial".into(), trial.to_owned()); }
            if let Some(run) = run { query.insert("run_id".into(), run.to_owned()); }
            let method = if action.is_null() { "GET" } else { "POST" };
            request(&user, &workspace, book_id, method, &query, action, "now")
        };
        let start = |operation| json!({"operation_id":operation,"expected_revision":0,"release_version":1,
            "action":{"kind":"start","lesson_id":"compute-or-bandwidth"}});
        for (trial, name, question) in [(None, "formal", "已发布课程"), (Some("first"), "first", "第一版样课"), (Some("revised"), "revised", "修改后样课")] {
            let view = send(trial, None, start(name)).unwrap();
            assert_eq!(view["summary"]["question"], question);
            let catalog = send(trial, None, Value::Null).unwrap();
            let progress = catalog["progress"].as_array().unwrap();
            assert_eq!(progress.len(), 1);
            assert_eq!(progress[0], view["progress"]);
        }
        let original = send(Some("first"), None, start("first")).unwrap();
        let id = original["progress"]["run_id"].as_str().unwrap();
        let pause = json!({"operation_id":"pause","run_id":id,"expected_revision":original["progress"]["revision"],"release_version":1,"action":{"kind":"pause"}});
        for wrong in [None, Some("revised")] {
            assert!(send(wrong, None, start("first")).is_err());
            assert!(send(wrong, Some(id), Value::Null).is_err());
            assert!(send(wrong, None, pause.clone()).is_err());
        }
        let paused = send(Some("first"), None, pause).unwrap();
        assert_eq!(paused["progress"]["status"], "paused");
        let resumed = send(Some("first"), None, json!({"operation_id":"resume","run_id":id,"expected_revision":paused["progress"]["revision"],
            "release_version":1,"action":{"kind":"resume"}})).unwrap();
        assert_eq!(resumed["progress"]["status"], "active");
        assert_eq!(resumed["summary"], original["summary"]);
        for bad in ["", "../..", "missing"] { assert!(send(Some(bad), None, Value::Null).is_err()); }
    }
}
