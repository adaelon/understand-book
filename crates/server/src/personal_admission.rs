//! Material-free requests use the same durable admission queue and execution.
use super::*;

impl RunAdmissions {
    pub(super) fn admit_personal(
        &self,
        access: &Arc<Authorization>,
        context: &AuthorizedContext,
        input: &Value,
        now: &str,
    ) -> Result<Value, ToolError> {
        let key = input["client_request_id"]
            .as_str()
            .filter(|s| !s.is_empty() && s.len() <= 128)
            .ok_or_else(invalid)?;
        let normalized = normalize("", input)?;
        if let Some(row) = self.by_key(context.user_id(), key)? {
            return self.repeat(access, context, &row, &normalized);
        }
        let provider = self
            .provider
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| fault("PROVIDER_NOT_CONFIGURED", "unavailable"))?;
        let mut user = context.user.lock().unwrap();
        if let Some(row) = self.by_key(context.user_id(), key)? {
            drop(user);
            return self.repeat(access, context, &row, &normalized);
        }
        let id = format!("turn_{}", uuid::Uuid::now_v7());
        let (mut session, reference, message) =
            crate::personal_chat::candidate(&user, input, Some(&id), now)?;
        let head = session.turns.iter().rev().nth(1).map(|t| t.turn_id.clone());
        let row = Admission {
            owner: context.user_id().into(),
            key: key.into(),
            turn: id,
            chat: session.id.clone(),
            workspace: String::new(),
            generation: 0,
            publication: None,
            state: "preparing".into(),
            cancel: false,
            closed: false,
            unsaved: false,
        };
        let goal = session
            .turns
            .last()
            .and_then(|t| t.goal_ref.as_ref())
            .and_then(|r| session.goals.iter().find(|g| g.id == r.id))
            .cloned();
        session.turns.last_mut().unwrap().admission_input = Some(FrozenTurn {
            board_context: None,
            exam_sprint: None,
            version: 1,
            learning_workspace_id: None,
            learning_workspace_goal: None,
            learning_workspace_members: None,
            owner: row.owner.clone(),
            workspace: String::new(),
            generation: 0,
            effect_revision: 0,
            chat: row.chat.clone(),
            turn: row.turn.clone(),
            ordinal: reference.user_turn_ordinal,
            head,
            publication: None,
            normalized,
            reader: None,
            music_question: None,
            confirmation: None,
            provider: provider.binding,
            message: message.clone(),
            agent_message: message,
            messages: session.messages.clone(),
            quote: None,
            presentation: None,
            teaching: teaching::FrozenTeachingPreparation {
                learning_workspace_id: None,
                context: None,
                events: vec![],
            },
            goal,
            now: now.into(),
            preparation_complete: true,
        });
        {
            let mut state = self.state.lock().unwrap();
            if state.stopping {
                return Err(fault("SERVICE_STOPPING", "unavailable"));
            }
            if state
                .deleting
                .contains(&(row.owner.clone(), row.chat.clone()))
            {
                return Err(busy());
            }
            let tx = state
                .control
                .connection
                .transaction()
                .map_err(|_| storage())?;
            let occupied:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM run_admissions WHERE owner_user_id=? AND chat_session_id=? AND dispatch_state IN ('preparing','queued','claimed'))",params![row.owner,row.chat],|r|r.get(0)).map_err(|_|storage())?;
            if occupied {
                return Err(busy());
            }
            let (total,own):(u64,u64)=tx.query_row("SELECT count(*),COALESCE(sum(owner_user_id=?),0) FROM run_admissions WHERE dispatch_state IN ('preparing','queued','claimed')",[&row.owner],|r|Ok((r.get(0)?,r.get(1)?))).map_err(|_|storage())?;
            if total >= self.limits.queued_runs as u64 || own >= self.limits.user_queued_runs as u64
            {
                return Err(fault("RUN_QUEUE_FULL", "rate_limit"));
            }
            tx.execute("INSERT INTO run_admissions(owner_user_id,client_request_id,turn_id,chat_session_id,workspace_id,workspace_generation,book_id,publication_id,dispatch_state) VALUES(?,?,?,?,NULL,0,NULL,NULL,'preparing')",params![row.owner,row.key,row.turn,row.chat]).map_err(|_|storage())?;
            tx.commit().map_err(|_| storage())?;
        }
        self.point("preparing")?;
        let result = (|| {
            crate::personal_chat::accept(&mut user, session, now)?;
            self.point("pending")?;
            self.complete_preparation(&mut user, &row, access)
        })();
        if let Err(e) = result {
            if e.error_code == "INJECTED_CRASH" {
                return Err(e);
            }
            if let Some(store) = &mut user.session_store {
                // Resolve an uncertain append before closing this admission.
                let _ = store;
                let user = &mut *user;
                if user
                    .session_store
                    .as_mut()
                    .unwrap()
                    .settle(&mut user.agent_history, &row.chat)
                    .is_err()
                {
                    self.mark_unsaved(&row);
                    return Err(storage());
                }
            }
            let has_input = frozen_input(&user, &row)?.is_some();
            if find_turn(&user, &row).is_some()
                && self
                    .end_pending(&mut user, &row, &e.error_code, false)
                    .is_err()
            {
                self.mark_unsaved(&row);
                return Err(storage());
            }
            self.update(&row, "admission_failed", !has_input)?;
            if let Err(error) = crate::operations::write_turn(&self.state.lock().unwrap().control.connection,
                &row.owner, &row.turn, "failed", "admission", Some(&e.error_code), multi_user_host::now()) {
                eprintln!("operations preparation write failed: {}", error.error_code);
            }
            return Err(e);
        }
        Ok(descriptor(&self.by_turn(&row.owner, &row.turn)?))
    }
}
