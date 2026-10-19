import { appendFileSync, cpSync, copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { prepareVideoVisualUnits } from '../../packages/core/src/video-visual-units';
import { runVideoVisualUnits, publishVideoVisualSnapshot } from '../../skills/build/video-visual-units';
const root = path.resolve('evals/video/generated/vi11'), workspace = path.join(root, 'workspace'), service = path.join(root, 'service');
mkdirSync(root, { recursive: true });
const origin = path.resolve('evals/video/generated/vi10/workspace');
if (!existsSync(path.join(workspace, 'video_source_manifest.json'))) {
  const link = JSON.parse(readFileSync(path.join(origin, 'video_publication.json'), 'utf8'));
  cpSync(path.resolve(origin, link.directory), workspace, { recursive: true });
}
// The published package omits preparation state. Preserve the workspace's
// import record so the official publisher also writes its resumable link.
if (!existsSync(path.join(workspace, 'video_import.json')))
  copyFileSync(path.join(origin, 'video_import.json'), path.join(workspace, 'video_import.json'));
const manifest = JSON.parse(readFileSync(path.join(workspace, 'video_source_manifest.json'), 'utf8'));
const options = { interval_ms: 1000, supplement_ms: 15000, change_threshold: 0.06, content_rect: [0, 0, 1, 0.89] as [number, number, number, number] };
const ranges = [[550000, 655000], [1180000, 1225000]].map(([start_ms, end_ms]) => ({ video: manifest.material.video, start_ms, end_ms }));
const plan = await prepareVideoVisualUnits(workspace, 'lecture-v1', options, ranges);
console.log(JSON.stringify({ scan_frames: plan.scan.samples.length, scan_candidates: plan.scan.candidate_indices.length,
  scan_ms: plan.scan.elapsed_ms, frames: plan.frames.length, tasks: plan.tasks.length }));
// Independent VI0a fixture with its own material and transcript positions.
const supplement = path.join(root, 'supplement');
if (!existsSync(path.join(supplement, 'video_source_manifest.json'))) {
  mkdirSync(path.join(supplement, 'media'), { recursive: true });
  copyFileSync('evals/video/generated/supplement.mp4', path.join(supplement, 'media/original.mp4'));
  const m = structuredClone(manifest), video = { media_id: 'vi0a-controlled-dtype-v1', media_revision: '1' };
  m.material = { book_id: 'vi11-controlled-supplement', video, transcript: { transcript_id: 'vi11-supplement-context', transcript_revision: '1', video } };
  m.source = { ...m.source, video, duration_ms: 6000, imported_path: 'media/original.mp4', original_path: 'VI0a controlled fixture' };
  m.coverage = [];
  writeFileSync(path.join(supplement, 'video_source_manifest.json'), JSON.stringify(m, null, 2));
  writeFileSync(path.join(supplement, 'transcript_map.json'), JSON.stringify({ transcript: m.material.transcript,
    segments: [0, 1, 2].map(i => ({ segment_id: `context-${i}`, text: `Controlled teaching context at occurrence ${i + 1}; no source audio.`,
      location: { video, start_ms: i * 2000, end_ms: (i + 1) * 2000 }, text_range: { lid: `p${i}`, range: { start: 0, end: 1 } }, words: [] })) }, null, 2));
}
await prepareVideoVisualUnits(supplement, 'repeat-v1', options);
if (process.argv[2]) {
  const publisher = path.resolve('target/debug/publish_book.exe');
  if (!existsSync(path.join(root, 'baseline-publication.json'))) {
    const baseline = await publishVideoVisualSnapshot(workspace, publisher, service);
    writeFileSync(path.join(root, 'baseline-publication.json'), JSON.stringify(baseline));
  }
  await runVideoVisualUnits(supplement, 'repeat-v1', path.resolve(process.argv[2]));
  await runVideoVisualUnits(workspace, 'lecture-v1', path.resolve(process.argv[2]), async () => {
    const receipt = await publishVideoVisualSnapshot(workspace, publisher, service);
    appendFileSync(path.join(root, 'publications.jsonl'), JSON.stringify(receipt) + '\n');
    console.log(JSON.stringify({ reference: receipt.reference, reused: receipt.reused, elapsed_ms: receipt.elapsed_ms }));
  });
}
