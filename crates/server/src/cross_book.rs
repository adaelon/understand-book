//! Cloud material authorization and publication reads.
use crate::{mcp_read::McpRead, published_library::PublishedLibrary};
use read_tools::ToolError;
use runtime::library::{LibraryReadResult, LibrarySource, READ_OPERATIONS};
use runtime::orchestrator::SourceBinding;
use serde_json::{json, Value};
use std::sync::Mutex;
pub use reader_service::cross_book::*;

pub(crate) fn read(library: &Mutex<PublishedLibrary>, owner: &str, members: Option<&[base_schema::PublishedBookRef]>, name: &str, mut arguments: Value) -> Result<LibraryReadResult, ToolError> {
    if name == "library.search" { return crate::workspace_retrieval::search(library, owner, members, arguments); }
    let service = McpRead::resident(library);
    let (mut body, source) = if name == "library.list" {
        (service.list_for_workspace(owner, members, arguments)?, None)
    } else if name == "library.read" {
        let reference = serde_json::from_value(arguments["published_book_ref"].clone())
            .map_err(|e| memory::teaching::invalid(e.to_string()))?;
        check_member(members, &reference)?;
        let operation = arguments.as_object_mut().and_then(|a| a.remove("operation"))
            .and_then(|v| v.as_str().map(str::to_owned))
            .filter(|op| READ_OPERATIONS.iter().any(|(name,_)| name == op))
            .ok_or_else(|| memory::teaching::invalid("Select an available library read operation"))?;
        let lid=arguments["lid"].clone();
        let mut body = service.call_for_user(owner, &format!("book_{operation}"), arguments)?;
        if operation == "text" { body["data"]["lid"]=lid; }
        body["operation"] = json!(operation);
        let reference = serde_json::from_value(body["published_book_ref"].clone())
            .map_err(|e| memory::teaching::invalid(e.to_string()))?;
        let publication = library.lock().unwrap().load(owner, &reference)?;
        let title = crate::book_title::read(publication.directory(), None).unwrap_or_else(|| "参考材料".into());
        (body, Some(LibrarySource { reference, book: publication.book.clone(), title }))
    } else { return Err(runtime::library::unavailable()); };
    for item in body["items"].as_array_mut().into_iter().flatten() {
        item["available_operations"] = json!(item["available_tools"].as_array().into_iter().flatten()
            .filter_map(|name| name.as_str()?.strip_prefix("book_"))
            .filter(|op| READ_OPERATIONS.iter().any(|(name,_)| name == op)).collect::<Vec<_>>());
        item.as_object_mut().unwrap().remove("available_tools");
        if let Ok(reference) = serde_json::from_value(item["published_book_ref"].clone()) {
            let publication = library.lock().unwrap().load(owner, &reference)?;
            if publication.directory().join("video_source_manifest.json").is_file() {
                let (manifest, _) = publication.book.load_video_source(publication.directory())?;
                item["video"] = json!({"published_book_ref":reference,"video":manifest.source.video,"duration_ms":manifest.source.duration_ms,
                    "transcript":manifest.material.transcript,"tools":["video.search","video.read_frames","video.review_audio"]});
            }
        }
    }
    rewrite_calls(&mut body);
    if let Some(source) = &source { scope_video_hints(&mut body, &source.reference); }
    Ok(LibraryReadResult { body, source })
}

fn rewrite_calls(body: &mut Value) {
    for next in body.get_mut("next_calls").and_then(Value::as_array_mut).into_iter().flatten() {
        let original = next["name"].as_str().unwrap_or_default().to_string();
        if original == "book_list" { next["name"] = json!("library.list"); }
        else if let Some(op) = original.strip_prefix("book_") {
            next["name"] = json!("library.read");
            next["arguments"]["operation"] = json!(op);
        }
    }
    match body {
        Value::Object(fields) => { for value in fields.values_mut() { rewrite_calls(value); } }
        Value::Array(items) => { for value in items { rewrite_calls(value); } }
        _ => {}
    }
}

pub(crate) fn source_view(library: &Mutex<PublishedLibrary>, owner: &str, binding: &runtime::orchestrator::SourceBinding, open: bool) -> Result<Value, ToolError> {
    source_view_at(library, owner, binding, open, None)
}

pub(crate) fn source_window(library: &Mutex<PublishedLibrary>, owner: &str, binding: &SourceBinding, input: &Value) -> Result<Value, ToolError> {
    let reference = base_schema::PublishedBookRef { book_id: binding.book_id.clone(),
        publication_id: binding.publication_id.clone().ok_or_else(crate::service_error::missing)? };
    let publication = library.lock().unwrap().load(owner, &reference)?;
    crate::board_api::source_window(&publication.book, binding, Some(&reference), input)
}

pub(crate) fn source_view_at(library: &Mutex<PublishedLibrary>, owner: &str, binding: &runtime::orchestrator::SourceBinding, open: bool, target_lid: Option<&str>) -> Result<Value, ToolError> {
    if let Some(music) = &binding.music {
        let reference = &music.target.publication;
        let loaded = library.lock().unwrap().load(owner, reference);
        let mut popup = crate::music_source::popup(binding, loaded.as_ref().map(|p|(p.book.as_ref(),p.directory()))
            .map_err(|e|crate::user_storage_paths::error(&e.error_code,&e.category,&e.message)), reference.url("music/audio"));
        // This cross-material source stays in its original popup; the current
        // Reader is not silently replaced by a different publication.
        popup["can_open_in_reader"] = json!(false);
        if open { popup["opened"] = json!(false); }
        return Ok(popup);
    }
    if let Some(media) = &binding.media {
        let loaded = library.lock().unwrap().load(owner, &media.publication);
        return Ok(crate::video_source::popup(binding, loaded.as_ref().map(|p|(p.book.as_ref(),p.directory()))
            .map_err(|e|crate::user_storage_paths::error(&e.error_code,&e.category,&e.message)), media.publication.url("video/media")));
    }
    let reference = base_schema::PublishedBookRef { book_id:binding.book_id.clone(),
        publication_id:binding.publication_id.clone().ok_or_else(crate::service_error::missing)? };
    let publication = library.lock().unwrap().load(owner, &reference)?;
    let range = binding.evidence_range.as_ref().ok_or_else(crate::service_error::missing)?;
    let source = publication.book.resolve_source(range, "zh-CN", Some(&binding.evidence_text_digest))?;
    let title = crate::book_title::read(publication.directory(), None).unwrap_or_else(|| "参考材料".into());
    let first = publication.book.base.lid_nodes.iter().find(|n| n.lid == range.start_lid).ok_or_else(crate::service_error::missing)?;
    let last = publication.book.base.lid_nodes.iter().find(|n| n.lid == range.end_lid).ok_or_else(crate::service_error::missing)?;
    let start = range.ranges.iter().find(|r|r.lid==range.start_lid).map_or(first.span.start, |r|first.span.start+r.range.start as usize);
    let end = range.ranges.iter().find(|r|r.lid==range.end_lid).map_or(last.span.end, |r|last.span.start+r.range.end as usize);
    let mut query = url::form_urlencoded::Serializer::new(String::new());
    query.append_pair("book_id", &reference.book_id).append_pair("publication_id", &reference.publication_id);
    if open && target_lid.is_some() {
        let lid = target_lid.unwrap();
        let node = publication.book.base.lid_nodes.iter().find(|node| node.lid == lid).ok_or_else(crate::service_error::missing)?;
        let leaf = publication.book.base.lid_nodes.iter().filter(|leaf| leaf.children.is_empty() && leaf.path.starts_with(&node.path))
            .min_by(|a,b| a.span.start.cmp(&b.span.start).then_with(|| a.path.cmp(&b.path))).ok_or_else(crate::service_error::missing)?;
        query.append_pair("start_lid", &leaf.lid).append_pair("end_lid", &leaf.lid);
    } else {
        query.append_pair("start_lid", &range.start_lid).append_pair("end_lid", &range.end_lid)
            .append_pair("start_utf16", &start.to_string()).append_pair("end_utf16", &end.to_string());
    }
    let query = query.finish();
    let source_url = format!("/?{query}");
    if open { Ok(json!({"source_ref_id":binding.source_ref_id,"opened":false,"source_url":source_url})) }
    else { Ok(json!({"source_ref_id":binding.source_ref_id,"label":binding.label_snapshot,"material_title":title,
        "heading_path":source.heading_path,"highlighted_quote":source.highlighted_quote,
        "context_before":source.context_before,"context_after":source.context_after,"excerpt":source.excerpt,
        "stale":false,"can_open_in_reader":true,"source_url":source_url})) }
}
