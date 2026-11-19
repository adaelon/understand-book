//! Preserve the source snapshot that was read; reopening never rewrites it with new analysis.
use crate::*;

pub fn popup(binding: &SourceBinding, loaded: Result<(&Book, &Path), ToolError>, media_url: String) -> Value {
    let saved = binding.music.as_ref().expect("music source binding");
    let mut view = json!({"source_ref_id":binding.source_ref_id,"label":saved.label,"material_title":saved.title,
        "highlighted_quote":saved.preview,"context_before":"","context_after":"","stale":false,
        "can_open_in_reader":true,"music":saved,"media_url":media_url,"heading_path":[]});
    let verify = (|| -> Result<Vec<Value>, ToolError> {
        let (book, directory) = loaded?;
        let reference = crate::music_interact::publication(directory)?;
        let manifest = book.load_music_source(directory)?;
        if reference != saved.target.publication { return Err(runtime::music::unavailable()); }
        saved.target.audio.validate(&manifest).map_err(read_tools::music::error)?;
        if saved.target.score.as_ref().is_some_and(|score| score.score != manifest.score.source
            || score.source_ids.iter().any(|id| !manifest.score_objects.iter().any(|object| &object.source_id == id))) {
            return Err(runtime::music::unavailable());
        }
        let event_pages: std::collections::BTreeSet<_> = manifest.events.iter().filter(|event| saved.target.event_ids.contains(&event.event_id))
            .flat_map(|event| &event.evidence).flat_map(|evidence| evidence.score_page_ids.iter().map(String::as_str)).collect();
        let read_pages: std::collections::BTreeSet<_> = saved.material["read"]["pages"].as_array().into_iter().flatten()
            .filter_map(|page| page["page_id"].as_str()).collect();
        let pages = manifest.pages.iter().filter(|page| event_pages.contains(page.page_id.as_str()) || read_pages.contains(page.page_id.as_str())
            || saved.target.score.as_ref().is_some_and(|score|
            score.source_ids.iter().any(|id| manifest.score_objects.iter().any(|object| &object.source_id == id && object.page_id == page.page_id))))
            .map(|page| json!({"page_id":page.page_id,"url":media_url.trim_end_matches("audio").to_owned()+"score/"+&page.page_id})).collect();
        Ok(pages)
    })();
    match verify {
        Ok(pages) => view["music_score_pages"] = json!(pages),
        Err(error) => {
            view["stale"] = json!(true);
            view["can_open_in_reader"] = json!(false);
            view["media_url"] = Value::Null;
            view["unavailable_reason"] = json!(error.message);
        }
    }
    view
}

pub(crate) fn local_popup(workspace: &reader_workspace::ReaderWorkspace, request: &AgentSourceRequest, binding: &SourceBinding) -> Value {
    let reference = &binding.music.as_ref().unwrap().target.publication;
    let loaded = crate::video_source::local_directory(workspace, reference)
        .and_then(|directory| Book::load(directory.to_str().ok_or_else(runtime::music::unavailable)?)
            .map(|book| (book, directory)).map_err(read_tools::music::error));
    let same_source = crate::music_interact::publication(&workspace.book_dir).ok().as_ref() == Some(reference);
    let mut value = popup(binding, loaded.as_ref().map(|(book, directory)| (book, directory.as_path()))
        .map_err(|e| user_storage_paths::error(&e.error_code, &e.category, &e.message)),
        format!("/api/book/music/source/{}/{}/audio", request.turn_id, request.source_ref_id));
    // The source popup retains original media while the main Reader stays in its selected publication.
    if !same_source { value["can_open_in_reader"] = json!(false); }
    value
}

pub(crate) fn local_asset(state: &AppState, path: &str) -> Result<PathBuf, ToolError> {
    let (turn, rest) = path.split_once('/').ok_or_else(service_error::missing)?;
    let (source, leaf) = rest.split_once('/').ok_or_else(service_error::missing)?;
    if leaf != "audio" && !leaf.starts_with("score/") { return Err(service_error::missing()); }
    let binding = crate::agent_source_binding(state, &AgentSourceRequest { turn_id: turn.into(), source_ref_id: source.into(), note_mem_id: None })?;
    let saved = binding.music.ok_or_else(service_error::missing)?;
    let directory = crate::video_source::local_directory(&state.workspace, &saved.target.publication)?;
    let manifest = crate::music_media::load(&directory)?;
    saved.target.audio.validate(&manifest).map_err(read_tools::music::error)?;
    crate::music_media::path(&directory, &format!("music/{leaf}"))
}
