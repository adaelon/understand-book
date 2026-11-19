// @vitest-environment happy-dom
import { afterEach, expect, it, vi } from 'vitest';
import { attachMusicAudio } from './music-audio';

const players: ReturnType<typeof attachMusicAudio>[] = [];
afterEach(() => { for (const player of players.splice(0)) player.dispose(); vi.useRealTimers(); vi.restoreAllMocks(); });

function recording(initialReady = 2) {
  vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval', 'performance'] });
  const audio = document.createElement('audio');
  let paused = true, ready = initialReady, error: MediaError | null = null;
  let requests: { resolve(): void; reject(reason: unknown): void }[] = [];
  Object.defineProperties(audio, {
    paused: { get: () => paused }, readyState: { get: () => ready }, error: { get: () => error },
  });
  vi.spyOn(audio, 'play').mockImplementation(() => {
    paused = false; audio.dispatchEvent(new Event('play'));
    return new Promise<void>((resolve, reject) => { requests.push({ resolve, reject }); });
  });
  const load = vi.spyOn(audio, 'load').mockImplementation(() => {
    for (const request of requests.splice(0)) request.reject(new DOMException('load interrupted play', 'AbortError'));
    paused = true; ready = 0; error = null; audio.currentTime = 0; audio.playbackRate = 1;
  });
  vi.spyOn(audio, 'pause').mockImplementation(() => {
    paused = true; audio.dispatchEvent(new Event('pause'));
    for (const request of requests.splice(0)) request.reject(new DOMException('paused', 'AbortError'));
  });
  const state = { loaded: vi.fn(), buffering: vi.fn(), error: vi.fn() };
  const player = attachMusicAudio(audio, state); players.push(player);
  return { audio, player, state, load,
    metadata() { ready = 2; audio.dispatchEvent(new Event('loadedmetadata')); },
    playing() { ready = 4; audio.dispatchEvent(new Event('playing')); for (const request of requests.splice(0)) request.resolve(); },
    networkError() { error = { code: 2 } as MediaError; audio.dispatchEvent(new Event('error')); },
  };
}

it('reconnects a pending range request after eight seconds and resolves the original play at the unsaved position', async () => {
  const { audio, player, state, load, metadata, playing } = recording();
  audio.currentTime = 28.4; audio.playbackRate = .75;
  let finished = false;
  const result = player.play().then(() => { finished = true; });
  audio.dispatchEvent(new Event('waiting'));
  vi.advanceTimersByTime(7000); expect(load).not.toHaveBeenCalled();
  vi.advanceTimersByTime(1000); await Promise.resolve();
  expect(load).toHaveBeenCalledOnce(); expect(finished).toBe(false);
  expect(player.restoring).toBe(true);
  metadata();
  expect(audio.currentTime).toBe(28.4); expect(audio.playbackRate).toBe(.75);
  expect(state.loaded).toHaveBeenCalledWith(true);
  playing(); await result;
  expect(state.buffering).toHaveBeenLastCalledWith(false);
  expect(state.error).toHaveBeenLastCalledWith('');
});

it('recovers a connection that stalls after playback already started', async () => {
  const { audio, player, load, metadata, playing } = recording();
  const started = player.play(); playing(); await started;
  audio.currentTime = 12.4; vi.advanceTimersByTime(1000);
  audio.dispatchEvent(new Event('waiting'));
  vi.advanceTimersByTime(8000);
  expect(load).toHaveBeenCalledOnce(); metadata(); playing();
  expect(audio.currentTime).toBe(12.4);
});

it('ends an unsuccessful reconnect with a retry message instead of buffering forever', async () => {
  const { audio, player, load, state } = recording();
  const failed = expect(player.play()).rejects.toThrow('点播放重试');
  vi.advanceTimersByTime(16000); await failed;
  expect(load).toHaveBeenCalledOnce(); expect(audio.paused).toBe(true);
  expect(state.buffering).toHaveBeenLastCalledWith(false);
  expect(state.error).toHaveBeenLastCalledWith(expect.stringContaining('点播放重试'));
  vi.advanceTimersByTime(60000); expect(load).toHaveBeenCalledOnce();
});

it('cancels a pending reconnect when the listener pauses, without resuming on visibility or online events', async () => {
  const { audio, player, load } = recording();
  const failed = expect(player.play()).rejects.toMatchObject({ name: 'AbortError' });
  vi.advanceTimersByTime(8000); audio.pause(); await failed;
  vi.advanceTimersByTime(30000);
  document.dispatchEvent(new Event('visibilitychange')); window.dispatchEvent(new Event('online'));
  expect(load).toHaveBeenCalledOnce(); expect(audio.play).toHaveBeenCalledTimes(2);
});

it('reloads a failed recording when the listener presses play again', async () => {
  const { audio, player, load, metadata, playing, networkError } = recording();
  audio.currentTime = 28.4;
  networkError(); expect(load).not.toHaveBeenCalled();
  const result = player.play(); expect(load).toHaveBeenCalledOnce();
  metadata(); playing(); await result;
  expect(audio.currentTime).toBe(28.4);
});

it('immediately retries a network error during playback and stops if that attempt also fails', async () => {
  const { player, load, playing, networkError, state } = recording();
  const started = player.play(); playing(); await started;
  networkError(); expect(load).toHaveBeenCalledOnce();
  networkError(); expect(load).toHaveBeenCalledOnce();
  expect(state.error).toHaveBeenLastCalledWith(expect.stringContaining('点播放重试'));
});

it('checks an expired background request when the page becomes visible again', async () => {
  const { player, load, playing } = recording();
  const started = player.play(); playing(); await started;
  vi.spyOn(performance, 'now').mockReturnValue(20000);
  Object.defineProperty(document, 'visibilityState', { configurable: true, value: 'visible' });
  document.dispatchEvent(new Event('visibilitychange'));
  expect(load).toHaveBeenCalledOnce();
});

it('keeps healthy playback attached and resets recovery when the material changes', async () => {
  const { audio, player, load, playing } = recording();
  const started = player.play(); playing(); await started;
  for (let i = 0; i < 20; i++) { audio.currentTime++; vi.advanceTimersByTime(1000); }
  expect(load).not.toHaveBeenCalled();
  player.reset(); vi.advanceTimersByTime(30000);
  expect(load).not.toHaveBeenCalled();
});

it('does not reconnect a healthy short loop that returns to the same position each second', async () => {
  const { audio, player, load, playing } = recording();
  const started = player.play(); playing(); await started;
  for (let i = 0; i < 40; i++) {
    audio.dispatchEvent(new Event('timeupdate'));
    vi.advanceTimersByTime(250);
  }
  expect(load).not.toHaveBeenCalled();
});

it('restores the saved opening when the first metadata arrives during a reconnect', async () => {
  const { player, state, metadata, playing } = recording(0);
  const result = player.play();
  vi.advanceTimersByTime(8000);
  metadata(); playing(); await result;
  expect(state.loaded).toHaveBeenCalledWith(false);
});
