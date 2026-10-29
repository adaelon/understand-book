import { execFileSync, spawn } from 'node:child_process';
import { appendFileSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { randomUUID } from 'node:crypto';
import { createInterface } from 'node:readline';
import { createRequire } from 'node:module';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import type { ExternalBuildCostLedger } from '../../packages/core/src/external-build-cost';

export interface VideoExecutorControl {
  signal?: AbortSignal;
  /** Independent translation/discourse groups; graph and visual dependencies stay ordered. */
  max_parallel?: 1 | 2;
  cost_ledger?: ExternalBuildCostLedger;
  model?: string;
  /** Reads accepted work, not a timer-derived estimate. */
  onProgress?: () => void | Promise<void>;
}

/** Every paid launch is admitted before spawn; a lost/failed turn is not a free turn. */
export async function runVideoCodex(input: { executable: string; args: string[]; run_dir: string;
  logical_call_id: string; complete: () => boolean }, control: VideoExecutorControl = {}) {
  control.signal?.throwIfAborted();
  const launch = randomUUID(), started = Date.now();
  const call = await control.cost_ledger?.admit(launch, `${control.cost_ledger.taskRef}:${input.logical_call_id}`, 'openai', control.model ?? 'codex_default', 'codex_cli_video');
  const events: unknown[] = [], usages: Record<string, unknown>[] = [];
  let spawned = false, exit: number | null = null, failure: unknown;
  let progress = Promise.resolve();
  try {
    control.signal?.throwIfAborted();
    const args = control.model ? [...input.args.slice(0, 1), '--model', control.model, ...input.args.slice(1)] : input.args;
    const child = spawn(input.executable, args, { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] });
    child.once('spawn', () => { spawned = true; });
    createInterface({ input: child.stdout }).on('line', line => {
      try {
        const event = JSON.parse(line);
        if (['turn.completed', 'turn.failed', 'error'].includes(event.type)) events.push(event);
        else if (event.item) events.push({ type: event.type, item_type: event.item.type, tool: event.item.tool, status: event.item.status });
        if (event.usage && typeof event.usage === 'object') usages.push(event.usage);
        progress = progress.then(() => control.onProgress?.()).catch(error => { failure ??= error; abort(); });
      } catch { /* Host diagnostics do not constitute model output. */ }
    });
    child.stderr.on('data', bytes => appendFileSync(path.join(input.run_dir, 'stderr.txt'), bytes));
    const abort = () => {
      // Codex owns a local MCP child; stopping only its parent would leave that child running.
      if (process.platform === 'win32' && child.pid) {
        const stop = spawn('taskkill', ['/PID', String(child.pid), '/T', '/F'], { windowsHide: true, stdio: 'ignore' });
        stop.on('error', () => child.kill());
      } else child.kill();
    };
    control.signal?.addEventListener('abort', abort, { once: true });
    if (control.signal?.aborted) abort();
    try {
      exit = await new Promise<number | null>((resolve, reject) => { child.once('close', resolve); child.once('error', reject); });
    } finally { control.signal?.removeEventListener('abort', abort); }
    await progress;
    if (failure) throw failure;
    control.signal?.throwIfAborted();
    if (exit !== 0 || !input.complete()) throw new Error('Video executor incomplete; resume the accepted contributions');
  } catch (error) { failure = error; }
  const complete = input.complete();
  writeFileSync(path.join(input.run_dir, 'run.json'), JSON.stringify({ execution_id: launch, elapsed_ms: Date.now() - started,
    exit_code: exit, complete, events, ...(failure ? { failure: String(failure) } : {}) }, null, 2));
  if (call) await control.cost_ledger!.settle(call, {
    // A spawned Codex without a terminal usage event is unknown, never not_sent.
    model_calls: spawned ? Math.max(1, usages.length) : 0, usages, succeeded: !failure && complete,
    outcome: control.signal?.aborted ? 'cancelled' : failure ? 'failed' : 'completed', source: 'codex_cli_stream_observations',
  });
  await control.onProgress?.();
  if (failure) throw failure;
  return { exit_code: exit, complete, events };
}

/** Compiled drivers re-enter their own command instead of addressing source .ts files. */
export function videoExecutorArgs(command: string, script: string, args: string[]) {
  return process.env.UNDERSTAND_BOOK_SIDECAR_SELF
    ? [command, ...args]
    : ['--import', pathToFileURL(createRequire(import.meta.url).resolve('tsx')).href, script, ...args];
}

/** Keep resumable CLI databases with the build, which may live on another drive. */
export function videoExecutorStorageArgs(workspace: string) {
  const directory = path.resolve(workspace, '.build/codex-state');
  mkdirSync(directory, { recursive: true });
  return ['-c', 'sqlite_home=' + JSON.stringify(directory)];
}

export function videoExecutorPrompt() {
  const sidecar = process.env.UNDERSTAND_BOOK_SIDECAR_SELF;
  return sidecar
    ? execFileSync(sidecar, ['prompt', 'automatic-build-dispatch-executor.md'], { encoding: 'utf8', windowsHide: true })
    : readFileSync(new URL('../../agents/automatic-build-dispatch-executor.md', import.meta.url), 'utf8');
}
