//! A disposable view of original work, its help conditions and exact assessments.
use super::*;
use memory::learning_evidence::LearningEvidence;
use std::collections::BTreeSet;

const CONTENT_BUDGET: usize = 6000;

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

fn add_read(reads: &mut Vec<Value>, tool: &str, arguments: Value) {
    let read = json!({"tool":tool,"arguments":arguments});
    if !reads.contains(&read) {
        reads.push(read);
    }
}

fn local_read(reads: &mut Vec<Value>, id: &str, evidence: bool) {
    add_read(
        reads,
        "tutor.step",
        if evidence {
            json!({"operation":"understanding","evidence_ref":id})
        } else {
            json!({"operation":"trace","event_id":id})
        },
    );
}

fn historical_read(reads: &mut Vec<Value>, records: &[Value], chat: &str, id: &str) {
    if let Some(after) = records
        .iter()
        .position(|row| row["record"]["event_id"] == id || row["record"]["evidence_id"] == id)
    {
        add_read(
            reads,
            "workspace.records.read",
            json!({
            "operation":"read","chat_session_id":chat,"after":after,"limit":1}),
        );
    }
}

fn keep(rows: &mut Vec<Value>, value: Value, remaining: &mut usize) -> bool {
    let size = value.to_string().chars().count();
    if rows.len() >= 6 || size > *remaining {
        return false;
    }
    *remaining -= size;
    rows.push(value);
    true
}

fn source_refs(value: &Value) -> Vec<Value> {
    value.as_array().into_iter().flatten().map(|source| json!({
        "source_ref_id":source["source_ref_id"],
        "source_id":source.get("source_id").or_else(||source.get("book_id")),
        "publication_id":source["publication_id"],"source_revision":source["source_revision"],
        "music_target":source.get("music").and_then(|music|music.get("target"))
            .or_else(||source.get("target").filter(|target|target.get("audio").is_some())),
        "evidence_range":source.get("evidence_range").cloned().unwrap_or_else(||
            json!({"start_lid":source["start_lid"],"end_lid":source["end_lid"]}))
    })).collect()
}

fn target_sources(target: &Value) -> Value {
    json!(target["source_bindings"].as_array().into_iter().flatten()
        .chain(target["music_bindings"].as_array().into_iter().flatten()).collect::<Vec<_>>())
}

#[cfg(test)]
mod music_tests {
    use super::*;

    #[test]
    fn music_continuity_keeps_original_target_without_copying_score_material() {
        let target = json!({"publication":{"book_id":"piece","publication_id":"first"},
            "audio":{"recording":{"recording_id":"recording","revision":"1"},"start_ms":1200,"end_ms":1600},
            "score":{"score":{"score_id":"score","revision":"1"},"source_ids":["n1"]},
            "occurrence_ids":["repeat"],"event_ids":["return"],"provenance_ids":["analysis"]});
        let history = source_refs(&json!([{"source_ref_id":"ref","book_id":"piece","source_revision":"original-text",
            "music":{"target":target,"material":{"score_objects":["large prepared material"]}}}]));
        let teaching = source_refs(&target_sources(&json!({"source_bindings":[],
            "music_bindings":[{"source_id":"piece","source_revision":"original-text","target":target}]})));
        assert_eq!(history[0]["music_target"],target);
        assert_eq!(teaching[0]["music_target"],target);
        assert_eq!(teaching[0]["source_revision"],"original-text");
        assert!(!json!(history).to_string().contains("large prepared material"));
    }
}

fn work(
    fact: &TeachingEvent,
    evidence: &[LearningEvidence],
    assessment: Option<&ResponseAssessment>,
) -> Value {
    let receipt = fact
        .payload
        .get("presentation_follow_up")
        .filter(|v| v.is_object())
        .unwrap_or(&fact.payload["scene"]["receipt"]);
    let sources = fact
        .payload
        .get("source_bindings")
        .or_else(|| fact.payload["interaction"].get("source_bindings"))
        .cloned()
        .unwrap_or_else(|| {
            evidence
                .first()
                .and_then(|e| e.target.as_ref())
                .map(|t| target_sources(&json!(t)))
                .unwrap_or(json!([]))
        });
    json!({
        "action_ref":(fact.kind==TeachingFact::LearnerAction).then_some(&fact.event_id),
        "fact_refs":[fact.event_id],"binding":fact.binding,
        "delivery_ref":fact.payload.get("delivery_ref").or_else(||fact.payload.get("invitation_context_ref")),
        "saved_state_ref":receipt["saved_state_ref"],"presentation_receipt":receipt,
        "published_book_ref":fact.payload["published_book_ref"],"source_refs":source_refs(&sources),
        "learner_quote_excerpt":fact.learner_expression().map(|text|preview(&json!(text),600)),
        "response_origin":fact.payload["response_origin"],
        "support_delivery_refs":strings(&fact.payload["support_delivery_refs"]),
        "assistance_refs":strings(&fact.payload["assistance_refs"]),"attempt":fact.payload["attempt"],
        "evidence_refs":evidence.iter().map(|e|&e.evidence_id).collect::<Vec<_>>(),
        "assessment_ref":assessment.map(|a|&a.action_ref),
        "assessment_status":assessment.map(|a|json!(a.status)).unwrap_or_else(||json!(
            if matches!(fact.payload["action"].as_str(),Some("submit"|"revise")) {"pending"} else {"unassessed"}))
    })
}

fn implication(evidence: &LearningEvidence, historical: bool) -> Value {
    json!({"evidence_ref":evidence.evidence_id,"action_ref":evidence.action_ref,
        "teaching_implication":preview(&json!(evidence.teaching_implication),600),
        "interpretation":preview(&json!(evidence.interpretation),600),
        "correction":evidence.correction.as_ref().map(|text|preview(&json!(text),600)),
        "supersedes":evidence.supersedes,"fact_refs":evidence.fact_refs,
        "scope":{"source_id":evidence.source_id,"source_revision":evidence.source_revision,
            "tutor_session_id":evidence.session_id,"design_ref":evidence.design_ref,"block_refs":evidence.block_refs},
        "historical":historical})
}

fn is_work(fact: &TeachingEvent) -> bool {
    fact.learner_expression().is_some()
        || fact.kind == TeachingFact::LearnerAction
            && matches!(fact.payload["action"].as_str(), Some("submit" | "revise"))
}

pub(super) fn context(
    state: &PrivateBookContext<'_>,
    binding: &TeachingBinding,
    current_design: &Value,
    related_interpretations: &[String],
    pending: &[TeachingEvent],
    reference: Option<&str>,
) -> Result<Value, ToolError> {
    let store = state
        .user
        .learning_store_for_workspace(state.learning_workspace_id)?;
    let goal_revision = workspace_goal_revision(state, binding)?;
    let design = &current_design["design"];
    let mut fact_refs = strings(&design["open_question_refs"]);
    let mut evidence_refs = related_interpretations.to_vec();
    for id in strings(&design["basis_refs"]) {
        match store.understanding_basis(binding, &id) {
            Ok((evidence, facts)) => {
                evidence_refs.push(evidence.evidence_id);
                fact_refs.extend(facts.into_iter().map(|fact| fact.event_id));
            }
            Err(error) if error.error_code == "TEACHING_INVALID" => {
                match store.learner_expression_fact(binding, &id) {
                    Ok(fact) => fact_refs.push(fact.event_id),
                    Err(error) if error.error_code == "TEACHING_INVALID" => {}
                    Err(error) => return Err(error),
                }
            }
            Err(error) => return Err(error),
        }
    }
    fact_refs.extend(reference.map(str::to_owned));
    let mut candidates: Vec<_> = pending
        .iter()
        .rev()
        .filter(|fact| {
            fact.binding.tutor_session_id == binding.tutor_session_id
                && fact.binding.source_id == binding.source_id
                && fact.binding.source_revision == binding.source_revision
                && is_work(fact)
        })
        .cloned()
        .collect();
    candidates.extend(store.teaching_continuity_facts(
        binding,
        current_design["design_ref"].as_str(),
        &strings(&design["focus_block_refs"]),
        &fact_refs,
    )?);
    // The current request already has its own grounding. Keep prior work ahead
    // of that duplicate so a longer demonstration history cannot evict it.
    candidates.sort_by_key(|fact| fact.binding.turn_id == binding.turn_id);

    // Keep work visible even when many earlier interpretations fill their
    // preview. Both sections share the existing 6,000-character allowance.
    let mut work_remaining = 4000;
    let mut context_remaining = CONTENT_BUDGET - work_remaining;
    let mut reads = Vec::new();
    let mut questions = Vec::new();
    let mut implications = Vec::new();
    let mut previous = Vec::new();
    let mut omitted = false;
    let mut seen_evidence = BTreeSet::new();
    for id in evidence_refs {
        if !seen_evidence.insert(id.clone()) {
            continue;
        }
        match store.understanding_basis(binding, &id) {
            Ok((evidence, _)) => {
                local_read(&mut reads, &id, true);
                omitted |= !keep(
                    &mut implications,
                    implication(&evidence, false),
                    &mut context_remaining,
                );
            }
            Err(error) if error.error_code == "TEACHING_INVALID" => {}
            Err(error) => return Err(error),
        }
    }
    for id in strings(&design["open_question_refs"]) {
        let fact = store.learner_expression_fact(binding, &id)?;
        local_read(&mut reads, &id, false);
        omitted |= !keep(
            &mut questions,
            json!({"fact_ref":id,
            "learner_quote_excerpt":preview(&json!(fact.learner_expression()),600),
            "support_delivery_refs":fact.payload["support_delivery_refs"],"assistance_refs":fact.payload["assistance_refs"]}),
            &mut context_remaining,
        );
    }

    let mut seen_facts = BTreeSet::new();
    for fact in candidates.iter().filter(|fact| is_work(fact)) {
        if !seen_facts.insert(fact.event_id.clone()) {
            continue;
        }
        local_read(&mut reads, &fact.event_id, false);
        let mut evidence = Vec::new();
        for item in store.teaching_expression_interpretations(&fact.event_id)? {
            match store.understanding_basis(binding, &item.evidence_id) {
                Ok(_) => {
                    local_read(&mut reads, &item.evidence_id, true);
                    if seen_evidence.insert(item.evidence_id.clone()) {
                        omitted |= !keep(
                            &mut implications,
                            implication(&item, false),
                            &mut context_remaining,
                        );
                    }
                    evidence.push(item);
                }
                Err(error) if error.error_code == "TEACHING_INVALID" => {}
                Err(error) => return Err(error),
            }
        }
        let assessment = store.assessment(&fact.event_id)?;
        let mut row = work(fact, &evidence, assessment.as_ref());
        row["historical"] = json!(false);
        // The trace is authoritative for the help that existed at input time.
        // Do not replace it with the activity's newer help list.
        let mut help = Vec::new();
        let help_count = strings(&fact.payload["assistance_refs"]).len()
            + strings(&fact.payload["support_delivery_refs"]).len();
        for id in strings(&fact.payload["assistance_refs"])
            .into_iter()
            .chain(strings(&fact.payload["support_delivery_refs"]))
        {
            local_read(&mut reads, &id, false);
            if help.len() < 2 {
                let saved = store.teaching_event(&id)?;
                if row["source_refs"].as_array().is_some_and(Vec::is_empty) {
                    row["source_refs"] = json!(source_refs(&target_sources(
                        &saved.payload["move"]["target"]
                    )));
                }
                let mut item=json!({"fact_ref":id,"content_excerpt":preview(
                    saved.payload.get("answer").unwrap_or(&saved.payload["text"]),600)});
                if saved.payload["actor"]=="teacher" {
                    item["actor"]=json!("teacher");
                }
                help.push(item);
            }
        }
        row["help"] = json!(help);
        row["help_truncated"] = json!(help_count > 2);
        if row.to_string().chars().count() > work_remaining && !row["help"].as_array().unwrap().is_empty() {
            // Preserve the original words and complete help references before
            // spending the remaining budget on repeated help prose.
            row["help"]=json!([]);
            row["help_truncated"]=json!(true);
        }
        omitted |= !keep(&mut previous, row, &mut work_remaining);
    }
    // The query deliberately reads one more page-sized candidate. Trace remains
    // the route to earlier work even when no particular omitted ID was loaded.
    if candidates.len() >= 13 {
        omitted = true;
        add_read(
            &mut reads,
            "tutor.step",
            json!({"operation":"trace","after":0}),
        );
    }

    if let Some(workspace) = state.learning_workspace_id {
        let selections = store.linked_record_selections()?;
        for (index, linked) in selections.iter().rev().enumerate() {
            let chat = &linked.selection.chat_session_id;
            add_read(
                &mut reads,
                "workspace.records.read",
                json!({"operation":"read","chat_session_id":chat,"after":0}),
            );
            if index >= 3 {
                omitted = true;
                continue;
            }
            let (selection, records) =
                crate::workspace_record_context::selected_records(state.user, workspace, chat)?;
            let facts = records
                .iter()
                .filter(|row| row["kind"] == "fact")
                .map(|row| {
                    serde_json::from_value::<TeachingEvent>(row["record"].clone())
                        .map_err(|e| invalid(e.to_string()))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let evidence = records
                .iter()
                .filter(|row| row["kind"] == "interpretation")
                .map(|row| {
                    serde_json::from_value::<LearningEvidence>(row["record"].clone())
                        .map_err(|e| invalid(e.to_string()))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let superseded: BTreeSet<_> = evidence
                .iter()
                .filter_map(|e| e.supersedes.as_deref())
                .collect();
            // Only selected expressions seed work. Causal help from another chat
            // remains help, and never imports that chat's learner responses.
            let selected_work: Vec<_> = facts
                .iter()
                .rev()
                .filter(|fact| {
                    is_work(fact)
                        && fact.binding.chat_session_id == *chat
                        && linked.selection.turn_ids.contains(&fact.binding.turn_id)
                })
                .collect();
            omitted |= selected_work.len() > 3;
            for fact in selected_work.into_iter().take(3) {
                let current: Vec<_> = evidence
                    .iter()
                    .filter(|e| {
                        !superseded.contains(e.evidence_id.as_str())
                            && (e.action_ref == fact.event_id
                                || e.fact_refs.contains(&fact.event_id))
                    })
                    .cloned()
                    .collect();
                historical_read(&mut reads, &records, chat, &fact.event_id);
                for id in strings(&fact.payload["assistance_refs"])
                    .into_iter()
                    .chain(strings(&fact.payload["support_delivery_refs"]))
                    .chain(current.iter().map(|item| item.evidence_id.clone()))
                {
                    historical_read(&mut reads, &records, chat, &id);
                }
                let assessment = records
                    .iter()
                    .find(|row| {
                        row["kind"] == "assessment" && row["record"]["action_ref"] == fact.event_id
                    })
                    .map(|row| {
                        serde_json::from_value::<ResponseAssessment>(row["record"].clone())
                            .map_err(|e| invalid(e.to_string()))
                    })
                    .transpose()?;
                let original = records.iter().find(|row| {
                    row["kind"] == "chat_turn" && row["turn_id"] == fact.binding.turn_id
                });
                let mut row = work(fact, &current, assessment.as_ref());
                row["historical"] = json!(true);
                row["current_mastery"] = json!(false);
                row["selection"] = selection.clone();
                if let Some(original) = original {
                    row["original_goal"] = original["workspace_goal"].clone();
                    row["music_question"] = original["music_question"].clone();
                    if row["published_book_ref"].is_null() {
                        row["published_book_ref"] = original["published_book_ref"].clone();
                    }
                    let sources = source_refs(&original["sources"]);
                    if !sources.is_empty() {
                        row["source_refs"] = json!(sources);
                    }
                }
                row["help"] = json!(facts
                    .iter()
                    .filter(|help| strings(&fact.payload["assistance_refs"])
                        .contains(&help.event_id)
                        || strings(&fact.payload["support_delivery_refs"]).contains(&help.event_id))
                    .take(2)
                    .map(
                        |help| json!({"fact_ref":help.event_id,"content_excerpt":preview(
                        help.payload.get("answer").unwrap_or(&help.payload["text"]),600)})
                    )
                    .collect::<Vec<_>>());
                row["help_truncated"] = json!(
                    strings(&fact.payload["assistance_refs"]).len()
                        + strings(&fact.payload["support_delivery_refs"]).len()
                        > 2
                );
                omitted |= !keep(&mut previous, row, &mut work_remaining);
                for item in current {
                    if seen_evidence.insert(format!("{chat}:{}", item.evidence_id)) {
                        let mut projected = implication(&item, true);
                        projected["selection"] = selection.clone();
                        omitted |= !keep(&mut implications, projected, &mut context_remaining);
                    }
                }
            }
            // Selected ordinary chat without teaching facts is still original work.
            let plain: Vec<_> = records
                .iter()
                .rev()
                .filter(|row| {
                    row["kind"] == "chat_turn"
                        && !facts.iter().any(|fact| {
                            is_work(fact)
                                && json!(fact.binding.turn_id) == row["turn_id"]
                                && fact.binding.chat_session_id == *chat
                        })
                })
                .collect();
            omitted |= plain.len() > 2;
            for original in plain.into_iter().take(2) {
                omitted |= !keep(
                    &mut previous,
                    json!({"historical":true,"current_mastery":false,"selection":selection,
                    "turn_id":original["turn_id"],"published_book_ref":original["published_book_ref"],
                    "music_question":original["music_question"],
                    "original_goal":original["workspace_goal"],"source_refs":source_refs(&original["sources"]),
                    "learner_quote_excerpt":preview(&original["user"],600),"assessment_status":"unassessed"}),
                    &mut work_remaining,
                );
            }
        }
    }
    Ok(
        json!({"scope":{"learning_workspace_id":state.learning_workspace_id,
        "workspace_goal_revision":goal_revision,"source_id":binding.source_id,"source_revision":binding.source_revision,
        "tutor_session_id":binding.tutor_session_id,"design_ref":current_design["design_ref"],
        "focus_block_refs":design["focus_block_refs"]},
        "previous_work":previous,"open_questions":questions,"teaching_implications":implications,
        "read_more":reads,"truncated":omitted,"content_char_budget":CONTENT_BUDGET,
        "authority":"Original facts, input-time assistance and action-specific assessments. Historical selections retain their original scope; no current mastery is inferred. Teacher working state is separate."}),
    )
}
