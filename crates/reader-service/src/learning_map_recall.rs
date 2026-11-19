//! Notes that touch the private learning map, structural recall, and link
//! suggestions (ADR-0178 §8, slice M2). Everything here is derived at read
//! time; only the user's removals are stored. A touch is a system
//! interpretation of where a note was written, never evidence of understanding.
//! Structural recall is an unproven hypothesis: it runs only when a caller asks
//! for it, which today is the M0 evaluation harness.
use crate::*;
use memory::learning_map::{MapChain, MapNode, MapSource};
use memory::{NoteRef, PersonalNoteView};
use memory::teaching::invalid;
use std::collections::{BTreeMap, BTreeSet};

/// Premise steps followed back from the focus.
const MAX_HOPS: u32 = 2;
/// Map relationships taken as the focus when only a query is known.
const QUERY_SEEDS: usize = 3;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Touch {
    pub(crate) note: NoteRef,
    pub(crate) node_id: String,
    /// "turn": written against the delivering turn; "anchor": anchored inside the cited passage of the same publication.
    pub(crate) via: &'static str,
}

fn lid_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let parts = |lid: &str| lid.split('.').map(|part| part.parse::<u64>().map_err(|_| part.to_owned())).collect::<Vec<_>>();
    parts(a).cmp(&parts(b))
}

/// Hierarchical LIDs: a descendant of the end LID is still inside the range.
pub(crate) fn lid_within(lid: &str, start: &str, end: &str) -> bool {
    lid_cmp(start, lid).is_le() && (lid_cmp(lid, end).is_le() || lid.starts_with(&format!("{end}.")))
}

fn touched_by(note: &PersonalNoteView, source: &MapSource) -> Option<&'static str> {
    let context = &note.value.context;
    let turn = context.turn_id.as_deref().or(note.value.based_on_turn.as_ref().map(|t| t.turn_id.as_str()));
    if turn == Some(source.turn_id.as_str()) { return Some("turn"); }
    // ADR-0169: an LID means nothing without its revision; only an identical publication pins it.
    let anchored = context.book_id.as_deref() == Some(source.source_id.as_str())
        && context.publication_id.is_some() && context.publication_id == source.publication_id
        && context.lid.as_deref().is_some_and(|lid| lid_within(lid, &source.start_lid, &source.end_lid));
    anchored.then_some("anchor")
}

pub(crate) fn touches(notes: &[PersonalNoteView], nodes: &[MapNode], removed: &BTreeSet<(String, String)>) -> Vec<Touch> {
    let mut found = BTreeMap::new();
    for note in notes {
        for node in nodes {
            if removed.contains(&(note.note_id.clone(), node.node_id.clone())) { continue; }
            let mut via: Vec<&'static str> = node.sources.iter().filter_map(|s| touched_by(note, s)).collect();
            via.sort_by_key(|v| *v != "turn");
            if let Some(via) = via.first() {
                found.insert((note.note_id.clone(), node.node_id.clone()), Touch {
                    note: NoteRef { note_id: note.note_id.clone(), revision: Some(note.current_revision) }, node_id: node.node_id.clone(), via });
            }
        }
    }
    found.into_values().collect()
}

/// Distance from the focus back through premises: 0 for the focus itself.
pub(crate) fn premise_distances(seeds: &BTreeSet<String>, chains: &[MapChain]) -> BTreeMap<String, u32> {
    let mut distance: BTreeMap<String, u32> = seeds.iter().map(|id| (id.clone(), 0)).collect();
    for hop in 1..=MAX_HOPS {
        let frontier: BTreeSet<String> = distance.iter().filter(|(_, d)| **d == hop - 1).map(|(id, _)| id.clone()).collect();
        for chain in chains.iter().filter(|c| frontier.contains(&c.conclusion)) {
            for premise in &chain.premises { distance.entry(premise.clone()).or_insert(hop); }
        }
    }
    distance
}

/// Notes ranked by how close the relationships they touch are to the focus.
pub(crate) fn rank(distances: &BTreeMap<String, u32>, touches: &[Touch]) -> Vec<NoteRef> {
    let mut best: BTreeMap<(&str, Option<u32>), u32> = BTreeMap::new();
    for touch in touches {
        if let Some(d) = distances.get(&touch.node_id) {
            best.entry((&touch.note.note_id, touch.note.revision)).and_modify(|b| *b = (*b).min(*d)).or_insert(*d);
        }
    }
    let mut ranked: Vec<((&str, Option<u32>), u32)> = best.into_iter().collect();
    ranked.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
    ranked.into_iter().map(|((note_id, revision), _)| NoteRef { note_id: note_id.into(), revision }).collect()
}

fn workspace(input: &Value) -> Option<&str> { input["learning_workspace_id"].as_str().filter(|id| !id.is_empty()) }

struct MapView { nodes: Vec<MapNode>, chains: Vec<MapChain>, touches: Vec<Touch> }

fn view(user: &user_runtime::UserRuntime, input: &Value) -> Result<Option<MapView>, ToolError> {
    let store = user.learning_store_for_workspace(workspace(input))?;
    if !store.learning_map_enabled()? { return Ok(None); }
    let (nodes, chains) = (store.map_nodes()?, store.map_chains()?);
    let notes = user.store.personal_notes(None, None, usize::MAX);
    let touches = touches(&notes, &nodes, &store.removed_note_attachments()?);
    Ok(Some(MapView { nodes, chains, touches }))
}

/// The third ranker. The focus is the relationships taught in `turn_id` when
/// given, else the few map statements most like the query.
pub(crate) fn structural(user: &user_runtime::UserRuntime, input: &Value) -> Result<(Vec<NoteRef>, Value), ToolError> {
    let Some(map) = view(user, input)? else { return Ok((vec![], json!({"status":"off"}))); };
    let seeds: BTreeSet<String> = match input["turn_id"].as_str() {
        Some(turn) => map.nodes.iter().filter(|n| n.sources.iter().any(|s| s.turn_id == turn)).map(|n| n.node_id.clone()).collect(),
        None => {
            let query = input["query"].as_str().unwrap_or_default();
            let mut similar: Vec<(f64, &MapNode)> = map.nodes.iter()
                .map(|n| (crate::learning_map_build::similarity(query, &n.statement), n)).filter(|(s, _)| *s >= 0.2).collect();
            similar.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.node_id.cmp(&b.1.node_id)));
            similar.into_iter().take(QUERY_SEEDS).map(|(_, n)| n.node_id.clone()).collect()
        }
    };
    let ranked = rank(&premise_distances(&seeds, &map.chains), &map.touches);
    Ok((ranked, json!({"status":"used","seeds":seeds.len(),"nodes":map.nodes.len()})))
}

/// The relationships one note touches, each with every cited passage (other books included).
pub(crate) fn note_relationships(user: &user_runtime::UserRuntime, input: &Value) -> Result<Value, ToolError> {
    let note_id = input["note_id"].as_str().ok_or_else(|| invalid("请选择想法"))?;
    let Some(map) = view(user, input)? else { return Ok(json!({"enabled":false,"relationships":[]})); };
    let relationships: Vec<Value> = map.touches.iter().filter(|t| t.note.note_id == note_id).filter_map(|touch| {
        let node = map.nodes.iter().find(|n| n.node_id == touch.node_id)?;
        Some(json!({"node_id":node.node_id,"statement":node.statement,"via":touch.via,"note":touch.note,
            "sources":node.sources.iter().map(|s| json!({"source_id":s.source_id,"source_revision":s.source_revision,
                "publication_id":s.publication_id,"start_lid":s.start_lid,"end_lid":s.end_lid,"quote":s.quote})).collect::<Vec<_>>()}))
    }).collect();
    Ok(json!({"enabled":true,"interpretation":"system","relationships":relationships}))
}

/// Two notes touching the two ends of one argument may be related; a suggestion
/// becomes a `NoteLink` only through the ordinary link action the user takes.
pub(crate) fn suggestions(user: &user_runtime::UserRuntime, input: &Value) -> Result<Value, ToolError> {
    let Some(map) = view(user, input)? else { return Ok(json!({"enabled":false,"suggestions":[]})); };
    let only = input["note_id"].as_str();
    let statement = |id: &str| map.nodes.iter().find(|n| n.node_id == id).map(|n| n.statement.clone()).unwrap_or_default();
    let touching = |id: &str| map.touches.iter().filter(|t| t.node_id == id).collect::<Vec<_>>();
    let mut seen = BTreeSet::new();
    let mut out = vec![];
    for chain in &map.chains {
        for premise in &chain.premises {
            for from in touching(premise) {
                for to in touching(&chain.conclusion) {
                    let pair = if from.note.note_id < to.note.note_id { (&from.note.note_id, &to.note.note_id) } else { (&to.note.note_id, &from.note.note_id) };
                    if from.note.note_id == to.note.note_id || only.is_some_and(|id| id != from.note.note_id && id != to.note.note_id)
                        || !seen.insert((pair.0.clone(), pair.1.clone()))
                        || user.store.note_links(&from.note.note_id).iter().any(|l| l.from.note_id == to.note.note_id || l.to.note_id == to.note.note_id) {
                        continue;
                    }
                    out.push(json!({"from":from.note,"to":to.note,"chain_id":chain.chain_id,
                        "premise":statement(premise),"conclusion":statement(&chain.conclusion)}));
                }
            }
        }
    }
    Ok(json!({"enabled":true,"suggestions":out}))
}

pub(crate) fn remove_touch(user: &user_runtime::UserRuntime, input: &Value) -> Result<Value, ToolError> {
    let (Some(note), Some(node)) = (input["note_id"].as_str(), input["node_id"].as_str()) else { return Err(invalid("请选择想法与关系")); };
    user.learning_store_for_workspace(workspace(input))?.remove_note_attachment(note, node)?;
    Ok(json!({"status":"removed"}))
}
