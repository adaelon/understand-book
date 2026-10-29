import { useEffect } from 'react';
import { session } from './api';

export function useAdminPresence(token?: string) {
  useEffect(() => {
    if (!token) return;
    let pending = false;
    const controller = new AbortController();
    const heartbeat = async () => {
      if (pending || document.visibilityState === 'hidden' || controller.signal.aborted) return;
      pending = true;
      try {
        await session.request('/account/presence', 'POST', {}, AbortSignal.any([controller.signal, AbortSignal.timeout(10_000)]));
      } catch {
        // The next interval retries without interrupting management work.
      } finally { pending = false; }
    };
    const visible = () => { void heartbeat(); };
    const timer = setInterval(visible, 30_000);
    document.addEventListener('visibilitychange', visible);
    window.addEventListener('online', visible);
    void heartbeat();
    return () => {
      controller.abort(); clearInterval(timer);
      document.removeEventListener('visibilitychange', visible);
      window.removeEventListener('online', visible);
    };
  }, [token]);
}
