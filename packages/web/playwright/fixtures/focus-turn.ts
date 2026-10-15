import { createApp } from 'vue';
import App from '../../src/App.vue';
import 'katex/dist/katex.min.css';
import '../../src/style.css';

// Instrument the production App; all interaction is through its public UI.
const app = createApp(App).mount('#app');
function component(node: any, name: string): any {
  if (!node) return null;
  if (node.component?.type.__name === name) return node.component.exposed;
  return component(node.component?.subTree, name) ?? (Array.isArray(node.children) ? node.children.map((n: any) => component(n, name)).find(Boolean) : null);
}
const reader = () => component(app.$.subTree, 'ReaderPane');
const longTasks: { start: number; duration: number }[] = [];
new PerformanceObserver(list => longTasks.push(...list.getEntries().map(e => ({ start: e.startTime, duration: e.duration })))).observe({ type: 'longtask', buffered: true });
type Sample = { name: string; inputType: string; input: number; received: number; firstChange: number | null; end: number | null; before: any; after?: any; frames: { at: number; transform: string[] }[] };
const samples: Sample[] = [];
let armed: string | null = null, active: Sample | null = null;
function observeInput(event: Event) {
  if (!armed || active) return;
  const target = event.target as Element;
  const key = event as KeyboardEvent;
  if (!(event.type === 'click' && target.closest('.focus-page-prev,.focus-page-next'))
    && !(event.type === 'keydown' && ['ArrowLeft', 'ArrowRight', 'PageUp', 'PageDown'].includes(key.key))
    && !(event.type === 'pointermove' && (event as PointerEvent).buttons !== 0 && target.closest('.focus-reader-surface,.pdf-focus-host'))) return;
  const sample: Sample = { name: armed, inputType: event.type, input: event.timeStamp, received: performance.now(), firstChange: null, end: null,
    before: reader().focusWork(), frames: [] };
  performance.mark(`${armed}-input`, { startTime: event.timeStamp });
  armed = null; active = sample; samples.push(sample);
  const frame = () => {
    if (active !== sample) return;
    const transforms = Array.from(document.querySelectorAll<HTMLElement>('.focus-screen:not([hidden])')).map(el => getComputedStyle(el).transform);
    const at = performance.now(); sample.frames.push({ at, transform: transforms });
    if (sample.firstChange === null && transforms.some(t => t !== 'none' && new DOMMatrixReadOnly(t).m41 !== 0)) sample.firstChange = at;
    if (active === sample && at - sample.received < 5000) requestAnimationFrame(frame);
  };
  if (!(window as any).focusTraceActive) requestAnimationFrame(frame);
}
for (const event of ['click', 'keydown', 'pointermove']) document.addEventListener(event, observeInput, true);
new MutationObserver(records => {
  if (!active || !records.some(r => r.attributeName === 'data-turn-phase')) return;
  const host = document.querySelector<HTMLElement>('.focus-reader-surface');
  if (host?.dataset.turnPhase !== 'idle') return;
  active.end = performance.now(); active.after = reader().focusWork();
  performance.mark(`${active.name}-end`); active = null;
}).observe(document.body, { attributes: true, subtree: true, attributeFilter: ['data-turn-phase'] });
Object.assign(window, { fp4: {
  available: () => !!reader(), snapshot: () => reader()?.focusSnapshot(), longTasks: () => longTasks,
  arm: (name: string) => { armed = name; }, samples: () => samples,
} });
