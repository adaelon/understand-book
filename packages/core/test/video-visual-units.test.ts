import { mkdtempSync, readFileSync, writeFileSync, rmSync, mkdirSync, renameSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { execFileSync } from 'node:child_process';
import path from 'node:path';
import { afterEach, expect, it, vi } from 'vitest';
import { prepareVideoVisualUnits, readVideoVisualPlan, admitVideoVisualCandidate, acceptVideoVisualCandidate,
  publishVideoVisualUnits, createVideoVisualSession } from '../src/video-visual-units';
import { DEFAULT_VISUAL_SCAN, extractVisualFrame, extractVisualFrames, scanVideoVisualCandidates, visualChange, selectVisualCandidates } from '../src/video-visual-scan';
import { createBuildExecutorMcpSession } from '../../../skills/build/build-executor-mcp';
import { BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3 as bootstrap } from '../src/build-executor-connection-capability';
import { runVideoVisualUnits, publishVideoVisualSnapshot } from '../../../skills/build/video-visual-units';
const dirs: string[] = [];
afterEach(() => { vi.unstubAllEnvs(); for (const dir of dirs.splice(0)) rmSync(dir, { recursive: true, force: true }); });
async function fixture() {
  const dir = mkdtempSync(path.join(tmpdir(), 'vi11-')); dirs.push(dir);
  const { manifest } = JSON.parse(readFileSync(new URL('../../../crates/base-schema/fixtures/video-contract.json', import.meta.url), 'utf8')).cases[0];
  manifest.source = { ...manifest.source, width: 64, height: 64, duration_ms: 6000, imported_path: 'media/original.mp4' };
  manifest.coverage = []; manifest.transcript_map_path = 'transcript_map.json';
  mkdirSync(path.join(dir, 'media'));
  execFileSync(process.env.FFMPEG || 'ffmpeg', ['-v', 'error', '-f', 'lavfi', '-i', 'color=white:s=64x64:r=10:d=6',
    '-vf', "drawbox=x=4:y=8:w=15:h=15:color=black:t=fill:enable='gte(t,2)*lt(t,4)'", '-c:v', 'libx264', '-crf', '0', '-y', path.join(dir, manifest.source.imported_path)], { windowsHide: true });
  const segments = Array.from({ length: 3 }, (_, i) => ({ segment_id: `s${i}`, text: `Explanation ${i}`, location: {
    video: manifest.material.video, start_ms: i * 2000, end_ms: (i + 1) * 2000 }, text_range: { lid: `p${i}`, range: { start: 0, end: 13 } }, words: [] }));
  writeFileSync(path.join(dir, 'video_source_manifest.json'), JSON.stringify(manifest));
  writeFileSync(path.join(dir, 'transcript_map.json'), JSON.stringify({ transcript: manifest.material.transcript, segments }));
  const plan = await prepareVideoVisualUnits(dir, 'first'); return { dir, plan };
}
function candidate(plan: Awaited<ReturnType<typeof prepareVideoVisualUnits>>, taskIndex = 0) {
  const task = plan.tasks[taskIndex];
  const groups: typeof plan.frames[] = [];
  for (const id of task.frame_ids) { const frame = plan.frames.find(f => f.id === id)!;
    if (groups.at(-1)?.[0].content_id === frame.content_id) groups.at(-1)!.push(frame); else groups.push([frame]); }
  return { input_ref: plan.input_ref, task_id: task.id, events: groups.map((g, i) => ({
    start_ms: i ? g[0].frame.actual_pts_ms : task.location.start_ms,
    end_ms: groups[i + 1]?.[0].frame.actual_pts_ms ?? task.location.end_ms,
    frame_ids: g.map(f => f.id), screen_text: '', description: g[0].content_id === plan.frames[0].id ? '白色画面' : '黑色方块', change: '观察到的状态' })) };
}
it('local changes survive camera movement and supplemental samples bound long unchanged gaps', () => {
  const before = new Uint8Array(128 * 64), after = before.slice();
  for (let y = 0; y < 64; y++) for (let x = 96; x < 128; x++) after[y * 128 + x] = 255;
  expect(visualChange(before, after, 128, 64, [0, 0, 0.75, 1])).toBe(0);
  after[10 * 128 + 10] = 255;
  expect(visualChange(before, after, 128, 64, [0, 0, 0.75, 1])).toBeGreaterThan(0);
  const samples = Array.from({ length: 41 }, (_, i) => ({ pts_ms: i * 1000, change: i === 8 ? 0.1 : 0 }));
  expect(selectVisualCandidates(samples, DEFAULT_VISUAL_SCAN)).toEqual([0, 7, 8, 23, 38, 40]);
});
it('reuses a completed publication when Rust orders interleaved batches differently', async () => {
  const dir = mkdtempSync(path.join(tmpdir(), 'vi11-resume-')); dirs.push(dir);
  const service = path.join(dir, 'service'), destination = path.join(service, 'library/published/book/publication');
  mkdirSync(destination, { recursive: true });
  const catalog = { version: 'video_enrichments.v1', material: {}, batches: [
    { batch_id: 'zh-001', coverage: [], files: ['zh.json'] }, { batch_id: 'visual-v1-001', coverage: [], files: ['vu.json'] }] };
  writeFileSync(path.join(dir, 'video_enrichments.json'), JSON.stringify(catalog));
  writeFileSync(path.join(destination, 'video_enrichments.json'), JSON.stringify({ ...catalog, batches: [...catalog.batches].reverse() }));
  writeFileSync(path.join(dir, 'video_publication.json'), JSON.stringify({ reference: { book_id: 'book', publication_id: 'publication' }, directory: destination }));
  expect(await publishVideoVisualSnapshot(dir, 'must-not-launch', service)).toEqual({ reused: true,
    reference: { book_id: 'book', publication_id: 'publication' }, elapsed_ms: 0 });
});
it('scans real microchanges and keeps source PTS when decoding a bounded source interval', async () => {
  const { dir, plan } = await fixture();
  expect(plan.scan.samples.map(s => s.pts_ms)).toEqual([0, 1000, 2000, 3000, 4000, 5000]);
  expect(plan.frames.map(f => f.content_id)).toEqual(['f00001', 'f00001', 'f00003', 'f00003', 'f00001', 'f00001']);
  const scan = await scanVideoVisualCandidates(path.join(dir, 'media/original.mp4'), DEFAULT_VISUAL_SCAN, { start_ms: 2000, end_ms: 4000 });
  expect(scan.samples.map(s => s.pts_ms)).toEqual([2000, 3000]);
}, 30000);
it('streamed preparation matches individual source PTS/pixels and resumes its saved frame preparation', async () => {
  const {dir,plan}=await fixture(), media=path.join(dir,'media/original.mp4');
  const batched=await extractVisualFrames(media,plan.manifest.source,[2050,2055,4150],path.join(dir,'batch'),[0,0,1,1]);
  expect(batched.map(f=>f.frame.actual_pts_ms)).toEqual([2100,4200]);
  for(const frame of batched){
    const single=await extractVisualFrame(media,plan.manifest.source,frame.requested_ms,path.join(dir,frame.id+'.png'));
    expect(frame.frame).toEqual(single.frame);
    expect(readFileSync(frame.path)).toEqual(single.png);
  }
  rmSync(path.join(dir,'.build/video-visual/first/plan.json'));
  vi.stubEnv('FFMPEG',path.join(dir,'must-not-decode-again'));
  const recovered=await prepareVideoVisualUnits(dir,'first');
  expect(recovered.frames).toEqual(plan.frames);expect(recovered.tasks).toEqual(plan.tasks);
},30000);
it('rejects foreign input, out-of-range units, absent/duplicate frames and missing native PNG', async () => {
  const { dir, plan } = await fixture(), c = candidate(plan), task = plan.tasks[0];
  for (const mutate of [
    (x: any) => x.input_ref = 'other', (x: any) => x.events[0].start_ms = -1,
    (x: any) => x.events.at(-1).end_ms = 9000, (x: any) => x.events[0].frame_ids = [],
    (x: any) => x.events[0].frame_ids.push('unseen'), (x: any) => x.events[0].frame_ids.push(x.events[0].frame_ids[0]),
    (x: any) => x.events[0].end_ms = 500,
  ]) { const invalid = structuredClone(c); mutate(invalid); expect(() => admitVideoVisualCandidate(plan, task, invalid)).toThrow(); }
  expect(admitVideoVisualCandidate(plan, task, c).map(u => u.transcript_segment_ids)).toEqual([['s0'], ['s1']]);
  renameSync(path.join(dir, plan.frames[0].path), path.join(dir, 'missing.png'));
  expect(() => acceptVideoVisualCandidate(dir, 'first', task.id, c)).toThrow();
  expect(readVideoVisualPlan(dir, 'first').tasks[0].status).toBe('pending');
}, 30000);
it('delivers native images, enforces final receipt, replays after interruption and skips exact repeated descriptions', async () => {
  const { dir, plan } = await fixture(), handoff = 'abhandoff1_' + 'a'.repeat(64);
  const make = () => createBuildExecutorMcpSession({ bootstrap_version: bootstrap.version, protocol_generation: bootstrap.session_protocol,
    session_private_root: dir, execute_request: createVideoVisualSession(dir, 'first', handoff) });
  let mcp = make(), ordinal = 0;
  const call = (name: string, args: unknown): any => {
    const rpc = mcp.handle_message({ jsonrpc: '2.0', id: ++ordinal, method: 'tools/call', params: { name, arguments: args } }) as any;
    return { ...JSON.parse(rpc.result.content[0].text), isError: rpc.result.isError, content: rpc.result.content };
  };
  let opened = call('executor.open', { version: 'automatic_build_executor_open_request.v3', opaque_handoff_ref: handoff });
  const request = opened.action.next_request;
  expect(call('executor.generation.start', { version: 'automatic_build_executor_generation_start_request.v3',
    opaque_session_ref: request.opaque_session_ref, generation_input_ref: request.generation_input_ref, confirmed_through_ordinal: 0 }).isError).toBe(true);
  let batch = call('executor.input.next', request);
  const imageBytes = batch.content.filter((c: any) => c.type === 'image').map((c: any) => c.data);
  expect(imageBytes.length).toBeGreaterThanOrEqual(4);
  expect(imageBytes[0]).toBe(readFileSync(path.join(dir, plan.frames[0].path)).toString('base64'));
  mcp = make(); opened = call('executor.open', { version: 'automatic_build_executor_open_request.v3', opaque_handoff_ref: handoff });
  batch = call('executor.input.next', opened.action.next_request);
  expect(batch.content.filter((c: any) => c.type === 'image').map((c: any) => c.data)).toEqual(imageBytes);
  const b = batch.action.batch;
  const granted = call('executor.generation.start', { version: 'automatic_build_executor_generation_start_request.v3',
    opaque_session_ref: b.opaque_session_ref, generation_input_ref: b.generation_input_ref, confirmed_through_ordinal: b.last_ordinal });
  const invalid = candidate(plan); invalid.events[0].frame_ids.push('undelivered');
  const retry = call('executor.submit_candidate', { version: 'automatic_build_executor_candidate_submit.v3',
    opaque_session_ref: granted.action.opaque_session_ref, candidate_sink_ref: granted.action.candidate_sink_ref, candidate: invalid });
  expect(retry.isError).not.toBe(true); expect(retry.action.kind).toBe('GENERATE');
  expect(retry.action.retry_feedback.expected).toContain('foreign frame');
  expect(readVideoVisualPlan(dir, 'first').tasks[0].status).toBe('running');
  const result = call('executor.submit_candidate', { version: 'automatic_build_executor_candidate_submit.v3',
    opaque_session_ref: granted.action.opaque_session_ref, candidate_sink_ref: granted.action.candidate_sink_ref, candidate: candidate(plan) });
  expect(result.isError).not.toBe(true); expect(result.action).toEqual({ kind: 'DONE', status: 'committed' });
  const final = readVideoVisualPlan(dir, 'first'); expect(final.tasks.map(t => t.attempts)).toEqual([2, 0]);
  const repeated = JSON.parse(readFileSync(path.join(dir, '.build/video-visual/first/accepted/vu-0002.json'), 'utf8'));
  expect(repeated[0].location.start_ms).toBe(4000); expect(repeated[0].transcript_segment_ids).toEqual(['s2']);
  expect(repeated[0].description_reused_from).toBe('first/vu-0001/1');
  const before = readFileSync(path.join(dir, '.build/video-visual/first/accepted/vu-0001.json'));
  await runVideoVisualUnits(dir, 'first', 'must-not-launch');
  expect(readFileSync(path.join(dir, '.build/video-visual/first/accepted/vu-0001.json'))).toEqual(before);
}, 30000);
it('publishes complete dependencies and preserves old media locations through resegmentation', async () => {
  const { dir, plan } = await fixture();
  expect(publishVideoVisualUnits(dir, 'first')).toHaveLength(0);
  for (const [i, task] of plan.tasks.entries()) acceptVideoVisualCandidate(dir, 'first', task.id, candidate(plan, i));
  publishVideoVisualUnits(dir, 'first');
  const file = path.join(dir, 'video-enrichments/visual-first-001/visual_units.json'), old = readFileSync(file);
  const units = JSON.parse(old.toString()).units;
  for (const unit of units) for (const frame of unit.representative_frames) expect(readFileSync(path.join(dir, frame.path)).length).toBe(frame.png_bytes);
  const second = await prepareVideoVisualUnits(dir, 'second');
  for (const [i, task] of second.tasks.entries()) {
    const c = candidate(second, i);
    c.events = [{ start_ms: task.location.start_ms, end_ms: task.location.end_ms, frame_ids: task.frame_ids,
      screen_text: '', description: '合并观察到的变化过程', change: '合并事件' }];
    acceptVideoVisualCandidate(dir, 'second', task.id, c);
  }
  publishVideoVisualUnits(dir, 'second'); expect(readFileSync(file)).toEqual(old);
  const catalog = JSON.parse(readFileSync(path.join(dir, 'video_enrichments.json'), 'utf8'));
  expect(catalog.batches).toHaveLength(2);
  expect(catalog.batches[0].coverage[0].accepted).toEqual([{ video: plan.manifest.material.video, start_ms: 0, end_ms: 6000 }]);
  const map = JSON.parse(readFileSync(path.join(dir, 'transcript_map.json'), 'utf8')); map.segments[0].text = 'revised';
  writeFileSync(path.join(dir, 'transcript_map.json'), JSON.stringify(map));
  expect(() => readVideoVisualPlan(dir, 'first')).toThrow('input changed');
}, 30000);
