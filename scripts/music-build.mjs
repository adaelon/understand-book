#!/usr/bin/env node
import { readFileSync, writeFileSync } from 'node:fs';
import { buildCuratedMusicWorkspace, calibrateMusicWorkspace } from '../packages/core/src/music-import.ts';
import { musicXmlFromBytes, parseMusicScore } from '../packages/core/src/music-score.ts';

const arguments_ = process.argv.slice(2);
const option = name => { const index = arguments_.indexOf(name); return index < 0 ? undefined : arguments_[index + 1]; };
const output = option('--output');
if (option('--calibration')) {
  if (!output) throw new Error('Calibration requires --output existing-workspace');
  const manifest = calibrateMusicWorkspace(output, JSON.parse(readFileSync(option('--calibration'), 'utf8')));
  console.log(JSON.stringify({ book_id: manifest.book_id, coverage: manifest.coverage }));
} else if (option('--inspect-score')) {
  const parsed = parseMusicScore(musicXmlFromBytes(readFileSync(option('--inspect-score'))));
  const result = JSON.stringify({ parts: parsed.parts, notes: parsed.notes, measure_durations: parsed.measure_durations }, null, 2);
  if (output) writeFileSync(output, result + '\n'); else console.log(result);
} else {
  if (!option('--input') || !output) throw new Error('Usage: node --import tsx scripts/music-build.mjs --input curation.json --output workspace');
  const manifest = await buildCuratedMusicWorkspace({ curationPath: option('--input'), bookDir: output, onProgress: progress => console.log(JSON.stringify(progress)) });
  console.log(JSON.stringify({ book_id: manifest.book_id, pages: manifest.pages.length, events: manifest.events.length, output }));
}
