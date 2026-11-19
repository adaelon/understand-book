//! A committed board part is public teacher support even while its run continues.
use super::*;

/// Only persisted public parts enter review. Candidate text and model-supplied
/// history cannot narrow the original request or substitute for an explanation.
pub(super) fn review_context(user: &crate::user_runtime::UserRuntime, turn: &AgentTurnRef) -> Value {
    let parts = user.agent_history.sessions.iter().find(|session|session.id==turn.session_id)
        .and_then(|session|session.turns.iter().find(|saved|saved.turn_id==turn.turn_id))
        .map(|saved|saved.domain.board_commits.as_slice()).unwrap_or_default();
    json!(parts.iter().map(|part|json!({"delivery_id":part.delivery.commit_id,
        "text":part.delivery_text,"answer_parts":part.answer_parts,
        "teaching_delivery_move_refs":part.teaching_delivery_move_refs})).collect::<Vec<_>>())
}

/// Reconstruct assistance from the durable local delivery, including after
/// cancellation or a crash between the board commit and its teaching trace.
pub(crate) fn record_board_local_delivery(
    user: &crate::user_runtime::UserRuntime, turn: &AgentTurnRef, delivery_id: &str, _now: &str,
) -> Result<(), ToolError> {
    let session=user.agent_history.sessions.iter().find(|session|session.id==turn.session_id)
        .ok_or_else(||invalid("原聊天不存在"))?;
    let saved=session.turns.iter().find(|saved|saved.turn_id==turn.turn_id)
        .ok_or_else(||invalid("原回合不存在"))?;
    let part=saved.domain.board_commits.iter().find(|part|part.delivery.commit_id.as_deref()==Some(delivery_id))
        .ok_or_else(||invalid("只有正式保存的画板局部可以登记教学交付"))?;
    let learning_workspace_id=session.learning_workspace_id.as_deref();
    let Some(bind)=bound_user(user,learning_workspace_id,&turn.turn_id)? else {return Ok(());};
    let parts:Vec<AgentAnswerPart>=part.answer_parts.iter().cloned()
        .map(serde_json::from_value).collect::<Result<_,_>>().map_err(|error|invalid(error.to_string()))?;
    let sources=part.source_bindings.iter().map(|source|runtime::orchestrator::AgentAnswerSource {
        source_ref_id:source.source_ref_id.clone(),label:source.label_snapshot.clone(),
    }).collect();
    let view=AgentAnswerView {parts,sources};
    let store=user.learning_store_for_workspace(learning_workspace_id)?;
    let mut deliveries=Vec::new();
    for reference in &part.teaching_delivery_move_refs {
        let selected=store.teaching_event(reference)?;
        if selected.kind!=TeachingFact::MoveSelected || selected.binding!=bind.binding {
            return Err(invalid("局部交付只能使用本轮接纳的教学选择"));
        }
        let movement:TeachingMove=serde_json::from_value(selected.payload["move"].clone())
            .map_err(|error|invalid(error.to_string()))?;
        // The selection was reviewed and frozen before commit. Replay neither
        // consults the current design nor reclassifies the learner's words.
        runtime::tutor::validate_delivery_scope(&json!({"operation":"select","delivery_scope":selected.payload["delivery_scope"],"move":movement}),true)?;
        if selected.payload["delivery_scope"]["kind"]!="board_part" {
            return Err(invalid("局部交付需要已接纳的 board_part 教学范围"));
        }
        let id=format!("delivery:{}:{}",turn.turn_id,movement.move_id);
        if already_delivered(user,learning_workspace_id,&id,&selected.event_id)? {continue;}
        let presentation=movement.presentation.as_ref().map(|reference| {
            if !view.parts.iter().any(|part|matches!(part,AgentAnswerPart::Presentation {presentation_id,revision}
                if presentation_id==&reference.presentation_id && revision==&reference.revision)) {
                return Err(invalid("局部教学选择的演示版本不在正式交付中"));
            }
            crate::presentation_store::PresentationStore::for_user(user)?.read_version(
                &runtime::presentation::PresentationOwner {book_id:session.book_id.clone(),session_id:turn.session_id.clone()},reference)
        }).transpose()?;
        let mut public_move=json!(movement);
        for field in ["hint","explanation","assessment","participation","coach_ref","coach_adjustment","delivery_text"] {
            public_move.as_object_mut().unwrap().remove(field);
        }
        deliveries.push((id,vec![selected.event_id],public_move,presentation,
            selected.payload["object_versions"].clone(),selected.payload["delivery_scope"].clone()));
    }
    if part.teaching_delivery_move_refs.is_empty() {
        // Ordinary/observing mode still delivers real support. It does not
        // manufacture a selected teaching move, invitation, or assessment.
        let references:Vec<_>=view.parts.iter().filter_map(|part|match part {
            AgentAnswerPart::Presentation {presentation_id,revision}=>Some(runtime::presentation::PresentationRef {
                presentation_id:presentation_id.clone(),revision:*revision}),_=>None,
        }).collect();
        if references.is_empty() {
            deliveries.push((format!("board-delivery:{}:{delivery_id}",turn.turn_id),vec![bind.event_id.clone()],
                json!({"move_id":delivery_id,"kind":"explain","prompt":part.delivery_text,"actions":[],"presentation":null}),
                None,Value::Null,Value::Null));
        }
        for reference in references {
            let version=crate::presentation_store::PresentationStore::for_user(user)?.read_version(
                &runtime::presentation::PresentationOwner {book_id:session.book_id.clone(),session_id:turn.session_id.clone()},&reference)?;
            deliveries.push((format!("board-delivery:{}:{delivery_id}:{}:{}",turn.turn_id,reference.presentation_id,reference.revision),
                vec![bind.event_id.clone()],json!({"move_id":delivery_id,"kind":"explain","prompt":part.delivery_text,"actions":[],"presentation":reference}),
                Some(version),Value::Null,Value::Null));
        }
    }
    for (id,causes,movement,presentation,objects,scope) in deliveries {
        append_user(user,learning_workspace_id,&bind.binding,id,TeachingFact::MessageDelivered,causes,
            json!({"move":movement,"object_versions":objects,"answer":part.delivery_text,"answer_view":view,
                "source_bindings":part.source_bindings,"presentation":presentation,
                "actor":"teacher","origin":"board_commit","board_delivery":part.delivery,
                "delivery_scope":scope}),&part.committed_at)?;
    }
    Ok(())
}

fn already_delivered(user:&crate::user_runtime::UserRuntime,workspace:Option<&str>,id:&str,selected:&str)->Result<bool,ToolError> {
    match user.learning_store_for_workspace(workspace)?.teaching_event(id) {
        Ok(fact) if fact.kind==TeachingFact::MessageDelivered && fact.causal_refs.iter().any(|reference|reference==selected)=>Ok(true),
        Ok(_)=>Err(invalid("教学交付身份已属于其他事实")),
        Err(error) if error.error_code=="TEACHING_INVALID"=>Ok(false),
        Err(error)=>Err(error),
    }
}

pub(super) fn move_was_delivered(user:&crate::user_runtime::UserRuntime,workspace:Option<&str>,selected:&TeachingEvent)->Result<bool,ToolError> {
    let move_id=selected.payload["move"]["move_id"].as_str().ok_or_else(||invalid("教学选择缺少身份"))?;
    already_delivered(user,workspace,&format!("delivery:{}:{move_id}",selected.binding.turn_id),&selected.event_id)
}

#[cfg(test)]
mod tests;
