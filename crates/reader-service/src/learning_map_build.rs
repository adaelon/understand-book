//! Background construction of the private learning map (ADR-0178 §7, slice M1).
//! The queue is the teaching trace: an opted-in workspace's delivered
//! explanations without coverage are pending. Extraction reads only the
//! delivered text and the passages it cited; the host keeps a relationship
//! only when its quote is really in the cited passage, and links across books
//! only on the identity model's explicit judgment.
use crate::*;
use memory::learning_map::{MapMerge, MapNode, MapSource};
use memory::teaching::{TeachingEvent, TeachingSourceBinding};
use runtime::learning_map::{ExtractedChain, ExtractedProposition, Extraction, IdentityJudgment};
use runtime::run_context::{CancellableAdapter, CancellationToken};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

/// Nodes from elsewhere offered per proposition; overlapping passages are always offered.
const SIMILAR_CANDIDATES: usize = 4;
const MIN_SIMILARITY: f64 = 0.2;
const MIN_QUOTE_CHARS: usize = 4;

pub(crate) struct Passage {
    pub(crate) binding: TeachingSourceBinding,
    pub(crate) text: String,
}

pub struct PendingBuild {
    pub workspace: Option<String>,
    pub delivery: TeachingEvent,
    pub(crate) book: Arc<read_tools::Book>,
    pub(crate) publication_id: Option<String>,
    pub(crate) passages: Vec<Passage>,
    pub(crate) nodes: Vec<MapNode>,
}

/// Deliveries scanned per tick, so ones waiting for another book never hide newer ones.
const SCAN: u32 = 64;

pub enum Opened {
    /// The book the delivery was taught from, with its publication when there is one.
    Book(Arc<read_tools::Book>, Option<String>),
    /// Not readable on this host right now (another local book is open): wait, spend nothing.
    Elsewhere,
    /// Never readable again: a bounded failure.
    Gone,
}

pub type OpenBook<'a> = dyn Fn(&TeachingEvent) -> Opened + 'a;

fn passages(delivery: &TeachingEvent, book: &read_tools::Book) -> Vec<Passage> {
    let bindings: Vec<TeachingSourceBinding> =
        serde_json::from_value(delivery.payload["move"]["target"]["source_bindings"].clone()).unwrap_or_default();
    let mut seen = BTreeSet::new();
    bindings.into_iter()
        .filter(|b| b.source_id == book.base.book_id && b.source_revision == book.source_fingerprint())
        .filter(|b| seen.insert((b.start_lid.clone(), b.end_lid.clone())))
        .filter_map(|binding| book.text(&binding.start_lid, Some(&binding.end_lid)).ok().map(|text| Passage { binding, text }))
        .collect()
}

pub fn next(user: &user_runtime::UserRuntime, open: &OpenBook<'_>) -> Result<Option<PendingBuild>, ToolError> {
    if !user.store.private_storage_available() { return Ok(None); }
    for workspace in crate::practice_check::scopes(user) {
        let mut store = user.learning_store_for_workspace(workspace.as_deref())?;
        if !store.learning_map_enabled()? { continue; }
        for delivery in store.uncovered_map_deliveries(SCAN)? {
            match open(&delivery) {
                Opened::Elsewhere => continue,
                Opened::Book(book, publication_id) => {
                    let passages = passages(&delivery, &book);
                    if !passages.is_empty() {
                        return Ok(Some(PendingBuild { nodes: store.map_nodes()?, workspace, delivery, book, publication_id, passages }));
                    }
                    // The open book is a different revision: the cited passages are gone.
                    store.record_map_failure(&delivery.event_id)?;
                }
                Opened::Gone => store.record_map_failure(&delivery.event_id)?,
            }
        }
    }
    Ok(None)
}

fn normalized(text: &str) -> String { text.split_whitespace().collect::<Vec<_>>().join(" ") }

/// Keep only relationships whose quote is really in the cited passage; chains
/// survive only when every member does.
pub(crate) fn verify(extraction: Extraction, passages: &[Passage]) -> (Vec<(ExtractedProposition, usize)>, Vec<ExtractedChain>) {
    let kept: Vec<(ExtractedProposition, usize)> = extraction.propositions.into_iter().filter_map(|p| {
        let quote = normalized(&p.quote);
        let index = passages.iter().position(|passage| passage.binding.start_lid == p.start_lid && passage.binding.end_lid == p.end_lid)?;
        (quote.chars().count() >= MIN_QUOTE_CHARS && normalized(&passages[index].text).contains(&quote)).then_some((p, index))
    }).collect();
    let keys: BTreeSet<&str> = kept.iter().map(|(p, _)| p.key.as_str()).collect();
    let chains = extraction.chains.into_iter()
        .filter(|c| keys.contains(c.conclusion.as_str()) && c.premises.iter().all(|p| keys.contains(p.as_str())))
        .collect();
    (kept, chains)
}

fn bigrams(text: &str) -> BTreeSet<(char, char)> {
    let chars: Vec<char> = text.to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect();
    chars.windows(2).map(|w| (w[0], w[1])).collect()
}

/// Shared wording only nominates a candidate; the identity model decides.
pub(crate) fn similarity(a: &str, b: &str) -> f64 {
    let (a, b) = (bigrams(a), bigrams(b));
    let union = a.union(&b).count();
    if union == 0 { 0.0 } else { a.intersection(&b).count() as f64 / union as f64 }
}

fn overlaps(book: &read_tools::Book, a: &TeachingSourceBinding, b: &MapSource) -> bool {
    let span = |lid: &str| book.base.lid_nodes.iter().find(|n| n.lid == lid).map(|n| n.span.clone());
    a.source_id == b.source_id && a.source_revision == b.source_revision
        && match (span(&a.start_lid), span(&a.end_lid), span(&b.start_lid), span(&b.end_lid)) {
            (Some(a1), Some(a2), Some(b1), Some(b2)) => a1.start < b2.end && b1.start < a2.end,
            _ => false,
        }
}

pub(crate) fn candidates(pending: &PendingBuild, kept: &[(ExtractedProposition, usize)]) -> BTreeMap<String, BTreeSet<String>> {
    kept.iter().filter_map(|(p, index)| {
        let binding = &pending.passages[*index].binding;
        let (near, rest): (Vec<&MapNode>, Vec<&MapNode>) = pending.nodes.iter()
            .partition(|node| node.sources.iter().any(|s| overlaps(&pending.book, binding, s)));
        let mut similar: Vec<(f64, &MapNode)> = rest.into_iter().map(|n| (similarity(&p.statement, &n.statement), n))
            .filter(|(score, _)| *score >= MIN_SIMILARITY).collect();
        similar.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.node_id.cmp(&b.1.node_id)));
        let ids: BTreeSet<String> = near.into_iter().chain(similar.into_iter().take(SIMILAR_CANDIDATES).map(|(_, n)| n))
            .map(|n| n.node_id.clone()).collect();
        (!ids.is_empty()).then(|| (p.key.clone(), ids))
    }).collect()
}

fn extraction_packet(pending: &PendingBuild) -> Value {
    json!({"explanation":pending.delivery.payload["answer"],
        "passages":pending.passages.iter().map(|p| json!({"start_lid":p.binding.start_lid,"end_lid":p.binding.end_lid,"text":p.text}))
            .collect::<Vec<_>>()})
}

fn identity_packet(pending: &PendingBuild, kept: &[(ExtractedProposition, usize)], offered: &BTreeMap<String, BTreeSet<String>>) -> Value {
    let node = |id: &String| pending.nodes.iter().find(|n| &n.node_id == id).map(|n| json!({"node_id":n.node_id,"statement":n.statement,
        "quotes":n.sources.iter().map(|s| json!({"source_id":s.source_id,"quote":s.quote})).collect::<Vec<_>>()}));
    json!({"propositions":kept.iter().filter(|(p, _)| offered.contains_key(&p.key)).map(|(p, _)| json!({"key":p.key,"statement":p.statement,
            "source_id":pending.book.base.book_id,"quote":p.quote,"candidates":offered[&p.key].iter().filter_map(node).collect::<Vec<_>>()}))
        .collect::<Vec<_>>()})
}

/// Pure assembly of one delivery's contribution from verified material.
pub(crate) fn assemble(pending: &PendingBuild, kept: &[(ExtractedProposition, usize)], chains: &[ExtractedChain],
    judgment: &IdentityJudgment) -> MapMerge {
    let delivery_ref = &pending.delivery.event_id;
    let linked: BTreeMap<&str, &str> = judgment.links.iter()
        .filter_map(|l| l.node_id.as_deref().map(|id| (l.key.as_str(), id))).collect();
    let id_of = |key: &str| linked.get(key).map(|id| id.to_string()).unwrap_or_else(|| memory::learning_map::node_id(delivery_ref, key));
    let mut merge = MapMerge { delivery_ref: delivery_ref.clone(), ..Default::default() };
    for (p, index) in kept {
        let id = id_of(&p.key);
        if !linked.contains_key(p.key.as_str()) && !merge.new_nodes.iter().any(|(n, _)| n == &id) {
            merge.new_nodes.push((id.clone(), p.statement.clone()));
        }
        let b = &pending.passages[*index].binding;
        merge.sources.push((id, MapSource { source_id: b.source_id.clone(), source_revision: b.source_revision.clone(),
            start_lid: b.start_lid.clone(), end_lid: b.end_lid.clone(), quote: p.quote.clone(), delivery_ref: delivery_ref.clone(),
            turn_id: pending.delivery.binding.turn_id.clone(), publication_id: pending.publication_id.clone() }));
    }
    for chain in chains {
        let conclusion = id_of(&chain.conclusion);
        let premises: BTreeSet<String> = chain.premises.iter().map(|k| id_of(k)).collect();
        // Two propositions judged to be one relationship can collapse an argument into a tautology.
        if !premises.contains(&conclusion) && !merge.chains.iter().any(|(p, c)| c == &conclusion && p.iter().cloned().collect::<BTreeSet<_>>() == premises) {
            merge.chains.push((premises.into_iter().collect(), conclusion));
        }
    }
    merge
}

pub(crate) fn build(adapter: &dyn ModelAdapter, pending: &PendingBuild) -> Result<MapMerge, String> {
    let extraction = runtime::learning_map::extract(adapter, &extraction_packet(pending)).map_err(|e| e.message)?;
    let (kept, chains) = verify(extraction, &pending.passages);
    let offered = candidates(pending, &kept);
    let judgment = if offered.is_empty() { IdentityJudgment::default() } else {
        runtime::learning_map::judge(adapter, &identity_packet(pending, &kept, &offered), &offered).map_err(|e| e.message)?
    };
    Ok(assemble(pending, &kept, &chains, &judgment))
}

/// A merge that no longer fits (the map was cleared meanwhile) counts as one failure.
pub fn record(user: &user_runtime::UserRuntime, pending: &PendingBuild, result: Result<MapMerge, String>) -> Result<(), ToolError> {
    let mut store = user.learning_store_for_workspace(pending.workspace.as_deref())?;
    if !store.learning_map_enabled()? { return Ok(()); }
    match result.map(|merge| store.merge_map(&merge)) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(_)) | Err(_) => store.record_map_failure(&pending.delivery.event_id),
    }
}

pub fn run(adapter: &dyn ModelAdapter, pending: &PendingBuild, cancel: &CancellationToken) -> Option<Result<MapMerge, String>> {
    adapter.set_run_cancellation(cancel.clone());
    let adapter = CancellableAdapter { inner: adapter, cancellation: cancel.clone() };
    let result = build(&adapter, pending);
    // An interrupted host leaves the delivery pending instead of spending a failure.
    (!cancel.is_cancelled()).then_some(result)
}

pub(crate) fn tick_local(state: &Arc<Mutex<AppState>>, provider: Option<ProviderConfig>, cancel: &CancellationToken) -> Result<bool, ToolError> {
    let Some(config) = provider else { return Ok(false); };
    let pending = {
        let guard = state.lock().unwrap();
        let book = guard.workspace.book.clone();
        let publication = guard.workspace.publication.as_ref().map(|p| p.reference.publication_id.clone());
        let open = move |delivery: &TeachingEvent| if delivery.binding.source_id == book.base.book_id {
            Opened::Book(book.clone(), publication.clone())
        } else { Opened::Elsewhere };
        next(&guard.user, &open)?
    };
    let Some(pending) = pending else { return Ok(false); };
    let adapter = ProviderRegistry::adapter_from_config_with_timeout(config, crate::SELECTION_TRANSLATION_TIMEOUT);
    let Some(result) = run(adapter.as_ref(), &pending, cancel) else { return Ok(false); };
    record(&state.lock().unwrap().user, &pending, result)?;
    Ok(true)
}

