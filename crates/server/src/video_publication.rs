//! Accepted batch dependencies and cumulative video publication snapshots.
use crate::published_library::{copy_file, equal_files, invalid, read_json, write_json};
use base_schema::{MediaLocation, TranscriptMap, TranscriptTranslation, VideoCapability, VideoCoverage, VideoMaterial, VideoSourceManifest};
use read_tools::ToolError;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};

const CATALOG: &str = "video_enrichments.json";
pub(crate) fn persist_local_binding(w: &crate::reader_workspace::ReaderWorkspace) -> Result<(), ToolError> {
    if let Some(session) = &w.session_path {
        let file = session.with_extension("video-publication.json");
        let reference = crate::video_draft::publication(w)?;
        write_json(&file.with_extension("tmp"), &reference)?;
        std::fs::rename(file.with_extension("tmp"), file).map_err(|_| invalid())?;
    }
    Ok(())
}

pub(crate) fn local_resume_directory(latest: &Path, session: &Option<std::path::PathBuf>) -> std::path::PathBuf {
    let pinned = session.as_ref().and_then(|p| std::fs::read(p.with_extension("video-publication.json")).ok())
        .and_then(|b| serde_json::from_slice::<base_schema::PublishedBookRef>(&b).ok());
    if let (Some(reference), Some(parent)) = (pinned, latest.parent()) {
        let dir = parent.join(&reference.publication_id);
        let manifest = read_json(&dir.join("publication.json")).ok()
            .and_then(|v| serde_json::from_value::<crate::published_library::PublicationManifest>(v).ok());
        if manifest.is_some_and(|m| m.reference == reference) { return dir; }
    }
    latest.into()
}
pub(crate) fn projection(book: &read_tools::Book, directory: &Path) -> Result<serde_json::Value, ToolError> {
    let (manifest, map) = book.load_video_source(directory)?;
    let p: crate::published_library::PublicationManifest = serde_json::from_value(read_json(&directory.join("publication.json"))?).map_err(|_| invalid())?;
    let translations = if directory.join("transcript_translations.json").exists() { read_json(&directory.join("transcript_translations.json"))? } else { serde_json::json!([]) };
    let progress = if directory.join("video_translation_progress.json").exists() { read_json(&directory.join("video_translation_progress.json"))? } else { serde_json::Value::Null };
    Ok(serde_json::json!({"publication":p.reference,"material":manifest.material,"source":manifest.source,
        "map":map,"coverage":manifest.coverage,"translations":translations,"translation_progress":progress}))
}

pub(crate) fn adopt_local(state: &mut crate::AppState) -> Result<serde_json::Value, ToolError> {
    use crate::published_library::{PublicationManifest, PublishedBook};
    use std::{fs, sync::Arc};
    if state.workspace.video_adoption_busy(&state.user)
        || state.workspace.active_agent_stream.as_ref().and_then(|w| w.upgrade()).is_some_and(|s| s.snapshot().persistence_state != "saved") {
        return Ok(serde_json::json!({"status":"busy"}));
    }
    let current = crate::video_draft::publication(&state.workspace)?;
    let parent = state.workspace.book_dir.parent().ok_or_else(invalid)?;
    let mut candidates: Vec<_> = fs::read_dir(parent).map_err(|_| invalid())?.filter_map(Result::ok)
        .filter_map(|entry| {
            let p: PublicationManifest = serde_json::from_slice(&fs::read(entry.path().join("publication.json")).ok()?).ok()?;
            (p.reference.book_id == current.book_id && p.previous_publication.is_some()).then_some((entry.path(), p))
        }).collect();
    candidates.sort_by(|a,b| b.1.reference.publication_id.cmp(&a.1.reference.publication_id));
    for (directory, manifest) in candidates {
        if manifest.reference == current { return Ok(serde_json::json!({"status":"current"})); }
        let mut ancestor = manifest.previous_publication.clone();
        while let Some(reference) = ancestor {
            if reference == current {
                let book = Arc::new(read_tools::Book::load(directory.to_str().ok_or_else(invalid)?).map_err(|_| invalid())?);
                let video = projection(&book, &directory)?;
                let publication = Arc::new(PublishedBook { reference: manifest.reference.clone(), book, manifest,
                    localization_cache: std::env::temp_dir().join("understand-book-video").join(&current.book_id).join("localization.json"), directory });
                let local = state.workspace.publication.is_none();
                let old_book = state.workspace.book.clone(); let old_dir = state.workspace.book_dir.clone();
                let old_publication = state.workspace.publication.clone();
                state.workspace.adopt_same_source_publication(publication)?;
                if local { state.workspace.publication = None; }
                if let Err(error) = persist_local_binding(&state.workspace) {
                    state.workspace.book = old_book; state.workspace.book_dir = old_dir; state.workspace.publication = old_publication;
                    return Err(error);
                }
                return Ok(serde_json::json!({"status":"adopted","video":video}));
            }
            let path = parent.join(&reference.publication_id).join("publication.json");
            let p: PublicationManifest = serde_json::from_value(read_json(&path)?).map_err(|_| invalid())?;
            ancestor = p.previous_publication;
        }
    }
    Ok(serde_json::json!({"status":"current"}))
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
struct Batch { batch_id: String, coverage: Vec<VideoCoverage>, files: Vec<String> }
#[derive(Serialize, Deserialize)]
struct Catalog { version: String, material: VideoMaterial, batches: Vec<Batch> }

fn catalog(dir: &Path, material: &VideoMaterial) -> Result<Catalog, ToolError> {
    if !dir.join(CATALOG).exists() { return Ok(Catalog { version: "video_enrichments.v1".into(), material: material.clone(), batches: vec![] }); }
    let c: Catalog = serde_json::from_value(read_json(&dir.join(CATALOG))?).map_err(|_| invalid())?;
    if c.version != "video_enrichments.v1" || c.material != *material { return Err(invalid()); }
    Ok(c)
}
fn union(mut ranges: Vec<MediaLocation>) -> Vec<MediaLocation> {
    ranges.sort_by(|a,b| a.start_ms.total_cmp(&b.start_ms));
    let mut merged: Vec<MediaLocation> = vec![];
    for range in ranges {
        if let Some(last) = merged.last_mut() {
            if range.start_ms <= last.end_ms { last.end_ms = last.end_ms.max(range.end_ms); continue; }
        }
        merged.push(range);
    }
    merged
}

/// Union immutable contributions from the latest registered snapshot and this
/// input. A stale producer contributes its new batch without deleting other batches.
pub(crate) fn assemble(input: &Path, previous: Option<&Path>, stage: &Path) -> Result<(), ToolError> {
    let file = stage.join("video_source_manifest.json");
    if !file.exists() { return Ok(()); }
    let mut manifest: VideoSourceManifest = serde_json::from_value(read_json(&file)?).map_err(|_| invalid())?;
    let map: TranscriptMap = serde_json::from_value(read_json(&stage.join(&manifest.transcript_map_path))?).map_err(|_| invalid())?;
    if let Some(old) = previous {
        let before: VideoSourceManifest = serde_json::from_value(read_json(&old.join("video_source_manifest.json"))?).map_err(|_| invalid())?;
        if before.material != manifest.material || before.source != manifest.source
            || !equal_files(&old.join(&before.transcript_map_path), &stage.join(&manifest.transcript_map_path))? { return Err(invalid()); }
    }
    let mut batches: BTreeMap<String, Batch> = BTreeMap::new();
    let mut translations: BTreeMap<String, TranscriptTranslation> = BTreeMap::new();
    let mut add_translations = |dir: &Path, rel: &str| -> Result<(), ToolError> {
        if !dir.join(rel).exists() { return Ok(()); }
        let items: Vec<TranscriptTranslation> = serde_json::from_value(read_json(&dir.join(rel))?).map_err(|_| invalid())?;
        for item in items {
            item.validate(&map).map_err(|_| invalid())?;
            if translations.get(&item.segment_id).is_some_and(|old| old != &item) { return Err(invalid()); }
            translations.insert(item.segment_id.clone(), item);
        }
        Ok(())
    };
    for dir in previous.into_iter().chain(std::iter::once(input)) {
        add_translations(dir, "transcript_translations.json")?;
        for batch in catalog(dir, &manifest.material)?.batches {
            if batch.batch_id.is_empty() || !batch.batch_id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
                || batch.files.is_empty() || batch.coverage.is_empty() { return Err(invalid()); }
            let prefix = format!("video-enrichments/{}/", batch.batch_id);
            for coverage in &batch.coverage {
                coverage.validate(&manifest.source).map_err(|_| invalid())?;
                if coverage.capability == VideoCapability::Transcript || coverage.accepted.is_empty()
                    || !coverage.pending.is_empty() || !coverage.failed.is_empty() { return Err(invalid()); }
            }
            if batches.get(&batch.batch_id).is_some_and(|b| b != &batch) { return Err(invalid()); }
            for dep in &batch.files {
                if !dep.starts_with(&prefix) { return Err(invalid()); }
                crate::published_library::relative(dep)?;
                if stage.join(dep).exists() {
                    if !equal_files(&stage.join(dep), &dir.join(dep))? { return Err(invalid()); }
                } else { copy_file(&dir.join(dep), stage, dep)?; }
                if dep.ends_with("/transcript_translations.json") { add_translations(dir, dep)?; }
            }
            if batch.coverage.iter().any(|c| c.capability == VideoCapability::Translation)
                && !batch.files.iter().any(|p| p == &format!("{prefix}transcript_translations.json")) { return Err(invalid()); }
            if batch.coverage.iter().any(|c| c.capability == VideoCapability::Graph)
                && !batch.files.iter().any(|p| p == &format!("{prefix}graph.json")) { return Err(invalid()); }
            if batch.coverage.iter().any(|c| c.capability == VideoCapability::Discourse)
                && !batch.files.iter().any(|p| p == &format!("{prefix}discourse.json")) { return Err(invalid()); }
            batches.insert(batch.batch_id.clone(), batch);
        }
    }
    // Accepted coverage comes from admitted batches whose complete dependency set
    // was copied above. Pending/failed producer claims never become availability.
    for capability in [VideoCapability::Translation, VideoCapability::VisualIndex, VideoCapability::Graph, VideoCapability::Discourse, VideoCapability::AudioReview] {
        let accepted = union(batches.values().flat_map(|b| &b.coverage).filter(|c| c.capability == capability).flat_map(|c| c.accepted.clone()).collect());
        if accepted.is_empty() {
            if manifest.coverage.iter().any(|c| c.capability == capability && !c.accepted.is_empty()) { return Err(invalid()); }
            continue;
        }
        if capability == VideoCapability::Translation {
            for segment in &map.segments {
                if accepted.iter().any(|r| r.start_ms < segment.location.end_ms && r.end_ms > segment.location.start_ms)
                    && !translations.contains_key(&segment.segment_id) { return Err(invalid()); }
            }
        }
        let mut coverage = VideoCoverage { capability, accepted, pending: vec![], failed: vec![] };
        coverage.pending = coverage.gaps(&manifest.source).map_err(|_| invalid())?;
        manifest.coverage.retain(|c| c.capability != capability);
        manifest.coverage.push(coverage);
    }
    manifest.validate().map_err(|_| invalid())?;
    write_json(&file, &manifest)?;
    // Build progress is a snapshot for readers, not permission to consume candidates.
    // Cumulative accepted translations win over a stale producer's failed/pending state.
    let progress_path = "video_translation_progress.json";
    let progress_dir = if input.join(progress_path).exists() { Some(input) }
        else { previous.filter(|p| p.join(progress_path).exists()) };
    if let Some(dir) = progress_dir {
        let mut progress = read_json(&dir.join(progress_path))?;
        if progress["version"] != "video_translation_progress.v1"
            || progress["material"] != serde_json::to_value(&manifest.material).map_err(|_| invalid())? { return Err(invalid()); }
        let mut seen = std::collections::BTreeSet::new();
        for task in progress["tasks"].as_array_mut().ok_or_else(invalid)? {
            let ids = task["segment_ids"].as_array().ok_or_else(invalid)?;
            if ids.is_empty() || !matches!(task["status"].as_str(), Some("pending" | "running" | "accepted" | "failed")) { return Err(invalid()); }
            for id in ids {
                let id = id.as_str().ok_or_else(invalid)?;
                if !seen.insert(id.to_string()) || !map.segments.iter().any(|s| s.segment_id == id) { return Err(invalid()); }
            }
            if ids.iter().all(|id| translations.contains_key(id.as_str().unwrap())) {
                task["status"] = serde_json::json!("accepted"); task["failure"] = serde_json::Value::Null;
            }
        }
        write_json(&stage.join(progress_path), &progress)?;
    }
    if !translations.is_empty() { write_json(&stage.join("transcript_translations.json"), &translations.into_values().collect::<Vec<_>>())?; }
    if !batches.is_empty() { write_json(&stage.join(CATALOG), &Catalog { version: "video_enrichments.v1".into(), material: manifest.material, batches: batches.into_values().collect() })?; }
    Ok(())
}
