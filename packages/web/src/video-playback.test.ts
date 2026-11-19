// @vitest-environment happy-dom
import { afterEach, expect, it, vi } from 'vitest';
import { flushPromises } from '@vue/test-utils';
import { attachVideoPlayback } from './video-playback';
const state = vi.hoisted(() => ({ instances: [] as any[], supported: true }));
vi.mock('hls.js', () => ({ default: class {
  static isSupported = () => state.supported;
  static Events = { ERROR: 'error' };
  static ErrorTypes = { NETWORK_ERROR: 'networkError' };
  on = vi.fn(); loadSource = vi.fn(); attachMedia = vi.fn(); destroy = vi.fn(); stopLoad = vi.fn();
  constructor(public config: unknown) { state.instances.push(this); }
} }));
afterEach(() => { state.instances.length = 0; state.supported = true; vi.restoreAllMocks(); });
it('starts at the saved position, stops fatal requests, and destroys the attachment', async () => {
  const video = document.createElement('video'), fail = vi.fn();
  const dispose = attachVideoPlayback(video, '/original', '/index.m3u8', 2460, fail);
  await flushPromises();
  const hls = state.instances[0];
  expect(hls.config).toMatchObject({ startPosition: 2460, maxBufferLength: 15 });
  expect(hls.loadSource).toHaveBeenCalledWith('/index.m3u8');
  expect(hls.attachMedia).toHaveBeenCalledWith(video);
  hls.on.mock.calls[0][1]('error', { fatal: true, type: 'networkError' });
  expect(hls.stopLoad).toHaveBeenCalledOnce();
  expect(fail).toHaveBeenCalledOnce();
  expect(video.getAttribute('src')).toBeNull();
  dispose(); expect(hls.destroy).toHaveBeenCalledOnce();
});
it('does not attach after unmount while the player module is loading', async () => {
  const dispose = attachVideoPlayback(document.createElement('video'), '/original', '/index.m3u8', 0, vi.fn());
  dispose(); await flushPromises();
  expect(state.instances).toHaveLength(0);
});
it('uses native HLS when MSE is unavailable', async () => {
  state.supported = false;
  const video = document.createElement('video');
  vi.spyOn(video, 'canPlayType').mockReturnValue('probably');
  const dispose = attachVideoPlayback(video, '/original', '/index.m3u8', 0, vi.fn());
  await flushPromises();
  expect(video.getAttribute('src')).toBe('/index.m3u8');
  expect(state.instances).toHaveLength(0); dispose();
});
it('reports original timestamps when HLS cannot be used by the browser', async () => {
  state.supported = false;
  const video = document.createElement('video'), timeline = vi.fn();
  vi.spyOn(video, 'canPlayType').mockReturnValue('');
  const dispose = attachVideoPlayback(video, '/original', '/index.m3u8', 0, vi.fn(), timeline);
  await flushPromises();
  expect(video.getAttribute('src')).toBe('/original');
  expect(timeline).toHaveBeenCalledWith(true);
  dispose();
});
