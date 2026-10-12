import { readFile, readdir, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { isDeepStrictEqual } from 'node:util';

export function projectResult(value) {
  if (typeof value === 'string') { try { return projectResult(JSON.parse(value)); } catch { return null; } }
  if (Array.isArray(value)) return value.map(projectResult).find(Boolean) ?? null;
  if (!value || typeof value !== 'object') return null;
  if (value.structuredContent) return value.structuredContent;
  if (value.structured_content) return value.structured_content;
  if (value.usage || value.error_code) return value;
  return projectResult(value.text ?? value.content ?? value.result);
}

export function sumUsage(values) {
  const known = values.filter(v => v?.usage);
  const summed = known.reduce((s, v) => ({ input_tokens: s.input_tokens + v.usage.input_tokens,
    cached_input_tokens: s.cached_input_tokens + (v.usage.cached_input_tokens ?? 0),
    output_tokens: s.output_tokens + v.usage.output_tokens }), { input_tokens: 0, cached_input_tokens: 0, output_tokens: 0 });
  return { ...summed, total_tokens: summed.input_tokens + summed.output_tokens, complete: values.length > 0 && known.length === values.length,
    missing_samples: values.length - known.length, actual_cost: null };
}

export function judgeRainwater({ rows, metrics, evidence }) {
  const tools = rows.filter(r => r.type === 'item.completed' && r.item?.type === 'mcp_tool_call').map(r => r.item);
  const calls = tools.map(t => ({ name: t.tool, input: t.arguments, result: projectResult(t.result), error: t.error }));
  const answer = metrics.final ?? '';
  const plainAnswer = answer.replaceAll('**', '');
  const pages = calls.filter(c => c.name === 'book_text' && c.result?.data?.text).map(c => c.result);
  const known = new Map(pages.flatMap(p => (p.sources ?? []).map(s => [s.source_url, p.data.text])));
  const links = [...answer.matchAll(/https:\/\/reader\.example\/\?[^\s)\]<>"）]+/g)].map(m => m[0]);
  const requirements = {
    completed: metrics.completed,
    discovered: calls.some(c => c.name === 'book_list'),
    located: calls.some(c => c.name === 'book_search_text'),
    numeric_answer: /37/.test(answer) && /19/.test(answer)
      && /(?:减少|下降|少了)(?:了|水量)?\s*[：:]?\s*18\s*升/.test(plainAnswer),
    south_excluded: /南池[\s：:，,]*(?:没有|未|不曾)参与/.test(plainAnswer),
    exact_pages: pages.length >= 2 && pages.every(p => isDeepStrictEqual(p.published_book_ref, evidence.reference)
      && p.data.text === evidence.source_text.slice(p.data.source_range.start, p.data.source_range.end)
      && p.sources.every(s => {
        const query = new URL(s.source_url).searchParams;
        return query.get('book_id') === evidence.reference.book_id && query.get('publication_id') === evidence.reference.publication_id
          && Number(query.get('start_utf16')) === p.data.source_range.start && Number(query.get('end_utf16')) === p.data.source_range.end;
      })),
    citations_read: links.length > 0 && links.every(link => known.has(link)),
    cited_facts: ['北池起始有 37 升水', '北池有 19 升水', '南池没有参与本次实验']
      .every(q => links.map(link => known.get(link) ?? '').join('\n').includes(q)),
    ledger: calls.filter(c => c.result?.usage?.charged_units === 1).length === evidence.calls.length
      && evidence.calls.every(c => c.user_id === 'alice' && c.units === 1 && c.debit_source === 'free_month')
      && evidence.usage.free_remaining === evidence.gift_units - evidence.calls.length,
  };
  return { verdict: Object.values(requirements).every(Boolean) ? 'pass' : 'fail', requirements, calls,
    tool_rejections: calls.filter(c => c.error || c.result?.error_code).length, tool_count: calls.length };
}

export function judgeResident(result, requests, metrics = [], input = null) {
  const reply = result.reply;
  const tools = requests.flatMap(r => r.input ?? []).filter(m => m.role === 'Tool');
  const historical = new Set((requests[0]?.input ?? []).filter(m => m.role === 'Tool').map(m => m.tool_call_id));
  const unique = [...new Map(tools.filter(m => !historical.has(m.tool_call_id)).map(m => [m.tool_call_id, m])).values()];
  const results = unique.map(m => { try { return JSON.parse(m.content); } catch { return {}; } });
  const namesById = new Map(requests.flatMap(r => r.input ?? []).flatMap(m => m.tool_calls ?? []).map(c => [c.id, c.name]));
  const toolNames = unique.map((m, i) => namesById.get(m.tool_call_id) ?? results[i].receipt?.tool ?? m.name);
  const records = result.reopened_records ?? [];
  const requirements = { completed: result.http_status === 200 && reply.incomplete === false && !!reply.answer };
  if (result.case === 'save-note') {
    requirements.note_persisted = records.some(r => r.type === 'note' && r.content === '平均速度＝总路程／总时间'
      && r.anchor?.lid === '1.1' && r.note_placement?.lid === '1.1');
  }
  if (result.case === 'explain-only') {
    requirements.read_only = records.length === 0 && (reply.effects ?? []).length === 0
      && !toolNames.some(name => ['reader.note', 'reader.highlight', 'memory.save'].includes(name));
    requirements.source_delivered = (reply.answer_view?.sources ?? []).length > 0;
    const quote = input?.input?.question_quote;
    const presented = results.filter((r, i) => toolNames[i] === 'source.present' && r.status === 'ok').map(r => r.model_body);
    requirements.selected_source = !!quote && quote.ranges.every(r => input.source.slice(r.range.start, r.range.end) === quote.resolved_quote)
      && (reply.answer_view?.sources ?? []).every(s => presented.some(p => p.source_ref_id === s.source_ref_id && p.preview === quote.resolved_quote));
  }
  if (result.case === 'tutor-continue') {
    requirements.teaching_delivered = result.deliveries.length > 0;
    requirements.move_saved = result.events.some(e => e.kind === 'move_selected');
  }
  const discardedCompletedTurn = reply.error_code === 'PROVIDER_ERROR' && reply.message?.includes('CODEX_SAMPLE_FAILED')
    && metrics.some(m => !m.completed && m.exit_code === 0 && !m.timed_out && m.final !== null && m.total_tokens !== null);
  return { verdict: discardedCompletedTurn ? 'unresolved' : Object.values(requirements).every(Boolean) ? 'pass' : 'fail', requirements,
    assessment_status: discardedCompletedTurn ? 'invalid_harness' : 'valid',
    diagnostic: discardedCompletedTurn ? 'discarded_completed_client_turn' : null,
    tool_rejections: results.filter(r => r.status === 'error' || r.error_code).length,
    blocked_reasons: results.flatMap(r => r.model_body?.blocked ?? []).map(b => b.reason),
    tool_names: toolNames, tool_count: unique.length,
    semantic_assessment: result.case === 'save-note' ? 'not_applicable' : 'needs_review' };
}

const json = async path => JSON.parse(await readFile(path, 'utf8'));
export function aggregate(records) {
  return { runs: records, count: records.length, pass: records.filter(r => r.verdict === 'pass').length,
    fail: records.filter(r => r.verdict === 'fail').length, unresolved: records.filter(r => r.verdict === 'unresolved').length };
}
export async function summarize(directory) {
  const records = [];
  const schedule = await json(resolve(directory, '../schedule.json'));
  for (const entry of schedule.planned) {
    const dir = resolve(directory, entry.name);
    try {
    if (entry.name.includes('rainwater')) {
      const metrics = await json(resolve(dir, 'agent/metrics.json'));
      const rows = (await readFile(resolve(dir, 'agent/events.jsonl'), 'utf8')).trim().split(/\r?\n/).map(JSON.parse);
      const evidence = await json(resolve(dir, 'evidence.json'));
      records.push({ run: entry.name, case: 'rainwater', ...judgeRainwater({ rows, metrics, evidence }),
        usage: sumUsage([metrics]), elapsed_ms: metrics.elapsed_ms, active_inputs: null });
    } else {
      let result;
      try { result = await json(resolve(dir, 'result.json')); } catch (e) {
        if (e.code === 'ENOENT') { records.push({ run: entry.name, verdict: 'unresolved', reason: 'missing_result' }); continue; } throw e;
      }
      const names = await readdir(dir);
      const requests = await Promise.all(names.filter(n => /^request-\d+.json$/.test(n)).sort().map(n => json(resolve(dir, n))));
      const metrics = await Promise.all(names.filter(n => /^sample-\d+$/.test(n)).sort().map(n => json(resolve(dir, n, 'metrics.json'))));
      const input = await json(resolve(dir, 'input.json'));
      records.push({ run: entry.name, case: result.case, ...judgeResident(result, requests, metrics, input), usage: sumUsage(metrics),
        elapsed_ms: result.elapsed_ms, active_inputs: requests.map(r => r.active_context ?? null),
        definition_bytes: requests.map(r => Buffer.byteLength(JSON.stringify(r.tools ?? []))) });
    }
    } catch (error) {
      if (error.code !== 'ENOENT') throw error;
      records.push({ run: entry.name, case: entry.task, verdict: 'unresolved', reason: 'missing_run_evidence' });
    }
  }
  const report = aggregate(records);
  await writeFile(resolve(directory, '../summary.json'), JSON.stringify(report, null, 2));
  return report;
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const report = await summarize(resolve(process.argv[2]));
  console.log(JSON.stringify({ count: report.count, pass: report.pass, fail: report.fail, unresolved: report.unresolved }));
}
