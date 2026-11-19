//! One projection of learner words and the support available when they were said.
use super::*;
use memory::learning_evidence::LearningEvidence;

pub(super) fn basis(
    state: &PrivateBookContext<'_>, binding: &TeachingBinding, reference: &str,
) -> Result<(Option<LearningEvidence>, Vec<TeachingEvent>), ToolError> {
    let store=state.user.learning_store_for_workspace(state.learning_workspace_id)?;
    match store.evidence(reference) {
        Ok(_) => store.understanding_basis(binding,reference).map(|(interpretation,facts)|(Some(interpretation),facts)),
        Err(error) if error.error_code=="TEACHING_INVALID" =>
            store.learner_expression_fact(binding,reference).map(|fact|(None,vec![fact])),
        Err(error) => Err(error),
    }
}

/// Saved designs are readable even if an old basis is no longer usable. New
/// design/move adoption validates those references before reaching this view.
pub(super) fn context(
    state: &PrivateBookContext<'_>, binding: &TeachingBinding, design: &Value,
    movements: &[TeachingMove], input_facts: &[TeachingEvent], interpretation_refs: &[String],
) -> Result<Value, ToolError> {
    let store=state.user.learning_store_for_workspace(state.learning_workspace_id)?;
    let mut facts=store.teaching_turn_events(&binding.tutor_session_id,&binding.turn_id)?;
    facts.extend(input_facts.iter().cloned());
    facts.retain(|fact|fact.binding.tutor_session_id==binding.tutor_session_id && fact.binding.turn_id==binding.turn_id
        && fact.binding.source_id==binding.source_id && fact.binding.source_revision==binding.source_revision
        && fact.learner_expression().is_some());
    let mut references=Vec::<String>::new();
    for field in ["basis_refs","open_question_refs"] {
        references.extend(design[field].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_owned));
    }
    references.extend_from_slice(interpretation_refs);
    for movement in movements {
        references.extend(movement.interpretation_refs.iter().cloned());
        if let Some(response)=&movement.responds_to { references.extend(response.fact_refs.iter().cloned()); }
    }
    references.sort(); references.dedup();
    let mut interpretations=Vec::new();
    let mut interpretation_support_refs=std::collections::BTreeSet::new();
    let mut expressions=Vec::new();
    let mut unavailable_basis_refs=Vec::new();
    for reference in references {
        match basis(state,binding,&reference) {
            Ok((interpretation,originals)) => {
                if let Some(interpretation)=interpretation {
                    interpretation_support_refs.extend(interpretation.support_delivery_refs.iter().chain(&interpretation.assistance_refs).cloned());
                    if let Some(text)=interpretation.correction.as_deref().filter(|text|!text.trim().is_empty()) {
                        // This endpoint stores the user's correction verbatim in
                        // the evidence record; it is not an assistant inference
                        // and does not pretend to be a teaching-event fact_ref.
                        expressions.push(json!({"kind":"learner_correction",
                            "evidence_ref":interpretation.evidence_id,"text":text,
                            "original_fact_refs":originals.iter().map(|fact|&fact.event_id).collect::<Vec<_>>() }));
                    }
                    let mut projected=json!({"evidence_ref":interpretation.evidence_id,
                        "interpretation":interpretation.interpretation,"teaching_implication":interpretation.teaching_implication,
                        "fact_refs":originals.iter().map(|fact|&fact.event_id).collect::<Vec<_>>(),
                        "learner_quote":interpretation.learner_quote,"nature":interpretation.nature,
                        "status":interpretation.status,"design_ref":interpretation.design_ref,"block_refs":interpretation.block_refs,
                        "correction":interpretation.correction,"support_delivery_refs":interpretation.support_delivery_refs,"assistance_refs":interpretation.assistance_refs});
                    timeline::merge_metadata(state,&interpretation,&mut projected)?;
                    interpretations.push(projected);
                }
                facts.extend(originals);
            }
            Err(error) if error.error_code=="TEACHING_INVALID" => unavailable_basis_refs.push(reference),
            Err(error) => return Err(error),
        }
    }
    let mut seen=std::collections::BTreeSet::new();
    facts.retain(|fact|seen.insert(fact.event_id.clone()));
    // A before/help/after comparison needs the actual words that prompted the
    // received help. Follow this one support edge, without walking the history.
    let received_support=facts.iter().flat_map(|fact|fact.payload["support_delivery_refs"].as_array().into_iter().flatten())
        .filter_map(Value::as_str).map(str::to_owned).collect::<std::collections::BTreeSet<_>>();
    for reference in received_support {
        let support=store.teaching_event(&reference)?;
        for reference in support.payload["move"]["responds_to"]["fact_refs"].as_array().into_iter().flatten().filter_map(Value::as_str) {
            if seen.contains(reference) { continue; }
            match store.learner_expression_fact(&support.binding,reference) {
                Ok(fact) => { seen.insert(fact.event_id.clone()); facts.push(fact); },
                // Historic moves sometimes linked clicks in responds_to. Those
                // remain context and must not become words in this projection.
                Err(error) if error.error_code=="TEACHING_INVALID" => {},
                Err(error) => return Err(error),
            }
        }
    }
    let mut support_refs=interpretation_support_refs;
    let mut ordered=facts.into_iter().map(|fact|store.teaching_event_sequence(&fact.event_id).map(|seq|(seq,fact))).collect::<Result<Vec<_>,_>>()?;
    // The current input is projected before its admission transaction persists
    // it, so it has a real occurrence time but no database sequence yet.
    ordered.sort_by_key(|(seq,fact)|(timeline::time_key(&fact.occurred_at),seq.unwrap_or(u64::MAX)));
    for (seq,fact) in ordered {
        for field in ["support_delivery_refs","assistance_refs"] {
            support_refs.extend(fact.payload[field].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_owned));
        }
        let readable=readable_fact(state,&fact)?;
        expressions.push(json!({"fact_ref":fact.event_id,"text":fact.learner_expression(),
            "occurred_at":fact.occurred_at,"fact_sequence":seq,"source_session_id":fact.binding.chat_session_id,"source_turn_id":fact.binding.turn_id,
            "response_origin":fact.payload["response_origin"],"support_delivery_refs":fact.payload["support_delivery_refs"],
            "assistance_refs":fact.payload["assistance_refs"],"invitation_context_ref":fact.payload["invitation_context_ref"],
            "scene":readable["scene"],"presentations":readable["presentations"]}));
    }
    let mut delivered_support=Vec::new();
    for reference in support_refs {
        let fact=store.teaching_event(&reference)?;
        let readable=readable_fact(state,&fact)?;
        let feedback=if fact.kind==TeachingFact::HelpDisplayed && fact.payload["action"]=="feedback" {
            fact.payload["assessment_ref"].as_str().map(|id|store.assessment(id)).transpose()?.flatten()
        } else { None };
        delivered_support.push(json!({"delivery_ref":fact.event_id,"kind":fact.kind,"occurred_at":fact.occurred_at,
            "original_ref":{"kind":"chat_turn","session_id":fact.binding.chat_session_id,"turn_id":fact.binding.turn_id},
            "answer":fact.payload["answer"],"text":fact.payload["text"],"move":fact.payload["move"],
            "presentation":readable["content"]["presentation"],"presentations":readable["presentations"],
            "scene":readable["scene"],"feedback":feedback,"actor":fact.payload["actor"],
            "action_ref":fact.payload["action_ref"],"before_receipt":fact.payload["before_receipt"]}));
    }
    Ok(json!({"expressions":expressions,"interpretations":interpretations,"delivered_support":delivered_support,
        "unavailable_basis_refs":unavailable_basis_refs,"expression_order":"oldest_first","unknown_when_absent":true,
        "change_rule":"Compare the same relationship across original expressions and input-time help. A later expression is a new observation, not proof of progress or permission to overwrite an accurate earlier observation. Supersedes corrects an interpretation."}))
}
