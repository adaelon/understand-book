import { computed, onBeforeUnmount, onMounted, ref, watch, type CSSProperties, type Ref } from 'vue';
import type { NoteWindowViewport } from './note-window';
import { WORKSPACE_LIMITS } from './workspace-layout';

export interface LauncherPoint { x: number; y: number }
export interface LauncherRect extends LauncherPoint { width: number; height: number }
const gap = 12, edge = 16;
const clamp = (value: number, low: number, high: number) => Math.max(low, Math.min(value, high));
const overlaps = (a: LauncherRect, b: LauncherRect) => a.x < b.x + b.width + gap && a.x + a.width + gap > b.x
  && a.y < b.y + b.height + gap && a.y + a.height + gap > b.y;

/** Find the nearest free space; temporary avoidance never changes the saved point. */
export function placeNoteLauncher(viewport: NoteWindowViewport, size: { width: number; height: number }, avoid: LauncherRect[], preferred?: LauncherPoint | null): LauncherPoint | null {
  const left = viewport.x + edge, top = viewport.y + edge;
  const right = Math.max(left, viewport.x + viewport.width - size.width - edge);
  const bottom = Math.max(top, viewport.y + viewport.height - size.height - edge);
  const fit = (point: LauncherPoint) => ({ x: clamp(point.x, left, right), y: clamp(point.y, top, bottom) });
  const wanted = fit(preferred ?? { x: right, y: bottom });
  if (!avoid.some(rect => overlaps({ ...wanted, ...size }, rect))) return wanted;
  // At each free horizontal edge, merge blocked x intervals and find the closest
  // free x. This also finds diagonal gaps between adjacent controls and composers.
  const rows = new Set([wanted.y, top, bottom, ...avoid.flatMap(rect => [
    clamp(rect.y - size.height - gap, top, bottom), clamp(rect.y + rect.height + gap, top, bottom),
  ])]);
  let best: LauncherPoint | null = null, distance = Infinity;
  for (const y of rows) {
    const intervals = avoid.filter(rect => y < rect.y + rect.height + gap && y + size.height + gap > rect.y)
      .map(rect => [rect.x - size.width - gap, rect.x + rect.width + gap])
      .sort((a, b) => a[0] - b[0]);
    const merged: number[][] = [];
    for (const interval of intervals) {
      const previous = merged.at(-1);
      if (previous && interval[0] < previous[1]) previous[1] = Math.max(previous[1], interval[1]);
      else merged.push(interval);
    }
    const blocked = merged.find(([start, end]) => wanted.x > start && wanted.x < end);
    for (const x of blocked ?? [wanted.x]) {
      if (x < left || x > right) continue;
      const delta = (x - wanted.x) ** 2 + (y - wanted.y) ** 2;
      if (delta < distance) { best = { x, y }; distance = delta; }
    }
  }
  return best;
}

// Playback cursors and fixed clock labels do not change control geometry. Their
// frame-by-frame mutations must not trigger SVG hit testing for the notes button.
// ResizeObserver still handles size changes; section/control changes remain observed.
export function noteLauncherLayoutMutation(record: MutationRecord): boolean {
  const target = record.target instanceof Element ? record.target : record.target.parentElement;
  if (!target || target.closest('[data-personal-notes]')) return false;
  if (record.type === 'attributes' && record.attributeName === 'style'
    && target.matches('.map-cursor, .map-overview-track > i, .music-progress > input, .music-score-page .note, .music-note-overlay, .music-note-overlay *')) return false;
  if ((record.type === 'characterData' || (record.type === 'childList'
    && [...record.addedNodes, ...record.removedNodes].every(node => node.nodeType === Node.TEXT_NODE)))
    && target.closest('.map-clock, .music-progress > span')) return false;
  return true;
}

export function useNoteLauncher(element: Ref<HTMLElement | null>, owner: () => string | null, visible: () => boolean) {
  const measureViewport = (): NoteWindowViewport => {
    const visual = window.visualViewport;
    return { x: visual?.offsetLeft ?? 0, y: visual?.offsetTop ?? 0,
      width: visual?.width || window.innerWidth, height: visual?.height || window.innerHeight };
  };
  const viewport = ref(measureViewport()), size = ref({ width: 104, height: 44 });
  const keyboardOpen = ref(false);
  const avoid = ref<LauncherRect[]>([]), preferred = ref<LauncherPoint | null>(null), dragging = ref(false);
  const placement = computed(() => placeNoteLauncher(viewport.value, size.value, dragging.value ? [] : avoid.value, preferred.value));
  const position = computed(() => placement.value ?? placeNoteLauncher(viewport.value, size.value, [], preferred.value)!);
  const style = computed<CSSProperties>(() => ({ left: `${position.value.x}px`, top: `${position.value.y}px`, visibility: placement.value ? undefined : 'hidden' }));
  let frame = 0, observer: MutationObserver | undefined, resize: ResizeObserver | undefined;
  let targets: Element[] = [], suppressClick = false;
  let gesture: { id: number; x: number; y: number; start: LauncherPoint; prior: LauncherPoint | null; target: HTMLElement } | null = null;
  const key = () => `understand-book:note-launcher:${owner()}`;
  function persist() {
    if (!owner() || !preferred.value) return;
    try { localStorage.setItem(key(), JSON.stringify(preferred.value)); } catch { /* Recording remains available without position storage. */ }
  }
  function refresh() {
    frame = 0;
    const visual = window.visualViewport;
    // Keyboard space is temporary. Account for zoom so pinching alone cannot hide the entry.
    keyboardOpen.value = window.innerWidth < WORKSPACE_LIMITS.wideMinWidth && !!visual
      && window.innerHeight - visual.height * visual.scale >= WORKSPACE_LIMITS.keyboardHeightLoss;
    if (!visible()) return;
    viewport.value = measureViewport();
    if (element.value) {
      const rect = element.value.getBoundingClientRect();
      if (rect.width && rect.height) size.value = { width: rect.width, height: rect.height };
    }
    const regions = '.agent-input, .board-conversation, .personal-chat-composer, .workspace-mobile-nav, .music-example-piano';
    const controls = 'button, a[href], input, textarea, select, summary, [role="button"], [role="tab"], [contenteditable="true"]';
    const found = Array.from(document.querySelectorAll(`${regions}, ${controls}`)).filter(target =>
      !target.closest('[data-personal-notes]') && !target.parentElement?.closest(regions));
    if (found.length !== targets.length || found.some((target, i) => target !== targets[i])) {
      targets.forEach(target => resize?.unobserve(target));
      targets = found; targets.forEach(target => resize?.observe(target));
    }
    const view = viewport.value;
    avoid.value = targets.flatMap(target => {
      const rect = target.getBoundingClientRect();
      if (!rect.width || !rect.height || getComputedStyle(target).visibility === 'hidden'
        || rect.bottom <= view.y || rect.top >= view.y + view.height) return [];
      // Ignore controls behind another surface or clipped out of a scrolling pane.
      const hit = document.elementsFromPoint(clamp(rect.x + rect.width / 2, view.x, view.x + view.width - 1),
        clamp(rect.y + rect.height / 2, view.y, view.y + view.height - 1))
        .find(node => !node.closest('[data-personal-notes]'));
      return hit && target.contains(hit) ? [{ x: rect.x, y: rect.y, width: rect.width, height: rect.height }] : [];
    });
  }
  function schedule() { if (!frame) frame = requestAnimationFrame(refresh); }
  function begin(event: PointerEvent) {
    if (event.button !== 0 || !event.isPrimary) return;
    suppressClick = false;
    gesture = { id: event.pointerId, x: event.clientX, y: event.clientY, start: { ...position.value },
      prior: preferred.value, target: event.currentTarget as HTMLElement };
  }
  function move(event: PointerEvent) {
    if (!gesture || event.pointerId !== gesture.id) return;
    const dx = event.clientX - gesture.x, dy = event.clientY - gesture.y;
    if (!dragging.value && Math.hypot(dx, dy) < 6) return;
    event.preventDefault();
    if (!dragging.value) gesture.target.setPointerCapture(event.pointerId);
    dragging.value = true; suppressClick = true;
    preferred.value = { x: gesture.start.x + dx, y: gesture.start.y + dy };
  }
  function finish(save = true) {
    if (!gesture) return;
    const current = gesture; gesture = null;
    if (current.target.hasPointerCapture(current.id)) current.target.releasePointerCapture(current.id);
    if (dragging.value && save) {
      preferred.value = { ...position.value };
      persist();
    } else if (!save) { preferred.value = current.prior; suppressClick = false; }
    dragging.value = false; schedule();
  }
  function end(event: PointerEvent) { if (gesture?.id === event.pointerId) finish(event.type !== 'pointercancel'); }
  function click(event: MouseEvent, open: () => void) {
    if (suppressClick) { event.preventDefault(); event.stopPropagation(); suppressClick = false; return; }
    open();
  }
  function keyboard(event: KeyboardEvent) {
    if (!event.altKey || !['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(event.key)) return;
    event.preventDefault(); event.stopPropagation();
    const step = event.shiftKey ? 40 : 10;
    preferred.value = placeNoteLauncher(viewport.value, size.value, [], {
      x: position.value.x + (event.key === 'ArrowLeft' ? -step : event.key === 'ArrowRight' ? step : 0),
      y: position.value.y + (event.key === 'ArrowUp' ? -step : event.key === 'ArrowDown' ? step : 0),
    });
    persist();
  }
  watch(owner, () => {
    finish(false); preferred.value = null;
    if (!owner()) return;
    try {
      const point = JSON.parse(localStorage.getItem(key()) ?? 'null');
      if (point && Number.isFinite(point.x) && Number.isFinite(point.y)) preferred.value = point;
    } catch { /* Use the default edge position. */ }
    schedule();
  }, { immediate: true });
  watch(visible, () => { finish(); schedule(); }, { flush: 'post' });
  onMounted(() => {
    resize = new ResizeObserver(schedule);
    const content = document.querySelector('.application-content');
    if (content) resize.observe(content);
    observer = new MutationObserver(records => {
      if (records.some(noteLauncherLayoutMutation)) schedule();
    });
    observer.observe(content ?? document.body, { subtree: true, childList: true, characterData: true, attributes: true, attributeFilter: ['class', 'style', 'hidden', 'open'] });
    window.addEventListener('resize', schedule);
    window.visualViewport?.addEventListener('resize', schedule);
    window.visualViewport?.addEventListener('scroll', schedule);
    document.addEventListener('scroll', schedule, true);
    window.addEventListener('pointermove', move, { passive: false });
    window.addEventListener('pointerup', end);
    window.addEventListener('pointercancel', end);
    schedule();
  });
  onBeforeUnmount(() => {
    finish(); cancelAnimationFrame(frame); observer?.disconnect(); resize?.disconnect();
    window.removeEventListener('resize', schedule);
    window.visualViewport?.removeEventListener('resize', schedule);
    window.visualViewport?.removeEventListener('scroll', schedule);
    document.removeEventListener('scroll', schedule, true);
    window.removeEventListener('pointermove', move);
    window.removeEventListener('pointerup', end);
    window.removeEventListener('pointercancel', end);
  });
  return { style, dragging, keyboardOpen, begin, click, keyboard };
}
