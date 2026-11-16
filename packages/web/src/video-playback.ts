import { t as uiText } from './i18n';
import type Hls from 'hls.js';

export function preloadVideoPlayback() { return import('hls.js'); }

/** Owns one media attachment, including cancellation during the module import. */
export function attachVideoPlayback(video: HTMLVideoElement, source: string, playlist: string | null,
  startPosition: number, fail: (message: string) => void, timeline?: (original: boolean) => void): () => void {
  let disposed = false, hls: Hls | undefined;
  if (!playlist) { timeline?.(true); video.src = source; }
  else {
    void preloadVideoPlayback().then(({ default: Hls }) => {
      if (disposed) return;
      // Some Chromium versions advertise native HLS but fetch a single-file
      // playlist's entire MP4. Prefer explicit MSE byte ranges where supported.
      if (!Hls.isSupported()) {
        const native = !!video.canPlayType('application/vnd.apple.mpegurl');
        timeline?.(!native); video.src = native ? playlist : source; return;
      }
      timeline?.(false);
      hls = new Hls({ startPosition, maxBufferLength: 15, maxMaxBufferLength: 30, maxBufferSize: 6 * 1024 * 1024, backBufferLength: 30 });
      hls.on(Hls.Events.ERROR, (_event, data) => {
        if (!disposed && data.fatal) {
          hls?.stopLoad();
          fail(data.type === Hls.ErrorTypes.NETWORK_ERROR ? uiText('视频片段暂时加载失败，请检查网络后重试。') : uiText('视频播放中断，请重试加载。'));
        }
      });
      hls.loadSource(playlist);
      hls.attachMedia(video);
    }).catch(() => { if (!disposed) fail(uiText('播放器未能加载，请检查网络后重试。')); });
  }
  return () => {
    disposed = true;
    hls?.destroy();
    video.pause();
    video.removeAttribute('src');
    video.load();
  };
}
