#!/usr/bin/env node
// Bridge the documented offline DTW experiment to the production curation contract.
// Its output remains pending: an estimated presentation hold is not a measured release.
import { readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { parseMusicScore, musicXmlFromBytes } from '../../packages/core/src/music-score.ts';

const [curationFile, candidateFile, outputFile] = process.argv.slice(2);
if (!curationFile || !candidateFile || !outputFile) throw new Error('Usage: node --import tsx scripts/music/prepare-alignment.mjs curation.json candidates.json output.json');
const curation = JSON.parse(readFileSync(curationFile, 'utf8'));
const raw = JSON.parse(readFileSync(candidateFile, 'utf8'));
const parsed = parseMusicScore(musicXmlFromBytes(readFileSync(path.resolve(path.dirname(curationFile), curation.score.musicxml_path))));
const provenance_id = `audio-alignment:${raw.method}`;
const ranges = raw.measures.map((measure, i) => ({ occurrence_id: `measure-${measure.index}-occurrence-1`, order: i, measure: measure.label,
  start_ms: measure.recording_seconds * 1000,
  end_ms: (raw.measures[i + 1]?.recording_seconds ?? raw.audio.trimmed_end_seconds) * 1000,
  score_object_ids: parsed.notes.filter(note => note.measure_index === measure.index).map(note => note.source_id),
}));
const notes = new Map(parsed.notes.map(note => [note.source_id, note]));
const skipped = [];
const note_alignments = raw.notes.flatMap(candidate => {
  const note = notes.get(candidate.source_id), occurrence = ranges[candidate.measure_index - 1];
  // Grace groups and tied continuations need separate review. Preserve the originals in candidates.json.
  const start_ms = candidate.onset_seconds === null ? null : candidate.onset_seconds * 1000;
  if (!note || !occurrence || candidate.grace || candidate.tied_stop || start_ms === null || start_ms < occurrence.start_ms || start_ms >= occurrence.end_ms) {
    skipped.push({ source_id: candidate.source_id, reason: 'unresolved grace/tie or onset outside candidate occurrence' }); return [];
  }
  return [{ occurrence_id: occurrence.occurrence_id, score_object_id: candidate.source_id, start_ms, end_ms: Math.min(start_ms + 160, raw.audio.duration_seconds * 1000), attack: true, provenance_id }];
});
const output = { occurrences: ranges, note_alignments,
  coverage: [{ start_ms: 0, end_ms: raw.audio.duration_seconds * 1000, granularity: 'note', method: 'audio_alignment', status: 'pending', provenance_id,
    detail: `原录音 STFT/chroma/DTW + 局部谱通量候选，未经过独立起音真值验收；${skipped.length} 个装饰音、延音或边界冲突未进入候选映射。end_ms 是 160 毫秒呈现停留，不表示实测止音。` }],
  provenance: [{ provenance_id, kind: 'audio_analysis', description: `实际处理原录音，${raw.method}；${raw.audio.frame_seconds * 1000} 毫秒帧。该结果尚未人工听核，不能宣称逐音符已校准。`, source_url: null, processor: raw.method }],
  review: { source_candidates: path.resolve(candidateFile), skipped, elapsed_seconds: raw.elapsed_seconds, status: 'pending' },
};
writeFileSync(outputFile, JSON.stringify(output, null, 2) + '\n');
curation.alignment_path = path.relative(path.dirname(curationFile), outputFile).replaceAll('\\', '/');
writeFileSync(curationFile, JSON.stringify(curation, null, 2) + '\n');
console.log(JSON.stringify({ occurrences: ranges.length, candidates: note_alignments.length, unresolved: skipped.length, status: 'pending' }));
