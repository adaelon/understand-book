import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, readFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { run, selectStreams, frameInfo, sourceTime, extractFrame, makeSupplement, audioPcm } from './media.mjs';

const here = dirname(fileURLToPath(import.meta.url));
mkdirSync(join(here, 'generated'), { recursive: true });
const out = mkdtempSync(join(here, 'generated', 'test-'));
const ffmpeg = process.env.FFMPEG || 'ffmpeg';

test('video stream selection skips an attached cover even when it precedes lecture', () => {
  const { video, audio } = selectStreams({ streams: [
    { index: 0, codec_type: 'video', disposition: { attached_pic: 1 } },
    { index: 1, codec_type: 'audio' },
    { index: 2, codec_type: 'video', disposition: { attached_pic: 0 } },
  ] });
  assert.equal(video.index, 2); assert.equal(audio.index, 1);
  assert.throws(() => selectStreams({ streams: [] }), /No moving video/);
});

test('source PTS uses exact ticks and time base rather than rounded display seconds', () => {
  const f = frameInfo('config in time_base: 1/16000, frame_rate: 30/1\nn: 0 pts:8882133 pts_time:555.133312 duration:533 fmt:yuv420p s:1920x1080');
  assert.equal(f.actual_pts_ms, 555133.3125);
  assert.equal(f.width, 1920); assert.equal(f.height, 1080);
  assert.throws(() => frameInfo(''), /Decoder did not return/);
});

test('clip locations retain nonzero original offset and reject the exclusive end', () => {
  const mapping = { source_start_ms: 550000, duration_ms: 36000 };
  assert.equal(sourceTime(mapping, 0), 550000);
  assert.equal(sourceTime(mapping, 18000), 568000);
  assert.throws(() => sourceTime(mapping, 36000), /outside clip/);
});

test('real FFmpeg: off-grid seek, original resolution, small code change, A-B-A repeat', () => {
  const source = makeSupplement(out);
  const frames = [1001, 3001, 5001].map((t, i) => {
    const path = join(out, `frame-${i}.png`);
    const f = extractFrame(source, 0, t, path);
    assert.equal(f.width, 1920); assert.equal(f.height, 1080);
    assert(Math.abs(f.actual_pts_ms - (Math.floor(t / 1000) * 1000 + 1000 / 30)) < 0.001);
    return run(ffmpeg, ['-v', 'error', '-i', path, '-pix_fmt', 'rgb24', '-f', 'rawvideo', 'pipe:1']).stdout;
  });
  assert(frames[0].equals(frames[2]), 'Both A occurrences should render identical pixels');
  let changedPixels = 0;
  for (let i = 0; i < frames[0].length; i += 3) {
    if (frames[0][i] !== frames[1][i] || frames[0][i + 1] !== frames[1][i + 1] || frames[0][i + 2] !== frames[1][i + 2]) changedPixels++;
  }
  assert(changedPixels > 100 && changedPixels < 1920 * 1080 * 0.05, `Expected localized code change, got ${changedPixels} pixels`);
});

test('real FFmpeg: fast extraction matches independent slow-seek PCM at a nonzero offset', () => {
  const source = join(out, 'audio.m4a');
  run(ffmpeg, ['-v', 'error', '-f', 'lavfi', '-i', 'sine=frequency=997:sample_rate=44100:duration=4',
    '-ac', '2', '-c:a', 'aac', '-y', source]);
  const fast = audioPcm(source, 0, 537, 2537);
  const slow = audioPcm(source, 0, 537, 2537, true);
  assert.equal(fast.length, 2 * 44100 * 4);
  assert(fast.equals(slow), 'Seek must not drift to an AAC packet boundary');
});

test('manifest references resolve and no pending audio is exposed as verified speech', () => {
  const manifest = JSON.parse(readFileSync(join(here, 'samples.json')));
  for (const s of manifest.samples) {
    for (const id of s.frame_ids) assert(manifest.frames.some(f => f.id === id), id);
    for (const id of s.clip_ids) assert(manifest.clips.some(c => c.id === id), id);
    if (s.audio_truth?.status === 'pending_listening') assert.equal(s.audio_truth.correct_words, null);
    for (const l of s.acceptable_locations) assert(l.start_ms < l.end_ms);
  }
});
