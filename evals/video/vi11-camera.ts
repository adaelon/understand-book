import { execFileSync } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
import assert from 'node:assert/strict';
import path from 'node:path';
import { scanVideoVisualCandidates, DEFAULT_VISUAL_SCAN } from '../../packages/core/src/video-visual-scan';
const root = path.resolve('evals/video/generated/vi11/camera'); mkdirSync(root, { recursive: true });
const file = path.join(root, 'camera-supplement.mp4');
// Controlled moving camera-like panel. The VI0a code remains unchanged outside it.
execFileSync(process.env.FFMPEG || 'ffmpeg', ['-v', 'error', '-i', 'evals/video/generated/supplement.mp4',
  '-f', 'lavfi', '-i', 'testsrc2=size=280x220:rate=30:duration=6', '-filter_complex', '[0:v][1:v]overlay=1640:0:shortest=1',
  '-an', '-c:v', 'libx264', '-crf', '0', '-y', file], { windowsHide: true });
const isolated = await scanVideoVisualCandidates(file, { ...DEFAULT_VISUAL_SCAN, content_rect: [0, 0, 0.85, 1] });
const baseline = await scanVideoVisualCandidates('evals/video/generated/supplement.mp4', { ...DEFAULT_VISUAL_SCAN, content_rect: [0, 0, 0.85, 1] });
const whole = await scanVideoVisualCandidates(file);
assert.deepEqual(isolated.samples, baseline.samples);
for (const scan of [isolated, whole]) for (const t of [2000, 4000]) {
  const times = scan.candidate_indices.map(i => scan.samples[i].pts_ms);
  assert(times.includes(t) && times.includes(t - 1000));
}
const result = { status: 'passed', source: 'VI0a supplement plus controlled moving panel, not lecture footage',
  isolated, baseline, whole, teaching_candidates_survive: true, excluded_camera_does_not_trigger_teaching_changes: true };
writeFileSync(path.join(root, 'verification.json'), JSON.stringify(result, null, 2)); console.log(JSON.stringify({ status: result.status }));
