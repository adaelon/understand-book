import { mkdtempSync, readFileSync, writeFileSync, rmSync, mkdirSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { afterEach, expect, it } from 'vitest';
import { prepareVideoTranslation, admitVideoTranslation, acceptVideoTranslation, publishVideoTranslationGroup,
  createVideoTranslationSession, failVideoTranslation } from '../src/video-translation';
import { createBuildExecutorMcpSession } from '../../../skills/build/build-executor-mcp';
import { BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3 as bootstrap } from '../src/build-executor-connection-capability';
import { runVideoTranslation, publishVideoTranslationSnapshot } from '../../../skills/build/video-translation';
const dirs: string[] = [];
afterEach(() => { for (const dir of dirs.splice(0)) rmSync(dir, { recursive: true, force: true }); });
function fixture(count = 40) {
  const dir = mkdtempSync(path.join(tmpdir(), 'vi10-')); dirs.push(dir);
  const f = JSON.parse(readFileSync(new URL('../../../crates/base-schema/fixtures/video-contract.json', import.meta.url), 'utf8')).cases[0];
  f.manifest.source.duration_ms = count * 1000;
  const segments = Array.from({ length: count }, (_, i) => ({ segment_id: `s${i}`, text: 'This is not overflow.',
    location: { video: f.manifest.material.video, start_ms: i * 1000, end_ms: (i + 1) * 1000 - 100 },
    text_range: { lid: String(i), range: { start: 0, end: 21 } }, words: [] }));
  f.manifest.coverage = [{ capability: 'transcript', accepted: [{ video: f.manifest.material.video, start_ms: 0, end_ms: count * 1000 }], pending: [], failed: [] }];
  f.manifest.transcript_map_path = 'transcript_map.json';
  writeFileSync(path.join(dir, 'video_source_manifest.json'), JSON.stringify(f.manifest));
  writeFileSync(path.join(dir, f.manifest.transcript_map_path), JSON.stringify({ transcript: f.manifest.material.transcript, segments }));
  return { dir, plan: prepareVideoTranslation(dir) };
}
const candidate = (plan: ReturnType<typeof prepareVideoTranslation>, index = 0) => ({ transcript: plan.material.transcript,
  translations: plan.tasks[index].segment_ids.map(segment_id => ({ segment_id, zh_text: '这不是上溢。' })) });
it('rejects missing, duplicate, blank and wrong-version candidates before accepting a complete task', () => {
  const { dir, plan } = fixture(), c = candidate(plan);
  for (const invalid of [ { ...c, translations: c.translations.slice(1) }, { ...c, translations: c.translations.map(() => c.translations[0]) },
    { ...c, transcript: { ...c.transcript, transcript_revision: 'other' } }, { ...c, translations: c.translations.map(t => ({ ...t, zh_text: ' ' })) } ])
    expect(() => admitVideoTranslation(plan, plan.tasks[0], invalid)).toThrow();
  expect(() => publishVideoTranslationGroup(dir, 'zh-001')).toThrow('incomplete');
  acceptVideoTranslation(dir, plan.tasks[0].id, c);
  expect(prepareVideoTranslation(dir).tasks[0].status).toBe('accepted');
  expect(() => acceptVideoTranslation(dir, plan.tasks[0].id, { ...c, translations: c.translations.map(t => ({ ...t, zh_text: 'changed' })) })).toThrow('cannot be replaced');
  const manifestFile = path.join(dir, 'video_source_manifest.json');
  const manifest = JSON.parse(readFileSync(manifestFile, 'utf8')); manifest.coverage = [];
  writeFileSync(manifestFile, JSON.stringify(manifest));
  expect(() => prepareVideoTranslation(dir)).toThrow('Complete English transcript required');
});
it('resumes only missing tasks, preserves accepted bytes, and aggregates full timeline coverage including silence', () => {
  const { dir, plan } = fixture(125);
  acceptVideoTranslation(dir, plan.tasks[0].id, candidate(plan));
  const accepted = path.join(dir, '.build/video-translation/accepted', plan.tasks[0].id + '.json');
  const before = readFileSync(accepted);
  failVideoTranslation(dir, 'zh-001', 'interrupted');
  expect(prepareVideoTranslation(dir).tasks[1].status).toBe('failed');
  for (let i = 1; i < plan.tasks.length; i++) acceptVideoTranslation(dir, plan.tasks[i].id, candidate(plan, i));
  publishVideoTranslationGroup(dir, 'zh-001'); publishVideoTranslationGroup(dir, 'zh-002'); publishVideoTranslationGroup(dir, 'zh-001');
  expect(readFileSync(accepted)).toEqual(before);
  const catalog = JSON.parse(readFileSync(path.join(dir, 'video_enrichments.json'), 'utf8'));
  expect(catalog.batches).toHaveLength(2);
  expect(catalog.batches.map((b: any) => b.coverage[0].accepted[0])).toEqual([
    { video: plan.material.video, start_ms: 0, end_ms: 120000 }, { video: plan.material.video, start_ms: 120000, end_ms: 125000 } ]);
  const mapFile = path.join(dir, 'transcript_map.json'), map = JSON.parse(readFileSync(mapFile, 'utf8'));
  map.segments[0].text = 'Changed'; writeFileSync(mapFile, JSON.stringify(map));
  expect(() => prepareVideoTranslation(dir)).toThrow('input changed');
});
it('uses the formal MCP phase gate and advances accepted tasks in one bounded handoff', () => {
  const { dir, plan } = fixture(40), handoff = 'abhandoff1_' + 'a'.repeat(64);
  const make = () => createBuildExecutorMcpSession({ bootstrap_version: bootstrap.version, protocol_generation: bootstrap.session_protocol,
    session_private_root: dir, execute_request: createVideoTranslationSession(dir, 'zh-001', handoff) });
  const mcp = make(); let ordinal = 0;
  const call = (name: string, args: unknown, server = mcp): any => {
    const rpc = server.handle_message({ jsonrpc: '2.0', id: ++ordinal, method: 'tools/call', params: { name, arguments: args } }) as any;
    return { ...JSON.parse(rpc.result.content[0].text), isError: rpc.result.isError };
  };
  let result = call('executor.open', { version: 'automatic_build_executor_open_request.v3', opaque_handoff_ref: handoff });
  for (let task = 0; task < plan.tasks.length; task++) {
    expect(result.action.kind).toBe('DELIVER_INPUT');
    let request = result.action.next_request, last: any;
    do {
      result = call('executor.input.next', request); expect(result.isError).not.toBe(true);
      last = result.action.batch;
      request = { ...request, ack_through_ordinal: last.last_ordinal };
    } while (!last.final_for_generation);
    result = call('executor.generation.start', { version: 'automatic_build_executor_generation_start_request.v3',
      opaque_session_ref: last.opaque_session_ref, generation_input_ref: last.generation_input_ref, confirmed_through_ordinal: last.last_ordinal });
    expect(result.action.kind).toBe('GENERATE');
    result = call('executor.submit_candidate', { version: 'automatic_build_executor_candidate_submit.v3',
      opaque_session_ref: result.action.opaque_session_ref, candidate_sink_ref: result.action.candidate_sink_ref, candidate: candidate(plan, task) });
    expect(result.isError).not.toBe(true);
  }
  expect(result.action).toEqual({ kind: 'DONE', status: 'committed' });
  const resumed = make();
  expect(call('executor.open', { version: 'automatic_build_executor_open_request.v3', opaque_handoff_ref: handoff }, resumed).action.status).toBe('committed');
  expect(prepareVideoTranslation(dir).tasks.map(t => t.attempts)).toEqual([1, 1]);
});
it('the driver republishes admitted work without launching the external model on resume', async () => {
  const { dir, plan } = fixture(3);
  acceptVideoTranslation(dir, plan.tasks[0].id, candidate(plan));
  const publications: string[] = [];
  await runVideoTranslation(dir, 'must-not-launch', async group => { publications.push(group); });
  expect(publications).toEqual(['zh-001']);
  expect(prepareVideoTranslation(dir).tasks[0].attempts).toBe(0);
});
it('recovers a committed publisher reply using its actual Windows directory spelling without another media copy', async () => {
  const { dir, plan } = fixture(3);
  acceptVideoTranslation(dir, plan.tasks[0].id, candidate(plan)); publishVideoTranslationGroup(dir, 'zh-001');
  const service = path.join(dir, 'service'), destination = path.join(service, 'library/published/book/publication');
  mkdirSync(destination, { recursive: true });
  for (const file of ['video_enrichments.json', 'video_translation_progress.json']) writeFileSync(path.join(destination, file), readFileSync(path.join(dir, file)));
  const catalog = JSON.parse(readFileSync(path.join(dir, 'video_enrichments.json'), 'utf8'));
  catalog.batches.push({ batch_id: 'visual-v1', coverage: [], files: ['visual.json'] });
  writeFileSync(path.join(dir, 'video_enrichments.json'), JSON.stringify(catalog));
  writeFileSync(path.join(destination, 'video_enrichments.json'), JSON.stringify({ ...catalog, batches: [...catalog.batches].reverse() }));
  const directory = process.platform === 'win32' ? '\\\\?\\' + destination : destination;
  writeFileSync(path.join(dir, 'video_publication.json'), JSON.stringify({ reference: { book_id: 'book', publication_id: 'publication' }, directory }));
  expect(await publishVideoTranslationSnapshot(dir, 'must-not-launch', service)).toEqual({ reused: true,
    reference: { book_id: 'book', publication_id: 'publication' }, elapsed_ms: 0 });
});
