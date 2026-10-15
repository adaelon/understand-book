import { copyFileSync, existsSync, mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import assert from 'node:assert/strict';
import path from 'node:path';
import { readVideoVisualPlan, type VisualUnit } from '../../packages/core/src/video-visual-units';
import { runVideoVisualUnits, publishVideoVisualSnapshot } from '../../skills/build/video-visual-units';
const root = path.resolve('evals/video/generated/vi11'), workspace = path.join(root, 'workspace'), supplement = path.join(root, 'supplement');
const out = path.resolve('evals/video/results/vi11-20261010'); mkdirSync(out, { recursive: true });
const plan = readVideoVisualPlan(workspace, 'lecture-v1'), repeat = readVideoVisualPlan(supplement, 'repeat-v1');
for (const p of [plan, repeat]) assert(p.tasks.every(t => t.status === 'accepted'));
const accepted = (dir: string, p: typeof plan) => p.tasks.flatMap(t => JSON.parse(readFileSync(path.join(dir, '.build/video-visual', p.analysis_id, 'accepted', `${t.id}.json`), 'utf8')) as VisualUnit[]);
const units = accepted(workspace, plan), repeated = accepted(supplement, repeat);
assert.deepEqual(repeated.map(u => [u.location.start_ms, u.location.end_ms]), [[0, 2000], [2000, 4000], [4000, 6000]]);
assert(repeated[0].screen_text.includes('torch.float16') && repeated[0].screen_text.includes('=='));
assert(repeated[1].screen_text.includes('torch.bfloat16') && repeated[1].screen_text.includes('!='));
assert.equal(repeated[2].description_reused_from, repeated[0].vu_id);
assert.deepEqual(repeated.map(u => u.transcript_segment_ids), [['context-0'], ['context-1'], ['context-2']]);
assert.equal(repeat.tasks[1].attempts, 0);
const link = JSON.parse(readFileSync(path.join(workspace, 'video_publication.json'), 'utf8')), directory = path.resolve(workspace, link.directory);
const manifest = JSON.parse(readFileSync(path.join(directory, 'video_source_manifest.json'), 'utf8'));
const coverage = manifest.coverage.find((c: any) => c.capability === 'visual_index');
assert.deepEqual(coverage.accepted, plan.ranges);
assert.equal(coverage.pending.reduce((n: number, c: any) => n + c.end_ms - c.start_ms, 0), manifest.source.duration_ms - 150000);
const catalog = JSON.parse(readFileSync(path.join(directory, 'video_enrichments.json'), 'utf8'));
for (const b of catalog.batches) for (const f of b.files) assert(existsSync(path.join(directory, f)));
assert.equal(JSON.parse(readFileSync(path.join(directory, 'transcript_translations.json'), 'utf8')).length, 720);
const baseline = JSON.parse(readFileSync(path.join(root, 'baseline-publication.json'), 'utf8'));
const oldDir = path.join(root, 'service/library/published', baseline.reference.book_id, baseline.reference.publication_id);
for (const f of ['source.txt', 'transcript_map.json', 'transcript_translations.json'])
  assert(readFileSync(path.join(oldDir, f)).equals(readFileSync(path.join(directory, f))));
const native = (args: string[]) => execFileSync(process.env.FFMPEG || 'ffmpeg', ['-v', 'error', ...args, '-frames:v', '1', '-pix_fmt', 'rgb24', '-f', 'rawvideo', 'pipe:1'],
  { windowsHide: true, maxBuffer: 16 * 1024 * 1024 });
const checked = [];
// Concrete checks: the representative and before/after frames must decode to
// original pixels; a new publication must not redirect a saved old media source.
for (const id of ['f00002', 'f00003', 'f00020', 'f00023', 'f00025', 'f00028', 'f00029', 'f00033', 'f00036']) {
  const f = plan.frames.find(f => f.id === id)!;
  const pixels = native(['-ss', String(f.frame.actual_pts_ms / 1000), '-i', path.join(directory, manifest.source.imported_path), '-map', '0:V:0']);
  assert(pixels.equals(native(['-i', path.join(workspace, f.path)])));
  assert(pixels.equals(native(['-ss', String(f.frame.actual_pts_ms / 1000), '-i', path.join(oldDir, manifest.source.imported_path), '-map', '0:V:0'])));
  checked.push({ frame_id: id, actual_pts_ms: f.frame.actual_pts_ms, original_pixels_equal: true, old_publication_pixels_equal: true });
}
const originalCandidates = plan.tasks.map(t => readFileSync(path.join(workspace, '.build/video-visual/lecture-v1/accepted', `${t.id}.json`)));
await runVideoVisualUnits(workspace, plan.analysis_id, 'must-not-launch');
assert.deepEqual(readVideoVisualPlan(workspace, plan.analysis_id).tasks.map(t => t.attempts), plan.tasks.map(t => t.attempts));
for (const [i, t] of plan.tasks.entries()) assert(originalCandidates[i].equals(readFileSync(path.join(workspace, '.build/video-visual/lecture-v1/accepted', `${t.id}.json`))));
assert.equal((await publishVideoVisualSnapshot(workspace, 'must-not-launch', path.join(root, 'service'))).reused, true);
const runs = [workspace, supplement].flatMap(dir => {
  const analysis = dir === workspace ? plan.analysis_id : repeat.analysis_id;
  return readdirSync(path.join(dir, '.build/video-visual', analysis, 'runs'), { recursive: true })
    .filter(f => f.toString().endsWith('run.json')).map(f => JSON.parse(readFileSync(path.join(dir, '.build/video-visual', analysis, 'runs', f.toString()), 'utf8')));
});
const publications = readdirSync(path.dirname(directory)).map(id => path.join(path.dirname(directory), id))
  .filter(dir => existsSync(path.join(dir, 'publication.json')));
const mediaBytes = statSync(path.join(directory, manifest.source.imported_path)).size;
const result = { status: 'passed', material: plan.manifest.material, publication: link.reference, baseline: baseline.reference,
  scan: { sampled_frames: plan.scan.samples.length, candidate_frames: plan.scan.candidate_indices.length, elapsed_ms: plan.scan.elapsed_ms,
    decoded_bytes: plan.scan.decoded_bytes, options: plan.options },
  semantic: { ranges: plan.ranges, units: units.length, frames: plan.frames.length, tasks: plan.tasks.length,
    png_bytes: plan.frames.reduce((n, f) => n + f.png_bytes, 0), coverage },
  repeated: { units: repeated.length, model_tasks: repeat.tasks.filter(t => t.attempts > 0).length, skipped_model_tasks: repeat.tasks.filter(t => t.attempts === 0).length,
    distinct_occurrences: true, transcript_contexts_preserved: true },
  reread: checked, english_and_chinese_unchanged: true, completed_resume_preserves_candidates_and_attempts: true,
  completed_publication_reused: true, runs, model: 'CLI default, low reasoning', monetary_cost: null,
  publication_count: publications.length, media_bytes_per_publication: mediaBytes, total_published_media_bytes: publications.length * mediaBytes,
  limitations: ['Semantic acceptance covers 150 seconds plus the controlled fixture, not the whole lecture.',
    'Semantic review observations are recorded separately; machine checks validate time/pixels/contracts.', 'Reader live Provider, DSH, Linux and installed package remain pending.'] };
writeFileSync(path.join(out, 'verification.json'), JSON.stringify(result, null, 2));
writeFileSync(path.join(out, 'visual-units.json'), JSON.stringify(units, null, 2));
writeFileSync(path.join(out, 'supplement-units.json'), JSON.stringify(repeated, null, 2));
copyFileSync(path.join(root, 'calibration/comparison.json'), path.join(out, 'calibration.json'));
copyFileSync(path.join(root, 'camera/verification.json'), path.join(out, 'camera.json'));
console.log(JSON.stringify({ status: 'passed', units: units.length, publication: link.reference }));
