//! Preparation media stays outside Book residency. Both hosts stream the same imported file.
use base_schema::{VideoImport, VideoSourceRef};
use read_tools::ToolError;
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};
use tiny_http::{Header, Response, StatusCode};

pub fn executable(name: &str) -> PathBuf {
    if let Some(value) = std::env::var_os(name.to_ascii_uppercase()).filter(|v| !v.is_empty()) { return value.into(); }
    if let Ok(current) = std::env::current_exe() {
        if let Some(directory) = current.parent() {
            let file = directory.join("resources/video/bin").join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
            if file.is_file() { return file; }
        }
    }
    name.into()
}

pub fn invalid(message: &str) -> ToolError {
    crate::user_storage_paths::error("VIDEO_UNAVAILABLE", "not_found", message)
}
pub fn safe_segment(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
pub fn read_import(dir: &Path) -> Result<VideoImport, ToolError> {
    let state: VideoImport = serde_json::from_slice(
        &fs::read(dir.join("video_import.json"))
            .map_err(|_| invalid("Video preparation is unavailable"))?,
    )
    .map_err(|_| invalid("Invalid video preparation"))?;
    state.validate().map_err(|e| invalid(&e))?;
    if !safe_segment(&state.video.media_id) || !safe_segment(&state.video.media_revision) {
        return Err(invalid("Invalid video identity"));
    }
    Ok(state)
}
pub fn projection(dir: &Path) -> Result<Value, ToolError> {
    let state = read_import(dir)?;
    let playable = state.source.is_some()
        && fs::metadata(dir.join("media/original.mp4"))
            .is_ok_and(|m| m.is_file() && m.len() as f64 == state.total_bytes);
    let build: Option<Value> = fs::read(dir.join("video_build.json")).ok().and_then(|b|serde_json::from_slice(&b).ok());
    let linked = linked_publication(dir).ok();
    let transcript = linked.as_ref().map(|(_, directory)| transcript_projection(directory)).transpose()?;
    let probe: Option<Value> = fs::read(dir.join("media/probe.json")).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok());
    let timeline_origin_ms = probe.as_ref().and_then(|p| p["format"]["start_time"].as_str())
        .and_then(|s| s.parse::<f64>().ok()).filter(|n| n.is_finite()).unwrap_or(0.0) * 1000.0;
    Ok(
        json!({"video":state.video,"title":state.title,"stage":state.stage,
        "copied_bytes":state.copied_bytes,"total_bytes":state.total_bytes,
        "duration_ms":state.source.as_ref().map(|s|s.duration_ms),
        "timeline_origin_ms":timeline_origin_ms,
        "playable":playable,"qa_ready":false,"english_ready":transcript.is_some(),
        "publication":linked.as_ref().map(|(reference,_)|reference),
        "transcript_progress":build.as_ref().map(|b|json!({"stage":b["stage"],"chunks":b["chunks"],"failure":b["failure"]})),
        "failure":if state.source.is_some() && !playable { Some("导入副本缺失或不完整".to_string()) } else { state.failure }}),
    )
}

pub fn linked_publication(dir: &Path) -> Result<(base_schema::PublishedBookRef, PathBuf), ToolError> {
    let link: Value = serde_json::from_slice(&fs::read(dir.join("video_publication.json")).map_err(|_|invalid("Transcript publication is not ready"))?)
        .map_err(|_|invalid("Invalid video publication link"))?;
    let reference: base_schema::PublishedBookRef = serde_json::from_value(link["reference"].clone()).map_err(|_|invalid("Invalid publication reference"))?;
    let directory = dir.join(link["directory"].as_str().ok_or_else(||invalid("Missing publication directory"))?);
    let publication: crate::published_library::PublicationManifest = serde_json::from_slice(&fs::read(directory.join("publication.json")).map_err(|_|invalid("Publication is unavailable"))?)
        .map_err(|_|invalid("Invalid publication"))?;
    let manifest: base_schema::VideoSourceManifest = serde_json::from_slice(&fs::read(directory.join("video_source_manifest.json")).map_err(|_|invalid("Video source is unavailable"))?)
        .map_err(|_|invalid("Invalid video source"))?;
    if publication.reference != reference || manifest.source.video != read_import(dir)?.video || manifest.material.book_id != reference.book_id {
        return Err(invalid("Video publication identity mismatch"));
    }
    Ok((reference, directory))
}
pub fn transcript_projection(directory: &Path) -> Result<Value, ToolError> {
    let book = read_tools::Book::load(directory.to_str().ok_or_else(||invalid("Invalid directory"))?).map_err(|e|invalid(&e))?;
    let (manifest,map) = book.load_video_source(directory)?;
    Ok(json!({"material":manifest.material,"coverage":manifest.coverage,"map":map}))
}
pub fn transcript(dir: &Path) -> Result<Value, ToolError> {
    let (reference,directory) = linked_publication(dir)?;
    let mut value = transcript_projection(&directory)?;
    value["publication"] = json!(reference);
    Ok(value)
}
/// A relative link lets an entire local workspace/service tree move without changing media identity.
pub fn link_publication(dir: &Path, destination: &Path, reference: &base_schema::PublishedBookRef) -> Result<(), ToolError> {
    let from = std::path::absolute(dir).map_err(|_|invalid("Invalid workspace"))?;
    let to = std::path::absolute(destination).map_err(|_|invalid("Invalid publication"))?;
    let a: Vec<_> = from.components().collect(); let b: Vec<_> = to.components().collect();
    let common = a.iter().zip(&b).take_while(|(x,y)|x == y).count();
    let relative = if common == 0 { to.clone() } else {
        let mut result = PathBuf::new();
        for _ in common..a.len() { result.push(".."); }
        for c in &b[common..] { result.push(c.as_os_str()); } result
    };
    let target = dir.join("video_publication.json");
    fs::write(target.with_extension("tmp"),json!({"reference":reference,"directory":relative}).to_string()).map_err(|_|invalid("Cannot save publication link"))?;
    fs::rename(target.with_extension("tmp"),target).map_err(|_|invalid("Cannot commit publication link"))
}
pub fn local_dirs(primary: Option<&Path>, library: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs = vec![];
    if let Some(primary) = primary.filter(|p| p.join("video_import.json").is_file()) {
        dirs.push(primary.to_path_buf());
    }
    if let Some(library) = library {
        if let Ok(entries) = fs::read_dir(library) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.join("video_import.json").is_file() && !dirs.contains(&path) {
                    dirs.push(path);
                }
            }
        }
    }
    dirs
}
pub fn route_parts(url: &str) -> Option<(VideoSourceRef, &str)> {
    let parts: Vec<_> = url
        .split('?')
        .next()?
        .trim_start_matches('/')
        .split('/')
        .collect();
    match parts.as_slice() {
        ["videos", id, revision, leaf] if safe_segment(id) && safe_segment(revision) => Some((
            VideoSourceRef {
                media_id: (*id).into(),
                media_revision: (*revision).into(),
            },
            *leaf,
        )),
        _ => None,
    }
}
pub fn media_path(dir: &Path) -> Result<PathBuf, ToolError> {
    let state = read_import(dir)?;
    let source = state
        .source
        .ok_or_else(|| invalid("Video is not ready for playback"))?;
    let path = dir.join(source.imported_path);
    if !fs::metadata(&path).is_ok_and(|m| m.is_file() && m.len() as f64 == state.total_bytes) {
        return Err(invalid("Imported video is missing or incomplete"));
    }
    Ok(path)
}

// A single range is what HTML video seek requests use. Unknown/multiple ranges are ignored (200).
fn byte_range(range: Option<&str>, size: u64) -> Result<Option<(u64, u64)>, ()> {
    let Some(value) = range.and_then(|s| s.strip_prefix("bytes=")) else {
        return Ok(None);
    };
    if value.contains(',') {
        return Ok(None);
    }
    let (start, end) = value.split_once('-').ok_or(())?;
    if start.is_empty() {
        let suffix: u64 = end.parse().map_err(|_| ())?;
        if suffix == 0 || size == 0 {
            return Err(());
        }
        return Ok(Some((size.saturating_sub(suffix), size - 1)));
    }
    let start: u64 = start.parse().map_err(|_| ())?;
    let end = if end.is_empty() {
        size.saturating_sub(1)
    } else {
        end.parse::<u64>()
            .map_err(|_| ())?
            .min(size.saturating_sub(1))
    };
    if start >= size || start > end {
        return Err(());
    }
    Ok(Some((start, end)))
}
pub fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()).unwrap_or("").to_ascii_lowercase().as_str() {
        "m3u8" => "application/vnd.apple.mpegurl",
        "mp3" => "audio/mpeg",
        "m4a" => "audio/mp4",
        "aac" => "audio/aac",
        "wav" => "audio/wav",
        "flac" => "audio/flac",
        "ogg" | "oga" => "audio/ogg",
        "opus" => "audio/ogg; codecs=opus",
        "svg" => "image/svg+xml",
        "musicxml" | "xml" => "application/vnd.recordare.musicxml+xml",
        "mxl" => "application/vnd.recordare.musicxml",
        _ => "video/mp4",
    }
}

pub fn accel_uri(path: &Path, root: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?.to_str()?.replace('\\', "/");
    let encoded: String = relative.bytes().map(|b| {
        if b.is_ascii_alphanumeric() || b"/-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }
    }).collect();
    Some(format!("/_private_media/{encoded}"))
}

pub fn stream_response(
    path: &Path,
    method: &str,
    range: Option<&str>,
) -> std::io::Result<Response<Box<dyn Read + Send>>> {
    let mut file = fs::File::open(path)?;
    let size = file.metadata()?.len();
    let mut headers = vec![
        Header::from_bytes("Content-Type", content_type(path)).unwrap(),
        Header::from_bytes("Accept-Ranges", "bytes").unwrap(),
        Header::from_bytes("Cache-Control", "no-store").unwrap(),
        Header::from_bytes("X-Content-Type-Options", "nosniff").unwrap(),
    ];
    let selected = if method == "HEAD" {
        Ok(None)
    } else {
        byte_range(range, size)
    };
    let (status, start, length) = match selected {
        Ok(Some((start, end))) => {
            headers.push(
                Header::from_bytes("Content-Range", format!("bytes {start}-{end}/{size}")).unwrap(),
            );
            (206, start, end - start + 1)
        }
        Ok(None) => (200, 0, size),
        Err(()) => {
            headers.push(Header::from_bytes("Content-Range", format!("bytes */{size}")).unwrap());
            (416, 0, 0)
        }
    };
    file.seek(SeekFrom::Start(start))?;
    Ok(Response::new(
        StatusCode(status),
        headers,
        Box::new(file.take(length)) as Box<dyn Read + Send>,
        Some(length as usize),
        None,
    )
    .with_chunked_threshold(usize::MAX))
}
pub fn serve(request: tiny_http::Request, path: &Path) {
    let range = request
        .headers()
        .iter()
        .find(|h| h.field.equiv("Range"))
        .map(|h| h.value.as_str());
    let response = stream_response(path, request.method().as_str(), range);
    match response {
        Ok(response) => {
            let _ = request.respond(response);
        }
        Err(_) => {
            let _ = request.respond(Response::empty(404));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(dir: &Path) {
        fs::create_dir_all(dir.join("media")).unwrap();
        // Transport fixture bytes, not a playable video; actual decode is covered by Core and browser tests.
        fs::write(dir.join("media/original.mp4"), b"0123456789").unwrap();
        fs::write(dir.join("media/probe.json"), "{}").unwrap();
        fs::write(dir.join("video_import.json"), json!({"version":"video_import.v1","video":{"media_id":"lecture","media_revision":"1"},"title":"Lecture","original_path":"gone.mp4","stage":"awaiting_transcript","copied_bytes":10,"total_bytes":10,"source":{"video":{"media_id":"lecture","media_revision":"1"},"imported_path":"media/original.mp4","original_path":"gone.mp4","duration_ms":1000,"width":160,"height":90},"failure":null}).to_string()).unwrap();
    }
    #[test]
    fn nginx_media_paths_and_playlist_type() {
        let root = std::path::Path::new("/data/reader");
        assert_eq!(super::accel_uri(&root.join("shared-cache/video-playback/a/media.mp4"), root).as_deref(), Some("/_private_media/shared-cache/video-playback/a/media.mp4"));
        assert_eq!(super::accel_uri(std::path::Path::new("/another/media.mp4"), root), None);
        assert_eq!(super::content_type(std::path::Path::new("index.m3u8")), "application/vnd.apple.mpegurl");
    }
    #[test]
    fn video_range_responses_stream_exact_bytes_and_head_length() {
        let root = tempfile::tempdir().unwrap();
        fixture(root.path());
        let path = media_path(root.path()).unwrap();
        for (method, range, status, bytes, length) in [
            ("GET", None, 200, "0123456789", 10),
            ("GET", Some("bytes=2-5"), 206, "2345", 4),
            ("GET", Some("bytes=7-"), 206, "789", 3),
            ("GET", Some("bytes=-3"), 206, "789", 3),
            ("GET", Some("bytes=8-999"), 206, "89", 2),
            ("GET", Some("bytes=10-"), 416, "", 0),
            ("GET", Some("bytes=5-2"), 416, "", 0),
            ("HEAD", Some("bytes=2-5"), 200, "0123456789", 10),
        ] {
            let response = stream_response(&path, method, range).unwrap();
            assert_eq!(response.status_code().0, status);
            assert_eq!(response.data_length(), Some(length));
            let mut body = String::new();
            response.into_reader().read_to_string(&mut body).unwrap();
            assert_eq!(body, bytes);
        }
        assert_eq!(byte_range(Some("bytes=1-2,4-5"), 10), Ok(None));
    }
    #[test]
    fn video_status_does_not_claim_missing_media_is_playable() {
        let root = tempfile::tempdir().unwrap();
        fixture(root.path());
        fs::remove_file(root.path().join("media/original.mp4")).unwrap();
        let status = projection(root.path()).unwrap();
        assert_eq!(status["playable"], false);
        assert!(status["failure"].is_string());
        assert!(media_path(root.path()).is_err());
    }

}
