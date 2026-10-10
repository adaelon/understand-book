// VI0a standalone media experiment. No product runtime or model service dependencies.
import { spawnSync } from 'node:child_process';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { resolve, join, dirname } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import assert from 'node:assert/strict';

const here = dirname(fileURLToPath(import.meta.url));
const ffmpeg = process.env.FFMPEG || 'ffmpeg';
const ffprobe = process.env.FFPROBE || 'ffprobe';
export function run(program, args, cwd) {
  const result = spawnSync(program, args, { cwd, maxBuffer: 128 * 1024 * 1024, windowsHide: true });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${program} exited ${result.status}: ${result.stderr.toString().slice(-5000)}`);
  return result;
}
export function probe(path) {
  return JSON.parse(run(ffprobe, ['-v', 'error', '-show_format', '-show_streams', '-of', 'json', path]).stdout);
}
export function selectStreams(info) {
  const video = info.streams.find(s => s.codec_type === 'video' && !s.disposition?.attached_pic);
  const audio = info.streams.find(s => s.codec_type === 'audio');
  assert(video, 'No moving video stream');
  return { video, audio };
}
export function frameInfo(log) {
  const timeBase = /config in time_base: (\d+)\/(\d+)/.exec(log);
  const frame = /n:\s*0\s+pts:\s*(-?\d+)\s+pts_time:.*?\bs:(\d+)x(\d+)/.exec(log);
  assert(timeBase && frame, 'Decoder did not return frame 0 with source PTS');
  return { actual_pts: Number(frame[1]), time_base: `${timeBase[1]}/${timeBase[2]}`,
    actual_pts_ms: Number(frame[1]) * Number(timeBase[1]) / Number(timeBase[2]) * 1000,
    width: Number(frame[2]), height: Number(frame[3]) };
}
export function extractFrame(source, stream, requestedMs, path) {
  const result = run(ffmpeg, ['-hide_banner', '-loglevel', 'info', '-copyts', '-ss', String(requestedMs / 1000),
    '-i', source, '-map', `0:${stream}`, '-vf', 'showinfo=checksum=0', '-frames:v', '1',
    '-fps_mode', 'passthrough', '-y', path]);
  return { requested_ms: requestedMs, ...frameInfo(result.stderr.toString()) };
}
export function audioPcm(source, stream, startMs, endMs, accurateFromStart = false) {
  // Decode a one-second preroll so AAC overlap state is ready before the requested sample.
  const seekMs = accurateFromStart ? 0 : Math.max(0, startMs - 1000);
  return run(ffmpeg, ['-v', 'error', '-ss', String(seekMs / 1000), '-i', source,
    '-ss', String((startMs - seekMs) / 1000), '-t', String((endMs - startMs) / 1000),
    '-map', `0:${stream}`, '-ac', '2', '-ar', '44100', '-c:a', 'pcm_s16le', '-f', 's16le', 'pipe:1']).stdout;
}
export function sourceTime(mapping, localMs) {
  assert(localMs >= 0 && localMs < mapping.duration_ms, 'Time outside clip');
  return mapping.source_start_ms + localMs;
}
function json(path, value) { writeFileSync(path, JSON.stringify(value, null, 2) + '\n'); }
function escapeHtml(value) {
  return String(value).replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]);
}
function gallery(manifest, result) {
  return `<!doctype html><meta charset="utf-8"><title>VI0a 样本与待听核原声</title>
  <style>body{font:18px/1.65 system-ui;max-width:1200px;margin:35px auto;padding:0 25px;background:#f6f7f9;color:#17202a}article{background:white;padding:24px;margin:24px 0;border-radius:14px}img,video{max-width:100%}code{word-break:break-all}.frames{display:grid;grid-template-columns:1fr 1fr;gap:18px}h1{font-size:30px}</style>
  <h1>VI0a · 样本、画面真值与待听核原声</h1><p>原片：CS336 2026 P2《PyTorch》。点击图片查看原分辨率；视频和音频均保留原片区间。原话待听核，烧录字幕只作为候选。</p>
  ${manifest.samples.map(s => `<article id="${s.id}"><h2>${escapeHtml(s.id)} · ${escapeHtml(s.title)}</h2>
    <p><b>问题：</b>${escapeHtml(s.question)}</p><p><b>预期：</b>${escapeHtml(s.expected.answer)}</p>
    <p>支持状态：${escapeHtml(s.expected.support)} · 定位范围：${escapeHtml(JSON.stringify(s.acceptable_locations))}</p>
    <div class="frames">${s.frame_ids.map(id => { const f = result.frames.find(f => f.id === id); return `<figure><a href="${f.path}"><img src="${f.path}"></a><figcaption>${id} · 请求 ${f.requested_ms} ms → 实际 ${f.actual_pts_ms} ms</figcaption></figure>`; }).join('')}</div>
    ${s.clip_ids.map(id => { const c = result.clips.find(c => c.id === id); return `<p>${id} · ${c.source_start_ms}–${c.source_end_ms} ms · ${c.source_id}</p><video controls preload="metadata" src="${c.path}"></video>${c.audio_path ? `<p>原声（待听核）</p><audio controls preload="none" src="${c.audio_path}"></audio>` : ''}`; }).join('')}
    <p>画面依据：${escapeHtml(s.visual_truth.join('；'))}</p>${s.audio_truth ? `<p>音频核对：${escapeHtml(JSON.stringify(s.audio_truth))}</p>` : ''}</article>`).join('')}`;
}
// Controlled teaching supplement: real lecture examples rendered at fixed positions.
// It has its own timeline; none of its timestamps represent the lecture.
export function makeSupplement(out) {
  const a = 'x = torch.tensor([1e-8], dtype=torch.float16)\nassert x == 0  # Underflow!';
  const b = 'x = torch.tensor([1e-8], dtype=torch.bfloat16)\nassert x != 0  # No underflow!';
  writeFileSync(join(out, 'a.txt'), a); writeFileSync(join(out, 'b.txt'), b);
  const label = 'VI0a controlled teaching supplement - NOT lecture footage';
  const draw = (file, condition) => `drawtext=font=Consolas:textfile=${file}:fontsize=38:fontcolor=black:x=100:y=300:line_spacing=30:enable='${condition}'`;
  const filter = `drawtext=font=Consolas:text='${label}':fontsize=26:fontcolor=black:x=100:y=100,` +
    draw('a.txt', 'lt(t,2)+gte(t,4)') + ',' + draw('b.txt', 'gte(t,2)*lt(t,4)');
  run(ffmpeg, ['-v', 'error', '-f', 'lavfi', '-i', 'color=c=white:s=1920x1080:r=30:d=6',
    '-vf', filter, '-c:v', 'libx264', '-preset', 'fast', '-crf', '0', '-pix_fmt', 'yuv420p', '-y', 'supplement.mp4'], out);
  return join(out, 'supplement.mp4');
}
export function replay(manifest, source, out) {
  mkdirSync(out, { recursive: true });
  const started = Date.now();
  const info = probe(source); const { video, audio } = selectStreams(info);
  assert(audio, 'Lecture audio missing');
  // Catch accidentally selecting the 480P file or an unrelated lecture before producing truth assets.
  assert.equal(Number(info.format.size), manifest.source.size_bytes, 'Unexpected source byte size');
  assert.equal(video.width, manifest.source.width); assert.equal(video.height, manifest.source.height);
  assert(Math.abs(Number(info.format.duration) * 1000 - manifest.source.duration_ms) < 1, 'Unexpected source duration');
  const supplement = makeSupplement(out);
  const result = { created_at: new Date().toISOString(), source_id: manifest.source.id, source_path: source,
    environment: { platform: process.platform, arch: process.arch, node: process.version,
      ffmpeg: run(ffmpeg, ['-version']).stdout.toString().split(/\r?\n/)[0],
      ffprobe: run(ffprobe, ['-version']).stdout.toString().split(/\r?\n/)[0] },
    probe: info, frames: [], clips: [] };
  for (const frame of manifest.frames) {
    const path = `frames/${frame.id}.png`; mkdirSync(join(out, 'frames'), { recursive: true });
    const f = extractFrame(frame.source_id === manifest.source.id ? source : supplement,
      frame.source_id === manifest.source.id ? video.index : 0, frame.requested_ms, join(out, path));
    assert(f.actual_pts_ms >= frame.requested_ms - 0.001 && f.actual_pts_ms - frame.requested_ms < 34,
      `Frame outside one-frame tolerance: ${frame.id}`);
    result.frames.push({ id: frame.id, source_id: frame.source_id, path, ...f });
  }
  for (const clip of manifest.clips) {
    const duration = clip.end_ms - clip.start_ms;
    assert(duration > 0 && clip.start_ms >= 0);
    const primary = clip.source_id === manifest.source.id;
    const input = primary ? source : supplement;
    const path = `clips/${clip.id}.mp4`; mkdirSync(join(out, 'clips'), { recursive: true });
    run(ffmpeg, ['-v', 'error', '-ss', String(clip.start_ms / 1000), '-i', input, '-t', String(duration / 1000),
      '-map', `0:${primary ? video.index : 0}`, ...(primary ? ['-map', `0:${audio.index}`] : []),
      '-c:v', 'libx264', '-preset', 'fast', '-crf', '18', '-pix_fmt', 'yuv420p',
      '-c:a', 'aac', '-movflags', '+faststart', '-y', join(out, path)]);
    const audioPath = primary ? `clips/${clip.id}.wav` : null;
    if (primary) {
      const seekMs = Math.max(0, clip.start_ms - 1000);
      run(ffmpeg, ['-v', 'error', '-ss', String(seekMs / 1000), '-i', input,
        '-ss', String((clip.start_ms - seekMs) / 1000), '-t', String(duration / 1000),
        '-map', `0:${audio.index}`, '-c:a', 'pcm_s16le', '-y', join(out, audioPath)]);
    }
    const mapping = { id: clip.id, source_id: clip.source_id, path, audio_path: audioPath,
      source_start_ms: clip.start_ms, source_end_ms: clip.end_ms, duration_ms: duration,
      mapping: 'source_ms = source_start_ms + clip_local_ms; end exclusive',
      video_encoding: 'H.264 CRF18 yuv420p; original resolution; audio AAC for playback',
      audio_encoding: primary ? 'PCM s16le; source 44100 Hz stereo' : null };
    result.clips.push(mapping); json(join(out, `clips/${clip.id}.location.json`), mapping);
  }
  result.elapsed_ms = Date.now() - started;
  json(join(out, 'replay.json'), result);
  writeFileSync(join(out, 'index.html'), gallery(manifest, result));
  return result;
}
export function verify(manifest, source, out) {
  const result = JSON.parse(readFileSync(join(out, 'replay.json')));
  const { video, audio } = selectStreams(probe(source));
  const checks = [];
  assert.deepEqual(result.frames.map(f => f.id), manifest.frames.map(f => f.id));
  assert.deepEqual(result.clips.map(c => c.id), manifest.clips.map(c => c.id));
  for (const spec of manifest.frames) {
    const f = result.frames.find(f => f.id === spec.id);
    assert.equal(f.requested_ms, spec.requested_ms); assert.equal(f.source_id, spec.source_id);
    const input = spec.source_id === manifest.source.id ? source : join(out, 'supplement.mp4');
    const reread = join(out, 'verify-frame.png');
    const decoded = extractFrame(input, spec.source_id === manifest.source.id ? video.index : 0, spec.requested_ms, reread);
    assert.equal(decoded.actual_pts, f.actual_pts); assert.equal(decoded.time_base, f.time_base);
    assert.equal(decoded.width, 1920); assert.equal(decoded.height, 1080);
    const pixels = p => run(ffmpeg, ['-v', 'error', '-i', p, '-frames:v', '1', '-pix_fmt', 'rgb24', '-f', 'rawvideo', 'pipe:1']).stdout;
    assert(pixels(reread).equals(pixels(join(out, f.path))), `Frame pixels differ: ${f.id}`);
    checks.push({ id: f.id, actual_pts_ms: f.actual_pts_ms, source_pixels_equal: true });
  }
  for (const spec of manifest.clips) {
    const c = result.clips.find(c => c.id === spec.id);
    assert.equal(c.source_start_ms, spec.start_ms); assert.equal(c.source_end_ms, spec.end_ms);
    assert.equal(c.source_id, spec.source_id); assert.equal(c.duration_ms, spec.end_ms - spec.start_ms);
    assert.deepEqual(JSON.parse(readFileSync(join(out, `clips/${c.id}.location.json`))), c);
    const info = probe(join(out, c.path)); const streams = selectStreams(info);
    assert.equal(streams.video.width, 1920); assert.equal(streams.video.height, 1080);
    assert(Math.abs(Number(info.format.duration) * 1000 - c.duration_ms) <= 50, `Clip duration: ${c.id}`);
    run(ffmpeg, ['-v', 'error', '-xerror', '-i', join(out, c.path), '-f', 'null', '-']);
    // Playback MP4 is lossy: compare actual pictures at both start and midpoint,
    // rather than trusting the sidecar's offset or the container duration alone.
    const clipFrameErrors = [0, c.duration_ms / 2].map(localMs => {
      const pixels = (path, stream, ms) => run(ffmpeg, ['-v', 'error', '-ss', String(ms / 1000),
        '-i', path, '-map', `0:${stream}`, '-frames:v', '1', '-pix_fmt', 'rgb24', '-f', 'rawvideo', 'pipe:1']).stdout;
      const primary = c.source_id === manifest.source.id;
      const expected = pixels(primary ? source : join(out, 'supplement.mp4'), primary ? video.index : 0, sourceTime(c, localMs));
      const actual = pixels(join(out, c.path), 0, localMs);
      assert.equal(actual.length, expected.length);
      let error = 0;
      for (let i = 0; i < actual.length; i++) error += Math.abs(actual[i] - expected[i]);
      const mean = error / actual.length;
      assert(mean < 3, `Clip frame differs at ${c.id}+${localMs}: ${mean}`);
      return { local_ms: localMs, source_ms: sourceTime(c, localMs), mean_absolute_pixel_error: mean };
    });
    let audioDurationMs = null;
    if (c.audio_path) {
      // Independent slow seek decodes from the beginning: catches wrong offsets / keyframe-only clipping.
      const original = audioPcm(source, audio.index, spec.start_ms, spec.end_ms, true);
      const wav = run(ffmpeg, ['-v', 'error', '-i', join(out, c.audio_path), '-f', 's16le', '-c:a', 'pcm_s16le', 'pipe:1']).stdout;
      audioDurationMs = wav.length / 4 / 44100 * 1000;
      // Container timestamps need not land on exact PCM sample boundaries.
      // Independent source PCM equality below remains exact, not tolerance-based.
      assert(Math.abs(audioDurationMs - c.duration_ms) < 1, `PCM duration: ${c.id}`);
      assert(original.equals(wav), `Audio offset or samples differ: ${c.id}`);
    }
    checks.push({ id: c.id, decoded: true, clip_frame_checks: clipFrameErrors, audio_duration_ms: audioDurationMs,
      source_audio_pcm_equal: c.audio_path ? true : null,
      midpoint_source_ms: sourceTime(c, c.duration_ms / 2) });
  }
  const verified = { checked_at: new Date().toISOString(), checks, status: 'passed',
    semantic_scope: 'Pixel/sample/timestamp fidelity only; visual annotations reviewed separately; audio words pending listening.' };
  json(join(out, 'verification.json'), verified); return verified;
}
if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const [command = 'replay', ...args] = process.argv.slice(2);
  const option = (name, fallback) => args.includes(name) ? args[args.indexOf(name) + 1] : fallback;
  const manifest = JSON.parse(readFileSync(join(here, 'samples.json')));
  const source = resolve(option('--source', process.env.VI0A_SOURCE || manifest.source.path));
  const out = resolve(option('--out', join(here, 'generated')));
  assert(['replay', 'verify'].includes(command), 'Usage: node evals/video/media.mjs replay|verify [--source PATH] [--out DIR]');
  const result = command === 'replay' ? replay(manifest, source, out) : verify(manifest, source, out);
  console.log(JSON.stringify({ command, out, frames: result.frames?.length, clips: result.clips?.length,
    checks: result.checks?.length, status: result.status || 'generated', elapsed_ms: result.elapsed_ms }));
}
