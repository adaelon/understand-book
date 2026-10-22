//! Workspace retrieval locates source; only subsequent text reads admit evidence.
use crate::{published_library::PublishedLibrary, semantic_retrieval::{self, SemanticPublication, SemanticSearchRequest}};
use base_schema::{PublishedBookRef, Span};
use read_tools::ToolError;
use runtime::library::LibraryReadResult;
use serde::Deserialize;
use serde_json::{json, Value};
use std::{collections::BTreeMap, sync::Mutex};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    query: String,
    limit: Option<usize>,
    published_book_refs: Option<Vec<PublishedBookRef>>,
}

#[derive(Clone)]
struct Candidate {
    reference: PublishedBookRef,
    span: Span,
    score: f64,
    lexical_rank: Option<usize>,
    semantic_rank: Option<usize>,
}

fn overlaps(a: &Candidate, b: &Candidate) -> bool {
    a.reference == b.reference && a.span.start < b.span.end && b.span.start < a.span.end
}

/// RRF weights rankings rather than comparing BM25 with cosine similarity.
fn fuse(lexical: Vec<Candidate>, semantic: Vec<Candidate>) -> Vec<Candidate> {
    let mut candidates: Vec<Candidate> = vec![];
    for (is_lexical, list) in [(true, lexical), (false, semantic)] {
        for (rank, mut candidate) in list.into_iter().enumerate() {
            let rank = rank + 1;
            if is_lexical { candidate.lexical_rank = Some(rank); }
            else { candidate.semantic_rank = Some(rank); }
            // A semantic chunk can bridge two lexical windows. Fold every
            // overlapping window so that the final result does not duplicate it.
            while let Some(index) = candidates.iter().position(|existing| overlaps(existing, &candidate)) {
                let existing = candidates.remove(index);
                candidate.span.start = existing.span.start.min(candidate.span.start);
                candidate.span.end = existing.span.end.max(candidate.span.end);
                candidate.lexical_rank = candidate.lexical_rank.into_iter().chain(existing.lexical_rank).min();
                candidate.semantic_rank = candidate.semantic_rank.into_iter().chain(existing.semantic_rank).min();
            }
            candidates.push(candidate);
        }
    }
    for candidate in &mut candidates {
        candidate.score = candidate.lexical_rank.into_iter().chain(candidate.semantic_rank)
            .map(|rank| 1.0 / (60.0 + rank as f64)).sum();
    }
    candidates.sort_by(|a,b| b.score.total_cmp(&a.score).then_with(|| a.reference.cmp(&b.reference)).then(a.span.start.cmp(&b.span.start)));
    candidates
}

pub(crate) fn search(library: &Mutex<PublishedLibrary>, owner: &str, members: Option<&[PublishedBookRef]>, arguments: Value) -> Result<LibraryReadResult, ToolError> {
    search_with(library, owner, members, arguments, semantic_retrieval::search)
}

pub(crate) fn search_with(library: &Mutex<PublishedLibrary>, owner: &str, members: Option<&[PublishedBookRef]>, arguments: Value,
    retrieve: impl FnOnce(&SemanticSearchRequest) -> Result<semantic_retrieval::SemanticSearchResult, String>) -> Result<LibraryReadResult, ToolError> {
    let members = members.ok_or_else(|| crate::user_storage_paths::error("WORKSPACE_REQUIRED", "validation", "library.search requires a learning workspace. For the current article use book.search or read the selected passage directly."))?;
    let input: Request = serde_json::from_value(arguments).map_err(|e| memory::teaching::invalid(e.to_string()))?;
    let limit = input.limit.unwrap_or(8);
    if input.query.trim().is_empty() || input.query.chars().count() > 4096 || !(1..=20).contains(&limit) {
        return Err(memory::teaching::invalid("query must contain 1–4096 characters and limit must be 1–20"));
    }
    let selected = input.published_book_refs.as_deref().unwrap_or(members);
    if input.published_book_refs.as_ref().is_some_and(Vec::is_empty) {
        return Err(memory::teaching::invalid("published_book_refs must not be empty"));
    }
    for reference in selected { crate::cross_book::check_member(Some(members), reference)?; }
    let (publications, cache_dir) = {
        let library = library.lock().unwrap();
        (library.list_manifests_page(owner, Some(selected), None, selected.len())?, library.retrieval_cache_directory())
    };
    let mut lexical = vec![];
    let mut requests = vec![];
    let mut titles = BTreeMap::new();
    for (manifest, _, title) in &publications {
        // Release the registry lock before ranking, source I/O or embedding work.
        // Books are loaded one at a time, so a large workspace does not pin them all.
        let publication = library.lock().unwrap().load(owner, &manifest.reference)?;
        let ranked = publication.book.search_ranked(&input.query, None, 20)?;
        lexical.extend(ranked.hits.into_iter().map(|hit| Candidate {
            reference: manifest.reference.clone(), span: hit.source_range_utf16, score: hit.score,
            lexical_rank: None, semantic_rank: None,
        }));
        requests.push(SemanticPublication { book_id: manifest.reference.book_id.clone(), publication_id: manifest.reference.publication_id.clone(),
            directory: publication.directory().to_path_buf(), source_revision: publication.book.source_fingerprint().into() });
        titles.insert(manifest.reference.clone(), title.clone());
    }
    lexical.sort_by(|a,b| b.score.total_cmp(&a.score).then_with(||a.reference.cmp(&b.reference)).then(a.span.start.cmp(&b.span.start)));
    let (semantic, semantic_status) = if requests.is_empty() {
        (vec![], json!({"status":"unavailable","reason":"No currently authorized workspace materials"}))
    } else {
        match retrieve(&SemanticSearchRequest { query: input.query.clone(), top_k: limit * 4, cache_dir, publications: requests }) {
            Ok(result) => {
                let status = json!({"status":result.status,"reason":result.reason,"indexed_publications":result.indexed_publications,
                    "embedded_chunks":result.embedded_chunks,"cache_hits":result.cache_hits,"query_tokens":result.query_tokens,"document_tokens":result.document_tokens});
                let hits = result.hits.into_iter().map(|hit| Candidate {
                    reference: PublishedBookRef { book_id: hit.book_id, publication_id: hit.publication_id },
                    span: Span { start: hit.start_utf16 as usize, end: hit.end_utf16 as usize }, score: hit.score,
                    lexical_rank: None, semantic_rank: None,
                }).collect();
                (hits, status)
            }
            Err(reason) => (vec![], json!({"status":"unavailable","reason":reason})),
        }
    };
    let candidates = fuse(lexical, semantic);
    // Authorization precedes final top-k as well as initial retrieval. An expired
    // grant never becomes a returned hit or a source preview.
    let allowed = {
        let library = library.lock().unwrap();
        titles.keys().filter(|reference| library.authorize(owner, reference).is_ok()).cloned().collect::<std::collections::BTreeSet<_>>()
    };
    let mut hits = vec![];
    for candidate in candidates.into_iter().filter(|hit| allowed.contains(&hit.reference)).take(limit) {
        let publication = library.lock().unwrap().load(owner, &candidate.reference)?;
        let book = &publication.book;
        let mut leaves: Vec<_> = book.base.lid_nodes.iter().filter(|node| node.children.is_empty()
            && node.span.start < candidate.span.end && candidate.span.start < node.span.end).collect();
        leaves.sort_by_key(|node| node.span.start);
        let (Some(first), Some(last)) = (leaves.first(), leaves.last()) else { continue; };
        let excerpt = book.text_in_source_range(&candidate.span)?.chars().take(360).collect::<String>();
        let ranges: Vec<_> = leaves.iter().map(|node| json!({"lid":node.lid,
            "start_utf16":candidate.span.start.max(node.span.start)-node.span.start,
            "end_utf16":candidate.span.end.min(node.span.end)-node.span.start})).collect();
        let call = json!({"name":"library.read","arguments":{"published_book_ref":candidate.reference,
            "operation":"text","lid":first.lid,"end_lid":last.lid,"start_utf16":candidate.span.start}});
        hits.push(json!({"published_book_ref":candidate.reference,"source_revision":book.source_fingerprint(),
            "title":titles[&candidate.reference],"rank":hits.len()+1,"score":candidate.score,
            "start_lid":first.lid,"end_lid":last.lid,"source_range_utf16":candidate.span,"ranges":ranges,"excerpt":excerpt,
            "lexical_rank":candidate.lexical_rank,"semantic_rank":candidate.semantic_rank,"next_calls":[call]}));
    }
    let next_calls: Vec<_> = hits.iter().flat_map(|hit| hit["next_calls"].as_array().unwrap().iter().cloned()).collect();
    Ok(LibraryReadResult { source: None, body: json!({"version":"workspace_search.v1","query":input.query,
        "scope":{"kind":"frozen_workspace_members","searched_publications":allowed.len()},
        "retrieval":{"lexical":"bm25","semantic":semantic_status},"exhaustive":false,
        "notice":"Ranked candidates are not exhaustive coverage or observed evidence. Read relevant text with next_calls before explaining or citing.",
        "hits":hits,"next_calls":next_calls}) })
}
