import { t as uiText } from './i18n';

/** Keep one recording attached; a dead range request must not leave play() pending forever. */
export function attachMusicAudio(audio: HTMLAudioElement, state: {
  loaded(reconnected: boolean): void;
  buffering(active: boolean): void;
  error(message: string): void;
}) {
  let disposed = false, wanted = false, retries = 0, attempt = 0;
  let timer: ReturnType<typeof setInterval> | undefined;
  let lastTime = audio.currentTime, lastProgress = performance.now();
  let restore: { time: number; rate: number } | null = null;
  let hadMetadata = audio.readyState > 0;
  let pending: { promise: Promise<void>; resolve(): void; reject(reason: unknown): void } | undefined;
  const events: [string, EventListener][] = [];
  function listen(name: string, handler: () => void) { audio.addEventListener(name, handler); events.push([name, handler]); }
  function stopTimer() { clearInterval(timer); timer = undefined; }
  function settle(failure?: unknown) {
    const current = pending; pending = undefined;
    if (failure) current?.reject(failure); else current?.resolve();
  }
  function fail(message: string) {
    wanted = false; attempt++; stopTimer();
    settle(new Error(message)); audio.pause(); state.buffering(false); state.error(message);
  }
  function startAttempt() {
    const current = ++attempt;
    void audio.play().then(() => {
      if (!disposed && current === attempt) settle();
    }, failure => {
      if (disposed || current !== attempt) return;
      // A real listener pause cancels the outstanding play, including during a reload.
      if (!wanted || audio.paused && failure?.name === 'AbortError') {
        wanted = false; stopTimer(); state.buffering(false); settle(failure);
      } else if (audio.error && retries === 0) reconnect();
      else fail(uiText('播放未开始：{0}。请点播放重试。', [failure.message]));
    });
  }
  function reconnect() {
    if (disposed || !wanted) return;
    if (retries >= 1) { fail(uiText('录音连接中断，请检查网络后点播放重试。')); return; }
    retries++;
    restore = { time: audio.currentTime, rate: audio.playbackRate };
    attempt++; // load() rejects the previous play promise; its caller waits for this attempt.
    audio.load();
    audio.currentTime = restore.time;
    audio.playbackRate = restore.rate;
    lastTime = restore.time; lastProgress = performance.now();
    state.buffering(true);
    startAttempt();
  }
  function checkProgress() {
    if (disposed || !wanted) return;
    if (!audio.seeking && audio.currentTime !== lastTime) {
      lastTime = audio.currentTime; lastProgress = performance.now(); retries = 0;
    } else if (performance.now() - lastProgress >= 8000) reconnect();
  }
  function watchProgress() {
    if (timer === undefined) timer = setInterval(checkProgress, 1000);
  }
  function cancel() {
    wanted = false; attempt++; stopTimer(); state.buffering(false);
    settle(new DOMException('Playback paused', 'AbortError'));
  }
  listen('loadedmetadata', () => {
    const saved = restore, reconnected = !!saved && hadMetadata; restore = null; hadMetadata = true;
    if (saved) { audio.currentTime = saved.time; audio.playbackRate = saved.rate; }
    state.loaded(reconnected);
  });
  listen('play', () => { wanted = true; watchProgress(); });
  listen('pause', () => { if (audio.paused) cancel(); });
  listen('ended', cancel);
  listen('waiting', () => { if (wanted) state.buffering(true); });
  listen('stalled', () => { if (wanted && audio.readyState < 3) state.buffering(true); });
  listen('playing', () => { state.buffering(false); state.error(''); });
  listen('timeupdate', () => {
    // Short loops can return to the same position between watchdog samples.
    if (wanted && !audio.paused && audio.readyState >= 3) {
      lastTime = audio.currentTime; lastProgress = performance.now(); retries = 0;
    }
  });
  listen('error', () => {
    if (wanted) reconnect();
    else { state.buffering(false); state.error(uiText('录音连接中断，请检查网络后点播放重试。')); }
  });
  function resume() { if (document.visibilityState === 'visible') checkProgress(); }
  document.addEventListener('visibilitychange', resume);
  window.addEventListener('online', checkProgress);
  return {
    play(): Promise<void> {
      if (pending) return pending.promise;
      let resolve!: () => void, reject!: (reason: unknown) => void;
      const promise = new Promise<void>((done, failed) => { resolve = done; reject = failed; });
      pending = { promise, resolve, reject };
      wanted = true; retries = 0; lastTime = audio.currentTime; lastProgress = performance.now();
      state.error(''); watchProgress();
      if (audio.error) reconnect(); else startAttempt();
      return promise;
    },
    get restoring() { return restore !== null; },
    reset() { cancel(); restore = null; hadMetadata = false; },
    dispose() {
      disposed = true; cancel();
      for (const [name, handler] of events) audio.removeEventListener(name, handler);
      document.removeEventListener('visibilitychange', resume);
      window.removeEventListener('online', checkProgress);
    },
  };
}
