// Exact/lexical and fusion prototypes follow the accepted SR3 policy, for this experiment only.
const lower = value => value.toLocaleLowerCase();
const byKey = (a, b) => a.key < b.key ? -1 : a.key > b.key ? 1 : 0;
export function lexicalLanes(records, query, focus) {
  const terms = lower(query).split(/\s+/u).filter(Boolean);
  const valid = records.filter(r => r.key !== focus).sort(byKey);
  const exact = valid.filter(r => [r.meaning, ...r.aliases].some(s => lower(s) === lower(query))).map(r => r.key);
  const lexical = valid.filter(r => !exact.includes(r.key) && terms.some(t => r.lexical_fields.some(s => lower(s).includes(t)))).map(r => r.key);
  return { exact, lexical };
}
export function fuse({ exact, lexical }, semantic, limit = 12) {
  const result = [...exact], seen = new Set(result);
  let l = 0, s = 0, added = 0;
  while (l < lexical.length || (s < semantic.length && added < limit)) {
    while (l < lexical.length && seen.has(lexical[l])) l++;
    if (l < lexical.length) { const key = lexical[l++]; result.push(key); seen.add(key); }
    while (s < semantic.length && seen.has(semantic[s])) s++;
    if (s < semantic.length && added < limit) { const key = semantic[s++]; result.push(key); seen.add(key); added++; }
  }
  return result;
}
export function metrics(ranking, relevant, k) {
  const page = ranking.slice(0, k);
  const hits = relevant.filter(key => page.includes(key)).length;
  return { hits, relevant: relevant.length, returned: page.length, recall: hits / relevant.length,
    precision_at_k: hits / k, precision_returned: page.length ? hits / page.length : 0 };
}
