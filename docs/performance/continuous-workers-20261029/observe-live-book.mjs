import { readFileSync, readdirSync, existsSync, writeFileSync, mkdirSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

// Maintenance observation: timing, lifecycle and accounting only. Never input/candidate bodies.
const [workspaceArg, outputArg] = process.argv.slice(2);
if (!workspaceArg || !outputArg) throw new Error('Usage: observe-live-book.mjs <workspace> <output.json>');
const workspace = path.resolve(workspaceArg);
const output = path.resolve(outputArg);
const observedAt = new Date().toISOString();
const read = file => existsSync(file) ? JSON.parse(readFileSync(file, 'utf8')) : undefined;
const directories = directory => existsSync(directory) ? readdirSync(directory, { withFileTypes: true })
  .filter(entry => entry.isDirectory()).map(entry => path.join(directory, entry.name)) : [];
const checkpoint = read(path.join(workspace, '.build/controller-inputs/root-checkpoint.json'));
if (!checkpoint?.invocation_ref) throw new Error('Live controller checkpoint is missing');
const repository = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..');
const { listAutomaticBuildWorkers } = await import(pathToFileURL(path.join(repository, 'packages/core/src/automatic-build-worker.ts')).href);
const workers = listAutomaticBuildWorkers(checkpoint.invocation_ref).map(worker => ({
  root_handoff_ref: worker.root_handoff_ref, status: worker.status, terminal_reason: worker.terminal_reason,
  assignments: worker.assignments, completed_assignments: worker.completed_assignments,
  delivered_input_bytes: worker.delivered_input_bytes, submitted_output_bytes: worker.submitted_output_bytes,
  wait_responses: worker.wait_responses ?? 0,
}));
const attempts = [];
for (const task of directories(path.join(workspace, '.build/automatic-build/v2/tasks/book_structure'))) {
  const id = decodeURIComponent(path.basename(task));
  const kind = /^unit:.*:fragment:/u.test(id) ? 'discovery' : /^structure:chapter:/u.test(id) ? 'chapter'
    : /^structure:theme:/u.test(id) ? 'theme' : /^structure:themes:/u.test(id) ? 'theme_control' : 'outline';
  for (const attempt of directories(path.join(task, 'attempts'))) {
    const timing = read(path.join(attempt, 'generation-timing.json'));
    const result = read(path.join(attempt, 'result.json'));
    const metrics = read(path.join(attempt, 'metrics.json'));
    attempts.push({ work_unit_id: id, kind, physical_attempt: Number(path.basename(attempt)),
      opaque_handoff_ref: timing?.opaque_handoff_ref, opened_at: timing?.opened_at,
      generation_started_at: timing?.generation_started_at, submitted_at: result?.created_at,
      status: metrics?.status, input_bytes: metrics?.input_bytes, output_bytes: metrics?.output_bytes,
      usage_source: metrics?.usage?.source, model: metrics?.usage?.model,
    });
  }
}
const count = values => values.reduce((out, key) => (out[key ?? 'unknown'] = (out[key ?? 'unknown'] ?? 0) + 1, out), {});
const successful = attempts.filter(attempt => attempt.status === 'committed');
const report = { version: 'continuous_workers_live_observation.v1', observed_at: observedAt,
  workspace, invocation_ref: checkpoint.invocation_ref, controller_progress: checkpoint.progress,
  registered_workers: workers.length, active_workers: workers.filter(worker => worker.status !== 'done').length,
  workers_completing_multiple_dispatches: workers.filter(worker => worker.completed_assignments.length > 1).length,
  completed_worker_assignments: workers.reduce((sum, worker) => sum + worker.completed_assignments.length, 0),
  worker_reasons: count(workers.map(worker => worker.terminal_reason ?? worker.status)),
  attempt_statuses: count(attempts.map(attempt => attempt.status)),
  successful_by_kind: count(successful.map(attempt => attempt.kind)),
  latest_success_at: successful.map(attempt => attempt.submitted_at).filter(Boolean).sort().at(-1),
  latest_successes: successful.sort((a, b) => (b.submitted_at ?? '').localeCompare(a.submitted_at ?? '')).slice(0, 32),
  workers, attempts,
};
mkdirSync(path.dirname(output), { recursive: true });
writeFileSync(output, JSON.stringify(report, null, 2) + '\n');
const { workers: _, attempts: __, latest_successes: ___, ...summary } = report;
console.log(JSON.stringify(summary));
