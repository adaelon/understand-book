// Usage: node evals/note-recall/run.mjs <cases.json> <report.json> [server] [--structural]
// Runs each labelled query through the local server's note search and writes the
// report to a path you choose. Keep both files outside the repository: they hold
// your private notes' identities.
import { readFile, writeFile } from 'node:fs/promises';
import { aggregate, compareFusion, scoreCase, validateCases } from './core.mjs';

const [casesPath, reportPath, server = 'http://127.0.0.1:8787'] = process.argv.slice(2).filter(arg => !arg.startsWith('--'));
if (!casesPath || !reportPath) { console.error('usage: node run.mjs <cases.json> <report.json> [server]'); process.exit(2); }
const cases = validateCases(JSON.parse(await readFile(casesPath, 'utf8')));
// --structural also runs every query with learning-map recall (M2); the map must be built and enabled.
const structural = process.argv.includes('--structural');
async function search(item, extra = {}) {
  const response = await fetch(`${server}/api/me/notes/search`, { method: 'POST', headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ query: item.query, limit: 20, ...(item.book_id ? { book_id: item.book_id } : {}), ...extra }) });
  if (!response.ok) throw new Error(`case ${item.id}: search failed with ${response.status}: ${await response.text()}`);
  const body = await response.json();
  if (body.semantic?.status && body.semantic.status !== 'ready') console.warn(`case ${item.id}: semantic ranker ${body.semantic.status} (${body.semantic.reason ?? ''})`);
  return body;
}
const results = [];
for (const item of cases) {
  const twoWay = (await search(item)).hits ?? [];
  if (!structural) { results.push(scoreCase(item, twoWay)); continue; }
  const body = await search(item, { structural: true, ...(item.turn_id ? { turn_id: item.turn_id } : {}),
    ...(item.learning_workspace_id ? { learning_workspace_id: item.learning_workspace_id } : {}) });
  if (body.structural?.status !== 'used') throw new Error(`case ${item.id}: learning map is ${body.structural?.status ?? 'unavailable'}`);
  results.push({ ...scoreCase(item, body.hits ?? []), fusion: compareFusion(item, twoWay, body.hits ?? []) });
}
const report = { generated_at: new Date().toISOString(), server, ...aggregate(results), results };
await writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`);
console.log(JSON.stringify({ cases: report.cases, recall: report.recall, target_set: report.target_set.length, gate: report.gate,
  fusion: report.fusion, m3_gate: report.m3_gate }, null, 2));
