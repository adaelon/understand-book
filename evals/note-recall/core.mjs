// M0 note-recall scoring (ADR-0178 §8). Pure functions over search hits that
// already carry each ranker's own rank, so one fused search measures all rankers.

export const RANKERS = ['lexical', 'semantic', 'fused', 'structural'];

/** Validate a hand-labelled case file without reading any search result. */
export function validateCases(data) {
  if (!data || !Array.isArray(data.cases) || data.cases.length === 0) throw new Error('cases must be a non-empty array');
  const ids = new Set();
  for (const item of data.cases) {
    if (typeof item.id !== 'string' || !item.id.trim() || ids.has(item.id)) throw new Error(`case id missing or duplicated: ${item.id}`);
    ids.add(item.id);
    if (typeof item.query !== 'string' || !item.query.trim()) throw new Error(`case ${item.id}: query is required`);
    if (!Array.isArray(item.relevant) || item.relevant.some(id => typeof id !== 'string' || !id)) throw new Error(`case ${item.id}: relevant must list note ids`);
  }
  return data.cases;
}

/** The rank each ranker gave a note, or null when it did not return it. */
export function ranksFor(hits, noteId) {
  const at = hits.findIndex(hit => hit.reference?.note_id === noteId);
  const hit = at >= 0 ? hits[at] : null;
  return {
    lexical: hit?.lexical_rank ?? null,
    semantic: hit?.semantic_rank ?? null,
    fused: at >= 0 ? at + 1 : null,
    structural: hit?.structural_rank ?? null,
  };
}

/** Recall@k per ranker, plus the relevant notes that both baseline rankers missed. */
export function scoreCase(item, hits, ks = [5, 10]) {
  const ranks = item.relevant.map(id => ({ id, ...ranksFor(hits, id) }));
  const recall = {};
  for (const ranker of RANKERS) {
    recall[ranker] = Object.fromEntries(ks.map(k => [k, item.relevant.length
      ? ranks.filter(r => r[ranker] !== null && r[ranker] <= k).length / item.relevant.length : null]));
  }
  const k = Math.max(...ks);
  const missedByBaselines = ranks.filter(r => !(r.lexical !== null && r.lexical <= k) && !(r.semantic !== null && r.semantic <= k)).map(r => r.id);
  return { id: item.id, relevant: item.relevant.length, recall, missed_by_baselines: missedByBaselines,
    structural_found: ranks.filter(r => missedByBaselines.includes(r.id) && r.structural !== null && r.structural <= k).map(r => r.id) };
}

/**
 * M2: the same query with and without structural recall. New recall is a
 * relevant note that enters the top k only with the third ranker; false recall
 * is an irrelevant note that does. Both are counted, never netted.
 */
export function compareFusion(item, twoWay, threeWay, k = 10) {
  const top = hits => new Set(hits.slice(0, k).map(hit => hit.reference?.note_id));
  const [before, after] = [top(twoWay), top(threeWay)];
  const entered = [...after].filter(id => !before.has(id));
  const relevant = new Set(item.relevant);
  return { k, new_recall: entered.filter(id => relevant.has(id)), false_recall: entered.filter(id => !relevant.has(id)),
    displaced_relevant: [...before].filter(id => relevant.has(id) && !after.has(id)) };
}

/** Macro-averaged recall over cases with at least one relevant note; the M0 target set. */
export function aggregate(results, ks = [5, 10]) {
  const scored = results.filter(r => r.relevant > 0);
  const mean = (ranker, k) => scored.length ? scored.reduce((sum, r) => sum + r.recall[ranker][k], 0) / scored.length : null;
  const targets = results.flatMap(r => r.missed_by_baselines.map(id => ({ case: r.id, note_id: id })));
  const found = results.flatMap(r => r.structural_found.map(id => ({ case: r.id, note_id: id })));
  const fusion = results.filter(r => r.fusion);
  const sum = key => fusion.reduce((total, r) => total + r.fusion[key].length, 0);
  return {
    cases: results.length, scored_cases: scored.length,
    recall: Object.fromEntries(RANKERS.map(ranker => [ranker, Object.fromEntries(ks.map(k => [k, mean(ranker, k)]))])),
    target_set: targets, structural_new_hits: found,
    ...(fusion.length ? { fusion: { cases: fusion.length, new_recall: sum('new_recall'), false_recall: sum('false_recall'),
      displaced_relevant: sum('displaced_relevant') } } : {}),
    // ADR-0178: M1–M3 proceed only if the baselines leave relevant notes unfound.
    gate: targets.length === 0 ? 'stop: both baselines already find every relevant note' : 'continue: baselines miss relevant notes',
    // M3 proceeds only if the third ranker finds what both baselines missed.
    ...(fusion.length ? { m3_gate: found.length > 0 ? 'continue: structural recall finds notes both baselines miss'
      : 'stop: structural recall adds nothing over the baselines' } : {}),
  };
}
