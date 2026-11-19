//! User-level note operations. No ReaderWorkspace or Book is required.
use crate::user_runtime::UserRuntime;
use memory::{NoteContent, NoteRef};
use read_tools::ToolError;
use serde::Deserialize;
use serde_json::{json, Value};

fn invalid(message: &str) -> ToolError {
    crate::user_storage_paths::error("INVALID_NOTE_REQUEST", "validation", message)
}
fn string<'a>(input: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    input[key]
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| invalid("缺少笔记或操作身份"))
}
fn number(input: &Value, key: &str) -> Result<u32, ToolError> {
    input[key]
        .as_u64()
        .and_then(|v| u32::try_from(v).ok())
        .filter(|v| *v > 0)
        .ok_or_else(|| invalid("缺少确切版本"))
}
fn parse<T: for<'a> Deserialize<'a>>(value: Value) -> Result<T, ToolError> {
    serde_json::from_value(value).map_err(|_| invalid("笔记内容格式不完整"))
}

/// Freeze source details while the originating scene is still available. This does not save a note or expose a draft to the Agent.
pub(crate) fn prepare(
    user: &UserRuntime,
    workspace: &crate::reader_workspace::ReaderWorkspace,
    input: &Value,
) -> Result<NoteContent, ToolError> {
    let selection = crate::parse_optional_selection_context(input)?;
    let placement = crate::parse_optional_note_placement(input)?;
    if let Some(value) = &placement {
        crate::validate_note_body_placement(&workspace.book, &workspace.book_dir, value)?;
    }
    if let Some(value) = &selection {
        for range in &value.ranges {
            workspace.book.text(&range.lid, None)?;
        }
    }
    let source = crate::note_api::resolve(
        user,
        workspace,
        input,
        selection.as_ref(),
        placement.as_ref(),
    )?;
    let lid = placement
        .as_ref()
        .map(|value| value.lid().to_owned())
        .or_else(|| {
            selection
                .as_ref()
                .and_then(|value| value.ranges.first().map(|r| r.lid.clone()))
        })
        .or_else(|| input["lid"].as_str().map(String::from));
    let session_id = source.as_ref().and_then(crate::note_api::source_session);
    let turn_id = source.as_ref().and_then(|note| match &note.association {
        memory::NoteAssociation::Answer { turn_id, .. } => Some(turn_id.clone()),
        memory::NoteAssociation::Presentation { receipt, .. } => Some(receipt.turn_id.clone()),
        _ => None,
    });
    Ok(NoteContent {
        content: input["content"].as_str().unwrap_or_default().into(),
        title: None,
        authorship: memory::NoteAuthorship::User,
        context: memory::NoteContext {
            book_id: Some(workspace.book.base.book_id.clone()),
            publication_id: workspace
                .publication
                .as_ref()
                .map(|p| p.reference.publication_id.clone()),
            workspace_id: workspace.learning_workspace_id.clone(),
            lid: lid.clone(),
            session_id,
            turn_id,
            label: input["label"].as_str().map(String::from),
            video_time_ms: None,
            ..Default::default()
        },
        source_refs: vec![],
        change_note: None,
        based_on_turn: None,
        source,
        capture: Some(memory::NoteCapture {
            anchor: memory::Anchor { lid, concept: None },
            selection_context: selection,
            placement,
        }),
    })
}

pub fn dispatch(
    user: &mut UserRuntime,
    action: &str,
    input: Value,
    now: &str,
) -> Result<Value, ToolError> {
    user.store.ensure_storage_available()?;
    let result = match action {
        "chat_create" => crate::personal_chat::create(user, now)?,
        "chat_history" => crate::personal_chat::history(user, string(&input, "session_id")?)?,
        "chats" => {
            json!({"sessions":user.agent_history.sessions.iter().filter(|s|s.book_id.is_empty()&&s.learning_workspace_id.is_none()).map(|s|json!({"session_id":s.id,"title":s.title,"updated_at":s.updated_at})).collect::<Vec<_>>()})
        }
        "search" => crate::personal_note_retrieval::search(user, &input)?,
        "learning_map_relationships" => crate::learning_map_recall::note_relationships(user, &input)?,
        "learning_map_suggestions" => crate::learning_map_recall::suggestions(user, &input)?,
        "learning_map_remove" => crate::learning_map_recall::remove_touch(user, &input)?,
        "attach_board" => crate::board_api::attach_personal_note(user, &input, now)?,
        "board_locations" => crate::board_api::personal_note_locations(user, string(&input,"note_id")?)?,
        "unattach_board" => crate::board_api::remove_personal_note_location(user, &input, now)?,
        "attachment_context" => {
            let target: memory::NoteAttachmentTarget = parse(input["target"].clone())?;
            let session = user.agent_history.sessions.iter().find(|s|s.id == target.session_id()).ok_or_else(|| invalid("原对话当前不可用"))?;
            let turn_id = match &target { memory::NoteAttachmentTarget::Message {turn_id,..} => turn_id, memory::NoteAttachmentTarget::Presentation {receipt,..} => &receipt.turn_id };
            let turn = session.turns.iter().find(|t|t.turn_id == *turn_id).ok_or_else(|| invalid("原消息当前不可用"))?;
            json!(memory::NoteContext {book_id:(!session.book_id.is_empty()).then(||session.book_id.clone()),publication_id:turn.published_book_ref.as_ref().map(|p|p.publication_id.clone()),workspace_id:session.learning_workspace_id.clone(),session_id:Some(session.id.clone()),turn_id:Some(turn_id.clone()),origin:Some(target),..Default::default()})
        }
        "attachments" => {
            if input["note_id"].as_str().is_none() && input["session_id"].as_str().is_none() {
                return Err(invalid("请选择想法或对话"));
            }
            json!({"items":user.store.note_attachments(input["note_id"].as_str(), input["session_id"].as_str())})
        }
        "attach" => {
            let mut target: memory::NoteAttachmentTarget = parse(input["target"].clone())?;
            let session = user.agent_history.sessions.iter().find(|s| s.id == target.session_id())
                .ok_or_else(|| invalid("原对话当前不可用"))?;
            let label = match &mut target {
                memory::NoteAttachmentTarget::Message { turn_id, role, quote, .. } => {
                    let turn = session.turns.iter().find(|t| t.turn_id == *turn_id).ok_or_else(|| invalid("原消息当前不可用"))?;
                    let text = match role {
                        memory::NoteMessageRole::User => turn.user_authored_text(),
                        memory::NoteMessageRole::Assistant => turn.outcome.as_ref().and_then(|o| o.answer.as_deref()).unwrap_or_default(),
                    };
                    if text.trim().is_empty() { return Err(invalid("这条消息没有可关联的文字")); }
                    format!("{} · {}", session.title, quote.as_deref().unwrap_or("这条消息"))
                }
                memory::NoteAttachmentTarget::Presentation { receipt, visible_step } => {
                    if !session.turns.iter().any(|t| t.turn_id == receipt.turn_id) { return Err(invalid("原演示当前不可用")); }
                    let follow_up = parse::<runtime::presentation::PresentationFollowUp>(json!(receipt))?;
                    let owner = runtime::presentation::PresentationOwner { book_id: session.book_id.clone(), session_id: session.id.clone() };
                    let store = crate::presentation_store::PresentationStore::for_user(user)?;
                    let version = store.read_version(&owner, &follow_up.reference)?;
                    let saved = store.read_retained_state(&owner, &follow_up)?;
                    *visible_step = saved.state.visible_step;
                    format!("{} · {}", version.content.title, visible_step.as_deref().unwrap_or("记录的演示状态"))
                }
            };
            json!(user.store.attach_personal_note(string(&input, "note_id")?, target, label, now)?)
        }
        "unattach" => {
            user.store.remove_note_attachment(string(&input, "attachment_id")?)?;
            json!({"removed":true})
        }
        "list" => {
            let limit = input["limit"].as_u64().unwrap_or(40).clamp(1, 99) as usize;
            let notes = user.store.personal_notes(
                input["book_id"].as_str(),
                input["before"].as_str(),
                limit + 1,
            );
            let has_more = notes.len() > limit;
            let notes: Vec<_> = notes.into_iter().take(limit).collect();
            json!({"next_cursor":has_more.then(||notes.last().map(|n|n.note_id.clone())).flatten(),"notes":notes})
        }
        "read" => json!(user
            .store
            .personal_note(&parse::<NoteRef>(input.clone())?)?),
        "thinking" => crate::note_thinking::page(user,&input)?,
        "thinking_evidence" => crate::note_thinking::evidence(user,&input,false)?,
        "thinking_correct" => crate::note_thinking::evidence(user,&input,true)?,
        "thinking_retry" => crate::note_thinking::retry(user,&input,now)?,
        "thinking_start" => crate::note_thinking::start(user,&input)?,
        "history" => {
            json!({"revisions":user.store.personal_note_history(string(&input,"note_id")?,input["before"].as_u64().and_then(|v|v.try_into().ok()),input["limit"].as_u64().unwrap_or(20) as usize)?,
            "discussions":user.store.note_discussions(string(&input,"note_id")?)})
        }
        "compare" => {
            let id = string(&input, "note_id")?;
            json!({"left":user.store.personal_note(&NoteRef{note_id:id.into(),revision:Some(number(&input,"left")?)})?,
                "right":user.store.personal_note(&NoteRef{note_id:id.into(),revision:Some(number(&input,"right")?)})?})
        }
        "discussion" => {
            let session = user
                .agent_history
                .sessions
                .iter()
                .find(|s| s.id == input["session_id"].as_str().unwrap_or_default())
                .ok_or_else(|| invalid("原讨论当前不可用"))?;
            let turn = session
                .turns
                .iter()
                .find(|t| t.turn_id == input["turn_id"].as_str().unwrap_or_default())
                .ok_or_else(|| invalid("原回合当前不可用"))?;
            json!({"session_id":session.id,"title":session.title,"personal":session.book_id.is_empty(),"turn":crate::scoped_turn_view(None,turn)})
        }
        "from_turn" | "from_assistant_turn" => {
            let session = user
                .agent_history
                .sessions
                .iter()
                .find(|s| s.id == input["session_id"].as_str().unwrap_or_default())
                .ok_or_else(|| invalid("原讨论当前不可用"))?;
            let turn = session
                .turns
                .iter()
                .find(|t| t.turn_id == input["turn_id"].as_str().unwrap_or_default())
                .ok_or_else(|| invalid("原表达当前不可用"))?;
            let assistant = action == "from_assistant_turn";
            let content = if assistant {
                turn.outcome
                    .as_ref()
                    .and_then(|o| o.answer.as_deref())
                    .ok_or_else(|| invalid("此回合没有已保存的回答"))?
            } else {
                turn.user_authored_text()
            };
            if content.trim().is_empty() {
                return Err(invalid("此回合没有自己的文字可以保存"));
            }
            let mut source_refs: Vec<NoteRef> = turn
                .domain
                .personal_notes
                .as_ref()
                .and_then(|v| serde_json::from_value(v["references"].clone()).ok())
                .unwrap_or_default();
            if assistant {
                for reference in turn
                    .domain
                    .personal_notes
                    .as_ref()
                    .and_then(|v| {
                        serde_json::from_value::<Vec<NoteRef>>(v["adopted_references"].clone()).ok()
                    })
                    .unwrap_or_default()
                {
                    if !source_refs.contains(&reference) {
                        source_refs.push(reference);
                    }
                }
            }
            json!(NoteContent {
                content: content.into(),
                title: None,
                authorship: if assistant {
                    memory::NoteAuthorship::AssistantDraft
                } else {
                    memory::NoteAuthorship::User
                },
                context: memory::NoteContext {
                    book_id: (!session.book_id.is_empty()).then(|| session.book_id.clone()),
                    publication_id: turn
                        .published_book_ref
                        .as_ref()
                        .map(|p| p.publication_id.clone()),
                    workspace_id: session.learning_workspace_id.clone(),
                    session_id: Some(session.id.clone()),
                    turn_id: Some(turn.turn_id.clone()),
                    label: Some("讨论中的表达".into()),
                    ..Default::default()
                },
                source_refs,
                change_note: None,
                based_on_turn: Some(memory::NoteTurnRef {
                    session_id: session.id.clone(),
                    turn_id: turn.turn_id.clone()
                }),
                source: None,
                capture: None
            })
        }
        "create" => {
            let content: NoteContent = parse(input["value"].clone())?;
            json!(user
                .store
                .create_personal_note(content, string(&input, "operation_id")?, now)?)
        }
        "revise" => {
            let content: NoteContent = parse(input["value"].clone())?;
            let id = string(&input, "note_id")?;
            json!(user.store.revise_personal_note(
                id,
                number(&input, "base_revision")?,
                content,
                string(&input, "operation_id")?,
                now
            )?)
        }
        "restore" => json!(user.store.restore_personal_note(
            string(&input, "note_id")?,
            number(&input, "base_revision")?,
            number(&input, "revision")?,
            string(&input, "operation_id")?,
            now
        )?),
        "detach" => {
            user.store
                .detach_personal_note(string(&input, "note_id")?)?;
            json!({"status":"detached"})
        }
        "delete" => {
            user.store.delete(string(&input, "note_id")?)?;
            json!({"status":"deleted"})
        }
        "links" => json!({"links":user.store.note_links(string(&input,"note_id")?)}),
        "link" => json!(user.store.link_personal_notes(
            parse(input["from"].clone())?,
            parse(input["to"].clone())?,
            input["reason"]
                .as_str()
                .filter(|s| !s.trim().is_empty())
                .map(String::from),
            now
        )?),
        "unlink" => {
            user.store
                .unlink_personal_notes(string(&input, "link_id")?)?;
            json!({"status":"removed"})
        }
        _ => return Err(invalid("不支持的笔记操作")),
    };
    if matches!(action, "create" | "revise" | "restore" | "delete") {
        crate::personal_note_retrieval::schedule(&user.store);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn note_placements_validate_messages_and_freeze_the_actual_presentation_page() {
        let (_root, mut state, metadata) = crate::tests::board_browser_host::fixture();
        let now = "2026-10-30T10:00:00Z";
        let note = dispatch(&mut state.user,"create",json!({"value":{"content":"这是我的理解","authorship":"user"},"operation_id":"placement-note"}),now).unwrap();
        let target = json!({"kind":"message","session_id":metadata["session_id"],"turn_id":metadata["turn_id"],"role":"user","quote":"某一句话","occurrence":0});
        let first = dispatch(&mut state.user,"attach",json!({"note_id":note["note_id"],"target":target}),now).unwrap();
        assert_eq!(first,dispatch(&mut state.user,"attach",json!({"note_id":note["note_id"],"target":target}),now).unwrap());
        let mut absent = target.clone(); absent["session_id"] = json!("not-this-users-chat");
        assert!(dispatch(&mut state.user,"attach",json!({"note_id":note["note_id"],"target":absent}),now).is_err());
        let receipt = state.private_context().save_presentation_state(metadata["session_id"].as_str().unwrap(),metadata["turn_id"].as_str().unwrap(),&parse(metadata["reference"].clone()).unwrap(),parse(json!({"values":{"return_speed":2},"visible_step":"比较两段用时","observed_result":"总时间","source_ref_ids":[]})).unwrap()).unwrap();
        let placed = dispatch(&mut state.user,"attach",json!({"note_id":note["note_id"],"target":{"kind":"presentation","receipt":receipt,"visible_step":"客户端过时的页"}}),now).unwrap();
        assert_eq!(placed["target"]["visible_step"],"比较两段用时");
        let context = dispatch(&mut state.user,"attachment_context",json!({"target":placed["target"]}),now).unwrap();
        assert_eq!(context["origin"]["receipt"],json!(receipt));
        let mut wrong = json!(receipt); wrong["state_revision"] = json!(99999);
        assert!(dispatch(&mut state.user,"attach",json!({"note_id":note["note_id"],"target":{"kind":"presentation","receipt":wrong,"visible_step":null}}),now).is_err());
        assert_eq!(dispatch(&mut state.user,"read",json!({"note_id":note["note_id"]}),now).unwrap(),note);
    }

    #[test]
    fn note_placements_board_retries_do_not_duplicate_cards_or_links_and_removal_keeps_note() {
        let (_root, mut state, metadata) = crate::tests::board_browser_host::fixture();
        let now = "2026-10-30T10:00:00Z";
        let board = crate::board_api::route(&state.user,&state.workspace,"create",&json!({"title":"有想法的画布"}),now).unwrap();
        let board = crate::board_api::route(&state.user,&state.workspace,"import",&json!({"board_id":board["board_id"],"board_revision":board["revision"],"source":metadata["sources"]["prebuild"]}),now).unwrap();
        let note = dispatch(&mut state.user,"create",json!({"value":{"content":"想法的正文","authorship":"user","context":{"label":"原阅读现场"}},"operation_id":"board-placement-note"}),now).unwrap();
        let input = json!({"note_id":note["note_id"],"board_id":board["board_id"],"workspace_id":state.workspace.learning_workspace_id,"item_id":board["items"][0]["item_id"]});
        let placed = dispatch(&mut state.user,"attach_board",input.clone(),now).unwrap();
        assert_eq!(placed,dispatch(&mut state.user,"attach_board",input,now).unwrap());
        let current = crate::board_api::route(&state.user,&state.workspace,"read",&json!({"board_id":board["board_id"]}),now).unwrap();
        assert_eq!(current["document"]["items"].as_array().unwrap().len(),2);
        assert_eq!(current["document"]["links"].as_array().unwrap().len(),1);
        let locations = dispatch(&mut state.user,"board_locations",json!({"note_id":note["note_id"]}),now).unwrap();
        assert_eq!(locations["items"].as_array().unwrap().len(),1);
        let mut remove = locations["items"][0].clone(); remove["note_id"] = note["note_id"].clone();
        dispatch(&mut state.user,"unattach_board",remove,now).unwrap();
        assert_eq!(dispatch(&mut state.user,"read",json!({"note_id":note["note_id"]}),now).unwrap(),note);
        let current = crate::board_api::route(&state.user,&state.workspace,"read",&json!({"board_id":board["board_id"]}),now).unwrap();
        assert!(current["document"]["links"].as_array().unwrap().is_empty());
    }
    #[test]
    fn prepared_source_survives_leaving_material_and_reopening_personal_store() {
        let directory = tempfile::tempdir().unwrap();
        let mut state = crate::tests::state_named(&format!("pn-capture-{}", uuid::Uuid::now_v7()));
        let path = directory.path().join("memory.json");
        state.user.store = memory::MemoryStore::open(&path).unwrap();
        let input = json!({"content":"","selection_context":{"status":"resolved","raw_quote":"XXX","resolved_quote":"XXX","ranges":[{"lid":"1.1","range":{"start":0,"end":3}}]},
            "note":{"association":{"kind":"selection"},"retained_excerpt":"XXX"},"label":"原资料"});
        let prepared = prepare(&state.user, &state.workspace, &input).unwrap();
        let book_id = prepared.context.book_id.clone();
        // Personal save has no workspace parameter: the source cannot be rebound to a later Reader.
        let saved = dispatch(
            &mut state.user,
            "create",
            json!({"value":prepared,"operation_id":"capture"}),
            "t1",
        )
        .unwrap();
        let store = memory::MemoryStore::open(path).unwrap();
        let note = store
            .personal_note(&NoteRef {
                note_id: saved["note_id"].as_str().unwrap().into(),
                revision: None,
            })
            .unwrap();
        assert_eq!(note.value.context.book_id, book_id);
        assert_eq!(note.value.record.content, "");
        assert!(
            matches!(note.value.record.note.unwrap().retained_excerpt,Some(memory::NoteExcerpt::Original{text}) if text=="XXX")
        );
        assert_eq!(
            note.value.record.selection_context.unwrap().ranges[0].lid,
            "1.1"
        );
    }
    #[test]
    fn personal_note_api_is_book_independent_and_preserves_original_on_conflict() {
        let dir = tempfile::tempdir().unwrap();
        let store = memory::MemoryStore::open(dir.path().join("memory.json")).unwrap();
        let mut user = UserRuntime::local(store, None, crate::AgentHistory::default(), None);
        let value = json!({"content":"自己的想法","authorship":"user"});
        let note = dispatch(
            &mut user,
            "create",
            json!({"value":value,"operation_id":"create"}),
            "t1",
        )
        .unwrap();
        let id = note["note_id"].as_str().unwrap();
        let second=dispatch(&mut user,"revise",json!({"note_id":id,"base_revision":1,"value":{"content":"新的表述","authorship":"user"},"operation_id":"edit"}),"t2").unwrap();
        assert_eq!(second["current_revision"], 2);
        assert_eq!(
            dispatch(
                &mut user,
                "revise",
                json!({"note_id":id,"base_revision":1,"value":value,"operation_id":"stale"}),
                "t3"
            )
            .unwrap_err()
            .error_code,
            "NOTE_REVISION_CONFLICT"
        );
        assert_eq!(
            dispatch(&mut user, "read", json!({"note_id":id,"revision":1}), "t3").unwrap()["value"]
                ["record"]["content"],
            "自己的想法"
        );
    }
}
