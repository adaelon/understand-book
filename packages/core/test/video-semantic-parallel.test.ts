import { afterEach, expect, it, vi } from 'vitest';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { runVideoSemanticSeries, type VideoSemanticJob } from '../../../skills/build/video-semantic-series';
import { runVideoCodex, type VideoExecutorControl } from '../../../skills/build/video-executor-runtime';

vi.mock('../../../skills/build/video-executor-runtime', async importOriginal => ({
  ...await importOriginal<typeof import('../../../skills/build/video-executor-runtime')>(),
  runVideoCodex: vi.fn(),
}));

const roots: string[] = [];
afterEach(() => {
  vi.mocked(runVideoCodex).mockReset();
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true });
});

function fixture(count = 8) {
  const root = mkdtempSync(path.join(tmpdir(), 'video-semantic-pool-')); roots.push(root);
  const f = JSON.parse(readFileSync(new URL('../../../crates/base-schema/fixtures/video-contract.json', import.meta.url), 'utf8')).cases[0];
  const publication = f.questions[0].publication, published = path.join(root, 'published');
  for (const [file, value] of Object.entries({ 'publication.json': { reference: publication }, 'base.json': f.base,
    'video_source_manifest.json': f.manifest, [f.manifest.transcript_map_path]: f.map })) {
    const target = path.join(published, file); mkdirSync(path.dirname(target), { recursive: true });
    writeFileSync(target, JSON.stringify(value));
  }
  writeFileSync(path.join(published, 'source.txt'), f.source_text);
  writeFileSync(path.join(root, 'video_source_manifest.json'), JSON.stringify(f.manifest));
  writeFileSync(path.join(root, 'video_publication.json'), JSON.stringify({ reference: publication, directory: 'published' }));
  const jobs: VideoSemanticJob[] = Array.from({ length: count }, (_, i) => ({ batch_id: `discourse_${i}`,
    selection: { vu_ids: [], segment_ids: [f.map.segments[0].segment_id] } }));
  const frozen = (job: VideoSemanticJob) => JSON.parse(readFileSync(path.join(root, '.build/video-discourse', job.batch_id, 'input.json'), 'utf8'));
  const accepted = () => existsSync(path.join(root, 'video_enrichments.json'))
    ? JSON.parse(readFileSync(path.join(root, 'video_enrichments.json'), 'utf8')).batches.map((b: any) => b.batch_id) as string[] : [];
  return { root, publication, jobs, frozen, accepted };
}

type Launch = Parameters<typeof runVideoCodex>[0];
function launchDetails(input: Launch) {
  const prefix = 'mcp_servers.understand_book_build_executor.args=';
  const args: string[] = JSON.parse(input.args.find(arg => arg.startsWith(prefix))!.slice(prefix.length));
  const start = args.indexOf('serve');
  const [workspace, stage, jobsFile, handoff, endpoint] = args.slice(start + 1);
  return { workspace, stage, jobs: JSON.parse(readFileSync(jobsFile, 'utf8')) as VideoSemanticJob[], handoff, endpoint,
    index: Number(input.logical_call_id.split(':').at(-1)) };
}

function client(input: Launch) {
  const details = launchDetails(input);
  expect(details.endpoint).toMatch(/^http:\/\/127\.0\.0\.1:/);
  let id = 0;
  const call = async (name: string, args: unknown): Promise<any> => {
    const response = await fetch(details.endpoint, { method: 'POST', headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ jsonrpc: '2.0', id: ++id, method: 'tools/call', params: { name, arguments: args } }) });
    expect(response.status).toBe(200);
    const rpc: any = await response.json();
    expect(rpc.result.isError).not.toBe(true);
    return JSON.parse(rpc.result.content[0].text);
  };
  const open = () => call('executor.open', { version: 'automatic_build_executor_open_request.v3', opaque_handoff_ref: details.handoff });
  const generate = async (delivery: any) => {
    expect(delivery.action.kind).toBe('DELIVER_INPUT');
    let request = delivery.action.next_request, semanticInput = '', batch: any;
    do {
      const response = await call('executor.input.next', request); batch = response.action.batch;
      semanticInput += batch.chunks.filter((c: any) => c.segment === 'semantic_input').map((c: any) => c.payload_utf8).join('');
      request = { ...request, ack_through_ordinal: batch.last_ordinal };
    } while (!batch.final_for_generation);
    const source = JSON.parse(semanticInput);
    expect(source).not.toHaveProperty('existing_nodes');
    const grant = await call('executor.generation.start', { version: 'automatic_build_executor_generation_start_request.v3',
      opaque_session_ref: batch.opaque_session_ref, generation_input_ref: batch.generation_input_ref, confirmed_through_ordinal: batch.last_ordinal });
    return { request: { version: 'automatic_build_executor_candidate_submit.v3', opaque_session_ref: grant.action.opaque_session_ref,
      candidate_sink_ref: grant.action.candidate_sink_ref }, candidate: { input_ref: source.input_ref, items: [{ local_id: 'rule',
        mode: 'informative', local_function: 'explanation', local_summary: '片段解释计算规则', vu_ids: [],
        support: { mode: 'text', members: [{ kind: 'text', segment_id: source.transcript[0].segment_id, support_role: '转写给出计算规则' }] }, relations: [] }] } };
  };
  const submit = async (delivery: any) => { const next = await generate(delivery); return call('executor.submit_candidate', { ...next.request, candidate: next.candidate }); };
  const finish = async (delivery?: any) => {
    let response = delivery ?? await open(), completed = 0;
    while (response.action.kind !== 'DONE') { response = await submit(response); completed++; }
    expect(response.action.status).toBe('committed');
    return completed;
  };
  return { ...details, call, open, generate, submit, finish };
}

function barrier(parties: number) {
  let arrivals = 0, release!: () => void;
  const promise = new Promise<void>(resolve => { release = resolve; });
  return async () => { if (++arrivals === parties) release(); await promise; };
}
function waitForAbort(control: VideoExecutorControl) {
  return new Promise<never>((_, reject) => {
    const aborted = () => reject(control.signal!.reason);
    if (control.signal?.aborted) aborted(); else control.signal!.addEventListener('abort', aborted, { once: true });
  });
}
const completed = () => ({ exit_code: 0, complete: true, events: [] });

it('runs two bounded discourse sessions, freezes one source snapshot, and preserves all interleaved admissions', async () => {
  const f = fixture(), firstPair = barrier(2), launched: number[] = [], directories: string[] = [];
  let active = 0, peak = 0;
  vi.mocked(runVideoCodex).mockImplementation(async input => {
    const c = client(input); launched.push(c.index); directories.push(input.run_dir);
    for (const job of f.jobs) expect(f.frozen(job).publication).toEqual(f.publication);
    expect(c.jobs.length).toBeLessThanOrEqual(3);
    peak = Math.max(peak, ++active);
    try {
      if (c.index < 2) await firstPair();
      expect(await c.finish()).toBe(c.jobs.length);
      expect(input.complete()).toBe(true);
      return completed();
    } finally { active--; }
  });
  await runVideoSemanticSeries(f.root, 'discourse', f.jobs, 'fake-codex');
  expect(peak).toBe(2); expect(active).toBe(0); expect(launched).toEqual([0, 1, 2]);
  expect(new Set(directories).size).toBe(3);
  expect(f.accepted().sort()).toEqual(f.jobs.map(job => job.batch_id).sort());
  for (const job of f.jobs) {
    const contribution = JSON.parse(readFileSync(path.join(f.root, `video-enrichments/${job.batch_id}/discourse.json`), 'utf8'));
    expect(contribution.input_publication).toEqual(f.publication);
  }
  await runVideoSemanticSeries(f.root, 'discourse', f.jobs, 'must-not-launch');
  expect(vi.mocked(runVideoCodex)).toHaveBeenCalledTimes(3);
});

it('allows an explicit one-slot discourse run without changing the three-task batch or logical identities', async () => {
  const f = fixture(6), sizes: number[] = [], ids: string[] = [];
  let active = 0, peak = 0;
  vi.mocked(runVideoCodex).mockImplementation(async input => {
    const c = client(input); sizes.push(c.jobs.length); ids.push(input.logical_call_id); peak = Math.max(peak, ++active);
    try { await c.finish(); return completed(); } finally { active--; }
  });
  await runVideoSemanticSeries(f.root, 'discourse', f.jobs, 'fake-codex', { max_parallel: 1 });
  expect(peak).toBe(1); expect(sizes).toEqual([3, 3]);
  expect(ids).toEqual([0, 1].map(i => `video:${path.resolve(f.root)}:discourse:${i}`));
});

it('stops a failed task before its successor, waits for its sibling, and resumes only the unaccepted suffix', async () => {
  const f = fixture(), firstPair = barrier(2), settled: number[] = [], launched: number[] = [];
  vi.mocked(runVideoCodex).mockImplementation(async (input, control = {}) => {
    const c = client(input); launched.push(c.index);
    try {
      const open = await c.open(); await firstPair();
      if (c.index !== 0) return await waitForAbort(control);
      const next = await c.submit(open), generated = await c.generate(next);
      generated.candidate.items[0].support.members[0].segment_id = 'undelivered';
      let result: any;
      for (let attempt = 0; attempt < 3; attempt++) result = await c.call('executor.submit_candidate', { ...generated.request, candidate: generated.candidate });
      expect(result.action).toEqual({ kind: 'DONE', status: 'retryable_failure' });
      throw new Error('semantic retryable failure');
    } finally { settled.push(c.index); }
  });
  await expect(runVideoSemanticSeries(f.root, 'discourse', f.jobs, 'fake-codex')).rejects.toThrow('semantic retryable failure');
  expect(launched).toEqual([0, 1]); expect(settled.sort()).toEqual([0, 1]);
  expect(f.accepted()).toEqual(['discourse_0']);
  const frozenRefs = f.jobs.map(job => f.frozen(job).input_ref), resumed: Array<{ index: number; batches: string[] }> = [];
  vi.mocked(runVideoCodex).mockImplementation(async input => {
    const c = client(input); resumed.push({ index: c.index, batches: c.jobs.map(j => j.batch_id) });
    await c.finish(); return completed();
  });
  await runVideoSemanticSeries(f.root, 'discourse', f.jobs, 'fake-codex');
  expect(resumed).toEqual([{ index: 0, batches: ['discourse_1', 'discourse_2'] },
    { index: 1, batches: ['discourse_3', 'discourse_4', 'discourse_5'] }, { index: 2, batches: ['discourse_6', 'discourse_7'] }]);
  expect(f.jobs.map(job => f.frozen(job).input_ref)).toEqual(frozenRefs);
  expect(f.accepted().sort()).toEqual(f.jobs.map(job => job.batch_id).sort());
});

it('stops new groups when budget admission rejects a sibling and waits for the active launch to settle', async () => {
  const f = fixture(), started = barrier(2), launched: number[] = [], settled: number[] = [];
  vi.mocked(runVideoCodex).mockImplementation(async (input, control = {}) => {
    const c = client(input); launched.push(c.index);
    try {
      await c.open(); await started();
      if (c.index === 1) throw new Error('budget admission denied');
      return await waitForAbort(control);
    } finally { settled.push(c.index); }
  });
  await expect(runVideoSemanticSeries(f.root, 'discourse', f.jobs, 'fake-codex')).rejects.toThrow('budget admission denied');
  expect(launched).toEqual([0, 1]); expect(settled.sort()).toEqual([0, 1]); expect(f.accepted()).toEqual([]);
});

it('forwards caller cancellation to both sessions and does not acquire another group', async () => {
  const f = fixture(), controller = new AbortController(), launched: number[] = [], settled: number[] = [];
  let opened = 0;
  vi.mocked(runVideoCodex).mockImplementation(async (input, control = {}) => {
    const c = client(input); launched.push(c.index);
    try {
      await c.open();
      if (++opened === 2) controller.abort(new Error('cancel discourse'));
      return await waitForAbort(control);
    } finally { settled.push(c.index); }
  });
  await expect(runVideoSemanticSeries(f.root, 'discourse', f.jobs, 'fake-codex', { signal: controller.signal })).rejects.toThrow('cancel discourse');
  expect(launched).toEqual([0, 1]); expect(settled.sort()).toEqual([0, 1]); expect(f.accepted()).toEqual([]);
});

it('keeps graph groups serial and leaves graph preparation in their local session', async () => {
  const f = fixture(7), groups: number[] = [];
  let active = 0, peak = 0;
  vi.mocked(runVideoCodex).mockImplementation(async input => {
    const c = launchDetails(input); groups.push(c.index); peak = Math.max(peak, ++active);
    expect(c.stage).toBe('graph'); expect(c.endpoint).toBeUndefined();
    expect(existsSync(path.join(f.root, '.build/video-graph'))).toBe(false);
    await Promise.resolve();
    for (const job of c.jobs) {
      const file = path.join(f.root, `video-enrichments/${job.batch_id}/graph.json`);
      mkdirSync(path.dirname(file), { recursive: true }); writeFileSync(file, '{}');
    }
    active--; return completed();
  });
  await runVideoSemanticSeries(f.root, 'graph', f.jobs, 'fake-codex', { max_parallel: 2 });
  expect(peak).toBe(1); expect(groups).toEqual([0, 1, 2]);
});
