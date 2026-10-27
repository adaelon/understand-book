//! Reliable understanding observations share the durable coach job.
use super::*;
use memory::learning::LearningStore;
use memory::teaching::TeachingEvent;
use runtime::tutor_coach::{CoachGuidanceDraft, CoachObservationDecision};

/// Group the two receipts for one authored expression, retaining every original
/// fact ID. All assistance is bounded before the expression, never this answer.
pub(super) fn freeze_facts(store: &LearningStore, events: &[TeachingEvent]) -> Result<Vec<Value>, ToolError> {
    let mut groups: Vec<Value> = Vec::new();
    for fact in events.iter().filter(|fact| fact.learner_expression().is_some()) {
        let text = fact.learner_expression().unwrap();
        let mut help_refs: Vec<String> = ["assistance_refs", "support_delivery_refs"].into_iter()
            .flat_map(|field| fact.payload[field].as_array().into_iter().flatten())
            .filter_map(|value| value.as_str().map(str::to_owned)).collect();
        // Ordinary or fallback answers may have no teaching move. They still
        // were visible before this input and must not imply independent work.
        for help in store.prior_chat_help_for_expression(fact)? {
            if !help_refs.contains(&help.event_id) { help_refs.push(help.event_id); }
        }
        let prior_help = help_refs.iter().map(|id| {
            let help = store.teaching_event(id)?;
            Ok(json!({"event_id":help.event_id,"binding":help.binding,"kind":help.kind,
                "occurred_at":help.occurred_at,"answer":help.payload["answer"],"text":help.payload["text"],
                "move":help.payload["move"],"usage":help.payload["usage"]}))
        }).collect::<Result<Vec<_>,ToolError>>()?;
        if let Some(group) = groups.iter_mut().find(|group| group["user_expression"] == text) {
            group["fact_refs"].as_array_mut().unwrap().push(json!(fact.event_id));
            if store.fact_has_interpretation(&fact.event_id)? { group["already_interpreted"] = json!(true); }
            for id in help_refs {
                if !group["assistance_refs"].as_array().unwrap().contains(&json!(id)) {
                    group["assistance_refs"].as_array_mut().unwrap().push(json!(id));
                }
            }
            for help in prior_help {
                if !group["prior_help"].as_array().unwrap().iter().any(|saved| saved["event_id"] == help["event_id"]) {
                    group["prior_help"].as_array_mut().unwrap().push(help);
                }
            }
            continue;
        }
        groups.push(json!({"fact_ref":fact.event_id,"fact_refs":[fact.event_id],
            "user_expression":text,"binding":fact.binding,"occurred_at":fact.occurred_at,
            "assistance_refs":help_refs,"prior_help":prior_help,
            "already_interpreted":store.fact_has_interpretation(&fact.event_id)?}));
    }
    Ok(groups)
}

/// Reconstitute only exact ranges actually registered in this delivered turn.
/// Immutable publication/version checks prevent later material becoming evidence.
pub(super) fn freeze_original_reads(user: &user_runtime::UserRuntime, job: &CoachJob, book: &Book, refresh: bool) -> Result<CoachJob, ToolError> {
    if job.input.origin == CoachJobOrigin::StageStart || matches!(job.status, CoachJobStatus::Ready | CoachJobStatus::Cancelled)
        || !refresh && job.input.context.get("verified_read_ranges").is_some() { return Ok(job.clone()); }
    if book.base.book_id != job.input.binding.source_id || book.source_fingerprint() != job.input.binding.source_revision {
        return Err(error("TUTOR_COACH_SOURCE_UNAVAILABLE", "理解观察的原资料版本已不可用。"));
    }
    let messages = user.session_store.as_ref().and_then(|store| store.logs.get(&job.input.binding.chat_session_id))
        .map(|log| log.turn_messages(&job.input.binding.turn_id)).transpose()?.unwrap_or_default();
    let ranges = runtime::orchestrator::recover_turn_text_read_ranges(book, &messages);
    user.learning_store_for_workspace(job.learning_workspace_id.as_deref())?
        .freeze_coach_read_ranges(&job.input.job_ref, job.attempt, &ranges)
}

pub(super) fn with_material(job: &CoachJob, book: &Book) -> Result<CoachJob, ToolError> {
    if book.base.book_id != job.input.binding.source_id || book.source_fingerprint() != job.input.binding.source_revision {
        return Err(error("TUTOR_COACH_SOURCE_UNAVAILABLE", "理解观察的原资料版本已不可用。"));
    }
    let bindings: Vec<SourceBinding> = serde_json::from_value(job.input.context.get("source_bindings").cloned().unwrap_or(json!([])))
        .map_err(|e| memory::teaching::invalid(e.to_string()))?;
    let mut sources = Vec::new();
    for binding in bindings.iter().filter(|binding| binding.book_id == book.base.book_id
        && binding.source_revision.as_deref() == Some(book.source_fingerprint())) {
        if let Some(range) = &binding.evidence_range {
            let source = book.resolve_source(range, "zh", (!binding.evidence_text_digest.is_empty()).then_some(binding.evidence_text_digest.as_str()))?;
            sources.push(json!({"source_ref_id":binding.source_ref_id,"range":range,"text":source.highlighted_quote}));
        } else if binding.media.is_some() || binding.music.is_some() {
            sources.push(json!({"source_ref_id":binding.source_ref_id,"media":binding.media,"music":binding.music}));
        }
    }
    for range in reviewed_ranges(job)? {
        let source = book.resolve_source(&range, "zh", None)?;
        sources.push(json!({"range":range,"text":source.highlighted_quote}));
    }
    let mut hydrated = job.clone();
    hydrated.input.context["read_sources"] = json!(sources);
    Ok(hydrated)
}

/// Delivery review persists the actual read ledger, which can cover more than
/// the small excerpt selected for display. Never infer reads from a target.
fn reviewed_ranges(job: &CoachJob) -> Result<Vec<EvidenceRange>, ToolError> {
    let mut ranges: Vec<EvidenceRange> = serde_json::from_value(job.input.context.get("verified_read_ranges").cloned().unwrap_or(json!([])))
        .map_err(|e| memory::teaching::invalid(e.to_string()))?;
    for fact in job.input.context["turn_facts"].as_array().into_iter().flatten()
        .filter(|fact| fact["kind"] == "agent_work_reviewed"
            && fact["binding"]["turn_id"] == job.input.binding.turn_id) {
        for source in fact["payload"]["source_evidence"].as_array().into_iter().flatten()
            .filter(|source| source["source_id"] == job.input.binding.source_id
                && source["source_revision"] == job.input.binding.source_revision) {
            let read: Vec<EvidenceRange> = serde_json::from_value(source["read_ranges"].clone())
                .map_err(|e| memory::teaching::invalid(e.to_string()))?;
            ranges.extend(read);
        }
    }
    Ok(ranges)
}

pub(super) fn commit(user: &user_runtime::UserRuntime, job: &CoachJob, guidance: &mut Value,
    material: Option<(&Book, &Path)>) -> Result<(), ToolError> {
    let Some(facts) = job.input.context.get("observation_facts").and_then(Value::as_array) else { return Ok(()); };
    if facts.is_empty() { return Ok(()); }
    let result: CoachGuidanceDraft = serde_json::from_value(guidance.clone())
        .map_err(|e| memory::teaching::invalid(format!("理解观察输出无效：{e}")))?;
    let store = user.learning_store_for_workspace(job.learning_workspace_id.as_deref())?;
    let bindings: Vec<SourceBinding> = serde_json::from_value(job.input.context["source_bindings"].clone())
        .map_err(|e| memory::teaching::invalid(e.to_string()))?;
    let mut ranges: Vec<EvidenceRange> = bindings.iter().filter(|source| source.book_id == job.input.binding.source_id
        && source.source_revision.as_deref() == Some(job.input.binding.source_revision.as_str()))
        .filter_map(|binding| binding.evidence_range.clone()).collect();
    ranges.extend(reviewed_ranges(job)?);
    let mut receipts = Vec::new();
    for fact in facts {
        let refs: Vec<String> = serde_json::from_value(fact["fact_refs"].clone()).map_err(|e| memory::teaching::invalid(e.to_string()))?;
        if refs.iter().map(|id| store.fact_has_interpretation(id)).collect::<Result<Vec<_>,_>>()?.contains(&true) {
            receipts.push(json!({"fact_ref":fact["fact_ref"],"status":"existing"}));
            continue;
        }
        let observation = result.observations.iter().find(|observation| Some(observation.fact_ref.as_str()) == fact["fact_ref"].as_str())
            .ok_or_else(|| memory::teaching::invalid("理解观察遗漏了本轮真实表达"))?;
        if observation.decision == CoachObservationDecision::NoNewInformation {
            receipts.push(json!({"fact_ref":observation.fact_ref,"status":"no_new_information","reason":observation.reason}));
            continue;
        }
        let candidate = observation.hypothesis.as_ref().ok_or_else(|| memory::teaching::invalid("理解观察缺少解释候选"))?;
        let (book, directory) = material.ok_or_else(|| memory::teaching::invalid("理解观察缺少原资料校验上下文"))?;
        if book.base.book_id != job.input.binding.source_id || book.source_fingerprint() != job.input.binding.source_revision {
            return Err(memory::teaching::invalid("理解观察来源版本不一致"));
        }
        let context = PrivateBookContext { user, learning_workspace_id:job.learning_workspace_id.as_deref(),
            book, book_dir:directory, messages:&[], selected_chat:Some(&job.input.binding.chat_session_id) };
        let request = json!({"operation":"evidence","nature":"hypothesis",
            "operation_id":format!("background:{}:{}",job.input.source_ref,observation.fact_ref),
            "target":candidate.target,"fact_refs":refs,"interpretation":candidate.interpretation,
            "teaching_implication":candidate.teaching_implication,"design_ref":candidate.design_ref,
            "block_refs":candidate.block_refs});
        let assistance:Vec<String> = serde_json::from_value(fact["assistance_refs"].clone()).map_err(|e| memory::teaching::invalid(e.to_string()))?;
        let accepted = crate::teaching::accept_hypothesis_with_assistance(&context, &job.input.binding, &request, &bindings, &ranges, &assistance)?;
        receipts.push(json!({"fact_ref":observation.fact_ref,"status":"recorded","evidence_ref":accepted["evidence_ref"]}));
    }
    guidance["observation_receipts"] = json!(receipts);
    Ok(())
}
