//! Window-owned question drafts. Admission copies the context before releasing the draft.
use crate::*;
use base_schema::VideoQuestionContext;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct VideoDraft {
    pub message: String,
    pub context: VideoQuestionContext,
}
pub(crate) type Drafts = BTreeMap<String, VideoDraft>;

fn invalid(message: &str) -> ToolError {
    user_storage_paths::error("VIDEO_DRAFT_INVALID", "conflict", message)
}
pub(crate) fn publication(w: &reader_workspace::ReaderWorkspace) -> Result<base_schema::PublishedBookRef, ToolError> {
    if let Some(p) = &w.publication { return Ok(p.reference.clone()); }
    let p: published_library::PublicationManifest = serde_json::from_slice(
        &std::fs::read(w.book_dir.join("publication.json")).map_err(|_| invalid("Video publication unavailable"))?
    ).map_err(|_| invalid("Video publication unavailable"))?;
    Ok(p.reference)
}
pub(crate) fn consumed(user: &user_runtime::UserRuntime, id: &str) -> bool {
    user.agent_history.sessions.iter().flat_map(|s| &s.turns)
        .any(|t| t.video_question.as_ref().is_some_and(|q| q.draft_id == id))
}
pub(crate) fn command(w: &mut reader_workspace::ReaderWorkspace, user: &user_runtime::UserRuntime, input: &Value) -> Result<Value, ToolError> {
    let window = input["window_id"].as_str().filter(|s| !s.is_empty()).ok_or_else(|| invalid("Window required"))?;
    let reference = publication(w)?;
    let current = w.video_drafts.get(window).filter(|d| d.context.publication == reference && !consumed(user, &d.context.draft_id));
    match input["action"].as_str() {
        Some("get") => return Ok(json!({"draft":current})),
        Some("save") => {
            let draft: VideoDraft = serde_json::from_value(input["draft"].clone()).map_err(|_| invalid("Draft required"))?;
            let (manifest, _) = w.book.load_video_source(&w.book_dir)?;
            draft.context.validate(&manifest).map_err(|e| invalid(&e))?;
            if draft.context.publication != publication(w)? { return Err(invalid("Draft publication changed")); }
            if consumed(user, &draft.context.draft_id) { return Err(invalid("Draft already handed to a turn")); }
            // Region and range are independent editable content; publication, material
            // and the presented frame remain bound until release.
            if current.is_some_and(|d| {
                let mut expected = d.context.clone();
                expected.region = draft.context.region.clone();
                expected.range = draft.context.range.clone();
                expected != draft.context
            }) { return Err(invalid("Release the previous draft before binding another frame")); }
            if draft.message.is_empty() && draft.context.region.is_none() && draft.context.range.is_none() {
                return Err(invalid("Empty draft must be released"));
            }
            w.video_drafts.insert(window.into(), draft);
        }
        Some("release") => {
            if current.is_some_and(|d| Some(d.context.draft_id.as_str()) != input["draft_id"].as_str()) {
                return Err(invalid("Draft changed"));
            }
            w.video_drafts.remove(window);
        }
        _ => return Err(invalid("Unknown draft action")),
    }
    Ok(json!({"draft":w.video_drafts.get(window)}))
}

pub(crate) fn for_input(w: &reader_workspace::ReaderWorkspace, user: &user_runtime::UserRuntime, input: &Value) -> Result<Option<VideoQuestionContext>, ToolError> {
    let Some(id) = input.get("video_draft_id").filter(|v| !v.is_null()) else {
        return if w.book_dir.join("video_source_manifest.json").is_file() {
            Err(invalid("A video question needs its input-time draft"))
        } else { Ok(None) };
    };
    let draft = input["video_window_id"].as_str().and_then(|window| w.video_drafts.get(window))
        .ok_or_else(|| invalid("Save the video draft before sending"))?;
    if id.as_str() != Some(&draft.context.draft_id) || consumed(user, &draft.context.draft_id) {
        return Err(invalid("Video draft no longer available"));
    }
    if draft.context.publication != publication(w)? { return Err(invalid("Draft publication changed")); }
    Ok(Some(draft.context.clone()))
}

pub(crate) fn load(path: &Option<PathBuf>) -> Drafts {
    path.as_ref().and_then(|p| std::fs::read(p.with_extension("video-drafts.json")).ok())
        .and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}
pub(crate) fn persist(w: &reader_workspace::ReaderWorkspace) -> Result<(), ToolError> {
    crate::video_publication::persist_local_binding(w)?;
    if let Some(path) = &w.session_path {
        let path = path.with_extension("video-drafts.json");
        std::fs::write(path.with_extension("tmp"), serde_json::to_vec(&w.video_drafts).unwrap())
            .and_then(|_| std::fs::rename(path.with_extension("tmp"), &path))
            .map_err(|_| invalid("Could not save video draft"))?;
    }
    Ok(())
}
