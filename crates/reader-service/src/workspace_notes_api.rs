//! Reusable notes belong to the accepted learning workspace, independently of a book or board.
use crate::*;
use memory::workspace_notes::{
    WorkspaceNote, WorkspaceNoteAuthor, WorkspaceNoteOrigin, WorkspaceNoteOriginKind,
    WorkspaceNoteRelation, WorkspaceNoteStore,
};

fn invalid(message: impl Into<String>) -> ToolError {
    invalid_note_request("INVALID_WORKSPACE_NOTE", &message.into())
}

fn string<'a>(input: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    input[key]
        .as_str()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| invalid(format!("Missing {key}")))
}

fn revision(input: &Value, key: &str) -> Result<u32, ToolError> {
    input[key]
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value > 0)
        .ok_or_else(|| invalid(format!("{key} is required")))
}

pub(crate) fn scoped_store(
    user: &user_runtime::UserRuntime,
    scope: Option<&str>,
) -> Result<WorkspaceNoteStore, ToolError> {
    if user.user_id().is_none() {
        return Err(service_error::missing());
    }
    let scope = scope.ok_or_else(|| invalid("Open a learning workspace to use its notebook"))?;
    let root = user
        .history_path
        .as_ref()
        .and_then(|path| path.parent())
        .ok_or_else(|| invalid("Private persistent storage is required"))?;
    WorkspaceNoteStore::open(root, Some(scope), true)
}

fn new_note(
    scope: Option<&str>,
    title: String,
    content: String,
    author: WorkspaceNoteAuthor,
    now: &str,
) -> WorkspaceNote {
    WorkspaceNote {
        note_id: format!("note-{}", uuid::Uuid::now_v7()),
        learning_workspace_id: scope.map(str::to_owned),
        title,
        content,
        revision: 1,
        created_at: now.into(),
        updated_at: now.into(),
        author,
        updated_by: author,
        origin: None,
        source_bindings: vec![],
    }
}

fn restore_source_publications(note: &mut WorkspaceNote) {
    let Some(origin) = &note.origin else { return };
    let Some(publication_id) = &origin.publication_id else { return };
    for binding in &mut note.source_bindings {
        if binding.book_id == origin.book_id && binding.publication_id.is_none() {
            binding.publication_id = Some(publication_id.clone());
        }
    }
}

fn execute(
    user: &user_runtime::UserRuntime,
    scope: Option<&str>,
    operation: &str,
    input: &Value,
    actor: WorkspaceNoteAuthor,
    now: &str,
) -> Result<Value, ToolError> {
    let store = scoped_store(user, scope)?;
    match operation {
        "list" => {
            let mut notebook = store.list()?;
            if let Some(query) = input["query"]
                .as_str()
                .filter(|query| !query.trim().is_empty())
            {
                let query = query.trim().to_lowercase();
                notebook.notes.retain(|note| {
                    note.title.to_lowercase().contains(&query)
                        || note.content.to_lowercase().contains(&query)
                });
                notebook.relations.retain(|relation| {
                    notebook.notes.iter().any(|note| {
                        note.note_id == relation.from_note_id || note.note_id == relation.to_note_id
                    })
                });
            }
            Ok(json!(notebook))
        }
        "read" => Ok(json!(store.read(string(input, "note_id")?)?)),
        "create" => Ok(json!(store.create(new_note(
            scope,
            string(input, "title")?.into(),
            input["content"]
                .as_str()
                .ok_or_else(|| invalid("Missing content"))?
                .into(),
            actor,
            now
        ))?)),
        "update" => Ok(json!(store.update(
            string(input, "note_id")?,
            revision(input, "note_revision")?,
            string(input, "title")?,
            input["content"]
                .as_str()
                .ok_or_else(|| invalid("Missing content"))?,
            actor,
            now
        )?)),
        "delete" => {
            store.remove(string(input, "note_id")?, revision(input, "note_revision")?)?;
            Ok(json!({"deleted":true}))
        }
        "relation_save" => {
            let id = input["relation_id"].as_str();
            let expected = id
                .map(|_| revision(input, "relation_revision"))
                .transpose()?;
            let existing = id
                .map(|id| {
                    store
                        .list()?
                        .relations
                        .into_iter()
                        .find(|relation| relation.relation_id == id)
                        .ok_or_else(|| invalid("This note relation is unavailable"))
                })
                .transpose()?;
            let relation = WorkspaceNoteRelation {
                relation_id: id
                    .map(str::to_owned)
                    .unwrap_or_else(|| format!("relation-{}", uuid::Uuid::now_v7())),
                from_note_id: string(input, "from_note_id")?.into(),
                to_note_id: string(input, "to_note_id")?.into(),
                label: string(input, "label")?.into(),
                reason: string(input, "reason")?.into(),
                revision: 1,
                created_at: existing
                    .as_ref()
                    .map(|relation| relation.created_at.clone())
                    .unwrap_or_else(|| now.into()),
                updated_at: now.into(),
                author: existing
                    .as_ref()
                    .map(|relation| relation.author)
                    .unwrap_or(actor),
            };
            Ok(json!(store.upsert_relation(relation, expected, now)?))
        }
        "relation_delete" => {
            store.remove_relation(
                string(input, "relation_id")?,
                revision(input, "relation_revision")?,
            )?;
            Ok(json!({"deleted":true}))
        }
        _ => Err(invalid("Unknown notebook operation")),
    }
}

/// The account route verifies ownership of this workspace before reaching storage.
pub fn account(
    user: &user_runtime::UserRuntime,
    scope: &str,
    operation: &str,
    input: &Value,
    now: &str,
) -> Result<Value, ToolError> {
    execute(
        user,
        Some(scope),
        operation,
        input,
        WorkspaceNoteAuthor::User,
        now,
    )
}

fn import(
    user: &user_runtime::UserRuntime,
    scope: Option<&str>,
    book: &Book,
    publication_id: Option<&str>,
    input: &Value,
    actor: WorkspaceNoteAuthor,
    now: &str,
) -> Result<Value, ToolError> {
    let store = scoped_store(user, scope)?;
    let source = &input["source"];
    let mut note = match string(source, "kind")? {
        "answer" => {
            let session_id = string(source, "session_id")?;
            let turn_id = string(source, "turn_id")?;
            let turn = workspace_record_context::permitted_turn(
                user,
                scope,
                &book.base.book_id,
                session_id,
                turn_id,
            )?;
            let outcome = turn
                .outcome
                .as_ref()
                .filter(|_| turn.status == AgentAssistantStatus::Completed)
                .ok_or_else(|| invalid("Only a completed answer can be saved as a note"))?;
            let source_book = turn
                .published_book_ref
                .as_ref()
                .map(|reference| reference.book_id.clone())
                .or_else(|| {
                    user.agent_history
                        .sessions
                        .iter()
                        .find(|session| session.id == session_id)
                        .map(|session| session.book_id.clone())
                })
                .ok_or_else(service_error::missing)?;
            let legacy = (source_book == book.base.book_id)
                .then(|| legacy_answer_projection(turn_id, outcome, book))
                .flatten();
            let view = outcome
                .answer_view
                .as_ref()
                .or_else(|| legacy.as_ref().map(|projection| &projection.view));
            let content = view
                .map(|view| {
                    view.parts
                        .iter()
                        .filter_map(|part| match part {
                            AgentAnswerPart::Markdown { text } | AgentAnswerPart::SocraticQuestion { text, .. } => Some(text.as_str()),
            AgentAnswerPart::MusicExample { example } => Some(example.label.as_str()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n\n")
                })
                .filter(|text| !text.trim().is_empty())
                .or_else(|| outcome.answer.clone())
                .filter(|text| !text.trim().is_empty())
                .ok_or_else(|| invalid("This answer has no written content to save"))?;
            let mut note = new_note(
                scope,
                turn.user.chars().take(120).collect(),
                content,
                WorkspaceNoteAuthor::Assistant,
                now,
            );
            note.origin = Some(WorkspaceNoteOrigin {
                kind: WorkspaceNoteOriginKind::Answer,
                book_id: source_book,
                publication_id: turn
                    .published_book_ref
                    .as_ref()
                    .map(|reference| reference.publication_id.clone()),
                session_id: Some(session_id.into()),
                turn_id: Some(turn_id.into()),
                mem_id: None,
            });
            note.source_bindings = note_api::bindings(if turn.source_bindings.is_empty() {
                legacy
                    .as_ref()
                    .map(|projection| projection.bindings.as_slice())
                    .unwrap_or(&[])
            } else {
                &turn.source_bindings
            });
            note
        }
        "reading_note" => {
            let mem_id = string(source, "mem_id")?;
            let record = user
                .store
                .recall(&RecallQuery {
                    book_id: Some(book.base.book_id.clone()),
                    mem_type: Some("note".into()),
                    ..Default::default()
                })
                .into_iter()
                .find(|record| record.mem_id == mem_id)
                .ok_or_else(service_error::missing)?;
            if record
                .note
                .as_ref()
                .is_some_and(|data| data.material.publication_id.as_deref() != publication_id)
            {
                return Err(invalid(
                    "Open the reading note's original publication before importing it",
                ));
            }
            let excerpt = record
                .note
                .as_ref()
                .and_then(|data| data.retained_excerpt.as_ref());
            let author = if !record.content.trim().is_empty() && record.note.is_some() {
                WorkspaceNoteAuthor::User
            } else if matches!(excerpt, Some(memory::NoteExcerpt::Assistant { .. })) {
                WorkspaceNoteAuthor::Assistant
            } else {
                WorkspaceNoteAuthor::Unknown
            };
            let content = if let Some(excerpt) = excerpt {
                let quote = excerpt
                    .text()
                    .lines()
                    .map(|line| format!("> {line}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                if record.content.trim().is_empty() {
                    quote
                } else {
                    format!("{quote}\n\n{}", record.content)
                }
            } else {
                record.content.clone()
            };
            let title = record
                .content
                .lines()
                .find(|line| !line.trim().is_empty())
                .or_else(|| excerpt.map(memory::NoteExcerpt::text))
                .unwrap_or("阅读笔记")
                .chars()
                .take(120)
                .collect();
            let mut note = new_note(scope, title, content, author, now);
            note.origin = Some(WorkspaceNoteOrigin {
                kind: WorkspaceNoteOriginKind::ReadingNote,
                book_id: record.book_id.clone(),
                publication_id: record
                    .note
                    .as_ref()
                    .and_then(|data| data.material.publication_id.clone())
                    .or_else(|| publication_id.map(str::to_owned)),
                session_id: record.source_session_id.clone(),
                turn_id: None,
                mem_id: Some(mem_id.into()),
            });
            note.source_bindings = record
                .note
                .as_ref()
                .map(|data| data.source_bindings.clone())
                .unwrap_or_default();
            // Selection/body notes carry their source through the original anchor, not a chat binding.
            if note.source_bindings.is_empty() {
                if let Some(lid) = record.anchor.lid.as_ref() {
                    let ranges = record
                        .selection_context
                        .as_ref()
                        .map(|selection| {
                            selection
                                .ranges
                                .iter()
                                .map(|range| read_tools::SourceSelectedRange {
                                    lid: range.lid.clone(),
                                    range: read_tools::SourceTextRange {
                                        start: range.range.start,
                                        end: range.range.end,
                                    },
                                })
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();
                    let evidence = EvidenceRange {
                        start_lid: ranges
                            .first()
                            .map(|range| range.lid.clone())
                            .unwrap_or_else(|| lid.clone()),
                        end_lid: ranges
                            .last()
                            .map(|range| range.lid.clone())
                            .unwrap_or_else(|| lid.clone()),
                        ranges,
                    };
                    if let Ok(resolved) = book.resolve_source(&evidence, "zh-CN", None) {
                        note.source_bindings = note_api::bindings(&[SourceBinding {
                            source_ref_id: format!("note-source-{mem_id}"),
                            book_id: book.base.book_id.clone(),
                            publication_id: publication_id.map(str::to_owned),
                            source_revision: Some(book.source_fingerprint().into()),
                            evidence_range: Some(evidence),
                            evidence_text_digest: resolved.evidence_text_digest,
                            media: None,
                            music: None,
                            label_snapshot: resolved.label,
                            preview_snapshot: resolved.preview,
                        }]);
                    }
                }
            }
            note
        }
        _ => return Err(invalid("Choose a delivered answer or a reading note")),
    };
    // Ordinary reading chats can retain implicit bindings without a publication.
    // Their original material, not the currently open reader, identifies it.
    restore_source_publications(&mut note);
    if let Some(title) = input["title"].as_str() {
        note.title = title.into();
    }
    if let Some(content) = input["content"].as_str() {
        if content != note.content {
            note.content = content.into();
            note.updated_by = actor;
        }
    }
    Ok(json!(store.create(note)?))
}

pub fn route(
    user: &user_runtime::UserRuntime,
    workspace: &reader_workspace::ReaderWorkspace,
    operation: &str,
    input: &Value,
    now: &str,
) -> Result<Value, ToolError> {
    if operation == "source.resolve" {
        return resolve_source(user, workspace, input);
    }
    if operation == "source.read" {
        let binding = source_binding(user, workspace, input)?;
        return crate::board_api::workspace_source_window(workspace, &binding, input);
    }
    if operation == "import" {
        return import(
            user,
            workspace.learning_workspace_id.as_deref(),
            &workspace.book,
            workspace
                .publication
                .as_ref()
                .map(|publication| publication.reference.publication_id.as_str()),
            input,
            WorkspaceNoteAuthor::User,
            now,
        );
    }
    execute(
        user,
        workspace.learning_workspace_id.as_deref(),
        operation,
        input,
        WorkspaceNoteAuthor::User,
        now,
    )
}

pub fn agent(
    user: &user_runtime::UserRuntime,
    scope: &run_scope::RunScope,
    mut input: Value,
    now: &str,
) -> Result<Value, ToolError> {
    let action = string(&input, "action")?.to_owned();
    if action == "import" {
        return import(
            user,
            scope.learning_workspace_id.as_deref(),
            scope.book.as_deref().ok_or_else(|| invalid("Importing a material source needs an active book"))?,
            scope
                .publication
                .as_ref()
                .map(|publication| publication.reference.publication_id.as_str()),
            &input,
            WorkspaceNoteAuthor::Assistant,
            now,
        );
    }
    if action == "list" {
        let limit = input["limit"].as_u64().unwrap_or(20) as usize;
        if !(1..=30).contains(&limit) {
            return Err(invalid("limit must be between 1 and 30"));
        }
        let offset = input["offset"].as_u64().unwrap_or(0) as usize;
        let value = execute(
            user,
            scope.learning_workspace_id.as_deref(),
            "list",
            &input,
            WorkspaceNoteAuthor::Assistant,
            now,
        )?;
        let notes = value["notes"].as_array().expect("notebook list");
        let summaries: Vec<_> = notes.iter().skip(offset).take(limit).map(|note| json!({"note_id":note["note_id"],"title":note["title"],
            "revision":note["revision"],"author":note["author"],"updated_by":note["updated_by"],"updated_at":note["updated_at"],
            "preview":note["content"].as_str().unwrap_or_default().chars().take(240).collect::<String>(),
            "read_request":{"action":"read","note_id":note["note_id"]}})).collect();
        return Ok(
            json!({"notes":summaries,"total":notes.len(),"next_offset":(offset.saturating_add(limit)<notes.len()).then(||offset+limit)}),
        );
    }
    if action == "read" {
        let store = scoped_store(user, scope.learning_workspace_id.as_deref())?;
        let mut note = store.read(string(&input, "note_id")?)?;
        if input
            .get("expected_revision")
            .is_some_and(|expected| expected.as_u64() != Some(u64::from(note.revision)))
        {
            return Err(ToolError {
                error_code: "NOTE_REVISION_CONFLICT".into(),
                category: "conflict".into(),
                message: "This note changed. Read it again from the beginning.".into(),
            });
        }
        let offset = input["offset"].as_u64().unwrap_or(0) as usize;
        let length = input["length"].as_u64().unwrap_or(4000) as usize;
        if !(1..=12000).contains(&length) {
            return Err(invalid("length must be between 1 and 12000"));
        }
        let total = note.content.chars().count();
        note.content = note.content.chars().skip(offset).take(length).collect();
        let end = offset.saturating_add(note.content.chars().count());
        let relations: Vec<_> = store
            .list()?
            .relations
            .into_iter()
            .filter(|relation| {
                relation.from_note_id == note.note_id || relation.to_note_id == note.note_id
            })
            .collect();
        let relation_offset = input["relation_offset"].as_u64().unwrap_or(0) as usize;
        let relation_total = relations.len();
        let relations: Vec<_> = relations
            .into_iter()
            .skip(relation_offset)
            .take(20)
            .collect();
        return Ok(
            json!({"note":note,"content_offset":offset,"end_offset":end,"total_characters":total,
            "next_offset":(end<total).then_some(end),"relations":relations,"relation_offset":relation_offset,"relation_total":relation_total,
            "next_relation_offset":(relation_offset.saturating_add(20)<relation_total).then(||relation_offset+20)}),
        );
    }
    if action.starts_with("relation_") {
        input["relation_revision"] = input["expected_revision"].clone();
    } else {
        input["note_revision"] = input["expected_revision"].clone();
    }
    execute(
        user,
        scope.learning_workspace_id.as_deref(),
        &action,
        &input,
        WorkspaceNoteAuthor::Assistant,
        now,
    )
}

pub fn source_binding(
    user: &user_runtime::UserRuntime,
    workspace: &reader_workspace::ReaderWorkspace,
    input: &Value,
) -> Result<SourceBinding, ToolError> {
    scoped_source_binding(user, workspace.learning_workspace_id.as_deref(), input)
}

pub fn scoped_source_binding(
    user: &user_runtime::UserRuntime,
    scope: Option<&str>,
    input: &Value,
) -> Result<SourceBinding, ToolError> {
    let mut note = scoped_store(user, scope)?.read(string(input, "note_id")?)?;
    restore_source_publications(&mut note);
    let binding = note
        .source_bindings
        .iter()
        .find(|binding| binding.source_ref_id == input["source_ref_id"])
        .ok_or_else(service_error::missing)?;
    serde_json::from_value(json!(binding)).map_err(|error| invalid(error.to_string()))
}

fn resolve_source(
    user: &user_runtime::UserRuntime,
    workspace: &reader_workspace::ReaderWorkspace,
    input: &Value,
) -> Result<Value, ToolError> {
    let binding = source_binding(user, workspace, input)?;
    let request = AgentSourceRequest {
        turn_id: "note".into(),
        source_ref_id: binding.source_ref_id.clone(),
        note_mem_id: None,
    };
    if binding.media.is_some() {
        return Ok(json!(video_source::local_popup(
            workspace, &request, &binding
        )));
    }
    if binding.music.is_some() {
        return Ok(json!(music_source::local_popup(
            workspace, &request, &binding
        )));
    }
    let material_title = answer_source_material_title(workspace, &binding);
    let same_publication = binding.book_id == workspace.book.base.book_id
        && binding.publication_id.as_ref()
            == workspace
                .publication
                .as_ref()
                .map(|p| &p.reference.publication_id);
    let resolved = if same_publication {
        binding
            .evidence_range
            .as_ref()
            .ok_or_else(service_error::missing)
            .and_then(|range| {
                workspace
                    .book
                    .resolve_source(range, "zh-CN", Some(&binding.evidence_text_digest))
            })
    } else {
        Err(service_error::missing())
    };
    Ok(json!(match resolved {
        Ok(source) => SourcePopupView {
            source_ref_id: binding.source_ref_id,
            label: source.label,
            material_title,
            highlighted_quote: source.highlighted_quote,
            context_before: source.context_before,
            context_after: source.context_after,
            stale: false,
            can_open_in_reader: true,
            excerpt: Some(source.excerpt),
            heading_path: source.heading_path
        },
        Err(_) => SourcePopupView {
            source_ref_id: binding.source_ref_id,
            label: binding.label_snapshot,
            material_title,
            highlighted_quote: binding.preview_snapshot,
            context_before: String::new(),
            context_after: String::new(),
            stale: true,
            can_open_in_reader: false,
            excerpt: None,
            heading_path: vec![]
        },
    }))
}
