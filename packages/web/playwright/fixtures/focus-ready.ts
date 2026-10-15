import { createApp } from 'vue';
import App from '../../src/App.vue';
import 'katex/dist/katex.min.css';
import '../../src/style.css';

// Production App, API adapters and component lifecycle. This driver only calls
// ReaderPane's FP3 preparation API; the public input controls are the FP4 slice.
const app = createApp(App).mount('#app');
function component(node: any, name: string): any {
  if (!node) return null;
  if (node.component?.type.__name === name) return node.component.exposed;
  return component(node.component?.subTree, name) ?? (Array.isArray(node.children) ? node.children.map((n: any) => component(n, name)).find(Boolean) : null);
}
const reader = () => component(app.$.subTree, 'ReaderPane');
const longTasks: { start: number; duration: number }[] = [];
new PerformanceObserver(list => longTasks.push(...list.getEntries().map(e => ({ start: e.startTime, duration: e.duration })))).observe({ type: 'longtask', buffered: true });
Object.assign(window, { fp3: {
  available: () => !!reader(),
  activate: (cursor?: any) => reader().prepareFocus(cursor),
  move: (direction: number) => {
    const before = reader().focusSnapshot();
    const at = performance.now(); const ready = reader().focusMove(direction); const after = reader().focusSnapshot();
    return { ready, at, elapsedMs: performance.now() - at, before, after };
  },
  snapshot: () => reader()?.focusSnapshot(),
  pause: (value: boolean) => reader().focusPause(value),
  longTasks: () => longTasks,
} });
