//! Derived HLS delivery files. Original media and its presentation timeline stay authoritative.
use base_schema::VideoSourceManifest;
use read_tools::ToolError;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{fs, path::{Path, PathBuf}, process::{Command, Stdio}};

#[derive(Debug, Serialize, Deserialize)]
pub struct PlaybackInfo {
    pub time_base: [u32; 2],
}

fn failed(message: impl Into<String>) -> ToolError {
    crate::user_storage_paths::error("VIDEO_PLAYBACK_UNAVAILABLE", "unavailable", &message.into())
}

pub(crate) fn source(directory: &Path) -> Result<VideoSourceManifest, ToolError> {
    let value: VideoSourceManifest = serde_json::from_slice(&fs::read(directory.join("video_source_manifest.json"))
        .map_err(|e| failed(e.to_string()))?).map_err(|e| failed(e.to_string()))?;
    value.validate().map_err(failed)?;
    Ok(value)
}

pub(crate) fn cache_directory(root: &Path, book_id: &str) -> Result<PathBuf, ToolError> {
    if !crate::is_valid_book_id(book_id) { return Err(crate::authorization::missing()); }
    Ok(root.join("shared-cache/video-playback").join(book_id))
}

pub(crate) fn ready(directory: &Path) -> Option<PlaybackInfo> {
    if !directory.join("index.m3u8").is_file() || !directory.join("media.mp4").is_file() { return None; }
    serde_json::from_slice(&fs::read(directory.join("info.json")).ok()?).ok()
}

/// Used by publication and the maintenance backfill. A complete directory is exposed atomically.
/// The publication contract already requires the original video to stay identical for a book_id.
pub fn prepare(root: &Path, directory: &Path) -> Result<Option<PathBuf>, ToolError> {
    if !directory.join("video_source_manifest.json").is_file() { return Ok(None); }
    let manifest = source(directory)?;
    let destination = cache_directory(root, &manifest.material.book_id)?;
    if ready(&destination).is_some() { return Ok(Some(destination)); }
    let parent = destination.parent().unwrap();
    fs::create_dir_all(parent).map_err(|e| failed(e.to_string()))?;
    let stage = tempfile::tempdir_in(parent).map_err(|e| failed(e.to_string()))?;
    let original = std::path::absolute(directory.join(&manifest.source.imported_path)).map_err(|e| failed(e.to_string()))?;
    let info = PlaybackInfo { time_base: crate::video_frames::presentation_time_base(directory, &manifest.source)? };
    let mut command = Command::new(crate::video_media::executable("ffmpeg"));
    command.current_dir(stage.path()).args(["-nostdin", "-hide_banner", "-loglevel", "error", "-n", "-copyts", "-i"])
        .arg(original).args(["-map", "0:V:0", "-map", "0:a:0?", "-c", "copy", "-avoid_negative_ts", "disabled",
            "-f", "hls", "-hls_time", "5", "-hls_playlist_type", "vod", "-hls_segment_type", "fmp4",
            "-hls_flags", "single_file+independent_segments", "-hls_segment_options"])
        .arg(format!("video_track_timescale={}", info.time_base[1]))
        .args(["-hls_segment_filename", "media.mp4", "index.m3u8"])
        .stdin(Stdio::null()).stdout(Stdio::null());
    #[cfg(windows)] {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let output = command.output().map_err(|e| failed(format!("Cannot prepare video playback: {e}")))?;
    if !output.status.success() { return Err(failed(String::from_utf8_lossy(&output.stderr).trim().to_owned())); }
    let playlist = fs::read_to_string(stage.path().join("index.m3u8")).map_err(|e| failed(e.to_string()))?;
    if !playlist.contains("#EXT-X-MAP:") || !playlist.contains("#EXT-X-ENDLIST") { return Err(failed("Incomplete video playlist")); }
    fs::write(stage.path().join("info.json"), serde_json::to_vec(&info).unwrap()).map_err(|e| failed(e.to_string()))?;
    // nginx receives only these derived files after HTTP authorization. Original
    // media and private Reader files keep their owner-only permissions.
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        for file in ["index.m3u8", "media.mp4"] {
            fs::set_permissions(stage.path().join(file), fs::Permissions::from_mode(0o644)).map_err(|e| failed(e.to_string()))?;
        }
        for directory in [parent, stage.path()] {
            fs::set_permissions(directory, fs::Permissions::from_mode(0o755)).map_err(|e| failed(e.to_string()))?;
        }
    }
    fs::rename(stage.path(), &destination).map_err(|e| failed(e.to_string()))?;
    Ok(Some(destination))
}

pub(crate) fn projection(directory: &Path, cache: &Path, publication: &base_schema::PublishedBookRef) -> Result<Value, ToolError> {
    if !directory.join("video_source_manifest.json").is_file() { return Ok(Value::Null); }
    let manifest = source(directory)?;
    let prepared = ready(cache);
    let time_base = match &prepared {
        Some(info) => info.time_base,
        None => crate::video_frames::presentation_time_base(directory, &manifest.source)?,
    };
    Ok(json!({"publication":publication,"material":manifest.material,"source":manifest.source,
        "time_base":time_base,"hls":prepared.is_some()}))
}

pub(crate) fn file(cache: &Path, leaf: &str) -> Result<PathBuf, ToolError> {
    let name = match leaf { "video/playback/index.m3u8" => "index.m3u8", "video/playback/media.mp4" => "media.mp4", _ => return Err(crate::authorization::missing()) };
    if ready(cache).is_none() { return Err(crate::authorization::missing()); }
    Ok(cache.join(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playback_files_require_a_completed_cache_and_known_leaf() {
        let root = tempfile::tempdir().unwrap();
        assert!(file(root.path(), "video/playback/media.mp4").is_err());
        fs::write(root.path().join("info.json"), r#"{"time_base":[1,16000]}"#).unwrap();
        fs::write(root.path().join("index.m3u8"), "#EXTM3U").unwrap();
        fs::write(root.path().join("media.mp4"), b"fixture").unwrap();
        assert_eq!(file(root.path(), "video/playback/media.mp4").unwrap(), root.path().join("media.mp4"));
        assert!(file(root.path(), "video/playback/../info.json").is_err());
    }
}
