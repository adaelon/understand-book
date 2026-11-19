//! Read-time history of learner expressions; background insertion is not learning time.
use super::*;
use memory::learning_evidence::LearningEvidence;
use std::collections::BTreeSet;

pub(crate) fn time_key(value: &str) -> Option<i128> {
    if let Ok(number)=value.parse::<f64>() {
        if number.is_finite() && number>=0.0 {
            return Some((number * if number>100_000_000_000.0 {1_000_000.0} else {1_000_000_000.0}) as i128);
        }
    }
    time::OffsetDateTime::parse(value,&time::format_description::well_known::Rfc3339).ok().map(|time|time.unix_timestamp_nanos())
}

pub(super) fn metadata(user: &crate::user_runtime::UserRuntime, learning_workspace_id: Option<&str>, evidence: &LearningEvidence) -> Result<Value, ToolError> {
    let store=user.learning_store_for_workspace(learning_workspace_id)?;
    let mut metadata=store.evidence_chronology(&evidence.evidence_id)?;
    metadata["source_session_title"]=json!(user.agent_history.sessions.iter()
        .find(|session|Some(session.id.as_str())==metadata["source_session_id"].as_str()).map(|session|&session.title));
    metadata["supersedes"]=json!(evidence.supersedes);
    metadata["superseded"]=json!(store.evidence_is_superseded(&evidence.evidence_id)?);
    Ok(metadata)
}

pub(super) fn merge_metadata(state: &PrivateBookContext<'_>, evidence: &LearningEvidence, view: &mut Value) -> Result<(), ToolError> {
    view.as_object_mut().unwrap().extend(metadata(state.user,state.learning_workspace_id,evidence)?.as_object().unwrap().clone());
    Ok(())
}

pub(super) fn expressions(user: &crate::user_runtime::UserRuntime, learning_workspace_id: Option<&str>, evidence: &LearningEvidence, view: &mut Value) -> Result<(), ToolError> {
    let store=user.learning_store_for_workspace(learning_workspace_id)?;
    let mut support_refs=evidence.assistance_refs.iter().chain(&evidence.support_delivery_refs).cloned().collect::<BTreeSet<_>>();
    let mut expressions=Vec::new();
    for (seq,fact) in store.evidence_expressions(evidence)? {
        for field in ["assistance_refs","support_delivery_refs"] {
            support_refs.extend(fact.payload[field].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_owned));
        }
        expressions.push(json!({"fact_ref":fact.event_id,"text":fact.learner_expression(),"occurred_at":time_key(&fact.occurred_at).map(|_|&fact.occurred_at),
            "fact_sequence":seq,"source_session_id":fact.binding.chat_session_id,"source_turn_id":fact.binding.turn_id,
            "assistance_refs":fact.payload["assistance_refs"],"support_delivery_refs":fact.payload["support_delivery_refs"],
            "response_origin":fact.payload["response_origin"]}));
    }
    let assistance_count=support_refs.len();
    let mut assistance=Vec::new();
    for id in support_refs {
        let fact=match store.teaching_event(&id) {
            Ok(fact) => fact,
            Err(error) if error.error_code=="TEACHING_INVALID" => continue,
            Err(error) => return Err(error),
        };
        // A displayed rubric stores a receipt, not another copy of its text.
        // Resolve only that displayed assessment and preserve its item order,
        // matching the feedback the activities UI actually showed.
        let feedback=if fact.kind==TeachingFact::HelpDisplayed && fact.payload["action"]=="feedback" {
            fact.payload["assessment_ref"].as_str().map(|id|store.assessment(id)).transpose()?.flatten()
        } else { None };
        let text=fact.payload.get("answer").and_then(Value::as_str)
            .or_else(||fact.payload.get("text").and_then(Value::as_str))
            .or_else(||fact.payload["move"].get("prompt").and_then(Value::as_str))
            .map(str::to_owned)
            .or_else(||feedback.map(|assessment|assessment.items.into_iter().map(|item|item.reason).collect::<Vec<_>>().join("\n")))
            .filter(|text|!text.trim().is_empty());
        if text.is_none() { continue; }
        assistance.push(json!({"fact_ref":id,"kind":fact.kind,"occurred_at":time_key(&fact.occurred_at).map(|_|&fact.occurred_at),
            "fact_sequence":store.teaching_event_sequence(&id)?,"text":text}));
    }
    assistance.sort_by_key(|item|(item["occurred_at"].as_str().and_then(time_key),item["fact_sequence"].as_u64()));
    view["expressions"]=json!(expressions);
    view["assistance_count"]=json!(assistance_count);
    view["assistance"]=json!(assistance);
    Ok(())
}

pub(super) fn page(state: &PrivateBookContext<'_>, id: &str, after: u64) -> Result<Value, ToolError> {
    let store=state.user.learning_store_for_workspace(state.learning_workspace_id)?;
    if store.evidence(id)?.source_id!=state.book.base.book_id { return Err(invalid("请打开此理解记录的原材料")); }
    let rows=store.understanding_timeline(id,after,13)?;
    let more=rows.len()>12;
    let entries=rows.into_iter().take(12).map(|(_,e)|evidence_view(state,&e.evidence_id)).collect::<Result<Vec<_>,_>>()?;
    Ok(json!({"entries":entries,"next":more.then_some(after+12),"order":"oldest_first"}))
}
