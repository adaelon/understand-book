import { afterEach, expect, it } from 'vitest';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { importVideo } from '../src/video-import';
import { validateFrameLocation } from '../src/video-contract';
import { DEFAULT_VISUAL_SCAN, extractVisualFrame, extractVisualFrames, scanVideoVisualCandidates } from '../src/video-visual-scan';
import { buildVideoTranscript } from '../src/video-transcript';
import { prepareVideoVisualUnits, readVideoVisualPlan } from '../src/video-visual-units';

const roots: string[] = [];
afterEach(() => roots.splice(0).forEach(root => rmSync(root, { recursive: true, force: true })));

it.each([0, 5])('keeps import, audio, scans and exact frames on the relative timeline for offset %s', async offset => {
  const root = mkdtempSync(path.join(tmpdir(), 'video-timeline-')); roots.push(root);
  const file = path.join(root, 'source.mp4'), workspace = path.join(root, 'workspace');
  execFileSync(process.env.FFMPEG || 'ffmpeg', ['-nostdin', '-v', 'error', '-f', 'lavfi', '-i', 'testsrc2=size=160x90:rate=10:duration=3',
    '-f', 'lavfi', '-i', 'sine=frequency=440:sample_rate=16000:duration=3', '-c:v', 'libx264', '-pix_fmt', 'yuv420p',
    '-c:a', 'aac', '-output_ts_offset', String(offset), file], { windowsHide: true });
  const imported = await importVideo(file, workspace), source = imported.source!;
  expect(imported.stage).toBe('awaiting_transcript');
  expect(source.duration_ms).toBeCloseTo(offset ? 3064 : 3000, 6);
  expect(readFileSync(path.join(workspace, source.imported_path))).toEqual(readFileSync(file));
  const first = offset ? 63.96484375 : 0;
  const scan = await scanVideoVisualCandidates(file);
  expect(scan.samples.map(sample => sample.pts_ms)).toEqual([first, first + 1000, first + 2000]);
  const bounded = await scanVideoVisualCandidates(file, DEFAULT_VISUAL_SCAN, { start_ms: 1000, end_ms: 2000 });
  expect(bounded.samples.map(sample => sample.pts_ms)).toEqual([first + 1000]);
  const frames = await extractVisualFrames(file, source, [500, 2900], path.join(root, 'batch'), [0, 0, 1, 1]);
  expect(frames.map(frame => frame.frame.actual_pts_ms)).toEqual([first + 500, first + 2900]);
  for (const frame of frames) {
    validateFrameLocation(frame.frame, source);
    const single = await extractVisualFrame(file, source, frame.requested_ms, path.join(root, `${frame.id}.png`));
    expect(single.frame).toEqual(frame.frame);
    expect(single.png).toEqual(readFileSync(frame.path));
    // A frame selected by the scan must be seekable again without advancing.
    const replay = await extractVisualFrame(file, source, frame.frame.actual_pts_ms, path.join(root, `replay-${frame.id}.png`));
    expect(replay.frame).toEqual(frame.frame);
  }
  const build = await buildVideoTranscript(workspace, { recognize: async (audio, output) => {
    const info = JSON.parse(execFileSync(process.env.FFPROBE || 'ffprobe', ['-v', 'error', '-show_format', '-of', 'json', audio], { windowsHide: true, encoding: 'utf8' }));
    expect(Number(info.format.duration) * 1000).toBeCloseTo(source.duration_ms, 3);
    writeFileSync(output, JSON.stringify({ engine: 'faster-whisper', model: 'small.en', language: 'en', segments: [
      { text: ' A frame.', start: .5, end: .8, words: [{ word: ' A frame.', start: .5, end: .8, probability: 1 }] },
    ] }));
  } });
  expect(build.stage).toBe('ready');
  const transcript = JSON.parse(readFileSync(path.join(workspace, 'transcript_map.json'), 'utf8'));
  expect(transcript.segments[0].location.start_ms).toBe(500);
  if (offset) {
    const plan = await prepareVideoVisualUnits(workspace, 'relative');
    const cache = path.join(workspace, '.build/video-visual/relative');
    const oldScan = { ...plan.scan }; delete oldScan.timeline;
    oldScan.samples = oldScan.samples.map(sample => ({ ...sample, pts_ms: sample.pts_ms + 5000 }));
    writeFileSync(path.join(cache, 'scan.json'), JSON.stringify({ source, scan: oldScan }));
    writeFileSync(path.join(cache, 'frames.json'), JSON.stringify({ source, options: plan.options, ranges: plan.ranges,
      frames: plan.frames.map(frame => ({ ...frame, frame: { ...frame.frame, actual_pts_ms: frame.frame.actual_pts_ms + 5000 } })) }));
    rmSync(path.join(cache, 'plan.json'));
    const resumed = await prepareVideoVisualUnits(workspace, 'relative');
    expect(resumed.frames.map(frame => frame.frame)).toEqual(plan.frames.map(frame => frame.frame));
    expect(resumed.scan.timeline).toBe('relative.v1');
    writeFileSync(path.join(cache, 'plan.json'), JSON.stringify({ ...plan, scan: oldScan }));
    expect(() => readVideoVisualPlan(workspace, 'relative')).toThrow('prepare a new analysis ID');
  }
}, 30000);
