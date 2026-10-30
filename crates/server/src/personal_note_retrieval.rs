//! Candidate recall is personal context, never evidence of a newly acquired skill.
use memory::{MemoryStore, NoteRef, NoteSearchHit};
use read_tools::ToolError;
use serde_json::{json, Value};
use std::collections::BTreeMap;

fn snapshot(store: &MemoryStore, query: Option<&str>, limit: usize) -> Value {
    json!({"kind":"personal_notes","cache_dir":store.note_index_path(),"snapshot_revision":store.document_revision(),"notes":store.note_index_documents(),"query":query,"top_k":limit})
}
pub(crate) fn schedule(store: &MemoryStore) {
    if std::env::var_os("UNDERSTAND_BOOK_EMBEDDING_CONFIG").is_none() {
        return;
    }
    let request = snapshot(store, None, 20);
    std::thread::spawn(move || {
        let _ = crate::semantic_retrieval::personal_notes(&request);
    });
}

pub(crate) fn search(user: &crate::user_runtime::UserRuntime, input: &Value) -> Result<Value, ToolError> {
    let prepared = prepare_user(user, input)?;
    let vectors = infer(&prepared);
    finish(&user.store, prepared, vectors)
}
#[cfg(test)]
fn search_with(
    store: &MemoryStore,
    input: &Value,
    semantic: impl FnOnce(&Value) -> Result<Value, String>,
) -> Result<Value, ToolError> {
    let prepared = prepare(store, input)?;
    let vectors = semantic(&prepared.request)
        .unwrap_or_else(|reason| json!({"status":"unavailable","reason":reason,"hits":[]}));
    finish(store, prepared, vectors)
}

pub(crate) struct SearchSnapshot {
    request: Value,
    lexical: Vec<NoteSearchHit>,
    limit: usize,
    book_id: Option<String>,
    /// Third ranker, only on request: (ranked notes, status).
    structural: Option<(Vec<NoteRef>, Value)>,
}

pub(crate) fn prepare(store: &MemoryStore, input: &Value) -> Result<SearchSnapshot, ToolError> {
    store.ensure_storage_available()?;
    let query = input["query"].as_str().unwrap_or_default();
    let limit = input["limit"].as_u64().unwrap_or(8).clamp(1, 20) as usize;
    let book_id = input["book_id"].as_str();
    let lexical = store.search_personal_notes(query, book_id, 40)?;
    let request = snapshot(store, Some(query), 80);
    Ok(SearchSnapshot {
        request,
        lexical,
        limit,
        book_id: book_id.map(str::to_owned),
        structural: None,
    })
}

/// `structural: true` adds learning-map recall as a third RRF ranker (M0 evaluation).
pub(crate) fn prepare_user(user: &crate::user_runtime::UserRuntime, input: &Value) -> Result<SearchSnapshot, ToolError> {
    let mut prepared = prepare(&user.store, input)?;
    if input["structural"] == true {
        prepared.structural = Some(crate::learning_map_recall::structural(user, input)?);
    }
    Ok(prepared)
}

/// Inference and vector-cache I/O run after the host releases its user state lock.
pub(crate) fn infer(prepared: &SearchSnapshot) -> Value {
    crate::semantic_retrieval::personal_notes(&prepared.request)
        .unwrap_or_else(|reason| json!({"status":"unavailable","reason":reason,"hits":[]}))
}

pub(crate) fn finish(
    store: &MemoryStore,
    prepared: SearchSnapshot,
    vectors: Value,
) -> Result<Value, ToolError> {
    let SearchSnapshot {
        request,
        lexical,
        limit,
        book_id,
        structural,
    } = prepared;
    let book_id = book_id.as_deref();
    let mut candidates: BTreeMap<String, NoteSearchHit> = lexical
        .into_iter()
        .filter(|hit| {
            store
                .personal_note(&NoteRef {
                    note_id: hit.reference.note_id.clone(),
                    revision: None,
                })
                .is_ok_and(|note| Some(note.current_revision) == hit.reference.revision)
        })
        .map(|mut hit| {
            hit.score = 1.0 / (60.0 + hit.lexical_rank.unwrap() as f64);
            (hit.reference.note_id.clone(), hit)
        })
        .collect();
    for (rank, hit) in vectors["hits"].as_array().into_iter().flatten().enumerate() {
        let Some(id) = hit["note_id"].as_str() else {
            continue;
        };
        let Ok(current) = store.personal_note(&NoteRef {
            note_id: id.into(),
            revision: None,
        }) else {
            continue;
        };
        if hit["revision"].as_u64() != Some(current.current_revision as u64)
            || book_id.is_some_and(|id| current.value.context.book_id.as_deref() != Some(id))
        {
            continue;
        }
        // Keep weakly related semantic neighbours out of automatic teaching context.
        if hit["score"].as_f64().unwrap_or_default() < 0.3 {
            continue;
        }
        let excerpt = request["notes"]
            .as_array()
            .and_then(|notes| notes.iter().find(|note| note["note_id"] == id))
            .and_then(|note| note["text"].as_str())
            .and_then(|text| {
                let units: Vec<_> = text.encode_utf16().collect();
                let start = hit["start_utf16"].as_u64()? as usize;
                let end = hit["end_utf16"].as_u64()? as usize;
                units.get(start..end).map(String::from_utf16_lossy)
            })
            .unwrap_or_else(|| current.value.record.content.chars().take(600).collect());
        let candidate = candidates
            .entry(id.into())
            .or_insert_with(|| NoteSearchHit {
                reference: NoteRef {
                    note_id: id.into(),
                    revision: Some(current.current_revision),
                },
                title: current.value.title.clone(),
                excerpt,
                authorship: current.value.authorship,
                context: current.value.context.clone(),
                lexical_rank: None,
                semantic_rank: None,
                structural_rank: None,
                score: 0.0,
            });
        candidate.semantic_rank = Some(rank + 1);
        candidate.score += 1.0 / (61.0 + rank as f64);
    }
    let structural_status = structural.as_ref().map(|(_, status)| status.clone());
    for (rank, reference) in structural.map(|(ranked, _)| ranked).unwrap_or_default().into_iter().enumerate() {
        let Ok(current) = store.personal_note(&NoteRef { note_id: reference.note_id.clone(), revision: None }) else { continue; };
        if reference.revision != Some(current.current_revision)
            || book_id.is_some_and(|id| current.value.context.book_id.as_deref() != Some(id)) {
            continue;
        }
        let candidate = candidates.entry(reference.note_id.clone()).or_insert_with(|| NoteSearchHit {
            reference: reference.clone(), title: current.value.title.clone(),
            excerpt: current.value.record.content.chars().take(600).collect(), authorship: current.value.authorship,
            context: current.value.context.clone(), lexical_rank: None, semantic_rank: None, structural_rank: None, score: 0.0 });
        candidate.structural_rank = Some(rank + 1);
        candidate.score += 1.0 / (61.0 + rank as f64);
    }
    let mut hits: Vec<_> = candidates.into_values().collect();
    hits.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then(a.reference.note_id.cmp(&b.reference.note_id))
    });
    hits.truncate(limit);
    Ok(
        json!({"hits":hits,"semantic":{"status":vectors["status"],"reason":vectors["reason"],"indexed_notes":vectors["indexed_notes"],"pending_notes":vectors["pending_notes"],
        "embedded_chunks":vectors["embedded_chunks"],"cache_hits":vectors["cache_hits"],"query_tokens":vectors["query_tokens"],"document_tokens":vectors["document_tokens"]},
        "structural":structural_status}),
    )
}

pub(crate) fn search_user(
    port: &impl crate::agent_run::UserStatePort,
    input: &Value,
) -> Result<Value, ToolError> {
    search_user_with(port, input, |prepared| infer(prepared))
}

fn search_user_with(
    port: &impl crate::agent_run::UserStatePort,
    input: &Value,
    infer: impl FnOnce(&SearchSnapshot) -> Value,
) -> Result<Value, ToolError> {
    port.check_access()?;
    let (owner, prepared) = port.with_user(|user| {
        let owner = user
            .user_id()
            .ok_or_else(crate::authorization::missing)?
            .to_string();
        Ok::<_, ToolError>((owner, prepare_user(user, input)?))
    })?;
    let vectors = infer(&prepared);
    port.check_access()?;
    port.with_user(|user| {
        user.check_owner(&owner)?;
        finish(&user.store, prepared, vectors)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn personal_notes_inference_releases_user_lock_and_drops_changed_candidates() {
        let root = tempfile::tempdir().unwrap();
        let mut state = crate::tests::state_named("pn-lock");
        state.user.store = MemoryStore::open(root.path().join("memory.json")).unwrap();
        let content =
            serde_json::from_value(json!({"content":"固定参数与输入","authorship":"user"}))
                .unwrap();
        let first = state
            .user
            .store
            .create_personal_note(content, "create", "1")
            .unwrap();
        let port = std::sync::Arc::new(std::sync::Mutex::new(state));
        let result=search_user_with(&port,&json!({"query":"固定参数"}), |_| {
            let mut state=port.try_lock().expect("inference must not prevent saving an expression");
            let content=serde_json::from_value(json!({"content":"我现在已经区分了固定参数和变化的激活","authorship":"user"})).unwrap();
            state.user.store.revise_personal_note(&first.note_id,1,content,"edit","2").unwrap();
            json!({"status":"ready","hits":[{"note_id":first.note_id,"revision":1,"score":0.95,"start_utf16":0,"end_utf16":7}]})
        }).unwrap();
        assert!(result["hits"].as_array().unwrap().is_empty());
        assert_eq!(
            port.lock()
                .unwrap()
                .user
                .store
                .personal_note(&NoteRef {
                    note_id: first.note_id,
                    revision: None
                })
                .unwrap()
                .current_revision,
            2
        );
    }
}
