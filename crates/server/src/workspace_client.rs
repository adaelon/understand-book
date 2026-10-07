//! Browser commands borrow the same user authority and the explicitly authorized scene.
use crate::*;

pub(crate) fn reply(reply: Reply) -> Result<Value, ToolError> {
    if reply.status == 200 { serde_json::from_str(&reply.body).map_err(|_| invalid_note_request("INVALID_RESPONSE", "Response unavailable")) }
    else {
        let value: Value = serde_json::from_str(&reply.body).unwrap_or_default();
        Err(ToolError { error_code: value["error_code"].as_str().unwrap_or("COMMAND_FAILED").into(), category: value["category"].as_str().unwrap_or("unavailable").into(), message: value["message"].as_str().unwrap_or("Operation unavailable").into() })
    }
}

pub(crate) fn read(user: &user_runtime::UserRuntime, workspace: &reader_workspace::ReaderWorkspace, action: &str, input: &Value, now: &str) -> Option<Result<Value, ToolError>> {
    let private = PrivateBookContext { user, book: &workspace.book, book_dir: &workspace.book_dir, messages: &workspace.messages, selected_chat: workspace.selected_chat.as_deref() };
    Some(match action {
        "profile/manifest" => reply(route_profile_manifest(&workspace.book, &HashMap::new())),
        "chat/history" => {
            if let Some(selected) = &workspace.selected_chat {
                agent_history_response(&user.agent_history, &workspace.book, Some(selected)).map(|v| json!(v))
            } else {
                let sessions = agent_session_summaries(&user.agent_history, &workspace.book);
                Ok(json!({"active_session_id":"", "sessions":sessions, "current":{"id":"", "book_id":workspace.book.base.book_id,"title":"", "turns":[],"goals":[]}}))
            }
        }
        "memory/recall" => {
            let get = |k: &str| input[k].as_str().map(str::to_owned);
            Ok(json!(user.store.recall(&RecallQuery { book_id: Some(workspace.book.base.book_id.clone()), lid: get("lid"), mem_type: get("type"), layer: get("layer"), text: get("text") })))
        }
        "agent/source.resolve" => (|| {
            let request = parse_agent_source_request(&input.to_string()).map_err(|r| reply(r).unwrap_err())?;
            let binding = workspace_source_binding(user, workspace, &request)?;
            let resolved = workspace.book.resolve_source(&binding.evidence_range, "zh-CN", Some(&binding.evidence_text_digest));
            Ok(match resolved {
                Ok(source) => json!({"source_ref_id":binding.source_ref_id,"label":source.label,"heading_path":source.heading_path,"highlighted_quote":source.highlighted_quote,"context_before":source.context_before,"context_after":source.context_after,"excerpt":source.excerpt,"stale":false,"can_open_in_reader":true}),
                Err(_) => json!({"source_ref_id":binding.source_ref_id,"label":binding.label_snapshot,"highlighted_quote":binding.preview_snapshot,"context_before":"","context_after":"","stale":true,"can_open_in_reader":false}),
            })
        })(),
        "tutor/understanding" | "tutor/evidence" | "tutor/evidence-list" | "tutor/correct" | "tutor/start" | "tutor/activities" | "tutor/display" | "tutor/action" | "tutor/help-displayed" | "tutor/feedback-displayed" => {
            reply(teaching::route(&private, Req { method: "POST", url: &format!("/{action}"), body: &input.to_string(), now }))
        }
        _ => return None,
    })
}

pub(crate) fn memory(user: &mut user_runtime::UserRuntime, workspace: &reader_workspace::ReaderWorkspace, action: &str, input: &Value, now: &str) -> Result<Value, ToolError> {
    let get = |key: &str| input[key].as_str().ok_or_else(|| invalid_note_request("INVALID_MEMORY_REQUEST", "Missing memory field"));
    if action != "memory/save" {
        let id = get("mem_id")?;
        if !user.store.recall(&RecallQuery { book_id: Some(workspace.book.base.book_id.clone()), ..Default::default() }).iter().any(|r| r.mem_id == id) { return Err(authorization::missing()); }
    }
    match action {
        "memory/save" if input["type"] == "note" => save_user_note(user, workspace, input, now).map(|v| json!(v)),
        "memory/save" => {
            let anchor = get("anchor_lid")?; workspace.book.text(anchor, None)?;
            user.store.save(SaveInput { mem_id: None, mem_type: get("type")?.into(), layer: input["layer"].as_str().unwrap_or("long_term").into(), book_id: workspace.book.base.book_id.clone(), anchor: Anchor { lid: Some(anchor.into()), concept: None }, content: get("content")?.into(), range: None, selection_context: parse_optional_selection_context(input)?, note_placement: None, citations: None, source_session_id: workspace.selected_chat.clone() }, now).map(|v| json!(v))
        }
        "memory/replace" => user.store.replace(ReplaceInput { mem_id: get("mem_id")?.into(), content: get("content")?.into(), selection_context: parse_optional_selection_context(input)? }, now).map(|v| json!(v)),
        "memory/delete" => { user.store.delete(get("mem_id")?)?; Ok(json!({"ok":true})) }
        "memory/reanchor" => reply(route_note_reanchor(user, workspace, input)),
        "memory/promote" => user.store.promote(PromoteInput { mem_id: get("mem_id")?.into(), from_layer: "session".into(), to_layer: "long_term".into() }).map(|v| json!(v)),
        _ => Err(authorization::forbidden()),
    }
}
