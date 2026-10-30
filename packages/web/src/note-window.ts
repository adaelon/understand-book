import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';

export interface NoteWindowFrame { x: number; y: number; width: number; height: number }
export interface NoteWindowViewport { x: number; y: number; width: number; height: number }
export type NoteWindowAction = 'move' | 'n' | 's' | 'e' | 'w' | 'ne' | 'nw' | 'se' | 'sw';

const gutter = 12;
const clamp = (value: number, min: number, max: number) => Math.min(Math.max(value, min), max);
function bounds(viewport: NoteWindowViewport) {
  return { left: viewport.x + gutter, top: viewport.y + gutter,
    right: viewport.x + viewport.width - gutter, bottom: viewport.y + viewport.height - gutter,
    width: Math.max(1, viewport.width - gutter * 2), height: Math.max(1, viewport.height - gutter * 2) };
}

export function fitNoteWindow(frame: NoteWindowFrame, viewport: NoteWindowViewport): NoteWindowFrame {
  const area = bounds(viewport);
  const width = clamp(frame.width, Math.min(300, area.width), area.width);
  const height = clamp(frame.height, Math.min(280, area.height), area.height);
  return { x: clamp(frame.x, area.left, area.right - width), y: clamp(frame.y, area.top, area.bottom - height), width, height };
}

export function defaultNoteWindow(viewport: NoteWindowViewport): NoteWindowFrame {
  const height = viewport.width < 600 ? Math.min(360, viewport.height * .48) : 360;
  return fitNoteWindow({ x: viewport.x + viewport.width - 424, y: viewport.y + viewport.height - height - 24, width: 400, height }, viewport);
}

export function changeNoteWindow(start: NoteWindowFrame, action: NoteWindowAction, dx: number, dy: number, viewport: NoteWindowViewport): NoteWindowFrame {
  if (action === 'move') return fitNoteWindow({ ...start, x: start.x + dx, y: start.y + dy }, viewport);
  const area = bounds(viewport), minWidth = Math.min(300, area.width), minHeight = Math.min(280, area.height);
  let left = start.x, top = start.y, right = left + start.width, bottom = top + start.height;
  if (action.includes('w')) left = clamp(left + dx, area.left, right - minWidth);
  if (action.includes('e')) right = clamp(right + dx, left + minWidth, area.right);
  if (action.includes('n')) top = clamp(top + dy, area.top, bottom - minHeight);
  if (action.includes('s')) bottom = clamp(bottom + dy, top + minHeight, area.bottom);
  return fitNoteWindow({ x: left, y: top, width: right - left, height: bottom - top }, viewport);
}

/** Keep the preferred frame when a keyboard temporarily shrinks the visible viewport. */
export function useNoteWindow(owner: () => string | null) {
  const measure = (): NoteWindowViewport => {
    const visual = window.visualViewport;
    return { x: visual?.offsetLeft ?? 0, y: visual?.offsetTop ?? 0, width: visual?.width || window.innerWidth, height: visual?.height || window.innerHeight };
  };
  const viewport = ref(measure()), preferred = ref<NoteWindowFrame | null>(null), interacting = ref(false);
  const frame = computed(() => preferred.value ? fitNoteWindow(preferred.value, viewport.value) : defaultNoteWindow(viewport.value));
  const style = computed(() => ({ left: `${frame.value.x}px`, top: `${frame.value.y}px`, width: `${frame.value.width}px`, height: `${frame.value.height}px` }));
  let gesture: { action: NoteWindowAction; pointerId: number; x: number; y: number; frame: NoteWindowFrame; target: HTMLElement } | null = null;
  const storageKey = () => `understand-book:note-window:${owner()}`;
  function persist() {
    if (!owner() || !preferred.value) return;
    try { localStorage.setItem(storageKey(), JSON.stringify(preferred.value)); } catch { /* Position is optional; drafts have separate persistence. */ }
  }
  function finish(save = true) {
    if (!gesture) return;
    const { target, pointerId } = gesture;
    gesture = null; interacting.value = false;
    if (target.hasPointerCapture(pointerId)) target.releasePointerCapture(pointerId);
    if (save) persist();
  }
  function begin(event: PointerEvent, action: NoteWindowAction) {
    if (event.button !== 0 || !event.isPrimary) return;
    if (action === 'move' && (event.target as Element).closest('button, input, textarea, a')) return;
    event.preventDefault();
    const target = event.currentTarget as HTMLElement;
    target.setPointerCapture(event.pointerId);
    gesture = { action, pointerId: event.pointerId, x: event.clientX, y: event.clientY, frame: { ...frame.value }, target };
    interacting.value = true;
  }
  function move(event: PointerEvent) {
    if (!gesture || gesture.pointerId !== event.pointerId) return;
    preferred.value = changeNoteWindow(gesture.frame, gesture.action, event.clientX - gesture.x, event.clientY - gesture.y, viewport.value);
  }
  function end(event: PointerEvent) { if (event.pointerId === gesture?.pointerId) finish(); }
  function keyboard(event: KeyboardEvent, action: 'move' | 'se') {
    if (event.target !== event.currentTarget || !['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(event.key)) return;
    event.preventDefault(); event.stopPropagation();
    const step = event.shiftKey ? 40 : 10;
    preferred.value = changeNoteWindow(frame.value, action, event.key === 'ArrowLeft' ? -step : event.key === 'ArrowRight' ? step : 0,
      event.key === 'ArrowUp' ? -step : event.key === 'ArrowDown' ? step : 0, viewport.value);
    persist();
  }
  function updateViewport() { viewport.value = measure(); }
  watch(owner, () => {
    finish(false); preferred.value = null;
    if (!owner()) return;
    try {
      const saved = JSON.parse(localStorage.getItem(storageKey()) ?? 'null');
      if (saved && ['x', 'y', 'width', 'height'].every(key => typeof saved[key] === 'number' && Number.isFinite(saved[key]))) preferred.value = saved;
    } catch { /* Start at the default position when no usable position was stored. */ }
  }, { immediate: true });
  onMounted(() => {
    window.addEventListener('resize', updateViewport);
    window.visualViewport?.addEventListener('resize', updateViewport);
    window.visualViewport?.addEventListener('scroll', updateViewport);
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', end);
    window.addEventListener('pointercancel', end);
  });
  onBeforeUnmount(() => {
    finish();
    window.removeEventListener('resize', updateViewport);
    window.visualViewport?.removeEventListener('resize', updateViewport);
    window.visualViewport?.removeEventListener('scroll', updateViewport);
    window.removeEventListener('pointermove', move);
    window.removeEventListener('pointerup', end);
    window.removeEventListener('pointercancel', end);
  });
  return { style, interacting, begin, keyboard, finish };
}
