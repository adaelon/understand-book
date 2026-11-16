//! Published music assets share the material grant and byte-range delivery path.
use base_schema::{MusicSourceManifest, PublishedBookRef};
use read_tools::ToolError;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub(crate) const MANIFEST: &str = "music_source_manifest.json";

fn invalid(message: &str) -> ToolError {
    crate::user_storage_paths::error("MUSIC_UNAVAILABLE", "not_found", message)
}

pub(crate) fn load(directory: &Path) -> Result<MusicSourceManifest, ToolError> {
    let manifest: MusicSourceManifest = serde_json::from_value(
        crate::published_library::read_json(&directory.join(MANIFEST))?
    ).map_err(|_| invalid("音乐资料无法读取"))?;
    manifest.validate().map_err(|e| invalid(&e))?;
    Ok(manifest)
}

pub(crate) fn files(manifest: &MusicSourceManifest) -> Vec<&str> {
    let mut paths = vec![manifest.recording.media_path.as_str()];
    paths.extend(manifest.score.musicxml_path.as_deref());
    paths.extend(manifest.score.pdf_path.as_deref());
    paths.extend(manifest.pages.iter().map(|p| p.path.as_str()));
    paths.extend(manifest.demonstrations.iter().map(|d| d.media_path.as_str()));
    paths.sort_unstable();
    paths.dedup();
    paths
}

pub(crate) fn validate_files(directory: &Path, manifest: &MusicSourceManifest) -> Result<(), ToolError> {
    for file in files(manifest) {
        crate::published_library::relative(file)?;
        if !directory.join(file).is_file() { return Err(invalid("音乐资源尚未准备完整")); }
    }
    Ok(())
}

pub(crate) fn capabilities(manifest: &MusicSourceManifest) -> Value {
    let names = manifest.capabilities();
    let mut value = json!({"coverage":manifest.coverage});
    for name in ["music", "music_score", "music_events", "music_note_alignment"] {
        value[name] = json!(names.iter().any(|n| n == name));
    }
    value
}

pub(crate) fn projection(directory: &Path) -> Result<Value, ToolError> {
    if !directory.join(MANIFEST).exists() || !directory.join("publication.json").exists() {
        return Ok(Value::Null);
    }
    let manifest = load(directory)?;
    validate_files(directory, &manifest)?;
    let published: crate::published_library::PublicationManifest = serde_json::from_value(
        crate::published_library::read_json(&directory.join("publication.json"))?
    ).map_err(|_| invalid("音乐发布尚未就绪"))?;
    if published.reference.book_id != manifest.book_id { return Err(invalid("音乐发布版本不一致")); }
    let reference = &published.reference;
    Ok(json!({"publication":reference,
        "audio_url":reference.url("music/audio"),
        "score_pages":manifest.pages.iter().map(|p| json!({"page_id":p.page_id,"url":reference.url(&format!("music/score/{}", p.page_id))})).collect::<Vec<_>>(),
        "demonstrations":manifest.demonstrations.iter().map(|d| json!({"demonstration_id":d.demonstration_id,"url":reference.url(&format!("music/demo/{}", d.demonstration_id))})).collect::<Vec<_>>(),
        "manifest":manifest}))
}

pub(crate) fn path(directory: &Path, leaf: &str) -> Result<PathBuf, ToolError> {
    // Publication/opening validates the complete score. Range and score-page
    // requests only need asset locations; rebuilding every note/event here
    // repeatedly stalls playback of full symphonies.
    #[derive(Deserialize)]
    struct Media { media_path: String }
    #[derive(Deserialize)]
    struct Score { musicxml_path: Option<String>, pdf_path: Option<String> }
    #[derive(Deserialize)]
    struct Demo { demonstration_id: String, media_path: String }
    #[derive(Deserialize)]
    struct Assets {
        recording: Media,
        score: Score,
        pages: Vec<base_schema::MusicScorePage>,
        demonstrations: Vec<Demo>,
    }
    let bytes = std::fs::read(directory.join(MANIFEST)).map_err(|_| invalid("音乐资料无法读取"))?;
    let manifest: Assets = serde_json::from_slice(&bytes).map_err(|_| invalid("音乐资料无法读取"))?;
    let relative = if leaf == "music/audio" { Some(manifest.recording.media_path.as_str()) }
        else if leaf == "music/source" { manifest.score.pdf_path.as_deref().or(manifest.score.musicxml_path.as_deref()) }
        else if let Some(id) = leaf.strip_prefix("music/score/") {
            manifest.pages.iter().find(|p| p.page_id == id).map(|p| p.path.as_str())
        } else if let Some(id) = leaf.strip_prefix("music/demo/") {
            manifest.demonstrations.iter().find(|d| d.demonstration_id == id).map(|d| d.media_path.as_str())
        } else { None }.ok_or_else(|| invalid("音乐资源不存在"))?;
    crate::published_library::relative(relative)?;
    let path = directory.join(relative);
    if !path.is_file() { return Err(invalid("音乐资源不存在")); }
    Ok(path)
}

pub(crate) fn local_path(directory: &Path, url: &str) -> Result<PathBuf, ToolError> {
    if let Some(leaf) = url.strip_prefix("/book/") { return path(directory, leaf); }
    let manifest: crate::published_library::PublicationManifest = serde_json::from_value(
        crate::published_library::read_json(&directory.join("publication.json"))?
    ).map_err(|_| invalid("音乐发布尚未就绪"))?;
    let prefix = format!("/books/{}/publications/{}/", manifest.reference.book_id, manifest.reference.publication_id);
    path(directory, url.strip_prefix(&prefix).ok_or_else(|| invalid("音乐发布版本不一致"))?)
}

pub(crate) fn reference(directory: &Path) -> Result<PublishedBookRef, ToolError> {
    let manifest: crate::published_library::PublicationManifest = serde_json::from_value(
        crate::published_library::read_json(&directory.join("publication.json"))?
    ).map_err(|_| invalid("音乐发布尚未就绪"))?;
    Ok(manifest.reference)
}
