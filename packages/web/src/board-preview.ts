import { createApp, defineComponent, h, nextTick, onMounted, ref } from 'vue';
import BoardCanvas from './components/BoardCanvas.vue';
import type { BoardDocument } from './generated/BoardDocument';
import { candidatePreviewVisuals } from './board-canvas-preview';
import { layoutMindMaps, MIND_MAP_FONT_FAMILY } from './mind-map-layout';
import { defaultBoardView } from './board-workspace';
import './style.css';

declare global {
  interface Window {
    __boardPreviewDocument: BoardDocument;
    __boardPreviewCandidateItemIds: string[];
    __boardPreviewReady?: boolean;
    __boardPreviewInspect?: () => unknown;
  }
}

const board = window.__boardPreviewDocument;
const candidateVisuals = candidatePreviewVisuals(board.items, window.__boardPreviewCandidateItemIds);
const Candidate = defineComponent({
  setup() {
    const view = ref(defaultBoardView());
    const canvas = ref<InstanceType<typeof BoardCanvas>>();
    onMounted(async () => {
      await nextTick();
      const labels = board.items.filter(item => item.target.kind === 'topic').map(item => item.title).join('');
      if (labels) await document.fonts.load(`500 16px ${MIND_MAP_FONT_FAMILY}`, labels);
      await document.fonts.ready;
      await nextTick();
      await new Promise<void>(resolve => requestAnimationFrame(() => resolve()));
      if (candidateVisuals.length) canvas.value?.focusTargets(candidateVisuals.map(item => ({ kind: 'item', item_id: item.item_id })));
      else canvas.value?.fit();
      await nextTick();
      await new Promise<void>(resolve => requestAnimationFrame(() => resolve()));
      window.__boardPreviewReady = true;
    });
    return () => h('main', { class: 'board-preview', style: 'display:grid;grid-template-rows:auto minmax(0,1fr);height:100vh' }, [
      h('header', { style: 'padding:12px 20px;border-bottom:1px solid var(--hairline);font-size:15px;font-weight:600;background:var(--reader-card)' }, board.title),
      h(BoardCanvas, { ref: canvas, items: board.items, mindMaps: board.mind_maps ?? [], links: board.links, view: view.value, readonly: true,
        onView: value => { view.value = value; } }),
    ]);
  },
});
createApp(Candidate).mount('#app');

// Read actual rendered geometry. The model also receives the screenshot; these
// findings point to concrete clipping/overlap locations rather than judging prose.
window.__boardPreviewInspect = () => {
  const issues: { kind: string; message: string }[] = [];
  // Previously retained content may have been deliberately rearranged by the
  // reader. Judge this candidate's additions and updates without rewriting that old layout.
  const visuals = candidateVisuals;
  const layout = layoutMindMaps(board.items, board.mind_maps ?? []);
  const native = visuals.map(item => {
    const element = document.querySelector<HTMLElement>(`[data-item-id="${CSS.escape(item.item_id)}"]`);
    const rect = element?.getBoundingClientRect();
    if (!rect?.width || !rect.height) issues.push({ kind: 'board_item_missing', message: `Visual ${item.item_id} has no rendered geometry` });
    const text = element?.querySelector<SVGGraphicsElement>('[data-board-drawing-text], [data-board-topic-text]');
    if (text) {
      const box = text.getBBox();
      const size = layout.byId.get(item.item_id) ?? item.size;
      if (box.x < -1 || box.y < -1 || box.x + box.width > size.width + 1 || box.y + box.height > size.height + 1) {
        issues.push({ kind: 'board_text_clipped', message: `Visual ${item.item_id} clips its text; inspect its typography and label` });
      }
    }
    const rich = element?.querySelector<HTMLElement>('[data-board-rich-text]');
    if (rich && rect) {
      const content = rich.getBoundingClientRect();
      const math = [...rich.querySelectorAll<HTMLElement>('.katex-html')].map(formula => formula.getBoundingClientRect());
      if (rich.dataset.boardRichOverflow === 'true' || [content, ...math].some(box => box.left < rect.left - 1 || box.top < rect.top - 1 || box.right > rect.right + 1 || box.bottom > rect.bottom + 1)) {
        issues.push({ kind: 'board_text_clipped', message: `Drawing ${item.item_id} clips its formatted text or equation; increase its size or shorten the visual label` });
      }
    }
    const path = element?.querySelector<SVGGraphicsElement>('.board-native-path');
    if (path) {
      const box = path.getBBox();
      if (box.x < -1 || box.y < -1 || box.x + box.width > item.size.width + 1 || box.y + box.height > item.size.height + 1) {
        issues.push({ kind: 'board_path_outside_bounds', message: `Path ${item.item_id} extends outside its declared size; correct its local coordinates or size so navigation includes the full path` });
      }
    }
    return { item_id: item.item_id, x: rect?.x, y: rect?.y, width: rect?.width, height: rect?.height, text: element?.textContent };
  });
  const rendered = new Map(board.items.map(item => [item.item_id, document.querySelector<HTMLElement>(`[data-item-id="${CSS.escape(item.item_id)}"]`)?.getBoundingClientRect()]));
  for (const drawing of visuals) {
    if (drawing.target.kind === 'drawing' && drawing.target.graphic.kind === 'path') continue;
    for (const other of board.items) {
      if (other.item_id === drawing.item_id || (other.target.kind === 'drawing' &&
        other.target.graphic.kind === 'path') || (visuals.includes(other) && other.item_id < drawing.item_id)) continue;
      const a = rendered.get(drawing.item_id), b = rendered.get(other.item_id);
      if (a && b && Math.min(a.right, b.right) - Math.max(a.left, b.left) > 2 &&
        Math.min(a.bottom, b.bottom) - Math.max(a.top, b.top) > 2) {
        issues.push({ kind: 'board_overlap', message: `Visual ${drawing.item_id} overlaps ${other.item_id}; inspect the screenshot and move the map anchor or free card` });
      }
    }
  }
  return { ready: window.__boardPreviewReady === true, board_id: board.board_id, revision: board.revision, native, issues };
};
