import { afterEach, expect, it } from 'vitest';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { ExternalBuildCostLedger, type ExternalBuildCostBinding } from '../src/external-build-cost';
import { runVideoCodex } from '../../../skills/build/video-executor-runtime';
import { OPERATOR_VIDEO_REQUIRED_CAPABILITIES, readOperatorVideoProgress, runOperatorVideoBuild } from '../../../skills/build/video-operator-build';
import { runVideoSemanticSeries } from '../../../skills/build/video-semantic-series';

const roots: string[] = [];
afterEach(() => { for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true }); });
function fixture() {
  const root = mkdtempSync(path.join(tmpdir(), 'video-operator-')); roots.push(root);
  const requests: Array<{ suffix: string; body: any }> = [], charges = new Map<string, any>();
  let blocked = false, receiptOffline = false;
  const binding: ExternalBuildCostBinding = { version: 'external_build_cost.v1', cost_owner: 'platform', task_ref: 'parent',
    max_micro_cny: 1000, reserved_per_execution_micro_cny: 100, confirmation: 'operator-budget' };
  const fetcher: typeof fetch = async (url, options) => {
    const suffix = String(url).split('/parent')[1], body = options?.body ? JSON.parse(String(options.body)) : undefined;
    requests.push({ suffix, body });
    if (suffix === '') return Response.json(binding);
    if (suffix === '/admit') {
      if (blocked) return new Response('', { status: 409 });
      charges.set(body.call_id, { state: 'sent', ...body });
      return Response.json({ admitted: true, call_id: body.call_id });
    }
    if (suffix.startsWith('/charges/')) return charges.has(suffix.slice(9)) ? Response.json({ charge: charges.get(suffix.slice(9)) }) : new Response('', { status: 404 });
    if (suffix === '/receipts') { if (receiptOffline) throw new Error('receipt offline'); return Response.json({}); }
    throw new Error(`Unexpected ${suffix}`);
  };
  const ledger = () => new ExternalBuildCostLedger('parent', binding, { base_url: 'http://operator', cookie: 'SECRET', csrf_token: 'SECRET', origin: 'http://operator' }, root, fetcher);
  const run = (name: string, script: string, complete = true) => { const run_dir = path.join(root, name); mkdirSync(run_dir, { recursive: true });
    return { executable: process.execPath, args: ['-e', script], run_dir, logical_call_id: `video:material:${name}`, complete: () => complete }; };
  return { root, ledger, requests, run, charges, block: () => { blocked = true; }, offline: (value: boolean) => { receiptOffline = value; } };
}

it('denied video admission never spawns Codex and advertises the actual partial receipt capability', async () => {
  const f = fixture(), marker = path.join(f.root, 'launched'); f.block();
  await expect(runVideoCodex(f.run('blocked', `require('fs').writeFileSync(${JSON.stringify(marker)},'bad')`), { cost_ledger: f.ledger() })).rejects.toThrow('http_409');
  expect(existsSync(marker)).toBe(false);
  expect(f.requests.find(r => r.suffix === '/admit')!.body).toMatchObject({ provider: 'openai', model: 'codex_default', receipt_capability: 'partial', budget_enforced: false });
  expect(f.requests.filter(r => r.suffix === '/receipts')).toEqual([]);
});

it('records actual Codex usage as partial, including a failed execution with returned usage', async () => {
  const f = fixture();
  for (const exit of [0, 2]) {
    const result = runVideoCodex(f.run(`exit-${exit}`, `console.log(JSON.stringify({type:'turn.completed',usage:{input_tokens:13,output_tokens:7}}));process.exit(${exit})`), { cost_ledger: f.ledger() });
    if (exit) await expect(result).rejects.toThrow('incomplete'); else await result;
  }
  const receipts = f.requests.filter(r => r.suffix === '/receipts').map(r => r.body);
  expect(receipts.map(r => r.status)).toEqual(['partial', 'partial']);
  expect(receipts.map(r => r.succeeded)).toEqual([true, false]);
  expect(receipts[0]).toMatchObject({ usage: { observations: [{ input_tokens: 13, output_tokens: 7 }] }, evidence: {
    source: 'codex_cli_stream_observations', monetary_cost_available: false, billing_granularity: 'execution_total' } });
  expect(receipts.every(r => !('provider_cost_micro_cny' in r))).toBe(true);
  expect(readFileSync(path.join(f.root, 'external-build-cost/parent', readdirSync(path.join(f.root, 'external-build-cost/parent'))[0]), 'utf8')).not.toContain('SECRET');
});

it('distinguishes an unobserved spawned failure from a process that was never started', async () => {
  const f = fixture();
  await expect(runVideoCodex(f.run('unknown', 'process.exit(2)', false), { cost_ledger: f.ledger() })).rejects.toThrow();
  const missing = f.run('missing', ''); missing.executable = path.join(f.root, 'missing-executable');
  await expect(runVideoCodex(missing, { cost_ledger: f.ledger() })).rejects.toThrow();
  expect(f.requests.filter(r => r.suffix === '/receipts').map(r => r.body.status)).toEqual(['unknown', 'not_sent']);
});

it('keeps same-named workspace material executions distinct under their shared parent budget', async () => {
  const f = fixture();
  for (const material of ['first', 'second']) {
    const workspace = path.join(f.root, material, 'workspace');
    await expect(runVideoSemanticSeries(workspace, 'graph', [{ batch_id: 'batch', selection: { vu_ids: [], segment_ids: [] } }],
      path.join(f.root, 'missing-executable'), { cost_ledger: f.ledger() })).rejects.toThrow();
  }
  const admissions = f.requests.filter(r => r.suffix === '/admit').map(r => r.body);
  expect(admissions.map(a => a.attempt)).toEqual([1, 1]);
  expect(admissions[0].logical_call_id).toBe(`parent:video:${path.join(f.root, 'first', 'workspace')}:graph:0`);
  expect(admissions[1].logical_call_id).toBe(`parent:video:${path.join(f.root, 'second', 'workspace')}:graph:0`);
});

it('cancels a running Codex launch and still writes its unknown terminal receipt', async () => {
  const f = fixture(), controller = new AbortController();
  const task = runVideoCodex(f.run('cancel', "console.log(JSON.stringify({type:'turn.started'}));setInterval(()=>{},1000)", false), {
    cost_ledger: f.ledger(), signal: controller.signal, onProgress: () => { controller.abort(); },
  });
  await expect(task).rejects.toThrow();
  expect(f.requests.find(r => r.suffix === '/receipts')!.body).toMatchObject({ status: 'unknown', succeeded: false, evidence: { outcome: 'cancelled' } });
}, 15000);

it('stops and receipts a running launch when the host rejects a progress acknowledgment', async () => {
  const f = fixture();
  await expect(runVideoCodex(f.run('rejected-ack', "console.log(JSON.stringify({type:'turn.started'}));setInterval(()=>{},1000)", false), {
    cost_ledger: f.ledger(), onProgress: async () => { throw new Error('host cancelled progress'); },
  })).rejects.toThrow('host cancelled progress');
  expect(f.requests.find(r => r.suffix === '/receipts')!.body).toMatchObject({ status: 'unknown', succeeded: false });
}, 15000);

it('recovers a failed terminal receipt without launching or charging for accepted work again', async () => {
  const f = fixture(); f.offline(true);
  await expect(runVideoCodex(f.run('terminal', "console.log(JSON.stringify({type:'turn.completed',usage:{output_tokens:7}}))"), { cost_ledger: f.ledger() })).rejects.toThrow('receipt offline');
  f.offline(false); await f.ledger().prepare();
  const receipts = f.requests.filter(r => r.suffix === '/receipts').map(r => r.body);
  expect(receipts).toHaveLength(2); expect(receipts[1]).toEqual(receipts[0]);
  expect(f.requests.filter(r => r.suffix === '/admit')).toHaveLength(1);
  const saved = path.join(f.root, 'video-enrichments/accepted/graph.json'); mkdirSync(path.dirname(saved), { recursive: true }); writeFileSync(saved, '{}');
  await runVideoSemanticSeries(f.root, 'graph', [{ batch_id: 'accepted', selection: { vu_ids: [], segment_ids: [] } }], 'must-not-launch', { cost_ledger: f.ledger() });
  expect(f.requests.filter(r => r.suffix === '/admit')).toHaveLength(1);
});

it('reuses a complete original video workspace with all source-understanding stages and no Tutor assets', async () => {
  const f = fixture(), workspace = path.join(f.root, 'workspace'), service = path.join(f.root, 'service');
  const published = path.join(service, 'library/published/book/publication');
  mkdirSync(path.join(workspace, '.build'), { recursive: true }); mkdirSync(published, { recursive: true });
  const video = { media_id: 'video', media_revision: '1' };
  writeFileSync(path.join(workspace, 'video_source_manifest.json'), JSON.stringify({ source: { duration_ms: 1000 }, material: { video } }));
  writeFileSync(path.join(workspace, 'transcript_map.json'), '{}');
  writeFileSync(path.join(workspace, '.build/operator-video-semantics.json'), '[]');
  const catalog = { version: 'video_enrichments.v1', material: {}, batches: ['translation', 'visual_index', 'graph', 'discourse'].map(capability => {
    const file = `video-enrichments/${capability}/accepted.json`; mkdirSync(path.dirname(path.join(workspace, file)), { recursive: true }); writeFileSync(path.join(workspace, file), '{}');
    return { batch_id: capability, files: [file], coverage: [{ capability, accepted: [{ video, start_ms: 0, end_ms: 1000 }], pending: [], failed: [] }] };
  }) };
  for (const dir of [workspace, published]) writeFileSync(path.join(dir, 'video_enrichments.json'), JSON.stringify(catalog));
  writeFileSync(path.join(workspace, 'video_publication.json'), JSON.stringify({ reference: { book_id: 'book', publication_id: 'publication' }, directory: published }));
  const observed: any[] = [];
  const result = await runOperatorVideoBuild({ input_path: workspace, workspace_dir: workspace, codex_executable: 'must-not-launch',
    publisher_executable: 'must-not-republish', publication_service_root: service, cost_ledger: f.ledger(), onProgress: p => { observed.push(p); } });
  expect(result.required_capabilities).toEqual(['text', 'video', 'translation', 'visual_index', 'graph', 'discourse']);
  expect(OPERATOR_VIDEO_REQUIRED_CAPABILITIES).not.toContain('tutor');
  expect(readOperatorVideoProgress(workspace)).toMatchObject({ stage: 'ready', status: 'completed', completed: 1, total: 1 });
  expect(f.requests.filter(r => r.suffix === '/admit')).toEqual([]);
  expect(observed.some(p => p.stage === 'ready')).toBe(true);
  expect(existsSync(path.join(workspace, 'teaching_map.json'))).toBe(false);
});

it('rejects a different raw MP4 before reusing a completed workspace and preserves its accepted source', async () => {
  const f = fixture(), workspace = path.join(f.root, 'workspace'); mkdirSync(workspace);
  const original = path.join(f.root, 'original.mp4'), other = path.join(f.root, 'different.mp4');
  writeFileSync(other, 'different');
  const video = { media_id: 'video', media_revision: '1' };
  const imported = JSON.stringify({ version: 'video_import.v1', video, title: 'original', original_path: original,
    stage: 'awaiting_transcript', copied_bytes: 1, total_bytes: 1, failure: null,
    source: { video, imported_path: 'media/original.mp4', original_path: original, duration_ms: 1000, width: 64, height: 64 } });
  writeFileSync(path.join(workspace, 'video_import.json'), imported);
  writeFileSync(path.join(workspace, 'video_source_manifest.json'), '{}');
  writeFileSync(path.join(workspace, 'transcript_map.json'), '{}');
  writeFileSync(path.join(workspace, 'source.txt'), 'accepted original transcript');
  await expect(runOperatorVideoBuild({ input_path: other, workspace_dir: workspace, codex_executable: 'must-not-launch',
    publisher_executable: 'must-not-publish', publication_service_root: f.root, cost_ledger: f.ledger() })).rejects.toThrow('already owns another video');
  expect(f.requests.filter(r => r.suffix === '/admit')).toEqual([]);
  expect(readFileSync(path.join(workspace, 'video_import.json'), 'utf8')).toBe(imported);
  expect(readFileSync(path.join(workspace, 'source.txt'), 'utf8')).toBe('accepted original transcript');
});
