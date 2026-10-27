import { t as uiText } from './i18n';
import { ref } from 'vue';
import type { VideoQuestionContext } from '../../core/src/generated/VideoQuestionContext';
export type VideoDraft = { message: string; context: VideoQuestionContext };
export type DraftCommand = { window_id: string; action: 'get' | 'save' | 'release'; draft?: VideoDraft; draft_id?: string };
export function videoWindowId() {
  let id = sessionStorage.getItem('video-question-window');
  if (!id) { id = crypto.randomUUID(); sessionStorage.setItem('video-question-window', id); }
  return id;
}
/** Actual input owns binding. Playback changes never enter this state machine. */
export function createVideoQuestion(windowId: string, command: (input: DraftCommand) => Promise<{ draft: VideoDraft | null }>, capture: (id: string) => VideoQuestionContext) {
  const draft = ref<VideoDraft | null>(null), error = ref(''), restoring = ref(false);
  let writes = Promise.resolve(), revision = 0;
  let queuedSave: { input: DraftCommand; version: number } | null = null;
  function enqueue(input: DraftCommand) {
    // Only the latest edit matters while a save is waiting. Release remains an
    // ordering barrier so cancelling and binding another frame cannot be merged.
    if (input.action === 'save' && queuedSave) {
      queuedSave.input = input;
      queuedSave.version = revision;
      return;
    }
    const queued = { input, version: revision };
    queuedSave = input.action === 'save' ? queued : null;
    writes = writes.catch(() => {}).then(async () => {
      if (queuedSave === queued) queuedSave = null;
      await command(queued.input);
      if (queued.version === revision) error.value = '';
    }).catch(failure => {
      if (queued.version === revision) error.value = (failure as Error).message;
      throw failure;
    });
    void writes.catch(() => {});
  }
  async function restore() {
    const version = revision; restoring.value = true;
    try {
      await writes.catch(() => {});
      const result = await command({ window_id: windowId, action: 'get' });
      if (revision === version) { draft.value = result.draft; error.value = ''; }
    } catch (failure) { if (revision === version) error.value = (failure as Error).message; }
    finally { restoring.value = false; }
  }
  function input(message: string) {
    revision++;
    if (!message && !draft.value?.context.region && !draft.value?.context.range && !draft.value?.context.image) { clear(); return; }
    try {
      if (!draft.value) draft.value = { message, context: capture(crypto.randomUUID()) };
      else draft.value.message = message;
      enqueue({ window_id: windowId, action: 'save', draft: structuredClone({ message, context: JSON.parse(JSON.stringify(draft.value.context)) }) });
    } catch (failure) { error.value = (failure as Error).message; }
  }
  function clear() {
    revision++;
    const id = draft.value?.context.draft_id;
    draft.value = null;
    if (id) enqueue({ window_id: windowId, action: 'release', draft_id: id });
  }
  function selectRegion(context: VideoQuestionContext) {
    revision++;
    const current = draft.value?.context;
    if (current && (current.draft_id !== context.draft_id || JSON.stringify(current.frame) !== JSON.stringify(context.frame))) {
      error.value = uiText('请回到问题绑定画面再框选'); return;
    }
    if (!context.region) return;
    if (!draft.value) draft.value = { message: '', context: JSON.parse(JSON.stringify(context)) };
    else {
      draft.value.context.region = JSON.parse(JSON.stringify(context.region));
      if (context.image) draft.value.context.image = JSON.parse(JSON.stringify(context.image));
      else delete draft.value.context.image;
    }
    input(draft.value.message);
  }
  function clearRegion() {
    if (!draft.value) return;
    draft.value.context.region = null;
    if (draft.value.context.image?.origin === 'selection') draft.value.context.image = null;
    input(draft.value.message);
  }
  function setImage(image: NonNullable<VideoQuestionContext['image']> | null, context?: VideoQuestionContext) {
    if (!draft.value && image) draft.value = { message: '', context: context ?? capture(crypto.randomUUID()) };
    if (!draft.value) return;
    // Removing a screenshot also removes its region so the next send won't silently
    // revert to the old tool-only selection. Uploaded pictures are independent.
    if (!image && draft.value.context.image?.origin === 'selection') draft.value.context.region = null;
    draft.value.context.image = image;
    input(draft.value.message);
  }
  function selectRange(context: VideoQuestionContext) {
    const current = draft.value?.context;
    if (current && (current.draft_id !== context.draft_id || JSON.stringify(current.frame) !== JSON.stringify(context.frame))) {
      error.value = uiText('问题绑定已改变，请重新选择时间范围'); return;
    }
    if (!context.range) return;
    if (!draft.value) draft.value = { message: '', context: JSON.parse(JSON.stringify(context)) };
    else draft.value.context.range = JSON.parse(JSON.stringify(context.range));
    input(draft.value.message);
  }
  function clearRange() {
    if (!draft.value) return;
    draft.value.context.range = null;
    input(draft.value.message);
  }
  async function submission() {
    // A second click after a failed save is an explicit retry of the same draft.
    if (error.value && draft.value) input(draft.value.message);
    await writes;
    if (!draft.value) throw new Error(uiText('请先绑定当前视频画面'));
    return { video_window_id: windowId, video_draft_id: draft.value.context.draft_id };
  }
  function accepted(id: string) {
    if (draft.value?.context.draft_id !== id) return;
    // Server's accepted turn is authoritative even when the creation response was lost.
    revision++; draft.value = null; error.value = '';
  }
  return { draft, error, restoring, input, restore, clear, selectRegion, clearRegion, selectRange, clearRange, setImage, submission, accepted };
}
export function videoTime(ms: number) {
  const seconds = Math.round(ms / 1000);
  return `${String(Math.floor(seconds / 60)).padStart(2, '0')}:${String(seconds % 60).padStart(2, '0')}`;
}
