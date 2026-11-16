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
            group["help_before_expression"]=json!(if group["assistance_refs"].as_array().unwrap().is_empty(){"no_recorded_prior_help"}else{"prior_help_recorded"});
            continue;
        }
        groups.push(json!({"fact_ref":fact.event_id,"fact_refs":[fact.event_id],
            "user_expression":text,"binding":fact.binding,"occurred_at":fact.occurred_at,
            "assistance_refs":help_refs,"prior_help":prior_help,"help_before_expression":if help_refs.is_empty(){"no_recorded_prior_help"}else{"prior_help_recorded"},
            "already_interpreted":store.fact_has_interpretation(&fact.event_id)?}));
    }
    Ok(groups)
}

/// Restore this turn's reads, or the actual sources behind its prior delivered
/// help. A conversational reply can use the question's sources without rereading.
pub(super) fn freeze_original_reads(user: &user_runtime::UserRuntime, job: &CoachJob, book: &Book, refresh: bool) -> Result<CoachJob, ToolError> {
    if job.input.origin == CoachJobOrigin::StageStart || matches!(job.status, CoachJobStatus::Ready | CoachJobStatus::Cancelled) { return Ok(job.clone()); }
    if book.base.book_id != job.input.binding.source_id || book.source_fingerprint() != job.input.binding.source_revision {
        return Err(error("TUTOR_COACH_SOURCE_UNAVAILABLE", "理解观察的原资料版本已不可用。"));
    }
    let log = user.session_store.as_ref().and_then(|store| store.logs.get(&job.input.binding.chat_session_id));
    let mut ranges: Vec<EvidenceRange> = if !refresh && job.input.context.get("verified_read_ranges").is_some() {
        serde_json::from_value(job.input.context["verified_read_ranges"].clone()).map_err(|e| memory::teaching::invalid(e.to_string()))?
    } else {
        let messages = log.map(|log| log.turn_messages(&job.input.binding.turn_id)).transpose()?.unwrap_or_default();
        runtime::orchestrator::recover_turn_text_read_ranges(book, &messages)
    };
    // Prepared lessons have no chat tool log. Actually read their frozen source
    // here before granting the same source coverage used by chat observations.
    if let Some(source) = job.input.context.pointer("/course_response/lesson/summary/source") {
        let start = source["start_lid"].as_str().ok_or_else(|| memory::teaching::invalid("课程缺少原文范围"))?;
        let end = source["end_lid"].as_str().ok_or_else(|| memory::teaching::invalid("课程缺少原文范围"))?;
        book.text(start, Some(end))?;
        ranges.push(serde_json::from_value(json!({"start_lid":start,"end_lid":end})).map_err(|e| memory::teaching::invalid(e.to_string()))?);
    }
    let mut bindings: Vec<SourceBinding> = serde_json::from_value(job.input.context.get("source_bindings").cloned().unwrap_or(json!([])))
        .map_err(|e| memory::teaching::invalid(e.to_string()))?;
    // Resident text citations historically omit source_revision. Verify their
    // exact saved digest against this job's source before hydrating them. This
    // also covers a follow-up answered entirely from the resident working set.
    bindings.retain(|source| source.book_id == job.input.binding.source_id
        && source.source_revision.as_deref().is_none_or(|v| v == job.input.binding.source_revision));
    for source in &mut bindings {
        if let Some(range) = &source.evidence_range {
            if source.source_revision.is_none() && source.evidence_text_digest.is_empty() { continue; }
            book.resolve_source(range, "zh", (!source.evidence_text_digest.is_empty()).then_some(source.evidence_text_digest.as_str()))?;
            source.source_revision = Some(job.input.binding.source_revision.clone());
            if !ranges.contains(range) { ranges.push(range.clone()); }
        }
    }
    if ranges.is_empty() && bindings.is_empty() {
        if let Some(session) = user.agent_history.sessions.iter().find(|s| s.id == job.input.binding.chat_session_id) {
            let help_turns: Vec<_> = job.input.context["observation_facts"].as_array().into_iter().flatten()
                .flat_map(|fact| fact["prior_help"].as_array().into_iter().flatten())
                .filter(|help| help["binding"]["chat_session_id"] == job.input.binding.chat_session_id
                    && help["binding"]["source_id"] == job.input.binding.source_id
                    && help["binding"]["source_revision"] == job.input.binding.source_revision)
                .filter_map(|help| help["binding"]["turn_id"].as_str()).collect();
            if let Some(end) = session.turns.iter().rposition(|turn| help_turns.contains(&turn.turn_id.as_str())) {
                // Stop at the nearest source-bearing exchange in this same
                // conversation/version. Later replies can never supply evidence.
                for turn in session.turns[..=end].iter().rev() {
                    let Some(context) = binding_context(user, job.learning_workspace_id.as_deref(), &turn.turn_id)? else { continue; };
                    if context["binding"]["source_id"] != job.input.binding.source_id
                        || context["binding"]["source_revision"] != job.input.binding.source_revision { break; }
                    let messages = log.map(|log| log.turn_messages(&turn.turn_id)).transpose()?.unwrap_or_default();
                    ranges = runtime::orchestrator::recover_turn_text_read_ranges(book, &messages);
                    for source in &turn.source_bindings {
                        if source.book_id != job.input.binding.source_id { continue; }
                        if source.source_revision.as_deref().is_some_and(|v| v != job.input.binding.source_revision) { continue; }
                        if let Some(range) = &source.evidence_range {
                            book.resolve_source(range, "zh", Some(&source.evidence_text_digest))?;
                            if !ranges.contains(range) { ranges.push(range.clone()); }
                        } else if source.source_revision.as_deref() == Some(job.input.binding.source_revision.as_str()) {
                            if !bindings.contains(source) { bindings.push(source.clone()); }
                        }
                    }
                    if !ranges.is_empty() || !bindings.is_empty() { break; }
                }
            }
        }
    }
    user.learning_store_for_workspace(job.learning_workspace_id.as_deref())?
        .freeze_coach_sources(&job.input.job_ref, job.attempt, &ranges, json!(bindings))
}

pub(super) fn with_material(job: &CoachJob, book: &Book, directory: &Path) -> Result<CoachJob, ToolError> {
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
        if sources.iter().any(|source| source["range"] == json!(range)) { continue; }
        let source = book.resolve_source(&range, "zh", None)?;
        sources.push(json!({"range":range,"text":source.highlighted_quote}));
    }
    let mut hydrated = job.clone();
    hydrated.input.context["read_sources"] = json!(sources);
    hydrated.input.context["verified_read_ranges"] = json!(reviewed_ranges(job)?);
    hydrated.input.context["knowledge_candidates"] = json!(crate::knowledge_tree::candidates(book,directory,&hydrated.input.context)?);
    if hydrated.input.context["course_response"].is_object() {
        scope_course_understanding(&mut hydrated.input.context, book);
    }
    Ok(hydrated)
}

/// Select from the input-time snapshot, never from the live store while a job is
/// waiting. Knowledge neighbours include related work from other conversations.
fn scope_course_understanding(context: &mut Value, book: &Book) {
    let candidates = context["knowledge_candidates"].as_array().cloned().unwrap_or_default();
    let references:Vec<_> = candidates.iter().flat_map(|node| {
        std::iter::once(node["reference"].clone()).chain(node["relations"].as_array().into_iter().flatten().map(|r|r["reference"].clone()))
    }).collect();
    let span = |range:&Value| {
        let start=range["start_lid"].as_str()?;
        let end=range["end_lid"].as_str()?;
        let a=book.base.lid_nodes.iter().find(|node|node.lid==start)?;
        let b=book.base.lid_nodes.iter().find(|node|node.lid==end)?;
        Some((a.span.start,b.span.end))
    };
    let source_ranges:Vec<_>=context["read_sources"].as_array().into_iter().flatten().filter_map(|s|span(&s["range"])).collect();
    let prior=context["teaching_context"]["learner_context"]["current_understanding"].as_array().cloned().unwrap_or_default();
    let relevant:Vec<_>=prior.into_iter().filter(|item| {
        item["knowledge_links"].as_array().into_iter().flatten().any(|link|references.contains(&link["reference"]))
            || item["target"]["source_bindings"].as_array().into_iter().flatten().filter_map(&span)
                .any(|(start,end)|source_ranges.iter().any(|(a,b)|start<*b && end>*a))
    }).collect();
    let omitted=relevant.len().saturating_sub(12);
    let selected:Vec<_>=relevant.into_iter().take(12).collect();
    let mut focuses=context["known_focuses"].as_array().cloned().unwrap_or_default();
    for item in &selected {
        if item["target"]["focus_ref"].is_string() && !focuses.iter().any(|target|target["focus_ref"]==item["target"]["focus_ref"]) {
            focuses.push(item["target"].clone());
        }
    }
    context["known_focuses"]=json!(focuses);
    context["teaching_context"]["learner_context"]["current_understanding"]=json!(selected);
    context["teaching_context"]["learner_context"]["omitted_related_understanding"]=json!(omitted);
}

/// Original successful read receipts cover more than a short display excerpt.
/// Real historical reviews may supplement them. Never infer reads from a target.
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

/// Check the exact material supplied to generation before its one correction
/// opportunity ends. No evidence is written until commit rechecks and accepts it.
pub(super) fn validate_material(job: &CoachJob, guidance: &CoachGuidanceDraft, book: &Book) -> Result<(), ToolError> {
    let ranges: Vec<EvidenceRange> = job.input.context["read_sources"].as_array().into_iter().flatten()
        .filter_map(|source| source.get("range")).cloned()
        .map(serde_json::from_value).collect::<Result<_,_>>()
        .map_err(|e| memory::teaching::invalid(e.to_string()))?;
    let candidates = job.input.context["knowledge_candidates"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    for observation in &guidance.observations {
        let Some(hypothesis) = &observation.hypothesis else { continue; };
        let fact = job.input.context["observation_facts"].as_array().into_iter().flatten()
            .find(|fact| fact["fact_ref"].as_str() == Some(observation.fact_ref.as_str()));
        if fact.is_some_and(|fact| fact["already_interpreted"] == true) { continue; }
        for source in hypothesis.target["source_bindings"].as_array().into_iter().flatten() {
            let start = source["start_lid"].as_str().unwrap_or("");
            let end = source["end_lid"].as_str().unwrap_or("");
            if let Some((missing, _)) = crate::teaching::missing_target_read(book, start, end, &ranges)? {
                return Err(memory::teaching::invalid(format!(
                    "target.source_bindings 的 {start}..{end} 超出 read_sources 实际提供的正文，缺少 {}。按本轮已读且支持判断的最小范围修改；沿用 focus_ref 不要求复制旧 source_bindings。", missing.start_lid)));
            }
        }
        let expression = fact.and_then(|fact| fact["user_expression"].as_str()).unwrap_or("");
        crate::knowledge_tree::validate_links(&hypothesis.understanding.knowledge_links, candidates, expression)?;
    }
    Ok(())
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
            "block_refs":candidate.block_refs,"understanding":candidate.understanding,"note_links":candidate.note_links});
        let assistance:Vec<String> = serde_json::from_value(fact["assistance_refs"].clone()).map_err(|e| memory::teaching::invalid(e.to_string()))?;
        let accepted = crate::teaching::accept_hypothesis_with_assistance(&context, &job.input.binding, &request, &bindings, &ranges, &assistance)?;
        receipts.push(json!({"fact_ref":observation.fact_ref,"status":"recorded","evidence_ref":accepted["evidence_ref"]}));
    }
    guidance["observation_receipts"] = json!(receipts);
    Ok(())
}
