//! Local curator access to an existing music build. This is not a publication.
use crate::{AppState, Reply};
use read_tools::ToolError;
use serde_json::{json, Value};
use std::{ffi::OsString, path::{Path, PathBuf}, process::Command};

const PREFIX: &str = "/build_workbench/music/";

fn unavailable(message: &str) -> ToolError {
    crate::user_storage_paths::error("MUSIC_CALIBRATION_UNAVAILABLE", "validation", message)
}

fn editable(state: &AppState) -> bool {
    !state.services.reader_only && state.workspace.publication.is_none()
        && !state.workspace.book_dir.join("publication.json").exists()
}

fn require_local(state: &AppState) -> Result<PathBuf, ToolError> {
    if !editable(state) { return Err(unavailable("请在未发布的本地音乐构建目录中检查对齐")); }
    let directory = state.workspace.book_dir.as_path();
    if !directory.join(crate::music_media::MANIFEST).is_file() {
        return Err(unavailable("当前工作区没有可校准的音乐资料"));
    }
    directory.canonicalize().map_err(|e| unavailable(&format!("音乐构建目录无法打开：{e}")))
}

pub(crate) fn context(state: &AppState) -> Result<Value, ToolError> {
    if !editable(state) || !state.workspace.book_dir.join(crate::music_media::MANIFEST).is_file() {
        return Ok(Value::Null);
    }
    let directory = require_local(state)?;
    let manifest = crate::music_media::load(&directory)?;
    crate::music_media::validate_files(&directory, &manifest)?;
    let history_path = directory.join(".build/music/calibrations.json");
    let history = if history_path.exists() { crate::published_library::read_json(&history_path)? } else { json!([]) };
    let source_query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("recording_id", &manifest.recording.source.recording_id)
        .append_pair("recording_revision", &manifest.recording.source.revision)
        .append_pair("score_id", &manifest.score.source.score_id)
        .append_pair("score_revision", &manifest.score.source.revision)
        .finish();
    let pages: Vec<Value> = manifest.pages.iter().map(|page| {
        let mut value = serde_json::to_value(page).expect("score page serializes");
        value["url"] = json!(format!("/api{PREFIX}score/{}?{source_query}", page.page_id));
        value
    }).collect();
    Ok(json!({"manifest":manifest,"audio_url":format!("/api{PREFIX}audio?{source_query}"),"score_pages":pages,"history":history}))
}

pub(crate) fn local_asset(state: &AppState, url: &str) -> Result<PathBuf, ToolError> {
    let directory = require_local(state)?;
    let (path, source) = crate::parse_query(url);
    let leaf = path.strip_prefix(PREFIX).ok_or_else(|| unavailable("音乐校准资源不存在"))?;
    if leaf != "audio" && !leaf.starts_with("score/") { return Err(unavailable("音乐校准资源不存在")); }
    let manifest = crate::music_media::load(&directory)?;
    if source.get("recording_id") != Some(&manifest.recording.source.recording_id)
        || source.get("recording_revision") != Some(&manifest.recording.source.revision)
        || source.get("score_id") != Some(&manifest.score.source.score_id)
        || source.get("score_revision") != Some(&manifest.score.source.revision) {
        return Err(unavailable("音乐材料已切换，请重新打开当前材料的校准页面"));
    }
    crate::music_media::path(&directory, &format!("music/{leaf}"))
}

fn calibration_command() -> Result<crate::BuiltinStageRunnerCommand, ToolError> {
    let root = crate::workspace_root();
    let executable_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf));
    let configured = std::env::var_os("UNDERSTAND_BOOK_BUILD_SIDECAR").filter(|v| !v.is_empty()).map(PathBuf::from);
    let mut runner = crate::resolve_builtin_stage_runner_command(&root, executable_dir.as_deref(), configured.as_deref())?;
    if runner.prefix_args.first().is_some_and(|v| v == "workbench-stage") {
        runner.prefix_args = vec![OsString::from("music-calibrate")];
    } else {
        // Reuse the installed/development Engine discovery, choosing its music-only entry.
        runner.prefix_args[1] = root.join("skills/build/music-calibration-cli.ts").into_os_string();
    }
    Ok(runner)
}

fn calibrate(state: &AppState, body: &str) -> Result<Value, ToolError> {
    let directory = require_local(state)?;
    let input: Value = serde_json::from_str(body).map_err(|_| unavailable("校准修订需要有效的 JSON 内容"))?;
    let manifest = crate::music_media::load(&directory)?;
    if input["recording"] != json!(manifest.recording.source) || input["score"] != json!(manifest.score.source) {
        return Err(unavailable("当前音乐工作区已改变，请重新打开后再保存修订"));
    }
    let runner = calibration_command()?;
    let input_path = directory.join(".build/music/ui-revision.json");
    crate::write_workbench_json(&input_path, &input)?;
    let mut command = Command::new(runner.program);
    command.current_dir(runner.current_dir).args(runner.prefix_args)
        .arg("--book-dir").arg(&directory).arg("--revision-file").arg(&input_path);
    #[cfg(windows)] { use std::os::windows::process::CommandExt; command.creation_flags(0x08000000); }
    let result = command.output().map_err(|e| unavailable(&format!("无法启动本地音乐校准：{e}")))?;
    let _ = std::fs::remove_file(&input_path);
    if !result.status.success() {
        let detail = String::from_utf8_lossy(&result.stderr);
        return Err(unavailable(&format!("校准未保存：{}", detail.trim())));
    }
    context(state)
}

pub(crate) fn dispatch(state: &AppState, action: &str, method: &str, body: &str) -> Reply {
    let result = match (action, method) {
        ("context", "GET") => context(state),
        ("calibrate", "POST") => calibrate(state, body),
        ("context" | "calibrate", _) => return crate::method_not_allowed(),
        _ => return crate::validation("MUSIC_CALIBRATION_ROUTE_UNKNOWN", "音乐校准操作不存在"),
    };
    match result { Ok(value) => crate::ok_json(&value), Err(error) => crate::err_reply(&error) }
}
