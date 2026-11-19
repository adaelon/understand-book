use super::*;
use crate::run_scope::RunScope;
use runtime::run_context::CancellationToken;

pub(super) struct ReaderActions {
    pub state: Arc<Mutex<crate::AppState>>,
    pub scope: RunScope,
    pub now: String,
    pub cancellation: CancellationToken,
}

const ACTIONS: [&str; 7] = [
    "reader_note",
    "reader_highlight",
    "reader_save_note",
    "reader_save_question",
    "reader_compare",
    "reader_goto",
    "reader_navigation_result",
];

pub(super) fn is_action(name: &str) -> bool {
    ACTIONS.contains(&name)
}

pub(super) fn label(name: &str) -> &'static str {
    match name {
        "reader_note" => "建议笔记",
        "reader_highlight" => "建议高亮",
        "reader_save_note" => "保存个人笔记",
        "reader_save_question" => "留下待继续的问题",
        "reader_compare" => "并排查看原文",
        "reader_goto" => "打开原文",
        _ => "检查阅读器显示结果",
    }
}

pub(super) fn tools() -> Vec<Value> {
    ACTIONS.iter().map(|&name| {
        let (description, fields): (&str, &[&str]) = match name {
            "reader_note" => ("Propose an editable note at an observed passage. The reader can keep or undo this suggestion.", &["source_ref_id","text"]),
            "reader_highlight" => ("Propose highlighting the exact quote selected with source_present. The reader can keep or undo it.", &["source_ref_id"]),
            "reader_save_note" => ("When the reader asks to save a note, save its exact wording at the observed passage. Returns the editable personal note and revision. Reuse operation_id on retry. Use reader_note for unsolicited suggestions.", &["source_ref_id","text","operation_id"]),
            "reader_save_question" => ("When asked to keep a question for later, save it at the observed passage as a learning task in this chat. Reuse operation_id on retry.", &["source_ref_id","text","operation_id"]),
            "reader_compare" => ("Request side-by-side display of two observed passages. Check the navigation result before claiming that they were displayed.", &["source_ref_id","other_source_ref_id"]),
            "reader_goto" => ("Request display of an observed passage in Reader.", &["source_ref_id"]),
            _ => ("Check a navigation request by its returned id.", &["id"]),
        };
        let properties:serde_json::Map<_,_> = fields.iter().map(|&key|(key.to_owned(),json!({"type":"string","minLength":1}))).collect();
        let read_only = name == "reader_navigation_result";
        json!({"name":name,"description":description,
            "annotations":{"readOnlyHint":read_only,"destructiveHint":false,"openWorldHint":false,
                "idempotentHint":read_only || matches!(name, "reader_note" | "reader_save_note" | "reader_save_question")},
            "inputSchema":{"type":"object","required":fields,
            "properties":properties,"additionalProperties":false}})
    }).collect()
}

impl ReaderActions {
    pub fn apply(
        &self,
        turn: &ExternalEvidence,
        name: &str,
        args: &Value,
    ) -> Result<(Value, Option<AgentEffect>), ToolError> {
        let field = |key: &str| {
            args[key]
                .as_str()
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| error("INVALID_READER_ACTION", format!("Missing {key}")))
        };
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        self.cancellation.check()?;
        self.scope.check_workspace(&state)?;
        if self.scope.user_id != crate::user_runtime::LOCAL_USER_ID
            || crate::course_context::covered_follow_up_chats(
                &state.user,
                self.scope.learning_workspace_id.as_deref(),
                self.scope.book_id(),
            )?
            .contains(&self.scope.chat_session_id)
        {
            return Err(error(
                "READER_ACTION_UNAVAILABLE",
                "当前学习现场没有开放页面与批注操作",
            ));
        }
        if name == "reader_navigation_result" {
            return Ok((
                crate::personal_connection::current_navigation_result(&state, field("id")?)?,
                None,
            ));
        }
        let location = |key| {
            let source = turn.source_range(field(key)?)?;
            if source.start_lid != source.end_lid {
                return Err(error("READER_RANGE_REQUIRED", "请选择一个段落内的准确引文"));
            }
            let range = source.ranges.first().map(|selected| memory::TextRange {
                start: selected.range.start,
                end: selected.range.end,
            });
            Ok((source.start_lid.clone(), range))
        };
        let (lid, range) = location("source_ref_id")?;
        let book = self.scope.reading_book();
        match name {
            "reader_save_note" | "reader_save_question" => {
                let result = crate::personal_connection::acp::save(
                    &mut state,
                    &self.scope,
                    (lid, range),
                    args,
                    name == "reader_save_question",
                    &self.now,
                )?;
                Ok((result, None))
            }
            "reader_note" => {
                let text = field("text")?;
                let saved = reader::Reader::save_note(
                    book,
                    &mut state.user.store,
                    &lid,
                    text,
                    "session",
                    &self.now,
                )?;
                state.workspace.reader.select_annotation(&lid);
                let effect =
                    (saved.status == memory::NoteSaveStatus::Created).then(|| AgentEffect::Note {
                        mem_id: saved.note_id.clone(),
                        lid,
                        text: text.into(),
                    });
                Ok((json!(saved), effect))
            }
            "reader_highlight" => {
                let saved = reader::Reader::save_highlight(
                    book,
                    &mut state.user.store,
                    &lid,
                    range.map(|r| (r.start, r.end)),
                    Some(self.scope.chat_session_id.clone()),
                    "session",
                    &self.now,
                )?;
                state.workspace.reader.select_annotation(&lid);
                Ok((
                    json!(saved),
                    Some(AgentEffect::Highlight {
                        mem_id: saved.highlight_id,
                        lid,
                    }),
                ))
            }
            "reader_compare" | "reader_goto" => {
                let mut sources = vec![(lid, range)];
                if name == "reader_compare" {
                    sources.push(location("other_source_ref_id")?);
                }
                Ok((
                    crate::personal_connection::request_current_navigation(&state, &sources)?,
                    None,
                ))
            }
            _ => Err(error("TOOL_NOT_FOUND", name)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acp_reader_actions_require_observed_sources_and_live_run() {
        let mut state = crate::tests::state_named(&format!("acp-actions-{}", uuid::Uuid::now_v7()));
        let root = tempfile::tempdir().unwrap();
        state.user.history_path = Some(root.path().join("history.json"));
        let book = state.workspace.book.clone();
        let original = "X".repeat(100);
        let turn_ref = crate::precommit_agent_turn(
            &mut state,
            &book.base.book_id,
            "Mark this.".into(),
            None,
            None,
            None,
            "1",
        )
        .unwrap();
        let scope = RunScope::capture(&state, &turn_ref, "Mark this.", None, None);
        let cancellation = CancellationToken::default();
        let state = Arc::new(Mutex::new(state));
        let evidence = AcpEvidence::start(book, vec![], "Mark this.").unwrap();
        evidence.enable_actions(state.clone(), scope, "1".into(), cancellation.clone());
        let mut session = evidence.session.lock().unwrap();
        assert!(session
            .call("reader_highlight", json!({"source_ref_id":"guessed"}))
            .is_err());
        session.call("book_text", json!({"lid":"1.1"})).unwrap();
        let source = session
            .call("source_present", json!({"quote":original}))
            .unwrap();
        let id = &source["source_ref_id"];
        let note = session
            .call(
                "reader_note",
                json!({"source_ref_id":id,"text":"A proposed note"}),
            )
            .unwrap();
        assert!(note["note_id"].is_string());
        let highlight = session
            .call("reader_highlight", json!({"source_ref_id":id}))
            .unwrap();
        assert!(highlight["highlight_id"].is_string());
        let records = state
            .lock()
            .unwrap()
            .user
            .store
            .recall(&memory::RecallQuery {
                book_id: Some("sample-book".into()),
                layer: Some("session".into()),
                ..Default::default()
            });
        assert!(records
            .iter()
            .any(|r| Some(r.mem_id.as_str()) == note["note_id"].as_str()));
        let args = json!({"source_ref_id":id,"text":"Mark this.","operation_id":"save-first"});
        let saved = session.call("reader_save_note", args.clone()).unwrap();
        assert_eq!(saved["value"]["authorship"], "user");
        assert_eq!(saved["current_revision"], 1);
        assert_eq!(
            saved["note_id"],
            session.call("reader_save_note", args).unwrap()["note_id"]
        );
        let note = session
            .call(
                "reader_save_note",
                json!({"source_ref_id":id,"text":"Agent summary","operation_id":"save-second"}),
            )
            .unwrap();
        assert_eq!(note["value"]["authorship"], "assistant_draft");
        let nav = session
            .call(
                "reader_compare",
                json!({"source_ref_id":id,"other_source_ref_id":id}),
            )
            .unwrap();
        assert!(nav["displayed"].is_null());
        cancellation.cancel();
        assert!(session
            .call(
                "reader_note",
                json!({"source_ref_id":id,"text":"Late note"})
            )
            .is_err());
        drop(session);
        assert_eq!(evidence.action_effects().len(), 2);
    }
}
