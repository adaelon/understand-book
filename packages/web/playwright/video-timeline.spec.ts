import { expect, test } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { importVideo } from '../../core/src/video-import';
import { extractVisualFrame } from '../../core/src/video-visual-scan';

test('real offset MP4 and aligned HLS keep subtitles and frozen frame references on the same timeline', async ({ page }, info) => {
  test.setTimeout(60000);
  const root = mkdtempSync(path.join(tmpdir(), 'video-browser-timeline-'));
  const observations: unknown[] = [];
  try {
    for (const offset of [5, 0]) {
      const input = path.join(root, `input-${offset}.mp4`), workspace = path.join(root, `workspace-${offset}`);
      execFileSync(process.env.FFMPEG || 'ffmpeg', ['-nostdin', '-v', 'error', '-f', 'lavfi', '-i', 'testsrc2=size=160x90:rate=10:duration=3',
        '-f', 'lavfi', '-i', 'sine=frequency=440:sample_rate=16000:duration=3', '-c:v', 'libx264', '-bf', offset ? '2' : '0',
        '-pix_fmt', 'yuv420p', '-c:a', 'aac', '-output_ts_offset', String(offset), input], { windowsHide: true });
      const imported = await importVideo(input, workspace), source = imported.source!;
      const probe = JSON.parse(readFileSync(path.join(workspace, 'media/probe.json'), 'utf8'));
      const origin = Number(probe.format.start_time) * 1000;
      const video = imported.video, transcript = { transcript_id: 't', transcript_revision: '1', video }, publication = { book_id: 'b', publication_id: 'p' };
      if (!offset) execFileSync(process.env.FFMPEG || 'ffmpeg', ['-nostdin', '-hide_banner', '-loglevel', 'error', '-copyts', '-start_at_zero', '-i', input,
        '-map', '0:V:0', '-map', '0:a:0?', '-c', 'copy', '-avoid_negative_ts', 'disabled', '-f', 'hls', '-hls_time', '5', '-hls_playlist_type', 'vod',
        '-hls_segment_type', 'fmp4', '-hls_flags', 'single_file+independent_segments', '-hls_segment_options', 'video_track_timescale=10240:movie_timescale=10240',
        '-hls_segment_filename', path.join(root, 'media.mp4'), path.join(root, 'index.m3u8')], { windowsHide: true });
      await page.route('**/timeline-media/**', async route => {
        const name = new URL(route.request().url()).pathname.split('/').at(-1)!;
        let body = readFileSync(name === 'media' ? input : path.join(root, name));
        const headers: Record<string, string> = { 'content-type': name.endsWith('.m3u8') ? 'application/vnd.apple.mpegurl' : 'video/mp4', 'accept-ranges': 'bytes' };
        const range = route.request().headers()['range'];
        if (range) {
          const [, startText, endText] = /bytes=(\d+)-(\d*)/.exec(range)!;
          const start = Number(startText), end = endText ? Number(endText) : body.length - 1;
          headers['content-range'] = `bytes ${start}-${end}/${body.length}`; body = body.subarray(start, end + 1);
        }
        await route.fulfill({ status: range ? 206 : 200, headers, body });
      });
      const published = { time_base: [1, 10240], timeline_origin_ms: origin, publication, material: { book_id: 'b', video, transcript }, source,
        map: { transcript, raw_result_path: 'raw.json', machine_transcription: { engine: 'fixture', model: 'fixture', language: 'en' }, segments: [
          { segment_id: 'middle', text: 'Middle of the original clip', location: { video, start_ms: 1000, end_ms: 2000 }, text_range: { lid: '1', range: { start: 0, end: 27 } }, words: [] },
        ] } };
      const props = { owner: 'timeline-test', embedded: true, mediaUrl: '/timeline-media/media', published,
        initial: { ...imported, duration_ms: source.duration_ms, timeline_origin_ms: origin, playable: true, qa_ready: false },
        playback: { ...published, hls: !offset } };
      await page.route('**/video-timeline-fixture', route => route.fulfill({ contentType: 'text/html', body: `<!doctype html><meta charset="utf-8"><div id="app"></div><script type="module">
        import {createApp, h, ref} from '/node_modules/.vite/deps/vue.js'; import Pane from '/src/components/VideoReaderPane.vue'; import '/src/style.css';
        createApp({setup(){const pane=ref();window.timelinePane=pane;return()=>h(Pane,{...${JSON.stringify(props)},ref:pane})}}).mount('#app');
      </script>` }));
      await page.goto('/video-timeline-fixture');
      await page.waitForFunction(() => (document.querySelector('video')?.readyState ?? 0) >= 2);
      const initial = await page.evaluate(async () => {
        const video = document.querySelector('video')!; video.muted = true;
        const seen = new Promise<number>(resolve => video.requestVideoFrameCallback((_, meta) => resolve(meta.mediaTime)));
        const initialCurrentTime = video.currentTime; await video.play(); const firstMediaTime = await seen; video.pause();
        return { initialCurrentTime, firstMediaTime, nativeDuration: video.duration };
      });
      if (offset) {
        await page.getByRole('button', { name: '播放', exact: true }).click();
        await expect.poll(() => page.locator('video').evaluate(video => (video as HTMLVideoElement).paused)).toBe(false);
        await page.getByRole('button', { name: '暂停', exact: true }).click();
        await page.getByRole('combobox', { name: '播放速度', exact: true }).selectOption('1.5');
        expect(await page.locator('video').evaluate(video => (video as HTMLVideoElement).playbackRate)).toBe(1.5);
        await page.getByRole('slider', { name: '音量', exact: true }).evaluate(input => {
          (input as HTMLInputElement).value = '0.5'; input.dispatchEvent(new Event('input', { bubbles: true }));
        });
        expect(await page.locator('video').evaluate(video => (video as HTMLVideoElement).volume)).toBe(.5);
        // Installed headless Chrome leaves requestFullscreen pending even on a
        // standalone button. Playback/seek controls are exercised here; native
        // fullscreen requires a windowed-browser acceptance run.
      }
      for (const logical of [1500, offset ? 3000 : 2900]) {
        if (offset) await page.getByRole('slider', { name: '观看位置', exact: true }).evaluate((input, ms) => {
          (input as HTMLInputElement).value = String(ms); input.dispatchEvent(new Event('input', { bubbles: true }));
        }, logical);
        else await page.evaluate(ms => (window as any).timelinePane.value.seek(ms), logical);
        await page.waitForFunction(() => !document.querySelector('video')!.seeking);
        await expect.poll(() => page.evaluate(() => {
          try { return (window as any).timelinePane.value.captureQuestion('browser').frame.actual_pts_ms; } catch { return null; }
        })).not.toBeNull();
        const captured = await page.evaluate(() => ({ raw: document.querySelector('video')!.currentTime, frame: (window as any).timelinePane.value.captureQuestion('browser').frame }));
        expect(captured.raw * 1000 - origin).toBeCloseTo(logical, 3);
        expect(captured.frame.actual_pts_ms).toBeGreaterThanOrEqual(logical - 101);
        expect(captured.frame.actual_pts_ms).toBeLessThanOrEqual(logical + .1);
        const decoded = await extractVisualFrame(input, source, captured.frame.actual_pts_ms, path.join(root, `read-${offset}-${logical}.png`));
        expect(decoded.frame).toEqual(captured.frame);
        if (logical === 1500) await expect(page.locator('[data-segment="middle"]')).toHaveAttribute('aria-current', 'true');
        observations.push({ offset, logical, origin, initial, captured });
      }
      if (offset) {
        await expect(page.getByRole('slider', { name: '观看位置', exact: true })).toBeVisible();
        await expect(page.locator('.relative-video-clock')).toHaveText('0:03 / 0:03');
        await page.screenshot({ path: info.outputPath('offset-relative-controls.png'), fullPage: true });
      }
      await page.unroute('**/timeline-media/**'); await page.unroute('**/video-timeline-fixture');
    }
    writeFileSync(info.outputPath('timeline-observations.json'), JSON.stringify(observations, null, 2));
    await info.attach('timeline-observations', { path: info.outputPath('timeline-observations.json'), contentType: 'application/json' });
  } finally { rmSync(root, { recursive: true, force: true }); }
});
