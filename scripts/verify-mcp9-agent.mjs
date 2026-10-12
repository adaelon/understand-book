// Deterministic acceptance: protocol results, exact UTF-16 evidence and SQLite facts.
import assert from 'node:assert/strict';
import { readFile, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';

const dir = resolve(process.argv[2]);
const evidence = JSON.parse(await readFile(resolve(dir, 'evidence.json'), 'utf8'));
async function transcript(name) {
  const rows = (await readFile(resolve(dir, `${name}.jsonl`), 'utf8')).trim().split(/\r?\n/).map(JSON.parse);
  if (rows.some(r => r.type === 'thread.started')) {
    const metadata = JSON.parse(await readFile(resolve(dir, 'client.json'), 'utf8'));
    assert(rows.some(r => r.type === 'turn.completed'), `${name}: incomplete Codex turn`);
    assert(!rows.some(r => r.type === 'turn.failed'), `${name}: Codex failure`);
    const items = rows.filter(r => r.type === 'item.completed').map(r => r.item);
    const tools = items.filter(i => i.type === 'mcp_tool_call');
    assert(tools.every(i => i.status === 'completed' && !i.error), `${name}: failed MCP call`);
    return {
      init: { client: metadata.client, model: metadata.model },
      final: { result: items.findLast(i => i.type === 'agent_message').text },
      calls: tools.map(i => ({ name: `mcp__${i.server}__${i.tool}`, input: i.arguments, result: projection(i.result) })),
    };
  }
  const init = rows.find(r => r.type === 'system' && r.subtype === 'init');
  const final = rows.findLast(r => r.type === 'result');
  assert.equal(final.is_error, false, `${name}: client failure`);
  assert.deepEqual(final.permission_denials, [], `${name}: denied tools`);
  const calls = new Map();
  for (const row of rows) for (const block of row.message?.content ?? []) {
    if (block.type === 'tool_use') calls.set(block.id, { name: block.name, input: block.input });
    if (block.type === 'tool_result') {
      const call = calls.get(block.tool_use_id);
      if (call) call.result = projection(block.content);
    }
  }
  return { init, final, calls: [...calls.values()] };
}
function projection(content) {
  if (typeof content === 'string') {
    try { return projection(JSON.parse(content)); } catch { return null; }
  }
  if (Array.isArray(content)) return content.map(projection).find(Boolean);
  if (!content || typeof content !== 'object') return null;
  if (content.structuredContent) return content.structuredContent;
  if (content.structured_content) return content.structured_content;
  if (content.usage || content.error_code) return content;
  return projection(content.text ?? content.content ?? content.result);
}
const reader = await transcript('reader');
const exhausted = await transcript('exhausted');
assert(reader.calls.some(c => c.name === 'mcp__beta__book_list'), 'Agent must discover its materials');
assert(reader.calls.some(c => c.name === 'mcp__beta__book_search_text'), 'Agent must locate evidence');
const pages = reader.calls.filter(c => c.name === 'mcp__beta__book_text' && c.result?.data?.text).map(c => c.result);
assert(pages.length >= 2, 'Reading must include continuation beyond the first 8000 characters');
const sourceUrls = new Map();
for (const page of pages) {
  assert.deepEqual(page.published_book_ref, evidence.reference);
  const { start, end } = page.data.source_range;
  assert.equal(page.data.text, evidence.source_text.slice(start, end));
  for (const source of page.sources) {
    const query = new URL(source.source_url).searchParams;
    assert.equal(query.get('book_id'), evidence.reference.book_id);
    assert.equal(query.get('publication_id'), evidence.reference.publication_id);
    assert.equal(Number(query.get('start_utf16')), start);
    assert.equal(Number(query.get('end_utf16')), end);
    sourceUrls.set(source.source_url, page.data.text);
  }
}
const answer = reader.final.result;
assert.match(answer, /18/);
assert.match(answer, /37/);
assert.match(answer, /19/);
assert.match(answer, /南池[^\n]*(没有|未|不)/);
const links = [...answer.matchAll(/https:\/\/reader\.example\/\?[^\s)\]<>"）]+/g)].map(m => m[0]);
assert(links.length > 0, 'Answer needs source links');
for (const link of links) assert(sourceUrls.has(link), `Citation was not read: ${link}`);
const cited = links.map(link => sourceUrls.get(link)).join('\n');
for (const quote of ['北池起始有 37 升水', '北池有 19 升水', '南池没有参与本次实验']) assert(cited.includes(quote), `Cited source omits ${quote}`);
const charged = reader.calls.filter(c => c.result?.usage?.charged_units === 1);
const counts = values => values.reduce((result, name) => ({ ...result, [name]: (result[name] ?? 0) + 1 }), {});
assert.deepEqual(counts(charged.map(c => c.name.replace('mcp__beta__', ''))), counts(evidence.calls.map(c => c.tool)));
assert(evidence.calls.every(c => c.user_id === 'alice' && c.units === 1 && c.debit_source === 'free_month'));
assert.equal(evidence.usage.free_remaining, evidence.gift_units - charged.length);
assert.equal(evidence.usage.paid_remaining, 0);
const stopped = exhausted.calls.filter(c => c.result?.error_code === 'quota_exhausted');
const zero = exhausted.calls.find(c => c.result?.usage?.free_remaining === 0 && c.result?.usage?.paid_remaining === 0);
assert(zero || stopped.length, 'Exhausted Agent must observe quota state');
assert(stopped.length <= 1, 'Agent must stop after quota_exhausted');
assert(!exhausted.calls.some(c => c.result?.usage?.charged_units === 1));
assert(exhausted.final.result.includes('https://reader.example/?account=mcp'), 'Explain purchase recovery');
assert.match(exhausted.final.result, /恢复|重置/);
const summary = {
  client: reader.init.client ?? `Claude Code ${reader.init.claude_code_version}`, model: reader.init.model,
  protocol: evidence.protocol, reader_tools: reader.calls.map(c => c.name),
  exhausted_tools: exhausted.calls.map(c => c.name), charged_calls: charged.length,
  gifted_units: evidence.gift_units, free_remaining: evidence.usage.free_remaining,
  cited_urls: links, reader_answer: answer, exhausted_answer: exhausted.final.result,
};
await writeFile(resolve(dir, 'verification.json'), JSON.stringify(summary, null, 2));
console.log(JSON.stringify({ client: summary.client, protocol: summary.protocol, charged_calls: charged.length, cited_urls: links.length, result: 'passed' }));
