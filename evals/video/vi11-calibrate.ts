import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { scanVideoVisualCandidates, selectVisualCandidates, DEFAULT_VISUAL_SCAN } from '../../packages/core/src/video-visual-scan';
const out = path.resolve('evals/video/generated/vi11/calibration'); mkdirSync(out, { recursive: true });
const spec = JSON.parse(readFileSync('evals/video/samples.json', 'utf8'));
const rows: any[] = [];
// Same labelled microchange, progressive and held-page samples for every option.
for (const interval_ms of [500, 1000, 2000]) for (const sample of [
  { id: 'micro', file: 'evals/video/generated/supplement.mp4', range: { start_ms: 0, end_ms: 6000 }, anchors: [2000, 4000] },
  { id: 'progressive', file: spec.source.path, range: { start_ms: 595000, end_ms: 655000 }, anchors: [600000, 615000, 630000, 650000] },
  { id: 'repeated', file: spec.source.path, range: { start_ms: 1180000, end_ms: 1225000 }, anchors: [1180000, 1200000, 1210000, 1220000] },
]) {
  const options = { ...DEFAULT_VISUAL_SCAN, interval_ms, content_rect: [0, 0, 1, 0.89] as [number, number, number, number] };
  const scan = await scanVideoVisualCandidates(sample.file, options, sample.range);
  writeFileSync(path.join(out, `${sample.id}-${interval_ms}.json`), JSON.stringify(scan));
  for (const change_threshold of [0.01, 0.03, 0.06]) {
    const indices = selectVisualCandidates(scan.samples, { ...options, change_threshold });
    const times = indices.map(i => scan.samples[i].pts_ms);
    // Microchanges require observations on BOTH sides within one second. For
    // held states record nearest observation distance, then visually verify it.
    const misses = sample.id === 'micro' ? sample.anchors.filter(t => !times.some(p => p >= t && p <= t + 1000)
      || !times.some(p => p < t && p >= t - 1000)) : [];
    const anchor_distances_ms = sample.anchors.map(t => ({ labelled_ms: t, nearest_ms: times.reduce((a, b) => Math.abs(a - t) <= Math.abs(b - t) ? a : b),
      distance_ms: Math.min(...times.map(p => Math.abs(p - t))) }));
    rows.push({ sample: sample.id, interval_ms, change_threshold, frames: scan.samples.length, candidates: times.length,
      misses, anchor_distances_ms, semantic_review: sample.id === 'micro' ? 'controlled' : 'requires_original_frame_review', times,
      elapsed_ms: scan.elapsed_ms, decoded_bytes: scan.decoded_bytes });
  }
}
writeFileSync(path.join(out, 'comparison.json'), JSON.stringify(rows, null, 2));
console.log(JSON.stringify(rows.map(({ times, ...row }) => row), null, 2));
