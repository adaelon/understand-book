//! Personal expressions accepted with a turn, separate from material and mastery facts.
use crate::*;

pub(crate) fn freeze(
    user: &user_runtime::UserRuntime,
    input: &Value,
    session: Option<&str>,
) -> Result<Option<Value>, ToolError> {
    let mut refs: Vec<memory::NoteRef> = input
        .get("note_refs")
        .filter(|v| !v.is_null())
        .map(|v| serde_json::from_value(v.clone()))
        .transpose()
        .map_err(|_| memory::teaching::invalid("想法引用格式无效"))?
        .unwrap_or_default();
    let reply = input["note_reply_to"].as_str();
    if let Some(id) = reply {
        let turn = user
            .agent_history
            .sessions
            .iter()
            .find(|s| Some(s.id.as_str()) == session)
            .and_then(|s| s.turns.iter().find(|t| t.turn_id == id))
            .ok_or_else(|| memory::teaching::invalid("原想法讨论当前不可用"))?;
        let context = turn
            .domain
            .personal_notes
            .as_ref()
            .ok_or_else(|| memory::teaching::invalid("回复对象不是想法讨论"))?;
        let mut inherited: Vec<memory::NoteRef> =
            serde_json::from_value(context["references"].clone())
                .map_err(|_| memory::teaching::invalid("原想法引用不可用"))?;
        let adopted: Vec<memory::NoteRef> = serde_json::from_value(
            context
                .get("adopted_references")
                .cloned()
                .unwrap_or(json!([])),
        )
        .map_err(|_| memory::teaching::invalid("原采用引用不可用"))?;
        for reference in adopted {
            if !inherited.contains(&reference) {
                inherited.push(reference);
            }
        }
        for reference in inherited {
            if !refs.contains(&reference) {
                refs.push(reference);
            }
        }
    }
    let prior = if let Some(binding) = input.get("personal_chat_ref").filter(|v| !v.is_null()) {
        let session = user
            .agent_history
            .sessions
            .iter()
            .find(|s| Some(s.id.as_str()) == binding["session_id"].as_str() && s.book_id.is_empty())
            .ok_or_else(|| memory::teaching::invalid("原个人讨论当前不可用"))?;
        let turn = session
            .turns
            .iter()
            .find(|t| Some(t.turn_id.as_str()) == binding["turn_id"].as_str())
            .ok_or_else(|| memory::teaching::invalid("原讨论回合当前不可用"))?;
        Some(
            json!({"reference":binding,"user_expression":turn.user_authored_text().chars().take(1600).collect::<String>(),"assistant_reply":turn.outcome.as_ref().and_then(|o|o.answer.as_ref()).map(|s|s.chars().take(4000).collect::<String>()),"meaning":"Explicitly brought-in historical discussion. No material mastery or source access is implied."}),
        )
    } else {
        None
    };
    if refs.is_empty() && prior.is_none() && input.get("understanding_ref").is_none_or(Value::is_null) {
        return Ok(None);
    }
    if refs.len() > 100 {
        return Err(memory::teaching::invalid("一次讨论最多引用一百条想法"));
    }
    let mut pages = Vec::new();
    let mut exact = Vec::new();
    for reference in refs {
        let page = user.store.read_personal_note_page(
            &json!({"note_id":reference.note_id,"revision":reference.revision,"limit_chars":1800}),
        )?;
        exact.push(page["reference"].clone());
        if pages.len() < 4 {
            pages.push(page);
        }
    }
    let understanding=input.get("understanding_ref").filter(|v|!v.is_null()).map(|reference|crate::knowledge_tree::evidence(user,reference)).transpose()?;
    Ok(Some(
        json!({"references":exact,"explicit":pages,"prior_discussion":prior,"understanding":understanding,"reply_to_turn":reply,"accepted_at":crate::service_error::now().to_string()}),
    ))
}

pub(crate) fn remember_adoption(
    user: &user_runtime::UserRuntime,
    workspace: Option<&str>,
    turn: &mut AgentChatTurn,
) -> Result<(), ToolError> {
    if turn.status != AgentAssistantStatus::Completed {
        return Ok(());
    }
    let Some(outcome) = turn.outcome.as_ref().filter(|o| {
        !o.incomplete && o.warning.is_none() && !o.teaching_delivery_move_refs.is_empty()
    }) else {
        return Ok(());
    };
    let store = user.learning_store_for_workspace(workspace)?;
    let mut references = Vec::<memory::NoteRef>::new();
    for id in &outcome.teaching_delivery_move_refs {
        let event = store.teaching_event(id)?;
        if event.binding.turn_id != turn.turn_id {
            continue;
        }
        let adopted: Vec<memory::NoteRef> = serde_json::from_value(
            event.payload["move"]
                .get("note_refs")
                .cloned()
                .unwrap_or(json!([])),
        )
        .map_err(|_| memory::teaching::invalid("教学采用的想法引用无效"))?;
        for reference in adopted {
            if !references.contains(&reference) {
                references.push(reference);
            }
        }
    }
    if !references.is_empty() {
        let context = turn
            .domain
            .personal_notes
            .get_or_insert_with(|| json!({"references":[],"explicit":[]}));
        context["adopted_references"] = json!(references);
    }
    Ok(())
}

pub(crate) fn evidence_notes(user: &user_runtime::UserRuntime, binding: &memory::teaching::TeachingBinding, links: Option<&Value>) -> Result<Vec<memory::NoteRef>,ToolError> {
    let links:Vec<memory::understanding::NoteEvidenceLink>=serde_json::from_value(links.cloned().unwrap_or(json!([]))).map_err(|e|memory::teaching::invalid(e.to_string()))?;
    let context=user.agent_history.sessions.iter().find(|s|s.id==binding.chat_session_id)
        .and_then(|s|s.turns.iter().find(|t|t.turn_id==binding.turn_id)).and_then(|t|t.domain.personal_notes.as_ref());
    let offered:Vec<memory::NoteRef>=context.map(|c|["references","adopted_references"].iter().flat_map(|field|c[*field].as_array().into_iter().flatten())
        .filter_map(|r|serde_json::from_value(r.clone()).ok()).collect()).unwrap_or_default();
    let mut refs=Vec::new();
    for link in links {
        if !offered.contains(&link.reference) || link.reference.revision.is_none() || link.quote.trim().is_empty() {
            return Err(memory::teaching::invalid("理解关联需要本轮确切笔记版本及对应原话"));
        }
        let note=user.store.personal_note(&link.reference)?;
        if !note.value.record.content.contains(&link.quote) {return Err(memory::teaching::invalid("理解关联片段不属于所引用的笔记版本"));}
        if !refs.contains(&link.reference) {refs.push(link.reference)}
    }
    Ok(refs)
}

pub fn for_turn(
    user: &mut user_runtime::UserRuntime,
    session: &str,
    turn_id: &str,
) -> Result<Value, ToolError> {
    let turn = user
        .agent_history
        .sessions
        .iter()
        .find(|s| s.id == session)
        .and_then(|s| s.turns.iter().find(|t| t.turn_id == turn_id))
        .ok_or_else(|| memory::teaching::invalid("想法讨论回合不可用"))?;
    let Some(context) = turn.domain.personal_notes.clone() else {
        return Ok(Value::Null);
    };
    let expression = turn.user_authored_text().to_owned();
    let now = context["accepted_at"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let references: Vec<memory::NoteRef> = serde_json::from_value(context["references"].clone())
        .map_err(|_| memory::teaching::invalid("想法引用不可用"))?;
    for reference in references {
        // Historical accepted input survives deletion; it must not recreate a current note.
        if user.store.personal_note(&reference).is_ok() {
            user.store.record_note_discussion(memory::NoteDiscussion {
                reference,
                turn: memory::NoteTurnRef {
                    session_id: session.into(),
                    turn_id: turn_id.into(),
                },
                created_at: now.clone(),
                expression: (!expression.trim().is_empty()).then(|| expression.clone()),
            })?;
        }
    }
    Ok(context)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn personal_notes_only_explicit_reply_carries_adopted_words_forward() {
        let root = tempfile::tempdir().unwrap();
        let mut state = crate::tests::state_named("pn-adoption");
        state.user.store = memory::MemoryStore::open(root.path().join("memory.json")).unwrap();
        let note = state
            .user
            .store
            .create_personal_note(
                serde_json::from_value(json!({"content":"我以为参数就是激活","authorship":"user"}))
                    .unwrap(),
                "create",
                "1",
            )
            .unwrap();
        let chat = crate::personal_chat::create(&mut state.user, "2").unwrap();
        let id = chat["session_id"].as_str().unwrap();
        let prepared = crate::personal_chat::prepare(
            &mut state,
            &json!({"session_id":id,"message":"向量为什么不同？"}).to_string(),
            "3",
        )
        .unwrap();
        let mut turn = state
            .user
            .agent_history
            .sessions
            .iter()
            .find(|s| s.id == id)
            .unwrap()
            .turns[0]
            .clone();
        let reference = memory::NoteRef {
            note_id: note.note_id.clone(),
            revision: Some(1),
        };
        let event = memory::teaching::TeachingEvent {
            event_id: "adopted-move".into(),
            binding: memory::teaching::TeachingBinding {
                tutor_session_id: "teaching".into(),
                session_revision: 1,
                control_revision: 1,
                source_id: "material".into(),
                source_revision: "v1".into(),
                map_revision: None,
                chat_session_id: id.into(),
                turn_id: turn.turn_id.clone(),
            },
            kind: memory::teaching::TeachingFact::MoveSelected,
            causal_refs: vec![],
            payload: json!({"move":{"note_refs":[reference]}}),
            occurred_at: "3".into(),
        };
        state
            .user
            .learning_store()
            .unwrap()
            .append_teaching(&event)
            .unwrap();
        turn.status = AgentAssistantStatus::Completed;
        turn.outcome=Some(serde_json::from_value(json!({"answer":"从参数和激活的区别开始。","incomplete":false,"turns":1,"tokens_spent":0,"effects":[],"trace":[],"memory_updates":[],"teaching_delivery_move_refs":["adopted-move"]})).unwrap());
        remember_adoption(&state.user, None, &mut turn).unwrap();
        assert_eq!(
            turn.domain.personal_notes.as_ref().unwrap()["adopted_references"],
            json!([reference])
        );
        state
            .user
            .agent_history
            .sessions
            .iter_mut()
            .find(|s| s.id == id)
            .unwrap()
            .turns[0] = turn;
        assert!(
            freeze(&state.user, &json!({"message":"新的无关话题"}), Some(id))
                .unwrap()
                .is_none()
        );
        let context = freeze(
            &state.user,
            &json!({"note_reply_to":prepared.turn_ref.turn_id}),
            Some(id),
        )
        .unwrap()
        .unwrap();
        assert_eq!(context["references"], json!([reference]));
        assert!(
            state.user.store.note_discussions(&note.note_id).is_empty(),
            "A recalled note is not itself a new user expression"
        );
        assert!(freeze(
            &state.user,
            &json!({"note_reply_to":prepared.turn_ref.turn_id}),
            Some("other-chat")
        )
        .is_err());
    }
}
