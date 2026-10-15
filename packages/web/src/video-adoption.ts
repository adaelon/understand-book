import { ref } from 'vue';
import type { PublishedVideo } from './video-api';
export type VideoAdoption = { status: 'busy' | 'current' | 'adopted'; video?: Omit<PublishedVideo, 'time_base'> };

/** Only one gap request at a time. The server rechecks draft/admission state
 * under the same workspace lock used by submission. */
export function createVideoAdoption(idle: () => boolean, request: () => Promise<VideoAdoption>, apply: (video: Omit<PublishedVideo, 'time_base'>) => void) {
  const adopting = ref(false), error = ref('');
  let disposed = false;
  async function refresh() {
    if (disposed || adopting.value || !idle()) return;
    adopting.value = true;
    try {
      const result = await request();
      if (!disposed && result.video) apply(result.video);
      error.value = '';
    } catch (failure) { if (!disposed) error.value = (failure as Error).message; }
    finally { adopting.value = false; }
  }
  return { adopting, error, refresh, dispose: () => { disposed = true; } };
}
