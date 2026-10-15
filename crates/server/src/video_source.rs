//! Source snapshots and playback resolve the saved publication, never the current VU index.
use crate::*;
use base_schema::{EvidenceTarget, MediaLocation, PublishedBookRef};
use runtime::video_source::{MediaSource, members, publication};

fn unavailable() -> ToolError { user_storage_paths::error("VIDEO_SOURCE_UNAVAILABLE", "unavailable", "原发布媒体当前不可用") }

pub(crate) fn snapshot(book: &Book, directory: &Path, target: EvidenceTarget) -> Result<MediaSource, ToolError> {
    let declared: published_library::PublicationManifest = serde_json::from_slice(&std::fs::read(directory.join("publication.json")).map_err(|_| unavailable())?).map_err(|_| unavailable())?;
    if publication(&target) != Some(&declared.reference) { return Err(unavailable()); }
    let (manifest, map) = book.load_video_source(directory)?;
    book.validate_video_evidence_target(&manifest, &target)?;
    let count: usize = members(&target).iter().map(|t| match t { EvidenceTarget::Visual { frames, regions, .. } => frames.len() + regions.len(), _ => 0 }).sum();
    if count > 8 { return Err(user_storage_paths::error("VIDEO_SOURCE_LIMIT", "validation", "Present at most eight frames per source")); }
    let transcript: Vec<_> = map.segments.iter().filter(|s| members(&target).iter().any(|t| match t {
        EvidenceTarget::Text { text_range, .. } => s.text_range.lid == text_range.lid && s.text_range.range.start < text_range.range.end && text_range.range.start < s.text_range.range.end,
        EvidenceTarget::Visual { frames, regions, .. } => frames.iter().chain(regions.iter().map(|r| &r.frame)).any(|f| s.location.start_ms <= f.actual_pts_ms && f.actual_pts_ms < s.location.end_ms),
        EvidenceTarget::Audio { location, .. } => s.location.start_ms < location.end_ms && location.start_ms < s.location.end_ms,
        _ => false,
    })).cloned().collect();
    let translations: Vec<base_schema::TranscriptTranslation> = if directory.join("transcript_translations.json").exists() {
        let all: Vec<base_schema::TranscriptTranslation> = serde_json::from_slice(&std::fs::read(directory.join("transcript_translations.json")).map_err(|_| unavailable())?).map_err(|_| unavailable())?;
        all.into_iter().filter(|t| t.validate(&map).is_ok() && transcript.iter().any(|s| s.segment_id == t.segment_id)).collect()
    } else { vec![] };
    Ok(MediaSource { target, publication: declared.reference, duration_ms: manifest.source.duration_ms, transcript, translations, observations: vec![], audio_reviews: vec![] })
}

pub(crate) fn text_snapshot(book: &Book, directory: &Path, range: &EvidenceRange) -> Result<Option<MediaSource>, ToolError> {
    if !directory.join("video_source_manifest.json").exists() { return Ok(None); }
    let (manifest, _) = book.load_video_source(directory)?;
    let p: published_library::PublicationManifest = serde_json::from_slice(&std::fs::read(directory.join("publication.json")).map_err(|_| unavailable())?).map_err(|_| unavailable())?;
    let targets: Vec<_> = range.ranges.iter().map(|r| EvidenceTarget::Text { publication: p.reference.clone(), transcript: Some(manifest.material.transcript.clone()), text_range: base_schema::TranscriptTextRange { lid: r.lid.clone(), range: base_schema::Span { start: r.range.start as usize, end: r.range.end as usize } } }).collect();
    let target = if targets.len() == 1 { targets[0].clone() } else { EvidenceTarget::Joint { members: targets.into_iter().map(|target| base_schema::JointEvidenceMember { target, support_role: "英文原文".into() }).collect() } };
    snapshot(book, directory, target).map(Some)
}

/// Local published layout: published/<book>/<publication>. Check identity before using bytes.
pub(crate) fn local_directory(workspace: &reader_workspace::ReaderWorkspace, reference: &PublishedBookRef) -> Result<PathBuf, ToolError> {
    for directory in [Some(workspace.book_dir.clone()), workspace.book_dir.parent().map(|p| p.join(&reference.publication_id)),
        workspace.book_dir.parent().and_then(|p| p.parent()).map(|p|p.join(&reference.book_id).join(&reference.publication_id))].into_iter().flatten() {
        let declared = std::fs::read(directory.join("publication.json")).ok().and_then(|v| serde_json::from_slice::<published_library::PublicationManifest>(&v).ok());
        if declared.is_some_and(|p| p.reference == *reference) { return Ok(directory); }
    }
    Err(unavailable())
}

pub(crate) fn popup(binding: &SourceBinding, loaded: Result<(&Book, &Path), ToolError>, media_url: String) -> Value {
    let saved = binding.media.as_ref().expect("media binding");
    let mut view = json!({"source_ref_id":binding.source_ref_id,"label":binding.label_snapshot,"highlighted_quote":binding.preview_snapshot,
        "context_before":"","context_after":"","stale":false,"can_open_in_reader":true,"media":saved,"media_url":media_url,"frames":[]});
    let result = (|| -> Result<Vec<Value>, ToolError> {
        let (book, directory) = loaded?;
        snapshot(book, directory, saved.target.clone())?;
        let mut images = vec![];
        let cache = std::env::temp_dir().join("understand-book-video-frames").join(&saved.publication.publication_id);
        for target in members(&saved.target) {
            if let EvidenceTarget::Visual { frames, regions, .. } = target {
                for (frame, region) in frames.iter().map(|f| (f, None)).chain(regions.iter().map(|r| (&r.frame, Some(r.rect_normalized)))) {
                    let batch = crate::video_frames::read_frames(book, directory, &saved.publication, "source-preview", &cache,
                        runtime::video::ReadFrames { location: MediaLocation { video: frame.video.clone(), start_ms: frame.actual_pts_ms, end_ms: (frame.actual_pts_ms + 1000.0).min(saved.duration_ms) }, sampling: vec![frame.actual_pts_ms], region }, &runtime::run_context::CancellationToken::default())?;
                    let image = batch.images.first().filter(|i| i.read.frame == *frame).ok_or_else(unavailable)?;
                    images.push(json!({"frame":frame,"region":image.region,"url":format!("data:image/png;base64,{}",image.png_base64)}));
                }
            }
        }
        Ok(images)
    })();
    match result { Ok(images) => view["frames"] = json!(images), Err(e) => { view["stale"] = json!(true); view["can_open_in_reader"] = json!(false); view["media_url"] = Value::Null; view["unavailable_reason"] = json!(e.message); } }
    view
}

pub(crate) fn local_popup(workspace: &reader_workspace::ReaderWorkspace, request: &AgentSourceRequest, binding: &SourceBinding) -> Value {
    let loaded = local_directory(workspace, &binding.media.as_ref().unwrap().publication)
        .and_then(|d| Book::load(d.to_str().ok_or_else(unavailable)?).map(|b| (b, d)).map_err(|_| unavailable()));
    popup(binding, loaded.as_ref().map(|(b,d)| (b,d.as_path())).map_err(|_| unavailable()),
        format!("/api/book/video/source/{}/{}/media", request.turn_id, request.source_ref_id))
}
