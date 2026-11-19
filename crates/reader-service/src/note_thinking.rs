//! Join original note versions to their explicitly related learning history.
use crate::{teaching, user_runtime::UserRuntime};
use memory::{
    learning_evidence::LearningEvidence,
    understanding::{relationship_key, UnderstandingSummary},
    NoteRef,
};
use read_tools::ToolError;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

fn invalid(message: &str) -> ToolError {
    memory::teaching::invalid(message)
}
type ScopedEvidence = (Option<String>, LearningEvidence);
#[cfg(test)]
mod tests;

fn seeds(user: &UserRuntime, note_id: &str) -> Result<Vec<ScopedEvidence>, ToolError> {
    user.store.personal_note(&NoteRef {
        note_id: note_id.into(),
        revision: None,
    })?;
    let mut result = user.learning_store()?.note_evidence(note_id)?;
    // Older records can be joined only through an explicit, single-note discussion.
    // Multi-note turns require the new, quoted attribution recorded on the evidence.
    for discussion in user.store.note_discussions(note_id) {
        let Some(session) = user
            .agent_history
            .sessions
            .iter()
            .find(|s| s.id == discussion.turn.session_id)
        else {
            continue;
        };
        let Some(turn) = session
            .turns
            .iter()
            .find(|t| t.turn_id == discussion.turn.turn_id)
        else {
            continue;
        };
        let Some(context) = &turn.domain.personal_notes else {
            continue;
        };
        let refs: Vec<NoteRef> =
            serde_json::from_value(context.get("references").cloned().unwrap_or(json!([])))
                .map_err(|_| invalid("原讨论笔记引用不可读"))?;
        if refs.len() != 1 || refs[0] != discussion.reference {
            continue;
        }
        let store = user.learning_store_for_workspace(session.learning_workspace_id.as_deref())?;
        for mut e in store.evidence_for_expression(&session.id, &turn.turn_id)? {
            // New interpretations deliberately omit note links when the learner changes topic.
            if !e.note_refs.is_empty() || e.interpreter_version == "resident.hypothesis.v2" {
                continue;
            }
            if result.iter().any(|(scope, old)| {
                scope == &session.learning_workspace_id && old.evidence_id == e.evidence_id
            }) {
                continue;
            }
            e.note_refs = vec![discussion.reference.clone()];
            result.push((session.learning_workspace_id.clone(), e));
        }
    }
    Ok(result)
}

fn summaries(
    user: &UserRuntime,
    seeds: &[ScopedEvidence],
) -> Result<Vec<UnderstandingSummary>, ToolError> {
    let scopes: BTreeSet<_> = seeds
        .iter()
        .map(|(workspace, e)| {
            (
                workspace.clone(),
                e.source_id.clone(),
                e.source_revision.clone(),
            )
        })
        .collect();
    let mut result = Vec::new();
    for (workspace, source, revision) in scopes {
        let store = user.learning_store_for_workspace(workspace.as_deref())?;
        let focuses: Vec<_> = seeds
            .iter()
            .filter(|(w, e)| {
                w == &workspace && e.source_id == source && e.source_revision == revision
            })
            .map(|(_, e)| relationship_key(e))
            .collect();
        for mut item in store.understanding_summaries_for(&source, &revision, &focuses)? {
            let matching: Vec<_> = seeds
                .iter()
                .filter(|(w, e)| {
                    w == &workspace
                        && e.source_id == source
                        && e.source_revision == revision
                        && relationship_key(e) == item.focus_ref
                        && e.capability == item.capability
                })
                .collect();
            if matching.is_empty() {
                continue;
            }
            for (_, e) in matching {
                for reference in &e.note_refs {
                    if !item.note_refs.contains(reference) {
                        item.note_refs.push(reference.clone())
                    }
                }
            }
            item.note_refs
                .retain(|r| user.store.personal_note(r).is_ok());
            item.source_session_title = user
                .agent_history
                .sessions
                .iter()
                .find(|s| Some(s.id.as_str()) == item.source_session_id.as_deref())
                .map(|s| s.title.clone());
            result.push(item);
        }
    }
    Ok(result)
}

pub(crate) fn page(user: &UserRuntime, input: &Value) -> Result<Value, ToolError> {
    let note_id = input["note_id"]
        .as_str()
        .ok_or_else(|| invalid("缺少笔记身份"))?;
    let note = user.store.personal_note(&NoteRef {
        note_id: note_id.into(),
        revision: None,
    })?;
    let seeds = seeds(user, note_id)?;
    let current = summaries(user, &seeds)?;
    let mut timeline = Vec::new();
    let mut before = None;
    loop {
        let versions = user.store.personal_note_history(note_id, before, 50)?;
        if versions.is_empty() {
            break;
        }
        before = versions.last().map(|v| v.revision);
        for version in versions {
            timeline.push(json!({"id":format!("revision:{}",version.revision),"kind":"revision","occurred_at":version.saved_at,
                "reference":{"note_id":note_id,"revision":version.revision},"text":version.record.content,"authorship":version.authorship,
                "change_note":version.change_note,"based_on_turn":version.based_on_turn,"restored_from":version.restored_from}));
        }
    }
    let discussions = user.store.note_discussions(note_id);
    let mut statuses = Vec::new();
    for discussion in &discussions {
        let session = user
            .agent_history
            .sessions
            .iter()
            .find(|s| s.id == discussion.turn.session_id);
        let workspace = session.and_then(|s| s.learning_workspace_id.as_deref());
        let store = user.learning_store_for_workspace(workspace)?;
        if let Some(job) = store.coach_job_for_turn(&discussion.turn.turn_id)? {
            statuses.push(job);
        }
        timeline.push(json!({"id":format!("discussion:{}:{}",discussion.turn.session_id,discussion.turn.turn_id),"kind":"discussion",
            "occurred_at":discussion.created_at,"reference":discussion.reference,"text":discussion.expression,
            "source_session_id":discussion.turn.session_id,"source_turn_id":discussion.turn.turn_id,"session_title":session.map(|s|&s.title)}));
    }
    // Include all observations of an explicitly connected relationship. Exact note versions
    // identify its origin even when later progress happened without editing the note.
    let mut observations: BTreeMap<(Option<String>, String), LearningEvidence> = BTreeMap::new();
    for (scope, e) in &seeds {
        observations.insert((scope.clone(), e.evidence_id.clone()), e.clone());
    }
    for item in &current {
        let store = user.learning_store_for_workspace(item.workspace_id.as_deref())?;
        let mut after = 0;
        loop {
            let entries = store.understanding_timeline(&item.evidence_ref, after, 20)?;
            if entries.is_empty() {
                break;
            }
            after += entries.len() as u64;
            for (_, e) in entries {
                if relationship_key(&e) == item.focus_ref && e.capability == item.capability {
                    observations.insert((item.workspace_id.clone(), e.evidence_id.clone()), e);
                }
            }
        }
    }
    for ((workspace, _), e) in observations {
        let store = user.learning_store_for_workspace(workspace.as_deref())?;
        let chronology = store.evidence_chronology(&e.evidence_id)?;
        timeline.push(json!({"id":e.evidence_id,"kind":if e.correction.is_some(){"correction"}else{"understanding"},
            "occurred_at":e.corrected_at.as_ref().map(|v|json!(v)).unwrap_or_else(||chronology["occurred_at"].clone()),"source_session_id":chronology["source_session_id"],"source_turn_id":chronology["source_turn_id"],
            "workspace_id":workspace,"source_id":e.source_id,"source_revision":e.source_revision,"label":e.label,"text":e.learner_quote,
            "interpretation":e.interpretation,"correction":e.correction,"note_refs":e.note_refs,
            "assisted":!e.assistance_refs.is_empty() || !e.support_delivery_refs.is_empty(),"attempt":e.attempt,"nature":e.nature,
            "superseded":store.evidence_is_superseded(&e.evidence_id)?,"questionable":store.evidence_is_questionable(&e)?}));
    }
    for (workspace, delivery) in user.learning_store()?.note_deliveries(note_id)? {
        let store = user.learning_store_for_workspace(workspace.as_deref())?;
        if delivery.payload["move"]["practice"].is_object()
            || delivery.payload["assessment_contract_ref"].is_string()
        {
            timeline.push(json!({"id":delivery.event_id,"kind":"activity","occurred_at":delivery.occurred_at,"text":delivery.payload["move"]["prompt"],
                "workspace_id":workspace,"source_id":delivery.binding.source_id,"source_session_id":delivery.binding.chat_session_id,"source_turn_id":delivery.binding.turn_id}));
            for event in store.teaching_activity_events(&delivery.event_id)? {
                if matches!(
                    event.payload["action"].as_str(),
                    Some("skip" | "request_hint" | "reveal")
                ) {
                    timeline.push(json!({"id":event.event_id,"kind":"action","occurred_at":event.occurred_at,"action":event.payload["action"],
                        "source_session_id":event.binding.chat_session_id,"source_turn_id":event.binding.turn_id}));
                }
            }
        }
    }
    timeline.sort_by_key(|entry| {
        (
            entry["occurred_at"]
                .as_str()
                .and_then(teaching::timeline::time_key),
            entry["id"].as_str().map(str::to_owned),
        )
    });
    let after = input["after"].as_u64().unwrap_or(0) as usize;
    let total = timeline.len();
    let entries: Vec<_> = timeline.into_iter().rev().skip(after).take(20).collect();
    let update_status = if statuses
        .iter()
        .any(|s| matches!(s["status"].as_str(), Some("pending" | "running")))
    {
        "pending"
    } else if statuses
        .iter()
        .any(|s| matches!(s["status"].as_str(), Some("failed" | "interrupted")))
    {
        "failed"
    } else {
        "ready"
    };
    Ok(
        json!({"note_id":note_id,"current_revision":note.current_revision,"current_understanding":current,"timeline":entries,
        "next_cursor":(after+20<total).then_some(after+20),"update_status":update_status,"updates":statuses}),
    )
}

pub(crate) fn retry(user: &UserRuntime, input: &Value, now: &str) -> Result<Value, ToolError> {
    let note_id = input["note_id"]
        .as_str()
        .ok_or_else(|| invalid("缺少笔记身份"))?;
    user.store.personal_note(&NoteRef {
        note_id: note_id.into(),
        revision: None,
    })?;
    let workspace = input["workspace_id"].as_str();
    let mut store = user.learning_store_for_workspace(workspace)?;
    let job = store
        .coach_job(input["job_ref"].as_str().unwrap_or_default())?
        .ok_or_else(|| invalid("理解整理任务不存在"))?;
    if !user.store.note_discussions(note_id).iter().any(|d| {
        d.turn.turn_id == job.input.binding.turn_id
            && d.turn.session_id == job.input.binding.chat_session_id
    }) {
        return Err(invalid("理解整理任务与此笔记没有明确关联"));
    }
    let result = store.retry_coach_job(&job.input.job_ref, now)?;
    Ok(json!({"status":result.status}))
}

pub(crate) fn evidence(
    user: &UserRuntime,
    input: &Value,
    correct: bool,
) -> Result<Value, ToolError> {
    let note_id = input["note_id"]
        .as_str()
        .ok_or_else(|| invalid("缺少笔记身份"))?;
    let workspace = input["workspace_id"].as_str();
    let id = input["evidence_ref"]
        .as_str()
        .ok_or_else(|| invalid("缺少理解依据"))?;
    let store = user.learning_store_for_workspace(workspace)?;
    let candidate = store.evidence(id)?;
    if !seeds(user, note_id)?.iter().any(|(scope, e)| {
        scope.as_deref() == workspace
            && e.source_id == candidate.source_id
            && e.source_revision == candidate.source_revision
            && relationship_key(e) == relationship_key(&candidate)
            && e.capability == candidate.capability
    }) {
        return Err(invalid("这条理解没有与此笔记明确关联"));
    }
    if correct {
        let next = user
            .learning_store_for_workspace(workspace)?
            .correct_evidence(
                id,
                input["operation_id"].as_str().unwrap_or_default(),
                input["text"].as_str().unwrap_or_default(),
            )?;
        user.learning_store_for_workspace(workspace)?
            .rebuild_learner_projection(&candidate.source_id)?;
        Ok(json!({"evidence_ref":next.evidence_id}))
    } else {
        teaching::retained_evidence_view(user, workspace, &candidate.source_id, id)
    }
}

pub(crate) fn start(user: &UserRuntime, input: &Value) -> Result<Value, ToolError> {
    let e = evidence(user, input, false)?;
    if e["superseded"] == true {
        return Err(invalid("这条判断已更新，请刷新后继续"));
    }
    let session = user
        .agent_history
        .sessions
        .iter()
        .find(|s| Some(s.id.as_str()) == e["source_session_id"].as_str())
        .ok_or_else(|| invalid("原讨论暂时不可用"))?;
    let turn = session
        .turns
        .iter()
        .find(|t| Some(t.turn_id.as_str()) == e["source_turn_id"].as_str())
        .ok_or_else(|| invalid("原表达暂时不可用"))?;
    let candidate: LearningEvidence =
        serde_json::from_value(e.clone()).map_err(|_| invalid("理解记录不可读"))?;
    let reference = candidate
        .note_refs
        .iter()
        .find(|r| Some(r.note_id.as_str()) == input["note_id"].as_str())
        .cloned()
        .or(seeds(user, input["note_id"].as_str().unwrap_or_default())?
            .into_iter()
            .rev()
            .find(|(scope, seed)| {
                scope.as_deref() == input["workspace_id"].as_str()
                    && seed.source_id == e["source_id"]
                    && seed.source_revision == e["source_revision"]
                    && relationship_key(seed) == relationship_key(&candidate)
                    && seed.capability == candidate.capability
            })
            .and_then(|(_, seed)| {
                seed.note_refs
                    .into_iter()
                    .find(|r| Some(r.note_id.as_str()) == input["note_id"].as_str())
            }))
        .ok_or_else(|| invalid("原笔记版本暂时不可用"))?;
    Ok(
        json!({"context":{"book_id":e["source_id"],"publication_id":turn.published_book_ref.as_ref().map(|p|&p.publication_id),
        "workspace_id":session.learning_workspace_id,"session_id":session.id,"turn_id":turn.turn_id,"label":e["label"]},
        "note_ref":reference,"source_revision":e["source_revision"],"target":e["target"],
        "understanding_ref":{"note_id":input["note_id"],"workspace_id":session.learning_workspace_id,"evidence_ref":e["evidence_id"]}}),
    )
}
