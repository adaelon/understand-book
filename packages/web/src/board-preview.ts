import { createApp, defineComponent, h, nextTick, onMounted, ref } from 'vue';
import BoardCanvas from './components/BoardCanvas.vue';
import type { BoardDocument } from './generated/BoardDocument';
import { candidatePreviewDrawings } from './board-canvas-preview';
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
const candidateDrawings = candidatePreviewDrawings(board.items, window.__boardPreviewCandidateItemIds);
const Candidate = defineComponent({
  setup() {
    const view = ref(defaultBoardView());
    const canvas = ref<InstanceType<typeof BoardCanvas>>();
    onMounted(async () => {
      await nextTick();
      if (candidateDrawings.length) canvas.value?.focusTargets(candidateDrawings.map(item => ({ kind: 'item', item_id: item.item_id })));
      else canvas.value?.fit();
      await nextTick();
      window.__boardPreviewReady = true;
    });
    return () => h('main', { class: 'board-preview', style: 'display:grid;grid-template-rows:auto minmax(0,1fr);height:100vh' }, [
      h('header', { style: 'padding:12px 20px;border-bottom:1px solid var(--hairline);font-size:15px;font-weight:600;background:var(--reader-card)' }, board.title),
      h(BoardCanvas, { ref: canvas, items: board.items, links: board.links, view: view.value, readonly: true,
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
  const drawings = candidateDrawings;
  const native = drawings.map(item => {
    const element = document.querySelector<HTMLElement>(`[data-item-id="${CSS.escape(item.item_id)}"]`);
    const rect = element?.getBoundingClientRect();
    if (!rect?.width || !rect.height) issues.push({ kind: 'board_item_missing', message: `Drawing ${item.item_id} has no rendered geometry` });
    const text = element?.querySelector<SVGGraphicsElement>('[data-board-drawing-text]');
    if (text) {
      const box = text.getBBox();
      if (box.x < -1 || box.y < -1 || box.x + box.width > item.size.width + 1 || box.y + box.height > item.size.height + 1) {
        issues.push({ kind: 'board_text_clipped', message: `Drawing ${item.item_id} clips its text; increase its size or shorten the visual label` });
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
  for (const drawing of drawings) {
    if (drawing.target.kind === 'drawing' && drawing.target.graphic.kind === 'path') continue;
    for (const other of board.items) {
      if (other.item_id === drawing.item_id || (other.target.kind === 'drawing' &&
        (other.target.graphic.kind === 'path' || drawings.includes(other) && other.item_id < drawing.item_id))) continue;
      const a = drawing.position, b = other.position;
      if (Math.min(a.x + drawing.size.width, b.x + other.size.width) - Math.max(a.x, b.x) > 2 &&
        Math.min(a.y + drawing.size.height, b.y + other.size.height) - Math.max(a.y, b.y) > 2) {
        issues.push({ kind: 'board_overlap', message: `Drawing ${drawing.item_id} overlaps ${other.item_id}; inspect the screenshot and adjust their positions` });
      }
    }
  }
  return { ready: window.__boardPreviewReady === true, board_id: board.board_id, revision: board.revision, native, issues };
};
