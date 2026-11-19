use super::*;
use crate::learning_task::{self, LearningTaskSelection};
use memory::{
    Anchor, NoteAuthorship, NoteBodyPlacement, NoteCapture, NoteContent, NoteContext, NoteRef,
    SelectedRange, SelectionContext, SelectionResolution, SelectionResolutionBasis,
};

fn note_source(
    book: &Book,
    source: &PassageRef,
    capture: &NoteCapture,
) -> Result<memory::NoteData> {
    let evidence = read_tools::EvidenceRange {
        start_lid: source.lid.clone(),
        end_lid: source.lid.clone(),
        ranges: source
            .range
            .iter()
            .map(|r| read_tools::SourceSelectedRange {
                lid: source.lid.clone(),
                range: read_tools::SourceTextRange {
                    start: r.start,
                    end: r.end,
                },
            })
            .collect(),
    };
    let resolved = book.resolve_source(&evidence, "zh-CN", None)?;
    let association = match capture.placement.as_ref() {
        Some(placement) => memory::NoteAssociation::BodyPlacement {
            placement: placement.clone(),
        },
        None => memory::NoteAssociation::Selection,
    };
    Ok(memory::NoteData {
        material: memory::NoteMaterial {
            book_id: source.book_id.clone(),
            publication_id: None,
        },
        association,
        retained_excerpt: None,
        source_bindings: vec![memory::NoteSourceBinding {
            source_ref_id: format!(
                "source_ref_{}",
                resolved
                    .evidence_text_digest
                    .rsplit('-')
                    .next()
                    .unwrap_or_default()
            ),
            book_id: source.book_id.clone(),
            publication_id: None,
            source_revision: Some(source.source_revision.clone()),
            evidence_range: Some(evidence),
            media: None,
            music: None,
            evidence_text_digest: resolved.evidence_text_digest,
            label_snapshot: resolved.label,
            preview_snapshot: resolved.preview,
        }],
    })
}

fn save_note(
    state: &mut AppState,
    grant: &Grant,
    book: &Book,
    source: &PassageRef,
    quote: &str,
    args: &Value,
    now: &str,
    task: bool,
    origin: Option<&crate::run_scope::RunScope>,
) -> Result<memory::PersonalNoteView> {
    let text = field(args, "text")?.to_owned();
    let chat = args.get("chat_session_id").and_then(Value::as_str);
    if let Some(id) = chat {
        if !state
            .user
            .agent_history
            .sessions
            .iter()
            .any(|c| c.id == id && c.matches_scope(grant.workspace_id.as_deref(), &source.book_id))
        {
            return Err(denied("原聊天不属于选中的材料或工作区"));
        }
    }
    let capture = NoteCapture {
        anchor: Anchor {
            lid: Some(source.lid.clone()),
            concept: None,
        },
        selection_context: source.range.as_ref().map(|range| SelectionContext {
            status: SelectionResolution::Resolved,
            resolution_basis: Some(SelectionResolutionBasis::Exact),
            raw_quote: quote.into(),
            resolved_quote: quote.into(),
            ranges: vec![SelectedRange {
                lid: source.lid.clone(),
                range: range.clone(),
            }],
        }),
        placement: source.range.is_none().then(|| NoteBodyPlacement::LidBlock {
            source_fingerprint: source.source_revision.clone(),
            lid: source.lid.clone(),
        }),
    };
    let operation = format!(
        "personal:{}:{}",
        if task { "task" } else { "note" },
        field(args, "operation_id")?
    );
    let authored_by_reader = origin.is_some_and(|scope| {
        state
            .user
            .agent_history
            .sessions
            .iter()
            .find(|session| session.id == scope.chat_session_id)
            .and_then(|session| {
                session
                    .turns
                    .iter()
                    .find(|turn| turn.turn_id == scope.turn_id)
            })
            .is_some_and(|turn| turn.user_authored_text().contains(&text))
    });
    let receipt = note_source(book, source, &capture)?;
    state.user.store.create_personal_note(
        NoteContent {
            content: text,
            title: None,
            authorship: if authored_by_reader {
                NoteAuthorship::User
            } else {
                NoteAuthorship::AssistantDraft
            },
            context: NoteContext {
                book_id: Some(source.book_id.clone()),
                workspace_id: grant.workspace_id.clone(),
                lid: Some(source.lid.clone()),
                session_id: chat.map(str::to_owned),
                ..Default::default()
            },
            capture: Some(capture),
            source: Some(receipt),
            source_refs: vec![],
            change_note: None,
            based_on_turn: origin.map(|scope| memory::NoteTurnRef {
                session_id: scope.chat_session_id.clone(),
                turn_id: scope.turn_id.clone(),
            }),
        },
        &operation,
        now,
    )
}

fn select_task(
    state: &mut AppState,
    grant: &Grant,
    source: &PassageRef,
    args: &Value,
    note: &memory::PersonalNoteView,
    now: &str,
) -> Result<Value> {
    let selection = LearningTaskSelection {
        learning_workspace_id: grant.workspace_id.clone(),
        published_book_ref: base_schema::PublishedBookRef {
            book_id: source.book_id.clone(),
            publication_id: "local".into(),
        },
        source_revision: source.source_revision.clone(),
        course_run_id: None,
        chat_session_id: args["chat_session_id"].as_str().map(str::to_owned),
        closed_book_id: None,
        selected_at: now.parse().unwrap_or_else(|_| crate::service_error::now()),
        conversation_turn_id: None,
        continue_conversation: false,
        external_question: Some(NoteRef {
            note_id: note.note_id.clone(),
            revision: Some(note.value.revision),
        }),
    };
    if let Some(previous) = learning_task::selections(&state.user)
        .iter()
        .find(|t| t.same_target(&selection))
    {
        return Ok(json!(previous));
    }
    let store = state
        .user
        .session_store
        .as_mut()
        .ok_or_else(|| storage("当前没有可接续的任务存储"))?;
    store.select_learning_task(&state.user.agent_history, selection.clone())?;
    Ok(json!(selection))
}

fn task_list(
    state: &AppState,
    grant: &Grant,
    book_id: Option<&str>,
    limited: bool,
) -> Result<Value> {
    if let Some(id) = book_id {
        grant.material(id)?;
    }
    let mut tasks = Vec::new();
    let mut visibility = BTreeMap::new();
    let mut revisions = BTreeMap::new();
    for selection in learning_task::selections(&state.user) {
        let id = &selection.published_book_ref.book_id;
        if selection.learning_workspace_id != grant.workspace_id
            || book_id.is_some_and(|book| book != id)
            || !grant.books.iter().any(|m| &m.book_id == id)
        {
            continue;
        }
        let (blocked, covered) = match visibility.entry(id) {
            std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::btree_map::Entry::Vacant(entry) => entry.insert((
                closed_source(state, id)?,
                crate::course_context::covered_follow_up_chats(
                    &state.user,
                    grant.workspace_id.as_deref(),
                    id,
                )?,
            )),
        };
        if limited
            || *blocked
            || selection
                .chat_session_id
                .as_ref()
                .is_some_and(|chat| covered.contains(chat))
        {
            if selection
                .chat_session_id
                .as_ref()
                .is_some_and(|chat| covered.contains(chat))
            {
                tasks.push(json!({"book_id":id,"kind":"closed_book","message":"请回到 Reader 的当前练习继续"}));
            }
            continue;
        }
        if let Some(mut task) = learning_task::project(&state.user, selection, None, |_| Ok(None))?
        {
            let revision = revisions.entry(id).or_insert_with(|| {
                grant
                    .load(state, id)
                    .ok()
                    .map(|book| book.source_fingerprint().to_owned())
            });
            if revision.as_deref() != Some(&selection.source_revision) {
                task = task.unavailable("原材料当前不可用或已更新，请在 Reader 中重新选择");
            }
            tasks.push(json!(task));
        }
    }
    Ok(json!({"tasks":tasks}))
}

fn search(book: &Book, args: &Value) -> Result<Value> {
    let input =
        book_tool_contracts::validate_search_text_input(json!({"query":field(args,"query")?,
        "cursor":args["cursor"],"page_size":args["page_size"].as_u64().unwrap_or(20)}))
        .map_err(|e| invalid(&e.message))?;
    let found = book.search_text(&input)?;
    let mut value = json!(found);
    for (source, item) in found
        .occurrences
        .iter()
        .zip(value["occurrences"].as_array_mut().unwrap())
    {
        for (r, out) in source
            .ranges
            .iter()
            .zip(item["ranges"].as_array_mut().unwrap())
        {
            out["source_ref"] = json!({"book_id":book.base.book_id,"source_revision":book.source_fingerprint(),"lid":r.lid,
                "range":{"start":r.start_utf16,"end":r.end_utf16}});
        }
    }
    Ok(value)
}

pub(super) fn request_navigation(
    state: &AppState,
    grant: &Grant,
    root: &Path,
    args: &Value,
    compare: bool,
) -> Result<Value> {
    let mut sources = Vec::new();
    for value in
        std::iter::once(&args["source_ref"]).chain(compare.then_some(&args["other_source_ref"]))
    {
        let (_, source, text) = grant.resolve(state, value)?;
        let material = grant.material(&source.book_id)?;
        sources.push(
            json!({"source_ref":source,"book_dir":material.dir,"title":material.title,"text":text}),
        );
    }
    let id = uuid::Uuid::now_v7().to_string();
    NAV.lock().unwrap().insert(root.to_path_buf(),json!({"id":id,"kind":if compare{"compare"}else{"goto"},"sources":sources,"displayed":null}));
    Ok(json!({"id":id,"status":"requested","displayed":null,"sources":sources}))
}

pub(super) fn call(
    state: &mut AppState,
    grant: &Grant,
    root: &Path,
    name: &str,
    args: &Value,
    limited: bool,
    now: &str,
    origin: Option<&crate::run_scope::RunScope>,
) -> Result<Value> {
    match name {
        "material_list" => Ok(
            json!({"materials":grant.books.iter().map(|m|json!({"book_id":m.book_id,"title":m.title})).collect::<Vec<_>>()}),
        ),
        "task_list" => task_list(state, grant, args["book_id"].as_str(), limited),
        "reader_goto" | "reader_compare" => {
            request_navigation(state, grant, root, args, name == "reader_compare")
        }
        "reader_navigation_result" => super::navigation_result(root, field(args, "id")?),
        "note_add" | "highlight_add" | "task_save" => {
            let (book, source, quote) = grant.resolve(state, &args["source_ref"])?;
            if crate::current_note_source_fingerprint(&grant.material(&source.book_id)?.dir)?
                != source.source_revision
            {
                return Err(error(
                    "PERSONAL_SOURCE_CHANGED",
                    "conflict",
                    "原材料已更新，请重新打开",
                ));
            }
            let operation_id = field(args, "operation_id")?;
            let mut result = if name == "highlight_add" {
                json!(reader::Reader::save_highlight(
                    &book,
                    &mut state.user.store,
                    &source.lid,
                    source.range.as_ref().map(|r| (r.start, r.end)),
                    None,
                    "long_term",
                    now
                )?)
            } else {
                if name == "task_save" && state.user.session_store.is_none() {
                    return Err(storage("当前没有任务存储"));
                }
                let note = save_note(
                    state,
                    grant,
                    &book,
                    &source,
                    &quote,
                    args,
                    now,
                    name == "task_save",
                    origin,
                )?;
                if name == "task_save" {
                    json!({"note":note,"task":select_task(state,grant,&source,args,&note,now)?})
                } else {
                    json!(note)
                }
            };
            result["operation_id"] = json!(operation_id);
            result["source_ref"] = json!(source);
            mark_changed(root);
            Ok(result)
        }
        "reader_scene"
        | "book_text"
        | "book_search_text"
        | "book_paper_metadata"
        | "highlight_list"
        | "note_list"
        | "note_read" => {
            let id = field(args, "book_id")?;
            let material = grant.material(id)?;
            match name {
                "highlight_list" => {
                    let book = grant.load(state, id)?;
                    let highlights = state
                        .user
                        .store
                        .recall(&memory::RecallQuery {
                            book_id: Some(id.into()),
                            mem_type: Some("highlight".into()),
                            ..Default::default()
                        })
                        .into_iter()
                        .rev()
                        .filter(|item| {
                            args["before"]
                                .as_str()
                                .is_none_or(|cursor| item.mem_id.as_str() < cursor)
                        })
                        .take(51)
                        .collect::<Vec<_>>();
                    let next = highlights
                        .get(49)
                        .filter(|_| highlights.len() > 50)
                        .map(|item| item.mem_id.clone());
                    let rows=highlights.into_iter().take(50).map(|item|{
                        let source=item.anchor.lid.as_deref().and_then(|lid|{
                            let text=passage(&book,lid,item.range.as_ref()).ok()?;
                            (text==item.content).then(||PassageRef{book_id:id.into(),source_revision:book.source_fingerprint().into(),
                                lid:lid.into(),range:item.range.clone()})
                        });
                        json!({"highlight_id":item.mem_id,"text":item.content,"source_ref":source})
                    }).collect::<Vec<_>>();
                    Ok(json!({"highlights":rows,"next_cursor":next}))
                }
                "reader_scene" => {
                    if state.workspace.book.base.book_id != id
                        || !same_directory(&state.workspace.book_dir, &material.dir)
                    {
                        return Ok(json!({"book_id":id,"visible":false}));
                    }
                    Ok(
                        json!({"book_id":id,"visible":true,"scene":crate::reader_state_response(&state.workspace.book,&state.workspace.reader),
                        "chat_session_id":state.workspace.selected_chat}),
                    )
                }
                "note_read" => {
                    let reference: NoteRef = serde_json::from_value(
                        json!({"note_id":field(args,"note_id")?,"revision":args["revision"]}),
                    )
                    .map_err(|_| invalid("笔记版本无效"))?;
                    let note = state.user.store.personal_note(&reference)?;
                    if note.value.context.book_id.as_deref() != Some(id)
                        || note.value.context.workspace_id != grant.workspace_id
                    {
                        return Err(denied("该笔记不属于已选范围"));
                    }
                    Ok(json!(note))
                }
                "note_list" => {
                    let notes = state.user.store.personal_notes_for_workspace(
                        id,
                        grant.workspace_id.as_deref(),
                        args["before"].as_str(),
                        51,
                    );
                    let next = notes
                        .get(49)
                        .filter(|_| notes.len() > 50)
                        .map(|n| n.note_id.clone());
                    Ok(
                        json!({"notes":notes.into_iter().take(50).collect::<Vec<_>>(),"next_cursor":next}),
                    )
                }
                _ => {
                    let book = grant.load(state, id)?;
                    match name {
                        "book_paper_metadata" => Ok(json!(book.paper_metadata_projection())),
                        "book_search_text" => search(&book, args),
                        _ => {
                            let lid = field(args, "lid")?;
                            let range: Option<memory::TextRange> =
                                serde_json::from_value(args["range"].clone())
                                    .map_err(|_| invalid("选区位置无效"))?;
                            let text = passage(&book, lid, range.as_ref())?;
                            Ok(
                                json!({"text":text,"source_ref":PassageRef{book_id:id.into(),source_revision:book.source_fingerprint().into(),lid:lid.into(),range}}),
                            )
                        }
                    }
                }
            }
        }
        "paper_metadata" => {
            let id = args["book_id"]
                .as_str()
                .unwrap_or(&state.workspace.book.base.book_id);
            let authorized = grant.material(id).is_ok();
            if authorized {
                if closed_source(state, id)? {
                    return Err(denied("请先返回 Reader 完成本书的闭卷练习"));
                }
                let book = grant.load(state, id)?;
                return crate::learning_extensions::prepared_metadata(
                    root.parent()
                        .ok_or_else(|| storage("无法定位本机用户目录"))?,
                    &book,
                    &grant.material(id)?.dir,
                );
            } else if !grant.manage
                || !grant.write
                || !grant.imported_in(id, &crate::state_library_root(state))
            {
                return Err(denied("该材料不属于已选范围"));
            }
            if closed_source(state, id)? {
                return Err(denied("请先返回 Reader 完成本书的闭卷练习"));
            }
            crate::learning_extensions::imported_metadata(
                profile(root)?,
                &crate::state_library_root(state).join(id),
                id,
            )
        }
        _ => {
            let current = &state.workspace.book.base.book_id;
            grant.material(current)?;
            let library = crate::state_library_root(state);
            check_import_origin(grant, profile(root)?, &library, name, args)?;
            if !matches!(name, "paper_import" | "agentero_import")
                && args["book_id"].as_str().is_some_and(|id| id != current)
            {
                return Err(denied("请先打开要管理的已授权材料"));
            }
            let result =
                crate::learning_extensions::dispatch(state, profile(root)?, name, args, now)?;
            record_extension_result(root, grant, &library, name, &result)?;
            Ok(result)
        }
    }
}
