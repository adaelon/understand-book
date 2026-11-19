//! Personal conversations own a normal session and Goal, with no material identity.
use crate::*;

fn invalid(message: &str) -> ToolError {
    user_storage_paths::error("PERSONAL_CHAT_INVALID", "validation", message)
}
pub fn session<'a>(
    user: &'a user_runtime::UserRuntime,
    id: &str,
) -> Result<&'a AgentChatSession, ToolError> {
    user.agent_history
        .sessions
        .iter()
        .find(|s| s.id == id && s.book_id.is_empty() && s.learning_workspace_id.is_none())
        .ok_or_else(|| invalid("这条个人讨论当前不可用"))
}
pub fn create(user: &mut user_runtime::UserRuntime, now: &str) -> Result<Value, ToolError> {
    let mut chat = new_agent_session("", now, 0);
    chat.id = format!("chat-{}", uuid::Uuid::now_v7());
    chat.title = "推敲我的想法".into();
    let id = chat.id.clone();
    if let Some(store) = &mut user.session_store {
        store.create(&mut user.agent_history, chat)?;
    } else {
        let mut candidate = user.agent_history.clone();
        candidate.sessions.push(chat);
        save_agent_history_path(&user.history_path, &candidate)?;
        user.agent_history = candidate;
    }
    Ok(json!({"session_id":id}))
}
pub fn history(user: &user_runtime::UserRuntime, id: &str) -> Result<Value, ToolError> {
    let chat = session(user, id)?;
    Ok(
        json!({"session_id":chat.id,"title":chat.title,"turns":chat.turns.iter().map(|t|scoped_turn_view(None,t)).collect::<Vec<_>>(),"goals":chat.goals}),
    )
}
pub fn candidate(
    user: &user_runtime::UserRuntime,
    input: &Value,
    turn_id: Option<&str>,
    now: &str,
) -> Result<(AgentChatSession, AgentTurnRef, String), ToolError> {
    let id = input["session_id"]
        .as_str()
        .ok_or_else(|| invalid("请选择一条讨论"))?;
    let mut chat = session(user, id)?.clone();
    if chat
        .turns
        .iter()
        .any(|t| t.status == AgentAssistantStatus::PendingAssistant)
    {
        return Err(user_storage_paths::error(
            "CHAT_BUSY",
            "conflict",
            "当前讨论仍在回答",
        ));
    }
    if input.get("expected_chat_head").is_some()
        && input["expected_chat_head"] != json!(chat.turns.last().map(|t| &t.turn_id))
    {
        return Err(user_storage_paths::error(
            "CHAT_HEAD_CHANGED",
            "conflict",
            "讨论已更新，请查看最新回答后再发送",
        ));
    }
    let message = input["message"]
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| invalid("请写下要讨论的想法"))?
        .to_owned();
    let notes = personal_note_context::freeze(user, input, Some(id))?;
    runtime::response_language::ResponseLanguage::from_request(input)?;
    let reference = append_pending_agent_turn(
        &mut chat,
        message.clone(),
        None,
        None,
        None,
        None,
        input["goal_id"].as_str(),
        input["goal_action"].as_str(),
        None,
        turn_id,
        presentation_requested(input)?,
        now,
    )?;
    chat.turns.last_mut().unwrap().domain.personal_notes = notes;
    Ok((chat, reference, message))
}
pub fn accept(
    user: &mut user_runtime::UserRuntime,
    chat: AgentChatSession,
    now: &str,
) -> Result<(), ToolError> {
    if let Some(store) = &mut user.session_store {
        store.accept(&mut user.agent_history, chat, now)?;
    } else {
        let mut candidate = user.agent_history.clone();
        let id = chat.id.clone();
        *candidate.sessions.iter_mut().find(|s| s.id == id).unwrap() = chat;
        save_agent_history_path(&user.history_path, &candidate)?;
        user.agent_history = candidate;
    }
    Ok(())
}
pub(crate) fn prepare(
    state: &mut AppState,
    body: &str,
    now: &str,
) -> Result<agent_run::PreparedAgentChat, ToolError> {
    let input: Value = serde_json::from_str(body).map_err(|_| invalid("请求格式不完整"))?;
    let (mut chat, reference, message) = candidate(&state.user, &input, None, now)?;
    if let Some(key) = input["client_request_id"].as_str() {
        chat.turns.last_mut().unwrap().domain.local_action =
            Some(session_event::LocalActionInput {
                client_request_id: key.into(),
                normalized: run_input::normalize("local", &input)?,
            });
    }
    let goal = chat
        .turns
        .last()
        .and_then(|t| t.goal_ref.as_ref())
        .and_then(|r| chat.goals.iter().find(|g| g.id == r.id))
        .cloned();
    chat.turns.last_mut().unwrap().domain.provider_binding=state.services.provider_binding.clone();
    let messages = chat.messages.clone();
    accept(&mut state.user, chat, now)?;
    let scope = run_scope::RunScope {
        board_context: None,
        learning_workspace_id: None,
        goal: None,
        members: None,
        publication: None,
        user_id: state.user.user_id().unwrap().into(),
        workspace_id: String::new(),
        workspace_generation: 0,
        chat_session_id: reference.session_id.clone(),
        turn_id: reference.turn_id.clone(),
        confirmation_id: None,
        book: None,
        book_dir: PathBuf::new(),
        provider_binding: state.services.adapter.model_runtime_profile(),
        reader_input: None,
        question_quote: None,
        music_question: None,
        presentation_follow_up: None,
    };
    Ok(agent_run::PreparedAgentChat {
        response_language: runtime::response_language::ResponseLanguage::from_request(&input)?,
        exam_calendar: None,
        accepted_workspace_goal: None,
        scope,
        tutor: None,
        goal,
        turn_ref: reference,
        message: message.clone(),
        agent_message: message,
        messages,
        now: now.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    #[derive(Clone, Default)]
    struct Probe(Arc<Mutex<Vec<String>>>);
    impl ModelAdapter for Probe {
        fn complete(
            &self,
            _: runtime::CompletionRequest,
        ) -> Result<runtime::ParsedResponse, runtime::AdapterError> {
            Err(runtime::AdapterError { message: "PN interface fixture does not provide background profile completions".into(), spend_stop: None })
        }
        fn complete_structured(&self, request: runtime::CompletionRequest) -> Result<Value, runtime::AdapterError> {
            crate::tests::tutor_loop_tests::fixture_coach_guidance(&request)
                .ok_or_else(|| runtime::AdapterError { message: "PN interface fixture supplies no additional learner judgment".into(), spend_stop: None })
        }
        fn chat(
            &self,
            plan: &runtime::AgentRequestPlan,
        ) -> Result<runtime::AssistantTurn, runtime::AdapterError> {
            let text = serde_json::to_string(&plan.ordered_messages()).unwrap();
            self.0.lock().unwrap().push(text);
            Ok(runtime::AssistantTurn {
                provider_continuation: None,
                text: Some(
                    "参数是已学到的固定值，激活还依赖这次输入。可以用同一函数代入不同输入来比较。"
                        .into(),
                ),
                tool_calls: vec![],
                usage_total_tokens: Some(17),
            })
        }
    }
    fn content(text: &str) -> Value {
        json!({"content":text,"authorship":"user","context":{},"source_refs":[]})
    }
    #[test]
    fn personal_notes_discussion_freezes_versions_and_reopens_without_material() {
        let dir = tempfile::tempdir().unwrap();
        let mut state = crate::tests::state_named("personal-lifecycle");
        state.user.store = memory::MemoryStore::open(&dir.path().join("memory.json")).unwrap();
        let history = dir.path().join("agent-history.json");
        let paths = session_store::SessionPaths::from_history(&history);
        std::fs::create_dir_all(&paths.root).unwrap();
        let (store, projection) = session_store::SessionStore::open(paths.clone()).unwrap();
        state.user.history_path = Some(history);
        state.user.session_store = Some(store);
        state.user.agent_history = projection;
        let note = personal_note_api::dispatch(
            &mut state.user,
            "create",
            json!({"value":content("参数不变，为什么表示会变化？"),"operation_id":"first"}),
            "1",
        )
        .unwrap();
        let chat = create(&mut state.user, "2").unwrap();
        let request = json!({"scope":"personal","session_id":chat["session_id"],"message":"我觉得参数和激活是同一个东西","note_refs":[{"note_id":note["note_id"],"revision":1}],"client_request_id":"discuss-1"});
        let prepared = prepare(&mut state, &request.to_string(), "3").unwrap();
        let turn_id = prepared.turn_ref.turn_id.clone();
        assert!(
            prepared.scope.book.is_none()
                && prepared.scope.reader_input.is_none()
                && prepared.scope.publication.is_none()
                && prepared.scope.learning_workspace_id.is_none()
        );
        personal_note_api::dispatch(&mut state.user,"revise",json!({"note_id":note["note_id"],"base_revision":1,"value":content("现在我知道参数与激活不同。"),"operation_id":"second"}),"4").unwrap();
        let probe = Probe::default();
        let port = agent_run::BorrowedAppPort(std::cell::RefCell::new(&mut state));
        let reply = agent_run::execute_prepared(&port, &probe, prepared, Default::default());
        assert_eq!(reply.reply.status, 200, "{}", reply.reply.body);
        let messages = probe.0.lock().unwrap();
        assert!(messages[0].contains("参数不变，为什么表示会变化"));
        assert!(messages[0].contains("no Book"));
        drop(messages);
        assert!(
            interaction_input::local_repeat(&mut state, &request.to_string())
                .unwrap()
                .is_some()
        );
        let timeline = state
            .user
            .store
            .note_discussions(note["note_id"].as_str().unwrap());
        assert_eq!(timeline.len(), 1);
        assert_eq!(timeline[0].reference.revision, Some(1));
        let fresh=prepare(&mut state,&json!({"scope":"personal","session_id":chat["session_id"],"message":"我明白了，这次输入改变的是激活。","note_reply_to":turn_id,"client_request_id":"discuss-2"}).to_string(),"5").unwrap();
        let ctx = personal_note_context::for_turn(
            &mut state.user,
            &fresh.turn_ref.session_id,
            &fresh.turn_ref.turn_id,
        )
        .unwrap();
        assert_eq!(ctx["references"][0]["revision"], 1);
        let token = runtime::run_context::CancellationToken::default();
        token.cancel();
        let reply = agent_run::execute_prepared(
            &agent_run::BorrowedAppPort(std::cell::RefCell::new(&mut state)),
            &probe,
            fresh,
            token,
        );
        assert_ne!(reply.reply.status, 200);
        let (_, reopened) = session_store::SessionStore::open(paths).unwrap();
        let saved = &reopened.sessions[0];
        assert!(saved.book_id.is_empty());
        assert_eq!(saved.turns.len(), 2);
        assert_eq!(saved.turns[0].status, AgentAssistantStatus::Completed);
        assert_eq!(saved.turns[1].status, AgentAssistantStatus::Cancelled);
        assert!(saved
            .turns
            .iter()
            .all(|turn| turn.published_book_ref.is_none() && turn.teaching_ref.is_none()));
    }
}
