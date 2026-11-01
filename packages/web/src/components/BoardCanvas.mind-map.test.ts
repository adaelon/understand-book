// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { mount } from '@vue/test-utils';
import { nextTick } from 'vue';
import BoardCanvas from './BoardCanvas.vue';
import type { BoardItem } from '../generated/BoardItem';
import type { BoardMindMap } from '../generated/BoardMindMap';
import type { BoardView } from '../generated/BoardView';

const view = { x: 0, y: 0, zoom: 1, selected_item_id: null, selected_block_id: null, reading_item_id: null,
  playback: null, presentation_states: {}, selected_item_ids: [], folded_topic_ids: [], mind_map_outline: false } satisfies BoardView;
function topic(id: string, title: string, children: string[] = []): BoardItem {
  return { item_id: id, title, position: { x: 9000, y: 9000 }, size: { width: 100, height: 50 },
    readable_content: '', explanation_blocks: [], source_bindings: [], created_at: '2026-10-28',
    target: { kind: 'topic', children, style: { color: null, bold: false }, based_on: [] } };
}
const items = [topic('root', 'Rust 切片', ['read', 'write']), topic('read', '只读切片 &[T]', ['example']),
  topic('write', '可写切片 &mut [T]'), topic('example', '读取 a[1..3] 的 20 和 30')];
const map: BoardMindMap = { map_id: 'rust', root_item_id: 'root', layout: 'right', anchor: { x: 100, y: 180 }, branch_sides: {} };
const wrappers: ReturnType<typeof mount>[] = [];
function setup(extra: Partial<InstanceType<typeof BoardCanvas>['$props']> = {}) {
  const wrapper = mount(BoardCanvas, { props: { items, mindMaps: [map], view: { ...view }, ...extra }, attachTo: document.body });
  Object.assign(wrapper.element, { setPointerCapture: vi.fn(), hasPointerCapture: () => true, releasePointerCapture: vi.fn() });
  wrapper.findAll('[data-topic-id]').forEach(node => Object.assign(node.element, { setPointerCapture: vi.fn(), hasPointerCapture: () => true, releasePointerCapture: vi.fn() }));
  Object.defineProperties(wrapper.element, { clientWidth: { value: 1000, configurable: true }, clientHeight: { value: 700, configurable: true } });
  wrappers.push(wrapper); return wrapper;
}
function rect(wrapper: ReturnType<typeof setup>, id: string) {
  const element = wrapper.get(`[data-topic-id="${id}"]`).element as HTMLElement;
  return { x: parseFloat(element.style.left), y: parseFloat(element.style.top), width: parseFloat(element.style.width), height: parseFloat(element.style.height) };
}
async function begin(wrapper: ReturnType<typeof setup>, id: string, pointerId = 1, pointerType = 'mouse') {
  const node = rect(wrapper, id), x = node.x + node.width / 2, y = node.y + node.height / 2;
  await wrapper.get(`[data-topic-id="${id}"]`).trigger('pointerdown', { pointerId, pointerType, button: 0, clientX: x, clientY: y });
  return { x, y };
}
afterEach(() => { wrappers.splice(0).forEach(wrapper => wrapper.unmount()); });

describe('semantic mind map canvas interactions', () => {
  it('renders topics as measured tree nodes, derives parent edges, and keeps folded branches out of geometry', async () => {
    const wrapper = setup();
    expect(wrapper.findAll('.board-card')).toHaveLength(0);
    expect(wrapper.findAll('[data-topic-id]')).toHaveLength(4);
    expect(wrapper.findAll('[data-topic-edge]')).toHaveLength(3);
    expect(rect(wrapper, 'root').x).toBe(100);
    await wrapper.get('[data-topic-edge="read"]').trigger('click');
    expect(wrapper.emitted('activate-target')).toEqual([[{ kind: 'topic_edge', child_item_id: 'read' }]]);
    await wrapper.setProps({ foldedIds: ['read'] });
    expect(wrapper.find('[data-topic-id="example"]').exists()).toBe(false);
    expect(wrapper.get('[data-topic-id="read"] [aria-label="展开 1 个子主题"]').text()).toBe('1');
    expect(items[0].position).toEqual({ x: 9000, y: 9000 });
    wrapper.vm.focusTargets([{ kind: 'item', item_id: 'root' }]);
    const focus = wrapper.emitted('view')![0][0] as BoardView;
    expect(focus.x).toBeGreaterThan(-1000);
  });

  it('keeps the camera on an already visible topic and pans outside topics without changing zoom', () => {
    const wrapper = setup({ view: { ...view, zoom: .6 } });
    wrapper.vm.locate('root');
    expect(wrapper.emitted('view')).toBeUndefined();
    Object.defineProperty(wrapper.element, 'clientWidth', { value: 390, configurable: true });
    wrapper.vm.locate('example');
    expect((wrapper.emitted('view')?.at(-1)?.[0] as BoardView).zoom).toBe(.6);
  });

  it('restores a readable topic from a below-25% overview and brings its full bounds into view', () => {
    const wrapper = setup({ view: { ...view, x: 200, y: 160, zoom: .08 } });
    Object.defineProperties(wrapper.element, { clientWidth: { value: 390, configurable: true }, clientHeight: { value: 500, configurable: true } });
    const topic = rect(wrapper, 'example');
    wrapper.vm.locate('example');
    const camera = wrapper.emitted('view')![0][0] as BoardView;
    expect(camera.zoom).toBeGreaterThan(.85);
    expect(topic.x * camera.zoom + camera.x).toBeGreaterThanOrEqual(0);
    expect((topic.x + topic.width) * camera.zoom + camera.x).toBeLessThanOrEqual(390);
    expect(topic.y * camera.zoom + camera.y).toBeGreaterThanOrEqual(0);
    expect((topic.y + topic.height) * camera.zoom + camera.y).toBeLessThanOrEqual(500);
    expect(wrapper.emitted('user-control')).toBeUndefined();
  });

  it('makes the percentage an accessible control that restores 100% zoom', async () => {
    const wrapper = setup({ view: { ...view, zoom: .08 } });
    const reset = wrapper.get('button[aria-label="恢复100%缩放"]');
    expect(reset.text()).toBe('8%');
    await reset.trigger('click');
    expect((wrapper.emitted('view')![0][0] as BoardView).zoom).toBe(1);
    expect(wrapper.emitted('user-control')).toEqual([['zoom']]);
    expect(wrapper.find('[aria-label="缩小画板"]').exists()).toBe(true);
    expect(wrapper.find('[aria-label="放大画板"]').exists()).toBe(true);
  });

  it('does not move the camera while the retained canvas is hidden at zero size', async () => {
    const wrapper = setup({ view: { ...view, x: 1500, y: 900, zoom: .6 } });
    Object.defineProperties(wrapper.element, { clientWidth: { value: 0, configurable: true }, clientHeight: { value: 0, configurable: true } });
    wrapper.vm.fit();
    wrapper.vm.locate('example');
    wrapper.vm.focusTargets([{ kind: 'item', item_id: 'read' }]);
    expect(wrapper.emitted('view')).toBeUndefined();
    expect(wrapper.props('view')).toMatchObject({ x: 1500, y: 900, zoom: .6 });

    Object.defineProperties(wrapper.element, { clientWidth: { value: 1000, configurable: true }, clientHeight: { value: 700, configurable: true } });
    wrapper.vm.locate('example');
    const camera = wrapper.emitted('view')?.at(-1)?.[0] as BoardView;
    expect(camera.zoom).toBe(.6); expect(camera.x).toBeLessThan(1500); expect(camera.y).toBeLessThan(900);
  });

  it('gives horizontal semantic edges a measurable 44px control at low zoom', async () => {
    const wrapper = setup({ view: { ...view, zoom: .25 } });
    const edge = wrapper.get('[data-topic-edge="example"]');
    const coordinates = edge.get('.mind-map-edge-line').attributes('d')!.match(/-?\d+(?:\.\d+)?/g)!.map(Number);
    expect(new Set(coordinates.filter((_, index) => index % 2 === 1)).size).toBe(1);
    expect(edge.get('.mind-map-edge-target').attributes()).toMatchObject({ width: '176', height: '176', 'pointer-events': 'all' });
    await edge.trigger('keydown', { key: 'Enter' });
    expect(wrapper.emitted('activate-target')).toEqual([[{ kind: 'topic_edge', child_item_id: 'example' }]]);
  });

  it('preserves an in-progress title through rerenders and does not commit Chinese composition or Shift+Enter', async () => {
    const wrapper = setup({ editingId: 'read', draftTitle: '正在输入' });
    await nextTick();
    const input = wrapper.get('textarea');
    await input.trigger('compositionstart');
    await input.trigger('keydown', { key: 'Enter' });
    expect(wrapper.emitted('topic-commit')).toBeUndefined();
    await input.trigger('compositionend');
    await input.trigger('keydown', { key: 'Enter', isComposing: true });
    await input.trigger('keydown', { key: 'Enter', shiftKey: true });
    expect(wrapper.emitted('topic-commit')).toBeUndefined();
    await input.setValue('只读切片：可以读取');
    expect(wrapper.emitted('topic-draft')?.at(-1)).toEqual(['只读切片：可以读取']);
    const textarea = input.element;
    await wrapper.setProps({ draftTitle: '只读切片：可以读取', selectedIds: ['read'] });
    expect(wrapper.get('textarea').element).toBe(textarea);
    expect((textarea as HTMLTextAreaElement).value).toBe('只读切片：可以读取');
    await input.trigger('keydown', { key: 'Enter' });
    expect(wrapper.emitted('topic-commit')).toHaveLength(1);
    expect(wrapper.emitted('topic-keydown')).toBeUndefined();
    await input.trigger('keydown', { key: 'Escape' });
    expect(wrapper.emitted('topic-cancel')).toHaveLength(1);
  });

  it('moves the entire map with the root and commits only its anchor once on release', async () => {
    const wrapper = setup(), before = rect(wrapper, 'example'), start = await begin(wrapper, 'root');
    await wrapper.trigger('pointermove', { pointerId: 1, clientX: start.x + 70, clientY: start.y + 40 });
    expect(rect(wrapper, 'example').x).toBe(before.x + 70);
    expect(wrapper.emitted('topic-root-move')).toBeUndefined();
    await wrapper.trigger('pointerup', { pointerId: 1 });
    expect(wrapper.emitted('topic-root-move')).toEqual([[{ mapId: 'rust', anchor: { x: 170, y: 220 } }]]);
    expect(wrapper.emitted('move')).toBeUndefined();
  });

  it('moves a complete branch under another topic with an explicit drop cue', async () => {
    const wrapper = setup(), target = rect(wrapper, 'write'), child = rect(wrapper, 'example');
    await begin(wrapper, 'read');
    await wrapper.trigger('pointermove', { pointerId: 1, clientX: target.x + target.width / 2, clientY: target.y + target.height / 2 });
    expect(wrapper.get('[data-topic-id="write"]').classes()).toContain('drop-child');
    expect(rect(wrapper, 'example').y).not.toBe(child.y);
    await wrapper.trigger('pointerup', { pointerId: 1 });
    expect(wrapper.emitted('topic-drop')).toEqual([[{ itemId: 'read', parentId: 'write', index: 0 }]]);
    expect(items[0].target.kind === 'topic' && items[0].target.children).toEqual(['read', 'write']);
  });

  it('cancels blank and cycle drops and computes same-parent insertion after removal', async () => {
    const wrapper = setup(), child = rect(wrapper, 'example');
    await begin(wrapper, 'read');
    await wrapper.trigger('pointermove', { pointerId: 1, clientX: child.x + child.width / 2, clientY: child.y + child.height / 2 });
    await wrapper.trigger('pointerup', { pointerId: 1 });
    expect(wrapper.emitted('topic-drop')).toBeUndefined();
    const original = rect(wrapper, 'read');
    await begin(wrapper, 'read');
    await wrapper.trigger('pointermove', { pointerId: 1, clientX: 2000, clientY: 2000 });
    await wrapper.trigger('pointerup', { pointerId: 1 });
    expect(rect(wrapper, 'read')).toEqual(original);
    expect(wrapper.emitted('topic-drop')).toBeUndefined();
    const target = rect(wrapper, 'write');
    await begin(wrapper, 'read');
    await wrapper.trigger('pointermove', { pointerId: 1, clientX: target.x + target.width / 2, clientY: target.y + target.height - 2 });
    expect(wrapper.get('[data-topic-id="write"]').classes()).toContain('drop-after');
    await wrapper.trigger('pointerup', { pointerId: 1 });
    expect(wrapper.emitted('topic-drop')).toEqual([[{ itemId: 'read', parentId: 'root', index: 1 }]]);
  });

  it('previews all selected movable branches and leaves a selected root anchored during a branch drag', async () => {
    const wrapper = setup({ selectedIds: ['root', 'read', 'write'] });
    const root = rect(wrapper, 'root'), second = rect(wrapper, 'write'), child = rect(wrapper, 'example');
    const start = await begin(wrapper, 'read');
    await wrapper.trigger('pointermove', { pointerId: 1, clientX: start.x + 80, clientY: start.y + 60 });
    expect(rect(wrapper, 'root')).toEqual(root);
    expect(rect(wrapper, 'write')).toMatchObject({ x: second.x + 80, y: second.y + 60 });
    expect(rect(wrapper, 'example')).toMatchObject({ x: child.x + 80, y: child.y + 60 });
    await wrapper.trigger('pointercancel', { pointerId: 1 });
    expect(rect(wrapper, 'write')).toEqual(second);
    expect(wrapper.emitted('topic-drop')).toBeUndefined();
  });

  it('starts a pinch without committing the interrupted branch drag', async () => {
    const wrapper = setup(), start = await begin(wrapper, 'read', 1, 'touch');
    await wrapper.trigger('pointermove', { pointerId: 1, pointerType: 'touch', clientX: start.x + 30, clientY: start.y + 20 });
    await wrapper.trigger('pointerdown', { pointerId: 2, pointerType: 'touch', button: 0, clientX: start.x + 130, clientY: start.y + 20 });
    await wrapper.trigger('pointermove', { pointerId: 2, pointerType: 'touch', clientX: start.x + 180, clientY: start.y + 20 });
    const camera = wrapper.emitted('view')?.at(-1)?.[0] as BoardView;
    expect(camera.zoom).toBeGreaterThan(1);
    await wrapper.trigger('pointerup', { pointerId: 1, pointerType: 'touch' });
    await wrapper.trigger('pointerup', { pointerId: 2, pointerType: 'touch' });
    expect(wrapper.emitted('topic-drop')).toBeUndefined();
    expect(wrapper.emitted('topic-root-move')).toBeUndefined();
    expect(wrapper.find('.mind-map-topic.dragging').exists()).toBe(false);
  });

  it('supports additive topic selection and a world-coordinate selection box', async () => {
    const wrapper = setup();
    await wrapper.get('[data-topic-id="read"]').trigger('pointerdown', { pointerId: 1, button: 0, shiftKey: true, clientX: 400, clientY: 200 });
    await wrapper.trigger('pointerup', { pointerId: 1 });
    expect(wrapper.emitted('topic-select')).toEqual([['read', true]]);
    const root = rect(wrapper, 'root');
    await wrapper.trigger('pointerdown', { pointerId: 2, button: 0, shiftKey: true, clientX: root.x - 5, clientY: root.y - 5 });
    await wrapper.trigger('pointermove', { pointerId: 2, clientX: root.x + root.width + 5, clientY: root.y + root.height + 5 });
    await wrapper.trigger('pointerup', { pointerId: 2 });
    expect(wrapper.emitted('topic-box-select')).toEqual([[['root']]]);
  });
});
