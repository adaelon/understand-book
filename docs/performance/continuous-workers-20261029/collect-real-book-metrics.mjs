import { existsSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL, fileURLToPath } from 'node:url';

const run = path.resolve(process.argv[2] ?? '');
const state = JSON.parse(readFileSync(path.join(run, 'control-state.json'), 'utf8'));
const read = (file) => existsSync(file) ? JSON.parse(readFileSync(file, 'utf8')) : undefined;
const directories = (dir) => existsSync(dir) ? readdirSync(dir, { withFileTypes: true }).filter(x => x.isDirectory()).map(x => path.join(dir, x.name)) : [];
const events = readFileSync(path.join(run, 'control-events.jsonl'), 'utf8').trim().split(/\r?\n/).filter(Boolean).map(line => JSON.parse(line));
const taskRoot = path.join(state.workspace_dir, '.build/automatic-build/v2/tasks/book_structure');
const attempts = [];
for (const task of directories(taskRoot)) for (const dir of directories(path.join(task, 'attempts'))) {
  const timing = read(path.join(dir, 'generation-timing.json'));
  const input = read(path.join(dir, 'input-observation.json'));
  const result = read(path.join(dir, 'result.json'));
  const metrics = read(path.join(dir, 'metrics.json'));
  // Deliberately read only Engine timing/accounting records, never candidate or input bodies.
  attempts.push({ work_unit_id: decodeURIComponent(path.basename(task)), physical_attempt: Number(path.basename(dir)),
    opened_at: timing?.opened_at, generation_started_at: timing?.generation_started_at,
    input_render_finished_at: input?.finished_at,
    completed_at: result?.created_at,
    result_status: metrics?.status, opaque_handoff_ref: timing?.opaque_handoff_ref,
    usage: metrics?.usage, bytes: metrics && { input: metrics.input_bytes, output: metrics.output_bytes },
    generation_timing: timing });
}
const epoch = (value) => value ? Date.parse(value) : NaN;
const generationTimes = attempts.map(x => epoch(x.generation_started_at)).filter(Number.isFinite);
const openedTimes = attempts.map(x => epoch(x.opened_at)).filter(Number.isFinite);
const committed = attempts.filter(x => x.result_status === 'committed');
const terminal = events.find(x => (x.response?.step ?? x.response)?.action?.kind === 'DONE');
const firstGeneration = generationTimes.length ? Math.min(...generationTimes) : undefined;
const firstOpen = openedTimes.length ? Math.min(...openedTimes) : undefined;
const end = terminal && epoch(terminal.finished_at);
const count = (values) => values.reduce((out, value) => { out[value ?? 'unknown'] = (out[value ?? 'unknown'] ?? 0) + 1; return out; }, {});
const liveChildren = new Set();
let peakOwnedChildren = 0;
for (const event of events) {
  for (const child of event.request?.terminal_children ?? []) liveChildren.delete(child);
  if (event.kind === 'child_started') liveChildren.add(event.child);
  peakOwnedChildren = Math.max(peakOwnedChildren, liveChildren.size);
}
const quantiles = (values) => {
  const sorted = values.filter(Number.isFinite).sort((a, b) => a - b);
  return { samples: sorted.length, p50_ms: sorted.length ? sorted[Math.ceil(sorted.length * .5) - 1] : null,
    p95_ms: sorted.length ? sorted[Math.ceil(sorted.length * .95) - 1] : null,
    mean_ms: sorted.length ? sorted.reduce((a,b) => a+b,0)/sorted.length : null };
};
const chapters = new Map();
for (const item of committed) {
  const match = /^structure:chapter:(.+):\d+-[a-f0-9]+$/u.exec(item.work_unit_id);
  if (!match) continue;
  const list = chapters.get(match[1]) ?? []; list.push(item); chapters.set(match[1], list);
}
const chapterGaps = [];
for (const [chapter, list] of chapters) {
  list.sort((a,b) => epoch(a.generation_started_at) - epoch(b.generation_started_at));
  for (let index=1; index<list.length; index++) {
    const gap = epoch(list[index].generation_started_at) - epoch(list[index-1].completed_at);
    if (Number.isFinite(gap) && gap >= 0) chapterGaps.push({ chapter, gap_ms: gap });
  }
}
const { resolveAutomaticBuildTarget } = await import(pathToFileURL(path.join(state.source_code, 'packages/core/src/build-orchestrator.ts')).href);
const { readAcceptedBookStructureClose } = await import(pathToFileURL(path.join(state.source_code, 'packages/core/src/automatic-build-close.ts')).href);
const { buildAutomaticBuildStageMetricsSummary } = await import(pathToFileURL(path.join(state.source_code, 'packages/core/src/automatic-build-metrics.ts')).href);
const target = resolveAutomaticBuildTarget(state.workspace_dir, run);
const accepted = readAcceptedBookStructureClose(target, 'full');
const engineMetrics = buildAutomaticBuildStageMetricsSummary(target, 'book_structure');
const repository = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..');
const { listAutomaticBuildWorkers } = await import(pathToFileURL(path.join(repository, 'packages/core/src/automatic-build-worker.ts')).href);
const workers = listAutomaticBuildWorkers(state.invocation_ref).map(worker => ({
  root_handoff_ref: worker.root_handoff_ref, status: worker.status, terminal_reason: worker.terminal_reason,
  assignment_count: worker.assignments.length, completed_assignment_count: worker.completed_assignments.length,
  delivered_input_bytes: worker.delivered_input_bytes, submitted_output_bytes: worker.submitted_output_bytes,
}));
const report = { version: 'continuous_workers_real_book_metrics.v1', run_dir: run, engine: state.engine,
  scope: state.scope, quality_profile: state.quality_profile, max_parallel: state.max_parallel,
  engine_done: !!terminal, accepted_full_quality_book_structure: !!accepted,
  first_generation_started_at: firstGeneration === undefined ? null : new Date(firstGeneration).toISOString(),
  engine_done_at: terminal?.finished_at ?? null,
  first_complete_input_to_done_ms: firstGeneration !== undefined && Number.isFinite(end) ? end - firstGeneration : null,
  first_open_to_done_ms: firstOpen !== undefined && Number.isFinite(end) ? end - firstOpen : null,
  native_child_starts: events.filter(x => x.kind === 'child_started').length,
  peak_owned_native_children: peakOwnedChildren,
  registered_worker_count: workers.length,
  workers_crossing_dispatches: workers.filter(worker => worker.assignment_count > 1).length,
  workers_completing_multiple_dispatches: workers.filter(worker => worker.completed_assignment_count > 1).length,
  worker_terminal_reasons: count(workers.map(worker => worker.terminal_reason)),
  worker_assignment_counts: workers.map(worker => worker.assignment_count),
  attempt_count: attempts.length, committed_attempts: committed.length,
  distinct_committed_work_units: new Set(committed.map(x => x.work_unit_id)).size,
  status_counts: count(attempts.map(x => x.result_status)),
  repeated_task_attempts: attempts.length - new Set(attempts.map(x => x.work_unit_id)).size,
  engine_retry_count: engineMetrics.retry_count,
  engine_attempt_count: engineMetrics.attempt_count,
  model_usage_sources: count(attempts.map(x => x.usage?.source)),
  models: [...new Set(attempts.map(x => x.usage?.model).filter(Boolean))],
  reasoning_efforts: [...new Set(attempts.map(x => x.usage?.reasoning_effort).filter(Boolean))],
  known_input_tokens: attempts.reduce((sum,x) => sum+(x.usage?.input_tokens ?? 0),0),
  known_output_tokens: attempts.reduce((sum,x) => sum+(x.usage?.output_tokens ?? 0),0),
  dynamic_chapter_commit_to_next_generation: quantiles(chapterGaps.map(x => x.gap_ms)),
  generation_confirmation_to_commit: quantiles(committed.map(x => epoch(x.completed_at) - epoch(x.generation_started_at))),
  chapter_gap_samples: chapterGaps,
  limitations: ['Token totals count known native/executor receipts only; absent usage remains unavailable.',
    'Primary duration starts at generation.start after the executor has acknowledged all input chunks and ends at Engine DONE.',
    'Commit timestamps are recorded at submission entry, before validation and writing; chapter gaps include that processing, continuation, and next-input delivery rather than pure idle time.',
    'Generation confirmation to commit measures input confirmation to submission entry, excluding subsequent submit processing.',
    'Both runs rebuild the whole BookStructure from the same accepted full-book prerequisites; this is not a whole-prebuild speedup estimate.'],
  attempts, workers };
writeFileSync(path.join(run, 'engine-metrics.json'), JSON.stringify(engineMetrics, null, 2) + '\n');
writeFileSync(path.join(run, 'metrics.json'), JSON.stringify(report, null, 2) + '\n');
const { attempts: _, chapter_gap_samples: __, workers: ___, ...summary } = report;
console.log(JSON.stringify(summary));
