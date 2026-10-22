//! Resident access to authorized publications and the same reader's LearningMemory.
use crate::{mcp_read::McpRead, published_library::PublishedLibrary};
use read_tools::ToolError;
use runtime::library::{LibraryReadResult, LibrarySource, READ_OPERATIONS};
use serde_json::{json, Value};
use std::sync::Mutex;
use memory::{teaching::{TeachingBinding, TeachingEvent, TeachingFact, TeachingTarget, TeachingSourceBinding, invalid},
    learning_evidence::{LearningEvidence, EvidenceNature}, assessment::AssessmentStatus};
use crate::{AgentTurnRef, PrivateBookContext, user_runtime::UserRuntime};
use runtime::orchestrator::SourceBinding;

pub(crate) fn check_member(members: Option<&[base_schema::PublishedBookRef]>, reference: &base_schema::PublishedBookRef) -> Result<(), ToolError> {
    if members.is_some_and(|members| !members.contains(reference)) {
        return Err(crate::user_storage_paths::error("WORKSPACE_MATERIAL_UNAVAILABLE", "not_found", "This publication is outside the run's learning workspace"));
    }
    Ok(())
}

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

/// Hints keep the same immutable publication when followed from a different book.
pub(crate) fn scope_video_hints(value: &mut Value, reference: &base_schema::PublishedBookRef) {
    match value {
        Value::Object(fields) => {
            if fields.contains_key("segment_id") && fields.contains_key("text_range") && fields.contains_key("location") {
                fields.insert("tool".into(), json!("library.read"));
                fields.insert("arguments".into(), json!({"published_book_ref":reference,"operation":"text","lid":fields["text_range"]["lid"]}));
            }
            if fields.contains_key("sampling") && fields.contains_key("location") {
                fields.insert("published_book_ref".into(), json!(reference));
            }
            if fields.get("tool").and_then(Value::as_str) == Some("book.text") {
                fields.insert("tool".into(), json!("library.read"));
                if let Some(arguments) = fields.get_mut("arguments") {
                    arguments["published_book_ref"] = json!(reference);
                    arguments["operation"] = json!("text");
                }
            }
            for child in fields.values_mut() { scope_video_hints(child, reference); }
        }
        Value::Array(items) => for child in items { scope_video_hints(child, reference); },
        _ => {}
    }
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

fn binding(book: &read_tools::Book, turn: &AgentTurnRef) -> TeachingBinding {
    TeachingBinding { tutor_session_id:format!("observation:{}",book.base.book_id), session_revision:0,control_revision:0,
        source_id:book.base.book_id.clone(),source_revision:book.source_fingerprint().into(),map_revision:None,
        chat_session_id:turn.session_id.clone(),turn_id:turn.turn_id.clone() }
}

pub(crate) fn learning(state: &PrivateBookContext<'_>, turn: &AgentTurnRef, reference: &base_schema::PublishedBookRef,
    request: &Value, sources: &[SourceBinding], now: &str) -> Result<Value, ToolError> {
    let bind=binding(state.book,turn);
    if request["operation"] == "read" {
        let mut result=crate::teaching::understanding(state,&bind.source_id,None,request["after"].as_u64().unwrap_or(0))?;
        result["published_book_ref"]=json!(reference);
        result["source_revision"]=json!(bind.source_revision);
        result["recent_facts"]=json!(state.user.learning_store_for_workspace(state.learning_workspace_id)?.teaching_recent(&bind.tutor_session_id,12)?.into_iter()
            .filter(|e|e.binding.source_revision==bind.source_revision).take(8).collect::<Vec<_>>());
        return Ok(result);
    }
    if request["operation"] != "observe" { return Err(invalid("未知参考书理解操作")); }
    let required=|key:&str| request[key].as_str().filter(|v|!v.trim().is_empty()).ok_or_else(||invalid(format!("缺少 {key}")));
    let quote=required("learner_quote")?;
    let session=state.user.agent_history.sessions.iter().find(|s|s.id==turn.session_id && s.learning_workspace_id.as_deref()==state.learning_workspace_id).ok_or_else(||invalid("原工作区聊天不存在"))?;
    let current=session.turns.iter().find(|t|t.turn_id==turn.turn_id).ok_or_else(||invalid("原回合不存在"))?;
    if !current.user_authored_text().contains(quote) { return Err(invalid("理解记录必须引用当前用户的真实表达")); }
    let ids:Vec<String>=serde_json::from_value(request["source_ref_ids"].clone()).map_err(|e|invalid(e.to_string()))?;
    if ids.is_empty() { return Err(invalid("理解记录需要本轮读过的参考来源")); }
    let cited=ids.iter().map(|id|sources.iter().find(|s|&s.source_ref_id==id
        && s.book_id==reference.book_id && s.publication_id.as_deref()==Some(&reference.publication_id)
        && s.source_revision.as_deref()==Some(state.book.source_fingerprint()))
        .cloned().ok_or_else(||invalid("来源未在本轮读取，或不属于指定参考书版本"))).collect::<Result<Vec<_>,_>>()?;
    let media_bindings:Vec<_>=cited.iter().filter_map(|source|source.media.as_ref())
        .map(|media|memory::teaching::TeachingMediaBinding {source_id:bind.source_id.clone(),source_revision:bind.source_revision.clone(),target:media.target.clone()}).collect();
    let music_bindings:Vec<_>=cited.iter().filter_map(|source|source.music.as_ref())
        .map(|music|memory::teaching::TeachingMusicBinding {source_id:bind.source_id.clone(),source_revision:bind.source_revision.clone(),target:music.target.clone()}).collect();
    let target=TeachingTarget {learning_focus:required("learning_focus")?.into(),expected_performance:required("expected_performance")?.into(),
        capability:required("capability")?.into(),object_refs:vec![],media_bindings:(!media_bindings.is_empty()).then_some(media_bindings),
        music_bindings:(!music_bindings.is_empty()).then_some(music_bindings),
        source_bindings:cited.iter().flat_map(|s| {
            let ranges = s.evidence_range.iter().map(|r|(r.start_lid.clone(),r.end_lid.clone()))
                .chain(s.media.iter().flat_map(|media| runtime::video_source::members(&media.target)).filter_map(|target| {
                    if let base_schema::EvidenceTarget::Text { text_range, .. } = target { Some((text_range.lid.clone(),text_range.lid.clone())) } else { None }
                }));
            ranges.map(|(start_lid,end_lid)|TeachingSourceBinding {source_id:bind.source_id.clone(),source_revision:bind.source_revision.clone(),start_lid,end_lid})
        }).collect()};
    target.validate(&bind)?;
    let mut store=state.user.learning_store_for_workspace(state.learning_workspace_id)?;
    let assistance=store.teaching_recent(&bind.tutor_session_id,12)?.into_iter()
        .filter(|e|e.binding.source_revision==bind.source_revision && e.binding.chat_session_id==turn.session_id
            && e.binding.turn_id!=turn.turn_id && e.payload["usage"]["kind"]=="assistant_delivery")
        .map(|e|e.event_id).collect::<Vec<_>>();
    let fact=store.append_teaching(&TeachingEvent {event_id:format!("library-user:{}:{}:{}",turn.turn_id,reference.book_id,reference.publication_id),
        binding:bind.clone(),kind:TeachingFact::UsageObserved,causal_refs:assistance.clone(),occurred_at:now.into(),
        payload:json!({"usage":{"kind":"user_message"},"response":current.user_authored_text(),"published_book_ref":reference,
            "original_ref":{"kind":"chat_turn","book_id":session.book_id,"session_id":session.id,"turn_id":current.turn_id},"assistance_refs":assistance})})?;
    let evidence=LearningEvidence {evidence_id:format!("library-interpretation:{}:{}:{}:{}",turn.turn_id,reference.book_id,reference.publication_id,required("operation_id")?),
        action_ref:fact.event_id.clone(),delivery_ref:None,design_ref:None,block_refs:vec![],session_id:bind.tutor_session_id,source_id:bind.source_id.clone(),source_revision:bind.source_revision,map_revision:None,
        object_id:None,object_revision:None,nature:EvidenceNature::Hypothesis,label:target.learning_focus.clone(),capability:target.capability.clone(),target:Some(target),
        fact_refs:vec![fact.event_id],teaching_implication:required("teaching_implication")?.into(),assistance_refs:assistance,attempt:0,assessment_ref:None,
        status:AssessmentStatus::Unassessed,interpretation:required("interpretation")?.into(),learner_quote:quote.into(),interpreter_version:"resident.hypothesis.v1".into(),
        supersedes:request["supersedes"].as_str().map(str::to_owned),correction:None,source_quotes:json!(cited)};
    let accepted=store.append_evidence(&evidence)?;
    store.rebuild_learner_projection(&bind.source_id)?;
    Ok(json!({"evidence_ref":accepted.evidence_id,"nature":accepted.nature,"status":accepted.status,"published_book_ref":reference,
        "teaching_implication":accepted.teaching_implication}))
}

/// Called only for a validated committed delivery. Persisted source versions also cover reused presentations.
pub(crate) fn record_delivery(user:&UserRuntime, session:&crate::AgentChatSession, turn:&crate::AgentChatTurn, outcome:&runtime::orchestrator::OuterOutcome, now:&str) -> Result<(),ToolError> {
    let mut references=std::collections::BTreeMap::new();
    for source in &turn.source_bindings {
        if let Some(publication_id)=&source.publication_id {
            references.entry(base_schema::PublishedBookRef {book_id:source.book_id.clone(),publication_id:publication_id.clone()}).or_insert_with(Vec::new).push(source);
        }
    }
    if references.is_empty() { return Ok(()); }
    let mut store=user.learning_store_for_workspace(session.learning_workspace_id.as_deref())?;
    for (reference,sources) in references {
        let revision=sources[0].source_revision.clone().ok_or_else(||invalid("参考来源缺少原文版本"))?;
        let binding=TeachingBinding {tutor_session_id:format!("observation:{}",reference.book_id),session_revision:0,control_revision:0,
            source_id:reference.book_id.clone(),source_revision:revision,map_revision:None,chat_session_id:session.id.clone(),turn_id:turn.turn_id.clone()};
        store.append_teaching(&TeachingEvent {event_id:format!("library-delivery:{}:{}:{}",turn.turn_id,reference.book_id,reference.publication_id),
            binding,kind:TeachingFact::UsageObserved,causal_refs:vec![],occurred_at:now.into(),
            payload:json!({"usage":{"kind":"assistant_delivery"},"published_book_ref":reference,"answer":outcome.answer,"answer_view":outcome.answer_view,
                "source_bindings":sources,"original_ref":{"kind":"chat_turn","book_id":session.book_id,"session_id":session.id,"turn_id":turn.turn_id}})})?;
    }
    Ok(())
}

pub(crate) fn source_view(library: &Mutex<PublishedLibrary>, owner: &str, binding: &runtime::orchestrator::SourceBinding, open: bool) -> Result<Value, ToolError> {
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
        publication_id:binding.publication_id.clone().ok_or_else(crate::authorization::missing)? };
    let publication = library.lock().unwrap().load(owner, &reference)?;
    let range = binding.evidence_range.as_ref().ok_or_else(crate::authorization::missing)?;
    let source = publication.book.resolve_source(range, "zh-CN", Some(&binding.evidence_text_digest))?;
    let title = crate::book_title::read(publication.directory(), None).unwrap_or_else(|| "参考材料".into());
    let first = publication.book.base.lid_nodes.iter().find(|n| n.lid == range.start_lid).ok_or_else(crate::authorization::missing)?;
    let last = publication.book.base.lid_nodes.iter().find(|n| n.lid == range.end_lid).ok_or_else(crate::authorization::missing)?;
    let start = range.ranges.iter().find(|r|r.lid==range.start_lid).map_or(first.span.start, |r|first.span.start+r.range.start as usize);
    let end = range.ranges.iter().find(|r|r.lid==range.end_lid).map_or(last.span.end, |r|last.span.start+r.range.end as usize);
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("book_id", &reference.book_id).append_pair("publication_id", &reference.publication_id)
        .append_pair("start_lid", &range.start_lid).append_pair("end_lid", &range.end_lid)
        .append_pair("start_utf16", &start.to_string()).append_pair("end_utf16", &end.to_string()).finish();
    let source_url = format!("/?{query}");
    if open { Ok(json!({"source_ref_id":binding.source_ref_id,"opened":false,"source_url":source_url})) }
    else { Ok(json!({"source_ref_id":binding.source_ref_id,"label":binding.label_snapshot,"material_title":title,
        "heading_path":source.heading_path,"highlighted_quote":source.highlighted_quote,
        "context_before":source.context_before,"context_after":source.context_after,"excerpt":source.excerpt,
        "stale":false,"can_open_in_reader":true,"source_url":source_url})) }
}
