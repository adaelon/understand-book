import { watch } from 'vue';
import { network } from './network-context';
import { networkFetch } from './network-client';

/** Presence describes a visible signed-in page, independent of its current book or chat. */
export function useAccountPresence(enabled: () => boolean = () => true) {
  watch(() => enabled() ? network.value.identity?.csrf_token : undefined, (token, _previous, cleanup) => {
    if (!token) return;
    let pending = false;
    const controller = new AbortController();
    const heartbeat = async () => {
      if (pending || document.visibilityState === 'hidden' || controller.signal.aborted) return;
      pending = true;
      try {
        await networkFetch('POST', '/account/presence', {}, AbortSignal.any([controller.signal, AbortSignal.timeout(10_000)]));
      } catch {
        // The next heartbeat retries; presence failures do not interrupt reading.
      } finally { pending = false; }
    };
    const visible = () => { void heartbeat(); };
    const timer = setInterval(visible, 30_000);
    document.addEventListener('visibilitychange', visible);
    window.addEventListener('online', visible);
    void heartbeat();
    cleanup(() => {
      controller.abort(); clearInterval(timer);
      document.removeEventListener('visibilitychange', visible);
      window.removeEventListener('online', visible);
    });
  }, { immediate: true });
}
