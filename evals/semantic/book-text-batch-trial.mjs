// A small product trial: identical known-location questions, isolated local Readers,
// provider usage from durable activities, and no paid grading or transport recorder.
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { tasks, version as datasetVersion } from './dataset.mjs';
import { allSpans, loadCorpus } from './core.mjs';
import { citationText } from './agent-core.mjs';
import { startServer } from './server.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const args = process.argv.slice(2);
const option = (key, fallback) => { const index = args.indexOf(key); return index < 0 ? fallback : args[index + 1]; };
const variant = option('--variant');
const bookDir = path.join(root, '.understand-book/quantification-essence');
const corpus = loadCorpus(bookDir);
const selectedIds = ['cross-01', 'cross-02', 'exact-02'];
const trialTasks = selectedIds.map(id => {
  const task = tasks.find(task => task.id === id);
  const spans = task.slots.flatMap(slot => slot.evidence.flatMap(group => {
    const matches = group.flatMap(anchor => allSpans(corpus.source, anchor));
    if (!matches.length) throw new Error(`Missing source anchor for ${id}`);
    return matches;
  }));
  const lids = corpus.leaves.filter(leaf => spans.some(span => leaf.start < span.end && leaf.end > span.start)).map(leaf => leaf.lid);
  if (!lids.length) throw new Error(`No source locations for ${id}`);
  return { id, question: task.question, lids,
    // Explicit user LID literals are a supported TurnLocatorLedger origin.
    // The answerer gets locations only, never source text or gold answer labels.
    message: `${task.question}\n\n已知相关原文位置：${lids.map(lid => `LID ${lid}`).join('；')}。请先读取所有这些位置的原文，再基于原文回答，并引用支持结论的来源。`,
    expected_facts: task.slots.map(({ key, description, accept }) => ({ key, description, accept })) };
});

if (args.includes('--validate')) {
  console.log(JSON.stringify({ dataset: datasetVersion, cases: trialTasks.map(({ id, lids, message }) => ({ id, lids, message })) }, null, 2));
  process.exit(0);
}
if (!['baseline', 'candidate'].includes(variant)) throw new Error('Use --variant baseline|candidate');
const binary = path.resolve(option('--binary', path.join(root, `tmp/book-text-batch-trial-20261014/${variant}-server.exe`)));
const output = path.resolve(option('--out', path.join(root, `docs/performance/book-text-batch-20261014/results/${variant}`)));

const usageKeys = ['input_tokens', 'output_tokens', 'reasoning_output_tokens', 'cached_input_tokens', 'cache_creation_input_tokens', 'total_tokens'];
function sumKnown(rows, get) {
  const values = rows.map(get);
  return values.length && values.every(Number.isFinite) ? values.reduce((sum, value) => sum + value, 0) : null;
}
function usageOf(models) {
  return Object.fromEntries(usageKeys.map(key => [key, sumKnown(models, model => model.usage?.[key])]));
}
function evidenceCovers(ref, lid) {
  const [start, end = start] = ref.split('..');
  const a = corpus.base.lid_nodes.find(node => node.lid === start);
  const b = corpus.base.lid_nodes.find(node => node.lid === end);
  const node = corpus.base.lid_nodes.find(node => node.lid === lid);
  return !!(a && b && node && a.span.start <= node.span.start && b.span.end >= node.span.end);
}
function statistics(task, outcome, history, elapsed, sources) {
  const activities = history?.current?.turns?.at(-1)?.run_summary?.activities ?? [];
  const models = activities.filter(activity => activity.kind === 'model');
  const tools = activities.filter(activity => activity.kind === 'tool');
  const evidenceRefs = [...new Set(tools.flatMap(tool => tool.evidence_refs ?? []))];
  return {
    elapsed_ms: elapsed,
    outcome_turns: outcome?.turns ?? null,
    incomplete: outcome?.incomplete ?? null,
    model_activity_count: models.length,
    model_request_diagnostics_count: models.filter(model => model.request_diagnostics).length,
    model_activities_without_usage: models.filter(model => !model.usage).length,
    usage: usageOf(models),
    // Known counts are a lower bound when an activity omitted a field, never a substitute total.
    reported_usage_lower_bound: Object.fromEntries(usageKeys.map(key => [key,
      models.some(model => Number.isFinite(model.usage?.[key]))
        ? models.reduce((sum, model) => sum + (model.usage?.[key] ?? 0), 0) : null])),
    model_calls: models.map(({ step_id, parent_step_id, name, status, duration_ms, model_name, usage, request_diagnostics }) =>
      ({ step_id, parent_step_id, name, status, duration_ms, model_name, usage: usage ?? null, request_diagnostics: request_diagnostics ?? null })),
    tool_count: tools.length,
    tools: tools.map(({ step_id, name, status, error_code, accepted_evidence_count, evidence_refs }) =>
      ({ step_id, name, status, error_code, accepted_evidence_count, evidence_refs })),
    failed_tool_count: tools.filter(tool => ['failed', 'rejected', 'cancelled'].includes(tool.status)).length,
    accepted_evidence_count: sumKnown(tools, tool => tool.accepted_evidence_count),
    evidence_refs: evidenceRefs,
    // Local Reader history may omit the sink-enriched evidence fields. That is unknown, not rejection.
    requested_locations_accepted: task.lids.map(lid => ({ lid, accepted: tools.some(tool => tool.accepted_evidence_count != null)
      ? evidenceRefs.some(ref => evidenceCovers(ref, lid)) : null })),
    source_count: sources.length,
    valid_source_count: sources.filter(source => source.valid).length,
  };
}

// Recompute statistics from already saved artifacts without a Reader or provider call.
if (args.includes('--summarize')) {
  const existing = JSON.parse(fs.readFileSync(path.join(output, 'run.json'), 'utf8'));
  if (existing.status === 'running') throw new Error('Wait for the trial to stop before re-summarizing');
  for (const row of existing.cases) {
    const read = suffix => JSON.parse(fs.readFileSync(path.join(output, `${row.id}.${suffix}.json`), 'utf8'));
    if (fs.existsSync(path.join(output, `${row.id}.outcome.json`))) {
      row.statistics = statistics(row, read('outcome'), read('history'), row.elapsed_ms, read('sources'));
    }
  }
  fs.writeFileSync(path.join(output, 'run.json'), `${JSON.stringify(existing, null, 2)}\n`);
  console.log(JSON.stringify({ variant, status: existing.status, recomputed_from_saved_artifacts: true }));
  process.exit(0);
}
if (!fs.existsSync(binary)) throw new Error('Requested Reader binary does not exist');
if (fs.existsSync(path.join(output, 'run.json'))) throw new Error('Output already exists; choose a new --out');
if (fs.existsSync(path.join(root, '.env'))) process.loadEnvFile(path.join(root, '.env'));
if (!process.env.OPENCODE_BASE_URL || !process.env.OPENCODE_API_KEY || !process.env.FLUID_LLM_MODEL) {
  throw new Error('Configure the existing provider base, key, and model');
}
process.env.SEMANTIC_EVAL_SERVER = binary;
fs.mkdirSync(output, { recursive: true });

const report = {
  version: 'book-text-batch-trial.v1', variant, dataset: datasetVersion,
  started_at: new Date().toISOString(), status: 'running', binary,
  model: process.env.FLUID_LLM_MODEL,
  corpus: { book_id: corpus.base.book_id, source_utf16: corpus.source.length },
  protocol: {
    samples_per_case: 1, cases_per_variant: 3, question_order: selectedIds,
    location_input: 'Exact source LIDs in the same user prompt for both variants; source text withheld.',
    model_instructions: 'Read all supplied locations and answer with citations; no named-tool instruction.',
    state: 'New local Reader process and isolated memory/private directory for every case.',
    usage: 'Final provider-reported model activity usage; any missing field stays null; reasoning is a subset of output.',
    grading: 'Source resolution and evidence acceptance recorded; facts reviewed from answers without another model.',
    timeout_ms: 300000,
  },
  cases: [],
};
const writeJson = (name, value) => fs.writeFileSync(path.join(output, name), `${JSON.stringify(value, null, 2)}\n`);
const save = () => writeJson('run.json', report);
save();

try {
  for (const task of trialTasks) {
    const memoryDir = fs.mkdtempSync(path.join(os.tmpdir(), `understand-book-batch-${variant}-${task.id}-`));
    const row = { ...task, started_at: new Date().toISOString(), status: 'running' };
    report.cases.push(row);
    save();
    let server;
    let started;
    let outcome;
    let history;
    const sources = [];
    try {
      server = await startServer(bookDir, memoryDir, root, {
        UNDERSTAND_BOOK_PRIVATE_DIR: path.join(memoryDir, 'private'),
        UB_OBSERVABILITY_MODE: 'off',
      });
      started = performance.now();
      outcome = await server.api('agent/chat', { message: task.message }, 'POST', 300000);
      row.elapsed_ms = performance.now() - started;
      writeJson(`${task.id}.outcome.json`, outcome);
      history = await server.api('agent/history');
      writeJson(`${task.id}.history.json`, history);
      const turnId = history.current.turns.at(-1).turn_id;
      for (const source of outcome.answer_view?.sources ?? []) {
        try {
          const resolved = await server.api('agent/source.resolve', { turn_id: turnId, source_ref_id: source.source_ref_id }, 'POST');
          const text = resolved.highlighted_quote;
          sources.push({ id: source.source_ref_id, valid: resolved.can_open_in_reader === true && resolved.stale === false
            && typeof text === 'string' && text.length > 0 && corpus.source.includes(text), resolved });
        } catch (error) { sources.push({ id: source.source_ref_id, valid: false, error: error.message }); }
      }
      writeJson(`${task.id}.sources.json`, sources);
      row.answer = citationText(outcome);
      row.statistics = statistics(task, outcome, history, row.elapsed_ms, sources);
      row.status = outcome.incomplete ? 'incomplete' : 'completed';
      if (!row.statistics.model_activity_count) throw new Error('Missing durable model activities; usage comparison unavailable');
    } catch (error) {
      row.status = 'failed';
      row.error = error.message;
      row.elapsed_ms ??= started === undefined ? null : performance.now() - started;
      if (server && !history) {
        try { history = await server.api('agent/history'); writeJson(`${task.id}.history.json`, history); } catch { /* Original error remains authoritative. */ }
      }
      row.statistics ??= statistics(task, outcome, history, row.elapsed_ms, sources);
      throw error; // No paid retries after a setup/provider failure.
    } finally {
      row.finished_at = new Date().toISOString();
      save();
      if (server) await server.stop();
    }
    console.log(JSON.stringify({ variant, id: row.id, status: row.status, elapsed_ms: Math.round(row.elapsed_ms),
      models: row.statistics.model_activity_count, tools: row.statistics.tool_count,
      usage: row.statistics.usage, sources: `${row.statistics.valid_source_count}/${row.statistics.source_count}` }));
  }
  report.status = 'completed';
} catch (error) {
  report.status = 'failed';
  report.error = error.message;
  process.exitCode = 1;
} finally {
  report.finished_at = new Date().toISOString();
  report.aggregate = {
    cases: report.cases.length,
    model_calls: sumKnown(report.cases, row => row.statistics?.model_activity_count),
    elapsed_ms: sumKnown(report.cases, row => row.elapsed_ms),
    usage: Object.fromEntries(usageKeys.map(key => [key, sumKnown(report.cases, row => row.statistics?.usage[key])])),
  };
  save();
  console.log(JSON.stringify({ variant, status: report.status, aggregate: report.aggregate, error: report.error ?? null }));
}
