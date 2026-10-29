import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { afterEach, expect, it, vi } from 'vitest';
import { prepareVideoTranslation } from '../src/video-translation';
import { runVideoTranslation } from '../../../skills/build/video-translation';
import { runVideoCodex, type VideoExecutorControl } from '../../../skills/build/video-executor-runtime';

vi.mock('../../../skills/build/video-executor-runtime', async original => ({
  ...await original<typeof import('../../../skills/build/video-executor-runtime')>(), runVideoCodex: vi.fn(),
}));
const dirs: string[] = [];
afterEach(() => { vi.clearAllMocks(); for (const dir of dirs.splice(0)) rmSync(dir, { recursive: true, force: true }); });

function fixture(count = 360) {
  const workspace = mkdtempSync(path.join(tmpdir(), 'video-parallel-')); dirs.push(workspace);
  const { manifest } = JSON.parse(readFileSync(new URL('../../../crates/base-schema/fixtures/video-contract.json', import.meta.url), 'utf8')).cases[0];
  manifest.source.duration_ms = count * 1000;
  manifest.transcript_map_path = 'transcript_map.json';
  manifest.coverage = [{ capability: 'transcript', accepted: [{ video: manifest.material.video, start_ms: 0, end_ms: count * 1000 }], pending: [], failed: [] }];
  writeFileSync(path.join(workspace, 'video_source_manifest.json'), JSON.stringify(manifest));
  writeFileSync(path.join(workspace, 'transcript_map.json'), JSON.stringify({ transcript: manifest.material.transcript,
    segments: Array.from({ length: count }, (_, i) => ({ segment_id: `s${i}`, text: 'This is not overflow.',
      location: { video: manifest.material.video, start_ms: i * 1000, end_ms: (i + 1) * 1000 - 100 },
      text_range: { lid: String(i), range: { start: 0, end: 21 } }, words: [] })) }));
  return workspace;
}

function model(options: { latency?: number; failGroup?: string } = {}) {
  let active = 0, peak = 0;
  const started: string[] = [];
  vi.mocked(runVideoCodex).mockImplementation(async (input, control: VideoExecutorControl = {}) => {
    const encoded = input.args.find(arg => arg.startsWith('mcp_servers.understand_book_build_executor.args='))!.split('=').slice(1).join('=');
    const args = JSON.parse(encoded) as string[];
    const endpoint = args.at(-1)!;
    const handoff = input.args.at(-1)!;
    const group = input.logical_call_id.split(':').at(-1)!;
    started.push(group); active++; peak = Math.max(peak, active);
    let id = 0, submitted = 0;
    const call = async (name: string, request: unknown) => {
      control.signal?.throwIfAborted();
      const response = await fetch(endpoint, { method: 'POST', headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ jsonrpc: '2.0', id: ++id, method: 'tools/call', params: { name, arguments: request } }) });
      expect(response.status).toBe(200);
      const rpc: any = await response.json();
      expect(rpc.result.isError, JSON.stringify(rpc)).toBe(false);
      return JSON.parse(rpc.result.content[0].text).action;
    };
    try {
      let action = await call('executor.open', { version: 'automatic_build_executor_open_request.v3', opaque_handoff_ref: handoff });
      while (action.kind !== 'DONE') {
        if (action.kind === 'DELIVER_INPUT') action = await call('executor.input.next', action.next_request);
        else if (action.kind === 'INPUT_BATCH') {
          const b = action.batch;
          action = await call(b.final_for_generation ? 'executor.generation.start' : 'executor.input.next', {
            version: b.final_for_generation ? 'automatic_build_executor_generation_start_request.v3' : 'automatic_build_executor_input_next_request.v4',
            opaque_session_ref: b.opaque_session_ref, generation_input_ref: b.generation_input_ref,
            ...(b.final_for_generation ? { confirmed_through_ordinal: b.last_ordinal } : { ack_through_ordinal: b.last_ordinal }),
          });
        } else if (action.kind === 'GENERATE') {
          await delay(options.latency ?? 20, undefined, { signal: control.signal });
          if (group === options.failGroup && submitted === 1) throw new Error('fixture model failed');
          const contract = action.output_contract;
          action = await call('executor.submit_candidate', { version: 'automatic_build_executor_candidate_submit.v3',
            opaque_session_ref: action.opaque_session_ref, candidate_sink_ref: action.candidate_sink_ref,
            candidate: { transcript: contract.transcript, translations: contract.segment_ids.map((segment_id: string) => ({ segment_id, zh_text: '这不是上溢。' })) } });
          submitted++;
          await control.onProgress?.();
        } else throw new Error(`Unexpected ${action.kind}`);
      }
      expect(action.status).toBe('committed');
      expect(input.complete()).toBe(true);
      return { exit_code: 0, complete: true, events: [] };
    } finally { active--; }
  });
  return { started, peak: () => peak, active: () => active };
}

it('accepts interleaved groups without losing tasks or publication coverage and resumes without model calls', async () => {
  const workspace = fixture(), worker = model();
  let publishing = 0, peakPublishing = 0;
  const publish = async () => {
    publishing++; peakPublishing = Math.max(peakPublishing, publishing);
    const before = readFileSync(path.join(workspace, 'video_enrichments.json'));
    await delay(25);
    expect(readFileSync(path.join(workspace, 'video_enrichments.json'))).toEqual(before);
    publishing--;
  };
  await runVideoTranslation(workspace, 'fixture', publish);
  expect(worker.peak()).toBe(2); expect(peakPublishing).toBe(1);
  const plan = prepareVideoTranslation(workspace);
  expect(plan.tasks.every(task => task.status === 'accepted' && task.attempts === 1)).toBe(true);
  const catalog = JSON.parse(readFileSync(path.join(workspace, 'video_enrichments.json'), 'utf8'));
  expect(catalog.batches).toHaveLength(3);
  expect(catalog.batches.flatMap((batch: any) => JSON.parse(readFileSync(path.join(workspace, batch.files.find((file: string) => file.endsWith('transcript_translations.json'))), 'utf8')))).toHaveLength(360);
  const calls = worker.started.length;
  peakPublishing = 0;
  await runVideoTranslation(workspace, 'must-not-launch', publish);
  expect(peakPublishing).toBe(1);
  expect(worker.started).toHaveLength(calls);
  expect(prepareVideoTranslation(workspace)).toEqual(plan);
});

it('stops refilling on failure, preserves accepted work and resumes only missing tasks', async () => {
  const workspace = fixture(), worker = model({ failGroup: 'zh-001', latency: 30 });
  await expect(runVideoTranslation(workspace, 'fixture')).rejects.toThrow('fixture model failed');
  expect(worker.started).toEqual(['zh-001', 'zh-002']);
  expect(worker.active()).toBe(0);
  const interrupted = prepareVideoTranslation(workspace);
  const accepted = interrupted.tasks.filter(task => task.status === 'accepted');
  expect(accepted.length).toBeGreaterThan(0);
  const bytes = accepted.map(task => readFileSync(path.join(workspace, '.build/video-translation/accepted', task.id + '.json')));
  expect(interrupted.tasks.filter(task => task.group === 'zh-003').every(task => task.attempts === 0 && task.status === 'pending')).toBe(true);
  model(); await runVideoTranslation(workspace, 'fixture');
  const resumed = prepareVideoTranslation(workspace);
  accepted.forEach((task, i) => {
    expect(resumed.tasks.find(t => t.id === task.id)?.attempts).toBe(task.attempts);
    expect(readFileSync(path.join(workspace, '.build/video-translation/accepted', task.id + '.json'))).toEqual(bytes[i]);
  });
});

it('cancels both active groups without starting queued work', async () => {
  const workspace = fixture(), worker = model({ latency: 50 }), stop = new AbortController();
  await expect(runVideoTranslation(workspace, 'fixture', undefined, { signal: stop.signal, onProgress: () => stop.abort() })).rejects.toThrow();
  expect(worker.started).toEqual(['zh-001', 'zh-002']); expect(worker.active()).toBe(0);
  expect(prepareVideoTranslation(workspace).tasks.filter(t => t.group === 'zh-003').every(t => t.attempts === 0)).toBe(true);
});

it('measures the same deterministic workload at one and two slots', async () => {
  const samples: Array<{ slots: number; elapsed_ms: number; peak: number }> = [];
  let expected: string[] | undefined;
  for (let repeat = 0; repeat < 3; repeat++) for (const slots of [1, 2] as const) {
    const workspace = fixture(), worker = model({ latency: 120 }), start = performance.now();
    await runVideoTranslation(workspace, 'fixture', undefined, { max_parallel: slots });
    samples.push({ slots, elapsed_ms: performance.now() - start, peak: worker.peak() });
    const plan = prepareVideoTranslation(workspace);
    expect(plan.tasks.every(task => task.status === 'accepted' && task.attempts === 1)).toBe(true);
    const contents = plan.tasks.map(task => readFileSync(path.join(workspace, '.build/video-translation/accepted', task.id + '.json'), 'utf8'));
    if (expected) expect(contents).toEqual(expected); else expected = contents;
    expect(worker.peak()).toBe(slots);
  }
  if (process.env.VIDEO_PARALLEL_EVIDENCE_DIR) {
    mkdirSync(process.env.VIDEO_PARALLEL_EVIDENCE_DIR, { recursive: true });
    writeFileSync(path.join(process.env.VIDEO_PARALLEL_EVIDENCE_DIR, 'translation-timing.json'), JSON.stringify({
      scenario: '360 transcript segments; 3 groups; 12 tasks; 120ms simulated model latency per candidate',
      real_model: false, outputs_equal: true, samples }, null, 2));
  }
}, 30_000);
